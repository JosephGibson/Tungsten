# Phase 2 — M12: Archetypal ECS Rewrite

**Status:** in progress  
**Branch:** `0.7`  
**Version:** `v0.7.0-alpha`  
**Goal:** Replace the naive `HashMap<TypeId, HashMap<EntityId, Box<dyn Any>>>` storage with an archetypal layout — contiguous per-archetype component arrays — plus generational entity IDs and multi-component tuple queries.  
**Non-goals:** parallel scheduling, change detection, command buffers, reactive queries, raw BlobVec columns (Bevy-style), mutable multi-component queries.

---

## Context

The naive ECS shipped in M2 was intentional (D-005, D-024): build it simple first, evolve on real pain. After M11, all Phase 2 systems (text, audio, hot reload, tilemaps, physics) are in place and provide a realistic workload to benchmark against. M12 is learning-motivated — the goal is understanding archetypal storage and cache-friendly iteration, not fixing a crisis.

**Prerequisite before writing any code:** log D-036 in `DECISIONS.md` confirming the decision to proceed and citing D-030.

**What changes:** the storage engine inside `tungsten-core::ecs`, plus two flagged scope expansions.  
**What stays the same:** every public `World` method signature, `entity.id() -> u32`, all 10 examples compile without modification.

---

## Prior Art — Recommended Model: hecs

| ECS | Language | Storage | Complexity | Notes |
|-----|----------|---------|------------|-------|
| **hecs** | Rust | `Box<dyn AnyVec>` (typed columns) | Low (~3 KLOC) | Clean archetypal, no unsafe, learning-appropriate |
| Bevy ECS | Rust | `BlobVec` (raw bytes) | Very high | Change detection, schedules, unsafe throughout |
| EnTT | C++ | Sparse-set | Medium | Not archetypal; wrong model |
| flecs | C | Archetypal | Very high | Too complex for scope |
| legion | Rust | Archetypal | Medium-high | Unsafe-heavy, less clear |

**Model after hecs.** Reasons:
1. Rust-native archetypal without raw-byte columns — teaches the concepts without unsafe noise
2. `Box<dyn AnyVec>` gives contiguous `Vec<T>` per column (cache-friendly) with type erasure at the column boundary only (one downcast per archetype per query type, not per element)
3. Archetype graph with lazy edges fits the Tungsten use-case exactly
4. Generational entity indices are clearly factored
5. Multi-component queries work by filtering archetypes then indexing parallel columns — directly teachable

**Deferred upgrade path:** `Box<dyn AnyVec>` → `BlobVec` (raw bytes + layout + drop_fn) is a bounded future step for maximum cache performance. Out of M12 scope.

---

## Scope Expansions

### [EXPANSION 1] Multi-component tuple queries

**Why:** The primary benefit of archetypal layout is efficient multi-component iteration. Adding `query2<A, B>()` alongside the existing `query<T>()` delivers this without breaking the existing API. The physics and render extraction systems currently use `query_entities::<T>()` + per-entity `get()` — exactly the pattern that tuple queries replace elegantly.

**Added:** `query2<A, B>()`, `query2_entities<A, B>()`, `query3<A, B, C>()`, `query3_entities<A, B, C>()` (immutable; mutable multi-queries require unsafe split-borrow — deferred).

### [EXPANSION 2] Generational entity IDs

**Why:** M12 is the natural upgrade window — the entity allocation table is being rewritten anyway. Generational indices catch stale-handle bugs that parent/child relationships in M13 will expose (D-021 deferred "upgrade only if bugs appear"; M13 will trigger exactly this risk).

**Impact:** `Entity` repr changes from `Entity(u32)` to `Entity { index: u32, generation: u32 }`. `entity.id()` continues to return the index as `u32` — source-compatible. Display impl stays identical.

---

## Architecture

### Files After Rewrite

```
crates/tungsten-core/src/ecs/
├── mod.rs           — re-exports (same as today)
├── entity.rs        — REWRITE: generational Entity + Entities allocation table
├── archetype.rs     — NEW: AnyColumn trait, TypedVec<T>, Archetype table
├── storage.rs       — NEW: Archetypes registry, EntityLocation, transitions
├── query.rs         — NEW: query2/query3 iteration helpers
├── resource.rs      — UNCHANGED
└── world.rs         — REWRITE: same public API + query2/query3

component.rs         — DELETED (replaced by archetype.rs + storage.rs)

crates/tungsten-core/benches/
└── ecs_bench.rs     — NEW: Criterion benchmarks
```

