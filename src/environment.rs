use crate::info::{Pokemon, validate_move_count};
use crate::{
    Action, ActionError, Battle, BattleEvent, BattleObservation, BattleSide, BattleState,
    OpponentObservation, PokemonState, TeamPreviewObservation, TeamState, calculate_reward,
};
use rand::{SeedableRng, rngs::SmallRng};

/// Environment safety limit, not a battle outcome or tournament timer.
/// Counts successful attack and voluntary-switch turns, excluding team selection
/// and forced replacements. A terminal result on the last turn takes precedence.
pub const MAX_EPISODE_TURNS: usize = 1_000;

/// An observation returned to the player during an episode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    TeamPreview(TeamPreviewObservation),
    Battle(BattleObservation),
}

/// A minimal episode loop around a battle-state transition function.
pub struct Environment<F> {
    preview: TeamPreviewObservation,
    opponent: TeamState,
    battle: Option<Battle>,
    // Used during preview; the active battle owns and advances the stream after selection.
    rng: SmallRng,
    player_revealed: [bool; 6],
    opponent_revealed: [bool; 6],
    elapsed_turns: usize,
    transition: F,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepOutcome {
    /// Observation from `BattleSide::Player`'s perspective.
    pub observation: Observation,
    /// Zero-sum reward from `BattleSide::Player`'s perspective.
    pub reward: f32,
    pub terminated: bool,
    /// The turn limit was reached without a terminal battle result.
    /// Stop stepping on either `terminated` or `truncated`, then reset.
    pub truncated: bool,
    /// Only this step's events, in resolution order; empty for team selection.
    pub events: Vec<BattleEvent>,
}

impl StepOutcome {
    /// Returns this step's zero-sum reward from the requested Trainer's perspective.
    pub fn reward_for(&self, side: BattleSide) -> f32 {
        match side {
            BattleSide::Player => self.reward,
            BattleSide::Opponent => -self.reward,
        }
    }
}

impl Environment<()> {
    /// Creates an environment using the core battle logic and one to four moves per Pokemon.
    /// Roster slots are preserved; the first selected slot is each side's lead.
    /// `reset` restores the supplied HP and returns to team preview, continuing the random stream.
    ///
    /// ```
    /// use omniring::{Action, ActionError, Environment};
    /// use omniring::info::Pokemon;
    ///
    /// # fn episode(player: [Pokemon; 6], opponent: [Pokemon; 6]) -> Result<(), ActionError> {
    /// let mut env = Environment::from_rosters(player, opponent, [0, 1, 2])?;
    /// let preview = env.reset();
    /// // During preview, the opponent action is ignored: its team was selected above.
    /// env.step(Action::SelectTeam([0, 1, 2]), Action::Move(0))?;
    /// let turn = env.step(Action::Move(0), Action::Move(0))?;
    /// // Use turn.observation and turn.reward for training.
    /// // End the episode when turn.terminated || turn.truncated.
    /// # Ok(())
    /// # }
    /// ```
    #[expect(
        clippy::type_complexity,
        reason = "Reuse the generic environment without boxing its resolver"
    )]
    pub fn from_rosters(
        player: [Pokemon; 6],
        opponent: [Pokemon; 6],
        opponent_selection: [usize; 3],
    ) -> Result<
        Environment<
            impl FnMut(
                &mut BattleState,
                Action,
                Action,
                &mut SmallRng,
            ) -> Result<Vec<BattleEvent>, ActionError>,
        >,
        ActionError,
    > {
        Self::from_rosters_with_seed(player, opponent, opponent_selection, rand::random())
    }

    /// Creates an environment with a reproducible battle random stream.
    /// Use `reset_with_seed` to replay an episode; ordinary `reset` continues the stream.
    #[expect(
        clippy::type_complexity,
        reason = "Reuse the generic environment without boxing its resolver"
    )]
    pub fn from_rosters_with_seed(
        player: [Pokemon; 6],
        opponent: [Pokemon; 6],
        opponent_selection: [usize; 3],
        seed: u64,
    ) -> Result<
        Environment<
            impl FnMut(
                &mut BattleState,
                Action,
                Action,
                &mut SmallRng,
            ) -> Result<Vec<BattleEvent>, ActionError>,
        >,
        ActionError,
    > {
        let preview = TeamPreviewObservation {
            player: preview_roster(&player)?,
            opponent: preview_roster(&opponent)?,
        };

        Environment::new_with_seed(
            preview,
            opponent_selection,
            seed,
            move |state, action, opponent_action, rng| {
                Battle::resolve_turn(state, &player, &opponent, action, opponent_action, rng)
            },
        )
    }
}

