//! The platformer fixture: a hero on a kinematic body under the move
//! actions and a jump slot, a ground, a door zone, a spark and a lift; M64's
//! conveyor, bobbing lift, acrobat, firebar and firefly; M65's fireballs,
//! bricks, climber, spiker and coin; M67's regen and haste on the runner;
//! M76's bolt off armour, saved mid-flight; 7.10a's squire with its scoped
//! lines, an inspired effect and a difficulty world source, and a knight
//! that halves blasts, floors its cold factor and crits from its stats;
//! 7.10m's rambler up a 2:1 ramp, down a steep one and through a ledge, and
//! its porter, whose body's mass a rolled ball pushes, carried up to a
//! ceiling, the hero pushed out of a sliding platform and crushed under a
//! falling deck, and a wisp steering its mover along a brick; 7.10b's
//! lamp, which burns its oil and goes out; its chemistry's kindling, lit
//! by a torch's hit, a clay pot cracked by a hard landing, a floor of
//! glass a pot cracks and planks a flare scorches for a while, a crate
//! fire chars, and a powder keg whose blast chars the crate beside it;
//! 7.10p's balls whose hard impacts spark under a rule's amount and
//! interval, charges whose blasts shake by their distance from a listener
//! following the hero, a crate whose break a recipe system plays as the
//! `crate_break` world cue, a turret under rapid fire whose flash waits
//! its interval, a blob whose frosting shows its band's overlay, and a keg
//! whose sparking look emits at its sheet's `fuse` point frame by frame,
//! and a leaper whose jumps and landing cue dust at its feet; 7.10g's
//! lander, whose landing plays its land clip once over its run, and its
//! swordsman, whose swing mirrors to the side it turns to; a doomed runner
//! its death policy stops and locks, a warded knight a kill passes, and a
//! hero and a crate past the world's bottom.

use glam::Vec2;
use tungsten_core::physics::Velocity;
use tungsten_core::{AnimationState, PluginSet, Transform, World};

use super::{DTS, body_at, drawn_at, frames_for, motion_reversed, point_of, recipe, spawn, tagged};
use crate::kit::{
    CharacterAnimator, Children, Counters, DamagePart, DamageRequest, EffectRequest, Effects,
    Facing, Flags, Health, KitPlugins, Line, LinearMotion, Modifier, PathFollowerPlugin,
    PlatformerController, PlatformerControllerPlugin, ProjectileExpired, Read, Stats, TagFilter,
    TagSet, Tags, WorldSource, WorldSources, ZoneEntered, damage, knock_back,
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

// 7.10a: scoped stat lines, context reads and world sources.

/// `entity`'s `damage` read in `context`.
fn damage_in(
    harness: &super::Harness,
    entity: tungsten_core::Entity,
    context: &[&str],
) -> Option<f32> {
    let names: Vec<String> = context.iter().map(|name| (*name).to_string()).collect();
    let sets = [TagSet::Names(&names)];
    harness
        .world()
        .get::<Stats>(entity)
        .unwrap()
        .read(Stats::DAMAGE, &Read::new(&sets))
}

#[test]
fn a_squires_own_line_and_an_inspired_effect_read_in_and_out_of_context() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let squire = spawn(&mut harness, "squire", [2000.0, -2000.0]);
        harness.step(1);
        let get = |harness: &super::Harness| {
            harness
                .world()
                .get::<Stats>(squire)
                .unwrap()
                .get(Stats::DAMAGE)
        };
        assert_eq!(get(&harness), Some(2.0), "dt {dt}");
        assert_eq!(damage_in(&harness, squire, &["damage.fire"]), Some(3.0));
        assert_eq!(damage_in(&harness, squire, &["skill.spell"]), Some(2.0));

        assert!(EffectRequest::new(squire, DataRef::by_id("inspired")).queue(harness.world_mut()));
        harness.step(1);
        assert_eq!(
            damage_in(&harness, squire, &["skill.spell"]),
            Some(4.0),
            "dt {dt}"
        );
        assert_eq!(
            damage_in(&harness, squire, &["skill.spell", "damage.fire"]),
            Some(6.0)
        );
        assert_eq!(
            get(&harness),
            Some(2.0),
            "dt {dt}: never the entity-level read"
        );
        harness.step(frames_for(2.1, dt));
        assert_eq!(
            damage_in(&harness, squire, &["skill.spell"]),
            Some(2.0),
            "dt {dt}: it left"
        );
    }
}

#[test]
fn a_difficulty_source_reaches_the_living_a_spawn_and_leaves_when_removed() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let squire = spawn(&mut harness, "squire", [2000.0, -2000.0]);
        let runner = spawn(&mut harness, "runner", [2400.0, -2000.0]);
        harness.step(1);
        let max =
            |harness: &super::Harness, entity| harness.world().get::<Health>(entity).unwrap().max();
        let hard = WorldSource::new(TagFilter::new().all(["team.enemy"]))
            .with_line(Stats::MAX_HEALTH, Line::new(Modifier::Mul(1.5)))
            .with_line(Stats::DAMAGE, Line::new(Modifier::Mul(1.5)));
        WorldSources::set(harness.world_mut(), "difficulty", hard);
        let runner_speed = harness
            .world()
            .get::<Stats>(runner)
            .and_then(|stats| stats.get(Stats::MOVE_SPEED));
        harness.step(1);
        assert_eq!(
            max(&harness, squire),
            6.0,
            "dt {dt}: the living, at the next step"
        );
        assert_eq!(damage_in(&harness, squire, &["damage.fire"]), Some(4.5));
        assert_eq!(
            harness
                .world()
                .get::<Stats>(runner)
                .and_then(|stats| stats.get(Stats::MOVE_SPEED)),
            runner_speed,
            "dt {dt}: a player is no enemy"
        );

        let late = spawn(&mut harness, "squire", [2800.0, -2000.0]);
        harness.step(1);
        let health = harness.world().get::<Health>(late).unwrap();
        assert_eq!(
            (health.current(), health.max()),
            (6.0, 6.0),
            "dt {dt}: in its first step"
        );

        assert!(WorldSources::remove(harness.world_mut(), "difficulty"));
        harness.step(1);
        assert_eq!(max(&harness, squire), 4.0, "dt {dt}: it left");
        assert_eq!(max(&harness, late), 4.0);
    }
}

