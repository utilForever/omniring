//! Pokemon species data for final-evolution Pokemon planned for this simulator.
//!
//! A `PokemonEntry` stores species-level facts: name, typing, base stats, and
//! the moves this sample dataset allows that species to learn. Battle-ready
//! Pokemon are built from this data by selecting four move names.

use crate::info::{
    BattleError, HeldItem, Nature, Pokemon, PokemonSpec, PokemonType, StatPoints, Stats,
};
use crate::techdex::{
    AIR_SLASH, AURA_SPHERE, BITE, CLOSE_COMBAT, DARK_PULSE, DRAGON_CLAW, DRAGON_DANCE,
    EXTREME_SPEED, FIRE_BLAST, FIRE_PUNCH, FLAMETHROWER, HEX, HURRICANE, HYDRO_PUMP, HYPNOSIS,
    METAL_CLAW, MoveEntry, QUICK_ATTACK, RAPID_SPIN, RAZOR_LEAF, SEED_BOMB, SHADOW_BALL,
    SHADOW_SNEAK, SLEEP_POWDER, SOLAR_BEAM, SWORDS_DANCE, THUNDER_PUNCH, VINE_WHIP, WATER_GUN,
    WATER_PULSE, WING_ATTACK,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PokemonEntry {
    pub name: &'static str,
    pub primary_type: PokemonType,
    pub secondary_type: Option<PokemonType>,
    pub base_stats: Stats,
    pub learnable_moves: &'static [MoveEntry],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokedexError {
    PokemonNotFound,
    MoveNotLearnable { slot: usize },
    InvalidPokemon(BattleError),
}

impl From<BattleError> for PokedexError {
    fn from(error: BattleError) -> Self {
        Self::InvalidPokemon(error)
    }
}

const CHARIZARD_MOVES: &[MoveEntry] = &[
    FLAMETHROWER,
    AIR_SLASH,
    DRAGON_CLAW,
    FIRE_BLAST,
    DRAGON_DANCE,
];
const VENUSAUR_MOVES: &[MoveEntry] = &[VINE_WHIP, RAZOR_LEAF, SLEEP_POWDER, SEED_BOMB, SOLAR_BEAM];
const BLASTOISE_MOVES: &[MoveEntry] = &[WATER_GUN, RAPID_SPIN, BITE, WATER_PULSE, HYDRO_PUMP];
const GENGAR_MOVES: &[MoveEntry] = &[SHADOW_BALL, HEX, HYPNOSIS, DARK_PULSE, SHADOW_SNEAK];
const LUCARIO_MOVES: &[MoveEntry] = &[
    AURA_SPHERE,
    METAL_CLAW,
    QUICK_ATTACK,
    SWORDS_DANCE,
    CLOSE_COMBAT,
];
const DRAGONITE_MOVES: &[MoveEntry] = &[
    WING_ATTACK,
    EXTREME_SPEED,
    FIRE_PUNCH,
    THUNDER_PUNCH,
    HURRICANE,
];

const POKEMON_DICT: &[PokemonEntry] = &[
    pokemon_entry(
        "Charizard",
        PokemonType::Fire,
        Some(PokemonType::Flying),
        stats(78, 84, 78, 109, 85, 100),
        CHARIZARD_MOVES,
    ),
    pokemon_entry(
        "Venusaur",
        PokemonType::Grass,
        Some(PokemonType::Poison),
        stats(80, 82, 83, 100, 100, 80),
        VENUSAUR_MOVES,
    ),
    pokemon_entry(
        "Blastoise",
        PokemonType::Water,
        None,
        stats(79, 83, 100, 85, 105, 78),
        BLASTOISE_MOVES,
    ),
    pokemon_entry(
        "Gengar",
        PokemonType::Ghost,
        Some(PokemonType::Poison),
        stats(60, 65, 60, 130, 75, 110),
        GENGAR_MOVES,
    ),
    pokemon_entry(
        "Lucario",
        PokemonType::Fighting,
        Some(PokemonType::Steel),
        stats(70, 110, 70, 115, 70, 90),
        LUCARIO_MOVES,
    ),
    pokemon_entry(
        "Dragonite",
        PokemonType::Dragon,
        Some(PokemonType::Flying),
        stats(91, 134, 95, 100, 100, 80),
        DRAGONITE_MOVES,
    ),
];

pub fn pokedex() -> &'static [PokemonEntry] {
    POKEMON_DICT
}

