use super::super::command_buffer::CommandBuffer;
use super::*;
use crate::ecs::{With, Without};

#[derive(Debug, Clone, PartialEq)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct Velocity {
    dx: f32,
    dy: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct Name(String);

#[test]
fn spawn_and_check_alive() {
    let mut world = World::new();
    let e = world.spawn();
    assert!(world.is_alive(e));
}

#[test]
fn despawn_removes_entity() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Position { x: 1.0, y: 2.0 });
    world.despawn(e);
    assert!(!world.is_alive(e));
    assert!(world.get::<Position>(e).is_none());
}

#[test]
fn insert_and_get_component() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Position { x: 3.0, y: 4.0 });
    let pos = world.get::<Position>(e).unwrap();
    assert_eq!(pos.x, 3.0);
    assert_eq!(pos.y, 4.0);
}

#[test]
fn get_mut_component() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Position { x: 0.0, y: 0.0 });
    world.get_mut::<Position>(e).unwrap().x = 10.0;
    assert_eq!(world.get::<Position>(e).unwrap().x, 10.0);
}

#[test]
fn query_iterates_matching_entities() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    let e3 = world.spawn();
    world.insert(e1, Position { x: 1.0, y: 0.0 });
    world.insert(e2, Position { x: 2.0, y: 0.0 });
    world.insert(e3, Name("no position".into()));

    let positions: Vec<_> = world.query::<(Entity, &Position)>().collect();
    assert_eq!(positions.len(), 2);
}

#[test]
#[allow(deprecated)]
fn query_entities_then_mutate() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    world.insert(e1, Position { x: 0.0, y: 0.0 });
    world.insert(e1, Velocity { dx: 1.0, dy: 2.0 });
    world.insert(e2, Position { x: 5.0, y: 5.0 });
    world.insert(e2, Velocity { dx: -1.0, dy: 0.0 });

    let entities = world.query_entities::<Velocity>();
    for entity in entities {
        let vel = world.get::<Velocity>(entity).unwrap().clone();
        let pos = world.get_mut::<Position>(entity).unwrap();
        pos.x += vel.dx;
        pos.y += vel.dy;
    }

    assert_eq!(world.get::<Position>(e1).unwrap().x, 1.0);
    assert_eq!(world.get::<Position>(e1).unwrap().y, 2.0);
    assert_eq!(world.get::<Position>(e2).unwrap().x, 4.0);
}

#[test]
fn resources() {
    let mut world = World::new();

    #[derive(Debug, PartialEq)]
    struct FrameSeconds(f32);

    world.insert_resource(FrameSeconds(0.016));
    assert_eq!(world.get_resource::<FrameSeconds>().unwrap().0, 0.016);

    world.get_resource_mut::<FrameSeconds>().unwrap().0 = 0.033;
    assert_eq!(world.get_resource::<FrameSeconds>().unwrap().0, 0.033);

    assert!(world.has_resource::<FrameSeconds>());
    let dt = world.remove_resource::<FrameSeconds>().unwrap();
    assert_eq!(dt.0, 0.033);
    assert!(!world.has_resource::<FrameSeconds>());
}

#[test]
fn resource_accessors_read_and_write_the_resource() {
    let mut world = World::new();
    world.insert_resource(Name("first".into()));
    assert_eq!(world.resource::<Name>(), &Name("first".into()));
    world.resource_mut::<Name>().0.push_str(" and last");
    assert_eq!(world.resource::<Name>().0, "first and last");
}

#[test]
#[should_panic(expected = "resource `tungsten_core::ecs::world::tests::Name` is not in the World")]
fn resource_panics_naming_the_missing_type() {
    let world = World::new();
    let _ = world.resource::<Name>();
}

#[test]
#[should_panic(
    expected = "resource `tungsten_core::ecs::world::tests::Velocity` is not in the World"
)]
fn resource_mut_panics_naming_the_missing_type() {
    let mut world = World::new();
    let _ = world.resource_mut::<Velocity>();
}

#[test]
fn remove_component() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Position { x: 1.0, y: 2.0 });
    let removed = world.remove_component::<Position>(e).unwrap();
    assert_eq!(removed, Position { x: 1.0, y: 2.0 });
    assert!(world.get::<Position>(e).is_none());
}

#[test]
#[should_panic(expected = "insert on dead entity")]
fn insert_on_dead_entity_panics() {
    let mut world = World::new();
    let e = world.spawn();
    world.despawn(e);
    world.insert(e, Position { x: 0.0, y: 0.0 });
}

#[test]
fn multiple_component_types() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Position { x: 1.0, y: 2.0 });
    world.insert(e, Velocity { dx: 3.0, dy: 4.0 });
    world.insert(e, Name("player".into()));

    assert!(world.has::<Position>(e));
    assert!(world.has::<Velocity>(e));
    assert!(world.has::<Name>(e));
}

#[test]
#[allow(deprecated)]
fn query2_returns_matching_entities() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    let e3 = world.spawn();

    world.insert(e1, Position { x: 1.0, y: 0.0 });
    world.insert(e1, Velocity { dx: 1.0, dy: 0.0 });

    world.insert(e2, Position { x: 2.0, y: 0.0 });

    world.insert(e3, Position { x: 3.0, y: 0.0 });
    world.insert(e3, Velocity { dx: 3.0, dy: 0.0 });

    let results: Vec<_> = world.query2::<Position, Velocity>().collect();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|(e, _, _)| *e != e2));
}

#[test]
#[allow(deprecated)]
fn query2_includes_supersets() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Position { x: 0.0, y: 0.0 });
    world.insert(e, Velocity { dx: 1.0, dy: 2.0 });
    world.insert(e, Name("full".into()));

    let results: Vec<_> = world.query2::<Position, Velocity>().collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e);
}

