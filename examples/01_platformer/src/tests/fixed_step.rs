//! The game on the fixed loop (M41), on its own schedule: drawn points at
//! 1/144 s, a tap between two steps, a double jump in a two-step frame and
//! the ball cap across two steps.

use super::*;
use crate::fireball::{FireballMissile, spawn_fireball};
use crate::state::{
    BALL_CAP, BALL_START_SPRITE_ID, BALL_VISUAL_DIAMETER, PLAYER_JUMP_IMPULSE,
    PLAYER_START_SPRITE_ID, PlayerPresentation,
};
use tungsten::core::{AssetRegistry, ParticleConfig, ParticleConfigRegistry, ParticleEmitter};
use tungsten::physics::PrevPosition;

const FAST: f32 = 1.0 / 144.0;
const SLOW: f32 = 1.0 / 30.0;
const PLAYER_TEXTURE: u32 = 950;
const BALL_TEXTURE: u32 = 951;

/// The game as `main` builds it, on the harness, with the authored level's
/// map, which the harness loads no manifest for.
fn game() -> Harness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    crate::setup::configure_app(&mut app);
    app.world_mut()
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("ex10_level".into(), level_map());
    Harness::new(app)
}

fn player(harness: &Harness) -> Entity {
    harness
        .world()
        .query::<(Entity, &Player)>()
        .next()
        .map(|(e, _)| e)
        .expect("the player is seeded")
}

/// `prev + (cur - prev) * alpha` from the world after the frame, and how
/// far that is from `Position`.
fn lerped(world: &World, entity: Entity) -> (Vec2, f32) {
    let alpha = world.get_resource::<Time>().unwrap().alpha();
    let cur = world.get::<Position>(entity).unwrap().0;
    let prev = world.get::<PrevPosition>(entity).unwrap().0;
    let drawn = prev + (cur - prev) * alpha;
    (drawn, drawn.distance(cur))
}

fn instances(harness: &Harness, texture: u32) -> Vec<Vec2> {
    harness
        .draw()
        .unwrap()
        .sprites
        .iter()
        .filter(|batch| batch.texture.0 == texture)
        .flat_map(|batch| batch.instances.iter().map(|i| Vec2::from_array(i.position)))
        .collect()
}

fn register_particles(harness: &mut Harness, names: &[&str]) {
    let world = harness.world_mut();
    let registry = world.get_resource_mut::<ParticleConfigRegistry>().unwrap();
    for name in names {
        let path = asset_path(&format!("particles/{name}.json"));
        let config = ParticleConfig::load(&path).unwrap();
        registry.register(format!("ex10_{name}"), path, config);
    }
}

#[test]
fn at_144_hz_sprites_and_drips_sit_at_their_bodies_drawn_points() {
    let mut harness = game();
    register_particles(&mut harness, &["fireball_drips"]);
    {
        let assets = harness
            .world_mut()
            .get_resource_mut::<AssetRegistry>()
            .unwrap();
        mock_sprite(assets, PLAYER_START_SPRITE_ID, PLAYER_TEXTURE, false);
        mock_sprite(assets, BALL_START_SPRITE_ID, BALL_TEXTURE, false);
    }
    harness.set_dt(FAST);
    harness.step(60);
    let player = player(&harness);
    let at = harness.world().get::<Position>(player).unwrap().0;
    let world = harness.world_mut();
    let ball = world.spawn_with(
        RigidBodyBundle::dynamic(
            Position(at + Vec2::new(120.0, -80.0)),
            Collider::circle(8.0),
        )
        .with_velocity(Velocity(Vec2::new(-200.0, 0.0)))
        .with((Ball,)),
    );
    let missile = spawn_fireball(world, at + Vec2::new(0.0, -120.0), Vec2::new(0.0, -400.0));
    let drips = world
        .get::<FireballMissile>(missile)
        .unwrap()
        .drips
        .unwrap();
    harness.press_action("move_right");

    let mut off_position = [0.0_f32; 3];
    for _ in 0..12 {
        harness.step(1);
        let world = harness.world();
        let (drawn, off) = lerped(world, player);
        let scale = world.get::<Transform>(player).unwrap().scale;
        let expected = Vec2::new(
            drawn.x - 64.0 * scale.x * 0.5,
            drawn.y + PLAYER_HALF.y - 61.0 * scale.y,
        );
        let players = instances(&harness, PLAYER_TEXTURE);
        assert_eq!(players.len(), 1);
        assert!(
            players[0].distance(expected) <= 1e-3,
            "{players:?} {expected}"
        );
        off_position[0] = off_position[0].max(off);

        let (drawn, off) = lerped(world, ball);
        let expected = drawn - BALL_VISUAL_DIAMETER * 0.5;
        assert!(
            instances(&harness, BALL_TEXTURE)
                .iter()
                .any(|p| p.distance(expected) <= 1e-3),
            "no ball instance at {expected}"
        );
        off_position[1] = off_position[1].max(off);

        let (drawn, off) = lerped(world, missile);
        let emitter = world.get::<Transform>(drips).unwrap().position;
        assert!(emitter.distance(drawn) <= 1e-3, "{emitter} {drawn}");
        off_position[2] = off_position[2].max(off);
    }
    // Each body was drawn away from its `Position` in some frame.
    assert!(
        off_position.iter().all(|&off| off > 1.0),
        "{off_position:?}"
    );
}

