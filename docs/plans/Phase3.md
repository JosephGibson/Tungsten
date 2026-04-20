---
status: in progress
goal: Ship Phase 3 capabilities for production-style 2D games
non-goals: networking, 3D, scripting, WASM, parallel scheduler, full UI library
files-to-touch: crates/tungsten-core/src/ecs/, crates/tungsten-core/src/, crates/tungsten/src/, crates/tungsten-render/src/, examples/, tungsten.json
---

# Phase 3 Rollout Plan

## Scope and Sequencing

| # | Milestone | Track | Depends on | Unblocks |
| --- | --- | --- | --- | --- |
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

Recommended execution order: `M12 -> M13 -> M14 -> M15 -> M16 -> M17 -> M18 -> M19 -> M20 -> M21 -> M22 -> M23 -> M24`.

Deferred to Phase 4: change detection, full UI library, save/load, scripting, parallel scheduler.

## Current Status

- Workspace version metadata: `0.17.0`
- Current branch: `0.17`
- Completed milestones: `M12` profiling baseline, `M13` command buffers, `M14` event queues, `M15` transform + render components, `M16` camera module, `M17` display state + config, `M18` runtime telemetry HUD, `M19` input mapping, `M20` scene/state system
- Next recommended milestone: `M22 — Sprite Atlases`
- In progress milestone: `M21 — Debug Tooling`
- Archived detailed milestone plans: [M12](archive/Phase3-Milestone12-plan.md), [M13](archive/Phase3-Milestone13-plan.md), [M14](archive/Phase3-Milestone14-plan.md), [M15](archive/Phase3-Milestone15-plan.md), [M16](archive/phase3-milestone16-plan.md), [M17](archive/Phase3-Milestone17-plan.md), [M18](archive/Phase3-Milestone18-plan.md), [M19](archive/phase3-milestone19-plan.md), [M20](archive/Phase3-Milestone20-plan.md)

## Execution Contract

- Implement milestones strictly in `M#` order unless a dependency explicitly permits parallel work.
- For each milestone, implement only the scoped deliverables, run required checks, update plan status notes, then move to the next milestone.
- Archive milestone-specific plans under `docs/plans/archive/` once they are `done`, so only active rollout docs remain at the top level.
- Do not introduce new runtime dependencies without a `DECISIONS.md` entry.
- Keep ownership boundaries explicit: ECS/data objects in `tungsten-core`, app wiring in `tungsten`, GPU/render primitives in `tungsten-render`.

## Core Objects Introduced In Phase 3

- `CommandBuffer` (`tungsten-core`): deferred structural world mutation
- `EventQueue<T>` (`tungsten-core`): typed 2-window event buffering
- `Transform` / `Sprite` / `Visibility` / `Tag` (`tungsten-core`): baseline gameplay/render components
- `CameraState` / `CameraController` / `CameraMode` (`tungsten` + core-facing data where needed): authoritative camera flow
- `DisplayState` / `DisplayConfig` (`tungsten-core`) + request/apply wiring (`tungsten`): runtime display settings boundary
- `DebugHud` telemetry model (`tungsten`): in-game text diagnostics and extension hook
- `StateStack` + `GameState` (`tungsten`): scene/state transitions

## Guardrails

- Keep ECS structural mutation deferred, then flush at a fixed frame boundary.
- Keep events typed and buffered for at least two update windows to avoid order-sensitive drops.
- Keep frame-boundary order explicit and stable: run systems -> flush command buffers -> flush event queues -> hot reload -> extract -> render.
- Prefer deterministic behavior: flush buffers in stable registration order, not undefined order.
- Keep the public API surface minimal: `World` gets `flush`; advanced behavior stays in resources/helpers.
- Define milestone completion by observable behavior and tests, not implementation details.
- Run validation per milestone: `cargo test --workspace`; run smoke tests when engine/example wiring changes.

## Milestones

### M12 - Performance Baseline + Profiling Harness

> **Status: complete** (`v0.9.0`, `2026-04-15`)

