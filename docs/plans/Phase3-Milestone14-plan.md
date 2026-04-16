---
status: draft
milestone: M14
version: 0.11.0
date: 2026-04-16
depends-on: M13
unblocks: M20, M21, M23, M24
---

# Phase 3 Milestone 14 — Event Queue

## Context

M13 (Command Buffers) gave systems a safe way to defer structural world mutations.
M14 generalises the existing per-system `CollisionEvents` pattern into a single
typed, two-window `EventQueue<T>` that works for any event type.  The two-window
design (previous + current) eliminates missed-read bugs when systems run in
different orders and makes per-system manual clear calls unnecessary.

M14 is a prerequisite for M20 (Scene/State), M21 (Debug Tooling), M23 (Particles),
and M24 (Tweens), which all need lightweight event signalling.

## Goal

Add `EventQueue<T>` as the canonical engine event-passing primitive.  Migrate
`CollisionEvents` to use it so physics behaviour is unchanged from the game-code
perspective, and wire automatic per-frame flush into the App frame loop.

## Non-goals

- Observer / push-callback / listener patterns
- Event filtering, tagging, or replay (deferred to Phase 4)
- Change detection or reactive systems
- Any milestone beyond M14 scope

## Affected files

| File | Change |
|------|--------|
| `crates/tungsten-core/src/ecs/event_queue.rs` | **create** — `EventQueue<T>` struct + tests |
| `crates/tungsten-core/src/ecs/mod.rs` | add module + re-export |
| `crates/tungsten-core/src/lib.rs` | re-export `EventQueue` |
| `crates/tungsten-core/src/physics/events.rs` | remove `CollisionEvents` struct (keep `CollisionEvent`) |
| `crates/tungsten-core/src/physics/mod.rs` | remove `CollisionEvents` re-export; update module doc |
| `crates/tungsten-core/src/physics/step.rs` | remove `clear()` calls; write via `send()`; update module doc + tests |
| `crates/tungsten/src/app.rs` | add `event_flushers` field + `register_event<T>()` + flush stage |
| `examples/01_platformer/src/main.rs` | replace all `CollisionEvents` with `EventQueue<CollisionEvent>` |
| `crates/tungsten-core/benches/ecs_bench.rs` | add event-queue flush benchmark (10 types) |
| `docs/LLM_INDEX.md` | add event queue row |
| `DESIGN.md` | replace `CollisionEvents` mentions with `EventQueue<CollisionEvent>` |
| `DECISIONS.md` | add D-040 |

## Pre-execution reads

Before writing any code, read these files to confirm current signatures:

1. `crates/tungsten-core/src/ecs/mod.rs` — module list and re-exports
2. `crates/tungsten-core/src/ecs/command_buffer.rs` — template for ECS-module style
3. `crates/tungsten-core/src/physics/events.rs` — `CollisionEvent` fields to keep
4. `crates/tungsten-core/src/physics/step.rs` — all `CollisionEvents` call sites
5. `crates/tungsten/src/app.rs` lines 1–130 — `App` struct fields + `new()` init block
6. `crates/tungsten/src/app.rs` lines 480–500 — command-buffer flush segment to extend
7. `examples/01_platformer/src/main.rs` — all `CollisionEvents` call sites (lines 32, 293–296, 335–337, 720)

---

## Implementation steps

### Phase 0 — DECISIONS.md entry

**Task 0.1** — Append to `DECISIONS.md`:

```
## D-040 — M14 EventQueue: two-window typed event buffering

Two-window design (`previous` + `current`) so readers always see at least the
most-recent frame's events regardless of system registration order.  `flush()`
rotates at the same frame boundary as CommandBuffer (after systems, before
extract/render).  Registration via `App::register_event::<T>()` stores a
type-erased flush closure; no separate scheduler or type registry needed.
`CollisionEvents` deleted (no backwards-compat shim); all call sites updated to
`EventQueue<CollisionEvent>`.
```

---

### Phase 1 — Create `EventQueue<T>`

**Task 1.1** — Create `crates/tungsten-core/src/ecs/event_queue.rs`:

```rust
//! Typed two-window event queue.
//!
//! Each `EventQueue<T>` resource holds two `Vec<T>` windows — `previous` and
//! `current`.  Senders call [`EventQueue::send`] during the update stage;
//! readers call [`EventQueue::iter`] (both windows) or
//! [`EventQueue::iter_current`] (this-frame window only).
//!
//! The App frame loop calls [`EventQueue::flush`] once per frame at the same
//! boundary as `CommandBuffer` flush (after all systems, before extract/render).
//! Flush rotates the windows: `previous ← current`, `current ← empty`.
//! Game systems must never call `flush` directly.

/// A typed two-window event buffer stored as a [`World`] resource.
///
/// [`World`]: crate::ecs::World
pub struct EventQueue<T> {
    current: Vec<T>,
    previous: Vec<T>,
}

impl<T> EventQueue<T> {
    pub fn new() -> Self {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
        }
    }

    /// Append `event` to the current frame's window.
    pub fn send(&mut self, event: T) {
        self.current.push(event);
    }

    /// Iterate all events across both windows (previous frame first, then current).
    ///
    /// This is the canonical reader API.  Systems that run before event senders
    /// within the same frame will still see last-frame's events via `previous`.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.previous.iter().chain(self.current.iter())
    }

    /// Iterate only the current frame's events.
    ///
    /// Use only when the calling system is guaranteed to run after all senders
    /// that write to this queue within the same frame (e.g. after `physics_step`).
    pub fn iter_current(&self) -> impl Iterator<Item = &T> {
        self.current.iter()
    }

    /// Returns `true` if both windows are empty.
    pub fn is_empty(&self) -> bool {
        self.current.is_empty() && self.previous.is_empty()
    }

    /// Total event count across both windows.
    pub fn len(&self) -> usize {
        self.current.len() + self.previous.len()
    }

    /// Rotate the windows: `previous ← current`, `current ← empty`.
    ///
    /// Called once per frame by the App event-flush stage.
    /// **Game systems must not call this method.**
    pub fn flush(&mut self) {
        // Reuse previous allocation for current next frame.
        self.previous.clear();
        std::mem::swap(&mut self.current, &mut self.previous);
    }
}

impl<T> Default for EventQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_queue_is_empty() {
        let q: EventQueue<i32> = EventQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert_eq!(q.iter().count(), 0);
    }

    #[test]
    fn send_appears_in_current_and_iter() {
        let mut q: EventQueue<i32> = EventQueue::new();
        q.send(1);
        q.send(2);
        assert_eq!(q.iter_current().copied().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn flush_moves_current_to_previous() {
        let mut q: EventQueue<i32> = EventQueue::new();
        q.send(1);
        q.flush();
        // After flush: previous=[1], current=[]
        assert_eq!(q.iter_current().count(), 0);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), vec![1]);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn flush_twice_drops_previous() {
        let mut q: EventQueue<i32> = EventQueue::new();
        q.send(1);
        q.flush();
        q.send(2);
        q.flush();
        // After second flush: previous=[2], current=[]
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn iter_sees_both_windows() {
        let mut q: EventQueue<i32> = EventQueue::new();
        q.send(1);
        q.flush();
        q.send(2);
        // previous=[1], current=[2]
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn flush_empty_is_idempotent() {
        let mut q: EventQueue<i32> = EventQueue::new();
        q.flush();
        q.flush();
        assert!(q.is_empty());
    }
}
```

---

### Phase 2 — Wire `EventQueue` into the ECS module

**Task 2.1** — Edit `crates/tungsten-core/src/ecs/mod.rs`.

Add `pub mod event_queue;` to the module list and extend `pub use`:

```rust
// before
mod archetype;
mod command_buffer;
mod entity;
mod resource;
mod storage;
mod world;

pub use command_buffer::{CommandBuffer, PendingEntity};
pub use entity::Entity;
pub use world::World;

// after
mod archetype;
mod command_buffer;
mod entity;
pub mod event_queue;
mod resource;
mod storage;
mod world;

pub use command_buffer::{CommandBuffer, PendingEntity};
pub use entity::Entity;
pub use event_queue::EventQueue;
pub use world::World;
```

`event_queue` is `pub mod` so `physics/step.rs` can use `crate::ecs::event_queue::EventQueue`
when the re-export path isn't available (within `tungsten-core` itself).

**Task 2.2** — Edit `crates/tungsten-core/src/lib.rs`.

