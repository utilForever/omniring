pub use crate::Battle;
use crate::info::{BattleError, Move, MoveCategory, Pokemon, type_effectiveness_against};
use crate::{Action, ActionError, BattleState, StateError, TeamState};
use rand::{RngExt, SeedableRng, rngs::StdRng};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fraction {
    pub numerator: u64,
    pub denominator: u64,
}

impl Fraction {
    pub const ONE: Self = Self::new(1, 1);
    pub const THREE_HALVES: Self = Self::new(3, 2);

    pub const fn new(numerator: u64, denominator: u64) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    fn apply_to(self, value: u64) -> Result<u64, BattleError> {
        if self.denominator == 0 {
            return Err(BattleError::InvalidDamageModifier);
        }

        Ok((value * self.numerator) / self.denominator)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageModifier {
    pub power_modifier: Fraction,
    pub attack_modifier: Fraction,
    pub defense_modifier: Fraction,
    // Apply the spread modifier only when the move hits multiple targets
    // pub spread: Fraction,
    pub weather: Fraction,
    pub critical: Fraction,
    pub random_percent: u8,
    pub stab: Fraction,
    pub type_effectiveness: Fraction,
    pub burn: Fraction,
    pub other: Fraction,
}

impl Default for DamageModifier {
    fn default() -> Self {
        Self {
            power_modifier: Fraction::ONE,
            attack_modifier: Fraction::ONE,
            defense_modifier: Fraction::ONE,
            // spread: Fraction::ONE,
            weather: Fraction::ONE,
            critical: Fraction::ONE,
            random_percent: 100,
            stab: Fraction::ONE,
            type_effectiveness: Fraction::ONE,
            burn: Fraction::ONE,
            other: Fraction::ONE,
        }
    }
}

impl DamageModifier {
    pub fn with_raw_random_roll(mut self, seed: Option<u64>) -> Result<Self, BattleError> {
        let roll = match seed {
            Some(seed) => {
                let mut rng = StdRng::seed_from_u64(seed);
                rng.random_range(85..=100)
            }
            None => rand::random_range(85..=100),
        };

        self.random_percent = roll;
        Ok(self)
    }

    pub fn update_from_battle(
        &mut self,
        attacker: &Pokemon,
        defender: &Pokemon,
        selected_move: &Move,
    ) {
        let is_stab = attacker.has_type(selected_move.r#type);

        self.stab = if is_stab {
            Fraction::THREE_HALVES
        } else {
            Fraction::ONE
        };

        let effectiveness = type_effectiveness_against(selected_move.r#type, defender);
        self.type_effectiveness = type_effectiveness_modifier(effectiveness);

        // TODO: Add more formula as needed, such as battle state, weather conditions, etc.
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DamageResult {
    pub damage: u16,
    pub effectiveness: f32,
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

impl Battle {
    /// Resolves two moves transactionally using the owned runtime state.
    /// Uses the immutable rosters supplied to `Battle::with_rosters`.
    pub fn simulate_turn(
        &mut self,
        first_move_index: usize,
        second_move_index: usize,
    ) -> Result<Turnresult, ActionError> {
        let rosters = self.rosters()?;
        let mut result = None;

        self.play_turn(
            Action::Move(first_move_index),
            Action::Move(second_move_index),
            |state, _, _| {
                result = Some(Self::resolve_moves(
                    state,
                    &rosters.0,
                    &rosters.1,
                    first_move_index,
                    second_move_index,
                )?);
                Ok(())
            },
        )?;

        Ok(result.expect("validated move actions require two active Pokemon"))
    }

    pub fn determine_turn_order(
        &self,
        first_move_index: usize,
        second_move_index: usize,
    ) -> Result<TurnOrder, ActionError> {
        let rosters = self.rosters()?;
        determine_turn_order(
            self.state(),
            &rosters.0,
            &rosters.1,
            first_move_index,
            second_move_index,
        )
    }

    /// Transition callback used inside `play_turn`'s transaction, after switches.
    pub(crate) fn resolve_turn(
        state: &mut BattleState,
        player: &[Pokemon; 6],
        opponent: &[Pokemon; 6],
        action: Action,
        opponent_action: Action,
    ) -> Result<(), ActionError> {
        match (action, opponent_action) {
            (Action::Move(first), Action::Move(second)) => {
                Self::resolve_moves(state, player, opponent, first, second).map(|_| ())
            }
            (Action::Move(slot), Action::Switch(_)) => Self::execute_move(
                &state.player,
                &mut state.opponent,
                player,
                opponent,
                slot,
                false,
            )
            .map(|_| ()),
            (Action::Switch(_), Action::Move(slot)) => Self::execute_move(
                &state.opponent,
                &mut state.player,
                opponent,
                player,
                slot,
                false,
            )
            .map(|_| ()),
            _ => Err(ActionError::WrongPhase),
        }
    }

    fn resolve_moves(
        state: &mut BattleState,
        player: &[Pokemon; 6],
        opponent: &[Pokemon; 6],
        first_move_index: usize,
        second_move_index: usize,
    ) -> Result<Turnresult, ActionError> {
        let order =
            determine_turn_order(state, player, opponent, first_move_index, second_move_index)?;
        let (faster, slower, faster_move_index, slower_move_index) = match order {
            TurnOrder::FirstPokemon => (
                &mut state.player,
                &mut state.opponent,
                first_move_index,
                second_move_index,
            ),
            TurnOrder::SecondPokemon => (
                &mut state.opponent,
                &mut state.player,
                second_move_index,
                first_move_index,
            ),
        };
        let (faster_roster, slower_roster) = match order {
            TurnOrder::FirstPokemon => (player, opponent),
            TurnOrder::SecondPokemon => (opponent, player),
        };
        let faster_move = &faster_roster[faster.slot_active().unwrap()].moves[faster_move_index];

        let first_attack = Self::execute_move(
            faster,
            slower,
            faster_roster,
            slower_roster,
            faster_move_index,
            false,
        )?;
        let second_attack = if slower.slot_active().is_none() {
            None
        } else {
            let faster_is_protected = is_protective_status_move(faster_move);
            Some(Self::execute_move(
                slower,
                faster,
                slower_roster,
                faster_roster,
                slower_move_index,
                faster_is_protected,
            )?)
        };

        Ok(Turnresult {
            first: first_attack,
            second: second_attack,
        })
    }

    /// Applies one move directly to the defending team, clearing its active slot on fainting.
    /// For a complete turn (including rollback, termination, and counting), use `simulate_turn`
    /// or call this from a `play_turn` transition.
    pub fn execute_move(
        attacker_team: &TeamState,
        defender_team: &mut TeamState,
        attacker_roster: &[Pokemon; 6],
        defender_roster: &[Pokemon; 6],
        move_index: usize,
        defender_is_protected: bool,
    ) -> Result<Attackresult, ActionError> {
        attacker_team.validate_action(Action::Move(move_index))?;

        let attacker = active_pokemon(attacker_team, attacker_roster)?;
        let defender = active_pokemon(defender_team, defender_roster)?;
        let selected_move = validate_move_index(attacker, move_index)?;
        let defender_slot = defender_team.slot_active().unwrap();

        if defender_is_protected && selected_move.power > 0 {
            return Ok(Attackresult {
                attacker: attacker.entry.name.to_string(),
                defender: defender.entry.name.to_string(),
                move_name: selected_move.name.clone(),
                damage: 0,
                effectiveness: 1.0,
                blocked: true,
                defender_hp_after: defender_team.roster()[defender_slot].hp_curr(),
            });
        }

        let damage_result = Self::calculate_damage(attacker, defender, selected_move, None)
            .map_err(ActionError::Battle)?;

        defender_team
            .damage_active(u32::from(damage_result.damage))
            .map_err(ActionError::InvalidState)?;

        Ok(Attackresult {
            attacker: attacker.entry.name.to_string(),
            defender: defender.entry.name.to_string(),
            move_name: selected_move.name.clone(),
            damage: damage_result.damage,
            effectiveness: damage_result.effectiveness,
            blocked: false,
            defender_hp_after: defender_team.roster()[defender_slot].hp_curr(),
        })
    }

    pub fn calculate_damage(
        attacker: &Pokemon,
        defender: &Pokemon,
        selected_move: &Move,
        seed: Option<u64>,
    ) -> Result<DamageResult, BattleError> {
        if selected_move.category == MoveCategory::Status || selected_move.power == 0 {
            // TODO: Handle status moves that affect stats, conditions, etc.
            //       For now, we return 0 damage for status moves.
            return Ok(DamageResult {
                damage: 0,
                effectiveness: 1.0,
            });
        }

        let (attack, defense) = match selected_move.category {
            MoveCategory::Physical => (attacker.stats.attack, defender.stats.defense),
            MoveCategory::Special => (
                attacker.stats.special_attack,
                defender.stats.special_defense,
            ),
            MoveCategory::Status => unreachable!("status moves return before damage calculation"),
        };

        if defense == 0 {
            return Err(BattleError::ZeroDefenseStat);
        }

        let mut modifiers = DamageModifier::default().with_raw_random_roll(seed)?;
        modifiers.update_from_battle(attacker, defender, selected_move);

        if !(85..=100).contains(&modifiers.random_percent) {
            return Err(BattleError::InvalidDamageRandomPercent {
                percent: modifiers.random_percent,
            });
        }

        let power = modifiers
            .power_modifier
            .apply_to(u64::from(selected_move.power))?;
        let attack = modifiers.attack_modifier.apply_to(u64::from(attack))?;
        let defense = modifiers.defense_modifier.apply_to(u64::from(defense))?;

        // reference for damage formula: https://bulbapedia.bulbagarden.net/wiki/Damage#Damage_formula
        let level_factor = (u64::from(attacker.level) * 2) / 5 + 2;
        let mut damage = ((level_factor * power * attack) / (50 * defense)) + 2;

        // This modifier only applies in double battles when the move actually hits multiple targets
        // damage = modifiers.spread.apply_to(damage)?;

        damage = modifiers.weather.apply_to(damage)?;
        damage = modifiers.critical.apply_to(damage)?;
        damage = apply_random_percent(damage, modifiers.random_percent);

        damage = modifiers.stab.apply_to(damage)?;
        damage = modifiers.type_effectiveness.apply_to(damage)?;
        damage = modifiers.burn.apply_to(damage)?;
        // Apply all remaining final damage modifiers that do not belong to the
        // explicit calculation stages above, such as screens, abilities, and items.
        damage = modifiers.other.apply_to(damage)?;

        let effectiveness = type_effectiveness_against(selected_move.r#type, defender);
        let damage = if effectiveness == 0.0 {
            0
        } else {
            damage.max(1).min(u64::from(u16::MAX)) as u16
        };

        Ok(DamageResult {
            damage,
            effectiveness,
        })
    }
}

fn apply_random_percent(value: u64, percent: u8) -> u64 {
    (value * u64::from(percent)) / 100
}

fn active_pokemon<'a>(
    team: &TeamState,
    roster: &'a [Pokemon; 6],
) -> Result<&'a Pokemon, ActionError> {
    team.slot_active()
        .map(|slot| &roster[slot])
        .ok_or(ActionError::InvalidState(StateError::InvalidActiveSlot))
}

fn determine_turn_order(
    state: &BattleState,
    player: &[Pokemon; 6],
    opponent: &[Pokemon; 6],
    first_move_index: usize,
    second_move_index: usize,
) -> Result<TurnOrder, ActionError> {
    state.validate_player_action(Action::Move(first_move_index))?;
    state
        .opponent
        .validate_action(Action::Move(second_move_index))?;

    let first = active_pokemon(&state.player, player)?;
    let second = active_pokemon(&state.opponent, opponent)?;
    let first_move = validate_move_index(first, first_move_index)?;
    let second_move = validate_move_index(second, second_move_index)?;

    Ok(
        match (first_move.priority, first.stats.speed)
            .cmp(&(second_move.priority, second.stats.speed))
        {
            std::cmp::Ordering::Greater => TurnOrder::FirstPokemon,
            std::cmp::Ordering::Less => TurnOrder::SecondPokemon,
            std::cmp::Ordering::Equal if rand::random() => TurnOrder::FirstPokemon,
            std::cmp::Ordering::Equal => TurnOrder::SecondPokemon,
        },
    )
}

fn validate_move_index(pokemon: &Pokemon, move_index: usize) -> Result<&Move, ActionError> {
    pokemon
        .moves
        .get(move_index)
        .ok_or(ActionError::Battle(BattleError::InvalidMoveIndex {
            index: move_index,
        }))
}

fn is_protective_status_move(selected_move: &Move) -> bool {
    selected_move.category == MoveCategory::Status
        && matches!(selected_move.name.as_str(), "Protect" | "Detect")
}

fn type_effectiveness_modifier(effectiveness: f32) -> Fraction {
    if effectiveness == 0.0 {
        Fraction::new(0, 1)
    } else if effectiveness == 0.5 {
        Fraction::new(1, 2)
    } else if effectiveness == 0.25 {
        Fraction::new(1, 4)
    } else if effectiveness == 2.0 {
        Fraction::new(2, 1)
    } else if effectiveness == 4.0 {
        Fraction::new(4, 1)
    } else {
        Fraction::ONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PokemonState;
    use crate::info::{Nature, PokemonType, StatPoints};
    use crate::pokedex::{build_pokemon_from_pokedex, find_pokemon};

    fn battle_from(p1: Pokemon, p2: Pokemon) -> (Battle, [Pokemon; 6], [Pokemon; 6]) {
        let team = |pokemon: &Pokemon| {
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
                Some(0),
            )
            .unwrap()
        };
        let state = BattleState {
            player: team(&p1),
            opponent: team(&p2),
            terminated: false,
            turn_count: 1,
        };

        let player = std::array::from_fn(|_| p1.clone());
        let opponent = std::array::from_fn(|_| p2.clone());
        let battle = Battle::with_rosters(state, player.clone(), opponent.clone()).unwrap();
        (battle, player, opponent)
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
    fn seeded_raw_random_roll_samples_percent_directly() {
        let first = DamageModifier::default()
            .with_raw_random_roll(Some(1))
            .unwrap()
            .random_percent;
        let second = DamageModifier::default()
            .with_raw_random_roll(Some(1))
            .unwrap()
            .random_percent;

        assert_eq!(first, second);
        assert_eq!(first, 98);
    }

    #[test]
    fn random_percent_truncates_fractional_damage() {
        assert_eq!(apply_random_percent(10, 85), 8);
        assert_eq!(apply_random_percent(3, 85), 2);
    }

    #[test]
    fn damage_applies_stab_and_type_effectiveness() {
        let attacker = charizard();
        let cases = [
            ("neutral", PokemonType::Dragon, pokemon("Venusaur"), 38, 1.0),
            ("stab", PokemonType::Flying, pokemon("Gengar"), 70, 1.0),
            ("resisted", PokemonType::Fire, pokemon("Dragonite"), 28, 0.5),
            (
                "super-effective",
                PokemonType::Fire,
                pokemon("Venusaur"),
                114,
                2.0,
            ),
            (
                "dual-type",
                PokemonType::Ice,
                pokemon("Dragonite"),
                152,
                4.0,
            ),
            ("immune", PokemonType::Normal, pokemon("Gengar"), 0, 0.0),
        ];

        for (case, move_type, defender, expected_damage, expected_effectiveness) in cases {
            let selected_move = Move::new(
                "Test Move",
                move_type,
                MoveCategory::Special,
                80,
                Some(100),
                0,
            );

            let result =
                Battle::calculate_damage(&attacker, &defender, &selected_move, Some(1)).unwrap();
            assert_eq!(result.damage, expected_damage, "{case}");
            assert_eq!(result.effectiveness, expected_effectiveness, "{case}");
        }
    }

    #[test]
    fn non_immune_moves_deal_at_least_one_damage() {
        let attacker = pokemon("Venusaur");
        let defender = pokemon("Charizard");
        let selected_move = Move::new(
            "Test Move",
            PokemonType::Grass,
            MoveCategory::Special,
            1,
            Some(100),
            0,
        );

        let result =
            Battle::calculate_damage(&attacker, &defender, &selected_move, Some(1)).unwrap();
        assert_eq!(result.damage, 1);
        assert_eq!(result.effectiveness, 0.25);
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

        let (battle, player, opponent) = battle_from(attacker, defender);
        let mut state = battle.state().clone();

        let result = Battle::execute_move(
            &state.player,
            &mut state.opponent,
            &player,
            &opponent,
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
    fn later_roster_edits_cannot_change_an_existing_battle() {
        let (mut battle, mut player, _) = battle_from(charizard(), charizard());
        battle.simulate_turn(3, 3).unwrap();
        player[0].moves.truncate(1);
        assert!(
            battle
                .state()
                .legal_player_actions()
                .contains(&Action::Move(3))
        );
        let result = battle.simulate_turn(3, 3).unwrap();
        assert_eq!(result.first.move_name, "Protect");
        assert_eq!(result.second.unwrap().move_name, "Protect");
        assert_eq!(battle.state().turn_count, 3);
    }

    #[test]
    fn bound_rosters_validate_move_counts_and_masks_on_both_teams() {
        let (battle, player, opponent) = battle_from(charizard(), charizard());
        for player_side in [true, false] {
            for slot in [0, 5] {
                for count in [0, 1, 5] {
                    let mut rosters = (player.clone(), opponent.clone());
                    let pokemon = if player_side {
                        &mut rosters.0[slot]
                    } else {
                        &mut rosters.1[slot]
                    };
                    pokemon.moves.resize(count, pokemon.moves[0].clone());
                    let expected = if count == 1 {
                        ActionError::InvalidState(StateError::InvalidMoveAvailability)
                    } else {
                        ActionError::Battle(BattleError::InvalidMoveCount { count })
                    };
                    assert_eq!(
                        Battle::with_rosters(battle.state().clone(), rosters.0, rosters.1),
                        Err(expected)
                    );
                }
            }
        }
    }

    #[test]
    fn bound_rosters_reject_transitions_that_enable_missing_moves() {
        let mut pokemon = charizard();
        pokemon.moves.truncate(1);
        let (mut battle, _, _) = battle_from(pokemon.clone(), pokemon);
        let previous = battle.state().clone();
        for player_side in [true, false] {
            assert_eq!(
                battle.play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                    let team = if player_side {
                        &mut state.player
                    } else {
                        &mut state.opponent
                    };
                    team.damage_active(1).unwrap();
                    let mut roster = team.roster().clone();
                    roster[5].move_availability[3] = true;
                    *team = TeamState::new(roster, *team.selected(), Some(1)).unwrap();
                    Ok(())
                }),
                Err(ActionError::InvalidState(
                    StateError::InvalidMoveAvailability
                )),
            );
            assert_eq!(battle.state(), &previous);
        }
    }

    #[test]
    fn simulation_requires_bound_rosters() {
        let (configured, _, _) = battle_from(charizard(), charizard());
        let mut battle = Battle::new(configured.state().clone());
        assert_eq!(battle.simulate_turn(0, 0), Err(ActionError::MissingRosters));
        assert_eq!(
            battle.determine_turn_order(0, 0),
            Err(ActionError::MissingRosters)
        );
        assert_eq!(battle.state(), configured.state());
    }

    #[test]
    fn failed_second_attack_leaves_the_entire_turn_unchanged() {
        let mut attacker = charizard();
        attacker.stats.defense = 0;

        let mut defender = venusaur();
        defender.stats.hp = u16::MAX;
        defender.current_hp = u16::MAX;

        let (mut battle, _, _) = battle_from(attacker, defender);
        let previous = battle.state().clone();
        assert_eq!(
            battle.simulate_turn(0, 0),
            Err(ActionError::Battle(BattleError::ZeroDefenseStat))
        );
        assert_eq!(battle.state(), &previous);
    }

    #[test]
    fn runtime_hp_and_move_availability_drive_core_turns_and_observations() {
        let (battle, mut player, mut opponent) = battle_from(charizard(), venusaur());
        let mut state = battle.state().clone();

        let mut roster = state.player.roster().clone();
        roster[0].move_availability[0] = false;

        state.player = TeamState::new(roster, *state.player.selected(), Some(0)).unwrap();
        state.player.damage_active(7).unwrap();
        state
            .opponent
            .damage_active(state.opponent.roster()[0].hp_curr() - 1)
            .unwrap();

        // The setup Pokemon's HP must never be read or written during resolution.
        for pokemon in player.iter_mut().chain(&mut opponent) {
            pokemon.current_hp = 0;
        }

        let setup = (player.clone(), opponent.clone());
        let previous = state.clone();
        let revealed = [true, false, false, false, false, false];

        let mut observation = state.observation(revealed).unwrap();
        assert_eq!(observation, state.observation(revealed).unwrap());
        assert!(!observation.player.roster()[0].move_availability[0]);

        observation.player.damage_active(u32::MAX).unwrap();
        assert_eq!(state, previous);

        let mut battle = Battle::with_rosters(state, player.clone(), opponent.clone()).unwrap();
        assert_eq!(
            battle.simulate_turn(0, 0),
            Err(ActionError::UnavailableMove)
        );
        assert_eq!(battle.state(), &previous);

        let protected = battle.simulate_turn(3, 0).unwrap();
        assert!(protected.second.unwrap().blocked);
        assert_eq!(battle.state().player, previous.player);
        assert_eq!(battle.state().opponent, previous.opponent);
        assert_eq!(battle.state().turn_count, 2);

        let knockout = battle.simulate_turn(1, 0).unwrap();
        assert!(knockout.second.is_none());
        assert_eq!(knockout.first.defender_hp_after, 0);
        assert_eq!(battle.state().opponent.roster()[0].hp_curr(), 0);
        assert_eq!(battle.state().opponent.slot_active(), None);
        assert_eq!(battle.state().turn_count, 3);

        battle
            .play_turn(
                Action::Switch(1),
                Action::Switch(1),
                |_, _, _| unreachable!(),
            )
            .unwrap();
        assert_eq!(battle.state().turn_count, 3);
        assert_eq!(battle.state().player.slot_active(), Some(1));
        assert_eq!(battle.state().opponent.slot_active(), Some(1));
        assert_eq!((player, opponent), setup);
    }

    #[test]
    fn priority_move_can_attack_before_faster_pokemon() {
        let slower = dragonite();
        let faster = charizard();

        let (mut battle, _, _) = battle_from(slower, faster);
        let order = battle.determine_turn_order(0, 0).unwrap();

        let result = battle.simulate_turn(0, 0).unwrap();
        assert_eq!(order, TurnOrder::FirstPokemon);
        assert_eq!(result.first.attacker, "Dragonite");
    }

    #[test]
    fn higher_priority_move_faster() {
        let slower = dragonite();
        let faster = charizard();

        let (mut battle, _, _) = battle_from(slower, faster);
        let order = battle.determine_turn_order(0, 3).unwrap();

        let result = battle.simulate_turn(0, 3).unwrap();
        assert_eq!(order, TurnOrder::SecondPokemon);
        assert_eq!(result.first.attacker, "Charizard");
    }

    #[test]
    fn lower_priority_move_slower() {
        let slower = venusaur();
        let faster = dragonite();

        let (mut battle, _, _) = battle_from(slower, faster);
        let order = battle.determine_turn_order(0, 3).unwrap();

        let result = battle.simulate_turn(0, 3).unwrap();
        assert_eq!(order, TurnOrder::FirstPokemon);
        assert_eq!(result.first.attacker, "Venusaur");
    }

    #[test]
    fn faster_pokemon_attacks_first_and_fainting_stops_counterattack() {
        let attacker = charizard();
        let defender = venusaur();

        let (mut battle, player, opponent) = battle_from(attacker.clone(), defender.clone());
        let mut state = battle.state().clone();
        state
            .opponent
            .damage_active(state.opponent.roster()[0].hp_curr() - 10)
            .unwrap();
        battle = Battle::with_rosters(state, player.clone(), opponent.clone()).unwrap();

        let result = battle.simulate_turn(0, 0).unwrap();
        assert_eq!(result.first.attacker, "Charizard");
        assert!(result.second.is_none());
    }

    #[test]
    fn second_input_can_act_first_when_it_is_faster() {
        let slower = venusaur();
        let faster = charizard();

        let (mut battle, _, _) = battle_from(slower, faster);
        let result = battle.simulate_turn(1, 0).unwrap();
        assert_eq!(result.first.attacker, "Charizard");
    }

    #[test]
    fn invalid_move_index_returns_error() {
        let attacker = charizard();
        let defender = venusaur();

        let (mut battle, _, _) = battle_from(attacker, defender);
        let result = battle.simulate_turn(4, 0);
        assert_eq!(result, Err(ActionError::UnavailableMove));
    }

    #[test]
    fn turn_cannot_start_with_fainted_pokemon() {
        let attacker = charizard();
        let defender = venusaur();

        let (mut battle, player, opponent) = battle_from(attacker.clone(), defender.clone());

        let mut state = battle.state().clone();
        state.player.damage_active(u32::MAX).unwrap();

        battle = Battle::with_rosters(state, player.clone(), opponent.clone()).unwrap();

        let result = battle.simulate_turn(0, 0);
        assert_eq!(result, Err(ActionError::UnavailableMove));
    }

    #[test]
    fn protect_blocks_the_second_damage_move() {
        let protector = charizard();
        let attacker = venusaur();

        let (mut battle, player, _) = battle_from(protector, attacker);
        let result = battle.simulate_turn(3, 0).unwrap();
        assert_eq!(result.first.move_name, "Protect");
        assert_eq!(result.first.damage, 0);
        assert!(!result.first.blocked);

        let second = result.second.unwrap();
        assert_eq!(second.move_name, "Vine Whip");
        assert_eq!(second.damage, 0);
        assert!(second.blocked);
        assert_eq!(
            battle.state().player.roster()[0].hp_curr(),
            u32::from(player[0].stats.hp)
        );
    }

    #[test]
    fn execute_move_requires_an_active_defender() {
        let attacker = charizard();
        let defender = venusaur();
        let (battle, player, opponent) = battle_from(attacker, defender);

        let mut state = battle.state().clone();
        state.opponent.damage_active(u32::MAX).unwrap();

        let previous = state.clone();
        assert_eq!(
            Battle::execute_move(
                &state.player,
                &mut state.opponent,
                &player,
                &opponent,
                0,
                false
            ),
            Err(ActionError::InvalidState(StateError::InvalidActiveSlot)),
        );
        assert_eq!(state, previous);
    }
}
