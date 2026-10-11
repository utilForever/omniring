use crate::info::Pokemon;
use crate::{Action, ActionError, Environment, StepOutcome};

/// The inputs needed to reproduce a completed episode through the normal environment.
/// Stores initial rosters (including HP) and actions, not intermediate battle states.
/// Reproduction requires the same library versions and target platform.
#[derive(Clone, Debug, PartialEq)]
pub struct BattleReplay {
    pub player: [Pokemon; 6],
    pub opponent: [Pokemon; 6],
    /// Legacy setup selection, still validated by the environment constructor.
    /// The first recorded opponent action determines the actual selection.
    pub opponent_selection: [usize; 3],
    pub seed: u64,
    /// Ordered (player, opponent) actions, including team selection and forced replacements.
    /// Both actions in the first pair must be `SelectTeam`, as in `Environment::step`.
    /// Older records using an opponent `Move`/`Switch` placeholder must be updated.
    pub actions: Vec<(Action, Action)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// Invalid initial rosters or opponent selection.
    InvalidSetup(ActionError),
    /// A failed step, including actions supplied after termination or truncation.
    InvalidAction {
        /// Zero-based index into `BattleReplay::actions`.
        step: usize,
        error: ActionError,
    },
    /// The actions ended before termination or truncation (including an empty sequence).
    Incomplete,
}

impl BattleReplay {
    /// Replays every action through `Environment::step`, returning its outcomes in order.
    /// The final outcome contains the observation at termination or truncation. The step trace
    /// uses the existing observations, rewards, events, and flags; no separate log is stored.
    /// Returns the first setup/action error, or `Incomplete` if more actions are needed.
    pub fn run(&self) -> Result<Vec<StepOutcome>, ReplayError> {
        let mut environment = Environment::from_rosters_with_seed(
            self.player.clone(),
            self.opponent.clone(),
            self.opponent_selection,
            self.seed,
        )
        .map_err(ReplayError::InvalidSetup)?;
        let mut outcomes = Vec::new();

        for (step, &(player, opponent)) in self.actions.iter().enumerate() {
            outcomes.push(
                environment
                    .step(player, opponent)
                    .map_err(|error| ReplayError::InvalidAction { step, error })?,
            );
        }

        if !outcomes
            .last()
            .is_some_and(|outcome| outcome.terminated || outcome.truncated)
        {
            return Err(ReplayError::Incomplete);
        }

        Ok(outcomes)
    }
}
