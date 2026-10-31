use crate::BattleSide;
use crate::info::BattleError;
use crate::state::{BattleState, StateError, TeamPreviewObservation, TeamState};

/// An action available during team preview or a battle turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Three distinct roster slots in order, with the lead first.
    SelectTeam([usize; 3]),
    Move(usize),
    Switch(usize),
}

/// Fixed action dimension for both Trainers in every episode phase.
/// Indices 0..120 are distinct ordered team selections in lexicographic order,
/// 120..124 are move slots 0..4, and 124..130 are roster switches to slots 0..6.
pub const ACTION_SPACE_SIZE: usize = 130;

const ACTIONS: [Action; ACTION_SPACE_SIZE] = {
    let mut actions = [Action::Move(0); ACTION_SPACE_SIZE];
    let mut index = 0;
    let mut first = 0;

    while first < 6 {
        let mut second = 0;

        while second < 6 {
            let mut third = 0;

            while third < 6 {
                if first != second && first != third && second != third {
                    actions[index] = Action::SelectTeam([first, second, third]);
                    index += 1;
                }

                third += 1;
            }

            second += 1;
        }

        first += 1;
    }

    let mut slot = 0;

    while slot < 6 {
        if slot < 4 {
            actions[120 + slot] = Action::Move(slot);
        }

        actions[124 + slot] = Action::Switch(slot);
        slot += 1;
    }

    actions
};

impl Action {
    /// Returns the stable index, or `None` for duplicate selections or out-of-range slots.
    /// This conversion does not check whether the action is legal in the current state.
    pub fn to_index(self) -> Option<usize> {
        ACTIONS.iter().position(|&action| action == self)
    }

    /// Decodes an index in `0..ACTION_SPACE_SIZE`, independently of the current state.
    /// The first and last selections are `[0, 1, 2]` and `[5, 4, 3]`.
    pub fn from_index(index: usize) -> Option<Self> {
        ACTIONS.get(index).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    /// An indexed action lies outside `0..ACTION_SPACE_SIZE`.
    InvalidActionIndex,
    InvalidTeamSelection,
    UnavailableMove,
    InvalidSwitch,
    WrongPhase,
    BattleTerminated,
    /// The environment reached its turn limit; reset before taking another action.
    EpisodeTruncated,
    MissingRosters,
    InvalidState(StateError),
    Battle(BattleError),
}

impl BattleState {
    /// Returns player actions in stable move-slot, then roster-slot order.
    pub fn legal_player_actions(&self) -> Vec<Action> {
        self.legal_actions(BattleSide::Player)
    }

    /// Returns the requested Trainer's actions in move-slot, then roster-slot order.
    pub fn legal_actions(&self, side: BattleSide) -> Vec<Action> {
        if self.terminated {
            return Vec::new();
        }

        let team = match side {
            BattleSide::Player => &self.player,
            BattleSide::Opponent => &self.opponent,
        };
        let mut actions = Vec::new();

        if let Some(active) = team.slot_active() {
            actions.extend(
                team.roster()[active]
                    .move_availability
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, &available)| available.then_some(Action::Move(slot))),
            );
        }

        actions.extend(
            (0..6).filter_map(|slot| team.can_switch_to(slot).then_some(Action::Switch(slot))),
        );
        actions
    }

    pub fn validate_player_action(&self, action: Action) -> Result<(), ActionError> {
        self.validate_action(BattleSide::Player, action)
    }

    /// Validates either Trainer's action using the same rules as turn resolution.
    pub fn validate_action(&self, side: BattleSide, action: Action) -> Result<(), ActionError> {
        if self.terminated {
            return Err(ActionError::BattleTerminated);
        }

        match side {
            BattleSide::Player => &self.player,
            BattleSide::Opponent => &self.opponent,
        }
        .validate_action(action)
    }
}

impl TeamState {
    pub(crate) fn validate_action(&self, action: Action) -> Result<(), ActionError> {
        match action {
            Action::Move(slot)
                if self.slot_active().is_some_and(|active| {
                    self.roster()[active].move_availability.get(slot).copied() == Some(true)
                }) =>
            {
                Ok(())
            }
            Action::Move(_) => Err(ActionError::UnavailableMove),
            Action::Switch(slot) if self.can_switch_to(slot) => Ok(()),
            Action::Switch(_) => Err(ActionError::InvalidSwitch),
            Action::SelectTeam(_) => Err(ActionError::WrongPhase),
        }
    }
}

impl TeamPreviewObservation {
    /// Returns all 120 ordered three-Pokemon selections lexicographically.
    pub fn legal_player_actions(&self) -> Vec<Action> {
        ACTIONS[..120].to_vec()
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
    use super::{ACTION_SPACE_SIZE, Action, ActionError};
    use crate::BattleSide;
    use crate::state::{BattleState, PokemonState, TeamPreviewObservation, TeamState};

    #[test]
    fn stable_indices_cover_every_action_and_reject_malformed_slots() {
        let mut expected = Vec::new();

        for first in 0..6 {
            for second in 0..6 {
                for third in 0..6 {
                    if first != second && first != third && second != third {
                        expected.push(Action::SelectTeam([first, second, third]));
                    }
                }
            }
        }

        expected.extend((0..4).map(Action::Move));
        expected.extend((0..6).map(Action::Switch));

        assert_eq!(ACTION_SPACE_SIZE, 130);
        assert_eq!(expected.len(), ACTION_SPACE_SIZE);

        for (index, action) in expected.into_iter().enumerate() {
            assert_eq!(Action::from_index(index), Some(action));
            assert_eq!(action.to_index(), Some(index));
        }

        for index in [ACTION_SPACE_SIZE, usize::MAX] {
            assert_eq!(Action::from_index(index), None);
        }

        for action in [
            Action::SelectTeam([0, 0, 1]),
            Action::SelectTeam([0, 1, 0]),
            Action::SelectTeam([0, 1, 1]),
            Action::SelectTeam([6, 1, 2]),
            Action::SelectTeam([0, 6, 2]),
            Action::SelectTeam([0, 1, usize::MAX]),
            Action::Move(4),
            Action::Move(usize::MAX),
            Action::Switch(6),
            Action::Switch(usize::MAX),
        ] {
            assert_eq!(action.to_index(), None);
        }
    }

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
        // The other Trainer follows the same slot validation without a swapped state.
        assert_eq!(
            state.legal_actions(BattleSide::Opponent),
            vec![
                Action::Move(0),
                Action::Move(1),
                Action::Move(2),
                Action::Move(3),
                Action::Switch(1),
                Action::Switch(2),
            ]
        );

        for side in [BattleSide::Player, BattleSide::Opponent] {
            for action in state.legal_actions(side) {
                assert_eq!(state.validate_action(side, action), Ok(()));
            }

            assert_eq!(
                state.validate_action(side, Action::Move(4)),
                Err(ActionError::UnavailableMove)
            );
            assert_eq!(
                state.validate_action(side, Action::Switch(6)),
                Err(ActionError::InvalidSwitch)
            );
            assert_eq!(
                state.validate_action(side, Action::SelectTeam([0, 1, 2])),
                Err(ActionError::WrongPhase)
            );
        }

        let mut terminated = state.clone();
        terminated.terminated = true;

        assert!(terminated.legal_player_actions().is_empty());
        assert_eq!(
            terminated.validate_player_action(Action::Move(0)),
            Err(ActionError::BattleTerminated)
        );
        assert!(terminated.legal_actions(BattleSide::Opponent).is_empty());
        assert_eq!(
            terminated.validate_action(BattleSide::Opponent, Action::Move(0)),
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
