# The engine perf pass's proposals (`engine-perf-proposals`) — in progress

- **status:** in progress
- **goal:** land the four proposals of the engine performance pass the owner chose on 2026-11-23, each judged: the physics pool's workers spinning for the next fork, bloom's pyramid from a quarter of the target above 1440 rows with a vignette after bloom drawn in bloom's composite, saves staged on the frame and committed on a writer thread, and the arrival pass that clamps a listed body against every pair partner in reach. The compositor-bypass hint stays in Later.
- **non-goals:** the compositor-bypass hint (an X11 dependency or FFI); any change to example 01's or the template's code or tests, their shells included; the exact levers benchmarks.md lists as left; new `unsafe` or a third-party dependency; a release cut.
- **files to touch:**
  - Step 1: `crates/tungsten-core/src/physics/step/pool.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/config.rs`, `crates/tungsten-core/src/tests/config.rs`, `crates/tungsten-core/src/tests/physics/workers.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs`, `api/tungsten-core.txt`, `docs/getting-started.md`, `DESIGN.md`, `docs/perf/benchmarks.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`.
  - Step 2: `crates/tungsten-render/src/targets.rs`, `crates/tungsten-render/src/post/bloom.rs`, `crates/tungsten-render/src/renderer.rs`, `crates/tungsten-render/src/shaders/stock/bloom_threshold.wgsl`, `assets/shaders/stock/bloom_threshold.wgsl`, `crates/tungsten-render/src/tests/bloom.rs`, `crates/tungsten-render/src/tests/targets.rs`, `crates/tungsten-core/src/config.rs`, `crates/tungsten-core/src/tests/config.rs`, `crates/tungsten/src/app.rs`, `examples/02_bench/tests/visual_regression.rs`, `examples/02_bench/tests/fixtures/` (one new reference image), `api/tungsten-core.txt`, `api/tungsten-render.txt`, `crates/tungsten-render/AGENTS.md`, `docs/getting-started.md`, `DESIGN.md`, `docs/perf/benchmarks.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`.
  - Step 3: `crates/tungsten-render/src/post/mod.rs`, `crates/tungsten-render/src/post/bloom.rs`, `crates/tungsten-render/src/renderer.rs`, `crates/tungsten-render/src/timing.rs`, `crates/tungsten-render/src/passes/order.rs`, `crates/tungsten-render/src/shaders/stock/bloom_composite.wgsl`, `assets/shaders/stock/bloom_composite.wgsl`, `crates/tungsten-render/src/tests/post.rs`, `crates/tungsten-render/src/tests/timing.rs`, `crates/tungsten-render/src/tests/bloom.rs`, `crates/tungsten-render/src/tests/passes_order.rs`, `scripts/perf-rows.json`, the GPU counter pins `just perf counters` names as moved, `api/tungsten-render.txt`, `crates/tungsten-render/AGENTS.md`, `DESIGN.md`, `docs/perf/benchmarks.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`.
  - Step 4: `crates/tungsten-core/src/persist.rs`, `crates/tungsten-core/src/tests/persist.rs`, `crates/tungsten-core/src/save.rs`, `crates/tungsten-core/src/tests/save.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs`, `api/tungsten-core.txt`, `AGENTS.md`, `DESIGN.md`, `docs/getting-started.md`, `docs/perf/benchmarks.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`.
  - Step 5: this plan's evidence log alone (the full sitting).
  - Step 6: `crates/tungsten-core/src/physics/step/arrival.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/workers.rs`; and only if every gate passes: the physics hashes' pins (`crates/tungsten-core/tests/integration/`), the kit recipes' pin (`crates/tungsten/src/tests/kit_recipes/`), the guard pins a changed motion moves (`examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/*.json`), `DESIGN.md`, `docs/perf/benchmarks.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`. Always: `ROADMAP.md`'s Later line.
  - Step 7: this plan (deleted), `ROADMAP.md`, `docs/perf/benchmarks.md`, `docs/known-issues.md`.
  - Every step: this plan's evidence log.
- **ordered steps:** (1) the pool's workers spin for the next fork, and its sitting; (2) bloom's first level by the target's rows; (3) a vignette after bloom drawn in bloom's composite; (4) saves staged on the frame and committed on a writer thread; (5) the full sitting of Steps 1 to 4; (6) the wider arrival pass as a gated probe; (7) the close.
- **done-when:** every step's done-when passes or the step is recorded stopped, quoted in the evidence log; the close's full sitting against `3940a739` reads no owned metric `regressed` without a fix or a known-issues row; `just check` passes and example 01 runs; the owner has the bloom review sheet; the plan deleted with its citations and the item out of Now.

