use omniring::info::{BattleError, Nature, Pokemon, StatPoints};
use omniring::pokedex::build_pokemon_from_pokedex;
use omniring::{Action, ActionError, BattleObservation, Environment, Observation, StateError};

#[test]
fn real_battles_run_to_win_or_loss_and_reset() {
    for player_wins in [true, false] {
        let strong = roster("Charizard");
        let weak = roster("Venusaur").map(|mut pokemon| {
            pokemon.stats.hp = 1;
            pokemon.current_hp = 1;
            pokemon
        });
        let (player, opponent) = if player_wins {
            (strong, weak)
        } else {
            (weak, strong)
        };

        let mut environment = Environment::from_rosters(player, opponent, [5, 2, 0]).unwrap();
        let preview = environment.reset();
        let selected = environment
            .step(Action::SelectTeam([4, 1, 3]), Action::Move(0))
            .unwrap();
        assert_eq!(selected.reward, 0.0);

        let initial = battle_observation(selected.observation);
        assert_eq!(initial.player.slot_active(), Some(4));
        assert_eq!(initial.opponent.slot_active(), Some(5));
        assert_eq!(
            initial.opponent.selection_revealed(),
            &[false, false, false, false, false, true]
        );

        let mut reward: f32 = 0.0;

        for turn in 0..3 {
            let outcome = environment.step(Action::Move(0), Action::Move(0)).unwrap();

            reward += outcome.reward;
            assert_eq!(outcome.terminated, turn == 2);

            let observation = battle_observation(outcome.observation);
            assert_eq!(observation.terminated, outcome.terminated);

            if player_wins {
                assert_eq!(observation.opponent.slot_active(), None);
                assert_eq!(observation.player.roster(), initial.player.roster());
            } else {
                assert_eq!(observation.player.slot_active(), None);
                assert_eq!(observation.opponent.roster(), initial.opponent.roster());
            }

            if turn < 2 {
                assert_eq!(
                    environment.step(Action::Move(0), Action::Move(0)),
                    Err(ActionError::UnavailableMove)
                );

                let (action, opponent_action) = if player_wins {
                    (Action::Move(0), Action::Switch([2, 0][turn]))
                } else {
                    (Action::Switch([1, 3][turn]), Action::Move(0))
                };

                let replacement = environment.step(action, opponent_action).unwrap();
                assert_eq!(replacement.reward, 0.0);
                assert!(!replacement.terminated);
            }
        }

        assert!((reward - if player_wins { 1.4 } else { -1.4 }).abs() < 1e-6);
        assert_eq!(
            environment.step(Action::Move(0), Action::Move(0)),
            Err(ActionError::BattleTerminated)
        );
        assert_eq!(environment.reset(), preview);

        let restarted = environment
            .step(Action::SelectTeam([4, 1, 3]), Action::Move(0))
            .unwrap();
        assert_eq!(battle_observation(restarted.observation), initial);
        assert!(environment.step(Action::Move(0), Action::Move(0)).is_ok());
    }
}

#[test]
fn second_turn_knockout_uses_persisted_hp_and_stops_the_counterattack() {
    for player_faster in [true, false] {
        let (player, opponent) = if player_faster {
            (roster("Charizard"), roster("Venusaur"))
        } else {
            (roster("Venusaur"), roster("Charizard"))
        };

        let mut environment = Environment::from_rosters(player, opponent, [0, 1, 2]).unwrap();
        environment
            .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
            .unwrap();

        // Flamethrower needs two hits with these stats, regardless of the damage roll.
        let first = battle_observation(
            environment
                .step(Action::Move(0), Action::Move(0))
                .unwrap()
                .observation,
        );

        for pokemon in [&first.player.roster()[0], &first.opponent.roster()[0]] {
            assert!(pokemon.hp_curr() > 0 && pokemon.hp_curr() < pokemon.hp_max());
        }

        let second = battle_observation(
            environment
                .step(Action::Move(0), Action::Move(0))
                .unwrap()
                .observation,
        );

        if player_faster {
            assert_eq!(second.opponent.roster()[0].hp_curr(), 0);
            assert_eq!(second.opponent.slot_active(), None);
            assert_eq!(second.player.roster(), first.player.roster());
        } else {
            assert_eq!(second.player.roster()[0].hp_curr(), 0);
            assert_eq!(second.player.slot_active(), None);
            assert_eq!(second.opponent.roster(), first.opponent.roster());
        }
    }
}