#[test]
fn a_knight_halves_blasts_by_damage_taken_and_its_cold_factor_stops_at_its_floor() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let knight = spawn(&mut harness, "knight", [2000.0, -2000.0]);
        harness.step(1);
        let health =
            |harness: &super::Harness| harness.world().get::<Health>(knight).unwrap().current();
        let hit = |kind: Option<&str>| match kind {
            Some(kind) => DamageRequest::from_parts(
                knight,
                [DamagePart::new(8.0).with_tags(Tags::new([kind]))],
            ),
            None => DamageRequest::new(knight, 8.0),
        };
        hit(None)
            .with_context(Tags::new(["hit.blast"]))
            .apply(harness.world_mut());
        assert_eq!(health(&harness), 36.0, "dt {dt}: a blast, halved");
        hit(None).apply(harness.world_mut());
        assert_eq!(health(&harness), 28.0, "dt {dt}: a plain hit, whole");
        hit(Some("damage.cold")).apply(harness.world_mut());
        assert_eq!(health(&harness), 24.0, "dt {dt}: cold at its factor");
        assert!(EffectRequest::new(knight, DataRef::by_id("frostbite")).queue(harness.world_mut()));
        harness.step(1);
        hit(Some("damage.cold")).apply(harness.world_mut());
        assert_eq!(
            health(&harness),
            21.0,
            "dt {dt}: a quarter of 0.5 stops at its 0.375 floor"
        );
        hit(Some("damage.cold"))
            .with_context(Tags::new(["hit.blast"]))
            .apply(harness.world_mut());
        assert_eq!(health(&harness), 19.5, "dt {dt}: a cold blast, both");
    }
}

#[test]
fn a_knights_hits_crit_from_its_stats_at_the_resolve_or_its_builder() {
    use crate::kit::KitRng;
    // Pinned: the first draw crits, the second does not.
    let seed = (0..10_000)
        .find(|&seed| {
            let mut rng = KitRng::seeded(seed);
            rng.unit() < 0.5 && rng.unit() >= 0.5
        })
        .unwrap();
    assert_eq!(seed, 2);
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        harness.world_mut().insert_resource(KitRng::seeded(seed));
        let knight = spawn(&mut harness, "knight", [2000.0, -2000.0]);
        let target = spawn(&mut harness, "knight", [2400.0, -2000.0]);
        harness.step(1);
        let health =
            |harness: &super::Harness| harness.world().get::<Health>(target).unwrap().current();
        assert!(
            DamageRequest::new(target, 4.0)
                .with_source(knight)
                .queue(harness.world_mut())
        );
        harness.step(1);
        assert_eq!(
            health(&harness),
            32.0,
            "dt {dt}: a crit at the resolve, 4 × 2"
        );
        let built = DamageRequest::new(target, 4.0)
            .with_source(knight)
            .roll_crit(harness.world_mut());
        assert!(!built.is_crit(), "dt {dt}: the builder's draw, none");
        built.apply(harness.world_mut());
        assert_eq!(health(&harness), 28.0, "dt {dt}");
        assert_eq!(harness.world().resource::<KitRng>().draws(), 2, "dt {dt}");
    }
}

#[test]
fn a_knights_skill_arms_its_hitbox_with_the_snapshot_for_that_swing() {
    use crate::kit::{
        Abilities, Ability, DamagedPart, KitRng, Payload, Skill, SkillLine, Used, use_ability,
    };
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        harness
            .world_mut()
            .insert_resource(crate::kit::KitExplain::new());
        let knight = spawn(&mut harness, "knight", [2000.0, -2000.0]);
        let squire = spawn(&mut harness, "squire", [2010.0, -2000.0]);
        // The swing: its hitbox armed for 0.1 s with the skill's physical 3
        // to 3, and the knight's edge gaining a quarter of it as cold.
        let swing = Ability::new()
            .with_payload(Payload::Hitbox(0.1))
            .with_skill(Skill::new().with_damage("damage.physical", 3.0, 3.0));
        let mut abilities = Abilities::new().with(DataRef::inline(swing));
        abilities.add_lines(
            "item:rime_edge",
            [SkillLine::extra("damage.physical", "damage.cold", 0.25)],
        );
        harness.world_mut().insert(knight, abilities);
        harness.step(1);
        // Its crit (Step 3's knight crits half the time) draws after the
        // range: a seed whose second draw misses.
        let seed = (0..10_000)
            .find(|&seed| {
                let mut rng = KitRng::seeded(seed);
                rng.unit();
                rng.unit() >= 0.5
            })
            .unwrap();
        harness.world_mut().insert_resource(KitRng::seeded(seed));
        assert_eq!(
            use_ability(harness.world_mut(), knight, "#0"),
            Used::Used,
            "dt {dt}"
        );
        let (mut hits, mut parts) = (Vec::new(), Vec::new());
        for _ in 0..frames_for(0.2, dt) {
            harness.step(1);
            hits.extend(
                harness
                    .events::<crate::kit::Damaged>()
                    .filter(|hit| hit.entity == squire)
                    .map(|hit| (hit.amount, hit.crit, hit.source)),
            );
            parts.extend(
                harness
                    .events::<DamagedPart>()
                    .filter(|part| part.entity == squire)
                    .map(|part| part.amount),
            );
        }
        assert_eq!(
            hits,
            [(3.75, false, Some(knight))],
            "dt {dt}: once, the snapshot's 3 and its extra, not the hitbox's 1"
        );
        assert_eq!(
            parts,
            [3.0, 0.75],
            "dt {dt}: the physical part, then a quarter of it as extra cold"
        );
        // Its why-record: the extra cold part from the knight's edge, the
        // crit draw that missed.
        let hit = harness
            .world()
            .resource::<crate::kit::KitExplain>()
            .last_hit()
            .unwrap();
        assert_eq!((hit.source, hit.amount), (Some(knight), 3.75), "dt {dt}");
        assert_eq!(
            hit.parts[1].origins[0].steps,
            [crate::kit::OriginStep::Extra {
                share: 0.25,
                to: "damage.cold".to_string(),
                source: "item:rime_edge".to_string(),
            }],
            "dt {dt}"
        );
        assert!(
            matches!(hit.crit, crate::kit::CritRecord::Drawn { chance, roll, .. } if chance == 0.5 && roll >= 0.5),
            "dt {dt}: {:?}",
            hit.crit
        );
    }
}

#[test]
fn a_numbered_targets_numbers_merge_tick_and_crit_by_its_cue_rule() {
    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];
    const GOLD: [u8; 4] = [255, 218, 0, 255];
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // `numbers.json`: merge over 0.25 s, ticks at 10, crits gold at 24,
        // over the genre's `numbers` damage types.
        let target = harness
            .world_mut()
            .spawn_with((Health::new(100.0), Tags::new(["numbered"])));
        harness.step(1);
        let part = |amount: f32, kind: &str| DamagePart::new(amount).with_tags(Tags::new([kind]));
        let mut seen: Vec<(String, f32, [u8; 4])> = Vec::new();
        let mut note = |harness: &super::Harness| {
            for run in &harness.draw().unwrap().world_labels {
                // Its colour as floated: the fade leaves the RGB.
                let [r, g, b, _] = run.color;
                let label = (run.content.clone(), run.font_size, [r, g, b, 255]);
                if !seen.contains(&label) {
                    seen.push(label);
                }
            }
        };
        // A cold bolt, a fire burn's ticks, then a crit; the bolt's second
        // half within the window.
        let world = harness.world_mut();
        DamageRequest::from_parts(target, [part(6.0, "elemental.cold")]).queue(world);
        DamageRequest::from_parts(target, [part(0.5, "elemental.fire")])
            .with_periodic(true)
            .queue(world);
        harness.step(1);
        note(&harness);
        let world = harness.world_mut();
        DamageRequest::from_parts(target, [part(2.5, "elemental.cold")]).queue(world);
        DamageRequest::from_parts(target, [part(0.5, "elemental.fire")])
            .with_periodic(true)
            .queue(world);
        DamageRequest::from_parts(target, [part(7.0, "elemental.fire")])
            .with_crit(true)
            .queue(world);
        harness.step(1);
        note(&harness);
        for _ in 0..frames_for(0.5, dt) {
            harness.step(1);
            note(&harness);
        }
        let label = |text: &str, size: f32, color: [u8; 4]| (text.to_string(), size, color);
        assert_eq!(
            seen,
            [
                label("6", 16.0, BLUE),
                label("7", 24.0, GOLD),
                label("2.5", 16.0, BLUE),
                label("1", 10.0, RED),
            ],
            "dt {dt}: the bolt's first at once, the crit in gold, then the bolt's follower and the burn's ticks as one"
        );
    }
}

