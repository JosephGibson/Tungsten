use std::any::TypeId;
use std::collections::HashMap;

use super::archetype::{Archetype, ArchetypeId, TypedVec, EMPTY_ARCHETYPE};
use super::entity::{Entities, Entity, EntityLocation};

// ---------------------------------------------------------------------------
// Archetypes registry
// ---------------------------------------------------------------------------

/// The central store for all entity and component data.
///
/// Owns the `Entities` allocation table and the `Vec<Archetype>` list.
/// ArchetypeId is an index into `archetypes`; archetype 0 is the empty
/// archetype that all freshly spawned entities live in.
pub(crate) struct Archetypes {
    pub archetypes: Vec<Archetype>,
    /// Sorted `Box<[TypeId]>` → ArchetypeId for O(1) archetype lookup.
    /// TypeId: Ord is stable since Rust 1.86 (D-036).
    index: HashMap<Box<[TypeId]>, ArchetypeId>,
    pub entities: Entities,
}

impl Archetypes {
    pub fn new() -> Self {
        let empty = Archetype::new(EMPTY_ARCHETYPE, Box::new([]));
        let mut index = HashMap::new();
        index.insert(Box::new([]) as Box<[TypeId]>, EMPTY_ARCHETYPE);
        Self {
            archetypes: vec![empty],
            index,
            entities: Entities::new(),
        }
    }

    // ------------------------------------------------------------------
    // Archetype lookup / creation
    // ------------------------------------------------------------------

    /// Find the archetype for `types` (must already be sorted), or create it.
    pub fn find_or_create(&mut self, types: &[TypeId]) -> ArchetypeId {
        if let Some(&id) = self.index.get(types) {
            return id;
        }
        let id = self.archetypes.len() as ArchetypeId;
        let arch = Archetype::new(id, types.into());
        self.archetypes.push(arch);
        self.index.insert(types.into(), id);
        id
    }

    // ------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------

    /// Spawn a new entity. Starts in the empty archetype (no components).
    pub fn spawn(&mut self) -> Entity {
        let entity = self.entities.alloc();
        let row = self.archetypes[EMPTY_ARCHETYPE as usize].entities.len() as u32;
        self.archetypes[EMPTY_ARCHETYPE as usize]
            .entities
            .push(entity);
        self.entities.set_location(
            entity,
            EntityLocation {
                archetype_id: EMPTY_ARCHETYPE,
                row,
            },
        );
        entity
    }

    /// Despawn an entity, removing it from its archetype.
    ///
    /// Panics if the entity is already dead.
    pub fn despawn(&mut self, entity: Entity) {
        let loc = self
            .entities
            .get(entity)
            .expect("despawn: entity is not alive");

        let arch = &mut self.archetypes[loc.archetype_id as usize];
        let displaced = arch.swap_remove_row(loc.row as usize);

        if let Some(displaced_entity) = displaced {
            self.entities.set_location(
                displaced_entity,
                EntityLocation {
                    archetype_id: loc.archetype_id,
                    row: loc.row,
                },
            );
        }

        self.entities.free(entity);
    }

    // ------------------------------------------------------------------
    // Component access
    // ------------------------------------------------------------------

    /// Insert (or overwrite) component `T` on `entity`.
    ///
    /// If the entity already has `T`, the value is overwritten in-place (no
    /// archetype transition). Otherwise the entity moves to a new archetype
    /// that includes `T`.
    ///
    /// Panics if the entity is dead (D-022).
    pub fn insert<T: 'static>(&mut self, entity: Entity, value: T) {
        assert!(
            self.entities.is_alive(entity),
            "insert on dead entity {entity}"
        );

        let loc = self.entities.get(entity).unwrap();
        let old_arch_id = loc.archetype_id;
        let row = loc.row as usize;

        let t_id = TypeId::of::<T>();

