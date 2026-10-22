//! The platformer fixture: a hero on a kinematic body under the move
//! actions and a jump slot, a ground, a door zone, a spark and a lift; M64's
//! conveyor, bobbing lift, acrobat, firebar and firefly; M65's fireballs,
//! bricks, climber, spiker and coin; M67's regen and haste on the runner;
//! M76's bolt off armour, saved mid-flight.

use glam::Vec2;
use tungsten_core::physics::Velocity;
use tungsten_core::{AnimationState, PluginSet, Transform, World};

use super::{DTS, body_at, drawn_at, frames_for, motion_reversed, point_of, recipe, spawn, tagged};
use crate::kit::{
    CharacterAnimator, Children, Counters, EffectRequest, Effects, Facing, Flags, Health,
    KitPlugins, LinearMotion, PathFollowerPlugin, PlatformerController, PlatformerControllerPlugin,
    ProjectileExpired, Stats, ZoneEntered, damage, knock_back,
};
use crate::state::{GameState, SceneEntity, StateContext, StateId, StateStack};
use tungsten_core::DataRef;

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

/// The animator's state and the clip it shows.
fn animated(harness: &super::Harness, entity: tungsten_core::Entity) -> (&'static str, String) {
    let world = harness.world();
    (
        world.get::<CharacterAnimator>(entity).unwrap().state(),
        world
            .get::<AnimationState>(entity)
            .unwrap()
            .animation_id
            .clone(),
    )
}

#[test]
fn a_conveyor_and_a_bobbing_lift_carry_the_hero_with_motion_built_in_reverse() {
    for dt in DTS {
        let mut harness = recipe(GENRE, motion_reversed(), dt);
        // Each platform's top at y 192, a hero's feet on it.
        let belt = spawn(&mut harness, "conveyor", [100.0, 200.0]);
        let rider = spawn(&mut harness, "hero", [100.0, 172.0]);
        let lift = spawn(&mut harness, "bob_lift", [500.0, 200.0]);
        let bobber = spawn(&mut harness, "hero", [500.0, 172.0]);
        harness.step(frames_for(0.1, dt));
        assert!(
            grounded(&harness, rider) && grounded(&harness, bobber),
            "dt {dt}: aboard"
        );
        let gap = |harness: &super::Harness, hero, platform| {
            body_at(harness, hero) - body_at(harness, platform)
        };
        let (belt_gap, lift_gap) = (gap(&harness, rider, belt), gap(&harness, bobber, lift));
        let belt_start = body_at(&harness, belt).x;
        let (mut low, mut high) = (f32::NEG_INFINITY, f32::INFINITY);
        for _ in 0..frames_for(2.0, dt) {
            harness.step(1);
            assert!(
                grounded(&harness, rider) && grounded(&harness, bobber),
                "dt {dt}"
            );
            let riding = gap(&harness, rider, belt) - belt_gap;
            assert!(
                riding.length() < 0.05,
                "dt {dt}: carried by the belt: {riding}"
            );
            let bobbing = gap(&harness, bobber, lift) - lift_gap;
            assert!(
                bobbing.length() < 0.05,
                "dt {dt}: carried by the lift: {bobbing}"
            );
            let y = body_at(&harness, lift).y;
            low = low.max(y);
            high = high.min(y);
        }
        assert!(
            (body_at(&harness, belt).x - belt_start - 80.0).abs() < 1.0,
            "dt {dt}"
        );
        assert!(low - high > 40.0, "dt {dt}: the lift bobbed {}", low - high);
    }
}

