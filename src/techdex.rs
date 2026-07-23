use crate::info::{Move, MoveCategory, PokemonType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveEntry {
    pub name: &'static str,
    pub move_type: PokemonType,
    pub category: MoveCategory,
    pub power: u16,
    pub accuracy: u8,
    pub priority: i8,
}

impl MoveEntry {
    pub fn to_move(self) -> Move {
        Move::new(
            self.name,
            self.move_type,
            self.category,
            self.power,
            self.accuracy,
            self.priority,
        )
    }
}

pub const FLAMETHROWER: MoveEntry = move_entry(
    "Flamethrower",
    PokemonType::Fire,
    MoveCategory::Special,
    90,
    100,
    0,
);
pub const AIR_SLASH: MoveEntry = move_entry(
    "Air Slash",
    PokemonType::Flying,
    MoveCategory::Special,
    75,
    95,
    0,
);
pub const DRAGON_CLAW: MoveEntry = move_entry(
    "Dragon Claw",
    PokemonType::Dragon,
    MoveCategory::Physical,
    80,
    100,
    0,
);
pub const FIRE_BLAST: MoveEntry = move_entry(
    "Fire Blast",
    PokemonType::Fire,
    MoveCategory::Special,
    110,
    85,
    0,
);
pub const DRAGON_DANCE: MoveEntry = move_entry(
    "Dragon Dance",
    PokemonType::Dragon,
    MoveCategory::Status,
    0,
    100,
    0,
);
pub const VINE_WHIP: MoveEntry = move_entry(
    "Vine Whip",
    PokemonType::Grass,
    MoveCategory::Physical,
    45,
    100,
    0,
);
pub const RAZOR_LEAF: MoveEntry = move_entry(
    "Razor Leaf",
    PokemonType::Grass,
    MoveCategory::Physical,
    55,
    95,
    0,
);
pub const SLEEP_POWDER: MoveEntry = move_entry(
    "Sleep Powder",
    PokemonType::Grass,
    MoveCategory::Status,
    0,
    75,
    0,
);
pub const SEED_BOMB: MoveEntry = move_entry(
    "Seed Bomb",
    PokemonType::Grass,
    MoveCategory::Physical,
    80,
    100,
    0,
);
pub const SOLAR_BEAM: MoveEntry = move_entry(
    "Solar Beam",
    PokemonType::Grass,
    MoveCategory::Special,
    120,
    100,
    0,
);
pub const WATER_GUN: MoveEntry = move_entry(
    "Water Gun",
    PokemonType::Water,
    MoveCategory::Special,
    40,
    100,
    0,
);
pub const RAPID_SPIN: MoveEntry = move_entry(
    "Rapid Spin",
    PokemonType::Normal,
    MoveCategory::Physical,
    50,
    100,
    0,
);
pub const BITE: MoveEntry = move_entry(
    "Bite",
    PokemonType::Dark,
    MoveCategory::Physical,
    60,
    100,
    0,
);
pub const WATER_PULSE: MoveEntry = move_entry(
    "Water Pulse",
    PokemonType::Water,
    MoveCategory::Special,
    60,
    100,
    0,
);
pub const HYDRO_PUMP: MoveEntry = move_entry(
    "Hydro Pump",
    PokemonType::Water,
    MoveCategory::Special,
    110,
    80,
    0,
);
pub const SHADOW_BALL: MoveEntry = move_entry(
    "Shadow Ball",
    PokemonType::Ghost,
    MoveCategory::Special,
    80,
    100,
    0,
);
pub const HEX: MoveEntry = move_entry("Hex", PokemonType::Ghost, MoveCategory::Special, 65, 100, 0);
pub const HYPNOSIS: MoveEntry = move_entry(
    "Hypnosis",
    PokemonType::Psychic,
    MoveCategory::Status,
    0,
    60,
    0,
);
pub const DARK_PULSE: MoveEntry = move_entry(
    "Dark Pulse",
    PokemonType::Dark,
    MoveCategory::Special,
    80,
    100,
    0,
);
pub const SHADOW_SNEAK: MoveEntry = move_entry(
    "Shadow Sneak",
    PokemonType::Ghost,
    MoveCategory::Physical,
    40,
    100,
    1,
);
pub const AURA_SPHERE: MoveEntry = move_entry(
    "Aura Sphere",
    PokemonType::Fighting,
    MoveCategory::Special,
    80,
    100,
    0,
);
pub const METAL_CLAW: MoveEntry = move_entry(
    "Metal Claw",
    PokemonType::Steel,
    MoveCategory::Physical,
    50,
    95,
    0,
);
pub const QUICK_ATTACK: MoveEntry = move_entry(
    "Quick Attack",
    PokemonType::Normal,
    MoveCategory::Physical,
    40,
    100,
    1,
);
pub const SWORDS_DANCE: MoveEntry = move_entry(
    "Swords Dance",
    PokemonType::Normal,
    MoveCategory::Status,
    0,
    100,
    0,
);
pub const CLOSE_COMBAT: MoveEntry = move_entry(
    "Close Combat",
    PokemonType::Fighting,
    MoveCategory::Physical,
    120,
    100,
    0,
);
pub const WING_ATTACK: MoveEntry = move_entry(
    "Wing Attack",
    PokemonType::Flying,
    MoveCategory::Physical,
    60,
    100,
    0,
);
pub const EXTREME_SPEED: MoveEntry = move_entry(
    "Extreme Speed",
    PokemonType::Normal,
    MoveCategory::Physical,
    80,
    100,
    2,
);
pub const FIRE_PUNCH: MoveEntry = move_entry(
    "Fire Punch",
    PokemonType::Fire,
    MoveCategory::Physical,
    75,
    100,
    0,
);
pub const THUNDER_PUNCH: MoveEntry = move_entry(
    "Thunder Punch",
    PokemonType::Electric,
    MoveCategory::Physical,
    75,
    100,
    0,
);
pub const HURRICANE: MoveEntry = move_entry(
    "Hurricane",
    PokemonType::Flying,
    MoveCategory::Special,
    110,
    70,
    0,
);

pub const ROCK_SLIDE: MoveEntry = move_entry(
    "Rock Slide",
    PokemonType::Rock,
    MoveCategory::Physical,
    75,
    90,
    0,
);

pub const PROTECT: MoveEntry = move_entry(
    "Protect",
    PokemonType::Normal,
    MoveCategory::Status,
    0,
    100,
    4,
);

pub const DETECT: MoveEntry = move_entry(
    "Detect",
    PokemonType::Fighting,
    MoveCategory::Status,
    0,
    100,
    4,
);

const MOVE_DICT: &[MoveEntry] = &[
    FLAMETHROWER,
    AIR_SLASH,
    DRAGON_CLAW,
    FIRE_BLAST,
    DRAGON_DANCE,
    VINE_WHIP,
    RAZOR_LEAF,
    SLEEP_POWDER,
    SEED_BOMB,
    SOLAR_BEAM,
    WATER_GUN,
    RAPID_SPIN,
    BITE,
    WATER_PULSE,
    HYDRO_PUMP,
    SHADOW_BALL,
    HEX,
    HYPNOSIS,
    DARK_PULSE,
    SHADOW_SNEAK,
    AURA_SPHERE,
    METAL_CLAW,
    QUICK_ATTACK,
    SWORDS_DANCE,
    CLOSE_COMBAT,
    WING_ATTACK,
    EXTREME_SPEED,
    FIRE_PUNCH,
    THUNDER_PUNCH,
    HURRICANE,
    ROCK_SLIDE,
    PROTECT,
    DETECT,
];

pub fn techdex() -> &'static [MoveEntry] {
    MOVE_DICT
}

pub fn find_move(name: &str) -> Option<&'static MoveEntry> {
    MOVE_DICT
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
}

const fn move_entry(
    name: &'static str,
    move_type: PokemonType,
    category: MoveCategory,
    power: u16,
    accuracy: u8,
    priority: i8,
) -> MoveEntry {
    MoveEntry {
        name,
        move_type,
        category,
        power,
        accuracy,
        priority,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_only_move_data() {
        let extreme_speed = find_move("Extreme Speed").unwrap();

        assert_eq!(techdex().len(), 33);
        assert_eq!(extreme_speed.priority, 2);
        assert_eq!(extreme_speed.move_type, PokemonType::Normal);
    }
}
