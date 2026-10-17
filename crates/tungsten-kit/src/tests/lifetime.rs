use super::*;
use crate::transform::hierarchy::{Children, spawn_child};
use glam::Vec2;
use tungsten_core::Stage;
use tungsten_core::Transform;

fn setup() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    let mut schedule = Schedule::new();
    LifetimePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One step of `dt` seconds: the clock, `fixed_update`, the command flush.
fn frame(world: &mut World, schedule: &mut Schedule, dt: f32) {
    world.resource_mut::<Time>().advance_frame(dt);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
}

#[test]
fn the_plugin_registers_its_system_in_the_tick_phase() {
    let (_, schedule) = setup();
    assert_eq!(schedule.stage_of(LIFETIME_TICK), Some(Stage::FixedUpdate));
    assert_eq!(schedule.len(), 8, "the seven anchors and the tick");
}

#[test]
fn the_entity_stays_until_its_time_has_passed() {
    let (mut world, mut schedule) = setup();
    let bolt = world.spawn_with((Lifetime::new(0.25),));
    frame(&mut world, &mut schedule, 0.1);
    frame(&mut world, &mut schedule, 0.1);
    assert!(world.is_alive(bolt));
    let timer = world.get::<Lifetime>(bolt).unwrap().timer();
    assert!(
        (timer.remaining() - 0.05).abs() < 1e-5,
        "{}",
        timer.remaining()
    );
    assert!(
        (timer.fraction() - 0.8).abs() < 1e-5,
        "{}",
        timer.fraction()
    );
    frame(&mut world, &mut schedule, 0.1);
    assert!(!world.is_alive(bolt));
}

#[test]
fn a_zero_lifetime_goes_at_its_first_tick() {
    let (mut world, mut schedule) = setup();
    let flash = world.spawn_with((Lifetime::new(0.0),));
    frame(&mut world, &mut schedule, 0.0);
    assert!(!world.is_alive(flash));
}

#[test]
fn a_paused_clock_holds_every_lifetime() {
    let (mut world, mut schedule) = setup();
    let bolt = world.spawn_with((Lifetime::new(0.25),));
    world.resource_mut::<Time>().pause();
    for _ in 0..10 {
        frame(&mut world, &mut schedule, 0.1);
    }
    assert!(world.is_alive(bolt));
    assert_eq!(world.get::<Lifetime>(bolt).unwrap().timer().elapsed(), 0.0);
    world.resource_mut::<Time>().set_paused(false);
    frame(&mut world, &mut schedule, 0.3);
    assert!(!world.is_alive(bolt));
}

#[test]
fn the_time_scale_stretches_a_lifetime() {
    let (mut world, mut schedule) = setup();
    let bolt = world.spawn_with((Lifetime::new(0.25),));
    world.resource_mut::<Time>().set_scale(0.5);
    frame(&mut world, &mut schedule, 0.3);
    assert!(world.is_alive(bolt));
    frame(&mut world, &mut schedule, 0.3);
    assert!(!world.is_alive(bolt));
}

#[test]
fn the_children_go_with_the_entity() {
    let (mut world, mut schedule) = setup();
    let bolt = world.spawn_with((Transform::default(), Lifetime::new(0.1)));
    let trail = spawn_child(
        &mut world,
        bolt,
        Vec2::new(-4.0, 0.0),
        (Transform::default(),),
    );
    let spark = spawn_child(
        &mut world,
        trail,
        Vec2::new(-2.0, 0.0),
        (Transform::default(),),
    );
    let other = world.spawn_with((Transform::default(),));
    frame(&mut world, &mut schedule, 0.2);
    assert!(!world.is_alive(bolt));
    assert!(!world.is_alive(trail));
    assert!(!world.is_alive(spark));
    assert!(world.is_alive(other));
}

#[test]
fn a_child_with_its_own_lifetime_leaves_its_parent_list() {
    let (mut world, mut schedule) = setup();
    let ship = world.spawn_with((Transform::default(),));
    let flare = spawn_child(&mut world, ship, Vec2::ZERO, (Lifetime::new(0.1),));
    frame(&mut world, &mut schedule, 0.2);
    assert!(!world.is_alive(flare));
    assert!(world.is_alive(ship));
    assert!(world.get::<Children>(ship).unwrap().is_empty());
}

#[test]
fn without_a_command_buffer_a_finished_lifetime_waits() {
    let (mut world, mut schedule) = setup();
    let commands = world.remove_resource::<CommandBuffer>().unwrap();
    let bolt = world.spawn_with((Lifetime::new(0.1),));
    world.resource_mut::<Time>().advance_frame(0.2);
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    assert!(world.is_alive(bolt));
    world.insert_resource(commands);
    frame(&mut world, &mut schedule, 0.0);
    assert!(!world.is_alive(bolt));
}

#[test]
#[should_panic(expected = "duration")]
fn a_negative_lifetime_panics() {
    let _ = Lifetime::new(-1.0);
}

#[test]
fn the_trace_records_one_despawn_at_the_step_its_time_ends() {
    use crate::foundation::trace::{KitTrace, TraceKind};

    let (mut world, mut schedule) = setup();
    world.insert_resource(KitTrace::default());
    let spark = world.spawn_with((Lifetime::new(0.1),));
    // Two steps in one frame, no flush between: the despawn is queued twice.
    for _ in 0..2 {
        world.resource_mut::<Time>().advance_frame(0.1);
        schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    }
    let despawns: Vec<_> = world.resource::<KitTrace>().about(spark).collect();
    assert_eq!(despawns.len(), 1);
    assert_eq!(despawns[0].kind, TraceKind::Despawn { by: "lifetime" });
    frame(&mut world, &mut schedule, 0.1);
    assert!(!world.is_alive(spark));
}
