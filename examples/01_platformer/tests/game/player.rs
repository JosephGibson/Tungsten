//! The courier on the kit's controller: a press's step, the air jump's
//! fresh press, its burst, tuck and refill, and the falls and escapes that
//! end a run, inside the invulnerability window too.

use example_01_platformer::features::level::{self, KILL_Y, MAP, TILE, WORLD_MAX, WORLD_MIN};
use example_01_platformer::features::player::components::Moves;
use example_01_platformer::features::player::systems::{AIR_JUMP_BURST, escaped};
use example_01_platformer::features::player::{BODY_OFFSET, PLAYER_HALF};
use example_01_platformer::shared::Burst;
use example_01_platformer::shell::game_over::GAME_OVER;
use glam::Vec2;
use tungsten::core::{
    AnimationState, Collider, Entity, ParticleConfigRegistry, ParticleEmitter, Position,
    TilemapRegistry, Time, Transform, Velocity,
};
use tungsten::kit::{Health, PlatformerController, damage};
use tungsten::testing::UiHarness;

use super::{centre, run, settle, teleport, top};

/// The kit's step at the harness's 60 Hz.
const DT: f32 = 1.0 / 60.0;

fn controller(harness: &UiHarness, player: Entity) -> &PlatformerController {
    harness
        .world()
        .get::<PlatformerController>(player)
        .expect("a live player has its controller")
}

fn health(harness: &UiHarness, player: Entity) -> Health {
    harness.world().get::<Health>(player).unwrap().clone()
}

fn clip(harness: &UiHarness, player: Entity) -> String {
    harness
        .world()
        .get::<AnimationState>(player)
        .unwrap()
        .animation_id
        .clone()
}

/// The bursts of the particle config `name`.
fn bursts(harness: &UiHarness, name: &str) -> Vec<Entity> {
    let world = harness.world();
    let config = world
        .resource::<ParticleConfigRegistry>()
        .id_for_name(name)
        .expect("a manifest config");
    world
        .query::<(Entity, &Burst, &ParticleEmitter)>()
        .filter(|(_, _, emitter)| emitter.config == config)
        .map(|(entity, ..)| entity)
        .collect()
}

/// A fresh press: up, then down again, read by the next frame.
fn press_again(harness: &mut UiHarness, action: &str) {
    harness.harness_mut().release_action(action);
    harness.harness_mut().press_action(action);
}

#[test]
fn a_press_moves_the_player_on_the_next_step() {
    let (mut harness, player) = run();
    // Settled on the turf, so the step after the press is a plain one.
    harness.step(30);
    let before = centre(&harness, player).x;
    harness.harness_mut().press_action("move_right");
    harness.step(1);
    let after = centre(&harness, player).x;
    assert!(
        after > before,
        "x = {after} after the press's frame, was {before}"
    );
}

#[test]
fn exhausted_air_jump_requires_ground_contact() {
    let (mut harness, player) = run();
    harness.step(10);
    let ground = centre(&harness, player);
    teleport(&mut harness, player, ground - Vec2::new(0.0, 400.0));
    // Past coyote time: a press now is the air jump.
    harness.step(10);
    harness.harness_mut().press_action("jump");
    harness.step(1);
    assert!(controller(&harness, player).air_jumped());
    assert_eq!(controller(&harness, player).air_jumps_left(), 0);
    harness.harness_mut().release_action("jump");
    harness.step(30);
    assert!(!controller(&harness, player).grounded(), "still in the air");
    let falling = harness.world().get::<Velocity>(player).unwrap().0.y;
    harness.harness_mut().press_action("jump");
    harness.step(1);
    let after = harness.world().get::<Velocity>(player).unwrap().0.y;
    assert!(
        !controller(&harness, player).jumped(),
        "a jump fired in the air"
    );
    assert!(after > falling, "it kept falling: {falling} -> {after}");
}

#[test]
fn despawn_out_of_bounds_kills_escaped_player_and_holds_the_death_position() {
    let (mut harness, player) = run();
    harness.step(5);
    // Past the box's right side at a height the kill row leaves alone.
    let out = Vec2::new(WORLD_MAX.x + PLAYER_HALF.x + 50.0, 20.0 * TILE);
    teleport(&mut harness, player, out);
    harness.step(1);
    assert!(health(&harness, player).is_dead());
    assert!(!harness.world().has::<PlatformerController>(player));
    let held = centre(&harness, player);
    harness.step(30);
    assert_eq!(
        centre(&harness, player),
        held,
        "the body stays where it died"
    );
    settle(&mut harness);
    assert_eq!(top(&harness), Some(GAME_OVER));
}

#[test]
fn despawn_out_of_bounds_keeps_player_while_collider_overlaps_bounds() {
    let (mut harness, player) = run();
    harness.step(5);
    let collider = *harness.world().get::<Collider>(player).unwrap();
    let start = Vec2::new(WORLD_MIN.x - PLAYER_HALF.x + 1.0, 30.0 * TILE);
    assert!(
        !escaped(start - BODY_OFFSET, Some(&collider)),
        "a pixel inside"
    );
    assert!(escaped(
        start - BODY_OFFSET - Vec2::X * 2.0,
        Some(&collider)
    ));
    teleport(&mut harness, player, start);
    harness.step(1);
    let health = health(&harness, player);
    assert_eq!(health.current(), health.max(), "an overlapping body lives");
}

#[test]
fn despawn_out_of_bounds_is_noop_for_in_bounds_player() {
    let (mut harness, player) = run();
    let collider = *harness.world().get::<Collider>(player).unwrap();
    let middle = (WORLD_MIN + WORLD_MAX) * 0.5;
    assert!(!escaped(middle, Some(&collider)));
    harness.step(60);
    let health = health(&harness, player);
    assert_eq!(health.current(), health.max());
    assert!(controller(&harness, player).grounded());
}

