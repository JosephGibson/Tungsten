use super::*;

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

// ------------------------------------------------------------------
// Basic lifecycle
// ------------------------------------------------------------------

#[test]
fn spawn_is_alive_in_empty_archetype() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    assert!(store.entities.is_alive(e));
    let loc = store.entities.get(e).unwrap();
    assert_eq!(loc.archetype_id, EMPTY_ARCHETYPE);
}

#[test]
fn despawn_frees_entity() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.despawn(e);
    assert!(!store.entities.is_alive(e));
}

#[test]
#[should_panic(expected = "despawn: entity is not alive")]
fn despawn_dead_entity_panics() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.despawn(e);
    store.despawn(e); // second despawn should panic
}

// ------------------------------------------------------------------
// Insert transitions
// ------------------------------------------------------------------

#[test]
fn insert_first_component_moves_to_single_type_archetype() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 1.0, y: 2.0 });

    let loc = store.entities.get(e).unwrap();
    assert_ne!(loc.archetype_id, EMPTY_ARCHETYPE);
    assert_eq!(
        store.get::<Position>(e).unwrap(),
        &Position { x: 1.0, y: 2.0 }
    );
}

#[test]
fn insert_second_component_moves_to_two_type_archetype() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 1.0, y: 2.0 });
    store.insert(e, Velocity { dx: 3.0, dy: 4.0 });

    assert_eq!(
        store.get::<Position>(e).unwrap(),
        &Position { x: 1.0, y: 2.0 }
    );
    assert_eq!(
        store.get::<Velocity>(e).unwrap(),
        &Velocity { dx: 3.0, dy: 4.0 }
    );

    // Both types must be in the same archetype.
    let loc = store.entities.get(e).unwrap();
    let arch = &store.archetypes[loc.archetype_id as usize];
    assert!(arch.has(TypeId::of::<Position>()));
    assert!(arch.has(TypeId::of::<Velocity>()));
}

#[test]
fn insert_overwrites_existing_component_in_place() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 1.0, y: 2.0 });
    let arch_before = store.entities.get(e).unwrap().archetype_id;
    store.insert(e, Position { x: 9.0, y: 9.0 }); // overwrite
    let arch_after = store.entities.get(e).unwrap().archetype_id;

    // No archetype transition should have happened.
    assert_eq!(arch_before, arch_after);
    assert_eq!(
        store.get::<Position>(e).unwrap(),
        &Position { x: 9.0, y: 9.0 }
    );
}

// ------------------------------------------------------------------
// Remove transitions
// ------------------------------------------------------------------

#[test]
fn remove_component_moves_back() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 1.0, y: 2.0 });
    store.insert(e, Velocity { dx: 3.0, dy: 4.0 });

    let removed = store.remove::<Velocity>(e);
    assert_eq!(removed, Some(Velocity { dx: 3.0, dy: 4.0 }));
    assert!(store.has::<Position>(e));
    assert!(!store.has::<Velocity>(e));
}

#[test]
fn remove_last_component_returns_to_empty_archetype() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 0.0, y: 0.0 });
    store.remove::<Position>(e);

    let loc = store.entities.get(e).unwrap();
    assert_eq!(loc.archetype_id, EMPTY_ARCHETYPE);
    assert!(!store.has::<Position>(e));
}

#[test]
fn remove_absent_component_returns_none() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 0.0, y: 0.0 });
    let result = store.remove::<Velocity>(e);
    assert!(result.is_none());
}

// ------------------------------------------------------------------
// get / get_mut / has
// ------------------------------------------------------------------

#[test]
fn get_absent_component_returns_none() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    assert!(store.get::<Position>(e).is_none());
}

#[test]
fn get_mut_modifies_value() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 0.0, y: 0.0 });
    store.get_mut::<Position>(e).unwrap().x = 99.0;
    assert_eq!(store.get::<Position>(e).unwrap().x, 99.0);
}

#[test]
fn get_on_dead_entity_returns_none() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 1.0, y: 2.0 });
    store.despawn(e);
    assert!(store.get::<Position>(e).is_none());
}

// ------------------------------------------------------------------
// Displaced entity bookkeeping
// ------------------------------------------------------------------