- Goal: establish reliable CPU/GPU diagnostics and a reproducible baseline before adding more engine complexity.
- Why now: feature work without early perf visibility compounds regressions and makes root-cause analysis harder.
- Design: define canonical perf scenes (at least platformer + one sprite-heavy stress scene) and fixed capture rules (build mode, backend, resolution, frame window); add CPU frame-stage timings for update, extract, render, audio, and hot-reload plus per-system timing summaries; add offline CPU profiling workflow notes/scripts (`perf`, `cargo flamegraph`, platform equivalent); add a GPU diagnostics path that exposes frame time + render-stage timing where available, supports GPU frame capture such as RenderDoc, and documents `WGPU_BACKEND` overrides and capture metadata; define milestone perf budget targets as FPS + frame-time envelopes per canonical scene.
- Done when: baseline captures are recorded for canonical scenes, the team can identify top CPU hotspots from representative profiling output, GPU capture workflow is documented and validated on at least one Linux machine, and later milestones reference this baseline when evaluating regressions.

### M13 - Command Buffers

> **Status: complete** (`v0.10.0`, `2026-04-15`)

- Goal: remove `&mut World` structural-mutation pressure inside system loops.
- Design: add `CommandBuffer` for `spawn`, `despawn`, `insert`, and `remove_component`; apply commands after systems run and before extract/render; return `PendingEntity` from `spawn`, resolved during flush; preserve command order within a buffer and stable system order across buffers.
- Done when: systems can spawn/despawn/insert/remove through command buffers, the `World` API change is limited to `flush`, and existing examples still pass smoke tests.
- Risk: placeholder IDs used too early; document the next-frame visibility rule clearly.

### M14 - Event Queue

> **Status: complete** (`v0.11.0`, `2026-04-16`)
> Detailed implementation plan archived at [`docs/plans/archive/Phase3-Milestone14-plan.md`](archive/Phase3-Milestone14-plan.md).

- Goal: replace ad hoc event resources with one typed engine pattern.
- Design: add `EventQueue<T> { current, previous }`; `send()` appends to `current`; readers iterate `previous + current`; `flush()` rotates buffers once per frame at the same boundary as command flush; migrate `CollisionEvents` to `EventQueue<CollisionEvent>`; add `App::register_event::<T>()`.
- Done when: physics behavior is unchanged from the user perspective after migration, the queue works for arbitrary event types, and flush is automatic with no per-system manual clear.
- Risk: missed reads from run-conditions; keep the two-window lifetime and document it.

### M15 - Transform + Render Components

> **Status: complete** (`v0.12.0`, `2026-04-16`)
> Detailed implementation plan archived at [`docs/plans/archive/Phase3-Milestone15-plan.md`](archive/Phase3-Milestone15-plan.md).

- Goal: make common sprite rendering data-driven without custom extract closures.
- Add: `Transform { position, rotation, scale }`, `Sprite { asset_id, color, z_order }`, `Visibility { visible }`, and `Tag { name }` (debug aid).
- Rules: keep physics `Position` separate (per `D-033`); add an explicit sync system for `Position -> Transform.position`; if no custom sprite extract is configured, use default extract for `Transform + Sprite + Visibility`; `Visibility` is required; entities intended for default sprite extract must add `Visibility`; the default remains explicit as `visible: true`.
- Done when: a new example renders rotated/scaled sprites with components only, existing examples with custom extract still work unchanged, the default extract path enforces `Visibility` with no implicit fallback, and at least one example validates the explicit `Visibility` migration path.

### M16 - Camera Module

> **Status: complete** (`v0.13.0`, `2026-04-17`)

- Goal: centralize camera behavior in one engine module/class-like API instead of ad hoc example logic.
- Design: add `CameraState` (`position`, `zoom`, `rotation`, viewport behavior), `CameraController` (follow target, dead-zone, smoothing, bounds clamp, shake), and `CameraMode` (free, follow entity, scripted); keep render integration through the existing camera math path; standardize ownership/update flow; run camera update as a normal system; write one authoritative camera state per frame; provide example-level hooks for gameplay-specific tuning without forking engine internals.
- Done when: platformer uses camera follow + bounds clamp through the camera module rather than one-off logic, render consumes position/zoom from the new authoritative camera state, and at least one deterministic scripted scenario tests camera behavior.

### M17 - Display State + Config

