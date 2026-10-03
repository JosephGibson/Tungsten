# Tungsten 1.0 criteria — rough draft

- **status:** draft
- **goal:** Agree what 1.0 means for Tungsten, and which workstreams, gates and order reach it. Start from the two owner-confirmed workstreams (the UI and text suite, a multi-core rendering pass) and the items earlier decisions deferred to "the 1.0 criteria plan", which this file is. Make building a game in its own repository practical: a template, a component kit, tooling and an authoring API. Close with benchmark, performance and QA phases.
- **non-goals:** Implementation, dependency or decision entries, release work, dates, or version numbers before 1.0. Detail beyond scoping: each agreed workstream gets its own plan (the UI suite already has one).
- **files to touch:** This draft only.
- **ordered steps:** Settle the 1.0 definition and the `wgpu`/`winit` API policy (§1, §7); tier the candidate workstreams (§3), including the developer-workflow ones (§2.2, §8.3–§8.7); run the multi-core spike and agree the frame-loop design with the game-facing stages (§5, §6, §8.6) before the UI text engine fixes ownership; collect API breaks for the freeze (§7); fix the closing phases and the release-candidate checks (§8.8, §9).
- **done-when:** The owner has confirmed the 1.0 definition, a must/should/later tier and a done-when sketch for every workstream and closing phase, the order of the coupled workstreams (§9), and an answer or an owner for every question in §10.

Date: 2026-10-02, at 0.38.0 (`95e1947`). Revised the same day with three systems the owner picked from a list of missing ones: a game clock with timers (W3, §6), and logs, crash reports, settings and save slots (W11, §8.2). Ultra rough: every name, tier, number and order below is a proposal for discussion. Statements about the code were checked against the tree on that date; benchmark figures are the dated readings in [benchmarks.md](../../perf/benchmarks.md). Revised 2026-10-03 at `afbc330` after an external critique: R1's real prerequisites (§5.1, §5.3), the extract's determinism invariant (§5.5), the API policy moved to the first step (§7, §9), atomic writes on Windows and a tested symbol path for crash files (§8.2). Revised again on 2026-10-03 with a developer-workflow review (§2.2) and the owner's answers to it: a game lives in its own repository on a git dependency; an in-repo template (W12), a `tungsten-kit` crate (W13), a headless test harness and a project CLI (W14), plugins with named stages, tuple queries and bundles (W15), prefabs with registered components (W16), and three closing phases: benchmark improvement, performance improvement and a final QA pass (W10, §8.8). Revised a third time on 2026-10-03 after a second external critique: where the `Sync` bound of R1's slices sits (§5.4); the order of logger and panic-hook installation, module maps, symbol matching and a CI symbolization check for crash files (§8.2); stage-local ordering constraints for plugins (§8.6). The critique also said `rename` does not replace a file on Windows. The std documentation says it does, so that text stands, and a folder `fsync` on Unix was added.

## Context digest

