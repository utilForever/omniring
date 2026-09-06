use crate::battle_logic::Battle as CoreBattle;
use crate::info::Pokemon;
use crate::{
    Action, ActionError, Battle, BattleObservation, BattleState, OpponentObservation, PokemonState,
    TeamPreviewObservation, TeamState, calculate_reward,
};

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
    opponent_revealed: [bool; 6],
    transition: F,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepOutcome {
    pub observation: Observation,
    pub reward: f32,
    pub terminated: bool,
}

impl Environment<()> {
    /// Creates an environment using the core battle logic and four moves per Pokemon.
    /// Roster slots are preserved; the first selected slot is each side's lead.
    /// `reset` restores the supplied HP and returns to team preview.
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
    /// // Use turn.observation, turn.reward, and turn.terminated for training.
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
        Environment<impl FnMut(&mut BattleState, Action, Action) -> Result<(), ActionError>>,
        ActionError,
    > {
        let preview = TeamPreviewObservation {
            player: preview_roster(&player)?,
            opponent: preview_roster(&opponent)?,
        };

        Environment::new(
            preview,
            opponent_selection,
            move |state, action, opponent_action| {
                // The rosters supply battle data; mutable HP belongs to the episode state.
                // ponytail: retain core battle state when turn-dependent effects are supported.
                let mut turn = CoreBattle::new(
                    active_pokemon(&player, &state.player),
                    active_pokemon(&opponent, &state.opponent),
                );
                let player_hp = turn.p1.current_hp;
                let opponent_hp = turn.p2.current_hp;

                match (action, opponent_action) {
                    (Action::Move(player_move), Action::Move(opponent_move)) => {
                        turn.simulate_turn(player_move, opponent_move).map(|_| ())
                    }
                    (Action::Move(slot), Action::Switch(_)) => {
                        CoreBattle::execute_move(&turn.p1, &mut turn.p2, slot, false).map(|_| ())
                    }
                    (Action::Switch(_), Action::Move(slot)) => {
                        CoreBattle::execute_move(&turn.p2, &mut turn.p1, slot, false).map(|_| ())
                    }
                    _ => return Err(ActionError::WrongPhase),
                }
                .map_err(ActionError::Battle)?;

                state
                    .player
                    .damage_active(u32::from(player_hp - turn.p1.current_hp))
                    .map_err(ActionError::InvalidState)?;
                state
                    .opponent
                    .damage_active(u32::from(opponent_hp - turn.p2.current_hp))
                    .map_err(ActionError::InvalidState)?;
                Ok(())
            },
        )
    }
}

fn preview_roster(roster: &[Pokemon; 6]) -> Result<[PokemonState; 6], ActionError> {
    let [a, b, c, d, e, f] = roster.each_ref().map(|pokemon| {
        PokemonState::new(
            u32::from(pokemon.current_hp),
            u32::from(pokemon.stats.hp),
            [true; 4],
        )
        .map_err(ActionError::InvalidState)
    });
    Ok([a?, b?, c?, d?, e?, f?])
}

fn active_pokemon(roster: &[Pokemon; 6], team: &TeamState) -> Pokemon {
    let slot = team
        .slot_active()
        .expect("turn resolution requires an active Pokemon");

    let mut pokemon = roster[slot].clone();
    pokemon.current_hp = u16::try_from(team.roster()[slot].hp_curr())
        .expect("episode HP cannot exceed the original roster's u16 HP");

    pokemon
}

