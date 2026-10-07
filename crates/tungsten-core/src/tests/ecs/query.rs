//! Tuple queries on a fragmented world, checked against per-entity lookups
//! in the order `query::<Entity>()` walks the archetypes.

use super::super::command_buffer::CommandBuffer;
use super::*;
use crate::ecs::World;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Pos(f32, f32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct Vel(f32, f32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct Health(f32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct Regen(f32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stats([f32; 2]);
#[derive(Debug, Clone, Copy, PartialEq)]
struct Follow(u32);
#[derive(Debug, Clone, Copy)]
struct Tag<const N: u8>;
/// Not `Send`, not `Sync`, not `Clone`: the queries carry no bound.
struct Shared(std::rc::Rc<std::cell::RefCell<u32>>);

/// 64 entities over 16 archetypes: tag bits 0–1, `Regen` on every third,
/// `Stats` on every fourth, `Follow` on every fifth; two spawned without
/// `Pos` and one without `Vel`, plus an empty archetype left by a move.
fn fragmented() -> World {
    let mut world = World::new();
    for index in 0..64_u32 {
        let entity = world.spawn();
        if index != 7 && index != 40 {
            world.insert(entity, Pos(index as f32, 2.0 * index as f32));
        }
        if index != 23 {
            world.insert(entity, Vel(1.0, -1.0));
        }
        world.insert(entity, Health(100.0 - index as f32));
        if index % 3 == 0 {
            world.insert(entity, Regen(0.5));
        }
        if index % 4 == 0 {
            world.insert(entity, Stats([1.0, 2.0]));
        }
        if index % 5 == 0 {
            world.insert(entity, Follow(index));
        }
        if index & 1 != 0 {
            world.insert(entity, Tag::<0>);
        }
        if index & 2 != 0 {
            world.insert(entity, Tag::<1>);
        }
        if index == 9 {
            world.insert(entity, Shared(std::rc::Rc::default()));
        }
    }
    // Leave one archetype empty: the entity that held `Shared` moves on.
    let shared = world
        .query_filtered::<Entity, With<Shared>>()
        .next()
        .unwrap();
    world.remove_component::<Shared>(shared);
    world
}

/// Every entity in query order: archetype order, then row order.
fn entities(world: &World) -> Vec<Entity> {
    world.query::<Entity>().collect()
}

/// One entity's row of the five-column read below, through `get`.
type Row = (Entity, Pos, Vel, Health, Option<Regen>, Option<Stats>);

fn row_by_lookup(world: &World, entity: Entity) -> Option<Row> {
    Some((
        entity,
        *world.get::<Pos>(entity)?,
        *world.get::<Vel>(entity)?,
        *world.get::<Health>(entity)?,
        world.get::<Regen>(entity).copied(),
        world.get::<Stats>(entity).copied(),
    ))
}

/// The five-column read through the query.
fn rows(world: &World) -> Vec<Row> {
    world
        .query::<(Entity, &Pos, &Vel, &Health, Option<&Regen>, Option<&Stats>)>()
        .map(|(e, p, v, h, r, s)| (e, *p, *v, *h, r.copied(), s.copied()))
        .collect()
}

#[test]
fn read_tuples_match_per_entity_lookups() {
    let world = fragmented();
    let expected: Vec<_> = entities(&world)
        .into_iter()
        .filter_map(|e| Some((e, *world.get::<Pos>(e)?, *world.get::<Vel>(e)?)))
        .collect();
    let tuple: Vec<_> = world
        .query::<(Entity, &Pos, &Vel)>()
        .map(|(e, p, v)| (e, *p, *v))
        .collect();
    assert_eq!(tuple, expected);
    assert_eq!(tuple.len(), 61);

    let expected: Vec<_> = entities(&world)
        .into_iter()
        .filter_map(|e| row_by_lookup(&world, e))
        .collect();
    let tuple = rows(&world);
    assert_eq!(tuple, expected);
    assert!(tuple.iter().any(|row| row.4.is_some()));
    assert!(tuple.iter().any(|row| row.4.is_none()));
}

#[test]
fn single_items_need_no_tuple() {
    let mut world = fragmented();
    let expected: Vec<Health> = entities(&world)
        .into_iter()
        .filter_map(|e| world.get::<Health>(e).copied())
        .collect();
    let single: Vec<Health> = world.query::<&Health>().copied().collect();
    assert_eq!(single, expected);
    assert_eq!(single.len(), 64);
    for health in world.query_mut::<&mut Health>() {
        health.0 += 1.0;
    }
    let bumped: Vec<Health> = world.query::<&Health>().copied().collect();
    assert!(bumped.iter().zip(&expected).all(|(b, a)| b.0 == a.0 + 1.0));
    assert_eq!(world.query::<Entity>().count(), 64);
    let optional: Vec<Option<Regen>> = world
        .query::<Option<&Regen>>()
        .map(Option::<&Regen>::copied)
        .collect();
    assert_eq!(optional.len(), 64);
    assert_eq!(optional.iter().filter(|r| r.is_some()).count(), 22);
}

#[test]
fn mutable_tuples_write_every_matching_row() {
    let mut expected = fragmented();
    let mut world = fragmented();
    // The same writes through `get_mut`, entity by entity.
    for entity in entities(&expected) {
        let Some(&vel) = expected.get::<Vel>(entity) else {
            continue;
        };
        if let Some(pos) = expected.get_mut::<Pos>(entity) {
            pos.0 += vel.0;
            pos.1 += vel.1;
        }
    }
    for (pos, vel) in world.query_mut::<(&mut Pos, &Vel)>() {
        pos.0 += vel.0;
        pos.1 += vel.1;
    }
    for entity in entities(&expected) {
        let (Some(&health), true) = (expected.get::<Health>(entity), expected.has::<Vel>(entity))
        else {
            continue;
        };
        if let Some(&regen) = expected.get::<Regen>(entity) {
            expected.get_mut::<Vel>(entity).unwrap().0 += regen.0 * health.0;
        }
        if let Some(stats) = expected.get_mut::<Stats>(entity) {
            stats.0[0] = health.0;
        }
    }
    for (health, vel, regen, stats) in
        world.query_mut::<(&Health, &mut Vel, Option<&Regen>, Option<&mut Stats>)>()
    {
        if let Some(regen) = regen {
            vel.0 += regen.0 * health.0;
        }
        if let Some(stats) = stats {
            stats.0[0] = health.0;
        }
    }
    let mut picked = Vec::new();
    for (entity, health) in world.query_mut::<(Entity, &mut Health)>() {
        if health.0 < 41.0 {
            picked.push(entity);
        }
    }
    let low: Vec<Entity> = entities(&expected)
        .into_iter()
        .filter(|&e| expected.get::<Health>(e).is_some_and(|h| h.0 < 41.0))
        .collect();
    assert_eq!(picked, low);

    let right = rows(&world);
    assert_eq!(rows(&expected), right);
    assert!(
        right
            .iter()
            .any(|row| row.5.is_some_and(|s| s.0[0] == row.3.0))
    );
    // The two entities without `Pos` are outside `rows`.
    let vels = |world: &World| -> Vec<(Entity, Vel)> {
        world
            .query::<(Entity, &Vel)>()
            .map(|(e, v)| (e, *v))
            .collect()
    };
    assert_eq!(vels(&expected), vels(&world));
}

#[test]
fn filters_narrow_by_archetype() {
    let mut expected = fragmented();
    let mut world = fragmented();
    for entity in entities(&expected) {
        if expected.has::<Follow>(entity) {
            continue;
        }
        let (Some(_), Some(&vel), Some(_)) = (
            expected.get::<Pos>(entity),
            expected.get::<Vel>(entity),
            expected.get::<Health>(entity),
        ) else {
            continue;
        };
        expected.get_mut::<Pos>(entity).unwrap().0 -= vel.0;
        expected.get_mut::<Health>(entity).unwrap().0 -= 1.0;
    }
    for (pos, vel, health) in
        world.query_mut_filtered::<(&mut Pos, &Vel, &mut Health), Without<Follow>>()
    {
        pos.0 -= vel.0;
        health.0 -= 1.0;
    }
    let followers: Vec<Entity> = entities(&world)
        .into_iter()
        .filter(|&e| world.has::<Follow>(e))
        .collect();
    let filtered: Vec<Entity> = world.query_filtered::<Entity, With<Follow>>().collect();
    assert_eq!(filtered, followers);
    assert_eq!(filtered.len(), 13);
    let both: Vec<Entity> = world
        .query_filtered::<Entity, (With<Follow>, Without<Stats>, ())>()
        .collect();
    assert_eq!(both.len(), 13 - 4);
    assert_eq!(rows(&expected), rows(&world));
}

#[test]
fn optional_only_queries_are_bounded_by_rows() {
    let world = fragmented();
    let rows: Vec<(Entity, Option<Regen>)> = world
        .query::<(Entity, Option<&Regen>)>()
        .map(|(e, r)| (e, r.copied()))
        .collect();
    assert_eq!(rows.len(), 64);
    let only_optional: Vec<Option<Stats>> = world
        .query::<(Option<&Stats>,)>()
        .map(|(s,)| s.copied())
        .collect();
    assert_eq!(only_optional.len(), 64);
    assert_eq!(only_optional.iter().filter(|s| s.is_some()).count(), 16);
    assert_eq!(World::new().query::<(Option<&Stats>,)>().count(), 0);
}

#[test]
fn slices_cover_each_archetype_once() {
    let mut world = fragmented();
    let mut rows_seen = 0;
    for (rows, (entities, pos, regen)) in world.query_slices::<(Entity, &Pos, Option<&Regen>)>() {
        assert_eq!(entities.len(), rows);
        assert_eq!(pos.len(), rows);
        if let Some(regen) = regen {
            assert_eq!(regen.len(), rows);
        }
        rows_seen += rows;
    }
    assert_eq!(rows_seen, 62);
    let unfollowed: usize = world
        .query_slices_filtered::<&Pos, Without<Follow>>()
        .map(|(rows, pos)| {
            assert_eq!(pos.len(), rows);
            rows
        })
        .sum();
    assert_eq!(
        unfollowed,
        world.query_filtered::<&Pos, Without<Follow>>().count()
    );
    for (rows, (vel, stats)) in world.query_mut_slices::<(&mut Vel, Option<&mut Stats>)>() {
        assert_eq!(vel.len(), rows);
        for vel in vel.iter_mut() {
            vel.1 = 0.0;
        }
        if let Some(stats) = stats {
            for stats in stats.iter_mut() {
                stats.0 = [0.0; 2];
            }
        }
    }
    assert!(world.query::<&Vel>().all(|v| v.1 == 0.0));
    assert!(world.query::<&Stats>().all(|s| s.0 == [0.0; 2]));
    for (_, vel) in world.query_mut_slices_filtered::<&mut Vel, With<Follow>>() {
        for vel in vel {
            vel.0 = -1.0;
        }
    }
    assert!(
        world
            .query::<(&Vel, Option<&Follow>)>()
            .all(|(v, f)| (v.0 == -1.0) == f.is_some())
    );
}

#[test]
fn unbounded_component_types_query() {
    let mut world = fragmented();
    let shared = world.spawn();
    world.insert(shared, Shared(std::rc::Rc::default()));
    world.insert(shared, Health(1.0));
    for (cell, health) in world.query_mut::<(&Shared, &mut Health)>() {
        *cell.0.borrow_mut() += 1;
        health.0 += 1.0;
    }
    assert_eq!(world.query::<&Shared>().count(), 1);
    assert_eq!(world.get::<Health>(shared).map(|h| h.0), Some(2.0));
}

#[test]
fn structural_changes_between_queries_are_seen() {
    let mut world = fragmented();
    let mut buffer = CommandBuffer::new();
    let spawned = buffer.spawn();
    buffer.insert_pending(spawned, Pos(0.0, 0.0));
    buffer.insert_pending(spawned, Vel(0.0, 0.0));
    world.flush(buffer);
    assert_eq!(world.query::<(&Pos, &Vel)>().count(), 62);
    let first = world
        .query::<(Entity, &Pos)>()
        .next()
        .map(|(e, _)| e)
        .unwrap();
    world.despawn(first);
    assert_eq!(world.query::<(&Pos, &Vel)>().count(), 61);
}

#[test]
#[should_panic(
    expected = "(&mut tungsten_core::ecs::query::tests::Pos, &mut tungsten_core::ecs::query::tests::Pos): component types must be distinct"
)]
fn two_mutable_items_of_one_type_panic() {
    let mut world = fragmented();
    let _ = world.query_mut::<(&mut Pos, &mut Pos)>().count();
}

#[test]
#[should_panic(
    expected = "(&tungsten_core::ecs::query::tests::Pos, &mut tungsten_core::ecs::query::tests::Pos): component types must be distinct"
)]
fn a_read_and_a_write_of_one_type_panic() {
    let mut world = fragmented();
    let _ = world.query_mut::<(&Pos, &mut Pos)>().count();
}

#[test]
#[should_panic(
    expected = "(&tungsten_core::ecs::query::tests::Pos, core::option::Option<&mut tungsten_core::ecs::query::tests::Pos>): component types must be distinct"
)]
fn an_optional_write_of_a_read_type_panics() {
    let mut world = fragmented();
    let _ = world.query_mut::<(&Pos, Option<&mut Pos>)>().count();
}

#[test]
fn two_shared_reads_of_one_type_are_allowed_read_only() {
    let world = fragmented();
    assert_eq!(world.query::<(&Pos, &Pos)>().count(), 62);
}