- Tungsten is at 0.38.0 with Phases 1–4 complete (M0–M31): archetypal ECS, manifest assets with hot reload, wgpu sprite, lit, material and mesh-particle rendering with an 18-effect post stack, SMAA and bloom, glyphon text, cpal audio, Tiled maps, a hand-rolled physics solver, a state stack with transitions, telemetry and a six-benchmark suite.
- "1.0" has no written definition. `D-088` and `D-094` already park one item here: the fixed-step accumulator with render interpolation, which is also the precondition for the deep-pile load limit in [known issues](../../known-issues.md#recorded-limits).
- Owner-confirmed for 1.0: the [UI, interface and text suite](w01-ui-text-suite.md) and a multi-core rendering pass; added the same day, a game clock with timers, logs and crash reports, and settings and save slots. Added 2026-10-03: a template, a component kit, a headless harness and project CLI, plugins and stages, tuple queries and bundles, prefabs, and budget-gated closing phases.
- No game has been built outside this repository. Every example runs from the repository root on the shared `tungsten.json`, `input.json` and `assets/manifest.json`, and every document is written for engine maintainers (§2.2).
- The frame is serial on the main thread. The only background threads allowed are the `cpal` callback and the `notify` watcher (`AGENTS.md` hard rule, kept by `D-067` when it dropped a parallel solver).
- No API-stability policy exists. The library crates are not marked `publish = false` and have not been published. Release archives ship examples for Linux and Windows only (`D-071`).

## 1. What 1.0 means: decide first

| Definition | What it promises | Work it adds |
| --- | --- | --- |
| **A.** Complete for making games | A small, complete game builds in its own repository on the public API alone, with no engine patches | The UI, input, audio, physics and frame-loop gaps (§2.1); the developer-workflow gaps (§2.2): template, kit, tooling, authoring API, prefabs |
| **B.** Stable library | 1.x releases add without breaking; a documented public surface; a stated MSRV and platform tiers | API audit and freeze (§7), rustdoc, a policy for `wgpu`/`winit` types in the API |
| **C.** Public release | crates.io packages, a user guide, licence notices in archives | B plus distribution (W9) |

Recommendation: A and B, with C's crates.io publication as its own question (§10). Make A testable with an **acceptance game**, started from the template (W12) in its own repository on a git dependency (owner, 2026-10-03), built only on the public API and `tungsten-kit`, with a title menu, settings (volume, rebinding, display), pause, settings persistence, gamepad play and a release archive from `tungsten package` (W14). Every gap it hits is a 1.0 candidate; anything it does not need is 1.x.

## 2. Gaps that bear on 1.0

### 2.1 Engine features

| Area | Gap | Source |
| --- | --- | --- |
| UI and text | No widget, layout or focus system. Text alignment, wrap modes, ellipsis and measurement exist in cosmic-text but are unreachable. No DPI model | UI draft §2 |
| Input | Modifiers, focus, IME and produced text are dropped. Unlisted keys become `KeyCode::Other(<winit discriminant>)`, which is not stable across winit versions. Keys held across a focus loss never release. No gamepad support anywhere in the tree | UI draft §2 |
| Frame loop and time | Variable dt split across fixed substeps, capped at 0.1 s per frame and 1/30 s per physics step. No accumulator, no interpolation. `DeltaTime` holds only `dt`: no game clock apart from real time, no time scale, pause, elapsed time, frame index or timer type | `D-088`, `D-094`, [DESIGN](../../../DESIGN.md#physics--m11), [`time.rs`](../../../crates/tungsten-core/src/time.rs) |
| Threads | Serial frame; `World` is neither `Send` nor `Sync` | §5.1 |
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
| ECS ergonomics | Queries are named by arity and filter (`query2_opt2_mut`, `query3_mut_without`). The common pattern collects entities, then calls `get` or `get_mut` per entity and clones around the borrow (`animation_system`). No spawn builder or bundle: the platformer's player takes 11 `insert` calls | `core/ecs/world.rs`, `examples/01_platformer/src/{systems,setup}.rs` |
| Silent mistakes | A sprite whose `asset_id` is not registered draws nothing and logs nothing; an unknown font logs a warning and falls back to sans-serif | `sprite_extract.rs` pass 1, `render/text.rs` `make_attrs` |
| Data-driven entities | A scene entry holds transform, sprite, visibility, tag and tweens: no physics, light, emitter or game component. Levels are code; the platformer's `level_layout.rs` is 2,336 generated lines | `core/assets/scene.rs`, `examples/01_platformer/tools/` |
| Testing a game | Tests copy the frame loop by hand (systems, then a manual event flush) and skip particles, tweens, the command flush and event rotation. No headless `App` steps frames | `examples/01_platformer/src/tests/main.rs` |
| Iteration | Not a pain point: a touched example rebuilds in 1.1 s and a touched core file in 2.5 s (dev profile, dependencies at `opt-level = 2`). Asset hot reload works, but only the platformer turns it on; config does not reload (DESIGN non-commitment) | Local timing, 2026-10-03; known-issues follow-ups |

## 3. Candidate workstreams and proposed tiers

| ID | Workstream | Source | Proposed tier |
| --- | --- | --- | --- |
| W1 | UI, interface and text suite | Owner | Must |
| W2 | Multi-core rendering pass | Owner | Must |
| W3 | Frame loop v2: fixed step, interpolation, game clock and timers, one stage map | Deferred here by `D-088`, `D-094`; the game clock from the owner | Must |
| W4 | API stabilization and freeze | Definition B | Must if B |
| W5 | Input: gamepad, focus, modifiers, DPI | §2; overlaps UI M3 | Should; must if the acceptance game uses a gamepad |
| W6 | Gameplay gaps: physics queries, sensors, kinematic bodies, layers; audio voices and buses | §2 | Kinematic bodies, sensors and shape queries must (W13's controllers and trigger zones need them); the rest should, cut by the acceptance game |
| W7 | Platform certification and support tiers | Known issues | Must (tiers at least) |
| W8 | Correctness burn-down | Known issues, engine findings | Must |
| W9 | Distribution and documentation | Known issues, definition C | Licence notices and a getting-started guide on the template must; the rest follows §1 |
| W10 | Closing phases: benchmark improvement (C1), performance improvement (C2), final QA pass (C3) | Benchmarks; owner, 2026-10-03 | Must; C2 is budget-gated (§8.8) |
| W11 | Shipping basics: logs, crash reports, settings and save slots | Owner | Must; save slots follow the acceptance game |
| W12 | Template project: `templates/basic`, the layout every game and example follows; the engine made self-contained for an outside repository | Owner, 2026-10-03; §2.2 | Must |
| W13 | `tungsten-kit`: generic components and systems in four groups (basics, transform hierarchy, character controllers, gameplay helpers) | Owner, 2026-10-03; §2.2 | Must per item, if a user ships it by the freeze (§8.4) |
| W14 | Tooling: headless test harness, project CLI (`new`, `check`, `package`) | Owner, 2026-10-03; §2.2 | Harness must; CLI `check` and `package` must, `new` should |
| W15 | Authoring API: plugins and named stages, additive extracts, tuple queries and bundles | Owner, 2026-10-03; §2.2 | Must, before the freeze |
| W16 | Prefabs: registered components in scenes, prefab files and Tiled objects | Owner, 2026-10-03; §2.2 | Must |

## 4. W1: UI, interface and text suite

The [UI draft](w01-ui-text-suite.md) is the source of truth; this section only places it.

- **Proposed 1.0 cut:** ladder steps M0–M6, from the text engine split to example migration. A settings screen needs the "broader controls" stage (sliders, checkboxes, scroll containers), so either that stage joins 1.0 or settings leave the acceptance game. Editable fields, IME, accessibility and docking stay after 1.0 unless §10 says otherwise. Controller navigation comes almost free if W5 lands a gamepad: UI navigation reads `ui_*` actions, and a gamepad adds bindings without widget changes (UI draft §7).
- **Coupling with W2.** The UI draft assumes a serial frame: layout takes the text engine mutably after systems, and draw takes it shared (UI draft §4). A pipelined render thread (§5.3, R4) draws frame N while layout runs frame N+1, so under the UI draft's glyph path T1, where glyphon draws from the engine's retained `Buffer`s, both would need the engine at once. T2's plain `PositionedGlyph` snapshot suits a render thread. The R4 answer is therefore an input to the T1/T2 gate, and should exist before M0 settles where the text engine lives.
- **Coupling with W3.** Input routing (UI draft §7) adds frame stages; design it with W3's stage map.
- **Coupling with W8.** M2's direct-versus-capture check needs the capture-completion contract (P3) first.

## 5. W2: multi-core rendering pass

### 5.1 Facts

- Frame order in [`app.rs`](../../../crates/tungsten/src/app.rs): systems → particles → tweens → command flush → event rotation → hot reload → extract → render → recycle → audio → telemetry, all on the main thread. `stage_extract` runs the quad, sprite, text, debug, light and mesh-particle extracts in sequence.
- `Renderer::render_frame_internal` ([`renderer.rs`](../../../crates/tungsten-render/src/renderer.rs)) acquires the surface texture first, then uploads the camera, prepares text, records every pass into one `frame_encoder`, submits once and presents. A GPU-bound frame blocks in the acquire before it records anything.
- `World` cannot cross a thread. `AnyColumn: Any` has no `Send` or `Sync` bound, resources are `HashMap<TypeId, Box<dyn Any>>`, components need only `T: 'static`, and the default extract keeps its buffers in `ExtractScratch(RefCell<…>)`. Tests store `Rc` components.
- The extract cannot be split as written. Queries return flattening iterators (`query3_opt2` chains archetypes through `flat_map`), and nothing hands out a column slice. The columns themselves are plain `Vec<T>`, and `tungsten-core` holds no `Rc`, `RefCell` or `Cell` outside tests, so slices of the five types the sprite extract reads (`Transform`, `Sprite`, `Visibility`, `UniformOverrideBlock`, `ParallaxLayer`) and `&AssetRegistry` are already `Sync`; no bound reaches user components. Pass 1 ([`sprite_extract.rs`](../../../crates/tungsten/src/sprite_extract.rs)) carries state from row to row: the class table and its `class_of` numbering, the `last_class` shortcut, the painter-order flag, the varying-bit masks, record indices in the sort keys and a `z_norm` preset by position.
- wgpu 30.0.1 asserts `Send + Sync` for `Device`, `Queue`, `CommandEncoder`, `CommandBuffer`, `RenderBundle` and `Surface`. `RenderBundleEncoder` is thread-local by design, so a worker creates its own. Recording on workers and a render thread are both possible on wgpu's side. Whether glyphon 0.12 and cosmic-text 0.19 types are `Send` is not checked.
- The engine crates contain no `unsafe`. rayon is in `Cargo.lock` only through criterion, a dev-dependency.
- `D-067` is the precedent. A deterministic colour-parallel physics solver on `std::thread::scope` gained 2.5% (134.6 → 131.2 ms) because the solve was about 2% of the frame, cost the serial path about 18%, and was reverted. Its determinism test must stay green under any parallelism.

### 5.2 Where frame time goes

| Row | Limited by | Main-thread costs (p50) | What multi-core can buy |
| --- | --- | --- | --- |
| `gpu` | GPU: `render_span` 10.39 ms; the CPU waits 7.53 of 10.72 ms in the acquire | `extract` 0.65, `render_encode` 2.12 ms | No frame rate; the GPU is the limit |
| `gpu-throughput` (~400,000 sprites) | `extract`: 13.28 of 14.68 ms `total` | `render_encode` 1.19 ms | Most of the extract, if it scales. The string ID compare is its largest cost, and a serial fix comes first |
| `integrated` | CPU: 8.37 ms of work against 6.30 ms of GPU time | `update` 5.62 (`physics_step` 5.06), `extract` 0.96, `render` 1.37 ms (stage means) | At most about 2.3 ms by taking extract and render off the critical path; the frame then waits on the GPU near 6.3 ms, about 25% faster. Physics stays serial (`D-067`) |
| `particles` | `extract`, the largest stage of 6.12 ms `total` | | Parallel extract |

`physics`, `physics-sparse`, `ecs` and `churn` measure simulation; this pass does not touch them. The pass pays where the extract dominates and where a CPU-bound frame has GPU slack. It cannot help a GPU-bound frame, so its done-when is written against these rows, not against "faster rendering".

### 5.3 Options

| ID | Option | Pays in | Cost and risk | Needs |
| --- | --- | --- | --- | --- |
| R0 | Serial first: interned sprite IDs, extract culling | The extracts of `gpu-throughput`, `particles` and `integrated` | Interned IDs change `Sprite.asset_id: String`, a public-API break that belongs before the freeze (§7) | Nothing new |
| R1 | Fork-join default and tilemap extracts: archetype column slices split across workers, per-worker instance and key buffers concatenated in row order, then today's sort and grouping | Extract-bound rows | A thread spawn per worker per call under `std::thread::scope`. A serial merge after the join: class renumbering, record-index offsets, the painter-order check across slice boundaries, the bit masks and the `z_norm` fix-up (§5.1). Custom extracts (`Fn(&World)`, as in example 01) stay serial unless they get the same slice API | The threading rule (§5.4); a core query that yields per-archetype column slices, new public surface (§7) with no `Sync` bound of its own, `T: Sync` applying only where a worker receives a slice (§5.4); per-worker `ExtractBuffers` in place of the one `ExtractScratch` cell. Not a `Sync` `World` |
| R2 | Two command buffers: record scene and post passes before the acquire, then acquire and record the swapchain stage and text ("late acquire"). Later, one `CommandEncoder` per worker submitted in order, or render bundles for static content such as tile layers | `render_encode` (1–2 ms); a shorter swapchain hold | A small win at about 60 batches; wgpu's internal locking may cap scaling (spike). Capture frames keep their blit (`D-087`) | Nothing new for the split; the threading rule for workers |
| R3 | Text preparation off the main thread | Text-heavy frames | glyphon's `prepare` takes the atlas mutably, so this waits for the UI draft's T1/T2 gate | W1 gate |
| R4 | Pipelined render thread: the main thread runs frame N+1's systems and extract while a render thread encodes, submits and presents frame N | CPU-bound rows; hides the acquire wait | One more frame of latency. Every renderer call the main thread makes today (texture and font uploads, hot reload, resize, post-AA switch, capture arm) becomes a command or a sync point. Two `FrameExtract`s in flight. `total` stops being the frame's work, so telemetry and benchmark semantics change. Frame cap and present-mode pacing (`D-043`) need re-checking | `Renderer: Send` (glyphon unverified). Extract output is already plain data (`D-018`) |
| R5 | Parallel asset decode at startup and reload | Load time | No load-time metric yet (benchmark proposal T2). Streaming stays a non-commitment | Startup metrics |

Recommendation: R0, then a spike of R1, then R2's split; decide R4 at a gate with a prototype and a latency measurement; R3 after the UI gate; R5 only if startup metrics ask for it. Bevy's `PipelinedRenderingPlugin` (extract on the main thread, render on its own thread) is the nearest design to study for R4; it was not reviewed for this draft.

### 5.4 Decisions the pass needs first

1. **Threading rule.** Replace the two-thread limit with, for example: fork-join workers only inside a stage, joined before the stage returns; worker count from `tungsten.json` with an environment override, where `0` is the serial reference path and stays tested; results bit-identical at every count; one render thread only if R4 lands. The audio callback and watcher rules stay.
2. **Worker mechanism.** (a) `std::thread::scope` per stage: safe, no dependency, `D-067`'s precedent, one OS thread spawn per worker per call. (b) A rayon `ThreadPool` owned by `App`: `D-015` rule 3, safe scopes over borrowed data, persistent threads. Not rayon's global pool, which sits badly with the no-global-state rule. (c) An own persistent pool: borrowed jobs need lifetime erasure, so it brings the engine's first `unsafe`. Spike with (a); choose (b) or (c) only if the measured spawn cost matters.
3. **`Send`/`Sync` policy for components and resources.** R1 and R4 need no bound on component types in general: the `World` stays on the main thread and only slices cross threads. Handing `&[T]` to a worker needs `T: Sync`, so that bound sits on the parallel extract's own queried types at its call site: the five engine types, already `Sync` (§5.1). The column-slice query itself carries no `Sync` bound, so making it public imposes nothing on user components. An extract over a user type that is not `Sync` still compiles, and runs serially; custom extracts stay serial in 1.0 (R1). A parallel system scheduler would need the bounds on every component, and adding them after 1.0 breaks every user type that holds an `Rc` or a `RefCell`. Decide before the freeze, even if no scheduler ships.

### 5.5 Gates and done-when sketch

- **Spike:** extract speedup at 0, 1, 2, 4 and 6 workers on `gpu-throughput` and `particles`, per-frame spawn overhead included and the serial merge timed on its own, on the reference machine.
- **Pixels:** `just visual` byte-equal at 0 workers and at the default count; direct and capture paths unchanged.
- **Determinism:** `tests/physics_determinism.rs` untouched; benchmark digests unchanged. Invariant: workers only fill row-ordered pass-1 output; the merge, sort, grouping and batch build stay on one thread, and no worker iterates a hash map. The order key is already total (`painter_order(z_order, entity id)`: integers, distinct), so the merged sort input equals the serial one. A headless test checks the batches byte-equal to the 0-worker path at 1, 2, 3, 4 and 6 workers, covering uneven splits, more workers than rows, empty archetypes, both sort paths (either side of the 1,024-key radix threshold), the in-painter-order fast path and a batch class that spans a slice boundary.
- **Performance:** `gpu-throughput` `extract` and, with R4, `integrated` `total` read `improved`; `gpu` not regressed after an A/A of the untouched tree; peak RSS bounded.
- **Latency (R4 only):** input-to-present measured and accepted in the decision entry.
- **Telemetry:** per-thread stage times; a `workload_version` or telemetry-format bump wherever a metric changes meaning.

## 6. W3: frame loop v2

- `D-088` and `D-094` park it here. A fixed-step accumulator with render interpolation changes the core/render seam, because the extract needs previous transforms (`D-088`).
- **Proposed shape:** physics, and opt-in gameplay systems, at a fixed step; a bound on steps per frame that replaces or wraps `max_step_dt`; the extract interpolates `Transform` between the last two steps. Smoke runs and benchmarks pin 1/60 s, so at a 60 Hz step they see one step per frame and should keep their digests.
- **What it unlocks:** the deep-pile load limit's lever (`contact_hertz`) needs a constant substep ([known issues](../../known-issues.md#recorded-limits)); frame-rate-independent gameplay; replays.
- **Game clock (owner-added).** Proposed: a `Time` resource in place of the single `dt`, holding real time (capped as `D-088` caps today's dt), game time (scaled by `time_scale`, zero while `paused`), elapsed time on both clocks, a frame index and, with the fixed step, the step size, the steps run this frame and the interpolation fraction. `DeltaTime` feeds tweens, particles, game feel, the camera, state transitions and the physics step today, and each must pick a clock: game time for simulation and animation, real time for UI, transitions, the HUD and hit-stop, so a pause menu animates over a frozen world. Every engine stage must treat a zero game step as a no-op. Smoke runs and benchmarks pin dt at scale 1 and should keep their digests. `DeltaTime.dt` stays as the game dt until the freeze (§7). A paused clock freezes motion, not input: the routed gameplay view (UI M3) keeps gameplay systems from acting on input under a pause menu.
- **Timers.** A plain `Timer` value (duration, elapsed, once or repeating), owned by a component or resource and ticked with whichever clock its owner picks. `tick(dt)` returns how many times it finished, so a long frame cannot swallow a repeat. No timer system or event queue in the first version.
- **Design together:** W1's input-routing stages, W2's render hand-off point (R4), W15's game-facing stages and plugins (§8.6) and telemetry semantics all edit the same stage list. Agree one frame-loop decision set before UI M0 and before R4.
- **Open:** physics only, or a fixed-update schedule for systems? Interpolation on by default, or opt-in per entity? What the HUD and telemetry call a "frame" once update and render rates differ. Which clock camera shake and particles follow during hit-stop? Does a paused game skip the physics stage or step it by zero?

## 7. W4: API stabilization and freeze

- **Policy to write:** what semver covers, the MSRV rule (`D-069` pins 1.98.1), deprecation in 1.x.
- **`wgpu` and `winit` in the API: decided in the first step, with §1, not at the freeze.** W1's draw and text API, R2 and R4's renderer changes and W5's input types are new public surface, and this policy decides their shape. The whole-crate re-exports and render's `wgpu`-typed signatures tie Tungsten's major version to wgpu's, which ships breaking majors several times a year (30.0.1 is locked). Options: hide them (`pub(crate)` pipelines, engine-owned format and input types), declare an advanced tier outside the promise, or accept a Tungsten major per wgpu major.
- **What hiding costs.** No example imports `wgpu` or `winit`. Examples reach render only through `Renderer` (texture and material registration in example 02), `TextSection`, `SpriteBatch`, `SpriteInstance`, `compare_png` and `validate_wgsl_source`, so most of the work is a curated umbrella re-export in place of `pub use tungsten_render as render`. The snag is `Renderer`: the umbrella and render are separate crates, so every method the umbrella calls is `pub` and reaches games with the type (`surface_format` returns a `wgpu::TextureFormat`). Hiding needs a game-facing handle or a `doc(hidden)` tier for those methods.
- **winit 0.31** is still a prerelease (known issues). If it ships before UI M0b, upgrade first so the input bridge is rewritten once.
- **Breaks to land before the freeze (candidates):** interned asset IDs (R0); real `KeyCode` variants (UI M3); a presented/skipped/failed render result and `App::run` returning runtime errors (P3); `#[non_exhaustive]` where public enums and config structs may grow; the `Send`/`Sync` policy (§5.4); a bundle-insert API ([engine findings](../../perf/benchmarks.md#engine-findings)); `DeltaTime` folded into the game clock (§6); winit 0.31; single-activation UI event reads (UI draft §7); the arity-named queries removed in favour of tuple queries (W15). New surface the freeze also covers: the column-slice query (R1), which builds on W15's query machinery; stages, plugins, the engine's system names, bundles and additive extracts (W15); the prefab format and component registry (W16); the headless harness (W14); the kit, unless it takes its own tier (§8.4).
- **Documentation:** `missing_docs` on public items, rustdoc examples, a getting-started guide that walks through the template (W12).
- The freeze is the last step before release candidates; every break above collects here first.

## 8. Other workstreams

### 8.1 W5–W10 in brief

| ID | Scope sketch | Open points |
| --- | --- | --- |
| W5 Input | A gamepad backend through the action map; focus loss releases held keys; modifiers; the scale-factor resource (UI M0) | A gamepad crate is a `D-015` rule 1 dependency: check its Linux backend for threads and `libudev` (both workflows install `libudev-dev`, yet no locked crate links it) |
| W6 Gameplay | Physics: kinematic bodies (G1), sensors (G2), layers and masks, ray and shape queries. Audio: voice handles, pause, fades, buses. Particles: the burst latch fix | The acceptance game cuts this; whatever it does not use moves to 1.x |
| W7 Platforms | Support tiers in the README: Linux is the only tested host today. A CPU-only Windows test job on the `windows-2025` image the release build already uses would cover core, persistence and the crash hook without a GPU, informational under `D-070`. Windows and macOS rendering and audio need hosts or an explicit lower tier; decide whether releases build macOS | `D-003` (native only) stands |
| W8 Correctness | Known-issues P3s, capture completion first since it gates UI M2; the engine-finding bugs; the shader ID allocator; queued transitions | One pass, like the P2 correctness pass |
| W9 Distribution | Third-party licence notices in archives, also for fonts the engine embeds (W12); the crates.io decision and crate names; user documentation, starting with a getting-started guide on the template (W12) | Follows §1 |
| W10 Closing phases | Benchmark improvement, performance improvement and a final QA pass; W10's earlier list moved into C1 (§8.8) | Which rows get absolute budgets and which relative ones |

### 8.2 W11: shipping basics

Owner-added on 2026-10-02. Logs, settings and saves all need a writable per-user folder, so that comes first. Assets keep resolving from the working directory, which the release launcher sets to the archive root; resolving them relative to the executable is not part of this workstream.

- **Per-user folder.** On Linux, settings under `$XDG_CONFIG_HOME`, saves under `$XDG_DATA_HOME` and logs under `$XDG_STATE_HOME`, each with its specification default when unset. On Windows, `%APPDATA%` for settings and saves and `%LOCALAPPDATA%` for logs. On macOS, `~/Library/Application Support` and `~/Library/Logs`. The folder name comes from a new stable game identifier in `tungsten.json`: `WindowConfig` holds only title, size and vsync, and a title can change or be translated. Resolve the paths in a pure function over an injected environment, so Linux unit tests cover all three platforms; a crate for this would be a `D-015` rule 1 dependency. An environment override points the folder elsewhere for development and tests.
- **Log file.** env_logger 0.11 writes to any `Write` through `Target::Pipe`, so it moves from the four examples into the umbrella crate and no crate is added. One file per run in the user folder, the last few runs kept; stderr stays in debug builds; `RUST_LOG` still filters. `log` accepts one global logger per process, so the order is fixed: `App::new` installs the engine's logger through `env_logger`'s `try_init`, and a `SetLoggerError` means a logger is already set, which the engine takes as "the game owns logging" and leaves alone. A game with its own logger installs it before `App::new`; one installed later fails with `SetLoggerError`. An opt-out skips the engine's logger entirely (question 14).
- **Crash report.** A panic hook writes the message, location, thread and `std::backtrace::Backtrace::force_capture()` to a crash file, then calls the previous hook. It runs before the release profile's `panic = "abort"` ends the process. The hook takes whatever hook is set when `App::new` runs (`take_hook`) and calls it after writing the crash file. A game hook set later replaces the engine's unless it chains through `take_hook` too, as the getting-started guide will show. The logger and the hook are process-wide by nature, like `log` itself; the app installs them once. Limit: only panics leave a crash file. A segfault or abort in a driver or below wgpu leaves none, because a signal handler cannot safely format a backtrace; that stays 1.x.
- **Symbols.** The release profile sets `debug = 1` and `strip = "none"`, but `release.yml` builds the archives with `CARGO_PROFILE_RELEASE_STRIP=debuginfo`. The linker drops the debug info, so nothing is left to symbolize with, and std's default backtrace prints names without addresses. The strip exists for size: `example-04` is 85.2 MB at `debug = 1` and 15.6 MB without debug info (local build, 2026-10-03). Proposed path, checked with a std-only probe on 1.98.1: build without the strip; on Linux, `objcopy --only-keep-debug` writes the debug file and `objcopy --strip-debug --add-gnu-debuglink` ships a binary of stripped size. The crash file records the backtrace in alternate form (`{:#}` prints each frame's runtime address), the game and engine versions and the target triple. On Linux it also records the mapped modules and their base addresses from `/proc/self/maps`, which the hook reads with std. That replaces the one-known-function load offset of the first probe, which covered only the executable: frames in libc, the Vulkan driver or the windowing libraries need their own module bases. `addr2line` on the tag's debug file, at each frame's address minus its module base, then resolves file and line. The probe's frames resolved to their source lines with no crate added. Versions and target pick the debug file. `package` and `release.yml` check that the shipped binary and its debug file carry the same GNU build-id (`readelf -n`; link with `-Wl,--build-id` if the toolchain does not add one), so a mismatch fails at build time, not when a crash arrives. On Windows, the strip should make rustc pass `/DEBUG:NONE` to the MSVC linker, which writes no PDB, so a crash file would carry no names (unchecked: look for `.pdb` files in a Windows build). The msvc build drops the strip and keeps its PDBs out of the archive, and the build checks that the binary's CodeView record (PDB GUID and age, `llvm-readobj --coff-debug-directory`) matches the kept PDB. With no module map from std on Windows, the crash file keeps the known-function offset for the executable; frames in system DLLs stay unresolved. `llvm-symbolizer`'s native PDB reader should resolve the rest; unchecked, so the CI check below settles it. Debug files and PDBs go to a per-tag release asset or workflow artifact, never the player archive.
- **Symbolization check in CI.** Next to the archive build in `release.yml`, on the Linux and `windows-2025` runners, a small probe built with the release profile panics on purpose. The job splits or keeps symbols the way packaging does and symbolizes the crash file, failing unless the probe's frames resolve to file and line. It needs no GPU; whether it blocks a release follows `D-070`.
- **No console on Windows.** `cfg_attr(not(debug_assertions), windows_subsystem = "windows")` on each example and on the launcher, which waits on the example through `Command::status` and would otherwise hold a console of its own. Without a console, the log file is the only log.
- **Settings.** Typed settings as JSON in the user folder, written with the `atomic_write` that `ActionMap::persist_to` already uses, moved to a shared helper: a temp file in the target's folder, `sync_all`, `rename`, then on Unix an `fsync` of the folder. `std::fs::rename` replaces an existing file on Windows as on Unix: the std documentation names `MoveFileExW` or `SetFileInformationByHandle` and says it replaces the original file (checked on 2026-10-03 against a critique that said otherwise). The guarantee is old or new, never torn; the folder `fsync` also makes a completed write survive a power loss on Unix. Power-loss durability on Windows is not claimed. On Windows a rename fails while another process (antivirus, indexer, sync client) holds the target open, so a write retries briefly, then returns a typed error. The engine persists its own: display settings, master volume (buses after W6) and input bindings, whose persistence moves from the working directory's `input.json` to the user folder (amends `D-045`). Layering: `tungsten.json` as shipped defaults, then user settings, then environment overrides, which stay last.
- **Invalid user files.** `D-008` and `D-045` make an invalid `tungsten.json` or `input.json` fatal. A file the game wrote must not stop it from starting: keep the bad file beside it, log a warning and use defaults. The difference needs its decision entry.
- **Save slots.** The engine owns the folder, atomic writes, a versioned envelope (game schema version, engine version, timestamp, slot name) and slot listing. The game owns its serde types and its migrations between schema versions. A failed load returns a typed error (`D-022`). Serializing the `World` stays out: it needs component reflection.
- **Captures stay clean.** Smoke runs, benchmarks and the pixel test never read user files, or read a pinned empty folder; otherwise a developer's saved resolution or vsync changes a capture.
- **Done-when sketch:** unit tests for folder resolution on each platform, atomic writes (a failure injected before the rename leaves the old file), a corrupt file falling back with a backup, and layering with the environment last; the CI symbolization check passes on both runners; a deliberate panic in a binary from the release workflow leaves a crash file whose frames resolve to file and line with that tag's debug file on Linux (owner check of the shipped archive), and with its PDB on Windows (owner check, as hosts allow); a Windows release opens no console (owner check, no host); smoke and benchmark digests unchanged.

### 8.3 W12: template project

Owner-added on 2026-10-03. A game lives in its own repository on a git dependency, `templates/basic` shows how, and examples 01, 03 and 04 move onto its layout. `02_bench` keeps its harness so its digests stay comparable.

- **Self-containment first.** What an outside repository lacks today (§2.2): (1) the engine's own assets. The HUD's `mono` font, JetBrains Mono Regular (115 KB), moves into the umbrella crate as embedded bytes for all engine text: HUD, inspector and overlays. Every game then ships its OFL notice (W9; owner, 2026-10-03). `sprite.wgsl` and `lit_sprite.wgsl` move inside `tungsten-render`, and `assets/shaders/` keeps copies for hot reload, as the stock post shaders already do (`D-091`). (2) No engine feature reads the shared root manifest. (3) Logging and the crash hook move into the umbrella (W11), so `main.rs` stops calling `env_logger::init()`. (4) W11's game identifier names the user folder.
- **Layout.**

  ```text
  templates/basic/
    Cargo.toml            tungsten and tungsten-kit: a path dependency in the workspace, a git tag once copied
    rust-toolchain.toml   the engine's pinned toolchain (D-069)
    tungsten.json         game identifier, window, render
    input.json            named actions; game code reads actions, never keys
    assets/manifest.json  the game's own fonts, sprites, sounds and prefabs; no shared root
    src/main.rs           about ten lines: App::new, DefaultPlugins, GamePlugin, run
    src/game.rs           GamePlugin: resources, events, systems by stage
    src/states.rs         title → gameplay → pause on the state stack (D-046)
    src/components.rs     game components, registered for prefabs (W16)
    tests/                headless harness tests (W14)
    README.md             run, test, hot reload, package
  ```

- **Practices it shows.** Stages and plugins in place of a hand-ordered list (W15); kit components before game code (W13); registry IDs and prefabs in place of paths and spawn code (`D-046`, W16); settings and one save slot (W11); actions in place of keys; `anyhow` in `main`, `thiserror` in modules and `log` for output (`AGENTS.md` conventions); asset hot reload in debug builds; a headless test per system.
- **It grows with the engine.** Every workstream that changes how a game is written lands its template change in the same step, so the template stays the current practice. It stays basic: a feature that would take it past a few hundred lines belongs in the acceptance game.
- **Kept honest.** A workspace member, built and tested by `just check` and smoke-run by `just smoke`. A second check copies it to a folder outside the workspace, points its dependency at this checkout, and builds, tests and smoke-runs it from there, so any hidden reliance on the repository root fails. That check shares the workspace's target folder and profiles. On 2026-10-03, a cold standalone build with a target folder of its own wrote 973 MB before it filled a disk that was already close to full.
- **Examples.** 01, 03 and 04 adopt the layout, plugins and kit, and drop their own copies of kit features. Each runs from its own folder with its own `tungsten.json`, `input.json` and `assets/`, as a copied template does (owner, 2026-10-03). The smoke script, the release launcher and the archive layout change to match. The shared root `assets/` keeps only what `02_bench` and the engine's hot-reload copies need.
- **Done-when sketch:** the template builds, tests and smoke-runs in the workspace; the outside copy does the same from a folder with no `tungsten.json`, `input.json` or `assets/` above it; its `main.rs` names no engine system; examples 01, 03 and 04 on the layout with their smoke matrices green and the transition and post-stack regressions byte-equal; `02_bench` digests unchanged.

### 8.4 W13: `tungsten-kit`

Owner-added on 2026-10-03, with all four groups in scope. A crate of generic components and systems that 2D games reuse; the template and the examples use it in place of their own copies.

- **Admission rule.** An item joins the kit if it is generic and has a user: the template, a migrated example or the acceptance game. Each item has rustdoc with an example and a harness test. An item with no user by the freeze moves to 1.x, which keeps four groups from becoming an open-ended must list.
- **Dependency direction.** Decided by the owner on 2026-10-03: the schedule, its stages and the `Plugin` trait live in `tungsten-core` over a `Schedule` type, not `App`. The kit depends on core, and the umbrella re-exports it behind a default feature, so a game has one dependency. Rejected: a kit that depends on the umbrella (two dependencies for games) and a feature-gated umbrella module (no crate boundary, no tier of its own). This fits `AGENTS.md` ("ECS → core"), but it only works if every plugin need goes through `Schedule` with no render types in core (`D-007`): systems by stage, event registration, inspector rows, component registration (W16) and extract contributions. A kit item that needs an extract hook can't hand over a `SpriteBatch`, which is a render type. It writes core components the default extract already reads, as the animated sprite does, or it waits for W15's extract-contribution design. The W15 spike checks this before the decision entry.
- **Basics.** An animated sprite that writes `Sprite.asset_id` from `AnimationState`, so the default extract animates (replaces example 01's `CurrentSprite` and `animation_system`); lifetime and despawn-after timers on W3's `Timer`; despawn outside a region or the view; a spawner (interval, count, W16 prefab ID); `cursor_to_world` on the camera; a sound played on an event.
- **Transform hierarchy.** `Parent`, `Children`, a local transform and a propagation stage before the extract, so the extract still reads one world `Transform` and the `D-018` seam and R1's slices stay as they are. Recursive despawn through the command buffer (`D-039`). Proposed for 1.0: physics bodies only on root entities, and the physics-to-`Transform` sync becomes an engine stage, not a game system. Interpolation (W3) applies to roots before propagation.
- **Character controllers.** A platformer controller (ground check, coyote time, jump buffer, variable jump height) and a top-down mover. Both need W6's kinematic bodies (G1), sensors (G2) and shape queries, which moves those from should to must. Example 01's player moves onto the platformer controller, and its ground detection (§2.2) goes.
- **Gameplay helpers.** Health and damage with invulnerability time and a hit flash on the `damage_flash` shader, which the kit takes over from the shared manifest; trigger zones on sensors (G2); a path follower for moving platforms on kinematic bodies (G1); a small per-entity state machine (enter, exit, update by state), separate from the game-level `StateStack`.
- **Stability.** The kit's API joins the freeze (§7) or ships as its own tier on its own semver track; decide with §1's policy.
- **Done-when sketch:** every item used by the template or a migrated example, with rustdoc and a harness test; example 01's copies of kit features deleted; `just physics-release` green, since the controllers move kinematic bodies through the physics step.

### 8.5 W14: tooling

Owner-picked on 2026-10-03: a headless test harness and a project CLI. JSON schemas for the config files, and live editing in the game (inspector edits, a tweak panel or a console on W1's widgets), wait for 1.x.

- **Headless harness.** An `App` with no window, surface or audio device that runs the real stage order: `step(n)` at a pinned dt, injected actions and cursor positions, events and resources readable between steps, and the CPU extracts runnable, so a test can check what a frame would draw. It lives in the umbrella behind a `testing` feature. Example tests that copy the frame loop move onto it; the physics determinism, containment and tunneling tests stay at the core level.
- **Project CLI**, a `tungsten` binary in `tools/`:
  - `new <name>` copies `templates/basic`, sets the game identifier and window title, and pins the engine as a git dependency on the current tag.
  - `check` loads `tungsten.json`, `input.json`, every manifest, scene and prefab with the engine's own loaders, checks that each referenced ID exists, and reports every error, not just the first. It cannot see string IDs in game code; R0's interned IDs and a once-per-ID warning for an unknown sprite (W8) cover that side.
  - `package` makes a release build and an archive for a standalone game: binary, assets and licence notices (W9), with the debug file kept apart (W11). It reuses the steps `release.yml` runs for the examples.
- **Dependencies.** The CLI is a tool, not a runtime dependency, but `D-015` still applies. Argument parsing (by hand or a crate) and archive writing (a crate, or the platform's `tar` and `zip`) each need their argument in a decision entry.
- **Done-when sketch:** the platformer's tests run through the harness with no hand-written loop; the output of `tungsten new` passes `tungsten check`, then builds, tests and packages outside the workspace; `check` reports every error in a seeded set of broken files; the packaged archive runs on the Linux reference machine.

### 8.6 W15: authoring API

Owner-picked on 2026-10-03: plugins with named stages, tuple queries and bundles. All of it is public API and lands before the freeze (§7).

- **Stages and plugins.** Games add systems to named stages. Proposed: `startup`; `pre_update` (input, engine toggles, the state dispatcher); `fixed_update` (physics and opt-in gameplay, W3); `update`; `post_update` (physics sync, hierarchy, animation, game feel, camera); then the engine's extract, render, audio and telemetry. Engine features become plugins (`PhysicsPlugin`, `CameraPlugin`, `GameFeelPlugin`, `ParticlesPlugin`, `TweensPlugin`, `DebugPlugin` and so on) collected in `DefaultPlugins`; a game leaves one out to replace it. The `Schedule` and `Plugin` live in `tungsten-core` (§8.4), and `App` drives the schedule. This is W3's stage map: one design, one decision set.
- **Order within a stage.** Registration order alone does not compose: engine defaults, the kit and the game each add systems, and the platformer's list already depends on pairs such as `shake_tick_system` before `camera_update_system` and the squash trigger after ground detection. Every system has a name, and engine names are public constants that the freeze covers. A system may declare `before` and `after` constraints on names in its own stage. A stable topological sort resolves them, and ties keep registration order, so the schedule stays deterministic. An unknown name or a cycle fails at startup and names both systems. The resolved schedule of `DefaultPlugins`, the template and each example is snapshot-tested, so an engine reorder shows up as a test diff, and the systems overlay lists it. The kit declares its constraints against engine names, and the guide asks game plugins that touch engine features to do the same.
- **Benchmarks keep their order.** `02_bench` wires engine systems itself. It starts from an empty plugin set, so no system runs twice and the digests stay.
- **Additive extracts.** By default the engine draws sprites and tilemaps. A game appends sprite and text contributions, and replaces the default only explicitly. Example 01's extract shrinks to what is its own (rainbow balls, outlined text, glows).
- **Tuple queries.** `world.query::<(&A, &mut B, Option<&C>)>()` with `With<T>` and `Without<T>` filters replaces the arity-named functions, which stay deprecated until the freeze removes them. `slice::get_disjoint_mut` (stable since Rust 1.86) borrows two columns of one archetype mutably without `unsafe`, so the engine's first `unsafe` (§5.1) should not be needed; a spike confirms it. R1's per-archetype column slices build on the same machinery.
- **Bundles.** `world.spawn_with((A, B, C))` and a command-buffer form, with a `Bundle` trait for tuples and game types. One archetype move per spawn instead of one per component, which is also the bundle-insert API in the [engine findings](../../perf/benchmarks.md#engine-findings).
- **Resource access.** Beside today's `Option` accessor, a panicking one that names the missing type, for the resources `App` always inserts.
- **Done-when sketch:** the template and examples 01, 03 and 04 list no engine system; snapshot tests of the resolved schedules for `DefaultPlugins`, the template and each example; startup failure tests for an unknown name and a cycle; `ecs` and `churn` not regressed, since queries and spawns are their hot paths; benchmark digests unchanged; every arity-named query removed before the freeze.

### 8.7 W16: prefabs

Owner-picked on 2026-10-03: data-driven entities with game components, without `World` serialization.

- **Component registry.** `app.register_component::<Health>("health")` for any serde type; engine and kit components come registered. Scene entries and prefab files carry `"components": { "health": { … }, "rigid_body": { … } }` beside today's fields.
- **Prefab assets.** A `prefabs` manifest section, which adds a row to the `AGENTS.md` assets table: prefab ID to file. `world.spawn_prefab(id, position)` and a command-buffer form. A spawner (W13), or a Tiled object whose class names a prefab, spawns the same way, and the object's properties override fields. Nested prefabs, and patching live entities on hot reload, wait for 1.x: a reload changes later spawns only.
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
- **Known issues:** no open P1 or P2; each P3 has a 1.x home or an accepted note.
- **API:** the public surface diffed against the freeze snapshot shows no change. A tool for this, such as `cargo-semver-checks`, is a `D-015` question.
- **Done-when sketch:** a dated QA record with each item passed or accepted by the owner; that release candidate tagged 1.0.

## 9. Order (sketch)

```text
§1 definition, acceptance game and the wgpu/winit API policy (§7)
 ├─ W8 capture contract and small bug fixes           (unblocks UI M2's check)
 ├─ W11 user folder, log file, crash report           (logs for every later test)
 ├─ W12 self-containment, template skeleton           (outside-copy check from the start)
 ├─ W14 headless harness                              (later tests use it)
 ├─ W2 R0 serial wins, R1 spike                     ┐
 ├─ W3 frame-loop design (fixed step, clock, R4?)   ├─ one frame-loop decision set,
 ├─ W15 stages and plugins                          │  an input to the UI T1/T2 gate
 │                                                  ┘
 ├─ W15 tuple queries and bundles                     (with R1's column slices)
 ├─ W1 M0 text engine split → M1 → gate → M2 … M6
 ├─ W2 R2 split, R4 if approved; W3 implementation
 ├─ W6 G1, G2, shape queries → W13 kit → W16 prefabs → W14 CLI
 ├─ W5, rest of W6, cut by the acceptance game; W11 settings and save slots
 ├─ examples 01, 03, 04 onto the template; acceptance game from the template
 ├─ C1 benchmark improvement                          (fixes that need API changes land here)
 └─ W4 freeze → C2 performance → release candidate → C3 final QA (W7 tiers, W9 distribution) → 1.0
```

Possible phase labels: Phase 5 foundations (first W8 items, W11 logs and crash reports, W12 self-containment and skeleton, W14 harness, W2 R0–R2, W3 with the game clock and W15's stages, W15 queries and bundles, W1 M0–M3); Phase 6 features (W1 M4–M6, W5, W6, W11 settings and save slots, W13, W16, W14 CLI, the example migration, the acceptance game); Phase 7 benchmark improvement and freeze (C1, W4); Phase 8 performance (C2); Phase 9 release candidates and final QA (C3, W7, W9). Milestone plans would continue at M32 under the [plan naming rule](../README.md#naming).

## 10. Questions for the owner

Answered on 2026-10-03: a game lives in its own repository on a git dependency, so crates.io is not needed for 1.0 (whether to publish anyway stays question 2); the template is the in-repo `templates/basic`; the kit is a new crate with all four groups; 1.0 tooling is the headless harness and the project CLI; plugins with named stages; tuple queries and bundles; prefabs with registered components; examples 01, 03 and 04 move to the template, the bench does not; the acceptance game starts from the template (part of question 1; its genre is still open); performance is budget-gated; the final QA is an owner playthrough with Linux certified and the other platforms on stated tiers (part of question 3; a macOS release build is still open). The kit's controllers and trigger zones make G1, G2 and shape queries must (part of question 10).

1. Which definition of 1.0 (§1)? Is an acceptance game the test, and what game is it?
2. Publish the library crates on crates.io for 1.0?
3. Platform tiers: certify Linux only, or find Windows and macOS hosts? Should releases build macOS?
4. Multi-core scope: fork-join only (R1, R2), or also a pipelined render thread (R4) with its extra frame of latency?
5. Worker mechanism: `std::thread::scope`, a rayon pool, or an own pool?
6. Require `Send + Sync` on components and resources before the freeze?
7. Fixed step: physics only, or a fixed-update schedule? Interpolation by default?
8. UI cut line: broader controls (sliders, checkboxes, scroll) in 1.0? A single-line text field with IME?
9. Gamepad in 1.0?
10. Minimum audio and physics features for 1.0 (W6)?
11. `wgpu` and `winit` types in the stable API: hide them, tier them, or accept their majors? Needed in the first step, before W1, W2 and W5 add surface (§7).
12. Which performance budgets gate 1.0?
13. Game clock: which engine stages keep running on real time while gameplay is paused (§6)?
14. Logs and crash reports: on by default in `App`, or one opt-in call?
15. Which settings does the engine persist itself, and does a corrupt user file reset to defaults (proposed) or stop the game as an invalid `tungsten.json` does?
16. Save slots in 1.0, or settings only?
17. Kit dependency direction (§8.4). *Answered 2026-10-03: `Schedule` and `Plugin` in core; the umbrella re-exports the kit.*
18. Where do examples run from (§8.3)? *Answered 2026-10-03: their own folders, like a copied template.*
19. Engine-owned text. *Answered 2026-10-03: embed JetBrains Mono; every game carries its OFL notice.*
20. CLI: `check` and `package` must and `new` should (proposed)? Argument parsing and archive writing by hand or by crate (§8.5)?
21. Hierarchy: physics bodies on root entities only for 1.0 (proposed)?
22. Controllers: anything beyond ground check, coyote time, jump buffer and variable jump, such as slopes, one-way platforms or ladders?
23. Closing order (§8.8). *Answered 2026-10-03: C1 runs before the freeze, so fixes that need API changes still land.*
24. Kit stability: inside the 1.0 promise, or a tier of its own (§8.4)?

## 11. Decisions this will likely need

IDs unassigned; each adds its `DECISION_INDEX.md` row in the same change.

- 1.0 definition and stability policy: semver scope, MSRV, platform tiers, the `wgpu`/`winit` policy.
- Threading rule, replacing the `AGENTS.md` two-thread limit and citing `D-067`; the worker mechanism (`D-015` rule 3 if rayon).
- `Send`/`Sync` bounds on components and resources.
- Fixed-step accumulator and interpolation: amends `D-088` and `D-094`, touches the `D-018` seam.
- Pipelined render thread, if R4.
- Interned asset IDs, keeping `D-009`'s ID model.
- A gamepad dependency (`D-015` rule 1).
- The UI draft's own list: dependencies, text engine, glyph path, DPI, input routing.
- Licence notices in release archives (extends `D-071` and `D-072`).
- Game clock and timers: real and game time, time scale and pause; amends `D-088`'s single dt and settles which clock state transitions (`D-093`) follow.
- Per-user folder, log file and crash report: `env_logger` moves into the umbrella crate; a game identifier joins `tungsten.json` (extends `D-008`).
- Release symbols: the strip leaves the release workflow, and debug files and PDBs are kept per tag outside the archives, with build-id and PDB matching and a CI symbolization probe (extends `D-071`).
- Settings and save slots: narrows DESIGN's "Save / load" non-commitment; binding persistence moves to the user folder (amends `D-045`); a user layer sits between `tungsten.json` and environment overrides, and an invalid user file falls back instead of failing (extends `D-008`).
- Standalone games: the engine owns the assets it uses (fonts, the sprite and lit shaders inside their crates), no engine feature reads the shared root manifest, and the template's layout is the supported game layout.
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
| The must list grows without a cut line | The tiers in §3; the acceptance game decides W5 and W6; everything else is 1.x |
| Multi-core gains fall short, as in `D-067` | Serial wins first, a spike with a written speedup curve and the serial merge timed on its own, and a gate before R4 |
| The API policy arrives after the API it governs | Decide the `wgpu`/`winit` policy with §1; W1, W2 and W5 design against it |
| W1, W2 and W3 each redesign the frame alone | One frame-loop decision set before UI M0 and R4 |
| Breaks surface after the freeze | Collect them in §7 from now on; freeze last |
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