#[test]
fn the_acrobat_faces_and_animates_its_run_falls_at_its_cap_and_is_knocked_back() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let acrobat = spawn(&mut harness, "acrobat", [0.0, 270.0]);
        // Far off the ground: it falls the whole recipe.
        let diver = spawn(&mut harness, "acrobat", [2000.0, -5000.0]);
        let mut fastest: f32 = 0.0;
        let mut frame = |harness: &mut super::Harness, count: u32| {
            for _ in 0..count {
                harness.step(1);
                let fall = harness.world().get::<Velocity>(diver).unwrap().0.y;
                assert!(fall <= 900.0 + 1e-3, "dt {dt}: at its cap: {fall}");
                fastest = fastest.max(fall);
            }
        };
        frame(&mut harness, 2);
        assert_eq!(
            animated(&harness, acrobat),
            ("idle", "acrobat_idle_right".into())
        );

        harness.press_action("move_left");
        frame(&mut harness, frames_for(0.3, dt));
        let facing = harness.world().get::<Facing>(acrobat).unwrap().direction();
        assert_eq!(facing, Vec2::NEG_X, "dt {dt}");
        assert_eq!(
            animated(&harness, acrobat),
            ("move", "acrobat_run_left".into())
        );
        harness.release_action("move_left");
        frame(&mut harness, 2);
        assert_eq!(
            animated(&harness, acrobat),
            ("idle", "acrobat_idle_left".into()),
            "dt {dt}: it keeps facing left"
        );

        harness.press_action("jump");
        frame(&mut harness, 2);
        assert_eq!(animated(&harness, acrobat), ("rise", "acrobat_rise".into()));
        harness.release_action("jump");
        let mut fell = false;
        for _ in 0..frames_for(1.0, dt) {
            frame(&mut harness, 1);
            fell |= animated(&harness, acrobat).0 == "fall";
            if fell && grounded(&harness, acrobat) {
                break;
            }
        }
        assert!(
            fell && grounded(&harness, acrobat),
            "dt {dt}: it fell and landed"
        );

        // Thrown up and right, its input locked against the stick.
        assert!(knock_back(
            harness.world_mut(),
            acrobat,
            Vec2::new(300.0, -400.0)
        ));
        harness.press_action("move_left");
        frame(&mut harness, frames_for(0.1, dt));
        let thrown = harness.world().get::<Velocity>(acrobat).unwrap().0;
        assert!(
            (thrown.x - 300.0).abs() < 1e-3,
            "dt {dt}: the launch, not the run: {thrown}"
        );
        frame(&mut harness, frames_for(0.3, dt));
        assert!(
            harness.world().get::<Velocity>(acrobat).unwrap().0.x < 0.0,
            "dt {dt}: the run once the lock ends"
        );
        harness.release_action("move_left");
        assert!(
            (fastest - 900.0).abs() < 1e-3,
            "dt {dt}: it reached its cap: {fastest}"
        );
    }
}

#[test]
fn a_firebars_ball_circles_its_pivot_and_a_firefly_trails_the_hero() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let hero = spawn(&mut harness, "hero", [0.0, 270.0]);
        let pivot = Vec2::new(300.0, 100.0);
        let ball = spawn(&mut harness, "firebar", pivot.to_array());
        let firefly = spawn(&mut harness, "firefly", [-200.0, 100.0]);
        for _ in 0..frames_for(2.0, dt) {
            harness.step(1);
            let radius = body_at(&harness, ball).distance(pivot);
            assert!((radius - 64.0).abs() < 1e-3, "dt {dt}: {radius}");
        }
        let beside = body_at(&harness, hero) + Vec2::new(-24.0, -32.0);
        assert!(
            body_at(&harness, firefly).distance(beside) < 0.5,
            "dt {dt}: it settled"
        );
        harness.press_action("move_right");
        harness.step(frames_for(0.5, dt));
        harness.release_action("move_right");
        let trail = body_at(&harness, hero).x - 24.0 - body_at(&harness, firefly).x;
        assert!(
            trail > 5.0,
            "dt {dt}: it trails the running hero by {trail}"
        );
    }
}

/// Runs `frames` frames, collecting each one's projectile ends.
fn ends(harness: &mut super::Harness, frames: u32) -> Vec<ProjectileExpired> {
    let mut expired = Vec::new();
    for _ in 0..frames {
        harness.step(1);
        expired.extend(harness.events::<ProjectileExpired>().copied());
    }
    expired
}

#[test]
fn a_fireball_stops_at_a_brick() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // The brick's left face at x 90.
        spawn(&mut harness, "brick", [100.0, 100.0]);
        let fireball = spawn(&mut harness, "fireball", [0.0, 100.0]);
        let expired = ends(&mut harness, frames_for(0.5, dt));
        assert_eq!(expired.len(), 1, "dt {dt}");
        assert!(
            expired[0].position.distance(Vec2::new(86.0, 100.0)) < 1e-3,
            "dt {dt}: at the face: {}",
            expired[0].position
        );
        assert!(!harness.world().is_alive(fireball));
    }
}

