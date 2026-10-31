use omniring::{
    Action, BATTLE_ENCODING_LEN, BattleSide, Environment, Observation, PokemonState,
    TEAM_PREVIEW_ENCODING_LEN, TeamPreviewObservation,
};

fn preview() -> TeamPreviewObservation {
    TeamPreviewObservation {
        player: std::array::from_fn(|slot| {
            PokemonState::new(
                100 + slot as u32,
                200 + slot as u32,
                [true, false, true, false],
            )
            .unwrap()
        }),
        opponent: std::array::from_fn(|slot| {
            PokemonState::new(
                300 + slot as u32,
                400 + slot as u32,
                [false, true, false, true],
            )
            .unwrap()
        }),
    }
}

#[test]
fn encoding_contract_covers_both_perspectives_through_fainting_and_reset() {
    let mut initial = preview();
    initial.player[0] = PokemonState::new(u32::MAX - 1, u32::MAX, [false; 4]).unwrap();
    initial.opponent[0] = PokemonState::new(0, 1, [false; 4]).unwrap();

    let mut env = Environment::new_with_seed(initial, [5, 4, 3], 46, |state, _, _, _| {
        state.player.damage_active(u32::MAX).unwrap();
        state.opponent.damage_active(u32::MAX).unwrap();
        Ok(Vec::new())
    })
    .unwrap();
    let sides = [BattleSide::Player, BattleSide::Opponent];
    let previews = sides.map(|side| {
        let observation = env.observation(side).unwrap();
        let Observation::TeamPreview(view) = &observation else {
            panic!("expected preview")
        };
        let encoded: [f64; TEAM_PREVIEW_ENCODING_LEN] = view.encode();

        assert_eq!(encoded.len(), 72);
        assert_eq!(observation.encode(), encoded);
        assert_eq!(observation.clone().encode(), encoded);

        let bases = match side {
            BattleSide::Player => [100.0, 300.0],
            BattleSide::Opponent => [300.0, 100.0],
        };

        for (team, base) in bases.into_iter().enumerate() {
            for slot in 0..6 {
                let expected = match (base, slot) {
                    (100.0, 0) => [4_294_967_294.0, 4_294_967_295.0, 0.0, 0.0, 0.0, 0.0],
                    (300.0, 0) => [0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                    (100.0, _) => [
                        base + slot as f64,
                        base + 100.0 + slot as f64,
                        1.0,
                        0.0,
                        1.0,
                        0.0,
                    ],
                    _ => [
                        base + slot as f64,
                        base + 100.0 + slot as f64,
                        0.0,
                        1.0,
                        0.0,
                        1.0,
                    ],
                };

                let offset = team * 36 + slot * 6;
                assert_eq!(encoded[offset..offset + 6], expected);
            }
        }
        encoded
    });

    let selected = env
        .step(Action::SelectTeam([1, 2, 3]), Action::SelectTeam([5, 4, 3]))
        .unwrap();

    assert_eq!(
        selected.observation.encode(),
        env.observation(sides[0]).unwrap().encode()
    );

    let selections = [
        [0.0, 1.0, 1.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    ];
    let leads = [[1, 2, 3], [5, 4, 3]];

    for round in 0..3 {
        for (index, side) in sides.into_iter().enumerate() {
            let observation = env.observation(side).unwrap();
            let Observation::Battle(view) = &observation else {
                panic!("expected battle")
            };
            let encoded: [f64; BATTLE_ENCODING_LEN] = view.encode();

            assert_eq!(encoded.len(), 87);
            assert_eq!(observation.encode(), encoded);
            assert_eq!(observation.clone().encode(), encoded);
            assert_eq!(encoded[72..78], selections[index]);
            assert_eq!(
                encoded[84..],
                [
                    leads[index][round] as f64,
                    leads[1 - index][round] as f64,
                    0.0
                ]
            );

            for slot in 0..6 {
                let revealed = leads[1 - index][..=round].contains(&slot);
                assert_eq!(encoded[78 + slot], if revealed { 1.0 } else { -1.0 });
            }

            if round == 0 {
                assert_eq!(encoded[..72], previews[index]);
            }
        }

        let outcome = env.step(Action::Move(0), Action::Move(1)).unwrap();

        assert_eq!(outcome.terminated, round == 2);

        for (index, side) in sides.into_iter().enumerate() {
            let encoded = env.observation(side).unwrap().encode();

            assert_eq!(encoded.len(), 87);
            assert_eq!(encoded[84..], [-1.0, -1.0, f64::from(round == 2)]);
            assert_eq!(encoded[leads[index][round] * 6], 0.0);
            assert_eq!(encoded[36 + leads[1 - index][round] * 6], 0.0);
            assert_eq!(encoded[78 + leads[1 - index][round]], 1.0);
        }

        if round < 2 {
            env.step(
                Action::Switch(leads[0][round + 1]),
                Action::Switch(leads[1][round + 1]),
            )
            .unwrap();
        }
    }

    env.reset();

    for (index, side) in sides.into_iter().enumerate() {
        assert_eq!(env.observation(side).unwrap().encode(), previews[index]);
    }
}

#[test]
fn unrevealed_selections_do_not_change_either_trainers_encoding() {
    for side in [BattleSide::Player, BattleSide::Opponent] {
        let mut expected = None;

        for first in 0..5 {
            for second in first + 1..5 {
                let mut env =
                    Environment::new_with_seed(preview(), [5, first, second], 46, |_, _, _, _| {
                        Ok(Vec::new())
                    })
                    .unwrap();

                for (own, opposing) in [
                    (
                        Action::SelectTeam([1, 2, 3]),
                        Action::SelectTeam([5, first, second]),
                    ),
                    (Action::Switch(2), Action::Switch(first)),
                    (Action::Switch(3), Action::Switch(second)),
                ] {
                    let (player, opponent) = match side {
                        BattleSide::Player => (own, opposing),
                        BattleSide::Opponent => (opposing, own),
                    };

                    env.step(player, opponent).unwrap();

                    let encoded = env.observation(side).unwrap().encode();

                    if matches!(own, Action::SelectTeam(_)) {
                        assert_eq!(encoded[78..84], [-1.0, -1.0, -1.0, -1.0, -1.0, 1.0]);

                        if let Some(expected) = &expected {
                            assert_eq!(&encoded, expected);
                        } else {
                            expected = Some(encoded);
                        }
                    }
                }

                // Even after three reveals, unseen slots retain the unknown sentinel.
                let encoded = env.observation(side).unwrap().encode();

                for slot in 0..6 {
                    assert_eq!(
                        encoded[78 + slot],
                        if [5, first, second].contains(&slot) {
                            1.0
                        } else {
                            -1.0
                        }
                    );
                }
            }
        }
    }
}