/// The hills: 16 px cells, the floor's top at y 240; a 2:1 ramp from x 64
/// up to a plateau at y 208 from x 128 to 224, a 45° slope down from it to
/// the floor at x 256, and a one-way ledge at y 160 from x 384 to 496.
fn hills(dt: f32) -> super::Harness {
    use tungsten_core::{
        LayerKind, Slope, TileProps, TilemapData, TilemapInstance, TilemapRegistry,
    };

    let mut harness = recipe(GENRE, KitPlugins::set(), dt);
    let rows = [
        (10, "........................=======........."),
        (13, "......ei######b........................."),
        (14, "....ei#########b........................"),
        (15, "########################################"),
    ];
    let mut map = TilemapData::new(16, 16, 40, 16);
    let solid = map.add_tile("solid");
    let slope = |map: &mut TilemapData, name: &str| {
        let tile = map.add_tile(name);
        let mut props = TileProps::default();
        props.slope = Some(Slope::parse(name).unwrap());
        map.set_tile_props(tile, props);
        tile
    };
    let (low, high, down) = (
        slope(&mut map, "2:1 low br"),
        slope(&mut map, "2:1 high br"),
        slope(&mut map, "45 bl"),
    );
    let ledge = map.add_tile("ledge");
    let mut props = TileProps::default();
    props.one_way = true;
    map.set_tile_props(ledge, props);
    let layer = map.add_layer("solid", LayerKind::Collision);
    for (row, letters) in rows {
        for (col, letter) in letters.chars().enumerate() {
            let tile = match letter {
                '#' => solid,
                'e' => low,
                'i' => high,
                'b' => down,
                '=' => ledge,
                _ => continue,
            };
            map.set_tile(layer, col as u32, row, tile);
        }
    }
    let world = harness.world_mut();
    world
        .resource_mut::<TilemapRegistry>()
        .insert("hills".into(), map);
    let instance = world.spawn();
    world.insert(instance, TilemapInstance::new("hills", Vec2::ZERO));
    harness
}

/// The rambler's feet and whether it stands.
fn rambler_feet(harness: &super::Harness, rambler: tungsten_core::Entity) -> (Vec2, bool) {
    let at = body_at(harness, rambler) + Vec2::new(0.0, 12.0);
    (at, grounded(harness, rambler))
}

#[test]
fn a_rambler_walks_up_a_ramp_slides_down_a_steep_one_and_drops_through_a_ledge() {
    for dt in DTS {
        // Up the 2:1 ramp, within its 50° slope limit and its 35° slide
        // limit: grounded each frame, onto the plateau.
        let mut harness = hills(dt);
        let rambler = spawn(&mut harness, "rambler", [24.0, 228.0]);
        harness.step(2);
        harness.press_action("move_right");
        for frame in 0..frames_for(1.0, dt) {
            harness.step(1);
            let (feet, standing) = rambler_feet(&harness, rambler);
            assert!(standing, "dt {dt}, frame {frame}: off the ramp at {feet}");
        }
        harness.release_action("move_right");
        let (feet, _) = rambler_feet(&harness, rambler);
        assert!(
            (feet.y - 208.0).abs() < 0.05 && (128.0..224.0).contains(&feet.x),
            "dt {dt}: on the plateau at {feet}"
        );

        // Set on the 45° slope, steeper than its slide limit: it slides down
        // it to the floor and stops there.
        let mut harness = hills(dt);
        let rambler = spawn(&mut harness, "rambler", [244.0, 207.5]);
        harness.step(frames_for(1.0, dt));
        let (feet, standing) = rambler_feet(&harness, rambler);
        assert!(
            standing && (feet.y - 240.0).abs() < 0.05 && feet.x > 256.0,
            "dt {dt}: slid to the floor, at {feet}"
        );
        harness.step(frames_for(0.25, dt));
        assert_eq!(rambler_feet(&harness, rambler).0, feet, "dt {dt}: it stops");

        // On the one-way ledge, down with a jump drops it through: it never
        // stands on the ledge again and lands on the floor.
        let mut harness = hills(dt);
        let rambler = spawn(&mut harness, "rambler", [440.0, 148.0]);
        harness.step(3);
        assert!(rambler_feet(&harness, rambler).1, "dt {dt}: on the ledge");
        harness.press_action("move_down");
        harness.press_action("jump");
        harness.step(1);
        harness.release_action("jump");
        harness.release_action("move_down");
        let mut landed = None;
        for frame in 0..frames_for(1.0, dt) {
            harness.step(1);
            let (feet, standing) = rambler_feet(&harness, rambler);
            if standing {
                landed = Some((frame, feet));
                break;
            }
            assert!(feet.y >= 160.0, "dt {dt}, frame {frame}: rose to {feet}");
        }
        let (_, feet) = landed.unwrap_or_else(|| panic!("dt {dt}: it never landed"));
        assert!(
            (feet.y - 240.0).abs() < 0.05,
            "dt {dt}: on the floor at {feet}"
        );
    }
}

#[test]
fn a_porter_is_knocked_by_a_ball_and_carried_up_to_a_ceiling() {
    for dt in DTS {
        // A ball of mass 10 rolled at 300 px/s into the porter, whose body
        // weighs 60: one push of 3,000 / 70 px/s, which moves it.
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let porter = spawn(&mut harness, "porter", [0.0, 270.0]);
        spawn(&mut harness, "roller", [-100.0, 282.0]);
        let mut pushes = Vec::new();
        for _ in 0..frames_for(1.0, dt) {
            harness.step(1);
            let pushed = harness
                .world()
                .get::<PlatformerController>(porter)
                .unwrap()
                .pushed();
            if pushed != Vec2::ZERO {
                pushes.push(pushed);
            }
        }
        let expected = 3_000.0 / 70.0;
        assert_eq!(pushes.len(), 1, "dt {dt}: {pushes:?}");
        assert!(
            (pushes[0].x - expected).abs() < 0.01 * expected,
            "dt {dt}: {pushes:?}"
        );
        assert!(body_at(&harness, porter).x > 0.5, "dt {dt}: it moved");

        // On a deck rising at 60 px/s under a ledge whose bottom is 12 px over
        // its head: carried up until its head is the skin under the ledge.
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "riser", [200.0, 250.0]);
        spawn(&mut harness, "ledge", [200.0, 180.0]);
        let porter = spawn(&mut harness, "porter", [200.0, 222.0]);
        harness.step(frames_for(0.5, dt));
        let head = body_at(&harness, porter).y - 20.0;
        assert!(
            (head - 190.01).abs() < 1e-3,
            "dt {dt}: its head at {head}, the ledge's bottom at 190"
        );
    }
}

