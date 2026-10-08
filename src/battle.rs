use std::sync::Arc;

use crate::damage::calculate_damage_with_rng;
pub use crate::damage::{DamageModifier, DamageResult, Fraction, calculate_damage};
use crate::info::{BattleError, Move, MoveCategory, Pokemon};
use crate::{Action, ActionError, BattleState, StateError, TeamState};
use rand::{RngExt, SeedableRng, rngs::SmallRng};

type Rosters = ([Pokemon; 6], [Pokemon; 6]);

/// Identifies a Trainer independently of Pokemon names or roster slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattleSide {
    Player,
    Opponent,
}

impl BattleSide {
    fn other(self) -> Self {
        match self {
            Self::Player => Self::Opponent,
            Self::Opponent => Self::Player,
        }
    }
}

/// Events from one successful turn. Selections precede resolution events; all slots are zero-based.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BattleEvent {
    /// An accepted move choice, recorded player first, even if its user faints before acting.
    MoveSelected {
        side: BattleSide,
        slot: usize,
        move_slot: usize,
    },
    /// An attempted move that failed its accuracy check.
    Miss {
        side: BattleSide,
        slot: usize,
        move_slot: usize,
    },
    MoveBlocked {
        side: BattleSide,
        slot: usize,
        move_slot: usize,
    },
    /// Actual HP lost, capped at the target's remaining HP.
    Damage {
        side: BattleSide,
        slot: usize,
        damage: u32,
        hp_before: u32,
        hp_after: u32,
    },
    Fainted {
        side: BattleSide,
        slot: usize,
    },
    /// `from` is `None` for a forced replacement after fainting.
    Switched {
        side: BattleSide,
        from: Option<usize>,
        to: usize,
    },
    /// `None` means both selected teams are defeated.
    BattleCompleted {
        winner: Option<BattleSide>,
    },
}

/// A single battle that owns its state and random stream and delegates turn resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Battle {
    state: BattleState,
    rosters: Option<Arc<Rosters>>,
    pub(crate) rng: SmallRng,
    events: Vec<BattleEvent>,
}

impl Battle {
    /// Creates a battle for custom `play_turn` resolvers, without bound rosters.
    /// Chooses a random seed once at construction.
    pub fn new(state: BattleState) -> Self {
        Self::with_seed(state, rand::random())
    }

    /// Creates a reproducible battle for custom `play_turn` resolvers, without bound rosters.
    /// For the built-in resolver, use `Battle::with_rosters_and_seed` instead.
    /// Cloning a battle also copies its random stream.
    /// Replays require the same initial state, actions, rosters, library versions, and target platform.
    pub fn with_seed(state: BattleState, seed: u64) -> Self {
        Self::with_rng(state, SmallRng::seed_from_u64(seed))
    }

    pub(crate) fn with_rng(state: BattleState, rng: SmallRng) -> Self {
        Self {
            state,
            rosters: None,
            rng,
            events: Vec::new(),
        }
    }

