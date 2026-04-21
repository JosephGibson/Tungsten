# Phase 2 archive — M7–M12 (complete)

This file is the archived Phase 2 plan. Phase 2 shipped in full as of `v0.7.0-alpha`.
Milestone detail is in `CHANGELOG.md`; design decisions are in `DECISIONS.md` (D-024–D-036).

---

# Tungsten — Phase 2 Plan

**Status:** Phase 2 complete. M7 (`v0.2.0-alpha.0`), M8 (`v0.3.0-alpha`), M9 (`v0.4.0-alpha`), M10 (`v0.5.0-alpha`), M11 (`v0.6.0-alpha`), M12 (`v0.7.0-alpha`) all shipped.
**Branch:** `0.7`
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
| `v0.6.0-alpha`   | M11       | 2D physics                            | **Complete**    |
| `v0.7.0-alpha`   | M12       | Archetypal ECS rewrite                | **Complete**    |

### Ordering rationale

1. **Text first** — most self-contained new subsystem. Fonts already staged in `assets/fonts/`. Every later milestone can use text for debug overlays and UI.
2. **Audio second** — other major "new subsystem" milestone. Early means M13 has sound available, and the API gets exercised across more milestones.
3. **Hot reload third** — architectural prerequisites already in place from M5 (registry-by-ID). Faster iteration loops for the content-heavy work that follows.
4. **Tilemaps fourth** — natural next step for building an actual game. Depends on sprite rendering, benefits from hot reload.
5. **Physics fifth** — collision shapes and resolution. Needs tilemaps for static geometry; benefits from hot reload for tuning.
6. **ECS rewrite sixth** — learning-motivated, not a prerequisite. Positioned last before the game to have maximum workload to benchmark against. Conditional per D-030.

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

**Shipped:** Hand-rolled `tungsten-core::physics` module with `Position`/`Velocity`/`Collider`/`RigidBody` components, `PhysicsConfig` + `CollisionEvents` resources, AABB/circle narrow-phase, uniform-grid broad-phase rebuilt per substep, MTV resolution with restitution, tunneling-aware substep driver, `LayerKind::Collision` tilemap layers read transiently as static AABBs, and `example-10-platformer` (side-scroller with gravity, grounded detection via `CollisionEvents`, bouncing circles). See `DECISIONS.md` D-033 and `CHANGELOG.md` for details.

### Acceptance criteria

- [x] AABB-vs-AABB and circle-vs-circle collision detection works correctly.
- [x] Dynamic bodies resolve against static bodies without tunneling at reasonable speeds.
- [x] A tilemap collision layer blocks entity movement.
- [x] Collision events accessible to game systems.
- [x] A new example demonstrates entities colliding with each other and with tilemap geometry.
- [x] `cargo test --workspace` passes. `cargo fmt` clean.

### Known limitations

- **Variable-dt physics.** Runs under a variable timestep with a substep cap (D-033). Can produce frame-rate-dependent behaviour at high speeds. Semi-fixed accumulator loop is the preferred upgrade if instability is observed.
- **Tilemap collider cost is O(tiles × substeps).** Proxies regenerated each substep (D-033). Budget: ≤128×128 tiles; larger maps should pre-bake a static spatial index.

---

## M12 — Archetypal ECS rewrite ✓ Complete

**Version:** `v0.7.0-alpha`

**Shipped:** Archetypal storage with `Box<dyn AnyColumn>` typed columns, `TypedVec<T>` per-component `Vec<T>`, archetype graph with lazy edge caching, generational entity IDs, `query2`/`query2_entities`/`query3`/`query3_entities` multi-component queries, Criterion benchmark suite. Decision to proceed logged in D-036 (cites D-030). ~6× improvement on single-type queries; ~200× on multi-component queries vs. naive `HashMap<TypeId, HashMap<u32, Box<dyn Any>>>` baseline. All 10 examples compile and smoke-test clean without modification.

### Acceptance criteria

- [x] Decision to proceed or skip logged in `DECISIONS.md` before the milestone begins (cite D-030).
- [x] All existing examples compile and pass without API changes.
- [x] `cargo test --workspace` passes — ECS test suite is the primary validation.
- [x] A benchmark comparing iteration speed (old vs new) on ≥10,000 entities with 3+ component types.
- [x] Query iteration is cache-friendly: components of the same archetype stored contiguously.
- [x] `DECISIONS.md` entry documenting the storage design and benchmark results.

---

## Out of scope for Phase 2

See `DESIGN.md` "Non-commitments" for the full list.
