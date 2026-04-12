# Tungsten — Phase 2 Plan

**Status:** Phase 2 in progress — **M7 complete** (`v0.2.0-alpha`); next milestone M8 (audio).
**Branch:** `0.2`
**Prerequisite:** Phase 1 complete (M0–M6), tagged `v0.1.0-alpha`.
**Companion docs:** `DESIGN.md` (architecture, Phase 1 milestones), `DECISIONS.md` (decision log, esp. D-024), `AGENTS.md` (operational rules).

---

## Overview

Phase 1 proved the engine's foundations: a hand-rolled ECS, a wgpu render pipeline, manifest-driven asset loading, input handling, and frame-based animation. Phase 2 turns those foundations into something that can run an actual game.

The rollout model is **one major milestone per alpha version**. Each alpha ships when its milestone is complete, tested, and demonstrated by a new or extended example. When all milestones are done, the engine graduates to **v1.0.0** with a small proof-of-concept game built on top of everything.

The milestone names and descriptions below match the terminology established in `DESIGN.md` ("After M6 — stop and reassess") and the gating observations recorded in `DECISIONS.md` D-024. The ordering reflects both dependency constraints and a preference for getting all major subsystems (rendering, audio, assets, physics) online before the big internal rewrite.

### Release map

| Version        | Milestone | Name                   |
| -------------- | --------- | ---------------------- |
| `v0.2.0-alpha` | M7        | Text rendering — **done** |
| `v0.3.0-alpha` | M8        | Audio                  |
| `v0.4.0-alpha` | M9        | Hot reload             |
| `v0.5.0-alpha` | M10       | Tilemaps               |
| `v0.6.0-alpha` | M11       | 2D physics             |
| `v0.7.0-alpha` | M12       | Archetypal ECS rewrite |
| `v1.0.0`       | M13       | A first actual game    |

### Ordering rationale

1. **Text rendering first** — it's the most self-contained new subsystem with the least dependency on other Phase 2 work. Fonts are already staged in `assets/fonts/`. Getting text on screen early means every subsequent milestone can use it for debug overlays, UI labels, and example polish.
2. **Audio second** — the other major "new subsystem" milestone. Tackling it early means the game milestone (M13) has sound available from the start, and the audio API gets exercised across more milestones.
3. **Hot reload third** — the architectural prerequisites are already in place from M5 (registry-by-ID invariant). Having hot reload before tilemaps and physics means faster iteration loops for all the content-heavy work that follows.
4. **Tilemaps fourth** — "the natural next thing for actually building a game" (DESIGN.md). Depends on sprite rendering (done), benefits from hot reload (done).
5. **2D physics fifth** — collision shapes and basic resolution. Benefits from having tilemaps to collide against and hot reload for tuning.
6. **Archetypal ECS rewrite sixth** — learning-motivated, not a prerequisite for any other milestone. Positioned last before the game so there's maximum real workload (text, audio, tilemaps, physics entities) to benchmark the rewrite against. If the naive ECS never hurts, this milestone can be descoped or reframed without blocking v1.0.
7. **A first actual game last** — the proof that the engine works. Everything else feeds into it.

---

## M7 — Text rendering

**Version:** `v0.2.0-alpha`
**Soft estimate:** Multiple weekends
**Learn:** Font loading, glyph rasterization, text layout, GPU text rendering with wgpu, the manifest pattern extended to a new asset type.

### Goals

- Render arbitrary text strings to the screen at specified positions, sizes, and colors.
- Support the three font families already staged in `assets/fonts/` (Inter, Source Serif 4, JetBrains Mono).
- Extend the asset manifest with a `fonts` section so fonts are loaded by ID, consistent with the sprite/animation pattern.
- Provide a text-drawing API in `tungsten-render` that integrates with the existing frame loop.

### Scope

- **In scope:** Font loading from TTF files, glyph shaping and layout, GPU upload and rendering, manifest integration, a new example (`06_text` or similar).
- **Out of scope:** Rich text (mixed fonts/colors in one block), text input/editing, UI widgets, text wrapping heuristics beyond basic line breaks, SDF text rendering.

### Approach

Per D-024, `glyphon` (built on `cosmic-text`) is the recommended crate. It's purpose-built for wgpu and satisfies D-015 rule 2 (implements a well-specified format that isn't the interesting part). The `fonts/README.md` confirms `cosmic-text`/`glyphon` handle the staged variable fonts via `swash`.

The text pipeline should sit alongside the existing quad and sprite pipelines in `tungsten-render`. Font assets get opaque handles in the registry, same pattern as textures.

### Acceptance criteria

