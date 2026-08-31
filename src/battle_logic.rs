use crate::info::{BattleError, Move, MoveCategory, Pokemon, type_effectiveness_against};
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
    pub const DAMAGE_RANDOM_RAW_MIN: u8 = 217;
    pub const DAMAGE_RANDOM_RAW_MAX: u8 = 255;

    pub fn with_raw_random_roll(mut self, seed: Option<u64>) -> Result<Self, BattleError> {
        let roll = match seed {
            Some(seed) => {
                let mut rng = StdRng::seed_from_u64(seed);
                rng.random_range(Self::DAMAGE_RANDOM_RAW_MIN..=Self::DAMAGE_RANDOM_RAW_MAX)
            }
            None => rand::random_range(Self::DAMAGE_RANDOM_RAW_MIN..=Self::DAMAGE_RANDOM_RAW_MAX),
        };

        self.random_percent = ((u16::from(roll) * 100) / 255) as u8;
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
pub struct AttackOutcome {
    pub attacker: String,
    pub defender: String,
    pub move_name: String,
    pub damage: u16,
    pub effectiveness: f32,
    pub blocked: bool, // protect moves
    pub defender_hp_after: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurnOutcome {
    pub first: AttackOutcome,
    pub second: Option<AttackOutcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOrder {
    FirstPokemon,
    SecondPokemon,
}

pub struct Battle {
    pub p1: Pokemon,
    pub p2: Pokemon,
    pub turn_count: u32,
    // TODO: Add more fields as needed, such as battle state, weather conditions, etc.
}

impl Battle {
    pub fn new(p1: Pokemon, p2: Pokemon) -> Self {
        Self {
            p1,
            p2,
            turn_count: 1,
        }
    }

    pub fn simulate_turn(
        &mut self,
        first_move_index: usize,
        second_move_index: usize,
    ) -> Result<TurnOutcome, BattleError> {
        if self.p1.is_fainted() || self.p2.is_fainted() {
            return Err(BattleError::FaintedPokemonCannotBattle);
        }

        let order = self.determine_turn_order(first_move_index, second_move_index)?;

        let outcome = self.resolve_turn_order(first_move_index, second_move_index, order)?;

        self.turn_count += 1;
        Ok(outcome)
    }

    pub fn determine_turn_order(
        &mut self,
        first_move_index: usize,
        second_move_index: usize,
    ) -> Result<TurnOrder, BattleError> {
        let first_pokemon = &mut self.p1;
        let second_pokemon = &mut self.p2;

        validate_move_index(first_pokemon, first_move_index)?;
        validate_move_index(second_pokemon, second_move_index)?;

        let first_move = &first_pokemon.moves[first_move_index];
        let second_move = &second_pokemon.moves[second_move_index];

        if first_move.priority > second_move.priority {
            Ok(TurnOrder::FirstPokemon)
        } else if first_move.priority < second_move.priority {
            Ok(TurnOrder::SecondPokemon)
        } else if first_pokemon.stats.speed > second_pokemon.stats.speed {
            Ok(TurnOrder::FirstPokemon)
        } else if first_pokemon.stats.speed < second_pokemon.stats.speed {
            Ok(TurnOrder::SecondPokemon)
        } else {
            let turn_order = match rand::random() {
                true => TurnOrder::FirstPokemon,
                false => TurnOrder::SecondPokemon,
            };
            Ok(turn_order)
        }
    }

    fn resolve_turn_order(
        &mut self,
        faster_move_index: usize,
        slower_move_index: usize,
        order: TurnOrder,
    ) -> Result<TurnOutcome, BattleError> {
        // TODO: Implement actual battle logic damage calculation, effectiveness, and handling of fainting.
        // For now, we will return dummy data for few test
        let (faster, slower, faster_move_index, slower_move_index) = match order {
            TurnOrder::FirstPokemon => (
                &mut self.p1,
                &mut self.p2,
                faster_move_index,
                slower_move_index,
            ),
            TurnOrder::SecondPokemon => (
                &mut self.p2,
                &mut self.p1,
                slower_move_index,
                faster_move_index,
            ),
        };

        let first_move_name = faster.moves[faster_move_index].name.to_string();
        slower.current_hp = slower.current_hp.saturating_sub(50);

        let dummy_first_attack = AttackOutcome {
            attacker: faster.entry.name.to_string(),
            defender: slower.entry.name.to_string(),
            move_name: first_move_name,
            damage: 50,
            effectiveness: 1.0,
            blocked: false,
            defender_hp_after: slower.current_hp,
        };

        if slower.current_hp == 0 {
            return Ok(TurnOutcome {
                first: dummy_first_attack,
                second: None,
            });
        }

        let second_move_name = slower.moves[slower_move_index].name.to_string();
        faster.current_hp = faster.current_hp.saturating_sub(50);
        let dummy_second_attack = AttackOutcome {
            attacker: slower.entry.name.to_string(),
            defender: faster.entry.name.to_string(),
            move_name: second_move_name,
            damage: 50,
            effectiveness: 1.0,
            blocked: false,
            defender_hp_after: faster.current_hp,
        };

        Ok(TurnOutcome {
            first: dummy_first_attack,
            second: Some(dummy_second_attack),
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
        damage = (damage * u64::from(modifiers.random_percent)) / 100;

        damage = modifiers.stab.apply_to(damage)?;
        damage = modifiers.type_effectiveness.apply_to(damage)?;
        damage = modifiers.burn.apply_to(damage)?;
        // Apply all remaining final damage modifiers that do not belong to the
        // explicit calculation stages above, such as screens, abilities, and items.
        damage = modifiers.other.apply_to(damage)?;

        let damage = if damage == 0 {
            0
        } else {
            damage.max(1).min(u64::from(u16::MAX)) as u16
        };

        let effectiveness = type_effectiveness_against(selected_move.r#type, defender);

        Ok(DamageResult {
            damage,
            effectiveness,
        })
    }
}

fn validate_move_index(pokemon: &Pokemon, move_index: usize) -> Result<(), BattleError> {
    pokemon
        .moves
        .get(move_index)
        .map(|_| ())
        .ok_or(BattleError::InvalidMoveIndex { index: move_index })
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

    #[test]
    fn seeded_raw_random_roll_is_reproducible() {
        let first = DamageModifier::default()
            .with_raw_random_roll(Some(42))
            .unwrap()
            .random_percent;
        let second = DamageModifier::default()
            .with_raw_random_roll(Some(42))
            .unwrap()
            .random_percent;

        assert_eq!(first, second);
        assert!((85..=100).contains(&first));
    }

    #[test]
    fn priority_move_can_attack_before_faster_pokemon() {
        let slower = dragonite();
        let faster = charizard();

        let mut battle = Battle::new(slower.clone(), faster.clone());
        let order = battle.determine_turn_order(0, 0).unwrap();
        let outcome = battle.simulate_turn(0, 0).unwrap();

        assert_eq!(order, TurnOrder::FirstPokemon);
        assert_eq!(outcome.first.attacker, "Dragonite");
    }

    #[test]
    fn higher_priority_move_faster() {
        let slower = dragonite();
        let faster = charizard();

        let mut battle = Battle::new(slower.clone(), faster.clone());

        let order = battle.determine_turn_order(0, 3).unwrap();
        let outcome = battle.simulate_turn(0, 3).unwrap();

        assert_eq!(order, TurnOrder::SecondPokemon);
        assert_eq!(outcome.first.attacker, "Charizard");
    }

    #[test]
    fn lower_priority_move_slower() {
        let slower = venusaur();
        let faster = dragonite();

        let mut battle = Battle::new(slower.clone(), faster.clone());

        let order = battle.determine_turn_order(0, 3).unwrap();
        let outcome = battle.simulate_turn(0, 3).unwrap();

        assert_eq!(order, TurnOrder::FirstPokemon);
        assert_eq!(outcome.first.attacker, "Venusaur");
    }

    #[test]
    fn faster_pokemon_attacks_first_and_fainting_stops_counterattack() {
        let attacker = charizard();
        let defender = venusaur();

        let mut battle = Battle::new(attacker.clone(), defender.clone());
        battle.p2.current_hp = 10;

        let outcome = battle.simulate_turn(0, 0).unwrap();

        assert_eq!(outcome.first.attacker, "Charizard");
        assert!(outcome.second.is_none());
    }

    #[test]
    fn second_input_can_act_first_when_it_is_faster() {
        let slower = venusaur();
        let faster = charizard();

        let mut battle = Battle::new(slower.clone(), faster.clone());
        let outcome = battle.simulate_turn(1, 0).unwrap();

        assert_eq!(outcome.first.attacker, "Charizard");
    }

    #[test]
    fn invalid_move_index_returns_error() {
        let attacker = charizard();
        let defender = venusaur();

        let mut battle = Battle::new(attacker.clone(), defender.clone());
        let result = Battle::simulate_turn(&mut battle, 4, 0);

        assert_eq!(result, Err(BattleError::InvalidMoveIndex { index: 4 }));
    }

    #[test]
    fn turn_cannot_start_with_fainted_pokemon() {
        let attacker = charizard();
        let defender = venusaur();

        let mut battle = Battle::new(attacker.clone(), defender.clone());
        battle.p1.current_hp = 0;

        let result = Battle::simulate_turn(&mut battle, 0, 0);

        assert_eq!(result, Err(BattleError::FaintedPokemonCannotBattle));
    }
}
