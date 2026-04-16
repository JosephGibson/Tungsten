---
status: draft
goal: Ship Phase 3 capabilities for production-style 2D games
non-goals: networking, 3D, scripting, WASM, parallel scheduler, full UI library
files-to-touch: crates/tungsten-core/src/ecs/, crates/tungsten-core/src/, crates/tungsten/src/, crates/tungsten-render/src/, examples/, tungsten.json
---

# Phase 3 Rollout Plan

## Scope and sequencing

| # | Milestone | Track | Depends on | Unblocks |
|---|---|---|---|---|
| M12 | Performance Baseline + Profiling Harness | Tooling | - | all Phase 3 milestones (objective perf gates) |
| M13 | Command Buffers | ECS Core | - | M20, M23, runtime spawn/despawn |
| M14 | Event Queue | ECS Core | - | M20, M21, M23, M24 |
| M15 | Transform + Render Components | Core Systems | - | M16, M18, M21, M22, M23, M24 |
| M16 | Camera Module | Core Systems | M15 | M18, gameplay polish |
| M17 | Display State + Config | Core Systems | - | M18, future settings menu |
| M18 | Runtime Telemetry HUD | Tooling | M16, M17 | faster iteration across all remaining milestones |
| M19 | Input Mapping | Core Systems | - | M20 |
| M20 | Scene/State System | Core Systems | M13, M14, M19 | game flow |
| M21 | Debug Tooling | Tooling | M14, M15 | ship/debug quality |
| M22 | Sprite Atlases | Rendering | M15 | render perf |
| M23 | Particle System | Core Systems | M13, M14, M15 | VFX baseline |
| M24 | Tween System | Core Systems | M14, M15 | UI/animation baseline |

Recommended execution order: M12 -> M13 -> M14 -> M15 -> M16 -> M17 -> M18 -> M19 -> M20 -> M21 -> M22 -> M23 -> M24.

Deferred to Phase 4: change detection, full UI library, save/load, scripting, parallel scheduler.

## Execution contract (for Claude Code)

- Implement milestones strictly in `M#` order unless a dependency explicitly allows parallel work.
- For each milestone: implement only scoped deliverables, run required checks, then update plan status notes before starting next milestone.
- Do not introduce new runtime dependencies without a `DECISIONS.md` entry.
- Keep ownership boundaries explicit: ECS/data objects in `tungsten-core`, app wiring in `tungsten`, GPU/render primitives in `tungsten-render`.

## Core objects introduced in Phase 3

- `CommandBuffer` (`tungsten-core`): deferred structural world mutation.
- `EventQueue<T>` (`tungsten-core`): typed 2-window event buffering.
- `Transform`/`Sprite`/`Visibility`/`Tag` (`tungsten-core`): baseline gameplay/render components.
- `CameraState`/`CameraController`/`CameraMode` (`tungsten` + core-facing data where needed): authoritative camera flow.
- `DisplayState` + config schema (`tungsten` + `tungsten.json`): runtime display settings boundary.
- `DebugHud` telemetry model (`tungsten`): in-game text diagnostics and extension hook.
- `StateStack` + `GameState` (`tungsten`): scene/state transitions.

## Guardrails (best-practice defaults)

- Keep ECS structural mutation deferred (command buffers), then flush at a fixed frame boundary.
- Keep events typed and buffered for at least two update windows to avoid order-sensitive drops.
- Keep frame-boundary order explicit and stable: run systems -> flush command buffers -> flush event queues -> extract/render.
- Prefer deterministic behavior: flush buffers in stable registration order, not undefined order.
- Keep public API surface minimal (`World` gets `flush`; advanced behavior lives in resources/helpers).
- Define milestone completion by observable behavior and tests, not implementation details.
- Run validation per milestone: `cargo test --workspace`; run smoke tests when engine/example wiring changes.

## Milestones

### M12 - Performance Baseline + Profiling Harness

> **Status: complete** (v0.9.0, 2026-04-15)