#[test]
#[allow(deprecated)]
fn query2_entities_then_mutate() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    world.insert(e1, Position { x: 0.0, y: 0.0 });
    world.insert(e1, Velocity { dx: 1.0, dy: 2.0 });
    world.insert(e2, Position { x: 5.0, y: 5.0 });
    world.insert(e2, Velocity { dx: -1.0, dy: 0.0 });

    let entities = world.query2_entities::<Position, Velocity>();
    for entity in entities {
        let vel = world.get::<Velocity>(entity).unwrap().clone();
        let pos = world.get_mut::<Position>(entity).unwrap();
        pos.x += vel.dx;
        pos.y += vel.dy;
    }

    assert_eq!(world.get::<Position>(e1).unwrap().x, 1.0);
    assert_eq!(world.get::<Position>(e2).unwrap().x, 4.0);
}

#[test]
fn query_mut_mutates_in_place() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    world.insert(e1, Position { x: 1.0, y: 0.0 });
    world.insert(e2, Position { x: 2.0, y: 0.0 });
    world.insert(e2, Velocity { dx: 0.0, dy: 0.0 });

    for (_, pos) in world.query_mut::<(Entity, &mut Position)>() {
        pos.x += 10.0;
    }

    assert_eq!(world.get::<Position>(e1).unwrap().x, 11.0);
    assert_eq!(world.get::<Position>(e2).unwrap().x, 12.0);
}

#[test]
#[allow(deprecated)]
fn query2_mut_integrates_velocity_into_position() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    let e3 = world.spawn();
    world.insert(e1, Position { x: 0.0, y: 0.0 });
    world.insert(e1, Velocity { dx: 1.0, dy: 2.0 });
    world.insert(e2, Position { x: 5.0, y: 5.0 });
    world.insert(e2, Velocity { dx: -1.0, dy: 0.0 });
    world.insert(e2, Name("superset".into()));
    world.insert(e3, Position { x: 9.0, y: 9.0 });

    for (_, pos, vel) in world.query2_mut::<Position, Velocity>() {
        pos.x += vel.dx;
        pos.y += vel.dy;
        vel.dx = 0.0;
    }

    assert_eq!(world.get::<Position>(e1).unwrap().x, 1.0);
    assert_eq!(world.get::<Position>(e1).unwrap().y, 2.0);
    assert_eq!(world.get::<Position>(e2).unwrap().x, 4.0);
    assert_eq!(world.get::<Velocity>(e2).unwrap().dx, 0.0);
    // e3 lacks Velocity: untouched.
    assert_eq!(world.get::<Position>(e3).unwrap().x, 9.0);
}

#[test]
#[allow(deprecated)]
fn query3_mut_yields_only_full_matches() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    world.insert(e1, Position { x: 1.0, y: 0.0 });
    world.insert(e1, Velocity { dx: 1.0, dy: 0.0 });
    world.insert(e1, Name("full".into()));
    world.insert(e2, Position { x: 2.0, y: 0.0 });
    world.insert(e2, Velocity { dx: 2.0, dy: 0.0 });

    let mut count = 0;
    for (entity, pos, vel, name) in world.query3_mut::<Position, Velocity, Name>() {
        assert_eq!(entity, e1);
        pos.x += vel.dx;
        name.0.push('!');
        count += 1;
    }

    assert_eq!(count, 1);
    assert_eq!(world.get::<Position>(e1).unwrap().x, 2.0);
    assert_eq!(world.get::<Name>(e1).unwrap().0, "full!");
    assert_eq!(world.get::<Position>(e2).unwrap().x, 2.0);
}

#[test]
fn query_mut_matches_query_order() {
    let mut world = World::new();
    for i in 0..4 {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        if i % 2 == 0 {
            world.insert(e, Velocity { dx: 0.0, dy: 0.0 });
        }
    }

    let immutable: Vec<_> = world
        .query::<(Entity, &Position)>()
        .map(|(e, _)| e)
        .collect();
    let mutable: Vec<_> = world
        .query_mut::<(Entity, &mut Position)>()
        .map(|(e, _)| e)
        .collect();
    assert_eq!(immutable, mutable);
}

#[test]
#[should_panic(expected = "query2_mut: component types must be distinct")]
#[allow(deprecated)]
fn query2_mut_same_type_panics() {
    let mut world = World::new();
    let _ = world.query2_mut::<Position, Position>();
}

#[test]
#[should_panic(expected = "query3_mut: component types must be distinct")]
#[allow(deprecated)]
fn query3_mut_duplicate_type_panics() {
    let mut world = World::new();
    let _ = world.query3_mut::<Position, Velocity, Velocity>();
}

#[test]
#[allow(deprecated)]
fn query3_returns_three_component_entities() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();

    world.insert(e1, Position { x: 1.0, y: 0.0 });
    world.insert(e1, Velocity { dx: 1.0, dy: 0.0 });
    world.insert(e1, Name("full".into()));

    world.insert(e2, Position { x: 2.0, y: 0.0 });
    world.insert(e2, Velocity { dx: 2.0, dy: 0.0 });

    let results: Vec<_> = world.query3::<Position, Velocity, Name>().collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e1);
}

#[test]
fn query_across_multiple_archetypes() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    let e3 = world.spawn();

    world.insert(e1, Position { x: 1.0, y: 0.0 });
    world.insert(e2, Position { x: 2.0, y: 0.0 });
    world.insert(e2, Velocity { dx: 0.0, dy: 0.0 });
    world.insert(e3, Position { x: 3.0, y: 0.0 });
    world.insert(e3, Velocity { dx: 0.0, dy: 0.0 });
    world.insert(e3, Name("three".into()));

    let positions: Vec<_> = world.query::<(Entity, &Position)>().collect();
    assert_eq!(positions.len(), 3);
}

#[test]
fn flush_spawn_entity_is_alive() {
    let mut world = World::new();
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();
    buffer.insert_pending(pending, Name("spawned".into()));

    world.flush(buffer);

    let results: Vec<_> = world.query::<(Entity, &Name)>().collect();
    assert_eq!(results.len(), 1);
    assert!(world.is_alive(results[0].0));
}