#[test]
fn a_bouncing_fireball_turns_at_a_brick_then_stops() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "brick", [100.0, 100.0]);
        spawn(&mut harness, "brick", [-100.0, 100.0]);
        let ricochet = spawn(&mut harness, "ricochet", [0.0, 100.0]);
        let mut turned = false;
        let mut expired = Vec::new();
        for _ in 0..frames_for(1.0, dt) {
            harness.step(1);
            expired.extend(harness.events::<ProjectileExpired>().copied());
            if let Some(motion) = harness.world().get::<LinearMotion>(ricochet)
                && motion.velocity().x < 0.0
                && !turned
            {
                turned = true;
                assert!(
                    point_of(&harness, ricochet).x <= 86.0 + 1e-3,
                    "dt {dt}: placed at the right brick"
                );
            }
        }
        assert!(turned, "dt {dt}: it turned");
        assert_eq!(expired.len(), 1, "dt {dt}");
        assert!(
            expired[0].position.distance(Vec2::new(-86.0, 100.0)) < 1e-3,
            "dt {dt}: stopped at the left brick: {}",
            expired[0].position
        );
    }
}

#[test]
fn a_spikers_touch_hurts_the_climber_and_throws_it_back() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        spawn(&mut harness, "spiker", [100.0, 270.0]);
        // Touching the spiker's left side, its feet on the ground.
        let climber = spawn(&mut harness, "climber", [82.0, 270.0]);
        harness.step(1);
        assert_eq!(
            harness.world().get::<Health>(climber).unwrap().current(),
            4.0,
            "dt {dt}: hurt once"
        );
        harness.step(frames_for(0.2, dt));
        let x = body_at(&harness, climber).x;
        assert!(x < 82.0 - 10.0, "dt {dt}: thrown back left: {x}");
    }
}

#[test]
fn a_coin_counts_once() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let climber = spawn(&mut harness, "climber", [100.0, 270.0]);
        let coin = spawn(&mut harness, "coin", [100.0, 270.0]);
        harness.step(frames_for(0.25, dt));
        assert!(!harness.world().is_alive(coin), "dt {dt}");
        assert_eq!(
            harness
                .world()
                .get::<Counters>(climber)
                .unwrap()
                .get("coins"),
            1,
            "dt {dt}: once"
        );
    }
}

// M67: effects on the runner.

#[test]
fn the_runners_regen_heals_it_each_period() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let runner = spawn(&mut harness, "runner", [0.0, 270.0]);
        harness.step(2);
        assert!(
            harness.world().get::<Effects>(runner).unwrap().has("regen"),
            "dt {dt}: the listed effect applied at the first step"
        );
        damage(harness.world_mut(), runner, 3.0);
        assert_eq!(
            harness.world().get::<Health>(runner).unwrap().current(),
            1.0
        );
        // Each second heals the curve's 2 at the scale of 1.
        harness.step(frames_for(0.9, dt));
        assert_eq!(
            harness.world().get::<Health>(runner).unwrap().current(),
            1.0,
            "dt {dt}"
        );
        harness.step(frames_for(0.2, dt));
        assert_eq!(
            harness.world().get::<Health>(runner).unwrap().current(),
            3.0,
            "dt {dt}"
        );
        harness.step(frames_for(1.0, dt));
        assert_eq!(
            harness.world().get::<Health>(runner).unwrap().current(),
            4.0,
            "dt {dt}: full"
        );
    }
}

#[test]
fn a_hasted_runner_runs_faster_for_its_seconds_and_no_longer() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let runner = spawn(&mut harness, "runner", [0.0, 270.0]);
        harness.step(2);
        harness.press_action("move_right");
        assert!(EffectRequest::new(runner, DataRef::by_id("hasted")).queue(harness.world_mut()));
        let speed = |harness: &super::Harness| harness.world().get::<Velocity>(runner).unwrap().0.x;
        harness.step(2);
        assert!(
            (speed(&harness) - 225.0).abs() < 1e-3,
            "dt {dt}: 150 and a half: {}",
            speed(&harness)
        );
        harness.step(frames_for(1.0, dt));
        assert!(
            (speed(&harness) - 225.0).abs() < 1e-3,
            "dt {dt}: still hasted"
        );
        harness.step(frames_for(0.6, dt));
        assert!(
            (speed(&harness) - 150.0).abs() < 1e-3,
            "dt {dt}: the stat's own: {}",
            speed(&harness)
        );
        assert!(
            !harness
                .world()
                .get::<Effects>(runner)
                .unwrap()
                .has("hasted")
        );
    }
}

