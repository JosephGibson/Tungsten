---
status: in progress
goal: "Close the four open P2 findings of docs/repo-review-2026-09-25.md and cap the frame dt, each with a regression test that fails before its fix, then move the remaining findings to a maintained document."
non-goals:
  - "The P3 findings of the review and its carried-forward items (they move to docs/known-issues.md unchanged)."
  - "Phase 4 milestone work (M31)."
  - "Performance work. Captures here only show that nothing regressed."
  - "A fixed-step accumulator with render interpolation. It changes the core/render seam (extract needs previous transforms) and belongs to the 1.0 criteria plan."
  - "Public API changes beyond what a fix needs."
  - "A release cut."
files to touch:
  - "crates/tungsten/src/app.rs, crates/tungsten/src/tests/app.rs, crates/tungsten/src/asset_loader.rs, crates/tungsten/src/hot_reload.rs, crates/tungsten/src/tests/hot_reload.rs"
  - "crates/tungsten-core/src/assets/manifest.rs, crates/tungsten-core/tests/composition.rs"
  - "crates/tungsten-core/src/physics/step.rs, crates/tungsten-core/src/tests/physics/step.rs, crates/tungsten-core/tests/physics_tunneling.rs"
  - "crates/tungsten-render/src/post/ (mod.rs, fullscreen.rs, the 17 effect files), crates/tungsten-render/src/renderer.rs, crates/tungsten-render/src/tests/post.rs"
  - "examples/04_shader_playground/src/main.rs, examples/04_shader_playground/tests/post_regression.rs (new)"
  - "justfile (one line in the visual recipe, if S3.b is approved)"
  - "DESIGN.md, DECISIONS.md, docs/DECISION_INDEX.md, CHANGELOG.md, docs/README.md, docs/LLM_INDEX.md, docs/perf/benchmarks.md (digest lines only)"
  - "docs/known-issues.md (new); docs/repo-review-2026-09-25.md moves to docs/plans/archive/"
  - "docs/plans/p2-correctness-pass.md"
ordered steps:
  - "0. Baseline: gates, counts, reference screenshots, perf baseline and A/A."
  - "1. Frame dt cap."
  - "2. Manifest roots: merged validation and merged reload."
  - "3. Stock post passes own their params per post-stack slot."
  - "4. Stock post shaders rebuild on reload."
  - "5. Physics arrival pass. Stops for design sign-off once its failing test exists."
  - "6. Docs close-out: decisions, design, changelog, known issues, review archived, plan archived."
done-when:
  - "Every new regression test was seen failing before its fix and passes after it."
  - "just check, just smoke, just visual, just script-test, just repo-check, just ctx and the release physics tests pass."
  - "No existing golden image is replaced; the reference screenshots of step 0 are byte-equal after steps 3 and 4."
  - "No owned metric of a physics or gpu row reads regressed against the previous step, or the reading has its recorded justification."
  - "The four P2 findings are gone from the review, which is archived; the remaining findings are in docs/known-issues.md."
  - "D-088 to D-092 exist with their index rows; DESIGN.md and CHANGELOG.md record the five fixes."
  - "This plan is done and archived; no plan is left in progress."
---

# P2 correctness pass

## Context digest

Branch `0.35`, tree `6288601`, workspace `0.34.0`. Five defects, each confirmed against the code on 2026-10-02; nothing is changed yet.

1. `App::stage_delta_time` writes the raw wall-clock dt outside smoke mode. Benchmarks, smoke runs and the pixel test pin dt, so none sees it.
2. `ResolvedManifest::load` rejects a material whose shader lives in another root before the merge can resolve it. `reload_manifest` reloads one root over the merged `LoadedManifest`, and `App` routes one manifest path.
3. Each stock post variant has one params UBO. Two passes of one variant draw with the parameters written last.
4. The 17 stock post pipelines are built from `include_str!`. No reload branch rebuilds them, and the reload still logs success.
5. The solver runs one velocity iteration per substep. A body that gains velocity from it is then moved against neighbours whose contact was solved earlier, or never admitted.

Step 5 is wider than the review's finding. The probe shows three mechanisms, spawn order decides most cases, and thin static walls fail too when the pusher is spawned after the body it pushes. The chosen design and its evidence are in "Step 5"; implementation waits for sign-off after the failing test exists.

Decisions `D-088` to `D-092` are reserved, one per fix. Git mutations are denied, so each step is handed over as a patch with a message. Perf capture sittings need the owner's go-ahead each time.

## Decisions for approval