#[test]
fn flush_spawn_insert_pending_components_visible() {
    let mut world = World::new();
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();
    buffer.insert_pending(pending, Position { x: 1.0, y: 2.0 });
    buffer.insert_pending(pending, Velocity { dx: 3.0, dy: 4.0 });

    world.flush(buffer);

    let results: Vec<_> = world.query::<(Entity, &Position, &Velocity)>().collect();
    assert_eq!(results.len(), 1);
    let (_, position, velocity) = results[0];
    assert_eq!(*position, Position { x: 1.0, y: 2.0 });
    assert_eq!(*velocity, Velocity { dx: 3.0, dy: 4.0 });
}

#[test]
fn flush_insert_live_entity() {
    let mut world = World::new();
    let entity = world.spawn();
    let mut buffer = CommandBuffer::new();

    buffer.insert(entity, Name("live".into()));
    world.flush(buffer);

    assert_eq!(world.get::<Name>(entity).unwrap(), &Name("live".into()));
}

#[test]
fn flush_despawn_entity_is_dead() {
    let mut world = World::new();
    let entity = world.spawn();
    let mut buffer = CommandBuffer::new();

    buffer.despawn(entity);
    world.flush(buffer);

    assert!(!world.is_alive(entity));
}

#[test]
fn flush_remove_component() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Position { x: 1.0, y: 2.0 });
    let mut buffer = CommandBuffer::new();

    buffer.remove_component::<Position>(entity);
    world.flush(buffer);

    assert!(world.get::<Position>(entity).is_none());
}

#[test]
fn flush_command_order_preserved() {
    let mut world = World::new();
    let entity = world.spawn();
    let mut buffer = CommandBuffer::new();

    buffer.insert(entity, Position { x: 1.0, y: 2.0 });
    buffer.insert(entity, Velocity { dx: 3.0, dy: 4.0 });
    buffer.remove_component::<Position>(entity);
    world.flush(buffer);

    assert!(world.get::<Position>(entity).is_none());
    assert_eq!(
        world.get::<Velocity>(entity),
        Some(&Velocity { dx: 3.0, dy: 4.0 })
    );
}

#[test]
fn flush_despawn_dead_entity_is_silent() {
    let mut world = World::new();
    let entity = world.spawn();
    world.despawn(entity);
    let mut buffer = CommandBuffer::new();

    buffer.despawn(entity);
    world.flush(buffer);

    assert!(!world.is_alive(entity));
}

#[test]
fn flush_insert_skips_entity_despawned_earlier_in_same_buffer() {
    let mut world = World::new();
    let entity = world.spawn();
    let mut buffer = CommandBuffer::new();

    buffer.despawn(entity);
    buffer.insert(entity, Name("late".into()));
    world.flush(buffer);

    assert!(!world.is_alive(entity));
    assert!(world.get::<Name>(entity).is_none());
}

#[test]
fn flush_empty_buffer_is_noop() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Position { x: 1.0, y: 2.0 });

    let before = world.query::<(Entity, &Position)>().count();
    world.flush(CommandBuffer::new());
    let after = world.query::<(Entity, &Position)>().count();

    assert_eq!(before, after);
}

#[test]
fn entity_count_tracks_spawn_and_despawn() {
    let mut world = World::new();
    assert_eq!(world.entity_count(), 0);
    let a = world.spawn();
    let b = world.spawn();
    let c = world.spawn();
    assert_eq!(world.entity_count(), 3);
    world.despawn(b);
    assert_eq!(world.entity_count(), 2);
    world.despawn(a);
    world.despawn(c);
    assert_eq!(world.entity_count(), 0);
}

#[test]
fn flush_multiple_pending_entities() {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Marker(u32);

    let mut world = World::new();
    let mut buffer = CommandBuffer::new();

    for i in 0..3u32 {
        let pending = buffer.spawn();
        buffer.insert_pending(pending, Marker(i));
    }

    world.flush(buffer);

    let mut markers: Vec<_> = world
        .query::<(Entity, &Marker)>()
        .map(|(entity, marker)| {
            assert!(world.is_alive(entity));
            marker.0
        })
        .collect();
    markers.sort_unstable();
    assert_eq!(markers, vec![0, 1, 2]);
}

#[test]
#[allow(deprecated)]
fn query2_opt2_yields_options_per_archetype() {
    let mut world = World::new();

    // Full match: all four components.
    let full = world.spawn();
    world.insert(full, Position { x: 1.0, y: 0.0 });
    world.insert(full, Velocity { dx: 2.0, dy: 0.0 });
    world.insert(full, Name("full".into()));

    // Required only.
    let bare = world.spawn();
    world.insert(bare, Position { x: 3.0, y: 0.0 });
    world.insert(bare, Velocity { dx: 4.0, dy: 0.0 });

    // Missing a required component: excluded entirely.
    let excluded = world.spawn();
    world.insert(excluded, Position { x: 5.0, y: 0.0 });
    world.insert(excluded, Name("excluded".into()));

    let rows: Vec<_> = world
        .query2_opt2::<Position, Velocity, Name, u32>()
        .map(|(e, p, v, n, extra)| (e, p.x, v.dx, n.map(|n| n.0.clone()), extra.copied()))
        .collect();

    assert_eq!(rows.len(), 2);
    let full_row = rows.iter().find(|(e, ..)| *e == full).unwrap();
    assert_eq!(
        (full_row.1, full_row.2, full_row.3.as_deref(), full_row.4),
        (1.0, 2.0, Some("full"), None)
    );
    let bare_row = rows.iter().find(|(e, ..)| *e == bare).unwrap();
    assert_eq!(
        (bare_row.1, bare_row.2, &bare_row.3, bare_row.4),
        (3.0, 4.0, &None, None)
    );
}