        // Case 1: entity already has T — overwrite in place, no transition.
        if self.archetypes[old_arch_id as usize].has(t_id) {
            *self.archetypes[old_arch_id as usize]
                .columns
                .get_mut(&t_id)
                .unwrap()
                .get_mut_erased(row)
                .downcast_mut::<T>()
                .unwrap() = value;
            return;
        }

        // Case 2: entity doesn't have T — compute new type set and transition.
        let new_arch_id = {
            // Check the lazy edge cache first.
            if let Some(&cached) = self.archetypes[old_arch_id as usize].add_edges.get(&t_id) {
                cached
            } else {
                let mut new_types: Vec<TypeId> = self.archetypes[old_arch_id as usize]
                    .component_types
                    .to_vec();
                new_types.push(t_id);
                new_types.sort();
                let id = self.find_or_create(&new_types);
                self.archetypes[old_arch_id as usize]
                    .add_edges
                    .insert(t_id, id);
                id
            }
        };

        // Split the borrow: we need &mut on both old and new archetypes.
        // Use indices to avoid simultaneous mutable refs into the same Vec.
        //
        // Safety / soundness: old_arch_id != new_arch_id (T was absent from
        // the old archetype, so they can't be the same archetype).
        debug_assert_ne!(old_arch_id, new_arch_id);

        // Move existing components from old → new archetype.
        // We use split_at_mut to hold two mutable references into archetypes.
        let (old_arch, new_arch) = split_two_mut(&mut self.archetypes, old_arch_id, new_arch_id);
        old_arch.move_components_to(row, new_arch);

        // Push T into the new archetype. Create the column if this is the
        // first entity of this type to enter the archetype.
        new_arch
            .columns
            .entry(t_id)
            .or_insert_with(|| Box::new(TypedVec::<T>(Vec::new())))
            .push_erased(Box::new(value));

        // Add entity to new archetype's entity list.
        let new_row = new_arch.entities.len() as u32;
        new_arch.entities.push(entity);

        // Remove entity from old archetype's entity list (swap-remove).
        let last = self.archetypes[old_arch_id as usize]
            .entities
            .len()
            .saturating_sub(1);
        self.archetypes[old_arch_id as usize]
            .entities
            .swap_remove(row);

        // Update current entity's location to new archetype.
        self.entities.set_location(
            entity,
            EntityLocation {
                archetype_id: new_arch_id,
                row: new_row,
            },
        );

