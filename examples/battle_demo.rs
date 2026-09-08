use omniring::info::{Nature, Pokemon, StatPoints};
use omniring::pokedex::build_pokemon_from_pokedex;
use omniring::{Action, ActionError, BattleObservation, Environment, Observation, PokemonState};

const SELECTION: [usize; 3] = [0, 1, 2];

fn main() -> Result<(), ActionError> {
    run_demo().map(|_| ())
}

fn run_demo() -> Result<BattleObservation, ActionError> {
    let player = roster(
        "Charizard",
        ["Flamethrower", "Air Slash", "Dragon Claw", "Protect"],
    );
    let opponent = roster(
        "Venusaur",
        ["Vine Whip", "Razor Leaf", "Sleep Powder", "Seed Bomb"],
    );

    println!(
        "Player: {} using {}; opponent: {} using {}",
        player[0].entry.name,
        player[0].moves[0].name,
        opponent[0].entry.name,
        opponent[0].moves[0].name,
    );
    println!("Each roster has six copies; both select slots {SELECTION:?}.");

    let mut environment = Environment::from_rosters(player, opponent, SELECTION)?;
    environment.reset();

    let mut outcome = environment.step(Action::SelectTeam(SELECTION), Action::Move(0))?;
    let mut total_reward = 0.0;
    let mut step = 0;

    loop {
        let Observation::Battle(observation) = outcome.observation else {
            unreachable!("team selection starts the battle");
        };

        total_reward += outcome.reward;

        println!(
            "Step {step}: player HP {:?}, opponent HP {:?}; reward {:+.3}",
            SELECTION.map(|slot| observation.player.roster()[slot].hp_curr()),
            SELECTION.map(|slot| observation.opponent.roster()[slot].hp_curr()),
            outcome.reward,
        );

        if outcome.terminated {
            let winner = if observation.player.slot_active().is_some() {
                "Player"
            } else {
                "Opponent"
            };

            println!("Result: {winner} wins! Total reward: {total_reward:+.3}");
            return Ok(observation);
        }

        assert!(step < 100, "demo did not terminate within 100 steps");

        let player_action = next_action(
            observation.player.slot_active(),
            observation.player.roster(),
        );
        let opponent_action = next_action(
            observation.opponent.slot_active(),
            observation.opponent.roster(),
        );

        println!("  Actions: player {player_action:?}, opponent {opponent_action:?}");

        outcome = environment.step(player_action, opponent_action)?;
        step += 1;
    }
}

fn roster(species: &str, moves: [&str; 4]) -> [Pokemon; 6] {
    let pokemon = build_pokemon_from_pokedex(
        species,
        50,
        StatPoints {
            hp: 16,
            attack: 10,
            defense: 10,
            special_attack: 10,
            special_defense: 10,
            speed: 10,
        },
        Nature::Hardy,
        moves,
    )
    .expect("the fixed demo Pokemon and moves must be valid");

    std::array::from_fn(|_| pokemon.clone())
}

fn next_action(active: Option<usize>, roster: &[PokemonState; 6]) -> Action {
    // Both scripted teams use move 0 and replace fainted Pokemon in selection order.
    if active.is_some() {
        Action::Move(0)
    } else {
        Action::Switch(
            SELECTION
                .into_iter()
                .find(|&slot| roster[slot].hp_curr() > 0)
                .expect("a non-terminal team must have a replacement"),
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn demo_finishes_after_all_three_selected_opponents_faint() {
        let observation = super::run_demo().unwrap();
        assert!(observation.terminated);
        assert!(observation.player.slot_active().is_some());
        assert_eq!(observation.opponent.slot_active(), None);
        assert!(
            observation.opponent.roster()[..3]
                .iter()
                .all(|pokemon| pokemon.hp_curr() == 0)
        );
        assert!(
            observation.opponent.roster()[3..]
                .iter()
                .all(|pokemon| pokemon.hp_curr() == pokemon.hp_max())
        );

        let lead = &observation.player.roster()[0];
        assert!(lead.hp_curr() > 0 && lead.hp_curr() < lead.hp_max());
    }
}
