use super::*;
use tungsten_core::physics::PhysicsPlugin;
use tungsten_core::{Plugin, Stage};

use crate::{
    ACTION_INPUT, AIM, ANIMATION_TICK, CAMERA_TARGET, CHARACTER_ANIMATOR, CONTACT_DAMAGE, CUES,
    ContactDamagePlugin, CuesPlugin, DAMAGE_RESOLVE, DEPTH_SORT, DESPAWN_OUTSIDE,
    DespawnOutsidePlugin, FACING, FOLLOW, HEALTH_TICK, HIT_FLASH_TICK, HIT_GRID, HITBOX_TICK,
    HealthPlugin, HitPlugin, KIT_DESPAWN, KNOCKBACK, KitPlugins, LIFETIME_TICK, LINEAR_MOTION,
    LifetimePlugin, ORBIT, OSCILLATOR, OSCILLATOR_PRESENT, OscillatorPlugin, PATH_FOLLOW,
    PICKUP_MAGNET, PICKUP_TICK, PLATFORMER_CONTROLLER, PROJECTILE_TICK, PickupPlugin,
    ProjectilePlugin, SHIELD_TICK, SPAWN_CHILDREN, SPAWNER_TICK, SPEED_LIMIT, SPEED_LIMIT_SETTLE,
    STAY_IN_VIEW, STEERING, SpawnChildrenPlugin, SpawnerPlugin, SpeedLimitPlugin, StayInViewPlugin,
    TOP_DOWN_MOVER, TRANSFORM_PROPAGATE_LATE, WRAP, WrapPlugin, ZONE_TICK,
};

/// The engine's names the phases order against: the physics plugin, the
/// tween tick and the camera, as `DefaultPlugins` registers them.
fn engine(schedule: &mut Schedule, world: &mut World) {
    PhysicsPlugin.build(schedule, world);
    schedule.add(Stage::PostUpdate, system(TWEEN_TICK, |_: &mut World| {}));
    schedule.add(
        Stage::PostUpdate,
        system(CAMERA_UPDATE, |_: &mut World| {}).after(TWEEN_TICK),
    );
}

fn kit(schedule: &mut Schedule, world: &mut World) {
    KitPlugins::set().build(schedule, world);
}

/// The resolved position of `name` in `stage`.
fn index(schedule: &Schedule, stage: Stage, name: &str) -> usize {
    schedule
        .names(stage)
        .position(|n| n == name)
        .unwrap_or_else(|| panic!("{name} is not in {stage}"))
}

/// Every phase's order against the engine's names, in a schedule built by
/// `build`.
fn check_order(build: fn(&mut Schedule, &mut World)) {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let fixed = |name: &str| index(&schedule, Stage::FixedUpdate, name);
    let post = |name: &str| index(&schedule, Stage::PostUpdate, name);

    assert!(fixed(KIT_TICK) > fixed(PHYSICS_PREV_SNAPSHOT));
    for member in [LIFETIME_TICK, HEALTH_TICK, SHIELD_TICK] {
        assert!(fixed(member) > fixed(KIT_TICK) && fixed(member) < fixed(KIT_INTENT));
    }
    for member in [ACTION_INPUT, AIM] {
        assert!(fixed(member) > fixed(KIT_INTENT) && fixed(member) < fixed(KIT_MOVE));
    }
    for member in [
        PATH_FOLLOW,
        PLATFORMER_CONTROLLER,
        TOP_DOWN_MOVER,
        LINEAR_MOTION,
        OSCILLATOR,
        STEERING,
        FOLLOW,
        ORBIT,
        KNOCKBACK,
        PICKUP_MAGNET,
        SPEED_LIMIT,
    ] {
        assert!(fixed(member) > fixed(KIT_MOVE), "{member} after its anchor");
        assert!(
            fixed(member) < fixed(PHYSICS_STEP),
            "{member} before the step"
        );
    }
    assert!(fixed(KIT_OVERLAP) > fixed(PHYSICS_STEP));
    for member in [
        DESPAWN_OUTSIDE,
        ZONE_TICK,
        SPEED_LIMIT_SETTLE,
        HIT_GRID,
        HITBOX_TICK,
        CONTACT_DAMAGE,
        PROJECTILE_TICK,
        PICKUP_TICK,
        WRAP,
        STAY_IN_VIEW,
    ] {
        assert!(fixed(member) > fixed(KIT_OVERLAP) && fixed(member) < fixed(KIT_RESOLVE));
    }
    assert!(fixed(DAMAGE_RESOLVE) > fixed(KIT_RESOLVE) && fixed(DAMAGE_RESOLVE) < fixed(KIT_REACT));
    assert!(fixed(KIT_REACT) > fixed(KIT_RESOLVE));
    assert!(fixed(SPAWNER_TICK) > fixed(KIT_REACT));
    assert!(fixed(KIT_DESPAWN) > fixed(CUES));

    assert!(post(KIT_PRESENT) > post(PHYSICS_SYNC));
    assert!(post(KIT_PRESENT) > post(TRANSFORM_PROPAGATE_LATE));
    assert!(post(KIT_PRESENT) > post(TWEEN_TICK));
    for member in [
        ANIMATION_TICK,
        HIT_FLASH_TICK,
        CAMERA_TARGET,
        OSCILLATOR_PRESENT,
        FACING,
        CHARACTER_ANIMATOR,
        DEPTH_SORT,
    ] {
        assert!(
            post(member) > post(KIT_PRESENT),
            "{member} after its anchor"
        );
        assert!(
            post(member) < post(CAMERA_UPDATE),
            "{member} before the camera"
        );
    }
}