Defaults are what phase 2 implements unless changed here. "New" marks a decision the request did not carry.

| ID | Decision | Default | Alternative |
| --- | --- | --- | --- |
| S1.a | Cap value | 0.1 s. One step of it leaves a settled awake pile at about 40 px/s | 0.05 s (about 13 px/s), which slows any game running under 20 FPS |
| S1.b | Where the cap lives | `const MAX_DT_SECS` beside `SMOKE_MODE_FIXED_DT_SECS` in `app.rs` | A `tungsten.json` field |
| S2.a | Validation | Parse and resolve per root, validate material → shader once on the merged graph. A single-root `load` keeps its strict contract | — |
| S2.b | Watcher, new | Built in `App::run`, so the order of `enable_hot_reload` and `set_manifest_roots` cannot matter. The "Hot reload watching" log lines move from configuration to run | Rebuild the watcher in both setters |
| S2.c | Layer 1, new | `tests/manifests.rs` keeps loading each shipped manifest alone, so a shipped cross-root reference would still fail there. None exists. Leave it and record it in `D-089` | Teach layer 1 the root sets |
| S3.a | Params ownership | One params UBO and bind group per post-stack slot, built on first use and kept. Pipelines stay per variant | One buffer with dynamic offsets (touches all 17 pipeline layouts) |
| S3.b | GPU regression home, new | `examples/04_shader_playground/tests/post_regression.rs` behind `TUNGSTEN_VISUAL_REGRESSION`, two new fixture values, and one added line in the `visual` recipe so `just visual` runs it. `AGENTS.md` is not touched | No recipe change; the test runs only by its own command |
| S4.a | Stock reload | Connect it | Document stock shaders as outside the matrix |
| S4.b | Body-edit capture | Start the playground in a staged asset tree with one edited stock WGSL. No example code | A playground startup fixture that calls `asset_loader::reload_shader` |
| S4.c | Shader IDs, new | Pre-seeding gives the 17 stock shaders render-side IDs 11–27; later shaders start at 28. The umbrella bridges by name, and the renderer's own constants (0–10) do not move | — |
| S5.a | Physics design, new | Arrival pass with a composite backward sweep (see "Step 5") | The same pass without the backward sweep. It closes the matrix too and leaves the pusher inside the pushed body in 71–96 of 512 cases per spawn order |
| S5.b | Flag threshold, new | `2 · linear_slop` of travel change per substep. The requested `4 · linear_slop` leaks 1 case of 512 | — |
| S5.c | Test scope, new | The matrix in three spawn orders (pusher first, pusher last, pushed body asleep), static and dynamic gates, both thicknesses; neither body may pass the gate | The requested matrix only (pusher first, plus the asleep variant) |
| S5.d | Residuals, new | Accept and record in `docs/known-issues.md`: a 1000:1 pusher at 120–480 px/s can still crush a body through a 4 px dynamic gate (6 of 320 slow-push cases, 41 today); at 15,360 px/s with the pusher spawned last, the pusher can end past the body it pushed (17 of 512) | Widen the fix (wall-last contact order plus static softness for heavy partners closed the slow crush in the probe and changes every pile) |
| S5.e | Digests, new | The three physics-bearing digests are expected to change. Record the new ones in `docs/perf/benchmarks.md`; `workload_version` stays 1 | — |
| S6.a | Live home | `docs/known-issues.md`, with a row in `docs/README.md` and in `docs/LLM_INDEX.md`. The review moves to `docs/plans/archive/` | — |
| S6.b | Released changelog line | No edit. Commit `e9081ef` already rewrote the `0.27.0` line to `docs/plans/archive/repo-review-2026-09-25.md`; the move makes it true | — |
| S6.c | Decision IDs | `D-088` dt cap, `D-089` manifest roots, `D-090` per-slot params, `D-091` stock reload, `D-092` arrival pass. Re-check the next free ID before editing | — |
| C.a | Capture sittings | Three, each the full suite at `--repeat 5`: baseline plus A/A at step 0, after step 4, after step 5 | Narrower `--only` lists |

## Rules for this pass

