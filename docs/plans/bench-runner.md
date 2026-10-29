# Benchmarks, part 1: runner and guards (bench-runner)

- **status:** in progress
- **goal:** Redo step 7.3 (§5.8's part 1). Rewrite the runner with sittings built in: a same-sitting A/A, the one-rerun way out, aligned builds against code-placement noise, a contamination check at the start, RSS compared within a sitting only, budget verdicts and a quick mode. Read GPU timestamps back without stalling the frame. Add CPU counter guards to `just check` and CI, and GPU draw-call and upload-byte counters checked locally. Port a minimal row set and add 01's full pit, the template frame and 01's areas. Purge the old suite and keep its carry-overs, rewrite `tungsten-perf`, and replace per-milestone sittings with the guards and a sitting per minor or major release.
- **non-goals:**
  - Budgets: the mechanism lands here, but no row carries a budget until 9.1 sets them. Expansion rows wait for 9.1; system rows land with their own steps from 7.8 on (§5.8).
  - Fixing any cost the rows show: that is 7.14's, 7.15's and 9.2's work.
  - Criterion micro-benchmarks (`D-037`), which stay as they are.
  - H2's release script (7.4).
  - Merging the GPU run into the timing run (Q5).
  - Deleting machine-local captures (Q9).
  - A template counter guard (Q7), capacity search and sweeps (Q8).
- **files to touch:**
  - Step 1: `crates/tungsten-core/src/ecs/{world.rs,archetype.rs,command_buffer.rs,mod.rs}` and their unit tests, `crates/tungsten/src/testing.rs`, `crates/tungsten/src/testing/guard.rs` (new), `api/`, `DESIGN.md`.
  - Step 2: `examples/02_bench/{Cargo.toml,src/main.rs,src/gen.rs,src/physics.rs,src/ecs.rs,src/churn.rs,src/particles.rs,src/gpu.rs,src/gpu/scene.rs,src/tests/mod.rs,src/tests/guards.rs,src/tests/guards/}` (one pin file per guard); `examples/01_platformer/src/features/run/{mod.rs,script.rs,pit.rs}`, `examples/01_platformer/src/features/level/mod.rs`, `examples/01_platformer/tests/game/{main.rs,frame_time.rs,guards.rs,guards/}`.
  - Step 3: `scripts/{perf.py,perf_stats.py,perf-rows.json,test-perf.py}` (new), `justfile`.
  - Step 4: `crates/tungsten-render/src/{timing.rs,renderer.rs,counters.rs}` (`counters.rs` is new) and every render file that uploads or draws (step 4 lists them from its baseline `rg`), `crates/tungsten-render/src/text/gpu.rs`, `crates/tungsten/src/{telemetry.rs,app.rs}`, `scripts/{perf.py,perf_stats.py,test-perf.py}` (the `from` attribution), `api/`, `DESIGN.md`, `docs/perf/profiling-workflow.md` (Telemetry lines), `docs/known-issues.md`.
  - Step 5: `scripts/{perf.py,perf_stats.py,perf-rows.json,test-perf.py}`.
  - Step 6: `justfile`; `scripts/{bench.py,bench_report.py,test-bench.py}` (deleted); `scripts/{smoke-examples.sh,test-smoke-examples.sh}`; `examples/02_bench/src/{main.rs,knobs.rs,physics.rs,ecs.rs,churn.rs,gpu.rs,particles.rs,integrated.rs,integrated/,tests/knobs.rs}` and `examples/02_bench/assets/manifest.json`; `crates/tungsten/src/app.rs` (comments only); `docs/perf/{benchmarks.md,profiling-workflow.md}`, `docs/plans/roadmap-redo.md` (9.2's row), `README.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`; `.claude/skills/{tungsten-perf,tungsten-wgpu,tungsten-next,tungsten-milestone}/SKILL.md`; `examples/03_fixtures/src/lib.rs` (comment only).
  - Step 7: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `.claude/skills/{tungsten-milestone,tungsten-perf}/SKILL.md`, `docs/{validation.md,releases.md,operating-rules.md}`, `docs/plans/README.md`, `docs/perf/profiling-workflow.md`, `scripts/{check-repo.py,test-check-repo.py}`.
  - Step 8: this plan's evidence log, plus `scripts/perf-rows.json` only when a row gains a known-drift metric.
  - Step 9: this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md` (Step 2's evidence row), `docs/known-issues.md`.
- **ordered steps:**
  1. CPU counters in core and the harness.
  2. CPU counter guards and 01's pit actions.
  3. The runner: rows, run and compare, beside the old one.
  4. GPU timestamps without stalling, and GPU counters.
  5. Sittings, quick mode and the counters check.
  6. Switch and purge.
  7. Per-milestone sittings retired, and the decision.
  8. The first sitting and provisional baselines.
  9. Close.
- **done-when:**
  - Every step's done-when passes and is quoted in the evidence log.
  - `just perf list` lists every §2.1 row, which covers every row phase-7's Step 2 names.
  - The counter guards pass in `just check`.
  - `just perf counters` passes locally.
  - Step 8's sitting reads every row `ok`, and the provisional baselines are saved.
  - The old runner and its data are gone, and the carry-overs are in benchmarks.md's lever list.
  - The tests pass, and example 01 runs with its pit row.
  - The plan is deleted with its citations, `bench-runner` is out of Now, and phase-7's Step 2 has its evidence row.

Item `phase-7`'s step 7.3 in [ROADMAP.md](../../ROADMAP.md), from [phase-7.md](phase-7.md)'s Step 2 and §2.8 and the [redo](roadmap-redo.md)'s 7.3 row, §5.8 and Q12. Written 2026-10-29 at `2464cbc9`. It runs under the [tungsten-milestone skill](../../.claude/skills/tungsten-milestone/SKILL.md) and the [operating rules](../operating-rules.md). It differs from the skill in one way: its own perf evidence is step 8's sitting, not a capture per step, which is the policy step 7 writes down.

## Context digest

- **Why now.** 7.3 heads Phase 7's main track (phase-7 §2.8). 7.8–7.13 then rebuild physics, saves, the kit and 01, so the guards have to exist before that churn starts (§5.8). 7.2 has landed `tungsten run --script` (`D-225`), and the game rows here are scripts on it.
- **What phase-7 asks of it** (Step 2):
  - A row set that measures every item on 7.14's and 7.15's lists (phase-7 §2.5): 01's full pit, the template frame, `ecs`, `particles`, `physics-sparse`, `gpu`, `gpu-throughput`, and a local GPU row of 01's areas (Q5 there).
  - Upload-byte and draw-call counters, checked locally.
  - The runner has to list each of these rows.
  - It retires per-milestone sittings (R10).
- **Later steps lean on it:**
  - From 7.8 on, each system step adds a row and a counter guard (phase-7 §3).
  - 7.14 and 7.15 take their gates' numbers on these rows.
  - The physics hashes move only in 7.8, under these guards.
- **Decisions cited:**
  - `D-078`, the suite, its stdlib runner, verdict rule and owned metrics; this plan amends it.
  - `D-095` and `D-141`, background load.
  - `D-070`, CPU-only informational CI.
  - `D-110`, the harness.
  - `D-037`, criterion.
  - `D-225`, scripted runs.
  - `D-135`, one move per bundle.
  - `D-196`, the archive's `Benchmark` launcher, which stays.
  - The new entry is `D-22[6]` (step 7, §2.8).
- **Not readable here.** The rewrite's §2.2, which seeded §5.8, sits in the plans archive, which agents don't read. §5.8 and Q12 carry what it adopted.

## 1. Audit

Read at `2464cbc9`. Here `B`, `R` and `T` are `scripts/bench.py` (1,513 lines), `scripts/bench_report.py` (2,204) and `scripts/test-bench.py` (1,161).

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The runner offers `describe`, `run` (with `--sweep`, `--profile`, `--gpu-timing`), `suite`, `compare`, `capacity` and `baseline`, all in stdlib Python | B:1429-1509 |
| A2 | Rows come from the bench binary's `describe` (`TUNGSTEN_BENCH_DESCRIBE`). There are eight: `physics`, `physics-sparse`, `ecs`, `churn`, `gpu`, `gpu-throughput`, `particles` and `integrated`, each with owned metrics and guards in Rust and at `workload_version` 1. The rows are selected by `TUNGSTEN_BENCH`/`_PRESET` and captured under `TUNGSTEN_SMOKE_FRAMES`, which pins dt | `examples/02_bench/src/knobs.rs:256-289`, `:205-228`, `:535-571`; `physics.rs:101`, `:128`; `ecs.rs:66`; `churn.rs:76`; `gpu.rs:157`, `:180`; `particles.rs:91`; `integrated.rs:138`; `main.rs:100`; B:280-287 |
| A3 | Verdicts: a Welch 95 % interval on per-run means with variance floored at 0.5 %; τ_rel is 3 / 5 / 8 % (p50 / p95 / p99); τ_abs is 0.05 ms for stages and 0.02 ms for systems and passes; RSS uses 2 % or 2 MiB; at least 3 runs per side. The digest is the sha256 of the `physics:` and `bench:` lines | R:637-716, R:292-302 |
| A4 | None of these exists: a same-sitting A/A, a rerun way out, placement handling or a short mode. Budgets exist only inside `capacity`, and RSS gets a verdict across any two captures | R:644 (a comment only); B:1093-1111; R:789; B:1114-1125; R:911-917 |
| A5 | The preflight refuses `nxcodec.bin` and foreign `cargo` or `rustc` before a build (`D-141`). Measured runs are scanned once a second, and tree edits invalidate a capture (`D-095`). Neither catches another agent session or a busy process such as an editor's watcher | B:238-245, :632, :960; B:313-338; B:50-51 |
| A6 | GPU metrics come from a separate diagnostic run with `TUNGSTEN_GPU_TIMING=1`, which is on for `gpu` and `integrated` | B:286-287, B:565-575; `gpu.rs:38`, `integrated.rs:55` |
| A7 | A timed frame blocks on `device.poll(wait_indefinitely())`, builds a new `TimingResources` (query set and two buffers) every frame, and has one readback buffer with no ring. wgpu is 30.0.1 | `crates/tungsten-render/src/timing.rs:161`, `:72-78`; `renderer.rs:1411`, `:1428-1437`; `Cargo.lock:3418`; known issues' Render F11 (`docs/known-issues.md:26`) |
| A8 | Nothing counts draw calls or upload bytes. Queue uploads sit across `sprite.rs` and the other pipelines. `RenderCounts` carries `sprite_instances` but no batch count | `crates/tungsten-render/src/sprite.rs:531`, `:609`, `:703`; `crates/tungsten/src/telemetry.rs:100-130`; `app.rs:1630` |
| A9 | The ECS counts no archetype moves, spawns or despawns, and nothing sums sent events. No `GlobalAlloc` exists anywhere in the repository | `crates/tungsten-core/src/ecs/world.rs:200-266`, `archetype.rs:297`, `event_queue.rs:122`, `world.rs:117-125` |
| A10 | The harness has no renderer. `step` clones the extract into `FrameDraw`, whose `sprites` batches hold their instances, so a count read through it allocates | `crates/tungsten/src/testing.rs:2`, `:152-178`, `:331-390` |
| A11 | The bench's `bench:` lines come from per-bench counters, and its guards are runner-checked counter expressions | `examples/02_bench/src/counters.rs:40-53`; `knobs.rs:205-228` |
| A12 | 01's full-pit test is ignored and release-only. It pours 12,000 balls from `tools/level.json`'s pit, settles them for 600 frames and wakes six enemies, then times five windows of 300 frames. It asserts state but sets no time threshold | `examples/01_platformer/tests/game/frame_time.rs:81-211`; `docs/known-issues.md:39` |
| A13 | 01's script hooks are the actions `start_run` and `place_player` and the probes `player` and `area`. The template's are `start_run`, `place_player` and `player`. The template has no frame-time test. 01 keeps map facts as constants documented against `tools/level.json` | `examples/01_platformer/src/features/run/script.rs:28`, `:89`; `templates/basic/src/sample/script.rs:28`; `examples/01_platformer/src/features/level/mod.rs:46` |
| A14 | Script mode pins dt at 60 Hz, shows its window and takes `TUNGSTEN_USER_DIR`. Actions take JSON arguments. Display overrides come from the environment | `DECISIONS.md:3377` (`D-225`); `crates/tungsten-core/src/script.rs:360-365`; `crates/tungsten-core/src/config.rs:12-17` |
| A15 | Things that depend on the bench: `just visual`'s first line (`gpu-visual.png`); smoke's 15 bench rows; the release's `TITLES` and its test; the launcher test | `justfile:46`; `scripts/smoke-examples.sh:262-280`; `scripts/test-smoke-examples.sh:31`, `:87`, `:94-128`, `:164`; `scripts/release.py:99`; `scripts/test-release.py:878`; `tools/launcher/src/main.rs:199` |
| A16 | Docs that name the runner or the per-plan captures: README's commands, the LLM index (at its 12,288-byte cap), four skills, validation, the repo checker's review lines and known issues | `README.md:48-51`; `docs/LLM_INDEX.md:30`, `:80-81`; `scripts/check-agent-context.py:25`; `.claude/skills/tungsten-perf/SKILL.md`; `tungsten-milestone/SKILL.md:29`; `tungsten-next/SKILL.md:53`; `tungsten-wgpu/SKILL.md:30`; `docs/validation.md:20`; `scripts/check-repo.py:532`, `:543`; `docs/known-issues.md:91-94`, `:109`, `:112`; `crates/tungsten/src/app.rs:2226`, `:2252` |
| A17 | Where per-milestone sittings are required: the milestone skill's step 3 ("captures every CPU row"), validation's ECS row, the repo checker's message and the done-when rules. The release guide has no sitting | `.claude/skills/tungsten-milestone/SKILL.md` (Writing the plan, 3); `docs/validation.md:20`; `scripts/check-repo.py:543`; `docs/perf/profiling-workflow.md:220-226`; `rg -n -i 'perf\|sitting' docs/releases.md` → none |
| A18 | `perf-runs/` is untracked: 20 GB in 895 entries, with old baselines under `perf-runs/baselines/`. Phase-7 §2.5 and the records cite paths in it | `.gitignore:31-32`; `du -sh perf-runs`; `docs/plans/phase-7.md` §2.5 |
| A19 | `benchmarks.md` (92.7 KB) holds the workloads, dated results, eleven engine findings, seven open proposals and "left open" lists. Proposal M1, a counting allocator, needs a decision, since its counters are global state, which AGENTS' hard rules forbid | `docs/perf/benchmarks.md:459-533`; AGENTS.md (Hard rules) |
| A20 | CI's Linux job runs `just check` and `just script-test`; its Windows job runs `cargo test --workspace`. Benches are built in the dev profile (`D-112`) | `.github/workflows/ci.yml:87`, `:93`, `:105`, `:127` |
| A21 | Dev profile: core, the umbrella, the kit and 01 build at opt-level 1, every other project crate at 0. The bench crate enables no `testing` feature | `Cargo.toml:90-111`; `examples/02_bench/Cargo.toml` |

## 2. Design

### 2.1 Rows

`scripts/perf-rows.json` is the one registry. Each row there holds:

- its name, an optional `group`, the package and the folder it runs from;
- how it is launched: the bench's `TUNGSTEN_BENCH` with a preset, or an inline script;
- its resolution, warm-up (setup frames included), measured frames (300) and repeats;
- `gpu` (whether it gets a GPU run);
- its owned metrics, validity guards, known-drift metrics and budgets (all empty in this plan);
- `workload_version`, and its GPU counter pins (step 5).

Owned metrics and guards move out of the bench's Rust rows into the registry, so one file serves every game.

| Row | Runs | Kind | Owned (p50, p95 unless noted) | Valid when |
| --- | --- | --- | --- | --- |
| `physics`, `physics-sparse`, `ecs`, `churn`, `gpu`, `gpu-throughput`, `particles` | `example-02-bench` at today's presets | As today | As `profiling-workflow.md:191-200`, ported from `describe` at step 3's tree | As today's guards, in the registry |
| `e01-pit` | 01 from its folder. The script: `start_run`, `fill_pit` to `BALL_CAP`, a 600-frame settle, `wake_foes`, then 180 warm-up and 300 measured frames, ending with the frame-time test's state asserts through 01's `pit` probe (`balls` = 12,000, `foes_alive` = 6, `foes_awake` ≥ 5) | CPU | `total`, `update` p50, `physics_step` p50, `extract` p50 | The script's report passes |
| `template` | The template's sample. The script: `start_run`, then 120 warm-up and 300 measured frames, asserting `gameplay` on top | CPU | `total`, `update` p50, `extract` p50 | Report passes |
| `e01-area-<slug>` ×6 (group `e01-areas`) | 01 at 3840 × 2160. The script: `start_run`, then `place_player` at the area's point, 60 warm-up and 300 frames, and an assert on the `area` probe | GPU only, `--repeat 3` | Scene pass, `render_span`, the bloom chain and vignette p50 | Report passes |

- The `integrated` row is dropped, because 01's pit and the template frame measure real games (Q1).
- The six areas are the ones in M77's survey: the clearing, aqueduct, ravine, hollow, mine and bridge. Step 3 picks a point inside each area from 01's `Area` zones (`examples/01_platformer/src/features/level/components.rs:44`) at its tree, and the script's `area` assert confirms it.
- The bench rows keep `workload_version` 1. The new rows start at 1.

### 2.2 Runner and capture

The runner is in stdlib Python 3.12 (`D-078`): `scripts/perf.py` holds the commands and `scripts/perf_stats.py` the pure parsing, statistics and verdicts that the tests import.

The commands:

- **`list [--json]`** lists the rows.
- **`run <row|group> [--repeat N] [--frames N] [--plain] [--allow-background]`** takes one capture.
- **`compare <a> <b>`** compares two captures.
- **`baseline save|list|rm`** manages saved baselines.
- **`sitting`** and **`counters`** are covered in §2.3 and §2.4.

What carries over from the old runner, unchanged:

- the environment hygiene: cleared names, `RUST_LOG` and `TUNGSTEN_PERF_LOG` (B:41-44, B:280-287);
- the provenance fields (B:175-193);
- the `/proc` scanner and own-tree rule (B:199-338);
- the D-078 verdict rule and its thresholds (R:637-716);
- the digest definition (R:292-302): the measured window's `physics:` lines, and the `bench:` lines of every window frame but the last. A frame's `bench:` line prints in the next frame (`examples/02_bench/src/counters.rs:4-6`), so the old runner never saw the last one. Lines printed in tail frames (§2.5) stay out, so a GPU run with tails and a timing run without them hash alike, and step 3's parity with the old runner holds;
- validity: runs exit 0, every measured frame is logged and the guards pass. A scripted row also needs its report to pass, and a bench row needs `bench-config` to name its preset.

What is new:

- **Build.** Builds are release builds with `-C force-frame-pointers=yes -C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=6`, so every function and every block that isn't a fallthrough starts on a 64-byte boundary on both sides (Q4). `--plain` drops the alignment for a comparison at shipping flags. Each tree builds into its own target folder under `target/perf/`.
- **Tree provenance.** Provenance records the tree's own commit, or the ref's for an export, which fixes `docs/known-issues.md:93`.
- **Capture files.** A capture is `perf-runs/<UTC>-<row>/capture.json` at schema 2:

  ```text
  {schema, kind:"capture", row, sitting, tree{commit,dirty,ref}, build{flags,package},
   workload{version, config, script_sha256, resolution, warmup, frames, tail},
   renderer{backend, adapter, present_mode, max_frame_latency}, machine{…}, valid,
   runs[{valid, reasons, digest, rss_peak_kib,
         cpu{stages,systems,counters}, gpu{stages,passes}|null}]}
  ```

  Each metric there is `{p50,p95,p99,mean}`. `workload.config` is the bench's resolved knobs from `bench-config`, or null for a scripted row, which pins its script by hash. The runner refuses schema-1 captures as baselines.
- **Comparability.** Compare suppresses every verdict on a mismatch in any of the old hard gates (R:776-792), now read from the capture's own fields:
  - the row, `workload.version`, every config knob, the script hash, the resolution, warm-up and frames;
  - the build flags;
  - the backend, adapter, present mode and frame latency;
  - the machine.

  A registry edit after a capture therefore cannot make two captures look alike.
- **RSS.** Peak RSS gets a verdict only between captures that share a `sitting` id. Across sittings, both values are printed without a verdict, since THP moves peak RSS by tens of MiB between sittings.
- **Budgets.** Each budget is checked against the candidate's 95 % interval: `within` if the interval's upper bound is under the budget, `over` if its lower bound is above it, otherwise `near`. No row has a budget yet; fixtures test the logic.

### 2.3 Sittings

`sitting [--base REF] [--rows LIST] [--quick] [--cpu-only] [--wait-quiet S] [--allow-background]`. `REF` defaults to the last tag reachable from `HEAD` (`git describe --tags --abbrev=0`). The candidate is the working tree.

1. **Contamination check at the start.** The sitting refuses with exit 2 on any of these:
   - `D-141`'s encoder or foreign build;
   - a process other than the runner and its ancestors above 20 % of a core over a 5 s sample (Q13);
   - the tree's dirty hash changing during that sample.

   The 1-minute load, the governor, AC power and the top five processes are recorded. `--wait-quiet S` polls every 10 s until 60 s run quiet, or gives up at S. An idle agent session in the tree passes; a working one shows up as a busy process.
2. **Base export.** The base is exported with `git archive REF` into `target/perf/base-tree/`, and both trees are built before anything is measured. The candidate's runner and registry drive both sides, so a base from before the registry still runs every row its binaries can. The export is deleted at the end unless `--keep` is given.
3. **Interleaving.** Per row, captures run as A1 B1 A2 B2, each at the row's repeats. A1↔A2 is the A/A; A1→B1 and A2→B2 are the direct pairs.
4. **Verdicts.** An owned metric is `regressed` when both direct pairs read `regressed` and the A/A reads `unchanged` or `noisy` on it.
5. **The rerun.** A row runs once more (A3 B3 A4 B4, replacing the first set) when either happens:
   - its A/A reads any owned metric `regressed` or `improved`;
   - one of its known-drift metrics reads `regressed` in a direct pair.

   The rerun's verdicts then stand alone. If its A/A is stable (no owned metric `regressed` or `improved`), a metric `regressed` in both of its direct pairs reads `regressed`, a known-drift metric included, since a stable A/A and two regressed pairs twice over is a reproducible regression. Only a rerun whose own A/A fails again reads `unresolved`. A run that the background scans invalidate is set aside and taken again once, after a wait for quiet.
6. **Row verdicts.** Each row reads `ok`, `regressed` (with the metrics), `unresolved`, `invalid`, or `new` (a row the base cannot run, measured on the candidate only).
7. **Output.** The sitting writes `perf-runs/<UTC>-sitting[-quick]/` with `sitting.json`:

   ```text
   {schema:2, kind:"sitting", id, base{ref,commit}, cand{commit,dirty}, quick, wall_s,
    rows[{name, verdict, regressed[], reran, aa, pairs, budgets}]}
   ```

   It also writes a README table and the captures under `captures/`.
8. **Quick mode (`--quick`).** It runs only the CPU rows, as A1 B1 A2 at `--repeat 3` and 150 frames, with no rerun. It is labelled informational and is never a release's evidence (Q14).
9. **`--cpu-only`** skips the GPU runs. It is for a base whose GPU timing differs in kind, as in step 8(b).

### 2.4 Counter guards

**CPU, in `just check` and CI:**

- **Core.** `World::counters()` returns a `Copy` `WorldCounters`:
  - `archetype_moves`: an entity's row moving between archetypes;
  - `spawned` and `despawned`;
  - `events_sent`, summed from each queue's window at `flush_events`.

  All of them count since the World was created and are never reset. Each is an add beside work that already moves memory (Q15).
- **Umbrella `testing`.**
  - `CountingAlloc` is a `GlobalAlloc` over `System` that counts allocations and bytes in thread-local `const` `Cell`s, which never allocate. A test binary installs it with `#[global_allocator]`, and `testing::allocations()` reads the calling thread's counts.
  - `CounterGuard::measure(&mut harness, warmup, frames)` steps the harness in a counting mode. In that mode `step` records each extract's batch and instance counts and copies nothing, so the harness itself adds no allocation (A10). The returned reading holds the window's totals of allocations, bytes, archetype moves, spawns, despawns and events, plus the per-frame maximum of batches and instances.
  - `reading.check(pin_path)` compares the reading against its own JSON pin file, one file per guard, so parallel tests never share a file. On a difference it fails, naming each counter with its old and new values and `TUNGSTEN_GUARDS_WRITE=1`, which rewrites that one file through a temporary file and a rename.
- **Pins.** Pins are exact and Linux's. On other systems a guard prints its reading and passes, since platform maths can change a float path (Q6). A rise in a cost counter needs a fix or its cause in the commit's plan.
- **Headless setup.** The harness runs no startup hook and loads no manifest (`crates/tungsten/src/testing.rs:18-21`). The bench registers its generated textures in startup hooks that need the renderer (`physics.rs:266`, `ecs.rs:188`, `churn.rs:178`, `particles.rs:420`), and `gpu` builds its tilemap and field there too (`gpu/scene.rs:94-103`). A sprite with no asset is skipped at extract (`crates/tungsten/src/sprite_extract.rs:411`), so a guard on that setup would pin an empty extract. So:
  - Step 1 adds `Harness::allocate_texture_handle()`, a handle from a harness-local counter, never a renderer's (`D-048` stands: no renderer runs).
  - Step 2 routes the bench's registration (`gen.rs`'s `register` and `register_atlas`, `gen.rs:132-140`) through a bench-local texture sink. The renderer's sink uploads pixels; the harness's sink takes a handle from `allocate_texture_handle` and uploads nothing. Everything else runs the same code in both: the atlas packing, the one handle per page that its cells share (`particles.rs:440`), the filters and the lit flags. So the core metadata and extract's batch keys (`sprite_extract.rs:427`) match the window's. Each guarded bench's startup splits into a world part, holding the entities, tilemap, clips and that registration, and a renderer-only part for materials. `main` runs both.
  - Before a guard reads or writes a pin, `TUNGSTEN_GUARDS_WRITE=1` included, it asserts its workload:
    - the bench's `row()` names the guarded row;
    - the entities it counts in the World with each of the row's components equal its configuration. These are counted, not taken from the bench's counters, since `gpu`'s `bench:` fields echo its parameters (`gpu.rs:535`);
    - on every window frame, the extracted instances equal the drawable entities it counts by the extract's own visibility and cull rule (`view.rs:60` for the culled views), so a sprite without its asset shows as a shortfall.
  - Step 4 checks, locally, that each guarded row's headless batch count equals the window's `sprite_batches` at the guard's configuration.
- **The guards**, each with the bench row's warm-up and a 60-frame window. Each names its own configuration, so it keeps its row's identity at a smaller size:

  | Guard | Preset | Why |
  | --- | --- | --- |
  | `physics` | `min` | pachinko at 1,000 balls (`physics.rs:74`) |
  | `physics-sparse` | `sparse-min` | `min` is pachinko; `sparse-min` keeps `mode` sparse (`physics.rs:82`) |
  | `ecs`, `churn`, `particles` | `min` | each bench's own small preset |
  | `gpu` | `min` | the default row at its small preset |
  | `gpu-throughput` | `throughput` with `TUNGSTEN_BENCH_SCALE` below 1, as step 2 picks | `row()` names `gpu-throughput` only at preset `throughput` (`gpu.rs:334`) |

  - 01's guard: `start_run`, `fill_pit` of 2,000 balls, a 120-frame settle, `wake_foes`, then a 60-frame window, asserting the `pit` probe (step 2) first.
  - The template has none (Q7).

**GPU, local (`just perf counters`):**

- The renderer counts the draw calls and upload bytes it issues each frame. Every `write_buffer`, `write_buffer_with` (the sprite instance stream, `sprite.rs:901`), `write_texture` and `create_buffer_init` goes through `crates/tungsten-render/src/counters.rs`, and so does every draw the engine records. `Renderer::frame_counters()` returns the counts.
- Text is drawn by glyphon, which uploads and draws inside `prepare` and `render` (`text/gpu.rs:550`, `:717`), out of the wrappers' reach. Its share is counted at the boundary as `text_renders` (`render` calls) beside `TextStats`' prepared batches and glyphs. `draw_calls` and `upload_bytes` are named as the engine's own and exclude glyphon's internals, and the docs say so.
- The umbrella copies them into `RenderCounts::draw_calls`, `upload_bytes` and `text_renders`, and sets `RenderCounts::sprite_batches` at extract beside `sprite_instances` (`app.rs:1630`). With `TUNGSTEN_PERF_LOG`, it prints a `counters: draw_calls=… upload_bytes=… text_renders=… sprite_batches=…` line after `frame:`, which stays out of the digest.
- `just perf counters [--write]` runs `gpu`, `gpu-throughput`, `template` and `e01-area-clearing` at the row's warm-up plus 30 frames, once each. It compares every measured frame against the row's pin in the registry. A row whose count varies by frame pins its window's sum instead.
- It runs beside `just smoke` and `just visual`, and the release chain gains it (§2.7).

### 2.5 GPU timestamps without stalling

- **Resources.** `TimingResources` is built once per pass layout (the post stack, bloom mips, SMAA, a capture's blit and the target size) and rebuilt only when that layout changes.
- **The ring.** It holds a ring of three slots, each with a query set, a resolve buffer, a readback buffer, the labels and a state (free, in flight or mapped).
- **Each timed frame:**
  1. It calls `device.poll(PollType::Poll)`, which does not block.
  2. It decodes each slot whose map has completed into `GpuFrameTimings`, then unmaps the slot and frees it.
  3. It takes a free slot, or runs untimed if none is free. Skipped frames are counted.
  4. It resolves and copies into the slot in the second command buffer.
  5. After the submit, it calls `map_async`, and does not wait.
- **The state machine.** The slot logic is plain Rust, apart from wgpu, so tests need no GPU.
- **Reporting.**
  - Each completed slot prints one `gpu_passes: from=<frame> …` line in the frame that decodes it, where `<frame>` is the app's number for the frame it measured. Two slots completing together print two lines. A frame that decodes none prints no `gpu_passes:` line.
  - `frame:`'s `gpu=` keeps the newest completed scene pass for the HUD. The runner takes no GPU metric from it.
  - The runner attaches each `gpu_passes:` line to the frame its `from` names, not to the latest `frame:` line as the old parser did (`scripts/bench_report.py:108-114`). It selects the measured window by those frame numbers, so warm-up samples stay out.
  - A GPU run takes four tail frames past the window, the ring's depth plus one, so the window's last slots complete before the process exits. No shutdown wait is added. A window with fewer than 95 % of its frames sampled is invalid, with ring-full skips counted.
  - From step 4 on, the old runner cannot read the new line. It runs no GPU row after step 3's parity, and step 6 deletes it.
- **Unchanged.** `finalize_capture`'s wait stays (`renderer.rs:2308`), since a capture is one frame.
- **Separate run kept.** The GPU run stays separate from the timing run (Q5).

### 2.6 Purge and carry-overs

**Deleted:**

- `bench.py`, `bench_report.py` and `test-bench.py`;
- the bench's `integrated` (`integrated.rs`, `integrated/`, its manifest entries and smoke's two rows);
- the bench's Rust row metadata (owned metrics, guards, bottleneck, key knobs; `knobs.rs:256-272`), now in the registry;
- `capacity`, `--sweep`, `--profile` and the HTML reports (Q8);
- `benchmarks.md`'s dated results.

**Kept:**

- `gpu-visual.png` and `just visual`;
- the `Benchmark` launcher (`D-196`; Tab now cycles five benchmarks);
- the bench's presets and knobs;
- the old data under `perf-runs/` (Q9).

**`docs/perf/benchmarks.md`, rewritten:**

- a section per row: workload, knobs and presets, owned metrics, guards;
- a Counter guards section;
- **Levers for 9.2**: every engine finding, open proposal and "left open" item at `2464cbc9`, one line each with its evidence pointer, or `closed` with its decision;
- Adding a row: a row and a counter guard land with the system step that needs them (§5.8).

The redo's 9.2 row links the lever list (Q11).

**`docs/perf/profiling-workflow.md`, rewritten** for the new runner. It keeps the machine facts that still hold:

- the encoder;
- other sessions and your own commands;
- KWin's acquire stall;
- RSS moving across sittings with THP;
- a busy editor watcher after a large clean;
- the remote-desktop contamination evidence;
- pacing precedence;
- backends;
- profiling with `perf`, `samply` and flamegraphs by hand;
- criterion.

### 2.7 Process after 7.3

- **No per-plan sitting.** A plan's perf checks are its counter guards, which `just check` runs.
- **Quick sitting.** A change to a hot path (ECS, frame loop, extract, physics, render) may quote a quick sitting as information (Q15).
- **Full sittings.** A plan that claims a speed-up, measures a list item (7.14, 7.15, 9.2) or changes a row's workload runs a full sitting as its evidence.
- **Release sitting.** A release that bumps the minor or major number runs `just perf sitting` against the previous tag before its cut, unattended with the owner disconnected. A `regressed` owned metric needs a fix or a known-issues entry with the sitting's path before the cut (Q12).
- **Release chain.** The release chain gains `just perf counters` beside `just smoke`.

### 2.8 Decision

One entry, `D-22[6]`, written by the tungsten-decision skill in step 7.

- **It amends `D-078`:**
  - the registry;
  - `integrated`, capacity and sweeps retired;
  - sittings with an A/A and one rerun;
  - aligned builds by default;
  - RSS within a sitting;
  - budget verdicts;
  - GPU timings reported late without a stall.
- **It amends `D-141`:** the check at the start of a sitting, the busy-process refusal and `--wait-quiet`.
- **It extends `D-070`:** counter guards are ordinary tests, and CI stays informational.
- **It records the exception to AGENTS' no-global-state rule:** `CountingAlloc`'s thread-local counters exist only in test binaries and never in a shipped build (benchmarks.md's M1).
- **It records the process in §2.7.**

## 3. Steps

Every code step's done-when also includes `cargo fmt --all -- --check` → exit 0 and `cargo clippy -p <each crate it touches> --all-targets --locked -- -D warnings` → exit 0. Test filters below are name prefixes the step gives its tests, and each one's `-- --list` must list at least the stated count, since a filter that matches nothing still exits 0. Python runs with `-B`.

### Step 1: CPU counters in core and the harness

- **Files:** as listed for step 1.
- **Change:** §2.4's CPU half:
  - `WorldCounters` and `World::counters()`, incremented where rows move, entities spawn or despawn, and windows flush, with the command buffer's flush included.
  - `testing::CountingAlloc`, `testing::allocations()`, `CounterGuard` and its counting mode in `testing/guard.rs`.
  - `Harness::allocate_texture_handle()` (§2.4's headless setup).
  - Rustdoc on each new public item.
  - `DESIGN.md`'s ECS and testing sections, one sentence each.
- **Done-when:**
  - `cargo test -p tungsten-core --locked --lib world_counters_` → passes, with ≥ 4 listed:
    - one insert, one move;
    - a three-component spawn through the command buffer, one move (`D-135`);
    - spawn and despawn;
    - events summed over two flushes.
  - `cargo test -p tungsten --locked --lib counter_guard_` → passes, with ≥ 5 listed:
    - a matching pin;
    - a mismatch whose message names the counter, both values and `TUNGSTEN_GUARDS_WRITE`;
    - a write;
    - two windows of one app reading equal counts with no allocation from the harness's own step;
    - a sprite registered on a handle from `allocate_texture_handle` drawn by the default extract, so it shows in the window's instance count, and two sprites on one handle falling in one batch.
  - `cargo test -p tungsten-core --locked` and `cargo test -p tungsten --locked` → pass.
  - `just api` → the diff adds `WorldCounters`, `World::counters` and the testing items and removes nothing. Commit it.
  - `just docs-ratchet` → passes.
- **Commit:** `feat(core): add world counters and the harness's counter guards`
- **Moves:** the API snapshots.

### Step 2: CPU counter guards and 01's pit actions

- **Files:** as listed for step 2.
- **Change:**
  - **The bench.**
    - Its app setup moves into a function that `main` and its tests both call.
    - Each guarded bench's startup splits into its world part and its renderer part (§2.4's headless setup). `main` runs both in its startup hook, so the window's frames are unchanged.
    - A `#[cfg(test)] #[global_allocator]` installs `CountingAlloc`.
    - `gen.rs`'s registration goes through the texture sink (§2.4).
    - `src/tests/guards.rs` runs one `guards_<row>` test per ported row, at §2.4's configuration for it, against `src/tests/guards/<row>.json`. Each asserts its workload before it touches a pin (Q16 covers a row whose split is not local).
    - The tests take `tungsten`'s `testing` feature as a dev-dependency.
  - **01.**
    - `features/run/pit.rs` takes the frame-time test's setup: the pour, the settle, the player on the deck and six foes awake with durable health.
    - The pit's interior, its shelves and the deck become constants beside `level/mod.rs`'s map constants, with a test against `tools/level.json`, as those have. Game code reads no `tools/` path.
    - Two new script actions use it: `fill_pit {count}` and `wake_foes`. `wake_foes` keeps the six foes it spawns in a resource.
    - A `pit` probe reports `{balls, foes_alive, foes_awake}`, read as the frame-time test reads them (`Health::is_dead`, `StateMachine::is_held`; `frame_time.rs:191-203`), since `count` sees only registered components (`crates/tungsten-core/src/script.rs:1342`).
    - `frame_time.rs` calls the shared setup and keeps its assertions.
    - `tests/game/guards.rs` holds the §2.4 01 guard, and `main.rs` installs the allocator.
- **Done-when:**
  - Bench guards:
    - `cargo test -p example-02-bench --locked guards_` → passes.
    - It lists one test per ported row, less any row Q16 skips, which the evidence names with its reason.
    - Each guard's workload assertion runs before its pin check, and failing it fails the test. Show this once, in the evidence, with a scratch edit that skips one texture registration in the harness's sink: the `gpu` guard fails on its instance shortfall, not on its pin, under `TUNGSTEN_GUARDS_WRITE=1` too, and its pin file is unchanged afterwards (`git diff --exit-code -- examples/02_bench/src/tests/guards/`). Then remove the scratch edit.
    - It takes ≤ 60 s wall.
  - For each of the seven bench rows, check that the split moved no workload: `just perf run <row> --repeat 1 --allow-background` (the old runner, local) gives the same first-run digest before the step's edits and after them. Quote the seven pairs.
  - 01's guard:
    - `cargo test -p example-01-platformer --locked --test game guards::` → passes, with ≥ 1 listed.
    - It takes ≤ 60 s wall.
  - Run both guard commands twice after one `TUNGSTEN_GUARDS_WRITE=1` run → both pass. Then `git status --porcelain -- examples/02_bench/src/tests/guards examples/01_platformer/tests/game/guards` lists only the new pin files, one per guard, unchanged since the write.
  - `cargo test -p example-01-platformer --release --locked --test game frame_time -- --ignored` → passes, and its scenario line reads 12,000 balls and 6 enemies.
  - `cargo test -p example-01-platformer --locked` and `cargo test -p example-02-bench --locked` → pass.
  - Local, on the GPU: from `examples/01_platformer`, `tungsten run --script` (the workspace's `tungsten-cli`) on a scratch script exits 0. The script does `start_run`, `fill_pit` to the cap and `wake_foes`, and asserts the `pit` probe at frame 900: `balls` = 12,000, `foes_alive` = 6, `foes_awake` ≥ 5.
  - `just smoke` → passes.
- **Commit:** `test(bench): add counter guards to the bench rows and example 01`
- **Moves:** none. The ported rows' `bench:` lines and digests stay unchanged.

### Step 3: The runner: rows, run and compare

- **Files:** as listed for step 3.
- **Change:**
  - §2.1's registry, with each bench row's owned metrics and guards ported from the binary's `TUNGSTEN_BENCH_DESCRIBE=1` schema at the step's tree.
  - §2.2's `list`, `run`, `compare` and `baseline`.
  - `test-perf.py` on fake binaries, as T:130-150 builds them.
  - `just perf-test` runs both test files.
  - `just perf` still runs the old runner.
- **Done-when:**
  - `python3 -B scripts/test-perf.py` → passes, with no GPU and no cargo. It covers:
    - parsing, guards and validity;
    - the digest: the last window frame's `bench:` line and tail frames left out, a run with tails and one without hashing alike;
    - provenance of an exported tree (a temp repository);
    - comparability: verdicts suppressed on each hard gate, a workload version, a knob, a script hash and a frame count among them;
    - Welch verdicts against `R`'s fixtures;
    - RSS within and across sittings;
    - budget verdicts.
  - `just perf-test` → passes.
  - Every named row is listed. This check prints nothing:

    ```bash
    for r in e01-pit template ecs particles physics-sparse gpu gpu-throughput physics churn e01-area-clearing; do python3 -B scripts/perf.py list --json | jq -e --arg r "$r" 'map(.name) | index($r)' >/dev/null || echo "$r"; done
    ```

  - Parity (local, GPU), for each of the seven bench rows:
    - `python3 -B scripts/perf.py run <row> --repeat 1 --allow-background` → valid.
    - Its `runs[0].digest` equals the digest from `just perf run <row> --repeat 1 --allow-background` on the same tree.
    - Quote the seven pairs.
  - `python3 -B scripts/perf.py run <row> --repeat 1 --allow-background` → valid for `e01-pit`, `template` and each `e01-area-*`, with its report passing.
  - `python3 -B scripts/perf.py compare` of a plain and an aligned `ecs` capture → verdicts suppressed, naming the build flags.
  - `git status --porcelain | rg -q __pycache__; test $? -eq 1` → exit 0.
  - `just script-test` → passes.
- **Commit:** `feat(perf): add the new benchmark runner beside the old`
- **Moves:** none.

### Step 4: GPU timestamps without stalling, and GPU counters

- **Files:** as listed for step 4. Before editing, run this to list every upload and draw site in the render source and record the list:

  ```bash
  rg -n '\.(write_buffer|write_buffer_with|write_texture|create_buffer_init|draw|draw_indexed)\(' crates/tungsten-render/src
  ```
- **Change:**
  - §2.5's ring and §2.4's GPU half.
  - `renderer.rs:1411`'s doc is corrected.
  - The `counters:` line goes into profiling-workflow's Telemetry lines, with `gpu_passes:` arriving late.
  - The timing part of known issues' Render F11 is closed.
  - Before the edits, capture the parent tree's ratio of the GPU run's `total` p50 to the timing run's: `python3 -B scripts/perf.py run gpu --repeat 3 --allow-background`.
- **Done-when:**
  - `rg -n 'wait_indefinitely' crates/tungsten-render/src/timing.rs; test $? -eq 1` → exit 0.
  - `rg -n -g '!counters.rs' -g '!tests/**' '\.(write_buffer|write_buffer_with|write_texture|create_buffer_init|draw|draw_indexed)\(' crates/tungsten-render/src; test $? -eq 1` → exit 0. Any baseline match that is not an upload or draw goes in the evidence, and the pattern is narrowed for it.
  - Every `text_renderer.render` and `prepare` call in `text/gpu.rs` is counted at the boundary: `text_renders` > 0 on a `gpu` frame with text.
  - `RenderCounts::sprite_batches` and the `counters:` line's `sprite_batches`. Locally, for each guarded bench row at its guard's configuration, the window's `sprite_batches` equals the headless guard's maximum batch count (§2.4). Quote the pairs.
  - `python3 -B scripts/test-perf.py` → passes, with `from` attribution tests:
    - a warm-up frame's sample landing inside the window is dropped;
    - two samples decoded in one frame attach to their own frames;
    - the window's last frames are sampled from the tail;
    - a window under 95 % sampled is invalid.
  - `cargo test -p tungsten-render --locked --lib timing_ring_` → passes, with ≥ 3 listed:
    - a full ring skips a frame and counts it;
    - each completed slot reports once;
    - a layout change rebuilds.
  - On the GPU, `python3 -B scripts/perf.py run gpu --repeat 3 --allow-background` shows these in every run:
    - the GPU run's `total` p50 is ≤ 1.10 × the timing run's (quote the parent's ratio beside it);
    - the capture is valid, so ≥ 95 % of its window's frames are sampled by `from`;
    - `cpu.counters.draw_calls.p50` > 0.
  - `just smoke` and `just visual` → pass, with no reference image changed.
  - `cargo test -p tungsten-render --locked` and `cargo test -p tungsten --locked` → pass.
  - `just api` → the diff adds the counter items and is committed.
  - `just docs-ratchet` → passes.
- **Commit:** `perf(render): read GPU timestamps without stalling, count draws and uploads`
- **Moves:**
  - the API snapshots;
  - `gpu_passes:` timings, which arrive up to three frames late;
  - the GPU run's frame pacing, which no longer stalls.

### Step 5: Sittings, quick mode and the counters check

- **Files:** as listed for step 5.
- **Change:**
  - §2.3's `sitting` with `--quick`, `--cpu-only` and `--wait-quiet`.
  - §2.4's `counters` with `--write` and the registry's pins.
- **Done-when:**
  - `python3 -B scripts/test-perf.py` → passes, with tests for:
    - the A1 B1 A2 B2 order;
    - a fake row whose A/A fails once, which reruns and reads `ok`, and twice, which reads `unresolved`;
    - a known-drift metric's rerun;
    - a metric `regressed` in both pairs of the first set and again of a rerun with a stable A/A, which reads `regressed`, apart from the unstable-A/A case;
    - a `new` row;
    - RSS verdicts only within a sitting;
    - budgets `within`, `near` and `over`;
    - the start check: refusals on the encoder, a foreign `cargo` and a busy process, and passes for an idle agent session and an excluded ancestor;
    - `--wait-quiet` timing out;
    - the base export recording the ref's commit;
    - quick's shape;
    - counter pins matching and mismatching.
  - Local:
    - `python3 -B scripts/perf.py counters --write`, then `python3 -B scripts/perf.py counters` twice → exit 0 both times.
    - `python3 -B scripts/perf.py sitting --quick --rows ecs --allow-background` → exit 0, writing `sitting.json` and `README.md`. This is functional only, not evidence.
  - `just script-test` → passes.
- **Commit:** `feat(perf): run sittings with an A/A, a rerun and a quick mode`
- **Moves:** none.

### Step 6: Switch and purge

- **Files:** as listed for step 6.
- **Change:**
  - **Runner switch.**
    - `just perf` runs `scripts/perf.py`, and `just perf-test` runs `test-perf.py` alone.
    - The old runner and its tests are deleted.
    - The comments that name `bench_report.py` (`app.rs:2226`, `:2252`) are corrected.
  - **The bench.**
    - `integrated` and the Rust row metadata leave it (§2.6).
    - Smoke's benchmark section drops to 13 rows, and its test follows.
  - **Docs.**
    - `benchmarks.md` and `profiling-workflow.md` are rewritten (§2.6), and the redo's 9.2 row links the lever list.
    - README's commands are updated, and the LLM index rows name the new files within its cap.
    - Known issues: `:91-94`, `:109` and `:112` are updated or closed.
  - **Skills.**
    - `tungsten-perf` is rewritten.
    - The command lines in `tungsten-wgpu`, `tungsten-next` and `tungsten-milestone` (`:29`'s `pgrep` target) are updated.
- **Done-when:**
  - `test -z "$(fd -t f '^(bench|bench_report|test-bench)\.py$' scripts)"` → exit 0.
  - `rg -n 'bench\.py|bench_report|test-bench' --glob '!docs/plans/**' --glob '!DECISIONS.md' --glob '!CHANGELOG.md' .; test $? -eq 1` → exit 0.
  - `rg -n -w 'integrated' examples/02_bench scripts/smoke-examples.sh scripts/test-smoke-examples.sh; test $? -eq 1` → exit 0. Run this on the tree first, and name any baseline match that means something else.
  - `just perf list` → every row. `just perf-test` and `just script-test` → pass.
  - `just smoke` → passes, its benchmark section reading 13. `just visual` → passes.
  - `git diff --quiet 2464cbc9 -- examples/02_bench/tests/fixtures/gpu-visual.png` → exit 0.
  - For each bench row, `just perf run <row> --repeat 1 --allow-background` → its digest equals step 3's parity value.
  - `rg -n '^## ' docs/perf/benchmarks.md` lists the row sections, `Counter guards`, `Levers for 9.2` and `Adding a row`, with no dated-results section.
  - The evidence maps each of the eleven findings, seven proposals and "left open" items at `2464cbc9` to a lever line or a `closed` note.
  - `just ctx` (the LLM index within 12,288 bytes) and `just repo-check` → pass.
  - `cargo test -p example-02-bench --locked` → passes.
- **Commit:** `refactor(perf)!: switch just perf to the new runner, purge the old`
- **Moves:**
  - smoke's benchmark rows, 15 → 13;
  - the bench's `describe` schema, which loses the row metadata;
  - the `Benchmark` launcher's Tab cycle, which goes to five benchmarks.

### Step 7: Per-milestone sittings retired, and the decision

- **Files:** as listed for step 7.
- **Change:**
  - §2.8's entry, with its index row and the marker lines on `D-078`, `D-141` and `D-070`, using the tungsten-decision skill. Recheck the next free ID right before committing.
  - §2.7 written into:
    - the milestone skill (Writing the plan, step 3; Running steps' captures line);
    - validation's matrix: ECS, physics and renderer rows, with `just perf counters` beside `just smoke` and `just visual` for render and extract changes;
    - the repo checker's two review lines and their test;
    - the release guide: the sitting before a minor or major cut, and `just perf counters` in step 1's chain;
    - the operating rules' Releases section;
    - the plans README's perf line;
    - profiling-workflow's done-when rules;
    - `tungsten-perf`'s policy lines.
- **Done-when:**
  - `rg -c '^## D-22[6] ' DECISIONS.md` → 1.
  - `rg -c '^\| `D-22[6]` ' docs/DECISION_INDEX.md` → 1.
  - `rg -n 'Amended by `D-22[6]`' DECISIONS.md` → matches in `D-078`, `D-141` and `D-070`.
  - `rg -n -i 'every CPU row' .claude/skills docs/validation.md docs/perf scripts/check-repo.py; test $? -eq 1` → exit 0.
  - `python3 -B scripts/test-check-repo.py`, `just ctx`, `just repo-check` and `just script-test` → pass.
- **Commit:** `docs(perf): retire per-plan sittings for guards and release sittings`
- **Moves:** none.

### Step 8: The first sitting and provisional baselines

- **Files:**
  - Captures go under `perf-runs/`, which is untracked.
  - `scripts/perf-rows.json` changes only when the A/A gives a row a known-drift metric, with the evidence beside it.
  - This plan's evidence row.
- **Change:**
  1. Check `df` first (at least 20 GB free).
  2. Run one background job with nothing beside it, then end the turn and wait for its notification. The job waits for quiet, so the owner disconnects NoMachine when this step starts. It runs:
     - (a) `just perf sitting --base HEAD --wait-quiet 10800`: the full sitting, same build on both sides, which validates the runner's noise floor;
     - (b) `just perf sitting --base 2464cbc9 --rows physics,physics-sparse,ecs,churn,particles,gpu,gpu-throughput --cpu-only`: steps 1 and 4's engine changes against the plan's base;
     - (c) `just perf sitting --quick`.
  3. Then run `just perf baseline save <(a)'s sitting> provisional-0.113`.
- **Done-when:**
  - (a): `jq -r '.rows[] | select(.verdict != "ok") | .name' <a>/sitting.json` → prints nothing, after the built-in rerun.
  - (a): every bench row's digests are equal across sides.
  - (a): `jq '.wall_s' <a>/sitting.json` → ≤ 3600.
  - (b): the same `jq` → prints nothing.
  - (c): `wall_s` ≤ 600.
  - `just perf baseline list` → shows `provisional-0.113`.
  - `just perf counters` → passes.
- **Commit:** `docs(plans): record the first benchmark sitting`, plus `chore(perf): …` for a registry change.
- **Moves:** none.

### Step 9: Close

- **Files:** as listed for step 9.
- **Change:**
  - Run the checks the change needs (the validation rows for core ECS, the renderer, scripts, the template-free examples and the API), then the demo.
  - Write phase-7's Step 2 evidence row with the commit range and this plan's done-when quoted.
  - Move open follow-ups to known issues or Later.
  - Take `bench-runner` out of Now, and update phase-7's Now line to say that 7.3 has landed and 7.4 is next.
  - Delete this plan and its citations.
- **Done-when:**
  - These pass: `just check`, `just script-test`, `just ctx`, `just repo-check`, `just api-check`, `just docs-ratchet`, `just smoke`, `just visual` and `just perf counters`.
  - `just perf run e01-pit --repeat 1` → valid.
  - `cd examples/01_platformer && cargo run` → starts and plays.
  - `rg -n 'bench-runner\.md' --glob '!docs/plans/archive/**' .; test $? -eq 1` → exit 0.
- **Ends with:**
  - the commits made and the evidence quoted;
  - what to try: `just perf list`, `just perf sitting --quick`, and a counter guard failing after an allocation added to a system;
  - the next prompt: tungsten-next's `plan` row for 7.4 (H2), with phase-7.md and the redo's 7.4 row and §4 as its sources.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Which rows to port | Q12's default: the bench's core rows that still measure something (ECS, physics, render), so every row except `integrated`. Add 01's full pit, the template frame and 01's areas (phase-7's Step 2). UI rows wait for 7.17's layout guards, since the template frame runs the shell's UI |
| Q2 | 01's areas as one row or six | Six rows in group `e01-areas` at 3840 × 2160, matching M77's survey. GPU only at `--repeat 3`, so a sitting stays near an hour |
| Q3 | The runner's home | A fresh stdlib Python runner (`D-078`) under `scripts/perf*.py`, with the `just perf` and `just perf-test` names kept, so the allow-list stays valid |
| Q4 | Code-placement noise | Aligned builds on both sides by default; `--plain` gives shipping flags. When a row whose frames run the changed code reads `regressed`, the manual control is still a placebo: the candidate with its change switched off, its addresses checked with `nm` (profiling-workflow's Profiling section) |
| Q5 | GPU timing | A ring of three without a stall. The GPU run stays separate; merging it into the timing run is for 9.1, once its cost is measured |
| Q6 | Guard pins across systems | Exact Linux pins, rewritten by `TUNGSTEN_GUARDS_WRITE=1`. Elsewhere a guard reports and passes |
| Q7 | A template counter guard | None in 7.3: `tungsten new` copies the template's tests into every game. 7.11 may add one beside the sample's tests |
| Q8 | Capacity, sweeps, `--profile`, HTML reports | Not carried. Profiling by hand stays in the workflow; 9.1's scaling rows replace capacity |
| Q9 | The old captures in `perf-runs/` (20 GB) | Left in place, since phase-7 §2.5 and the records cite them. The new runner refuses them as baselines, and the owner deletes old folders when they choose |
| Q10 | Criterion | Unchanged (`D-037`), with CI's dev-profile bench build kept |
| Q11 | Where 9.2's lever list lives | `docs/perf/benchmarks.md` § Levers for 9.2, linked from the redo's 9.2 row; phase-7's `9.2` rows point there |
| Q12 | The release sitting | Minor and major releases only, against the last reachable tag, unattended. A `regressed` owned metric gets a fix or a known-issues entry before the cut. H2's script takes this over |
| Q13 | The start check's thresholds | A busy process is above 20 % of a core over 5 s. Load, governor and AC power are recorded, not refused on. Tune only by a test with a fixture |
| Q14 | Quick mode's shape | CPU rows only, A1 B1 A2, `--repeat 3`, 150 frames, no rerun, informational |
| Q15 | The counters' cost on hot paths | One add per row move, spawn, despawn and flush, beside memory moves already there. Step 8(b) judges it |
| Q16 | A bench row whose startup does not split into a world part and a renderer part within its own files (`gpu`'s heavy material is the likely case) | Its CPU guard is skipped with the reason recorded, and `just perf counters` also pins that row's `sprite_instances` and batch count locally. A guard is never pinned on a workload it has not asserted |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:**
  - the API snapshots (steps 1 and 4);
  - the timing of `gpu_passes:` (step 4);
  - smoke's benchmark rows, 15 → 13 (step 6);
  - the bench's `describe` schema and the `Benchmark` Tab cycle (step 6);
  - the home of 01's full-pit setup, from the test to `src/`, with its assertions kept (step 2).

  Each move is recorded in the evidence log.
- **Must not move:**
  - the physics hashes;
  - the kit's determinism pin;
  - the pinned containment hash;
  - the ported rows' workloads (`workload_version` 1) and digests (step 3's parity values, checked again in steps 6 and 8);
  - `gpu-visual.png`, `just visual`'s comparisons, and the post and transition regressions;
  - 01's save schema (23);
  - `D-181`'s template budgets, since the template is not edited.

## 6. Stop conditions

- **A done-when check fails.** Restore the step's files from the last commit, mark the step skipped, and continue with the steps that don't depend on it. Steps 6–8 depend on 3 and 5. Step 8(b) depends on 4.
- **A counter is not deterministic.** If it differs between two runs on one machine, drop it from that guard and record it. A guard left with no deterministic counter is dropped.
- **The ring doesn't complete.** If `PollType::Poll` never completes a map on the reference adapter, the decode waits instead on the submission of the slot being decoded, which is at least two frames old (wgpu 30's submission-index wait). That wait blocks only while the GPU runs two frames behind, and step 4's 1.10 ratio judges it. If that fails too, the timestamp half of step 4 is skipped and its counters land alone. 7.15 runs "on 7.3's non-stalling timestamps" (`docs/plans/roadmap-redo.md:138`), so the plan then does not close: steps 5–8 run, step 9 does not, `bench-runner` stays in Now, and the session ends with the resume prompt naming the ring as the one open item.
- **Step 8(a) reads a row `unresolved`.** Add the metric to that row's known-drift list with the evidence, and rerun the row once. If it is still `unresolved`, the row stays listed, its sittings report it without verdicts, and the result is recorded; the plan still closes.
- **Step 8(b) reads an owned CPU metric `regressed` in both pairs after the rerun.** Make one fix that moves the count off the per-entity path, then run the sitting again. If it still regresses, the cost and its justification go in `D-22[6]`'s consequences.
- **No quiet within 3 hours, or less than 20 GB free after a `just clean-target` dry run.** Step 8 is not run. The close says the provisional baselines are owed, and the next release's sitting takes them.
- **A full sitting runs over 60 minutes.** The `e01-areas` group leaves the default set and becomes opt-in with `--rows`, recorded.
- **A file outside §files to touch needs a change.** Record it as a follow-up unless the step cannot pass without it, and name the file in the evidence.

## 7. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-29, reading the uncommitted draft on `2464cbc9`. Footer: `617.7 s, tokens in 1083226 (cached 996736), out 10974 (reasoning 6231), commands 33, output 321.7 KB (largest 26 KB), usage 5h 0% → 24%, week 25% → 29%`. Seven findings, all accepted. They changed done-when checks, the sitting's rerun rule and step 4's stop condition, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] The GPU counter inventory misses the sprite instance upload and glyphon's work | `crates/tungsten-render/src/sprite.rs:901` (`write_buffer_with`); `text/gpu.rs:550`, `:717` (glyphon `prepare` and `render` take the raw queue and pass) | accepted | §2.4: `write_buffer_with` wrapped. Glyphon is counted at its boundary as `text_renders` beside `TextStats`, and the totals are named as the engine's own. Step 4's `rg` patterns gain `write_buffer_with` and a `text_renders` check |
| 2 | [HIGH] Headless guards can pass without constructing the workload | `crates/tungsten/src/testing.rs:18-21` (no startup hook, no manifest); `examples/02_bench/src/gpu/scene.rs:94-103`; `physics.rs:266`, `ecs.rs:188`, `churn.rs:178`, `particles.rs:420` (textures registered in startup hooks); `crates/tungsten/src/sprite_extract.rs:411` (no asset, sprite skipped) | accepted | §2.4's headless setup: `Harness::register_sprite` (step 1); each bench's startup split into world and renderer parts (step 2); workload assertions before any pin, with a seeded failure shown; Q16 narrowed |
| 3 | [MEDIUM] A reproducible regression on a known-drift metric becomes `unresolved` | §2.3's rerun rule; §2.7 acts on `regressed` only | accepted | §2.3: after a stable rerun A/A, two regressed pairs read `regressed`; only an unstable rerun A/A reads `unresolved`. Step 5 tests both |
| 4 | [MEDIUM] Delayed GPU results lack the identity needed to select measured frames | `scripts/bench_report.py:108-114` (a companion attaches to the latest frame), `:366` (window by position) | accepted | §2.5: `gpu_passes: from=<frame>`, one line per completed slot, four tail frames and a 95 % sampling floor. The runner selects by `from` (step 4's files and tests); the old runner's GPU check is dropped |
| 5 | [MEDIUM] Applying `min` to every row changes which workload is guarded | `examples/02_bench/src/physics.rs:74-90` (`min` is pachinko, `sparse-min` sparse); `gpu.rs:334` (`gpu-throughput` only at `throughput`) | accepted | §2.4: a guard table with each row's own preset and scale, and each guard asserts that the bench's `row()` names it |
| 6 | [MEDIUM] The pit script cannot express all the required state assertions | `examples/01_platformer/src/features/run/script.rs:71-90` (probes `player`, `area`); `crates/tungsten-core/src/script.rs:1342` (`count`); `frame_time.rs:186-206` | accepted | Step 2 adds 01's `pit` probe (`balls`, `foes_alive`, `foes_awake`), and §2.1's `e01-pit` and step 2's script assert it |
| 7 | [MEDIUM] The blocking-readback fallback leaves a required downstream dependency unfinished | `docs/plans/roadmap-redo.md:138` (7.15 "on 7.3's non-stalling timestamps") | accepted | §6: fall back to a wait on a submission two frames old, judged by the same ratio. If that fails, the plan does not close and ends with a resume prompt |

Round 2: the same reviewer at xhigh, 2026-10-29, reading the revised draft with the Critique section and the log cut, and a focus note on round 1's changes. Footer: `723.1 s, tokens in 1338532 (cached 1238144), out 11322 (reasoning 6887), commands 24, output 305.5 KB (largest 27.9 KB), usage 5h 24% → 52%, week 29% → 34%`. Five findings, all accepted. Four repair round 1's fixes and one, 2.4, is new. No third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] The workload assertions can approve an incomplete GPU scene | `examples/02_bench/src/gpu.rs:535` (`bench:` echoes `params.sprites`, `glow`, `lights`) | accepted | §2.4: populations counted in the World; instances equal to the drawable entities counted by the extract's rule; the assertion runs before a write too. Step 2's scratch failure runs under `TUNGSTEN_GUARDS_WRITE=1`, with its pin unchanged |
| 2.2 | [MEDIUM] The headless sprite helper cannot reproduce the existing batching workload | `examples/02_bench/src/gen.rs:132-140` (one handle per packed page); `particles.rs:440`; `crates/tungsten/src/sprite_extract.rs:427` (batch key: handle, filter, material) | accepted | `register_sprite` became `allocate_texture_handle`, with the bench's own packing run through a texture sink so pages, filters and lit flags match. Step 4 checks the headless batch count against the window's new `sprite_batches` |
| 2.3 | [MEDIUM] GPU tail frames change the supposedly unchanged digest | `examples/02_bench/src/counters.rs:4-6` (a frame's `bench:` line prints in the next frame); `scripts/bench_report.py:292-302`, `:420-424` | accepted | §2.2: the digest excludes the last window frame's `bench:` line and the tail frames, as the old runner's did. Step 3 tests a run with tails against one without |
| 2.4 | [MEDIUM] Schema 2 does not specify enough workload provenance for safe comparisons | `scripts/bench_report.py:776-792` (the hard gates) | accepted | §2.2: the capture's `workload` and `renderer` blocks, and every old hard gate read from them. Step 3 tests suppression on each gate |
| 2.5 | [MEDIUM] Parallel guard tests can overwrite each other's pins | §2.4's shared `guards.json`; cargo runs tests in parallel | accepted | One pin file per guard, written through a temporary file and a rename. The file lists and step 2's checks follow |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-29 | pass | `world_counters_` 4 listed, 4 passed; `counter_guard_` 5 listed, 5 passed (three reruns alike); `cargo test -p tungsten-core` 1,170 + 33 + 18 pass; `cargo test -p tungsten` 743 + 27 pass; `just api` adds `WorldCounters`, `World::counters`, `testing::{Allocations, CounterGuard, CountingAlloc, allocations}` and `Harness::allocate_texture_handle`, removes nothing; `just docs-ratchet` "no rise"; fmt and clippy (core, umbrella, all targets) exit 0. Outside the file list: the counts sit in `ecs/storage.rs` (`move_row`, `spawn`, `despawn`), the flush reads `ecs/event_queue.rs`'s new `sent_since_flush`, and core's `lib.rs` re-exports `WorldCounters`. DESIGN.md has no testing section: its sentence is in Performance and Profiling | `crates/tungsten/src/testing/guard.rs` |
| 2 | 2026-10-29 | pass, one check failed before the step | Bench: `cargo test -p example-02-bench --locked guards_` lists 7 (`guards_physics`, `_physics_sparse`, `_ecs`, `_churn`, `_particles`, `_gpu`, `_gpu_throughput` at scale 0.01), passes in 0.23 s, alike with `--test-threads=1` and run alone; none skipped under Q16 (`gpu`'s material split is local: `scene::rebuild_heavy`). Seeded failure (scratch: `gen::register` skips `bench_glow`): `guards_gpu` fails "window frame 0: the extract drew 9228 instances of 9428 drawable", under `TUNGSTEN_GUARDS_WRITE=1` too; `git diff --exit-code -- examples/02_bench/src/tests/guards/` exit 0; scratch removed. Digests before → after the split, old runner, `--repeat 1`: physics `c10885dddd6115a4` = ; physics-sparse `9df5ade5305646dc` = ; ecs `c785506ced04de4c` = ; churn `86014148b7a94681` = ; gpu `f45f26ebdf706840` (and its GPU run) = ; gpu-throughput `121f8ef1503d8697` (and GPU run) = ; particles `08a3c7c126a0d43a` =. 01: `--test game guards::` lists 1, passes in 0.59 s. Write once, then two plain runs of both commands pass; `git status --porcelain` shows only the 8 new pins. The default sprite rule counts the extract's cull (`outside_view`), since particles cull 2 at the view's edge. 01's pin reads 0 sprite batches: headless 01 loads no sprite assets. `cargo test -p example-02-bench` 14 + 4 pass; `-p example-01-platformer` 443 pass, 1 ignored (incl. `the_pit_constants_restate_level_json`). GPU: `tungsten run --script` (start_run 2, fill_pit 12,000 at 30, wake_foes at 630) → pit at 900 `balls` 12000, `foes_alive` 6, `foes_awake` 6, exit 0; `fill_pit` at frame 3 fails "no run" before the run's state enters. `just smoke` exit 0 (15/15 benchmark rows). fmt, clippy exit 0. **Failed:** `cargo test -p example-01-platformer --release --locked --test game frame_time -- --ignored`: scenario line "balls 12000, enemies 6", window 0 passes, then "window 1: 4 awake". HEAD's own `frame_time.rs` on this tree fails identically ("window 1: 4 awake", deterministic), so the failure predates the step and the move kept behaviour; a follow-up | `examples/02_bench/src/tests/guards/`, `examples/01_platformer/tests/game/guards/pit.json`; scratch `pit-script.json` |
| 3 | 2026-10-29 | pass | `python3 -B scripts/test-perf.py`: 24 tests OK, no GPU or cargo (parsing, guards, validity on fake games, the digest with and without tails, an export's provenance in a temp repository, each of 13 hard gates, `R`'s Welch fixtures, RSS within and across sittings, budgets). `just perf-test` 37 + 24 OK. The list check prints nothing. Parity, same tree, `--repeat 1 --allow-background`, old → new, every new capture valid: physics `c10885dddd6115a4` =, physics-sparse `9df5ade5305646dc` =, ecs `c785506ced04de4c` =, churn `86014148b7a94681` =, gpu `f45f26ebdf706840` =, gpu-throughput `121f8ef1503d8697` =, particles `08a3c7c126a0d43a` =. Valid with their reports passing: e01-pit (total p50 30.51 ms, pit asserts at frame 1110), template (player `/health/0` 5 > 0), e01-area-clearing, -aqueduct, -ravine, -hollow, -mine, -bridge (`area` == each name; 01's passes include `post0_bloom_threshold`, `_composite`, `post1_vignette`). `compare` aligned vs `--plain` ecs: "Verdicts suppressed: build flags: … vs -C force-frame-pointers=yes". No `__pycache__`; `just script-test` and `just repo-check` exit 0. Choices: the bench's `stage.gpu` is ported as `gpu.scene` (the GPU run's `scene` pass, §2.5); the template, which this plan does not edit, has no state probe, so its row asserts the `player` probe's health instead of gameplay on top; area points stand on `clearing`, `aqueduct_floor`, `rv_ledge_2`, `hollow_floor_1`, `cut_floor_1` and `troll_bridge`, held by `place_player` | `scripts/perf.py`, `scripts/perf_stats.py`, `scripts/perf-rows.json`; `perf-runs/20261029T1536*`–`1541*` |

## Follow-ups

- 01's release `frame_time` test fails at HEAD `6664ed3b`'s own code: window 1 reads 4 of the six foes awake (≥ 5 asserted), deterministically. The e01-pit row's single window (wake + 480 frames) and the frame-900 script read 5 and 6. Owner: the test's assertion or the foes' dormancy rule (`StateMachine::is_held`).