fn preview_roster(roster: &[Pokemon; 6]) -> Result<[PokemonState; 6], ActionError> {
    let [a, b, c, d, e, f] = roster.each_ref().map(|pokemon| {
        validate_move_count(pokemon.moves.len()).map_err(ActionError::Battle)?;

        PokemonState::new(
            u32::from(pokemon.current_hp),
            u32::from(pokemon.stats.hp),
            std::array::from_fn(|slot| slot < pokemon.moves.len()),
        )
        .map_err(ActionError::InvalidState)
    });
    Ok([a?, b?, c?, d?, e?, f?])
}

impl<F> Environment<F>
where
    F: FnMut(
        &mut BattleState,
        Action,
        Action,
        &mut SmallRng,
    ) -> Result<Vec<BattleEvent>, ActionError>,
{
    /// Creates an environment with a randomly chosen seed and a custom transition.
    /// The transition receives the battle's RNG as its fourth argument.
    pub fn new(
        preview: TeamPreviewObservation,
        opponent_selection: [usize; 3],
        transition: F,
    ) -> Result<Self, ActionError> {
        Self::new_with_seed(preview, opponent_selection, rand::random(), transition)
    }

    /// Creates a seeded environment. Custom transitions must use the supplied RNG for
    /// random decisions; state captured by the callback is not reset or rolled back.
    /// Return attack events in resolution order, or `Ok(Vec::new())` for none.
    /// The battle adds move selections, action-driven switches, and completion; custom HP changes must
    /// include their own damage/faint events. Events are trusted, not inferred from HP.
    pub fn new_with_seed(
        preview: TeamPreviewObservation,
        opponent_selection: [usize; 3],
        seed: u64,
        transition: F,
    ) -> Result<Self, ActionError> {
        preview.validate_player_action(Action::SelectTeam(opponent_selection))?;

        let opponent = selected_team(preview.opponent.clone(), opponent_selection)?;

        Ok(Self {
            preview,
            opponent,
            battle: None,
            rng: SmallRng::seed_from_u64(seed),
            player_revealed: [false; 6],
            opponent_revealed: [false; 6],
            elapsed_turns: 0,
            transition,
        })
    }

    /// Restores team preview, initial HP, and the turn budget while continuing the random stream.
    pub fn reset(&mut self) -> Observation {
        if let Some(battle) = self.battle.take() {
            self.rng = battle.rng;
        }

        self.player_revealed = [false; 6];
        self.opponent_revealed = [false; 6];
        self.elapsed_turns = 0;

        Observation::TeamPreview(self.preview.clone())
    }

    /// Restores team preview and restarts the random stream from `seed`.
    /// The same rosters, seed, and actions reproduce an episode with the same library versions and target platform.
    pub fn reset_with_seed(&mut self, seed: u64) -> Observation {
        let preview = self.reset();
        self.rng = SmallRng::seed_from_u64(seed);
        preview
    }

    /// Returns an independent snapshot with `player` referring to the requested Trainer.
    /// Only opposing slots that have entered battle are revealed as selected.
    /// This borrows the canonical state and does not advance the battle or its RNG.
    pub fn observation(&self, side: BattleSide) -> Result<Observation, ActionError> {
        let Some(battle) = &self.battle else {
            let (player, opponent) = match side {
                BattleSide::Player => (&self.preview.player, &self.preview.opponent),
                BattleSide::Opponent => (&self.preview.opponent, &self.preview.player),
            };

            return Ok(Observation::TeamPreview(TeamPreviewObservation {
                player: player.clone(),
                opponent: opponent.clone(),
            }));
        };
        let state = battle.state();
        let (player, opponent, revealed) = match side {
            BattleSide::Player => (&state.player, &state.opponent, self.opponent_revealed),
            BattleSide::Opponent => (&state.opponent, &state.player, self.player_revealed),
        };

        observation(player, opponent, revealed, state.terminated).map(Observation::Battle)
    }

    /// Advances an episode, capped at `MAX_EPISODE_TURNS` successful battle turns.
    /// After truncation, returns `ActionError::EpisodeTruncated` until reset.
    pub fn step(
        &mut self,
        action: Action,
        opponent_action: Action,
    ) -> Result<StepOutcome, ActionError> {
        if self.battle.is_none() {
            let Action::SelectTeam(selection) = action else {
                return Err(ActionError::WrongPhase);
            };
            self.preview.validate_player_action(action)?;

            let player = selected_team(self.preview.player.clone(), selection)?;
            let opponent = self.opponent.clone();

            self.player_revealed[player.slot_active().unwrap()] = true;
            self.opponent_revealed[opponent.slot_active().unwrap()] = true;

            let battle = self.battle.insert(Battle::with_rng(
                BattleState {
                    player,
                    opponent,
                    terminated: false,
                },
                self.rng.clone(),
            ));

            return Ok(StepOutcome {
                observation: Observation::Battle(observation(
                    &battle.state().player,
                    &battle.state().opponent,
                    self.opponent_revealed,
                    false,
                )?),
                reward: 0.0,
                terminated: false,
                truncated: false,
                events: Vec::new(),
            });
        }

        let previous = self.battle.as_ref().unwrap().clone();

        if self.elapsed_turns >= MAX_EPISODE_TURNS && !previous.state().terminated {
            return Err(ActionError::EpisodeTruncated);
        }

        let elapsed_turns = self.elapsed_turns
            + usize::from(
                previous.state().player.slot_active().is_some()
                    && previous.state().opponent.slot_active().is_some(),
            );
        let player_revealed = self.player_revealed;
        let opponent_revealed = self.opponent_revealed;
        let outcome = (|| {
            let battle = self.battle.as_mut().unwrap();
            battle.play_turn(action, opponent_action, &mut self.transition)?;

            let state = battle.state();

            for (team, action, revealed) in [
                (&state.player, action, &mut self.player_revealed),
                (
                    &state.opponent,
                    opponent_action,
                    &mut self.opponent_revealed,
                ),
            ] {
                // A successful switch reveals its slot even if the incoming Pokemon fainted.
                if let Action::Switch(slot) = action {
                    revealed[slot] = true;
                }

                if let Some(active) = team.slot_active() {
                    revealed[active] = true;
                }

                // A custom transition must leave both perspectives valid.
                if revealed
                    .iter()
                    .zip(team.selected())
                    .any(|(&seen, &selected)| seen && !selected)
                {
                    return Err(ActionError::InvalidTeamSelection);
                }
            }

            Ok(StepOutcome {
                observation: Observation::Battle(observation(
                    &state.player,
                    &state.opponent,
                    self.opponent_revealed,
                    state.terminated,
                )?),
                reward: calculate_reward(previous.state(), state),
                terminated: state.terminated,
                truncated: !state.terminated && elapsed_turns >= MAX_EPISODE_TURNS,
                events: battle.events().to_vec(),
            })
        })();

        if outcome.is_err() {
            self.battle = Some(previous);
            self.player_revealed = player_revealed;
            self.opponent_revealed = opponent_revealed;
        } else {
            self.elapsed_turns = elapsed_turns;
        }

        outcome
    }
}

