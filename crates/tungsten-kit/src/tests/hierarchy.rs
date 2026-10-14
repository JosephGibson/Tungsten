use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// A marker for the tests' entities.
struct Tag;

fn setup() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(CommandBuffer::new());
    let mut schedule = Schedule::new();
    HierarchyPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One frame: `post_update`, then the command flush.
fn frame(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
    flush(world);
}

fn flush(world: &mut World) {
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
}

fn root_at(world: &mut World, x: f32, y: f32) -> Entity {
    world.spawn_with((Transform::from_position(Vec2::new(x, y)),))
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Transform>(entity).unwrap().position
}

fn children(world: &World, entity: Entity) -> Vec<Entity> {
    world.get::<Children>(entity).unwrap().iter().collect()
}

fn move_by(world: &mut World, entity: Entity, delta: Vec2) {
    world.get_mut::<Transform>(entity).unwrap().position += delta;
}

#[test]
fn the_plugin_registers_two_passes_in_post_update() {
    let (_, schedule) = setup();
    assert_eq!(
        schedule.resolved_text().lines().nth(4),
        Some("post_update: transform_propagate, transform_propagate_late")
    );
}

#[test]
fn spawn_child_places_the_child_and_lists_it() {
    let (mut world, _) = setup();
    let player = root_at(&mut world, 100.0, 50.0);
    let ring = spawn_child(&mut world, player, Vec2::new(0.0, -20.0), (Tag,));
    let shield = spawn_child(&mut world, player, Vec2::new(8.0, 0.0), (Tag,));
    assert_eq!(world.get::<Parent>(ring).unwrap().entity(), player);
    assert_eq!(
        world.get::<LocalOffset>(ring),
        Some(&LocalOffset(Vec2::new(0.0, -20.0)))
    );
    assert_eq!(position(&world, ring), Vec2::new(100.0, 30.0));
    assert_eq!(position(&world, shield), Vec2::new(108.0, 50.0));
    let list = world.get::<Children>(player).unwrap();
    assert_eq!(list.len(), 2);
    assert!(!list.is_empty());
    assert_eq!(children(&world, player), [ring, shield]);
}

#[test]
fn spawn_child_keeps_the_bundles_rotation_and_scale() {
    let (mut world, _) = setup();
    let player = root_at(&mut world, 10.0, 10.0);
    let own = Transform {
        position: Vec2::new(-500.0, -500.0),
        rotation: 0.75,
        scale: Vec2::new(2.0, 3.0),
    };
    let child = spawn_child(&mut world, player, Vec2::new(1.0, 2.0), (own, Tag));
    let transform = *world.get::<Transform>(child).unwrap();
    assert_eq!(transform.position, Vec2::new(11.0, 12.0));
    assert_eq!(transform.rotation, 0.75);
    assert_eq!(transform.scale, Vec2::new(2.0, 3.0));
    assert!(world.has::<Tag>(child));
}

#[test]
#[should_panic(expected = "holds a RigidBody")]
fn spawn_child_refuses_a_body() {
    let (mut world, _) = setup();
    let player = root_at(&mut world, 0.0, 0.0);
    spawn_child(&mut world, player, Vec2::ZERO, (RigidBody::dynamic(),));
}

#[test]
fn a_refused_child_is_despawned_and_the_parent_left_as_it_was() {
    let (mut world, _) = setup();
    let player = root_at(&mut world, 0.0, 0.0);
    let count = world.entity_count();
    let refused = catch_unwind(AssertUnwindSafe(|| {
        spawn_child(&mut world, player, Vec2::ZERO, (RigidBody::dynamic(),))
    }));
    assert!(refused.is_err());
    assert_eq!(world.entity_count(), count);
    assert!(!world.has::<Children>(player));
}

#[test]
#[should_panic(expected = "is not alive")]
fn spawn_child_refuses_a_dead_parent() {
    let (mut world, _) = setup();
    let gone = root_at(&mut world, 0.0, 0.0);
    world.despawn(gone);
    spawn_child(&mut world, gone, Vec2::ZERO, (Tag,));
}

#[test]
#[should_panic(expected = "has no Transform")]
fn spawn_child_refuses_a_parent_without_a_transform() {
    let (mut world, _) = setup();
    let bare = world.spawn_with((Tag,));
    spawn_child(&mut world, bare, Vec2::ZERO, (Tag,));
}

