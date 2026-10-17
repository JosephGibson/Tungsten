//! The platformer fixture: a hero on a kinematic body under the move
//! actions and a jump slot, a ground, a door zone, a spark and a lift.

use tungsten_core::physics::Velocity;
use tungsten_core::{PluginSet, Transform, World};

use super::{DTS, body_at, drawn_at, frames_for, recipe, spawn, tagged};
use crate::kit::{
    Children, Flags, Health, KitPlugins, PathFollowerPlugin, PlatformerController,
    PlatformerControllerPlugin, Stats, ZoneEntered,
};
use crate::state::{GameState, SceneEntity, StateContext, StateId, StateStack};

const GENRE: &str = "platformer";

/// The hero standing on the ground: the ground's top at y 290, the hero's
/// feet on it.
fn hero_on_ground(dt: f32, plugins: PluginSet) -> (super::Harness, tungsten_core::Entity) {
    let mut harness = recipe(GENRE, plugins, dt);
    spawn(&mut harness, "ground", [0.0, 300.0]);
    let hero = spawn(&mut harness, "hero", [0.0, 270.0]);
    harness.step(2);
    (harness, hero)
}

fn grounded(harness: &super::Harness, hero: tungsten_core::Entity) -> bool {
    harness
        .world()
        .get::<PlatformerController>(hero)
        .unwrap()
        .grounded()
}

#[test]
fn the_hero_runs_jumps_over_the_ground_and_lands_grounded() {
    for dt in DTS {
        let (mut harness, hero) = hero_on_ground(dt, KitPlugins::set());
        assert!(grounded(&harness, hero), "dt {dt}: it landed");
        let start = body_at(&harness, hero);

        harness.press_action("move_right");
        harness.step(frames_for(0.5, dt));
        harness.release_action("move_right");
        let ran = body_at(&harness, hero);
        assert!(ran.x > start.x + 80.0, "dt {dt}: it ran: {ran}");
        assert!((ran.y - start.y).abs() < 0.1, "dt {dt}: on the ground");

        // Held for 0.3 s: a release while rising would cut the jump.
        harness.press_action("jump");
        harness.step(frames_for(0.3, dt));
        harness.release_action("jump");
        let mut top = ran.y;
        let mut landed = false;
        for _ in 0..frames_for(1.5, dt) {
            harness.step(1);
            top = top.min(body_at(&harness, hero).y);
            if top < ran.y - 50.0 && grounded(&harness, hero) {
                landed = true;
                break;
            }
        }
        assert!(top < ran.y - 50.0, "dt {dt}: it rose: {top}");
        assert!(landed, "dt {dt}: it landed again");
        assert!((body_at(&harness, hero).y - start.y).abs() < 0.1);
    }
}

#[test]
fn the_door_zone_sends_one_entry_for_the_hero() {
    for dt in DTS {
        let (mut harness, hero) = hero_on_ground(dt, KitPlugins::set());
        let door = spawn(&mut harness, "door", [200.0, 260.0]);
        harness.press_action("move_right");
        let mut entered = Vec::new();
        for _ in 0..frames_for(2.0, dt) {
            harness.step(1);
            entered.extend(harness.events::<ZoneEntered>().copied());
        }
        assert_eq!(entered.len(), 1, "dt {dt}: {entered:?}");
        assert_eq!((entered[0].zone, entered[0].entity), (door, hero));
        assert!(entered[0].position.x > 150.0);
    }
}

#[test]
fn the_spark_expires() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let spark = spawn(&mut harness, "spark", [0.0, 0.0]);
        harness.step(frames_for(0.1, dt));
        assert!(harness.world().is_alive(spark), "dt {dt}: 0.1 of 0.2 s");
        harness.step(frames_for(0.2, dt));
        assert!(!harness.world().is_alive(spark), "dt {dt}: 0.3 s");
    }
}

#[test]
fn the_lift_carries_the_hero_with_the_plugins_in_reverse_order() {
    for dt in DTS {
        // The controller registered before the follower: the within-phase
        // edge, not registration order, puts the follower first.
        let plugins = KitPlugins::set()
            .without::<PathFollowerPlugin>()
            .without::<PlatformerControllerPlugin>()
            .with(PlatformerControllerPlugin)
            .with(PathFollowerPlugin);
        let mut harness = recipe(GENRE, plugins, dt);
        // The lift's top at y 192; the hero's feet on it, at the lift's
        // left-to-right path's start.
        let lift = spawn(&mut harness, "lift", [100.0, 200.0]);
        let hero = spawn(&mut harness, "hero", [100.0, 172.0]);
        harness.step(frames_for(0.1, dt));
        assert!(grounded(&harness, hero), "dt {dt}: it landed on the lift");
        let offset = body_at(&harness, hero).x - body_at(&harness, lift).x;
        let start = body_at(&harness, lift).x;
        for _ in 0..frames_for(1.0, dt) {
            harness.step(1);
            let (lift_at, hero_at) = (body_at(&harness, lift), body_at(&harness, hero));
            assert!(grounded(&harness, hero), "dt {dt}: still aboard");
            assert!(
                (hero_at.x - lift_at.x - offset).abs() < 0.05,
                "dt {dt}: carried: {hero_at} on {lift_at}"
            );
        }
        assert!(
            body_at(&harness, lift).x > start + 90.0,
            "dt {dt}: the lift moved"
        );
    }
}

