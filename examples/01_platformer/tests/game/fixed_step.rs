//! The game on the fixed loop across frame rates: a tap between two steps
//! at 144 Hz, an air jump in a two-step frame at 30 Hz, the small balls
//! held at the cap across two-step frames, and the level's clock drawn
//! forward in frames that run no step.

use example_01_platformer::features::level::systems::drawn_time;
use example_01_platformer::features::player::components::Moves;
use example_01_platformer::features::player::systems::AIR_JUMP_BURST;
use example_01_platformer::features::spells::components::Spell;
use example_01_platformer::features::toys::components::Ball;
use example_01_platformer::features::toys::{self, BALL_CAP};
use example_01_platformer::shared::Burst;
use glam::Vec2;
use tungsten::core::{
    AnimationRegistry, AssetRegistry, Entity, Position, PrevPosition, Time, Transform,
};
use tungsten::core::{ParticleConfigRegistry, ParticleEmitter, Velocity};
use tungsten::kit::Children;
use tungsten::kit::PlatformerController;
use tungsten::testing::UiHarness;

use super::{aim_at, centre, choose, mock_sprite, pin_camera, run, steps_this_frame};

const FAST: f32 = 1.0 / 144.0;
const SLOW: f32 = 1.0 / 30.0;

/// Steps a frame at a time until one runs a step, so the next frame at
/// 1/144 s runs none.
fn step_until_a_step_ran(harness: &mut UiHarness) {
    loop {
        harness.step(1);
        if steps_this_frame(harness) > 0 {
            return;
        }
    }
}

#[test]
fn at_144_hz_a_grounded_tap_between_two_steps_jumps() {
    let (mut harness, player) = run();
    harness.harness_mut().set_dt(FAST);
    harness.step(150);
    let controller = |harness: &UiHarness| {
        let world = harness.world();
        world.get::<PlatformerController>(player).unwrap().clone()
    };
    assert!(controller(&harness).grounded());
    step_until_a_step_ran(&mut harness);
    harness.harness_mut().press_action("jump");
    harness.step(1);
    assert_eq!(steps_this_frame(&harness), 0);
    harness.harness_mut().release_action("jump");
    while steps_this_frame(&harness) == 0 {
        harness.step(1);
    }
    // Released before the step, so the jump cut halves it at once.
    let velocity = harness.world().get::<Velocity>(player).unwrap().0;
    assert!(controller(&harness).jumped(), "the tap did not jump");
    assert!(velocity.y < -1280.0 * 0.4, "{velocity}");
}

#[test]
fn at_30_hz_a_double_jump_in_a_frames_first_step_plays_its_effect() {
    let (mut harness, player) = run();
    harness.harness_mut().set_dt(SLOW);
    harness.step(40);
    let world = harness.world();
    assert!(
        world
            .get::<PlatformerController>(player)
            .unwrap()
            .grounded()
    );
    harness.harness_mut().press_action("jump");
    harness.step(1);
    harness.harness_mut().release_action("jump");
    harness.step(2);
    let world = harness.world();
    assert!(
        !world
            .get::<PlatformerController>(player)
            .unwrap()
            .grounded()
    );
    assert!(!world.get::<Moves>(player).unwrap().tuck);
    harness.harness_mut().press_action("jump");
    harness.step(1);
    assert_eq!(steps_this_frame(&harness), 2);
    let world = harness.world();
    let controller = world.get::<PlatformerController>(player).unwrap();
    assert_eq!(
        controller.air_jumps_left(),
        0,
        "the frame's first step jumped"
    );
    assert!(world.get::<Moves>(player).unwrap().tuck);
    let config = world
        .resource::<ParticleConfigRegistry>()
        .id_for_name(AIR_JUMP_BURST)
        .unwrap();
    assert!(
        world
            .query::<(&Burst, &ParticleEmitter)>()
            .any(|(_, emitter)| emitter.config == config),
        "no air-jump burst"
    );
}

#[test]
fn at_144_hz_the_scene_clock_is_drawn_forward_in_every_frame() {
    let (mut harness, _) = run();
    harness.harness_mut().set_dt(FAST);
    harness.step(60);
    let mut last = drawn_time(harness.world());
    let mut stepless = 0;
    for _ in 0..12 {
        harness.step(1);
        stepless += u32::from(steps_this_frame(&harness) == 0);
        let drawn = drawn_time(harness.world());
        assert!((drawn - last - FAST).abs() < 1e-4, "{last} -> {drawn}");
        last = drawn;
    }
    assert!(stepless > 0);
}

