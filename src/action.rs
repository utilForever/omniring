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
