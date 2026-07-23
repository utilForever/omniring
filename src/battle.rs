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

#[cfg(test)]
mod tests {
    use super::Battle;
    use crate::{Action, ActionError, BattleState, PokemonState, TeamState};

    #[test]
    fn validates_delegates_and_stops_after_termination() {
        let mut battle = Battle::new(state());

        assert_eq!(
            battle.play_turn(Action::Switch(1), Action::Move(0), |_, _, _| unreachable!()),
            Err(ActionError::WrongPhase)
        );
        assert_eq!(
            battle.play_turn(Action::Move(0), Action::Move(1), |_, _, _| unreachable!()),
            Err(ActionError::UnavailableMove)
        );
        assert_eq!(
            battle.play_turn(Action::Move(0), Action::Move(0), |_, _, _| Err(
                ActionError::InvalidSwitch
            )),
            Err(ActionError::InvalidSwitch)
        );

        let state = battle
            .play_turn(
                Action::Move(1),
                Action::Move(2),
                |state, player, opponent| {
                    assert_eq!((player, opponent), (Action::Move(1), Action::Move(2)));
                    state.terminated = true;
                    Ok(())
                },
            )
            .unwrap();

        assert!(state.terminated);
        assert_eq!(
            battle.play_turn(Action::Move(0), Action::Move(0), |_, _, _| unreachable!()),
            Err(ActionError::BattleTerminated)
        );
    }

    fn state() -> BattleState {
        BattleState {
            player: team([true; 4]),
            opponent: team([true, false, true, true]),
            terminated: false,
        }
    }

    fn team(move_availability: [bool; 4]) -> TeamState {
        TeamState::new(
            std::array::from_fn(|_| PokemonState::new(100, 100, move_availability).unwrap()),
            [true, true, true, false, false, false],
            Some(0),
        )
        .unwrap()
    }
}