No changes to `tungsten-render`, `tungsten`, or any example.

---

## Data Layout

### Entity (generational)

```rust
// entity.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl Entity {
    /// Backward-compatible: returns index (same semantics as bare u32 id before).
    pub fn id(self) -> u32 { self.index }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Entity({})", self.index)  // identical output to before
    }
}

// Internal allocation state
pub(crate) struct EntityMeta {
    generation: u32,
    location: Option<EntityLocation>,  // None = dead or uninitialized
}

#[derive(Clone, Copy)]
pub(crate) struct EntityLocation {
    pub archetype_id: ArchetypeId,
    pub row: u32,
}

pub(crate) struct Entities {
    meta: Vec<EntityMeta>,
    free: Vec<u32>,  // recycled indices
}

impl Entities {
    fn alloc(&mut self) -> Entity;        // prefer free list, else grow meta
    fn free(&mut self, entity: Entity);   // bump generation, push index to free
    fn get(&self, entity: Entity) -> Option<EntityLocation>;  // validate generation
    fn set_location(&mut self, entity: Entity, loc: EntityLocation);
    fn is_alive(&self, entity: Entity) -> bool;
}
```

### Column Storage

```rust
// archetype.rs

/// Type-erased interface for a Vec<T> column.
/// Implementors are TypedVec<T> — a plain Vec<T> with type-erased operations.
pub(crate) trait AnyColumn: Any {
    fn push_erased(&mut self, val: Box<dyn Any>);
    fn swap_remove_erased(&mut self, row: usize) -> Box<dyn Any>;
    fn get_erased(&self, row: usize) -> &dyn Any;
    fn get_mut_erased(&mut self, row: usize) -> &mut dyn Any;
    fn len(&self) -> usize;
    fn type_id(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;      // downcast to &TypedVec<T> in query loops
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub(crate) struct TypedVec<T: 'static>(pub Vec<T>);

impl<T: 'static> AnyColumn for TypedVec<T> {
    fn push_erased(&mut self, val: Box<dyn Any>) {
        self.0.push(*val.downcast::<T>().unwrap());
    }
    fn swap_remove_erased(&mut self, row: usize) -> Box<dyn Any> {
        Box::new(self.0.swap_remove(row))
    }
    fn get_erased(&self, row: usize) -> &dyn Any { &self.0[row] }
    fn get_mut_erased(&mut self, row: usize) -> &mut dyn Any { &mut self.0[row] }
    fn len(&self) -> usize { self.0.len() }
    fn type_id(&self) -> TypeId { TypeId::of::<T>() }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
```

**Cache layout:** `TypedVec<Position>` is a `Vec<Position>` — fully contiguous. Query loops access `col.0[i]` for sequential row indices. This is the cache win vs. the current `HashMap<u32, Box<dyn Any>>`. Downcast cost is one per archetype per component type per query call, not per element.

### Archetype Table

```rust
// archetype.rs

pub(crate) type ArchetypeId = u32;
pub(crate) const EMPTY_ARCHETYPE: ArchetypeId = 0;

pub(crate) struct Archetype {
    pub id: ArchetypeId,
    /// Sorted list of component TypeIds — uniquely defines this archetype.
    pub component_types: Box<[TypeId]>,
    /// Columns: one Vec<T> per component type.
    pub columns: HashMap<TypeId, Box<dyn AnyColumn>>,
    /// Entity at each row index. Always same length as every column.
    pub entities: Vec<Entity>,
    /// Lazy edges: adding TypeId T moves to this archetype.
    pub add_edges: HashMap<TypeId, ArchetypeId>,
    /// Lazy edges: removing TypeId T moves to this archetype.
    pub remove_edges: HashMap<TypeId, ArchetypeId>,
}

impl Archetype {
    pub fn new(id: ArchetypeId, component_types: Box<[TypeId]>) -> Self;
    pub fn has(&self, type_id: TypeId) -> bool;
    pub fn row_count(&self) -> usize { self.entities.len() }

    /// Swap-remove the row at `row`. Returns the entity displaced from the
    /// last position (if any) so the caller can update its EntityLocation.
    pub fn swap_remove_row(&mut self, row: usize) -> Option<Entity>;

    /// Move all component columns for `row` into `dest` archetype (appending
    /// a new row there). Caller then inserts any new component into dest.
    /// Note: does NOT move the entity list entry — caller handles that.
    pub fn move_components_to(&mut self, row: usize, dest: &mut Archetype);
}
```

