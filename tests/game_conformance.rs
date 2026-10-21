use missile_command::{
    game::{Game, Input, MissileKind, Phase, Vec2},
    waves,
};

#[test]
fn revision_one_wave_tables_match_program_bytes() {
    let counts = [
        12, 15, 18, 12, 16, 14, 17, 10, 13, 16, 19, 12, 14, 16, 18, 14, 16, 18, 20,
    ];
    let smart = [0, 0, 0, 0, 0, 1, 1, 2, 3, 4, 4, 5, 5, 6, 6, 7, 7, 7, 7];
    for (i, &count) in counts.iter().enumerate() {
        assert_eq!(waves::config(i as u32 + 1).icbms, count);
        assert_eq!(waves::config(i as u32 + 1).smart_bombs, smart[i]);
    }
    assert_eq!(waves::config(100).icbms, 20);
    assert_eq!(waves::config(1).missile_delay, 0x4d0);
    assert_eq!(waves::config(15).missile_delay, 0);
    assert_eq!(waves::config(11).multiplier, 6);
    assert_eq!(waves::config(256).multiplier, 6);
    assert_eq!(waves::palette(1), waves::palette(21));
}

#[test]
fn identical_seed_and_input_replay_produce_identical_complete_state() {
    let mut first = Game::new(0x1980);
    first.start();
    let mut second = first.clone();
    for frame in 0..12_000 {
        let input = Input {
            aim: Vec2::new(
                ((frame * 37) % 256) as f32,
                ((frame * 13) % 170 + 30) as f32,
            ),
            fire: [frame % 31 == 0, frame % 47 == 0, frame % 61 == 0],
            start: false,
        };
        first.update(input);
        second.update(input);
        assert_eq!(first, second, "replay diverged at frame {frame}");
    }
}

#[test]
fn unattended_game_reaches_game_over_and_restart_resets_all_play_state() {
    let mut game = Game::new(1);
    game.start();
    for _ in 0..30_000 {
        game.update(Input::default());
        assert!(
            game.missiles
                .iter()
                .all(|m| m.position.x.is_finite() && m.position.y.is_finite())
        );
        if matches!(game.phase, Phase::GameOver { .. }) {
            break;
        }
    }
    assert!(
        matches!(game.phase, Phase::GameOver { .. }),
        "unattended game stuck in {:?}",
        game.phase
    );
    assert_eq!(game.alive_cities(), 0);
    game.update(Input {
        start: true,
        ..Input::default()
    });
    assert_eq!(game.phase, Phase::Playing);
    assert_eq!(game.wave, 1);
    assert_eq!(game.score, 0);
    assert_eq!(game.alive_cities(), 6);
    assert!(game.missiles.is_empty());
}

#[test]
fn input_sanitization_and_long_running_entity_limits_hold() {
    let mut game = Game::new(99);
    game.start();
    for frame in 0..15_000 {
        game.update(Input {
            aim: Vec2::new(f32::NAN, f32::INFINITY),
            fire: [frame % 3 == 0; 3],
            start: false,
        });
        assert!(game.aim.x.is_finite() && game.aim.y.is_finite());
        assert!(game.explosions.len() <= 20);
        assert!(game.fliers.len() <= 1);
        assert!(
            game.missiles
                .iter()
                .filter(|m| matches!(m.kind, MissileKind::Interceptor { .. }))
                .count()
                <= 8
        );
        assert!(
            game.missiles
                .iter()
                .filter(|m| !matches!(m.kind, MissileKind::Interceptor { .. }))
                .count()
                <= 8
        );
        assert!(game.bases.iter().all(|b| b.ammo <= 10));
    }
}

#[test]
fn vertical_interceptors_match_recorded_original_rom_positions() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../docs/evidence/mame-movement.json")).unwrap();
    let mut game = Game::new(0x1980);
    game.start();
    for _ in 0..122 {
        game.update(Input::default());
    }
    for (base, key, x) in [(0, "side", 20.0), (1, "center", 123.0)] {
        let samples = reference[key].as_array().unwrap();
        for (age, sample) in samples.iter().enumerate() {
            let mut fire = [false; 3];
            fire[base] = age == 0;
            game.update(Input {
                aim: Vec2::new(x, 100.0),
                fire,
                start: false,
            });
            let missile = game
                .missiles
                .iter()
                .find(|m| m.kind == MissileKind::Interceptor { base })
                .unwrap();
            let native_y = 230.0 - missile.position.y;
            assert_eq!(
                missile.position.x as u64,
                sample["native_x"].as_u64().unwrap()
            );
            assert_eq!(
                native_y as u64,
                sample["native_y"].as_u64().unwrap(),
                "{key} sample {age}"
            );
            assert_eq!(
                u64::from(game.bases[base].ammo),
                sample["ammo"][base].as_u64().unwrap()
            );
        }
    }
}
