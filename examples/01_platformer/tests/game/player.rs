//! The wizard on the kit's controller: a press's step, the air jump's
//! fresh press, its burst, tuck and refill, the falls and escapes that end
//! a run, inside the invulnerability window too, a blow's tint and hurt
//! clip, and each cast's gesture with the staff's flare.

use example_01_platformer::features::level::{self, KILL_Y, MAP, TILE, WORLD_MAX, WORLD_MIN};
use example_01_platformer::features::player::components::Moves;
use example_01_platformer::features::player::systems::{
    AIR_JUMP_BURST, DEATH_CLIP, HURT_CLIP, escaped,
};
use example_01_platformer::features::player::{BODY_OFFSET, PLAYER_HALF};
use example_01_platformer::features::run::DEATH_SECS;
use example_01_platformer::shared::Burst;
use example_01_platformer::shell::game_over::GAME_OVER;
use glam::Vec2;
use tungsten::core::{
    AnimationState, Collider, Entity, ParticleConfigRegistry, ParticleEmitter, Position,
    TilemapRegistry, Time, Transform, Velocity,
};
use tungsten::kit::{Health, PlatformerController, damage};
use tungsten::testing::UiHarness;

use example_01_platformer::game::GAMEPLAY;

use super::{ball, body, calm, centre, fall_dead, ground, run, settle, stand, teleport, top};

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
fn despawn_out_of_bounds_kills_escaped_player_whose_body_drops() {
    let (mut harness, player) = run();
    harness.step(5);
    // Past the box's right side at a height the kill row leaves alone.
    let out = Vec2::new(WORLD_MAX.x + PLAYER_HALF.x + 50.0, 20.0 * TILE);
    teleport(&mut harness, player, out);
    harness.step(1);
    assert!(health(&harness, player).is_dead());
    let held = centre(&harness, player);
    harness.step(30);
    let fell = centre(&harness, player);
    assert!(
        (fell.x - held.x).abs() < 0.01 && fell.y > held.y + 10.0,
        "the body drops where it died: {held} -> {fell}"
    );
    fall_dead(&mut harness);
    assert_eq!(top(&harness), Some(GAME_OVER));
}

#[test]
fn the_dead_fall_for_two_seconds_before_game_over() {
    let (mut harness, player) = run();
    harness.step(5);
    // Killed in the air above the spawn's turf, running right.
    let at = centre(&harness, player);
    teleport(&mut harness, player, at - Vec2::new(0.0, 200.0));
    harness.harness_mut().press_action("move_right");
    harness.step(3);
    damage(harness.world_mut(), player, 1000.0);
    harness.step(1);
    assert!(health(&harness, player).is_dead());
    let died = centre(&harness, player);
    // Its input is dead: it runs and jumps no more and drops straight down
    // until it lands.
    harness.harness_mut().press_action("jump");
    let mut since = 0;
    while !controller(&harness, player).grounded() {
        harness.step(1);
        since += 1;
        assert!(since < 120, "it lands");
    }
    harness.harness_mut().release_action("jump");
    harness.harness_mut().release_action("move_right");
    let landed = centre(&harness, player);
    assert!(
        (landed.x - died.x).abs() < 0.01 && landed.y > died.y + 150.0,
        "{died} -> {landed}"
    );
    assert_eq!(clip(&harness, player), DEATH_CLIP, "the death clip holds");
    // Game over waits out the death's two game seconds.
    let wait = (DEATH_SECS * 60.0).round() as u32;
    harness.step(wait - 6 - since);
    assert_eq!(top(&harness), Some(GAMEPLAY), "0.1 s before game over");
    assert!(
        (centre(&harness, player) - landed).length() < 0.01,
        "it lies still"
    );
    harness.step(12);
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
        (430, 72, 64, 64)
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
    let shown = moves
        .shown
        .as_ref()
        .map(|shown| shown.state.animation_id.trim_end_matches("_left"));
    assert!(
        moves.landing == 0.0 && shown == Some(DEATH_CLIP),
        "the death clip at once: {moves:?}"
    );
    fall_dead(&mut harness);
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
    fall_dead(&mut harness);
    assert_eq!(top(&harness), Some(GAME_OVER));
    assert!(harness.world().resource::<Time>().is_paused());
    harness.step(60);
    assert_eq!(top(&harness), Some(GAME_OVER));
    assert!(health(&harness, player).is_dead());
}