- **Git.** No `git add`, `commit`, `apply` or `mv`. Per step: one patch and one message file in the scratchpad, made with plain `diff -u` and `a/` `b/` labels, checked with `patch -p1` on a `git archive HEAD` export, plus a script that runs `git apply --cached` and `git commit -F` per patch. Every commit must pass `just repo-check` on its own, so decisions and their index rows travel together. No `Co-Authored-By` or "Claude Code" lines anywhere.
- **Captures.** Ask before each sitting. Each suite is one blocking foreground call with nothing beside it. `pgrep -x nxcodec.bin` must print nothing before and after; it was running while this plan was written. Verdict checks read "not regressed": an effect just under the threshold reads `noisy` in every capture.
- **Start.** Set `status: in progress`. Read `crates/tungsten-render/AGENTS.md` before step 3. Load the `tungsten-wgpu` skill for steps 3 and 4 and the `tungsten-perf` skill before any capture.
- **Docs.** Re-read a file right before editing it. Touch only the docs listed in the header. The docs-cleanup pass is committed (`e9081ef`, `6288601`).
- **Archive.** Never open, search or list `docs/plans/archive/`. Moving a known filename into it is allowed.
- **Stops.** Before each capture sitting; after step 5's failing test (design sign-off); before the two manual checks that open a window (steps 2 and 4).
- **Reporting.** A failing or skipped check is reported as such.

## Step 0 — Baseline

1. `just check`, `just script-test`, `just repo-check`, `just ctx`. Record the test counts.
2. `WGPU_BACKEND=vulkan just smoke` and `just visual`. Record the row counts.
3. `RUSTFLAGS="-C force-frame-pointers=yes" cargo test --release -p tungsten-core --locked --test physics_determinism --test physics_tunneling --test physics_containment`. Record the determinism hash it prints.
4. Reference screenshots of example 04 at `TUNGSTEN_SMOKE_FRAMES=8 TUNGSTEN_CAPTURE_FRAME=6 TUNGSTEN_CAPTURE_RESOLUTION=1280x720`, kept in the scratchpad: fixtures `empty`, `all`, and `bloom_only` with `TUNGSTEN_BLOOM_FIXTURE=on` (the helper is in `docs/showcase/README.md`). Capture each twice and confirm the two are byte-equal before relying on them.
5. After the owner's go-ahead: `WGPU_BACKEND=vulkan just perf suite --repeat 5` twice, as two blocking calls. Save the first as baseline `p2-base` and compare the second with it. The A/A must show no `regressed` or `improved` owned metric outside the readings `docs/perf/profiling-workflow.md` lists for an untouched tree.

## Step 1 — Frame dt cap

**Confirmed.** `stage_delta_time` (`crates/tungsten/src/app.rs:681`) writes `now.duration_since(last)` unless `smoke_frames_remaining` is set. `scripts/bench.py:217` and `examples/02_bench/tests/visual_regression.rs:35` set `TUNGSTEN_SMOKE_FRAMES`, so benchmarks and the pixel test run at 1/60 s and no capture is needed for this step. The stall probe, rerun on this tree, matches the earlier output byte for byte:

| Scene (awake, on a static floor, gravity 900) | One step of 0.05 s | 0.1 s | 0.2 s | 0.5–1 s | 2 s |
| --- | --- | --- | --- | --- | --- |
| Player-like box driven at 560 px/s | unaffected | unaffected | unaffected | unaffected | unaffected |
| 5-box stack, residual speed | 13.4 px/s | 40.1 | 117.7 | 258–282 | 1 of 5 through the floor |
| 30-circle pile, residual speed | 12.7 px/s | 39.5 | 112.9 | 179–213 | 6 of 30 through the floor |

Sleeping piles are unaffected at every stall.

**Change.**

- `const MAX_DT_SECS: f32 = 0.1` beside `SMOKE_MODE_FIXED_DT_SECS`.
- A free function `frame_dt_secs(elapsed: Duration, smoke: bool) -> f32` beside `redraw_schedule`: smoke returns the pinned value, otherwise `elapsed` capped at `MAX_DT_SECS`. `stage_delta_time` calls it.

**Tests.**

- `crates/tungsten/src/tests/app.rs`: 2 s gives the cap, 16 ms passes through, smoke gives 1/60 s whatever elapsed. Write them against the extracted function before the cap exists and watch the 2 s case fail.
- `crates/tungsten-core/tests/physics_tunneling.rs`: the 30-circle pile and the 5-box stack, sleeping off, settled for 240 steps, take one step of 0.1 s and then 120 steps of 1/60 s. No body ends below the floor and none exceeds 50 px/s afterwards. The literal 0.1 carries a comment naming `MAX_DT_SECS`. This one pins the rationale and passes on the current tree.

**Done when.** The three app tests and the pile test pass; `just check` passes.

