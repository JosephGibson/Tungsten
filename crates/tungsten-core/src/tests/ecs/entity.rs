use super::*;

#[test]
fn alloc_gives_distinct_entities() {
    let mut e = Entities::new();
    let a = e.alloc();
    let b = e.alloc();
    assert_ne!(a, b);
    assert_eq!(a.index, 0);
    assert_eq!(b.index, 1);
}

#[test]
fn alloc_free_reuses_index_with_bumped_generation() {
    let mut e = Entities::new();
    let a = e.alloc();
    e.set_location(
        a,
        EntityLocation {
            archetype_id: 0,
            row: 0,
        },
    );
    e.free(a);

    let b = e.alloc();
    assert_eq!(b.index, a.index, "slot should be reused");
    assert_eq!(b.generation, a.generation + 1, "generation must be bumped");
}

#[test]
fn stale_handle_returns_none() {
    let mut e = Entities::new();
    let a = e.alloc();
    e.set_location(
        a,
        EntityLocation {
            archetype_id: 0,
            row: 0,
        },
    );
    e.free(a);
    let _b = e.alloc();

    assert!(e.get(a).is_none());
    assert!(!e.is_alive(a));
}

#[test]
fn is_alive_false_after_free() {
    let mut e = Entities::new();
    let a = e.alloc();
    e.set_location(
        a,
        EntityLocation {
            archetype_id: 0,
            row: 0,
        },
    );
    assert!(e.is_alive(a));
    e.free(a);
    assert!(!e.is_alive(a));
}

#[test]
fn is_alive_false_before_location_set() {
    let mut e = Entities::new();
    let a = e.alloc();
    assert!(!e.is_alive(a));
}

#[test]
fn set_and_get_location() {
    let mut e = Entities::new();
    let a = e.alloc();
    let loc = EntityLocation {
        archetype_id: 3,
        row: 7,
    };
    e.set_location(a, loc);
    assert_eq!(e.get(a), Some(loc));
    assert!(e.is_alive(a));
}

#[test]
fn free_list_order_is_lifo() {
    let mut e = Entities::new();
    let a = e.alloc();
    let b = e.alloc();
    e.set_location(
        a,
        EntityLocation {
            archetype_id: 0,
            row: 0,
        },
    );
    e.set_location(
        b,
        EntityLocation {
            archetype_id: 0,
            row: 1,
        },
    );
    e.free(b);
    e.free(a);

    let c = e.alloc();
    assert_eq!(c.index, a.index);
    assert_eq!(c.generation, 1);
}

#[test]
fn live_count_tracks_alloc_and_free() {
    let mut e = Entities::new();
    assert_eq!(e.live_count(), 0);
    let mut handles = Vec::new();
    for _ in 0..5 {
        let h = e.alloc();
        e.set_location(
            h,
            EntityLocation {
                archetype_id: 0,
                row: 0,
            },
        );
        handles.push(h);
    }
    assert_eq!(e.live_count(), 5);
    e.free(handles[0]);
    e.free(handles[1]);
    assert_eq!(e.live_count(), 3);
    let _ = e.alloc();
    assert_eq!(e.live_count(), 4);
}

#[test]
fn entity_id_returns_index() {
    let e = Entity {
        index: 42,
        generation: 7,
    };
    assert_eq!(e.id(), 42);
}

#[test]
fn entity_display_matches_pre_m12() {
    let e = Entity {
        index: 5,
        generation: 3,
    };
    assert_eq!(format!("{e}"), "Entity(5)");
}
