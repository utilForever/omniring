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
                observation: Observation::Battle(observation(state, self.opponent_revealed)),
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
            observation: Observation::Battle(observation(state, self.opponent_revealed)),
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

fn observation(state: &BattleState, opponent_revealed: [bool; 6]) -> BattleObservation {
    BattleObservation {
        player: state.player.clone(),
        opponent: OpponentObservation::new(&state.opponent, opponent_revealed)
            .expect("the active opponent is always revealed"),
        terminated: state.terminated,
    }
}