Add `EventQueue` to the `pub use ecs::{...}` line:

```rust
// before
pub use ecs::{CommandBuffer, Entity, PendingEntity, World};
// after
pub use ecs::{CommandBuffer, Entity, EventQueue, PendingEntity, World};
```

---

### Phase 3 — App-level registration and frame-loop flush

**Task 3.1** — Edit `crates/tungsten/src/app.rs`: add `event_flushers` field to
the `App` struct (after `gpu_timing_enabled`):

```rust
    /// Type-erased flush closures for registered event queues.
    /// Populated by `register_event::<T>()`. Flushed once per frame after
    /// the command-buffer flush stage and before hot-reload/extract/render.
    event_flushers: Vec<Box<dyn FnMut(&mut World)>>,
```

**Task 3.2** — Edit `App::new()`: replace the `CollisionEvents` resource init
with `EventQueue<CollisionEvent>`, build `event_flushers` as a local variable
before the `Self { ... }` return, and add it to the struct literal.

The existing `new()` body sets up resources on `world` then returns `Self { ... }`.
Apply these three edits without restructuring the function:

```rust
// 1. Replace (line 95):
        world.insert_resource(CollisionEvents::new());
// with:
        world.insert_resource(EventQueue::<CollisionEvent>::new());

// 2. After the last world.insert_resource call and before Self { ... }, insert:
        let mut event_flushers: Vec<Box<dyn FnMut(&mut World)>> = Vec::new();
        event_flushers.push(Box::new(|world: &mut World| {
            if let Some(q) = world.get_resource_mut::<EventQueue<CollisionEvent>>() {
                q.flush();
            }
        }));

// 3. In the Self { ... } block, add the new field:
            event_flushers,
```

**Task 3.3** — Edit imports in `crates/tungsten/src/app.rs`:

```rust
// remove
use tungsten_core::physics::{CollisionEvents, PhysicsConfig};
// add
use tungsten_core::physics::{CollisionEvent, PhysicsConfig};

// also add EventQueue to the existing tungsten_core import line:
// before
use tungsten_core::{
    AssetRegistry, AudioCommands, Camera2D, CommandBuffer, Config, DeltaTime, InputState, World,
};
// after
use tungsten_core::{
    AssetRegistry, AudioCommands, Camera2D, CommandBuffer, Config, DeltaTime, EventQueue,
    InputState, World,
};
```

**Task 3.4** — Add `register_event<T>()` to `impl App`.  Because `EventQueue` is
now a top-level import (Task 3.3), no local `use` is needed inside the method:

```rust
    /// Register an event type `T` with the engine.
    ///
    /// Inserts an `EventQueue<T>` resource into the `World` and schedules its
    /// flush once per frame (after systems, before extract/render).
    /// Call during app construction, before `run()`.
    pub fn register_event<T: 'static>(&mut self) {
        self.world.insert_resource(EventQueue::<T>::new());
        self.event_flushers.push(Box::new(|world: &mut World| {
            if let Some(q) = world.get_resource_mut::<EventQueue<T>>() {
                q.flush();
            }
        }));
    }
```

Note: `App::new()` inserts `EventQueue::<CollisionEvent>::new()` directly (Task 3.2)
rather than calling `register_event` — this avoids needing to restructure `new()`
into a two-step `let mut app = …` form.

**Task 3.5** — Insert the event-flush stage in the frame loop, immediately after
the command-buffer flush block and before `// --- Hot reload stage ---`:

```rust
                // --- Event queue flush stage ---
                // Frame order invariant (Phase 3 guardrail):
                //   run systems -> flush commands -> flush events -> hot-reload -> extract -> render
                for flusher in self.event_flushers.iter_mut() {
                    flusher(&mut self.world);
                }
```

---

### Phase 4 — Migrate physics step to `EventQueue<CollisionEvent>`

**Task 4.1** — Edit `crates/tungsten-core/src/physics/step.rs` imports and
module doc comment:

```rust
// Update the module-level doc comment line 6:
//   1. Clear the `CollisionEvents` resource.
// to:
//   1. Populate `EventQueue<CollisionEvent>` with resolved contacts.

// Replace import:
use super::events::{CollisionEvent, CollisionEvents};
// with:
use super::events::CollisionEvent;
use crate::ecs::event_queue::EventQueue;
```

