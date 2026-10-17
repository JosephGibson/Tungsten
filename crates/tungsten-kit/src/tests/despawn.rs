use super::*;
use crate::transform::hierarchy::spawn_child;
use tungsten_core::Stage;

fn setup() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(CommandBuffer::new());
    let mut schedule = Schedule::new();
    DespawnOutsidePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One step: `fixed_update`, then the command flush.
fn frame(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
}

fn at(x: f32, y: f32, rule: DespawnOutside) -> (Transform, DespawnOutside) {
    (Transform::from_position(Vec2::new(x, y)), rule)
}

#[test]
fn the_plugin_registers_its_system_in_the_overlap_phase() {
    let (_, schedule) = setup();
    assert_eq!(schedule.stage_of(DESPAWN_OUTSIDE), Some(Stage::FixedUpdate));
    assert_eq!(schedule.len(), 8, "the seven anchors and the system");
}

#[test]
fn a_region_takes_what_leaves_its_closed_box() {
    let (mut world, mut schedule) = setup();
    let arena = DespawnOutside::region(Vec2::ZERO, Vec2::new(100.0, 50.0));
    let inside = world.spawn_with(at(50.0, 25.0, arena));
    let on_the_edge = world.spawn_with(at(100.0, 0.0, arena));
    let left = world.spawn_with(at(-0.5, 25.0, arena));
    let below = world.spawn_with(at(50.0, 50.5, arena));
    frame(&mut world, &mut schedule);
    assert!(world.is_alive(inside));
    assert!(world.is_alive(on_the_edge));
    assert!(!world.is_alive(left));
    assert!(!world.is_alive(below));
}

#[test]
fn a_view_takes_what_lies_beyond_its_margin() {
    let (mut world, mut schedule) = setup();
    let mut camera = CameraState::new();
    camera.position = Vec2::new(100.0, 0.0);
    camera.zoom = 2.0;
    world.insert_resource(camera);
    world.insert_resource(WindowSize {
        width: 400,
        height: 200,
    });
    // The view covers (100, 0) to (300, 100).
    let view = DespawnOutside::view(10.0);
    let inside = world.spawn_with(at(200.0, 50.0, view));
    let within_the_margin = world.spawn_with(at(305.0, 50.0, view));
    let beyond_it = world.spawn_with(at(311.0, 50.0, view));
    let above = world.spawn_with(at(200.0, -10.5, view));
    frame(&mut world, &mut schedule);
    assert!(world.is_alive(inside));
    assert!(world.is_alive(within_the_margin));
    assert!(!world.is_alive(beyond_it));
    assert!(!world.is_alive(above));
}

#[test]
fn a_view_entity_stays_without_a_window_size_or_a_camera() {
    let (mut world, mut schedule) = setup();
    let far = world.spawn_with(at(1e6, 1e6, DespawnOutside::view(0.0)));
    world.insert_resource(CameraState::new());
    frame(&mut world, &mut schedule);
    assert!(world.is_alive(far));
    world.remove_resource::<CameraState>();
    world.insert_resource(WindowSize {
        width: 640,
        height: 360,
    });
    frame(&mut world, &mut schedule);
    assert!(world.is_alive(far));
}

#[test]
fn the_children_go_with_the_entity() {
    let (mut world, mut schedule) = setup();
    let arena = DespawnOutside::region(Vec2::ZERO, Vec2::new(100.0, 100.0));
    let bolt = world.spawn_with(at(150.0, 50.0, arena));
    let glow = spawn_child(
        &mut world,
        bolt,
        Vec2::new(-200.0, 0.0),
        (Transform::default(),),
    );
    frame(&mut world, &mut schedule);
    assert!(!world.is_alive(bolt));
    assert!(!world.is_alive(glow));
}

#[test]
fn an_entity_without_a_transform_is_not_tested() {
    let (mut world, mut schedule) = setup();
    let loose = world.spawn_with((DespawnOutside::region(Vec2::ZERO, Vec2::ONE),));
    frame(&mut world, &mut schedule);
    assert!(world.is_alive(loose));
}

#[test]
fn a_zero_area_region_is_allowed() {
    let point = DespawnOutside::region(Vec2::ONE, Vec2::ONE);
    assert_eq!(point, DespawnOutside::region(Vec2::ONE, Vec2::ONE));
}

#[test]
#[should_panic(expected = "lies above")]
fn a_region_with_min_above_max_panics() {
    let _ = DespawnOutside::region(Vec2::new(0.0, 10.0), Vec2::new(10.0, 5.0));
}

#[test]
#[should_panic(expected = "not finite")]
fn a_region_with_a_nan_panics() {
    let _ = DespawnOutside::region(Vec2::new(f32::NAN, 0.0), Vec2::ONE);
}

#[test]
#[should_panic(expected = "negative or not finite")]
fn a_negative_margin_panics() {
    let _ = DespawnOutside::view(-1.0);
}

#[test]
#[should_panic(expected = "negative or not finite")]
fn an_infinite_margin_panics() {
    let _ = DespawnOutside::view(f32::INFINITY);
}

#[test]
fn the_trace_records_one_despawn_a_frame() {
    use crate::foundation::trace::{KitTrace, TraceKind};

    let (mut world, mut schedule) = setup();
    world.insert_resource(Time::new());
    world.insert_resource(KitTrace::default());
    let arena = DespawnOutside::region(Vec2::ZERO, Vec2::new(10.0, 10.0));
    let stray = world.spawn_with(at(50.0, 5.0, arena));
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    // Two steps of one frame, then the flush.
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    frame(&mut world, &mut schedule);
    let despawns: Vec<_> = world.resource::<KitTrace>().about(stray).collect();
    assert_eq!(despawns.len(), 1);
    assert_eq!(
        despawns[0].kind,
        TraceKind::Despawn {
            by: DESPAWN_OUTSIDE
        }
    );
    assert!(!world.is_alive(stray));
}
