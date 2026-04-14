# Tungsten — Phase 2 Plan

**Status:** Phase 2 in progress. **M7 complete** (`v0.2.0-alpha.0`), **M8 complete** (`v0.3.0-alpha`), **M9 complete** (`v0.4.0-alpha`), **M10 complete** (`v0.5.0-alpha`), **M11 complete** (`v0.6.0-alpha`). Next: **M12 ECS rewrite (conditional)** or **M13 first game**.
**Branch:** `0.6`
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
| `v0.5.0-alpha`   | M10       | Tilemaps                              | **Complete**    |
| `v0.6.0-alpha`   | M11       | 2D physics                            | Complete        |
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

## M10 — Tilemaps ✓ Complete

**Version:** `v0.5.0-alpha`
**Shipped:** Custom `.tmj` JSON tilemap format, `TilemapData`/`TilemapRegistry`/`TilemapInstance` in `tungsten-core`, manifest `tilemaps` section, `Camera2D` resource feeding view-projection into the sprite and quad pipelines, `extract_tilemaps(&World)` helper with visible-AABB tile culling, `.tmj` hot reload on the existing watcher path, `example-09-tilemap` with a 48×30 two-render-layer map plus a non-rendering collision layer (M11 seam). See `DECISIONS.md` D-032 and `CHANGELOG.md` for details.

### Acceptance criteria

- [x] A tilemap loads from a JSON file registered in the manifest.
- [x] Multiple layers render in correct order.
- [x] Camera scrolling works — arrow keys or WASD pan a viewport across a map larger than the window.
- [x] Performance is reasonable for maps of at least 100×100 tiles (visible-AABB culling makes cost proportional to viewport, not map size).
- [x] A new example demonstrates a scrollable tilemap with multiple layers.
- [x] `cargo test --workspace` passes. `cargo fmt` clean.

---

## M11 — 2D physics ✓ Complete

**Version:** `v0.6.0-alpha`
**Soft estimate:** Multiple weekends
**Learn:** AABB / circle / SAT collision, collision response and resolution, spatial data structures, physics as ECS components and systems.

**Shipped:** Hand-rolled `tungsten-core::physics` module with `Position`/`Velocity`/`Collider`/`RigidBody` components, `PhysicsConfig` + `CollisionEvents` resources, AABB/circle narrow-phase, uniform-grid broad-phase rebuilt per substep, MTV resolution with restitution, tunneling-aware substep driver, `LayerKind::Collision` tilemap layers read transiently as static AABBs, and `example-10-platformer` (side-scroller with gravity, grounded detection via `CollisionEvents`, bouncing circles). See `DECISIONS.md` D-033 and `CHANGELOG.md` for details.

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

- [x] AABB-vs-AABB and circle-vs-circle collision detection works correctly.
- [x] Dynamic bodies resolve against static bodies without tunneling at reasonable speeds.
- [x] A tilemap collision layer blocks entity movement.
- [x] Collision events accessible to game systems.
- [x] A new example demonstrates entities colliding with each other and with tilemap geometry.
- [x] `cargo test --workspace` passes. `cargo fmt` clean.

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