#[test]
fn the_engine_first_then_the_kit_resolves_each_phase_in_place() {
    check_order(|schedule, world| {
        engine(schedule, world);
        kit(schedule, world);
    });
}

#[test]
fn the_kit_first_then_the_engine_resolves_each_phase_in_place() {
    check_order(|schedule, world| {
        kit(schedule, world);
        engine(schedule, world);
    });
}

#[test]
fn ensure_phases_adds_each_anchor_once() {
    let mut schedule = Schedule::new();
    ensure_phases(&mut schedule);
    assert_eq!(schedule.len(), 7);
    ensure_phases(&mut schedule);
    assert_eq!(schedule.len(), 7);
    for phase in Phase::ALL {
        assert_eq!(schedule.stage_of(phase.name()), Some(phase.stage()));
    }
    schedule.resolve().unwrap();
    assert_eq!(
        schedule.names(Stage::FixedUpdate).collect::<Vec<_>>(),
        [
            KIT_TICK,
            KIT_INTENT,
            KIT_MOVE,
            KIT_OVERLAP,
            KIT_RESOLVE,
            KIT_REACT
        ]
    );
    assert_eq!(
        schedule.names(Stage::PostUpdate).collect::<Vec<_>>(),
        [KIT_PRESENT]
    );
}

#[test]
fn a_game_system_joins_a_phase_whichever_side_of_the_kit_it_registers() {
    for game_first in [true, false] {
        let mut world = World::new();
        let mut schedule = Schedule::new();
        engine(&mut schedule, &mut world);
        let add_game = |schedule: &mut Schedule| {
            ensure_phases(schedule);
            schedule.add(
                Stage::FixedUpdate,
                Phase::Move.join(system("magnet", |_: &mut World| {})),
            );
            schedule.add(
                Stage::PostUpdate,
                Phase::Present.join(system("nameplate", |_: &mut World| {})),
            );
        };
        if game_first {
            add_game(&mut schedule);
            kit(&mut schedule, &mut world);
        } else {
            kit(&mut schedule, &mut world);
            add_game(&mut schedule);
        }
        schedule.resolve().unwrap();
        let fixed = |name: &str| index(&schedule, Stage::FixedUpdate, name);
        let post = |name: &str| index(&schedule, Stage::PostUpdate, name);
        assert!(fixed("magnet") > fixed(KIT_MOVE) && fixed("magnet") < fixed(PHYSICS_STEP));
        assert!(fixed("magnet") < fixed(KIT_OVERLAP));
        assert!(post("nameplate") > post(KIT_PRESENT) && post("nameplate") < post(CAMERA_UPDATE));
        // Inside a phase, registration order.
        assert_eq!(fixed("magnet") < fixed(PATH_FOLLOW), game_first);
    }
}

#[test]
fn joining_without_the_anchors_fails_to_resolve() {
    let mut schedule = Schedule::new();
    schedule.add(
        Stage::FixedUpdate,
        Phase::Tick.join(system("cooldowns", |_: &mut World| {})),
    );
    assert!(schedule.resolve().is_err());
}

#[test]
fn the_phases_name_their_anchors_and_stages() {
    assert_eq!(
        Phase::ALL.map(Phase::name),
        [
            KIT_TICK,
            KIT_INTENT,
            KIT_MOVE,
            KIT_OVERLAP,
            KIT_RESOLVE,
            KIT_REACT,
            KIT_PRESENT
        ]
    );
    assert!(
        Phase::ALL[..6]
            .iter()
            .all(|phase| phase.stage() == Stage::FixedUpdate)
    );
    assert_eq!(Phase::Present.stage(), Stage::PostUpdate);
}

/// A plugin's build, for the edge table.
type Build = fn(&mut Schedule, &mut World);