#[test]
fn children_follow_their_parent_down_the_chain() {
    let (mut world, mut schedule) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    let turret = spawn_child(&mut world, ship, Vec2::new(10.0, 0.0), (Tag,));
    let barrel = spawn_child(&mut world, turret, Vec2::new(0.0, -5.0), (Tag,));
    move_by(&mut world, ship, Vec2::new(3.0, 4.0));
    frame(&mut world, &mut schedule);
    assert_eq!(position(&world, turret), Vec2::new(13.0, 4.0));
    assert_eq!(position(&world, barrel), Vec2::new(13.0, -1.0));
    // A new offset moves the child, and its own child with it.
    world.get_mut::<LocalOffset>(turret).unwrap().0 = Vec2::new(-10.0, 0.0);
    frame(&mut world, &mut schedule);
    assert_eq!(position(&world, turret), Vec2::new(-7.0, 4.0));
    assert_eq!(position(&world, barrel), Vec2::new(-7.0, -1.0));
}

#[test]
fn the_propagation_never_touches_rotation_or_scale() {
    let (mut world, mut schedule) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    let flag = spawn_child(&mut world, ship, Vec2::new(0.0, -8.0), (Tag,));
    {
        let ship_transform = world.get_mut::<Transform>(ship).unwrap();
        ship_transform.rotation = 1.0;
        ship_transform.scale = Vec2::splat(4.0);
    }
    world.get_mut::<Transform>(flag).unwrap().rotation = -0.5;
    frame(&mut world, &mut schedule);
    let transform = *world.get::<Transform>(flag).unwrap();
    assert_eq!(transform.position, Vec2::new(0.0, -8.0));
    assert_eq!(transform.rotation, -0.5);
    assert_eq!(transform.scale, Vec2::ONE);
}

#[test]
fn a_child_without_an_offset_sits_on_its_parent() {
    let (mut world, mut schedule) = setup();
    let ship = root_at(&mut world, 5.0, 6.0);
    let marker = spawn_child(&mut world, ship, Vec2::new(1.0, 1.0), (Tag,));
    world.remove_component::<LocalOffset>(marker);
    frame(&mut world, &mut schedule);
    assert_eq!(position(&world, marker), Vec2::new(5.0, 6.0));
}

#[test]
fn removing_the_parent_detaches_the_child_where_it_stands() {
    let (mut world, mut schedule) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    let pod = spawn_child(&mut world, ship, Vec2::new(5.0, 0.0), (Tag,));
    let kept = spawn_child(&mut world, ship, Vec2::new(-5.0, 0.0), (Tag,));
    world.remove_component::<Parent>(pod);
    move_by(&mut world, ship, Vec2::new(100.0, 0.0));
    frame(&mut world, &mut schedule);
    assert_eq!(position(&world, pod), Vec2::new(5.0, 0.0));
    assert_eq!(position(&world, kept), Vec2::new(95.0, 0.0));
    assert_eq!(children(&world, ship), [kept]);
}

#[test]
fn a_detached_child_with_children_becomes_a_root() {
    let (mut world, mut schedule) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    let pod = spawn_child(&mut world, ship, Vec2::new(5.0, 0.0), (Tag,));
    let light = spawn_child(&mut world, pod, Vec2::new(0.0, 1.0), (Tag,));
    world.remove_component::<Parent>(pod);
    move_by(&mut world, pod, Vec2::new(10.0, 0.0));
    frame(&mut world, &mut schedule);
    assert_eq!(position(&world, light), Vec2::new(15.0, 1.0));
}

#[test]
fn a_parent_despawned_directly_takes_its_subtree_at_the_first_pass() {
    let (mut world, mut schedule) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    let turret = spawn_child(&mut world, ship, Vec2::ZERO, (Tag,));
    let barrel = spawn_child(&mut world, turret, Vec2::ZERO, (Tag,));
    let other = root_at(&mut world, 0.0, 0.0);
    let others_child = spawn_child(&mut world, other, Vec2::ZERO, (Tag,));
    world.despawn(ship);
    schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    assert!(!world.is_alive(turret));
    assert!(!world.is_alive(barrel));
    assert!(world.is_alive(other) && world.is_alive(others_child));
}

#[test]
fn the_late_pass_only_writes_positions() {
    let (mut world, _) = setup();
    let mut late = Walk::default();
    let ship = root_at(&mut world, 0.0, 0.0);
    let pod = spawn_child(&mut world, ship, Vec2::new(1.0, 0.0), (Tag,));
    let orphan_parent = root_at(&mut world, 0.0, 0.0);
    let orphan = spawn_child(&mut world, orphan_parent, Vec2::ZERO, (Tag,));
    let detached = spawn_child(&mut world, ship, Vec2::ZERO, (Tag,));
    world.remove_component::<Parent>(detached);
    world.despawn(orphan_parent);
    move_by(&mut world, ship, Vec2::new(2.0, 0.0));
    late.propagate(&mut world);
    assert_eq!(position(&world, pod), Vec2::new(3.0, 0.0));
    assert!(world.is_alive(orphan));
    assert_eq!(children(&world, ship), [pod, detached]);
}