/// Steps one frame at a time until one runs a step, so the next frame
/// at 1/144 s runs none.
fn step_until_a_step_ran(harness: &mut Harness) {
    loop {
        harness.step(1);
        if harness
            .world()
            .get_resource::<Time>()
            .unwrap()
            .fixed_steps_this_frame()
            > 0
        {
            return;
        }
    }
}

fn steps_this_frame(harness: &Harness) -> u32 {
    harness
        .world()
        .get_resource::<Time>()
        .unwrap()
        .fixed_steps_this_frame()
}

#[test]
fn at_144_hz_a_grounded_tap_between_two_steps_jumps() {
    let mut harness = game();
    harness.set_dt(FAST);
    harness.step(150);
    let player = player(&harness);
    assert!(harness.world().get::<Player>(player).unwrap().grounded);
    step_until_a_step_ran(&mut harness);
    harness.press_action("jump");
    harness.step(1);
    assert_eq!(steps_this_frame(&harness), 0);
    harness.release_action("jump");
    while steps_this_frame(&harness) == 0 {
        harness.step(1);
    }
    let velocity = harness.world().get::<Velocity>(player).unwrap().0;
    assert!(
        velocity.y < -PLAYER_JUMP_IMPULSE * 0.5,
        "the tap did not jump: {velocity}"
    );
}

#[test]
fn at_30_hz_a_double_jump_in_a_frames_first_step_plays_its_effect() {
    let mut harness = game();
    register_particles(&mut harness, &["jump_puff", "double_jump"]);
    harness.set_dt(SLOW);
    harness.step(40);
    let player = player(&harness);
    assert!(harness.world().get::<Player>(player).unwrap().grounded);
    harness.press_action("jump");
    harness.step(1);
    harness.release_action("jump");
    harness.step(2);
    let world = harness.world();
    assert!(!world.get::<Player>(player).unwrap().grounded);
    assert!(!world.get::<PlayerPresentation>(player).unwrap().aerial_tuck);
    harness.press_action("jump");
    harness.step(1);
    assert_eq!(steps_this_frame(&harness), 2);
    let world = harness.world();
    assert!(world.get::<Player>(player).unwrap().air_jump_used);
    assert!(world.get::<PlayerPresentation>(player).unwrap().aerial_tuck);
    let double_jump = world
        .get_resource::<ParticleConfigRegistry>()
        .unwrap()
        .id_for_name("ex10_double_jump")
        .unwrap();
    assert!(
        world
            .query::<(Entity, &crate::state::TransientEmitter)>()
            .any(|(e, _)| world
                .get::<ParticleEmitter>(e)
                .is_some_and(|emitter| emitter.config == double_jump)),
        "no double-jump effect"
    );
}

#[test]
fn at_30_hz_both_spawners_stop_at_the_cap_across_two_steps() {
    let mut harness = game();
    let world = harness.world_mut();
    world
        .get_resource_mut::<ActionMap>()
        .unwrap()
        .replace_bindings(
            "spawn_ball",
            vec![Binding::Mouse {
                button: MouseButton::Left,
            }],
        );
    let live = world.query::<(Entity, &Ball)>().count();
    for _ in 0..BALL_CAP - live - 1 {
        let ball = world.spawn();
        world.insert(ball, Ball);
    }
    harness.set_dt(SLOW);
    harness.set_cursor(240.0, 144.0);
    harness.press_action("spawn_ball");
    harness.press_action("spawn_small_ball");
    for _ in 0..3 {
        harness.step(1);
        assert_eq!(steps_this_frame(&harness), 2);
        assert_eq!(harness.world().query::<(Entity, &Ball)>().count(), BALL_CAP);
    }
}

#[test]
fn at_144_hz_the_scene_clock_is_drawn_forward_in_every_frame() {
    let mut harness = game();
    // The startup hook the harness skips inserts it with the obstacles.
    harness
        .world_mut()
        .insert_resource(crate::gameplay::SceneTime::default());
    harness.set_dt(FAST);
    harness.step(60);
    let mut last = crate::extract::drawn_scene_time(harness.world());
    let mut stepless = 0;
    for _ in 0..12 {
        harness.step(1);
        stepless += u32::from(steps_this_frame(&harness) == 0);
        let drawn = crate::extract::drawn_scene_time(harness.world());
        assert!((drawn - last - FAST).abs() < 1e-4, "{last} -> {drawn}");
        last = drawn;
    }
    assert!(stepless > 0);
}