/// Each M65 edge (§2.10 of its plan): the earlier system with the plugin
/// that registers it, then the later one with its plugin.
const OVERLAP_EDGES: [(&str, Build, &str, Build); 23] = [
    (
        OSCILLATOR,
        |s, w| OscillatorPlugin.build(s, w),
        PICKUP_MAGNET,
        |s, w| PickupPlugin.build(s, w),
    ),
    (
        PICKUP_MAGNET,
        |s, w| PickupPlugin.build(s, w),
        SPEED_LIMIT,
        |s, w| SpeedLimitPlugin.build(s, w),
    ),
    (
        HITBOX_TICK,
        |s, w| HitPlugin.build(s, w),
        DESPAWN_OUTSIDE,
        |s, w| DespawnOutsidePlugin.build(s, w),
    ),
    (
        HITBOX_TICK,
        |s, w| HitPlugin.build(s, w),
        WRAP,
        |s, w| WrapPlugin.build(s, w),
    ),
    (
        HITBOX_TICK,
        |s, w| HitPlugin.build(s, w),
        STAY_IN_VIEW,
        |s, w| StayInViewPlugin.build(s, w),
    ),
    (
        CONTACT_DAMAGE,
        |s, w| ContactDamagePlugin.build(s, w),
        DESPAWN_OUTSIDE,
        |s, w| DespawnOutsidePlugin.build(s, w),
    ),
    (
        CONTACT_DAMAGE,
        |s, w| ContactDamagePlugin.build(s, w),
        WRAP,
        |s, w| WrapPlugin.build(s, w),
    ),
    (
        CONTACT_DAMAGE,
        |s, w| ContactDamagePlugin.build(s, w),
        STAY_IN_VIEW,
        |s, w| StayInViewPlugin.build(s, w),
    ),
    (
        PROJECTILE_TICK,
        |s, w| ProjectilePlugin.build(s, w),
        DESPAWN_OUTSIDE,
        |s, w| DespawnOutsidePlugin.build(s, w),
    ),
    (
        PROJECTILE_TICK,
        |s, w| ProjectilePlugin.build(s, w),
        WRAP,
        |s, w| WrapPlugin.build(s, w),
    ),
    (
        PROJECTILE_TICK,
        |s, w| ProjectilePlugin.build(s, w),
        STAY_IN_VIEW,
        |s, w| StayInViewPlugin.build(s, w),
    ),
    (
        PICKUP_TICK,
        |s, w| PickupPlugin.build(s, w),
        DESPAWN_OUTSIDE,
        |s, w| DespawnOutsidePlugin.build(s, w),
    ),
    (
        PICKUP_TICK,
        |s, w| PickupPlugin.build(s, w),
        WRAP,
        |s, w| WrapPlugin.build(s, w),
    ),
    (
        PICKUP_TICK,
        |s, w| PickupPlugin.build(s, w),
        STAY_IN_VIEW,
        |s, w| StayInViewPlugin.build(s, w),
    ),
    (
        HIT_GRID,
        |s, w| PickupPlugin.build(s, w),
        DESPAWN_OUTSIDE,
        |s, w| DespawnOutsidePlugin.build(s, w),
    ),
    (
        HIT_GRID,
        |s, w| ProjectilePlugin.build(s, w),
        WRAP,
        |s, w| WrapPlugin.build(s, w),
    ),
    (
        HIT_GRID,
        |s, w| ContactDamagePlugin.build(s, w),
        STAY_IN_VIEW,
        |s, w| StayInViewPlugin.build(s, w),
    ),
    (
        CUES,
        |s, w| CuesPlugin.build(s, w),
        KIT_DESPAWN,
        |s, w| LifetimePlugin.build(s, w),
    ),
    (
        CUES,
        |s, w| CuesPlugin.build(s, w),
        KIT_DESPAWN,
        |s, w| HealthPlugin.build(s, w),
    ),
    (
        CUES,
        |s, w| CuesPlugin.build(s, w),
        KIT_DESPAWN,
        |s, w| ProjectilePlugin.build(s, w),
    ),
    (
        CUES,
        |s, w| CuesPlugin.build(s, w),
        KIT_DESPAWN,
        |s, w| PickupPlugin.build(s, w),
    ),
    (
        SPAWNER_TICK,
        |s, w| SpawnerPlugin.build(s, w),
        KIT_DESPAWN,
        |s, w| DespawnOutsidePlugin.build(s, w),
    ),
    (
        SPAWN_CHILDREN,
        |s, w| SpawnChildrenPlugin.build(s, w),
        KIT_DESPAWN,
        |s, w| LifetimePlugin.build(s, w),
    ),
];

#[test]
fn each_overlap_edge_holds_with_its_two_plugins_alone_in_either_order() {
    for (earlier, build_earlier, later, build_later) in OVERLAP_EDGES {
        for earlier_first in [true, false] {
            let mut world = World::new();
            let mut schedule = Schedule::new();
            if earlier_first {
                build_earlier(&mut schedule, &mut world);
                build_later(&mut schedule, &mut world);
            } else {
                build_later(&mut schedule, &mut world);
                build_earlier(&mut schedule, &mut world);
            }
            schedule.resolve().unwrap();
            let fixed = |name: &str| index(&schedule, Stage::FixedUpdate, name);
            assert!(
                fixed(earlier) < fixed(later),
                "{earlier} before {later}, {} first",
                if earlier_first { earlier } else { later }
            );
        }
    }
}