#[test]
fn displaced_entity_location_updated_after_despawn() {
    let mut store = Archetypes::new();
    let e0 = store.spawn();
    let e1 = store.spawn();
    let e2 = store.spawn();
    store.insert(e0, Position { x: 0.0, y: 0.0 });
    store.insert(e1, Position { x: 1.0, y: 1.0 });
    store.insert(e2, Position { x: 2.0, y: 2.0 });

    // e0, e1, e2 should all be in the same archetype, rows 0, 1, 2.
    store.despawn(e1); // e2 displaces into row 1

    // e2 should still be accessible at its new location.
    assert_eq!(
        store.get::<Position>(e2).unwrap(),
        &Position { x: 2.0, y: 2.0 }
    );
    // e1 is dead.
    assert!(!store.entities.is_alive(e1));
}

#[test]
fn displaced_entity_location_updated_after_insert_transition() {
    let mut store = Archetypes::new();
    let e0 = store.spawn();
    let e1 = store.spawn();

    // Both entities get Position first → same archetype, rows 0, 1.
    store.insert(e0, Position { x: 0.0, y: 0.0 });
    store.insert(e1, Position { x: 1.0, y: 1.0 });

    // e0 transitions to a new archetype (adds Velocity). e1 displaces to row 0
    // in the Position-only archetype.
    store.insert(e0, Velocity { dx: 5.0, dy: 0.0 });

    assert_eq!(
        store.get::<Position>(e1).unwrap(),
        &Position { x: 1.0, y: 1.0 }
    );
    assert_eq!(
        store.get::<Position>(e0).unwrap(),
        &Position { x: 0.0, y: 0.0 }
    );
    assert_eq!(
        store.get::<Velocity>(e0).unwrap(),
        &Velocity { dx: 5.0, dy: 0.0 }
    );
}

// ------------------------------------------------------------------
// Stale handle safety
// ------------------------------------------------------------------

#[test]
fn stale_handle_after_despawn_and_respawn_does_not_alias() {
    let mut store = Archetypes::new();
    let old = store.spawn();
    store.insert(old, Position { x: 1.0, y: 2.0 });
    store.despawn(old);

    let new_e = store.spawn(); // reuses the slot with bumped generation
    assert_eq!(new_e.index, old.index);
    assert_ne!(new_e.generation, old.generation);

    store.insert(new_e, Position { x: 99.0, y: 0.0 });

    // Old handle must not see the new entity's data.
    assert!(store.get::<Position>(old).is_none());
    assert_eq!(store.get::<Position>(new_e).unwrap().x, 99.0);
}

// ------------------------------------------------------------------
// Archetype edge caching
// ------------------------------------------------------------------

#[test]
fn add_edge_cached_on_second_transition() {
    let mut store = Archetypes::new();
    let e0 = store.spawn();
    let e1 = store.spawn();

    store.insert(e0, Position { x: 0.0, y: 0.0 });
    // After this, add_edge for Position should be cached on empty archetype.
    store.insert(e1, Position { x: 1.0, y: 1.0 });

    assert_eq!(store.get::<Position>(e0).unwrap().x, 0.0);
    assert_eq!(store.get::<Position>(e1).unwrap().x, 1.0);

    // Both should be in the same archetype (same edge should have been used).
    let loc0 = store.entities.get(e0).unwrap();
    let loc1 = store.entities.get(e1).unwrap();
    assert_eq!(loc0.archetype_id, loc1.archetype_id);
}

// ------------------------------------------------------------------
// insert on dead entity
// ------------------------------------------------------------------

#[test]
#[should_panic(expected = "insert on dead entity")]
fn insert_on_dead_entity_panics() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.despawn(e);
    store.insert(e, Position { x: 0.0, y: 0.0 });
}

// ------------------------------------------------------------------
// archetypes_with queries
// ------------------------------------------------------------------

#[test]
fn archetypes_with_returns_supersets() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 0.0, y: 0.0 });
    store.insert(e, Velocity { dx: 1.0, dy: 0.0 });
    store.insert(e, Name("player".into()));

    // Archetypes containing Position (superset of {Position}).
    let count = store.archetypes_with::<Position>().count();
    assert!(count >= 1);

    let two_count = store
        .archetypes_with_two(TypeId::of::<Position>(), TypeId::of::<Velocity>())
        .count();
    assert!(two_count >= 1);
}

#[test]
fn archetypes_with_excludes_missing_type() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 0.0, y: 0.0 });
    // No Velocity inserted.

    let count = store
        .archetypes_with_two(TypeId::of::<Position>(), TypeId::of::<Velocity>())
        .count();
    assert_eq!(count, 0);
}