**Swap-remove invariant:** removing entity at row `r` moves the last row to `r` in all columns simultaneously. The caller is responsible for updating the moved entity's `EntityLocation.row` to `r`. This is the critical bookkeeping step in `despawn` and archetype transitions.

### Archetypes Registry

```rust
// storage.rs

pub(crate) struct Archetypes {
    archetypes: Vec<Archetype>,
    /// Sorted Box<[TypeId]> → ArchetypeId (fast lookup).
    index: HashMap<Box<[TypeId]>, ArchetypeId>,
    pub entities: Entities,
}

impl Archetypes {
    pub fn new() -> Self;  // creates empty archetype at index 0

    /// Find existing archetype for `types` (must be sorted), or create it.
    pub fn find_or_create(&mut self, types: &[TypeId]) -> ArchetypeId;

    pub fn spawn(&mut self) -> Entity;
    pub fn despawn(&mut self, entity: Entity);

    /// Add component T: moves entity to archetype +{T}.
    /// Panics if entity is dead (D-022).
    pub fn insert<T: 'static>(&mut self, entity: Entity, value: T);

    /// Remove component T: moves entity to archetype -{T}. Returns removed value.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T>;

    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T>;
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T>;
    pub fn has<T: 'static>(&self, entity: Entity) -> bool;

    pub fn archetypes_with<T: 'static>(&self) -> impl Iterator<Item = &Archetype>;
    pub fn archetypes_with_two(&self, a: TypeId, b: TypeId) -> impl Iterator<Item = &Archetype>;
    pub fn archetypes_with_three(&self, a: TypeId, b: TypeId, c: TypeId) -> impl Iterator<Item = &Archetype>;
}
```

**`insert<T>` algorithm (archetype transition):**
1. Validate entity alive — panic if dead (D-022)
2. Get current location `(arch_id, row)`
3. If entity already has T: overwrite `columns[T].get_mut_erased(row)` in place, return
4. Compute new type set: `sort(current_types ∪ {TypeId::of::<T>()})`
5. Lookup/create target archetype via `find_or_create`; cache in `old.add_edges[T] = target_id`
6. `old_arch.move_components_to(row, &mut new_arch)`
7. Push `value` into `new_arch.columns[T]`
8. Push entity into `new_arch.entities`
9. Update entity location to `(new_arch_id, new_arch.entities.len() - 1)`
10. If swap-remove displaced a last-row entity in old_arch, update *that* entity's location to `(old_arch_id, row)`

**`remove<T>` algorithm:** symmetric — new type set = current_types − {T}, move all columns *except* T, extract T's value via `swap_remove_erased`, return it.

---

## Public API (World)

### Preserved (unchanged signatures — all examples compile unchanged)

```rust
pub struct World { archetypes: Archetypes, resources: ResourceMap }

// Entity lifecycle
pub fn spawn(&mut self) -> Entity
pub fn despawn(&mut self, entity: Entity)
pub fn is_alive(&self, entity: Entity) -> bool

// Single-entity component access
pub fn insert<T: 'static>(&mut self, entity: Entity, component: T)
pub fn remove_component<T: 'static>(&mut self, entity: Entity) -> Option<T>
pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T>
pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T>
pub fn has<T: 'static>(&self, entity: Entity) -> bool

// Single-type queries (unchanged)
pub fn query<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)>
pub fn query_entities<T: 'static>(&self) -> Vec<Entity>

// Resources (unchanged)
pub fn insert_resource<T: 'static>(&mut self, resource: T)
pub fn get_resource<T: 'static>(&self) -> Option<&T>
pub fn get_resource_mut<T: 'static>(&mut self) -> Option<&mut T>
pub fn has_resource<T: 'static>(&self) -> bool
pub fn remove_resource<T: 'static>(&mut self) -> Option<T>
```