#[test]
fn protect_and_switches_use_real_moves_and_preserve_benched_hp() {
    let mut environment =
        Environment::from_rosters(roster("Charizard"), roster("Venusaur"), [0, 1, 2]).unwrap();
    let selected = environment
        .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
        .unwrap();
    let initial = battle_observation(selected.observation);

    let protected = environment.step(Action::Move(3), Action::Move(0)).unwrap();
    assert_eq!(protected.reward, 0.0);
    assert_eq!(battle_observation(protected.observation), initial);

    let outcome = environment
        .step(Action::Move(0), Action::Switch(1))
        .unwrap();
    assert!(outcome.reward > 0.0);

    let switched = battle_observation(outcome.observation);
    let opponent_hp = switched.opponent.roster()[1].hp_curr();
    assert!(opponent_hp > 0 && opponent_hp < initial.opponent.roster()[1].hp_curr());
    assert_eq!(switched.player.roster(), initial.player.roster());
    assert_eq!(switched.opponent.roster()[0], initial.opponent.roster()[0]);
    assert_eq!(
        switched.opponent.selection_revealed(),
        &[true, true, false, false, false, false]
    );

    let outcome = environment
        .step(Action::Switch(1), Action::Move(0))
        .unwrap();
    assert!(outcome.reward < 0.0);

    let switched = battle_observation(outcome.observation);
    let player_hp = switched.player.roster()[1].hp_curr();
    assert!(player_hp < initial.player.roster()[1].hp_curr());
    assert_eq!(switched.player.roster()[0], initial.player.roster()[0]);
    assert_eq!(switched.opponent.roster()[1].hp_curr(), opponent_hp);

    for slot in [2, 0, 1] {
        let outcome = environment
            .step(Action::Switch(slot), Action::Switch(slot))
            .unwrap();
        assert_eq!(outcome.reward, 0.0);

        let observation = battle_observation(outcome.observation);
        assert_eq!(observation.player.slot_active(), Some(slot));
        assert_eq!(observation.opponent.slot_active(), Some(slot));
        assert_eq!(observation.player.roster()[1].hp_curr(), player_hp);
        assert_eq!(observation.opponent.roster()[1].hp_curr(), opponent_hp);
    }
}

#[test]
fn failed_core_turn_leaves_hp_unchanged_for_the_next_step() {
    let mut player = roster("Charizard");
    player[0].stats.defense = 0;

    let mut opponent = roster("Venusaur");
    opponent[0].stats.hp = u16::MAX;
    opponent[0].current_hp = u16::MAX;

    let mut environment = Environment::from_rosters(player, opponent, [0, 1, 2]).unwrap();
    let selected = environment
        .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
        .unwrap();
    assert_eq!(
        environment.step(Action::Move(4), Action::Move(0)),
        Err(ActionError::UnavailableMove)
    );
    assert_eq!(
        environment.step(Action::Move(0), Action::Move(0)),
        Err(ActionError::Battle(BattleError::ZeroDefenseStat))
    );

    let retried = environment.step(Action::Move(3), Action::Move(0)).unwrap();
    assert_eq!(retried, selected);
}

#[test]
fn reveals_an_opponent_that_faints_on_switch_in() {
    let mut opponent = roster("Venusaur");
    opponent[1].current_hp = 1;

    let replacement_hp = u32::from(opponent[2].current_hp);

    let mut environment =
        Environment::from_rosters(roster("Charizard"), opponent, [0, 1, 2]).unwrap();
    environment
        .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
        .unwrap();

    let outcome = environment
        .step(Action::Move(0), Action::Switch(1))
        .unwrap();
    assert!(!outcome.terminated);

    let observation = battle_observation(outcome.observation);
    assert_eq!(observation.opponent.roster()[1].hp_curr(), 0);
    assert_eq!(observation.opponent.slot_active(), None);
    assert_eq!(
        observation.opponent.selection_revealed(),
        &[true, true, false, false, false, false]
    );

    let replacement = environment
        .step(Action::Move(0), Action::Switch(2))
        .unwrap();
    assert_eq!(replacement.reward, 0.0);

    let observation = battle_observation(replacement.observation);
    assert_eq!(observation.opponent.roster()[2].hp_curr(), replacement_hp);
}

#[test]
fn rejects_invalid_hp_in_either_roster() {
    for invalid_player in [true, false] {
        for (hp, max_hp) in [(0, 0), (101, 100)] {
            let mut player = roster("Charizard");
            let mut opponent = roster("Venusaur");
            let invalid = if invalid_player {
                &mut player[5]
            } else {
                &mut opponent[5]
            };

            invalid.current_hp = hp;
            invalid.stats.hp = max_hp;

            assert!(matches!(
                Environment::from_rosters(player, opponent, [0, 1, 2]),
                Err(ActionError::InvalidState(StateError::InvalidHp))
            ));
        }
    }
}

fn battle_observation(observation: Observation) -> BattleObservation {
    let Observation::Battle(observation) = observation else {
        panic!("expected a battle observation")
    };

    observation
}

fn roster(species: &str) -> [Pokemon; 6] {
    let moves = match species {
        "Charizard" => ["Flamethrower", "Air Slash", "Dragon Claw", "Protect"],
        "Venusaur" => ["Vine Whip", "Razor Leaf", "Sleep Powder", "Seed Bomb"],
        _ => unreachable!(),
    };
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
    .unwrap();

    std::array::from_fn(|_| pokemon.clone())
}