#[test]
fn the_reverse_order_still_resolves_the_follower_before_the_controller() {
    let plugins = KitPlugins::set()
        .without::<PathFollowerPlugin>()
        .without::<PlatformerControllerPlugin>()
        .with(PlatformerControllerPlugin)
        .with(PathFollowerPlugin);
    let mut app = crate::App::new(tungsten_core::Config::default()).unwrap();
    app.add_plugins(plugins);
    app.resolve_schedule().unwrap();
    let text = app.schedule().resolved_text();
    let fixed = text.lines().nth(2).unwrap();
    let follow = fixed.find("path_follow").unwrap();
    let controller = fixed.find("platformer_controller").unwrap();
    assert!(follow < controller, "{fixed}");
}

#[test]
fn the_runner_runs_at_its_move_speed_stat_with_its_max_health() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let runner = spawn(&mut harness, "runner", [0.0, 270.0]);
        harness.step(2);
        let world = harness.world();
        let health = world.get::<Health>(runner).unwrap();
        assert_eq!(
            (health.current(), health.max()),
            (4.0, 4.0),
            "dt {dt}: 3/3 becomes 4/4"
        );
        assert_eq!(
            world.get::<Stats>(runner).unwrap().get("jump_boost"),
            Some(1.0)
        );
        harness.press_action("move_right");
        harness.step(frames_for(0.5, dt));
        let speed = harness.world().get::<Velocity>(runner).unwrap().0.x;
        assert!(
            (speed - 150.0).abs() < 1e-3,
            "dt {dt}: the stat's 150, not the controller's 200: {speed}"
        );
    }
}

#[test]
fn the_key_sets_the_runners_flag_and_goes() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let runner = spawn(&mut harness, "runner", [0.0, 270.0]);
        let key = spawn(&mut harness, "key", [80.0, 270.0]);
        harness.step(2);
        assert!(!harness.world().get::<Flags>(runner).unwrap().has("has_key"));
        harness.press_action("move_right");
        harness.step(frames_for(1.0, dt));
        let world = harness.world();
        assert!(
            world.get::<Flags>(runner).unwrap().has("has_key"),
            "dt {dt}"
        );
        assert!(!world.is_alive(key), "dt {dt}: the key went");
    }
}

/// A room whose `on_enter` spawns the torch with its `SceneEntity`.
struct Room;

impl GameState for Room {
    fn id(&self) -> StateId {
        "room"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let torch = ctx
            .world
            .prepare_prefab("torch", [300.0, 200.0])
            .unwrap()
            .with((SceneEntity {
                state_id: ctx.state_id,
            },));
        torch.spawn(ctx.world);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, _world: &mut World) {}
}

#[test]
fn the_torchs_glow_hangs_above_it_and_leaves_with_its_state() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        harness.step(1);
        harness
            .world_mut()
            .resource_mut::<StateStack>()
            .request_push(Room);
        harness.step(2);
        let torch = tagged(&harness, "torch")[0];
        let glows = tagged(&harness, "glow");
        assert_eq!(glows.len(), 1, "dt {dt}");
        let glow = glows[0];
        let world = harness.world();
        assert_eq!(
            world
                .get::<Children>(torch)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            [glow]
        );
        assert_eq!(world.get::<SceneEntity>(glow).unwrap().state_id, "room");
        assert_eq!(
            drawn_at(&harness, glow),
            drawn_at(&harness, torch) + glam::Vec2::new(0.0, -12.0)
        );

        harness
            .world_mut()
            .get_mut::<Transform>(torch)
            .unwrap()
            .position
            .x += 50.0;
        harness.step(1);
        assert_eq!(
            drawn_at(&harness, glow),
            glam::Vec2::new(350.0, 188.0),
            "dt {dt}: it follows"
        );

        harness
            .world_mut()
            .resource_mut::<StateStack>()
            .request_pop();
        harness.step(1);
        assert!(
            tagged(&harness, "torch").is_empty() && tagged(&harness, "glow").is_empty(),
            "dt {dt}"
        );
    }
}