#[test]
fn the_hero_is_pushed_out_of_a_sliding_platform_and_crushed_under_a_falling_deck() {
    for dt in DTS {
        // A platform sliding at 120 px/s into the hero's side pushes it
        // along: never deeper in it than the platform's move in a step.
        let (mut harness, hero) = hero_on_ground(dt, KitPlugins::set());
        let slider = spawn(&mut harness, "slider", [40.0, 280.0]);
        let mut deepest = 0.0_f32;
        for _ in 0..frames_for(0.75, dt) {
            harness.step(1);
            let gap = (body_at(&harness, hero) - body_at(&harness, slider)).abs();
            let overlap = Vec2::new(20.0, 30.0) - gap;
            deepest = deepest.max(overlap.min_element());
        }
        assert!(deepest <= 120.0 * dt + 1e-3, "dt {dt}: {deepest} px deep");
        assert!(body_at(&harness, hero).x < -20.0, "dt {dt}: pushed along");

        // A deck falling at 120 px/s onto the hero where it stands: the
        // ground cuts its push short, so it is crushed.
        let (mut harness, hero) = hero_on_ground(dt, KitPlugins::set());
        spawn(&mut harness, "crusher", [0.0, 210.0]);
        let mut crushed = false;
        for _ in 0..frames_for(0.75, dt) {
            harness.step(1);
            crushed |= harness
                .world()
                .get::<PlatformerController>(hero)
                .unwrap()
                .crushed();
        }
        assert!(crushed, "dt {dt}: crushed under the deck");
    }
}

#[test]
fn a_wisp_steers_its_mover_along_a_brick_and_lands_on_a_point() {
    for dt in DTS {
        // Arriving at a point behind a brick whose left face is at x 290: the
        // mover's casts stop the wisp at the brick, at the point's height.
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "brick", [300.0, 0.0]);
        let wisp = spawn(&mut harness, "wisp", [200.0, 0.0]);
        harness.step(frames_for(3.0, dt));
        let at = body_at(&harness, wisp);
        assert!(
            at.x <= 282.0 + 1e-3 && at.x > 281.9,
            "dt {dt}: at the brick: {at}"
        );
        assert!(at.y > 10.0 && at.y <= 20.0, "dt {dt}: slid along it: {at}");

        // A point the game sets in the open: it lands on it.
        let point = Vec2::new(150.0, -40.0);
        harness
            .world_mut()
            .get_mut::<crate::kit::Steering>(wisp)
            .unwrap()
            .set_target(crate::kit::Target::point(point));
        harness.step(frames_for(6.0, dt));
        let at = body_at(&harness, wisp);
        assert!(at.distance(point) < 1e-3, "dt {dt}: on the point: {at}");
    }
}

#[test]
fn a_lamp_burns_its_oil_and_goes_out() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let lamp = spawn(&mut harness, "lamp", [100.0, 100.0]);
        // Its listed `lit` applies at its first step and burns 1 oil a
        // second from the next, 3 s of it.
        harness.step(1);
        let oil = |harness: &super::Harness| {
            harness
                .world()
                .get::<Stats>(lamp)
                .and_then(|stats| stats.base("oil"))
                .unwrap()
        };
        let lit = |harness: &super::Harness| {
            harness
                .world()
                .get::<Tags>(lamp)
                .is_some_and(|tags| tags.has("status.lit"))
        };
        assert!(lit(&harness), "dt {dt}");
        let mut frames = 0;
        while lit(&harness) {
            harness.step(1);
            frames += 1;
            assert!(oil(&harness) < 3.0, "dt {dt}: burning");
            assert!(frames < frames_for(5.0, dt), "dt {dt}: still lit");
        }
        let burned = frames as f32 * dt;
        assert!(
            (burned - 3.0).abs() <= 2.0 * dt + 1e-3,
            "dt {dt}: {burned} s"
        );
        assert_eq!(oil(&harness), 0.0, "dt {dt}: dry");
        let effects = harness.world().get::<Effects>(lamp).unwrap();
        assert!(!effects.has("lit"), "dt {dt}: out");
    }
}

#[test]
fn a_torch_ignites_by_its_hit() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let kindling = spawn(&mut harness, "kindling", [100.0, 100.0]);
        let brand = harness.world_mut().spawn();
        harness.step(1);
        // A fire hit of 4 from the brand: the kindling rule applies
        // `smoulder` at half the hit a second, credited to the brand.
        DamageRequest::from_parts(
            kindling,
            [DamagePart::new(4.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .with_source(brand)
        .apply(harness.world_mut());
        harness.step(1);
        let world = harness.world();
        let instance = world
            .get::<Effects>(kindling)
            .unwrap()
            .instances()
            .next()
            .unwrap()
            .clone();
        assert_eq!(instance.rate(), Some(2.0), "dt {dt}");
        assert_eq!(instance.source(), Some(brand), "dt {dt}");
        assert_eq!(world.get::<Health>(kindling).unwrap().current(), 6.0);
        // Its two seconds: a tick of 1 every half second.
        harness.step(frames_for(2.0, dt) + 2);
        let world = harness.world();
        assert_eq!(
            world.get::<Health>(kindling).unwrap().current(),
            2.0,
            "dt {dt}"
        );
        assert!(
            !world.get::<Effects>(kindling).unwrap().has("smoulder"),
            "dt {dt}"
        );
    }
}

#[test]
fn a_clay_pot_cracks_on_a_hard_landing() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ledge", [200.0, 300.0]);
        spawn(&mut harness, "ledge", [400.0, 300.0]);
        let hard = spawn(&mut harness, "clay_pot", [200.0, 270.0]);
        let soft = spawn(&mut harness, "clay_pot", [400.0, 270.0]);
        // Dropped onto the ledges at 600 and 150 px/s: the rule cracks a
        // pot whose impact closes at 300 px/s or more.
        for (pot, speed) in [(hard, 600.0), (soft, 150.0)] {
            harness.world_mut().get_mut::<Velocity>(pot).unwrap().0 = Vec2::new(0.0, speed);
        }
        harness.step(frames_for(0.5, dt));
        let cracked = |harness: &crate::testing::Harness, pot| {
            harness
                .world()
                .get::<Tags>(pot)
                .is_some_and(|tags| tags.has("status.cracked"))
        };
        assert!(cracked(&harness, hard), "dt {dt}: the hard landing");
        assert!(!cracked(&harness, soft), "dt {dt}: the soft one");
    }
}

