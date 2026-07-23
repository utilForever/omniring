use crate::{Action, ActionError, BattleState};

/// A single battle that owns its state and delegates turn resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Battle {
    state: BattleState,
}

impl Battle {
    pub fn new(state: BattleState) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &BattleState {
        &self.state
    }

    pub fn play_turn(
        &mut self,
        player_action: Action,
        opponent_action: Action,
        resolve_turn: impl FnOnce(&mut BattleState, Action, Action) -> Result<(), ActionError>,
    ) -> Result<&BattleState, ActionError> {
        if self.state.terminated {
            return Err(ActionError::BattleTerminated);
        }

        if !matches!(player_action, Action::Move(_)) || !matches!(opponent_action, Action::Move(_))
        {
            return Err(ActionError::WrongPhase);
        }

        self.state.player.validate_action(player_action)?;
        self.state.opponent.validate_action(opponent_action)?;
        resolve_turn(&mut self.state, player_action, opponent_action)?;
        Ok(&self.state)
    }
}
