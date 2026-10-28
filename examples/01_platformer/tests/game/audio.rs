//! The listener (e01sfx): the one-shots and the placed loops by their
//! distance from the player.

use example_01_platformer::features::audio::components::{Music, Sound};
use example_01_platformer::features::audio::{self, FIRE_CRACKLE, LISTEN_FULL, LISTEN_SILENT};
use example_01_platformer::features::dummy;
use example_01_platformer::features::elements::components::BallBurn;
use example_01_platformer::features::props::components::PropKind;
use example_01_platformer::features::props::{self, CRATE_HALF};
use example_01_platformer::save::{self, SaveGame};
use example_01_platformer::shared::body_centre;
use example_01_platformer::shell::{self as game_shell, RunStart};
use glam::Vec2;
use tungsten::core::{AudioCommand, Entity, SoundRegistry, Time};
use tungsten::kit::CueListener;
use tungsten::testing::UiHarness;

use super::combat::apply;
use super::{ball, centre, ground, harness, player_entity, run, settle, stand, teleport};

const TOP: f32 = 1000.0;
const TOSS: &str = "ex10_toss_sfx";

/// The volumes `id` played at this frame, as one-shots.
fn plays(harness: &UiHarness, id: &str) -> Vec<f32> {
    let sound = harness.world().resource::<SoundRegistry>().get_by_id(id);
    harness
        .harness()
        .audio()
        .iter()
        .filter_map(|command| match command {
            AudioCommand::Play {
                handle,
                volume,
                looping: false,
                ..
            } if Some(*handle) == sound => Some(*volume),
            _ => None,
        })
        .collect()
}

fn volume(harness: &UiHarness, id: &str) -> f32 {
    let registry = harness.world().resource::<SoundRegistry>();
    registry.get_volume(registry.get_by_id(id).expect(id))
}

fn listener(harness: &UiHarness) -> CueListener {
    *harness.world().resource::<CueListener>()
}

/// The crackle's volume last set, after `frames` frames.
fn crackle(harness: &mut UiHarness, frames: u32) -> f32 {
    harness.step(frames);
    harness.world().resource::<Music>().crackle_level
}

#[test]
fn a_cue_sound_plays_by_its_distance_from_the_player() {
    let (mut harness, player) = run();
    let level = volume(&harness, TOSS);
    let mut heard = Vec::new();
    for x in [0.0, 500.0, 2000.0, 3500.0] {
        let at = centre(&harness, player) + Vec2::new(x, 0.0);
        audio::play(harness.world_mut(), Sound::Tossed, at);
        harness.step(1);
        heard.push((plays(&harness, TOSS), level * listener(&harness).gain(at)));
    }
    assert_eq!(heard[0].0, [level], "the player's own, at full");
    assert_eq!(heard[1].0, [level], "within {LISTEN_FULL} px");
    let (quieter, expected) = (&heard[2].0, heard[2].1);
    assert!(
        quieter.len() == 1 && (quieter[0] - expected).abs() < 1e-4,
        "{heard:?}"
    );
    assert!(
        expected > 0.2 * level && expected < 0.35 * level,
        "{expected}"
    );
    assert_eq!(heard[3].0, [] as [f32; 0], "past {LISTEN_SILENT} px");
}

#[test]
fn the_crackle_follows_the_player_away_from_a_burning_crate() {
    let (mut harness, player) = run();
    ground(&mut harness, TOP);
    stand(&mut harness, player, 500.0, TOP);
    let at = Vec2::new(200.0, TOP - CRATE_HALF.y - 0.5);
    let crate_ = props::spawn(harness.world_mut(), PropKind::Crate, at);
    harness.step(1);
    assert!(props::light(harness.world_mut(), crate_, None));
    let level = volume(&harness, FIRE_CRACKLE);
    assert!(
        (crackle(&mut harness, 10) - level).abs() < 1e-4,
        "300 px off"
    );
    stand(&mut harness, player, 2200.0, TOP);
    let far = crackle(&mut harness, 10);
    let fire = body_centre(harness.world(), crate_).expect("a body");
    let distance = listener(&harness).position().distance(fire);
    assert!((1900.0..2100.0).contains(&distance), "{distance}");
    let expected = level * listener(&harness).gain(fire);
    assert!((far - expected).abs() < 0.011, "{far} for {expected}");
    assert!(far > 0.0 && far < 0.5 * level, "{far}");
    teleport(&mut harness, player, fire + Vec2::new(3300.0, 0.0));
    assert_eq!(crackle(&mut harness, 3), 0.0, "past {LISTEN_SILENT} px");
}

#[test]
fn a_burning_dummy_ball_or_player_each_raise_the_crackle() {
    let lit: [fn(&mut UiHarness, Entity) -> Entity; 3] = [
        |harness, player| {
            let at = centre(harness, player) + Vec2::new(200.0, -80.0);
            let target = dummy::spawn(harness.world_mut(), at);
            apply(harness, target, "burning", None);
            target
        },
        |harness, player| {
            let at = centre(harness, player) + Vec2::new(150.0, -40.0);
            let small = ball(harness, at, true);
            harness.world_mut().insert(
                small,
                BallBurn {
                    remaining: 6.0,
                    exposed: true,
                },
            );
            small
        },
        |harness, player| {
            apply(harness, player, "burning", None);
            player
        },
    ];
    for (i, light) in lit.into_iter().enumerate() {
        let (mut harness, player) = run();
        assert_eq!(crackle(&mut harness, 2), 0.0, "case {i}: nothing burns");
        light(&mut harness, player);
        assert!(crackle(&mut harness, 3) > 0.0, "case {i}");
    }
}

#[test]
fn a_load_puts_the_listener_at_the_loaded_player() {
    let (mut first, player) = run();
    let start = centre(&first, player);
    let away = start + Vec2::new(1400.0, -1200.0);
    teleport(&mut first, player, away);
    let saved = save::capture(first.world()).expect("a live run captures");
    let saved: SaveGame =
        serde_json::from_value(serde_json::to_value(&saved).unwrap()).expect("a save loads");
    let mut loaded = harness();
    loaded.world_mut().resource_mut::<Time>().pause();
    game_shell::start_run(loaded.world_mut(), RunStart::from(saved), None);
    settle(&mut loaded);
    let player = player_entity(&loaded);
    let here = listener(&loaded).position();
    assert!(here.distance(centre(&loaded, player)) < 1.0, "{here}");
    assert!(
        here.distance(away) < 1.0 && here.distance(start) > LISTEN_FULL,
        "{here}"
    );
}
