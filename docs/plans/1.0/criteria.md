# Tungsten 1.0 criteria — rough draft

- **status:** draft
- **goal:** Agree what 1.0 means for Tungsten, and which workstreams, gates and order reach it. Start from the two owner-confirmed workstreams (the UI and text suite, a multi-core rendering pass) and the items earlier decisions deferred to "the 1.0 criteria plan", which this file is. Make building a game in its own repository practical: a template, a component kit, tooling and an authoring API. Close with benchmark, performance and QA phases.
- **non-goals:** Implementation, dependency or decision entries, release work, dates, or version numbers before 1.0. Detail beyond scoping: each agreed workstream gets its own plan (the UI suite already has one).
- **files to touch:** This draft only.
- **ordered steps:** Settle the 1.0 definition and the `wgpu`/`winit` API policy (§1, §7); tier the candidate workstreams (§3), including the developer-workflow ones (§2.2, §8.3–§8.7); run the multi-core spike and agree the frame-loop design with the game-facing stages (§5, §6, §8.6) before UI input routing (§4); collect API breaks for the freeze (§7); fix the closing phases and the release-candidate checks (§8.8).
- **done-when:** The owner has confirmed the 1.0 definition, a must/should/later tier and a done-when sketch for every workstream and closing phase, the order of the coupled workstreams (the [implementation plan](implementation-plan.md), §9), and an answer or an owner for every question in §10.