> **Status: complete** (`v0.14.0`, `2026-04-17`)
> Detailed implementation plan archived at [`docs/plans/archive/Phase3-Milestone17-plan.md`](archive/Phase3-Milestone17-plan.md).

- Goal: introduce a display abstraction that owns runtime display/window settings and prepares for future settings UI.
- Design: add core-owned `DisplayState` / `DisplayConfig`, store display settings in the workspace-root `tungsten.json` `display` section, expose runtime changes through `request_display_settings`, and apply all window/surface mutations only at the top of `RedrawRequested`; keep menu UI out of scope for Phase 3 because this milestone only establishes the data model and application path.
- Done when: the engine starts with display settings from config and reports active values in runtime telemetry, one API boundary exists for resolution/fullscreen/vsync changes even if examples expose it only through debug keys, and invalid display config fails gracefully with safe defaults plus a warning log.

### M18 - Runtime Telemetry HUD

> **Status: complete** (`v0.15.0`, `2026-04-18`)
> Detailed implementation plan archived at [`docs/plans/archive/Phase3-Milestone18-plan.md`](archive/Phase3-Milestone18-plan.md).

- Goal: add a lightweight in-game HUD for developers and playtesters.
- Why early: it makes correctness and perf issues visible during normal gameplay rather than only after failures.
- Design: add a `DebugHud` resource rendered through the existing text pipeline; add an engine telemetry model of key/value rows in a fixed screen corner; built-in rows include FPS + frame time (`ms`), camera mode + position/zoom, display mode (resolution, fullscreen, vsync), player position/speed when a tagged player entity exists, active state/scene name, entity count and sprite count, and a last-frame system timing summary for the top `N` slowest systems; default toggle is `F4`; default state is off in release-oriented examples; add an opt-in extension point so examples can register custom rows without engine changes.
- Done when: platformer shows FPS, camera position, player position, and player speed in real time; HUD shows camera and display state from `M16` / `M17`; the toggle is reliable and does not interfere with existing debug toggles; HUD cost is negligible at Phase 3 scale and captured in benchmark notes.

### M19 - Input Mapping

> **Status: complete** (`v0.16.0`, `2026-04-19`)
> Detailed implementation plan archived at [`docs/plans/archive/phase3-milestone19-plan.md`](archive/phase3-milestone19-plan.md).

- Goal: replace hardcoded key checks with action-based bindings.
- Design: add `ActionMap` loaded from optional `input.json`; keep API parity with input state through `is_pressed`, `just_pressed`, and `just_released`; add mouse buttons, cursor + wheel surfaces, live `input.json` hot reload, runtime persist writes, and engine-owned action bindings for HUD/display/exit controls.
- Done when: in-tree examples read gameplay input through actions, rebinding through `input.json` works at runtime, mouse buttons and scroll dispatch through the same pipeline, and engine-owned controls round-trip through the action map rather than hardcoded key branches.

### M20 - Scene/State System

> **Status: complete** (`v0.17.0`, `2026-04-20`)
> Detailed implementation plan archived at [`docs/plans/archive/Phase3-Milestone20-plan.md`](archive/Phase3-Milestone20-plan.md).

- Goal: support `MainMenu -> Gameplay -> Pause -> Gameplay` style flow without manual world-reset logic.
- Design: add a `GameState` trait with `on_enter`, `on_exit`, and state-scoped systems; add `StateStack` with deferred `push` / `pop` / `replace`; tag scene-owned entities and clean them on exit; add a minimal `scene.json` for data-driven scene entity spawn.
- Done when: example flow is `MainMenu -> Gameplay -> Pause -> Gameplay`, enter/exit hooks spawn and despawn cleanly, and smoke tests pass.
- Risk: runtime system-list churn; prefer a single dispatcher system over app-loop rewiring.

### M21 - Debug Tooling

> **Status: in progress** (target `v0.18.0`)
> Detailed implementation plan: [`docs/plans/archive/Phase3-Milestone-21-debug-tooling.md`](archive/Phase3-Milestone-21-debug-tooling.md).