- [x] `assets/manifest.json` has a `fonts` section; fonts are loaded by ID, never by path.
- [x] Text renders correctly at multiple sizes with at least two font families (sans + mono).
- [x] A new example demonstrates text rendering: labels, a debug overlay with FPS, and mixed font usage.
- [x] `cargo test --workspace` passes. `cargo fmt` clean.
- [x] `DECISIONS.md` entry for the `glyphon`/`cosmic-text` dependency, citing D-015 rule 2.

### Dependencies

- Phase 1 complete (M0–M6).
- Font files in `assets/fonts/` (already staged).

### Release `v0.2.0-alpha`

Workspace and library crate versions are **`0.2.0-alpha`**. When this commit is on `main` (or `0.2`), tag with:

`git tag -a v0.2.0-alpha -m "M7 text rendering"`

---

## M8 — Audio

**Version:** `v0.3.0-alpha`
**Soft estimate:** Multiple weekends
**Learn:** Audio device abstraction, sample decoding, mixing, playback control, the manifest pattern extended to sounds.

### Goals

- Play sound effects and looping background music from assets registered in the manifest.
- Wire audio into the ECS as a resource so systems can trigger sounds.
- Fill the `assets/sounds/` directory (currently a placeholder) with actual content.

### Scope

- **In scope:** Audio device init via `cpal` (D-015 rule 1 — platform API abstraction), sample decoding via `symphonia` (D-015 rule 2 — data format), a basic mixer, manifest `sounds` section, volume control, looping, one-shot playback, a new example.
- **Out of scope:** Spatial/positional audio, streaming large files, DSP effects, MIDI. These are future decisions if audio becomes a focus area.

### Approach

D-024 notes that `symphonia` (decoder) is likely fine under D-015 rule 2. The mixer question — `kira` vs hand-rolled — needs a `DECISIONS.md` entry. A hand-rolled mixer is more aligned with the project's "build it to learn it" ethos, but `kira` exists if the mixer turns out to be uninteresting yak-shaving.

