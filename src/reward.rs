use crate::state::{BattleState, TeamState};

pub const WIN_REWARD: f32 = 1.0;
pub const LOSS_REWARD: f32 = -1.0;
pub const FAINT_REWARD: f32 = 0.1;
pub const HP_PROGRESS_REWARD: f32 = 0.1;

/// Calculates a zero-sum reward from the player's perspective.
///
/// A newly completed battle adds `WIN_REWARD` or `LOSS_REWARD` when exactly
/// one selected team has fainted. A termination with both or neither team
/// fainted has no outcome reward. Each newly fainted Pokemon adds or subtracts
/// `FAINT_REWARD`, and the change in mean normalized HP across the selected
/// team is scaled by `HP_PROGRESS_REWARD`.
pub fn calculate_reward(previous: &BattleState, current: &BattleState) -> f32 {
    if previous.terminated {
        return 0.0;
    }

    let outcome = if current.terminated {
        match (
            all_selected_fainted(&current.player),
            all_selected_fainted(&current.opponent),
        ) {
            (false, true) => WIN_REWARD,
            (true, false) => LOSS_REWARD,
            _ => 0.0,
        }
    } else {
        0.0
    };
    let faint_progress = newly_fainted(&previous.opponent, &current.opponent) as f32
        - newly_fainted(&previous.player, &current.player) as f32;
    let hp_progress = normalized_hp(&previous.opponent)
        - normalized_hp(&current.opponent)
        - normalized_hp(&previous.player)
        + normalized_hp(&current.player);

    outcome + FAINT_REWARD * faint_progress + HP_PROGRESS_REWARD * hp_progress
}

fn all_selected_fainted(team: &TeamState) -> bool {
    (0..team.roster().len())
        .filter(|&slot| team.selected()[slot])
        .all(|slot| team.roster()[slot].hp_curr() == 0)
}

fn newly_fainted(previous: &TeamState, current: &TeamState) -> usize {
    (0..previous.roster().len())
        .filter(|&slot| {
            previous.selected()[slot]
                && previous.roster()[slot].hp_curr() > 0
                && current.roster()[slot].hp_curr() == 0
        })
        .count()
}

fn normalized_hp(team: &TeamState) -> f32 {
    team.roster()
        .iter()
        .zip(team.selected())
        .filter(|(_, selected)| **selected)
        .map(|(pokemon, _)| pokemon.hp_curr() as f32 / pokemon.hp_max() as f32)
        .sum::<f32>()
        / 3.0
}