/// The courier on a test floor at y 1000, at rest, one step a frame, with
/// small balls in two layers over its head (`over`) or stacked beside it.
fn buried(over: bool) -> (UiHarness, Entity) {
    const FLOOR: f32 = 1000.0;
    let (mut harness, player) = run();
    calm(&mut harness);
    ground(&mut harness, FLOOR);
    stand(&mut harness, player, 600.0, FLOOR);
    harness.harness_mut().set_dt(DT);
    harness.step(10);
    let at = centre(&harness, player);
    if over {
        for layer in 0..2 {
            for i in 0..5 {
                let x = at.x - 30.0 + 15.0 * i as f32;
                let y = at.y - PLAYER_HALF.y - 8.0 - 15.0 * layer as f32;
                ball(&mut harness, Vec2::new(x, y), true);
            }
        }
    } else {
        for side in [-1.0, 1.0] {
            for row in 0..3 {
                let x = at.x + side * (PLAYER_HALF.x + 8.0);
                ball(
                    &mut harness,
                    Vec2::new(x, FLOOR - 8.0 - 15.0 * row as f32),
                    true,
                );
            }
        }
    }
    harness.step(1);
    (harness, player)
}

#[test]
fn a_jump_under_small_balls_shoves_them_off_and_rises_clear() {
    let feet = |h: &UiHarness, player: Entity| centre(h, player).y + PLAYER_HALF.y;
    let (mut harness, player) = buried(true);
    let floor = feet(&harness, player);
    harness.harness_mut().press_action("jump");
    let mut cleared = false;
    for _ in 0..30 {
        harness.step(1);
        let controller = controller(&harness, player);
        if !controller.grounded() {
            assert_eq!(controller.air_jumps_left(), 1, "the air jump kept");
        }
        if floor - feet(&harness, player) >= 100.0 {
            cleared = true;
            break;
        }
    }
    harness.harness_mut().release_action("jump");
    assert!(cleared, "it rose clear of the balls");
    // Beside it alone, one press jumps once, no launch follows, and the air
    // jump is left.
    let (mut harness, player) = buried(false);
    harness.harness_mut().press_action("jump");
    let mut jumps = 0;
    let mut rise = Vec::new();
    for _ in 0..20 {
        harness.step(1);
        let controller = controller(&harness, player);
        jumps += usize::from(controller.jumped());
        if !controller.grounded() {
            assert_eq!(controller.air_jumps_left(), 1, "the air jump left");
        }
        rise.push(harness.world().get::<Velocity>(player).unwrap().0.y);
    }
    harness.harness_mut().release_action("jump");
    assert_eq!(jumps, 1, "one jump");
    let first = rise.iter().position(|&v| v < 0.0).expect("it rose");
    assert!(
        rise[first + 1] > rise[first],
        "slowing, not launched: {rise:?}"
    );
}

#[test]
fn a_blow_tints_the_courier_red_for_a_moment() {
    use example_01_platformer::features::combat::{Hit, Kind};
    use tungsten::core::Sprite;
    let (mut harness, player) = run();
    harness.harness_mut().set_dt(DT);
    harness.step(2);
    let tint = |h: &UiHarness| h.world().get::<Sprite>(player).unwrap().color;
    assert_eq!(tint(&harness), [255; 4]);
    assert!(Hit::new(player, [(Kind::Physical, 5.0)]).queue(harness.world_mut()));
    harness.step(1);
    let red = tint(&harness);
    assert!(
        red[0] == 255 && red[1] < 160 && red[2] < 160,
        "red: {red:?}"
    );
    // The hurt clip's frame shows the step after the blow.
    let shown = |h: &UiHarness| {
        let world = h.world();
        let id = world.get::<Sprite>(player).unwrap().asset_id;
        let assets = world.resource::<tungsten::core::AssetRegistry>();
        assets.sprite_name(id).unwrap().to_owned()
    };
    assert_eq!(clip(&harness, player).trim_end_matches("_left"), HURT_CLIP);
    assert!(
        shown(&harness).starts_with(HURT_CLIP),
        "{}",
        shown(&harness)
    );
    // Gone after 0.12 s, a frame's slack.
    harness.step(9);
    assert_eq!(tint(&harness), [255; 4]);
    // The clip after its 0.2 s.
    harness.step(4);
    assert!(!clip(&harness, player).starts_with(HURT_CLIP));
}