#[test]
fn a_parent_tweened_between_the_passes_draws_with_its_children() {
    let (mut world, _) = setup();
    let mut schedule = Schedule::new();
    HierarchyPlugin.build(&mut schedule, &mut world);
    // A stand-in for the tween tick, between the two passes.
    schedule.add(
        Stage::PostUpdate,
        system(TWEEN_TICK, |world: &mut World| {
            let ships: Vec<Entity> = world
                .query_filtered::<Entity, (With<Children>, Without<Parent>)>()
                .collect();
            for ship in ships {
                world.get_mut::<Transform>(ship).unwrap().position.x += 7.0;
            }
        }),
    );
    schedule.resolve().unwrap();
    assert_eq!(
        schedule.resolved_text().lines().nth(4),
        Some("post_update: transform_propagate, tween_tick, transform_propagate_late")
    );
    let ship = root_at(&mut world, 0.0, 0.0);
    let flag = spawn_child(&mut world, ship, Vec2::new(0.0, -3.0), (Tag,));
    frame(&mut world, &mut schedule);
    assert_eq!(position(&world, ship), Vec2::new(7.0, 0.0));
    assert_eq!(position(&world, flag), Vec2::new(7.0, -3.0));
}

#[test]
#[should_panic(expected = "has a Parent and a RigidBody")]
fn the_first_pass_refuses_a_body_on_a_child() {
    let (mut world, mut schedule) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    let pod = spawn_child(&mut world, ship, Vec2::ZERO, (Tag,));
    world.insert(pod, RigidBody::dynamic());
    frame(&mut world, &mut schedule);
}

#[test]
fn a_body_may_be_a_parent() {
    let (mut world, mut schedule) = setup();
    let ship = world.spawn_with((Transform::default(), RigidBody::dynamic()));
    let flame = spawn_child(&mut world, ship, Vec2::new(0.0, 4.0), (Tag,));
    move_by(&mut world, ship, Vec2::new(1.0, 1.0));
    frame(&mut world, &mut schedule);
    assert_eq!(position(&world, flame), Vec2::new(1.0, 5.0));
}

#[test]
fn a_root_without_a_transform_leaves_its_children_where_they_are() {
    let (mut world, mut schedule) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    let pod = spawn_child(&mut world, ship, Vec2::new(2.0, 0.0), (Tag,));
    world.remove_component::<Transform>(ship);
    frame(&mut world, &mut schedule);
    assert_eq!(position(&world, pod), Vec2::new(2.0, 0.0));
}

#[test]
fn despawn_recursive_takes_the_subtree_at_the_flush() {
    let (mut world, _) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    let turret = spawn_child(&mut world, ship, Vec2::ZERO, (Tag,));
    let barrel = spawn_child(&mut world, turret, Vec2::ZERO, (Tag,));
    let sibling = root_at(&mut world, 0.0, 0.0);
    despawn_recursive(world.resource_mut::<CommandBuffer>(), ship);
    assert!(world.is_alive(ship) && world.is_alive(barrel));
    flush(&mut world);
    assert!(!world.is_alive(ship));
    assert!(!world.is_alive(turret));
    assert!(!world.is_alive(barrel));
    assert!(world.is_alive(sibling));
}

#[test]
fn despawn_recursive_spares_a_child_detached_earlier_in_the_flush() {
    let (mut world, _) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    let pod = spawn_child(&mut world, ship, Vec2::ZERO, (Tag,));
    let kept = spawn_child(&mut world, ship, Vec2::ZERO, (Tag,));
    let commands = world.resource_mut::<CommandBuffer>();
    commands.remove_component::<Parent>(kept);
    despawn_recursive(commands, ship);
    flush(&mut world);
    assert!(!world.is_alive(ship));
    assert!(!world.is_alive(pod));
    assert!(world.is_alive(kept));
}

#[test]
fn despawn_recursive_drops_the_entity_from_its_parents_list() {
    let (mut world, _) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    let pod = spawn_child(&mut world, ship, Vec2::ZERO, (Tag,));
    let kept = spawn_child(&mut world, ship, Vec2::ZERO, (Tag,));
    despawn_recursive(world.resource_mut::<CommandBuffer>(), pod);
    flush(&mut world);
    assert!(!world.is_alive(pod));
    assert_eq!(children(&world, ship), [kept]);
}

#[test]
fn despawn_recursive_of_a_dead_entity_is_a_no_op() {
    let (mut world, _) = setup();
    let ship = root_at(&mut world, 0.0, 0.0);
    world.despawn(ship);
    let count = world.entity_count();
    despawn_recursive(world.resource_mut::<CommandBuffer>(), ship);
    despawn_recursive(world.resource_mut::<CommandBuffer>(), ship);
    flush(&mut world);
    assert_eq!(world.entity_count(), count);
}
