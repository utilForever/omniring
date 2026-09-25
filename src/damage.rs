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
    use crate::info::{Nature, PokemonType, StatPoints};
    use crate::pokedex::find_pokemon;

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
        let attacker = pokemon("Charizard");
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
}