// M72: mana a cast pauses, and an air jump.

#[test]
fn a_mages_mana_regenerates_to_its_max_and_a_cast_pauses_it() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let mage = spawn(&mut harness, "mage", [0.0, 270.0]);
        harness.step(2);
        let mana = |harness: &super::Harness| {
            let stats = harness.world().get::<Stats>(mage).unwrap();
            stats.base("mana").unwrap()
        };
        // 1.5 each 0.1 s: 15 a second.
        harness.step(frames_for(1.0, dt));
        assert!(
            (mana(&harness) - 55.0).abs() <= 1.5,
            "dt {dt}: {}",
            mana(&harness)
        );
        // The spell costs 20 and casts for a second, which holds the regen.
        harness.press_action("attack");
        harness.step(1);
        harness.release_action("attack");
        let cast = mana(&harness);
        assert!(cast < 40.0, "dt {dt}: the cost was spent: {cast}");
        harness.step(frames_for(0.9, dt));
        assert_eq!(mana(&harness), cast, "dt {dt}: held while casting");
        harness.step(frames_for(0.5, dt));
        assert!(mana(&harness) > cast + 3.0, "dt {dt}: it resumed");
        harness.step(frames_for(6.0, dt));
        assert_eq!(mana(&harness), 100.0, "dt {dt}: capped at max_mana");
    }
}

#[test]
fn an_air_jump_reaches_a_ledge_one_jump_cannot() {
    for dt in DTS {
        // `jumper` has one air jump, `runner` none; a ledge's top at y 160,
        // 130 px above the ground's, past one jump's 90 px peak.
        let land = |id: &str| {
            let mut harness = recipe(GENRE, KitPlugins::set(), dt);
            spawn(&mut harness, "ground", [0.0, 300.0]);
            spawn(&mut harness, "ledge", [200.0, 170.0]);
            let hero = spawn(&mut harness, id, [0.0, 270.0]);
            harness.step(2);
            harness.press_action("move_right");
            harness.press_action("jump");
            harness.step(frames_for(0.3, dt));
            harness.release_action("jump");
            harness.step(1);
            harness.press_action("jump");
            harness.step(frames_for(1.0, dt));
            harness.release_action("move_right");
            harness.release_action("jump");
            harness.step(frames_for(0.5, dt));
            let controller = harness.world().get::<PlatformerController>(hero).unwrap();
            (controller.grounded(), body_at(&harness, hero).y)
        };
        let (grounded, y) = land("jumper");
        assert!(
            grounded && (y - 140.0).abs() < 0.1,
            "dt {dt}: on the ledge: {y}"
        );
        let (grounded, y) = land("runner");
        assert!(
            grounded && (y - 270.0).abs() < 0.1,
            "dt {dt}: back on the ground: {y}"
        );
    }
}

// M76: walls by their physics layers. A bolt turns off armour in layer 2
// and ends at a tile, outside its bounce layers, with a bounce left; saved
// mid-flight, it resumes with the same meetings.