**Docs (step 6).** `D-088`, superseding the "Variable-dt with substep cap" known limit of `D-033` (`DECISIONS.md:154`; the substep cap went with `D-064`). It records the fixed-step accumulator as a decision for the 1.0 criteria plan. `DESIGN.md:72` and `:232` name the cap and still say there is no accumulator.

## Step 2 — Manifest roots

**Confirmed.**

- `ResolvedManifest::load` (`crates/tungsten-core/src/assets/manifest.rs:376-394`) fails a material whose shader is not in the same file. `load_and_merge_many` (`:400`) calls it per root, so a cross-root reference fails at boot. `merge` (`:455-468`) checks against the shaders merged so far, so root order would decide even without the first check.
- `merge_material_resolves_shader_from_sibling_manifest` (`src/tests/assets/manifest.rs:401`) builds both structs by hand and never calls `load`.
- No shipped manifest crosses roots: the root manifest's one material uses a root shader, and the platformer and bench materials use their own.
- `reload_manifest` (`crates/tungsten/src/asset_loader.rs:1400`) loads one root, diffs it against the global registries (every sprite of the other root logs "removed — keeping stale") and stores that one root as `LoadedManifest` (`:1692`).
- `App::enable_hot_reload` (`app.rs:348`) stores one path and `process_hot_reload` compares only it (`:410`). An edit to another root manifest reaches the `"json"` branch (`:447`) and is dropped.
- The platformer calls `enable_hot_reload` before `set_manifest_roots` and passes `MANIFEST_LOCAL` (`examples/01_platformer/src/setup.rs:98-99`, `:142`). It already watches both asset directories, so the shared manifest's events arrive and are discarded.

**Change.**

- Core: private `load_unvalidated` (parse, resolve paths), private `merge_unchecked` (duplicate IDs only), private `validate_cross_refs` (reports the missing material with the smallest ID, so the error does not depend on map order). `load` is `load_unvalidated` plus `validate_cross_refs`, as today. `merge` is unchanged. `load_and_merge_many` loads and merges every root unchecked, then validates once.
- Umbrella: `reload_manifest(roots: &[PathBuf], …)` rebuilds the merged graph with `load_and_merge_many`, diffs that against the registries and stores it as `LoadedManifest`. A failure logs and keeps the last good state, as a parse failure does today.
- `App`: a free function gives the reload set, `manifest_roots` when non-empty, else the path given to `enable_hot_reload`. A second one tells whether a changed path is in that set. `process_hot_reload` reloads at most once per drained batch.
- `App::enable_hot_reload` records its arguments; `App::run` builds the watcher with the input map and every root manifest as extra files.
- `HotReloadWatcher::new` skips an extra file that a recursive root already covers, so a root manifest inside a watched asset directory gets no second watch on its parent. Added during implementation.

**Tests.**

- `crates/tungsten-core/tests/composition.rs`: a material in one root resolves a shader in the other through `load_and_merge_many`, in both root orders; a shader missing from every root is still `MaterialShaderMissing`. Both orders fail before the change.
- `crates/tungsten/src/tests/app.rs`: the reload set and the path test, as pure functions.
- Manual, after the owner's go-ahead: run the platformer with `RUST_LOG=info`, edit `assets/manifest.json` (a `uniform_defaults` value of `damage_flash`), expect one "Manifest reloaded" line and no "removed — keeping stale" line, then restore the file. Record the log lines here.

**Done when.** The composition tests fail before and pass after; `just check` and `just smoke` pass; the manual check is recorded.

**Docs (step 6).** `D-089`, amending `D-017`, `D-052` and `D-053`. `DESIGN.md:197` loses its two limitation sentences.

## Step 3 — Per-slot post params

**Confirmed.** `StockPipeline` (`crates/tungsten-render/src/post/mod.rs:68`) holds one `params_ubo`, one `params_bg` and one `params_written`. `record_pass` (`:360`) writes the buffer while recording. Two passes of one variant with different parameters queue two writes to one buffer before submission, and both draws read the second. The write skip of `D-085` does not help: the two payloads alternate, so both are written every frame. All 17 pipelines share `params_bgl`. Bloom already owns objects per slot (`post/bloom.rs:154`), and the call site has the slot index `pi` (`renderer.rs:1429-1440`).

**Change.**

- `PostStackRenderer` gets the params of each post-stack slot: a buffer, its bind group and the bytes it holds, built on first use and kept between frames. The layout is shared, so a slot serves whichever variant sits in it.
- The container is generic over its GPU objects, as `slot_cache` is, so its rules are testable without a device.
- `StockPipeline` keeps the pipeline only. `record_pass` takes the slot index.
- A resize does not drop the slots: they hold no target views.

