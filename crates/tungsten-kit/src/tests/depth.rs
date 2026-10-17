use super::*;
use glam::Vec2;
use serde_json::json;
use tungsten_core::{SpriteAssetId, Stage};

use crate::spawn_child;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    DepthSortPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn present(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
}

fn sorted(world: &mut World, sort: DepthSort, y: f32) -> Entity {
    world.spawn_with((
        sort,
        Transform::from_position(Vec2::new(0.0, y)),
        Sprite::new(SpriteAssetId::new(0)),
    ))
}

fn z(world: &World, entity: Entity) -> i32 {
    world.get::<Sprite>(entity).unwrap().z_order
}

#[test]
fn what_stands_lower_draws_later() {
    let (mut world, mut schedule) = rig();
    let high = sorted(&mut world, DepthSort::new().with_offset(16.0), 50.0);
    let low = sorted(&mut world, DepthSort::new().with_offset(16.0), 80.0);
    present(&mut world, &mut schedule);
    assert_eq!((z(&world, high), z(&world, low)), (66, 96));
    world.get_mut::<Transform>(high).unwrap().position.y = 90.4;
    present(&mut world, &mut schedule);
    assert!(z(&world, high) > z(&world, low), "it walked below");
    assert_eq!(z(&world, high), 106);
}

#[test]
fn the_base_lifts_a_layer_and_the_sum_clamps_to_i32() {
    let (mut world, mut schedule) = rig();
    let floor = sorted(&mut world, DepthSort::new().with_base(-5000), 10.0);
    let roof = sorted(&mut world, DepthSort::new().with_base(i32::MAX), 1e9);
    let pit = sorted(&mut world, DepthSort::new().with_base(i32::MIN), -1e9);
    present(&mut world, &mut schedule);
    assert_eq!(z(&world, floor), -4990);
    assert_eq!(z(&world, roof), i32::MAX);
    assert_eq!(z(&world, pit), i32::MIN);
}

#[test]
fn a_child_is_sorted_and_an_entity_without_a_sprite_is_left() {
    let (mut world, mut schedule) = rig();
    let parent = world.spawn_with((Transform::from_position(Vec2::new(0.0, 30.0)),));
    let shadow = spawn_child(
        &mut world,
        parent,
        Vec2::new(0.0, 12.0),
        (DepthSort::new(), Sprite::new(SpriteAssetId::new(0))),
    );
    let ghost = world.spawn_with((DepthSort::new(), Transform::default()));
    present(&mut world, &mut schedule);
    assert_eq!(z(&world, shadow), 42);
    assert!(world.get::<Sprite>(ghost).is_none());
}

#[test]
fn a_save_round_trips_and_refuses_what_is_not_whole() {
    let sort = DepthSort::new().with_offset(-8.5).with_base(7);
    let saved = serde_json::to_value(sort).unwrap();
    assert_eq!(serde_json::from_value::<DepthSort>(saved).unwrap(), sort);
    for refused in [
        json!({ "base": 0.5 }),
        json!({ "base": 3_000_000_000_u64 }),
        json!({ "z": 1 }),
    ] {
        assert!(
            serde_json::from_value::<DepthSort>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}