#[test]
fn a_glass_floor_cracks_and_a_scorched_cell_reverts() {
    use tungsten_core::{LayerKind, TileProps, TilemapData, TilemapInstance, TilemapRegistry};

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // A floor of 16 px cells from x 100 at y 300: glass in columns 0 to
        // 5, planks in 6 to 11, built in code with their materials.
        let mut map = TilemapData::new(16, 16, 12, 1);
        let tile = |map: &mut TilemapData, sprite: &str, material: Option<&str>| {
            let tile = map.add_tile(sprite);
            if let Some(material) = material {
                let mut props = TileProps::default();
                props.material = Some(material.into());
                map.set_tile_props(tile, props);
            }
            tile
        };
        let glass = tile(&mut map, "glass", Some("glass"));
        tile(&mut map, "glass_cracked", None);
        let planks = tile(&mut map, "planks", Some("planks"));
        tile(&mut map, "soot", None);
        let layer = map.add_layer("floor", LayerKind::Collision);
        for col in 0..12 {
            map.set_tile(layer, col, 0, if col < 6 { glass } else { planks });
        }
        let world = harness.world_mut();
        world
            .resource_mut::<TilemapRegistry>()
            .insert("floor".into(), map);
        world.spawn_with((TilemapInstance::new("floor", Vec2::new(100.0, 300.0)),));
        let sprite = |harness: &crate::testing::Harness, col: u32| {
            let data = harness
                .world()
                .resource::<TilemapRegistry>()
                .get("floor")
                .unwrap();
            data.tileset[data.tile_at(0, col, 0).unwrap() as usize].clone()
        };
        // A pot dropped at 600 px/s onto column 2: `glass_cracks` sets the
        // glass it lands on to `glass_cracked`.
        let pot = spawn(&mut harness, "clay_pot", [140.0, 280.0]);
        harness.world_mut().get_mut::<Velocity>(pot).unwrap().0 = Vec2::new(0.0, 600.0);
        // A flare 18 px above column 8's centre, sending `heat` within 24 px
        // for a second: `planks_scorch` applies `scorched`, whose end sets
        // `soot` for 2 s.
        let brand = harness
            .world_mut()
            .spawn_with((Transform::from_position(Vec2::new(236.0, 290.0)),));
        EffectRequest::new(brand, DataRef::by_id("flare")).apply(harness.world_mut());
        harness.step(frames_for(0.5, dt));
        assert_eq!(sprite(&harness, 2), "glass_cracked", "dt {dt}");
        assert_eq!(sprite(&harness, 5), "glass", "dt {dt}: away from the pot");
        assert_eq!(sprite(&harness, 8), "planks", "dt {dt}: still scorching");
        harness.step(frames_for(2.0, dt));
        assert_eq!(sprite(&harness, 8), "soot", "dt {dt}");
        assert_eq!(
            (sprite(&harness, 7), sprite(&harness, 9)),
            ("planks".into(), "planks".into())
        );
        harness.step(frames_for(2.0, dt));
        assert_eq!(sprite(&harness, 8), "planks", "dt {dt}: reverted");
        assert_eq!(
            sprite(&harness, 2),
            "glass_cracked",
            "dt {dt}: a crack stays"
        );
    }
}

#[test]
fn a_crate_chars_in_fire() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let burnt = spawn(&mut harness, "wood_crate", [100.0, 100.0]);
        let singed = spawn(&mut harness, "wood_crate", [300.0, 100.0]);
        harness.step(1);
        // Fire of 6 on one and 3 on the other: `chars_crates` (min 5) makes
        // the first a charred crate at its point.
        for (target, amount) in [(burnt, 6.0), (singed, 3.0)] {
            DamageRequest::from_parts(
                target,
                [DamagePart::new(amount).with_tags(Tags::new(["elemental.fire"]))],
            )
            .apply(harness.world_mut());
        }
        harness.step(1);
        let world = harness.world();
        assert!(!world.is_alive(burnt), "dt {dt}");
        assert!(world.is_alive(singed), "dt {dt}: under the rule's min");
        let charred: Vec<Vec2> = world
            .query::<(&tungsten_core::Tag, &Transform)>()
            .filter(|(tag, _)| tag.name == "crate_charred")
            .map(|(_, transform)| transform.position)
            .collect();
        assert_eq!(charred, [Vec2::new(100.0, 100.0)], "dt {dt}");
    }
}

