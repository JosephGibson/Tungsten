//! The toys' spawns and their upkeep: slot 6 opens a black hole at the
//! cursor, the hue's turn, the holes' pull and life, the escaped balls'
//! cull, the small balls' rainbow beside the bowling balls' own colours,
//! and the rainbow's pure primaries.

use example_01_platformer::features::level::WORLD_MAX;
use example_01_platformer::features::player::systems::escaped;
use example_01_platformer::features::spells::components::Spell;
use example_01_platformer::features::toys::components::{BallHue, BlackHole, SmallBall};
use example_01_platformer::features::toys::systems::{ball_look, bounds, pull, rainbow_rgba, tick};
use example_01_platformer::features::toys::{self, BALL_RADIUS, HOLE_RADIUS, HOLE_SECS};
use glam::Vec2;
use tungsten::core::{
    AnimationRegistry, AssetRegistry, AudioCommand, Collider, Entity, Position, RigidBodyBundle,
    SoundRegistry, Sprite, Time, Transform, Velocity,
};
use tungsten::extract_sprites_default;
use tungsten::kit::Children;
use tungsten::testing::UiHarness;

use super::{aim_at, ball, choose, mock_sprite, pin_camera, run, teleport};

/// A plain dynamic body of unit mass about `centre`.
fn body(harness: &mut UiHarness, centre: Vec2) -> Entity {
    harness.world_mut().spawn_with(RigidBodyBundle::dynamic(
        Position(centre),
        Collider::circle(BALL_RADIUS),
    ))
}

#[test]
fn rainbow_ball_hue_system_advances_each_ball_hue() {
    let (mut harness, _) = run();
    let e = ball(&mut harness, Vec2::new(800.0, 400.0), true);
    let world = harness.world_mut();
    *world.get_mut::<BallHue>(e).unwrap() = BallHue {
        hue: 0.95,
        speed: 0.2,
    };
    world.resource_mut::<Time>().advance_frame(0.5);
    tick(world);
    let hue = world.get::<BallHue>(e).unwrap();
    assert!((hue.hue - 0.05).abs() < 1e-6, "{}", hue.hue);
}

#[test]
fn black_hole_force_system_pulls_dynamic_body_toward_hole() {
    let (mut harness, _) = run();
    let centre = Vec2::new(1200.0, 600.0);
    toys::place_black_hole(harness.world_mut(), centre);
    let e = body(&mut harness, centre + Vec2::new(HOLE_RADIUS * 0.5, 0.0));
    pull(harness.world_mut());
    let v = harness.world().get::<Velocity>(e).unwrap().0;
    assert!(v.x < 0.0, "toward the hole, got {v}");
    assert_eq!(v.y, 0.0);
}

#[test]
fn black_hole_force_system_ignores_bodies_outside_radius() {
    let (mut harness, _) = run();
    let centre = Vec2::new(1200.0, 600.0);
    toys::place_black_hole(harness.world_mut(), centre);
    let e = body(&mut harness, centre + Vec2::new(HOLE_RADIUS + 10.0, 0.0));
    pull(harness.world_mut());
    assert_eq!(harness.world().get::<Velocity>(e).unwrap().0, Vec2::ZERO);
}

#[test]
fn black_hole_lifetime_system_despawns_expired_hole() {
    let (mut harness, _) = run();
    let hole = toys::place_black_hole(harness.world_mut(), Vec2::new(1200.0, 600.0));
    harness.step(1);
    let dust: Vec<Entity> = harness
        .world()
        .get::<Children>(hole)
        .expect("its dust")
        .iter()
        .collect();
    assert_eq!(dust.len(), 1);
    harness.world_mut().get_mut::<BlackHole>(hole).unwrap().left = 0.01;
    harness.step(1);
    let world = harness.world();
    assert!(!world.is_alive(hole));
    assert!(!world.is_alive(dust[0]), "its dust goes with it");
    assert_eq!(world.query::<&BlackHole>().count(), 0);
}

#[test]
fn despawn_out_of_bounds_culls_escaped_balls_and_keeps_in_bounds_balls() {
    let (mut harness, _) = run();
    let inside = ball(&mut harness, Vec2::new(1000.0, 1000.0), false);
    let partly = ball(
        &mut harness,
        Vec2::new(100.0, WORLD_MAX.y + BALL_RADIUS - 1.0),
        false,
    );
    let outside = ball(
        &mut harness,
        Vec2::new(100.0, WORLD_MAX.y + BALL_RADIUS + 1.0),
        false,
    );
    let world = harness.world_mut();
    for (e, kept) in [(inside, true), (partly, true), (outside, false)] {
        let at = world.get::<Position>(e).unwrap().0;
        assert_eq!(escaped(at, world.get::<Collider>(e)), !kept);
    }
    bounds(world);
    assert!(world.is_alive(inside) && world.is_alive(partly));
    assert!(!world.is_alive(outside));
}