#[test]
fn real_map_dimensions_spawn_and_fall_death() {
    let (mut harness, player) = run();
    let world = harness.world();
    let map = world.resource::<TilemapRegistry>().get(MAP).unwrap();
    assert_eq!(
        (map.width, map.height, map.tile_width, map.tile_height),
        (184, 50, 64, 64)
    );
    let text = std::fs::read_to_string("tools/level.json").unwrap();
    let source: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(source["kill_row"].as_f64().unwrap() as f32 * TILE, KILL_Y);
    let feet = level::marker(world, level::SPAWN_MARKER).unwrap();
    harness.step(60);
    assert!(controller(&harness, player).grounded());
    let stood = centre(&harness, player) + Vec2::new(0.0, PLAYER_HALF.y);
    assert!((stood - feet).length() < 1.0, "{stood} on {feet}");
    // Past the kill row with a landing pending.
    teleport(&mut harness, player, Vec2::new(feet.x, KILL_Y + 1.0));
    harness
        .world_mut()
        .get_mut::<Moves>(player)
        .unwrap()
        .landing = 0.18;
    harness.step(1);
    assert!(health(&harness, player).is_dead());
    let moves = harness.world().get::<Moves>(player).unwrap();
    assert!(moves.landing == 0.0 && moves.shown.is_none(), "{moves:?}");
    // Gameplay's update sees the death at the next frame's dispatch.
    harness.step(2);
    settle(&mut harness);
    assert_eq!(top(&harness), Some(GAME_OVER));
}

#[test]
fn double_jump_needs_a_fresh_press_emits_once_and_refills_on_landing() {
    let (mut harness, player) = run();
    harness.step(10);
    assert!(controller(&harness, player).grounded());
    harness.harness_mut().press_action("jump");
    harness.step(1);
    assert!(controller(&harness, player).jumped());
    assert!(!controller(&harness, player).air_jumped());
    harness.step(10);
    assert_eq!(
        controller(&harness, player).air_jumps_left(),
        1,
        "holding jump spent the air jump"
    );
    press_again(&mut harness, "jump");
    let world = harness.world();
    let takeoff = world.get::<Position>(player).unwrap().0 + BODY_OFFSET + Vec2::Y * PLAYER_HALF.y;
    harness.step(1);
    assert!(controller(&harness, player).air_jumped());
    assert_eq!(controller(&harness, player).air_jumps_left(), 0);
    // The step's mean: the jump's speed, less half a step of gravity.
    let rise = harness.world().get::<Velocity>(player).unwrap().0.y;
    let expected = -1280.0 + level::GRAVITY * DT * 0.5;
    assert!((rise - expected).abs() < 1e-2, "{rise} vs {expected}");
    let burst = bursts(&harness, AIR_JUMP_BURST);
    assert_eq!(burst.len(), 1);
    let at = harness.world().get::<Transform>(burst[0]).unwrap().position;
    assert!((at - takeoff).length() < 1e-3, "{at} vs {takeoff}");
    // A third fresh press neither jumps nor bursts again.
    press_again(&mut harness, "jump");
    harness.step(1);
    assert!(!controller(&harness, player).jumped());
    assert_eq!(bursts(&harness, AIR_JUMP_BURST).len(), 1);
    harness.harness_mut().release_action("jump");
    harness.step(150);
    assert!(controller(&harness, player).grounded());
    assert_eq!(
        controller(&harness, player).air_jumps_left(),
        1,
        "landing refills it"
    );
}

#[test]
fn aerial_jump_plays_the_tuck_clip_until_the_ascent_ends() {
    let (mut harness, player) = run();
    harness.step(10);
    harness.harness_mut().press_action("jump");
    harness.step(1);
    harness.harness_mut().release_action("jump");
    harness.step(2);
    assert_eq!(clip(&harness, player), "ex10_player_jump");
    harness.harness_mut().press_action("jump");
    harness.step(1);
    assert_eq!(clip(&harness, player), "ex10_player_double_jump");
    harness.step(1);
    assert_eq!(clip(&harness, player), "ex10_player_double_jump");
    let mut frames = 0;
    while harness.world().get::<Velocity>(player).unwrap().0.y < 0.0 {
        harness.step(1);
        frames += 1;
        assert!(frames < 120, "the rise never ended");
    }
    harness.step(1);
    assert_eq!(clip(&harness, player), "ex10_player_fall");
    harness.harness_mut().release_action("jump");
    while !controller(&harness, player).grounded() {
        harness.step(1);
    }
    harness.step(20);
    harness.harness_mut().press_action("jump");
    harness.step(1);
    assert_eq!(clip(&harness, player), "ex10_player_jump");
}

#[test]
fn a_fall_is_fatal_during_immunity_and_the_screen_advances_while_game_time_is_paused() {
    let (mut harness, player) = run();
    harness.step(5);
    damage(harness.world_mut(), player, 10.0);
    assert!(health(&harness, player).is_invulnerable());
    let at = centre(&harness, player);
    teleport(&mut harness, player, Vec2::new(at.x, KILL_Y + 1.0));
    harness.step(1);
    assert!(
        health(&harness, player).is_dead(),
        "the window does not save a fall"
    );
    harness.step(2);
    settle(&mut harness);
    assert_eq!(top(&harness), Some(GAME_OVER));
    assert!(harness.world().resource::<Time>().is_paused());
    harness.step(60);
    assert_eq!(top(&harness), Some(GAME_OVER));
    assert!(health(&harness, player).is_dead());
}