**Task 4.2** — Remove early-return `clear()` (~line 63):

```rust
// delete this block entirely
        if let Some(events) = world.get_resource_mut::<CollisionEvents>() {
            events.clear();
        }
```

**Task 4.3** — Remove start-of-step `clear()` (~line 74):

```rust
// delete this block entirely
    if let Some(events) = world.get_resource_mut::<CollisionEvents>() {
        events.clear();
    }
```

**Task 4.4** — Update event-sink write (~line 301):

```rust
// before
        if let Some(sink) = world.get_resource_mut::<CollisionEvents>() {
            sink.events.extend(events);
        }
// after
        if let Some(sink) = world.get_resource_mut::<EventQueue<CollisionEvent>>() {
            for e in events {
                sink.send(e);
            }
        }
```

**Task 4.5** — Update `seed_world()` in the test module (~line 389):

```rust
// before
        world.insert_resource(CollisionEvents::new());
// after
        world.insert_resource(EventQueue::<CollisionEvent>::new());
```

**Task 4.6** — Update `CollisionEventsExt` test trait (~line 624):

```rust
// before
    impl CollisionEventsExt for CollisionEvents {
        fn iter_any_tile(&self) -> bool {
            self.events.iter().any(|e| e.b.is_none())
        }
    }
// after
    impl CollisionEventsExt for EventQueue<CollisionEvent> {
        fn iter_any_tile(&self) -> bool {
            self.iter_current().any(|e| e.b.is_none())
        }
    }
```

**Task 4.7** — Update the two test assertions that read `CollisionEvents`
(~lines 442 and 486):

```rust
// before (both occurrences)
        let events = world.get_resource::<CollisionEvents>().unwrap();
// after
        let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
```

The assertions (`!events.is_empty()`, `events.iter_any_tile()`) are unchanged.

---

### Phase 5 — Delete `CollisionEvents` and stale re-exports

**Task 5.1** — Edit `crates/tungsten-core/src/physics/events.rs`.

Delete the `CollisionEvents` struct and its entire `impl` block (lines 28–62).
Keep `CollisionEvent` and its doc comment unchanged.

**Task 5.2** — Edit `crates/tungsten-core/src/physics/mod.rs`.

Update the module doc comment (line 15) and re-export:

```rust
// Update doc line:
//! `components.rs`; the step function, broadphase grid, and narrow-phase
//! shape tests are in sibling modules. `CollisionEvents` is populated
//! each step for gameplay systems to read.
// to:
//! `components.rs`; the step function, broadphase grid, and narrow-phase
//! shape tests are in sibling modules. Collision contacts are delivered via
//! `EventQueue<CollisionEvent>` each step for gameplay systems to read.

// Update re-export:
// before
pub use events::{CollisionEvent, CollisionEvents};
// after
pub use events::CollisionEvent;
```

**Task 5.3** — Edit `crates/tungsten-core/src/lib.rs`.

Remove `CollisionEvents` from the physics `pub use` block:

```rust
// before
pub use physics::{
    aabb_vs_aabb, aabb_vs_circle, circle_vs_circle, physics_step, Aabb, BodyKind, Collider,
    CollisionEvent, CollisionEvents, Contact, PhysicsConfig, Position, RigidBody, Shape,
    SpatialGrid, Velocity,
};
// after
pub use physics::{
    aabb_vs_aabb, aabb_vs_circle, circle_vs_circle, physics_step, Aabb, BodyKind, Collider,
    CollisionEvent, Contact, PhysicsConfig, Position, RigidBody, Shape,
    SpatialGrid, Velocity,
};
```

---

### Phase 6 — Migrate platformer example

**Task 6.1** — Read `examples/01_platformer/src/main.rs` in full before editing.

**Task 6.2** — Replace the `CollisionEvents` import (line 32).

The import block starting at line 31 currently includes `CollisionEvents`.
After migration it should include `CollisionEvent` (singular) instead, and
`EventQueue` should be added to the `tungsten` or `tungsten_core` import
(whichever the file uses for `World`).  Adjust to match the file's existing
grouping style.

**Task 6.3** — Update `ground_detection` (~lines 293–296):

