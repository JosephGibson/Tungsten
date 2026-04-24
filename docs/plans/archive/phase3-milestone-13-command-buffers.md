---
status: done
milestone: M13
goal: Implement M13 — Command Buffers (deferred structural world mutation)
non-goals: parallel command buffers, per-system buffer isolation, cancel-spawn, command priorities
files to touch:
  - crates/tungsten-core/src/ecs/command_buffer.rs  (new)
  - crates/tungsten-core/src/ecs/mod.rs
  - crates/tungsten-core/src/ecs/world.rs
  - crates/tungsten-core/src/lib.rs
  - crates/tungsten-core/benches/ecs_bench.rs
  - crates/tungsten/src/app.rs
  - crates/tungsten/src/telemetry.rs
  - DECISIONS.md
  - docs/plans/Phase3.md  (mark M12 + M13 complete)
---

# Phase 3 Milestone 13 — Command Buffers

## Risks and Open Questions

> **Read this section before touching any code.**

| # | Risk | Mitigation |
|---|------|-----------|
| R1 | `PendingEntity` used after a *different* buffer is flushed | `PendingEntity` IDs are buffer-local `u32` indices. Resolving a stale pending ID from a previous flush will either panic (index out of bounds) or silently reference the wrong entity. Document clearly in the type's doc comment: "A `PendingEntity` is only valid for the buffer that produced it, and only until that buffer is flushed." |
| R2 | Double-despawn: system calls `world.despawn(e)` directly, then a queued `Despawn(e)` fires during flush | Guard flush's despawn path with `if world.is_alive(entity)`. Skip silently. |
| R3 | `Remove` command on an entity that was despawned earlier in the same flush pass | `Archetypes::remove` already returns `None` on a dead entity via the `entities.get(e)?` guard. No special handling needed in flush. |
| R4 | `insert_pending` called with a pending_id from a *previous* flush | Per-flush `pending_entities` vec is local to each `flush()` call. A stale `PendingEntity` whose `id` exceeds the vec length will panic with an index-out-of-bounds. This is the correct failure mode — programmer error per D-022's spirit. Document in the type's doc comment. |
| R5 | Frame order regression | Flush must happen **after all systems run** and **before extract/render**. Enforced by a comment block at the flush insertion point in `app.rs`. |

**Open questions (no blocking answer needed for M13):**