#[test]
#[allow(deprecated)]
fn query2_opt2_matches_query2_order() {
    // The writeback zip in physics relies on query2_opt2 / query2_opt2_mut
    // iterating the exact archetype/row order of query2 over the same
    // required pair.
    let mut world = World::new();
    for i in 0..6u32 {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 0.0, dy: 0.0 });
        if i % 2 == 0 {
            world.insert(e, Name(format!("{i}")));
        }
    }

    let base: Vec<_> = world
        .query2::<Position, Velocity>()
        .map(|(e, ..)| e)
        .collect();
    let opt: Vec<_> = world
        .query2_opt2::<Position, Velocity, Name, u32>()
        .map(|(e, ..)| e)
        .collect();
    let opt_mut: Vec<_> = world
        .query2_opt2_mut::<Position, Velocity, Name, u32>()
        .map(|(e, ..)| e)
        .collect();
    assert_eq!(base, opt);
    assert_eq!(base, opt_mut);
}

#[test]
#[allow(deprecated)]
fn query2_opt2_mut_writes_required_and_optional() {
    let mut world = World::new();

    let with_name = world.spawn();
    world.insert(with_name, Position { x: 0.0, y: 0.0 });
    world.insert(with_name, Velocity { dx: 1.0, dy: 0.0 });
    world.insert(with_name, Name("old".into()));

    let without_name = world.spawn();
    world.insert(without_name, Position { x: 0.0, y: 0.0 });
    world.insert(without_name, Velocity { dx: 2.0, dy: 0.0 });

    for (_e, vel, pos, _extra, name) in world.query2_opt2_mut::<Velocity, Position, u32, Name>() {
        pos.x += vel.dx;
        if let Some(name) = name {
            name.0 = "new".into();
        }
    }

    assert_eq!(world.get::<Position>(with_name).unwrap().x, 1.0);
    assert_eq!(world.get::<Position>(without_name).unwrap().x, 2.0);
    assert_eq!(world.get::<Name>(with_name).unwrap().0, "new");
}

#[test]
#[should_panic(expected = "component types must be distinct")]
#[allow(deprecated)]
fn query2_opt2_mut_duplicate_type_panics() {
    let mut world = World::new();
    let _ = world
        .query2_opt2_mut::<Position, Velocity, Position, Name>()
        .count();
}

#[test]
#[allow(deprecated)]
fn query3_mut_without_skips_archetypes_with_the_excluded_type() {
    #[derive(Debug)]
    struct Excluded;

    let mut world = World::new();
    let kept = world.spawn();
    let skipped = world.spawn();
    for (entity, x) in [(kept, 1.0), (skipped, 2.0)] {
        world.insert(entity, Position { x, y: 0.0 });
        world.insert(entity, Velocity { dx: 1.0, dy: 0.0 });
        world.insert(entity, Name("agent".into()));
    }
    world.insert(skipped, Excluded);

    let mut seen = Vec::new();
    for (entity, pos, vel, _name) in
        world.query3_mut_without::<Position, Velocity, Name, Excluded>()
    {
        pos.x += vel.dx;
        seen.push(entity);
    }

    assert_eq!(seen, vec![kept]);
    assert_eq!(world.get::<Position>(kept).unwrap().x, 2.0);
    assert_eq!(world.get::<Position>(skipped).unwrap().x, 2.0);
}

#[test]
#[should_panic(expected = "query3_mut_without: excluded type must differ")]
#[allow(deprecated)]
fn query3_mut_without_rejects_excluding_a_queried_type() {
    let mut world = World::new();
    let _ = world.query3_mut_without::<Position, Velocity, Name, Velocity>();
}

#[test]
#[allow(deprecated)]
fn query3_opt2_matches_query3_order_with_per_archetype_optionals() {
    #[derive(Debug, PartialEq)]
    struct Tag(u8);
    #[derive(Debug, PartialEq)]
    struct Extra(u8);

    let mut world = World::new();
    for i in 0..6u8 {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: f32::from(i),
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 0.0, dy: 0.0 });
        world.insert(e, Name(format!("e{i}")));
        if i % 2 == 0 {
            world.insert(e, Tag(i));
        }
        if i % 3 == 0 {
            world.insert(e, Extra(i));
        }
    }
    let unrelated = world.spawn();
    world.insert(unrelated, Tag(99));

    let plain: Vec<_> = world
        .query3::<Position, Velocity, Name>()
        .map(|(e, ..)| e)
        .collect();
    let rows: Vec<_> = world
        .query3_opt2::<Position, Velocity, Name, Tag, Extra>()
        .map(|(e, pos, _, _, tag, extra)| (e, pos.x as u8, tag.map(|t| t.0), extra.map(|x| x.0)))
        .collect();
    assert_eq!(rows.iter().map(|row| row.0).collect::<Vec<_>>(), plain);
    for (_, i, tag, extra) in rows {
        assert_eq!(tag, (i % 2 == 0).then_some(i));
        assert_eq!(extra, (i % 3 == 0).then_some(i));
    }
}

/// Component family for the insert-run tests: one type per `K`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Part<const K: usize>(u32);

impl<const K: usize> Part<K> {
    /// Constructor that also works through `with_part!`'s type alias.
    fn of(value: u32) -> Self {
        Self(value)
    }
}

/// Dispatches a part index to its component type.
macro_rules! with_part {
    ($k:expr, $part:ident => $body:expr) => {
        match $k {
            0 => {
                type $part = Part<0>;
                $body
            }
            1 => {
                type $part = Part<1>;
                $body
            }
            2 => {
                type $part = Part<2>;
                $body
            }
            3 => {
                type $part = Part<3>;
                $body
            }
            4 => {
                type $part = Part<4>;
                $body
            }
            _ => {
                type $part = Part<5>;
                $body
            }
        }
    };
}

const PARTS: usize = 6;