#[test]
fn only_small_marbles_shift_hue_while_orbs_render_untinted() {
    let (mut harness, _) = run();
    let at = Vec2::new(800.0, 2100.0);
    harness
        .world_mut()
        .resource_mut::<tungsten::core::CameraState>()
        .position = at - Vec2::splat(200.0);
    let small: Vec<Entity> = (0..4)
        .map(|i| ball(&mut harness, at + Vec2::new(i as f32 * 20.0, 0.0), true))
        .collect();
    let bowling: Vec<Entity> = (0..3)
        .map(|i| ball(&mut harness, at + Vec2::new(i as f32 * 40.0, 60.0), false))
        .collect();
    let world = harness.world_mut();
    assert!(
        small
            .iter()
            .all(|e| world.has::<BallHue>(*e) && world.has::<SmallBall>(*e))
    );
    assert!(bowling.iter().all(|e| !world.has::<BallHue>(*e)));
    // Every frame of each clip on its own atlas.
    let frames: Vec<(String, u32)> = {
        let registry = world.resource::<AnimationRegistry>();
        let assets = world.resource::<AssetRegistry>();
        [("ex10_ball_small_spin", 2), ("ex10_bowling_roll", 1)]
            .into_iter()
            .flat_map(|(clip, handle)| {
                registry
                    .get(clip)
                    .unwrap()
                    .frames
                    .iter()
                    .map(move |f| (f.sprite, handle))
            })
            .map(|(id, handle)| (assets.sprite_name(id).unwrap().to_owned(), handle))
            .collect()
    };
    for (name, handle) in &frames {
        mock_sprite(world.resource_mut::<AssetRegistry>(), name, *handle, true);
    }
    ball_look(world);
    let batches = extract_sprites_default(world);
    let colors = |texture: u32| -> Vec<[u8; 4]> {
        batches
            .iter()
            .filter(|b| b.texture.0 == texture)
            .flat_map(|b| b.instances.iter().map(|i| i.color))
            .collect()
    };
    assert_eq!(colors(1).len(), bowling.len());
    assert!(colors(1).iter().all(|c| *c == [255; 4]));
    assert_eq!(colors(2).len(), small.len());
    assert!(colors(2).iter().all(|c| *c != [255; 4]));
    let tinted = world.get::<Sprite>(small[0]).unwrap().color;
    let hue = world.get::<BallHue>(small[0]).unwrap().hue;
    assert_eq!(tinted, rainbow_rgba(hue));
}

#[test]
fn rainbow_rgba_keeps_primary_hues_fully_saturated() {
    assert_eq!(rainbow_rgba(0.0), [255, 0, 0, 255]);
    assert_eq!(rainbow_rgba(1.0 / 3.0), [0, 255, 0, 255]);
    assert_eq!(rainbow_rgba(2.0 / 3.0), [0, 0, 255, 255]);
}

#[test]
fn spawn_black_hole_system_creates_attractor_at_cursor_on_right_click() {
    // `cast` on slot 6 for the right click: placed for its life, not dragged.
    let (mut harness, player) = run();
    pin_camera(&mut harness, Vec2::new(200.0, 200.0));
    let at = Vec2::new(520.0, 480.0);
    // Within the listener's full hearing, so its sound plays at its volume.
    teleport(&mut harness, player, at + Vec2::new(0.0, 200.0));
    aim_at(&mut harness, at);
    choose(&mut harness, Spell::BlackHole);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    let world = harness.world();
    let holes: Vec<(Entity, BlackHole)> = world
        .query::<(Entity, &BlackHole)>()
        .map(|(e, hole)| (e, *hole))
        .collect();
    assert_eq!(holes.len(), 1);
    let (hole, life) = holes[0];
    assert_eq!(life.left, HOLE_SECS);
    let placed = world.get::<Transform>(hole).unwrap().position;
    assert!((placed - at).length() < 1e-3, "at the cursor: {placed}");
    let sound = world
        .resource::<SoundRegistry>()
        .get_by_id("ex10_black_hole_sfx");
    let plays: Vec<(f32, bool)> = harness
        .harness()
        .audio()
        .iter()
        .filter_map(|command| match command {
            AudioCommand::Play {
                handle,
                volume,
                looping,
                ..
            } if Some(*handle) == sound => Some((*volume, *looping)),
            _ => None,
        })
        .collect();
    assert_eq!(plays.len(), 1, "the press plays the hole's sound once");
    assert!((plays[0].0 - 0.65).abs() < 1e-6 && !plays[0].1);
}