Audio playback runs on a dedicated thread (via `cpal`'s callback model), but the API surface presented to game code is synchronous: systems write to an `AudioCommands` resource (or similar), and the audio thread drains commands each callback. No async runtime.

### Acceptance criteria

- [ ] `assets/manifest.json` has a `sounds` section; sounds are loaded by ID.
- [ ] At least one sound effect and one looping track play correctly.
- [ ] A new example demonstrates audio playback triggered by input or game events.
- [ ] Volume control works (at minimum: master volume, per-sound volume).
- [ ] `cargo test --workspace` passes. `cargo fmt` clean.
- [ ] `DECISIONS.md` entries for `cpal`, `symphonia`, and the mixer approach (hand-rolled vs `kira`).

### Dependencies

- M7 (text rendering) — not a hard technical dependency, but the versioning model requires M7 to ship first. Text is useful for audio example UI.

---

## M9 — Hot reload

**Version:** `v0.4.0-alpha`
**Soft estimate:** A weekend or two
**Learn:** File watching, cross-thread messaging, GPU resource replacement at runtime, the payoff of the registry-by-ID architecture.

### Goals

- When an asset file changes on disk, detect the change and swap the asset at the next frame boundary without restarting the engine.
- Cover sprites, animations, and fonts. Audio hot reload is a stretch goal.
- If the manifest itself changes, reload and reconcile.

### Scope

- **In scope:** File watching via `notify` (D-015 rule 1), change detection, decode-and-reupload for sprites, animation JSON reparse, font reloading, manifest reconciliation, visual confirmation in an existing or new example.
- **Out of scope:** Hot reload of engine config (`tungsten.json`), hot reload of Rust code, hot reload of shaders (possible future extension).

### Approach

The sketch in `DESIGN.md` ("Hot reload — Phase 2") is the blueprint: a background thread runs `notify` on the assets directory, sends file-change messages to the main thread via a channel, and at the next frame boundary the main thread resolves file paths back to asset IDs, decodes the new data, uploads to the GPU, and swaps the handle in the registry. Existing components referencing by ID see the new data automatically.

The M5 architecture already preserves the registry-by-ID invariant (confirmed in D-024). No game code holds direct GPU handles.

### Acceptance criteria

- [ ] Modifying a sprite PNG on disk causes the rendered sprite to update within a few frames, without restart.
- [ ] Modifying an animation JSON on disk causes the animation to update live.
- [ ] Modifying the manifest (adding/removing an entry) is handled gracefully — new assets load, removed assets either warn or are cleaned up.
- [ ] No crash or resource leak on rapid successive changes.
- [ ] `cargo test --workspace` passes. `cargo fmt` clean.
- [ ] `DECISIONS.md` entry for the `notify` dependency, citing D-015 rule 1.

### Dependencies

- M7 (text) and M8 (audio) shipped. Font hot reload depends on M7's font loading infrastructure.
- The registry-by-ID invariant from M5 (already in place).

---

## M10 — Tilemaps

**Version:** `v0.5.0-alpha`
**Soft estimate:** Multiple weekends
**Learn:** Tile-based map representation, efficient tilemap rendering (batching, culling), map data formats, the manifest pattern extended to maps, camera/viewport concepts.

### Goals

- Load and render tile-based maps from a data-driven format registered in the manifest.
- Support multiple tile layers (background, foreground, collision).
- Efficient rendering — tilemaps can be large; naive per-tile draw calls won't scale.

### Scope

- **In scope:** A custom JSON tilemap format (consistent with the animation format precedent from D-010), manifest `tilemaps` section, tileset references via sprite IDs, multi-layer rendering, camera scrolling/viewport, a new example showing a scrollable map.
- **Out of scope:** Tiled (`.tmx`) import (could be a converter, not a runtime dependency), infinite/procedural maps, auto-tiling, tile animations (possible extension using the existing animation system).

### Approach

Tilemaps reference sprites from the existing manifest by ID — a tileset is a collection of sprite IDs, not a separate texture atlas. This keeps the architecture consistent and benefits from hot reload (M9). The tilemap renderer batches tiles into a single draw call per layer, similar to the existing sprite instancing.

A camera/viewport resource controls which portion of the map is visible, enabling scrolling. This resource is useful beyond tilemaps and becomes part of the engine's core.

### Acceptance criteria

- [ ] A tilemap loads from a JSON file registered in the manifest.
- [ ] Multiple layers render in correct order (background behind foreground).
- [ ] Camera scrolling works — arrow keys or WASD pan the viewport across a map larger than the window.
- [ ] Performance is reasonable for maps of at least 100x100 tiles.
- [ ] A new example demonstrates a scrollable tilemap with multiple layers.
- [ ] `cargo test --workspace` passes. `cargo fmt` clean.

### Dependencies

- M5 (sprite rendering, asset registry) — tilemaps build on the sprite infrastructure.
- M9 (hot reload) — not required, but tilemap iteration benefits greatly from live reloading.

---

## M11 — 2D physics

**Version:** `v0.6.0-alpha`
**Soft estimate:** Multiple weekends
**Learn:** Collision detection algorithms (AABB, circle, SAT), collision response and resolution, spatial data structures, physics as ECS components and systems.

### Goals

- Provide basic 2D collision detection and response as ECS components and systems.
- Support common collision shapes: axis-aligned bounding boxes (AABB), circles.
- Integrate with the existing `Position` and `Velocity` components.

### Scope

- **In scope:** Collision shapes (AABB, circle), broad-phase detection, narrow-phase resolution, static and dynamic bodies, collision events/callbacks, a tilemap collision layer (integrating with M10), a new example demonstrating physics interactions.
- **Out of scope:** Joints/constraints, continuous collision detection (CCD), physics simulation stability at high speeds, soft bodies, fluid simulation. This is game-jam-grade physics, not Box2D.

### Approach

Hand-rolled, consistent with the project's "build it to learn it" principle. No external physics crate. The physics system runs during the tick phase, after movement systems and before rendering.

Collision data is ECS components: a `Collider` component (shape + offset), a `RigidBody` component (static vs dynamic, mass). A physics system performs broad-phase (spatial hash or grid) then narrow-phase (shape-vs-shape tests) each tick. Collision events are collected into a resource that other systems can read.

Tilemap collision layers (from M10) provide static geometry — tiles marked as solid in the tilemap data generate static colliders.

### Acceptance criteria

- [ ] AABB-vs-AABB and circle-vs-circle collision detection works correctly.
- [ ] Dynamic bodies resolve collisions against static bodies (no tunneling at reasonable speeds).
- [ ] A tilemap collision layer blocks entity movement.
- [ ] Collision events are accessible to game systems.
- [ ] A new example demonstrates entities colliding with each other and with tilemap geometry.
- [ ] `cargo test --workspace` passes. `cargo fmt` clean.

### Dependencies

- M10 (tilemaps) — for tilemap collision layers.
- Phase 1 ECS (M2) — physics components and systems use the existing World.

---

## M12 — Archetypal ECS rewrite

**Version:** `v0.7.0-alpha`
**Soft estimate:** Multiple weekends (possibly the longest milestone)
**Learn:** Archetypal storage, cache-friendly iteration, component move semantics, the tradeoffs between HashMap-of-Any and columnar storage, real-world benchmarking.

### Goals

- Replace the naive `HashMap<TypeId, HashMap<EntityId, Box<dyn Any>>>` storage with an archetypal layout.
- Maintain the existing public API (`World`, `Entity`, component operations, queries, resources) — no breaking changes to examples or game code.
- Measure and document the performance difference with real workloads from M7–M11.

### Scope

- **In scope:** Archetype table storage, component arrays laid out contiguously per archetype, archetype graph for add/remove transitions, updated query iteration, benchmarks comparing old vs new on representative workloads (many entities with physics, tilemaps, animations).
- **Out of scope:** Parallel system scheduling, change detection, command buffers, reactive queries. These are potential future extensions, not part of the rewrite.

### Approach

D-024 confirms the naive ECS works fine at Phase 1 scale. D-005 says "if naive stays good enough forever, that's a success, not a failure." This milestone is learning-motivated — the goal is understanding archetypal storage, not fixing a performance crisis.

The rewrite should be internal to `tungsten-core`. The `World` API stays the same; the storage engine behind it changes. All existing examples and any M7–M11 code should compile and run without modification after the rewrite.

If the rewrite proves uninteresting or overly painful, it can be descoped to a partial rewrite (e.g., archetypal iteration for queries, HashMap fallback for everything else) without blocking v1.0.

### Acceptance criteria

- [ ] All existing examples (01–05 plus any M7–M11 examples) compile and pass without API changes.
- [ ] `cargo test --workspace` passes — the ECS test suite is the primary validation.
- [ ] A benchmark comparing iteration speed (old vs new) on at least 10,000 entities with 3+ component types.
- [ ] Query iteration is cache-friendly: components of the same archetype are stored contiguously.
- [ ] `DECISIONS.md` entry documenting the storage design and benchmark results.

### Dependencies

- All prior milestones (M7–M11) — the rewrite happens last so there's a real workload to test against.
- The existing `World` public API from M2 — the contract is "same API, different internals."

---

## M13 — A first actual game

**Version:** `v1.0.0`
**Soft estimate:** Multiple weekends
**Learn:** What works, what's missing, what's painful — the engine's first real stress test from the user's perspective.

### Goals

- Build a small but complete game using only the Tungsten engine.
- Exercise every major subsystem: rendering (sprites, text, tilemaps), audio, input, physics, animation, the ECS.
- Identify gaps, pain points, and missing conveniences that would inform a hypothetical Phase 3.

### Scope

- **In scope:** A playable game with a beginning and an end (or a clear loop). Player movement, collision, at least one game mechanic, sound effects, background music, text (title screen, score/UI), a tilemap-based level. All assets registered in the manifest. The game ships as a new example or a top-level `game/` crate.
- **Out of scope:** Polish, save/load, multiple levels (unless trivial), menus beyond a title screen. This is a proof-of-concept, not a product.

### Approach

The game genre should be whatever is fun and tractable — a top-down action game, a simple platformer, a Pac-Man-like. The choice gets made at M13 start based on what feels interesting. The game lives in the repo alongside the examples, uses the same manifest system, and follows all the same rules (no hardcoded paths, no external engine crates, no global state).

Game-specific components and systems live in the game crate, not in the library crates. The engine stays general; the game is the consumer.

### Acceptance criteria

- [ ] The game is playable: it starts, the player can interact, there is a win/lose/loop condition.
- [ ] Every major engine subsystem is exercised (sprites, text, tilemaps, audio, physics, animation, input, ECS).
- [ ] All assets are manifest-driven.
- [ ] The game runs at a smooth framerate on the development machine.
- [ ] A retrospective section in `DECISIONS.md` or a dedicated document captures what worked, what didn't, and what's missing.
- [ ] `cargo test --workspace` passes. `cargo fmt` clean.

### Dependencies

- All of Phase 2 (M7–M12). This is the capstone.

---

## What this plan does not cover

Items from `DESIGN.md` "Non-commitments" remain non-committed. None of the following are scheduled or scoped:

- Networking / multiplayer
- Scripting
- Editor tooling
- Asset preprocessing / build pipeline
- 3D rendering
- WASM / browser support
- Hot reload of config (`tungsten.json`)
- Save / load
- GUI library
- Texture atlases / sprite sheet packing
- GPU-compressed texture formats
- Skeletal animation
- Streaming or async asset loading

Any of these could appear in a future phase, but not without an explicit decision and a `DECISIONS.md` entry.

## Kill criteria

The Phase 1 kill criteria in `DESIGN.md` still apply. Two additions for Phase 2:

- **A milestone consistently feels like a chore rather than learning** — consider descoping or reframing it. The archetypal ECS rewrite (M12) is the most likely candidate; it's explicitly okay to descope it if the naive version stays adequate.
- **The game milestone (M13) can't find a genre that's fun to build** — that's a signal, not a failure. The engine still works; the game just needs a different shape.