### New additions (EXPANSION 1)

```rust
// Immutable 2-component query — primary cache-friendly iteration path
pub fn query2<A: 'static, B: 'static>(&self)
    -> impl Iterator<Item = (Entity, &A, &B)>

// Entity-ID collection for 2-type mutation pattern
pub fn query2_entities<A: 'static, B: 'static>(&self) -> Vec<Entity>

// Immutable 3-component query
pub fn query3<A: 'static, B: 'static, C: 'static>(&self)
    -> impl Iterator<Item = (Entity, &A, &B, &C)>

// Entity-ID collection for 3-type mutation pattern
pub fn query3_entities<A: 'static, B: 'static, C: 'static>(&self) -> Vec<Entity>
```

**Mutable multi-queries deferred.** Getting `&A` and `&mut B` from the same `HashMap<TypeId, Box<dyn AnyColumn>>` requires `unsafe` split-borrow or index-based indirection. The existing safe pattern (`query2_entities` + `get_mut` per entity) is the mutation path.

**query2 implementation:**
```rust
pub fn query2<A: 'static, B: 'static>(&self) -> impl Iterator<Item = (Entity, &A, &B)> {
    let a_id = TypeId::of::<A>();
    let b_id = TypeId::of::<B>();
    self.archetypes.archetypes_with_two(a_id, b_id)
        .flat_map(move |arch| {
            // One downcast per archetype per type — not per element.
            let col_a = arch.columns[&a_id].as_any().downcast_ref::<TypedVec<A>>().unwrap();
            let col_b = arch.columns[&b_id].as_any().downcast_ref::<TypedVec<B>>().unwrap();
            (0..arch.row_count()).map(move |i| (arch.entities[i], &col_a.0[i], &col_b.0[i]))
        })
}
```

---

## Implementation Phases

### Phase 1 — Entity (generational)
**Files:** `entity.rs`

Replace `Entity(pub(crate) u32)` with `Entity { index: u32, generation: u32 }`. Implement `EntityMeta`, `EntityLocation`, `Entities` (alloc with free-list preference, free with generation bump). Keep `entity.id() -> u32` returning `index`; keep Display.

**Done when:** standalone unit tests pass — alloc, free, generation mismatch returns None, free-list index reuse with bumped generation.

### Phase 2 — Column + Archetype table
**Files:** `archetype.rs` (new)

Implement `AnyColumn` trait and `TypedVec<T>`. Implement `Archetype` with `new`, `has`, `row_count`, `swap_remove_row`, `move_components_to`. Columns start empty and are created via `Box::new(TypedVec::<T>(Vec::new()))` when a new archetype is constructed.

**Done when:** unit tests for push/swap-remove/move on a 2-type archetype pass. Specifically: spawn entity in archetype, swap-remove middle row, verify displaced entity is correct, verify both columns have consistent length.

### Phase 3 — Archetypes registry + transitions
**Files:** `storage.rs` (new)

Implement `Archetypes`. Empty archetype at index 0 has zero columns. `find_or_create` maintains the sorted `Box<[TypeId]>` index. `insert<T>` and `remove<T>` implement the transition algorithm described above. Lazy edge caching: after computing the transition target, write it into `arch.add_edges` / `arch.remove_edges` for O(1) repeat transitions.

**Done when:** unit tests covering: spawn in empty archetype, first insert transitions to single-type archetype, second insert transitions to 2-type, remove transitions back, despawn cleans up, get/get_mut return correct values, dead-entity get returns None.

### Phase 4 — World rewrite
**Files:** `world.rs`, `mod.rs`, delete `component.rs`

Replace `World` internals. `alive: Vec<bool>` is gone — `is_alive` delegates to `archetypes.entities.is_alive(entity)`. The panic in `insert` on dead entity is preserved via the `Archetypes::insert` validator (D-022). Wire all public methods through `Archetypes`.

**Done when:** all 9 existing `world.rs` unit tests pass byte-for-byte unchanged.

### Phase 5 — Multi-component queries (EXPANSION 1)
**Files:** `query.rs` (new), `world.rs`

Move query iteration logic into `query.rs` (or implement inline in `world.rs` — either is fine). Implement `query2`, `query2_entities`, `query3`, `query3_entities`. Add unit tests: 2-type query across archetypes with different supersets, 3-type query, `query2_entities` + `get_mut` mutation pattern.

