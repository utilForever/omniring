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

#[cfg(test)]
mod tests {
    use super::calculate_reward;
    use crate::state::{BattleState, PokemonState, TeamState};

    #[test]
    fn rewards_hp_faints_and_terminal_outcomes_once() {
        let initial = state([100; 3], [100; 3], false);
        let progress = state([90; 3], [80; 3], false);

        assert_close(calculate_reward(&initial, &progress), 0.01);

        let win = state([100; 3], [0; 3], true);
        let loss = state([0; 3], [100; 3], true);
        let draw = state([0; 3], [0; 3], true);
        let stopped = state([100; 3], [100; 3], true);

        assert_close(calculate_reward(&initial, &win), 1.4);
        assert_close(calculate_reward(&initial, &loss), -1.4);
        assert_eq!(calculate_reward(&initial, &draw), 0.0);
        assert_eq!(calculate_reward(&initial, &stopped), 0.0);
        assert_eq!(calculate_reward(&win, &win), 0.0);
    }

    fn state(player_hp: [u32; 3], opponent_hp: [u32; 3], terminated: bool) -> BattleState {
        BattleState {
            player: team(player_hp),
            opponent: team(opponent_hp),
            terminated,
        }
    }

    fn team(hp: [u32; 3]) -> TeamState {
        let roster = std::array::from_fn(|slot| {
            PokemonState::new(if slot < 3 { hp[slot] } else { 100 }, 100, [true; 4]).unwrap()
        });
        let active = hp.iter().position(|&hp| hp > 0);

        TeamState::new(roster, [true, true, true, false, false, false], active).unwrap()
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
    }
}
