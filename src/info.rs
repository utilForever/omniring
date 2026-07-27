use crate::pokedex::PokemonEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleFormat {
    Singles,
    // Doubles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChampionsRules {
    pub format: BattleFormat,
    pub level_cap: u8,
    pub team_size: u8,
    pub selected_team_size: u8,
    pub mega_stones_enabled: bool,
}

impl ChampionsRules {
    pub fn singles() -> Self {
        Self {
            format: BattleFormat::Singles,
            level_cap: 50,
            team_size: 6,
            selected_team_size: 3,
            mega_stones_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokemonType {
    Normal,
    Fire,
    Water,
    Electric,
    Grass,
    Ice,
    Fighting,
    Poison,
    Ground,
    Flying,
    Psychic,
    Bug,
    Rock,
    Ghost,
    Dragon,
    Dark,
    Steel,
    Fairy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveCategory {
    Physical,
    Special,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub hp: u16,
    pub attack: u16,
    pub defense: u16,
    pub special_attack: u16,
    pub special_defense: u16,
    pub speed: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatPoints {
    pub hp: u8,
    pub attack: u8,
    pub defense: u8,
    pub special_attack: u8,
    pub special_defense: u8,
    pub speed: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatName {
    Attack,
    Defense,
    SpecialAttack,
    SpecialDefense,
    Speed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nature {
    // 1. Neutral (5 natures)
    Hardy,   // Neutral
    Docile,  // Neutral
    Serious, // Neutral
    Bashful, // Neutral
    Quirky,  // Neutral

    // 2. Raises Attack (4 natures)
    Lonely,  // Attack+, Defense-
    Adamant, // Attack+, Special Attack-
    Naughty, // Attack+, Special Defense-
    Brave,   // Attack+, Speed-

    // 3. Raises Defense (4 natures)
    Bold,    // Defense+, Attack-
    Impish,  // Defense+, Special Attack-
    Lax,     // Defense+, Special Defense-
    Relaxed, // Defense+, Speed-

    // 4. Raises Special Attack (4 natures)
    Modest, // Special Attack+, Attack-
    Mild,   // Special Attack+, Defense-
    Rash,   // Special Attack+, Special Defense-
    Quiet,  // Special Attack+, Speed-

    // 5. Raises Special Defense (4 natures)
    Calm,    // Special Defense+, Attack-
    Gentle,  // Special Defense+, Defense-
    Careful, // Special Defense+, Special Attack-
    Sassy,   // Special Defense+, Speed-

    // 6. Raises Speed (4 natures)
    Timid, // Speed+, Attack-
    Hasty, // Speed+, Defense-
    Jolly, // Speed+, Special Attack-
    Naive, // Speed+, Special Defense-
}

impl Nature {
    /// Returns the stat increased by this nature (+10%)
    pub fn increased(self) -> Option<StatName> {
        use Nature::*;
        use StatName::*;

        match self {
            Lonely | Adamant | Naughty | Brave => Some(Attack),
            Bold | Impish | Lax | Relaxed => Some(Defense),
            Modest | Mild | Rash | Quiet => Some(SpecialAttack),
            Calm | Gentle | Careful | Sassy => Some(SpecialDefense),
            Timid | Hasty | Jolly | Naive => Some(Speed),
            Hardy | Docile | Serious | Bashful | Quirky => None,
        }
    }

    /// Returns the stat decreased by this nature (-10%)
    pub fn decreased(self) -> Option<StatName> {
        use Nature::*;
        use StatName::*;

        match self {
            Bold | Modest | Calm | Timid => Some(Attack),
            Lonely | Mild | Gentle | Hasty => Some(Defense),
            Adamant | Impish | Careful | Jolly => Some(SpecialAttack),
            Naughty | Lax | Rash | Naive => Some(SpecialDefense),
            Brave | Relaxed | Quiet | Sassy => Some(Speed),
            Hardy | Docile | Serious | Bashful | Quirky => None,
        }
    }

    /// Returns this nature's multiplier for a stat (1.1, 0.9, or 1.0)
    pub fn multiplier_for(self, stat: StatName) -> f32 {
        if self.increased() == Some(stat) {
            1.1
        } else if self.decreased() == Some(stat) {
            0.9
        } else {
            1.0
        }
    }
}

impl Stats {
    pub const fn new(
        hp: u16,
        attack: u16,
        defense: u16,
        special_attack: u16,
        special_defense: u16,
        speed: u16,
    ) -> Self {
        Self {
            hp,
            attack,
            defense,
            special_attack,
            special_defense,
            speed,
        }
    }

    /// Applies the nature modifier (1.1 or 0.9) and returns the adjusted stats
    pub fn apply_nature(self, nature: Nature) -> Self {
        Self {
            hp: self.hp, // Nature does not affect HP
            attack: (self.attack as f32 * nature.multiplier_for(StatName::Attack)) as u16,
            defense: (self.defense as f32 * nature.multiplier_for(StatName::Defense)) as u16,
            special_attack: (self.special_attack as f32
                * nature.multiplier_for(StatName::SpecialAttack))
                as u16,
            special_defense: (self.special_defense as f32
                * nature.multiplier_for(StatName::SpecialDefense))
                as u16,
            speed: (self.speed as f32 * nature.multiplier_for(StatName::Speed)) as u16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MegaStone {
    CharizarditeX,
    CharizarditeY,
    Venusaurite,
    Blastoisinite,
    Gengarite,
    Lucarionite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldItem {
    MegaStone(MegaStone),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub name: String,
    pub r#type: PokemonType,
    pub category: MoveCategory,
    pub power: u16,
    pub accuracy: Option<u8>,
    pub priority: i8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pokemon {
    pub entry: &'static PokemonEntry,

    pub level: u8,
    pub stat_points: StatPoints,
    pub nature: Nature,
    pub ability: Option<String>,
    pub item: Option<HeldItem>,
    pub moves: [Move; 4],

    pub current_hp: u16,
    pub can_mega_evolve: bool,
    pub has_mega_evolved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleError {
    InvalidMoveIndex { index: usize },
    InvalidStatPoints,
    InvalidDamageModifier,
    InvalidDamageRandomRawRoll { raw_roll: u8 },
    ZeroDefenseStat,
    InvalidDamageRandomPercent { percent: u8 },
    FaintedPokemonCannotAttack,
    FaintedPokemonCannotBeTargeted,
    FaintedPokemonCannotBattle,
}

impl StatPoints {
    pub const MAX_TOTAL: u8 = 66;
    pub const MAX_PER_STAT: u8 = 32;

    pub fn total(self) -> u16 {
        u16::from(self.hp)
            + u16::from(self.attack)
            + u16::from(self.defense)
            + u16::from(self.special_attack)
            + u16::from(self.special_defense)
            + u16::from(self.speed)
    }

    pub fn is_valid_for_champions(self) -> bool {
        self.total() <= u16::from(Self::MAX_TOTAL)
            && self.hp <= Self::MAX_PER_STAT
            && self.attack <= Self::MAX_PER_STAT
            && self.defense <= Self::MAX_PER_STAT
            && self.special_attack <= Self::MAX_PER_STAT
            && self.special_defense <= Self::MAX_PER_STAT
            && self.speed <= Self::MAX_PER_STAT
    }
}

impl MegaStone {
    pub fn compatible_species(self) -> &'static str {
        match self {
            Self::CharizarditeX | Self::CharizarditeY => "Charizard",
            Self::Venusaurite => "Venusaur",
            Self::Blastoisinite => "Blastoise",
            Self::Gengarite => "Gengar",
            Self::Lucarionite => "Lucario",
        }
    }
}

impl HeldItem {
    pub fn is_mega_stone_for(self, species_name: &str) -> bool {
        match self {
            Self::MegaStone(stone) => stone
                .compatible_species()
                .eq_ignore_ascii_case(species_name),
        }
    }
}

impl Move {
    pub fn new(
        name: impl Into<String>,
        r#type: PokemonType,
        category: MoveCategory,
        power: u16,
        accuracy: Option<u8>,
        priority: i8,
    ) -> Self {
        Self {
            name: name.into(),
            r#type,
            category,
            power,
            accuracy,
            priority,
        }
    }
}

pub fn calculate_max_hp(base_hp: u16, level: u8, stat_points: u8) -> u16 {
    let numerator = (2 * u32::from(base_hp) + 31 + 2 * u32::from(stat_points)) * u32::from(level);
    (numerator / 100 + 10 + u32::from(level)) as u16
}

impl Pokemon {
    pub fn new(
        entry: &'static PokemonEntry,
        level: u8,
        stat_points: StatPoints,
        nature: Nature,
        item: Option<HeldItem>,
        moves: [Move; 4],
    ) -> Result<Self, BattleError> {
        if !stat_points.is_valid_for_champions() {
            return Err(BattleError::InvalidStatPoints);
        }
        let max_hp = calculate_max_hp(entry.base_stats.hp, level, stat_points.hp);
        let can_mega_evolve = item
            .as_ref()
            .is_some_and(|i| i.is_mega_stone_for(entry.name));
        Ok(Self {
            entry,
            level,
            stat_points,
            nature,
            ability: None,
            item,
            moves,
            current_hp: max_hp,
            can_mega_evolve,
            has_mega_evolved: false,
        })
    }

    pub fn is_fainted(&self) -> bool {
        self.current_hp == 0
    }
}

pub fn type_effectiveness_against(attack_type: PokemonType, defender: &Pokemon) -> f32 {
    let primary = type_effectiveness(attack_type, defender.entry.primary_type);
    let secondary = defender.entry.secondary_type.map_or(1.0, |defense_type| {
        type_effectiveness(attack_type, defense_type)
    });

    primary * secondary
}

pub fn type_effectiveness(attack_type: PokemonType, defense_type: PokemonType) -> f32 {
    use PokemonType::{
        Bug, Dark, Dragon, Electric, Fairy, Fighting, Fire, Flying, Ghost, Grass, Ground, Ice,
        Normal, Poison, Psychic, Rock, Steel, Water,
    };

    match (attack_type, defense_type) {
        (Normal, Rock | Steel) => 0.5,
        (Normal, Ghost) => 0.0,
        (Fire, Fire | Water | Rock | Dragon) => 0.5,
        (Fire, Grass | Ice | Bug | Steel) => 2.0,
        (Water, Water | Grass | Dragon) => 0.5,
        (Water, Fire | Ground | Rock) => 2.0,
        (Electric, Electric | Grass | Dragon) => 0.5,
        (Electric, Water | Flying) => 2.0,
        (Electric, Ground) => 0.0,
        (Grass, Fire | Grass | Poison | Flying | Bug | Dragon | Steel) => 0.5,
        (Grass, Water | Ground | Rock) => 2.0,
        (Ice, Fire | Water | Ice | Steel) => 0.5,
        (Ice, Grass | Ground | Flying | Dragon) => 2.0,
        (Fighting, Poison | Flying | Psychic | Bug | Fairy) => 0.5,
        (Fighting, Normal | Ice | Rock | Dark | Steel) => 2.0,
        (Fighting, Ghost) => 0.0,
        (Poison, Poison | Ground | Rock | Ghost) => 0.5,
        (Poison, Grass | Fairy) => 2.0,
        (Poison, Steel) => 0.0,
        (Ground, Grass | Bug) => 0.5,
        (Ground, Fire | Electric | Poison | Rock | Steel) => 2.0,
        (Ground, Flying) => 0.0,
        (Flying, Electric | Rock | Steel) => 0.5,
        (Flying, Grass | Fighting | Bug) => 2.0,
        (Psychic, Psychic | Steel) => 0.5,
        (Psychic, Fighting | Poison) => 2.0,
        (Psychic, Dark) => 0.0,
        (Bug, Fire | Fighting | Poison | Flying | Ghost | Steel | Fairy) => 0.5,
        (Bug, Grass | Psychic | Dark) => 2.0,
        (Rock, Fighting | Ground | Steel) => 0.5,
        (Rock, Fire | Ice | Flying | Bug) => 2.0,
        (Ghost, Dark) => 0.5,
        (Ghost, Psychic | Ghost) => 2.0,
        (Ghost, Normal) => 0.0,
        (Dragon, Steel) => 0.5,
        (Dragon, Dragon) => 2.0,
        (Dragon, Fairy) => 0.0,
        (Dark, Fighting | Dark | Fairy) => 0.5,
        (Dark, Psychic | Ghost) => 2.0,
        (Steel, Fire | Water | Electric | Steel) => 0.5,
        (Steel, Ice | Rock | Fairy) => 2.0,
        (Fairy, Fire | Poison | Steel) => 0.5,
        (Fairy, Fighting | Dragon | Dark) => 2.0,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pokedex::find_pokemon;

    fn tackle() -> Move {
        Move::new(
            "Tackle",
            PokemonType::Normal,
            MoveCategory::Physical,
            40,
            Some(100),
            0,
        )
    }

    #[test]
    fn champions_singles_uses_mega_stones() {
        let rules = ChampionsRules::singles();

        assert_eq!(rules.format, BattleFormat::Singles);
        assert_eq!(rules.level_cap, 50);
        assert_eq!(rules.team_size, 6);
        assert_eq!(rules.selected_team_size, 3);
        assert!(rules.mega_stones_enabled);
    }

    #[test]
    fn type_chart_handles_dual_type_multiplier() {
        let venusaur = Pokemon::new(
            find_pokemon("Venusaur").unwrap(),
            50,
            StatPoints {
                hp: 32,
                attack: 32,
                defense: 2,
                special_attack: 0,
                special_defense: 0,
                speed: 0,
            },
            Nature::Hardy,
            None,
            [tackle(), tackle(), tackle(), tackle()],
        )
        .unwrap();

        assert_eq!(
            type_effectiveness_against(PokemonType::Fire, &venusaur),
            2.0
        );
        assert_eq!(
            type_effectiveness_against(PokemonType::Grass, &venusaur),
            0.25
        );
    }

    #[test]
    fn rejects_invalid_champions_stat_points() {
        let invalid_points = StatPoints {
            hp: 33,
            attack: 33,
            defense: 0,
            special_attack: 0,
            special_defense: 0,
            speed: 0,
        };

        let result = Pokemon::new(
            find_pokemon("Venusaur").unwrap(),
            50,
            invalid_points,
            Nature::Hardy,
            None,
            [tackle(), tackle(), tackle(), tackle()],
        );

        assert_eq!(result, Err(BattleError::InvalidStatPoints));
    }

    #[test]
    fn mega_stone_marks_compatible_pokemon_as_mega_evolvable() {
        let charizard = Pokemon::new(
            find_pokemon("Charizard").unwrap(),
            50,
            StatPoints {
                hp: 32,
                attack: 32,
                defense: 2,
                special_attack: 0,
                special_defense: 0,
                speed: 0,
            },
            Nature::Hardy,
            Some(HeldItem::MegaStone(MegaStone::CharizarditeX)),
            [tackle(), tackle(), tackle(), tackle()],
        )
        .unwrap();

        assert!(charizard.can_mega_evolve);
        assert!(!charizard.has_mega_evolved);
    }

    #[test]
    fn hp_calculation_reflects_base_hp_and_stat_points() {
        let level = 50;

        let hp_low_base = calculate_max_hp(60, level, 0);
        assert_eq!(hp_low_base, 135);

        let hp_high_base = calculate_max_hp(78, level, 0);
        assert_eq!(hp_high_base, 153);

        // 3. Stat Points가 변경되었을 때 HP가 달라지는지 검증 (Stat Points 0 -> 32)
        let hp_with_stat_points = calculate_max_hp(78, level, 32);
        assert_eq!(hp_with_stat_points, 185);
    }
}