**Goal:** Establish reliable CPU/GPU diagnostics and a reproducible baseline before adding more engine complexity.

**Why now:** Feature work without early perf visibility risks compounding regressions and makes root-cause analysis harder.

**Design:**
- Define canonical perf scenes (at least platformer + one sprite-heavy stress scene) and fixed capture rules (build mode, backend, resolution, frame window).
- Add CPU frame-stage timings (update/extract/render/audio/hot-reload) and per-system timing summaries to telemetry output.
- Add offline CPU profiling workflow notes/scripts (`perf`/`cargo flamegraph`/platform equivalent) for repeatable captures.
- Add GPU diagnostics path:
  - expose frame time + render-stage timing where available,
  - support GPU frame capture workflow (e.g., RenderDoc),
  - document backend overrides (`WGPU_BACKEND`) and capture metadata.
- Define milestone-level perf budget targets (fps + frame-time envelopes) per canonical scene.

**Done when:**
- Baseline captures are recorded for canonical scenes (CPU and GPU-facing metrics).
- Team can identify top CPU hotspots from profiling output for a representative scene.
- GPU capture workflow is documented and validated on at least one Linux machine.
- Subsequent milestones reference this baseline when evaluating regressions.

### M13 - Command Buffers

> **Status: complete** (v0.10.0, 2026-04-15)

**Goal:** Remove `&mut World` structural-mutation pressure inside system loops.

**Design:**
- Add `CommandBuffer` for `spawn`, `despawn`, `insert`, `remove_component`.
- Apply commands after systems run and before extract/render.
- `spawn` returns `PendingEntity` resolved during flush; real entity ID guaranteed after flush.
- Preserve command order within a buffer; across buffers use stable system order.

**Done when:**
- Systems can spawn/despawn/insert/remove through command buffers.
- `World` API change is limited to `flush`.
- Existing examples still pass smoke tests.

**Risk to manage:** placeholder IDs used too early; document next-frame visibility rule clearly.

### M14 - Event Queue

**Goal:** Replace ad hoc event resources with one typed engine pattern.

**Design:**
- Add `EventQueue<T> { current, previous }` resource pattern.
- `send()` appends to current; readers iterate previous + current.
- `flush()` rotates buffers once per frame at the same boundary as command flush.
- Migrate `CollisionEvents` to `EventQueue<CollisionEvent>`.
- Registration path: `App::register_event::<T>()`.

**Done when:**
- Physics runs unchanged from user perspective after migration.
- Queue works for arbitrary event types.
- Flush is automatic and does not require per-system manual clear.

**Risk to manage:** missed reads from run-conditions; keep two-window lifetime and document it.

### M15 - Transform + Render Components

**Goal:** Make common sprite rendering data-driven without custom extract closures.

**Components to add:**
- `Transform { position, rotation, scale }`
- `Sprite { asset_id, color, z_order }`
- `Visibility { visible }`
- `Tag { name }` (debug aid)

**Rules:**
- Keep physics `Position` separate (per D-033); add explicit sync system for `Position -> Transform.position`.
- If no custom sprite extract is configured, use default extract for `Transform + Sprite + Visibility` (Visibility required).
- M15 migration rule: entities intended for default sprite extract must add `Visibility`; default should be explicit (`visible: true`).

**Done when:**
- A new example renders rotated/scaled sprites with components only.
- Existing examples with custom extract still work unchanged.
- Default extract path enforces `Visibility` as required (no implicit fallback).
- At least one example validates explicit `Visibility` migration in default extract path.

### M16 - Camera Module

**Goal:** Centralize camera behavior in one engine module/class-like API instead of ad hoc per-example logic.

**Design:**
- Add a dedicated camera type stack:
  - `CameraState` (position, zoom, rotation, viewport behavior)
  - `CameraController` (follow target, dead-zone, smoothing, bounds clamp, shake)
  - `CameraMode` (free, follow entity, scripted)
