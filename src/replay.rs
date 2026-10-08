use crate::info::Pokemon;
use crate::{Action, ActionError, Environment, StepOutcome};

/// The inputs needed to reproduce a completed episode through the normal environment.
/// Stores initial rosters (including HP) and actions, not intermediate battle states.
/// Reproduction requires the same library versions and target platform.
#[derive(Clone, Debug, PartialEq)]
pub struct BattleReplay {
    pub player: [Pokemon; 6],
    pub opponent: [Pokemon; 6],
    pub opponent_selection: [usize; 3],
    pub seed: u64,
    /// Ordered (player, opponent) actions, including team selection and forced replacements.
    /// The first player action must select a team; its opponent action is ignored,
    /// just as it is in `Environment::step`.
    pub actions: Vec<(Action, Action)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// Invalid initial rosters or opponent selection.
    InvalidSetup(ActionError),
    /// A failed step, including actions supplied after battle termination.
    InvalidAction {
        /// Zero-based index into `BattleReplay::actions`.
        step: usize,
        error: ActionError,
    },
    /// The actions ended before the battle terminated (including an empty sequence).
    Incomplete,
}

impl BattleReplay {
    /// Replays every action through `Environment::step`, returning its outcomes in order.
    /// The final outcome contains the terminal observation. The step trace uses the
    /// existing observations, rewards, events, and termination flags; no separate log is stored.
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

        if !outcomes.last().is_some_and(|outcome| outcome.terminated) {
            return Err(ReplayError::Incomplete);
        }

        Ok(outcomes)
    }
}