#[test]
fn a_powder_kegs_blast_chars_a_crate() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let keg = spawn(&mut harness, "powder_keg", [200.0, 100.0]);
        let crate_ = spawn(&mut harness, "wood_crate", [230.0, 100.0]);
        let kindling = spawn(&mut harness, "kindling", [400.0, 100.0]);
        harness.step(1);
        // Fire on the keg: `kegs_blow` blasts fire 10 within 80 px, so the
        // crate 30 px off takes 6.25, which `chars_crates` (min 5) chars.
        DamageRequest::from_parts(
            keg,
            [DamagePart::new(1.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .apply(harness.world_mut());
        harness.step(4);
        let world = harness.world();
        assert!(!world.is_alive(crate_), "dt {dt}: charred");
        let charred: Vec<Vec2> = world
            .query::<(&tungsten_core::Tag, &Transform)>()
            .filter(|(tag, _)| tag.name == "crate_charred")
            .map(|(_, transform)| transform.position)
            .collect();
        assert_eq!(charred, [Vec2::new(230.0, 100.0)], "dt {dt}");
        assert_eq!(
            world.get::<Health>(kindling).unwrap().current(),
            10.0,
            "dt {dt}: out of reach"
        );
    }
}

// 7.10p: cue rule filters and limits.

/// A contact's begin as the cue event `impact`: the pushed body at its
/// point, the closing speed its amount.
#[allow(clippy::unnecessary_wraps)]
fn impact(event: &tungsten_core::physics::ContactBegan) -> Option<crate::kit::CueEvent> {
    Some(crate::kit::CueEvent::of(event.a).with_amount(event.speed))
}

/// A recipe harness whose game registers `impact` and stages the impact
/// rule, `staged/impacts.json`, outside the shared manifest, which names no
/// game event.
fn impact_recipe(dt: f32) -> super::Harness {
    use crate::kit::CuesOnEvent;
    use tungsten_core::StagedLoad;

    let plugins = KitPlugins::set().with(CuesOnEvent::<tungsten_core::physics::ContactBegan>::new(
        "impact", impact,
    ));
    let mut harness = recipe(GENRE, plugins, dt);
    let mut staged = StagedLoad::new();
    staged
        .add_data(
            harness.world(),
            "cue_rules",
            "impacts",
            super::fixture_dir(GENRE).join("staged/impacts.json"),
        )
        .unwrap();
    staged.commit(harness.world_mut()).unwrap();
    harness
}

#[test]
fn a_balls_hard_impact_sparks_once_under_its_rules_amount_and_interval() {
    use crate::kit::{KitTrace, TraceKind};
    use tungsten_core::{LayerKind, TilemapData, TilemapInstance, TilemapRegistry};

    for dt in DTS {
        let mut harness = impact_recipe(dt);
        // A floor of 16 px cells from x 100 at y 300.
        let mut map = TilemapData::new(16, 16, 20, 1);
        let stone = map.add_tile("stone");
        let layer = map.add_layer("floor", LayerKind::Collision);
        for col in 0..20 {
            map.set_tile(layer, col, 0, stone);
        }
        let world = harness.world_mut();
        world
            .resource_mut::<TilemapRegistry>()
            .insert("floor".into(), map);
        world.spawn_with((TilemapInstance::new("floor", Vec2::new(100.0, 300.0)),));
        // Dropped at 600 and 150 px/s: the rule sparks an impact of 300 px/s
        // or more, once a quarter second per ball.
        let hard = spawn(&mut harness, "bouncer", [140.0, 280.0]);
        let soft = spawn(&mut harness, "bouncer", [260.0, 280.0]);
        for (ball, speed) in [(hard, 600.0), (soft, 150.0)] {
            harness.world_mut().get_mut::<Velocity>(ball).unwrap().0 = Vec2::new(0.0, speed);
        }
        harness.step(frames_for(1.0, dt));
        let trace = harness.world().resource::<KitTrace>();
        let sparks = |ball| {
            trace
                .about(ball)
                .filter(|entry| matches!(&entry.kind, TraceKind::Spawn { prefab, .. } if prefab == "spark"))
                .count()
        };
        let held = trace
            .about(hard)
            .filter(|entry| matches!(entry.kind, TraceKind::Skipped { action: "all", .. }))
            .count();
        assert_eq!(sparks(hard), 1, "dt {dt}: the hard landing, once");
        assert!(
            held >= 1,
            "dt {dt}: the landing's other cells' begins, held by the interval"
        );
        assert_eq!(sparks(soft), 0, "dt {dt}: the soft one under its amount");
    }
}

// 7.10p: a shake's falloff.

#[test]
fn a_blasts_shake_falls_off_with_its_distance_from_the_hero() {
    use crate::kit::CueListener;
    use tungsten_core::{EventQueue, ShakeEvent};

    for dt in DTS {
        let (mut harness, hero) = hero_on_ground(dt, KitPlugins::set());
        harness
            .world_mut()
            .insert_resource(CueListener::new(100.0, 600.0).following(hero));
        // Charges at 25, 250 and 600 px from the hero: a shake whole within
        // 50 px, falling to none at 450.
        let mut shakes = Vec::new();
        for x in [25.0, 250.0, 600.0] {
            let charge = spawn(&mut harness, "charge", [x, 270.0]);
            harness.step(1);
            damage(harness.world_mut(), charge, 5.0);
            harness.step(1);
            let sent: Vec<f32> = harness
                .world()
                .resource::<EventQueue<ShakeEvent>>()
                .iter()
                .map(|shake| shake.trauma_add)
                .collect();
            shakes.push(sent);
        }
        assert_eq!(
            shakes,
            [vec![0.8], vec![0.4], vec![]],
            "dt {dt}: whole, half, none"
        );
    }
}

// 7.10p: named world cues.

/// A recipe's crates: each dead `material.crate` plays the `crate_break`
/// world cue at its point, its max health the amount, and goes.
struct CrateBreaks;

impl tungsten_core::Plugin for CrateBreaks {
    fn build(&self, schedule: &mut tungsten_core::Schedule, _world: &mut World) {
        use crate::kit::{Phase, WorldCue};
        use tungsten_core::{CommandBuffer, Entity, system};

        let breaks = system("recipe_crate_breaks", |world: &mut World| {
            let broken: Vec<(Entity, Vec2, f32)> = world
                .query::<(Entity, &Health, &Tags, &Transform)>()
                .filter(|(_, health, tags, _)| health.is_dead() && tags.has("material.crate"))
                .map(|(entity, health, _, transform)| (entity, transform.position, health.max()))
                .collect();
            for (entity, at, max) in broken {
                WorldCue::new("crate_break", at)
                    .with_amount(max)
                    .play(world);
                // Broken once: a frame's second step finds it no more.
                world.remove_component::<Health>(entity);
                world.resource_mut::<CommandBuffer>().despawn(entity);
            }
        });
        schedule.add(
            tungsten_core::Stage::FixedUpdate,
            Phase::React.join(breaks.before(crate::kit::CUES)),
        );
    }
}

#[test]
fn a_crates_break_plays_its_world_cue_at_its_point() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set().with(CrateBreaks), dt);
        let wood = spawn(&mut harness, "wood_crate", [120.0, 80.0]);
        harness.step(1);
        damage(harness.world_mut(), wood, 100.0);
        harness.step(1);
        assert!(!harness.world().is_alive(wood), "dt {dt}");
        let sparks: Vec<Vec2> = tagged(&harness, "spark")
            .into_iter()
            .map(|spark| drawn_at(&harness, spark))
            .collect();
        assert_eq!(sparks, [Vec2::new(120.0, 80.0)], "dt {dt}: at its point");
        let counters = harness.world().resource::<Counters>();
        assert_eq!(counters.get("crates_broken"), 1, "dt {dt}");
        harness.step(frames_for(1.0, dt));
        assert_eq!(
            harness.world().resource::<Counters>().get("crates_broken"),
            1,
            "dt {dt}: once"
        );
    }
}

// 7.10p: the hit flash's interval.

#[test]
fn a_turret_under_rapid_fire_flashes_once_an_interval() {
    use tungsten_core::{MaterialRegistry, Sprite};

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let turret = spawn(&mut harness, "turret", [0.0, 0.0]);
        harness.step(1);
        let flash = harness
            .world()
            .resource::<MaterialRegistry>()
            .get("damage_flash")
            .unwrap();
        // A blow every 0.1 s for 2 s.
        let every = frames_for(0.1, dt);
        let frames = frames_for(2.0, dt);
        let (mut starts, mut off, mut was) = (Vec::new(), 0, false);
        for frame in 0..frames {
            if frame % every == 0 {
                damage(harness.world_mut(), turret, 1.0);
            }
            harness.step(1);
            let on = harness.world().get::<Sprite>(turret).unwrap().material_id == Some(flash);
            if on && !was {
                starts.push(frame as f32 * dt);
            }
            off += u32::from(!on);
            was = on;
        }
        let expected = [0.0, 0.5, 1.0, 1.5];
        assert_eq!(starts.len(), 4, "dt {dt}: {starts:?}");
        assert!(
            starts
                .iter()
                .zip(expected)
                .all(|(at, want)| (at - want).abs() < 1e-3),
            "dt {dt}: {starts:?}"
        );
        assert!(off * 2 >= frames, "dt {dt}: off {off} of {frames} frames");
    }
}