fn part_value(world: &World, entity: Entity, k: usize) -> Option<u32> {
    match k {
        0 => world.get::<Part<0>>(entity).map(|part| part.0),
        1 => world.get::<Part<1>>(entity).map(|part| part.0),
        2 => world.get::<Part<2>>(entity).map(|part| part.0),
        3 => world.get::<Part<3>>(entity).map(|part| part.0),
        4 => world.get::<Part<4>>(entity).map(|part| part.0),
        _ => world.get::<Part<5>>(entity).map(|part| part.0),
    }
}

/// Everything structural a flush can change: each archetype's type key, its
/// entities in row order and every entity's part values, in archetype order.
type Snapshot = Vec<(Vec<TypeId>, Vec<(Entity, [Option<u32>; PARTS])>)>;

fn snapshot(world: &World) -> Snapshot {
    world
        .archetypes
        .archetypes
        .iter()
        .map(|arch| {
            let rows = arch
                .entities
                .iter()
                .map(|&entity| {
                    (
                        entity,
                        std::array::from_fn(|k| part_value(world, entity, k)),
                    )
                })
                .collect();
            (arch.component_types.to_vec(), rows)
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Spawn,
    InsertPending {
        pending: usize,
        part: usize,
        value: u32,
    },
    InsertLive {
        entity: usize,
        part: usize,
        value: u32,
    },
    Remove {
        entity: usize,
        part: usize,
    },
    Despawn {
        entity: usize,
    },
}

/// The same world for both sides of a comparison: entities spread over a
/// few archetypes, in a fixed order.
fn seeded_world() -> (World, Vec<Entity>) {
    let mut world = World::new();
    let mut entities = Vec::new();
    for i in 0..12u32 {
        let entity = world.spawn();
        for k in 0..PARTS {
            if (i >> (k % 3)) & 1 == 1 && !(i as usize + k).is_multiple_of(4) {
                with_part!(k, P => world.insert(entity, P::of(i * 10 + k as u32)));
            }
        }
        entities.push(entity);
    }
    (world, entities)
}

/// What a flush must equal: every spawn first, then each command through the
/// immediate API, one at a time and in order.
fn apply_one_by_one(world: &mut World, live: &[Entity], ops: &[Op]) {
    let pending: Vec<Entity> = ops
        .iter()
        .filter(|op| matches!(op, Op::Spawn))
        .map(|_| world.spawn())
        .collect();
    for &op in ops {
        match op {
            Op::Spawn => {}
            Op::InsertPending {
                pending: index,
                part,
                value,
            } => with_part!(part, P => world.insert(pending[index], P::of(value))),
            Op::InsertLive {
                entity,
                part,
                value,
            } => {
                if world.is_alive(live[entity]) {
                    with_part!(part, P => world.insert(live[entity], P::of(value)));
                }
            }
            Op::Remove { entity, part } => {
                with_part!(part, P => { world.remove_component::<P>(live[entity]); });
            }
            Op::Despawn { entity } => world.despawn(live[entity]),
        }
    }
}

fn record(buffer: &mut CommandBuffer, live: &[Entity], ops: &[Op]) {
    let mut pending = Vec::new();
    for &op in ops {
        match op {
            Op::Spawn => pending.push(buffer.spawn()),
            Op::InsertPending {
                pending: index,
                part,
                value,
            } => with_part!(part, P => buffer.insert_pending(pending[index], P::of(value))),
            Op::InsertLive {
                entity,
                part,
                value,
            } => with_part!(part, P => buffer.insert(live[entity], P::of(value))),
            Op::Remove { entity, part } => {
                with_part!(part, P => buffer.remove_component::<P>(live[entity]));
            }
            Op::Despawn { entity } => buffer.despawn(live[entity]),
        }
    }
    assert_eq!(buffer.len(), ops.len());
}

/// Flushes each command list of `frames` through one buffer, as the app does
/// frame after frame, and compares the world with the one-by-one result.
fn assert_flush_equals_one_by_one(frames: &[Vec<Op>]) {
    let (mut expected, live) = seeded_world();
    let (mut flushed, flushed_live) = seeded_world();
    assert_eq!(live, flushed_live);
    let mut buffer = CommandBuffer::new();
    for ops in frames {
        apply_one_by_one(&mut expected, &live, ops);
        record(&mut buffer, &live, ops);
        flushed.flush_reusing(&mut buffer);
        assert!(buffer.is_empty());

        assert_eq!(snapshot(&flushed), snapshot(&expected), "ops: {ops:?}");
        assert_eq!(flushed.entity_count(), expected.entity_count());
    }
    // The entity table agrees too: the next spawn reuses the same slot.
    assert_eq!(flushed.spawn(), expected.spawn());
}

/// A fixed command list per seed: spawns with insert runs, runs on live
/// entities, late inserts, removals and despawns.
fn random_ops(seed: u64, base_value: u32) -> Vec<Op> {
    // xorshift: a fixed sequence per seed, no dependency.
    fn next(state: &mut u64) -> usize {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state >> 11) as usize
    }

    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut ops = Vec::new();
    let mut spawned = 0;
    let mut value = base_value;
    while ops.len() < 60 {
        match next(&mut state) % 10 {
            // A spawn followed by a run on it, as game code writes them.
            0..=2 => {
                ops.push(Op::Spawn);
                spawned += 1;
                for _ in 0..next(&mut state) % 7 {
                    value += 1;
                    ops.push(Op::InsertPending {
                        pending: spawned - 1,
                        part: next(&mut state) % PARTS,
                        value,
                    });
                }
            }
            // A run on a live entity, which may be dead by now and may
            // already hold some of the parts.
            3..=6 => {
                let entity = next(&mut state) % 12;
                for _ in 0..=next(&mut state) % 5 {
                    value += 1;
                    ops.push(Op::InsertLive {
                        entity,
                        part: next(&mut state) % PARTS,
                        value,
                    });
                }
            }
            // A late insert on an earlier spawn: its own run.
            7 if spawned > 0 => {
                value += 1;
                ops.push(Op::InsertPending {
                    pending: next(&mut state) % spawned,
                    part: next(&mut state) % PARTS,
                    value,
                });
            }
            8 => ops.push(Op::Remove {
                entity: next(&mut state) % 12,
                part: next(&mut state) % PARTS,
            }),
            _ => ops.push(Op::Despawn {
                entity: next(&mut state) % 12,
            }),
        }
    }
    ops
}

#[test]
fn flush_insert_runs_equal_one_by_one_inserts_on_random_command_lists() {
    for seed in 1..=300u64 {
        assert_flush_equals_one_by_one(&[random_ops(seed, 1_000)]);
    }
}

#[test]
fn flushes_over_several_frames_equal_one_by_one_application() {
    for seed in 1..=100u64 {
        let frames: Vec<_> = (0..3)
            .map(|frame| random_ops(seed * 7 + frame, 1_000 * (frame as u32 + 1)))
            .collect();
        assert_flush_equals_one_by_one(&frames);
    }
}

#[test]
fn flush_insert_run_last_write_wins() {
    let mut world = World::new();
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();
    buffer.insert_pending(pending, Part::<0>(1));
    buffer.insert_pending(pending, Part::<1>(2));
    buffer.insert_pending(pending, Part::<0>(3));

    world.flush(buffer);

    let rows: Vec<_> = world.query::<(Entity, &Part<0>, &Part<1>)>().collect();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].1.0, rows[0].2.0), (3, 2));
    assert_eq!(world.entity_count(), 1);
}