- Goal: ship practical debugging/profiling tools using current render primitives.
- Note: M18 ships first and provides persistent textual telemetry; M21 focuses on geometric overlays and inspection workflows.
- Deliverables: `DebugDraw` with `draw_aabb`, `draw_circle`, and `draw_line`, cleared each frame; physics debug overlay toggle on `F1`; per-system timing overlay with rolling average on `F2`; text-only entity inspector with opt-in `Inspectable` trait on `F3`; screenshot capture + baseline image-diff helper for visual regression checks.
- Done when: all overlays toggle and display correctly in platformer, collider visuals match expected world bounds, and screenshot capture and image-diff checks run for at least one representative scene.

### M22 - Sprite Atlases

- Goal: reduce texture bind churn while keeping the game-facing API unchanged.
- Design: pack sprites into atlas textures at load time with an in-engine packer and no new dependency; store UV rect per sprite asset; keep sprite ID access unchanged; split atlases by sampler mode (`nearest`, `linear`); on hot-reload growth, allow full rebuild and log a warning.
- Done when: existing examples render correctly, texture count is measurably lower on representative scenes, and filter behavior is unchanged with `nearest` / `linear` parity verified against pre-atlas output.

### M23 - Particle System

- Goal: provide reusable particle effects without new render-pipeline work.
- Design: `ParticleEmitter` supports burst/continuous modes and bounded emission; a tick system advances particles, emits new particles, and despawns expired particles through the command buffer; reuse the `Sprite` path from `M15`; support hot-reloadable emitter config.
- Done when: an example shows explosion/trail effects and runtime config edits apply.

### M24 - Tween System

- Goal: add lightweight property animation and completion signaling.
- Design: add a `Tween` component with target, easing, duration, and elapsed; use built-in easings only and add no dependency; on completion, emit `TweenComplete` and remove the tween through the command buffer; allow scene JSON to define tweens.
- Done when: an example animates UI/state transitions and reacts to `TweenComplete`.

## Benchmark And Quality Gates

Track these in the existing bench suite: `50k` position integration; `10k` entities across `5` archetypes (`query3`); broad-phase rebuild (`5k` dynamic bodies); sprite extract batch build (`2k` sprites); atlas pack startup cost (`200` sprites); command-buffer flush cost (`1k` deferred spawns); event-queue flush cost (`10` queue types).

Add automated in-game checks:

- deterministic screenshot tests for representative scenes (fixed frame, camera, seed)
- scripted input playback for at least one `menu -> gameplay -> pause` scenario
- optional AI-based visual triage on failures (non-blocking; not a release gate)
- telemetry HUD snapshot check for at least one representative example (HUD values present and updating)
- display config load/apply check with fallback behavior for invalid values
- perf baseline replay check for canonical scenes (compare against the M12 capture envelope)

Close each milestone only after:

- `cargo test --workspace`
- smoke tests for impacted engine/example wiring
- a benchmark pass, for affected scenarios, against baseline thresholds where the baseline is the immediately previous milestone on the same machine/profile/build mode
- steady-state runtime benches (`integration`, `query3`, `broad-phase`, `sprite extract`, `flush`) stay within `<= 10%` regression
- startup-only bench (`atlas pack`) stays within `<= 20%` regression unless accompanied by a documented runtime win
- any threshold break gets explicit rationale in `DECISIONS.md` before milestone close
- a `DECISIONS.md` entry exists for any non-obvious design choice or new dependency

## Phase 3 Done When

- [x] Multi-screen game loop (`menu` / `gameplay` / `pause`) ships without custom extract plumbing
- [ ] Runtime spawn/despawn and event flows work without `&mut World` iteration hazards
- [ ] Debug overlays are one-key-toggle in at least one representative example
- [ ] Performance baseline and profiling workflow exist before major feature milestones
- [x] Camera module owns camera behavior for at least one representative gameplay example
- [x] Display state/config layer is active and future settings-menu-ready
- [x] Runtime telemetry HUD exposes core state (`FPS` / `camera` / `player` / system timing) in a representative example
- [ ] Deterministic screenshot + scripted input checks run for representative flows
- [ ] Sprite atlas path is transparent to game code and reduces texture pressure
- [ ] Bench scenarios above are recorded and reviewed for regressions
- [ ] Example smoke runs pass for all examples
