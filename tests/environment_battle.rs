use omniring::info::{BattleError, Nature, Pokemon, StatPoints};
use omniring::pokedex::build_pokemon_from_pokedex;
use omniring::{
    Action, ActionError, Battle, BattleObservation, BattleReplay, BattleState, Environment,
    Observation, PokemonState, ReplayError, StateError, TeamState,
};

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
        assert!(matches!(preview, Observation::TeamPreview(_)));

        let selected = environment
            .step(Action::SelectTeam([4, 1, 3]), Action::Move(0))
            .unwrap();
        assert_eq!(selected.reward, 0.0);
        assert!(!selected.terminated);

        let initial = battle_observation(selected.observation);
        assert!(!initial.terminated);
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

            let (losing_roster, selection) = if player_wins {
                (observation.opponent.roster(), [5, 2, 0])
            } else {
                (observation.player.roster(), [4, 1, 3])
            };

            for (slot, pokemon) in losing_roster.iter().enumerate() {
                let expected_hp = if selection[..=turn].contains(&slot) {
                    0
                } else {
                    1
                };
                assert_eq!(pokemon.hp_curr(), expected_hp, "turn {turn}, slot {slot}");
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

                let replaced = battle_observation(replacement.observation);
                assert!(!replaced.terminated);
                assert_eq!(replaced.player.roster(), observation.player.roster());
                assert_eq!(replaced.opponent.roster(), observation.opponent.roster());
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
        assert_eq!(restarted.reward, 0.0);
        assert!(!restarted.terminated);
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
    assert_eq!(retried.observation, selected.observation);
    assert_eq!(retried.reward, selected.reward);
    assert_eq!(retried.terminated, selected.terminated);
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

#[test]
fn only_equipped_move_slots_are_available_in_preview_and_battle() {
    let mut player = roster("Charizard");
    let mut opponent = roster("Charizard");
    for (team, counts) in [
        (&mut player, [1, 2, 3, 4, 1, 2]),
        (&mut opponent, [4, 3, 2, 1, 4, 3]),
    ] {
        for (pokemon, count) in team.iter_mut().zip(counts) {
            pokemon.moves.swap(0, 3); // Protect keeps the successful turn deterministic.
            pokemon.moves.truncate(count);
        }
    }

    let mut environment = Environment::from_rosters(player, opponent, [0, 4, 5]).unwrap();
    let preview = environment.reset();
    let Observation::TeamPreview(ref teams) = preview else {
        panic!("expected team preview");
    };
    let masks = [
        [true, false, false, false],
        [true, true, false, false],
        [true, true, true, false],
        [true, true, true, true],
    ];
    assert_eq!(
        teams.player.each_ref().map(|p| p.move_availability),
        [masks[0], masks[1], masks[2], masks[3], masks[0], masks[1]]
    );
    assert_eq!(
        teams.opponent.each_ref().map(|p| p.move_availability),
        [masks[3], masks[2], masks[1], masks[0], masks[3], masks[2]]
    );

    for (lead, mask) in masks.into_iter().enumerate() {
        assert_eq!(environment.reset(), preview);
        let selected = environment
            .step(Action::SelectTeam([lead, 4, 5]), Action::Move(0))
            .unwrap();
        let initial = battle_observation(selected.observation.clone());
        assert_eq!(initial.player.roster()[lead].move_availability, mask);

        for slot in (lead + 1)..=4 {
            assert_eq!(
                environment.step(Action::Move(slot), Action::Move(0)),
                Err(ActionError::UnavailableMove)
            );
        }
        let protected = environment.step(Action::Move(0), Action::Move(0)).unwrap();
        assert_eq!(protected.observation, selected.observation);
        assert_eq!(protected.reward, selected.reward);
        assert_eq!(protected.terminated, selected.terminated);
        // Switch the opponent to its three-move reserve before testing its empty slot.
        let switched = environment
            .step(Action::Move(0), Action::Switch(5))
            .unwrap();
        assert_eq!(
            environment.step(Action::Move(0), Action::Move(3)),
            Err(ActionError::UnavailableMove)
        );
        let protected = environment.step(Action::Move(0), Action::Move(0)).unwrap();
        assert_eq!(protected.observation, switched.observation);
        assert_eq!(protected.reward, switched.reward);
        assert_eq!(protected.terminated, switched.terminated);
    }
}

#[test]
fn rejects_invalid_move_counts_in_either_roster() {
    for invalid_player in [true, false] {
        for count in [0, 5] {
            let mut player = roster("Charizard");
            let mut opponent = roster("Venusaur");
            let invalid = if invalid_player {
                &mut player[5]
            } else {
                &mut opponent[5]
            };
            invalid.moves.resize(count, invalid.moves[0].clone());

            assert!(matches!(
                Environment::from_rosters(player, opponent, [0, 1, 2]),
                Err(ActionError::Battle(BattleError::InvalidMoveCount { count: actual })) if actual == count
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

#[test]
fn recorded_battle_replays_seeded_steps_and_rejects_invalid_sequences() {
    let player_selection = [4, 1, 3];
    let opponent_selection = [5, 2, 0];
    let initial_player = roster("Charizard").map(|mut pokemon| {
        pokemon.current_hp -= 7;
        pokemon
    });
    let initial_opponent = initial_player.clone().map(|mut pokemon| {
        pokemon.current_hp -= 5;
        pokemon
    });

    for seed in [46, 47] {
        let mut replay = BattleReplay {
            player: initial_player.clone(),
            opponent: initial_opponent.clone(),
            opponent_selection,
            seed,
            actions: Vec::new(),
        };
        let mut live = Environment::from_rosters_with_seed(
            replay.player.clone(),
            replay.opponent.clone(),
            opponent_selection,
            seed,
        )
        .unwrap();
        let mut expected = Vec::new();

        // Include team selection and voluntary switches before recording a complete battle.
        for actions in [
            (Action::SelectTeam(player_selection), Action::Move(0)),
            (Action::Switch(1), Action::Move(2)),
            (Action::Move(2), Action::Switch(2)),
        ] {
            expected.push(live.step(actions.0, actions.1).unwrap());
            replay.actions.push(actions);
        }

        while !expected.last().unwrap().terminated {
            assert!(expected.len() < 100, "seeded episode failed to terminate");

            let Observation::Battle(observation) = &expected.last().unwrap().observation else {
                panic!("expected a battle after team selection");
            };
            let next_action =
                |active: Option<usize>, team: &[PokemonState; 6], selection: [usize; 3]| {
                    if active.is_some() {
                        Action::Move(2)
                    } else {
                        Action::Switch(
                            selection
                                .into_iter()
                                .find(|&slot| team[slot].hp_curr() > 0)
                                .unwrap(),
                        )
                    }
                };
            let actions = (
                next_action(
                    observation.player.slot_active(),
                    observation.player.roster(),
                    player_selection,
                ),
                next_action(
                    observation.opponent.slot_active(),
                    observation.opponent.roster(),
                    opponent_selection,
                ),
            );

            expected.push(live.step(actions.0, actions.1).unwrap());
            replay.actions.push(actions);
        }

        let terminal = battle_observation(expected.last().unwrap().observation.clone());

        assert!(terminal.terminated);
        assert_ne!(
            terminal.player.slot_active().is_none(),
            terminal.opponent.slot_active().is_none()
        );
        assert!(expected[..expected.len() - 1].iter().any(|step| matches!(
            &step.observation,
            Observation::Battle(state) if state.player.slot_active().is_none() || state.opponent.slot_active().is_none()
        )));
        // Compare every observation, reward, and termination flag, including the terminal state.
        assert_eq!(replay.run().unwrap(), expected);
        assert_eq!(replay.run().unwrap(), expected);
        assert_eq!(replay.player, initial_player);
        assert_eq!(replay.opponent, initial_opponent);

        let complete_actions = replay.actions.clone();
        replay.actions.push((Action::Move(2), Action::Move(2)));

        assert_eq!(
            replay.run(),
            Err(ReplayError::InvalidAction {
                step: complete_actions.len(),
                error: ActionError::BattleTerminated,
            })
        );

        for length in [0, 1, complete_actions.len() - 1] {
            replay.actions = complete_actions[..length].to_vec();
            assert_eq!(replay.run(), Err(ReplayError::Incomplete));
        }

        for (step, actions, error) in [
            (
                0,
                (Action::Move(0), Action::Move(0)),
                ActionError::WrongPhase,
            ),
            (
                0,
                (Action::SelectTeam([0, 0, 1]), Action::Move(0)),
                ActionError::InvalidTeamSelection,
            ),
            (
                1,
                (Action::Move(4), Action::Move(2)),
                ActionError::UnavailableMove,
            ),
            (
                1,
                (Action::Move(2), Action::Move(4)),
                ActionError::UnavailableMove,
            ),
            (
                1,
                (Action::Switch(0), Action::Move(2)),
                ActionError::InvalidSwitch,
            ),
        ] {
            replay.actions = complete_actions.clone();
            replay.actions[step] = actions;

            assert_eq!(
                replay.run(),
                Err(ReplayError::InvalidAction { step, error })
            );
        }

        replay.actions = complete_actions;
        replay.opponent_selection = [0, 0, 1];

        assert_eq!(
            replay.run(),
            Err(ReplayError::InvalidSetup(ActionError::InvalidTeamSelection))
        );

        replay.opponent_selection = opponent_selection;

        for player_side in [true, false] {
            let mut invalid = replay.clone();
            let roster = if player_side {
                &mut invalid.player
            } else {
                &mut invalid.opponent
            };

            roster[0].moves.clear();

            assert_eq!(
                invalid.run(),
                Err(ReplayError::InvalidSetup(ActionError::Battle(
                    BattleError::InvalidMoveCount { count: 0 }
                )))
            );
        }
    }
}

fn runtime_team(hp: u32, mask: [bool; 4]) -> TeamState {
    TeamState::new(
        std::array::from_fn(|_| PokemonState::new(hp, hp, mask).unwrap()),
        [true, true, true, false, false, false],
        Some(0),
    )
    .unwrap()
}

fn runtime_state() -> BattleState {
    BattleState {
        player: runtime_team(100_000, [true; 4]),
        opponent: runtime_team(100_000, [true; 4]),
        terminated: false,
    }
}

#[test]
fn cloned_battles_replay_the_same_random_turns() {
    let player = roster("Charizard");
    let opponent = roster("Charizard");

    let mut original = Battle::new(runtime_state());
    original
        .play_turn_with_rosters(&player, &opponent, Action::Move(0), Action::Move(0))
        .unwrap();

    let mut replay = original.clone();

    for _ in 0..16 {
        let actual = original
            .play_turn_with_rosters(&player, &opponent, Action::Move(0), Action::Move(0))
            .unwrap();
        let expected = replay
            .play_turn_with_rosters(&player, &opponent, Action::Move(0), Action::Move(0))
            .unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn seeded_speed_ties_replay_without_fixing_the_winner() {
    let roster = roster("Charizard");
    let initial = BattleState {
        player: runtime_team(1, [true; 4]),
        opponent: runtime_team(1, [true; 4]),
        terminated: false,
    };
    let mut winners = [false; 2];

    for seed in 0..32 {
        let mut first = Battle::with_seed(initial.clone(), seed);
        let mut replay = Battle::with_seed(initial.clone(), seed);
        let actual = first
            .play_turn_with_rosters(&roster, &roster, Action::Move(0), Action::Move(0))
            .unwrap();
        let expected = replay
            .play_turn_with_rosters(&roster, &roster, Action::Move(0), Action::Move(0))
            .unwrap();

        assert_eq!(actual, expected, "seed {seed}");
        assert_ne!(actual.player.slot_active(), actual.opponent.slot_active());

        winners[usize::from(actual.player.slot_active().is_some())] = true;
    }

    assert_eq!(winners, [true, true]);
}

#[test]
fn seeded_environment_replays_damage_switches_and_resets() {
    let roster = roster("Charizard").map(|mut pokemon| {
        pokemon.stats.hp = u16::MAX;
        pokemon.current_hp = u16::MAX;
        pokemon
    });
    let mut environment =
        Environment::from_rosters_with_seed(roster.clone(), roster, [0, 1, 2], 46).unwrap();
    let preview = environment.reset();
    let mut episodes = Vec::new();

    // First run, ordinary reset, then explicit reseeding followed by another ordinary reset.
    for episode in 0..4 {
        if episode == 2 {
            assert_eq!(environment.reset_with_seed(46), preview);
        } else {
            assert_eq!(environment.reset(), preview);
        }

        // Repeated preview resets must not change the stream either.
        assert_eq!(environment.reset(), preview);

        let mut outcomes = vec![
            environment
                .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
                .unwrap(),
        ];

        for (player, opponent) in [
            (Action::Move(0), Action::Move(0)),
            (Action::Move(1), Action::Move(2)),
            (Action::Switch(1), Action::Move(0)),
            (Action::Move(0), Action::Switch(1)),
            (Action::Move(2), Action::Move(1)),
            (Action::Move(0), Action::Move(0)),
        ] {
            outcomes.push(environment.step(player, opponent).unwrap());
        }

        episodes.push(outcomes);
    }

    assert_eq!(episodes[0], episodes[2]);
    assert_eq!(episodes[1], episodes[3]);
    assert_ne!(episodes[0], episodes[1]);
}

#[test]
fn failed_turn_preserves_randomness_for_the_next_valid_turn() {
    let mut player = roster("Charizard");
    player[0].stats.defense = 0;

    let opponent = roster("Venusaur").map(|mut pokemon| {
        pokemon.stats.hp = u16::MAX;
        pokemon.current_hp = u16::MAX;
        pokemon
    });
    let mut actual =
        Environment::from_rosters_with_seed(player.clone(), opponent.clone(), [0, 1, 2], 46)
            .unwrap();
    let mut control = Environment::from_rosters_with_seed(player, opponent, [0, 1, 2], 46).unwrap();

    actual
        .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
        .unwrap();
    control
        .step(Action::SelectTeam([0, 1, 2]), Action::Move(0))
        .unwrap();

    assert_eq!(
        actual.step(Action::Move(4), Action::Move(0)),
        Err(ActionError::UnavailableMove)
    );
    // The first attack consumes a damage roll; the second fails on zero defense.
    assert_eq!(
        actual.step(Action::Move(0), Action::Move(0)),
        Err(ActionError::Battle(BattleError::ZeroDefenseStat))
    );

    for _ in 0..16 {
        assert_eq!(
            actual.step(Action::Move(0), Action::Move(2)).unwrap(),
            control.step(Action::Move(0), Action::Move(2)).unwrap()
        );
    }
}

#[test]
fn direct_battle_uses_canonical_hp_for_either_turn_order() {
    for player_faster in [true, false] {
        let (mut player, mut opponent) = if player_faster {
            (roster("Charizard"), roster("Venusaur"))
        } else {
            (roster("Venusaur"), roster("Charizard"))
        };

        for pokemon in player.iter_mut().chain(opponent.iter_mut()) {
            pokemon.current_hp = 0;
        }

        let original_rosters = (player.clone(), opponent.clone());
        let mut battle = Battle::new(runtime_state());

        for _ in 0..2 {
            let previous = battle.state().clone();
            let next = battle
                .play_turn_with_rosters(&player, &opponent, Action::Move(0), Action::Move(0))
                .unwrap();

            for (before, after) in [
                (&previous.player, &next.player),
                (&previous.opponent, &next.opponent),
            ] {
                assert!(after.roster()[0].hp_curr() < before.roster()[0].hp_curr());
                assert!(after.roster()[0].hp_curr() > u32::from(u16::MAX));
                assert_eq!(&after.roster()[1..], &before.roster()[1..]);
            }
        }

        assert_eq!((player, opponent), original_rosters);
    }
}

#[test]
fn direct_battle_keeps_runtime_move_availability() {
    let player = roster("Charizard");
    let opponent = roster("Venusaur");

    let mut battle = Battle::new(runtime_state());
    battle
        .play_turn(Action::Move(0), Action::Move(0), |state, _, _, _| {
            state.player = runtime_team(100_000, [true, false, true, true]);
            Ok(Vec::new())
        })
        .unwrap();

    let previous = battle.state().clone();

    assert!(!previous.legal_player_actions().contains(&Action::Move(1)));
    assert_eq!(
        battle.play_turn_with_rosters(&player, &opponent, Action::Move(1), Action::Move(0),),
        Err(ActionError::UnavailableMove)
    );
    assert_eq!(battle.state(), &previous);

    battle
        .play_turn_with_rosters(&player, &opponent, Action::Move(3), Action::Move(0))
        .unwrap();

    assert_eq!(battle.state(), &previous); // Protect changes neither HP nor mask.

    let mut incomplete_player = player;
    incomplete_player[0].moves.truncate(1);

    assert_eq!(
        battle.play_turn_with_rosters(
            &incomplete_player,
            &opponent,
            Action::Move(2),
            Action::Move(0),
        ),
        Err(ActionError::Battle(BattleError::InvalidMoveIndex {
            index: 2
        }))
    );
    assert_eq!(battle.state(), &previous);
}

#[test]
fn direct_battle_rolls_back_an_error_after_the_first_attack() {
    for player_faster in [true, false] {
        let mut faster = roster("Charizard");
        faster[0].stats.defense = 0;

        let slower = roster("Venusaur");
        let (player, opponent, retry) = if player_faster {
            (faster, slower, (Action::Move(3), Action::Move(0)))
        } else {
            (slower, faster, (Action::Move(0), Action::Move(3)))
        };
        let initial = runtime_state();
        let mut battle = Battle::with_seed(initial.clone(), 46);
        let before = battle.clone();

        assert_eq!(
            battle.play_turn_with_rosters(&player, &opponent, Action::Move(0), Action::Move(0),),
            Err(ActionError::Battle(BattleError::ZeroDefenseStat))
        );
        assert_eq!(battle.state(), &initial);
        assert_eq!(battle, before); // Failed damage rolls must not advance the random stream.

        battle
            .play_turn_with_rosters(&player, &opponent, retry.0, retry.1)
            .unwrap();

        assert_eq!(battle.state(), &initial);
    }
}