    /// Binds immutable calculation data for the lifetime of a direct battle.
    /// Cloned battles share these rosters and copy their runtime state and random stream.
    /// Live HP and move availability come from `state`, not from the rosters.
    ///
    /// ```
    /// use omniring::{Action, ActionError, Battle, BattleState};
    /// use omniring::info::Pokemon;
    /// # fn turn(state: BattleState, player: [Pokemon; 6], opponent: [Pokemon; 6]) -> Result<(), ActionError> {
    /// let mut battle = Battle::with_rosters(state, player, opponent);
    /// battle.play_turn_with_rosters(Action::Move(0), Action::Move(0))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_rosters(state: BattleState, player: [Pokemon; 6], opponent: [Pokemon; 6]) -> Self {
        Self::with_rosters_and_seed(state, player, opponent, rand::random())
    }

    /// Binds immutable rosters and seeds the random stream for a reproducible direct battle.
    /// Replays require the same initial state, actions, rosters, library versions, and target platform.
    pub fn with_rosters_and_seed(
        state: BattleState,
        player: [Pokemon; 6],
        opponent: [Pokemon; 6],
        seed: u64,
    ) -> Self {
        Self {
            rosters: Some(Arc::new((player, opponent))),
            ..Self::with_seed(state, seed)
        }
    }

    pub fn state(&self) -> &BattleState {
        &self.state
    }

    /// Events from the last successful turn. Failed turns leave this slice unchanged.
    /// Copy or append these events after each turn to retain an episode history.
    pub fn events(&self) -> &[BattleEvent] {
        &self.events
    }

    /// Resolves a turn using the immutable rosters bound at construction.
    /// HP, selection, active slots, and move availability come from this battle's state.
    /// Returns `ActionError::MissingRosters` for a battle created with `Battle::new` or `Battle::with_seed`.
    /// A failed turn leaves the state and random stream unchanged. An available move missing from the
    /// bound roster returns `ActionError::Battle(BattleError::InvalidMoveIndex)`.
    pub fn play_turn_with_rosters(
        &mut self,
        player_action: Action,
        opponent_action: Action,
    ) -> Result<&BattleState, ActionError> {
        let rosters = self.rosters.clone().ok_or(ActionError::MissingRosters)?;

        self.play_turn(
            player_action,
            opponent_action,
            |state, action, opponent_action, rng| {
                Self::resolve_turn(state, &rosters.0, &rosters.1, action, opponent_action, rng)
            },
        )
    }

    /// Resolves a turn with a custom transition using this battle's random stream.
    /// The callback returns its attack outcomes in order; selections, switches, and completion are added here.
    /// Return `Ok(Vec::new())` when the transition has no events to report.
    /// State, random draws, and events are committed only on success. Side effects captured by
    /// the callback are the caller's responsibility and cannot be rolled back here.
    pub fn play_turn(
        &mut self,
        player_action: Action,
        opponent_action: Action,
        resolve_turn: impl FnOnce(
            &mut BattleState,
            Action,
            Action,
            &mut SmallRng,
        ) -> Result<Vec<BattleEvent>, ActionError>,
    ) -> Result<&BattleState, ActionError> {
        if self.state.terminated {
            return Err(ActionError::BattleTerminated);
        }

        let replacement_pending = self.state.player.slot_active().is_none()
            || self.state.opponent.slot_active().is_none();
        let mut next = self.state.clone();
        let mut rng = self.rng.clone();
        let mut events = Vec::new();

        next.validate_action(BattleSide::Player, player_action)?;
        next.validate_action(BattleSide::Opponent, opponent_action)?;

        if !replacement_pending {
            for (side, team, action) in [
                (BattleSide::Player, &next.player, player_action),
                (BattleSide::Opponent, &next.opponent, opponent_action),
            ] {
                if let Action::Move(move_slot) = action {
                    events.push(BattleEvent::MoveSelected {
                        side,
                        slot: team.slot_active().unwrap(),
                        move_slot,
                    });
                }
            }
        }

        if let Action::Switch(slot) = player_action {
            let from = next.player.slot_active();

            next.player
                .switch_to(slot)
                .map_err(|_| ActionError::InvalidSwitch)?;
            events.push(BattleEvent::Switched {
                side: BattleSide::Player,
                from,
                to: slot,
            });
        }

        if let Action::Switch(slot) = opponent_action {
            let from = next.opponent.slot_active();

            next.opponent
                .switch_to(slot)
                .map_err(|_| ActionError::InvalidSwitch)?;
            events.push(BattleEvent::Switched {
                side: BattleSide::Opponent,
                from,
                to: slot,
            });
        }

        if !replacement_pending
            && (matches!(player_action, Action::Move(_))
                || matches!(opponent_action, Action::Move(_)))
        {
            events.extend(resolve_turn(
                &mut next,
                player_action,
                opponent_action,
                &mut rng,
            )?);
        }

        next.terminated =
            !next.player.has_available_selected() || !next.opponent.has_available_selected();

        if next.terminated {
            let winner = if next.player.has_available_selected() {
                Some(BattleSide::Player)
            } else if next.opponent.has_available_selected() {
                Some(BattleSide::Opponent)
            } else {
                None
            };

            events.push(BattleEvent::BattleCompleted { winner });
        }

        self.state = next;
        self.rng = rng;
        self.events = events;
        Ok(&self.state)
    }

    /// Resolves attacks against the candidate owned by `Battle::play_turn`.
    /// Actions are validated and switches applied before this resolver runs.
    pub(crate) fn resolve_turn(
        state: &mut BattleState,
        player_roster: &[Pokemon; 6],
        opponent_roster: &[Pokemon; 6],
        action: Action,
        opponent_action: Action,
        rng: &mut SmallRng,
    ) -> Result<Vec<BattleEvent>, ActionError> {
        let player_slot = state
            .player
            .slot_active()
            .ok_or(ActionError::InvalidState(StateError::InvalidActiveSlot))?;
        let opponent_slot = state
            .opponent
            .slot_active()
            .ok_or(ActionError::InvalidState(StateError::InvalidActiveSlot))?;

        let player = &player_roster[player_slot];
        let opponent = &opponent_roster[opponent_slot];

        let mut events = Vec::new();

        match (action, opponent_action) {
            (Action::Move(first), Action::Move(second)) => {
                let result = simulate_turn(
                    player,
                    &mut state.player,
                    opponent,
                    &mut state.opponent,
                    first,
                    second,
                    rng,
                )?;
                let (side, attacker, defender, first_move, second_move) = match result.order {
                    TurnOrder::FirstPokemon => (
                        BattleSide::Player,
                        player_slot,
                        opponent_slot,
                        first,
                        second,
                    ),
                    TurnOrder::SecondPokemon => (
                        BattleSide::Opponent,
                        opponent_slot,
                        player_slot,
                        second,
                        first,
                    ),
                };

                result
                    .first
                    .append_events(&mut events, side, attacker, defender, first_move);

                if let Some(second) = result.second {
                    second.append_events(
                        &mut events,
                        side.other(),
                        defender,
                        attacker,
                        second_move,
                    );
                }
            }
            (Action::Move(slot), Action::Switch(_)) => execute_move(
                player,
                &state.player,
                opponent,
                &mut state.opponent,
                slot,
                false,
                rng,
            )?
            .append_events(
                &mut events,
                BattleSide::Player,
                player_slot,
                opponent_slot,
                slot,
            ),
            (Action::Switch(_), Action::Move(slot)) => execute_move(
                opponent,
                &state.opponent,
                player,
                &mut state.player,
                slot,
                false,
                rng,
            )?
            .append_events(
                &mut events,
                BattleSide::Opponent,
                opponent_slot,
                player_slot,
                slot,
            ),
            _ => return Err(ActionError::WrongPhase),
        }
        Ok(events)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttackResult {
    pub attacker: String,
    pub defender: String,
    pub move_name: String,
    pub damage: u16,
    pub effectiveness: f32,
    pub blocked: bool, // protect moves
    pub missed: bool,
    pub defender_hp_before: u32,
    pub defender_hp_after: u32,
}

impl AttackResult {
    fn append_events(
        &self,
        events: &mut Vec<BattleEvent>,
        side: BattleSide,
        attacker: usize,
        defender: usize,
        move_slot: usize,
    ) {
        if self.missed {
            events.push(BattleEvent::Miss {
                side,
                slot: attacker,
                move_slot,
            });
        } else if self.blocked {
            events.push(BattleEvent::MoveBlocked {
                side,
                slot: attacker,
                move_slot,
            });
        } else if self.defender_hp_after < self.defender_hp_before {
            events.push(BattleEvent::Damage {
                side: side.other(),
                slot: defender,
                damage: self.defender_hp_before - self.defender_hp_after,
                hp_before: self.defender_hp_before,
                hp_after: self.defender_hp_after,
            });

            if self.defender_hp_after == 0 {
                events.push(BattleEvent::Fainted {
                    side: side.other(),
                    slot: defender,
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurnResult {
    pub order: TurnOrder,
    pub first: AttackResult,
    pub second: Option<AttackResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOrder {
    FirstPokemon,
    SecondPokemon,
}

fn determine_turn_order(
    first: &Pokemon,
    second: &Pokemon,
    first_slot: usize,
    second_slot: usize,
    rng: &mut SmallRng,
) -> Result<TurnOrder, BattleError> {
    validate_move_index(first, first_slot)?;
    validate_move_index(second, second_slot)?;

    let first_key = (first.moves[first_slot].priority, first.stats.speed);
    let second_key = (second.moves[second_slot].priority, second.stats.speed);

    Ok(match first_key.cmp(&second_key) {
        std::cmp::Ordering::Greater => TurnOrder::FirstPokemon,
        std::cmp::Ordering::Less => TurnOrder::SecondPokemon,
        std::cmp::Ordering::Equal if rng.random() => TurnOrder::FirstPokemon,
        std::cmp::Ordering::Equal => TurnOrder::SecondPokemon,
    })
}

fn simulate_turn(
    first: &Pokemon,
    first_team: &mut TeamState,
    second: &Pokemon,
    second_team: &mut TeamState,
    first_move: usize,
    second_move: usize,
    rng: &mut SmallRng,
) -> Result<TurnResult, ActionError> {
    if first_team.slot_active().is_none() || second_team.slot_active().is_none() {
        return Err(ActionError::Battle(BattleError::FaintedPokemonCannotBattle));
    }

    let order = determine_turn_order(first, second, first_move, second_move, rng)
        .map_err(ActionError::Battle)?;
    let (faster, faster_team, faster_move, slower, slower_team, slower_move) = match order {
        TurnOrder::FirstPokemon => (
            first,
            first_team,
            first_move,
            second,
            second_team,
            second_move,
        ),
        TurnOrder::SecondPokemon => (
            second,
            second_team,
            second_move,
            first,
            first_team,
            first_move,
        ),
    };
    let first = execute_move(
        faster,
        faster_team,
        slower,
        slower_team,
        faster_move,
        false,
        rng,
    )?;
    let second = if slower_team.slot_active().is_none() {
        None
    } else {
        Some(execute_move(
            slower,
            slower_team,
            faster,
            faster_team,
            slower_move,
            !first.missed
                && !first.blocked
                && is_protective_status_move(&faster.moves[faster_move]),
            rng,
        )?)
    };

    Ok(TurnResult {
        order,
        first,
        second,
    })
}

fn execute_move(
    attacker: &Pokemon,
    attacker_team: &TeamState,
    defender: &Pokemon,
    defender_team: &mut TeamState,
    move_index: usize,
    defender_is_protected: bool,
    rng: &mut SmallRng,
) -> Result<AttackResult, ActionError> {
    if attacker_team.slot_active().is_none() {
        return Err(ActionError::Battle(BattleError::FaintedPokemonCannotAttack));
    }

    let selected_move = attacker.moves.get(move_index).ok_or(ActionError::Battle(
        BattleError::InvalidMoveIndex { index: move_index },
    ))?;
    let target = defender_team.slot_active();
    let defender_hp_before = target.map_or(0, |slot| defender_team.roster()[slot].hp_curr());
    let blocked = target.is_none() || (defender_is_protected && selected_move.power > 0);
    let missed = !blocked
        && selected_move.accuracy.is_some_and(|accuracy| {
            accuracy == 0 || (accuracy < 100 && rng.random_range(0..100_u8) >= accuracy)
        });
    let result = if blocked || missed {
        DamageResult {
            damage: 0,
            effectiveness: 1.0,
        }
    } else {
        let result = calculate_damage_with_rng(attacker, defender, selected_move, rng)
            .map_err(ActionError::Battle)?;
        defender_team
            .damage_active(u32::from(result.damage))
            .map_err(ActionError::InvalidState)?;
        result
    };

    Ok(AttackResult {
        attacker: attacker.entry.name.to_string(),
        defender: defender.entry.name.to_string(),
        move_name: selected_move.name.clone(),
        damage: result.damage,
        effectiveness: result.effectiveness,
        blocked,
        missed,
        defender_hp_before,
        defender_hp_after: target.map_or(0, |slot| defender_team.roster()[slot].hp_curr()),
    })
}

fn validate_move_index(pokemon: &Pokemon, move_index: usize) -> Result<(), BattleError> {
    pokemon
        .moves
        .get(move_index)
        .map(|_| ())
        .ok_or(BattleError::InvalidMoveIndex { index: move_index })
}

fn is_protective_status_move(selected_move: &Move) -> bool {
    selected_move.category == MoveCategory::Status
        && matches!(selected_move.name.as_str(), "Protect" | "Detect")
}

#[cfg(test)]
mod state_tests {
    use super::Battle;
    use crate::{Action, ActionError, BattleState, PokemonState, TeamState};

    #[test]
    fn roster_turn_without_bound_rosters_leaves_state_and_rng_unchanged() {
        for mut battle in [Battle::new(state()), Battle::with_seed(state(), 46)] {
            let before = battle.clone();

            assert_eq!(
                battle.play_turn_with_rosters(Action::Move(0), Action::Move(0)),
                Err(ActionError::MissingRosters)
            );
            assert_eq!(battle, before);
        }
    }

    #[test]
    fn failed_resolution_discards_all_runtime_mutations() {
        let initial = state();
        let mut battle = Battle::new(initial.clone());

        assert_eq!(
            battle.play_turn(Action::Switch(1), Action::Move(0), |next, _, _, _| {
                next.player.damage_active(25).unwrap();
                next.opponent = TeamState::new(
                    team([false; 4]).roster().clone(),
                    [false, true, true, true, false, false],
                    Some(1),
                )
                .unwrap();
                next.terminated = true;
                Err(ActionError::InvalidSwitch)
            }),
            Err(ActionError::InvalidSwitch)
        );
        assert_eq!(battle.state(), &initial);
    }

    #[test]
    fn validates_delegates_and_stops_after_termination() {
        let mut battle = Battle::new(state());

        assert_eq!(
            battle.play_turn(
                Action::Switch(0),
                Action::Move(0),
                |_, _, _, _| unreachable!()
            ),
            Err(ActionError::InvalidSwitch)
        );
        assert_eq!(
            battle.play_turn(
                Action::Move(0),
                Action::Move(1),
                |_, _, _, _| unreachable!()
            ),
            Err(ActionError::UnavailableMove)
        );
        assert_eq!(
            battle.play_turn(Action::Move(0), Action::Move(0), |_, _, _, _| Err(
                ActionError::InvalidSwitch
            )),
            Err(ActionError::InvalidSwitch)
        );

        let state = battle
            .play_turn(
                Action::Move(1),
                Action::Move(2),
                |state, player, opponent, _| {
                    assert_eq!((player, opponent), (Action::Move(1), Action::Move(2)));

                    state.opponent.damage_active(1_000).unwrap();
                    state.opponent.switch_to(1).unwrap();
                    state.opponent.damage_active(1_000).unwrap();
                    state.opponent.switch_to(2).unwrap();
                    state.opponent.damage_active(1_000).unwrap();
                    Ok(Vec::new())
                },
            )
            .unwrap();

        assert!(state.terminated);
        assert_eq!(
            battle.play_turn(
                Action::Move(0),
                Action::Move(0),
                |_, _, _, _| unreachable!()
            ),
            Err(ActionError::BattleTerminated)
        );
    }

    #[test]
    fn non_terminal_faint_requires_switch_before_moves_resume() {
        let mut battle = Battle::new(state());

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _, _| {
                state.player.damage_active(1_000).unwrap();
                Ok(Vec::new())
            })
            .unwrap();
        assert_eq!(state.player.slot_active(), None);
        assert!(!state.terminated);
        assert_eq!(
            state.legal_player_actions(),
            vec![Action::Switch(1), Action::Switch(2)]
        );
        assert_eq!(
            battle.play_turn(
                Action::Move(0),
                Action::Move(0),
                |_, _, _, _| unreachable!()
            ),
            Err(ActionError::UnavailableMove)
        );

        let state = battle
            .play_turn(Action::Switch(1), Action::Move(0), |_, _, _, _| {
                panic!("forced replacements do not resolve moves")
            })
            .unwrap();
        assert_eq!(state.player.slot_active(), Some(1));

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _, _| {
                state.opponent.damage_active(1).unwrap();
                Ok(Vec::new())
            })
            .unwrap();
        assert_eq!(state.opponent.roster()[0].hp_curr(), 99);
    }

    #[test]
    fn switch_only_turn_does_not_run_move_resolution() {
        let mut battle = Battle::new(state());

        let state = battle
            .play_turn(Action::Switch(1), Action::Switch(1), |_, _, _, _| {
                panic!("switch-only turns have no moves to resolve")
            })
            .unwrap();
        assert_eq!(state.player.slot_active(), Some(1));
        assert_eq!(state.opponent.slot_active(), Some(1));
    }

    #[test]
    fn battle_ends_only_after_all_selected_opponents_faint() {
        let mut battle = Battle::new(state());

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _, _| {
                state.opponent.damage_active(1_000).unwrap();
                Ok(Vec::new())
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Switch(1), |_, _, _, _| {
                panic!("forced replacements do not resolve moves")
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _, _| {
                state.opponent.damage_active(1_000).unwrap();
                Ok(Vec::new())
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Switch(2), |_, _, _, _| {
                panic!("forced replacements do not resolve moves")
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _, _| {
                state.opponent.damage_active(1_000).unwrap();
                Ok(Vec::new())
            })
            .unwrap();
        assert!(state.terminated);
        assert!(
            state.opponent.roster()[3..]
                .iter()
                .all(|pokemon| pokemon.hp_curr() > 0)
        );
        assert!(state.legal_player_actions().is_empty());
    }

    fn state() -> BattleState {
        BattleState {
            player: team([true; 4]),
            opponent: team([true, false, true, true]),
            terminated: false,
        }
    }

    fn team(move_availability: [bool; 4]) -> TeamState {
        TeamState::new(
            std::array::from_fn(|_| PokemonState::new(100, 100, move_availability).unwrap()),
            [true, true, true, false, false, false],
            Some(0),
        )
        .unwrap()
    }
}

#[cfg(test)]
mod resolution_tests {
    use super::*;
    use crate::PokemonState;
    use crate::info::{Nature, PokemonType, StatPoints};
    use crate::pokedex::{build_pokemon_from_pokedex, find_pokemon};

    #[test]
    fn accuracy_checks_preserve_hp_and_consume_only_needed_random_draws() {
        let defender = venusaur();
        let mut saw_hit_and_miss = [false; 2];

        for accuracy in [None, Some(0), Some(50), Some(100), Some(255)] {
            for blocked in [false, true] {
                for status in [false, true] {
                    for seed in 0..16 {
                        let mut attacker = charizard();
                        attacker.moves[0].accuracy = accuracy;

                        if status {
                            attacker.moves[0].category = MoveCategory::Status;
                            attacker.moves[0].power = 0;
                        }

                        let mut expected_rng = SmallRng::seed_from_u64(seed);
                        let blocked = blocked && !status;
                        let missed = !blocked
                            && match accuracy {
                                Some(0) => true,
                                Some(50) => expected_rng.random_range(0..100_u8) >= 50,
                                _ => false,
                            };
                        let damage = if missed || blocked {
                            0
                        } else {
                            calculate_damage_with_rng(
                                &attacker,
                                &defender,
                                &attacker.moves[0],
                                &mut expected_rng,
                            )
                            .unwrap()
                            .damage
                        };
                        let mut rng = SmallRng::seed_from_u64(seed);
                        let mut target = team(&defender);
                        let hp_before = target.roster()[0].hp_curr();
                        let result = execute_move(
                            &attacker,
                            &team(&attacker),
                            &defender,
                            &mut target,
                            0,
                            blocked,
                            &mut rng,
                        )
                        .unwrap();

                        assert_eq!(
                            (result.missed, result.blocked, result.damage),
                            (missed, blocked, damage)
                        );
                        assert_eq!(
                            result.defender_hp_after,
                            hp_before.saturating_sub(u32::from(damage))
                        );
                        assert_eq!(target.roster()[0].hp_curr(), result.defender_hp_after);
                        assert_eq!(rng, expected_rng);

                        if accuracy == Some(50) && !blocked {
                            saw_hit_and_miss[usize::from(missed)] = true;
                        }
                    }
                }
            }
        }

        assert_eq!(saw_hit_and_miss, [true, true]);
    }

    #[test]
    fn a_missed_protect_does_not_block_the_counterattack() {
        let mut protector = charizard();
        protector.moves[3].accuracy = Some(0);

        let attacker = venusaur();
        let result = simulate_turn(
            &protector,
            &mut team(&protector),
            &attacker,
            &mut team(&attacker),
            3,
            0,
            &mut SmallRng::seed_from_u64(46),
        )
        .unwrap();
        assert!(result.first.missed);

        let second = result.second.unwrap();
        assert!(!second.blocked);
        assert!(second.damage > 0);
    }

    #[test]
    fn collecting_events_preserves_turn_state_and_random_stream() {
        let player = charizard();
        let opponent = charizard(); // Same names and speed must not obscure the acting side.

        for seed in 0..16 {
            let initial = BattleState {
                player: team(&player),
                opponent: team(&opponent),
                terminated: false,
            };
            let mut expected = initial.clone();
            let mut rng = SmallRng::seed_from_u64(seed);
            let turn = simulate_turn(
                &player,
                &mut expected.player,
                &opponent,
                &mut expected.opponent,
                2,
                0,
                &mut rng,
            )
            .unwrap();

            let mut battle = Battle::with_rosters_and_seed(
                initial.clone(),
                std::array::from_fn(|_| player.clone()),
                std::array::from_fn(|_| opponent.clone()),
                seed,
            );
            battle
                .play_turn_with_rosters(Action::Move(2), Action::Move(0))
                .unwrap();

            assert_eq!(battle.state(), &expected);
            assert_eq!(battle.rng, rng);

            let first = match turn.order {
                TurnOrder::FirstPokemon => BattleSide::Player,
                TurnOrder::SecondPokemon => BattleSide::Opponent,
            };
            let events = battle.events();

            assert_eq!(events.len(), 4);
            assert_eq!(
                events[0],
                BattleEvent::MoveSelected {
                    side: BattleSide::Player,
                    slot: 0,
                    move_slot: 2
                }
            );
            assert_eq!(
                events[1],
                BattleEvent::MoveSelected {
                    side: BattleSide::Opponent,
                    slot: 0,
                    move_slot: 0
                }
            );

            for (index, side) in [(2, first.other()), (3, first)] {
                let (before, after) = match side {
                    BattleSide::Player => (&initial.player, &expected.player),
                    BattleSide::Opponent => (&initial.opponent, &expected.opponent),
                };
                let hp_before = before.roster()[0].hp_curr();
                let hp_after = after.roster()[0].hp_curr();

                assert_eq!(
                    events[index],
                    BattleEvent::Damage {
                        side,
                        slot: 0,
                        damage: hp_before - hp_after,
                        hp_before,
                        hp_after
                    }
                );
            }
        }
    }

    fn team(pokemon: &Pokemon) -> TeamState {
        TeamState::new(
            std::array::from_fn(|_| {
                PokemonState::new(
                    u32::from(pokemon.current_hp),
                    u32::from(pokemon.stats.hp),
                    std::array::from_fn(|slot| slot < pokemon.moves.len()),
                )
                .unwrap()
            }),
            [true, true, true, false, false, false],
            (pokemon.current_hp > 0).then_some(0),
        )
        .unwrap()
    }

    fn valid_stat_points() -> StatPoints {
        StatPoints {
            hp: 16,
            attack: 10,
            defense: 10,
            special_attack: 10,
            special_defense: 10,
            speed: 10,
        }
    }

    fn charizard() -> Pokemon {
        build_pokemon_from_pokedex(
            "Charizard",
            50,
            valid_stat_points(),
            Nature::Hardy,
            ["Flamethrower", "Air Slash", "Dragon Claw", "Protect"],
        )
        .unwrap()
    }

    fn venusaur() -> Pokemon {
        build_pokemon_from_pokedex(
            "Venusaur",
            50,
            valid_stat_points(),
            Nature::Hardy,
            ["Vine Whip", "Razor Leaf", "Sleep Powder", "Seed Bomb"],
        )
        .unwrap()
    }

    fn dragonite() -> Pokemon {
        build_pokemon_from_pokedex(
            "Dragonite",
            50,
            valid_stat_points(),
            Nature::Hardy,
            [
                "Extreme Speed",
                "Fire Punch",
                "Thunder Punch",
                "Dragon Tail",
            ],
        )
        .unwrap()
    }

    fn pokemon(species: &str) -> Pokemon {
        Pokemon::new(
            find_pokemon(species).unwrap(),
            50,
            valid_stat_points(),
            Nature::Hardy,
            None,
            std::array::from_fn::<_, 4, _>(|_| {
                Move::new(
                    "Test Move",
                    PokemonType::Normal,
                    MoveCategory::Status,
                    0,
                    None,
                    0,
                )
            }),
        )
        .unwrap()
    }

    #[test]
    fn immune_move_result_reports_zero_effectiveness() {
        let mut attacker = charizard();
        let defender = pokemon("Gengar");
        let hp_before = u32::from(defender.current_hp);

        attacker.moves[0] = Move::new(
            "Test Move",
            PokemonType::Normal,
            MoveCategory::Special,
            80,
            Some(100),
            0,
        );

        let result = execute_move(
            &attacker,
            &team(&attacker),
            &defender,
            &mut team(&defender),
            0,
            false,
            &mut SmallRng::seed_from_u64(0),
        )
        .unwrap();
        assert_eq!(result.damage, 0);
        assert_eq!(result.effectiveness, 0.0);
        assert_eq!(result.defender_hp_after, hp_before);
        assert!(!result.blocked);
    }

    #[test]
    fn priority_move_can_attack_before_faster_pokemon() {
        let slower = dragonite();
        let faster = charizard();

        let order =
            determine_turn_order(&slower, &faster, 0, 0, &mut SmallRng::seed_from_u64(0)).unwrap();
        let result = simulate_turn(
            &slower,
            &mut team(&slower),
            &faster,
            &mut team(&faster),
            0,
            0,
            &mut SmallRng::seed_from_u64(0),
        )
        .unwrap();
        assert_eq!(order, TurnOrder::FirstPokemon);
        assert_eq!(result.first.attacker, "Dragonite");
    }

    #[test]
    fn higher_priority_move_faster() {
        let slower = dragonite();
        let faster = charizard();

        let order =
            determine_turn_order(&slower, &faster, 0, 3, &mut SmallRng::seed_from_u64(0)).unwrap();
        let result = simulate_turn(
            &slower,
            &mut team(&slower),
            &faster,
            &mut team(&faster),
            0,
            3,
            &mut SmallRng::seed_from_u64(0),
        )
        .unwrap();
        assert_eq!(order, TurnOrder::SecondPokemon);
        assert_eq!(result.first.attacker, "Charizard");
    }

    #[test]
    fn lower_priority_move_slower() {
        let slower = venusaur();
        let faster = dragonite();

        let order =
            determine_turn_order(&slower, &faster, 0, 3, &mut SmallRng::seed_from_u64(0)).unwrap();
        let result = simulate_turn(
            &slower,
            &mut team(&slower),
            &faster,
            &mut team(&faster),
            0,
            3,
            &mut SmallRng::seed_from_u64(0),
        )
        .unwrap();
        assert_eq!(order, TurnOrder::FirstPokemon);
        assert_eq!(result.first.attacker, "Venusaur");
    }

    #[test]
    fn faster_pokemon_attacks_first_and_fainting_stops_counterattack() {
        let attacker = charizard();
        let defender = venusaur();

        let mut defender_team = team(&defender);
        defender_team
            .damage_active(u32::from(defender.current_hp) - 10)
            .unwrap();
        let result = simulate_turn(
            &attacker,
            &mut team(&attacker),
            &defender,
            &mut defender_team,
            0,
            0,
            &mut SmallRng::seed_from_u64(0),
        )
        .unwrap();
        assert_eq!(result.first.attacker, "Charizard");
        assert!(result.second.is_none());
    }

    #[test]
    fn second_input_can_act_first_when_it_is_faster() {
        let slower = venusaur();
        let faster = charizard();

        let result = simulate_turn(
            &slower,
            &mut team(&slower),
            &faster,
            &mut team(&faster),
            1,
            0,
            &mut SmallRng::seed_from_u64(0),
        )
        .unwrap();
        assert_eq!(result.first.attacker, "Charizard");
    }

    #[test]
    fn invalid_move_index_returns_error() {
        let attacker = charizard();
        let defender = venusaur();

        let result = simulate_turn(
            &attacker,
            &mut team(&attacker),
            &defender,
            &mut team(&defender),
            4,
            0,
            &mut SmallRng::seed_from_u64(0),
        );
        assert_eq!(
            result,
            Err(ActionError::Battle(BattleError::InvalidMoveIndex {
                index: 4
            }))
        );
    }

    #[test]
    fn turn_cannot_start_with_fainted_pokemon() {
        let attacker = charizard();
        let defender = venusaur();

        let mut attacker_team = team(&attacker);
        attacker_team.damage_active(u32::MAX).unwrap();
        let result = simulate_turn(
            &attacker,
            &mut attacker_team,
            &defender,
            &mut team(&defender),
            0,
            0,
            &mut SmallRng::seed_from_u64(0),
        );
        assert_eq!(
            result,
            Err(ActionError::Battle(BattleError::FaintedPokemonCannotBattle))
        );
    }

    #[test]
    fn protect_blocks_the_second_damage_move() {
        let protector = charizard();
        let attacker = venusaur();
        let mut protector_team = team(&protector);
        let result = simulate_turn(
            &protector,
            &mut protector_team,
            &attacker,
            &mut team(&attacker),
            3,
            0,
            &mut SmallRng::seed_from_u64(0),
        )
        .unwrap();
        assert_eq!(result.first.move_name, "Protect");
        assert_eq!(result.first.damage, 0);
        assert!(!result.first.blocked);

        let second = result.second.unwrap();
        assert_eq!(second.move_name, "Vine Whip");
        assert_eq!(second.damage, 0);
        assert!(second.blocked);
        assert_eq!(
            protector_team.roster()[0].hp_curr(),
            u32::from(protector.stats.hp)
        );
    }

    #[test]
    fn execute_move_to_fainted_defender_returns_blocked_result() {
        let attacker = charizard();
        let defender = venusaur();
        let mut defender_team = team(&defender);
        defender_team.damage_active(u32::MAX).unwrap();
        let result = execute_move(
            &attacker,
            &team(&attacker),
            &defender,
            &mut defender_team,
            0,
            false,
            &mut SmallRng::seed_from_u64(0),
        )
        .unwrap();
        assert_eq!(result.damage, 0);
        assert!(result.blocked);
        assert_eq!(result.defender_hp_after, 0);
    }
}
