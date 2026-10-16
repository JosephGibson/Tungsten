# Phase 6 milestone 60: event cursors and spawn inheritance (W13e) — done

- **status:** done
- **goal:** The kit catalogue's F1 ([w13-kit](../../1.0/w13-kit.md) §7) in core: `EventCursor`, a reader's place in an `EventQueue` by sequence, which reads each event once from any stage or fixed step and counts what the queue dropped before the read; and spawn inheritance, a World registry of what a spawn takes from its source, applied at the command flush to a prefab queued from that source, a source an earlier command despawned dropping the spawn. `StatePlugin` registers `SceneEntity`, so the kit's `Spawner`, which spawns from its spawner, leaves its spawns with the spawner's state and a spawner due in its state's exit frame leaves nothing; `SoundOnEvent` reads through a cursor, behaviour unchanged.
- **non-goals:** The two-phase load, `DataRegistry` and `DataRef` (W13f); `Source`, `Zone`, tags, phase anchors, `KitPlugins`, the kit's scene components and the recipe harness (W13g); cues and the event trace (W13h); the umbrella's own event readers (game feel, tweens, particles, UI), which keep `iter` and `iter_current`; a spawn from a source that is not a prefab, or made at once rather than queued (Q7, Q8); state ownership of an entity that takes a state's marker in its exit frame after the dispatcher has listed its entities (a recorded limit, §2.3); any physics, render, extract, schema, dependency, thread, system or stage change.
- **files to touch:** step 1: `crates/tungsten-core/src/ecs/{event_queue,command_buffer,world,mod}.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/assets/prefab.rs`, `crates/tungsten-core/src/tests/ecs/{event_queue,command_buffer}.rs`, `crates/tungsten-core/src/tests/assets/prefab.rs`, `crates/tungsten/src/tests/fixed_step.rs`, `api/tungsten-core.txt`; step 2: `crates/tungsten-kit/src/{sound,trigger}.rs`, `crates/tungsten-kit/src/tests/sound.rs`; step 3: `crates/tungsten/src/{plugins,state}.rs`, `crates/tungsten-kit/src/spawner.rs`, `crates/tungsten-kit/src/tests/spawner.rs`, `crates/tungsten/src/tests/{kit,prefabs}.rs`, `templates/basic/AGENTS.md`, `docs/getting-started.md`; step 4: none in the tree (evidence in `perf-runs/`, scratch in `target/`); step 5: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w13-kit.md`; step 6: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, `docs/plans/1.0/implementation-plan.md` (the register row), `docs/plans/1.0/README.md` (the four owned Now fields), this plan (moved to `docs/plans/archive/1.0/`); this plan's evidence log in every step.
- **ordered steps:** (1) the event cursor and spawn inheritance in core, `just api`, then the step 1 API critique; (2) the kit's readers onto the cursor; (3) kit spawns leave with their state; (4) every CPU row and the physics hashes; (5) records; (6) Release 0.80.
- **done-when:** every step's evidence row quoted, step 2's possibly as skipped (§7); workflow §5 closed (§8); the release checks pass after the cut, every other gate having run in steps 1–4; the preflight prints the command block and the post-merge block.

Candidate W13e ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M60, release 0.80. Written 2026-10-16 at `0732305`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

The catalogue's finalizing review (w13-kit §1, at `0732305`) found that the kit's base needs two things core lacks. **R1:** events have no exactly-once reader. A reader picks `iter` or `iter_current` by where it runs against its senders and the stage, and `SoundOnEvent` keeps a handled count that holds only for one run a frame outside the fixed steps; the catalogue adds about ten reactive readers in both stages (cues, the trace, zones, abilities). **R7:** kit spawns outlive their state. `SceneEntity` is the umbrella's, so a kit on core alone cannot copy it, and a spawner due in the frame its state exits spawns after the dispatcher has listed the state's entities.

F1 answers both in core. It stands on `D-040` (two windows), `D-128` (registration on the World), `D-137` (step views), `D-139` (held frames; it left "a reader cursor" open), `D-039` and `D-084` (the flush), `D-157` (prefab spawns: "A prefab spawn adds no `SceneEntity`"), `D-046` (scene-owned cleanup) and `D-155` (the kit on core alone, opt-in plugins). It closes `docs/known-issues.md:52` for the spawner, and w13-kit's follow-ups for an exactly-once reader (M50) and state ownership of spawned entities (M51).

Later candidates expect it. F3 (W13g) derives `Source` through the registry's function form and reads `Zone` entries through a cursor. A (W13h) reads cues and the trace through cursors, and its `prefab` action spawns from the event's entity. C and E spawn projectiles, drops and payloads the same way. Level C (workflow §4): an Opus 5.5 max plan, the API sketch in §2, step 1's API critique before the rest; no `D-108` pair.

## 1. Audit

Read at `0732305`, the 0.79 release; the tree's uncommitted changes are documents only. Paths are the [LLM index](../../../LLM_INDEX.md)'s prefixes.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `EventQueue` holds `current`, `previous` and, for frames with no fixed step, up to 256 held frames, but no sequence. `iter` reads held (first step only), previous, current; `iter_current` the current window, in a later step only what was sent since the step before ended; `flush` drops `previous`, and the held frames unless the frame holds. A reader has no place of its own in the queue (catalogue R1). | `core/ecs/event_queue.rs:39-54`, `:75-102`, `:108-138` |
| A2 | `SoundOnEvent`'s reader remembers the current window's length at its last run and skips that many of the previous window at the next. That is right for one run a frame outside the steps; inside them the step views and held frames change what `iter_current` and `len` return. Nine unit tests and one harness test pin the behaviour. | `crates/tungsten-kit/src/sound.rs:130-157`; `crates/tungsten-kit/src/tests/sound.rs:61-201`; `tungsten/tests/kit.rs:246-314` |
| A3 | `TriggerZone`, the kit's other reader, reads `SensorEntered` through `iter_current` after `physics_step`: once a step, through `D-137`'s step view. Ordered before the step it would read nothing. | `crates/tungsten-kit/src/trigger.rs:92-127`; `crates/tungsten-kit/src/tests/trigger.rs`; `tungsten/tests/kit.rs:1240-1286` |
| A4 | No other kit system reads events: `EventQueue` appears in the kit only in `sound.rs`, `trigger.rs` and `health.rs:226`, a sender. | `rg -n 'EventQueue' crates/tungsten-kit/src` |
| A5 | `register_event` keeps a function pointer and a `TypeId` per type in the World's own fields, is idempotent and returns whether the type was new; `flush_events` calls the pointers by index. The registry mirrors it. | `core/ecs/world.rs:18-30`, `:70-100` |
| A6 | The flush allocates every pending spawn first, then replays in recording order; an insert run on a dead target drops its values in order. The World's bundle scratch stages values and applies them as one move. | `core/ecs/world.rs:681-748` (`:725-730`), `:185-199` |
| A7 | `Command` has `Spawn`, `Insert`, `Call` and `Despawn`; `call` passes one entity, so a derive (a source and a target) needs a variant of its own. `Command` is 24 bytes, and 24 with `Derive { source: Entity, target: CommandTarget }` added (a layout probe built with the pinned 1.98.1 on this machine). | `core/ecs/command_buffer.rs:119-148`, `:243-251` |
| A8 | `PreparedPrefab` has no source. `spawn` flushes a private buffer now; `record` writes one run (`Transform`, `Sprite`, `Visibility`, `Tag`, `Tween`, the components in name order, the `with` bundles); `CommandBuffer::spawn_prefab`, in the same file, queues it. `Debug` prints the ID alone and a test pins it; the queued form's test counts 8 commands. | `core/assets/prefab.rs:225-302`, `:602-628`; `core/tests/assets/prefab.rs:352-384` |
| A9 | Queries scan every archetype, and `insert_run` creates each archetype along a run's add edges, one per type the entity lacks. Where derived types join a spawn decides how many archetypes a sourced prefab adds: ahead of its run, a second copy of the prefab's chain per prefab shape; after it, one archetype per derived type at the end of the shared chain (`P+A`, then `P+A+B`). | `core/ecs/world.rs:251`, `:301`; `core/ecs/storage.rs:182-217` (`:194-200`) |
| A10 | `SceneEntity { state_id }` is `Clone` and `Copy`. On a pop or a replace the dispatcher (`PreUpdate`, after the display hotkeys) lists the state's entities and queues their despawns, then runs `on_exit`; the flush comes after `PostUpdate`. So an entity given the state's marker at once in `on_exit`, through `with`, already outlives the state. | `tungsten/state.rs:33-37`, `:247-262`, `:461-496` (`:464-468`); `tungsten/plugins.rs:104-117`; `tungsten/app.rs:1212-1218`, `:1642-1646` |
| A11 | `Spawner` ticks in `PostUpdate` and queues `spawn_prefab` with no source; its doc says its entities "carry no state marker" (known issues; catalogue R7). In the exit frame the spawner is still alive at its tick, since despawns wait for the flush. | `crates/tungsten-kit/src/spawner.rs:26-29`, `:151-184`; `docs/known-issues.md:52` |
| A12 | `StatePlugin` adds the dispatcher alone; `App::new` inserts `StateStack`. The bench's plugin set includes `StatePlugin`, so a registration there runs once at startup in every row; no row spawns from a source. | `tungsten/plugins.rs:104-117`; `tungsten/app.rs:301`; `examples/02_bench/src/main.rs:81-87` |
| A13 | A frame with no step holds its queues while the clock advances and `fixed_update` has a system; each step turns the step view on and ends it. A pause drops the held frames, so a fixed reader of `iter` starts over after one. | `tungsten/app.rs:1088-1157`; `tungsten/tests/fixed_step.rs:244-304` |
| A14 | `api/tungsten-core.txt` lists `EventQueue` at two paths, each `World` method twice and `PreparedPrefab`'s methods three times; the umbrella's and the kit's snapshots list no core item; the snapshot prints `#[non_exhaustive]` and no `#[must_use]`. | `api/tungsten-core.txt:5130`, `:14165`, `:5284`, `:17138`, `:1275`, `:2903`, `:15340` |
| A15 | Nothing outside the kit's tests uses `SoundOnEvent`, `TriggerZone` or `Spawner`. The template spawns its player in `on_enter` with an explicit `SceneEntity`; its `AGENTS.md` and the guide tell a state to pass its `SceneEntity` through `with`. | `templates/basic/src/states.rs:146`; `templates/basic/AGENTS.md:14`; `docs/getting-started.md:164` |
| A16 | Records: the LLM index holds 12,284 of its 12,288 bytes, a strict `>` check; DESIGN describes the handled count and the prefab spawn's `with`; w13-kit's M50 and M51 follow-ups and its known-issues pointer close here. | `scripts/check-agent-context.py:25`, `:148`; `DESIGN.md:96`, `:459`, `:471`, `:477`; `docs/plans/1.0/w13-kit.md:472`, `:477`, `:509` |
| A17 | Another session's uncommitted text: `w13-kit.md:488` is a heading that starts with a `sed` expression where the catalogue review's round-2 critique line should be, over that round's three findings, inside Follow-ups. This plan edits other lines of that file and leaves these. | `docs/plans/1.0/w13-kit.md:487-494`; `git diff HEAD -- docs/plans/1.0/w13-kit.md` |
| A18 | `insert_resource` replaces a resource of the same type, and `resource_mut` allows an assignment, so a registered queue can be swapped for a fresh one; tests insert fresh queues at setup, and only `register_event` builds one in library code. `SoundOnEvent`'s handled count reads a swapped queue's windows as it finds them, so it plays the fresh queue's events. | `core/ecs/world.rs:76`, `:623-625`; `core/ecs/resource.rs:16-18`; `crates/tungsten-kit/src/sound.rs:151-154`; `rg -n 'insert_resource\(EventQueue' crates examples` |

## 2. Design

### 2.1 The event cursor

```rust
// core/ecs/event_queue.rs; re-exported at `tungsten_core` and `tungsten_core::ecs`.

/// A reader's place in an `EventQueue<T>`, by sequence: each `read` yields the
/// events sent since the last read that the queue still holds, once each, in
/// the order they were sent, whatever the stage, the fixed step or the
/// reader's place among the senders. A cursor follows its queue: when the
/// World's queue is replaced, its next read starts at the new queue's
/// oldest event.
pub struct EventCursor<T> { /* next: u64, queue: Option<sync::Weak<()>>, PhantomData<fn() -> T> */ }

impl<T> EventCursor<T> {
    /// A cursor whose first read yields every event the queue then holds.
    #[must_use]
    pub fn new() -> Self;
    /// The events past this cursor, oldest first. The cursor moves past all
    /// of them when `read` returns, whether or not the iterator is driven to
    /// its end.
    pub fn read<'q>(&mut self, queue: &'q EventQueue<T>) -> EventRead<'q, T>;
}
impl<T> Default for EventCursor<T> {}   // new()
impl<T> Clone for EventCursor<T> {}     // no `T: Clone`; a clone is another reader at the same place
impl<T> fmt::Debug for EventCursor<T> {}

/// One `EventCursor::read`: its events, oldest first, and how many the queue
/// dropped before the cursor reached them.
#[must_use = "a read moves its cursor past every event it holds"]
pub struct EventRead<'q, T> { /* held frames' tail, previous, current, missed */ }

impl<T> EventRead<'_, T> {
    /// The events past the cursor that the queue dropped before this read:
    /// rotated out by flushes since the last read, or past the held-frame
    /// cap. Always 0 for a reader that reads every frame. A reader in
    /// `fixed_update` misses what frames with a stopped clock dropped, and
    /// what the 256-frame hold cap dropped when its steps came further apart
    /// than that, as in deep slow motion.
    #[must_use]
    pub fn missed(&self) -> u64;
}
impl<'q, T> Iterator for EventRead<'q, T> { type Item = &'q T; }
impl<T> ExactSizeIterator for EventRead<'_, T> {}
impl<T> std::iter::FusedIterator for EventRead<'_, T> {}
impl<T> fmt::Debug for EventRead<'_, T> {}   // its length and `missed`
```

- **Sequence.** Every event a queue keeps is a contiguous tail of all it was sent: held frames oldest first, then `previous`, then `current` (`flush` moves `previous` to the held frames or drops it, and `current` to `previous`). So the queue needs two private counters, `first: u64`, the sequence of its oldest event (every event dropped so far), and `held_len: usize`, the events in its held frames, which `flush` keeps; its end is `first + held_len + previous.len() + current.len()`. It also gains an identity, an `Arc<()>` allocated when it is built. `send` is unchanged.
- **Read.** The cursor's place is the next sequence it has not read. A read starts at its place or at `first`, whichever is later, and yields up to the end; `missed` is how far `first` had passed its place. A new cursor starts at `first` (Q3). Unread held events are found from the back, so a read costs O(1) unless it reaches into held frames, at most 256 of them.
- **A replaced queue** (Q15). A queue swapped in (A18) starts its sequence at 0, so a place alone cannot tell it from the old one. A cursor therefore holds a `Weak` of its queue's identity from its first read. A weak pointer keeps its allocation, so no later queue can share that address, and a queue that moves keeps its `Arc`. A read whose queue has another identity adopts it and starts at its oldest event, missing nothing, as `SoundOnEvent`'s handled count reads a fresh queue's windows. So replacing a queue, by `insert_resource`, by assignment, or by a removal and an insert, restarts its readers, the kit's installed ones included; the events the old queue held unread are neither read nor counted. Checking costs one pointer comparison a read, and `EventQueue` stays `Send` and `Sync`.
- **Views.** The cursor ignores `D-137`'s step views and reads held frames like any other events. A reader that runs every frame (`PreUpdate`, `Update`, `PostUpdate`) never misses. A `fixed_update` reader through a cursor reads each event once at any frame rate while the clock runs and its steps come within 256 frames with events of each other. After a pause, or across a longer gap in deep slow motion, it counts in `missed` what was dropped, where a reader of `iter` silently started over.
- `iter`, `iter_current`, `len`, `is_empty`, `flush` and the holds keep their behaviour and docs; `EventQueue`'s doc gains a paragraph pointing at the cursor.

### 2.2 Spawn inheritance

```rust
// core/ecs/world.rs
impl World {
    /// A spawn from a source (`CommandBuffer::spawn_prefab_from`) takes a
    /// clone of the source's `T` when the source has one and the spawn does
    /// not. Idempotent per type: the first registration of `T`, this or
    /// `derive_from_source`, stands. Returns whether `T` was new.
    pub fn inherit_from_source<T: Clone + 'static>(&mut self) -> bool;
    /// A spawn from a source takes `derive(world, source)` when that returns
    /// a value and the spawn has no `T`: what a spawn takes that is not a
    /// copy, such as the first entity of a spawn chain. Idempotent per type,
    /// as `inherit_from_source`.
    pub fn derive_from_source<T: 'static>(&mut self, derive: fn(&World, Entity) -> Option<T>) -> bool;
}

// core/assets/prefab.rs, beside `spawn_prefab`
impl CommandBuffer {
    /// Queues `prefab` as `spawn_prefab` does, spawned from `source`: at the
    /// flush, after its own fields and `with` bundles, the entity takes what
    /// the World's spawn inheritance gives from `source` as the flush finds
    /// it, in one more archetype move when anything is given. A source an
    /// earlier command despawned drops the spawn: nothing of it remains.
    pub fn spawn_prefab_from(&mut self, source: Entity, prefab: PreparedPrefab) -> PendingEntity;
}
```

- **Registry.** Beside the event flushers: a `Vec` of monomorphized `fn(&mut World, Entity, Entity)` derivers in registration order and a `TypeId` set. A `derive_from_source` function is kept in a crate-private resource keyed by its type, and `inherit_from_source::<T>` is `derive_from_source::<T>` with a clone of the source's `T`. No box per spawn, no `unsafe`; a derive reads the World and cannot change it.
- **Command.** `Command::Derive { source, target }`, recorded by `spawn_prefab_from` right after the spawn's run through a crate-private `CommandBuffer` recorder for a pending target. `Command` stays 24 bytes (A7). `PreparedPrefab`, `spawn` and `spawn_prefab` are unchanged, so a spawn with no source records and flushes exactly as today.
- **Flush.** A dead target does nothing. A dead source despawns the target, which drops the spawn whole, since the pending entity and its run were allocated and applied before. Otherwise each deriver stages, for a type the target lacks, its value in the World's bundle scratch, and one `apply_bundle` moves the target once. Derived types land after the prefab's own, so a type the prefab or a `with` bundle gives keeps its own value (Q6). A sourced prefab shares the prefab's archetype chain and adds one archetype per derived type at its end, `P+A` and then `P+A+B`, once per prefab shape and derived set (A9).
- **Queued only** (Q7). Inheritance happens at the flush, as the card puts it. A spawn made at once from a source in a state's exit frame, in `on_exit` say, would take the marker after the dispatcher has listed the state's entities and outlive the state; queued there, its derive follows the dispatcher's despawns and drops it.
- **Seam.** Core names no umbrella or kit type (`D-155`'s direction): the umbrella registers its own marker, and the kit only calls `spawn_prefab_from`. No render type, and the extract still reads `&World` (`D-007`, `D-018`).

### 2.3 The exit frame

A state with a spawner (`Transform`, `Spawner`, `SceneEntity { state_id: "level" }`) whose period ends in the frame the state is popped:

| Stage | What happens |
| --- | --- |
| `PreUpdate`, `state_dispatcher` | The pop applies: `despawn_scene_entities` queues despawns for the spawner and every spawn that took its `SceneEntity`, then `on_exit` runs (A10) |
| `PostUpdate`, `spawner_tick` | The spawner is alive and due: `prepare_prefab(..)`, then `spawn_prefab_from(spawner, ..)` queues a spawn, its run and a derive (A11) |
| Command flush | Pass 1 allocates the spawn. The despawns, recorded first, apply first; the spawn's run applies; its derive finds the spawner dead and despawns it |
| After the flush | Nothing of the state remains; a spawner outside any state, due in the same frame, has spawned (the test's control) |

In any other frame the derive finds the spawner alive, so the spawn takes `SceneEntity { "level" }` and the next exit's listing includes it. The same holds for a spawn queued in `on_exit` from one of the state's entities: its derive follows the despawns, so it is dropped. The recorded limit (Q14) is an entity that takes the state's marker in the exit frame after the listing but not behind the despawns. One is a spawn queued from a state's entity by a `pre_update` system ordered before `state_dispatcher`, whose derive comes ahead of them. The other, as before M60, is an entity spawned at once in `on_exit` with the marker passed through `with`. No kit system does either.

### 2.4 The umbrella and the kit

- **`StatePlugin`** (`plugins.rs`) calls `world.inherit_from_source::<SceneEntity>()` beside the dispatcher (Q9). Its rustdoc, `SceneEntity`'s and `state.rs`'s module doc say that a prefab queued from a scene entity leaves with its state.
- **`SoundOnEvent`** (`sound.rs`): the reader keeps an `EventCursor<E>` in place of `handled`. It reads once a frame in `PostUpdate` as before, returns at once on an empty read, and otherwise plays as before; with no `SoundRegistry` or `AudioCommands` the read still moves the cursor, so nothing replays, and a queue replaced between runs plays its events once, as today. Its rustdoc's second paragraph says that it reads through a cursor. Its plugin, name and stage are unchanged.
- **`TriggerZone`** (`trigger.rs`): the system's state gains an `EventCursor<SensorEntered>`, which `read_zone` reads; after `physics_step` it reads each step's entries as `iter_current` did, and in any order it reads each once (Q10).
- **`Spawner`** (`spawner.rs`): the tick queries `(Entity, &mut Spawner, &Transform)`, keeps the spawner's entity with each due spawn, and queues `world.prepare_prefab(&prefab, position)` through `spawn_prefab_from(spawner, prepared)`. Its rustdoc's "carry no state marker" sentence becomes: each spawn comes from its spawner, takes what the World's spawn inheritance gives, a state's `SceneEntity` under `StatePlugin`, and a spawner despawned earlier in the frame's commands spawns nothing.
- **Template and guide** (Q12): `templates/basic/AGENTS.md:14` and `docs/getting-started.md:164` each gain one clause: a prefab queued from an entity with `CommandBuffer::spawn_prefab_from(entity, prepared)` takes that entity's `SceneEntity`, as the kit's `Spawner` spawns. The template's code is unchanged.

### 2.5 Tests

Names as written; each asserts what its name says.

- **Step 1, `core/tests/ecs/event_queue.rs`:**
  - `a_cursor_reads_each_event_once_across_reads_and_flushes`
  - `a_cursor_reads_events_sent_after_its_read_at_the_next` (a reader before its sender)
  - `a_cursor_reads_across_step_views_and_held_frames_once` (two steps; three held frames; a later step's `iter` against the cursor's read)
  - `a_cursor_counts_what_a_flush_dropped_before_its_read` (two flushes unread: `missed` is the dropped count; the rest read once)
  - `a_cursor_counts_the_frames_the_hold_cap_dropped` (257 held frames with events)
  - `a_new_cursor_starts_at_the_oldest_held_event` (a queue that has dropped events: `missed` 0)
  - `a_cursor_starts_over_on_a_replaced_queue` (the new queue's end below, at and above the old place: each read yields the new queue's events once, `missed` 0)
  - `a_moved_queue_keeps_its_readers` (a queue moved to another binding between reads: nothing read twice)
  - `two_cursors_read_one_queue_apart`
  - `a_read_knows_its_length_and_marks_every_event_read` (`len`, then a read dropped after one item yields nothing next time)
- **Step 1, `core/tests/ecs/command_buffer.rs`:**
  - `a_command_stays_24_bytes` (`size_of::<Command>()`)
  - `a_derive_after_a_run_adds_one_archetype_per_derived_type` (two types derived onto a bundle spawn's pending entity: `world.archetypes`, which tests under `ecs` reach, gains exactly `P+A` and `P+A+B` over the spawn without a source)
- **Step 1, `core/tests/assets/prefab.rs`**, each with registered test components:
  - `a_spawn_from_a_source_takes_what_it_inherits_and_nothing_else` (an unregistered type is not copied; a source without the type gives nothing)
  - `spawn_prefab_from_records_one_derive_after_the_run` (9 commands where `spawn_prefab` records 8)
  - `a_queued_spawn_derives_from_the_source_as_the_flush_finds_it` (an insert and a removal on the source recorded earlier in the same buffer)
  - `a_source_despawned_by_an_earlier_command_drops_the_spawn` (the entity count unchanged, the pending entity dead, the next spawn of the same prefab in the buffer keeps its own `with` value)
  - `a_source_despawned_by_a_later_command_still_spawns`
  - `the_prefab_and_a_with_bundle_keep_their_own_values`
  - `derive_from_source_keeps_a_spawn_chains_first_entity` (a test `Origin(Entity)`, F3's `Source` shape: A spawns B, B spawns C, both name A)
  - `the_first_registration_of_a_type_stands` (both forms return `false` the second time; the first derive stays)
- **Step 1, `tungsten/tests/fixed_step.rs`:** `a_cursor_reads_each_event_once_at_any_frame_rate_and_counts_what_was_dropped`. One event type sent by systems in `update`, in `fixed_update` after its reader and in `post_update`, under one counter; a cursor reader first in `fixed_update` and one in `post_update`.
  - At dt 1/30, 1/60, 1/144 and 1/1000 s, each reader reads the ids in order, none twice, `missed` 0, and the fixed reader lags by at most the ids sent since its last step.
  - A pause of five frames, then a resume until a step runs: the fixed reader's ids stay increasing with no repeat, and its `missed` equals the ids it skipped.
  - A time scale of 0.001 at 1/60 s, one step in about 1,000 frames, until a step runs: the fixed reader's `missed` is above 0, and every id sent before that step is either read once or counted missed, never both.
  - Throughout, the `post_update` reader misses nothing.
- **Step 2, `crates/tungsten-kit/src/tests/sound.rs`:** `a_replaced_queue_plays_its_events_once` (two events played, the queue replaced, two sent: both play, then nothing), written and run first against today's `sound.rs`, where the handled count already passes it. With it, the nine existing `SoundOnEvent` unit tests, the five `TriggerZone` unit tests, the doctests and `tungsten/tests/kit.rs`, all unchanged, pin the behaviour.
- **Step 3, `crates/tungsten-kit/src/tests/spawner.rs`:** `a_spawn_takes_what_its_spawner_passes_on_and_a_despawned_spawner_spawns_nothing`, on core alone: a test component registered with `inherit_from_source`; a despawn of the spawner recorded before its tick in a due frame.
- **Step 3, `tungsten/tests/kit.rs`**, each through `Harness` with a `Level` state whose `on_enter` spawns a spawner with its `SceneEntity`, and a control spawner outside any state with the same interval and phase:
  - `a_spawners_entities_leave_with_its_state`: the level's spawns carry `SceneEntity { "level" }` and the control's none; after the pop's frame nothing of the level remains and the control's spawns do.
  - `a_spawner_due_in_its_states_exit_frame_leaves_nothing`: dt 0.05 s, interval 0.1 s, the pop requested so that its frame is due; after it the level has no spawn and no spawner, the control has spawned in that frame, and the entity count is the control's.
- **Step 3, `tungsten/tests/prefabs.rs`:** `a_spawn_queued_in_on_exit_from_a_states_entity_leaves_nothing`: a state whose `on_exit` queues `spawn_prefab_from` from one of its own entities; after the exit's frame no entity carries its marker and the entity count is what it was before the state entered.

### 2.6 Cost and the freeze

- `send` is unchanged; `flush` adds two counter updates per registered queue per frame; a queue allocates one `Arc` when built, and a read compares one pointer; the command flush gains one match arm that no row reaches. The bench registers `SceneEntity` once at startup (A12). A sourced spawn records one more command, and when something is derived, makes one more move.
- New public surface, for the freeze: `EventCursor` (`new`, `read`, `Default`, `Clone`, `Debug`) and `EventRead` (`missed`, an exact-size, fused iterator, `Debug`) at `tungsten_core` and `tungsten_core::ecs`; `World::inherit_from_source` and `derive_from_source`; `CommandBuffer::spawn_prefab_from`. No compile break. One behaviour change: the kit's `Spawner` spawns from its spawner. Both get break-ledger rows (step 5).

## 3. Steps

Helpers the run writes in its scratchpad, never in the tree, since the rtk hook rewrites piped `cargo`, `git diff` and `comm` output:

- `snapshot_delta.py <file>` prints `added N removed M`, the `Counter` difference of `git cat-file -p HEAD:<file>` and the file.
- `tests_listed.py <package> <name>…` runs `cargo test -p <package> --lib --locked -- --list` through `subprocess` and prints `missing: none` or the names it lacks.
- `tests_kept.py <package> <test file>…` reads each file's `#[test]` function names at `HEAD` (`git cat-file`) and prints `dropped: none` or those the same listing lacks, so no existing test is removed or renamed.
- `new_docs.py` runs `cargo rustc -p tungsten-core --lib --locked -- -W missing_docs` through `subprocess` and prints `undocumented new items: N`, counting the `missing documentation` warnings on lines that `git diff -U0 HEAD -- crates/tungsten-core/src` adds.

### Step 1: The event cursor and spawn inheritance in core

- **Files:** `crates/tungsten-core/src/ecs/event_queue.rs`, `crates/tungsten-core/src/ecs/command_buffer.rs`, `crates/tungsten-core/src/ecs/world.rs`, `crates/tungsten-core/src/ecs/mod.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/assets/prefab.rs`, `crates/tungsten-core/src/tests/ecs/event_queue.rs`, `crates/tungsten-core/src/tests/ecs/command_buffer.rs`, `crates/tungsten-core/src/tests/assets/prefab.rs`, `crates/tungsten/src/tests/fixed_step.rs`, `api/tungsten-core.txt`
- **Change:** in order: (a) the queue's sequence and identity, `EventCursor` and `EventRead` per §2.1, exported from `ecs/mod.rs` and `lib.rs`, with a rustdoc example of a reader across a flush; (b) the registry, `Command::Derive` with its recorder, the flush arm and `CommandBuffer::spawn_prefab_from` per §2.2, with the prefab module doc's spawn paragraph; (c) the tests of §2.5's step 1; (d) `cargo fmt --all`, then `just api`. Then the level C API critique (the skill's "Running steps", `--from` on `api/` and `templates/`), its findings recorded in §9 as `C.<n>`, step 1 changed where a finding holds, all before step 2.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0
  - `cargo test -p tungsten-core --locked -q` → exit 0, the existing event-queue, command-buffer and prefab tests passing with their bodies unchanged (only imports may change)
  - `python3 -B <scratchpad>/tests_listed.py tungsten-core <the twenty step 1 core names of §2.5>` → `missing: none`; `python3 -B <scratchpad>/tests_kept.py tungsten-core crates/tungsten-core/src/tests/ecs/event_queue.rs crates/tungsten-core/src/tests/ecs/command_buffer.rs crates/tungsten-core/src/tests/assets/prefab.rs` → `dropped: none`; `python3 -B <scratchpad>/tests_kept.py tungsten crates/tungsten/src/tests/fixed_step.rs` → `dropped: none`
  - `cargo test -p tungsten --lib --locked testing::fixed_step` → exit 0; `python3 -B <scratchpad>/tests_listed.py tungsten a_cursor_reads_each_event_once_at_any_frame_rate_and_counts_what_was_dropped` → `missing: none`
  - `just lint` → exit 0
  - `python3 -B <scratchpad>/new_docs.py` → `undocumented new items: 0`
  - after `just api`: `git diff --quiet HEAD -- api/tungsten.txt api/tungsten-kit.txt api/tungsten-render.txt` → exit 0; `python3 -B <scratchpad>/snapshot_delta.py api/tungsten-core.txt` → `removed 0` with `added` above 0, the count kept for the ledger row; `rg -c '^pub struct tungsten_core::(ecs::)?EventCursor<T>$' api/tungsten-core.txt` → 2; `rg -c '^pub struct tungsten_core::(ecs::)?EventRead<' api/tungsten-core.txt` → 2; `rg -c '^pub fn tungsten_core::World::inherit_from_source<' api/tungsten-core.txt` → 2; `rg -c '^pub fn tungsten_core::World::derive_from_source<' api/tungsten-core.txt` → 2; `rg -c '^pub fn tungsten_core::CommandBuffer::spawn_prefab_from\(' api/tungsten-core.txt` → 2
  - the API critique ran, or is recorded as not run with its error line (§9)
- **Moves:** `api/tungsten-core.txt`, additions only.

### Step 2: The kit's readers onto the cursor

- **Files:** `crates/tungsten-kit/src/sound.rs`, `crates/tungsten-kit/src/trigger.rs`, `crates/tungsten-kit/src/tests/sound.rs`
- **Change:** per §2.4, in order: §2.5's step 2 test added and run against today's `sound.rs`; `SoundOnEvent`'s reader on an `EventCursor<E>` with its rustdoc's second paragraph; `TriggerZone`'s system on an `EventCursor<SensorEntered>`. No other test file changes.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0
  - `cargo test -p tungsten-kit --lib --locked sound::tests::a_replaced_queue_plays_its_events_once` → exit 0 before `sound.rs` changes and again after
  - `cargo test -p tungsten-kit --locked -q` → exit 0
  - `cargo test -p tungsten --lib --locked kit_tests` → exit 0
  - `git diff --quiet HEAD -- crates/tungsten-kit/src/tests/trigger.rs crates/tungsten/src/tests/kit.rs` → exit 0, and `python3 -B <scratchpad>/snapshot_delta.py crates/tungsten-kit/src/tests/sound.rs` → `removed 0` (the behaviour pinned by unchanged tests)
  - `rg -c 'handled' crates/tungsten-kit/src/sound.rs` → no match (4 before); `rg -c 'EventCursor' crates/tungsten-kit/src/sound.rs` → at least 1
  - `rg -c 'sensors\.iter_current' crates/tungsten-kit/src/trigger.rs` → no match (1 before); `rg -c 'EventCursor(::)?<SensorEntered>' crates/tungsten-kit/src/trigger.rs` → at least 1
  - `just lint` → exit 0
  - `just api`, then `git diff --quiet HEAD -- api/tungsten-kit.txt api/tungsten.txt` → exit 0
- **Moves:** none.

### Step 3: Kit spawns leave with their state

- **Files:** `crates/tungsten/src/plugins.rs`, `crates/tungsten/src/state.rs`, `crates/tungsten-kit/src/spawner.rs`, `crates/tungsten-kit/src/tests/spawner.rs`, `crates/tungsten/src/tests/kit.rs`, `crates/tungsten/src/tests/prefabs.rs`, `templates/basic/AGENTS.md`, `docs/getting-started.md`
- **Change:** per §2.4: `StatePlugin`'s registration and the three docs; the `Spawner`'s tick and rustdoc; §2.5's step 3 tests; the template's and the guide's clauses.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0
  - `cargo test -p tungsten-kit --locked -q` → exit 0; `python3 -B <scratchpad>/tests_listed.py tungsten-kit a_spawn_takes_what_its_spawner_passes_on_and_a_despawned_spawner_spawns_nothing` → `missing: none`
  - `cargo test -p tungsten --lib --locked kit_tests` and `cargo test -p tungsten --lib --locked asset_loader::prefab` (the module `tests/prefabs.rs` belongs to) → exit 0; `python3 -B <scratchpad>/tests_listed.py tungsten a_spawners_entities_leave_with_its_state a_spawner_due_in_its_states_exit_frame_leaves_nothing a_spawn_queued_in_on_exit_from_a_states_entity_leaves_nothing` → `missing: none`
  - `python3 -B <scratchpad>/tests_kept.py tungsten crates/tungsten/src/tests/kit.rs crates/tungsten/src/tests/prefabs.rs` and `python3 -B <scratchpad>/tests_kept.py tungsten-kit crates/tungsten-kit/src/tests/spawner.rs` → `dropped: none`
  - `git diff --quiet HEAD -- crates/tungsten/src/tests/plugins.rs templates/basic/src templates/basic/tests` → exit 0, and `cargo test -p tungsten --lib --locked plugins` and `cargo test -p tungsten-template-basic --locked -q` → exit 0 (`DefaultPlugins`' and the template's resolved schedules pinned unchanged)
  - `rg -c 'state marker' crates/tungsten-kit/src/spawner.rs` → no match (1 before); `rg -c 'spawn_prefab_from' crates/tungsten-kit/src/spawner.rs` → at least 1
  - `rg -c 'spawn_prefab_from' templates/basic/AGENTS.md` → 1 and `rg -c 'spawn_prefab_from' docs/getting-started.md` → 1 (0 before)
  - `just lint` → exit 0; `cargo test --workspace --locked -q` → exit 0
  - `just api`, then `git diff --quiet HEAD -- api/tungsten.txt api/tungsten-kit.txt api/tungsten-render.txt` → exit 0
  - `python3 -B scripts/check-repo.py` → `0 error(s)`
- **Moves:** none.

### Step 4: Every CPU row and the physics hashes

- **Files:** none in the tree. Evidence goes in `perf-runs/<YYYYMMDD>-m60-events-ownership/` (gitignored: list it with `fd -I`), scratch trees and target dirs in `target/m60-*`, which the step deletes.
- **Change:** one sitting under the [capture rules](../../../perf/profiling-workflow.md#comparison-rule-and-capture-rules) and workflow §6, after steps 1 and 3, with step 2 when it landed. "Every CPU row" is the six rows whose owned metrics are CPU stages and systems: `physics`, `physics-sparse`, `ecs`, `churn`, `particles` and `integrated` (Q13).
  1. **Before the sitting.** `df -h target` reads at least 10 GiB free (17 GB on 2026-10-16). Build the base tree from `HEAD` (`git archive HEAD | tar -x -C target/m60-base`; `HEAD` is 0.79's code). Prebuild both binaries with the runner's command (`RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release -p example-02-bench`), each tree in its own target dir, each build run twice to see `Finished`, and tell the two apart by `sha256sum`. Write a placement listing from `nm -C -n` on both: each owned row's hot functions modulo 64 (`World::flush_reusing`, the `EventQueue` flushers, `physics_step`, the `ecs` and `churn` systems, the particle systems). It is information only.
  2. **The sitting, one job.** Launch it in the background and keep the session idle until it completes, since it outlasts the 10-minute foreground limit; nothing of this session runs or edits beside it. It waits for 60 s with no `nxcodec.bin`. It captures four suites, `just perf suite --only physics,physics-sparse,ecs,churn,particles,integrated --repeat 5`, in the order base-a, candidate-a, base-b, candidate-b (the base from its own tree), and sets a suite the runner marks invalid aside and reruns it once. It copies the base suites out of `target/m60-base/perf-runs/` with `cp -a`, checked by `diff -rq`'s exit code. It runs the compares base-a → base-b and candidate-a → candidate-b (the A/As), and base-a → candidate-a and base-b → candidate-b (the direct pairs), then a `summarize.py` of every owned metric per compare, then `just physics-release`.
  3. **Reruns, in the same job.** In an A/A, the A/A note's `ecs` per-run-mode rows (`stats_decay`, `regen`, `follow`, `buffs`, `team_bags`, `accelerate`, `integrate`) may trip with nothing changed; such a trip is reported and needs no rerun when the row's direct pairs read clean. Any other trip, an owned metric `regressed` or `improved` in an A/A or `regressed` in a direct pair, takes one rerun of its row's four captures in the same order (`--only <row>`), whose compares decide. `particles` `animate_sprites` p95 and `particle_count_refresh` p50 and p95 have tripped on unchanged builds in earlier sittings, so expect them. A row still tripping after its rerun gets one aligned pair as evidence: both trees built with `TUNGSTEN_PERF_RUSTFLAGS="-C force-frame-pointers=yes -C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=6"`, each in its own target dir, the row captured once a side. Then the stop (§7).
  4. **After.** Write the evidence README, naming each tree by hand: the base tree under `target/` records the enclosing repository's commit and dirty hash. Compare `git status --short` with its reading before the sitting. Delete the scratch dirs with `shutil.rmtree` and quote their absence.
- **Done-when:**
  - every suite valid, or valid at its one rerun
  - both A/As: no owned metric `regressed` or `improved`, the A/A note's `ecs` per-run-mode rows excepted, through the reruns
  - both direct pairs: no owned metric `regressed`, through the reruns
  - the first-run digests match in all four compares: the behaviour of every row is unchanged
  - `just physics-release` → exit 0, which asserts the determinism hash `0x088e_c07a_73c1_b168` and the containment hash `0xaee2_72e0_1ffc_4e4c`
  - `python3 -B -c 'import glob; print(glob.glob("target/m60-*"))'` → `[]`
  - the evidence row gives per row each owned metric's base and candidate values with its verdict, peak RSS per row (reported, judged by nobody), the placement listing's moves, and the suite paths
- **Moves:** none.

### Step 5: Records

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w13-kit.md`
- **Change:** each record per what landed:
  1. **Decision.** Recheck the next free ID (`rg -n '^## D-1' DECISIONS.md | tail -3`, `git status --short`), then write 170 with tungsten-decision (§5): the decision per §2; the why from R1 and R7, §2.3's frame and A9's placement, with step 4's numbers; the consequences: the marker lines, the tests of §2.5, the two ledger rows, the recorded limit of §2.3 and the follow-ups. Add the index row at the end of ECS / Runtime Flow, and each amended row's note.
  2. **DESIGN.md.** Add a `### Event Cursors and Spawn Inheritance — M60` section after M58's, from §2.1–§2.4. At `:459`, when step 2 landed, "counting what it handled so that" becomes "reading through an `EventCursor`, so that". At `:471`, add that a prefab queued from a source takes the source's `SceneEntity` (`spawn_prefab_from`). At `:477`, the spawner spawns from its entity.
  3. **Known issues.** Rewrite `:52` (`Spawned entities and states`): a prefab queued from a source (`spawn_prefab_from`), as the kit's `Spawner` spawns, takes its source's `SceneEntity` under `StatePlugin`, and a source despawned earlier in the frame's commands spawns nothing. A prefab spawned with no source, from a system or a Tiled map, still carries none unless it passes one through `with`. An entity that takes a state's marker in its exit frame after the dispatcher's listing, ahead of its despawns, outlives the state: one queued from a state's entity before `state_dispatcher`, or spawned at once in `on_exit` with the marker (§2.3). When step 2 was skipped, add a row: `SoundOnEvent` keeps its handled count and `TriggerZone` its `iter_current`, named `M60 step 2`.
  4. **LLM index.** Three label edits, each old text then its new one, `<new ID>` standing for the decision's backticked ID; at `0732305` they take the file from 12,284 to 12,280 bytes:

     ```text
     Event lifetime/flush, events held for the next step (`D-040`, `D-139`)
     Event flush, holds, `EventCursor` (`D-040`, `D-139`, <new ID>)

     Prefabs, Tiled objects, tilemap builder (`D-157`)
     Prefabs, spawn inheritance, Tiled objects, tilemap builder (`D-157`, <new ID>)

     despawn outside, sound per event, cursor, hierarchy, spawner, platformer, top-down mover, health,
     despawn, sound per event, cursor, hierarchy, spawner, controllers, health,
     ```
  5. **Break ledger** (w04): two rows. One for the additions, as §2.6 lists them, with step 1's `added` count and "none is removed". One for the `Spawner`'s spawns from its spawner, a behaviour change and not a compile break. Each ends `| 0.80 (M60) |`.
  6. **w13-kit.md.** Its revision line (`:10`) gains W13e's landing in 0.80 (M60, the new ID). The M50 follow-up for an exactly-once reader (`:472`, when step 2 landed) and the M51 one for state ownership (`:477`) are marked landed in M60, and `:509`'s known-issues pointer closed. The garbled lines at `:487-494` (A17) stay as they are.
- **Done-when:**
  - `rg -c '^## D-17[0] — ' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-17[0]:\*\*' DECISIONS.md` → 3, or 2 when step 2 was skipped (no `D-155` marker)
  - `rg -c '^\| `D-17[0]` \|' docs/DECISION_INDEX.md` → 1; `rg -c '^\| `D-040` .*D-17[0]' docs/DECISION_INDEX.md` → 1; `rg -c '^\| `D-157` .*D-17[0]' docs/DECISION_INDEX.md` → 1; `rg -c '^\| `D-155` .*D-17[0]' docs/DECISION_INDEX.md` → 1 when step 2 landed, no match when it was skipped
  - `rg -c '^### Event Cursors and Spawn Inheritance — M60$' DESIGN.md` → 1; `rg -c 'counting what it handled' DESIGN.md` → no match when step 2 landed (1 before)
  - ``rg -c 'carry no `SceneEntity`' docs/known-issues.md`` → no match (1 before); `rg -c 'spawn_prefab_from' docs/known-issues.md` → at least 1 (0 before)
  - `just ctx` → exit 0; `python3 -B -c 'import os; print(os.path.getsize("docs/LLM_INDEX.md"))'` → at most 12288; `rg -c 'EventCursor' docs/LLM_INDEX.md` → 1; `rg -c 'spawn inheritance' docs/LLM_INDEX.md` → 1
  - `rg -c '\| 0\.80 \(M60\) \|$' docs/plans/1.0/w04-api-freeze.md` → 2
  - `rg -c 'M60' docs/plans/1.0/w13-kit.md` → at least 1 (0 before)
  - `just repo-check` → exit 0
- **Moves:** none.

### Step 6: Release 0.80

- **Files:** `CHANGELOG.md`, `DESIGN.md` (step 5's section beside the cut's status line), `Cargo.toml`, `Cargo.lock`, the register row (`docs/plans/1.0/implementation-plan.md` §10), the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 (§8) and this step's evidence row with `status: done`. Then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.80.0` once, every release check once after it, then the preflight with `--message`. Before the cut: `git status --short` and `rg -n '^## D-1' DECISIONS.md | tail -3` show other sessions' files, which the report names and the run never reverts; `git diff --quiet HEAD -- Cargo.lock` → exit 0 (no dependency moved); DESIGN.md's status line gains event cursors and spawn inheritance in a few words.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.80.0 --repo JosephGibson/Tungsten --message 'Update 0.80: M60 event cursors and spawn inheritance'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: the plan prompt for W13f, `docs/plans/1.0/phase6-milestone-61-kit-core-data-assets.md`, from [tungsten-next's prompts](../../../../.claude/skills/tungsten-next/prompts.md).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Where does the cursor live? | Core, beside `EventQueue` (w13-kit Q7): the umbrella's readers share the problem, and core cannot name the kit |
| Q2 | Does a read move the cursor at once, or as its iterator is driven? | At once: `EventRead` borrows the queue alone, knows its length before iterating and never yields an event twice. A reader that stops early has chosen to drop the rest |
| Q3 | Where does a new cursor start? | At the oldest event the queue holds at its first read, missing nothing: a reader added mid-run should not count drops from before it existed |
| Q4 | Is `missed` per read or kept on the cursor, and of what type? | Per read, `u64`, as the sequence is |
| Q5 | Does F1 land the function form (`derive_from_source`) as well as the copy? | Yes: F3's `Source` needs it (w13-kit §7), and a chain-origin test pins its shape now, so F3 adds no core API |
| Q6 | Do derived types land before or after the spawn's own? | After, in one move that skips each type the spawn already has: the prefab's archetype chain is shared (A9), and the prefab's and `with`'s values win |
| Q7 | Is there a spawn from a source made at once, beside the queued one? | No: inheritance happens at the command flush (the card's words), and a spawn made at once in a state's exit frame would take the marker after the dispatcher's listing and outlive the state (critique 2). `PreparedPrefab` and `spawn` are unchanged, and a dead source can only drop a queued spawn |
| Q8 | A spawn from a source that is not a prefab (`CommandBuffer` bundles)? | Not in F1: every kit spawn is a prefab spawn (w13-kit §7). A follow-up for when a game asks |
| Q9 | Who registers `SceneEntity`? | `StatePlugin`, beside the dispatcher that despawns by it; an app without it has no state cleanup to inherit |
| Q10 | Does `TriggerZone` move onto the cursor in F1? | Yes: w13-kit §2 rule 4 and §11 ("Every kit reader uses `EventCursor`") make F1 its natural home, and its tests pin the behaviour unchanged |
| Q11 | One decision or two? | One, 170, as w13-kit §13 says: one landed decision per milestone |
| Q12 | Do the template and the guide learn `spawn_prefab_from`? | One clause each (workflow §5 line 6): they tell a game how a spawn gets its state, and that changes; the template's code does not |
| Q13 | Which rows are "every CPU row"? | The six whose owned metrics are CPU stages and systems; the GPU rows own the extract, the encode and passes, which this code does not run |
| Q14 | An entity that takes a state's marker in its exit frame after the listing, ahead of the despawns, outlives the state (§2.3). Fix it? | No: a recorded limit (known issues). No kit system does it. A sweep at the flush by state ID would also take the entities of a state that replaces itself, so the fix is the state stack's, not core's |
| Q15 | How does a reader survive a replaced queue? | Through the queue's identity (§2.1, critique 2.1): a cursor that finds another queue starts at its oldest event, missing nothing, as `SoundOnEvent`'s handled count read a fresh queue. The alternative, round 1's rule that a replaced queue takes new cursors, gave the kit's installed readers no way to take one |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

- 170: W13e landed (M60): `EventCursor` and `EventRead`, a reader's place in an `EventQueue` by sequence; spawn inheritance (`World::inherit_from_source`, `derive_from_source`, `CommandBuffer::spawn_prefab_from`) applied at the flush, a source despawned by an earlier command dropping the spawn; `StatePlugin` registers `SceneEntity`; `SoundOnEvent` and `TriggerZone` read through cursors; the kit's spawner spawns from its spawner. Amends `D-040` (its readers), `D-155` (its sound clause, when step 2 lands) and `D-157` (its spawning and spawner clauses); takes up the cursor `D-139` left open.

The next free ID at `0732305` on 2026-10-16; step 5 checks it again.

## 6. Invariants

- **May move:** `api/tungsten-core.txt`, by additions only (step 1), with the added count in the ledger row and the `CHANGELOG.md` line.
- **Must not move:**
  - the determinism hash and the pinned containment hash (step 4's `just physics-release`), the row digests (step 4), `gpu-visual.png` and the post, transition and UI overlay references (no render, extract or UI path changes, so `just visual` is not run)
  - `DefaultPlugins`' and the template's resolved schedules (step 3)
  - `api/tungsten.txt`, `api/tungsten-kit.txt` and `api/tungsten-render.txt` (steps 1–3); `Cargo.lock` before the cut (step 6)
  - `iter`, `iter_current`, `len`, `is_empty` and the holds (the existing queue tests); `SoundOnEvent`'s and `TriggerZone`'s behaviour (their unchanged tests); a prefab spawn with no source (the existing prefab tests)
  - the kit on core alone, core naming no kit or umbrella type, opt-in plugins, two background threads, no new dependency (`D-015`), prefab and scene file formats

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with the steps that don't depend on it. Steps 2 and 3 depend on step 1; step 4 on steps 1 and 3; step 5 records what landed. The release needs steps 1, 3, 4 and 5. Step 2 may be skipped: its records follow §3 step 5's per-outcome checks, and its tests are already in the release's `just check`.
- The step 1 API critique finds a shape tied to one genre, or a finding that holds and cannot be fixed inside step 1's files: redesign within §2's semantics or stop before step 2 (w13-kit §14).
- `just api` shows a removed line in any snapshot, an unplanned break: stop the step.
- A row still tripping after its rerun (step 4's item 3), or a moved physics hash: stop at the release (w13-kit §14, "F1 moves a CPU row past its noise"), with the aligned pair and the placement listing recorded for the owner; the release waits for a release session after the owner has seen why (workflow §3).
- The machine never quiets: the job keeps waiting for the encoder and never captures beside it; the session never captures with `--allow-background`.
- A file outside §files to touch needs a change: stop the step, restore its files and record the need as a follow-up.
- The LLM index's edits do not fit within 12,288 bytes as planned: shorten the edited rows' labels further; never drop a row's paths.
- Another session takes 170: use the next free ID and say so in the report.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–4, release | Steps 1–3: fmt, the crates' tests, lint and `just api`; step 4: every CPU row and `just physics-release`; the release step runs `just check`, `just smoke`, `just script-test` and the rest of the release checks once, after the cut. `just visual` is not owed: no render, extract or UI path changes |
| 3 Invariants | 3, 4, 6 | §6 |
| 4 Decisions | 5 | One, 170 |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 5 | Rustdoc and the snapshot in step 1 (`new_docs.py`); two ledger rows in step 5 |
| 6 Template and guide | 3 | One clause each; the template's code unchanged |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 5 | No new file; three row edits within the budget |
| 8 Design | 5 | The M60 section and three edits to M50's and M51's |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 5, release | Known issues `:52` rewritten; no backlog row (nothing cut); the gap log has no row (`acceptance-game.md`); w13-kit's revision line and follow-ups; the register row and the owned fields in the release (`python3 -B scripts/roadmap.py summary` supplies them) |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

**Round 1**, 2026-10-16: the `critique` skill with `--repo` on the tree at `0732305` with this plan uncommitted, §9 and the evidence log cut with `sed`. gpt-6-astra, effort max (a level C plan's first round), context 872,000, output cap 4,000, 334.1 s, 826,587 tokens in (731,392 cached), 10,316 out (6,504 reasoning), 34 commands, 287.6 KB output (largest 19.2 KB), 5h window 54% → 76%, week 29% → 32%. Four findings, each checked against the lines it cites; all accepted. They changed the API (`spawn_prefab_from` in place of `PreparedPrefab::with_source`), tests and done-when checks, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Queue replacement silently loses events unless the cursor happens to be ahead | `core/ecs/world.rs:623-625` and `core/ecs/resource.rs:16-18`: `insert_resource` swaps a queue, and §2.1 caught only a cursor past the new end | Accepted | §2.1's contract: a replaced queue takes new cursors (A18, Q15). Queue identity rejected: a World counter misses a queue built in code, and an address changes when a queue moves, which would replay its events. Superseded by 2.1, which found the contract unusable for installed readers and adds an identity neither objection applies to |
| 2 | [MEDIUM] Immediate sourced spawns can survive an exit even when created after the dispatcher's cleanup commands | `tungsten/state.rs:464-468` (the listing, then `on_exit`); `core/assets/prefab.rs:260-266` (a spawn now flushes a private buffer) | Accepted | No immediate form: `CommandBuffer::spawn_prefab_from` replaces `PreparedPrefab::with_source` (§2.2, Q7), so every sourced spawn derives at the flush, behind the dispatcher's despawns. `a_spawn_queued_in_on_exit_from_a_states_entity_leaves_nothing` joins step 3. §2.3, Q14 and the known-issues text name what is left, including the case from before M60 of an entity given the marker at once in `on_exit` |
| 3 | [MEDIUM] Reading every fixed step does not guarantee zero missed events | `core/time.rs:256-264` (any positive scale is kept); `core/ecs/event_queue.rs:110-114` (the cap drops the oldest held frame) | Accepted | `missed`'s rustdoc and §2.1's Views bullet name the cap and deep slow motion; the fixed-step harness test gains a phase at scale 0.001 that expects a positive `missed`, every id read once or counted, never both |
| 4 | [MEDIUM] One bundle move still creates intermediate archetypes for inherited components | `core/ecs/world.rs:193-199` (`apply_bundle` runs `insert_run`); `core/ecs/storage.rs:194-200` (an edge per missing type) | Accepted | A9 and §2.2 count one archetype per derived type at the end of the prefab's chain (`P+A`, then `P+A+B`), pinned by `a_derive_after_a_run_adds_one_archetype_per_derived_type`. Deriving after the run still stands: before it, each prefab shape's whole chain would be copied |

**Round 2**, 2026-10-16, started at the owner's request with the 5h window at 76%: the same reviewer on the revised plan, §9 and the evidence log cut with `sed`, a `--focus-file` naming round 1's changed sections and its one rejected suggestion. gpt-6-astra, effort xhigh, context 872,000, output cap 4,000, 252.4 s, 893,651 tokens in (797,696 cached), 6,469 out (3,427 reasoning), 20 commands, 329.6 KB output (largest 55.1 KB), 5h window 76% → 97%, week 32% → 36%. It found round 1's archetype and exit-frame changes consistent with the repository, and raised one finding, checked and accepted. There is no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Installed kit readers cannot take the fresh cursors that queue replacement requires | `crates/tungsten-kit/src/sound.rs:119-126` and `crates/tungsten-kit/src/trigger.rs:95-102` (each cursor in a private system closure); `sound.rs:151-154` (the handled count plays a fresh queue's two events, where round 1's rule would skip them); `core/schedule.rs:344-351` (a second install fails on its name) | Accepted | §2.1 gives each queue an identity, an `Arc<()>` that cursors hold as a `Weak`, so a read that finds another queue starts at its oldest event: replacing a queue restarts every reader, installed ones included, as the handled count did (Q15, A18). `a_cursor_starts_over_on_a_replaced_queue` and `a_moved_queue_keeps_its_readers` replace round 1's two replacement tests, and step 2 adds `a_replaced_queue_plays_its_events_once` through the plugin, run first against today's `sound.rs` |

**Step 1 API critique**, 2026-10-16, 09:07–09:09 UTC: the `critique` skill with `--raw --repo` on `git --no-pager diff HEAD -- api/ templates/` and the untracked files under them, 12,538 characters, under the soft limit, so no `--force`. It did not run at step 1's end: the 5h window read 97% at 06:05 UTC, resetting at 09:05, and API critiques have cost 9–20 points. A background waiter queued it behind the reset and steps 2–5 ran meanwhile (M52's practice), with step 1's files backed up; so the critic read the tree with steps 2–5 in it, and its template diff held step 3's clause. gpt-6-astra, effort xhigh, context 872,000, output cap 4,000, 107.2 s, 296,855 tokens in (242,944 cached), 2,827 out (1,279 reasoning), 21 commands, 148.1 KB output (largest 35.8 KB), 5h window 3% → 19%, week 36% → 39%. **No concerns:** it checked cursor sequencing, missed-event accounting, queue replacement, partial reads, and inheritance's precedence and despawn order, and found the template's guidance backed by `crates/tungsten/src/plugins.rs:117` (the registration) and `crates/tungsten-kit/src/spawner.rs:181` (the spawner's call), both checked. No `C.<n>` rows; step 1 is unchanged, so steps 2–5's checks and step 4's sitting stand.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-16 | Pass; the API critique ran after the Codex reset, after steps 2–5, with no concerns (§9) | `cargo fmt --all -- --check` exit 0; `cargo test -p tungsten-core --locked -q` exit 0, 1,108 passed, 0 failed (the existing event-queue, command-buffer and prefab tests unchanged); `tests_listed.py tungsten-core` (the 20 names) → `missing: none`; `tests_kept.py tungsten-core` (three files) → `dropped: none`; `tests_kept.py tungsten crates/tungsten/src/tests/fixed_step.rs` → `dropped: none`; `cargo test -p tungsten --lib --locked testing::fixed_step` 14 passed; its listing → `missing: none`; `just lint` exit 0; `new_docs.py` → `undocumented new items: 0` (886 `missing_docs` warnings, none on an added line); after `just api`: the umbrella, kit and render snapshots unchanged (exit 0), `snapshot_delta.py api/tungsten-core.txt` → `added 76 removed 0`; the five `rg -c` counts 2, 2, 2, 2, 2; the API critique: `## No concerns` (§9, its raw copy in the session's scratchpad) | `crates/tungsten-core/src/ecs/{event_queue,command_buffer,world,mod}.rs`, `lib.rs`, `assets/prefab.rs`, the three core test files, `crates/tungsten/src/tests/fixed_step.rs`, `api/tungsten-core.txt` |
| 2 | 2026-10-16 | Pass | `cargo fmt --all -- --check` exit 0; `a_replaced_queue_plays_its_events_once` passed on 0.79's `sound.rs` and again after; `cargo test -p tungsten-kit --locked -q` 155 + 14 doctests passed; `cargo test -p tungsten --lib --locked kit_tests` 26 passed; `git diff --quiet HEAD -- crates/tungsten-kit/src/tests/trigger.rs crates/tungsten/src/tests/kit.rs` exit 0; `snapshot_delta.py crates/tungsten-kit/src/tests/sound.rs` → `added 19 removed 0`; `handled` no match, `EventCursor` 4 in `sound.rs`; `sensors\.iter_current` no match, `EventCursor(::)?<SensorEntered>` 2 in `trigger.rs`; `just lint` exit 0; after `just api` the kit and umbrella snapshots unchanged (exit 0) | `crates/tungsten-kit/src/{sound,trigger}.rs`, `crates/tungsten-kit/src/tests/sound.rs` |
| 3 | 2026-10-16 | Pass | `cargo fmt --all -- --check` exit 0; `cargo test -p tungsten-kit --locked -q` 156 + 14 passed, its listing → `missing: none`; `kit_tests` 28 and `asset_loader::prefab` 11 passed, the three names → `missing: none`; `tests_kept.py` → `dropped: none` for both packages; the pinned files unchanged (exit 0), `plugins` 13 and `tungsten-template-basic` 11 passed; `state marker` no match, `spawn_prefab_from` 2 in `spawner.rs`, 1 in the template's `AGENTS.md` and 1 in the guide (0 before each); `just lint` exit 0; `cargo test --workspace --locked -q` exit 0, 2,168 passed, 0 failed, 8 ignored; after `just api` the umbrella, kit and render snapshots unchanged, core still `added 76 removed 0`; `check-repo.py` → `0 error(s)`. Negative control: without `StatePlugin`'s registration the two `kit_tests` fail, restored and checked with `cmp` | `crates/tungsten/src/{plugins,state}.rs`, `crates/tungsten-kit/src/spawner.rs`, `crates/tungsten-kit/src/tests/spawner.rs`, `crates/tungsten/src/tests/{kit,prefabs}.rs`, `templates/basic/AGENTS.md`, `docs/getting-started.md` |
| 4 | 2026-10-16 | Pass | One sitting, 06:17–06:30 UTC, four `suite --only physics,physics-sparse,ecs,churn,particles,integrated --repeat 5`, every suite valid on its first attempt (base tree `git archive` of `0732305`, binaries base `07ba52952cb36536`, candidate `6df10d1bfc7d49fd`, second build passes compiled nothing); both A/As 0 owned `regressed` or `improved` (base 28 unchanged, 11 noisy; candidate 32 and 7); both direct pairs 0 owned `regressed` (`--fail-on regressed` exit 0; A 24 unchanged, 15 noisy; B 28 and 11), so no rerun and no aligned pair; first-run digests match in all six rows of all four compares; `just physics-release` exit 0 (7 + 1 + 7 passed, the two hashes asserted, their tests unchanged); `glob("target/m60-*")` → `[]`; `git status --short` identical before and after; no encoder, no foreign build, the other sessions idle. Placement listing: 43 of 44 hot functions moved modulo 64 (information only). Per row, each owned metric as direct pair A base→candidate, then pair B, mean of the runs in ms (u `unchanged`, n `noisy`): `physics`: physics_step p50 6.172→6.180 u, 6.166→6.182 u; physics_step p95 6.318→6.366 u, 6.326→6.330 u; update p95 6.372→6.428 u, 6.382→6.388 u; peak RSS 126.2→125.9, 126.2→126.1 MiB · `physics-sparse`: physics_step p50 3.688→3.674 u, 3.694→3.672 u; physics_step p95 3.750→3.748 u, 3.748→3.750 u; peak RSS 121.3→121.3, 121.1→121.3 MiB · `ecs`: update p50 10.416→10.520 u, 10.468→10.518 u; update p95 10.878→10.842 u, 10.820→10.902 u; brain p50 2.128→2.172 u, 2.130→2.170 u; cooldowns p50 0.452→0.446 n, 0.448→0.450 u; buffs p50 0.662→0.642 n, 0.654→0.642 n; regen p50 0.092→0.084 n, 0.086→0.084 u; stats_decay p50 0.252→0.182 n, 0.220→0.184 n; accelerate p50 0.230→0.240 n, 0.236→0.236 u; follow p50 1.732→1.822 n, 1.800→1.820 n; integrate p50 0.230→0.238 n, 0.234→0.236 u; bounds_wrap p50 0.316→0.312 u, 0.314→0.312 u; heading p50 2.512→2.520 u, 2.520→2.518 u; tint p50 0.376→0.384 u, 0.378→0.382 u; age_phase p50 0.460→0.468 u, 0.464→0.468 u; team_bags p50 0.240→0.246 n, 0.246→0.246 u; faction_histogram p50 0.260→0.260 u, 0.260→0.260 u; peak RSS 157.0→157.0, 156.8→156.9 MiB · `churn`: flush p50 2.438→2.450 u, 2.420→2.456 n; flush p95 2.692→2.702 u, 2.666→2.724 u; churn_scan p50 0.342→0.324 n, 0.342→0.326 n; churn_despawn p50 0.010→0.010 u, 0.010→0.010 u; churn_toggle p50 0.040→0.044 u, 0.040→0.040 u; churn_spawn p50 0.332→0.348 n, 0.330→0.330 u; peak RSS 135.4→135.7, 135.6→135.5 MiB · `particles`: particle_count_refresh p50 0.644→0.634 n, 0.638→0.654 n; particle_count_refresh p95 0.866→0.820 n, 0.924→0.834 n; particle_emit p50 0.098→0.096 u, 0.098→0.092 u; particle_emit p95 0.130→0.128 u, 0.134→0.126 n; particle_tick p50 1.466→1.468 u, 1.472→1.472 u; particle_tick p95 1.536→1.542 u, 1.552→1.564 u; animate_sprites p50 0.240→0.238 u, 0.240→0.238 u; animate_sprites p95 0.286→0.282 n, 0.280→0.282 n; peak RSS 139.1→139.2, 139.7→139.7 MiB · `integrated`: total p50 8.060→8.062 u, 8.038→7.994 u; total p95 8.550→8.504 u, 8.512→8.458 u; total p99 9.410→9.054 n, 9.122→9.338 n; total jitter 1.350→0.992 n, 1.084→1.344 n; peak RSS 182.0→182.7, 180.9→182.1 MiB | `perf-runs/20261016-m60-events-ownership/` (`README.md`, `summary.md`, `placement.md`, `suites.txt`, `base/`, `compare-*`, `scripts/`); candidate suites `perf-runs/20261016T062254Z-suite`, `perf-runs/20261016T062646Z-suite` |
| 5 | 2026-10-16 | Pass | Next free ID rechecked (`D-169` the last; no other session's decision): `D-170` written; `rg -c '^## D-17[0] — ' DECISIONS.md` → 1; `^\*\*Amended by D-17[0]:\*\*` → 3 (`D-040`, `D-155`, `D-157`); the index rows: `D-170` 1, `D-040` 1, `D-157` 1, `D-155` 1; `^### Event Cursors and Spawn Inheritance — M60$` → 1, `counting what it handled` no match; ``carry no `SceneEntity` `` no match, `spawn_prefab_from` 1 in known issues; `just ctx` exit 0 (Agent context checks: OK); `docs/LLM_INDEX.md` 12,280 bytes (12,284 before), `EventCursor` 1, `spawn inheritance` 1; `\| 0\.80 \(M60\) \|$` → 2 in w04; `M60` 4 in w13-kit (0 at `HEAD`), its garbled lines at `:487-494` kept as found (A17); `just repo-check` exit 0, `0 error(s)` | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w13-kit.md` |
| 6 | 2026-10-16 | Closed; the cut, the release checks and the preflight follow in the session's report (this plan is archived first) | Workflow §5 (§8): 1 evidence, rows 1–5 quoted; 2 gates, steps 1–4's ran, the release chain runs once after the cut, `just visual` not owed; 3 invariants, the hashes and the six rows' digests unchanged (step 4), `DefaultPlugins`' and the template's schedules unchanged (step 3), the umbrella's, kit's and render snapshots unchanged, `git diff --quiet HEAD -- Cargo.lock` exit 0 before the cut; 4 decisions, `D-170` and its index rows written before the docs citing it (step 5); 5 API, two ledger rows, `undocumented new items: 0`, the snapshot +76 −0; 6 the template's and the guide's clauses (step 3); 7 routes, no new file, `just ctx` exit 0; 8 `DESIGN.md`'s M60 section and status line; 9 known issues `:52` rewritten, no backlog row (nothing cut), no gap-log row, w13-kit current, the register row filled, the README's owned fields by the cut; 10 one `CHANGELOG.md` `[Unreleased]` entry, `status: done`, archived. Other sessions' uncommitted files in the tree, named and left as found: `docs/plans/1.0/{README.md,backlog-1.x.md,criteria.md,implementation-plan.md,roadmap.json,w13-kit.md}` (this run also edits the README's owned fields, the register row and w13-kit's M60 lines), `docs/plans/kit-catalogue.md` (deleted), `docs/plans/archive/kit-catalogue.md`, `docs/plans/phase7-rewrite.md`; `rg -n '^## D-1' DECISIONS.md | tail -3` → `D-168`, `D-169`, `D-170`, none another session's | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/implementation-plan.md`, this plan at `docs/plans/archive/1.0/` |

## Follow-ups

- **A spawn from a source that is not a prefab** (Q8): a public bundle form on `CommandBuffer`, when a game spawns bundles from an entity; the kit's spawns are prefab spawns.
- **An entity that takes a state's marker after the listing in its exit frame** (Q14, §2.3): known issues; a fix belongs to the state stack, not core's flush.
- **The umbrella's own readers** (game feel, tweens, particles, UI) stay on `iter` and `iter_current`; move one to a cursor when its place against its senders matters.
- **`w13-kit.md:488`** (A17): the catalogue review's garbled heading and its round-2 findings sit inside Follow-ups, uncommitted; the owner sorts them before the 0.80 commit, which takes the whole tree.
