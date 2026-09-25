use crate::info::Pokemon;
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

    /// Resolves a turn using immutable calculation data for the corresponding roster slots.
    /// HP, selection, active slots, and move availability come from this battle's state.
    /// A failed turn leaves the state unchanged. An available move missing from the
    /// supplied roster returns `ActionError::Battle(BattleError::InvalidMoveIndex)`.
    pub fn play_turn_with_rosters(
        &mut self,
        player: &[Pokemon; 6],
        opponent: &[Pokemon; 6],
        player_action: Action,
        opponent_action: Action,
    ) -> Result<&BattleState, ActionError> {
        self.play_turn(
            player_action,
            opponent_action,
            |state, action, opponent_action| {
                crate::battle_logic::resolve_turn(state, player, opponent, action, opponent_action)
            },
        )
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

        let replacement_pending = self.state.player.slot_active().is_none()
            || self.state.opponent.slot_active().is_none();
        let mut next = self.state.clone();

        next.player.validate_action(player_action)?;
        next.opponent.validate_action(opponent_action)?;

        if let Action::Switch(slot) = player_action {
            next.player
                .switch_to(slot)
                .map_err(|_| ActionError::InvalidSwitch)?;
        }

        if let Action::Switch(slot) = opponent_action {
            next.opponent
                .switch_to(slot)
                .map_err(|_| ActionError::InvalidSwitch)?;
        }

        if !replacement_pending
            && (matches!(player_action, Action::Move(_))
                || matches!(opponent_action, Action::Move(_)))
        {
            resolve_turn(&mut next, player_action, opponent_action)?;
        }

        next.terminated =
            !next.player.has_available_selected() || !next.opponent.has_available_selected();
        self.state = next;
        Ok(&self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::Battle;
    use crate::{Action, ActionError, BattleState, PokemonState, TeamState};

    #[test]
    fn failed_resolution_discards_all_runtime_mutations() {
        let initial = state();
        let mut battle = Battle::new(initial.clone());

        assert_eq!(
            battle.play_turn(Action::Switch(1), Action::Move(0), |next, _, _| {
                next.player.damage_active(25).unwrap();
                next.opponent = TeamState::new(
                    team([false; 4]).roster().clone(),
                    [false, true, true, true, false, false],
                    Some(1),
                )
                .unwrap();
                next.terminated = true;
                Err(ActionError::InvalidSwitch)
            }),
            Err(ActionError::InvalidSwitch)
        );
        assert_eq!(battle.state(), &initial);
    }

    #[test]
    fn validates_delegates_and_stops_after_termination() {
        let mut battle = Battle::new(state());

        assert_eq!(
            battle.play_turn(Action::Switch(0), Action::Move(0), |_, _, _| unreachable!()),
            Err(ActionError::InvalidSwitch)
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

                    state.opponent.damage_active(1_000).unwrap();
                    state.opponent.switch_to(1).unwrap();
                    state.opponent.damage_active(1_000).unwrap();
                    state.opponent.switch_to(2).unwrap();
                    state.opponent.damage_active(1_000).unwrap();
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

    #[test]
    fn non_terminal_faint_requires_switch_before_moves_resume() {
        let mut battle = Battle::new(state());

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                state.player.damage_active(1_000).unwrap();
                Ok(())
            })
            .unwrap();
        assert_eq!(state.player.slot_active(), None);
        assert!(!state.terminated);
        assert_eq!(
            state.legal_player_actions(),
            vec![Action::Switch(1), Action::Switch(2)]
        );
        assert_eq!(
            battle.play_turn(Action::Move(0), Action::Move(0), |_, _, _| unreachable!()),
            Err(ActionError::UnavailableMove)
        );

        let state = battle
            .play_turn(Action::Switch(1), Action::Move(0), |_, _, _| {
                panic!("forced replacements do not resolve moves")
            })
            .unwrap();
        assert_eq!(state.player.slot_active(), Some(1));

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                state.opponent.damage_active(1).unwrap();
                Ok(())
            })
            .unwrap();
        assert_eq!(state.opponent.roster()[0].hp_curr(), 99);
    }

    #[test]
    fn switch_only_turn_does_not_run_move_resolution() {
        let mut battle = Battle::new(state());

        let state = battle
            .play_turn(Action::Switch(1), Action::Switch(1), |_, _, _| {
                panic!("switch-only turns have no moves to resolve")
            })
            .unwrap();
        assert_eq!(state.player.slot_active(), Some(1));
        assert_eq!(state.opponent.slot_active(), Some(1));
    }

    #[test]
    fn battle_ends_only_after_all_selected_opponents_faint() {
        let mut battle = Battle::new(state());

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                state.opponent.damage_active(1_000).unwrap();
                Ok(())
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Switch(1), |_, _, _| {
                panic!("forced replacements do not resolve moves")
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                state.opponent.damage_active(1_000).unwrap();
                Ok(())
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Switch(2), |_, _, _| {
                panic!("forced replacements do not resolve moves")
            })
            .unwrap();
        assert!(!state.terminated);

        let state = battle
            .play_turn(Action::Move(0), Action::Move(0), |state, _, _| {
                state.opponent.damage_active(1_000).unwrap();
                Ok(())
            })
            .unwrap();
        assert!(state.terminated);
        assert!(
            state.opponent.roster()[3..]
                .iter()
                .all(|pokemon| pokemon.hp_curr() > 0)
        );
        assert!(state.legal_player_actions().is_empty());
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
