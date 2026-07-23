use crate::info::{BattleError, Move, MoveCategory, Pokemon, type_effectiveness};

pub const DAMAGE_RANDOM_RAW_MIN: u8 = 217;
pub const DAMAGE_RANDOM_RAW_MAX: u8 = 255;
pub const DAMAGE_RANDOM_PERCENT_TABLE: [u8; 39] = [
    85, 85, 85, 86, 86, 87, 87, 87, 88, 88, 89, 89, 89, 90, 90, 90, 91, 91, 92, 92, 92, 93, 93, 94,
    94, 94, 95, 95, 96, 96, 96, 97, 97, 98, 98, 98, 99, 99, 100,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageModifier {
    pub numerator: u32,
    pub denominator: u32,
}

impl DamageModifier {
    pub const ONE: Self = Self::new(1, 1);
    pub const THREE_HALVES: Self = Self::new(3, 2);

    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    fn apply_to(self, value: u64) -> Result<u64, BattleError> {
        if self.denominator == 0 {
            return Err(BattleError::InvalidDamageModifier);
        }

        Ok((value * u64::from(self.numerator)) / u64::from(self.denominator))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageModifiers {
    pub power_modifier: DamageModifier,
    pub attack_modifier: DamageModifier,
    pub defense_modifier: DamageModifier,
    pub mod1: DamageModifier,
    pub critical: DamageModifier,
    pub mod2: DamageModifier,
    pub random_percent: u8,
    pub mod3: DamageModifier,
}

impl Default for DamageModifiers {
    fn default() -> Self {
        Self {
            power_modifier: DamageModifier::ONE,
            attack_modifier: DamageModifier::ONE,
            defense_modifier: DamageModifier::ONE,
            mod1: DamageModifier::ONE,
            critical: DamageModifier::ONE,
            mod2: DamageModifier::ONE,
            random_percent: 100,
            mod3: DamageModifier::ONE,
        }
    }
}

impl DamageModifiers {
    pub fn with_raw_random_roll(mut self) -> Result<Self, BattleError> {
        let raw_roll = rand::random_range(DAMAGE_RANDOM_RAW_MIN..=DAMAGE_RANDOM_RAW_MAX);
        self.random_percent = damage_random_percent_from_raw(raw_roll)?;
        Ok(self)
    }
}

pub fn damage_random_percent_from_raw(raw_roll: u8) -> Result<u8, BattleError> {
    if !(DAMAGE_RANDOM_RAW_MIN..=DAMAGE_RANDOM_RAW_MAX).contains(&raw_roll) {
        return Err(BattleError::InvalidDamageRandomRawRoll { raw_roll });
    }

    let index = usize::from(raw_roll - DAMAGE_RANDOM_RAW_MIN);
    Ok(DAMAGE_RANDOM_PERCENT_TABLE[index])
}

#[derive(Debug, Clone, PartialEq)]
pub struct DamageResult {
    pub damage: u16,
    pub effectiveness: f32,
    pub stab: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttackOutcome {
    pub attacker: String,
    pub defender: String,
    pub move_name: String,
    pub damage: u16,
    pub effectiveness: f32,
    pub blocked: bool,
    pub defender_hp_after: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurnOutcome {
    // if second fainted, second is None
    pub first: Option<AttackOutcome>,
    pub second: Option<AttackOutcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOrder {
    FirstPokemon,
    SecondPokemon,
}

pub fn calculate_damage(
    attacker: &Pokemon,
    defender: &Pokemon,
    selected_move: &Move,
) -> Result<DamageResult, BattleError> {
    calculate_damage_with_modifiers(
        attacker,
        defender,
        selected_move,
        DamageModifiers::default(),
    )
}

pub fn calculate_damage_with_modifiers(
    attacker: &Pokemon,
    defender: &Pokemon,
    selected_move: &Move,
    modifiers: DamageModifiers,
) -> Result<DamageResult, BattleError> {
    if selected_move.category == MoveCategory::Status || selected_move.power == 0 {
        return Ok(DamageResult {
            damage: 0,
            effectiveness: 1.0,
            stab: false,
        });
    }

    let (attack, defense) = match selected_move.category {
        MoveCategory::Physical => (attacker.stats.attack, defender.stats.defense),
        MoveCategory::Special => (attacker.stats.special_attack, defender.stats.special_defense),
        MoveCategory::Status => unreachable!("status moves return before damage calculation"),
    };
    let power = modifiers.power_modifier.apply_to(u64::from(selected_move.power))?;
    let attack = modifiers.attack_modifier.apply_to(u64::from(attack))?;
    let defense = modifiers.defense_modifier.apply_to(u64::from(defense))?;
    if defense == 0 {
        return Err(BattleError::ZeroDefenseStat);
    }
    if !(85..=100).contains(&modifiers.random_percent) {
        return Err(BattleError::InvalidDamageRandomPercent {
            percent: modifiers.random_percent,
        });
    }

    let primary_effectiveness = type_effectiveness(selected_move.move_type, defender.primary_type);
    let secondary_effectiveness = defender.secondary_type.map_or(1.0, |defense_type| {
        type_effectiveness(selected_move.move_type, defense_type)
    });
    let effectiveness = primary_effectiveness * secondary_effectiveness;

    // Discard the decimal part before each calc
    let mut damage = u64::from(attacker.level);
    damage *= 2;
    damage /= 5;
    damage += 2;
    damage *= power;
    damage *= attack;
    damage /= 50;
    damage /= defense;
    damage = modifiers.mod1.apply_to(damage)?;
    damage += 2;
    damage = modifiers.critical.apply_to(damage)?;
    damage = modifiers.mod2.apply_to(damage)?;
    damage *= u64::from(modifiers.random_percent);
    damage /= 100;

    let stab = attacker.has_type(selected_move.move_type);
    if stab {
        damage = DamageModifier::THREE_HALVES.apply_to(damage)?;
    }
    damage = type_effectiveness_modifier(primary_effectiveness).apply_to(damage)?;
    damage = type_effectiveness_modifier(secondary_effectiveness).apply_to(damage)?;
    damage = modifiers.mod3.apply_to(damage)?;

    let damage = if effectiveness == 0.0 {
        0
    } else {
        damage.max(1).min(u64::from(u16::MAX)) as u16
    };

    Ok(DamageResult {
        damage,
        effectiveness,
        stab,
    })
}

fn type_effectiveness_modifier(effectiveness: f32) -> DamageModifier {
    if effectiveness == 0.0 {
        DamageModifier::new(0, 1)
    } else if effectiveness == 0.5 {
        DamageModifier::new(1, 2)
    } else if effectiveness == 2.0 {
        DamageModifier::new(2, 1)
    } else {
        DamageModifier::ONE
    }
}

pub fn execute_attack(
    attacker: &Pokemon,
    defender: &mut Pokemon,
    move_index: usize,
) -> Result<AttackOutcome, BattleError> {
    execute_move(attacker, defender, move_index, false)
}

fn execute_move(
    attacker: &Pokemon,
    defender: &mut Pokemon,
    move_index: usize,
    defender_is_protected: bool,
) -> Result<AttackOutcome, BattleError> {
    if attacker.is_fainted() {
        return Err(BattleError::FaintedPokemonCannotAttack);
    }
    if defender.is_fainted() {
        return Err(BattleError::FaintedPokemonCannotBeTargeted);
    }

    let selected_move = attacker
        .moves
        .get(move_index)
        .ok_or(BattleError::InvalidMoveIndex { index: move_index })?;

    // if defender_is_protected && selected_move.effect == MoveEffect::Damage {
    //     return Ok(AttackOutcome {
    //         attacker: attacker.name.clone(),
    //         defender: defender.name.clone(),
    //         move_name: selected_move.name.clone(),
    //         damage: 0,
    //         effectiveness: 1.0,
    //         blocked: true,
    //         defender_hp_after: defender.current_hp,
    //     });
    // }

    let damage_result = calculate_damage(attacker, defender, selected_move)?;
    defender.current_hp = defender.current_hp.saturating_sub(damage_result.damage);

    Ok(AttackOutcome {
        attacker: attacker.name.clone(),
        defender: defender.name.clone(),
        move_name: selected_move.name.clone(),
        damage: damage_result.damage,
        effectiveness: damage_result.effectiveness,
        blocked: false,
        defender_hp_after: defender.current_hp,
    })
}

pub fn simulate_turn(
    first_pokemon: &mut Pokemon,
    first_move_index: usize,
    second_pokemon: &mut Pokemon,
    second_move_index: usize,
) -> Result<TurnOutcome, BattleError> {
    if first_pokemon.is_fainted() {
        return Err(BattleError::FaintedPokemonCannotBattle);
    }
    if second_pokemon.is_fainted() {
        return Err(BattleError::FaintedPokemonCannotBattle);
    }

    match determine_turn_order(
        first_pokemon,
        first_move_index,
        second_pokemon,
        second_move_index,
    )? {
        TurnOrder::FirstPokemon => resolve_turn_order(
            first_pokemon,
            first_move_index,
            second_pokemon,
            second_move_index,
        ),
        TurnOrder::SecondPokemon => resolve_turn_order(
            second_pokemon,
            second_move_index,
            first_pokemon,
            first_move_index,
        ),
    }
}

pub fn determine_turn_order(
    first_pokemon: &Pokemon,
    first_move_index: usize,
    second_pokemon: &Pokemon,
    second_move_index: usize,
) -> Result<TurnOrder, BattleError> {
    validate_move_index(first_pokemon, first_move_index)?;
    validate_move_index(second_pokemon, second_move_index)?;

    let first_move = &first_pokemon.moves[first_move_index];
    let second_move = &second_pokemon.moves[second_move_index];

    if first_move.priority > second_move.priority {
        Ok(TurnOrder::FirstPokemon)
    } else if first_move.priority < second_move.priority {
        Ok(TurnOrder::SecondPokemon)
    } else if first_pokemon.stats.speed >= second_pokemon.stats.speed {
        Ok(TurnOrder::FirstPokemon)
    } else {
        Ok(TurnOrder::SecondPokemon)
    }
}

fn resolve_turn_order(
    faster: &mut Pokemon,
    faster_move_index: usize,
    slower: &mut Pokemon,
    slower_move_index: usize,
) -> Result<TurnOutcome, BattleError> {
    let faster_move = faster.moves[faster_move_index].clone();
    let first = execute_move(faster, slower, faster_move_index, false)?;
    let second = if slower.is_fainted() {
        None
    } else {
        Some(execute_move(slower, faster, slower_move_index, false)?)
        
        // let faster_is_protected = faster_move.effect == MoveEffect::ProtectUser;
        // Some(execute_move(
        //     slower,
        //     faster,
        //     slower_move_index,
        //     faster_is_protected,
        // )?)
    };

    Ok(TurnOutcome {
        first: Some(first),
        second,
    })
}

fn validate_move_index(pokemon: &Pokemon, move_index: usize) -> Result<(), BattleError> {
    pokemon
        .moves
        .get(move_index)
        .map(|_| ())
        .ok_or(BattleError::InvalidMoveIndex { index: move_index })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::info::{Nature, StatPoints};
    use crate::pokedex::build_pokemon_from_pokedex;

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
            Nature::neutral(),
            ["Flamethrower", "Air Slash", "Dragon Claw", "Protect"],
        )
        .unwrap()
    }

    fn venusaur() -> Pokemon {
        build_pokemon_from_pokedex(
            "Venusaur",
            50,
            valid_stat_points(),
            Nature::neutral(),
            ["Vine Whip", "Razor Leaf", "Sleep Powder", "Seed Bomb"],
        )
        .unwrap()
    }

    fn lucario() -> Pokemon {
        build_pokemon_from_pokedex(
            "Lucario",
            50,
            valid_stat_points(),
            Nature::neutral(),
            ["Aura Sphere", "Quick Attack", "Detect", "Close Combat"],
        )
        .unwrap()
    }

    fn dragonite() -> Pokemon {
        build_pokemon_from_pokedex(
            "Dragonite",
            50,
            valid_stat_points(),
            Nature::neutral(),
            ["Extreme Speed", "Fire Punch", "Thunder Punch", "Rock Slide"],
        )
        .unwrap()
    }

    #[test]
    fn calculates_stab_and_super_effective_damage() {
        let attacker = charizard();
        let defender = venusaur();

        let result = calculate_damage(&attacker, &defender, &attacker.moves[0]).unwrap();

        assert_eq!(result.effectiveness, 2.0);
        assert!(result.stab);
        assert_eq!(result.damage, 134);
    }

    #[test]
    fn applies_modifiers_in_official_left_to_right_order() {
        let attacker = charizard();
        let defender = venusaur();
        let modifiers = DamageModifiers {
            critical: DamageModifier::THREE_HALVES,
            random_percent: 85,
            ..DamageModifiers::default()
        };

        let result =
            calculate_damage_with_modifiers(&attacker, &defender, &attacker.moves[0], modifiers)
                .unwrap();

        assert_eq!(result.damage, 168);
    }

    #[test]
    fn maps_raw_random_rolls_to_official_damage_percent_distribution() {
        let expected_counts = [
            (85, 3),
            (86, 2),
            (87, 3),
            (88, 2),
            (89, 3),
            (90, 3),
            (91, 2),
            (92, 3),
            (93, 2),
            (94, 3),
            (95, 2),
            (96, 3),
            (97, 2),
            (98, 3),
            (99, 2),
            (100, 1),
        ];

        for (percent, expected_count) in expected_counts {
            let actual_count = DAMAGE_RANDOM_PERCENT_TABLE
                .iter()
                .filter(|&&candidate| candidate == percent)
                .count();

            assert_eq!(actual_count, expected_count);
        }

        assert_eq!(damage_random_percent_from_raw(217), Ok(85));
        assert_eq!(damage_random_percent_from_raw(255), Ok(100));
    }

    #[test]
    fn modifiers_can_be_created_from_raw_random_roll() {
        let modifiers = DamageModifiers::default()
            .with_raw_random_roll(217)
            .unwrap();

        assert_eq!(modifiers.random_percent, 85);
    }

    #[test]
    fn status_moves_deal_no_damage() {
        let attacker = charizard();
        let defender = venusaur();

        let result = calculate_damage(&attacker, &defender, &attacker.moves[3]).unwrap();

        assert_eq!(result.damage, 0);
        assert_eq!(result.effectiveness, 1.0);
        assert!(!result.stab);
    }

    #[test]
    fn priority_move_can_attack_before_faster_pokemon() {
        let mut slower = dragonite();
        let mut faster = charizard();

        let order = determine_turn_order(&slower, 0, &faster, 0).unwrap();
        let outcome = simulate_turn(&mut slower, 0, &mut faster, 0).unwrap();

        assert_eq!(order, TurnOrder::FirstPokemon);
        assert_eq!(outcome.first.unwrap().attacker, "Dragonite");
    }

    #[test]
    fn faster_pokemon_attacks_first_and_fainting_stops_counterattack() {
        let mut attacker = charizard();
        let mut defender = venusaur();
        defender.current_hp = 10;

        let outcome = simulate_turn(&mut attacker, 0, &mut defender, 0).unwrap();

        assert_eq!(outcome.first.unwrap().attacker, "Charizard");
        assert!(outcome.second.is_none());
        assert_eq!(defender.current_hp, 0);
    }

    #[test]
    fn second_input_can_act_first_when_it_is_faster() {
        let mut slower = venusaur();
        let mut faster = charizard();

        let outcome = simulate_turn(&mut slower, 1, &mut faster, 0).unwrap();

        assert_eq!(outcome.first.unwrap().attacker, "Charizard");
    }

    #[test]
    fn invalid_move_index_returns_error() {
        let mut attacker = charizard();
        let mut defender = venusaur();

        let result = simulate_turn(&mut attacker, 4, &mut defender, 0);

        assert_eq!(result, Err(BattleError::InvalidMoveIndex { index: 4 }));
    }

    #[test]
    fn turn_cannot_start_with_fainted_pokemon() {
        let mut attacker = charizard();
        let mut defender = venusaur();
        attacker.current_hp = 0;

        let result = simulate_turn(&mut attacker, 0, &mut defender, 0);

        assert_eq!(result, Err(BattleError::FaintedPokemonCannotBattle));
    }

    #[test]
    fn cannot_attack_an_already_fainted_target() {
        let attacker = charizard();
        let mut defender = venusaur();
        defender.current_hp = 0;

        let result = execute_attack(&attacker, &mut defender, 0);

        assert_eq!(result, Err(BattleError::FaintedPokemonCannotBeTargeted));
    }

    #[test]
    fn zero_defense_stat_returns_error_instead_of_dividing_by_zero() {
        let attacker = charizard();
        let mut defender = venusaur();
        defender.stats.special_defense = 0;

        let result = calculate_damage(&attacker, &defender, &attacker.moves[1]);

        assert_eq!(result, Err(BattleError::ZeroDefenseStat));
    }

    #[test]
    fn dual_type_effectiveness_can_reach_four_times() {
        let attacker = dragonite();
        let defender = charizard();

        let result = calculate_damage(&attacker, &defender, &attacker.moves[3]).unwrap();

        assert_eq!(result.effectiveness, 4.0);
    }

    #[test]
    fn protect_user_status_move_blocks_incoming_damage() {
        let mut protector = charizard();
        let mut attacker = venusaur();

        let outcome = simulate_turn(&mut protector, 3, &mut attacker, 0).unwrap();

        assert_eq!(outcome.first.as_ref().unwrap().move_name, "Protect");
        assert_eq!(outcome.first.as_ref().unwrap().damage, 0);
        assert!(!outcome.first.as_ref().unwrap().blocked);
        assert!(outcome.second.as_ref().unwrap().blocked);
        assert_eq!(protector.current_hp, protector.stats.hp);
    }

    #[test]
    fn detect_user_status_move_blocks_incoming_damage() {
        let mut protector = lucario();
        let mut attacker = dragonite();

        let outcome = simulate_turn(&mut protector, 2, &mut attacker, 1).unwrap();

        assert_eq!(outcome.first.as_ref().unwrap().move_name, "Detect");
        assert!(outcome.second.as_ref().unwrap().blocked);
        assert_eq!(protector.current_hp, protector.stats.hp);
    }
}