#[test]
fn flush_insert_run_overwrites_components_the_entity_has() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Part::<0>(1));
    world.insert(entity, Part::<1>(1));
    let other = world.spawn();
    world.insert(other, Part::<0>(40));
    world.insert(other, Part::<1>(41));
    let mut buffer = CommandBuffer::new();

    buffer.insert(entity, Part::<0>(5));
    buffer.insert(entity, Part::<2>(7));
    buffer.insert(entity, Part::<1>(9));
    world.flush(buffer);

    assert_eq!(world.get::<Part<0>>(entity), Some(&Part(5)));
    assert_eq!(world.get::<Part<1>>(entity), Some(&Part(9)));
    assert_eq!(world.get::<Part<2>>(entity), Some(&Part(7)));
    // The row left behind keeps its own values.
    assert_eq!(world.get::<Part<0>>(other), Some(&Part(40)));
    assert_eq!(world.get::<Part<1>>(other), Some(&Part(41)));
    assert!(!world.has::<Part<2>>(other));
}

#[test]
fn flush_insert_run_without_a_new_type_overwrites_in_place() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Part::<0>(1));
    world.insert(entity, Part::<1>(2));
    let before = world.archetypes.entities.get(entity);
    let mut buffer = CommandBuffer::new();

    buffer.insert(entity, Part::<1>(20));
    buffer.insert(entity, Part::<0>(10));
    buffer.insert(entity, Part::<1>(21));
    world.flush(buffer);

    assert_eq!(world.archetypes.entities.get(entity), before);
    assert_eq!(world.get::<Part<0>>(entity), Some(&Part(10)));
    assert_eq!(world.get::<Part<1>>(entity), Some(&Part(21)));
}

#[test]
fn flush_insert_run_creates_the_archetypes_one_by_one_inserts_would() {
    let mut world = World::new();
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();
    buffer.insert_pending(pending, Part::<0>(1));
    buffer.insert_pending(pending, Part::<1>(2));
    buffer.insert_pending(pending, Part::<2>(3));

    world.flush(buffer);

    // Empty, {0}, {0, 1}, {0, 1, 2}, in the order the inserts name them.
    let keys: Vec<usize> = world
        .archetypes
        .archetypes
        .iter()
        .map(|arch| arch.component_types.len())
        .collect();
    assert_eq!(keys, vec![0, 1, 2, 3]);
    assert!(world.archetypes.archetypes[1].has(TypeId::of::<Part<0>>()));
    assert!(world.archetypes.archetypes[2].has(TypeId::of::<Part<1>>()));
    // The archetypes the row skipped hold no row, and queries walk them.
    for arch in &world.archetypes.archetypes[..3] {
        assert!(arch.entities.is_empty());
        assert_eq!(arch.columns.len(), arch.component_types.len());
    }
    assert_eq!(world.archetypes.archetypes[3].entities.len(), 1);
    assert_eq!(world.query::<(Entity, &Part<0>)>().count(), 1);
    assert_eq!(world.query::<(Entity, &Part<0>, &Part<1>)>().count(), 1);
    assert_eq!(world.query_mut::<(Entity, &mut Part<1>)>().count(), 1);
}

#[test]
fn flush_inserts_on_different_entities_are_not_merged() {
    let mut world = World::new();
    let first = world.spawn();
    let second = world.spawn();
    let mut buffer = CommandBuffer::new();

    buffer.insert(first, Part::<0>(1));
    buffer.insert(second, Part::<1>(2));
    buffer.insert(first, Part::<2>(3));
    world.flush(buffer);

    // Archetypes appear in command order: {0}, {1}, then {0, 2}.
    let archetypes = &world.archetypes.archetypes;
    assert_eq!(archetypes.len(), 4);
    assert_eq!(&*archetypes[1].component_types, [TypeId::of::<Part<0>>()]);
    assert_eq!(&*archetypes[2].component_types, [TypeId::of::<Part<1>>()]);
    assert_eq!(archetypes[3].entities, vec![first]);
    assert_eq!(world.get::<Part<0>>(first), Some(&Part(1)));
    assert_eq!(world.get::<Part<2>>(first), Some(&Part(3)));
    assert_eq!(world.get::<Part<1>>(second), Some(&Part(2)));
}