// 7.10p: status looks.

#[test]
fn a_blobs_frosting_shows_its_bands_overlay_as_it_wears() {
    use tungsten_core::{Sprite, Visibility};

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let blob = super::register_sprite(&mut harness, "blob", 16, 16);
        for (id, size) in [
            ("frost_light_0", 8),
            ("frost_light_1", 8),
            ("frost_heavy_0", 12),
            ("frost_heavy_1", 12),
        ] {
            super::register_sprite(&mut harness, id, size, size);
        }
        let blob = harness.world_mut().spawn_with((
            Transform::from_position(Vec2::new(100.0, 100.0)),
            Sprite::new(blob),
            Visibility::default(),
        ));
        EffectRequest::new(blob, DataRef::by_id("frosting"))
            .with_amount(3.5)
            .apply(harness.world_mut());
        // The overlays drawn: each one's top-left and size.
        let overlays = |harness: &super::Harness| -> Vec<([f32; 2], [f32; 2])> {
            harness
                .draw()
                .unwrap()
                .sprites
                .iter()
                .flat_map(|batch| &batch.instances)
                .filter(|instance| instance.size != [16.0, 16.0])
                .map(|instance| (instance.position, instance.size))
                .collect()
        };
        harness.step(1);
        assert_eq!(
            overlays(&harness),
            [([102.0, 102.0], [12.0, 12.0])],
            "dt {dt}: band 2's heavy frost on its centre"
        );
        harness.step(frames_for(0.75, dt));
        assert_eq!(
            overlays(&harness),
            [([104.0, 104.0], [8.0, 8.0])],
            "dt {dt}: worn below 3, band 1's light frost"
        );
        harness.step(frames_for(3.0, dt));
        assert!(overlays(&harness).is_empty(), "dt {dt}: worn out");
    }
}

#[test]
fn a_kegs_sparks_start_at_its_fuse_point_frame_by_frame() {
    use std::collections::HashSet;
    use tungsten_core::{AnimationState, AssetRegistry, Particle, Sprite, Visibility};

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        harness
            .register_sprites(super::fixture_dir(GENRE).join("manifest.json"))
            .unwrap();
        let assets = harness.world().resource::<AssetRegistry>();
        let frames = [
            assets.sprite_id("keg_0").unwrap(),
            assets.sprite_id("keg_1").unwrap(),
        ];
        let fuse = |sprite| {
            if sprite == frames[0] {
                Vec2::new(4.0, 0.0)
            } else {
                Vec2::new(12.0, 1.0)
            }
        };
        let keg = harness.world_mut().spawn_with((
            Transform::from_position(Vec2::new(0.0, 100.0)),
            Sprite::new(frames[0]),
            AnimationState::new("keg_burn"),
            Visibility::default(),
        ));
        EffectRequest::new(keg, DataRef::by_id("sparking")).apply(harness.world_mut());
        let (mut seen, mut checked, mut changes) = (HashSet::new(), 0, 0);
        let mut shown = frames[0];
        for _ in 0..frames_for(1.0, dt) {
            // The keg moves at 600 px/s; the sparks emit before this frame's
            // animation, from the frame it shows now.
            let at = {
                let transform = harness.world_mut().get_mut::<Transform>(keg).unwrap();
                transform.position.x += 600.0 * dt;
                transform.position
            };
            let expected = at + fuse(shown);
            harness.step(1);
            let new: Vec<(tungsten_core::Entity, Vec2)> = harness
                .world()
                .query::<(tungsten_core::Entity, &Particle, &Transform)>()
                .filter(|(entity, ..)| !seen.contains(entity))
                .map(|(entity, _, transform)| (entity, transform.position))
                .collect();
            for (entity, start) in new {
                assert!(
                    start.distance(expected) < 1e-3,
                    "dt {dt}: a spark at {start}, the fuse at {expected}"
                );
                seen.insert(entity);
                checked += 1;
            }
            let now = harness.world().get::<Sprite>(keg).unwrap().asset_id;
            changes += u32::from(now != shown);
            shown = now;
        }
        assert!(checked >= 20, "dt {dt}: {checked} sparks");
        assert!(changes >= 5, "dt {dt}: {changes} frame changes");
    }
}

// 7.10p: the controller's cue events.

/// Where each live `dust` is.
fn dust(harness: &super::Harness) -> Vec<Vec2> {
    tagged(harness, "dust")
        .into_iter()
        .map(|entity| drawn_at(harness, entity))
        .collect()
}

#[test]
fn a_leapers_jumps_and_landing_cue_dust_at_its_feet() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let leaper = spawn(&mut harness, "leaper", [0.0, 270.0]);
        harness.step(2);
        assert!(dust(&harness).is_empty(), "dt {dt}: standing cues none");
        // A ground jump: dust where its feet stood, on the ground's top.
        harness.press_action("jump");
        harness.step(1);
        harness.release_action("jump");
        let puffs = dust(&harness);
        assert_eq!(puffs.len(), 1, "dt {dt}");
        assert!(
            puffs[0].distance(Vec2::new(0.0, 290.0)) < 0.5,
            "dt {dt}: {puffs:?}"
        );
        // An air jump, then the landing's dust once the first has gone.
        harness.step(frames_for(0.1, dt));
        harness.press_action("jump");
        harness.step(1);
        harness.release_action("jump");
        let landings = |harness: &super::Harness| {
            harness
                .world()
                .get::<Counters>(leaper)
                .unwrap()
                .get("landings")
        };
        let mut frames = 0;
        while landings(&harness) == 0 && frames < frames_for(1.5, dt) {
            harness.step(1);
            frames += 1;
        }
        let counters = harness.world().get::<Counters>(leaper).unwrap();
        assert_eq!(
            ["jumps", "air_jumps", "landings"].map(|name| counters.get(name)),
            [1, 1, 1],
            "dt {dt}"
        );
        assert!(grounded(&harness, leaper), "dt {dt}");
        let landed_at = body_at(&harness, leaper) + Vec2::new(0.0, 20.0);
        let puffs = dust(&harness);
        assert_eq!(
            puffs.len(),
            1,
            "dt {dt}: the jump's gone, the landing's new"
        );
        assert!(
            puffs[0].distance(landed_at) < 1.0,
            "dt {dt}: {puffs:?} at {landed_at}"
        );
        harness.step(frames_for(1.0, dt));
        assert_eq!(landings(&harness), 1, "dt {dt}: once");
    }
}

