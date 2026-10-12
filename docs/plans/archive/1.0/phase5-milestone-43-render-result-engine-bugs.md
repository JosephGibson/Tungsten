# Phase 5 milestone 43: render results, capture completion and engine bugs (W8a) — draft

- **status:** done
- **goal:** Land the presented, skipped or failed render result with explicit capture completion and `App::run` returning the runtime render failure, the break ledger's planned row that W1 M2's direct-versus-capture checks wait on; and fix the engine findings W8a carries: burst and pulse semantics (the burst latch and the 0.40 follow-up), tilemaps at `z_norm` 0 under `gpu_depth`, the per-frame lit-plus-material warning and the silent unknown sprite.
- **non-goals:** W8b (the shader ID allocator, queued transitions, `render.max_frame_latency = 0`, the other P3s); device-loss recovery, and wgpu's fatal paths: a validation error no error scope catches, and device loss inside a blocking poll, still panic into the crash hook (`D-119`, A21); hot-reload, audio and asset errors, which stay logged; the tile and light extracts' surface size (known issues); tile layers ordered among sprites by z (1.x backlog); renormalizing depth across extract sources (`D-138` clause 5 stands); a tileset entry with no registered sprite (W14b `check`); W2 R2's acquire split; benchmark sources beyond `rearm_bursts`.
- **files to touch:** step 1: `crates/tungsten-render/src/{renderer.rs,screenshot.rs,lib.rs,tests/screenshot.rs}`, `crates/tungsten/src/{app.rs,capture.rs (new),lib.rs,tests/capture.rs (new),tests/app.rs}`, `examples/02_bench/tests/visual_regression.rs`, `api/tungsten-render.txt`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`. Step 2: `crates/tungsten-core/src/{components.rs,assets/particle.rs,tests/assets/particle.rs}`, `crates/tungsten/src/particles.rs`, `crates/tungsten/tests/particles.rs`, `examples/02_bench/src/particles.rs`, `api/tungsten-core.txt`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/perf/benchmarks.md`, `docs/known-issues.md`. Step 3: `crates/tungsten/src/{tilemap_extract.rs,extract.rs,tests/tilemap_extract.rs,tests/extract.rs}`, `examples/02_bench/tests/visual_regression.rs`, `examples/01_platformer/src/{extract.rs,tests/presentation.rs}`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/getting-started.md`, `docs/perf/benchmarks.md`, `docs/known-issues.md`. Step 4: `crates/tungsten/src/{sprite_extract.rs,tests/sprite_extract.rs}`, `docs/perf/benchmarks.md`. Step 5: none in the tree (evidence under `perf-runs/`). Step 6: the release files (§3).
- **ordered steps:** (1) Render result and capture completion (decision 142). (2) Burst and pulse semantics (decision 143). (3) Tiles at the far plane (decision 144). (4) Once-per-ID extract warnings. (5) Capture sitting and gates. (6) Release 0.61.
- **done-when:** every step's evidence row quoted; workflow §5 closed (§8); the release checks, plus the gates by change no step ran, pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W8a ([implementation plan](../../1.0/implementation-plan.md) §3 card, level B), milestone M43, release 0.61. Written 2026-10-12 at `bb71160`, on branch `0.61`, which already holds 0.61's uncut workflow changes (`D-140`, `D-141`). Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. W8 has not graduated ([w08](../../1.0/w08-correctness.md)), so [criteria](../../1.0/criteria.md) §8.1 and §2.1 are the scoping source.

## Context digest

- W8a is the register's first open stop (queue item 3, implementation plan §6). Its card puts capture completion first, because W1 M2 compares the direct and capture paths and needs a capture it can trust ([w01](../../1.0/w01-ui-text-suite.md) §2), then the [engine findings](../../../perf/benchmarks.md#engine-findings) and the 0.40 follow-up on bursts and pulses (implementation plan §3).
- It stands on `D-087` (capture frames keep the blit tail), `D-047` (offscreen readback and the `TUNGSTEN_CAPTURE_*` variables), `D-022` (runtime conditions return `Result`), `D-138` (the default channel's tiles at the far plane; custom extracts left to W8a), `D-061` (lit wins over a material), `D-113` and `D-114` (interned IDs, culling) and `D-133` (the `particles` row owns the particle systems).
- The surface-acquire policy (skip on a timeout, an occluded window or a second outdated surface; fail on a second loss or a validation error) came with 0.27.0 and has no decision; decision 142 is its first record.
- Next: W1 M0b with W2 R2. R2 splits `render_frame_internal` around the acquire, so the outcome and the one-frame capture must hold when a command buffer is submitted before the acquire; W1 M2 then builds its overlay checks on this contract.
- Breaks for the [ledger](../../1.0/w04-api-freeze.md#break-ledger): the render result (four `render_frame*` returns, two `capture_frame*` returns) and `App::run`'s errors; `ParticleEmitterState`'s new field; two behaviour changes (`once: false` bursts, far-plane tiles).

## 1. Audit

Read at `bb71160`. Paths start at `crates/` unless another root is named.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | A skipped acquire returns `Ok(())`: the policy skips on `Timeout`, `Occluded` and a second `Outdated`, and `acquire_texture` gives `Ok(None)` | `tungsten-render/src/renderer.rs:1089`, `1101`, `1238`; `surface_acquire.rs:62-64` |
| A2 | An armed capture survives a skip: it is taken after the acquire, so the next presented frame writes it, whichever frame that is | `renderer.rs:1274` |
| A3 | Readback, encode and IO failures only warn; `finalize_capture` returns `ScreenshotError` | `renderer.rs:1599-1610`, `1852-1907` |
| A4 | `capture_frame` and `capture_frame_direct` return `Result<(), ScreenshotError>` and never fail; `render_frame_full_timed` sizes its timestamps from `pending_capture` before the acquire | `tungsten-render/src/screenshot.rs:40-59`; `renderer.rs:1204` |
| A5 | `stage_render` counts every `Ok` as rendered and logs "captured frame N -> path" without knowing whether a file was written | `tungsten/src/app.rs:1101-1112`, `1159-1171` |
| A6 | A render error is logged every frame and the loop goes on; `App::run` returns only setup failures (window, renderer, manifests). The acquire fails as `RenderError::Surface` after a second `Lost`, on `Validation` or when recreation fails | `app.rs:1158`, `429-443`, `1743`, `1810`, `1825`; `renderer.rs:1093-1098`; `surface_acquire.rs:66-70` |
| A7 | `run_frame` ends `Completed` or `ExitRequested`; `window_event` and the harness compare against `ExitRequested` only | `app.rs:761-767`, `1910-1918`; `tungsten/src/testing.rs:116` |
| A8 | Only `app.rs` calls `render_frame_full*` and `capture_frame*`; every binary returns `app.run()` from `main`; smoke judges exit codes; the visual tests assert exit success and the PNG's existence | `rg` over the workspace; `templates/basic/src/main.rs:21`; `scripts/smoke-examples.sh:59-90`; `examples/02_bench/tests/visual_regression.rs:46-52` |
| A9 | The burst latch is `continuous_accum >= 1.0`, set on firing and never reset; `once: false` only skips `drained`, so `ParticleSystemDrained` never comes. `Pulse { total_pulses: Some(0) }` fires one pulse, then drains | `tungsten/src/particles.rs:324-333`, `341-358` |
| A10 | `validate` checks `count`, `count_per_pulse` and `interval_sec`, not `total_pulses`; `ParticleConfigRegistry::register` does not validate, so code-built configs reach the emitter unchecked | `tungsten-core/src/assets/particle.rs:346-367`, `498-511` |
| A11 | Only the bench's `particles` row uses `once: false`, re-arming by writing `continuous_accum = 0.0` each period; all 13 shipped burst manifests say `"once": true`; `integrated` seeds `continuous_accum` as a torch's continuous phase | `examples/02_bench/src/particles.rs:180`, `478-489`; `examples/01_platformer/assets/particles/*.json`; `examples/02_bench/src/integrated/scene.rs:236` |
| A12 | `extract_tilemaps` and `extract_tilemap_layers` write `z_norm` 0; the default channel rewrites them to 1.0 | `tungsten/src/tilemap_extract.rs:81-82`, `218`; `tungsten/src/extract.rs:142-150` |
| A13 | Under `gpu_depth` the scene pass adds a `LessEqual` test with depth writes and keeps batch order, and the default sprite extract's `z_norm` falls with painter order to 0: depths that never rise in draw order draw what `cpu_stable` draws, and tiles at 0 drawn before default sprites hide them. This holds for the stock and lit shaders, which write `z_norm` as clip depth (A20) | `tungsten-render/src/sprite.rs:14-17`, `1056`; `renderer.rs:818`; `passes/order.rs:135`; `tungsten/src/sprite_extract.rs:5-7` |
| A14 | Custom extracts that call the tile extracts: `gpu` (tiles, then default sprites; the smoke matrix's `gpu_depth` rows), `integrated` (default parallax, tiles, the other default sprites), example 01 (layers between its own instances, every one at `z_norm` 0) | `examples/02_bench/src/gpu.rs:446-450`; `scripts/smoke-examples.sh:154-163`; `examples/02_bench/src/integrated.rs:407-421`; `examples/01_platformer/src/extract.rs:329-334`, `469`, `119`, `367`, `456`, `504`, `617` |
| A15 | A lit sprite carrying a material logs a warning per sprite per frame; no benchmark row has one | `sprite_extract.rs:334-340`; `examples/02_bench/src/gpu/scene.rs:166-181` |
| A16 | A sprite ID with no asset (an interned name never loaded) draws nothing and logs nothing; the text engine and the mesh-particle pipeline log such misses once | `sprite_extract.rs:324-326`; `tungsten-core/src/assets/registry.rs:60-70`, `96-98`; `tungsten-render/src/text/engine.rs:112-114`; `tungsten-render/src/mesh_particle.rs:261-262` |
| A17 | `ExtractScratch` keeps the extract buffers across frames in a `RefCell`; a world without it, or a nested extract, gets fresh buffers | `sprite_extract.rs:175-200` |
| A18 | The known-issues P3 row cites `D-029` for surface recovery; `D-029` is the audio mixer, and the acquire policy came with 0.27.0 | `docs/known-issues.md` (`stage_render` row); `DECISIONS.md:138`; `CHANGELOG.md:592` |
| A19 | `crates/tungsten-render/AGENTS.md` has 30 B of its 4,096 B left; `docs/LLM_INDEX.md` has 1,340 B of 12,288 B | `scripts/check-agent-context.py:22-26`; `wc -c` |
| A20 | Material shaders send `z_norm` through `view_proj`, where the stock and lit shaders write it as clip depth, so material sprites order differently under `gpu_depth`; the `visual` preset keeps `shader_share` 0.1, drawn with `bench_heavy` (found by critique round 1) | `assets/shaders/damage_flash.wgsl:55`; `examples/01_platformer/assets/shaders/soft_glow.wgsl:61`; `examples/02_bench/assets/shaders/bench_heavy.wgsl:62`; `assets/shaders/sprite.wgsl:57-58`; `assets/shaders/lit_sprite.wgsl:79-80`; `examples/02_bench/src/gpu.rs:69` |
| A21 | No error scope or `on_uncaptured_error` handler covers the acquire, and wgpu's default handler panics, so `CurrentSurfaceTexture::Validation` arrives only through a handler a game installs; `Device::poll` panics on any error but a timeout, so device loss during the capture readback or the GPU-timing readback panics (found by critique round 2) | `renderer.rs:1074`, `1867`; `tungsten-render/src/timing.rs:158`; `shader_hot_reload.rs:27-30`; in wgpu 30.0.1, `src/api/surface_texture.rs:74-79`, `src/backend/wgpu_core.rs:344-350`, `692-694`, `1916-1926`; in wgpu-core 30.0.1, `src/device/life.rs:148-156` |

## 2. Design

### Render result (decision 142)

```rust
// tungsten_render::renderer, re-exported at the crate root
#[must_use]
#[non_exhaustive] // the enum only: the umbrella's tests build its variants
#[derive(Debug)]
pub enum RenderOutcome {
    /// Drawn, submitted and presented. `capture` is the armed capture's
    /// result: the path written, or the readback, encode or IO error.
    Presented { capture: Option<Result<PathBuf, ScreenshotError>> },
    /// No surface texture (timeout, occluded, or still outdated after one
    /// reconfigure): nothing presented, and an armed capture is dropped
    /// unwritten. Work recorded before the acquire may have run.
    Skipped,
}
// render_frame, render_frame_with_quads, render_frame_full and
// render_frame_full_timed return Result<RenderOutcome, RenderError>;
// capture_frame and capture_frame_direct return () (Q4).
```

- `Err(RenderError)` is the failed frame the acquire returns: a surface lost again after recreation, which device loss produces, or a failed recreation. No device recovery; `surface_acquire.rs` keeps the policy and its tests. wgpu's fatal paths stay outside the contract (A21): a validation error with no error scope, and device loss inside a blocking poll, panic into the crash hook (`D-119`).
- A capture is armed for the next `render_frame*` call only; arming again before it replaces it. The frame takes it before the acquire through a GPU-free helper in `screenshot.rs`, `capture_for_frame`, which hands it on only to a presented frame, as `surface_acquire.rs` keeps the acquire policy. The renderer logs nothing about captures.
- `crates/tungsten/src/capture.rs` (new, `pub(crate)`) takes `CaptureConfig` and `parse_capture_config` from `app.rs`. `TUNGSTEN_CAPTURE_FRAME=N` names the N-th frame that reaches render, presented or skipped (Q2), today's index when nothing skips, and arms that frame only. `Presented` with `Ok` logs `captured frame N -> path` at info. `Presented` with `Err`, a skipped frame N (Q1) or a run that ends before frame N records a capture error, logged once at error level.
- `stage_render` records `Err(RenderError)` as the fatal error, with the frame number as context, logged once; `run_frame` returns a new `FrameEnd::RenderFailed` and `window_event` exits. A capture error lets the run go on (Q3). `App::run` returns the fatal error, else the capture error; its rustdoc says so.
- A skipped frame still runs every other stage and writes its perf lines, as now. When R2 submits a buffer before the acquire, a skip still returns `Skipped` and drops the capture, whose readback stays after the acquire.

### Bursts and pulses (decision 143)

- `Burst { count, once }` fires `count` on the emitter's first tick. `once: true`: then drained, with `ParticleSystemDrained` once its particles are gone, as now. `once: false`: re-armable; it fires again on the first tick after each `ParticleEmitterState::rearm_burst()` and never drains. `rearm_burst` does nothing on a drained emitter.
- The latch moves to `pub burst_fired: bool` on `ParticleEmitterState` (Q6); `continuous_accum` is the continuous accumulator only.
- `Pulse { total_pulses: Some(0) }` emits nothing and drains on its first tick; `ParticleConfig::validate` rejects it, as it rejects `count: 0` (Q7).
- The bench's `rearm_bursts` calls `rearm_burst()` on the same schedule, so the `particles` row's emission, workload version and digest stay.

### Tiles at the far plane (decision 144, amends `D-138`)

- `extract_tilemaps` and `extract_tilemap_layers` write `z_norm` 1.0, the far plane (Q8), and the default channel's rewrite loop goes, with the same bytes out. Under `cpu_stable` `z_norm` is a pass-through, so nothing draws differently there. Rustdoc: under `gpu_depth` tiles draw under everything drawn after them at a nearer depth; a caller that draws a layer over its own instances writes the layer's `z_norm`.
- Example 01 writes its tile layers at `z_norm` 0, its painter convention (Q9), so its `gpu_depth` frame stays. `integrated` still needs `cpu_stable`, since its tiles sit between default sprites; its binary's row note keeps the old reason, as bench sources stay untouched (as under `D-085`), and benchmarks.md gives the new one.

### Once-per-ID warnings (no decision; `D-061` stands)

- `extract_sprites_default` logs a sprite ID with no asset once per ID, naming the sprite and that it draws nothing, and a lit sprite carrying a material once per sprite ID (Q10). The flags are a bit vector in `ExtractBuffers`, indexed by `SpriteAssetId::index`, so the once holds wherever `ExtractScratch` is (every `App`); a bare world warns per call.
- The row loop keeps its two existing branch tests; the flag and log work sits in a `#[cold]`, out-of-line helper, which step 4's probe gate checks.

## 3. Steps

Shared: the baseline is `target/m43-base`, a `git archive HEAD` copy made in step 1 and built with its own `CARGO_TARGET_DIR` under `target/`: debug for `just visual`, release with the runner's flags for steps 4 and 5. Evidence goes in `perf-runs/<date>-m43-render-result/`. Captures that two runs write to the same temp path are copied aside between the runs.

### Step 1: Render result and capture completion (decision 142)

- **Files:** `crates/tungsten-render/src/renderer.rs`, `screenshot.rs`, `lib.rs` and `tests/screenshot.rs`; `crates/tungsten/src/app.rs`, `capture.rs` (new), `lib.rs`, `tests/capture.rs` (new) and `tests/app.rs`; `examples/02_bench/tests/visual_regression.rs`; `api/tungsten-render.txt`; `DECISIONS.md`; `docs/DECISION_INDEX.md`; `docs/plans/1.0/w04-api-freeze.md`; `DESIGN.md`; `docs/LLM_INDEX.md`; `docs/known-issues.md`
- **Change:** (1) Decision 142 and its index row, with `D-047`'s marker (choice 3). (2) §2's renderer side: `RenderOutcome`; `capture_for_frame`, with tests in `tungsten-render/src/tests/screenshot.rs` (an armed capture meets a skipped and a failed acquire and is gone, unwritten, and the next presented frame gets none); the readback result returned; the warning gone. (3) `capture.rs` and the app wiring: `FrameEnd::RenderFailed`, the fatal error, `App::run`. (4) GPU-free tests: `tests/capture.rs` takes each transition (written, a write error, a skipped target, a target never reached, nothing armed after the target); `tests/app.rs` feeds a synthetic `Err` (the frame ends `RenderFailed`, the error recorded) and a capture error (the error the run returns). The helper's one call before the acquire's early return is read, not run: nothing forces a skip on a real surface. (5) Two visual tests: `capture_past_the_last_frame_fails_the_run` (frame 9 of 8 smoke frames) and `unwritable_capture_path_fails_the_run` (a path under a regular file), each asserting a non-zero exit, no PNG and a log line naming the frame or the error. (6) `just api`, the ledger row, rustdoc, DESIGN's frame-loop paragraph, `tungsten/capture.rs` in the LLM index's screenshots row, the known-issues P3 row removed (the plan session filed A21's row).
- **Done-when:** `cargo test -p tungsten-render --locked` → pass, the new `screenshot::tests` cases among them; `cargo test -p tungsten --locked --lib` → pass, the new `capture::tests` and `app::tests` cases among them; `rg -n 'Render error: |screenshot capture failed' crates/` → no match; `just visual` → pass, with the two new tests; each capture that this tree's `just visual` and the baseline's both write is `cmp`-equal to its pair; `just smoke` → pass; `just api` → `api/tungsten-render.txt` adds `RenderOutcome` and changes the six methods' returns at each path the snapshot lists, removes nothing, and `api/tungsten.txt` and `api/tungsten-core.txt` do not change; `just api-check` → pass; the ledger's render-result row reads Landed (decision 142) in 0.61 (M43); `cargo test -p tungsten-core --test decision_index --locked` and `just ctx` → pass.
- **Moves:** none.

### Step 2: Burst and pulse semantics (decision 143)

- **Files:** `crates/tungsten-core/src/components.rs`, `assets/particle.rs` and `tests/assets/particle.rs`; `crates/tungsten/src/particles.rs`; `crates/tungsten/tests/particles.rs`; `examples/02_bench/src/particles.rs`; `api/tungsten-core.txt`; `DECISIONS.md`; `docs/DECISION_INDEX.md`; `docs/plans/1.0/w04-api-freeze.md`; `docs/perf/benchmarks.md`; `docs/known-issues.md`
- **Change:** Decision 143 first. Then the tests that need no new API, run on the old code: `pulse_with_zero_total_emits_nothing_and_drains` and the `validate` case. Then §2's field, method and `plan_emission`, with `burst_not_once_fires_again_after_rearm` (three bursts over three re-arms, never drained, no `ParticleSystemDrained`) and `burst_once_ignores_rearm`, which call the new method; `rearm_bursts` on the method, with its module comment; `EmissionKind`'s rustdoc; ledger rows for the field (it breaks a struct literal without `..`, of which the tree has none) and for `once: false`; benchmarks.md's burst finding marked fixed and its `particles` text; the known-issues follow-up removed.
- **Done-when:** on the old code the zero-pulse test and the `validate` case fail (output quoted); after the change `cargo test -p tungsten --test particles --locked` → pass; `cargo test -p tungsten-core --locked particle` → pass; `rg -n 'continuous_accum' crates/tungsten/src/particles.rs examples/02_bench/src/particles.rs` → only `plan_emission`'s `Continuous` arm; `cargo test -p example-02-bench --locked` → pass; `just api` → `api/tungsten-core.txt` adds `burst_fired` and `rearm_burst` and changes nothing else. The `particles` digest is step 5's check.
- **Moves:** none.

### Step 3: Tiles at the far plane (decision 144)

- **Files:** `crates/tungsten/src/tilemap_extract.rs`, `extract.rs`, `tests/tilemap_extract.rs` and `tests/extract.rs`; `examples/02_bench/tests/visual_regression.rs`; `examples/01_platformer/src/extract.rs` and `tests/presentation.rs`; `DECISIONS.md`; `docs/DECISION_INDEX.md`; `docs/plans/1.0/w04-api-freeze.md`; `DESIGN.md`; `docs/getting-started.md`; `docs/perf/benchmarks.md`; `docs/known-issues.md`
- **Change:** First the control: the `gpu` row's `visual` preset captured with `TUNGSTEN_BENCH_SET=tile_layers=0,shader_share=0` under `TUNGSTEN_RENDER_DEPTH_SORT=gpu_depth` and under `cpu_stable`; `shader_share=0` keeps A20's material depth out of both. Then `gpu_depth_draws_what_cpu_stable_draws` (tiles on, `shader_share=0`, tolerance 0), run on the old code. Then decision 144 with `D-138`'s marker; §2's change; the reference extract in `tests/tilemap_extract.rs` at 1.0 and a test that both tile extracts write 1.0; example 01's tile layers at 0, with an assertion in its presentation test (Q9); the ledger row (a behaviour change); DESIGN's tilemap paragraph; the guide's sentence on the tile extracts; benchmarks.md's tile finding marked fixed, with the `gpu` and `integrated` notes; a known-issues follow-up for the `integrated` binary's row note, which only this step makes stale.
- **Done-when:** the control pair is `cmp`-equal; the new test fails on the old code with `pixels_above_tolerance` > 0 (quoted) and passes after; `cargo test -p tungsten --locked tilemap_extract` and `cargo test -p tungsten --locked extract::tests` → pass; `cargo test -p example-01-platformer --locked presentation` → pass; `just visual` → pass, its fixture capture `cmp`-equal to the baseline's; `rg -n 'Both write .z_norm. 0' DESIGN.md` → no match.
- **Moves:** none.

### Step 4: Once-per-ID extract warnings

- **Files:** `crates/tungsten/src/sprite_extract.rs` and `tests/sprite_extract.rs`; `docs/perf/benchmarks.md`
- **Change:** §2's flags and cold helper. Tests: an unknown ID over three extracts sets its flag on the first and draws what the reference extract draws; a lit sprite with a material likewise. The probe gate. benchmarks.md's lit-plus-material finding marked fixed.
- **Done-when:** `cargo test -p tungsten --locked sprite_extract` → pass, the byte-equality tests unchanged; `rg -n 'log::warn' crates/tungsten/src/sprite_extract.rs` → only in the cold helper; probe gate: `perf annotate` of the default extract's row loop from a 360-frame `perf record` of `gpu-throughput` on this build and on the baseline's (release, the runner's flags) → the same instruction count in the loop and no added stack loads or stores, both listings in the evidence folder.
- **Moves:** none.

### Step 5: Capture sitting and gates

- **Files:** none in the tree.
- **Change:** The M38–M42 sitting: four prebuilt release binaries (the baseline and this tree, each plain and with the align-all flags), each shown to be which tree by a string or symbol only this tree has; one background sitting of `just perf suite --repeat 5`: baseline, tree, baseline, tree, then the aligned pair; the compares; Q11's reruns, if any; then `just smoke`, `just visual` and `just physics-release`. Cost on the judged rows: one outcome value per frame in every row, one `bool` read per burst emitter per frame in `particles`, and per sprite or tile nothing outside two branches the rows are not expected to take (A15).
- **Done-when:** Q11's rule holds over every owned metric of every CPU row, the card's `gpu-throughput` and `particles` included; every first-run digest matches, 8 of 8 rows in every pair; `just smoke`, `just visual` and `just physics-release` exit 0 (the visual log's one `load_shaders 'fade'` ERROR is a deliberate test); the evidence row quotes each compare's owned summary.
- **Moves:** none.

### Step 6: Release 0.61

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), `docs/plans/1.0/w08-correctness.md` (its Decisions line), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.61.0` once, every release check once after it (smoke, visual and the physics release tests ran in step 5), then the preflight with `--message`. `[Unreleased]` keeps the workflow lines already there (`D-140`, `D-141`) and gains M43's.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.61.0 --repo JosephGibson/Tungsten --message 'Update 0.61: <summary>'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for the next register row, W1 M0b with W2 R2 (`phase5-milestone-44-input-groundwork-late-acquire.md`; sources `w01-ui-text-suite.md` §11 and `w02-multi-core-rendering.md`).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Capture frame N is skipped: fail, or slide to the next presented frame? | Fail: the run returns an error naming frame N. Sliding writes another world frame under N's name |
| Q2 | What does N count? | Frames that reach render, presented or skipped: the world's frame, today's index when nothing skips |
| Q3 | When does a capture error end the run? | At its end: the run goes on and `App::run` returns it. A render failure ends the loop at once |
| Q4 | Do `capture_frame*` keep a `Result` that never fails? | No: they return `()`; failures arrive in `RenderOutcome` |
| Q5 | Does `Skipped` carry a reason? | No public reason; the renderer logs the acquire status at debug |
| Q6 | What does `once: false` mean? | Re-armable through `rearm_burst`, never drained, the latch in `burst_fired`. Alternatives: keep `continuous_accum` as the latch (no new field); fire again once the burst's particles are gone (moves the `particles` workload) |
| Q7 | `total_pulses: Some(0)`? | `validate` rejects it; a code-built config carrying it emits nothing and drains |
| Q8 | The tile extracts' depth? | 1.0, the far plane, as the default channel has it. Alternatives: a depth argument (new surface W16b may revisit); keep 0 and document it |
| Q9 | Example 01 under `gpu_depth`? | Its extract writes its tile layers at 0, so its frame stays. Alternative: leave it, recording that its terrain and foreground would draw under its sprites |
| Q10 | How long does a warning stay quiet? | Once per sprite ID for the life of `ExtractScratch`, with no repeat after a hot reload |
| Q11 | Step 5's pass rule? | No owned metric reads `regressed` in any of the three pairs comparing the baseline with this tree (both direct pairs, the aligned pair). A metric that does is unresolved: its row runs again, baseline and tree, `--repeat 10` each; it passes when that pair reads it not `regressed`, and otherwise stops the release for the owner, who accepts the measured cost or not (a placement claim needs the regression policy's control). Both A/A pairs read 0 `regressed` and 0 `improved` on owned metrics, except these whole metrics, reported with per-run values: `gpu-throughput` `render_encode`; the `ecs` per-run-mode systems (`stats_decay`, `regen`, `follow`, `buffs`, `team_bags`, `accelerate`, `integrate`); `particles` `animate_sprites` and `particle_count_refresh`; `gpu` `extract` p95 |
| Q12 | Which decisions? | Decision 142 for the render result, 143 for bursts and pulses, 144 for far-plane tiles; the warnings need none |

Approved 2026-10-12 with the stated defaults.

Changed after approval, 2026-10-12, so that no finding depends on a step landing: the plan session filed A20, A21 and three follow-ups (the unknown mesh, the silent tileset entry, the skeleton's decision IDs) in known issues. Steps 1 and 4 no longer file them, and step 3 files only the `integrated` note its change makes stale.

## 5. Decisions expected

- Decision 142: the render result (`RenderOutcome`, `Err` as the failed frame), one-frame captures whose result comes back, and `App::run` returning the render failure, else the capture error; records the 0.27.0 acquire policy; amends `D-047` (choice 3).
- Decision 143: burst `once` and pulse `total_pulses` semantics, `burst_fired` and `rearm_burst`.
- Decision 144: the tile extracts at the far plane; amends `D-138` (clause 6, and clause 3's rewrite).

These are the next free IDs at `bb71160`, written here without the `D-` prefix: `just repo-check` rejects an active plan that cites an ID `DECISIONS.md` does not have yet (`check_docs`, from `D-140`). Each step checks the next free ID again before it writes one, and records the ID it took.

## 6. Invariants

- **May move:** nothing.
- **Must not move:** the determinism hash, the pinned containment hash, the eight row digests (`particles` among them), `gpu-visual.png`, the post and transition regressions, and every `cpu_stable` capture (steps 1 and 3 compare them with `cmp`).

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Steps 1–4 are independent; step 5 measures what landed.
- Step 1 is the card's first deliverable and W1 M2's input, so the release needs it: if step 1 is skipped, step 6 does not run. The run ends after steps 2–5 with the evidence, and the release waits for the owner (workflow §3). A skipped step 2, 3 or 4 releases without its fix: its finding stays in known issues and benchmarks.md, and the register row and w08's placement name it for W8b.
- A `cpu_stable` capture differs from the baseline's (steps 1, 3 and 5): the step is skipped; pixels may not move.
- Step 4's probe gate still differs with the work out of line: skip step 4; the warnings move to W8b.
- A visual run fails on a skipped target frame (Q1): stop for the owner.
- Step 5: a metric fails Q11's rule, an A/A pair trips outside its list, or a digest differs: stop before the release, with the compares; any justification is the owner's (regression policy).
- A file outside a step's list needs a change: stop for the owner.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–5, release | Targeted tests in 1–4; perf, smoke, visual and the physics release tests in 5; the release checks once, after the cut |
| 3 Invariants | 1, 3, 5 | `cmp` of the `cpu_stable` captures in 1 and 3; digests and hashes in 5 |
| 4 Decisions | 1, 2, 3 | Decisions 142–144, each before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 2, 3 | Snapshot changes in 1 and 2; step 3 changes behaviour, not signatures |
| 6 Template and guide | 3 | The guide's sentence on the tile extracts. The template writes nothing W8a changes: its `main` already returns `app.run()` |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 1 | `tungsten/capture.rs` in the screenshots row; `crates/tungsten-render/AGENTS.md` untouched (A19) |
| 8 Design | 1, 3 | Render results in the frame loop; the tilemap paragraph |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 1–4, release | Known issues lose the P3 row (step 1) and the bursts follow-up (step 2) and gain the `integrated` note (step 3); the plan session filed A20, A21 and three follow-ups; benchmarks.md's findings; the register row, w08's Decisions line and the owned fields at the release. No backlog row; the gap log is empty. `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

Closed 2026-10-12 at step 6: every line holds. Step 3's skip takes its share out of lines 4, 5, 6 and 8 (decision 144, its ledger row, the guide's sentence, DESIGN's tilemap paragraph); line 9 names its finding for W8b in known issues, benchmarks.md, the register row and w08's placement.

## 9. Critique

The `critique` skill with `--repo` (gpt-6.1-sol through Codex, another model family), 2026-10-12, on `bb71160` and this draft. Round 1: gpt-6.1-sol, effort xhigh, repo mode, 400.7 s, 1,173,558 tokens in (1,042,304 cached), 11,972 out (6,655 reasoning). Round 1 changed the done-when checks of steps 1 and 3, so round 2 ran on the revised plan: gpt-6.1-sol, effort xhigh, repo mode, 606.8 s, 2,070,452 tokens in (1,910,912 cached), 17,114 out (10,662 reasoning). There is no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] The tile control includes a separate depth defect: `visual` keeps `shader_share` 0.1, and `bench_heavy` sends `z_norm` through the camera while the stock shader writes it as clip depth, so the tile-free control can differ through real ordering errors; an equal count of differing pixels is not the same pixels | `examples/02_bench/src/gpu.rs:69` (`visual` does not set it); `bench_heavy.wgsl:62`; `sprite.wgsl:57-58`; `lit_sprite.wgsl:79-80`; `damage_flash.wgsl:55` and `soft_glow.wgsl:61` do the same | Accepted | Step 3's control and new test set `shader_share=0` and stay exact; the fallback stop is gone; A20 and a follow-up record the material depth |
| 2 | [MEDIUM] `Skipped` promises nothing recorded, submitted or presented, which R2's recording before the acquire breaks | `w02-multi-core-rendering.md:94` | Accepted | §2: `Skipped` means nothing presented and the capture dropped; work before the acquire may have run |
| 3 | [MEDIUM] The capture tests miss the renderer's stale capture: the early return at `renderer.rs:1238` comes before `take()` at `:1274`, so the umbrella's tests could pass with the bug in place | `renderer.rs:1238`, `1274` | Accepted | §2 and step 1: the GPU-free `capture_for_frame` in `screenshot.rs`, tested on a skipped and a failed acquire and the presented frame after them |
| 4 | [MEDIUM] Skipping step 1 still lets the release close W8a, though W1 M2 needs the contract; the roadmap reads the release, not skipped steps | `implementation-plan.md:106`; `scripts/roadmap.py:308-330`, `357-360` | Accepted | §7: the release needs step 1; a skipped step 2, 3 or 4 releases without its fix and names it for W8b |
| 2.1 | [MEDIUM] Some promised render failures still panic before producing a result: the acquire has no error scope while wgpu's default validation handler panics, and device loss during a blocking readback can panic at the polls | wgpu 30.0.1 `surface_texture.rs:74-79` (`Validation` only when a scope or handler caught it), `wgpu_core.rs:1916-1926` and `344-350` (`poll` panics on any error but a timeout); wgpu-core `life.rs:148-156`; `rg` finds no `on_uncaptured_error` in the engine; `renderer.rs:1867`; `timing.rs:158` | Accepted | Narrowed: §2, the non-goals and A21 limit `Err` to what the acquire returns; wgpu's fatal paths stay panics into the crash hook; a known-issues row in step 1 (Follow-ups) |
| 2.2 | [MEDIUM] Q11 dismisses a regression in one pair as placement or drift without proving either; the policy wants a justification for every owned regression and a control for a placement claim | `docs/perf/profiling-workflow.md:211-212` | Accepted | Q11: any `regressed` reruns its row with `--repeat 10`, then passes or stops for the owner; step 5 runs the reruns |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-12 | Done; took `D-142`, the next free ID | `cargo test -p tungsten-render --locked` → 136 + 8 passed, the four new `screenshot::tests` among them; `cargo test -p tungsten --locked --lib` → 320 passed, seven `capture::tests` and four new `app::tests` among them; `rg -n 'Render error: \|screenshot capture failed' crates/` → no match (exit 1); `just visual` → exit 0, 4 + 5 + 1 passed with `capture_past_the_last_frame_fails_the_run` and `unwritable_capture_path_fails_the_run`, plus the one expected `fade` ERROR; captures both visual runs wrote: 26 pairs, 26 `cmp`-equal; `just smoke` → exit 0 (examples 4/4, benchmarks 15/15, every matrix passed); `just api` → `api/tungsten-render.txt` +38 −12: 26 `RenderOutcome` lines (two paths) and the six methods' new returns at both paths, nothing else removed; `api/tungsten.txt` and `api/tungsten-core.txt` unchanged; `just api-check` → "Public API snapshots current: 3 crates"; the ledger row reads Landed (`D-142`) in 0.61 (M43); `decision_index` → 2 passed; `just ctx` → OK; fmt check and workspace clippy exit 0. The helper's one call before the acquire's early return is read, not run | `perf-runs/20261012-m43-render-result/step1/` |
| 2 | 2026-10-12 | Done; took `D-143`, the next free ID | On the old code `pulse_with_zero_total_emits_nothing_and_drains` failed ("panicked at crates/tungsten/tests/particles.rs:200:5: zero pulses drain on the first tick") and `config_rejects_zero_total_pulses` failed ("called `Result::unwrap_err()` on an `Ok` value: ()"); after the change `cargo test -p tungsten --test particles --locked` → 15 passed, with `burst_not_once_fires_again_after_rearm` and `burst_once_ignores_rearm`; `cargo test -p tungsten-core --locked particle` → 29 passed; `rg -n 'continuous_accum' …` → lines 336–338 of `crates/tungsten/src/particles.rs` only, `plan_emission`'s `Continuous` arm; `cargo test -p example-02-bench --locked` → 7 + 4 passed; `just api` → `api/tungsten-core.txt` +6 −0, `burst_fired` and `rearm_burst` (with its `impl` line) at both paths, the other two snapshots unchanged since step 1; `just api-check` → current. Clippy's `struct_excessive_bools` on the fourth flag takes an `allow` with its reason | `perf-runs/20261012-m43-render-result/step2/` |
| 3 | 2026-10-12 | Skipped: the control failed (§7, a failed done-when check) | Control on this tree, before any step 3 edit (debug `example-02-bench`, `visual`, `TUNGSTEN_BENCH_SET=tile_layers=0,shader_share=0`): `cmp control-gpu_depth.png control-cpu_stable.png` → "differ: byte 36, line 3"; decoded, 750,690 of 921,600 pixels above tolerance 0, max delta 197. Each mode is deterministic (a second `gpu_depth` capture is `cmp`-equal). Diagnostics: with `lit=0`, 746,821 pixels; with `post=none`, 55,692, max delta 230, so the post stack spreads a local difference; with `lit=0,textures=1`, `cmp`-equal. The default extract writes `z_norm` in painter order but draws each z-run batch by batch, one batch per class, so where two classes overlap within a z-run the depth test keeps the earlier entity while `cpu_stable` keeps the later batch: A13 holds for one class per z-run only. No step 3 file changed (each `cmp`-equal to its copy); `gpu_depth_draws_what_cpu_stable_draws` was not written and decision 144 not taken. The tile finding stays in benchmarks.md, and the new one goes to known issues (Follow-ups), both for W8b | `perf-runs/20261012-m43-render-result/step3/` (control pair, `diag/`) |
| 4 | 2026-10-12 | Done; the probe gate reads "fewer", not "the same" (see key numbers), and the owner accepted the shorter row loop as passing it, 2026-10-12 | `cargo test -p tungsten --locked sprite_extract` → 44 passed, the three new tests among them, the file's existing 1,446 lines unchanged; `rg -n 'log::warn' crates/tungsten/src/sprite_extract.rs` → lines 208 and 213, both in `warn_once` (`#[cold]`, `#[inline(never)]`). Probe gate (release, `-C force-frame-pointers=yes`, 360 frames of `gpu-throughput`, `perf record -e cycles:u -F 1999`): the row loop is the innermost natural loop around the hottest instruction of `extract_sprites_default::{closure#0}`; its blocks at ≥ 3 samples an instruction hold 218 instructions, 33 stack loads and 25 stack stores on the baseline, and 192, 23 and 19 on this tree. Retired instructions in the closure (`-e instructions:u -c 1000003`): 31,894 samples on the baseline, 29,152 on this tree, 8.6% fewer, about 19 a sprite. The baseline's inline `log::warn!` (an indirect logger call at `913420`) sat in the closure, and this tree's two `warn_once` calls take 0 samples. Nothing is added, but the count is not "the same": the plan did not foresee a shorter loop. I read it as a pass, and the owner accepted it as one. The encoder ran during the recordings, which moves timing, not code shape | `perf-runs/20261012-m43-render-result/step4/` (`annotate-*.txt`, `rowloop-*.txt`, `scripts/`) |
| 5 | 2026-10-12 | Done after the owner's acceptance: Q11 (§7) stopped the release on `ecs` `system.bounds_wrap` p50, still `regressed` after its rerun; the owner accepted the +0.05 ms as code placement on the aligned pair (0.316 → 0.316), 2026-10-12 | Sitting 18:59–19:36 UTC, four prebuilt binaries (baseline plain and aligned without the M43 symbols, the tree's two with `warn_once` and `capture::Capture`); six `suite --repeat 5`, all valid; four first attempts lost runs to `nxcodec.bin` blips after the disconnect, each set aside and rerun once. Owned summaries (regressed/improved/unchanged/noisy): direct 2/0/41/15, direct-again 1/0/47/10, aligned 0/0/45/13, baseline A/A 0/0/45/13, tree A/A 0/0/47/11, cross baseline-vs-tree-again 1/0/40/17, cross baseline-again-vs-tree 2/0/45/11; first-run digests match in 8 of 8 rows in every pair. The regressions: `bounds_wrap` p50 0.318 → 0.366 (direct) and 0.312 → 0.364 (direct-again), and `gpu-throughput` `render_encode` p95 1.872 → 2.588 (direct). Q11 rerun (`ecs` and `gpu-throughput`, `--repeat 10` each, baseline then tree): 1/0/15/4, digests 2 of 2; `render_encode` p95 2.152 → 2.026 `noisy`, so it passes; `bounds_wrap` p50 0.315 → 0.368, Δ +0.053 [+0.047, +0.059], τ 0.020, `regressed`. Placement, for the owner: `bounds_wrap`'s code is untouched and its address moved from 0 to 48 mod 64 (`placement.txt`); every plain run reads 0.31–0.32 ms on the baseline and 0.36–0.38 on the tree, while the aligned pair reads 0.316 → 0.316, Δ +0.000 [−0.008, +0.008]; `ecs` `total` p50 10.91 → 10.92 ms. The regression policy's own control (a padded baseline reproducing the verdict) has not run; the owner accepted the aligned pair as the placement evidence. Gates, after the sitting: `just smoke` exit 0 (examples 4/4, benchmarks 15/15), `just visual` exit 0 (4 + 5 + 1, the one `fade` ERROR), `just physics-release` exit 0 (4 + 1 + 7); the 26 visual captures `cmp`-equal to step 1's baseline ones | `perf-runs/20261012-m43-render-result/step5/` (`summary.md`, `sitting.log`, `rerun.log`, `gates.log`, `placement.txt`, `captures/`, `scripts/`) |
| 6 | 2026-10-12 | Done to the cut; the plan archived before it | Owner, 2026-10-12: step 5's `bounds_wrap` p50 (+0.05 ms) accepted as code placement on the aligned pair, step 4's shorter row loop as passing its probe gate. Records: the register row (cut as `v0.61.0`, `D-142`, `D-143`, step 3 named for W8b), w08's Decisions and Candidates lines, `[Unreleased]`'s Summary and M43's lines beside the workflow ones (`D-140`, `D-141`), DESIGN's status prose, the README's four owned fields from `roadmap.py summary` after the cut; workflow §5 closed (§8). `just release-cut 0.61.0`, the release checks and the preflight run after the archive, and their output is in the session's hand-off (releases.md step 1) | — |

## Follow-ups

- An unknown particle mesh logs on every emission attempt (`crates/tungsten/src/particles.rs:174-178`): in known issues, for W8b.
- A tileset entry with no registered sprite stays silent (`crates/tungsten/src/tilemap_extract.rs:67`): in known issues, for W14b's `check`.
- The skeleton asks for expected decision IDs that `check_docs` rejects (§5): in known issues, to fix before the M44 plan session.
- The `integrated` binary's row note still says its tiles draw at `z_norm` 0 (`examples/02_bench/src/integrated.rs:148`); step 3 files it in known issues, since only its change makes the note stale.
- Skipped frames are not counted in telemetry; W1 M2 or C1, if a check needs them.
- wgpu's fatal paths (A21): a validation error at the acquire with no error scope, and device loss inside a blocking poll, panic rather than return: in known issues, for W8b to scope the acquire or move to the backlog.
- Material shaders send `z_norm` through `view_proj` (A20), so material sprites order differently from stock and lit ones under `gpu_depth`: in known issues, for W8b.
- Step 3 (skipped): under `gpu_depth` the default extract writes `z_norm` in painter order while each z-run draws batch by batch, one batch per class, so `gpu_depth` differs from `cpu_stable` wherever two classes overlap within a z-run (the `visual` control, 55,692 pixels before post; `cmp`-equal with `lit=0,textures=1`). The far-plane tiles wait on it; the sprite extract's module comment (`crates/tungsten/src/sprite_extract.rs:6-7`) claims `GpuDepth` reproduces the CPU order. In known issues, for W8b.
