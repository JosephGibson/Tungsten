use super::*;
use tungsten_core::physics::{PhysicsPlugin, RigidBodyBundle};
use tungsten_core::{Bundle, DebugDraw, Stage};

use crate::{CameraTarget, DespawnOutside, Spawner, TagFilter, Zone};

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
fn without_a_debug_draw_resource_nothing_happens() {
    let (mut world, mut schedule) = rig();
    world.remove_resource::<DebugDraw>();
    world.resource_mut::<KitDebug>().set_enabled(true);
    world.spawn_with((Transform::default(), CameraTarget));
    schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    assert!(!world.has_resource::<DebugDraw>());
}