Date: 2026-10-02, at 0.38.0 (`95e1947`); the revisions since are listed under [Revisions](#revisions) at the end. Ultra rough: every name, tier, number and order below is a proposal for discussion, except what cites the definition gate's decisions (`D-102`–`D-104`). Statements about the code were checked against the tree on the date of the revision that made them; benchmark figures are the dated readings in [benchmarks.md](../../perf/benchmarks.md). The [implementation plan](implementation-plan.md) turns this into phases, gates and candidates; the amendments it proposed (its §8) were accepted at the definition gate and are folded in.

## Context digest

- Tungsten is at 0.38.0 with Phases 1–4 complete (M0–M31): archetypal ECS, manifest assets with hot reload, wgpu sprite, lit, material and mesh-particle rendering with an 18-effect post stack, SMAA and bloom, glyphon text, cpal audio, Tiled maps, a hand-rolled physics solver, a state stack with transitions, telemetry and a six-benchmark suite.
- "1.0" has no written definition. `D-088` and `D-094` already park one item here: the fixed-step accumulator with render interpolation, which is also the precondition for the deep-pile load limit in [known issues](../../known-issues.md#recorded-limits).
- Owner-confirmed for 1.0: the [UI, interface and text suite](w01-ui-text-suite.md) and a multi-core rendering pass; added the same day, a game clock with timers, logs and crash reports, and settings and save slots. Added 2026-10-03: a template, a component kit, a headless harness and project CLI, plugins and stages, tuple queries and bundles, prefabs, and budget-gated closing phases.
- No game has been built outside this repository. Every example runs from the repository root on the shared `tungsten.json`, `input.json` and `assets/manifest.json`, and every document is written for engine maintainers (§2.2).
- The frame is serial on the main thread. The only background threads allowed are the `cpal` callback and the `notify` watcher (`AGENTS.md` hard rule, kept by `D-067` when it dropped a parallel solver).
- No API-stability policy exists. The library crates are not marked `publish = false` and have not been published. Release archives ship examples for Linux and Windows only (`D-071`).
- The definition gate (2026-10-03, [implementation plan](implementation-plan.md) §11) settled definitions A and B, one acceptance game, gamepad, save slots, sliders and checkboxes, and R4 in 1.x (`D-102`); the stability and `wgpu`/`winit` policy (`D-103`); and the API snapshot tool (`D-104`).

## 1. What 1.0 means: decide first

| Definition | What it promises | Work it adds |
| --- | --- | --- |
| **A.** Complete for making games | A small, complete game builds in its own repository on the public API alone, with no engine patches | The UI, input, audio, physics and frame-loop gaps (§2.1); the developer-workflow gaps (§2.2): template, kit, tooling, authoring API, prefabs |
| **B.** Stable library | 1.x releases add without breaking; a documented public surface; a stated MSRV and platform tiers | API audit and freeze (§7), rustdoc, a policy for `wgpu`/`winit` types in the API |
| **C.** Public release | crates.io packages, a user guide, licence notices in archives | B plus distribution (W9) |

Decided at the definition gate: A and B, with C's crates.io publication left to question 2 (`D-102`). A is tested by one **acceptance game**, a top-down, survivors-like auto-shooter specified in [acceptance-game.md](acceptance-game.md) and started from the template (W12) when W15a lands (implementation plan amendment 8), in its own repository on a git dependency (owner, 2026-10-03), built only on the public API and `tungsten-kit`, with a title menu, settings (volume, rebinding, display), pause, settings persistence, gamepad play, a save slot and a release archive from `tungsten package` (W14). Every gap it hits is a 1.0 candidate; anything it does not need is 1.x.

## 2. Gaps that bear on 1.0

### 2.1 Engine features

| Area | Gap | Source |
| --- | --- | --- |
| UI and text | No widget, layout or focus system. Text alignment, wrap modes, ellipsis and measurement exist in cosmic-text but are unreachable. No DPI model | UI draft §2 |
| Input | Modifiers, focus, IME and produced text are dropped. Unlisted keys become `KeyCode::Other(<winit discriminant>)`, which is not stable across winit versions. Keys held across a focus loss never release. No gamepad support anywhere in the tree | UI draft §2 |
| Frame loop and time | Variable dt split across fixed substeps, capped at 0.1 s per frame and 1/30 s per physics step. No accumulator, no interpolation. `DeltaTime` holds only `dt`: no game clock apart from real time, no time scale, pause, elapsed time, frame index or timer type | `D-088`, `D-094`, [DESIGN](../../../DESIGN.md#physics--m11), [`time.rs`](../../../crates/tungsten-core/src/time.rs) |
| Threads | Serial frame; `World` is neither `Send` nor `Sync` | [w02](w02-multi-core-rendering.md#facts) |
| Physics | `BodyKind` is `Static` or `Dynamic`: no kinematic bodies (proposal G1), no sensors (G2), no collision layers or masks, no world ray or shape queries (`sweep_aabb_vs_aabb` and `SpatialGrid::query` exist as primitives) | [`physics/components.rs`](../../../crates/tungsten-core/src/physics/components.rs), [open proposals](../../perf/benchmarks.md#open-proposals) |
| Audio | `AudioCommand` is `Play`, `Stop`, `StopAll` and `SetMasterVolume`: no voice handles, pause, fades, pitch, pan or buses (music versus effects volume). P3s: channel mapping, allocation in the callback, dropped commands on a full ring | [`audio.rs`](../../../crates/tungsten-core/src/audio.rs), known issues |
| Rendering | Screen text draws above transitions; mesh particles draw above every sprite; at most 16 lights (G3); the default extract does not cull; sprite IDs are strings | Known issues, [engine findings](../../perf/benchmarks.md#engine-findings) |
| Errors | Runtime render errors are logged, not returned; a skipped acquisition can report capture success (P3) | Known issues |
| Diagnostics | The examples log to stderr through `env_logger::init()`. No binary sets `windows_subsystem`, so a Windows release opens a console window. No log file and no panic hook, so a crash in a windowed build leaves no trace | Example `main.rs` files |
| Persistence | `tungsten.json` and `input.json` are read from the working directory, and `ActionMap::persist` writes `input.json` back there. No per-user folder; display settings and volume are not persisted; no settings or save API (save/load is a [DESIGN](../../../DESIGN.md#non-commitments) non-commitment) | [`app.rs`](../../../crates/tungsten/src/app.rs), `D-008`, `D-045` |
| API surface | The umbrella re-exports whole crates (`pub use tungsten_core as core`, `pub use tungsten_render as render`). Render's public items take and return `wgpu` types; `translate_mouse_button` takes a `winit` type. No `missing_docs` lint | [`lib.rs`](../../../crates/tungsten/src/lib.rs) |
| Platforms | Metal, DX12, CoreAudio, WASAPI and the Windows archive launch never checked on real hosts; no macOS archive | Known issues, `D-071` |
| Distribution | Archives carry no third-party licence notices for statically linked crates | Known issues |
| Bugs on record | Burst emitters ignore `once` (the latch never resets); tilemaps at `z_norm` 0 cover every sprite under `gpu_depth`; a lit sprite with a material logs once per sprite per frame | Engine findings |

### 2.2 Developer workflow

Reviewed on 2026-10-03 at `afbc330` by reading how the four examples are wired and tested.

| Area | Pain point | Source |
| --- | --- | --- |
| Project setup | A game cannot live outside this repository. Every example runs from the root on the shared `tungsten.json`, `input.json` and `assets/manifest.json`. The debug HUD asks for the shared manifest's `mono` font. `tungsten-render` embeds `sprite.wgsl` and `lit_sprite.wgsl` from `../../../assets/shaders/`, outside its crate folder: a git dependency tolerates that, a packaged crate does not. No template and no getting-started guide; every document is written for engine maintainers | `debug_hud.rs` (`font_id: "mono"`), `render/renderer.rs`, `render/lit_sprite.rs`, [doc map](../../README.md) |
| App wiring | `App::new` inserts 35 resources, six event queues and seven engine systems, but the game registers the rest of the engine itself, in order: `physics_step`, `sync_position_to_transform`, `shake_tick_system`, the squash systems, `camera_update_system`. The platformer's `RUNTIME_SYSTEM_ORDER` holds 34 systems kept in order by comments. One flat list, no stages, no grouping | `app.rs` `App::new`, `examples/01_platformer/src/setup.rs` |
| Extract | One closure per kind replaces the default. The default sprite extract skips tilemaps, so a tilemap game composes `extract_sprites_default` with `extract_tilemaps` (as the bench does); the platformer replaced the default with a 1,023-line extract of its own | `app.rs` `stage_extract`, `examples/02_bench/src/gpu.rs`, `examples/01_platformer/src/extract.rs` |
| Common gameplay | `AnimationState` and `AnimationRegistry` are core types, but the system that advances them and the `CurrentSprite` it writes live in example 01, so animation never reaches the default extract. Ground detection, `Health`, lifetimes, out-of-bounds despawn, transient emitters, `cursor_to_world` and damage feedback are example code too. Physics `Position` reaches `Transform` through a system the game adds; no parent/child transforms | `examples/01_platformer/src/{systems,state,gameplay}.rs`, `core/components.rs` |
| ECS ergonomics | Queries are named by arity and filter (`query2_opt2_mut`, `query3_mut_without`). The common pattern collects entities, then calls `get` or `get_mut` per entity and clones around the borrow (`animation_system`). No spawn builder or bundle: the platformer's player takes 13 `insert` calls | `core/ecs/world.rs`, `examples/01_platformer/src/{systems,setup}.rs` |
| Silent mistakes | A sprite whose `asset_id` is not registered draws nothing and logs nothing; an unknown font logs a warning and falls back to sans-serif | `sprite_extract.rs` pass 1, `render/text/engine.rs` `section_attrs` |
| Data-driven entities | A scene entry holds transform, sprite, visibility, tag and tweens: no physics, light, emitter or game component. Levels are code; the platformer's `level_layout.rs` is 2,336 generated lines | `core/assets/scene.rs`, `examples/01_platformer/tools/` |
| Testing a game | Tests copy the frame loop by hand (systems, then a manual event flush) and skip particles, tweens, the command flush and event rotation. No headless `App` steps frames | `examples/01_platformer/src/tests/main.rs` |
| Iteration | Not a pain point: a touched example rebuilds in 1.1 s and a touched core file in 2.5 s (dev profile, dependencies at `opt-level = 2`), measured before `D-096` built `tungsten-core` at opt-level 1 in dev, which took a clean rebuild of core and its dependents from 5.0 to 6.8 s. Asset hot reload works, but only the platformer turns it on; config does not reload (DESIGN non-commitment) | Local timing, 2026-10-03; known-issues follow-ups |

## 3. Candidate workstreams and proposed tiers

| ID | Workstream | Source | Proposed tier |
| --- | --- | --- | --- |
| W1 | UI, interface and text suite | Owner | Must |
| W2 | Multi-core rendering pass | Owner | Must; R4 moved to 1.x (`D-102`), R1 and R3 too after the spikes (`D-131`, `D-126`): R0 and R2 remain |
| W3 | Frame loop v2: fixed step, interpolation, game clock and timers, one stage map | Deferred here by `D-088`, `D-094`; the game clock from the owner | Must |
| W4 | API stabilization and freeze | Definition B | Must (`D-102`) |
| W5 | Input: gamepad. Focus, modifiers and DPI are UI M0b's (implementation plan amendment 2) | §2 | Must (`D-102`) |
| W6 | Gameplay gaps: physics queries, sensors, kinematic bodies, layers; audio voices and buses | §2 | Tiered at the frame-loop gate from the acceptance game's mechanics rows (`D-132`): kinematic bodies, sensors, shape queries, shape and ray casts, layers and masks, audio buses and a voice limit per sound must; voice handles and fades should; audio pause 1.x |
| W7 | Platform certification and support tiers | Known issues | Must (tiers at least) |
| W8 | Correctness burn-down | Known issues, engine findings | Must |
| W9 | Distribution and documentation | Known issues, definition C | Licence notices and a getting-started guide on the template must; crates.io follows question 2 (`D-102`) |
| W10 | Closing phases: benchmark improvement (C1), performance improvement (C2), final QA pass (C3) | Benchmarks; owner, 2026-10-03 | Must; C2 is budget-gated (§8.8) |
| W11 | Shipping basics: logs, crash reports, settings and save slots | Owner | Must, save slots included (`D-102`) |
| W12 | Template project: `templates/basic`, the layout every game and example follows; the engine made self-contained for an outside repository | Owner, 2026-10-03; §2.2 | Must |
| W13 | `tungsten-kit`: generic components and systems in four groups (basics, transform hierarchy, character controllers, gameplay helpers) | Owner, 2026-10-03; §2.2 | Must per item, if a user ships it by the freeze (§8.4). Tiered at the frame-loop gate from the acceptance game's mechanics rows (`D-132`): every item has the game as a user except `cursor_to_world`, the platformer controller and the path follower, which are must only if example 01 migrates by the freeze |
| W14 | Tooling: headless test harness, project CLI (`new`, `check`, `package`) | Owner, 2026-10-03; §2.2 | Harness must; CLI `check` and `package` must, `new` should |
| W15 | Authoring API: plugins and named stages, additive extracts, tuple queries and bundles | Owner, 2026-10-03; §2.2 | Must, before the freeze |
| W16 | Prefabs: registered components in scenes, prefab files and Tiled objects | Owner, 2026-10-03; §2.2 | Must |

The definition gate confirmed the tiers that cite `D-102`, and the frame-loop gate tiered W6 and W13 from the acceptance game's mechanics rows (`D-132`, implementation plan §11). The rest stay as proposed.

## 4. W1: UI, interface and text suite

The [UI draft](w01-ui-text-suite.md) is the source of truth; this section only places it.

- **1.0 cut:** ladder steps M0–M6, from the text engine split to example migration, plus W1 BC before M4: sliders and checkboxes from the "broader controls" stage, which the settings screen needs (question 8, `D-102`; implementation plan amendment 15). Scroll containers are in M5 already. Editable fields, IME, accessibility and docking stay after 1.0. Controller navigation comes almost free now that W5 lands a gamepad (question 9): UI navigation reads `ui_*` actions, and a gamepad adds bindings without widget changes (UI draft §7).
- **Coupling with W2.** The UI draft assumes a serial frame: layout takes the text engine mutably after systems, and draw takes it shared (UI draft §4). A pipelined render thread ([w02](w02-multi-core-rendering.md#options), R4) draws frame N while layout runs frame N+1, so under the UI draft's glyph path T1, where glyphon draws from the engine's retained `Buffer`s, both would need the engine at once. T2's plain `PositionedGlyph` snapshot suits a render thread. The R4 answer was therefore an input to the T1/T2 gate and to where M0 puts the text engine. The definition gate moved R4 to 1.x (question 4, `D-102`), so M0a and M1 run in Phase 5's Track A without waiting for the frame-loop design (implementation plan amendment 7).
- **Coupling with W3.** Input routing (UI draft §7) adds frame stages; design it with W3's stage map. M3 waits for that design and W15a; M0b is independent of it (amendment 7).
- **Coupling with W8.** M2's direct-versus-capture check needs the capture-completion contract (P3) first.
- **Couplings with W11, W12 and W14.** M5's log console reads W11a's engine logger (amendment 4); M1's `UiHarness` builds on W14a's harness (amendment 5); M6 migrates examples 01, 03 and 04 in one pass each with W12b's move onto the template layout (amendment 6).

## 5. W2: multi-core rendering pass

Graduated on 2026-10-06: [w02](w02-multi-core-rendering.md) holds the scope, the R1 spike's verdict, the candidates and their done-when checks; this section only places it.

- **Tier:** Must; 1.0's scope is R0, landed in 0.45 (`D-113`, `D-114`), and R2, the late-acquire split, last in Phase 5's Track A with W1 M0b (`D-108`). R1 left 1.0 after its spike and R3 with the glyph gate's T1b (`D-131`, `D-126`); R4 is 1.x (question 4, `D-102`) and R5 waits for startup metrics. Each has a [backlog](backlog-1.x.md) row.
- **Couplings:** R4's answer let W1 M0a and M1 run in Track A (§4, amendment 7); W1 M2's overlay extends R2's split final pass (implementation plan §3); the threading rule, the worker mechanism and the `Send`/`Sync` policy are answered with no worker in 1.0 and no bound (`D-131`), so the worker-thread capture rules wait (§12); R0's ID break is in W4's ledger (§7), and the column-slice query is W15b's (§8.6, `D-130`).

## 6. W3: frame loop v2

Graduated on 2026-10-06: [w03](w03-frame-loop.md) holds the scope, the design spike's verdict, the candidates and their done-when checks; this section only places it.

- **Tier:** Must. W3a (`Time` and `Timer`) and W3b (the fixed step and interpolation) run second and fourth in Phase 5's Track C, after W15a and W15b, on the design the frame-loop gate settled (`D-129`, with `D-128`'s stages): real and game clocks, at most two fixed steps a frame, interpolation on by default through `PrevPosition` in the physics sync, so the `D-018` seam is untouched, and transitions, the HUD and UI on real time. The Phase 5 QA pass follows Track C (implementation plan amendment 18).
- **Couplings:** W3's stage map is W15's (§8.6, `D-128`); W1 M3's routing stage runs in `pre_update` once a frame on the real clock (§4, amendment 7); W13's timers and root interpolation (§8.4, `D-132`) and W6a's kinematic bodies in the fixed step (§8.1) build on it; the deep-pile limit's `contact_hertz` lever needs W3b's constant step ([known issues](../../known-issues.md#recorded-limits)); `DeltaTime`'s deprecation is a row in W4's ledger (§7); R4's hand-off point joins only if R4 comes back from 1.x (`D-102`).

## 7. W4: API stabilization and freeze

Graduated on 2026-10-03: [w04](w04-api-freeze.md) holds the scope, the break ledger, the candidates and their done-when checks; this section only places it.

- **Tier:** Must (`D-102`). W4a, the stability and `wgpu`/`winit` policy, landed at the definition gate as `D-103` and `D-104`; W4b, the freeze, runs in Phase 7 after C1.
- **Breaks:** every public-API break bound for 1.0 is a row in the [break ledger](w04-api-freeze.md#break-ledger), not a list here. A workstream that adds a break records it there.

## 8. Other workstreams

### 8.1 W5–W10 in brief

| ID | Scope sketch | Open points |
| --- | --- | --- |
| W5 Input | A gamepad backend through the action map. Focus-loss release, modifiers and the scale-factor resource are UI M0b's (implementation plan amendment 2) | A gamepad crate is a `D-015` rule 1 dependency: check its Linux backend for threads and `libudev` (no locked crate links it, and 0.40 QA step 12 removed `libudev-dev` from both workflows, so a crate that needs it puts the package back) |
| W6 Gameplay | Physics: kinematic bodies (G1), sensors (G2), layers and masks, ray and shape queries. Audio: voice handles, pause, fades, buses. The burst latch fix is W8a's (amendment 10) | The acceptance game cuts this, from its mechanics rows at the frame-loop gate; whatever it does not use moves to 1.x |
| W7 Platforms | Support tiers in the README: Linux is the only tested host today. A CPU-only Windows test job on the `windows-2025` image the release build already uses would cover core, persistence and the crash hook without a GPU, informational under `D-070`; it lands beside W11a as W7a (amendment 9). Windows and macOS rendering and audio need hosts or an explicit lower tier; decide whether releases build macOS | `D-003` (native only) stands |
| W8 Correctness | Known-issues P3s, capture completion first since it gates UI M2; the engine-finding bugs, the burst latch among them (amendment 10); the shader ID allocator; queued transitions | W8a in Phase 5. A QA pass in the 0.40 shape closes each of Phases 5 and 6, with W8b inside the second (amendment 18) |
| W9 Distribution | Graduated on 2026-10-03: [w09](w09-distribution.md) holds the scope, candidates W9a–W9c and their done-when checks; this row only places it | Licence notices and the guide Must; crates.io follows Q2 (`D-102`) |
| W10 Closing phases | Benchmark improvement, performance improvement and a final QA pass; W10's earlier list moved into C1 (§8.8) | Which rows get absolute budgets and which relative ones |

### 8.2 W11: shipping basics

Graduated on 2026-10-03: [w11](w11-shipping-basics.md) holds the scope, the candidates and their done-when checks; this section only places it.

- **Tier:** Must, save slots included (`D-102`). W11a (user folder, log file, crash report, release symbols, no console on Windows) runs in Phase 5's Track A beside W7a; W11b (settings) and W11c (save slots) run in Phase 6.
- **Couplings:** one engine logger, which W1 M5's log console reads (implementation plan amendment 4); W12a takes logging out of the examples' `main.rs` and shares W11's game identifier; W14b's `package` keeps debug files apart.

### 8.3 W12: template project

Graduated on 2026-10-03: [w12](w12-template.md) holds the scope, the layout, the candidates and their done-when checks; this section only places it.

- **Tier:** Must. W12a (self-containment, the template skeleton and the outside-copy check) runs in Phase 5's Track A after W11a and W9a; W12b moves examples 01, 03 and 04 onto the layout in Phase 6, one pass per example with W1 M6 (implementation plan amendment 6).
- **Couplings:** every workstream that changes how a game is written updates the template in the same step; the sprite shaders stay in `assets/shaders/` unless question 2 says yes (implementation plan amendment 14); the kit (§8.4), the authoring API (§8.6) and prefabs (§8.7) each change the template when they land.

### 8.4 W13: `tungsten-kit`

Owner-added on 2026-10-03, with all four groups in scope. A crate of generic components and systems that 2D games reuse; the template and the examples use it in place of their own copies.

- **Admission rule.** An item joins the kit if it is generic and has a user: the template, a migrated example or the acceptance game. Each item has rustdoc with an example and a harness test. An item with no user by the freeze moves to 1.x, which keeps four groups from becoming an open-ended must list.
- **Dependency direction.** Decided by the owner on 2026-10-03: the schedule, its stages and the `Plugin` trait live in `tungsten-core` over a `Schedule` type, not `App`. The kit depends on core, and the umbrella re-exports it behind a default feature, so a game has one dependency. Rejected: a kit that depends on the umbrella (two dependencies for games) and a feature-gated umbrella module (no crate boundary, no tier of its own). This fits `AGENTS.md` ("ECS → core"), but it only works if every plugin need goes through `Schedule` with no render types in core (`D-007`, `D-016`): systems by stage, event registration, inspector rows, component registration (W16a) and extract contributions. A kit item that needs an extract hook can't hand over a `SpriteBatch`, which is a render type. It writes core components the default extract already reads, as the animated sprite does, or it waits for W15's extract-contribution design. The W15 spike checks this before the decision entry.
- **Basics.** An animated sprite that writes `Sprite.asset_id` from `AnimationState`, so the default extract animates (replaces example 01's `CurrentSprite` and `animation_system`); lifetime and despawn-after timers on W3's `Timer`; despawn outside a region or the view; a spawner (interval, count, W16 prefab ID), which lands after W16b's prefab assets (implementation plan amendment 3); `cursor_to_world` on the camera; a sound played on an event.
- **Transform hierarchy.** `Parent`, `Children`, a local transform and a propagation stage before the extract, so the extract still reads one world `Transform` and the `D-018` seam and R1's slices stay as they are. Recursive despawn through the command buffer (`D-039`). Proposed for 1.0: physics bodies only on root entities, and the physics-to-`Transform` sync becomes an engine stage, not a game system. Interpolation (W3) applies to roots before propagation.
- **Character controllers.** A platformer controller (ground check, coyote time, jump buffer, variable jump height) and a top-down mover. Both need W6's kinematic bodies (G1), sensors (G2) and shape queries, which moves those from should to must. Example 01's player moves onto the platformer controller, and its ground detection (§2.2) goes.
- **Gameplay helpers.** Health and damage with invulnerability time and a hit flash on the `damage_flash` shader, which the kit takes over from the shared manifest; trigger zones on sensors (G2); a path follower for moving platforms on kinematic bodies (G1); a small per-entity state machine (enter, exit, update by state), separate from the game-level `StateStack`.
- **Stability.** Decided at the definition gate (question 24, `D-103`): the kit's API is inside the 1.0 promise through the umbrella's re-export, with `#[non_exhaustive]` on kit settings structs, and joins the freeze (§7).
- **Done-when sketch:** every item used by the template or a migrated example, with rustdoc and a harness test; example 01's copies of kit features deleted; `just physics-release` green, since the controllers move kinematic bodies through the physics step.

### 8.5 W14: tooling

Graduated on 2026-10-03: [w14](w14-tooling.md) holds the scope, the candidates and their done-when checks; this section only places it.

- **Tier:** the harness Must; the CLI's `check` and `package` Must and `new` Should, as proposed until question 20. W14a, the harness, runs first in Phase 5's Track A; W14b, the CLI, in Phase 6 after W12a, W9a, W11a and W16b.
- **Couplings:** W1 M1's `UiHarness` builds on W14a (implementation plan amendment 5); the CLI is the `tungsten-cli` package in `tools/cli/` with a `tungsten` binary (amendment 11); `check` reads prefabs (§8.7), and `package` reuses W9's notices and W11's symbol handling.

### 8.6 W15: authoring API

Graduated on 2026-10-06: [w15](w15-authoring-api.md) holds the scope, the spikes' verdicts, the candidates and their done-when checks; this section only places it.

- **Tier:** Must, before the freeze. W15a (stages and plugins), W15b (tuple queries and bundles) and W15c (additive extracts) run first, third and fifth in Phase 5's Track C, with W3a and W3b between them, after the frame-loop gate settled the design (`D-128`, `D-130`, `D-131`); the Phase 5 QA pass follows W15c (implementation plan amendment 18).
- **Couplings:** W3's stage map is W15a's (§6); W1 M3's routing stage joins `pre_update` after the state dispatcher, so M3 waits for W15a (§4, amendment 7); each candidate updates the template and the guide in its own step (§8.3, amendment 16), and the acceptance game's repository starts from the template when W15a lands (§1, amendment 8); the kit's `Schedule` and `Plugin` in core (§8.4) and W16a's registry (§8.7) build on W15a; W3b's `RigidBodyBundle` carries `PrevPosition` on W15b's bundles (`D-129`); extract contributions stay the umbrella's, since render types cannot enter core (`D-007`, `D-016`).

### 8.7 W16: prefabs

Owner-picked on 2026-10-03: data-driven entities with game components, without `World` serialization.

- **Component registry.** `app.register_component::<Health>("health")` for any serde type; engine and kit components come registered. Scene entries and prefab files carry `"components": { "health": { … }, "rigid_body": { … } }` beside today's fields. This is W16a, which lands before the kit so that kit components register as they land (implementation plan amendment 3).
- **Prefab assets.** W16b, before the kit's spawner (amendment 3). A `prefabs` manifest section, which adds a row to the [assets](../../assets.md) table (`D-106`): prefab ID to file. `world.spawn_prefab(id, position)` and a command-buffer form. A spawner (W13), or a Tiled object whose class names a prefab, spawns the same way, and the object's properties override fields. Nested prefabs, and patching live entities on hot reload, wait for 1.x: a reload changes later spawns only.
- **Errors.** An unknown component name or a bad field fails the load and names the file, entry and field (`D-022`). `tungsten check` (W14) reports it without running the game.
- **Out of scope.** World snapshots and saving by reflection; save slots stay game-owned serde types (W11).
- **Done-when sketch:** the template's player and enemy come from prefabs; example 01 spawns at least one object kind from a Tiled object layer; a malformed-prefab test for each kind of error.

### 8.8 Closing phases (W10)

Owner-added on 2026-10-03: three phases end the road to 1.0, and performance is budget-gated.

**C1 Benchmark improvement.**

- **Scope.** A dated 1.0 baseline of the six benchmarks. New rows for what games will run: UI layout and draw (W1), text-heavy screens, startup and load time (proposal T2), a frame of the template or acceptance game, hierarchy propagation and kit systems at scale, and tuple-query iteration in `ecs`. Also W10's earlier list: recalibrate the rows below their band (`workload_version` bumps), an RSS growth threshold for `churn`, proposal T1 (particle and tween timings), the frame-cap overshoot and the HUD `fps` row from `interval_ms`.
- **Budgets.** Each row gets a 1.0 budget in `benchmarks.md`. Rows that stand for a game frame get an absolute budget, for example p95 under 16.7 ms at 60 Hz on the reference machine. Other rows get a relative one: not regressed against the 0.38 baseline after an A/A. The profiling workflow's capture rules apply, extended for worker threads (§12).
- **Before the freeze.** C1's first reading lists the rows over budget. A fix that needs an API change lands before the freeze; the rest wait for C2.
- **Done-when sketch:** every row has a budget and a dated baseline; an A/A of the untouched tree gives no `regressed` or `improved` verdict on any row (the profiling workflow's A/A check); `just perf-test` green.

**C2 Performance improvement.**

- **Scope.** C1's list of rows over budget, largest gap first. Known candidates: whatever R0–R2 left, extract culling, `physics_step` in `integrated`, text preparation, startup decode (R5).
- **Rules.** After the freeze, internal changes only; an API change waits for 2.0 or takes a freeze exception in its decision entry. Each change ships compare verdicts on every CPU row its code touches, a padded-baseline build to rule out code-placement effects, and either unchanged digests or a `workload_version` bump with its reason.
- **Done-when sketch:** every budget passes or has an exception accepted in a decision entry; no row regressed against C1's baseline; `just visual`, `just smoke` and `just physics-release` green.

**C3 Final QA pass.** It runs on a release candidate; a failure is fixed and a new candidate tagged.

- **Automated:** `just check`, `just smoke`, `just visual`, `just physics-release`, `just script-test`, `just deps`, `just repo-check`, the release preflight and a `--no-pr` rehearsal; a clean clone built with the pinned toolchain; `tungsten new`, `check`, tests and `package` outside the workspace.
- **Owner playthrough on the Linux reference machine,** from a scripted checklist: the acceptance game from its release archive, start to finish; settings changed, kept across a restart, and a corrupt settings file falling back; rebinding; gamepad; pause and transitions; a save slot written, loaded, and loaded again after a schema bump; a deliberate panic leaving a crash file that symbolizes; each display mode and present mode; hot reload in a debug build.
- **Platforms:** Windows and macOS checked as far as hosts allow, results in [known issues](../../known-issues.md#platform-checks-with-no-host-available), and support tiers stated in the README (W7).
- **Documentation:** the getting-started guide followed word for word on a clean machine; rustdoc clean under `missing_docs`; README and DESIGN describe 1.0.
- **Agent-built games:** a fresh Claude Code session in a copy of the template, given only its `AGENTS.md`, the getting-started guide and rustdoc, adds a scripted feature with no engine patches (RC-A9; implementation plan amendment 16).
- **Known issues:** no open P1 or P2; each P3 has a 1.x home or an accepted note.
- **API:** the public surface diffed against the freeze snapshot shows no change. The snapshot is `cargo-public-api`'s tracked text file (`D-104`), chosen at the definition gate so that new surface shows release by release rather than all at once at the freeze (implementation plan amendment 17).
- **Done-when sketch:** a dated QA record with each item passed or accepted by the owner; that release candidate tagged 1.0.

## 9. Order

The [implementation plan](implementation-plan.md) holds the order: three phases (5 Foundations, 6 Features, 7 Release) in place of the five this section sketched, six gates, the candidates in dependency order and the register from candidate to milestone and release. The definition gate agreed it on 2026-10-03, and this section's sketch gave way to this link (implementation plan amendment 13).

## 10. Questions for the owner

Open questions keep their numbers; answered ones are under [Answered](#answered) below. The [implementation plan](implementation-plan.md) §7 says by when each is needed.

- **2.** Publish the library crates on crates.io for 1.0?
- **3.** Platform tiers: certify Linux only, or find Windows and macOS hosts? Should releases build macOS?
- **12.** Which performance budgets gate 1.0?
- **15.** Which settings does the engine persist itself, and does a corrupt user file reset to defaults (proposed) or stop the game as an invalid `tungsten.json` does?
- **20.** CLI: `check` and `package` must and `new` should (proposed)? Argument parsing and archive writing by hand or by crate (§8.5)?
- **22.** Controllers: anything beyond ground check, coyote time, jump buffer and variable jump, such as slopes, one-way platforms or ladders?

### Answered

Answered on 2026-10-03: a game lives in its own repository on a git dependency, so crates.io is not needed for 1.0 (whether to publish anyway stays question 2); the template is the in-repo `templates/basic`; the kit is a new crate with all four groups; 1.0 tooling is the headless harness and the project CLI; plugins with named stages; tuple queries and bundles; prefabs with registered components; examples 01, 03 and 04 move to the template, the bench does not; the acceptance game starts from the template (part of question 1, answered in full below); performance is budget-gated; the final QA is an owner playthrough with Linux certified and the other platforms on stated tiers (part of question 3; a macOS release build is still open). The kit's controllers and trigger zones make G1, G2 and shape queries must (part of question 10).

- **1.** Which definition of 1.0 (§1)? Is an acceptance game the test, and what game is it? *Answered 2026-10-03 at the definition gate: A and B, with crates.io left to question 2; one acceptance game, a top-down, survivors-like auto-shooter, whose pitch, screens and mechanics are due at the frame-loop gate (`D-102`).*
- **4.** Multi-core scope: fork-join only (R1, R2), or also a pipelined render thread (R4) with its extra frame of latency? *Answered 2026-10-03 at the definition gate: fork-join only; R4 moves to 1.x and comes back only if C1 shows a game-frame row over budget that R4 would fix (`D-102`).*
- **5.** Worker mechanism: `std::thread::scope`, a rayon pool, or an own pool? *Answered 2026-10-06 at the frame-loop gate: moot for 1.0, since R1 left it after its spike; a rayon `ThreadPool` owned by `App` if a fork-join stage ever lands (`D-131`).*
- **6.** Require `Send + Sync` on components and resources before the freeze? *Answered 2026-10-06 at the frame-loop gate: no; a parallel path bounds the types it slices at its own call site (`D-131`).*
- **7.** Fixed step: physics only, or a fixed-update schedule? Interpolation by default? *Answered 2026-10-06 at the frame-loop gate: a `fixed_update` stage that takes any system, physics as one plugin's pair in it; interpolation on by default through `PrevPosition` spawned with the body; at most two steps a frame (`D-129`).*
- **8.** UI cut line: broader controls (sliders, checkboxes, scroll) in 1.0? A single-line text field with IME? *Answered 2026-10-03 at the definition gate: sliders and checkboxes in, as W1 BC before M4; scroll containers are in M5 already; text fields and IME out (`D-102`).*
- **9.** Gamepad in 1.0? *Answered 2026-10-03 at the definition gate: in; W5 is Must (`D-102`).*
- **10.** Minimum audio and physics features for 1.0 (W6)? *Answered 2026-10-06 at the frame-loop gate from the acceptance game's mechanics rows: kinematic bodies, sensors, shape queries, shape and ray casts, layers and masks, audio buses and a voice limit per sound must; voice handles and fades should; audio pause 1.x (`D-132`).*
- **11.** `wgpu` and `winit` types in the stable API: hide them, tier them, or accept their majors? *Answered 2026-10-03 at the definition gate: hide them, behind a curated umbrella re-export, engine-owned input and format types, and a game-facing handle or `doc(hidden)` tier for the `Renderer` methods that use `wgpu` types (`D-103`).*
- **13.** Game clock: which engine stages keep running on real time while gameplay is paused (§6)? *Answered 2026-10-06 at the frame-loop gate: state transitions, the HUD, overlays, the inspector, UI animation and navigation timing run on real time; tweens, particles, game feel, the camera and game systems on the game clock; a paused game skips the physics stage (`D-129`).*
- **14.** Logs and crash reports: on by default in `App`, or one opt-in call? *Answered 2026-10-05 with M35's approval, its stated default: on by default. `App::new` installs the engine logger unless the game set one and, where the game has a user folder, the crash hook; a game opts out by installing its own logger before `App::new` and its own hook after it (`D-119`).*
- **16.** Save slots in 1.0, or settings only? *Answered 2026-10-03 at the definition gate: in; W11c is Must (`D-102`).*
- **17.** Kit dependency direction (§8.4). *Answered 2026-10-03: `Schedule` and `Plugin` in core; the umbrella re-exports the kit.*
- **18.** Where do examples run from (§8.3)? *Answered 2026-10-03: their own folders, like a copied template.*
- **19.** Engine-owned text. *Answered 2026-10-03: embed JetBrains Mono; every game carries its OFL notice.*
- **21.** Hierarchy: physics bodies on root entities only for 1.0 (proposed)? *Answered 2026-10-06 at the frame-loop gate: yes; the acceptance game places its orbiting blades each fixed step (`D-132`).*
- **23.** Closing order (§8.8). *Answered 2026-10-03: C1 runs before the freeze, so fixes that need API changes still land.*
- **24.** Kit stability: inside the 1.0 promise, or a tier of its own (§8.4)? *Answered 2026-10-03 at the definition gate: inside the promise, with `#[non_exhaustive]` on kit settings structs (`D-103`).*

## 11. Decisions this will likely need

IDs unassigned; each adds its `DECISION_INDEX.md` row in the same change.

- 1.0 definition and stability policy: semver scope, MSRV, platform tiers, the `wgpu`/`winit` policy. *Written at the definition gate as `D-102` (definition and scope) and `D-103` (semver scope, MSRV, `wgpu`/`winit`), with the API snapshot tool as `D-104` (implementation plan amendment 17); platform tiers wait for question 3 (W7).*
- Threading rule, replacing the `AGENTS.md` two-thread limit and citing `D-067`; the worker mechanism (`D-015` rule 3 if rayon).
- `Send`/`Sync` bounds on components and resources.
- Fixed-step accumulator and interpolation: amends `D-088` and `D-094`, touches the `D-018` seam.
- Pipelined render thread, if R4 comes back from 1.x (`D-102`).
- Interned asset IDs, keeping `D-009`'s ID model.
- A gamepad dependency (`D-015` rule 1).
- W6: kinematic bodies, sensors and shape queries (amends the body model of `D-033` and touches the CCD and sleep rules of `D-064` and `D-065`); audio voices, pause, fades and buses (the audio policy for `D-034`'s command ring that known issues asks for).
- The UI draft's own list: dependencies, text engine, glyph path, DPI, input routing.
- Licence notices in release archives (extends `D-071` and `D-072`).
- Game clock and timers: real and game time, time scale and pause; amends `D-088`'s single dt and settles which clock state transitions (`D-093`) follow.
- Per-user folder, log file and crash report: `env_logger` moves into the umbrella crate; a game identifier joins `tungsten.json` (extends `D-008`). The entry also says how the process-wide logger and panic hook sit with the no-global-state hard rule: they are `log`'s and std's single slots, set once by `App::new` (§8.2), not state the engine keeps. The same entry records the one engine logger that W1 M5's log console reads (implementation plan amendment 4).
- Release symbols: the strip leaves the release workflow, and debug files and PDBs are kept per tag outside the archives, with build-id and PDB matching and a CI symbolization probe (extends `D-071`).
- Settings and save slots: narrows DESIGN's "Save / load" non-commitment; binding persistence moves to the user folder (amends `D-045`); a user layer sits between `tungsten.json` and environment overrides, and an invalid user file falls back instead of failing (extends `D-008`).
- Standalone games: the engine owns the assets it uses (fonts; the sprite and lit shaders stay in `assets/shaders/` unless question 2 says yes, and the same entry settles the stock shaders' two copies, implementation plan amendment 14), no engine feature reads the shared root manifest, and the template's layout is the supported game layout. Amends the workspace-root wording of `D-008`, `D-013` and `D-045`: a game's `tungsten.json`, `input.json` and `assets/` sit in its own folder, which is the working directory (question 18).
- `tungsten-kit`: its dependency direction, admission rule and a row in `AGENTS.md`'s "Where code goes".
- Stages and plugins, with named systems and stage-local ordering constraints, in W3's frame-loop decision set; amends the frame order of `D-018` for user systems.
- Tuple queries and bundles; the arity-named queries deprecated, then removed at the freeze.
- Additive extracts, with tilemaps in the default extract.
- Component registry and prefabs: a `prefabs` manifest section; extends the scene format of `D-046`.
- Headless harness behind a `testing` feature of the umbrella.
- Project CLI and its dependencies (`D-015`).
- Per-row 1.0 budgets and C2's freeze-exception rule (extends `D-078`).

## 12. Risks

| Risk | Mitigation |
| --- | --- |
| The must list grows without a cut line | The tiers in §3; the acceptance game's mechanics rows tier W6 and W13 at the frame-loop gate; everything else is 1.x |
| Multi-core gains fall short, as in `D-067` | Serial wins first, a spike with a written speedup curve and the serial merge timed on its own; R4 moved to 1.x (`D-102`) |
| The API policy arrives after the API it governs | Decided with §1 at the definition gate (`D-103`); W1, W2 and W5 design against it |
| W1, W2 and W3 each redesign the frame alone | One frame-loop decision set at the frame-loop gate, before UI M3 and W15a (implementation plan amendment 7) |
| Breaks surface after the freeze | Collect them in the [break ledger](w04-api-freeze.md#break-ledger) from now on, with the API snapshot (`D-104`) showing new surface release by release; freeze last |
| Platforms cannot be certified without hosts | Support tiers instead of claims |
| Threaded captures are noisier | Extend the profiling workflow's capture rules before the R1 spike: worker count in `bench-config`, background load checked |
| User files leak into captures and tests | Smoke runs, benchmarks and the pixel test ignore user files (§8.2) |
| Crash files that cannot be symbolized | Debug files and PDBs kept per tag; module bases from `/proc/self/maps` on Linux; build-id and PDB GUID/age matched at build time; a CI probe on both runners symbolizes a deliberate panic (§8.2) |
| Engine and game fight over the global logger or panic hook | A fixed order: a logger installed before `App::new` wins, and the engine's hook chains whatever hook came first (§8.2) |
| A game update breaks old saves | A versioned envelope with game-owned migrations; a failed load is a typed error, not a panic |
| Five owner-added workstreams on top of eleven | The acceptance game still cuts; W6 grows only by what the kit's controllers and zones need; a kit item with no user by the freeze moves to 1.x |
| The template rots, or depends on the repository root without anyone noticing | A workspace member in `just check` and `just smoke`, plus the outside-copy check (§8.3) |
| The kit becomes a grab bag | The admission rule (§8.4): generic, used, documented, tested |
| Plugins hide the system order, or one plugin's order breaks another | Named systems with stage-local `before`/`after` constraints, resolved by a stable sort; snapshot tests of the resolved schedules; the systems overlay lists them; `02_bench` keeps explicit wiring (§8.6) |
| Tuple queries slow the hot path or need `unsafe` | A spike on `get_disjoint_mut`; `ecs` and `churn` not regressed |
| C2 finds a fix that needs an API change | C1 runs before the freeze (§8.8) |
| Standalone builds fill the disk | The outside-copy check shares the workspace's target folder; check free space before it runs |

## Revisions

- 2026-10-02 at 0.38.0 (`95e1947`): first draft.
- 2026-10-02: three systems the owner picked from a list of missing ones: a game clock with timers (W3, §6), and logs, crash reports, settings and save slots (W11, §8.2).
- 2026-10-03 at `afbc330`, after an external critique: R1's real prerequisites (§5.1, §5.3), the extract's determinism invariant (§5.5), the API policy moved to the first step (§7, §9), atomic writes on Windows and a tested symbol path for crash files (§8.2).
- 2026-10-03, with a developer-workflow review (§2.2) and the owner's answers to it: a game lives in its own repository on a git dependency; an in-repo template (W12), a `tungsten-kit` crate (W13), a headless test harness and a project CLI (W14), plugins with named stages, tuple queries and bundles (W15), prefabs with registered components (W16), and three closing phases: benchmark improvement, performance improvement and a final QA pass (W10, §8.8).
- 2026-10-03, after a second external critique: where the `Sync` bound of R1's slices sits (§5.4); the order of logger and panic-hook installation, module maps, symbol matching and a CI symbolization check for crash files (§8.2); stage-local ordering constraints for plugins (§8.6). The critique also said `rename` does not replace a file on Windows. The std documentation says it does, so that text stands, and a folder `fsync` on Unix was added.
- 2026-10-03: moved to this folder (`d15d74d`). Then, on `941b63e`, this list and the answered questions moved out of the header and §10's open list with their text unchanged (implementation plan amendment 12); §9 points to the implementation plan; §2.2's iteration row notes `D-096`.
- 2026-10-03, against the 0.40 tree (`9cd5709`; HEAD was `1d9bf8c`, a renderer-only commit): claims re-checked. §2.2 counts the player's inserts (13); §7 drops `validate_wgsl_source`, which no example has used since 0.40 QA step 5; §8.1's `libudev-dev` note follows 0.40 QA step 12; §8.4 cites `D-016` beside `D-007`; §11 names the decisions standalone games and the engine logger amend, points at amendment 14 and adds W6's.
- 2026-10-03 at `56f08ca`, the definition gate (implementation plan §11): questions 1, 4, 8, 9, 11, 16 and 24 moved to [Answered](#answered) with their answers (`D-102`, `D-103`); the open list keeps its numbers as bold labels. The accepted amendments (implementation plan §8) are folded in by later commits.
- 2026-10-03 on `db1177c`: W4, W9, W11, W12 and W14 graduated. §7, §8.2, §8.3, §8.5 and §8.1's W9 row moved verbatim into [w04](w04-api-freeze.md), [w11](w11-shipping-basics.md), [w12](w12-template.md), [w14](w14-tooling.md) and [w09](w09-distribution.md) respectively, with implementation plan amendments 4, 5, 6, 9, 11, 14, 16, 17 and 19 folded in there; each leaves a placement paragraph here. The other amendments are not yet folded.
- 2026-10-03 on `db1177c`: the remaining accepted amendments and the definition gate's answers folded in. The header and context digest cite the gate; §1 states the decided definition and the acceptance game (amendments 1, 8); §3's tiers cite `D-102`, with W5 narrowed to the gamepad (amendment 2) and W6 and W13 tiered at the frame-loop gate; §4 holds W1 BC and the couplings (amendments 4–7, 15); §5.3 and §6 move R4 to 1.x and the frame-loop decision set before UI M3 (amendment 7); §8.1's W5–W8 rows (amendments 2, 9, 10, 18); §8.4 and §8.7 split W16 (amendment 3) and settle the kit's stability (`D-103`); §8.8's C3 gains RC-A9 and the snapshot tool (amendments 16, 17); §9 links to the implementation plan (amendment 13); §11 and §12 follow.
- 2026-10-05, with M35 (W11a with W7a): question 14 moved to [Answered](#answered) with its plan's stated default (`D-119`).
- 2026-10-06 on `01f9c55`, uncommitted: W15 graduated. §8.6 moved verbatim into [w15](w15-authoring-api.md) with the frame-loop gate's decisions (`D-128`, `D-130`, `D-131`) and implementation plan amendments 7, 8, 16 and 18 folded in there; it leaves a placement paragraph here.
- 2026-10-06 on `01f9c55`, uncommitted: W2 graduated. §5 moved verbatim into [w02](w02-multi-core-rendering.md), its subsections as headings there, with R0's decisions (`D-113`, `D-114`), the gates' answers (`D-126`, `D-131`), the column-slice query (`D-130`) and implementation plan amendment 7 folded in there; it leaves a placement paragraph here, and §2.1 and §4 link the headings that hold what they cited.
- 2026-10-06 on `01f9c55`, uncommitted: W3 graduated. §6 moved verbatim into [w03](w03-frame-loop.md) with the frame-loop gate's decisions (`D-128`, `D-129`, `D-131`) and implementation plan amendments 7, 16 and 18 folded in there; it leaves a placement paragraph here.