Item `engine-perf-proposals` in [ROADMAP.md](../../ROADMAP.md), off the roadmap by the owner's request of 2026-11-23 ("do all of them but the x11 one", of the engine performance pass's measured proposals). Written 2026-11-23 at `3940a739`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

- **The owner's decision (2026-11-23):** of the pass's five proposals awaiting a decision, land all but the compositor-bypass hint. The pass's exactness rule (every digest and pixel kept) is lifted for these four alone: pixels may change in bloom's glow, motion may change in the arrival pass. Steps 1 and 4 still keep every digest and pixel.
- **What the proposals measured** (the pass's Proposals table, `git show 548876f8:docs/plans/engine-perf-pass.md`): spin, `total` p50 / p95 / p99 −0.24 / −0.56 / −0.86 ms in `e01-pit` and −0.38 / −0.88 / −1.00 in `e01-stress` on the probe, three runs a side; bloom's first two levels and last two upsamples 2.27 ms of 9.2 at 2160p, the vignette's pass 1.69 ms at 2160p and 0.45 at 1080p; a save frame 7 to 16 ms and once 394 ms in `e01-play`, about 3.4 ms of it user CPU; the arrival pass's searches 3.4 to 3.6 ms of `e01-pit`'s 20 highest frames.
- **The arrival proposal's history** (`git show cb4b706b:docs/plans/cpu-stress-pass.md`, evidence row 6): on 2026-11-11 the variant that read every partner in reach changed motion, moved the containment hash and failed 01's `an_open_hole_sweeps_small_balls_from_its_core_a_few_a_step` (one ball left in reach); the variant held to the walk's own candidates and order kept every hash and gained nothing on the tail. Its speed-up came from clamping a listed body against partners whose approach the regular contacts already solve.
- **Stands on:** `D-298` (the pool), `D-060` (bloom), `D-087` (the last full-screen stage), `D-159` (atomic writes), `D-092` (the arrival pass), `D-226` (sittings). **Amends** `D-298` (Step 1), `D-060` (Steps 2 and 3), `D-220`'s thread list as `D-298` left it (Step 4), `D-092` (Step 6, only if it lands).
- **Evidence** is machine-local under `perf-runs/20261123-engine-perf-proposals/` (`README.md` is the log; `tools/` holds the readers, copied from `perf-runs/20261123-engine-perf/tools/`).
- **Later items expect** 7.16 to start from a tree whose rows' reference numbers and levers `docs/perf/benchmarks.md` states.

## 1. Audit

| # | Finding | Where |
| --- | --- | --- |
| A1 | A worker blocks in `recv` between forks; the caller spins `ANSWER_SPIN` for the answers, then blocks. The pool is remade when the participant count changes. | `step/pool.rs:219-232`, `:327-335`, `:363-379` |
| A2 | The probe's spinning pool polled `try_recv` under `spin_loop` until 3 ms had passed since the worker's last answer, and the caller waited for answers under the same patience. | `perf-runs/20261123-engine-perf/probe/apply_probe2.py:111-186` |
| A3 | `PhysicsWorkers` is `Copy`, read once a step by an out-of-line `read_workers`; `App::run` inserts it from `physics.workers`; `App::new`, the harness and the guards have none. | `physics/mod.rs:201`, `step/mod.rs:506`, `app.rs:1625` |
| A4 | Bloom's first level is half the target; the level count is capped by `render.bloom_max_mips` (6) and by size. The threshold pass reads the scene with one bilinear tap a texel, which covers a 2×2 footprint and no more. | `targets.rs:70-76`, `:625-668`; `shaders/stock/bloom_threshold.wgsl` |
| A5 | Upsample offsets are in source texels, so the glow's width follows the pyramid's size: at 2160p today's glow is half as wide, as a share of the screen, as at 1080p. | `shaders/stock/bloom_upsample.wgsl` |
| A6 | `PostPass::Bloom` and `PostPass::Vignette` are typed passes; 01's stack is bloom then vignette. The composite writes `mix(scene, scene + bloom · intensity, radius)` to an 8-bit sRGB target, which clamps it to 1; the vignette then reads it and writes `mix(rgb, color, t · strength)`. | `tungsten-core/src/post.rs:355-407`; 01's `features/level/mod.rs:343-356`; `bloom_composite.wgsl`, `vignette.wgsl` |
| A7 | Seven rows own `gpu.post1_vignette` and `gpu.post0_bloom_composite` (`gpu`, six `e01-area-*`); the fold reaches the six area rows (A14). A metric one side lacks reads no verdict, and a row reads `regressed` only on a metric both pairs read so. | `scripts/perf-rows.json`; `scripts/perf_stats.py:318-354`, `scripts/perf.py:739-758` |
| A8 | `just visual` compares its captures with a tolerance of 2 a channel; `tools/stills.py` compares 01's stills by bytes: six area stills at 3840×2160 and three (`pit`, `stress`, `play`) at 1920×1080. | `examples/02_bench/tests/visual_regression.rs:87`; `tools/stills.py` |
| A9 | `SaveSlots` is `Clone`, holds only the folder, and every method takes `&self`. `save` serializes and calls `write_atomic`: folder, temp file, `write_all`, `sync_all`, `rename` (retried on Windows), the folder's `fsync`. | `save.rs:153-156`, `:325-348`; `persist.rs:55-135` |
| A10 | The shells set `retry_from` and `checkpointed` when `save` returns `Ok` and only log a failure; they read slots through `list` and `load` on the World's `SaveSlots`, and their tests through a second `SaveSlots::new` on the same folder. The template's shell and 01's are byte copies. | `examples/01_platformer/src/shell/mod.rs:406-440`, `:521-540`; `templates/basic/src/shell/mod.rs` |
| A11 | The arrival search takes a listed body's candidates from the pair grid's walk of its box grown by its own travel, then cuts by the relative velocity. A partner outside that box that approaches by its own velocity passes the cut. | `step/arrival.rs:107-161` |
| A13 | A game may replace a stock post shader, the vignette's and bloom's composite among them: the loader rebuilds that effect's pipeline from the manifest's file at startup when its bytes differ from the compiled-in source, and on a reload. | `post/mod.rs:22-29`; `renderer.rs:1016-1046` (`D-091`) |
| A14 | Example 01 runs no SMAA (its area rows record `scene`, bloom's passes, `post1_vignette`, `text`); the `gpu` row runs `smaa_high` after bloom and the vignette, at 1920×1080. Every bench knob enters a capture's config, and a knob one side lacks is a hard mismatch. | the close sitting's captures; `examples/02_bench/src/knobs.rs:352`; `scripts/perf_stats.py:424-427` |
| A12 | AGENTS.md is 6,143 of 6,144 bytes and the renderer's 4,094 of 4,096; each needs trims for a new clause. | `wc -c` |

## 2. Design

### 2.1 How a step is judged

As the pass was (`perf-runs/20261123-engine-perf/README.md`): every sitting takes `--base 3940a739`; Δ(sitting) is the mean over the two judged pairs of candidate − base; a step's own move is Δ(its sitting) − Δ(the sitting before it), by `tools/step_delta.py`. Every step keeps every digest (`tools/judged.py`) until Step 6. Stills (`tools/stills.py`, by bytes) compare a step's binary with its predecessor's, each kept aside under `target/engine-perf2/bin/<step>/` when its step's checks pass: Step 1 against the base's (nine of nine equal), Step 2 by §2.3, Step 3 by §2.4, Step 4 against Step 3's (nine of nine equal). Nothing runs beside a capture or a sitting.

Two sittings before the arrival probe: Step 1's (`--cpu-only`), which reads the spin alone, and Step 5's full one, which reads Steps 2 to 4 on metrics the spin does not touch (`gpu.*`, `stage.render`, the save frames). Step 5's is the close's unless Step 6 lands.

**Timed checks wait for the owner's client (added at Step 1's commit).** The runner refuses a capture beside the remote-desktop encoder, and the owner was connected while the steps were written. So each of Steps 1 to 4 commits on its untimed checks (tests, `just check`, the pixel checks, the counters), and the timed ones run in one batch once the client disconnects: Step 1's sitting on an export of its own commit, which reads the spin alone (`perf-runs/20261101T195322Z-sitting/sit_export.py`, its builds its own), then Steps 2 and 3's by-hand readings on their kept binaries, Step 4's `e01-play` capture, and Step 5. The evidence log marks each timed check not run until it has.

A committed step whose later gate fails (a sitting's, Step 5's) is taken out by a commit of its own that restores that step's files from the commit before it (`git show <commit>^:<path> > <path>`), its decision marked superseded by a new entry.

### 2.2 Spin (Step 1)

- `PhysicsWorkers` gains a spin time: `with_spin(Duration)` and `spin()`; its constructors park. `physics.worker_spin_ms` (0 to 50, default 3) is what `App::run` passes; 0 parks at once.
- A worker that has answered polls its channel under `std::hint::spin_loop` until the spin time has passed since its answer or the step has ended, then blocks in `recv`. The step's end clears one shared flag (`AtomicBool`) through an out-of-line call; a fork sets it. So the workers are busy between a step's forks and for no longer than the step: a step that forks once does not buy 3 ms of five cores after it. The caller's wait for the answers spins for the larger of the spin time and `ANSWER_SPIN`.
- The pool is remade when the spin time changes, as when the count does. Chunk claims, scratches and the merge are untouched, so every count stays bit-identical; the lockstep tests run again with spin on.
- No environment variable: a capture records `physics_workers_env` alone, and a second variable would let two captures differ unseen.

### 2.3 Bloom's first level (Step 2)

- The first level is the target halved once while the target has at most 1440 rows, twice above (`auto`, the default): 1080 → 540 rows, 1440 → 720, 2160 → 540. `render.bloom_base` takes `auto`, `half` (today's pyramid at any size) or `quarter` (at any size); `TUNGSTEN_RENDER_BLOOM_BASE` overrides it, as its sibling key's variable does. Startup-only, like the level count. The bench gains no knob: a knob the base lacks is a hard mismatch in a comparison (A14), so the pixel test and the by-hand readings set the variable.
- The level cap counts from the first level, so at 2160p the default six levels are 1080p's own (960×540 down to 30×16) and the glow keeps the share of the screen it has at 1080p. The size limit follows the first level.
- The threshold pass reads a quarter-size texel's 4×4 footprint with four bilinear taps at ±1 source texel; at half it keeps its one tap, by a flag in its uniform block, so a target of 1440 rows or fewer draws the pixels it draws today.
- Pixels change only where the base is a quarter: a softer, wider glow at 2160p. Under `half` the tree draws the base's nine stills byte for byte; under `auto` the three 1080p stills stay equal and the six 2160p ones are the owner's review, a contact sheet of `half` beside `auto`.

### 2.4 The vignette in bloom's composite (Step 3)

- A `Vignette` is drawn by bloom's composite when all four hold: it directly follows a `Bloom`; it is the stack's last pass; no SMAA tail follows the stack; and both the vignette's shader and bloom's composite are the compiled-in sources, neither replaced at startup or by a reload (A13). The composite then writes `mix(clamp(mixed, 0, 1), color, t · strength)`, the clamp standing in for the 8-bit store the fold removes, and the stack has one slot fewer: no pass, target or timing label for that vignette. Any other stack is drawn as today, so a game's own vignette shader, a pass after the vignette (a dither's or an outline's threshold could turn one step into more) and SMAA's edge test all read what they read today.
- A replaced module turns the fold off for the rest of the run: the renderer records the replacement where it rebuilds the pipeline, and the next frame plans the unfolded stack.
- The folded composite carries its own timing label, `post<N>_bloom_composite_vignette`, so `gpu.post0_bloom_composite` does not read `regressed` for holding the vignette's work; the six `e01-area-*` rows own the new label in place of the two old ones, and the `gpu` row, under SMAA, keeps its own. `gpu.render_span` is the judge.
- The fold is decided once a frame, before the pass order, the targets, the timing table and the counters read the stack; a stack that does not fold takes today's path untouched.
- Pixels: a folded frame differs from today's by at most one step a channel (one 8-bit rounding fewer, nothing after it to amplify it); a frame that does not fold is byte-equal.

### 2.5 Saves written behind (Step 4)

- `persist` splits its write: `stage` makes the folder, creates the temp file and writes it; `commit` syncs it, renames it over the slot (the Windows retries) and syncs the folder. `write_atomic` is the two in a row, unchanged in what it does.
- `SaveSlots::write_behind()` turns the mode on; `App::run` calls it for a windowed run, and `App::new`, the harness, `tungsten check` and every test keep the synchronous write. In the mode `save` serializes and stages on the frame, so `NoFolder`, `InvalidName`, `Serialize` and every folder, create or write failure (permissions, a full disk) still return at once; the commit runs on one writer thread (`tungsten-saves`), started at the first save, falling back to the synchronous write if it cannot start.
- Reads see a save at once: while a slot's commit is pending, `load` and `list` read its staged file; `delete` cancels it. The writer syncs outside the lock a read takes. Each rename attempt takes the lock, checks that its save is still the slot's newest and not cancelled, tries once and drops the pending entry on success; a Windows sharing failure releases the lock before its backoff sleep, so a read never waits on a retry. A newer save of a slot supersedes an older pending one.
- A commit that fails is logged at error level, leaves the old file, and is returned by `SaveSlots::flush()`, which waits for the queue; `App::run` flushes at exit, and dropping the last `SaveSlots` joins the writer. A crash loses at most the saves whose commits were pending, each leaving its old file whole.
- The save file's bytes do not change.

### 2.6 The wider arrival pass (Step 6)

The first round's bodies within their pair budgets read their candidates from the pair list's partners, taken by a counting sort when one body in 256 pairs is listed, and go through today's cut; the rest walk the grid. This is 2026-11-11's first variant, rebuilt on the pooled tree: its patch survives only as a summary. By A11 it clamps a listed body against partners the regular contacts already solve, which is what failed 01's sweep test then. The step is therefore a probe behind gates in a fixed order (01's tests unedited, the tunnelling and containment oracles, core's tests, then the sitting), and it lands only if every gate passes.

## 3. Steps

### Step 1: the pool's workers spin for the next fork

- **Files:** as listed for Step 1.
- **Change:** §2.2; the lockstep tests under spin; the step's decision amending `D-298`, with its index row and `D-298`'s marker line; the key in the getting-started guide; DESIGN.md's pool paragraph; ROADMAP.md's Later line removed; benchmarks.md's lever line; `status: in progress`.
- **Done-when:**
  - `cargo test -p tungsten-core --lib pool::tests` → exit 0: the three lockstep scenes bit-equal at 2, 3, 4, 6 and 12 participants with a spin of 1 ms; a worker inside its spin time takes the next fork without blocking, and one past it or past the step's end blocks (test-only counts of each); a changed spin time remakes the pool.
  - `cargo test -p tungsten-core --lib config` → exit 0: the key's default 3, 0 accepted, 51 refused. `cargo test -p tungsten --lib app` → exit 0: a windowed run's `PhysicsWorkers` carries the key's time.
  - `just check` → exit 0, no guard pin rewritten; `just api-check` → exit 0; `just repo-check` → exit 0.
  - `nm -S` of the release binary: `physics_step`'s size within 64 bytes of the base's, and `build_pairs`, `arrival_pass` and `substep_rest` equal to the base's.
  - The sitting (`WGPU_BACKEND=vulkan just perf sitting --cpu-only --base 3940a739`) → exit 0; `judged.py` → exit 0 (every pair's digests equal); `stills.py` → `9 of 9 equal`.
  - `step_delta.py`: `stage.total` p50 falls in both pairs of `e01-pit`, `e01-stress` and `blast` (six readings of six), and the mean p95 falls by at least 0.25 ms in `e01-pit` and 0.4 in `e01-stress` (floors under the probe's −0.56 and −0.88); `e01-play`'s `system.physics_step` p50 moves by less than 0.02 ms.
- **Commit:** `perf(physics): spin the pool's workers for a step's next fork`
- **Moves:** nothing pinned: the harness has no workers.

### Step 2: bloom's first level by the target's rows

- **Files:** as listed for Step 2.
- **Change:** §2.3; `render.bloom_base` and its variable; one pixel test of the forced quarter with its new reference image; the step's decision amending `D-060`; the renderer's AGENTS.md bullet within its byte cap; DESIGN.md; the guide's key.
- **Done-when:**
  - `cargo test -p tungsten-render` → exit 0: the first level and level sizes at 720, 1080, 1440, 1441 and 2160 rows under `auto`, `half` and `quarter`; the level cap against a quarter's size limit; the threshold's flag packed; the shader mirrors byte-equal and valid.
  - `just visual` → exit 0 with no existing reference image changed (`git status` names one new file under `examples/02_bench/tests/fixtures/`).
  - `TUNGSTEN_RENDER_BLOOM_BASE=half` and `stills.py` against the base's binary → `9 of 9 equal`: `half` is today's pyramid. Then without the variable → the three 1080p stills equal, the six 2160p ones differing.
  - `just smoke` → exit 0; `just perf counters` → exit 0, no pin moved; `just perf-test` → exit 0.
  - By hand at 2160p, alone, two areas, two runs a side (`tools/proposals.py bloom`, extended to the variable, `half` against `auto` on the tree's binary): `gpu.render_span` p50 falls by at least 1.0 ms in both areas (estimate 1.7).
  - The review sheet exists: 01's six areas at 2160p, `half` beside `auto`, with each pair's PSNR, under `perf-runs/20261123-engine-perf-proposals/bloom-review/`.
- **Commit:** `perf(render): start bloom's pyramid at a quarter above 1440 rows`
- **Moves:** pixels above 1440 rows; nothing pinned.

### Step 3: a vignette after bloom drawn in bloom's composite

- **Files:** as listed for Step 3.
- **Change:** §2.4; `scripts/perf-rows.json`'s six area rows; the counter pins; the step's decision amending `D-060`; the renderer's AGENTS.md; DESIGN.md.
- **Done-when:**
  - `cargo test -p tungsten-render` → exit 0: `[Bloom, Vignette]` without SMAA plans one slot with the labels `post0_bloom_threshold` and `post0_bloom_composite_vignette`; the same stack under SMAA, `[Vignette, Bloom]`, `[Bloom, Tonemap, Vignette]`, `[Bloom, Vignette, Dither]`, `[Bloom, Vignette, Vignette]` and a lone `Vignette` plan as today; a replaced vignette module, and a replaced composite module, each plan `[Bloom, Vignette]` as today from then on; the composite's block carries the vignette's values when it folds; the timing table's pass count matches the recorded passes for each stack.
  - `just visual` twice, its captures kept by `TMPDIR` before the step's edits and after: every capture of a frame that does not fold byte-equal (`cmp`); every capture of a folded frame within one step a channel (`tools/visual_diff.py` prints the largest difference per file); `just visual` itself → exit 0 with no reference image replaced.
  - `stills.py` against Step 2's binary, read by `visual_diff.py`: every one of the nine stills within one step a channel.
  - By hand, a game's own vignette: with `assets/shaders/stock/vignette.wgsl` edited to tint red (the file restored after), a bench capture of bloom then vignette shows the tint, at startup and after a reload of the edit; the capture's path in the log.
  - `just perf counters` → the rows whose frame folds read one pass and one draw fewer a frame, their pins rewritten and the write run twice; no other pin moves. `just smoke` → exit 0. `just perf-test` → exit 0.
  - By hand as in Step 2, Step 2's binary against the tree's: `gpu.render_span` p50 falls by at least 1.0 ms at 2160p in both areas (bound 1.69).
- **Commit:** `perf(render): draw a vignette after bloom in bloom's composite`
- **Moves:** a folded frame's pixels by at most one step a channel; the folded rows' pass and draw counts by one; two timing labels become one.

### Step 4: saves staged on the frame, committed on a writer thread

- **Files:** as listed for Step 4.
- **Change:** §2.5; the step's decision amending `D-220`'s thread list (as `D-298` left it) and `D-159`; AGENTS.md's thread rule within its byte cap; DESIGN.md's threads and saves text; the guide's saves text; ROADMAP.md's Later line removed.
- **Done-when:**
  - `cargo test -p tungsten-core --lib persist` → exit 0: `stage` then `commit` leaves the bytes `write_atomic` leaves, through the same injected failures (a failed write, a failed rename, the Windows retries); a staged file dropped uncommitted leaves no temp file.
  - `cargo test -p tungsten-core --lib save` → exit 0, the writer held by a test gate: a `save` returns before its commit and `load` and `list` already read it; a failed stage returns its error and queues nothing; a failed commit keeps the old file, removes the temp file and is `flush`'s error; `delete` cancels a pending commit; of two saves of one slot the newer stands; a `load` returns while a rename's retry sleeps; dropping the slots commits what is queued; a writer that cannot start writes synchronously. Each test's negative control named in the log.
  - `cargo test -p tungsten --lib app` → exit 0: a windowed run's `SaveSlots` writes behind, and `App::new`'s does not.
  - `just check` → exit 0 with no test of example 01 or the template edited and no guard pin rewritten; `just api-check` → exit 0; `just repo-check` → exit 0.
  - `WGPU_BACKEND=vulkan just perf run e01-play` → valid, its digest the base's in Step 1's sitting; `stills.py` against Step 3's binary → `9 of 9 equal`.
  - `tools/save_frames.py` on that capture against the base's `e01-play` captures of Step 1's sitting: in every run of the tree each of the five entry frames reads `system.shell_save` under 6 ms (base 7 to 16), and no frame reads `total` over 100 ms.
- **Commit:** `perf(saves): stage a save on the frame and commit it on a writer thread`
- **Moves:** the thread rule, by the decision. No pin: the harness writes synchronously.

### Step 5: the full sitting of Steps 1 to 4

- **Files:** this plan's evidence log.
- **Change:** `WGPU_BACKEND=vulkan just perf sitting --base 3940a739`, then `judged.py`, `step_delta.py` and `save_frames.py` on it.
- **Done-when:** the sitting → no row `regressed` on an owned metric; `judged.py` → exit 0, every pair's digests equal; `gpu.render_span` p50 `improved` in both pairs in at least five of the six `e01-area-*` rows if Steps 2 and 3 landed; `save_frames.py` → Step 4's floors on the sitting's own `e01-play` captures.
- **A failed check:** the step whose metric it is gets its one revision and the rows are sat again (`--rows`); failing again, it is taken out by §2.1's commit.
- **Commit:** `docs(plans): record the proposals' full sitting`

### Step 6: the wider arrival pass, a gated probe

- **Files:** as listed for Step 6.
- **Change:** §2.6, uncommitted until every gate passes.
- **Gates, in order; the first that fails stops the step:**
  1. `cargo test -p example-01-platformer --locked` → exit 0 with no test edited, `an_open_hole_sweeps_small_balls_from_its_core_a_few_a_step` among them.
  2. `just physics-release` → every tunnelling case passes unedited and containment reads `escaped` 0; its two hashes move and are re-pinned, old and new in the decision.
  3. `cargo test -p tungsten-core --locked` → exit 0, no oracle entry added; the lockstep tests bit-equal at every count; the kit recipes' two-run assertion holds.
  4. The sitting (`--cpu-only`) → no owned metric `regressed`; `step_delta.py`: `e01-pit`'s `stage.total` p95 falls by at least 1.0 ms in both pairs against Step 5's sitting, and `physics`' `system.physics_step` p50 does not rise by more than 0.1 ms (2026-11-11's first variant cost it 0.7).
- **If every gate passes:** the step's decision amending `D-092`, the pins re-recorded with their causes, commit `perf(physics): read the arrival pass's candidates from the pair list`.
- **If a gate fails:** the files restored from the last commit, the failure quoted in the evidence log, and ROADMAP.md's Later line rewritten with today's finding in a commit of its own (`docs(roadmap): …`). No test or oracle is edited to pass.

### Step 7: the close

- **Files:** as listed for Step 7.
- **Change:** a full sitting against `3940a739` again only if Step 6 landed (Step 5's stands otherwise); benchmarks.md's row sections and levers; ROADMAP.md's Later lines for what landed and what stopped, the item out of Now; known issues for an accepted regression; the plan deleted.
- **Done-when:** the standing full sitting reads no row `regressed` on an owned metric without a fix or a known-issues row; `just check`, `just repo-check`, `just api-check` → exit 0; `just template-check` → exit 0 if Step 4 landed.
- **Commit:** `docs(plans): close the engine perf pass's proposals`

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Whether the spin is on by default. | On at 3 ms, since the owner asked for the gain. If Step 1's sitting does not show its floors, the key stays and its default becomes 0, recorded as not shown. |
| Q2 | The glow's look at 2160p under a quarter base. | `auto` lands as the default and the sheet goes to the owner; `render.bloom_base: "half"` restores today's look. |
| Q3 | Whether the vignette fold is in the owner's "all of them". | Yes: it shares ROADMAP.md's Later line with the quarter base; the report's list left it out, and the owner was told so on 2026-11-23. Its own commit, so it reverts alone. |
| Q4 | What a game learns of a failed commit. | The log line and `flush()`'s error; the shells stay as they are. A shell that wants a manual save confirmed calls `flush()` after it. |
| Q5 | A step's own move reads inside the noise of a sitting. | Steps 2 to 4: the step keeps its commits when nothing reads `regressed` and its by-hand floor holds. Step 1: Q1. |

## 5. Invariants

- **May move:** bloom's pixels above 1440 rows (Step 2); a folded frame's pixels by one step a channel, its pass and draw counts by one and its timing labels (Step 3); the thread rule (Step 4); motion, the physics hashes and the pins they move (Step 6, only if it lands).
- **Must not move:** every row's digest through Step 5; Step 1's and Step 4's stills against their predecessors'; a frame of at most 1440 rows that does not fold, byte for byte, and every frame under `render.bloom_base: "half"` that does not fold; what a game's own vignette or composite shader draws; a save file's bytes and the atomic write's promise (the old file whole until a synced temp file replaces it); what the harness, `tungsten check`, the tests and the counter guards do (no workers, no writer); example 01's and the template's sources and tests; the public behaviour of `write_atomic`.

## 6. Stop conditions

- A done-when check fails: one revision, then the step's files are restored from the last commit, the step is marked stopped with the failure quoted, and the next step runs. No step depends on another: with Step 2 stopped, Step 3's stills and by-hand floor are read against Step 1's binary, and Step 4's against the last step that landed.
- Steps 1 or 4 move a digest or a still against its predecessor: the step stops.
- A sitting reads a row `unresolved` or `invalid`: it is taken once more for those rows; a second failure stops the step. `spread`'s `system.effect_tick` reading `regressed` in a step that changes no kit code is retaken once alone (known issues' row).
- Step 6: its gates, as written.
- The meter's `chain:` line (`python3 -B perf-runs/713-chain/spend.py --since 2026-11-23T08:00:00Z`) reads 35 %: stop, record what is left.

## 7. Critique

gpt-6-astra through the critique skill, 2026-11-23, the working tree at `3940a739` with this plan uncommitted, round 1 of 1: the pass's request named one round, so the changed checks and step order below went unreviewed. Footer: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 331.7 s, tokens in 947019 (cached 851840), out 10428 (reasoning 4907), commands 34, output 410.6 KB (largest 53.3 KB), usage 5h 0% → 23%, week 5% → 8%`.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | HIGH: folding every `[Bloom, Vignette]` pair bypasses a game's own vignette shader, replaced at startup or by a reload. | `post/mod.rs:22-29` (`STOCK_SHADERS`), `renderer.rs:1016-1046` (`rebuild_stock_with_module`, `D-091`) | Right | A13, §2.4: the fold needs both shaders compiled-in, a replacement turns it off for the run; Step 3 tests each and shows a tinted vignette by hand. |
| 2 | HIGH: a new bench knob is a hard mismatch against the fixed base, so the `gpu` row loses its verdicts. | `examples/02_bench/src/knobs.rs:352`, `scripts/perf_stats.py:424-427` | Right | A14, §2.3: no knob; the variable alone. `examples/02_bench/src/gpu.rs` leaves Step 2's files. |
| 3 | HIGH: Step 2's stills gate reads six 2160p captures, the pixels it changes. | `tools/stills.py`'s scenes; an area row's `gpu-script.json` (3840×2160) | Right (found while the round ran, too) | A8, §2.1, §2.3: `half` reproduces the base's nine; `auto` keeps the three 1080p ones; each later step compares with its predecessor's binary. |
| 4 | MEDIUM: Step 4's digest and save-frame gates sat after the arrival probe, which may change motion, and a committed step cannot be restored from the last commit. | Step 4, Step 6, §6 as written | Right | The full sitting is Step 5, before the probe; Step 4 reads its own `e01-play` capture; §2.1 says how a committed step is taken out. |
| 5 | MEDIUM: renaming under the reads' lock carries the Windows retry sleeps (20 to 160 ms) onto the frame. | `persist.rs:113-131`, `:143-146` | Right | §2.5: one attempt a lock hold, the sleep outside it, the save revalidated each attempt; Step 4 tests a read during a retry. |
| 6 | MEDIUM: one step at the fold does not bound the final image when a pass after it thresholds (a dither's `floor`). | `shaders/stock/dither.wgsl:49`; SMAA's edge test by the same argument | Right | §2.4: the fold needs the vignette last and no SMAA tail, so nothing after it amplifies; the `gpu` row, under SMAA, is unchanged (A14). |

## Evidence log

| Step | Date | State | Evidence |
| --- | --- | --- | --- |
| 1 | 2026-11-23 | landed on its untimed checks; its sitting, `nm -S` and stills not run yet (the encoder running) | `cargo test -p tungsten-core --lib pool::tests` → 14 passed, each lockstep scene at 2, 3, 4, 6 and 12 participants parked and spinning 1 ms; four negative controls each failed their test and were restored (no `rest_workers` call: `the_step_rests_its_workers_when_its_last_fork_has_run`; a worker that ignores the step's end, and one that never spins: `a_spinning_worker_takes_its_steps_next_fork_and_parks_when_the_step_ends`; a pool not remade on a new spin: `a_changed_spin_remakes_the_pool`). `just check` → exit 0, no guard pin rewritten; `just api-check` → exit 0; `just repo-check` → exit 0. `Fork` and `PhysicsWorkers` keep their sizes: the spin time sits in their padding. One call added to `physics_step`, out of line, after the last substep. |

## Follow-ups

None yet.