pub fn find_pokemon(name: &str) -> Option<&'static PokemonEntry> {
    POKEMON_DICT
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
}

pub fn build_pokemon_from_pokedex(
    species_name: &str,
    level: u8,
    stat_points: StatPoints,
    nature: Nature,
    move_names: [&str; 4],
) -> Result<Pokemon, PokedexError> {
    build_pokemon_from_pokedex_with_item(species_name, level, stat_points, nature, None, move_names)
}

pub fn build_pokemon_from_pokedex_with_item(
    species_name: &str,
    level: u8,
    stat_points: StatPoints,
    nature: Nature,
    item: Option<HeldItem>,
    move_names: [&str; 4],
) -> Result<Pokemon, PokedexError> {
    let species = find_pokemon(species_name).ok_or(PokedexError::PokemonNotFound)?;
    let moves = [
        find_learnable_move(species, move_names[0], 0)?.to_move(),
        find_learnable_move(species, move_names[1], 1)?.to_move(),
        find_learnable_move(species, move_names[2], 2)?.to_move(),
        find_learnable_move(species, move_names[3], 3)?.to_move(),
    ];

    Ok(Pokemon::new(PokemonSpec {
        name: species.name.to_string(),
        level,
        primary_type: species.primary_type,
        secondary_type: species.secondary_type,
        stats: species.base_stats,
        stat_points,
        nature,
        item,
        moves,
    })?)
}

fn find_learnable_move(
    species: &PokemonEntry,
    move_name: &str,
    slot: usize,
) -> Result<MoveEntry, PokedexError> {
    species
        .learnable_moves
        .iter()
        .copied()
        .find(|entry| entry.name.eq_ignore_ascii_case(move_name))
        .ok_or(PokedexError::MoveNotLearnable { slot })
}

const fn stats(
    hp: u16,
    attack: u16,
    defense: u16,
    special_attack: u16,
    special_defense: u16,
    speed: u16,
) -> Stats {
    Stats {
        hp,
        attack,
        defense,
        special_attack,
        special_defense,
        speed,
    }
}

const fn pokemon_entry(
    name: &'static str,
    primary_type: PokemonType,
    secondary_type: Option<PokemonType>,
    base_stats: Stats,
    learnable_moves: &'static [MoveEntry],
) -> PokemonEntry {
    PokemonEntry {
        name,
        primary_type,
        secondary_type,
        base_stats,
        learnable_moves,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn contains_final_evolution_entries() {
        assert_eq!(pokedex().len(), 6);
        assert!(find_pokemon("Charizard").is_some());
        assert!(find_pokemon("Dragonite").is_some());
        assert!(find_pokemon("Pikachu").is_none());
    }

    #[test]
    fn exposes_base_stats_and_learnable_moves_per_pokemon() {
        let charizard = find_pokemon("Charizard").unwrap();

        assert_eq!(charizard.base_stats.special_attack, 109);
        assert_eq!(charizard.base_stats.speed, 100);
        assert!(
            charizard
                .learnable_moves
                .iter()
                .any(|move_entry| move_entry.name == "Flamethrower")
        );
    }

    #[test]
    fn builds_pokemon_with_four_selected_moves() {
        let lucario = build_pokemon_from_pokedex(
            "Lucario",
            50,
            valid_stat_points(),
            Nature::neutral(),
            ["Aura Sphere", "Quick Attack", "Metal Claw", "Close Combat"],
        )
        .unwrap();

        assert_eq!(lucario.moves.len(), 4);
        assert_eq!(lucario.moves[1].name, "Quick Attack");
        assert_eq!(lucario.moves[1].priority, 1);
        assert!(lucario.item.is_none());
    }

    #[test]
    fn rejects_moves_missing_from_species_learnset() {
        let result = build_pokemon_from_pokedex(
            "Lucario",
            50,
            valid_stat_points(),
            Nature::neutral(),
            ["Aura Sphere", "Quick Attack", "Metal Claw", "Flamethrower"],
        );

        assert_eq!(result, Err(PokedexError::MoveNotLearnable { slot: 3 }));
    }
}