fn selected_team(
    roster: [PokemonState; 6],
    selection: [usize; 3],
) -> Result<TeamState, ActionError> {
    let mut selected = [false; 6];

    for slot in selection {
        selected[slot] = true;
    }

    TeamState::new(roster, selected, Some(selection[0]))
        .map_err(|_| ActionError::InvalidTeamSelection)
}

fn observation(
    player: &TeamState,
    opponent: &TeamState,
    opponent_revealed: [bool; 6],
    terminated: bool,
) -> Result<BattleObservation, ActionError> {
    Ok(BattleObservation {
        player: player.clone(),
        opponent: OpponentObservation::new(opponent, opponent_revealed)
            .map_err(|_| ActionError::InvalidTeamSelection)?,
        terminated,
    })
}

#[cfg(test)]
mod tests {
    use rand::RngExt;
    use std::cell::Cell;

    use super::{Environment, MAX_EPISODE_TURNS, Observation};
    use crate::{
        Action, ActionError, BattleEvent, BattleSide, BattleState, PokemonState,
        TeamPreviewObservation, TeamState,
    };

    #[test]
    fn custom_events_are_forwarded_and_failed_steps_preserve_previous_events() {
        let miss = BattleEvent::Miss {
            side: BattleSide::Player,
            slot: 0,
            move_slot: 0,
        };
        let fail = Cell::new(false);

        let mut environment = Environment::new_with_seed(
            TeamPreviewObservation {
                player: roster(100),
                opponent: roster(100),
            },
            [0, 1, 2],
            46,
            |state, _, _, rng| {
                if fail.get() {
                    state.player.damage_active(50).unwrap();
                    let _ = rng.random::<u64>();
                    return Err(ActionError::InvalidSwitch);
                }
                Ok(vec![miss.clone()])
            },
        )
        .unwrap();
        environment
            .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
            .unwrap();

        let first = environment.step(Action::Move(0), Action::Move(0)).unwrap();
        assert_eq!(
            first.events,
            vec![
                BattleEvent::MoveSelected {
                    side: BattleSide::Player,
                    slot: 0,
                    move_slot: 0
                },
                BattleEvent::MoveSelected {
                    side: BattleSide::Opponent,
                    slot: 0,
                    move_slot: 0
                },
                miss.clone(),
            ]
        );

        let before = environment.battle.clone();

        fail.set(true);
        assert_eq!(
            environment.step(Action::Move(0), Action::Switch(1)),
            Err(ActionError::InvalidSwitch)
        );
        assert_eq!(environment.battle, before);

        fail.set(false);
        assert_eq!(
            environment.step(Action::Move(0), Action::Move(0)).unwrap(),
            first
        );
    }

