use crate::state::{BattleState, TeamPreviewObservation};

/// An action available during team preview or a battle turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Three distinct roster slots in order, with the lead first.
    SelectTeam([usize; 3]),
    Move(usize),
    Switch(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    InvalidTeamSelection,
    UnavailableMove,
    InvalidSwitch,
    WrongPhase,
    BattleTerminated,
}

impl BattleState {
    /// Returns player actions in stable move-slot, then roster-slot order.
    pub fn legal_player_actions(&self) -> Vec<Action> {
        if self.terminated {
            return Vec::new();
        }

        let mut actions = Vec::new();

        if let Some(active) = self.player.slot_active() {
            actions.extend(
                self.player.roster()[active]
                    .move_availability
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, &available)| available.then_some(Action::Move(slot))),
            );
        }

        actions.extend((0..6).filter_map(|slot| {
            (self.player.selected()[slot]
                && self.player.slot_active() != Some(slot)
                && self.player.roster()[slot].hp_curr() > 0)
                .then_some(Action::Switch(slot))
        }));
        actions
    }

    pub fn validate_player_action(&self, action: Action) -> Result<(), ActionError> {
        if self.terminated {
            return Err(ActionError::BattleTerminated);
        }

        match action {
            Action::Move(slot)
                if self.player.slot_active().is_some_and(|active| {
                    self.player.roster()[active]
                        .move_availability
                        .get(slot)
                        .copied()
                        == Some(true)
                }) =>
            {
                Ok(())
            }
            Action::Move(_) => Err(ActionError::UnavailableMove),
            Action::Switch(slot)
                if self.player.selected().get(slot).copied() == Some(true)
                    && self.player.slot_active() != Some(slot)
                    && self.player.roster()[slot].hp_curr() > 0 =>
            {
                Ok(())
            }
            Action::Switch(_) => Err(ActionError::InvalidSwitch),
            Action::SelectTeam(_) => Err(ActionError::WrongPhase),
        }
    }
}

impl TeamPreviewObservation {
    /// Returns all 120 ordered three-Pokemon selections lexicographically.
    pub fn legal_player_actions(&self) -> Vec<Action> {
        let mut actions = Vec::with_capacity(120);

        for first in 0..6 {
            for second in 0..6 {
                for third in 0..6 {
                    if first != second && first != third && second != third {
                        actions.push(Action::SelectTeam([first, second, third]));
                    }
                }
            }
        }

        actions
    }

    pub fn validate_player_action(&self, action: Action) -> Result<(), ActionError> {
        match action {
            Action::SelectTeam(slots)
                if slots.iter().all(|&slot| slot < 6)
                    && slots[0] != slots[1]
                    && slots[0] != slots[2]
                    && slots[1] != slots[2] =>
            {
                Ok(())
            }
            Action::SelectTeam(_) => Err(ActionError::InvalidTeamSelection),
            _ => Err(ActionError::WrongPhase),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, ActionError};
    use crate::state::{BattleState, PokemonState, TeamPreviewObservation, TeamState};

    #[test]
    fn exposes_and_validates_deterministic_actions_without_mutating_state() {
        let preview = TeamPreviewObservation {
            player: roster(100),
            opponent: roster(100),
        };
        let selections = preview.legal_player_actions();

        assert_eq!(selections.len(), 120);
        assert_eq!(selections.first(), Some(&Action::SelectTeam([0, 1, 2])));
        assert_eq!(selections.last(), Some(&Action::SelectTeam([5, 4, 3])));
        assert!(
            selections
                .iter()
                .all(|&action| preview.validate_player_action(action).is_ok())
        );
        assert_eq!(
            preview.validate_player_action(Action::SelectTeam([0, 0, 1])),
            Err(ActionError::InvalidTeamSelection)
        );
        assert_eq!(
            preview.validate_player_action(Action::SelectTeam([0, 1, 6])),
            Err(ActionError::InvalidTeamSelection)
        );
        assert_eq!(
            preview.validate_player_action(Action::Move(0)),
            Err(ActionError::WrongPhase)
        );

        let mut player_roster = roster(100);
        player_roster[0].move_availability = [true, false, true, false];
        player_roster[2] = PokemonState::new(0, 100, [true; 4]).unwrap();

        let state = BattleState {
            player: TeamState::new(
                player_roster,
                [true, true, true, false, false, false],
                Some(0),
            )
            .unwrap(),
            opponent: TeamState::new(
                roster(100),
                [true, true, true, false, false, false],
                Some(0),
            )
            .unwrap(),
            terminated: false,
        };
        let unchanged = state.clone();
        let actions = state.legal_player_actions();

        assert_eq!(
            actions,
            vec![Action::Move(0), Action::Move(2), Action::Switch(1)]
        );
        assert!(
            actions
                .iter()
                .all(|&action| state.validate_player_action(action).is_ok())
        );
        assert_eq!(
            state.validate_player_action(Action::Move(4)),
            Err(ActionError::UnavailableMove)
        );
        assert_eq!(
            state.validate_player_action(Action::Move(1)),
            Err(ActionError::UnavailableMove)
        );
        assert_eq!(
            state.validate_player_action(Action::Switch(0)),
            Err(ActionError::InvalidSwitch)
        );
        assert_eq!(
            state.validate_player_action(Action::Switch(2)),
            Err(ActionError::InvalidSwitch)
        );
        assert_eq!(
            state.validate_player_action(Action::SelectTeam([0, 1, 2])),
            Err(ActionError::WrongPhase)
        );
        assert_eq!(state, unchanged);

        let mut terminated = state.clone();
        terminated.terminated = true;
        assert!(terminated.legal_player_actions().is_empty());
        assert_eq!(
            terminated.validate_player_action(Action::Move(0)),
            Err(ActionError::BattleTerminated)
        );

        let replacement = BattleState {
            player: TeamState::new(roster(100), [true, true, true, false, false, false], None)
                .unwrap(),
            opponent: state.opponent.clone(),
            terminated: false,
        };
        let actions = replacement.legal_player_actions();

        assert_eq!(
            actions,
            vec![Action::Switch(0), Action::Switch(1), Action::Switch(2)]
        );
        assert!(
            actions
                .iter()
                .all(|&action| replacement.validate_player_action(action).is_ok())
        );
        assert_eq!(
            replacement.validate_player_action(Action::Move(0)),
            Err(ActionError::UnavailableMove)
        );
    }

    fn roster(hp: u32) -> [PokemonState; 6] {
        std::array::from_fn(|_| PokemonState::new(hp, hp, [true; 4]).unwrap())
    }
}
