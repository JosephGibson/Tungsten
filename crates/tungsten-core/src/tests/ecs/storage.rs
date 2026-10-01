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
    store.despawn(e);
}

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
    store.insert(e, Position { x: 9.0, y: 9.0 });
    let arch_after = store.entities.get(e).unwrap().archetype_id;

    assert_eq!(arch_before, arch_after);
    assert_eq!(
        store.get::<Position>(e).unwrap(),
        &Position { x: 9.0, y: 9.0 }
    );
}

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

#[test]
fn displaced_entity_location_updated_after_despawn() {
    let mut store = Archetypes::new();
    let e0 = store.spawn();
    let e1 = store.spawn();
    let e2 = store.spawn();
    store.insert(e0, Position { x: 0.0, y: 0.0 });
    store.insert(e1, Position { x: 1.0, y: 1.0 });
    store.insert(e2, Position { x: 2.0, y: 2.0 });

    store.despawn(e1);

    assert_eq!(
        store.get::<Position>(e2).unwrap(),
        &Position { x: 2.0, y: 2.0 }
    );
    assert!(!store.entities.is_alive(e1));
}

#[test]
fn displaced_entity_location_updated_after_insert_transition() {
    let mut store = Archetypes::new();
    let e0 = store.spawn();
    let e1 = store.spawn();

    store.insert(e0, Position { x: 0.0, y: 0.0 });
    store.insert(e1, Position { x: 1.0, y: 1.0 });

    // Transition displaces e1 in source archetype.
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

#[test]
fn stale_handle_after_despawn_and_respawn_does_not_alias() {
    let mut store = Archetypes::new();
    let old = store.spawn();
    store.insert(old, Position { x: 1.0, y: 2.0 });
    store.despawn(old);

    let new_e = store.spawn();
    assert_eq!(new_e.index, old.index);
    assert_ne!(new_e.generation, old.generation);

    store.insert(new_e, Position { x: 99.0, y: 0.0 });

    assert!(store.get::<Position>(old).is_none());
    assert_eq!(store.get::<Position>(new_e).unwrap().x, 99.0);
}

#[test]
fn add_edge_cached_on_second_transition() {
    let mut store = Archetypes::new();
    let e0 = store.spawn();
    let e1 = store.spawn();

    store.insert(e0, Position { x: 0.0, y: 0.0 });
    store.insert(e1, Position { x: 1.0, y: 1.0 });

    assert_eq!(store.get::<Position>(e0).unwrap().x, 0.0);
    assert_eq!(store.get::<Position>(e1).unwrap().x, 1.0);

    let loc0 = store.entities.get(e0).unwrap();
    let loc1 = store.entities.get(e1).unwrap();
    assert_eq!(loc0.archetype_id, loc1.archetype_id);
}

#[test]
#[should_panic(expected = "insert on dead entity")]
fn insert_on_dead_entity_panics() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.despawn(e);
    store.insert(e, Position { x: 0.0, y: 0.0 });
}

#[test]
fn archetypes_with_returns_supersets() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 0.0, y: 0.0 });
    store.insert(e, Velocity { dx: 1.0, dy: 0.0 });
    store.insert(e, Name("player".into()));
    // {P}, {P, V} and {P, V, N} hold `Position`, each at its key position.
    let with_position: Vec<_> = store.archetypes_with::<Position>().collect();
    assert_eq!(with_position.len(), 3);
    for (arch, index) in with_position {
        assert_eq!(arch.component_types[index], TypeId::of::<Position>());
        assert!(arch.columns[index].typed::<Position>().is_some());
    }
    assert_eq!(store.archetypes_with_mut::<Name>().count(), 1);

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

    let count = store
        .archetypes_with_two(TypeId::of::<Position>(), TypeId::of::<Velocity>())
        .count();
    assert_eq!(count, 0);
}

#[test]
fn archetype_columns_are_created_with_the_archetype_in_key_order() {
    let mut store = Archetypes::new();
    let e = store.spawn();
    store.insert(e, Position { x: 1.0, y: 2.0 });
    store.insert(e, Velocity { dx: 3.0, dy: 4.0 });
    store.insert(e, Name("named".into()));
    store.remove::<Velocity>(e);

    // Empty, P, PV, PVN and PN: every archetype on the path keeps its columns.
    assert_eq!(store.archetypes.len(), 5);
    for arch in &store.archetypes {
        assert!(arch.component_types.is_sorted());
        assert_eq!(arch.columns.len(), arch.component_types.len());
        // Each column stores the type its key names, one value per row.
        for (&type_id, column) in arch.component_types.iter().zip(&arch.columns) {
            let rows = if type_id == TypeId::of::<Position>() {
                column.typed::<Position>().map(|column| column.0.len())
            } else if type_id == TypeId::of::<Velocity>() {
                column.typed::<Velocity>().map(|column| column.0.len())
            } else {
                column.typed::<Name>().map(|column| column.0.len())
            };
            assert_eq!(rows, Some(arch.entities.len()));
        }
    }
    assert_eq!(
        store.get::<Position>(e).unwrap(),
        &Position { x: 1.0, y: 2.0 }
    );
    assert_eq!(store.get::<Name>(e).unwrap(), &Name("named".into()));
}