    #[test]
    fn observation_snapshots_do_not_mutate_or_alias_battle_state() {
        let original = state([100; 3], false);
        let before = original.clone();
        let revealed = [true, false, false, false, false, false];
        let expected =
            super::observation(&original.player, &original.opponent, revealed, false).unwrap();

        let mut snapshot = expected.clone();
        snapshot.player.damage_active(50).unwrap();
        snapshot.player.switch_to(1).unwrap();
        snapshot.terminated = true;

        assert_ne!(snapshot, expected);
        assert_eq!(
            super::observation(&original.player, &original.opponent, revealed, false).unwrap(),
            expected
        );
        assert_eq!(original, before);
    }

    #[test]
    fn runs_a_hidden_information_episode_from_preview_to_reset() {
        let preview = TeamPreviewObservation {
            player: roster(100),
            opponent: roster(100),
        };

        assert!(Environment::new(preview.clone(), [0, 0, 1], |_, _, _, _| Ok(Vec::new())).is_err());

        let terminal = state([0; 3], true);
        let transitions = Cell::new(0);
        let mut environment =
            Environment::new(preview.clone(), [0, 1, 2], |state, player, opponent, _| {
                assert_eq!((player, opponent), (Action::Move(0), Action::Move(0)));
                transitions.set(transitions.get() + 1);
                *state = terminal.clone();
                Ok(Vec::new())
            })
            .unwrap();

        assert_eq!(
            environment.reset(),
            Observation::TeamPreview(preview.clone())
        );
        assert_eq!(
            environment.step(Action::Move(0), Action::Move(0)),
            Err(ActionError::WrongPhase)
        );

        let selected = environment
            .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
            .unwrap();

        assert_eq!(selected.reward, 0.0);
        assert!(!selected.terminated);

        assert!(matches!(
            selected.observation,
            Observation::Battle(observation)
                if observation.opponent.selection_revealed()
                    == &[true, false, false, false, false, false]
        ));
        assert_eq!(transitions.get(), 0);

        assert_eq!(
            environment.step(Action::Move(0), Action::Move(4)),
            Err(ActionError::UnavailableMove)
        );
        assert_eq!(transitions.get(), 0);

        let outcome = environment.step(Action::Move(0), Action::Move(0)).unwrap();

        assert!((outcome.reward - 1.4).abs() < 1e-6);
        assert!(outcome.terminated);
        assert!(matches!(outcome.observation, Observation::Battle(_)));
        assert_eq!(transitions.get(), 1);
        assert_eq!(
            environment.step(Action::Move(0), Action::Move(0)),
            Err(ActionError::BattleTerminated)
        );
        assert_eq!(transitions.get(), 1);
        assert_eq!(
            environment.reset(),
            Observation::TeamPreview(preview.clone())
        );
    }

