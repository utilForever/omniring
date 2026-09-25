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

/// Calculates damage without reading or changing either Pokemon's runtime HP.
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

fn apply_random_percent(value: u64, percent: u8) -> u64 {
    (value * u64::from(percent)) / 100
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

            let result = calculate_damage(&attacker, &defender, &selected_move, Some(1)).unwrap();
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

        let result = calculate_damage(&attacker, &defender, &selected_move, Some(1)).unwrap();
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