/// 7.10g: the lander falls onto the ground, its `landed` cue plays the land
/// clip once over its state, and it runs on in its run clip after the
/// clip's 300 ms.
#[test]
fn the_hero_lands_on_its_land_clip_and_runs_again() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let lander = spawn(&mut harness, "lander", [0.0, 150.0]);
        let shown = |harness: &super::Harness| {
            harness
                .world()
                .get::<AnimationState>(lander)
                .map(|state| state.animation_id.clone())
        };
        let mut frames = 0;
        while shown(&harness).as_deref() != Some("acrobat_land") {
            assert!(frames < frames_for(1.0, dt), "dt {dt}: lands within 1 s");
            harness.step(1);
            frames += 1;
        }
        let controller = harness.world().get::<PlatformerController>(lander);
        assert!(controller.unwrap().grounded(), "dt {dt}: on the ground");
        harness.press_action("move_right");
        for _ in 1..frames_for(0.3, dt) {
            harness.step(1);
            assert_eq!(shown(&harness).as_deref(), Some("acrobat_land"), "dt {dt}");
        }
        harness.step(1);
        assert_eq!(
            shown(&harness).as_deref(),
            Some("acrobat_run_right"),
            "dt {dt}: running again"
        );
        let animator = harness.world().get::<CharacterAnimator>(lander).unwrap();
        assert_eq!((animator.state(), animator.playing()), ("move", None));
    }
}

/// 7.10g: the swordsman's swing, written facing right, strikes the post on
/// its right; turned left by a step of its run, the same swing mirrors onto
/// the post on its left.
#[test]
fn a_knights_swing_mirrors_when_it_turns() {
    use crate::kit::Struck;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let swordsman = spawn(&mut harness, "swordsman", [0.0, 270.0]);
        let right = spawn(&mut harness, "straw_post", [24.0, 270.0]);
        let left = spawn(&mut harness, "straw_post", [-24.0, 270.0]);
        let swing = |harness: &mut super::Harness| {
            harness.press_action("attack");
            let mut struck = Vec::new();
            for frame in 0..frames_for(0.25, dt) {
                harness.step(1);
                if frame == 0 {
                    harness.release_action("attack");
                }
                struck.extend(
                    harness
                        .events::<Struck>()
                        .filter(|hit| hit.entity == swordsman)
                        .map(|hit| hit.target),
                );
            }
            struck
        };
        harness.step(2);
        assert_eq!(swing(&mut harness), [right], "dt {dt}: facing right");
        harness.press_action("move_left");
        harness.step(1);
        harness.release_action("move_left");
        let facing = harness.world().get::<Facing>(swordsman).unwrap();
        assert_eq!(facing.direction(), Vec2::NEG_X, "dt {dt}: turned");
        assert_eq!(swing(&mut harness), [left], "dt {dt}: mirrored");
    }
}

/// 7.10g: a runner killed mid-jump, its death policy disarming it, falls
/// straight down from where it died: its run stops, its rise carries on
/// into its fall, and its move actions do nothing more.
#[test]
fn a_dead_hero_falls_straight_and_locked() {
    use crate::kit::kill;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ground", [0.0, 300.0]);
        let runner = spawn(&mut harness, "doomed_runner", [0.0, 270.0]);
        harness.press_action("move_right");
        harness.step(frames_for(0.2, dt));
        harness.press_action("jump");
        harness.step(2);
        harness.release_action("jump");
        let velocity = harness.world().get::<Velocity>(runner).unwrap().0;
        assert!(
            velocity.x > 0.0 && velocity.y < 0.0,
            "dt {dt}: running up: {velocity}"
        );
        kill(harness.world_mut(), runner);
        harness.release_action("move_right");
        harness.press_action("move_left");
        harness.step(1);
        let x = body_at(&harness, runner).x;
        let rise = harness.world().get::<Velocity>(runner).unwrap().0;
        assert_eq!(rise.x, 0.0, "dt {dt}: its run stopped");
        assert!(rise.y < 0.0, "dt {dt}: its rise kept");
        harness.step(frames_for(0.5, dt));
        let fall = harness.world().get::<Velocity>(runner).unwrap().0;
        assert_eq!(body_at(&harness, runner).x, x, "dt {dt}: straight down");
        assert!(
            fall.x == 0.0 && fall.y > rise.y,
            "dt {dt}: falling, locked: {fall}"
        );
        harness.release_action("move_left");
    }
}

/// 7.10g: the warded knight is immune to cold and wears a shield; `kill`
/// passes both and takes all its health and the shield at once.
#[test]
fn a_kill_passes_a_shield_and_immunity() {
    use crate::kit::{Damaged, Hit, Shield, kill};
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let knight = spawn(&mut harness, "warded_knight", [0.0, 5000.0]);
        harness.world_mut().insert(knight, Shield::new(10.0));
        let cold = DamageRequest::from_parts(
            knight,
            [DamagePart::new(30.0).with_tags(Tags::new(["damage.cold"]))],
        );
        assert_eq!(
            cold.apply(harness.world_mut()),
            Hit::Ignored,
            "dt {dt}: immune"
        );
        harness.step(1);
        assert_eq!(kill(harness.world_mut(), knight), Hit::Killed, "dt {dt}");
        let world = harness.world();
        assert!(world.get::<Health>(knight).unwrap().is_dead(), "dt {dt}");
        assert_eq!(
            world.get::<Shield>(knight).unwrap().current(),
            0.0,
            "dt {dt}"
        );
        let killed: Vec<(f32, f32)> = world
            .resource::<tungsten_core::EventQueue<Damaged>>()
            .iter_current()
            .filter(|hit| hit.entity == knight && hit.killed)
            .map(|hit| (hit.amount, hit.absorbed))
            .collect();
        assert_eq!(
            killed,
            [(50.0, 10.0)],
            "dt {dt}: its 40 and its shield's 10"
        );
        assert_eq!(
            kill(harness.world_mut(), knight),
            Hit::Ignored,
            "dt {dt}: once"
        );
    }
}

/// 7.10g: past the level's `WorldBounds`, 32 px under its bottom, the
/// falling hero is killed, its body kept for its death policy, and the
/// crate, without health, is despawned.
#[test]
fn the_worlds_bottom_kills_the_hero_and_takes_a_crate() {
    use crate::kit::{Damaged, WorldBounds};
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        harness
            .world_mut()
            .insert_resource(WorldBounds::new(Vec2::ZERO, Vec2::new(800.0, 400.0)));
        let hero = spawn(&mut harness, "falling_hero", [100.0, 300.0]);
        let crate_ = spawn(&mut harness, "falling_crate", [200.0, 300.0]);
        let mut kills = 0;
        let mut frames = 0;
        while harness.world().is_alive(crate_)
            || !harness.world().get::<Health>(hero).unwrap().is_dead()
        {
            assert!(frames < frames_for(2.0, dt), "dt {dt}: both fall past");
            harness.step(1);
            frames += 1;
            kills += harness
                .events::<Damaged>()
                .filter(|hit| hit.entity == hero && hit.killed)
                .count();
        }
        assert!(
            body_at(&harness, hero).y > 432.0,
            "dt {dt}: under the margin"
        );
        harness.step(frames_for(0.5, dt));
        kills += harness
            .events::<Damaged>()
            .filter(|hit| hit.entity == hero && hit.killed)
            .count();
        assert_eq!(kills, 1, "dt {dt}: killed once");
        assert!(harness.world().is_alive(hero), "dt {dt}: its body kept");
    }
}
