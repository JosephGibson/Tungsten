# Tungsten — Phase 2 Plan

**Status:** Phase 2 in progress. **M7 complete** (`v0.2.0-alpha.0`), **M8 complete** (`v0.3.0-alpha`), **M9 complete** (`v0.4.0-alpha`). Next: **M10 tilemaps**.
**Branch:** `0.4`
**Prerequisite:** Phase 1 complete (M0–M6), tagged `v0.1.0-alpha`.
**Companion docs:** `DESIGN.md` (architecture), `DECISIONS.md` (decision log, esp. D-024 — Phase 1 exit observations), `AGENTS.md` (operational rules).

---

## Overview

Phase 1 proved the foundations: hand-rolled ECS, wgpu render pipeline, manifest-driven assets, input, and frame-based animation. Phase 2 turns those foundations into something that can run an actual game.

**Rollout model:** one major milestone per alpha version. Each alpha ships when its milestone is complete, tested, and demonstrated by a new or extended example. When all milestones are done, the engine graduates to **v1.0.0** with a small proof-of-concept game built on top.

### Release map

| Version          | Milestone | Name                                  | Status          |
| ---------------- | --------- | ------------------------------------- | --------------- |
| `v0.2.0-alpha.0` | M7        | Text rendering                        | **Complete**    |
| `v0.3.0-alpha`   | M8        | Audio                                 | **Complete**    |
| `v0.4.0-alpha`   | M9        | Hot reload                            | **Complete**    |
| `v0.5.0-alpha`   | M10       | Tilemaps                              | In progress     |
| `v0.6.0-alpha`   | M11       | 2D physics                            | Planned         |
| `v0.7.0-alpha`   | M12       | Archetypal ECS rewrite                | **Conditional** (D-030) |
| `v1.0.0`         | M13       | A first actual game                   | Planned         |

### Ordering rationale

1. **Text first** — most self-contained new subsystem. Fonts already staged in `assets/fonts/`. Every later milestone can use text for debug overlays and UI.
2. **Audio second** — other major "new subsystem" milestone. Early means M13 has sound available, and the API gets exercised across more milestones.
3. **Hot reload third** — architectural prerequisites already in place from M5 (registry-by-ID). Faster iteration loops for the content-heavy work that follows.
4. **Tilemaps fourth** — natural next step for building an actual game. Depends on sprite rendering, benefits from hot reload.
5. **Physics fifth** — collision shapes and resolution. Needs tilemaps for static geometry; benefits from hot reload for tuning.
6. **ECS rewrite sixth** — learning-motivated, not a prerequisite. Positioned last before the game to have maximum workload to benchmark against. Conditional per D-030.
7. **The game last** — the proof that the engine works. Everything else feeds into it.

---

## M7 — Text rendering ✓ Complete

**Version:** `v0.2.0-alpha.0`
**Shipped:** `glyphon` / `cosmic-text` / `swash` text pipeline, `fonts` manifest section, `TextSection` render API, `example-06-text`. Three font families staged (Inter, Source Serif 4, JetBrains Mono). See `DECISIONS.md` D-026 and `CHANGELOG.md` for details.

## M8 — Audio ✓ Complete

**Version:** `v0.3.0-alpha`
**Shipped:** `cpal` output device, hand-rolled mixer on callback thread, `symphonia` eager decode (OGG/WAV/MP3), `sounds` manifest section, `AudioCommands` resource, `example-07-audio`. See `DECISIONS.md` D-027/D-028/D-029 and `CHANGELOG.md` for details.

## M9 — Hot reload ✓ Complete

**Version:** `v0.4.0-alpha`
**Shipped:** `notify`-based file watcher on a dedicated thread, 50ms debounce, live reload for sprites / animations / fonts / manifest, `example-08-hot-reload`, `App::enable_hot_reload`. See `DECISIONS.md` D-031 and `CHANGELOG.md` for details.

---

## M10 — Tilemaps

**Version:** `v0.5.0-alpha`
**Soft estimate:** Multiple weekends
**Learn:** Tile-based map representation, efficient tilemap rendering (batching, culling), map data formats, manifest extension to maps, camera/viewport concepts.

### Goals

- Load and render tile-based maps from a data-driven format registered in the manifest.
- Support multiple tile layers (background, foreground, collision).
- Efficient rendering — tilemaps can be large; naive per-tile draw calls won't scale.

### Scope

- **In scope:** Custom JSON tilemap format (consistent with the animation precedent from D-010), manifest `tilemaps` section, tileset references via sprite IDs, multi-layer rendering, camera scrolling/viewport, new example with a scrollable map.
- **Out of scope:** Tiled `.tmx` import (possible future converter), infinite/procedural maps, auto-tiling, tile animations (possible extension over the existing animation system).

### Approach

Tilemaps reference sprites from the existing manifest by ID — a tileset is a collection of sprite IDs, not a separate atlas. Consistent with the rest of the architecture and benefits from M9 hot reload. The tilemap renderer batches tiles into a single draw call per layer, similar to sprite instancing. A new camera/viewport resource controls the visible portion of the map and becomes part of the engine core.

### Acceptance criteria

- [ ] A tilemap loads from a JSON file registered in the manifest.
- [ ] Multiple layers render in correct order.
- [ ] Camera scrolling works — arrow keys or WASD pan a viewport across a map larger than the window.
- [ ] Performance is reasonable for maps of at least 100×100 tiles.
- [ ] A new example demonstrates a scrollable tilemap with multiple layers.
- [ ] `cargo test --workspace` passes. `cargo fmt` clean.

### Dependencies

M5 (sprite rendering, asset registry). M9 not required but hot reload pays off heavily during map iteration.