#[test]
fn sprays_and_small_balls_fade_out_at_the_release() {
    use example_01_platformer::features::audio::components::{Sound, Sounded};
    use example_01_platformer::features::toys::components::BallSpawner;
    use example_01_platformer::features::toys::{LOOP_FADE, SMALL_BALLS_LOOP};
    use tungsten::core::Voice;
    let (mut harness, _) = run();
    harness.harness_mut().set_dt(1.0 / 60.0);
    pin_camera(&mut harness, Vec2::new(200.0, 400.0));
    aim_at(&mut harness, Vec2::new(700.0, 500.0));
    let registry = harness.world().resource::<SoundRegistry>();
    let loops: Vec<_> = [
        "ex10_flame_spray_sfx",
        "ex10_ice_spray_sfx",
        SMALL_BALLS_LOOP,
    ]
    .map(|id| registry.get_by_id(id).expect(id))
    .to_vec();
    // Each loop's voice with its stop's fade, and the frames' sounds.
    type Voices = Vec<(Voice, Option<f32>)>;
    let frames = |h: &mut UiHarness, n: u32, voices: &mut Voices, heard: &mut Vec<Sound>| {
        for _ in 0..n {
            h.step(1);
            heard.extend(h.harness().events::<Sounded>().map(|s| s.sound));
            for command in h.harness().audio() {
                match command {
                    AudioCommand::Play {
                        handle,
                        voice,
                        looping: true,
                        ..
                    } if loops.contains(handle) => voices.push((*voice, None)),
                    AudioCommand::StopVoice { voice, fade, .. } => {
                        for (own, stop) in voices.iter_mut() {
                            if own == voice {
                                *stop = Some(*fade);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    };
    let faded = |voices: &Voices| voices.iter().all(|&(_, stop)| stop == Some(LOOP_FADE));
    let (mut voices, mut heard): (Voices, Vec<Sound>) = (Vec::new(), Vec::new());
    // Each spray's loop fades out at the release; the flamethrower's tail
    // plays, as the cold spray's does.
    for (slot, end) in [
        (Spell::Flamethrower, Sound::FireEnded),
        (Spell::IceSpray, Sound::ColdEnded),
    ] {
        choose(&mut harness, slot);
        voices.clear();
        heard.clear();
        harness.harness_mut().press_action("cast");
        frames(&mut harness, 10, &mut voices, &mut heard);
        harness.harness_mut().release_action("cast");
        frames(&mut harness, 3, &mut voices, &mut heard);
        assert_eq!(voices.len(), 1, "one loop: {voices:?}");
        assert!(faded(&voices), "{voices:?}");
        assert_eq!(heard.iter().filter(|&&s| s == end).count(), 1, "its tail");
    }
    // Small balls pour on their loop, with no `placed`, and it fades too.
    choose(&mut harness, Spell::SmallBalls);
    voices.clear();
    heard.clear();
    harness.harness_mut().press_action("cast");
    frames(&mut harness, 10, &mut voices, &mut heard);
    assert!(!heard.contains(&Sound::Placed), "no placed: {heard:?}");
    harness.harness_mut().release_action("cast");
    frames(&mut harness, 3, &mut voices, &mut heard);
    assert_eq!(voices.len(), 1, "one loop: {voices:?}");
    assert!(faded(&voices), "{voices:?}");
    // Pouring under a pause stops the loop, and a quit to the title leaves
    // no loop playing.
    voices.clear();
    harness.harness_mut().press_action("cast");
    frames(&mut harness, 5, &mut voices, &mut heard);
    harness.harness_mut().press_action("state_pause");
    frames(&mut harness, 2, &mut voices, &mut heard);
    harness.harness_mut().release_action("state_pause");
    frames(&mut harness, 10, &mut voices, &mut heard);
    assert_eq!(voices.len(), 1, "{voices:?}");
    assert!(faded(&voices), "stopped under the pause: {voices:?}");
    super::press(&mut harness, "Quit to Title");
    super::press(&mut harness, "Quit");
    frames(&mut harness, 5, &mut voices, &mut heard);
    harness.harness_mut().release_action("cast");
    assert!(harness.world().resource::<BallSpawner>().voice.is_none());
    assert!(faded(&voices), "none playing: {voices:?}");
}