- Keep render integration through existing camera math path; this milestone standardizes ownership and update flow.
- Camera update runs as a normal system and writes one authoritative camera state per frame.
- Provide example-level hooks for gameplay-specific camera tuning without forking engine internals.

**Done when:**
- Platformer uses camera follow + bounds clamp through the camera module, not custom one-off logic.
- Camera position/zoom used by render path comes from the new authoritative camera state.
- Camera behavior is testable in at least one deterministic scripted scenario.

### M17 - Display State + Config

**Goal:** Introduce a display abstraction that owns runtime display/window settings and prepares for future settings UI.

**Design:**
- Add `DisplayState` resource/class-like module for current display mode, resolution, vsync, scale mode, and fullscreen intent.
- Add file-backed display config (`display.json` or `tungsten.json` section) with load/validate/apply flow at startup.
- Runtime changes go through a single API (`apply_display_settings`) so future in-game settings menu can reuse it.
- Keep menu UI out of scope for Phase 3; this milestone only establishes data model + application path.

**Done when:**
- Engine starts with display settings from config and reports active values in runtime telemetry.
- Resolution/fullscreen/vsync change path exists behind one API boundary (even if examples expose it via debug keys only).
- Invalid display config fails gracefully (fallback to safe defaults + log warning).

### M18 - Runtime Telemetry HUD

**Goal:** Add a lightweight in-game HUD that exposes high-value runtime state for developers and playtesters.

**Why early:** It speeds up every later milestone by making correctness/perf issues visible during normal gameplay, not only after failures.

**Design:**
- Add a `DebugHud` resource rendered via the existing text pipeline (no new UI library).
- Add a small telemetry model in the engine (key/value rows) and render it in a fixed screen corner.
- Built-in rows include:
  - FPS + frame time (ms)
  - Camera mode + position/zoom
  - Display mode (resolution, fullscreen, vsync)
  - Player position/speed (when a tagged player entity exists)
  - Active state/scene name
  - Entity count and sprite count
  - Last-frame system timing summary (top N slowest systems)
- Toggle key defaults to `F4`; HUD is off by default in release-oriented examples.
- Add an opt-in extension point so examples can register custom rows without changing engine code.

**Done when:**
- Platformer example shows FPS, camera position, player position, and player speed in real time.
- HUD shows camera and display state from `M16`/`M17` resources.
- HUD toggle works reliably and does not interfere with existing debug toggles.
- HUD rendering has negligible cost at Phase 3 scale (captured in benchmark notes).

### M19 - Input Mapping

**Goal:** Replace hardcoded key checks with action-based bindings.

**Design:**
- Add `ActionMap` resource loaded from optional `input.json`.
- API parity with input state: `is_pressed`, `just_pressed`, `just_released`.
- Keep hot-reload behavior for `input.json`.

**Done when:**
- An example migrates from raw key checks to action checks.
- Rebinding via `input.json` works at runtime.

### M20 - Scene/State System

**Goal:** Support menu/gameplay/pause transitions without manual world reset logic.

**Design:**
- Add `GameState` trait (`on_enter`, `on_exit`, state-scoped systems).
- Add `StateStack` resource with deferred `push/pop/replace`.
- Tag scene-owned entities and clean them on exit.
- Add minimal `scene.json` format for data-driven scene entity spawn.

**Done when:**
- Example flow: MainMenu -> Gameplay -> Pause -> Gameplay.
- Enter/exit hooks spawn/despawn cleanly.
- Smoke tests pass.

**Risk to manage:** runtime system-list churn; prefer a single dispatcher system over app-loop rewiring.

### M21 - Debug Tooling

**Goal:** Ship practical debugging/profiling tools using current render primitives.

**Note:** M18 ships first and provides the persistent textual telemetry layer; M21 focuses on geometric overlays/inspection workflows.

**Deliverables:**
- `DebugDraw` resource (`draw_aabb`, `draw_circle`, `draw_line`), cleared each frame.
- Physics debug overlay toggle (`F1`).
- Per-system timing overlay with rolling average (`F2`).
- Text-only entity inspector (`F3`) with opt-in `Inspectable` trait.
- Screenshot capture + baseline image-diff helper for visual regression checks.