    #[test]
    fn rolls_back_failed_transitions_and_observations() {
        let preview = TeamPreviewObservation {
            player: roster(100),
            opponent: roster(100),
        };
        let transitions = Cell::new(0);
        let mut environment =
            Environment::new_with_seed(preview, [0, 1, 2], 46, |state, _, _, rng| {
                let turn = transitions.get();
                transitions.set(turn + 1);
                let _ = rng.random::<u64>();

                match turn {
                    0 => {
                        state.terminated = true;
                        Err(ActionError::InvalidSwitch)
                    }
                    1 | 2 => {
                        let team = if turn == 1 {
                            &mut state.opponent
                        } else {
                            &mut state.player
                        };

                        *team = TeamState::new(
                            roster(100),
                            [false, true, true, true, false, false],
                            Some(1),
                        )
                        .unwrap();
                        Ok(Vec::new())
                    }
                    _ => Ok(Vec::new()),
                }
            })
            .unwrap();

        let selected = environment
            .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
            .unwrap();
        let before = environment.battle.clone();
        let reverse_before = environment.observation(BattleSide::Opponent).unwrap();

        assert_eq!(
            environment.step(Action::Move(0), Action::Switch(1)),
            Err(ActionError::InvalidSwitch)
        );
        assert_eq!(environment.battle, before);
        assert_eq!(
            environment.observation(BattleSide::Opponent).unwrap(),
            reverse_before
        );
        assert_eq!(
            environment.step(Action::Move(0), Action::Switch(1)),
            Err(ActionError::InvalidTeamSelection)
        );
        assert_eq!(environment.battle, before);
        assert_eq!(
            environment.observation(BattleSide::Opponent).unwrap(),
            reverse_before
        );
        assert_eq!(
            environment.step(Action::Switch(1), Action::Move(0)),
            Err(ActionError::InvalidTeamSelection)
        );
        assert_eq!(environment.battle, before);
        assert_eq!(
            environment.observation(BattleSide::Opponent).unwrap(),
            reverse_before
        );

        let retried = environment.step(Action::Move(0), Action::Move(0)).unwrap();

        assert_eq!(retried.observation, selected.observation);
        assert_eq!(retried.reward, 0.0);
        assert!(!retried.terminated);
        assert_eq!(transitions.get(), 4);

        // Neither a failed transition nor a failed observation consumed a turn.
        for turn in 2..=MAX_EPISODE_TURNS {
            let action = Action::Switch((turn - 1) % 2);
            let outcome = environment.step(action, action).unwrap();
            assert_eq!(outcome.truncated, turn == MAX_EPISODE_TURNS);
        }

        let before = environment.battle.clone();
        assert_eq!(
            environment.step(Action::Move(0), Action::Move(0)),
            Err(ActionError::EpisodeTruncated)
        );
        assert_eq!(environment.battle, before);
    }