- Q1: Bench — spawn only, or spawn + insert? → Plan implements 1k spawns with 2 components each (more realistic; Phase3.md's "cost" includes the archetype transitions from inserting components).
- Q2: Buffer lifetime — cleared and reused (pool) or replaced fresh each frame? → Fresh each frame. Simplest; no perf concern at Phase 3 scale; revisit in Phase 4 if flush appears in profiling.

---

## Context

M13 is the first ECS Core milestone of Phase 3. It introduces `CommandBuffer`, a type
that collects deferred structural world mutations (`spawn`, `despawn`, `insert`,
`remove_component`) so systems never need to hold `&mut World` during iteration for
structural changes.

**Why now:** M13 unblocks M20 (Scene/State System) and M23 (Particle System). Both need
runtime spawn/despawn without iteration-hazard bugs. Building the pattern now, while the
system list is small, is cheaper than retrofitting it across a larger example surface later.

**Intended outcome:** Any system closure can call `world.get_resource_mut::<CommandBuffer>()`,
queue mutations, and have them applied atomically at a fixed frame boundary (after all systems
run, before extract). Existing system closures that do not use command buffers are completely
unchanged — `SystemFn = Box<dyn FnMut(&mut World)>` stays the same.

---

## Frame order (post-M13 invariant)

```
RedrawRequested:
  1. Delta time
  2. Update:      run all registered systems
                  ↑ systems may write to CommandBuffer resource
  3. Flush:       drain CommandBuffer resource → world.flush(buf)   ← NEW
                  mutations are NOT visible to frame-N systems (they ran before flush)
                  mutations ARE visible to extract/render within frame N
                  mutations ARE visible to systems starting frame N+1
  4. Hot reload:  process_hot_reload()
  5. Extract:     extract_quads / extract_sprites / extract_text
  6. Render:      render_frame_full[_timed]
  7. Audio:       drain AudioCommands
  8. Input:       input.begin_frame()
  9. Telemetry:   update FrameTimings resource
```

---

## API shape

### `PendingEntity` — `crates/tungsten-core/src/ecs/command_buffer.rs`

```rust
/// Opaque handle to an entity queued for spawn in a [`CommandBuffer`]
/// but not yet flushed into the [`World`].
///
/// **Lifetime rule:** a `PendingEntity` is only valid for the buffer that
/// produced it, and only until [`World::flush`] is called for that buffer.
/// Do not store a `PendingEntity` across a flush boundary or use it with
/// a different buffer — doing so will panic or corrupt entity state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PendingEntity(pub(crate) u32);
```

### `CommandBuffer` — `crates/tungsten-core/src/ecs/command_buffer.rs`

```rust
/// Collects deferred structural mutations to apply to the [`World`] at a
/// fixed frame boundary (after all systems run, before extract/render).
///
/// Cannot derive `Debug` because internal command variants hold
/// `Box<dyn …>` types. Cannot derive `Default` for the same reason —
/// use `CommandBuffer::new()` or the manual `impl Default`.
pub struct CommandBuffer {
    commands: Vec<Command>,
    pending_count: u32,
}

impl CommandBuffer {
    pub fn new() -> Self
    pub fn is_empty(&self) -> bool
    pub fn len(&self) -> usize

    /// Queue a spawn. Returns a [`PendingEntity`] handle that can be passed to
    /// [`insert_pending`](Self::insert_pending) within this buffer, before flush.
    pub fn spawn(&mut self) -> PendingEntity

    /// Queue a component insert on a live (already-existing) entity.
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T)

    /// Queue a component insert on a not-yet-flushed pending entity.
    pub fn insert_pending<T: 'static>(&mut self, pending: PendingEntity, component: T)

    /// Queue a component removal from a live entity.
    /// No-op at flush time if the entity is dead or lacks the component.
    pub fn remove_component<T: 'static>(&mut self, entity: Entity)

    /// Queue a despawn. No-op at flush time if the entity is already dead.
    pub fn despawn(&mut self, entity: Entity)
}
```

### `World::flush` — `crates/tungsten-core/src/ecs/world.rs`

```rust
/// Apply all commands in `buffer` to the world, then drop the buffer.
///
/// **Two-pass algorithm:**
/// Pass 1 — scan for `Spawn` commands; call `self.spawn()` for each,
///           building `pending_entities: Vec<Entity>` indexed by `pending_id`.
/// Pass 2 — replay commands in registration order:
///   - `Spawn { .. }`              → no-op (already handled in pass 1)
///   - `Insert { Live(e), .. }`    → `self.insert(e, component)` (skipped if e dead)
///   - `Insert { Pending(id), .. }`→ `self.insert(pending_entities[id], component)`
///                                   (pending entity can't be dead — just spawned in pass 1)
///   - `Remove(f)`                 → `f(self)` — closure captures entity + type;
///                                   no-op if entity dead or component absent (Archetypes::remove)
///   - `Despawn(e)`                → `if self.is_alive(e) { self.despawn(e) }`
///
/// **Visibility:** entities spawned in frame N are queryable by extract/render in
/// frame N and by systems starting frame N+1.
pub fn flush(&mut self, buffer: CommandBuffer)
```

---

## Ordered steps

> **Execute phases in numbered order.** Phase 4 (App) depends on Phase 3 (FrameTimings)
> because `app.rs` writes `ft.flush_ms` which does not exist until Phase 3 adds the field.

### Phase 0 — Pre-flight

- [ ] **Task 0.1** — Run `cargo test --workspace` and confirm all tests pass before any changes.
  - **Acceptance:** Zero test failures on clean tree.
  - **Deps:** none.

---

### Phase 1 — Core types: `CommandBuffer` + `PendingEntity`

- [ ] **Task 1.1** — Create `crates/tungsten-core/src/ecs/command_buffer.rs`.

  **Target file:** `crates/tungsten-core/src/ecs/command_buffer.rs` (new file)

  **Required imports at the top of the file:**
  ```rust
  use super::entity::Entity;
  use super::world::World;
  ```

  **Internal plumbing** — all items below are `pub(super)` so that `world.rs`
  (a sibling module within `ecs`) can import and pattern-match them:

  ```rust
  pub(super) trait ComponentSetter: 'static {
      fn apply(self: Box<Self>, world: &mut World, entity: Entity);
  }

  pub(super) struct InsertSetter<T: 'static> {
      pub(super) component: T,
  }

  impl<T: 'static> ComponentSetter for InsertSetter<T> {
      fn apply(self: Box<Self>, world: &mut World, entity: Entity) {
          world.insert(entity, self.component);
      }
  }

  pub(super) enum CommandTarget {
      Live(Entity),
      Pending(u32),
  }

  pub(super) enum Command {
      Spawn    { pending_id: u32 },
      Insert   { target: CommandTarget, setter: Box<dyn ComponentSetter> },
      Remove   (Box<dyn FnOnce(&mut World)>),  // closure captures entity + type statically
      Despawn  (Entity),
  }
  ```

  **Public surface:**
  ```rust
  pub struct PendingEntity(pub(crate) u32);

  pub struct CommandBuffer {
      pub(super) commands: Vec<Command>,
      pub(super) pending_count: u32,
  }
  ```

  **Implement all methods from the API shape section above.**

  For `remove_component<T: 'static>`:
  ```rust
  pub fn remove_component<T: 'static>(&mut self, entity: Entity) {
      self.commands.push(Command::Remove(Box::new(move |world: &mut World| {
          world.remove_component::<T>(entity);
      })));
  }
  ```

  **`Default` impl** — cannot use `#[derive(Default)]` because `Command` holds
  `Box<dyn …>` fields. Implement manually:
  ```rust
  impl Default for CommandBuffer {
      fn default() -> Self {
          Self::new()
      }
  }
  ```

  **Acceptance criteria:**
  - `cargo check -p tungsten-core` passes.
  - All methods implemented with correct signatures.
  - `pub(super)` on `Command`, `CommandTarget`, `ComponentSetter`, `InsertSetter`.

  **Deps:** Task 0.1.

- [ ] **Task 1.2** — Add unit tests in `command_buffer.rs` under `#[cfg(test)] mod tests`.

  These tests verify the buffer's own state only — not flush behavior (that is tested in world.rs).
  Use a dummy `Entity { index: 0, generation: 0 }` for tests that need an entity value; no
  `World` is needed here.

  | Test name | What it checks |
  |-----------|---------------|
  | `new_buffer_is_empty` | `CommandBuffer::new().is_empty() == true`, `len() == 0` |
  | `spawn_increments_len` | each `spawn()` increments `len()` by 1 |
  | `spawn_returns_distinct_pending_ids` | two spawns return different `PendingEntity` values |
  | `despawn_queued` | `despawn(e)` increases `len()` by 1 |
  | `insert_live_queued` | `insert(e, v)` increases `len()` by 1 |
  | `insert_pending_queued` | `insert_pending(p, v)` increases `len()` by 1 |
  | `remove_component_queued` | `remove_component::<T>(e)` increases `len()` by 1 |

  **Acceptance criteria:** all 7 tests pass.

  **Deps:** Task 1.1.

---

### Phase 2 — `World::flush`

- [ ] **Task 2.1** — Add `World::flush` to `crates/tungsten-core/src/ecs/world.rs`.

  **Change:** add one import and one method to the existing `world.rs`.

  **Import to add** (with existing `use super::…` imports at the top of world.rs):
  ```rust
  use super::command_buffer::{Command, CommandBuffer, CommandTarget};
  ```

  **Method to add** to `impl World` (after the `remove_resource` method, before the `Default` impl):

  ```rust
  pub fn flush(&mut self, buffer: CommandBuffer) {
      // Pass 1: allocate real entities for every Spawn command, in pending_id order.
      // pending_entities[i] holds the real Entity for PendingEntity(i).
      let mut pending_entities: Vec<Entity> = Vec::new();
      for cmd in &buffer.commands {
          if let Command::Spawn { pending_id } = cmd {
              debug_assert_eq!(*pending_id as usize, pending_entities.len(),
                  "pending_id must be allocated sequentially");
              pending_entities.push(self.spawn());
          }
      }

      // Pass 2: replay all commands in registration order.
      for cmd in buffer.commands {
          match cmd {
              Command::Spawn { .. } => {} // handled in pass 1
              Command::Insert { target, setter } => {
                  let entity = match target {
                      CommandTarget::Live(e) => {
                          // Guard: entity may have been despawned between buffer
                          // creation and this flush (e.g., by another command earlier
                          // in this same buffer or by a direct world.despawn call).
                          if !self.is_alive(e) { continue; }
                          e
                      }
                      CommandTarget::Pending(id) => {
                          // Pending entities were just spawned in pass 1 — always alive.
                          pending_entities[id as usize]
                      }
                  };
                  setter.apply(self, entity);
              }
              Command::Remove(f) => f(self), // no-op if entity dead or component absent
              Command::Despawn(entity) => {
                  if self.is_alive(entity) {
                      self.despawn(entity);
                  }
              }
          }
      }
  }
  ```

  **Acceptance criteria:**
  - `cargo check -p tungsten-core` passes.
  - `World::flush` is the only new public method added to `World`.
  - Existing public API surface is otherwise unchanged.

  **Deps:** Task 1.1.

- [ ] **Task 2.2** — Add `World::flush` integration tests in `crates/tungsten-core/src/ecs/world.rs`
  under the existing `#[cfg(test)] mod tests` block (after the existing M12 query tests).

  Add `use super::super::command_buffer::CommandBuffer;` inside the test module, or use the
  crate path `use tungsten_core::ecs::CommandBuffer` — whichever resolves after Task 3.1.

  | Test name | What it checks |
  |-----------|---------------|
  | `flush_spawn_entity_is_alive` | `buf.spawn()` → `world.flush(buf)` → entity `is_alive` |
  | `flush_spawn_insert_pending_components_visible` | `spawn` + `insert_pending` × 2 → flush → both components queryable on the spawned entity |
  | `flush_insert_live_entity` | `buf.insert(existing_entity, T)` → flush → component visible |
  | `flush_despawn_entity_is_dead` | `buf.despawn(e)` → flush → `!world.is_alive(e)` |
  | `flush_remove_component` | `buf.remove_component::<T>(e)` → flush → `world.get::<T>(e).is_none()` |
  | `flush_command_order_preserved` | `buf.insert(e, A)`, `buf.insert(e, B)`, `buf.remove_component::<A>(e)` — after flush entity has B but not A |
  | `flush_despawn_dead_entity_is_silent` | direct `world.despawn(e)` then `buf.despawn(e)` → flush does not panic |
  | `flush_empty_buffer_is_noop` | `world.flush(CommandBuffer::new())` on a populated world leaves entity count unchanged |
  | `flush_multiple_pending_entities` | spawn 3 pending entities each with a distinct marker component → after flush all 3 alive with correct components |

  **Acceptance criteria:** all 9 tests pass; `cargo test -p tungsten-core` green.

  **Deps:** Tasks 1.1, 2.1.

---

### Phase 3 — Re-exports

- [ ] **Task 3.1** — Add `command_buffer` module to `crates/tungsten-core/src/ecs/mod.rs`.

  **Target file:** `crates/tungsten-core/src/ecs/mod.rs`

  **Change:** add two lines after the existing module declarations:
  ```rust
  mod command_buffer;
  pub use command_buffer::{CommandBuffer, PendingEntity};
  ```

  **Acceptance:** `cargo check -p tungsten-core` passes; `tungsten_core::ecs::CommandBuffer`
  and `tungsten_core::ecs::PendingEntity` are accessible.

  **Deps:** Task 1.1.

- [ ] **Task 3.2** — Re-export from `crates/tungsten-core/src/lib.rs`.

  **Target file:** `crates/tungsten-core/src/lib.rs`

  **Change:** extend the existing `pub use ecs::{Entity, World};` line to include the
  new types:
  ```rust
  pub use ecs::{CommandBuffer, Entity, PendingEntity, World};
  ```

  **Acceptance:** `use tungsten_core::{CommandBuffer, PendingEntity}` compiles from
  the `tungsten` crate.

  **Deps:** Task 3.1.

---

### Phase 4 — `FrameTimings` update

> This phase must complete before Phase 5 because `app.rs` writes `ft.flush_ms`.

- [ ] **Task 4.1** — Add `flush_ms` field to `FrameTimings` in `crates/tungsten/src/telemetry.rs`.

  **Target file:** `crates/tungsten/src/telemetry.rs`

  **Change:** add one field to `FrameTimings` after the `hot_reload_ms` field:
  ```rust
  /// Time spent draining and applying the `CommandBuffer` resource each frame.
  /// Includes all deferred spawn/despawn/insert/remove mutations from this frame's systems.
  pub flush_ms: f32,
  ```

  `Default` already derives `0.0` for `f32` fields. Update the `default_is_zero` test
  to assert `ft.flush_ms == 0.0`.

  **Acceptance criteria:**
  - `FrameTimings { flush_ms: 0.0, .. }` is the zero-value default.
  - `default_is_zero` test updated and passing.
  - `cargo test -p tungsten` green.

  **Deps:** Task 0.1.

---

### Phase 5 — App integration

- [ ] **Task 5.1** — Insert `CommandBuffer` resource and wire flush in `crates/tungsten/src/app.rs`.

  **Target file:** `crates/tungsten/src/app.rs`

  **Change a — import:** add `CommandBuffer` to the existing `tungsten_core` import line:
  ```rust
  use tungsten_core::{AssetRegistry, AudioCommands, Camera2D, CommandBuffer, Config,
                      DeltaTime, InputState, World};
  ```

  **Change b — resource insertion:** in `App::new()`, after the line
  `world.insert_resource(GpuFrameTimings::default());`, add:
  ```rust
  world.insert_resource(CommandBuffer::new());
  ```

  **Change c — flush stage:** in the `RedrawRequested` handler, insert the flush stage
  immediately after the system-loop block ends (after `let update_ms = …`) and before
  the `process_hot_reload()` call. Add this comment block to make the invariant explicit:

  ```rust
  // --- Flush stage: apply deferred command buffers ---
  // Frame order invariant (Phase 3 guardrail):
  //   run systems → flush commands → hot-reload → extract → render
  // Mutations are NOT visible to frame-N systems (they ran before this point).
  // Mutations ARE visible to extract/render within this frame.
  let flush_start = Instant::now();
  let flush_buf = self.world
      .remove_resource::<CommandBuffer>()
      .expect("CommandBuffer resource missing — was it removed by a system?");
  self.world.flush(flush_buf);
  self.world.insert_resource(CommandBuffer::new());
  let flush_ms = flush_start.elapsed().as_secs_f64() as f32 * 1000.0;
  ```

  **Change d — telemetry write:** in the `FrameTimings` write block, add:
  ```rust
  ft.flush_ms = flush_ms;
  ```

  **Change e — perf log:** add `flush={flush_ms:.2}ms` to the `TUNGSTEN_PERF_LOG` debug
  format string and its argument list, consistent with the existing stage entries.

  **Acceptance criteria:**
  - `cargo build --workspace` succeeds.
  - Both existing examples launch without panic.
  - `FrameTimings::flush_ms` is non-zero on a frame that flushed commands.

  **Deps:** Tasks 3.2, 4.1.

---

### Phase 6 — Benchmark

- [ ] **Task 6.1** — Add `bench_command_buffer_flush_1k` to `crates/tungsten-core/benches/ecs_bench.rs`.

  **Target file:** `crates/tungsten-core/benches/ecs_bench.rs`

  **Import to add** (alongside existing `use tungsten_core::…` imports):
  ```rust
  use tungsten_core::CommandBuffer;
  ```

  **New benchmark function** (add before the `criterion_group!` macro):
  ```rust
  fn bench_command_buffer_flush_1k(c: &mut Criterion) {
      c.bench_function("command_buffer_flush_1k_spawns", |b| {
          b.iter(|| {
              let mut world = World::new();
              let mut buf = CommandBuffer::new();
              for i in 0..1_000u32 {
                  let p = buf.spawn();
                  buf.insert_pending(p, Position { x: i as f32, y: 0.0 });
                  buf.insert_pending(p, Velocity { dx: 1.0, dy: 0.0 });
              }
              world.flush(buf);
              black_box(&world);
          });
      });
  }
  ```

  **Update `criterion_group!`** to include the new function:
  ```rust
  criterion_group!(
      benches,
      bench_spawn_insert,
      bench_query_single,
      bench_query2_homogeneous,
      bench_query2_fragmented,
      bench_query2_10k_5archetypes_pv,
      bench_spawn_despawn_1k,
      bench_command_buffer_flush_1k,   // ← new
      bench_naive_query_single,
      bench_naive_query2_via_entities,
  );
  ```

  **Acceptance criteria:**
  - `cargo bench -p tungsten-core -- command_buffer_flush_1k_spawns` runs without error.

  **Deps:** Tasks 3.2, 2.1.

---

### Phase 7 — DECISIONS.md

- [ ] **Task 7.1** — Add `D-039` entry to `DECISIONS.md`.

  **Target file:** `DECISIONS.md`

  **Append after the D-038 entry.** Write the entry now; fill in the bench number
  (`~Xµs`) after Task 8.3 records it.

  ```markdown
  ## D-039 — M13 CommandBuffer: two-pass flush, closure-typed removes, resource-based delivery

  **Date:** <date>
  **Decision:** Implement `CommandBuffer` as a `Vec<Command>` stored as a `World` resource.
  `App` inserts a fresh buffer before each frame's systems run and drains it immediately
  after (flush stage, before hot-reload and extract). Four operations: `spawn` →
  `PendingEntity`, `insert` / `insert_pending` (live vs. pending target), `remove_component`,
  `despawn`. Flush algorithm: two-pass — allocate real entities for all `Spawn` commands
  first (building a `Vec<Entity>` indexed by pending_id), then replay all mutations in
  registration order. Type-erased component insert uses a private `ComponentSetter` trait
  object (`pub(super)` within the `ecs` module). Type-erased remove uses
  `Box<dyn FnOnce(&mut World)>` capturing entity and type statically — avoids adding a
  type-erased remove method to `Archetypes`. Stale-despawn guard: `if world.is_alive(e)`
  in the flush despawn arm. Next-frame visibility rule for systems: entities spawned in
  frame N are queryable by systems starting frame N+1 (but visible to extract/render in
  frame N). No new crate dependencies (D-015 satisfied).
  Bench: `command_buffer_flush_1k_spawns` ≈ ~Xµs (1k spawns + 2k inserts via buffer vs.
  direct `spawn_despawn_1k` ≈ ~Yµs; see bench run <date>).
  ```

  **Acceptance criteria:** Entry present after D-038; no earlier entry modified.

  **Deps:** Task 0.1 (can be written before bench numbers; update after Task 8.3).

---

### Phase 8 — Verification

- [ ] **Task 8.1** — Full test suite.

  ```bash
  cargo test --workspace
  ```

  **Acceptance:** zero failures.

- [ ] **Task 8.2** — Smoke tests against both examples.

  Use the canonical smoke script:
  ```bash
  ./scripts/smoke-examples.sh
  ```

  Or invoke directly (confirmed package names from `examples/*/Cargo.toml`):
  ```bash
  TUNGSTEN_SMOKE_FRAMES=3 cargo run -p example-01-platformer
  TUNGSTEN_SMOKE_FRAMES=3 cargo run -p example-02-sprite-stress
  ```

  **Acceptance:** both examples exit cleanly after 3 frames; no panics.

- [ ] **Task 8.3** — Run the command buffer benchmark; record result in D-039.

  ```bash
  cargo bench -p tungsten-core -- command_buffer_flush_1k_spawns
  ```

  Compare against `spawn_despawn_1k` (existing bench). Flush-1k adds 2 inserts per entity
  so expect it to run roughly 2–3× slower than `spawn_despawn_1k`. Update the `~Xµs` and
  `~Yµs` placeholders in DECISIONS.md D-039 with the actual numbers.

  **Acceptance:** benchmark runs; D-039 numbers filled in.

- [ ] **Task 8.4** — Regression check on steady-state benchmarks.

  ```bash
  cargo bench -p tungsten-core -- query_single query2 spawn_insert spawn_despawn
  ```

  **Acceptance:** no result exceeds +10% vs. the pre-M13 baseline (Phase3.md gate).
  If a regression is detected, document rationale in DECISIONS.md before closing M13.

- [ ] **Task 8.5** — Clippy.

  ```bash
  cargo clippy --workspace --all-targets
  ```

  **Acceptance:** no new warnings from M13 changes (advisory per AGENTS.md).

- [ ] **Task 8.6** — Update `docs/plans/Phase3.md`.

  Add a status note below the M12 and M13 headings:

  ```markdown
  ### M12 - Performance Baseline + Profiling Harness
  > **Status: complete** (v0.9.0, 2026-04-15)
  ```

  ```markdown
  ### M13 - Command Buffers
  > **Status: complete** (v0.10.x, <date>)
  ```

  **Acceptance:** Phase3.md reflects M12 + M13 complete.

---

## References

| Link | Why useful |
|------|-----------|
| [Bevy `Commands` source (bevy_ecs/src/system/commands/mod.rs)](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ecs/src/system/commands/mod.rs) | Reference implementation in a mature Rust ECS — see type-erased component application, entity pre-allocation, and the `EntityCommands` builder pattern. |
| [Catherine West "Using Rust For Game Development" (RustConf 2018)](https://kyren.github.io/2018/09/14/rustconf-talk.html) | Foundational writeup on ECS data ownership in Rust; explains why `&mut World` during iteration is structurally unsafe and motivates deferred command buffers. |
| [Flecs Deferred Mode documentation](https://www.flecs.dev/flecs/md_docs_2Relationships.html) | Production C ECS documentation on deferred mutations; useful for edge cases: nested flush ordering, stale entity guards, and command ordering guarantees. |
| [Sander Mertens "ECS back and forth, Part 2"](https://skypjack.github.io/2019-03-07-ecs-baf-part-2/) | Background on archetype-based ECS mutation patterns; explains why structural mutations (archetype transitions) must be deferred during iteration. |
| [Unity ECS `EntityCommandBuffer` API docs](https://docs.unity3d.com/Packages/com.unity.entities@1.0/api/Unity.Entities.EntityCommandBuffer.html) | Production API reference; validates the `PendingEntity` resolution pattern and multi-buffer ordering semantics used in this design. |

---

## Done-when checks

All of the following are true:

- [ ] `CommandBuffer`, `PendingEntity` exported from `tungsten_core` root.
- [ ] `World::flush(&mut self, buffer: CommandBuffer)` exists and applies commands via two-pass algorithm.
- [ ] `FrameTimings::flush_ms` field exists and is populated each frame by `App`.
- [ ] `CommandBuffer` inserted as a World resource by `App::new()`; replaced fresh after flush each frame.
- [ ] All 7 `CommandBuffer` unit tests pass.
- [ ] All 9 `World::flush` integration tests pass.
- [ ] `cargo test --workspace` is green.
- [ ] Both existing examples pass 3-frame smoke test without panic.
- [ ] `bench_command_buffer_flush_1k_spawns` runs; result recorded in `DECISIONS.md` D-039.
- [ ] No steady-state benchmark regresses by more than 10% vs. M12 baseline.
- [ ] `docs/plans/Phase3.md` updated to mark M12 + M13 complete.
