//! Bundles through the direct and the buffered forms: one archetype move,
//! the values a run of inserts would leave, and a World a panicking bundle
//! leaves as it was.

use std::any::TypeId;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use glam::Vec2;

use super::super::archetype::Archetype;
use super::*;
use crate::ecs::{CommandBuffer, Entity, World};
use crate::physics::{
    BodyKind, Collider, Position, PrevPosition, RigidBody, RigidBodyBundle, Velocity,
};

#[derive(Debug, Clone, Copy, PartialEq)]
struct A(u32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct B(u32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct C(u32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct D(u32);
/// Counts its clones' owners, so a test sees a dropped value.
struct Tracked(#[allow(dead_code)] Rc<()>);

/// A game bundle: two components from one field.
struct Pair(u32);

impl Bundle for Pair {
    fn put<S: BundleSink>(self, sink: &mut S) {
        sink.put(A(self.0));
        sink.put(B(self.0 + 1));
    }
}

/// A game bundle whose `put` panics after two values.
struct Panics(Rc<()>);

impl Bundle for Panics {
    fn put<S: BundleSink>(self, sink: &mut S) {
        sink.put(A(100));
        sink.put(Tracked(self.0.clone()));
        panic!("the bundle's put panicked");
    }
}

/// Each archetype's type key and rows, in archetype order.
fn layout(world: &World) -> Vec<(Vec<TypeId>, Vec<Entity>)> {
    world
        .archetypes
        .archetypes
        .iter()
        .map(|arch| (arch.component_types.to_vec(), arch.entities.clone()))
        .collect()
}

/// The archetype `entity` lives in.
fn archetype_of(world: &World, entity: Entity) -> &Archetype {
    let loc = world.archetypes.entities.get(entity).unwrap();
    &world.archetypes.archetypes[loc.archetype_id as usize]
}

/// Whether an archetype never held a row. A run of inserts moves the row
/// through every archetype on its add edges; one move skips them, so they
/// exist with nothing ever pushed.
fn never_held_a_row(arch: &Archetype) -> bool {
    arch.entities.capacity() == 0
}

fn key<T: 'static>() -> TypeId {
    TypeId::of::<T>()
}

fn sorted(mut types: Vec<TypeId>) -> Vec<TypeId> {
    types.sort();
    types
}

#[test]
fn spawn_with_moves_once_into_the_bundles_archetype() {
    let mut world = World::new();
    let entity = world.spawn_with((A(1), B(2), C(3)));

    assert_eq!(world.get::<A>(entity), Some(&A(1)));
    assert_eq!(world.get::<B>(entity), Some(&B(2)));
    assert_eq!(world.get::<C>(entity), Some(&C(3)));
    let arch = archetype_of(&world, entity);
    assert_eq!(
        arch.component_types.to_vec(),
        sorted(vec![key::<A>(), key::<B>(), key::<C>()])
    );
    assert_eq!(arch.entities, vec![entity]);
    // Empty, {A}, {A, B}, {A, B, C}: the inserts' archetypes exist, and the
    // two between the spawn and the end never held the row.
    let archetypes = &world.archetypes.archetypes;
    assert_eq!(archetypes.len(), 4);
    assert!(never_held_a_row(&archetypes[1]));
    assert!(never_held_a_row(&archetypes[2]));
    assert_eq!(world.entity_count(), 1);
}

#[test]
fn insert_bundle_moves_once_and_overwrites_a_type_the_entity_has() {
    let mut world = World::new();
    let entity = world.spawn_with((A(1), B(2)));
    let other = world.spawn_with((A(40), B(41)));

    world.insert_bundle(entity, (B(20), C(30), D(40)));

    assert_eq!(world.get::<A>(entity), Some(&A(1)));
    assert_eq!(world.get::<B>(entity), Some(&B(20)));
    assert_eq!(world.get::<C>(entity), Some(&C(30)));
    assert_eq!(world.get::<D>(entity), Some(&D(40)));
    // The row left behind keeps its own values.
    assert_eq!(world.get::<A>(other), Some(&A(40)));
    assert_eq!(world.get::<B>(other), Some(&B(41)));
    assert!(!world.has::<C>(other));
    // {A, B, C} sits between {A, B} and {A, B, C, D}, and the row skipped it.
    let between = world
        .archetypes
        .archetypes
        .iter()
        .find(|arch| {
            arch.component_types.to_vec() == sorted(vec![key::<A>(), key::<B>(), key::<C>()])
        })
        .unwrap();
    assert!(never_held_a_row(between));
    assert_eq!(archetype_of(&world, entity).component_types.len(), 4);
}

#[test]
fn a_type_named_twice_keeps_the_later_value() {
    let mut world = World::new();
    let entity = world.spawn_with((A(1), B(2), A(3)));
    assert_eq!(world.get::<A>(entity), Some(&A(3)));
    assert_eq!(archetype_of(&world, entity).component_types.len(), 2);

    world.insert_bundle(entity, (B(5), B(6)));
    assert_eq!(world.get::<B>(entity), Some(&B(6)));
    assert_eq!(world.get::<A>(entity), Some(&A(3)));
}

#[test]
fn chain_puts_both_bundles_in_order() {
    let mut world = World::new();
    let entity = world.spawn_with(Pair(1).with((C(3),)).with((A(4),)));

    assert_eq!(world.get::<A>(entity), Some(&A(4)));
    assert_eq!(world.get::<B>(entity), Some(&B(2)));
    assert_eq!(world.get::<C>(entity), Some(&C(3)));
    assert_eq!(archetype_of(&world, entity).component_types.len(), 3);
}

#[test]
fn rigid_body_bundles_carry_the_components_bodies_spawn_with() {
    let mut world = World::new();
    let ground = world.spawn_with(RigidBodyBundle::r#static(
        Position::new(0.0, 0.0),
        Collider::aabb(Vec2::splat(8.0)),
    ));
    let rest = world.spawn_with(RigidBodyBundle::dynamic(
        Position::new(1.0, 2.0),
        Collider::circle(4.0),
    ));
    let ball = world.spawn_with(
        RigidBodyBundle::dynamic(Position::new(5.0, 6.0), Collider::circle(2.0))
            .with_velocity(Velocity::new(3.0, 4.0))
            .with_body(RigidBody::dynamic().with_restitution(0.5))
            .with((A(7),)),
    );

    assert_eq!(archetype_of(&world, ground).component_types.len(), 4);
    assert!(!world.has::<Velocity>(ground));
    assert_eq!(world.get::<RigidBody>(ground), Some(&RigidBody::r#static()));
    assert_eq!(archetype_of(&world, rest).component_types.len(), 5);
    assert_eq!(world.get::<Velocity>(rest), Some(&Velocity(Vec2::ZERO)));
    assert_eq!(
        world.get::<RigidBody>(rest).map(|b| b.kind),
        Some(BodyKind::Dynamic)
    );
    assert_eq!(world.get::<Position>(ball), Some(&Position::new(5.0, 6.0)));
    assert_eq!(world.get::<Velocity>(ball), Some(&Velocity::new(3.0, 4.0)));
    assert_eq!(
        world.get::<RigidBody>(ball).map(|b| b.restitution),
        Some(0.5)
    );
    assert_eq!(world.get::<A>(ball), Some(&A(7)));
    for (body, at) in [
        (ground, Vec2::ZERO),
        (rest, Vec2::new(1.0, 2.0)),
        (ball, Vec2::new(5.0, 6.0)),
    ] {
        assert_eq!(world.get::<PrevPosition>(body), Some(&PrevPosition(at)));
    }

    // Each body ends in the archetype its components inserted one by one
    // give it.
    let by_inserts = world.spawn();
    world.insert(by_inserts, Position::new(0.0, 0.0));
    world.insert(by_inserts, PrevPosition(Vec2::ZERO));
    world.insert(by_inserts, Collider::aabb(Vec2::ONE));
    world.insert(by_inserts, RigidBody::r#static());
    assert_eq!(
        world
            .archetypes
            .entities
            .get(by_inserts)
            .unwrap()
            .archetype_id,
        world.archetypes.entities.get(ground).unwrap().archetype_id
    );
    world.insert(by_inserts, Velocity(Vec2::ZERO));
    world.insert(by_inserts, RigidBody::dynamic());
    assert_eq!(
        world
            .archetypes
            .entities
            .get(by_inserts)
            .unwrap()
            .archetype_id,
        world.archetypes.entities.get(rest).unwrap().archetype_id
    );
}

#[test]
fn buffered_bundles_flush_as_the_direct_forms_apply() {
    let mut direct = World::new();
    let live = direct.spawn();
    direct.insert(live, A(9));
    let spawned = direct.spawn_with((A(1), B(2), C(3)));
    direct.insert_bundle(live, (B(8), C(7)));
    let later = direct.spawn_with((C(5), A(6)));

    let mut buffered = World::new();
    let buffered_live = buffered.spawn();
    buffered.insert(buffered_live, A(9));
    let mut buffer = CommandBuffer::new();
    buffer.spawn_with((A(1), B(2), C(3)));
    buffer.insert_bundle(buffered_live, (B(8), C(7)));
    let pending = buffer.spawn();
    buffer.insert_bundle_pending(pending, (C(5), A(6)));
    buffered.flush(buffer);

    // The same archetypes, created in the same order, with the same rows.
    assert_eq!(layout(&buffered), layout(&direct));
    for entity in [live, spawned, later] {
        assert_eq!(buffered.get::<A>(entity), direct.get::<A>(entity));
        assert_eq!(buffered.get::<B>(entity), direct.get::<B>(entity));
        assert_eq!(buffered.get::<C>(entity), direct.get::<C>(entity));
    }
    assert_eq!(direct.get::<B>(live), Some(&B(8)));
    assert_eq!(direct.get::<A>(later), Some(&A(6)));
}

#[test]
fn a_panicking_bundle_leaves_the_world_as_it_was() {
    let tracker = Rc::new(());
    let mut world = World::new();
    let entity = world.spawn_with((A(1), B(2)));
    let before = layout(&world);

    let insert = catch_unwind(AssertUnwindSafe(|| {
        world.insert_bundle(entity, Panics(tracker.clone()));
    }));
    assert!(insert.is_err());
    let spawn = catch_unwind(AssertUnwindSafe(|| {
        world.spawn_with(Panics(tracker.clone()));
    }));
    assert!(spawn.is_err());

    assert_eq!(world.entity_count(), 1);
    assert_eq!(layout(&world), before);
    assert_eq!(world.get::<A>(entity), Some(&A(1)));
    assert_eq!(world.get::<B>(entity), Some(&B(2)));
    assert!(!world.has::<Tracked>(entity));

    // The next bundle starts from an empty scratch: nothing staged before
    // the panics reaches it, and the staged values are dropped.
    let next = world.spawn_with((A(5), C(6)));
    assert_eq!(world.get::<A>(next), Some(&A(5)));
    assert_eq!(world.get::<C>(next), Some(&C(6)));
    assert!(!world.has::<Tracked>(next));
    assert_eq!(Rc::strong_count(&tracker), 1);
    world.insert_bundle(entity, (A(7),));
    assert_eq!(world.get::<A>(entity), Some(&A(7)));
}

#[test]
#[should_panic(expected = "insert on dead entity")]
fn insert_bundle_on_a_dead_entity_panics() {
    let mut world = World::new();
    let entity = world.spawn();
    world.despawn(entity);
    world.insert_bundle(entity, (A(1),));
}