```rust
// before
/// Scans `CollisionEvents` and flags the player as grounded on an upward contact.
fn ground_detection(world: &mut World) {
    let events = match world.get_resource::<CollisionEvents>() {
        Some(e) => e.events.clone(),
        None => return,
    };
// after
/// Scans `EventQueue<CollisionEvent>` and flags the player as grounded on an upward contact.
fn ground_detection(world: &mut World) {
    let events: Vec<CollisionEvent> = match world.get_resource::<EventQueue<CollisionEvent>>() {
        Some(q) => q.iter().copied().collect(),
        None => return,
    };
```

The remainder of the function (iterating `&events`) is unchanged.

**Task 6.4** — Update the contact-count read (~lines 335–337):

```rust
// before
    let contacts = world
        .get_resource::<CollisionEvents>()
        .map(|e| e.len())
// after
    let contacts = world
        .get_resource::<EventQueue<CollisionEvent>>()
        .map(|q| q.len())
```

**Task 6.5** — Update the test module world setup (~line 720):

```rust
// before
        world.insert_resource(CollisionEvents::new());
// after
        world.insert_resource(EventQueue::<CollisionEvent>::new());
```

After Tasks 6.2–6.5, verify with `grep CollisionEvents examples/01_platformer/src/main.rs`
that no references remain.

---

### Phase 7 — Add benchmark

**Task 7.1** — Update the top-of-file `use` in
`crates/tungsten-core/benches/ecs_bench.rs` to import `EventQueue`:

```rust
// before
use tungsten_core::{CommandBuffer, World};
// after
use tungsten_core::{CommandBuffer, EventQueue, World};
```

**Task 7.2** — Append the following to `crates/tungsten-core/benches/ecs_bench.rs`,
before the `criterion_group!` macro:

```rust
// ---------------------------------------------------------------------------
// M14 — EventQueue flush cost (10 queue types, 100 events each)
//
// Measures the combined cost of populating 10 typed event queues with 100
// events each and flushing them all once — the per-frame steady-state cost
// if 10 event types are registered. Allocation is included because Vec::new()
// is effectively free; what we care about is the memcpy/clear path of flush.
// Results inform the Phase 3 benchmark gate for event-queue flush cost.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev00(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev01(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev02(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev03(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev04(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev05(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev06(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev07(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev08(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev09(u32);

fn bench_event_queue_flush_10_types(c: &mut Criterion) {
    c.bench_function("event_queue_flush_10_types", |b| {
        b.iter(|| {
            let mut q0: EventQueue<Ev00> = EventQueue::new();
            let mut q1: EventQueue<Ev01> = EventQueue::new();
            let mut q2: EventQueue<Ev02> = EventQueue::new();
            let mut q3: EventQueue<Ev03> = EventQueue::new();
            let mut q4: EventQueue<Ev04> = EventQueue::new();
            let mut q5: EventQueue<Ev05> = EventQueue::new();
            let mut q6: EventQueue<Ev06> = EventQueue::new();
            let mut q7: EventQueue<Ev07> = EventQueue::new();
            let mut q8: EventQueue<Ev08> = EventQueue::new();
            let mut q9: EventQueue<Ev09> = EventQueue::new();
            for i in 0..100u32 {
                q0.send(Ev00(i)); q1.send(Ev01(i)); q2.send(Ev02(i));
                q3.send(Ev03(i)); q4.send(Ev04(i)); q5.send(Ev05(i));
                q6.send(Ev06(i)); q7.send(Ev07(i)); q8.send(Ev08(i));
                q9.send(Ev09(i));
            }
            black_box(&mut q0).flush(); black_box(&mut q1).flush();
            black_box(&mut q2).flush(); black_box(&mut q3).flush();
            black_box(&mut q4).flush(); black_box(&mut q5).flush();
            black_box(&mut q6).flush(); black_box(&mut q7).flush();
            black_box(&mut q8).flush(); black_box(&mut q9).flush();
        });
    });
}
```

**Task 7.3** — Add `bench_event_queue_flush_10_types` to the existing
`criterion_group!` macro call:

