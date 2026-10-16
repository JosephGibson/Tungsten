use super::*;
use crate::{KIT_INTENT, KIT_MOVE, KIT_OVERLAP, KIT_REACT, KIT_RESOLVE, KIT_TICK};
use glam::Vec2;
use tungsten_core::Stage;
use tungsten_core::physics::{
    Collider, PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PhysicsPlugin, Position, RigidBodyBundle,
    Velocity,
};
use tungsten_core::{Bundle, Time};

struct Zone;
struct Player;
struct Enemy;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    TriggerZone::<Zone, Player>::new("zone_player").build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// Runs `count` frames of one step each, collecting the frames' entries.
fn frames(
    world: &mut World,
    schedule: &mut Schedule,
    count: usize,
) -> Vec<TriggerEntered<Zone, Player>> {
    let mut entered = Vec::new();
    for _ in 0..count {
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
        entered.extend(
            world
                .resource::<EventQueue<TriggerEntered<Zone, Player>>>()
                .iter_current()
                .copied(),
        );
        world.flush_events();
    }
    entered
}

fn mover(world: &mut World, x: f32, vx: f32, rest: impl Bundle) -> Entity {
    world.spawn_with(
        RigidBodyBundle::kinematic(Position::new(x, 0.0), Collider::aabb(Vec2::splat(8.0)))
            .with_velocity(Velocity::new(vx, 0.0))
            .with(rest),
    )
}

#[test]
fn the_zone_runs_after_the_step() {
    let (world, schedule) = rig();
    assert!(world.has_event::<TriggerEntered<Zone, Player>>());
    assert_eq!(
        schedule.names(Stage::FixedUpdate).collect::<Vec<_>>(),
        [
            PHYSICS_PREV_SNAPSHOT,
            KIT_TICK,
            KIT_INTENT,
            KIT_MOVE,
            PHYSICS_STEP,
            KIT_OVERLAP,
            "zone_player",
            KIT_RESOLVE,
            KIT_REACT
        ]
    );
}

#[test]
fn each_entry_of_the_target_type_sends_one_event() {
    let (mut world, mut schedule) = rig();
    let zone = world.spawn_with(
        RigidBodyBundle::sensor(Position::new(100.0, 0.0), Collider::aabb(Vec2::splat(16.0)))
            .with((Zone,)),
    );
    let player = mover(&mut world, 0.0, 600.0, (Player,));
    let enemy = mover(&mut world, 200.0, -600.0, (Enemy,));
    let entered = frames(&mut world, &mut schedule, 30);
    assert_eq!(entered, [TriggerEntered::new(zone, player)]);
    let _ = enemy;
    // It comes back through: a second entry.
    world.get_mut::<Velocity>(player).unwrap().0.x = -600.0;
    let back = frames(&mut world, &mut schedule, 30);
    assert_eq!(back, [TriggerEntered::new(zone, player)]);
}

#[test]
fn a_sensor_without_the_zone_type_and_a_tile_never_trigger() {
    let (mut world, mut schedule) = rig();
    world.spawn_with(RigidBodyBundle::sensor(
        Position::new(100.0, 0.0),
        Collider::aabb(Vec2::splat(16.0)),
    ));
    mover(&mut world, 0.0, 600.0, (Player,));
    assert!(frames(&mut world, &mut schedule, 30).is_empty());
    // A tile reaches the zone's system as `other: None`.
    let zone = world.spawn_with((Zone,));
    world
        .resource_mut::<EventQueue<SensorEntered>>()
        .send(SensorEntered::new(zone, None));
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    assert!(
        world
            .resource::<EventQueue<TriggerEntered<Zone, Player>>>()
            .is_empty()
    );
}

#[test]
fn two_zones_over_different_types_are_two_plugins() {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    TriggerZone::<Zone, Player>::new("zone_player").build(&mut schedule, &mut world);
    TriggerZone::<Zone, Enemy>::new("zone_enemy").build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let zone = world.spawn_with(
        RigidBodyBundle::sensor(Position::new(100.0, 0.0), Collider::aabb(Vec2::splat(16.0)))
            .with((Zone,)),
    );
    let enemy = mover(&mut world, 200.0, -600.0, (Enemy,));
    let mut enemies = Vec::new();
    for _ in 0..30 {
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
        enemies.extend(
            world
                .resource::<EventQueue<TriggerEntered<Zone, Enemy>>>()
                .iter_current()
                .copied(),
        );
        assert!(
            world
                .resource::<EventQueue<TriggerEntered<Zone, Player>>>()
                .is_empty()
        );
        world.flush_events();
    }
    assert_eq!(enemies, [TriggerEntered::new(zone, enemy)]);
}

#[test]
fn the_event_compares_and_prints_by_its_entities() {
    let mut world = World::new();
    let (a, b) = (world.spawn(), world.spawn());
    let event = TriggerEntered::<Zone, Player>::new(a, b);
    assert_eq!(event, TriggerEntered::new(a, b));
    assert_ne!(event, TriggerEntered::new(b, a));
    assert!(format!("{event:?}").starts_with("TriggerEntered { zone: "));
    assert_eq!(
        format!("{:?}", TriggerZone::<Zone, Player>::new("gems")),
        "TriggerZone { name: \"gems\" }"
    );
}