    #[test]
    fn forced_replacements_do_not_consume_move_turns() {
        let preview = TeamPreviewObservation {
            player: roster(100),
            opponent: roster(100),
        };
        let transitions = Cell::new(0);
        let mut environment = Environment::new(preview, [0, 1, 2], |state, player, opponent, _| {
            if !matches!((player, opponent), (Action::Move(_), Action::Move(_))) {
                return Err(ActionError::WrongPhase);
            }

            transitions.set(transitions.get() + 1);
            state.opponent.damage_active(1_000).unwrap();
            Ok(Vec::new())
        })
        .unwrap();

        environment
            .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
            .unwrap();
        environment.elapsed_turns = MAX_EPISODE_TURNS - 3;

        let first_faint = environment.step(Action::Move(0), Action::Move(0)).unwrap();
        assert!(!first_faint.terminated);
        assert!(!first_faint.truncated);
        assert_eq!(transitions.get(), 1);

        let first_replacement = environment
            .step(Action::Move(0), Action::Switch(1))
            .unwrap();
        assert_eq!(first_replacement.reward, 0.0);
        assert!(!first_replacement.terminated);
        assert!(!first_replacement.truncated);
        assert_eq!(transitions.get(), 1);

        let second_faint = environment.step(Action::Move(0), Action::Move(0)).unwrap();
        assert!(!second_faint.terminated);
        assert!(!second_faint.truncated);

        let second_replacement = environment
            .step(Action::Move(0), Action::Switch(2))
            .unwrap();
        assert_eq!(second_replacement.reward, 0.0);
        assert!(!second_replacement.terminated);
        assert!(!second_replacement.truncated);
        assert_eq!(transitions.get(), 2);

        let final_faint = environment.step(Action::Move(0), Action::Move(0)).unwrap();
        assert!((final_faint.reward - 1.133_333_3).abs() < 1e-6);
        assert!(final_faint.terminated);
        assert!(!final_faint.truncated);
        assert_eq!(transitions.get(), 3);
    }

    #[test]
    fn terminal_results_on_the_last_turn_are_not_truncated() {
        for (player_hp, opponent_hp, winner, reward) in [
            ([100; 3], [0; 3], Some(BattleSide::Player), 1.4),
            ([0; 3], [100; 3], Some(BattleSide::Opponent), -1.4),
            ([0; 3], [0; 3], None, 0.0),
        ] {
            let mut environment = Environment::new(
                TeamPreviewObservation {
                    player: roster(100),
                    opponent: roster(100),
                },
                [0, 1, 2],
                |state, _, _, _| {
                    state.player = team(player_hp);
                    state.opponent = team(opponent_hp);
                    Ok(Vec::new())
                },
            )
            .unwrap();
            environment
                .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
                .unwrap();
            environment.elapsed_turns = MAX_EPISODE_TURNS - 1;

            let outcome = environment.step(Action::Move(0), Action::Move(0)).unwrap();
            assert!(outcome.terminated);
            assert!(!outcome.truncated);
            assert!((outcome.reward - reward).abs() < 1e-6);
            assert!((outcome.reward_for(BattleSide::Opponent) + reward).abs() < 1e-6);

            assert_eq!(
                outcome.events.last(),
                Some(&BattleEvent::BattleCompleted { winner })
            );
            assert_eq!(
                environment.step(Action::Move(0), Action::Move(0)),
                Err(ActionError::BattleTerminated)
            );
        }
    }

    fn state(opponent_hp: [u32; 3], terminated: bool) -> BattleState {
        BattleState {
            player: team([100; 3]),
            opponent: team(opponent_hp),
            terminated,
        }
    }

    fn team(hp: [u32; 3]) -> TeamState {
        TeamState::new(
            std::array::from_fn(|slot| {
                PokemonState::new(if slot < 3 { hp[slot] } else { 100 }, 100, [true; 4]).unwrap()
            }),
            [true, true, true, false, false, false],
            hp.iter().position(|&hp| hp > 0),
        )
        .unwrap()
    }

    fn roster(hp: u32) -> [PokemonState; 6] {
        std::array::from_fn(|_| PokemonState::new(hp, hp, [true; 4]).unwrap())
    }
}
