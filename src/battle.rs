pub use crate::damage::{DamageModifier, DamageResult, Fraction, calculate_damage};
use crate::info::{BattleError, Move, MoveCategory, Pokemon};
use crate::{Action, ActionError, BattleState, StateError, TeamState};

/// A single battle that owns its state and delegates turn resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Battle {
    state: BattleState,
}

impl Battle {
    pub fn new(state: BattleState) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &BattleState {
        &self.state
    }

    /// Resolves a turn using immutable calculation data for the corresponding roster slots.
    /// HP, selection, active slots, and move availability come from this battle's state.
    /// A failed turn leaves the state unchanged. An available move missing from the
    /// supplied roster returns `ActionError::Battle(BattleError::InvalidMoveIndex)`.
    pub fn play_turn_with_rosters(
        &mut self,
        player: &[Pokemon; 6],
        opponent: &[Pokemon; 6],
        player_action: Action,
        opponent_action: Action,
    ) -> Result<&BattleState, ActionError> {
        self.play_turn(
            player_action,
            opponent_action,
            |state, action, opponent_action| {
                resolve_turn(state, player, opponent, action, opponent_action)
            },
        )
    }

    pub fn play_turn(
        &mut self,
        player_action: Action,
        opponent_action: Action,
        resolve_turn: impl FnOnce(&mut BattleState, Action, Action) -> Result<(), ActionError>,
    ) -> Result<&BattleState, ActionError> {
        if self.state.terminated {
            return Err(ActionError::BattleTerminated);
        }

        let replacement_pending = self.state.player.slot_active().is_none()
            || self.state.opponent.slot_active().is_none();
        let mut next = self.state.clone();

        next.player.validate_action(player_action)?;
        next.opponent.validate_action(opponent_action)?;

        if let Action::Switch(slot) = player_action {
            next.player
                .switch_to(slot)
                .map_err(|_| ActionError::InvalidSwitch)?;
        }

        if let Action::Switch(slot) = opponent_action {
            next.opponent
                .switch_to(slot)
                .map_err(|_| ActionError::InvalidSwitch)?;
        }

        if !replacement_pending
            && (matches!(player_action, Action::Move(_))
                || matches!(opponent_action, Action::Move(_)))
        {
            resolve_turn(&mut next, player_action, opponent_action)?;
        }

        next.terminated =
            !next.player.has_available_selected() || !next.opponent.has_available_selected();
        self.state = next;
        Ok(&self.state)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attackresult {
    pub attacker: String,
    pub defender: String,
    pub move_name: String,
    pub damage: u16,
    pub effectiveness: f32,
    pub blocked: bool, // protect moves
    pub defender_hp_after: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Turnresult {
    pub first: Attackresult,
    pub second: Option<Attackresult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOrder {
    FirstPokemon,
    SecondPokemon,
}

/// Resolves attacks against the candidate owned by `Battle::play_turn`.
/// Actions are validated and switches applied before this resolver runs.
pub(crate) fn resolve_turn(
    state: &mut BattleState,
    player_roster: &[Pokemon; 6],
    opponent_roster: &[Pokemon; 6],
    action: Action,
    opponent_action: Action,
) -> Result<(), ActionError> {
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

    match (action, opponent_action) {
        (Action::Move(first), Action::Move(second)) => simulate_turn(
            player,
            &mut state.player,
            opponent,
            &mut state.opponent,
            first,
            second,
        )
        .map(|_| ()),
        (Action::Move(slot), Action::Switch(_)) => execute_move(
            player,
            &state.player,
            opponent,
            &mut state.opponent,
            slot,
            false,
        )
        .map(|_| ()),
        (Action::Switch(_), Action::Move(slot)) => execute_move(
            opponent,
            &state.opponent,
            player,
            &mut state.player,
            slot,
            false,
        )
        .map(|_| ()),
        _ => Err(ActionError::WrongPhase),
    }
}

fn determine_turn_order(
    first: &Pokemon,
    second: &Pokemon,
    first_slot: usize,
    second_slot: usize,
) -> Result<TurnOrder, BattleError> {
    validate_move_index(first, first_slot)?;
    validate_move_index(second, second_slot)?;

    let first_key = (first.moves[first_slot].priority, first.stats.speed);
    let second_key = (second.moves[second_slot].priority, second.stats.speed);

    Ok(match first_key.cmp(&second_key) {
        std::cmp::Ordering::Greater => TurnOrder::FirstPokemon,
        std::cmp::Ordering::Less => TurnOrder::SecondPokemon,
        std::cmp::Ordering::Equal if rand::random() => TurnOrder::FirstPokemon,
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
) -> Result<Turnresult, ActionError> {
    if first_team.slot_active().is_none() || second_team.slot_active().is_none() {
        return Err(ActionError::Battle(BattleError::FaintedPokemonCannotBattle));
    }

    let order = determine_turn_order(first, second, first_move, second_move)
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
    let first = execute_move(faster, faster_team, slower, slower_team, faster_move, false)?;
    let second = if slower_team.slot_active().is_none() {
        None
    } else {
        Some(execute_move(
            slower,
            slower_team,
            faster,
            faster_team,
            slower_move,
            is_protective_status_move(&faster.moves[faster_move]),
        )?)
    };
    Ok(Turnresult { first, second })
}

fn execute_move(
    attacker: &Pokemon,
    attacker_team: &TeamState,
    defender: &Pokemon,
    defender_team: &mut TeamState,
    move_index: usize,
    defender_is_protected: bool,
) -> Result<Attackresult, ActionError> {
    if attacker_team.slot_active().is_none() {
        return Err(ActionError::Battle(BattleError::FaintedPokemonCannotAttack));
    }

    let selected_move = attacker.moves.get(move_index).ok_or(ActionError::Battle(
        BattleError::InvalidMoveIndex { index: move_index },
    ))?;
    let target = defender_team.slot_active();
    let blocked = target.is_none() || (defender_is_protected && selected_move.power > 0);
    let result = if blocked {
        DamageResult {
            damage: 0,
            effectiveness: 1.0,
        }
    } else {
        let result = calculate_damage(attacker, defender, selected_move, None)
            .map_err(ActionError::Battle)?;
        defender_team
            .damage_active(u32::from(result.damage))
            .map_err(ActionError::InvalidState)?;
        result
    };

    Ok(Attackresult {
        attacker: attacker.entry.name.to_string(),
        defender: defender.entry.name.to_string(),
        move_name: selected_move.name.clone(),
        damage: result.damage,
        effectiveness: result.effectiveness,
        blocked,
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
    fn failed_resolution_discards_all_runtime_mutations() {
        let initial = state();
        let mut battle = Battle::new(initial.clone());

        assert_eq!(
            battle.play_turn(Action::Switch(1), Action::Move(0), |next, _, _| {
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
            battle.play_turn(Action::Switch(0), Action::Move(0), |_, _, _| unreachable!()),
            Err(ActionError::InvalidSwitch)
        );
        assert_eq!(
            battle.play_turn(Action::Move(0), Action::Move(1), |_, _, _| unreachable!()),
            Err(ActionError::UnavailableMove)
        );
        assert_eq!(
            battle.play_turn(Action::Move(0), Action::Move(0), |_, _, _| Err(
                ActionError::InvalidSwitch
            )),
            Err(ActionError::InvalidSwitch)
        );

        let state = battle
            .play_turn(
                Action::Move(1),
                Action::Move(2),
                |state, player, opponent| {
                    assert_eq!((player, opponent), (Action::Move(1), Action::Move(2)));

                    state.opponent.damage_active(1_000).unwrap();
                    state.opponent.switch_to(1).unwrap();
                    state.opponent.damage_active(1_000).unwrap();
                    state.opponent.switch_to(2).unwrap();
                    state.opponent.damage_active(1_000).unwrap();
                    Ok(())
                },
            )
            .unwrap();

        assert!(state.terminated);
        assert_eq!(
            battle.play_turn(Action::Move(0), Action::Move(0), |_, _, _| unreachable!()),
            Err(ActionError::BattleTerminated)
        );
    }

    #[test]
    fn non_terminal_faint_requires_switch_before_moves_resume() {
        let mut battle = Battle::new(state());

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                state.player.damage_active(1_000).unwrap();
                Ok(())
            })
            .unwrap();
        assert_eq!(state.player.slot_active(), None);
        assert!(!state.terminated);
        assert_eq!(
            state.legal_player_actions(),
            vec![Action::Switch(1), Action::Switch(2)]
        );
        assert_eq!(
            battle.play_turn(Action::Move(0), Action::Move(0), |_, _, _| unreachable!()),
            Err(ActionError::UnavailableMove)
        );

        let state = battle
            .play_turn(Action::Switch(1), Action::Move(0), |_, _, _| {
                panic!("forced replacements do not resolve moves")
            })
            .unwrap();
        assert_eq!(state.player.slot_active(), Some(1));

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                state.opponent.damage_active(1).unwrap();
                Ok(())
            })
            .unwrap();
        assert_eq!(state.opponent.roster()[0].hp_curr(), 99);
    }

    #[test]
    fn switch_only_turn_does_not_run_move_resolution() {
        let mut battle = Battle::new(state());

        let state = battle
            .play_turn(Action::Switch(1), Action::Switch(1), |_, _, _| {
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
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                state.opponent.damage_active(1_000).unwrap();
                Ok(())
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Switch(1), |_, _, _| {
                panic!("forced replacements do not resolve moves")
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                state.opponent.damage_active(1_000).unwrap();
                Ok(())
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Switch(2), |_, _, _| {
                panic!("forced replacements do not resolve moves")
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                state.opponent.damage_active(1_000).unwrap();
                Ok(())
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

        let order = determine_turn_order(&slower, &faster, 0, 0).unwrap();
        let result = simulate_turn(
            &slower,
            &mut team(&slower),
            &faster,
            &mut team(&faster),
            0,
            0,
        )
        .unwrap();
        assert_eq!(order, TurnOrder::FirstPokemon);
        assert_eq!(result.first.attacker, "Dragonite");
    }

    #[test]
    fn higher_priority_move_faster() {
        let slower = dragonite();
        let faster = charizard();

        let order = determine_turn_order(&slower, &faster, 0, 3).unwrap();
        let result = simulate_turn(
            &slower,
            &mut team(&slower),
            &faster,
            &mut team(&faster),
            0,
            3,
        )
        .unwrap();
        assert_eq!(order, TurnOrder::SecondPokemon);
        assert_eq!(result.first.attacker, "Charizard");
    }

    #[test]
    fn lower_priority_move_slower() {
        let slower = venusaur();
        let faster = dragonite();

        let order = determine_turn_order(&slower, &faster, 0, 3).unwrap();
        let result = simulate_turn(
            &slower,
            &mut team(&slower),
            &faster,
            &mut team(&faster),
            0,
            3,
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
        )
        .unwrap();
        assert_eq!(result.damage, 0);
        assert!(result.blocked);
        assert_eq!(result.defender_hp_after, 0);
    }
}