**Done when:** new tests pass; all 9 existing `world.rs` tests still pass.

### Phase 6 — Integration + examples
`cargo test --workspace` — expected green with no example changes. Run smoke tests: `TUNGSTEN_SMOKE_FRAMES=120 ./scripts/smoke-examples.sh`. Check physics system, animation, tilemap extraction — all use the public World API which is unchanged.

**Done when:** `cargo test --workspace` green; 10 examples smoke-clean.

### Phase 7 — Benchmarks + DECISIONS.md
**Files:** `crates/tungsten-core/benches/ecs_bench.rs`, `DECISIONS.md`, `PHASE2.md`

**Benchmarks (Criterion):**
- Spawn 10k entities with 3 component types, iterate via `query::<Position>()`
- Spawn 10k entities across 5 different archetypes, iterate via `query2::<Position, Velocity>()`
- Compare against naive baseline: tag current HEAD before rewriting, bench both, record ratio

Add `criterion` to `[dev-dependencies]` in `tungsten-core/Cargo.toml` if absent.

**DECISIONS.md** D-036: confirm proceed decision citing D-030, storage design (Box<dyn AnyVec>, typed columns, archetype graph), generational IDs rationale, benchmark results, expansion justifications.

**PHASE2.md:** update M12 status to Complete, check off all acceptance criteria.

---

## Files Modified / Created

| Path | Action | Notes |
|------|--------|-------|
| `crates/tungsten-core/src/ecs/entity.rs` | Rewrite | Generational Entity + Entities table |
| `crates/tungsten-core/src/ecs/component.rs` | Delete | Replaced by archetype.rs + storage.rs |
| `crates/tungsten-core/src/ecs/archetype.rs` | Create | AnyColumn trait, TypedVec<T>, Archetype |
| `crates/tungsten-core/src/ecs/storage.rs` | Create | Archetypes, EntityLocation, transitions |
| `crates/tungsten-core/src/ecs/query.rs` | Create | query2/query3 iteration helpers |
| `crates/tungsten-core/src/ecs/resource.rs` | Unchanged | ResourceMap stays |
| `crates/tungsten-core/src/ecs/world.rs` | Rewrite | Same API + query2/query3 |
| `crates/tungsten-core/src/ecs/mod.rs` | Update | Re-export new modules |
| `crates/tungsten-core/benches/ecs_bench.rs` | Create | Criterion benchmarks |
| `DECISIONS.md` | Append | D-036 |
| `PHASE2.md` | Update | M12 status + criteria |

No changes to `tungsten-render`, `tungsten`, or any example.

---

## Utilities to Reuse

- `ResourceMap` (`resource.rs`) — unchanged, re-imported as-is in `World`
- `Entity` Display impl — keep identical format string `Entity({})`
- All physics, animation, tilemap components and systems — unaffected (use `World` public API)
- `DeltaTime`, `InputState`, `Camera2D` — resources, unchanged

---

## Acceptance Criteria

- [ ] D-036 logged in `DECISIONS.md` before any code is written
- [ ] All existing examples compile and pass without modification
- [ ] `cargo test --workspace` passes (9 existing world tests + new tests)
- [ ] Benchmark comparing iteration speed ≥10k entities, 3+ component types (old vs new)
- [ ] Query iteration is cache-friendly: components of same archetype stored contiguously (`Vec<T>`)
- [ ] `DECISIONS.md` entry documents storage design and benchmark results
- [ ] [EXPANSION 1] `query2`, `query2_entities`, `query3`, `query3_entities` added and tested
- [ ] [EXPANSION 2] Generational entity IDs; `entity.id()` returns index (`u32`), source-compatible

## Done When

`cargo test --workspace` is green, all 10 examples smoke-test clean, benchmarks are documented in D-036, and `PHASE2.md` M12 acceptance criteria are all checked.

---

## Non-Goals / Deferred

- Mutable multi-component queries (unsafe split-borrow required)
- Parallel system scheduling
- Change detection / dirty flags
- Command buffers / deferred despawn
- Raw `BlobVec` columns (Bevy-style raw byte storage)
- SparseSet storage for frequently-added/removed components
- Dynamic components / reflection