**Done when:**
- All overlays toggle and display correctly in platformer example.
- Collider visuals match expected world bounds.
- Screenshot capture and image-diff checks run for at least one representative scene.

### M22 - Sprite Atlases

**Goal:** Reduce texture bind churn while keeping game API unchanged.

**Design:**
- Pack sprites into atlas textures at load time (in-engine packer, no new dep).
- Store UV rect per sprite asset and keep sprite ID access unchanged.
- Split atlases by sampler mode (`nearest` vs `linear`).
- On hot-reload growth, allow full rebuild and log warning.

**Done when:**
- Existing examples render correctly.
- Texture count is measurably lower on representative scenes.
- Filter behavior is unchanged (`nearest` and `linear` parity verified against pre-atlas output).

### M23 - Particle System

**Goal:** Provide reusable particle effects without new render pipeline work.

**Design:**
- `ParticleEmitter` supports burst/continuous modes and bounded emission.
- Tick system advances particles, emits new particles, despawns expired particles via command buffer.
- Reuse `Sprite` path from M15.
- Support hot-reloadable emitter config.

**Done when:** Example shows explosion/trail effects and runtime config edits apply.

### M24 - Tween System

**Goal:** Add lightweight property animation and completion signaling.

**Design:**
- `Tween` component with target, easing, duration, elapsed.
- Built-in easings only (no dependency).
- On completion: emit `TweenComplete` event and remove tween via command buffer.
- Scene JSON can define tweens.

**Done when:** Example animates UI/state transitions and reacts to `TweenComplete`.

## Benchmark and quality gates

Track these in existing bench suite:
- 50k position integration
- 10k entities across 5 archetypes (`query3`)
- Broad-phase rebuild (5k dynamic bodies)
- Sprite extract batch build (2k sprites)
- Atlas pack startup cost (200 sprites)
- Command-buffer flush cost (1k deferred spawns)
- Event-queue flush cost (10 queue types)

Add automated in-game checks:
- Deterministic screenshot tests for representative scenes (fixed frame, camera, seed).
- Scripted input playback for at least one menu -> gameplay -> pause scenario.
- Optional AI-based visual triage on failures (non-blocking; not a release gate).
- Telemetry HUD snapshot check for at least one representative example (HUD values present and updating).
- Display config load/apply check with fallback behavior for invalid values.
- Perf baseline replay check for canonical scenes (compare against M12 capture envelope).

Close each milestone only after:
- `cargo test --workspace`
- Smoke tests for impacted engine/example wiring
- Benchmark pass (for affected scenarios) against baseline with thresholds:
  - Baseline is the immediately previous milestone on the same machine/profile/build mode.
  - Steady-state runtime benches (`integration`, `query3`, `broad-phase`, `sprite extract`, `flush`): <= 10% regression.
  - Startup-only bench (`atlas pack`): <= 20% regression unless accompanied by a documented runtime win.
  - Any threshold break requires explicit rationale in `DECISIONS.md` before milestone close.
- `DECISIONS.md` entry for any non-obvious design choice or new dependency

## Phase 3 done when

- [ ] Multi-screen game loop (menu/gameplay/pause) ships without custom extract plumbing
- [ ] Runtime spawn/despawn and event flows work without `&mut World` iteration hazards
- [ ] Debug overlays are one-key-toggle in at least one representative example
- [ ] Performance baseline and profiling workflow exist before major feature milestones
- [ ] Camera module owns camera behavior for at least one representative gameplay example
- [ ] Display state/config layer is active and future settings-menu-ready
- [ ] Runtime telemetry HUD exposes core state (FPS/camera/player/system timing) in a representative example
- [ ] Deterministic screenshot + scripted input checks run for representative flows
- [ ] Sprite atlas path is transparent to game code and reduces texture pressure
- [ ] Bench scenarios above are recorded and reviewed for regressions
- [ ] Example smoke runs pass for all examples