#[test]
fn each_gesture_shows_its_clip_and_the_staff_flares() {
    use example_01_platformer::features::gun;
    use example_01_platformer::features::level::components::Glow;
    use example_01_platformer::features::player::components::StaffGlow;
    use tungsten::core::{AssetRegistry, Sprite};
    use tungsten::kit::Stats;
    const TOP: f32 = 1000.0;
    let (mut harness, player) = run();
    calm(&mut harness);
    ground(&mut harness, TOP);
    stand(&mut harness, player, 600.0, TOP);
    harness.harness_mut().set_dt(DT);
    harness.step(10);
    super::pin_camera(&mut harness, Vec2::new(0.0, 500.0));
    let stats = harness.world_mut().get_mut::<Stats>(player).unwrap();
    stats.set_base("max_mana", 1000.0);
    stats.set_base("mana", 1000.0);
    let base = harness.world().get::<StaffGlow>(player).unwrap().0;
    assert_eq!(base.reach(), 175.0, "the prefab's reach");
    let glow = |h: &UiHarness| *h.world().get::<Glow>(player).unwrap();
    let shown = |h: &UiHarness| {
        let world = h.world();
        let id = world.get::<Sprite>(player).unwrap().asset_id;
        let assets = world.resource::<AssetRegistry>();
        assets.sprite_name(id).unwrap().to_owned()
    };
    let ahead = centre(&harness, player) + Vec2::new(240.0, -120.0);
    // Each pressed spell's one-shot the frame after its press, the staff
    // flaring over it, its reach the prefab's, and 0.3 s on its flare done.
    for (slot, wanted) in [
        (0, "ex10_player_bolt"),
        (4, "ex10_player_push"),
        (6, "ex10_player_conjure"),
        (7, "ex10_player_toss"),
    ] {
        super::choose(&mut harness, slot);
        super::aim_at(&mut harness, ahead);
        harness.harness_mut().press_action("cast");
        harness.step(1);
        harness.harness_mut().release_action("cast");
        assert_eq!(clip(&harness, player), wanted, "slot {slot}");
        assert!(glow(&harness).radius > base.radius, "slot {slot}: a flare");
        assert_eq!(glow(&harness).reach(), base.reach(), "slot {slot}");
        harness.step(20);
        assert_eq!(glow(&harness).radius, base.radius, "slot {slot}: done");
        assert_eq!(glow(&harness).reach(), base.reach(), "slot {slot}");
    }
    // Standing still, the held flamethrower levels the staff along the aim
    // and a shot raises the pistol, each over the idle clip.
    super::choose(&mut harness, 1);
    super::aim_at(&mut harness, ahead);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    assert!(
        shown(&harness).starts_with("ex10_player_staff_"),
        "{}",
        shown(&harness)
    );
    harness.harness_mut().release_action("cast");
    harness.step(20);
    harness.harness_mut().press_action(gun::SHOOT);
    harness.step(1);
    harness.harness_mut().release_action(gun::SHOOT);
    assert!(
        shown(&harness).starts_with("ex10_player_aim_"),
        "{}",
        shown(&harness)
    );
}