impl<F> Environment<F>
where
    F: FnMut(&mut BattleState, Action, Action) -> Result<(), ActionError>,
{
    pub fn new(
        preview: TeamPreviewObservation,
        opponent_selection: [usize; 3],
        transition: F,
    ) -> Result<Self, ActionError> {
        preview.validate_player_action(Action::SelectTeam(opponent_selection))?;

        let opponent = selected_team(preview.opponent.clone(), opponent_selection)?;

        Ok(Self {
            preview,
            opponent,
            battle: None,
            opponent_revealed: [false; 6],
            transition,
        })
    }

    pub fn reset(&mut self) -> Observation {
        self.battle = None;
        self.opponent_revealed = [false; 6];
        Observation::TeamPreview(self.preview.clone())
    }

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

            self.opponent_revealed[opponent.slot_active().unwrap()] = true;

            let battle = self.battle.insert(Battle::new(BattleState {
                player,
                opponent,
                terminated: false,
            }));

            return Ok(StepOutcome {
                observation: Observation::Battle(observation(
                    battle.state(),
                    self.opponent_revealed,
                )?),
                reward: 0.0,
                terminated: false,
            });
        }

        let previous = self.battle.as_ref().unwrap().clone();
        let opponent_revealed = self.opponent_revealed;
        let outcome = (|| {
            let battle = self.battle.as_mut().unwrap();
            let state = battle.play_turn(action, opponent_action, &mut self.transition)?;

            if let Some(active) = state.opponent.slot_active() {
                self.opponent_revealed[active] = true;
            }

            Ok(StepOutcome {
                observation: Observation::Battle(observation(state, self.opponent_revealed)?),
                reward: calculate_reward(previous.state(), state),
                terminated: state.terminated,
            })
        })();

        if outcome.is_err() {
            self.battle = Some(previous);
            self.opponent_revealed = opponent_revealed;
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
    state: &BattleState,
    opponent_revealed: [bool; 6],
) -> Result<BattleObservation, ActionError> {
    Ok(BattleObservation {
        player: state.player.clone(),
        opponent: OpponentObservation::new(&state.opponent, opponent_revealed)
            .map_err(|_| ActionError::InvalidTeamSelection)?,
        terminated: state.terminated,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{Environment, Observation};
    use crate::{
        Action, ActionError, BattleState, PokemonState, TeamPreviewObservation, TeamState,
    };

    #[test]
    fn runs_a_hidden_information_episode_from_preview_to_reset() {
        let preview = TeamPreviewObservation {
            player: roster(100),
            opponent: roster(100),
        };

        assert!(Environment::new(preview.clone(), [0, 0, 1], |_, _, _| Ok(())).is_err());

        let terminal = state([0; 3], true);
        let transitions = Cell::new(0);
        let mut environment =
            Environment::new(preview.clone(), [0, 1, 2], |state, player, opponent| {
                assert_eq!((player, opponent), (Action::Move(0), Action::Move(0)));
                transitions.set(transitions.get() + 1);
                *state = terminal.clone();
                Ok(())
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
        let mut environment = Environment::new(preview, [0, 1, 2], |state, _, _| {
            let turn = transitions.get();
            transitions.set(turn + 1);

            match turn {
                0 => {
                    state.terminated = true;
                    Err(ActionError::InvalidSwitch)
                }
                1 => {
                    state.opponent = TeamState::new(
                        roster(100),
                        [false, true, true, true, false, false],
                        Some(1),
                    )
                    .unwrap();
                    Ok(())
                }
                _ => Ok(()),
            }
        })
        .unwrap();

        let selected = environment
            .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
            .unwrap();
        assert_eq!(
            environment.step(Action::Move(0), Action::Move(0)),
            Err(ActionError::InvalidSwitch)
        );
        assert_eq!(
            environment.step(Action::Move(0), Action::Move(0)),
            Err(ActionError::InvalidTeamSelection)
        );

        let retried = environment.step(Action::Move(0), Action::Move(0)).unwrap();

        assert_eq!(retried.observation, selected.observation);
        assert_eq!(retried.reward, 0.0);
        assert!(!retried.terminated);
        assert_eq!(transitions.get(), 3);
    }

    #[test]
    fn forced_replacements_do_not_consume_move_turns() {
        let preview = TeamPreviewObservation {
            player: roster(100),
            opponent: roster(100),
        };
        let transitions = Cell::new(0);
        let mut environment = Environment::new(preview, [0, 1, 2], |state, player, opponent| {
            if !matches!((player, opponent), (Action::Move(_), Action::Move(_))) {
                return Err(ActionError::WrongPhase);
            }

            transitions.set(transitions.get() + 1);
            state.opponent.damage_active(1_000).unwrap();
            Ok(())
        })
        .unwrap();

        environment
            .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
            .unwrap();

        let first_faint = environment.step(Action::Move(0), Action::Move(0)).unwrap();
        assert!(!first_faint.terminated);
        assert_eq!(transitions.get(), 1);

        let first_replacement = environment
            .step(Action::Move(0), Action::Switch(1))
            .unwrap();
        assert_eq!(first_replacement.reward, 0.0);
        assert!(!first_replacement.terminated);
        assert_eq!(transitions.get(), 1);

        let second_faint = environment.step(Action::Move(0), Action::Move(0)).unwrap();
        assert!(!second_faint.terminated);

        let second_replacement = environment
            .step(Action::Move(0), Action::Switch(2))
            .unwrap();
        assert_eq!(second_replacement.reward, 0.0);
        assert!(!second_replacement.terminated);
        assert_eq!(transitions.get(), 2);

        let final_faint = environment.step(Action::Move(0), Action::Move(0)).unwrap();
        assert!((final_faint.reward - 1.133_333_3).abs() < 1e-6);
        assert!(final_faint.terminated);
        assert_eq!(transitions.get(), 3);
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
