use crate::info::{BattleError, Move, MoveCategory, Pokemon, type_effectiveness_against};

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
    pub stab: bool,
    pub defender_hp_after: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurnOutcome {
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
) -> DamageResult {
    if selected_move.category == MoveCategory::Status || selected_move.power == 0 {
        return DamageResult {
            damage: 0,
            effectiveness: 1.0,
            stab: false,
        };
    }

    let (attack, defense) = match selected_move.category {
        MoveCategory::Physical => (attacker.stats.attack, defender.stats.defense),
        MoveCategory::Special => (
            attacker.stats.special_attack,
            defender.stats.special_defense,
        ),
        MoveCategory::Status => unreachable!("status moves return before damage calculation"),
    };

    let level_factor = (2 * u32::from(attacker.level)) / 5 + 2;
    let base_damage = (((level_factor * u32::from(selected_move.power) * u32::from(attack))
        / u32::from(defense))
        / 50)
        + 2;
    let stab = attacker.has_type(selected_move.move_type);
    let effectiveness = type_effectiveness_against(selected_move.move_type, defender);
    let stab_modifier = if stab { 1.5 } else { 1.0 };
    let modified_damage = (base_damage as f32 * stab_modifier * effectiveness).floor() as u16;
    let damage = if effectiveness == 0.0 {
        0
    } else {
        modified_damage.max(1)
    };

    DamageResult {
        damage,
        effectiveness,
        stab,
    }
}

pub fn execute_attack(
    attacker: &Pokemon,
    defender: &mut Pokemon,
    move_index: usize,
) -> Result<AttackOutcome, BattleError> {
    if attacker.is_fainted() {
        return Err(BattleError::FaintedPokemonCannotAttack);
    }

    let selected_move = attacker
        .moves
        .get(move_index)
        .ok_or(BattleError::InvalidMoveIndex { index: move_index })?;
    let damage_result = calculate_damage(attacker, defender, selected_move);
    defender.current_hp = defender.current_hp.saturating_sub(damage_result.damage);

    Ok(AttackOutcome {
        attacker: attacker.name.clone(),
        defender: defender.name.clone(),
        move_name: selected_move.name.clone(),
        damage: damage_result.damage,
        effectiveness: damage_result.effectiveness,
        stab: damage_result.stab,
        defender_hp_after: defender.current_hp,
    })
}

pub fn simulate_turn(
    first_pokemon: &mut Pokemon,
    first_move_index: usize,
    second_pokemon: &mut Pokemon,
    second_move_index: usize,
) -> Result<TurnOutcome, BattleError> {
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
    let first = execute_attack(faster, slower, faster_move_index)?;
    let second = if slower.is_fainted() {
        None
    } else {
        Some(execute_attack(slower, faster, slower_move_index)?)
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
    use crate::info::{Nature, PokemonSpec, PokemonType, StatPoints, Stats};

    fn tackle() -> Move {
        Move::new(
            "Tackle",
            PokemonType::Normal,
            MoveCategory::Physical,
            40,
            100,
            0,
        )
    }

    fn quick_attack() -> Move {
        Move::new(
            "Quick Attack",
            PokemonType::Normal,
            MoveCategory::Physical,
            40,
            100,
            1,
        )
    }

    fn ember() -> Move {
        Move::new(
            "Ember",
            PokemonType::Fire,
            MoveCategory::Special,
            40,
            100,
            0,
        )
    }

    fn vine_whip() -> Move {
        Move::new(
            "Vine Whip",
            PokemonType::Grass,
            MoveCategory::Physical,
            45,
            100,
            0,
        )
    }

    fn splash() -> Move {
        Move::new(
            "Splash",
            PokemonType::Normal,
            MoveCategory::Status,
            0,
            100,
            0,
        )
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

    fn charmander() -> Pokemon {
        Pokemon::new(PokemonSpec {
            name: "Charmander".to_string(),
            level: 50,
            primary_type: PokemonType::Fire,
            secondary_type: None,
            stats: Stats {
                hp: 120,
                attack: 80,
                defense: 65,
                special_attack: 95,
                special_defense: 75,
                speed: 90,
            },
            stat_points: valid_stat_points(),
            nature: Nature::neutral(),
            item: None,
            moves: [ember(), tackle(), splash(), tackle()],
        })
        .unwrap()
    }

    fn bulbasaur() -> Pokemon {
        Pokemon::new(PokemonSpec {
            name: "Bulbasaur".to_string(),
            level: 50,
            primary_type: PokemonType::Grass,
            secondary_type: Some(PokemonType::Poison),
            stats: Stats {
                hp: 125,
                attack: 82,
                defense: 83,
                special_attack: 85,
                special_defense: 85,
                speed: 70,
            },
            stat_points: valid_stat_points(),
            nature: Nature::neutral(),
            item: None,
            moves: [vine_whip(), tackle(), splash(), tackle()],
        })
        .unwrap()
    }

    #[test]
    fn calculates_stab_and_super_effective_damage() {
        let attacker = charmander();
        let defender = bulbasaur();

        let result = calculate_damage(&attacker, &defender, &attacker.moves[0]);

        assert_eq!(result.effectiveness, 2.0);
        assert!(result.stab);
        assert_eq!(result.damage, 63);
    }

    #[test]
    fn status_moves_deal_no_damage() {
        let attacker = charmander();
        let defender = bulbasaur();

        let result = calculate_damage(&attacker, &defender, &attacker.moves[2]);

        assert_eq!(result.damage, 0);
        assert_eq!(result.effectiveness, 1.0);
        assert!(!result.stab);
    }

    #[test]
    fn priority_move_can_attack_before_faster_pokemon() {
        let mut slower = bulbasaur();
        slower.moves[0] = quick_attack();
        let mut faster = charmander();

        let order = determine_turn_order(&slower, 0, &faster, 0).unwrap();
        let outcome = simulate_turn(&mut slower, 0, &mut faster, 0).unwrap();

        assert_eq!(order, TurnOrder::FirstPokemon);
        assert_eq!(outcome.first.unwrap().attacker, "Bulbasaur");
    }

    #[test]
    fn faster_pokemon_attacks_first_and_fainting_stops_counterattack() {
        let mut attacker = charmander();
        let mut defender = bulbasaur();
        defender.current_hp = 10;

        let outcome = simulate_turn(&mut attacker, 0, &mut defender, 0).unwrap();

        assert_eq!(outcome.first.unwrap().attacker, "Charmander");
        assert!(outcome.second.is_none());
        assert_eq!(defender.current_hp, 0);
    }

    #[test]
    fn second_input_can_act_first_when_it_is_faster() {
        let mut slower = bulbasaur();
        let mut faster = charmander();

        let outcome = simulate_turn(&mut slower, 1, &mut faster, 0).unwrap();

        assert_eq!(outcome.first.unwrap().attacker, "Charmander");
    }

    #[test]
    fn invalid_move_index_returns_error() {
        let mut attacker = charmander();
        let mut defender = bulbasaur();

        let result = simulate_turn(&mut attacker, 4, &mut defender, 0);

        assert_eq!(result, Err(BattleError::InvalidMoveIndex { index: 4 }));
    }
}