**Tests.**

- `crates/tungsten-render/src/tests/post.rs`, no GPU: two slots keep objects and bytes of their own; equal payloads on the second frame need no write and build nothing; swapping two slots' payloads writes both; a far slot index leaves the others alone.
- `examples/04_shader_playground/tests/post_regression.rs`, behind `TUNGSTEN_VISUAL_REGRESSION`: fixture `fade_pair` is `[Fade(red, 0.5), Fade(blue, 0.5)]`, fixture `fade_twice` is `[Fade(blue, 0.5), Fade(blue, 0.5)]`. More than half the pixels must differ by more than 8. On the current tree both render as the second, so the count is 0: watch it fail first. `compare_png` comes through `tungsten::render`.
- The example's header comment lists the fixture values, the two new ones included.
- With S3.b, the `visual` recipe gains `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-04-shader-playground --test post_regression --locked -- --nocapture`. Its existing line stays as it is.

**Done when.** The capture test fails before and passes after; the three reference screenshots of step 0 are byte-equal (`cmp`); `cargo test -p tungsten-render`, `just smoke`, `just visual` and `just script-test` pass with the golden image untouched.

**Docs (step 6).** `D-090`, amending the "Uniform writes" bullet of `D-085` and `D-058`.

## Step 4 — Stock post shader reload

**Confirmed.**

- `Renderer::upload_shader` (`crates/tungsten-render/src/renderer.rs:649`) and `reload_shader` (`:726`) carry the same chain twice: sprite, three SMAA names, four bloom names, lit sprite. A stock name matches none, gets a cache commit and a material rebuild that finds no material, and `reload_shader` logs "reloaded".
- `Renderer::new` seeds IDs 0–10 (sprite, SMAA, bloom, lit sprite, `emissive_mask`, `rim_light`) and starts allocating at 11 (`:493`). The 17 stock shaders are not seeded, so `load_shaders` validates and commits each at startup, which `D-057` describes otherwise.
- The stock pipelines are built in `post/<effect>.rs::build` from `include_str!` through `fullscreen::build_pipeline`, which creates its own module.
- The manifest IDs of the 17 equal `PostPass::kind_name`.
- The render crate uses no error scope. A layout change that Naga accepts stays outside the body-edit contract for every pipeline, as `D-057` says.

**Change.**

- A table of the 17 names with their compiled-in sources, and a pure function from a `PostPass` to its stock name.
- `Renderer::new` seeds the cache from the table before it builds the post stack; `next_shader_id` starts at 28. Pipelines are built from the cached modules through a `build_pipeline` variant that takes a module.
- `StockPipeline::rebuild_with_module` builds the new pipeline into a local and assigns it, as the bloom stages do. Format comes from `surface_config`.
- The duplicated chain becomes one private helper that both functions call, moved as it is, with the stock branch added once.

**Tests.**

- `src/tests/post.rs`: the table has 17 distinct names; a `match` over `PostPass` with no wildcard maps every variant but bloom to a table entry equal to its `kind_name`. A new variant stops compiling.
- `tests/shader_coverage.rs` stays green: both trees validate and the mirrors are byte-equal.
- `post_regression.rs`, Unix only: stage a temporary directory that mirrors what the playground reads through symlinks (`tungsten.json`, `input.json`, `examples/04_shader_playground/assets`, and `assets/` entry by entry), except `assets/shaders/stock/fade.wgsl`, which is a copy with `params.v0.rgb` replaced by `params.v0.bgr`. The test asserts the text is there before replacing it. Run the playground in that directory with `fade_twice`. Its capture must differ from the unstaged one in more than half the pixels. On the current tree the two are equal.
- Manual, after the owner's go-ahead: run the platformer with `RUST_LOG=info` (its stack has a vignette), edit `assets/shaders/stock/vignette.wgsl`, expect "shader 'vignette' reloaded" and a visible change, then restore the file so the mirror stays byte-equal.

**Done when.** The staged capture test fails before and passes after; the reference screenshots are byte-equal; `cargo test -p tungsten-render`, `just smoke`, `just visual` and the step-3 test pass. After the owner's go-ahead, the full suite reads no `regressed` owned metric against `p2-base`. Steps 1 to 4 can move `gpu`, `gpu-throughput` and `integrated`; the other rows show code placement.

**Docs (step 6).** `D-091`, amending `D-057` and `D-058`. `DESIGN.md:216` is replaced by a stock row in the matrix.

## Step 5 — Physics arrival pass