#[test]
fn flush_remove_between_inserts_splits_the_run() {
    let mut world = World::new();
    let entity = world.spawn();
    let mut buffer = CommandBuffer::new();

    buffer.insert(entity, Part::<0>(1));
    buffer.remove_component::<Part<0>>(entity);
    buffer.insert(entity, Part::<1>(2));
    world.flush(buffer);

    assert!(!world.has::<Part<0>>(entity));
    assert_eq!(world.get::<Part<1>>(entity), Some(&Part(2)));
}

#[test]
fn flush_insert_run_on_dead_entity_drops_its_values() {
    use std::rc::Rc;

    struct Held(#[allow(dead_code)] Rc<()>);
    struct AlsoHeld(#[allow(dead_code)] Rc<()>);

    let tracker = Rc::new(());
    let mut world = World::new();
    let entity = world.spawn();
    let mut buffer = CommandBuffer::new();

    buffer.despawn(entity);
    buffer.insert(entity, Held(tracker.clone()));
    buffer.insert(entity, AlsoHeld(tracker.clone()));
    assert_eq!(Rc::strong_count(&tracker), 3);
    world.flush(buffer);

    assert!(!world.is_alive(entity));
    assert_eq!(Rc::strong_count(&tracker), 1);
    assert_eq!(world.query::<(Entity, &Held)>().count(), 0);
}

#[test]
fn flush_reusing_empties_the_buffer_and_keeps_its_storage() {
    let mut world = World::new();
    let mut buffer = CommandBuffer::new();
    let record_frame = |buffer: &mut CommandBuffer, base: u32| {
        for i in 0..32 {
            let pending = buffer.spawn();
            buffer.insert_pending(pending, Part::<0>(base + i));
            buffer.insert_pending(pending, Part::<1>(base + i));
        }
    };

    record_frame(&mut buffer, 0);
    assert_eq!(buffer.len(), 96);
    world.flush_reusing(&mut buffer);

    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
    assert_eq!(world.entity_count(), 32);
    let capacity = buffer.commands.capacity();
    assert!(capacity >= 96);

    // Pending handles restart, and an equal frame fits the kept storage.
    record_frame(&mut buffer, 100);
    assert_eq!(buffer.commands.capacity(), capacity);
    world.flush_reusing(&mut buffer);

    assert_eq!(world.entity_count(), 64);
    let mut values: Vec<u32> = world
        .query::<(Entity, &Part<0>, &Part<1>)>()
        .map(|(_, a, b)| {
            assert_eq!(a.0, b.0);
            a.0
        })
        .collect();
    values.sort_unstable();
    let expected: Vec<u32> = (0..32).chain(100..132).collect();
    assert_eq!(values, expected);
}

#[test]
fn flush_keeps_values_of_one_type_in_step_past_a_dead_target() {
    let mut world = World::new();
    let first = world.spawn();
    let dead = world.spawn();
    let last = world.spawn();
    let mut buffer = CommandBuffer::new();

    buffer.insert(first, Part::<0>(1));
    buffer.despawn(dead);
    buffer.insert(dead, Part::<0>(2));
    buffer.insert(dead, Part::<1>(20));
    buffer.insert(last, Part::<0>(3));
    buffer.insert(last, Part::<1>(30));
    world.flush(buffer);

    assert_eq!(world.get::<Part<0>>(first), Some(&Part(1)));
    assert!(!world.is_alive(dead));
    assert_eq!(world.get::<Part<0>>(last), Some(&Part(3)));
    assert_eq!(world.get::<Part<1>>(last), Some(&Part(30)));
}

#[test]
fn unflushed_buffer_drops_its_values() {
    use std::rc::Rc;

    struct Held(#[allow(dead_code)] Rc<()>);

    let tracker = Rc::new(());
    let mut world = World::new();
    let entity = world.spawn();
    let mut buffer = CommandBuffer::new();
    buffer.insert(entity, Held(tracker.clone()));
    buffer.insert(entity, Held(tracker.clone()));
    assert_eq!(Rc::strong_count(&tracker), 3);

    drop(buffer);

    assert_eq!(Rc::strong_count(&tracker), 1);
}

/// Parts on overlapping residues of 48 entities: part `k` on every row but
/// one in `k + 2`, so each arity shape below matches several archetypes and
/// each optional part is present in some and absent in others.
fn arity_world() -> World {
    let mut world = World::new();
    for i in 0..48u32 {
        let entity = world.spawn();
        for k in 0..PARTS {
            if !(i + k as u32).is_multiple_of(k as u32 + 2) {
                with_part!(k, P => world.insert(entity, P::of(i * 10 + k as u32)));
            }
        }
    }
    world
}

type P0 = Part<0>;
type P1 = Part<1>;
type P2 = Part<2>;
type P3 = Part<3>;
type P4 = Part<4>;
type P5 = Part<5>;

/// The deprecation notes' claim: each arity-named read is its tuple form
/// row for row, in the same order.
#[test]
#[allow(deprecated)]
fn tuple_forms_read_every_arity_shape_row_for_row() {
    let world = arity_world();

    let tuple: Vec<Entity> = world.query_filtered::<Entity, With<P0>>().collect();
    assert_eq!(tuple, world.query_entities::<P0>());
    let tuple: Vec<Entity> = world
        .query_filtered::<Entity, (With<P0>, With<P1>)>()
        .collect();
    assert_eq!(tuple, world.query2_entities::<P0, P1>());
    let tuple: Vec<Entity> = world
        .query_filtered::<Entity, (With<P0>, With<P1>, With<P2>)>()
        .collect();
    assert_eq!(tuple, world.query3_entities::<P0, P1, P2>());
    assert!(!tuple.is_empty());

    let arity: Vec<_> = world
        .query2::<P0, P1>()
        .map(|(e, a, b)| (e, *a, *b))
        .collect();
    let tuple: Vec<_> = world
        .query::<(Entity, &P0, &P1)>()
        .map(|(e, a, b)| (e, *a, *b))
        .collect();
    assert_eq!(tuple, arity);

    let arity: Vec<_> = world
        .query3::<P0, P1, P2>()
        .map(|(e, a, b, c)| (e, *a, *b, *c))
        .collect();
    let tuple: Vec<_> = world
        .query::<(Entity, &P0, &P1, &P2)>()
        .map(|(e, a, b, c)| (e, *a, *b, *c))
        .collect();
    assert_eq!(tuple, arity);

    let arity: Vec<_> = world
        .query2_opt2::<P0, P1, P3, P4>()
        .map(|(e, a, b, c, d)| (e, *a, *b, c.copied(), d.copied()))
        .collect();
    let tuple: Vec<_> = world
        .query::<(Entity, &P0, &P1, Option<&P3>, Option<&P4>)>()
        .map(|(e, a, b, c, d)| (e, *a, *b, c.copied(), d.copied()))
        .collect();
    assert_eq!(tuple, arity);
    assert!(tuple.iter().any(|row| row.3.is_none()));
    assert!(tuple.iter().any(|row| row.3.is_some()));

    let arity: Vec<_> = world
        .query3_opt2::<P0, P1, P2, P3, P4>()
        .map(|(e, a, b, c, d, f)| (e, *a, *b, *c, d.copied(), f.copied()))
        .collect();
    let tuple: Vec<_> = world
        .query::<(Entity, &P0, &P1, &P2, Option<&P3>, Option<&P4>)>()
        .map(|(e, a, b, c, d, f)| (e, *a, *b, *c, d.copied(), f.copied()))
        .collect();
    assert_eq!(tuple, arity);
}

/// The mutable arity-named queries and their tuple forms, the same body on
/// two copies of one world, leave the copies equal.
#[test]
#[allow(deprecated)]
fn tuple_forms_write_every_arity_shape_row_for_row() {
    let mut arity = arity_world();
    let mut tuple = arity_world();

    let pair = |e: Entity, a: &mut P0, b: &mut P1| {
        a.0 += e.id();
        b.0 += a.0;
    };
    for (e, a, b) in arity.query2_mut::<P0, P1>() {
        pair(e, a, b);
    }
    for (e, a, b) in tuple.query_mut::<(Entity, &mut P0, &mut P1)>() {
        pair(e, a, b);
    }

    let triple = |e: Entity, a: &mut P0, b: &mut P1, c: &mut P2| {
        c.0 += (a.0 ^ b.0) + e.id();
        a.0 += 1;
    };
    for (e, a, b, c) in arity.query3_mut::<P0, P1, P2>() {
        triple(e, a, b, c);
    }
    for (e, a, b, c) in tuple.query_mut::<(Entity, &mut P0, &mut P1, &mut P2)>() {
        triple(e, a, b, c);
    }

    let optional = |e: Entity, a: &P0, b: &mut P1, c: Option<&P3>, d: Option<&mut P4>| {
        b.0 += a.0 + c.map_or(7, |c| c.0);
        if let Some(d) = d {
            d.0 += e.id();
        }
    };
    for (e, a, b, c, d) in arity.query2_opt2_mut::<P0, P1, P3, P4>() {
        optional(e, a, b, c, d);
    }
    for (e, a, b, c, d) in tuple.query_mut::<(Entity, &P0, &mut P1, Option<&P3>, Option<&mut P4>)>()
    {
        optional(e, a, b, c, d);
    }

    let mut excluded = 0;
    for (e, a, b, c) in arity.query3_mut_without::<P0, P1, P2, P5>() {
        triple(e, a, b, c);
        excluded += 1;
    }
    for (e, a, b, c) in
        tuple.query_mut_filtered::<(Entity, &mut P0, &mut P1, &mut P2), Without<P5>>()
    {
        triple(e, a, b, c);
    }
    assert!(excluded > 0);

    assert_eq!(snapshot(&tuple), snapshot(&arity));
}

#[test]
fn the_fixed_event_view_reaches_every_registered_queue() {
    let mut world = World::new();
    world.register_event::<i32>();
    world.register_event::<u8>();
    world.get_resource_mut::<EventQueue<i32>>().unwrap().send(1);
    world.get_resource_mut::<EventQueue<u8>>().unwrap().send(1);

    world.set_fixed_event_view(true);
    world.end_fixed_step_events();
    world.get_resource_mut::<EventQueue<u8>>().unwrap().send(2);
    assert_eq!(world.get_resource::<EventQueue<i32>>().unwrap().len(), 0);
    assert_eq!(
        world
            .get_resource::<EventQueue<u8>>()
            .unwrap()
            .iter_current()
            .copied()
            .collect::<Vec<_>>(),
        [2]
    );

    world.set_fixed_event_view(false);
    assert_eq!(world.get_resource::<EventQueue<i32>>().unwrap().len(), 1);
    assert_eq!(world.get_resource::<EventQueue<u8>>().unwrap().len(), 2);

    world.flush_events();
    world.set_fixed_event_view(true);
    assert_eq!(
        world.get_resource::<EventQueue<u8>>().unwrap().len(),
        2,
        "the next frame's first step reads the previous frame"
    );
}

#[test]
fn the_fixed_step_hold_reaches_every_registered_queue() {
    let mut world = World::new();
    world.register_event::<i32>();
    world.register_event::<u8>();
    world.get_resource_mut::<EventQueue<i32>>().unwrap().send(1);
    world.get_resource_mut::<EventQueue<u8>>().unwrap().send(1);
    world.flush_events();
    world.hold_events_for_fixed_step();
    world.flush_events();

    world.set_fixed_event_view(true);
    assert_eq!(world.get_resource::<EventQueue<i32>>().unwrap().len(), 1);
    assert_eq!(world.get_resource::<EventQueue<u8>>().unwrap().len(), 1);
    world.set_fixed_event_view(false);
    assert!(
        world.get_resource::<EventQueue<i32>>().unwrap().is_empty(),
        "the frame view has rotated past it"
    );
}