#[test]
fn archetype_ids_follow_first_transition_order() {
    let mut store = Archetypes::new();
    let archetype_of =
        |store: &Archetypes, entity| store.entities.get(entity).unwrap().archetype_id;

    let a = store.spawn();
    store.insert(a, Position { x: 0.0, y: 0.0 });
    assert_eq!(archetype_of(&store, a), 1);
    store.insert(a, Velocity { dx: 0.0, dy: 0.0 });
    assert_eq!(archetype_of(&store, a), 2);

    // The same component set reached along another edge is the same archetype.
    let b = store.spawn();
    store.insert(b, Velocity { dx: 1.0, dy: 0.0 });
    assert_eq!(archetype_of(&store, b), 3);
    store.insert(b, Position { x: 1.0, y: 0.0 });
    assert_eq!(archetype_of(&store, b), 2);

    store.remove::<Position>(a);
    assert_eq!(archetype_of(&store, a), 3);
    assert_eq!(store.archetypes.len(), 4);
}

#[test]
fn remove_returns_the_value_and_keeps_the_other_components() {
    let mut store = Archetypes::new();
    let e0 = store.spawn();
    let e1 = store.spawn();
    for (entity, tag) in [(e0, "zero"), (e1, "one")] {
        store.insert(entity, Position { x: 1.0, y: 2.0 });
        store.insert(entity, Name(tag.into()));
        store.insert(entity, Velocity { dx: 3.0, dy: 4.0 });
    }

    assert_eq!(store.remove::<Name>(e0), Some(Name("zero".into())));

    assert!(!store.has::<Name>(e0));
    assert_eq!(
        store.get::<Position>(e0).unwrap(),
        &Position { x: 1.0, y: 2.0 }
    );
    assert_eq!(
        store.get::<Velocity>(e0).unwrap(),
        &Velocity { dx: 3.0, dy: 4.0 }
    );
    // The displaced row keeps its own values.
    assert_eq!(store.get::<Name>(e1).unwrap(), &Name("one".into()));
}

#[test]
fn get_finds_the_column_and_row_in_every_archetype() {
    let mut store = Archetypes::new();
    let mut entities = Vec::new();
    for i in 0..9u32 {
        let e = store.spawn();
        // Three archetypes hold `Position`, at a different column index each.
        if i % 3 >= 1 {
            store.insert(
                e,
                Velocity {
                    dx: i as f32,
                    dy: 0.0,
                },
            );
        }
        store.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        if i % 3 == 2 {
            store.insert(e, Name(format!("{i}")));
        }
        entities.push(e);
    }
    // Displace a row in each archetype.
    for &i in &[0usize, 1, 2] {
        store.despawn(entities[i]);
    }

    for (i, &e) in entities.iter().enumerate() {
        if i < 3 {
            assert!(store.get::<Position>(e).is_none());
            assert!(store.get_mut::<Position>(e).is_none());
            continue;
        }
        assert_eq!(store.get::<Position>(e).unwrap().x, i as f32);
        assert_eq!(store.get_mut::<Position>(e).unwrap().x, i as f32);
        assert_eq!(
            store.get::<Velocity>(e).map(|velocity| velocity.dx),
            (i % 3 >= 1).then_some(i as f32)
        );
        assert_eq!(
            store.get::<Name>(e).map(|name| name.0.clone()),
            (i % 3 == 2).then(|| format!("{i}"))
        );
    }
}

#[test]
fn get_agrees_with_has_for_every_entity_and_type() {
    #[derive(Debug, PartialEq)]
    struct Part<const K: usize>(u32);

    macro_rules! parts {
        ($callback:ident) => {
            $callback!(0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23);
        };
    }

    // Entities over random subsets of 24 types: wide keys in many archetypes.
    let mut store = Archetypes::new();
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut expected = Vec::new();
    for entity_index in 0..300u32 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let mask = state as u32 & (state >> 32) as u32 | 1 << (entity_index % 24);
        let e = store.spawn();
        macro_rules! insert {
            ($($k:literal)*) => {
                $(
                    if mask & (1 << $k) != 0 {
                        store.insert(e, Part::<$k>(entity_index * 100 + $k));
                    }
                )*
            };
        }
        parts!(insert);
        expected.push((e, entity_index, mask));
    }
    assert!(store.archetypes.len() > 100);

    for &(e, entity_index, mask) in &expected {
        macro_rules! check {
            ($($k:literal)*) => {
                $(
                    let value = (mask & (1 << $k) != 0).then_some(Part::<$k>(entity_index * 100 + $k));
                    assert_eq!(store.has::<Part<$k>>(e), value.is_some());
                    assert_eq!(store.get::<Part<$k>>(e), value.as_ref());
                    assert_eq!(store.get_mut::<Part<$k>>(e).map(|part| part.0), value.map(|part| part.0));
                )*
            };
        }
        parts!(check);
    }
}