/// `prev + (cur - prev) * alpha` of a body after the frame, and how far
/// that is from its `Position`.
fn lerped(harness: &UiHarness, entity: Entity) -> (Vec2, f32) {
    let world = harness.world();
    let alpha = world.resource::<Time>().alpha();
    let cur = world.get::<Position>(entity).unwrap().0;
    let prev = world.get::<PrevPosition>(entity).unwrap().0;
    let drawn = prev + (cur - prev) * alpha;
    (drawn, drawn.distance(cur))
}

/// The positions the frame drew on atlas `texture`.
fn instances(harness: &UiHarness, texture: u32) -> Vec<Vec2> {
    harness
        .harness()
        .draw()
        .unwrap()
        .sprites
        .iter()
        .filter(|batch| batch.texture.0 == texture)
        .flat_map(|batch| batch.instances.iter().map(|i| Vec2::from_array(i.position)))
        .collect()
}

#[test]
fn at_144_hz_sprites_and_drips_sit_at_their_bodies_drawn_points() {
    let (mut harness, player) = run();
    // Every frame of the courier's and the bowling ball's clips on its own atlas.
    let world = harness.world_mut();
    let frames: Vec<(String, u32)> = {
        let registry = world.resource::<AnimationRegistry>();
        let assets = world.resource::<AssetRegistry>();
        registry
            .iter()
            .filter(|(name, _)| name.starts_with("ex10_player") || *name == "ex10_bowling_roll")
            .flat_map(|(name, clip)| {
                let texture = if name.starts_with("ex10_player") {
                    950
                } else {
                    951
                };
                clip.frames.iter().map(move |f| (f.sprite, texture))
            })
            .map(|(id, texture)| (assets.sprite_name(id).unwrap().to_owned(), texture))
            .collect()
    };
    let assets = world.resource_mut::<AssetRegistry>();
    for (name, texture) in &frames {
        if assets.get_sprite(name).is_none() {
            mock_sprite(assets, name, *texture, false);
        }
    }
    harness.harness_mut().set_dt(FAST);
    harness.step(60);
    let at = centre(&harness, player);
    let heavy = toys::spawn_ball(harness.world_mut(), at + Vec2::new(120.0, -80.0), false);
    harness.world_mut().get_mut::<Velocity>(heavy).unwrap().0 = Vec2::new(-200.0, 0.0);
    let missile = toys::fire_missile(
        harness.world_mut(),
        at + Vec2::new(0.0, -160.0),
        Vec2::new(0.0, -400.0),
    )
    .unwrap();
    // The missile's drips hang from it from its first step.
    while harness.world().get::<Children>(missile).is_none() {
        harness.step(1);
    }
    let drips: Vec<Entity> = harness
        .world()
        .get::<Children>(missile)
        .unwrap()
        .iter()
        .collect();
    harness.harness_mut().press_action("move_right");
    let mut off = [0.0_f32; 3];
    for _ in 0..12 {
        harness.step(1);
        let (drawn, away) = lerped(&harness, player);
        let players = instances(&harness, 950);
        assert_eq!(players.len(), 1);
        assert!(players[0].distance(drawn) <= 1e-3, "{players:?} {drawn}");
        off[0] = off[0].max(away);
        let (drawn, away) = lerped(&harness, heavy);
        assert!(
            instances(&harness, 951)
                .iter()
                .any(|p| p.distance(drawn) <= 1e-3),
            "no ball drawn at {drawn}"
        );
        off[1] = off[1].max(away);
        let (drawn, away) = lerped(&harness, missile);
        let world = harness.world();
        for (drip, offset) in drips.iter().zip([Vec2::new(8.0, 8.0)]) {
            let emitter = world.get::<Transform>(*drip).unwrap().position;
            assert!(
                emitter.distance(drawn + offset) <= 1e-3,
                "{emitter} {drawn}"
            );
        }
        off[2] = off[2].max(away);
    }
    // Each body was drawn away from its `Position` in some frame.
    assert!(off.iter().all(|&away| away > 1.0), "{off:?}");
}

#[test]
fn at_30_hz_both_spawners_stop_at_the_cap_across_two_steps() {
    // The small balls' spell on slot 9 is the one spawner left.
    let (mut harness, _) = run();
    pin_camera(&mut harness, Vec2::new(600.0, 200.0));
    aim_at(&mut harness, Vec2::new(1200.0, 500.0));
    choose(&mut harness, Spell::SmallBalls);
    let live = toys::ball_count(harness.world());
    for _ in 0..BALL_CAP - live - 1 {
        harness.world_mut().spawn_with((Ball,));
    }
    harness.harness_mut().set_dt(SLOW);
    harness.harness_mut().press_action("cast");
    for _ in 0..3 {
        harness.step(1);
        assert_eq!(steps_this_frame(&harness), 2);
        assert_eq!(harness.world().query::<&Ball>().count(), BALL_CAP);
    }
}
