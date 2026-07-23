//! Public entry points for the Pokemon Champions single-battle simulator.
//!
//! `pokedex` stores Pokemon species data, `techdex` stores move data, `info`
//! stores shared battle types and rules, and `battle` resolves turn order and
//! damage.

pub mod battle;
pub mod info;
pub mod pokedex;
pub mod techdex;

pub use pokedex::{
    build_pokemon_from_pokedex, build_pokemon_from_pokedex_with_item, find_pokemon, pokedex,
};
pub use techdex::{find_move, techdex};
