use crate::{
    Action, ActionError, BattleObservation, BattleState, OpponentObservation, PokemonState,
    TeamPreviewObservation, TeamState, calculate_reward,
};

/// An observation returned to the player during an episode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    TeamPreview(TeamPreviewObservation),
    Battle(BattleObservation),
}

/// A minimal episode loop around a battle-state transition function.
pub struct Environment<F> {
    preview: TeamPreviewObservation,
    opponent: TeamState,
    state: Option<BattleState>,
    opponent_revealed: [bool; 6],
    transition: F,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepOutcome {
    pub observation: Observation,
    pub reward: f32,
    pub terminated: bool,
}

impl<F> Environment<F>
where
    F: FnMut(&mut BattleState, Action),
{
    pub fn new(
        preview: TeamPreviewObservation,
        opponent_selection: [usize; 3],
        transition: F,
    ) -> Result<Self, ActionError> {
        preview.validate_player_action(Action::SelectTeam(opponent_selection))?;

        let opponent = selected_team(preview.opponent.clone(), opponent_selection)?;

        Ok(Self {
            preview,
            opponent,
            state: None,
            opponent_revealed: [false; 6],
            transition,
        })
    }

    pub fn reset(&mut self) -> Observation {
        self.state = None;
        self.opponent_revealed = [false; 6];
        Observation::TeamPreview(self.preview.clone())
    }

    pub fn step(&mut self, action: Action) -> Result<StepOutcome, ActionError> {
        if self.state.is_none() {
            self.preview.validate_player_action(action)?;

            let Action::SelectTeam(selection) = action else {
                unreachable!();
            };

            let player = selected_team(self.preview.player.clone(), selection)?;
            let opponent = self.opponent.clone();

            self.opponent_revealed[opponent.slot_active().unwrap()] = true;

            let state = self.state.insert(BattleState {
                player,
                opponent,
                terminated: false,
            });

            return Ok(StepOutcome {
                observation: Observation::Battle(observation(state, self.opponent_revealed)?),
                reward: 0.0,
                terminated: false,
            });
        }

        let state = self.state.as_mut().unwrap();
        state.validate_player_action(action)?;

        let previous = state.clone();

        (self.transition)(state, action);

        if let Some(active) = state.opponent.slot_active() {
            self.opponent_revealed[active] = true;
        }

        Ok(StepOutcome {
            observation: Observation::Battle(observation(state, self.opponent_revealed)?),
            reward: calculate_reward(&previous, state),
            terminated: state.terminated,
        })
    }
}

fn selected_team(
    roster: [PokemonState; 6],
    selection: [usize; 3],
) -> Result<TeamState, ActionError> {
    let mut selected = [false; 6];

    for slot in selection {
        selected[slot] = true;
    }

    TeamState::new(roster, selected, Some(selection[0]))
        .map_err(|_| ActionError::InvalidTeamSelection)
}

fn observation(
    state: &BattleState,
    opponent_revealed: [bool; 6],
) -> Result<BattleObservation, ActionError> {
    Ok(BattleObservation {
        player: state.player.clone(),
        opponent: OpponentObservation::new(&state.opponent, opponent_revealed)
            .map_err(|_| ActionError::InvalidTeamSelection)?,
        terminated: state.terminated,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{Environment, Observation};
    use crate::{
        Action, ActionError, BattleState, PokemonState, TeamPreviewObservation, TeamState,
    };

    #[test]
    fn runs_a_hidden_information_episode_from_preview_to_reset() {
        let preview = TeamPreviewObservation {
            player: roster(100),
            opponent: roster(100),
        };

        assert!(Environment::new(preview.clone(), [0, 0, 1], |_, _| {}).is_err());

        let terminal = state([0; 3], true);
        let transitions = Cell::new(0);
        let mut environment = Environment::new(preview.clone(), [0, 1, 2], |state, _| {
            transitions.set(transitions.get() + 1);
            *state = terminal.clone();
        })
        .unwrap();

        assert_eq!(
            environment.reset(),
            Observation::TeamPreview(preview.clone())
        );
        assert_eq!(
            environment.step(Action::Move(0)),
            Err(ActionError::WrongPhase)
        );

        let selected = environment.step(Action::SelectTeam([0, 1, 2])).unwrap();

        assert_eq!(selected.reward, 0.0);
        assert!(!selected.terminated);

        let Observation::Battle(observation) = selected.observation else {
            panic!("team selection must start the battle");
        };

        assert_eq!(
            observation.opponent.selection_revealed(),
            &[true, false, false, false, false, false]
        );
        assert_eq!(transitions.get(), 0);

        let outcome = environment.step(Action::Move(0)).unwrap();

        assert!((outcome.reward - 1.4).abs() < 1e-6);
        assert!(outcome.terminated);
        assert!(matches!(outcome.observation, Observation::Battle(_)));
        assert_eq!(transitions.get(), 1);
        assert_eq!(
            environment.step(Action::Move(0)),
            Err(ActionError::BattleTerminated)
        );
        assert_eq!(transitions.get(), 1);
        assert_eq!(
            environment.reset(),
            Observation::TeamPreview(preview.clone())
        );
    }

    #[test]
    fn reports_a_transition_that_breaks_the_reveal_invariant() {
        let preview = TeamPreviewObservation {
            player: roster(100),
            opponent: roster(100),
        };
        let mut environment = Environment::new(preview, [0, 1, 2], |state, _| {
            state.opponent = TeamState::new(
                roster(100),
                [false, true, true, true, false, false],
                Some(1),
            )
            .unwrap();
        })
        .unwrap();

        environment.step(Action::SelectTeam([0, 1, 2])).unwrap();
        assert_eq!(
            environment.step(Action::Move(0)),
            Err(ActionError::InvalidTeamSelection)
        );
    }

    fn state(opponent_hp: [u32; 3], terminated: bool) -> BattleState {
        BattleState {
            player: team([100; 3]),
            opponent: team(opponent_hp),
            terminated,
        }
    }

    fn team(hp: [u32; 3]) -> TeamState {
        TeamState::new(
            std::array::from_fn(|slot| {
                PokemonState::new(if slot < 3 { hp[slot] } else { 100 }, 100, [true; 4]).unwrap()
            }),
            [true, true, true, false, false, false],
            hp.iter().position(|&hp| hp > 0),
        )
        .unwrap()
    }

    fn roster(hp: u32) -> [PokemonState; 6] {
        std::array::from_fn(|_| PokemonState::new(hp, hp, [true; 4]).unwrap())
    }
}
