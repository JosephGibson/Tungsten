use super::*;
use tungsten_core::physics::{PhysicsPlugin, RigidBodyBundle};
use tungsten_core::{Bundle, DebugDraw, Stage};

use crate::{
    Aim, CameraTarget, DespawnOutside, Follow, Orbit, Spawner, StayInView, TagFilter, Target, Wrap,
    Zone,
};
use tungsten_core::Plugin;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(DebugDraw::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    KitDebugPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn frame(world: &mut World, schedule: &mut Schedule) -> usize {
    world.resource_mut::<DebugDraw>().clear();
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
    world.resource::<DebugDraw>().len()
}

#[test]
fn the_plugin_inserts_its_resource_off_and_its_system_in_the_present_phase() {
    let (world, schedule) = rig();
    assert!(!world.resource::<KitDebug>().enabled());
    assert_eq!(schedule.stage_of(KIT_DEBUG_DRAW), Some(Stage::PostUpdate));
}

#[test]
fn enabled_it_draws_a_path_followers_polyline_and_disabled_nothing() {
    let (mut world, mut schedule) = rig();
    world.spawn_with(
        RigidBodyBundle::kinematic(Position::new(0.0, 0.0), Collider::aabb(Vec2::splat(8.0))).with(
            (PathFollower::new(
                [Vec2::ZERO, Vec2::new(100.0, 0.0), Vec2::new(100.0, 50.0)],
                50.0,
            )
            .looping(),),
        ),
    );
    assert_eq!(frame(&mut world, &mut schedule), 0, "off by default");
    world.resource_mut::<KitDebug>().set_enabled(true);
    // Two legs, the loop's closing leg and the target's two-line cross.
    assert_eq!(frame(&mut world, &mut schedule), 5);
    world.resource_mut::<KitDebug>().set_enabled(false);
    assert_eq!(frame(&mut world, &mut schedule), 0);
}

#[test]
fn each_shaped_item_draws_once() {
    let (mut world, mut schedule) = rig();
    world.resource_mut::<KitDebug>().set_enabled(true);
    world.spawn_with((
        Transform::from_position(Vec2::new(10.0, 10.0)),
        DespawnOutside::region(Vec2::ZERO, Vec2::splat(100.0)),
    ));
    world.spawn_with((
        Transform::from_position(Vec2::new(10.0, 10.0)),
        DespawnOutside::view(16.0),
    ));
    world.spawn_with(
        RigidBodyBundle::sensor(Position::new(50.0, 50.0), Collider::circle(8.0))
            .with((Zone::new(TagFilter::new()),)),
    );
    world.spawn_with((
        Transform::from_position(Vec2::new(20.0, 20.0)),
        Spawner::new("slime", 1.0),
    ));
    world.spawn_with((
        Transform::from_position(Vec2::new(30.0, 30.0)),
        CameraTarget,
    ));
    // The region's box, the zone's circle, the spawner's circle and the
    // camera target's cross (two lines); a view rule has no shape.
    assert_eq!(frame(&mut world, &mut schedule), 5);
}

#[test]
fn the_hit_items_draw_their_shapes_at_their_points() {
    let (mut world, mut schedule) = rig();
    world.resource_mut::<KitDebug>().set_enabled(true);
    let shape = crate::HitShape::circle(4.0);
    let at = |x: f32| Transform::from_position(Vec2::new(x, 0.0));
    world.spawn_with((at(0.0), crate::Hurtbox::new(shape)));
    world.spawn_with((at(10.0), crate::Hitbox::new(shape, 1.0)));
    world.spawn_with((
        at(20.0),
        crate::ContactDamage::new(1.0).with_shape(shape, Vec2::ZERO),
    ));
    world.spawn_with((at(30.0), crate::ContactDamage::new(1.0)));
    world.spawn_with((at(40.0), crate::Pickup::new(shape)));
    // A contact damage without its own shape draws through its hurtbox.
    assert_eq!(frame(&mut world, &mut schedule), 4);
}

#[test]
fn the_step_count_follows_the_kits_despawn_in_either_order() {
    for despawn_first in [true, false] {
        let mut world = World::new();
        let mut schedule = Schedule::new();
        if despawn_first {
            crate::LifetimePlugin.build(&mut schedule, &mut world);
            KitDebugPlugin.build(&mut schedule, &mut world);
        } else {
            KitDebugPlugin.build(&mut schedule, &mut world);
            crate::LifetimePlugin.build(&mut schedule, &mut world);
        }
        schedule.resolve().unwrap();
        let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
        assert_eq!(order.last(), Some(&KIT_TRACE_STEP), "{order:?}");
    }
}

#[test]
fn the_motion_items_draw_their_circles_goals_regions_and_rays() {
    let (mut world, mut schedule) = rig();
    world.insert_resource(tungsten_core::Time::new());
    crate::OrbitPlugin.build(&mut schedule, &mut world);
    crate::FollowPlugin.build(&mut schedule, &mut world);
    crate::AimPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    world.resource_mut::<KitDebug>().set_enabled(true);
    let anchor = Target::point(Vec2::new(5.0, 5.0));
    world.spawn_with((
        Orbit::new(40.0, 1.0).around(anchor.clone()),
        Position::new(0.0, 0.0),
    ));
    world.spawn_with((Follow::new(anchor.clone()), Position::new(0.0, 0.0)));
    world.spawn_with((Wrap::region(Vec2::ZERO, Vec2::splat(64.0)),));
    world.spawn_with((Wrap::view(4.0),));
    world.spawn_with((StayInView::region(Vec2::ZERO, Vec2::splat(32.0)),));
    world.spawn_with((Aim::at(anchor), Position::new(0.0, 0.0)));
    // Before their first step the orbit, the follower and the aim draw
    // nothing: the two boxes.
    assert_eq!(frame(&mut world, &mut schedule), 2);
    world
        .resource_mut::<tungsten_core::Time>()
        .advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    // The orbit's circle and its centre's cross (two lines), the goal's
    // cross (two), the wrap's and the clamp's boxes and the aim's ray; a
    // view wrap has no shape.
    assert_eq!(frame(&mut world, &mut schedule), 8);
}

#[test]
fn without_a_debug_draw_resource_nothing_happens() {
    let (mut world, mut schedule) = rig();
    world.remove_resource::<DebugDraw>();
    world.resource_mut::<KitDebug>().set_enabled(true);
    world.spawn_with((Transform::default(), CameraTarget));
    schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    assert!(!world.has_resource::<DebugDraw>());
}

#[test]
fn the_plugin_inserts_a_trace_and_counts_each_step_last_in_the_react_phase() {
    let (mut world, mut schedule) = rig();
    assert_eq!(
        world.resource::<KitTrace>().capacity(),
        KitTrace::DEFAULT_CAPACITY
    );
    assert_eq!(schedule.stage_of(KIT_TRACE_STEP), Some(Stage::FixedUpdate));
    for _ in 0..3 {
        schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    }
    assert_eq!(world.resource::<KitTrace>().steps(), 3);
}

#[test]
fn the_view_logs_only_the_entries_pushed_while_it_is_on() {
    use crate::foundation::trace::{TraceKind, trace};

    let (mut world, _) = rig();
    let slime = world.spawn();
    let despawn = || TraceKind::Despawn { by: "cues" };
    let mut logged = 0;
    trace(&mut world, slime, despawn);
    log_trace(&world, &mut logged);
    assert_eq!(logged, 1, "off: it follows the trace and logs nothing");
    world.resource_mut::<KitDebug>().set_enabled(true);
    trace(&mut world, slime, despawn);
    trace(&mut world, slime, despawn);
    let trace = world.resource::<KitTrace>();
    assert_eq!(trace.since(logged).count(), 2, "the two new ones");
    log_trace(&world, &mut logged);
    assert_eq!(logged, 3);
    assert_eq!(world.resource::<KitTrace>().since(logged).count(), 0);
    world.resource_mut::<KitTrace>().clear();
    assert_eq!(
        world.resource::<KitTrace>().since(logged).count(),
        0,
        "a clear logs nothing"
    );
}