```rust
// before
criterion_group!(
    benches,
    bench_spawn_insert,
    // ... existing entries ...
    bench_naive_query2_via_entities,
);
// after — append one line:
criterion_group!(
    benches,
    bench_spawn_insert,
    // ... existing entries ...
    bench_naive_query2_via_entities,
    bench_event_queue_flush_10_types,
);
```

---

### Phase 8 — Update navigation docs

**Task 8.1** — Edit `docs/LLM_INDEX.md`: add an event queue row after the ECS row:

```markdown
| Event queue (`EventQueue<T>`, frame flush) | [`crates/tungsten-core/src/ecs/event_queue.rs`](../crates/tungsten-core/src/ecs/event_queue.rs) |
```

**Task 8.2** — Edit `DESIGN.md`: replace the two `CollisionEvents` mentions with
`EventQueue<CollisionEvent>`.

Search for `CollisionEvents` in `DESIGN.md`; there are two occurrences in the
Resources sections.  Replace both:

```
# before (each occurrence)
CollisionEvents
# after
EventQueue<CollisionEvent>
```

---

### Phase 9 — Verify

Run in order; each command must exit 0 before proceeding:

```bash
cargo fmt --all
cargo test --workspace
./scripts/smoke-examples.sh
cargo bench -p tungsten-core --bench ecs_bench -- event_queue_flush_10_types
```

Additional manual checks:
- `grep -r CollisionEvents crates/ examples/` — must return zero matches
  (plan file and archived docs are expected to match, source code must not).
- Bench output: record the wall-clock result in a comment near `bench_event_queue_flush_10_types`.

Expected outcomes:
- All existing physics tests pass under the new resource type.
- New `EventQueue` unit tests (6 cases) pass.
- Platformer and sprite-stress smoke tests pass.

---

## Done-when checklist

- [ ] `EventQueue<T>` in `tungsten-core::ecs::event_queue` with `send`, `flush`,
      `iter`, `iter_current`, `is_empty`, `len`, `Default`
- [ ] `EventQueue` re-exported from `tungsten_core` crate root
- [ ] `App::register_event::<T>()` inserts resource and registers flush closure
- [ ] Event-flush stage in App frame loop: after command-buffer flush, before hot-reload
- [ ] `CollisionEvents` struct deleted from `physics/events.rs`
- [ ] `CollisionEvent` struct unchanged
- [ ] `physics_step` writes via `send()`; no manual `clear()` calls remain
- [ ] Platformer: no `CollisionEvents` references remain anywhere in the file
- [ ] `cargo test --workspace` — zero failures
- [ ] `smoke-examples.sh` — platformer and sprite-stress pass
- [ ] Benchmark `bench_event_queue_flush_10_types` compiles and runs; result recorded
- [ ] D-040 added to `DECISIONS.md`
- [ ] `docs/LLM_INDEX.md` has event queue row
- [ ] `DESIGN.md` `CollisionEvents` mentions replaced
- [ ] Status updated to `in progress` at start, `done` when checklist complete

## Open questions / assumptions

- **A-1:** `EventQueue::flush()` is `pub` so the `tungsten` crate's App closure
  can call it across crate boundaries.  Alternative: `pub(crate)` + internal
  dispatch mechanism inside `tungsten-core`.  Current choice: `pub` with doc
  warning, consistent with how `CommandBuffer::new()` and other ECS types expose
  lifecycle methods publicly.

- **A-2:** Physics tests call `physics_step` directly without an App flush loop.
  Existing tests each call `physics_step` once — no multi-step isolation issue.
  If a future test needs per-step event isolation, it should call
  `world.get_resource_mut::<EventQueue<CollisionEvent>>().unwrap().flush()`
  between steps.

- **A-3:** `iter()` returns previous-then-current (two-frame window).  Game code
  that needs only this-frame events uses `iter_current()`.  The platformer's
  `ground_detection` migrates to `iter()`, which is correct: it tolerates
  two-frame coyote-time semantics and is safe regardless of system order.

- **A-4:** Event registration is startup-only via `register_event<T>()`.
  Dynamic runtime registration is not in scope for M14.

- **A-5:** `App::new()` directly inserts `EventQueue::<CollisionEvent>::new()`
  and builds the flush closure inline, rather than calling `register_event`.
  This avoids restructuring `new()` into a `let mut app` form.  The two paths
  are semantically identical; `register_event` is the public API for game code
  to add their own event types.