#[test]
fn an_air_jump_and_a_spell_switch_each_sound_once() {
    use example_01_platformer::features::audio::components::{Sound, Sounded};
    let (mut harness, player) = buried(false);
    // Clear of the balls beside it.
    for ball in harness
        .world()
        .query::<(
            Entity,
            &example_01_platformer::features::toys::components::SmallBall,
        )>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>()
    {
        harness.world_mut().despawn(ball);
    }
    harness.step(2);
    let heard = |h: &UiHarness, sound: Sound| {
        h.harness()
            .events::<Sounded>()
            .filter(|sounded| sounded.sound == sound)
            .count()
    };
    let jump = |h: &mut UiHarness, frames: u32| -> usize {
        h.harness_mut().press_action("jump");
        let mut sounds = 0;
        for _ in 0..frames {
            h.step(1);
            sounds += heard(h, Sound::AirJumped);
        }
        h.harness_mut().release_action("jump");
        h.step(1);
        sounds + heard(h, Sound::AirJumped)
    };
    assert_eq!(jump(&mut harness, 3), 0, "a ground jump is silent");
    harness.step(4);
    assert!(!controller(&harness, player).grounded());
    assert_eq!(jump(&mut harness, 3), 1, "the air jump sounds once");
    // A switch clicks once; choosing the chosen slot again, nothing.
    super::choose(&mut harness, 3);
    assert_eq!(heard(&harness, Sound::Switched), 1);
    super::choose(&mut harness, 3);
    assert_eq!(heard(&harness, Sound::Switched), 0);
}

/// The wizard at x 600 on the sky's ground, walking right for `frames`
/// frames into the body `place` sets down: his centre's x each frame, and
/// the body's centre before and after.
fn walk_into(
    place: impl FnOnce(&mut UiHarness) -> Entity,
    frames: usize,
) -> (Vec<f32>, Vec2, Vec2) {
    const FLOOR: f32 = 1000.0;
    let (mut harness, player) = run();
    calm(&mut harness);
    ground(&mut harness, FLOOR);
    stand(&mut harness, player, 600.0, FLOOR);
    harness.harness_mut().set_dt(DT);
    harness.step(10);
    let thing = place(&mut harness);
    harness.step(1);
    let start = body(&harness, thing);
    harness.harness_mut().press_action("move_right");
    let mut xs = Vec::new();
    for _ in 0..frames {
        harness.step(1);
        xs.push(centre(&harness, player).x);
    }
    harness.harness_mut().release_action("move_right");
    (xs, start, body(&harness, thing))
}

#[test]
fn the_wizard_walks_small_bodies_aside() {
    use example_01_platformer::features::props::{self, CRATE_HALF, components::PropKind};
    use example_01_platformer::features::toys::{self, IRON_HALF};
    const FLOOR: f32 = 1000.0;
    // A small ball ahead rolls off before him.
    let (xs, start, end) = walk_into(|h| ball(h, Vec2::new(680.0, FLOOR - 7.5), true), 30);
    assert!(
        end.x - start.x > 100.0,
        "the ball went ahead: {start} {end}"
    );
    assert!(xs[29] > 700.0, "he walked on: {}", xs[29]);
    // A crate ahead moves with him, at his run less its 12 of 60.
    let crate_at = Vec2::new(700.0, FLOOR - CRATE_HALF.y - 0.5);
    let (xs, start, end) = walk_into(
        |h| props::spawn(h.world_mut(), PropKind::Crate, crate_at),
        50,
    );
    let pace = (xs[45] - xs[15]) / (30.0 * DT);
    assert!(
        (pace - 448.0).abs() <= 44.8,
        "his pace behind the crate: {pace} px/s"
    );
    assert!(
        end.x - start.x > 200.0,
        "the crate went with him: {start} {end}"
    );
    assert!(
        xs[49] + PLAYER_HALF.x <= end.x - CRATE_HALF.x + 0.5,
        "behind it: {} {end}",
        xs[49]
    );
    // Iron, 400, stops him.
    let iron_at = Vec2::new(700.0 + IRON_HALF.x, FLOOR - IRON_HALF.y);
    let (xs, start, end) = walk_into(|h| toys::place_iron(h.world_mut(), iron_at), 40);
    assert!(start.distance(end) < 1.0, "the iron held: {start} {end}");
    assert!(
        (xs[39] + PLAYER_HALF.x - 700.0).abs() < 0.5,
        "he stands at it: {}",
        xs[39]
    );
}