        // If swap-remove displaced an entity, update its location.
        if row < last {
            let displaced = self.archetypes[old_arch_id as usize].entities[row];
            self.entities.set_location(
                displaced,
                EntityLocation {
                    archetype_id: old_arch_id,
                    row: row as u32,
                },
            );
        }
    }

    /// Remove component `T` from `entity`. Returns the removed value, or
    /// `None` if the entity doesn't have `T`.
    ///
    /// The entity moves to the archetype without `T`. If this was the only
    /// component, the entity moves back to the empty archetype.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        let loc = self.entities.get(entity)?;
        let old_arch_id = loc.archetype_id;
        let row = loc.row as usize;

        let t_id = TypeId::of::<T>();
        if !self.archetypes[old_arch_id as usize].has(t_id) {
            return None;
        }

        // Compute the destination archetype (current types minus T).
        let new_arch_id = {
            if let Some(&cached) = self.archetypes[old_arch_id as usize]
                .remove_edges
                .get(&t_id)
            {
                cached
            } else {
                let new_types: Vec<TypeId> = self.archetypes[old_arch_id as usize]
                    .component_types
                    .iter()
                    .copied()
                    .filter(|&tid| tid != t_id)
                    .collect();
                // new_types is already sorted (we filtered a sorted slice).
                let id = self.find_or_create(&new_types);
                self.archetypes[old_arch_id as usize]
                    .remove_edges
                    .insert(t_id, id);
                id
            }
        };

        // Move all columns except T from old → new archetype.
        let (old_arch, new_arch) = split_two_mut(&mut self.archetypes, old_arch_id, new_arch_id);
        old_arch.move_components_to(row, new_arch);

        // Extract T's value from the source (it was skipped by move_components_to).
        let t_boxed = self.archetypes[old_arch_id as usize]
            .columns
            .get_mut(&t_id)
            .unwrap()
            .swap_remove_erased(row);
        let t_value = *t_boxed.downcast::<T>().unwrap();

        // Add entity to new archetype's entity list.
        let new_row = self.archetypes[new_arch_id as usize].entities.len() as u32;
        self.archetypes[new_arch_id as usize].entities.push(entity);

        // Swap-remove entity from old archetype's entity list.
        let last = self.archetypes[old_arch_id as usize]
            .entities
            .len()
            .saturating_sub(1);
        self.archetypes[old_arch_id as usize]
            .entities
            .swap_remove(row);

        // Update current entity's location.
        self.entities.set_location(
            entity,
            EntityLocation {
                archetype_id: new_arch_id,
                row: new_row,
            },
        );

        // Update displaced entity's location if swap-remove moved one.
        if row < last {
            let displaced = self.archetypes[old_arch_id as usize].entities[row];
            self.entities.set_location(
                displaced,
                EntityLocation {
                    archetype_id: old_arch_id,
                    row: row as u32,
                },
            );
        }

        Some(t_value)
    }

    // ------------------------------------------------------------------
    // Single-entity component access
    // ------------------------------------------------------------------

    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        let loc = self.entities.get(entity)?;
        let arch = &self.archetypes[loc.archetype_id as usize];
        arch.columns
            .get(&TypeId::of::<T>())?
            .get_erased(loc.row as usize)
            .downcast_ref::<T>()
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        let loc = self.entities.get(entity)?;
        let arch = &mut self.archetypes[loc.archetype_id as usize];
        arch.columns
            .get_mut(&TypeId::of::<T>())?
            .get_mut_erased(loc.row as usize)
            .downcast_mut::<T>()
    }

    pub fn has<T: 'static>(&self, entity: Entity) -> bool {
        let Some(loc) = self.entities.get(entity) else {
            return false;
        };
        self.archetypes[loc.archetype_id as usize].has(TypeId::of::<T>())
    }

    // ------------------------------------------------------------------
    // Archetype-level iteration helpers (for queries)
    // ------------------------------------------------------------------

    /// Iterate over all archetypes that contain component type `T`.
    pub fn archetypes_with<T: 'static>(&self) -> impl Iterator<Item = &Archetype> {
        let t_id = TypeId::of::<T>();
        self.archetypes.iter().filter(move |a| a.has(t_id))
    }

    /// Iterate over all archetypes that contain both `a` and `b`.
    pub fn archetypes_with_two(&self, a: TypeId, b: TypeId) -> impl Iterator<Item = &Archetype> {
        self.archetypes
            .iter()
            .filter(move |arch| arch.has(a) && arch.has(b))
    }

    /// Iterate over all archetypes that contain `a`, `b`, and `c`.
    pub fn archetypes_with_three(
        &self,
        a: TypeId,
        b: TypeId,
        c: TypeId,
    ) -> impl Iterator<Item = &Archetype> {
        self.archetypes
            .iter()
            .filter(move |arch| arch.has(a) && arch.has(b) && arch.has(c))
    }
}

// ---------------------------------------------------------------------------
// Helper: split a Vec into two mutable references by index
// ---------------------------------------------------------------------------

/// Return mutable references to two distinct elements in a slice.
///
/// Panics if `a == b`.
fn split_two_mut(
    archetypes: &mut [Archetype],
    a: ArchetypeId,
    b: ArchetypeId,
) -> (&mut Archetype, &mut Archetype) {
    let (a, b) = (a as usize, b as usize);
    assert_ne!(a, b, "split_two_mut: indices must differ");
    if a < b {
        let (left, right) = archetypes.split_at_mut(b);
        (&mut left[a], &mut right[0])
    } else {
        let (left, right) = archetypes.split_at_mut(a);
        (&mut right[0], &mut left[b])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
}
