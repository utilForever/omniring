/// The minimum internal simulator state of a Pokemon Champions Single Battle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BattleState {
    pub player: TeamState,
    pub opponent: TeamState,
    pub terminated: bool,
}

/// The public six-Pokemon rosters shown before team selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamPreviewObservation {
    pub player: [PokemonState; 6],
    pub opponent: [PokemonState; 6],
}

/// One Trainer's six-Pokemon roster and completed three-Pokemon selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamState {
    roster: [PokemonState; 6],
    selected: [bool; 6],
    slot_active: Option<usize>,
}

impl TeamState {
    pub fn new(
        roster: [PokemonState; 6],
        selected: [bool; 6],
        slot_active: Option<usize>,
    ) -> Result<Self, StateError> {
        if selected.iter().filter(|&&slot| slot).count() != 3 {
            return Err(StateError::InvalidSelectionCount);
        }

        if let Some(slot_active) = slot_active {
            let Some(pokemon) = roster.get(slot_active) else {
                return Err(StateError::InvalidActiveSlot);
            };

            if !selected[slot_active] || pokemon.hp_curr() == 0 {
                return Err(StateError::InvalidActiveSlot);
            }
        }

        Ok(Self {
            roster,
            selected,
            slot_active,
        })
    }

    pub fn roster(&self) -> &[PokemonState; 6] {
        &self.roster
    }

    pub fn selected(&self) -> &[bool; 6] {
        &self.selected
    }

    pub fn slot_active(&self) -> Option<usize> {
        self.slot_active
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    InvalidSelectionCount,
    InvalidActiveSlot,
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
    use super::{BattleState, PokemonState, StateError, TeamPreviewObservation, TeamState};

    #[test]
    fn represents_a_single_battle() {
        let preview = TeamPreviewObservation {
            player: roster(100),
            opponent: roster(80),
        };
        let player = TeamState::new(
            preview.player.clone(),
            [true, true, true, false, false, false],
            Some(0),
        )
        .unwrap();
        let opponent = TeamState::new(
            preview.opponent.clone(),
            [false, true, false, true, false, true],
            Some(1),
        )
        .unwrap();
        let state = BattleState {
            player,
            opponent,
            terminated: false,
        };

        assert_eq!(preview.player.len(), 6);
        assert_eq!(preview.opponent.len(), 6);
        assert_eq!(state.player.roster().len(), 6);
        assert_eq!(state.opponent.roster()[1].hp_curr(), 80);
        assert!(!state.terminated);
    }

    #[test]
    fn rejects_invalid_team_state() {
        assert_eq!(
            TeamState::new(roster(100), [true; 6], None),
            Err(StateError::InvalidSelectionCount)
        );
        assert_eq!(
            TeamState::new(
                roster(100),
                [true, true, true, false, false, false],
                Some(5),
            ),
            Err(StateError::InvalidActiveSlot)
        );
        assert_eq!(
            TeamState::new(
                roster(100),
                [true, true, true, false, false, false],
                Some(6),
            ),
            Err(StateError::InvalidActiveSlot)
        );

        let mut fainted = roster(100);
        fainted[0] = PokemonState::new(0, 100, [true; 4]).unwrap();

        assert_eq!(
            TeamState::new(fainted, [true, true, true, false, false, false], Some(0),),
            Err(StateError::InvalidActiveSlot)
        );
    }

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

    fn roster(hp: u32) -> [PokemonState; 6] {
        std::array::from_fn(|_| PokemonState::new(hp, hp, [true; 4]).unwrap())
    }
}