### What the probe shows

The request's probe was rerun on this tree and matches the earlier output byte for byte. A is a circle of radius 8 and mass M moving at speed v. B is a circle of radius 8 and mass 1 at rest, a gap short of the gate. The gate is an AABB 800 px tall. No gravity, 60 steps. Per spawn order there are 512 cases:

- gate static, or dynamic with mass 1e6;
- gate 4 or 32 px thick;
- gate spawned before or after B;
- M of 1, 10, 100 or 1000;
- v of 960, 1,920, 7,680 or 15,360 px/s;
- gap of 0.5, 3, 12 or 60 px.

A case fails when B ends beyond the gate.

**Three mechanisms, not two.** A per-substep trace of the failing cases:

| Spawn order | Fail | No B–gate pair in the crossing substep | B–gate contact solved before the push | B–gate contact solved after the push, B still through |
| --- | --- | --- | --- | --- |
| A first (the request's probe) | 67 | 18 | 36 | 13 |
| A last, B asleep before impact | 95 | 24 | 68 | 3 |
| A last, everything awake | 104 | 22 | 79 | 3 |

- The first two mechanisms are the review's finding. A resting B has a pair radius of 1 px, and the one velocity iteration (`step.rs:1434`) gives the last word to whichever contact comes later in the pair list. In 13, 36 and 36 of the "solved before the push" cases B is marched through over several substeps, not thrown through in one.
- The third column is the soft contact giving way under a sustained load. B creeps 0.3–0.4 px per substep until its centre passes the gate's, where the contact normal flips. It needs no impact: a 1000:1 pusher at 120 px/s does it.
- Spawn order decides. With A spawned last, B crosses thin **static** gates in 22 cases and 32 px gates in 3. The request's "a static gate never fails, a thick gate never fails" holds for one spawn order only.
- The statics-only sweep (`speculative_pass`, `step.rs:1576`) does not cover these. It skips travel under one half-extent per substep, and `sweep_aabb_vs_aabb` ignores an overlapping start (`collision.rs:218`); a body resting on a wall overlaps it by about `linear_slop`.
- A itself ends beyond the gate in 2, 1 and 0 cases.

**Candidates measured.** Failing cases of 512 per spawn order, in the order of the table above:

| Variant | A first | B asleep | A last |
| --- | --- | --- | --- |
| Current tree | 67 | 95 | 104 |
| (ii) velocity bound, as an oracle: every pair radius and admission margin assume twice the fastest speed in the world | 56 | 87 | 86 |
| (i) without an order: each flagged body clamped against its neighbours as the grid lists them | 25 | 121 | 118 |
| Regular contact list sorted wall-last, nothing else | 50 | 62 | 49 |
| Arrival pass, forward sweep only | 0 | 0 | 0 |
| Arrival pass with the composite backward sweep (chosen) | 0 | 0 | 0 |
| Chosen, threshold `4 · linear_slop` | 1 | 0 | 0 |
| Chosen, threshold `8 · linear_slop` | 23 | 32 | 26 |

- (ii) is rejected. Even its upper bound supplies only the missing pair; the contact is then still solved before the push.
- (i) without an order is rejected. It repeats the solver's order dependence inside the pass and makes two of three spawn orders worse.
- Sorting the regular contacts changes every pile and adds failures on thick gates.

**The backward sweep.** End state of the cases where B is held:

| Variant | Pusher overlaps B by more than 1 px | Pusher ends past B |
| --- | --- | --- |
| Current tree | 2 / 12 / 4 | 26 / 117 / 105 |
| Forward sweep only | 96 / 95 / 71 | 150 / 194 / 159 |
| With the backward sweep | 0 / 1 / 4 | 0 / 0 / 17 |

The forward sweep alone turns "B through the gate" into "the pusher inside B". The backward sweep stops the pusher behind B and hands its momentum to the gate. The 17 left are all at 15,360 px/s with A spawned last and M of 10 or more; nothing passes the gate.

**Side checks of the chosen variant.**

- Slow pushes (320 cases: 60–960 px/s, 600 steps, thin gate, both spawn orders): 41 cross on the current tree, 9 of them a static gate. 6 cross with the pass, all a dynamic gate with a 1000:1 pusher spawned first.
- Free trains of three bodies, no wall, mass ratios to 1000:1: 8 of 8 keep their order and all their momentum. The forward sweep alone breaks the order in one.
- With the pass on by default in a scratch copy of the core crate: 453 of 453 unit tests pass, `physics_tunneling` passes, and in release `physics_containment` reads 0 of 3,000 escaped and `physics_determinism` is bit-identical between runs. Its hash changes, `0x0b8e2a27f2961571` to `0x088ec07a73c1b168`.
- The 3,000-body pile of that test: the pass runs in 106 of 720 substeps, all during the fall and compression, on 1,525 bodies. No body is flagged once the pile has settled, asleep or kept awake.
- Rough wall time, same binary, encoder running, not a capture: 240 steps with sleeping on take 514–517 ms without the pass and 522 ms with it. 600 steps of the pile kept awake take 1,210 and 1,356 ms without it in two sets and 1,252 ms with it. The difference is inside that run's noise; the captures decide.
- The stall probe of step 1 reads the same at 0.1 s with the pass on.

The probes, the prototype diff and their outputs are in `perf-runs/20261002-p2-gate-probe/` (machine-local, gitignored; its README lists the switches). The prototype is not the patch: it uses statics for its switches and counters and clears its per-proxy state for every proxy each substep.

### Design

One pass in `substep`, after the biased iterations and before position integration. It leaves the pair list, the contact list, warm-start impulses and events alone.

It is candidate (i) with three changes the probe forced:

- Neighbours come from a grid query at the new velocity, not from a pair repair. The regular repair of `D-081` picks the pair up in the next substep.
- Pairs are clamped directly instead of re-solving the regular contacts.
- Pairs are ordered, and a pressed chain is clamped as one body.

1. **Admitted velocity.** The gravity loop stores each awake dynamic proxy's velocity as the narrow phase saw it, before gravity is added.
2. **Flag.** After the biased iterations, an awake dynamic proxy whose velocity differs from the admitted one by more than `2 · linear_slop / sub_dt` joins a worklist, in proxy order. Two bodies under the threshold change a pair's closing travel by at most `4 · linear_slop`, the admission slack of `D-064`. A substep without contacts skips the scan.
3. **Pairs.** Each listed body queries the pair grid, and the repair grid for proxies repaired since the last build, with its AABB grown by `|v| · sub_dt + 4 · linear_slop`. Every candidate, static, sleeping or dynamic, goes through `narrow_phase` with the margin `D-064` would give the current velocities. A hit is an arrival pair: its normal and its allowance, `(linear_slop − penetration) / sub_dt` when positive, else 0. Two listed bodies share one pair.
4. **Order.** The mover of a pair is the member whose own velocity closes it faster. A body pushed by a mover sits one level deeper than it; depths are relaxed at most 4 times, so a cycle cannot loop. Pairs are sorted by the mover's depth, then by the target's inverse mass, lighter targets first: pushers first, walls last. The sort is stable.
5. **Forward sweep.** A pair closing faster than its allowance gets the impulse that removes the excess, split by inverse mass; a static or sleeping member has none. A body whose velocity now differs from its admitted one by more than the threshold joins the worklist for the next round. A round is steps 3 to 6 for the bodies new on the list. At most 4 rounds; a body joins once per substep.
6. **Backward sweep.** In reverse order, a pair that is tight (closing within 0.001 px of travel per substep of its allowance) or still closing too fast marks its mover as pressed on its target. If it still closes too fast, the mover is clamped against the target's pressed chain as one body: the chain's combined mass takes the impulse and every member gets the same velocity change. A target's chain counts only when it is pressed along the pair's direction (within 60°). A chain that ends at a static or sleeping body has no inverse mass, so the mover takes all of it.
7. Position integration, relax, restitution and the statics sweep follow as today.

Properties:

- **Cost.** One store per awake body in the gravity loop and one compare per awake body per substep. Nothing per contact. Grid queries, narrow-phase calls and the sort run only for flagged bodies. The pass's per-proxy state is reset for the bodies it touched, not for all.
- **Determinism.** Proxy order, grid query order, a stable sort with `total_cmp`. No map, no thread.
- **Sleep.** The pass wakes nobody. A sleeper is immovable in it, and the regular wake test fires in the next substep.
- **Restitution.** A clamp is inelastic. A body stopped by one does not bounce in that substep.
- **Unchanged.** `solver_iterations`, contact softness, the pair list, the order of the contact list and the statics sweep.
- **Completeness.** A candidate the query misses has an inflated AABB that does not reach the listed body's grown AABB. An unflagged candidate's speed is within the threshold of the one its travel budget was checked with (`D-081`), so the gap exceeds the pair's margin. A flagged one runs its own query.

### Tests

Written first; the three in `physics_tunneling.rs` fail on the current tree.

- `crates/tungsten-core/tests/physics_tunneling.rs`:
  - The matrix above in all three spawn orders, 1,536 cases, about 3 s in debug. Neither A nor B may end beyond the gate. Fails today in 266 cases for B.
  - Slow pushes against a thin static wall in both spawn orders, 160 cases. Fails today in 9.
  - Free trains: order and momentum kept. A guard; it passes today.
  - The header's guarantee names pushed bodies, both spawn orders and the asleep variant, and states the two residuals.
- `crates/tungsten-core/src/tests/physics/step.rs`, with `cfg(test)` counters beside `pair_builds`: a settled stack never enters the pass; a body pushed onto a static wall arrives at it in either spawn order and its pusher stops behind it; the pair-contact and warm-start oracles still hold with the pass running.

**Stop here** and show the owner the failing output with this section before any change to `step.rs`.

### After sign-off

- Implement, then `just check` and the release physics tests of step 0. Record the new determinism hash.
- After the owner's go-ahead: the full suite, compared with the step-4 suite for every row.
  - Owned metrics of `physics`, `physics-sparse` and `integrated` must not read `regressed`.
  - For a `regressed` or `improved` row whose code did not change, compare `nm -C --defined-only` of both builds first. The placebo is this tree with the flag threshold set to infinity: it has the candidate's layout and the baseline's behaviour.
  - Report each row's digest. `ecs`, `churn`, `particles`, `gpu` and `gpu-throughput` must keep theirs. For a physics-bearing row that changes, show that the pass applied a clamp in that world.
  - If the scan itself reads as a regression, the options are to skip it for substeps without contacts, already in the design, or to flag from impulse deltas inside the solve loop. The second is a per-contact compare and needs the owner's approval.

**Done when.** The three tunnelling tests fail before and pass after; `just check`, the release physics tests and `just smoke` pass; the capture reads no `regressed` owned metric or carries its justification; digests are reported.

**Docs (step 6).** `D-092`, amending `D-064` (solver-injected velocity is checked before integration, against every neighbour) and the step order of `D-063`. `D-075`, `D-080` and `D-081` stand; the pass relies on their grids and budgets. `DESIGN.md`'s physics section names the pass. `docs/perf/benchmarks.md:117` and `:443` take the new digests.

## Step 6 — Docs close-out

One patch, after step 5.

- **`DECISIONS.md` and `docs/DECISION_INDEX.md`.** `D-088` to `D-092` with their rows: `D-088` and `D-092` under "ECS / Runtime Flow", the others under "Assets / Rendering". An amended entry gets its `**Amended by D-0NN:**` or `**Superseded by D-0NN:**` line under its heading; its text is not edited.
- **`DESIGN.md`.** Lines 72 and 232 (dt cap), 197 (merged reload), 216 and a stock row in the matrix, the physics section (arrival pass).
- **`CHANGELOG.md`.** `[Unreleased]` gains a "Fixed" list of the five fixes and a line for `docs/known-issues.md` and the archived review; its summary line names the pass. The `0.27.0` line is not edited (S6.b).
- **`docs/known-issues.md`**, short:
  - The five P3 rows of the review, unchanged.
  - Its platform checks and follow-ups, each checked against the tree first. Two are already resolved and are dropped: PCM WAV decode (`D-077`), and `perf-capture.sh`, which no longer exists. `actionlint`, `libudev-dev` and the `RUSTSEC-2026-0192` exception are still open.
  - The `player.png` deletion candidate.
  - This pass's residuals: S5.d, and the layer-1 limit of S2.c.
- **`docs/README.md` and `docs/LLM_INDEX.md`.** One row each pointing at `docs/known-issues.md`. No row for the review. The repository checker's document list is not extended (tooling is out of scope), so check the new file's links by hand.
- **The review.** Remove the four P2 rows and replace its "live follow-up record" sentence with a pointer to `../../known-issues.md`, then move the file to `docs/plans/archive/repo-review-2026-09-25.md`. It has no other Markdown links to fix. Git tracks no file at that path.
- **This plan.** Record outcomes, set `status: done`, move it to `docs/plans/archive/`.
- `just ctx`, `just repo-check`, `git diff --check`, then `just check`.

## Hand-off

Patches `01-dt-cap` to `06-docs-closeout`, each with its message, and the commit script, in the session's scratchpad. This file is untracked: commit it once approved, or let patch 01 carry it. The closing report lists per step: the test seen failing, the gates run with their counts, capture verdicts with paths, digests, the two manual checks, and anything skipped.