/// What a shot met: "passed", "bounced" or "expired", where, and the wall
/// as (an entity or a tile, its normal, its layers).
type Meeting = (&'static str, Vec2, Option<(bool, Vec2, u32)>);

/// A wall as a meeting names it.
fn wall_of(wall: crate::kit::WallContact) -> (bool, Vec2, u32) {
    (wall.entity.is_some(), wall.normal, wall.layers)
}

/// Runs `frames` frames, collecting each one's meetings.
fn meetings(harness: &mut super::Harness, frames: u32) -> Vec<Meeting> {
    use crate::kit::{ProjectileBounced, ProjectilePassed};

    let mut out = Vec::new();
    for _ in 0..frames {
        harness.step(1);
        out.extend(
            harness
                .events::<ProjectilePassed>()
                .map(|event| ("passed", event.position, Some(wall_of(event.wall)))),
        );
        out.extend(
            harness
                .events::<ProjectileBounced>()
                .map(|event| ("bounced", event.position, Some(wall_of(event.wall)))),
        );
        out.extend(
            harness
                .events::<ProjectileExpired>()
                .map(|event| ("expired", event.position, event.wall.map(wall_of))),
        );
    }
    out
}

/// The armoury: armour whose left face is at x 90 and a column of 16 px
/// tiles over x −120 to −104.
fn armoury(dt: f32) -> super::Harness {
    use tungsten_core::{LayerKind, TilemapData, TilemapInstance, TilemapRegistry};

    let mut harness = recipe(GENRE, KitPlugins::set(), dt);
    spawn(&mut harness, "armour", [100.0, 100.0]);
    let mut map = TilemapData::new(16, 16, 1, 12);
    let solid = map.add_tile("solid");
    let layer = map.add_layer("solid", LayerKind::Collision);
    map.fill(layer, solid);
    let world = harness.world_mut();
    world
        .resource_mut::<TilemapRegistry>()
        .insert("column".into(), map);
    let instance = world.spawn();
    world.insert(
        instance,
        TilemapInstance::new("column", Vec2::new(-120.0, 0.0)),
    );
    harness
}

/// The game's save of its bolt in flight.
#[derive(serde::Serialize, serde::Deserialize)]
struct BoltSave {
    projectile: crate::kit::Projectile,
    motion: LinearMotion,
    at: [f32; 2],
}

impl tungsten_core::SaveData for BoltSave {
    const SCHEMA: u32 = 1;
}

/// The bolt's meetings at `dt` before its save point and after it; with
/// `load`, the game saves it there and loads it into a fresh armoury.
fn bolt_run(dt: f32, load: bool) -> (Vec<Meeting>, Vec<Meeting>) {
    let mut harness = armoury(dt);
    let bolt = spawn(&mut harness, "bolt", [0.0, 100.0]);
    let before = meetings(&mut harness, frames_for(0.4, dt));
    if load {
        let world = harness.world();
        let at = body_at(&harness, bolt);
        let save = BoltSave {
            projectile: world.get::<crate::kit::Projectile>(bolt).unwrap().clone(),
            motion: world.get::<LinearMotion>(bolt).unwrap().clone(),
            at: [at.x, at.y],
        };
        let dir = std::env::temp_dir().join(format!(
            "tungsten_recipe_bolt_{}_{}",
            std::process::id(),
            frames_for(1.0, dt)
        ));
        let slots = tungsten_core::SaveSlots::new(Some(dir.clone()));
        slots.save("bolt", &save).unwrap();
        let loaded: BoltSave = slots.load("bolt").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        harness = armoury(dt);
        let bolt = spawn(&mut harness, "bolt", loaded.at);
        let world = harness.world_mut();
        world.insert(bolt, loaded.projectile);
        world.insert(bolt, loaded.motion);
    }
    let after = meetings(&mut harness, frames_for(0.6, dt));
    (before, after)
}

#[test]
fn a_bolt_turns_off_armour_and_ends_at_a_tile_and_resumes_after_a_load() {
    let left = Vec2::new(-1.0, 0.0);
    for dt in DTS {
        let (before, after) = bolt_run(dt, false);
        assert_eq!(before.len(), 1, "dt {dt}: {before:?}");
        assert_eq!(
            (before[0].0, before[0].2),
            ("bounced", Some((true, left, 1 << 2))),
            "dt {dt}"
        );
        assert!(
            before[0].1.distance(Vec2::new(86.0, 100.0)) < 1e-3,
            "dt {dt}: {before:?}"
        );
        assert_eq!(after.len(), 1, "dt {dt}: {after:?}");
        assert_eq!(
            (after[0].0, after[0].2),
            ("expired", Some((false, -left, 1))),
            "dt {dt}: at the tile, its bounce left"
        );
        assert!(
            after[0].1.distance(Vec2::new(-100.0, 100.0)) < 1e-3,
            "dt {dt}: {after:?}"
        );
        let (loaded_before, loaded_after) = bolt_run(dt, true);
        assert_eq!(loaded_before, before, "dt {dt}");
        assert_eq!(loaded_after.len(), after.len(), "dt {dt}: {loaded_after:?}");
        for (loaded, kept) in loaded_after.iter().zip(&after) {
            assert!(
                loaded.0 == kept.0 && loaded.2 == kept.2 && loaded.1.distance(kept.1) < 1e-3,
                "dt {dt}: {loaded:?} against {kept:?}"
            );
        }
    }
}
