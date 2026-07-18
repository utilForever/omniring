#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    InvalidHp,
}

/// The battle state of one Pokemon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PokemonState {
    hp_curr: u32,
    hp_max: u32,
    /// Availability mask for the Pokemon's four move slots.
    pub move_availability: [bool; 4],
}

impl PokemonState {
    pub fn new(
        hp_curr: u32,
        hp_max: u32,
        move_availability: [bool; 4],
    ) -> Result<Self, StateError> {
        if hp_max == 0 || hp_curr > hp_max {
            return Err(StateError::InvalidHp);
        }

        Ok(Self {
            hp_curr,
            hp_max,
            move_availability,
        })
    }

    pub fn hp_curr(&self) -> u32 {
        self.hp_curr
    }

    pub fn hp_max(&self) -> u32 {
        self.hp_max
    }
}

#[cfg(test)]
mod tests {
    use super::{PokemonState, StateError};

    #[test]
    fn rejects_invalid_pokemon_state() {
        assert_eq!(
            PokemonState::new(101, 100, [true; 4]),
            Err(StateError::InvalidHp)
        );
        assert_eq!(
            PokemonState::new(0, 0, [true; 4]),
            Err(StateError::InvalidHp)
        );
    }
}