---

## M11 — 2D physics

**Version:** `v0.6.0-alpha`
**Soft estimate:** Multiple weekends
**Learn:** AABB / circle / SAT collision, collision response and resolution, spatial data structures, physics as ECS components and systems.

### Goals

- Basic 2D collision detection and response as ECS components and systems.
- Support AABB and circle shapes.
- Integrate with existing `Position` / `Velocity` components.

### Scope

- **In scope:** Collision shapes (AABB, circle), broad-phase detection, narrow-phase resolution, static and dynamic bodies, collision events, tilemap collision layer integration (from M10), a demo example.
- **Out of scope:** Joints, constraints, continuous collision detection, soft bodies, fluid simulation. Game-jam-grade physics, not Box2D.

### Approach

Hand-rolled, consistent with the build-it-to-learn-it principle. The physics system runs during tick, after movement systems and before rendering. Components: `Collider` (shape + offset), `RigidBody` (static vs dynamic, mass). Broad-phase via spatial hash/grid; narrow-phase via shape-vs-shape tests. Collision events collected into a resource that game systems read.

Tilemap collision layers (from M10) provide static geometry — tiles marked solid in tilemap data generate static colliders.

### Acceptance criteria

- [ ] AABB-vs-AABB and circle-vs-circle collision detection works correctly.
- [ ] Dynamic bodies resolve against static bodies without tunneling at reasonable speeds.
- [ ] A tilemap collision layer blocks entity movement.
- [ ] Collision events accessible to game systems.
- [ ] A new example demonstrates entities colliding with each other and with tilemap geometry.
- [ ] `cargo test --workspace` passes. `cargo fmt` clean.

### Dependencies

M10 (tilemap collision layers). Phase 1 ECS.

---

## M12 — Archetypal ECS rewrite (conditional)

**Version:** `v0.7.0-alpha`
**Soft estimate:** Multiple weekends (possibly the longest milestone)
**Learn:** Archetypal storage, cache-friendly iteration, component move semantics, columnar vs HashMap storage tradeoffs, real-world benchmarking.

**This milestone is conditional.** After M11, assess whether the naive ECS has caused measurable friction — slow queries, borrow fights under load, correctness issues with many entities. If yes, proceed. If not, skip M12 and go directly to M13. Descoping is not failure (D-005, D-030).

### Goals

- Replace the naive `HashMap<TypeId, HashMap<EntityId, Box<dyn Any>>>` storage with an archetypal layout.
- Maintain the existing public API — no breaking changes to examples or game code.
- Measure and document the performance difference with real M7–M11 workloads.

### Scope

- **In scope:** Archetype table storage, contiguous per-archetype component arrays, archetype graph for add/remove transitions, updated query iteration, benchmarks on representative workloads.
- **Out of scope:** Parallel system scheduling, change detection, command buffers, reactive queries.

### Approach

D-024 confirmed the naive ECS works fine at Phase 1 scale. D-005 says "if naive stays good enough forever, that's a success, not a failure." If this milestone proceeds, it's learning-motivated — the goal is understanding archetypal storage, not fixing a crisis.

The rewrite is internal to `tungsten-core`. The `World` API stays the same; the storage engine behind it changes.

### Acceptance criteria

- [ ] **Decision to proceed or skip logged in `DECISIONS.md` before the milestone begins** (cite D-030).
- [ ] All existing examples compile and pass without API changes.
- [ ] `cargo test --workspace` passes — ECS test suite is the primary validation.
- [ ] A benchmark comparing iteration speed (old vs new) on ≥10,000 entities with 3+ component types.
- [ ] Query iteration is cache-friendly: components of the same archetype stored contiguously.
- [ ] `DECISIONS.md` entry documenting the storage design and benchmark results.

### Dependencies

All prior milestones — the rewrite happens last so there's real workload to test against. The `World` public API from M2.

---

## M13 — A first actual game

**Version:** `v1.0.0`
**Soft estimate:** Multiple weekends
**Learn:** What works, what's missing, what's painful — the engine's first real stress test from the user's perspective.

### Goals

- Build a small but complete game using only the Tungsten engine.
- Exercise every major subsystem: sprites, text, tilemaps, audio, input, physics, animation, ECS.
- Identify gaps, pain points, and missing conveniences for a hypothetical Phase 3.

### Scope

- **In scope:** A playable game with a beginning and an end (or clear loop). Player movement, collision, at least one mechanic, sound effects, music, text (title, score/UI), a tilemap level. All assets manifest-driven. Ships as a new example or a top-level `game/` crate.
- **Out of scope:** Polish, save/load, multiple levels (unless trivial), menus beyond a title screen. Proof-of-concept, not product.

### Approach

Genre is decided at M13 start, not before. Don't pre-commit to a design that may not survive contact with the actual engine state after M11–M12. The game lives alongside the examples, uses the same manifest system, follows all the same rules. Game-specific components and systems live in the game crate, not library crates — the engine stays general.

### Acceptance criteria

- [ ] Playable: starts, player can interact, has a win/lose/loop condition.
- [ ] Every major engine subsystem is exercised.
- [ ] All assets are manifest-driven.
- [ ] Runs at a smooth framerate on the development machine.
- [ ] A retrospective captures what worked, what didn't, and what's missing.
- [ ] `cargo test --workspace` passes. `cargo fmt` clean.

### Dependencies

All of Phase 2 (M7–M12). The capstone.

---

## Out of scope for Phase 2

See `DESIGN.md` "Non-commitments" for the full list. Nothing in that list is scheduled or scoped for Phase 2 without an explicit `DECISIONS.md` entry.
