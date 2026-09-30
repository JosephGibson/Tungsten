# Benchmark suite redesign

status: done
goal: replace the four `STRESS_SCENE` scenes, `scripts/perf-capture.sh` and the `docs/perf/` content with six scalable, deterministic benchmarks (`physics`, `ecs`, `churn`, `gpu`, `particles`, `integrated`), each owning one bottleneck, plus a standard-library Python runner with suite runs, capacity search, peak-RSS capture and a noise-aware compare report (Markdown and self-contained HTML).
non-goals: Criterion benches in `crates/*/benches/` beyond updates an API change forces; any engine feature under "Gaps for approval" until approved one by one; per-frame allocation counting (gap M1); CI perf gating (`D-070`); changes to `tungsten.json` pacing defaults; interleaved A/B builds or cross-machine normalization; Windows/macOS capture parity; changes to examples 01, 03 and 04; Git commits (the owner handles Git).
files to touch: see "Files to touch".
ordered steps: P1 harness, runner and physics; P2 compare and capacity search; P3 ecs and churn; P4 gpu, particles, approved telemetry gaps and the visual test; P5 integrated and suite runs; P6 retire the old suite, rewrite the docs, record the decision. Details under "Phases".
done-when: see "Done when". In short: `just smoke` runs every benchmark at `min` and `default`; capacity search reports a maximum scale for every tracked row; `just perf compare` writes Markdown and HTML reports with peak RSS, and `just script-test` covers it; the perf docs and `docs/LLM_INDEX.md` name only the new suite; `just check`, `just repo-check`, `just ctx` and `just visual` pass.

## Context digest

- Branch `0.32`, workspace `0.31.0`, written 2026-09-30. Today `examples/02_sprite_stress/` selects one of four scenes through `STRESS_SCENE`: `baseline` (2,000 sprites, present-bound), `ecs-high-load` (50,000 agents; the cost is mostly a spatial-hash neighbor search), `physics-stress` (3,000 circles that fall asleep at frame 286 of 360) and `render-features` (4,000 sprites). `scripts/perf-capture.sh` is 1,166 lines of bash and awk. `scripts/smoke-examples.sh` runs the M25 MSAA × depth-sort matrix and the render-features GPU-timing row on this package. `just visual` compares the `baseline` scene with `baseline-sprite-stress.png`.
- The design relies on these engine facts:
  - Physics: bodies are `Static` or `Dynamic`, shapes are AABB or circle, and there are no sensors. Every touching contact emits one `CollisionEvent` per substep, and events drain once per frame. Island sleeping follows `D-065`. Tile collision layers push one static proxy per solid tile every frame.
  - ECS: queries take at most 3 required and 2 optional components. Every `CommandBuffer` insert is a boxed command and a separate archetype move.
  - Rendering: the default sprite extract doesn't cull and sorts every sprite each frame. `LIGHT_CAP` is 16.
  - Particles and animation: the particle and tween stages run outside `update_ms` and `flush_ms`. `ParticleBudget` defaults to 10,000. Animation has no engine system, and each frame change clones a `String`.
  - Assets: lit textures, particle configs, animations and tilemaps can all be registered from code. Material shaders must be manifest `.wgsl` files.
- Tooling constraints: CI runs `just script-test` on ubuntu-latest with no GPU; the new scripts target Python 3.12 (ubuntu-24.04) and the standard library only. `AGENTS.md` sits at its 6,144-byte `just ctx` limit. `docs/LLM_INDEX.md` and skill bodies are capped at 8 KiB. `just smoke` uses the dev profile, which builds workspace crates at opt-level 0. `perf-runs/` is gitignored.
- Overlap: the draft `docs/plans/debug-cleanup-docs-pass.md` also edits `smoke-examples.sh`, `profiling-workflow.md`, the `tungsten-perf` skill, DESIGN's perf section, `LLM_INDEX.md` and `check-repo.py`. Whichever plan lands second rebases on the other. This plan rewrites the perf doc and the skill wholesale, so that plan's trims to those two files become moot.

## Decisions for approval

Defaults apply unless you say otherwise.

- **A1 Location.** Rewrite the suite as `examples/02_bench/` (package `example-02-bench`): one binary, six benchmarks selected at runtime, one shared harness. It is built beside `02_sprite_stress` during P1–P5, so every phase stays runnable and the old suite stays available for side-by-side checks. P6 deletes the old package. Alternative: six example packages plus a shared helper crate, which means more build targets and a new workspace library for example code.
- **A2 Scale transport.** The binary reads environment variables (see "Scale configuration"). It owns the knob schema and prints it on request, and runner flags map 1:1 onto the variables. Alternatives: CLI flags, although the smoke script and the visual test only pass environment variables; or a JSON config file, which becomes a second source of truth for knob ranges.
- **A3 Tooling language.** Python 3.12 standard library, in three files:
  - `scripts/bench.py`: build, run, suite, capacity search and baselines;
  - `scripts/bench_report.py`: parsing, statistics, verdicts and the Markdown and HTML reports;
  - `scripts/test-bench.py`: the tests.

  It replaces bash because the new work needs structured config validation, `os.wait4` rusage, bisection, statistics and HTML generation. `just perf <subcommand>` wraps it. Alternative: keep a bash runner and use Python only for analysis.
- **A4 Capture defaults.** Captures are telemetry-only by default, and `--profile` opts into profilers; today it is the other way round. Compare-grade captures use `--repeat 5`, and a verdict needs at least 3 runs per side.
- **A5 Verdicts.** The rule under "Compare" replaces the current policy that regressions above 10% are noteworthy.
- **A6 Physics sleeping.** Sleeping stays enabled, as shipped. A validity guard rejects any capture that has a sleeping body in the measured window. `sleep=off` is a diagnostic knob only.
- **A7 Visual test.** The test moves to the `gpu` benchmark's `visual` preset, with a regenerated fixture. Alternatives: an `integrated` visual preset, which covers more but would churn the fixture with every gameplay tweak; or keeping the old sprite grid as a non-benchmark fixture mode, which keeps the PNG but contradicts "replace".
- **A8 Retired numbers.** The April 2026 Vulkan pacing matrix is the only evidence for the shipped `Immediate / 1` default, so it moves into the new decision entry. Other historical numbers stay in `CHANGELOG.md`, `DECISIONS.md` and Git history, and none of them is comparable with the new suite. Alternative: drop the matrix.
- **A9 Default calibration.** In each benchmark's phase, tune the primary counts so the default p95 total lands between 8 and 16 ms on the reference machine, then record the final defaults in `docs/perf/benchmarks.md`. Physics keeps 8,000 balls and tunes `fill` instead.
- **A10 RSS series.** Besides peak RSS, the runner samples `/proc/<pid>/statm` at 10 Hz and reports a growth rate over the second half of the run. The growth rate is judged only for `churn`, as a leak check. Alternative: peak RSS only.
- **A11 Audio stays out.** The root manifest's sounds load but nothing plays. The mixer runs on the `cpal` thread outside frame timings and depends on the audio device.

## Gaps for approval

Engine features. The design works without each of them; each would extend a benchmark.

| ID | Gap | Would enable | Without it |
| --- | --- | --- | --- |
| G1 | Kinematic bodies: velocity-driven, infinite mass, velocity transferred to dynamics | Moving paddles and spinners in `physics`; moving platforms in `integrated` | Bumper pegs plus the anti-stall rule. Moving a `Static` body by writing `Position` is no substitute: the solver sees zero velocity and pushes only through penetration recovery, capped at `max_push_speed` (120 px/s). |
| G2 | Sensor colliders: overlap only, enter/exit events, no contact response | Real bin sensors | Solid bin floors whose first contact event triggers the teleport. The consumer pattern is the same. |
| G3 | More than 16 lights, via a storage buffer or tiled/clustered lighting; would supersede part of `D-061` | A light sweep past 16 | A sweep over 0–16. The extract keeps the 16 lights nearest the view. |
| G4 | GPU particle simulation | GPU-side particles | The CPU path of `D-051` |

Instrumentation. Both changes fit `D-038` (inline `Instant` timing), and I recommend approving both.

| ID | Gap | Would enable | Without it |
| --- | --- | --- | --- |
| T1 | `particles_ms` and `tweens_ms` in `FrameTimings` and the `frame:` line, timed around `stage_particles` and `stage_tweens` | The owned metric of `particles`, and attribution in `integrated` | The analysis reports `unattributed = total − Σ stages`, which then also holds the particle, tween and event-flush time |
| T2 | One `startup:` line with window creation, renderer init, manifest load, user startup, audio init and total | Load-time metrics for every benchmark and in compare reports | Load time stays unmeasured |

Tooling:

| ID | Gap | Would enable | Without it |
| --- | --- | --- | --- |
| M1 | A counting `#[global_allocator]` | Allocations per frame for every benchmark, such as boxed commands in `churn` and `String` clones in animation | Not measured. It needs a `DECISIONS.md` entry, because the allocator's counters are process-global mutable state (atomics in a `static`), which AGENTS.md forbids. Proposed scope if approved: `example-02-bench` only, behind a cargo feature, atomics only, and used only in a separate diagnostic run like GPU timing, since counting costs time. A code-free interim option is `heaptrack` over two window lengths; the difference gives steady-state allocations per frame. |

Observations, not gaps. The suite exposes these costs on purpose; fixing them is separate engine work:

- Tile collision proxies are rebuilt from a full-map scan every frame (`gather_tilemap_proxies`).
- The default sprite extract doesn't cull and sorts every sprite every frame.
- There is no bundle insert, so spawning an entity with k components costs k boxed commands and k archetype moves.
- Sprite IDs are `String`s: each animation frame change allocates, and the extract resolves IDs by string.

## Feedback on the proposed benchmarks

**Physics.**
- Constant motion plus recirculation is the right fix. It removes the two-regime capture and the parser's awake-phase special case.
- Mixed radii are good, because a uniform grid degrades as sizes spread: large bodies span many cells and small ones crowd cells. Keep the default ratio at 3:1 (9 px against 3 px, with a 32 px cell) and offer a `wide` 8:1 mix.
- Agitators: the engine has no kinematic bodies (G1), so paddles and spinners can't be built honestly. Existing features give an alternative: 20% of the pegs act as pinball bumpers driven by collision events, and an anti-stall rule kicks or re-spawns any ball that stays slow.
- Sensor bins: there are no sensors (G2). Solid bin floors read through collision events give the same consumer pattern.
- Collision events are plentiful without extra work: the engine emits one per touching contact per substep, so volume is about 4 × the touching contacts. The benchmark consumes them for bumpers and bins, which also measures event-driven gameplay load.
- "Contact solving" here mostly means contact generation. Per `D-067` and the `step.rs` notes, at 25,000 awake bodies the contact-solve passes take about 2% of the frame; the cost is pair query, narrow phase and contact build. The owned metric is therefore `physics_step`, with pairs and contacts recorded as workload counters.
- Sparse mode: agreed. Speed is its main knob, because speculative margins grow each proxy's AABB with its travel (`D-075`), which raises cell references per proxy. It needs speed renormalization, because the soft solver dissipates energy, and a low density (1% area fraction) so pairs per proxy stay low.
- The default is 8,000 bodies (down from the proposed 10,000). The old 3,000-body pile averaged 2.67 ms while awake, so 8,000 bodies in active flow should put `physics_step` near 6–10 ms.

**ECS.**
- Agreed. Today's `ecs-high-load` mostly measures a neighbor search plus physics integration and the camera. The new benchmark measures iteration only.
- Queries take at most 3 required and 2 optional components. "Many components" therefore means 16 components per entity, touched by 14 systems with different query shapes, rather than one wide query.
- Add archetype fragmentation and random access as knobs. After entity count, they drive iteration cost the most.
- Render a fixed 4,096-entity sample, so render cost doesn't grow with scale.

**Entity churn.**
- Agreed, and it will expose a real cost. Without a bundle insert, spawning an entity with k components takes k boxed commands and k archetype moves, which is O(k²) column copies.
- Add a `mode` knob. `deferred` goes through the `CommandBuffer` (`D-039`); `immediate` makes direct `World` calls inside a system. Comparing the two separates command-buffer overhead from raw archetype moves.
- Use FIFO lifetimes for a steady, deterministic population, and toggle status components to move existing entities. Make it the leak detector (RSS growth, A10).

**GPU.**
- Agreed on sprite counts, the light sweep, the scrolling tilemap, changing glyphs and shader load.
- I read "client-side load from shaders" as per-fragment shader cost on the GPU. A bench material loops a count taken from a uniform, so one knob scales fragment cost without a recompile. The material path accepts full WGSL (`D-058`).
- The light sweep is capped at 16 (G3).
- Particle effects: particles are CPU entities (`D-051`), so emitters here would add simulation and churn that belong to other benchmarks. Instead, this benchmark draws what particles cost the GPU, many small overlapping alpha-blended sprites, as a fixed "glow" layer.
- Changing text is mostly CPU work: every changed section is reshaped with advanced shaping during the render stage. It stays here as render-path CPU cost.
- Other taxing loads worth adding:
  - batch fragmentation: interleaved z layers, textures, materials and lit sprites raise draw calls and pipeline switches;
  - MSAA 4× against SMAA High;
  - bloom mip count;
  - resolution.
- Keep a `throughput` preset (400,000 small unlit sprites, no post) to track CPU render prep (`extract`, `render_encode`), the job the old 100k row did.

**Particles and animations.**
- Agreed. This benchmark owns particle emit, tick and count refresh, plus animation playback.
- The particle stage has no timing today, because it runs between update and flush. Hence T1.
- The benchmark raises `ParticleBudget`, a resource, to reach about 60,000 live particles.
- It declares a 180-frame warm-up. With lifetimes up to 2.5 s and the 1/60 s smoke dt, that is what steady state takes.
- Animation playback is game code around `AnimationState::advance`, since the engine has no animation system. The benchmark uses the efficient query path, so what it measures is the engine's share: registry lookup by string, a `String` clone per frame change, and the `Sprite.asset_id` rewrite.

**Integrated.**
- Agreed. It is the only benchmark judged on total frame time and its tail (p99, jitter).
- Generate the level procedurally instead of reusing `examples/01_platformer/`, which `platformer-polish-pass.md` is reworking. A moving workload would invalidate comparisons.
- Drive the camera on a deterministic path (`CameraMode::Scripted`).

**Tooling.**
- Scaling: one `scale` multiplier plus per-knob overrides and named presets. The binary describes its own knobs, so the tooling can't drift from it.
- Capacity search: define the statistic (p95 of `total` by default), probe with short runs, confirm the boundary with repeats, and report the limiting stage, so each result also checks the benchmark's intended bottleneck. Results are machine-specific and informational (`D-070`).
- Analysis: run comparability checks before any verdict, treat runs as the statistical unit, and add an A/A self-check.
- Memory: the runner reads peak RSS with `os.wait4`, so no engine change is needed. The allocation counter is gap M1.
- Replace, don't layer: agreed. The new suite is built alongside the old one and the old one is deleted in P6.
- Additions:
  - per-frame workload counters, plus a determinism check across repeats;
  - a validity guard per benchmark;
  - a warm-up length per benchmark;
  - `--sweep`, for the light sweep and density curves.
- Not included: a separate load-time benchmark covering atlas packing and manifest parsing. You asked for six benchmarks; T2 reports startup time for every benchmark instead.

## Ownership

Each benchmark is judged only on the costs it owns. Every other metric is reported but never produces a verdict that counts against that benchmark.

| Cost | Owner, judged on | Also present in, reported only |
| --- | --- | --- |
| Narrow phase, contact build, solve, collision events, sleep bookkeeping | `physics` (pachinko): `physics_step` | `integrated` |
| Proxy gather, broadphase build, pair query | `physics` (sparse): `physics_step` | pachinko, `integrated` |
| Query iteration, column access, random entity lookup | `ecs`: `update`, per-system rows | all |
| `CommandBuffer` recording and flush, archetype moves, entity allocation and free | `churn`: `flush`, churn system rows, RSS growth | `particles`, `integrated` |
| Particle emit, tick and count refresh; animation playback | `particles`: `particles` stage (T1), `animate_sprites` row | `integrated` |
| Default extract, encode, text shaping, GPU passes | `gpu`: `gpu`, `render_span`, per-pass rows, `extract`, `render_encode` | `particles`, `integrated` |
| Cross-system effects: tile proxies × bodies, event → spawn cascades, batch fragmentation, full-frame composition | `integrated`: `total` p50/p95/p99, p99 − p50 | — |
| Acquire and present pacing | Nobody: this is environment, reported and never judged | all |

## Benchmark designs

### Common harness

- **One binary.** `example-02-bench` resolves its configuration (see "Scale configuration") and calls `<bench>::configure(&mut app, &cfg)`. Without `TUNGSTEN_BENCH` it runs `physics` at `default`, which is also the `cargo run -p example-02-bench` demo. It keeps `TUNGSTEN_OVERLAYS_ON` for interactive inspection.
- **Determinism.** Every random choice comes from `Pcg32::seeded(seed)`. Captures run under `TUNGSTEN_SMOKE_FRAMES`, which pins dt to 1/60 s, so a given build replays an identical workload. No rendered text depends on wall-clock time: there are no FPS readouts, and the engine HUD covers interactive use.
- **Assets.** `gen.rs` generates discs, soft dots, sphere and bevel normal maps, emissive masks, tiles, parallax strips and animation frames. They are registered through `AssetRegistry::register_sprite` and `Renderer::upload_lit_texture`, like today's `__generated__` sprites. `ParticleConfigRegistry::register`, `AnimationRegistry::insert` and `TilemapRegistry::insert` cover the other asset types.
  - The only asset files are `examples/02_bench/assets/manifest.json` and the `bench_heavy` shader and material it lists (`D-057`, `D-058`). This manifest loads after the root manifest (`D-035`, `D-052`), and root fonts serve all text. IDs are prefixed `bench_`.
  - Example-local shaders fall outside `shader_coverage.rs`. They are validated when they load, which the smoke rows exercise.
- **Lean view.** `view.rs` provides a culled, unsorted extract over `Position` plus a small visual component. `physics`, `ecs` and `churn` use it, so their render cost stays flat as scale grows. `gpu`, `particles` and `integrated` use the engine extracts, whose cost they own or report.
- **Telemetry lines.** `bench-config:` (once, JSON) and `bench:` (every frame) use the `log` target `bench`, the same stream as the engine lines. The first system of frame N + 1 logs frame N's `bench:` line, so it follows frame N's `frame:`, `systems:`, `gpu_passes:` and `physics:` group like every other companion line. The last frame has no `bench:` line.
- **Workload version.** Each benchmark has a `workload_version` constant, starting at 1. A bench code change that alters the work bumps it; engine changes never do.
- **Dependencies.** `serde_json`, already a workspace dependency, formats the JSON lines. No new dependency is added.

### 1. `physics`

Pachinko owns the narrow phase, contact build, solve, events and sleep bookkeeping. Sparse owns proxy gather, broadphase build and pair query. Both are judged on the `physics_step` system row (p50 and p95) and on `update` p95. Rows: `physics` and `physics-sparse`. Warm-up: 120 frames.

**Pachinko world at the default scale (8,000 balls):**

| Item | Value |
| --- | --- |
| Balls | Radii 3, 4.5, 6 and 9 px at 35/30/25/10% (2,800 / 2,400 / 2,000 / 800 balls); mean area 82.7 px²; largest diameter `d_max` = 18 px; restitution 0.3; gravity 900 px/s² |
| Pegs | Static circles, `r_p` = 5 px, in staggered rows. Horizontal pitch `p_x = max(64, ceil16(3·d_max + 2·r_p))` = 64 px; vertical pitch `p_y = 0.75·p_x` = 48 px; odd rows offset by `p_x/2`. The in-row gap is 54 px (3.0 `d_max`) and the diagonal gap 47.7 px (2.65 `d_max`), so no ball can wedge between two pegs. The pitch follows `d_max` for the other radius mixes. |
| Vertical layout, top to bottom | Ceiling (static AABB, 80 px thick); spawn band, 128 px; peg field, 32 rows = 1,536 px; bin zone, 160 px. Interior height H = 1,824 px. |
| Bins | Dividers every `2·p_x` = 128 px (static AABBs 4 px wide, each capped with a static circle so nothing rests on a flat top); one static AABB floor per bin, tagged `BinFloor` |
| Width | `W = N·Ā / (fill·H)`, rounded up to whole bins: 8,000 × 82.7 / (0.21 × 1,824) = 1,727, so W = 1,792 px (14 bins, 28 peg columns). The actual fill is 0.202. |
| Statics | About 880 pegs (about 176 of them bumpers), 2 walls, the ceiling, 13 dividers with caps and 14 floors: about 920 static proxies |
| Camera | Zoom = `min(1920/W, 1080/H)`, clamped at the fit-height value 0.592, centered. The whole default board is visible (a 3,243 × 1,824 world-px view), and the smallest ball is 3.6 px on screen. |
| Scaling | Balls scale with `scale` and H stays fixed, so W grows in whole bins. Beyond about 1.9× the camera stays centered, and the culled view draws only the visible balls (about 15,000 at most). Render cost plateaus while physics keeps growing. |
| `min` preset | 1,000 balls; W clamps to 4 bins (512 px), for a fill of 0.089 |

**How every body stays awake.** The engine puts an island to sleep only when every member has stayed below 20 px/s for 0.5 s (`D-065`). Four mechanisms keep each ball's slow streak shorter than that:

1. **Recirculation.** A bin floor teleports a ball to the spawn band on its first floor contact, so no pile forms anywhere. Every ball is always falling, bouncing or respawning.
   - The consumer is a system registered after `physics_step`. It reads `EventQueue::<CollisionEvent>::iter_current()`; `iter()` would also yield the previous frame's window and apply each event twice.
   - A teleport writes `Position` and `Velocity` directly, and the next gather picks the change up (`D-066`).
   - Targets use a seeded-random x across the board, a y inside the band and a small downward velocity. Overlaps with balls already in the band resolve through the soft push-out, which is capped by `max_push_speed`.
2. **Bumpers.** A fixed 20% of the pegs set any touching ball's velocity to at least `kick` (260 px/s) along the contact normal. This breaks symmetric stacks and throws balls sideways and upward.
3. **Anti-stall.** A bench component times each ball's streak below 30 px/s. That threshold is stricter than the engine's 20 px/s, so this timer always runs ahead of the engine's.
   - At 0.25 s the ball gets a seeded kick of 200 px/s, upward and sideways.
   - At 0.4 s the ball respawns in the band.

   No ball can reach the engine's 0.5 s, so no island can sleep.
4. **Guard.** The runner rejects a capture if any measured frame's `physics:` line shows `sleeping > 0`. `stall_kicks` and `respawns` should stay near zero; sustained values mean the geometry jams, and `fill` or the peg pitch needs retuning.

**Sparse world at the default scale (50,000 bodies):**
- Circles of radius 2 px, speeds 600–1,400 px/s in random directions, no gravity, restitution 1 on bodies and on the static AABB walls.
- Arena area = `N·π·r² / density`. At density 0.01 that gives 10,560 × 5,952 px (16:9, in multiples of 32).
- Speeds are renormalized into `[speed_min, speed_max]` every frame, far above 20 px/s, so nothing ever sleeps. The same guard applies.
- The camera zoom is 0.5, centered, for a 3,840 × 2,160 world-px view: about 13% of the arena, or about 6,600 visible bodies. That count stays constant as the arena grows with scale.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `mode` | `pachinko` | `pachinko`, `sparse` | | |
| `balls` | 8,000 | 200–400,000 | yes | Board width follows |
| `fill` | 0.21 | 0.05–0.40 | | Ball area ÷ board area |
| `radius_mix` | `mixed` | `uniform`, `mixed`, `wide` | | `uniform`: 4.5 px; `mixed`: as above; `wide`: 2/4/8/16 px at 40/30/20/10% |
| `peg_radius` | 5 | 2–12 px | | |
| `bumpers` | 0.2 | 0–1 | | Share of pegs that kick |
| `kick` | 260 | 0–1,000 px/s | | Bumper exit speed |
| `gravity` | 900 | 100–3,000 px/s² | | |
| `restitution` | 0.3 | 0–1 | | Balls |
| `bodies` | 50,000 | 1,000–2,000,000 | yes | Sparse only; arena area follows |
| `radius` | 2 | 1–8 px | | Sparse only |
| `speed_min`, `speed_max` | 600, 1,400 | 50–8,000 px/s | | Sparse only |
| `density` | 0.01 | 0.001–0.05 | | Sparse only; body area ÷ arena area |
| `substeps` | 4 | 1–8 | | `PhysicsConfig::substeps` |
| `iterations` | 1 | 1–8 | | `PhysicsConfig::solver_iterations` |
| `cell` | 32 | 8–256 px | | `PhysicsConfig::broadphase_cell_size` |
| `sleep` | `on` | `on`, `off` | | The guard requires zero sleepers either way |
| `render` | `culled` | `culled`, `none` | | `none` draws no bodies |
| `seed` | 1 | u64 | | Present on every benchmark |

- **Presets:** `min`, `default`, `sparse-min` (2,000 bodies) and `sparse`.
- **Counters:** `balls` or `bodies`, `teleports`, `bumper_kicks`, `stall_kicks`, `respawns`, `events` and `visible`. The engine's `physics:` line adds proxies, dynamic, sleeping, pairs and contacts.
- **Guards:** no sleepers in any measured frame. In pachinko, `teleports > 0` in every measured frame.
- **Calibration checks.** Pachinko needs at least 1.0 contact per ball, otherwise raise `fill`. Sparse needs pairs per proxy ≤ 0.5 and contacts per proxy ≤ 0.2, and the grid build and pair query must lead the `physics_step` samples in a `--profile` capture. Otherwise, lower `density`.

### 2. `ecs`

Owns steady-state query iteration. Judged on `update` (p50 and p95) and the per-system rows. Warm-up: 60 frames.

- **Components.** Each entity carries up to 16 components of mixed sizes: `Pos`, `Vel`, `Acc`, `Heading`, `Health`, `Regen`, `Cooldowns([f32; 4])`, `Brain` (state, timer, seed), `Team`, `Faction`, `Stats([f32; 8])`, `Tint`, `Phase`, `Age`, `Follow(Entity)` and `Bag([u16; 16])`. On top of those, zero-sized tags `T0`–`T7` fragment the archetypes.
- **Systems.** 14 systems, each a different query shape:
  - dense writes: integrate, accelerate, heading via atan2;
  - sparse matches: regen, stats decay;
  - branching: cooldown timers, the brain state machine, bounds wrap;
  - random access: follower → leader `get::<Pos>`;
  - read-only reductions: bag totals per team, a faction histogram;
  - the `query2_opt2` optional-column path;
  - cheap ticks: tint from health, age and phase.
- **Invariants.** No structural changes happen after startup. Only the `view` sample carries sprites.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `entities` | 250,000 | 1,000–4,000,000 | yes | |
| `fragmentation` | 8 | 1–256 | | Tag-component combinations |
| `optional` | `on` | `on`, `off` | | Optional-component patterns: `Acc` 80%, `Regen` 40%, `Stats` 50%, `Bag` 25% |
| `followers` | 0.1 | 0–0.5 | | Share doing a random-access leader lookup |
| `systems` | `all` | `all`, `dense`, `sparse` | | `dense`: whole-population systems only; `sparse`: optional-component systems only |
| `view` | 4,096 | 0–65,536 | | Rendered sample, fixed across scale |

- **Presets:** `min` (1,000 entities) and `default`.
- **Counters:** `entities`, `archetypes`, `lookups`, `structural`, plus a per-frame digest of positions.
- **Guards:** `structural == 0`, and a constant entity count. The digest must match across repeats; it isn't compared across captures.
- **Calibration check:** `update` is at least 70% of `total`.

### 3. `churn`

Owns structural change: command recording, `World::flush`, archetype moves, and entity allocation and free. Judged on `flush` (p50 and p95), the churn system rows, and RSS growth. Warm-up: 60 frames. The population is pre-aged, so churn is steady from the first frame.

- **Population.** A steady population is replaced FIFO: every entity lives exactly `1/turnover` frames, and each spawn inserts `components` components, one archetype move each.
- **Status toggles.** A share of the population gains or loses one of `statuses` components every frame, which moves an existing, wide entity.
- **`deferred` mode** records everything through `CommandBuffer` and measures it in `flush`.
- **`immediate` mode** performs the same operations directly on `World` inside a system and measures them in that system's row. The difference between the two modes is the command-buffer overhead.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `population` | 100,000 | 1,000–2,000,000 | yes | |
| `turnover` | 0.05 | 0–0.5 | | Share replaced per frame: 5,000 spawns and 5,000 despawns at the default |
| `toggles` | 0.05 | 0–0.5 | | Share gaining or losing a status per frame |
| `components` | 6 | 2–12 | | Components per spawn |
| `statuses` | 3 | 1–8 | | Distinct status components |
| `mode` | `deferred` | `deferred`, `immediate` | | |
| `view` | 0 | 0–16,384 | | Rendered sample |

- **Presets:** `min` (population 1,000) and `default`.
- **Counters:** `population`, `spawned`, `despawned`, `inserted`, `removed` and `commands`.
- **Guards:** the population stays constant, and `spawned == despawned` in every frame.
- **Calibration check:** `flush` plus the churn systems make up at least 60% of `total`.

### 4. `gpu`

Owns the render path: the default extract, encode, text shaping, and every GPU pass. Rows: `gpu`, judged on `gpu`, `render_span` and the per-pass rows (from the GPU diagnostic run) plus `extract` and `render_encode`; and `gpu-throughput`, judged on `extract` and `render_encode`. Warm-up: 60 frames. The GPU diagnostic run is on.

- **Sprites.** A sprite field spread over `z_layers` interleaved layers mixes nearest- and linear-filtered textures. Half the sprites are lit, with normal and emissive maps; 10% use the `bench_heavy` material. `bench_heavy` reuses the vertex stage and bind layout of `damage_flash.wgsl` and loops `material.i.x` times per fragment.
- **Glow layer.** A fixed layer of soft alpha-blended sprites supplies particle-like overdraw.
- **Lights.** `lights` point lights move on seeded paths, plus one directional light.
- **Tilemap.** A render-only tilemap of 2,048 × 512 tiles, `tile_layers` deep, scrolls under a scripted camera. The tilemap extract culls it.
- **Text.** 100 sections of 40 characters each, rebuilt every frame from the frame index.
- **Post chain.** The chain follows `post`, with the `aa` and `bloom_mips` settings on top.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `sprites` | 120,000 | 0–2,000,000 | yes | |
| `sprite_px` | 24 | 4–128 | | |
| `z_layers` | 8 | 1–64 | | Interleaving; drives batch count |
| `lit` | 0.5 | 0–1 | | Share with normal and emissive maps |
| `lights` | 16 | 0–16 | | Point lights (G3 for more) |
| `glow` | 20,000 | 0–500,000 | yes | Overdraw sprites |
| `glow_px` | 48 | 8–256 | | |
| `shader_share` | 0.1 | 0–1 | | Share drawn with `bench_heavy` |
| `shader_iters` | 32 | 0–512 | | Per-fragment loop count |
| `tile_layers` | 3 | 0–4 | | |
| `tile_px` | 16 | 8, 16, 32 | | |
| `scroll` | 600 | 0–4,000 px/s | | |
| `zoom` | 1.0 | 0.25–2 | | |
| `glyphs` | 4,000 | 0–50,000 | yes | 40 per section |
| `text_change` | 1.0 | 0–1 | | Share of sections rewritten per frame |
| `post` | `full` | `none`, `light`, `full` | | `light`: bloom and vignette. `full`: bloom, god rays, fog, chromatic aberration, film grain, color adjust, tonemap and vignette |
| `aa` | `smaa_high` | `off`, `smaa_high`, `msaa4` | | |
| `bloom_mips` | 6 | 1–8 | | |
| `resolution` | 1920x1080 | 1280x720 up to the display size | | |

- **Presets:**
  - `min`: 1,000 sprites, 200 glow sprites, 4 lights, one tile layer, 200 glyphs, `post=light`, `aa=off`.
  - `default`.
  - `throughput`: 400,000 unlit 8 px sprites with one texture and one z layer; no glow, shader, tiles, text, post or AA.
  - `visual`: see "Visual test".
- **Counters:** `sprites`, `glow`, `visible_tiles`, `glyphs_changed` and `lights`. The expected batch count is derived from the layout and logged in `bench-config`.
- **Guard:** the per-pass rows need `timestamp_query: true`. Without it those rows are n/a, and the capture stays valid.
- **Sweeps:** `just perf run gpu --sweep lights=0,1,2,4,8,16`.
- **Calibration check:** at `default`, the GPU span exceeds `update + extract + render_encode`, so the frame is GPU-bound.

### 5. `particles`

Owns particle emit, tick and count refresh, and animation playback. Judged on the `particles` stage (T1) and the `animate_sprites` row, both p50 and p95. Warm-up: 180 frames.

- **Emitters.** Emitters draw on `configs` generated `ParticleConfig`s that span:
  - every emission kind: continuous, burst and pulse;
  - every velocity model: cone, radial and vector;
  - gravity and drag;
  - scale, color and alpha curves of 2–6 points;
  - both blend modes;
  - lifetimes of 0.3–2.5 s.

  The emitters move along Lissajous paths. `ParticleBudget` is raised to `budget`.
- **Animated sprites.** They play `clips` generated clips of 8 frames each, some looping and some one-shot; the bench restarts finished one-shots. `animate_sprites` walks `query2_mut::<AnimationState, Sprite>` and calls `advance`.
- **Render path.** Particles and animated sprites go through the default extract. That cost belongs to `gpu` and is only reported here.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `emitters` | 400 | 0–20,000 | yes | |
| `configs` | 12 | 1–64 | | |
| `rate` | 1.0 | 0.1–10 | | Emission-rate multiplier; about 60,000 live particles at the default |
| `budget` | 100,000 | 1,000–5,000,000 | yes | `ParticleBudget::global_cap` |
| `animated` | 40,000 | 0–1,000,000 | yes | |
| `clips` | 8 | 1–32 | | |
| `anim_fps` | 12 | 1–60 | | |
| `motion` | `on` | `on`, `off` | | Emitter movement |

- **Presets:** `min` (8 emitters, 500 animated sprites) and `default`.
- **Counters:** `live`, `emitters`, `animated` and `frame_changes`.
- **Guard:** `live` stays within ±10% of the calibrated target over the measured window.
- **Calibration check:** the particle stage plus `animate_sprites` make up at least 50% of `total`.

### 6. `integrated`

Owns the effects that only appear when systems interact. Judged on `total` p50, p95 and p99, and on the jitter p99 − p50. Warm-up: 120 frames. The GPU diagnostic run is on.

- **Level.** A seeded generator builds a side-scroller level 96 tiles high (16 px tiles) and `level_tiles` wide. It has a collision layer (ground, platforms, walls, stairs) and two decoration layers. Four parallax layers sit behind it.
- **Actors.** Walkers are dynamic AABBs with tile collision and a patrol AI that turns on wall events. Each has a lit, animated walk clip with normal and emissive maps and a damage-flash material tween.
- **Projectiles.** 10% of the actors are casters that fire circle projectiles at 700 px/s. A hit despawns the projectile, spawns a spark burst, shakes the camera through trauma when near the view, and triggers the flash tween.
- **Scenery.** Dynamic crates sit in piles; sleeping is allowed here, and hits wake them. Lit props are static. Pickups bob through tweens.
- **Torches.** Each torch carries a point light and a fire emitter. The extract culls the lights to 16.
- **Game feel.** Actors squash and stretch on landing.
- **Text.** A HUD shows changing counters, and `tags` name tags follow actors.
- **Post.** Bloom, vignette and tonemap, plus SMAA High.
- **Camera.** The scripted camera ping-pongs along the level at `camera_speed`, so any level length works.

The interaction costs it exposes, each visible through a counter or a stage row:

- tile proxies × dynamic bodies, in the `physics:` line's `proxies`, which `tile_collision=off` isolates;
- collision event → burst → `CommandBuffer` spawn → flush cascades, which show up as tail spikes;
- batch fragmentation from lit, animated, material, parallax, particle and tile sprites interleaved by z;
- light culling with hundreds of off-screen lights;
- text, post and SMAA over the fully composed frame;
- several systems writing `Transform` in one frame (physics sync, animation, squash, extract).

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `level_tiles` | 1,536 | 256–16,384 | | Level width in tiles |
| `actors` | 3,000 | 0–100,000 | yes | |
| `crates` | 2,000 | 0–100,000 | yes | |
| `props` | 6,000 | 0–200,000 | yes | |
| `torches` | 300 | 0–10,000 | yes | |
| `pickups` | 1,000 | 0–50,000 | yes | |
| `fire_rate` | 0.3 | 0–5 per s | | Per caster |
| `tags` | 64 | 0–512 | | Changing name-tag sections |
| `tile_collision` | `on` | `on`, `off` | | |
| `post` | `game` | `none`, `game`, `full` | | `game`: bloom, vignette, tonemap |
| `aa` | `smaa_high` | `off`, `smaa_high` | | |
| `camera_speed` | 360 | 0–2,000 px/s | | |

- **Presets:** `min` (256 tiles, 50 actors, 50 crates, 100 props, 8 torches, 20 pickups, 4 tags) and `default`.
- **Counters:** `actors`, `projectiles`, `hits`, `particles`, `lights` and `camera_x`.
- **Guard:** the view stays inside the level.
- **Calibration target:** a default p95 total of 10–16 ms, a heavy game frame at 60 Hz.

## Scale configuration

The binary reads these environment variables (A2):

| Variable | Meaning |
| --- | --- |
| `TUNGSTEN_BENCH` | Benchmark name; default `physics` |
| `TUNGSTEN_BENCH_PRESET` | Named knob set; default `default`. Every benchmark has `min` and `default`. |
| `TUNGSTEN_BENCH_SCALE` | Float multiplier, default 1.0, on the preset's scalable knobs (the "Scales" column). Integer knobs round and clamp to their range. |
| `TUNGSTEN_BENCH_SET` | `knob=value,knob=value` overrides, applied last |
| `TUNGSTEN_BENCH_DESCRIBE` | `1` prints the JSON schema of every benchmark and exits before a window opens: knobs, types, defaults, ranges, presets, warm-up, GPU-timing default, owned metrics and tracked rows |

Values resolve in this order: preset, then scale, then overrides, then validation. An unknown name or an out-of-range value is a fatal error naming the knob and its range, the same fail-fast rule today's `STRESS_*` parsing follows. The resolved config is logged once:

```text
bench-config: {"bench":"physics","row":"physics","workload_version":1,"preset":"default","scale":1.0,"seed":1,"knobs":{"mode":"pachinko","balls":8000,"fill":0.21,...},"derived":{"world":[1792,1824],"pegs":880,"zoom":0.592}}
bench: balls=8000 teleports=25 bumper_kicks=330 stall_kicks=0 respawns=0 events=30600 visible=8000
```

Runner flags map 1:1 onto the variables: `--preset`, `--scale`, a repeatable `--set k=v`, and `--sweep k=v1,v2,…`. The runner validates every request against `describe` before it launches anything, and it records the resolved config. Capacity search raises `scale`, or a single knob with `--axis`.

The transport is environment variables for four reasons:

- the smoke script, the visual test and the runner already launch with environment variables;
- engine and fixture overrides use the same `TUNGSTEN_*` transport;
- no CLI-parsing dependency is needed;
- the binary stays the single source of truth for its knobs.

```bash
TUNGSTEN_BENCH=physics TUNGSTEN_BENCH_PRESET=sparse TUNGSTEN_BENCH_SCALE=2 \
  TUNGSTEN_BENCH_SET=speed_max=2000,cell=64 cargo run --release -p example-02-bench
just perf run physics --preset sparse --scale 2 --set speed_max=2000 --set cell=64 --repeat 5
just perf run gpu --sweep lights=0,1,2,4,8,16 --repeat 3
just perf suite --repeat 5
just perf capacity --all --budget 144hz
just perf compare main-2026-10-01 perf-runs/20261002T101500Z-physics
```

## Tooling

### Runner (`scripts/bench.py`)

Subcommands, all wrapped by `just perf`:

| Subcommand | Does |
| --- | --- |
| `describe [bench]` | Prints knobs, ranges, presets and tracked rows from the binary |
| `run <bench> [--preset] [--scale] [--set k=v]… [--frames 300] [--warmup N] [--repeat N] [--gpu-timing on\|off] [--present-mode M] [--max-frame-latency N] [--profile [--call-graph dwarf\|fp] [--sample-frequency HZ]] [--sweep k=v1,v2,…] [--compare BASELINE]` | Runs one capture, a sweep, or a capture followed by a comparison |
| `suite [--preset] [--scale] [--only rows] [--repeat N] [--compare BASELINE]` | Runs every tracked row listed by `describe` with the same flags; `--preset` replaces only the preset of rows whose own preset is `default` (P5) |
| `capacity (<bench>… \| --all) [--budget 60hz\|144hz\|MS] [--stat p95] [--axis scale\|KNOB] [--tolerance 0.05] [--frames 180]` | Capacity search |
| `compare <baseline> <candidate> [--out DIR] [--fail-on regressed] [--force]` | Compares two captures or two suites; a baseline name or a directory works on either side |
| `baseline save <capture> <name>` / `baseline list` / `baseline rm <name>` | Keeps named baselines under `perf-runs/baselines/`, which is gitignored and machine-local |

Each capture follows these steps:

1. **Provenance, read before the build.** The runner records:
   - the commit;
   - the dirty-tree fingerprint, computed with today's algorithm;
   - the cpu0 governor and the ACPI platform profile;
   - the AC power state, the kernel and the CPU model;
   - `rustc --version`;
   - the build flags: `TUNGSTEN_PERF_RUSTFLAGS` if set, otherwise `-C force-frame-pointers=yes`, which replaces `.cargo/config.toml`'s `target-cpu=native`, as today.
2. **One release build** of `example-02-bench`.
3. **A timing run for each repeat**, plus a GPU diagnostic run for benchmarks with GPU timing on (`TUNGSTEN_GPU_TIMING=1`), which inflates CPU timings.
   - The child environment clears any inherited `TUNGSTEN_BENCH*`, `TUNGSTEN_GPU_TIMING`, `TUNGSTEN_RENDER_*`, `TUNGSTEN_DISPLAY_*`, `TUNGSTEN_CAPTURE_*`, `TUNGSTEN_OVERLAYS_ON` and `RUST_LOG`.
   - It sets `TUNGSTEN_SMOKE_FRAMES` to warm-up plus frames, `TUNGSTEN_PERF_LOG=1` and `RUST_LOG=tungsten::app=debug,bench=debug`.
   - Present overrides are passed only to the child, as today.
   - The binary runs from the repo root and writes nothing there.
4. **Reap and rusage.** The child is launched with `subprocess.Popen` and reaped with `os.wait4`; the runner sets `returncode` from the wait status, so Popen never waits a second time. rusage supplies `peak_rss_kib` (`ru_maxrss`), user and system CPU seconds, minor and major faults, and voluntary and involuntary context switches. Under A10 the runner also polls `/proc/<pid>/statm` every 100 ms.
5. **Parse and check.** The runner parses the logs, computes per-run statistics, evaluates the guards and the determinism digest, and writes `capture.json` and a human-readable `README.md`.
6. **`--profile`, run-1 only:** perf stat, perf record and a flamegraph folded from that recording, written to `profile/`. This is today's logic, ported.

Capture layout:

```text
perf-runs/<UTC>-<row>[-<preset>][-s<scale>][-set<hash6>][-<present>-lat<N>]/
  capture.json          schema 1: row, bench, workload_version, resolved config, frames, warm-up,
                        build flags, provenance, renderer metadata, per-run stats and rusage,
                        guards, digests, valid flag
  README.md
  run-1/telemetry.log   run-1/gpu.log   run-1/rss.tsv   ... run-N/
  profile/              perf-stat.txt, perf-record.data, perf-record.log, flamegraph.svg
perf-runs/<UTC>-suite/<row>/…  plus suite.json and README.md
perf-runs/<UTC>-capacity-<budget>/  capacity.json, capacity.md, capacity.html, probes/
perf-runs/baselines/<name>/  a copied capture plus baseline.json (name, source, date, machine fingerprint)
```

The parser reads these lines: `backend:`, `frame:` (with the T1 keys when present), `systems:`, `gpu_passes:` (with `render_span`), `physics:`, `bench:`, `bench-config:` and `startup:` (T2). Companion lines attach to the preceding `frame:` line, and missing companions are tolerated. System names are matched as literal keys, as today.

**Validity.** A capture is valid when all of these hold:

- every run exits 0;
- every run reports the requested number of measured frames;
- `bench-config` matches the request;
- the benchmark's guards pass;
- the deterministic counters match across repeats.

An invalid capture stays on disk with `valid: false`. The runner then exits 3, and `compare` refuses verdicts unless `--force` is given. Captures made by the old script have no `capture.json`, and `compare` rejects them with that reason.

### Compare (`scripts/bench_report.py`)

1. **Comparability.** A hard mismatch suppresses verdicts. The hard fields are:
   - row and `workload_version`;
   - resolved knobs;
   - frames and warm-up;
   - build flags;
   - backend, adapter, present mode and frame latency;
   - the machine fingerprint.

   Trajectory-dependent counters (pairs, contacts, events, teleports, hits) that drift by more than 5% raise a "workload drift" warning. Engine changes legitimately move them, so verdicts still appear, marked.
2. **Metrics.**
   - Stages: `total`, `update`, `flush`, `particles` and `tweens` (T1), `hot_reload`, `extract`, `render`, `render_acquire`, `render_encode`, `render_submit_present`, `audio`, and `unattributed`.
   - Every system row and GPU pass row, plus `gpu` and `render_span`.
   - The workload counters.
   - Peak RSS, RSS growth (A10), CPU time, and startup (T2).
3. **Statistics.** The run is the unit: frames within a run are autocorrelated and share run-level state such as clocks, thermals and memory placement. For each run and metric the tool computes mean, p50, p95, p99 and max; percentiles use the nearest rank, as today.
4. **Verdict per metric and statistic** (A5):
   - Δ = mean of the per-run candidate values − mean of the per-run baseline values.
   - The interval is a 95% Welch interval over the per-run values. Each side's variance is floored at (0.5% of its mean)². The t quantiles come from a built-in table, since the standard library has no Student t.
   - The practical threshold is τ = max(τ_rel × baseline, τ_abs). τ_rel is 3% for mean and p50, 5% for p95 and 8% for p99. τ_abs is 0.05 ms for stages and 0.02 ms for systems and GPU passes. For peak RSS, τ is 2% and 2 MiB.
   - Jitter (p99 − p50) takes p99's threshold: τ = max(8% × the baseline's per-run p99 mean, τ_abs). A τ relative to the small jitter itself would read `noisy` in every A/A (P5).
   - `regressed`: the interval is entirely above 0 and Δ > τ. `improved`: the interval is entirely below 0 and Δ < −τ. `unchanged`: the interval lies within ±τ. `noisy`: anything else; rerun with more repeats or on a quieter machine.
   - Fewer than 3 runs on either side shows the deltas without a verdict.
5. **`compare.md`** contains, in order:
   - a header with both captures, their commits, the machine and the comparability status;
   - the owned-metric table (baseline, candidate, Δ, Δ%, interval, verdict);
   - the stages;
   - the systems whose |Δ| > τ;
   - the GPU passes;
   - memory: peak RSS per side, with its verdict;
   - workload counters;
   - guard notes.
6. **`compare.html`** is self-contained: inline CSS and SVG, with no scripts, fonts or network requests. Each row gets:
   - an ECDF of the pooled `total` frames per side, with p50, p95 and p99 markers and budget lines at 16.7 and 6.9 ms;
   - the run-1 frame-time series for each side;
   - stacked stage bars;
   - per-system horizontal bars (p50 with p95 whiskers), sorted by baseline cost;
   - GPU pass bars;
   - peak-RSS bars;
   - the counter table.

   Two fixed series colors are used, and verdict badges carry text labels, not color alone. Light and dark schemes follow `prefers-color-scheme`.
7. **Exit code.** `--fail-on regressed` exits non-zero when any owned metric regresses. This is for local scripting only (`D-070`).
8. **A/A check.** Comparing two captures of the same build must yield no `regressed` or `improved` verdict on owned metrics. Run it once per machine before trusting verdicts.
9. **Sweeps.** `--sweep` writes one capture per value, plus `sweep.md` and `sweep.html` plotting the owned metrics against the knob.

### Capacity search

1. **Probes.** Each probe runs the benchmark's warm-up plus 180 measured frames once, telemetry only, with no GPU diagnostic run. A probe passes when p95 `total` (or `--stat`) is at most the budget: 60hz is 16.7 ms, 144hz is 6.9 ms, or a millisecond value.
2. **Bracketing.** Start at scale 1. Double on a pass until the first fail or the knob's upper bound, and halve on a fail until the first pass or the lower bound.
3. **Bisection.** Bisect geometrically between the last pass and the first fail until their ratio is at most 1 + `tolerance` (5%), or until an integer knob can't move by one step.
4. **Confirmation.** Rerun the best pass 3 times; it holds if the median p95 is within the budget. Otherwise step down one tolerance step and confirm again, at most 3 times.
5. **Edge cases.** A child that fails (device lost, out of memory) counts as a fail and the reason is recorded. Hitting a bound is reported as "≥ max (bound reached)" or "< min (budget not reachable)".
6. **Report.** For each row the report gives:
   - budget and maximum scale;
   - the key counts at that scale, such as balls or entities;
   - p95 and p99;
   - the limiting stage, meaning the largest mean among the top `update` system, `flush`, `particles`, `extract`, `render_encode`, the present wait (`render_acquire` + `render_submit_present`) and `unattributed`, flagged ✓ or ✗ against the row's declared bottleneck;
   - peak RSS;
   - the probe count and the wall time.

   It is written to `capacity.md`, `capacity.json` and `capacity.html` (p95 against scale on a log axis, with the budget line). The results are machine-specific and informational.

### Memory

- **Peak RSS** is recorded for every run through `os.wait4`, with no engine change. The timing run's value is the one reported, because the diagnostic run allocates query buffers. It includes driver-mapped memory, so compare it only on one machine and driver.
- **RSS growth (A10)** is the slope of the 10 Hz samples over the second half of the run, in KiB/s. It is judged for `churn` and reported for the others.
- **Allocation counting** is gap M1.

## Where the benchmarks live

`examples/02_bench/` replaces `examples/02_sprite_stress/` (A1):

```text
examples/02_bench/
  Cargo.toml                          package example-02-bench (tungsten, tungsten-core, tungsten-render, glam,
                                      anyhow, log, env_logger, serde_json: all workspace dependencies)
  assets/manifest.json                bench_heavy shader and material
  assets/shaders/bench_heavy.wgsl
  src/main.rs                         config (1920x1080, vsync off), dispatch, describe, overlays
  src/knobs.rs                        schema, presets, scale, overrides, validation
  src/counters.rs                     bench-config and bench: lines
  src/gen.rs                          procedural textures, animation frames, tiles
  src/view.rs                         culled, unsorted extract
  src/physics.rs  src/ecs.rs  src/churn.rs  src/gpu.rs  src/particles.rs
  src/integrated.rs  src/integrated/level.rs
  src/tests/                          knob resolution and validation; pachinko geometry invariants
  tests/visual_regression.rs
  tests/fixtures/README.md  tests/fixtures/gpu-visual.png
```

Examples keep light tests: only what "Done when" needs, plus knob parsing and pachinko geometry, which are easy to get wrong and cheap to test. Split any module that grows past about 600 lines.

Smoke coverage lives in a new "Benchmarks (pkg: example-02-bench)" section of `scripts/smoke-examples.sh`. Each row sets `TUNGSTEN_BENCH` and `TUNGSTEN_BENCH_PRESET`:

- `physics`: `min`, `default`, `sparse-min`, `sparse`;
- `ecs`: `min`, `default`;
- `churn`: `min`, `default`;
- `gpu`: `min`, `default` (with `TUNGSTEN_GPU_TIMING=1`, replacing the render-features timing row), `throughput`;
- `particles`: `min`, `default`;
- `integrated`: `min`, `default`.

The M25 MSAA × depth-sort matrix moves to `gpu` at `min`. The generic example loop also runs `example-02-bench` without environment variables, which runs `physics` at `default`. The dev profile builds workspace crates at opt-level 0, so each phase records its row durations. If a default-scale row exceeds 60 s, `run_row` gets an optional per-row timeout, tested in `test-smoke-examples.sh`.

### Visual test

- A new `examples/02_bench/tests/visual_regression.rs` keeps today's shape: 8 smoke frames, capture frame 5 at 1280×720, tolerance 2, `pixels_above_tolerance == 0`, and the `D-047` fallback of fewer than 16. It sets `TUNGSTEN_BENCH=gpu` and `TUNGSTEN_BENCH_PRESET=visual`.
- The `visual` preset renders:
  - 2,000 sprites over 3 z layers, half of them lit with normal and emissive maps;
  - 4 point lights plus the directional light;
  - `bench_heavy` on 10% of the sprites at 8 iterations;
  - one tile layer;
  - 64 static glyphs drawn from the frame index;
  - bloom and vignette, with AA off and a fixed camera.

  Everything is deterministic under the pinned dt.
- The fixture `gpu-visual.png` is regenerated on the reference machine in a dev build, with the command given in the new fixture README. That README keeps the determinism note and adds a reference-machine block: OS, GPU, driver, wgpu, rustc, date, commit and SHA-256.
- `just visual` switches to `-p example-02-bench`. The fixture is accepted only after two consecutive passes. P6 deletes the old test and fixture along with the old package.

## Docs to rewrite

- **`docs/perf/profiling-workflow.md`** is fully rewritten, and its title becomes "Profiling Workflow". Sections:
  1. The comparison rule, and the canonical capture table: release, frame-pointer flags, Vulkan, 1920×1080, `present_mode=auto` with vsync off and latency 1, each benchmark's warm-up plus 300 frames, `--repeat 5`, and GPU timing only in the diagnostic run.
  2. Quick start.
  3. Configuration: the variables, resolution order, presets and scale.
  4. Capture layout and provenance.
  5. Telemetry lines.
  6. Validity and determinism.
  7. Compare: comparability, statistics, verdicts, thresholds, outputs, the A/A check and exit codes.
  8. Capacity search.
  9. Memory.
  10. Tracked rows, budgets and regression policy (a `regressed` verdict on an owned metric needs a fix, or a justification in `DECISIONS.md` or the plan).
  11. Profiling: `--profile`, DWARF and FP stacks, sample frequency, manual flamegraph/perf/samply, and the GPU-timing caveat.
  12. Hotspots: today's list plus `gather_tilemap_proxies`, `build_pairs`, `World::flush`, `extract_sprites_default`, `particle_tick_system` and the text `prepare`.
  13. Frame-pacing overrides and policy, pointing to the new decision entry for the matrix.
  14. The backend reference and RenderDoc.
  15. Criterion micro-benchmarks (`D-037`, `just bench-build`).

  The old stage guardrails (update under 4 ms and so on) are dropped, because the benchmarks load their owned stage on purpose. The budgets that remain are the capacity budgets.
- **`docs/perf/benchmarks.md`** is new. It holds:
  - the ownership table;
  - one section per benchmark: bottleneck, workload, knobs, presets, counters and guards, owned metrics, calibrated defaults on the reference machine, and the `workload_version` history;
  - a checklist for adding a benchmark: module, knobs, `min` and `default` presets, counters, guards, smoke rows, tracked row, docs.

  It is added to `DOCS` in `scripts/check-repo.py`.
- **The tracked rows table** is rewritten in `profiling-workflow.md`:

| Row | Command | Judge on | Guard |
| --- | --- | --- | --- |
| `physics` | `just perf run physics --repeat 5` | `physics_step` p50/p95, `update` p95 | No sleepers; teleports in every frame |
| `physics-sparse` | `just perf run physics --preset sparse --repeat 5` | `physics_step` p50/p95 | No sleepers |
| `ecs` | `just perf run ecs --repeat 5` | `update` p50/p95, per-system rows | No structural changes |
| `churn` | `just perf run churn --repeat 5` | `flush` p50/p95, churn systems, RSS growth | Constant population |
| `gpu` | `just perf run gpu --repeat 5` | `gpu`, `render_span`, per-pass rows; `extract`, `render_encode` | Timestamp queries available |
| `gpu-throughput` | `just perf run gpu --preset throughput --repeat 5` | `extract`, `render_encode` p50/p95 | — |
| `particles` | `just perf run particles --repeat 5` | `particles` p50/p95 (T1), `animate_sprites` | Live count within ±10% of target |
| `integrated` | `just perf run integrated --repeat 5` | `total` p50/p95/p99, p99 − p50 | Camera inside the level |
| capacity | `just perf capacity --all --budget 60hz` (and `144hz`) | Maximum scale per row (informational) | — |

  Every row also reports peak RSS. `just perf suite --repeat 5` runs all eight rows.
- **`docs/LLM_INDEX.md`**, staying under 8 KiB:
  - the "Telemetry, perf logging" row becomes "Telemetry, perf lines" → `tungsten/telemetry.rs`, `tungsten/app.rs`, `docs/perf/profiling-workflow.md`;
  - a new row, "Perf runner, capacity search, compare reports (`D-078`)" → `scripts/bench.py`, `scripts/bench_report.py`, `scripts/test-bench.py`;
  - "Sprite stress, perf scenes" becomes two rows: "Benchmark harness, knobs, presets" → `examples/02_bench/src/main.rs`, `examples/02_bench/src/knobs.rs`, `docs/perf/benchmarks.md`; and "One benchmark's workload" → `examples/02_bench/src/<bench>.rs`.
- **`AGENTS.md`** gets three edits, which together shrink it by about 16 bytes; it is exactly at its 6,144-byte limit:
  - `just perf ecs-high-load 300 #` becomes `just perf suite --repeat 5 #`;
  - the visual command uses `-p example-02-bench`;
  - the tests-table row becomes "Scripts or perf tooling".
- **`README.md`**: the run line becomes `cargo run -p example-02-bench  # benchmark suite; TUNGSTEN_BENCH selects one`, and the perf block lists `just perf suite --repeat 5`, `just perf compare`, `just perf capacity --all` and `just perf-test`.
- **`DESIGN.md`**, section "Performance Baseline + Profiling Harness — Phase 3 M12": keep the telemetry bullets, updated for T1 and T2 if they are approved. Replace the "Canonical scenes" and "Capture tooling" bullets with one bullet pointing at both perf docs. If the cleanup plan has already reduced the section to a pointer, only check that pointer.
- **`.claude/skills/tungsten-perf/SKILL.md`** is rewritten for the new commands, capture rules, owned metrics, guards and verdict labels, keeping the "do not" list and staying under the 8 KiB body limit.
- **`DECISIONS.md`** gets `D-078`, "Benchmark suite v2". It records:
  - the six benchmarks in `example-02-bench` and the environment-variable scale config;
  - ownership and validity guards;
  - Python standard-library tooling replacing `perf-capture.sh`;
  - the capture defaults (A4) and the verdict rule (A5);
  - capacity search as informational, and peak RSS through `wait4`;
  - the April 2026 pacing matrix (A8);
  - that it narrows clause 6 of `D-044`: HUD overhead is measured on the new suite instead of sprite-stress runs.

  Add its `docs/DECISION_INDEX.md` row, which a test enforces. Follow the existing "Narrowed by" precedent (`D-002`/`D-070`) for `D-044`. Approved gaps G1–G3 and M1 get their own entries; T1 and T2 fold into `D-078`.
- **`CHANGELOG.md`** gets an `[Unreleased]` entry: a Summary naming this plan, then Added, Changed and Removed.
- **Comments:** `crates/tungsten/src/app.rs` (two parser references, `format_perf_systems_line` and `format_perf_physics_line`) and `.cargo/config.toml` (the RUSTFLAGS note).
- **`docs/known-issues.md`**: only if it exists by then and lists the `perf-capture.sh` output-directory finding. Close that finding; the script is gone.

## Files to touch

- **New:**
  - `examples/02_bench/**`, as listed above;
  - `scripts/bench.py`, `scripts/bench_report.py`, `scripts/test-bench.py`;
  - `docs/perf/benchmarks.md`.
- **Changed:**
  - `Cargo.toml` (members) and `Cargo.lock`;
  - `justfile`: `perf`; `perf-legacy`, which exists in P1–P5 only; `perf-test`; `visual`;
  - `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh`, `scripts/check-repo.py`, `.claude/settings.json`;
  - `docs/perf/profiling-workflow.md`, `docs/LLM_INDEX.md`, `AGENTS.md`, `README.md`, `DESIGN.md`, `.claude/skills/tungsten-perf/SKILL.md`;
  - `DECISIONS.md`, `docs/DECISION_INDEX.md`, `CHANGELOG.md`;
  - `crates/tungsten/src/app.rs`: comments, plus T1 and T2 if approved;
  - `crates/tungsten/src/telemetry.rs` (T1), and `crates/tungsten/src/tests/app.rs` or `tests/telemetry.rs` for their format tests;
  - `.cargo/config.toml` (a comment);
  - `docs/known-issues.md`, only in the case described above.
- **Deleted:** `examples/02_sprite_stress/` (the whole package, including its fixture), `scripts/perf-capture.sh`, `scripts/test-perf-capture.sh`.
- **Not touched:** `crates/*/benches/*`, even though their comments still name the retired scenes (out of scope); examples 01, 03 and 04; `tungsten.json`.

## Phases

Each phase ends with a runnable, smoke-tested subset. After each phase, record the check results and calibrated values under "Progress".

### P1: Harness, runner, physics

1. Scaffold `examples/02_bench/` and add it to `members` beside `02_sprite_stress`. Refresh `Cargo.lock` with a plain `cargo build -p example-02-bench`, because `just check` runs with `--locked`.
2. Build the harness: `main.rs`, `knobs.rs`, `counters.rs`, `gen.rs` and `view.rs`, with `TUNGSTEN_BENCH_DESCRIBE`.
3. Write `physics.rs` (pachinko and sparse) with its counters, at `workload_version` 1.
4. Write the example tests:
   - knob resolution and validation errors;
   - pachinko invariants for every radius mix: row gap ≥ 3·`d_max`, diagonal gap ≥ 2.5·`d_max`, the board fits the camera at scale 1, and initial balls overlap neither pegs nor each other.
5. Write `scripts/bench.py` (`describe`, `run` with repeat, profile and present overrides, `baseline`), the parsing and capture half of `scripts/bench_report.py`, and `scripts/test-bench.py`. The tests cover:
   - grouping of companion lines;
   - literal system names;
   - nearest-rank percentiles;
   - guards;
   - child-environment hygiene;
   - provenance helpers;
   - `os.wait4` reaping against a Python child that allocates 64 MiB, which must report peak RSS ≥ 64 MiB and propagate its exit code.
6. Update the `justfile`: `perf` becomes `python3 -B scripts/bench.py "$@"`; `perf-legacy` becomes `./scripts/perf-capture.sh "$@"`; `perf-test` runs both test scripts.
7. Add the smoke "Benchmarks" section with the four physics rows, and update the stub metadata and expected runs in `test-smoke-examples.sh`.
8. Add `Bash(cargo run -p example-02-bench)` to `.claude/settings.json`.
9. Calibrate physics (A9 and the physics calibration checks).

Checks:
- `just check` passes.
- `just smoke` passes on every row; record the bench-row durations.
- `just script-test` passes.
- `just perf run physics --repeat 3` and `just perf run physics --preset sparse --repeat 3` write valid captures: guards pass, `sleeping=0` in every measured frame, peak RSS present, identical digests across repeats.

### P2: Compare and capacity search

1. In `bench_report.py`, add comparability checks, Welch verdicts, `compare.md`, `compare.html` and the sweep report.
2. In `bench.py`, add `compare`, `run --compare`, `run --sweep` and `capacity`.
3. Extend the tests:
   - each verdict label, n < 3, the absolute floors and the RSS thresholds;
   - comparability failures: `workload_version`, knobs, build flags, machine;
   - two synthetic captures produce both outputs, and both contain peak RSS;
   - the HTML has no external references;
   - capacity search against a fake monotone model converges within tolerance, reports a reached bound and an unreachable budget, and survives one noisy probe at the boundary.

Checks:
- `just script-test` passes.
- An A/A comparison of two physics captures of one build (`--repeat 5`) has no `regressed` or `improved` verdict on owned metrics.
- `just perf capacity physics --budget 60hz` and `--budget 144hz` report a maximum scale.
- `compare.html` renders offline.

### P3: ECS and churn

1. Write `ecs.rs`.
2. Write `churn.rs` (both modes).
3. Add runner guards for both.
4. Add their four smoke rows.
5. Calibrate.

Checks:
- `just check` and `just smoke` pass.
- `--repeat 3` captures of both are valid.
- `churn`'s RSS growth is about 0 at the default.
- Capacity search reports both rows.

### P4: GPU, particles, telemetry gaps, visual test

1. If approved, add T1 and T2 in `crates/tungsten/src/{app,telemetry}.rs`, with format tests, and teach the parser the new keys. The HUD doesn't change.
2. Write `gpu.rs`, `examples/02_bench/assets/manifest.json` and `bench_heavy.wgsl`, with the `min`, `default`, `throughput` and `visual` presets.
3. Write `particles.rs`.
4. Smoke:
   - add the `gpu` and `particles` rows;
   - move the M25 matrix to `gpu` at `min`;
   - fold the render-features timing row into the `gpu` default row;
   - update `test-smoke-examples.sh`.
5. Visual: add the new test, fixture and README, and switch the `just visual` recipe.
6. Calibrate.

Checks:
- `just check` and `just smoke` pass.
- `just visual` passes twice in a row.
- `just perf run gpu --repeat 3` shows the per-pass GPU rows.
- `just perf run gpu --sweep lights=0,4,8,16 --repeat 3` writes the sweep report.
- The particles capture is valid, with the `particles` stage present if T1 was approved.
- Capacity search reports the `gpu`, `gpu-throughput` and `particles` rows.

### P5: Integrated and suite runs

1. Write `integrated.rs` and `integrated/level.rs`.
2. Add `bench.py suite`, which takes the tracked rows from `describe`, plus suite-level compare and the suite README.
3. Add the smoke rows, then calibrate.

Checks:
- `just check` and `just smoke` pass.
- `just perf suite --repeat 3` finishes with every capture valid.
- `just perf capacity --all --budget 60hz` reports all eight rows.

### P6: Retire the old suite; docs; decision

1. Delete `examples/02_sprite_stress/`, `scripts/perf-capture.sh` and `scripts/test-perf-capture.sh`.
   - Drop the old workspace member, then refresh `Cargo.lock`.
   - Remove the `perf-legacy` recipe, and point `perf-test` at the new tests only.
   - Remove `example-02-sprite-stress` from the smoke stub and from `.claude/settings.json`.
2. Apply every item under "Docs to rewrite", plus the `DOCS` entry in `check-repo.py`.
3. Add `D-078`, its index row and the `D-044` note. Add the `[Unreleased]` changelog entry. Close the finding in `known-issues.md` if it applies.
4. Run every "Done when" check. Then set this plan's status to `done` and move it to `docs/plans/archive/`.

If phases ship separately rather than on one branch, record `D-078` with P1 instead. Avoid cutting a release between P1 and P6: `D-071` would ship both 02 packages.

## Done when

1. `just smoke` passes. Its Benchmarks section runs every benchmark at `min` and `default` with `TUNGSTEN_SMOKE_FRAMES=3`, plus physics `sparse-min` and `sparse` and gpu `throughput`, and it runs the M25 matrix on `gpu` at `min`.
2. `just perf capacity --all --budget 60hz` and `--budget 144hz` report a maximum scale, or an explicit bound result, for every tracked row.
3. Given two captures, `just perf compare <A> <B>` writes `compare.md` and `compare.html`, both including peak RSS. `just script-test` runs `scripts/test-bench.py`, which covers both outputs.
4. `rg -n 'sprite-stress|ecs-high-load|physics-stress|render-features|perf-capture|STRESS_' docs/perf docs/LLM_INDEX.md` prints nothing, and both documents describe the new suite.
5. `rg -l 'example-02-sprite-stress|02_sprite_stress|perf-capture|STRESS_(SCENE|COUNT)' AGENTS.md README.md DESIGN.md justfile Cargo.toml .cargo .claude docs/perf docs/LLM_INDEX.md scripts examples crates` prints nothing.
6. `just check` passes.
7. `just repo-check` and `just ctx` pass. This covers the byte budgets (`AGENTS.md` ≤ 6,144 B, `LLM_INDEX.md` ≤ 8 KiB, skill body ≤ 8 KiB) and the `D-078` index row.
8. `just visual` passes twice in a row on the reference machine.
9. An A/A comparison of two `just perf suite --repeat 5` runs of one build has no `regressed` or `improved` verdict on any owned metric, and every capture is valid.
10. The calibrated defaults are recorded in `docs/perf/benchmarks.md` and under "Progress".

## Risks

- **Debug smoke duration.** Default-scale rows run with engine crates at opt-level 0. ECS spawning alone is about 4M single-component inserts. Durations are measured in P1 and P3, with a per-row timeout as the fallback.
- **Pachinko jams.** A jam leads to sleepers and an invalid capture. Mitigation: stall kicks, respawns and the guard; tune `fill` and the peg pitch.
- **Sparse not broadphase-bound.** The P1 calibration checks it with a profile; lower `density` or raise `speed_max` if needed.
- **No timestamp queries.** The per-pass rows become n/a, and the capture stays valid.
- **Driver-sensitive fixture.** Covered by the `D-047` fallback and the recorded reference-machine block.
- **Old numbers aren't comparable.** `D-078` states this, and baselines restart with the new suite.
- **CI Python version.** Target 3.12: no 3.13+ features and no third-party modules.
- **Plan conflicts.** `debug-cleanup-docs-pass.md` edits some of the same files; see the context digest.

## Progress

Each phase appends its check results and calibrated defaults here.

### P1: harness, runner, physics (2026-09-30)

Implemented steps 1–9 with no engine or library-crate change:
- `examples/02_bench/`: `main.rs`, `knobs.rs`, `counters.rs`, `gen.rs`, `view.rs`, `physics.rs`, and `src/tests/{knobs,physics}.rs`;
- `scripts/bench.py` (`describe`, `run`, `baseline`), the parsing and capture half of `scripts/bench_report.py`, and `scripts/test-bench.py`;
- the `justfile` recipes `perf`, `perf-legacy` and `perf-test`, the smoke "Benchmarks" section with its stub rows, and the `.claude/settings.json` allow.

Reference machine: Ryzen 5 6600H, Radeon 660M (RADV REMBRANDT), Vulkan, `immediate` present mode with latency 1, Linux 7.2.7-arch1-1, rustc 1.98.1, governor `performance`. The host exposes no AC or platform-profile sysfs entries, so both read `n/a`.

Checks:
- `just check`: pass.
- `just smoke`: pass on every row (examples 5/5, Benchmarks 4/4). Bench-row durations, dev build with 3 frames and `cargo run` overhead included: plain `example-02-bench` 0.47 s; `physics` at `min` 0.28 s, `default` 0.47 s, `sparse-min` 0.29 s, `sparse` 0.43 s. No row needs a per-row timeout.
- `just script-test`: pass, including the 9 tests in `test-bench.py`.
- `just perf run physics --repeat 3` (`perf-runs/20260930T051622Z-physics`): valid. Guards pass: `sleeping` is 0 in all 300 measured frames of every run, and teleports run 8–29 per frame. Digests match (`c972b2818d486617` in all 3 runs). Peak RSS: 131.5, 131.5 and 131.8 MiB.
- `just perf run physics --preset sparse --repeat 3` (`perf-runs/20260930T051644Z-physics-sparse`): valid. `sleeping` is 0 in every measured frame, digests match (`c95fadc938a3689e` in all 3 runs), and peak RSS is 127.4, 126.7 and 127.2 MiB.

Calibrated defaults, as medians of the 3 runs above. They replace the defaults the "Benchmark designs" tables list, and P6 records them in `docs/perf/benchmarks.md`.

| Row | Default change | World | `physics_step` p50 / p95 | `update` p95 | `total` p95 | Calibration check |
| --- | --- | --- | --- | --- | --- | --- |
| `physics` | `fill` 0.21 → 0.36; `balls` stays 8,000 | 1,024 × 1,824 px: 8 bins, 496 pegs, 99 bumpers, actual fill 0.354, zoom 0.592 | 13.37 / 13.99 ms | 14.06 ms | 14.58 ms | 1.11 contacts per ball (8,861) |
| `physics-sparse` | `bodies` 50,000 → 8,000 | 4,224 × 2,368 px; about 6,660 bodies visible | 8.06 / 9.83 ms | 9.86 ms | 10.11 ms | 0.229 pairs and 0.118 contacts per proxy |

- Pachinko at `fill` 0.21 gave 0.64 contacts per ball (a 1,792 px board, `total` p95 10.6 ms). A single-run sweep of `fill` = 0.25, 0.30, 0.35 and 0.40 gave 0.74, 0.89, 0.99 and 1.11 contacts per ball. The board width rounds up to whole bins, so reaching 1.0 takes 8 bins, meaning `fill` ≥ 0.3542; 0.36 is the smallest round value that gets there. Stall kicks stay near 0.01 per frame and respawns at 0, so the denser board doesn't jam. The denser board needed more placement tries, so `PLACE_ATTEMPTS` rose to 4,096. The worst case measured was 92 tries at the default and 371 at `fill` 0.40 with the `uniform` mix.
- Sparse at 50,000 bodies gave a `total` p95 of 78 ms. Sweeping `bodies` over 6,000, 8,000, 10,000 and 12,000 gave `total` p95 values of 7.8, 10.6, 13.9 and 15.7 ms. A `--profile --call-graph fp` capture at 8,000 bodies (`perf-runs/20260930T051050Z-physics-sparse-set0f9980`) splits the `physics_step` samples as follows. Pair query (`build_pairs`) 54.5% and grid build (`SpatialGrid::build`) 6.2% lead. `speculative_pass` takes 29.9%, 22% of which is `SpatialGrid::query`. Narrow phase takes 1.4% and the solver 0.7%. So `density` stays at 0.01.
- Pachinko RSS grows by 4–6 KiB/s over the second half of a run; this is reported only, and judged for `churn` alone.
- The exploratory sweep captures also sit in `perf-runs/`, which is gitignored.

For P2 and later phases:
- The binary has one addition to the transport: `TUNGSTEN_BENCH_DESCRIBE=config` prints the resolved `bench-config` object. It exits non-zero with the knob error on a bad request. `bench.py run` gets the config this way, so it never re-implements resolution, then compares each run's `bench-config:` line with that object as parsed JSON.
- Describe rows carry `owned` entries (`stage.<frame key>` or `system.<name>`, plus statistics) and `guards` (`physics_max`, `counter_min`). `bench_report.GROUPS` and `metric_value` map these onto `capture.json`. There, `runs[].stats.<group>.<name>` holds `n`, `mean`, `min`, `p50`, `p95`, `p99` and `max`; each run also has `rusage`, `rss_growth_kib_s` and `digest`, and optionally `gpu_run`. Compare and capacity should reuse these.
- The digest hashes the exact `physics:` and `bench:` text of every measured frame. GPU diagnostic runs join the match; with `--gpu-timing on` they matched the timing run.
- The `bench.teleports >= 1` guard assumes a busy board. At `min` (1,000 balls) the flow is about 3 per frame, so single frames can see no teleport; decide whether capacity probes apply guards before probing small scales.
- `AGENTS.md`, `docs/perf/`, `docs/LLM_INDEX.md` and the `tungsten-perf` skill still show `just perf <scene>`. For the old scene names, `bench.py` points to `just perf-legacy`. P6 rewrites those docs.
- `gen` is a reserved keyword in edition 2024, so `main.rs` declares the module as `mod r#gen;`; the file stays `src/gen.rs`.

### P2: compare and capacity search (2026-09-30)

Implemented steps 1–3 with no engine or library-crate change:
- `examples/02_bench/src/knobs.rs`: describe rows gain `bottleneck` (the limiting stage capacity search expects: `system.<name>`, `update` for any system, `stage.flush`, `stage.particles`, `stage.extract`, `stage.render_encode`, `present` or `stage.unattributed`) and `key_knobs` (reported as key counts; the scaled ones bound the `scale` axis). Both physics rows declare `system.physics_step`, with `balls` and `bodies` respectively. `workload_version` stays 1: describe output only.
- `scripts/bench_report.py`: comparability (hard fields and soft environment notes), workload drift, Welch verdicts, `compare.md`, a self-contained `compare.html` (ECDF, run-1 series, stage stack, system, GPU-pass and RSS bars, counters), the sweep report, and the capacity report with the limiting-stage analysis.
- `scripts/bench.py`: `compare` (baseline name or directory on either side; `--out`, `--fail-on regressed` exits 1, `--force` overrides only an invalid capture, never a hard mismatch), `run --compare`, `run --sweep`, and `capacity` (`--budget`, `--stat`, `--axis`, `--tolerance`, `--frames`). Compare also writes `compare.json`, for P5's suite compare.
- `scripts/test-bench.py`: 8 new tests covering the step 3 list; 17 in total.

Checks (Vulkan, `immediate` / latency 1, governor `performance`, tree at `d6e667f` plus the P2 diff):
- `just script-test`: pass. `test-bench.py` also passes under Python 3.12.13. `just check`: pass, for the describe change.
- A/A: `perf-runs/20260930T062834Z-physics` against `perf-runs/20260930T062907Z-physics`, both valid at `--repeat 5`. Report in `perf-runs/20260930T062938Z-compare-physics/`. Owned verdicts: 0 regressed, 0 improved, 3 unchanged, 0 noisy. Over all 59 judged metric–statistic pairs (owned, stages, systems, peak RSS): 58 unchanged and 1 noisy, `stage.render` p99 (not owned; 0.33 → 0.37 ms against τ 0.05 ms).

  | Owned metric | Baseline | Candidate | Δ (95% interval) | τ |
  | --- | --- | --- | --- | --- |
  | `physics_step` p50 | 13.34 | 13.31 | −0.02 [−0.13, +0.09] | 0.40 |
  | `physics_step` p95 | 13.65 | 13.60 | −0.05 [−0.19, +0.09] | 0.68 |
  | `update` p95 | 13.72 | 13.66 | −0.06 [−0.20, +0.09] | 0.69 |

  Peak RSS: 131.5 against 131.6 MiB, unchanged (τ 2.6 MiB). Workload counters are identical on both sides, so there is no drift.
- Capacity: `perf-runs/20260930T063117Z-capacity-60hz` and `perf-runs/20260930T063250Z-capacity-144hz`. Every result is confirmed as a maximum; no bound was reached. No probe failed a guard; the smallest probe had 4,000 balls. p95, p99, the limiting stage and peak RSS are medians over the 3 confirmation probes.

  | Row | Budget | Max scale | Key count | p95 / p99 (ms) | Limiting stage | Peak RSS | Probes | Wall |
  | --- | --- | --- | --- | --- | --- | --- | --- | --- |
  | `physics` | 60hz | 1.189 | balls 9,514 | 16.55 / 17.04 | `system.physics_step` ✓ 15.73 ms | 133.3 MiB | 9 | 49 s |
  | `physics-sparse` | 60hz | 1.542 | bodies 12,338 | 16.08 / 16.36 | `system.physics_step` ✓ 13.61 ms | 129.7 MiB | 9 | 39 s |
  | `physics` | 144hz | 0.545 | balls 4,362 | 6.73 / 6.82 | `system.physics_step` ✓ 6.42 ms | 127.7 MiB | 9 | 22 s |
  | `physics-sparse` | 144hz | 0.707 | bodies 5,657 | 6.86 / 6.94 | `system.physics_step` ✓ 5.65 ms | 126.2 MiB | 9 | 18 s |

- `compare.html` renders offline. The no-external-references test passes, and a tag-balance parse passes for both `compare.html` and `sweep.html`. Headless Chromium with a dead proxy rendered every section in the light and dark schemes.
- Sweep: `just perf run physics --sweep fill=0.30,0.33,0.36,0.40 --repeat 1` (`perf-runs/20260930T063037Z-physics-sweep-fill`). All four captures are valid. `physics_step` p50 is 11.99, 12.37, 13.21 and 13.31 ms, and `sweep.md` and `sweep.html` are written.

For P3 and later phases:
- Every new row must declare `bottleneck` and `key_knobs`. `ecs` can declare `update`, since its cost spreads over 14 systems. A `scale` axis needs at least one scaled key knob. Its bounds are the tightest `min/value` and `max/value` over those knobs.
- RSS growth has a teardown artifact. The last 100 ms `statm` sample can land while the child shuts down: in candidate run 1, RSS fell from 135,612 to 122,508 KiB, and the fit gave −897 KiB/s where the other runs gave −9 to +5 KiB/s. Before `churn` judges growth, P3 should end the fit at the last measured frame or drop the final samples. A5 also defines no growth threshold, so P3 needs one; compare reports growth only.
- Capacity probes judge only the budget statistic. Hard problems (exit code, missing frames, config mismatch) fail a probe; guard failures are recorded per probe and listed in `capacity.md`. `bench_report.hard_problems` separates the two.
- The Welch df rounds down to the next tabulated t entry. With n = 3 and unequal floored variances it lands just under 4, so t(3) applies: conservative, as intended.
- One probe at warm-up 120 plus 180 frames takes 2–5 s at these scales; one row takes about 9 probes.

### P3: ecs and churn (2026-09-30)

Implemented steps 1–5 with no engine or library-crate change. `immediate` mode needs none: systems already receive `&mut World`.
- `examples/02_bench/src/ecs.rs` (427 lines) with its 14 systems in `src/ecs/systems.rs` (273 lines), and `churn.rs` (536 lines), both at `workload_version` 1 with `min` and `default` presets, registered in `main.rs`. `ecs` declares `bottleneck` `update` with `key_knobs` `entities`; `churn` declares `stage.flush` with `population`.
- `knobs.rs`: three new `Guard` kinds, `CounterMax` (`counter_max`), `CounterConst` (`counter_const`) and `CounterEq` (`counter_eq`). `ecs` has `bench.structural <= 0` and `bench.entities constant`; `churn` has `bench.population constant` and `bench.spawned == bench.despawned`. Physics' work is unchanged, so its `workload_version` stays 1.
- `scripts/bench_report.py`: `guard_name` and `evaluate_guard` handle the new kinds, still over every measured frame but the last. `rss_growth` drops samples within `RSS_TEARDOWN_S` (0.2 s) of the last one before fitting.
- `scripts/test-bench.py`: the guard test covers the three new kinds (pass, failure detail, a missing `other` counter), and one new case covers the trimmed fit; 18 tests. `src/tests/knobs.rs`: `every_preset_resolves` loops over every benchmark.
- `scripts/smoke-examples.sh`: the Benchmarks section runs 8 rows; `test-smoke-examples.sh` expects them.

Design details the "Benchmark designs" sections leave open:
- **`ecs`.** `Pos` is the engine's `Position`, so `view.rs` draws the 4,096-entity sample unchanged. No engine system reads `Position` unless a benchmark registers one. The 14 systems, in run order, are:
  - `brain`: a state machine over `query3_mut`;
  - `cooldowns`: branching timers;
  - `buffs`: the `query2_opt2_mut` path;
  - `regen` and `stats_decay`: sparse matches;
  - `accelerate`: a dense write over the 80% that hold `Acc`;
  - `follow`: random access, one `World::get::<Position>` per follower;
  - `integrate`, `bounds_wrap` and `heading` (`atan2`): dense writes;
  - `tint` and `age_phase`: cheap ticks;
  - `team_bags` and `faction_histogram`: read-only reductions.

  `dense` selects the 8 whole-population systems, and `sparse` the 6 optional-component ones.
- **`ecs` counters and guards.** `archetypes` counts the distinct signatures spawned, since the engine exposes no archetype count; the default gives 488. A last system, `ecs_audit`, sets `structural` to the commands queued during `update` plus any change in the live entity count.
- **`ecs` digest.** `digest=0x…` is FNV-1a over every position's bits plus the two reductions. It parses as n/a, so it stays out of the counter stats and the drift check, and it feeds the determinism digest. Computing it costs 0.39 ms per frame in `bench_counters`, which falls inside `update`.
- **`ecs` owned metrics.** The row owns `stage.update` p50/p95 plus each of the 14 system rows at p50. P5's suite A/A will show whether 14 per-system verdicts stay quiet.
- **`churn` selection.** Targets follow spawn serials: frame f despawns `[f·S, (f+1)·S)` and spawns `[P + f·S, P + (f+1)·S)`. A deferred spawn has no `Entity` until flush, so both modes find their targets with one scan over `Life` (`churn_scan`). The structural calls go through one trait implemented for `World` (immediate) and `CommandBuffer` (deferred). Systems run in the order `churn_scan`, `churn_despawn`, `churn_toggle`, `churn_spawn`.
- **`churn` status toggles differ from the design.** A first version toggled a sliding window of survivors and gave `removed=0`: with `toggles` / `turnover` = 1, an entity is toggled once or twice per life, and its second toggle always picked a different status. Now survivors whose serial is f modulo `groups` gain a status at frame f and lose it at f + 1. Gains skip entities that expire at f + 1, so every frame's losses equal the previous frame's gains exactly. `groups = round(2·(P − 2S) / (toggles·P))` makes gains plus losses about `toggles` of the population; the default is 36 groups.
- **Toggle cost.** Toggled rows are scattered through a large archetype, so each toggle move is memory-latency bound. At the default, spawn and despawn alone (`--set toggles=0`, `perf-runs/20260930T080141Z-churn-setbf6388`) cost 4.18 ms of `flush` for 50,000 commands, about 0.1 µs per move. The 6,250 toggle moves add 4.36 ms, about 0.70 µs each.

RSS teardown fix. The child's RSS starts falling 13–16 ms after its last `frame:` line, and it exits 22–34 ms after it: 3 runs each of physics, ecs and churn with 10 ms polling. Only the final 100 ms sample can land in that window. Recomputed from each run's `rss.tsv`:

| Capture | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 |
| --- | --- | --- | --- | --- | --- |
| `20260930T062907Z-physics` (P2 candidate), before | **−896.9** | +1.8 | +5.3 | +1.8 | −8.8 |
| after | **+5.7** | +2.0 | +5.8 | +2.0 | +3.9 |
| `20260930T062834Z-physics` (P2 baseline), before | −5.8 | +3.5 | +1.8 | +3.7 | +1.9 |
| after | +4.1 | +3.9 | +2.0 | +4.1 | +2.0 |

In KiB/s. Run 5 of the candidate had a partial teardown too: its last sample was 192 KiB lower. After the fix, physics A/A reads +2.0 to +5.8 KiB/s.

Calibrated defaults, as medians of the `--repeat 3` captures on a quiet machine:

| Row | Default | `total` p95 per run | Owned | Calibration check |
| --- | --- | --- | --- | --- |
| `ecs` | `entities` stays 250,000 | 12.29, 12.61, 12.23 ms | `update` p50 10.84 / p95 11.84 ms | `update` is 95.7% of `total` (means), ≥ 70% ✓ |
| `churn` | `population` 100,000 → 125,000 | 11.37, 11.28, 10.95 ms | `flush` p50 8.74 / p95 10.01 ms | `flush` + churn systems are 96.8% of `total`, ≥ 60% ✓ |

- `ecs` per-system p50 medians: `heading` 2.59, `brain` 2.22, `follow` 1.90, `buffs` 0.66, `age_phase` 0.48, `cooldowns` 0.45, `tint` 0.41, `integrate` 0.27, `accelerate` 0.26, `bounds_wrap` 0.26, `team_bags` 0.25, `faction_histogram` 0.25, `stats_decay` 0.17, `regen` 0.09 ms. Followers do 24,967 lookups per frame.
- `churn` at 100,000 was valid but gave a `total` p95 of 8.65–8.97 ms, within 1 ms of the band floor (`perf-runs/20260930T075343Z-churn`). A single-run sweep (`perf-runs/20260930T075422Z-churn-sweep-population`) gave p95 9.01, 10.99, 14.37 and 16.81 ms at 100,000, 125,000, 150,000 and 175,000. 125,000 leaves at least 3 ms to either edge. At the default each frame spawns and despawns 6,250 entities and makes 3,125 gains and 3,125 losses: 40,625 inserts, 3,125 removals and 56,250 commands.
- Excluded captures. A NoMachine client connected at 07:38:36Z, and its encoder (`nxcodec.bin`) slowed every third frame through memory contention. In the slow frames `integrate` ran 2.0× and `follow` 1.8× slower, and ecs `total` p95 read 18.2 ms. The captures from 07:38 to its disconnect (`20260930T074035Z-churn`, `074145Z-churn-setbf6388`, `074206Z-churn-setaa3d97`, `074713Z-ecs`) are excluded. Every number here was taken after the disconnect.

Checks (Vulkan, `immediate` / latency 1, governor `performance`, tree at `f362485` plus the P3 diff):
- `just check`: pass.
- `just smoke`: pass (examples 5/5, Benchmarks 8/8). New row durations, dev build with 3 frames and `cargo run` overhead included, median of 3: `ecs` at `min` 0.30 s and `default` 11.55 s; `churn` at `min` 0.25 s and `default` 1.44 s. No row needs a per-row timeout.
- `just script-test`: pass. `test-bench.py` also passes under Python 3.12.13.
- `just perf run ecs --repeat 3` (`perf-runs/20260930T075317Z-ecs`): valid. Guards pass: `structural` is 0 and `entities` 250,000 in all 299 checked frames. Digests match (`c785506ced04de4c` in all 3 runs). Peak RSS: 162.9, 162.7 and 163.2 MiB.
- `just perf run churn --repeat 3` (`perf-runs/20260930T075518Z-churn`): valid. Guards pass: `population` is 125,000 and `spawned` = `despawned` = 6,250 in all 299 checked frames. Digests match (`86014148b7a94681`). Peak RSS: 143.5, 143.4 and 143.1 MiB.
- `just perf run churn --set mode=immediate --repeat 3` (`perf-runs/20260930T075535Z-churn-setaa3d97`): valid, digests match (`9f8a62ba0f39f49d`), and peak RSS is 138.8, 139.4 and 138.8 MiB.
- Moving the ecs systems into `src/ecs/systems.rs` came after the captures above. It moved code without changing it: `just check`, `just smoke` and `just script-test` pass again, and one ecs run (`perf-runs/20260930T080541Z-ecs`) reproduces digest `c785506ced04de4c` with a `total` p95 of 11.49 ms.

Command-buffer overhead. Means in ms, from the median run of each side:

| Mode | `flush` | `churn_scan` | `churn_despawn` | `churn_toggle` | `churn_spawn` | Structural total | `total` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `deferred` | 8.54 | 0.43 | 0.03 | 0.10 | 0.35 | 9.45 | 9.75 |
| `immediate` | 0 | 0.35 | 1.41 | 2.18 | 2.99 | 6.93 | 7.19 |

The `CommandBuffer` costs 2.52 ms per frame, 36% over the direct calls or about 45 ns per command over 56,250 commands. It is the boxed command and setter per operation, plus replay. Peak RSS is 4.5 MiB higher deferred.

RSS growth (A10), per run, KiB/s:
- `churn` at the default: 0.0, 0.0, 0.0 (`immediate`: 0.0, 0.0, 0.0);
- `churn` at 100,000: +6.8, +3.9, 0.0;
- `physics`, P2's A/A: −9 to +5 as reported, +2.0 to +5.8 after the teardown fix;
- `ecs`: 0.0, 0.0, 0.0.

Churn runs last 2.8 s, so the second-half fit covers about 1.3 s. One 4 KiB page over that span reads as about 3 KiB/s; the +6.8 run grew 12 KiB in total.

Proposed growth threshold (A5 has none; compare still reports growth only; not implemented, for approval): judge growth for `churn` only.
- A capture reads `leaking` when the median of its per-run slopes exceeds 32 KiB/s and every run exceeds 16 KiB/s.
- Compare marks growth `regressed` when the candidate's median exceeds the baseline's by more than 32 KiB/s under the same all-runs rule.
- Why 32 KiB/s: it is about 5× the largest slope observed on any row (6.8 KiB/s) and 10× the one-page resolution. At churn's ~100 fps it is about 330 bytes per frame, or 0.05 bytes per spawned entity.
- Why every run: one glibc heap extension of 132 KiB inside a single run's fit would read about 100 KiB/s, and the all-runs rule keeps that from counting.

Capacity (`perf-runs/20260930T075556Z-capacity-60hz` and `perf-runs/20260930T075732Z-capacity-144hz`). Every result is confirmed as a maximum; no bound was reached and no probe failed a guard. p95, p99, the limiting stage and peak RSS are medians over the 3 confirmation probes.

| Row | Budget | Max scale | Key count | p95 / p99 (ms) | Limiting stage | Peak RSS | Probes | Wall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `ecs` | 60hz | 1.354 | entities 338,565 | 16.19 / 16.64 | `system.heading` ✓ 3.51 ms (declared `update`) | 177.8 MiB | 9 | 45 s |
| `churn` | 60hz | 1.347 | population 168,359 | 16.13 / 16.58 | `stage.flush` ✓ 12.58 ms | 154.8 MiB | 12 | 46 s |
| `ecs` | 144hz | 0.500 | entities 125,000 | 6.78 / 7.34 | `system.heading` ✓ 1.34 ms (declared `update`) | 144.2 MiB | 9 | 21 s |
| `churn` | 144hz | 0.648 | population 81,053 | 6.75 / 7.68 | `stage.flush` ✓ 5.03 ms | 136.6 MiB | 9 | 15 s |

- `ecs` at 144hz lands exactly on the first halving step. Probes at 0.707, 0.595, 0.545 and 0.522 all failed, and 0.522 / 0.5 is within the 5% tolerance, so capacity lies between 125,000 and 130,534 entities.
- `churn` at 60hz needed a step down: 1.414 passed its bisect probe but failed 2 of 3 confirmations.

For P4 and later phases:
- Captures are sensitive to other screen and GPU users on this iGPU machine: a remote-desktop encoder moved ecs p95 by 50%. Before calibrating, check that nothing like `nxcodec.bin` is running. The runner records no background-load provenance; P6 could add the check to the capture rules.
- The guard kinds are `physics_max`, `counter_min`, `counter_max`, `counter_const` and `counter_eq`. P4's `particles` guard (live count within ±10% of target) needs a range kind, or two guards on the same counter.
- A hex `bench:` token such as `digest=0x…` is the pattern for a per-frame fingerprint that must stay out of numeric stats. Tokens `float()` accepts, such as `1e5` or `inf`, would be counted as numbers.
- The churn population's archetype layout scrambles over time (swap-remove), so structural cost depends on access locality as much as on count. Workload changes that alter which entities a system touches can move `flush` severalfold at the same command count.

### P4: gpu, particles and the visual test (2026-09-30)

Step 1 is skipped: no gap (G1–G4, T1, T2, M1) is approved. Steps 2–6 are implemented with no engine or library-crate change.
- `examples/02_bench/src/gpu.rs` (561 lines) with scene building in `src/gpu/scene.rs` (399 lines), and `particles.rs` (529 lines), both at `workload_version` 1, registered in `main.rs`.
- `examples/02_bench/assets/manifest.json` lists the `bench_heavy` shader and material. `assets/shaders/bench_heavy.wgsl` keeps `damage_flash.wgsl`'s vertex inputs and bind layout and loops `material.i.x` times per fragment (capped at 4,096).
- `gen.rs`: atlas pages (`register_atlas`), lit sprites (`register_lit`), unlit patterns, soft dots, sparks, orb (sphere normals) and crate (bevel normals) sets with emissive masks, tiles and animation frames.
- `knobs.rs`:
  - a `CounterBand` guard (`counter_band`): every checked frame within `tolerance` of the window's median;
  - a row `note`, which `describe` prints;
  - a `Bench::engine_config` hook that runs before the window opens. `physics`, `ecs` and `churn` use the no-op `keep_engine_config`; their digests are unchanged (below), so their `workload_version` stays 1.
- `scripts/bench_report.py`: `guard_name` and `evaluate_guard` handle `counter_band`. The capture README takes GPU-owned metrics from the GPU diagnostic runs (a Source column) and states when they are n/a (no diagnostic run, no timestamp queries, or a pass the configuration lacks).
- `scripts/bench.py`: the run summary reads GPU-owned metrics the same way; `describe` prints row notes. The capacity `scale` axis floors a key knob whose range starts at 0 at its first nonzero value (1 for integers). The old bound was 0, so an unreachable budget would have halved forever.
- `scripts/test-bench.py`: the guard test covers `counter_band` (pass with its median and largest deviation, a failing frame, a missing counter); 18 tests.
- `scripts/smoke-examples.sh`: 13 Benchmarks rows. The M25 matrix runs on `gpu` at `min`, and the render-features timing row is folded into the `gpu` default row (`TUNGSTEN_GPU_TIMING=1`). `test-smoke-examples.sh` expects them. `example-02-sprite-stress` stays in the generic loop until P6.
- `examples/02_bench/tests/visual_regression.rs`, `tests/fixtures/gpu-visual.png` and `tests/fixtures/README.md`; `just visual` runs `-p example-02-bench`. The old test and fixture stay for P6.

Owned GPU metrics resolve from `gpu_run` in `capture.json`, the capture README, the run summary, compare and the sweep report. Compare and the sweep already used `gpu_run`; the README and the run summary read the timing runs and showed n/a, which is now fixed. A copy of the `gpu` capture with `timestamp_query` false and no pass rows renders every owned GPU metric as n/a, adds the note to Validity and stays valid. This machine supports timestamp queries.

Design details the "Benchmark designs" sections leave open:
- **`gpu` layout.**
  - The sprite field and glow layer carry `ParallaxLayer` factor 0, so they stay on screen while the scripted camera ping-pongs across the 2,048 × 512 tilemap. Nothing moves them after startup.
  - Kinds are assigned by Weyl sequences, so the shares interleave within every z layer: `lit` orbs and crates, `shader_share` heavy sprites, the rest unlit textures.
  - A new `textures` knob (1–8, default 4) sets the number of unlit textures, alternating nearest and linear; the `throughput` preset needs one.
  - `bench-config` derives the expected batches: 56 sprite batches (8 layers × 7 keys), 1 glow and 1 per tile layer (tiles share an atlas page). The startup hook checks the sprite extract against that and logs the result.
- **`bench_heavy` loop count.** `shader_iters` is written into the material's authored defaults (`MaterialRegistry::allocate`, then `Renderer::reload_material`), so all heavy sprites share one UBO. A per-entity `UniformOverrideBlock` would make the default extract hash 256 bytes per heavy sprite every frame.
- **Registries after manifest composition.** Composition replaces `TilemapRegistry`, so the tilemap is inserted in the startup hook; inserted in `configure`, it vanished.
- **`particles` configs.** Config i emits about 150 × `rate` live particles per emitter. Continuous configs set `rate_hz` = target ÷ mean lifetime; pulses use 0.2–0.35 s intervals; bursts re-arm every 0.4–0.7 s.
  - The engine latches a burst after it fires (`continuous_accum` stays 1) whether or not `once` is set, so `rearm_bursts` clears the latch.
  - Pulse timers and burst phases start staggered.
  - `bench-config` logs `live_model`: emitters × 150 × `rate`, capped by `budget`.
- **`particles` animation.** 8-frame clips share one atlas page, so all animated sprites form one batch key. Even clips loop; odd clips are one-shots that `animate_sprites` restarts at frame 0, counted as a frame change. The bench inserts its own `AnimationRegistry`, since it loads no manifest.
- **`particles` guard.** `counter_band` checks that `live` is steady around its own median, not its level. The level shows in `live_model` and in compare's workload-drift check.
- **Row notes.** `gpu`: its GPU metrics come from the diagnostic run and are n/a without timestamp queries. `particles`: without T1, `unattributed` holds the particle stage (plus event flush), so the row owns it with `animate_sprites`.

Calibrated defaults, as medians of the `--repeat 3` captures. They replace the defaults the "Benchmark designs" tables list:

| Row | Default change | `total` p50 / p95 / p99 per run | Owned | Calibration check |
| --- | --- | --- | --- | --- |
| `gpu` | `sprites` 120,000 → 2,000; `post` full → light | 11.09/12.55/14.08, 11.10/12.77/13.95, 11.12/12.65/14.45 | `gpu` (scene pass) p50 7.82 / p95 8.30; `render_span` p50 11.48 / p95 11.95; `extract` p50 1.95 / p95 2.28; `render_encode` p50 2.68 / p95 3.07 | GPU span 11.19–11.46 ms (means) > `update` + `extract` + `render_encode` 4.74–4.79 ms ✓ |
| `gpu-throughput` | none (no band) | 25.62/26.22/27.20, 25.63/26.18/26.52, 25.57/26.38/26.58 | `extract` p50 23.99 / p95 24.45; `render_encode` p50 1.31 / p95 1.51 | — |
| `particles` | `emitters` 400 → 280; `animated` 40,000 → 8,000 | 11.02/11.64/12.18, 10.98/11.56/12.18, 10.89/11.54/11.88 | `unattributed` p50 2.38 / p95 2.98; `animate_sprites` p50 0.27 / p95 0.32 | `unattributed` + `animate_sprites` = 24.8–25.1% of `total` (means), ≥ 50% ✗ |

- **`gpu` at the design defaults did not fit the band.** 120,000 sprites gave a `total` p95 of 161 ms (scene pass 152 ms, about 1.2 µs per sprite; `perf-runs/20260930T085925Z-gpu`).
  - With `sprites=0`, the full post chain (5.7 ms GPU, god rays 2.6), SMAA (1.3 ms), glow, tiles and text still gave 14.7 ms p95 (`20260930T090141Z-gpu-set7fe83e`).
  - Removing the glow as well cut the scene pass to 0.41 ms but left p95 at 14.3 ms (`20260930T090232Z-gpu-setc7bc1b`): SMAA cost depends on content, and `smaa_blend_weights` rose from 0.23 to 2.56 ms on the bare tiles.
  - Tuning `sprites` alone would have meant about 500 sprites at 15.98 ms p95, on the band's edge (1,000 gave 17.13 ms; `20260930T090307Z-gpu-sweep-sprites`).
  - Asked, the owner left the choice to me. `post` now defaults to `light` (bloom and vignette); `full` stays a knob value. A single-run sweep (`20260930T164516Z-gpu-sweep-sprites`) gave p95 10.38, 12.81, 15.47 and 20.37 ms at 0, 2,000, 4,000 and 8,000 sprites. 2,000 leaves at least 3 ms to either edge.
  - The owned pass list follows the light chain: `post0_bloom_threshold`, `post0_bloom_composite` and `post1_vignette`, plus SMAA, text and present at p50. The bloom mip passes are reported, not owned.
- **The `particles` share check fails by construction.** At the design defaults `live` reached its target (median 60,416, largest deviation 1.7%), but `total` p95 was 24.0 ms and the default extract took 15.71 of 23.03 ms (means; `20260930T164814Z-particles`). Particles and animated sprites draw through it with `String` IDs: about 0.16 µs per sprite, against 0.06 µs in `gpu-throughput`'s one-texture field.
  - Per frame, one live particle costs about 0.24 µs (0.066 of it unattributed) and one animated sprite about 0.2 µs (0.038 in `animate_sprites`), so no mix reaches 50%.
  - Swept at `animated=8000` (`20260930T164848Z-particles-set0648e8-sweep-emitters`): `emitters` 240, 280 and 320 gave live medians of 36,258, 42,288 and 48,351, p95 of 9.78, 12.20 and 13.42 ms, and shares of 23.8%, 24.1% and 25.0%. A NoMachine client reconnected between 16:48:48Z and about 16:49:10Z, possibly during this sweep, so its absolute numbers are indicative only; the clean `--repeat 3` capture at 280 confirms p95 11.56 ms.
  - 280 emitters (about 70% of the 60,000 target) and 8,000 animated sprites land mid-band.
- **`gpu-throughput`** spends about 60 ns per sprite in `extract`, mostly the sort; `render_encode` is 1.3 ms.

Checks (Vulkan, `immediate` / latency 1, governor `performance`, tree at `d85a3dc` plus the P4 diff). `nxcodec.bin` was checked before every capture, probe and visual run, and after each from 16:50Z on. Three NoMachine sessions paused the work: until 08:58:50Z, from about 09:04Z (after the full-post sprite sweep) to 16:44:49Z, and from about 16:49Z to 16:49:55Z. No capture listed here overlaps them except the flagged sweep. The exploratory captures in the calibration notes are single runs.
- `just check`: pass. `just repo-check`: pass. `just script-test`: pass; `test-bench.py` also passes under Python 3.12.13.
- `just smoke`: pass (examples 5/5, M25 matrix 4/4, Benchmarks 13/13, other fixture sections unchanged). New row durations, dev build with 3 frames and `cargo run` overhead included, median of 3: `gpu` at `min` 0.41 s, `default` with GPU timing 0.61 s, `throughput` 2.50 s; `particles` at `min` 0.25 s and `default` 0.35 s; an M25 row (`msaa=4 depth_sort=gpu_depth`) 0.41 s. No row needs a per-row timeout.
- **M25 precedence: the environment wins.**
  - `gpu` applies `aa` and `bloom_mips` through `engine_config` only where `TUNGSTEN_RENDER_MSAA`, `TUNGSTEN_RENDER_POST_AA` or `TUNGSTEN_RENDER_BLOOM_MAX_MIPS` is unset, and logs a warning when one overrides a knob. The benchmark never sets `depth_sort`, so `TUNGSTEN_RENDER_DEPTH_SORT` always applies.
  - Verified with dev captures at `min` (`aa=off`): two `TUNGSTEN_RENDER_MSAA=1` captures were byte-identical, the MSAA 4 capture differed, and `gpu_depth` differed from `cpu_stable`.
  - The runner clears `TUNGSTEN_RENDER_*` for captures, so captures always follow the knobs.
- `just visual`: passed twice in a row (16:51:19Z and 16:51:20Z). The fixture was regenerated in a dev build with the README command. Reference-machine block:
  - OS: Arch Linux, kernel 7.2.7-arch1-1, X11 session
  - GPU: AMD Radeon 660M (integrated, Ryzen 5 6600H)
  - Driver: Mesa 26.2.3 RADV (`RADV REMBRANDT`), Vulkan API 1.4.354
  - wgpu backend: Vulkan, wgpu 30.0.1
  - Toolchain: rustc 1.98.1, `dev` profile
  - Date: 2026-09-30; commit `d85a3dc` plus the P4 changes
  - SHA-256: `0521a2a0c32c9774c9f4f26926eb2f85f3349cc1bedbb86dc4ea0a58c377d42b`
- `just perf run gpu --repeat 3` (`perf-runs/20260930T164606Z-gpu`): valid, with the per-pass GPU rows in the README. Digests match (`f45f26ebdf706840` in all 3 timing and 3 GPU diagnostic runs). `timestamp_query` is true. Peak RSS: 732.6, 731.8 and 730.7 MiB.
- `just perf run gpu --preset throughput --repeat 3` (`perf-runs/20260930T164703Z-gpu-throughput`): valid, no guards. Digests match (`121f8ef1503d8697`). Peak RSS: 251.2, 251.6 and 258.7 MiB.
- `just perf run particles --repeat 3` (`perf-runs/20260930T165044Z-particles`): valid. The guard passes: `live` stays in 41,607..43,097 around a median of 42,288, a largest deviation of 1.9%, over 299 frames. Digests match (`08a3c7c126a0d43a`). Peak RSS: 141.1, 140.7 and 141.1 MiB. RSS growth is 13.2, 6.9 and 10.8 KiB/s (reported only).
- Unchanged work elsewhere: single-run captures after the harness changes reproduce P1–P3's digests: `physics` `c972b2818d486617`, `physics-sparse` `c95fadc938a3689e`, `ecs` `c785506ced04de4c`, `churn` `86014148b7a94681` (`perf-runs/20260930T1700*`).

Lights sweep: `just perf run gpu --sweep lights=0,4,8,16 --repeat 3` (`perf-runs/20260930T165149Z-gpu-sweep-lights`) wrote `sweep.md` and `sweep.html`, and all four captures are valid. Medians of the per-run values, in ms:

| `lights` | `gpu` (scene) p50 / p95 | `render_span` p50 / p95 | `extract` p50 | `render_encode` p50 | `total` p95 | Peak RSS (MiB) |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 6.54 / 6.68 | 10.20 / 10.36 | 1.93 | 2.93 | 11.89 | 732.7 |
| 4 | 6.53 / 7.14 | 10.21 / 10.80 | 1.93 | 2.86 | 12.40 | 731.6 |
| 8 | 7.39 / 7.70 | 11.04 / 11.36 | 1.94 | 2.69 | 12.44 | 731.4 |
| 16 | 6.89 / 8.14 | 10.55 / 11.78 | 1.93 | 2.66 | 12.52 | 731.5 |

At 2,000 sprites, 1,000 of them lit, lighting adds little: the lit fragment count is small. The scene pass at `lights=16` measured 7.82 ms p50 in the 16:46Z capture and 6.89 ms here. It is stable within a capture (7.68–7.84 across runs) but moved about 12% between captures.

Capacity (`perf-runs/20260930T165405Z-capacity-60hz` and `perf-runs/20260930T165615Z-capacity-144hz`). No probe failed a guard. p95, p99, the limiting stage and peak RSS are medians over the 3 confirmation probes; for an unreachable budget they come from the last probe.

| Row | Budget | Max scale | Key counts | p95 / p99 (ms) | Limiting stage | Peak RSS | Probes | Wall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `gpu` | 60hz | 1.542 | sprites 3,084 | 16.46 / 21.23 | `present` ✓ 7.73 ms | 747.6 MiB | 9 | 35 s |
| `gpu-throughput` | 60hz | 0.595 | sprites 237,842 | 16.34 / 16.63 | `stage.extract` ✓ 14.95 ms | 212.7 MiB | 9 | 42 s |
| `particles` | 60hz | 1.297 | emitters 363, animated 10,375 | 16.58 / 16.97 | `stage.extract` ✗ 10.16 ms (declared `stage.unattributed`) | 147.4 MiB | 9 | 49 s |
| `gpu` | 144hz | < 0.0005 (budget not reachable) | sprites 1 | 8.70 / 8.85 | `present` ✓ 6.43 ms | 171.2 MiB | 12 | 26 s |
| `gpu-throughput` | 144hz | 0.310 | sprites 124,186 | 6.67 / 7.02 | `stage.extract` ✓ 5.46 ms | 177.5 MiB | 10 | 25 s |
| `particles` | 144hz | 0.648 | emitters 182, animated 5,187 | 6.50 / 7.08 | `stage.extract` ✗ 3.97 ms (declared `stage.unattributed`) | 136.9 MiB | 9 | 20 s |

- `gpu` at 144hz: the fixed load (bloom, vignette, SMAA High, three tile layers and text at 1080p) keeps p95 at 8.7 ms even at the scale floor (1 sprite, 10 glow sprites, 2 glyphs), so the search reports the bound. Before the lower-bound fix this row would have halved without end.
- `particles` is `extract`-limited at both budgets, the same finding as its share check. The declaration stays `stage.unattributed`.
- `particles` at 60hz: one of the three confirmations failed (17.20 ms), but the median held.

For P5 and later phases:
- **Tiles under `gpu_depth`.** `extract_tilemaps` writes `z_norm` 0, the nearest depth, so under `TUNGSTEN_RENDER_DEPTH_SORT=gpu_depth` the tilemap covers every sprite (the M25 `gpu_depth` rows run but render only tiles and text). `integrated` combines tiles and sprites, so it should run with `cpu_stable`, the default. This is an engine observation for a separate fix.
- **Burst emitters never repeat.** `EmissionKind::Burst` fires once whatever `once` says, because the `continuous_accum` latch never resets. `integrated`'s spark bursts need re-arming, a fresh emitter per hit, or `spawn_particle_via`. This is an engine observation.
- **Text RSS.** The text pipeline keeps every changed section's shaped buffer for 360 frames (`BUFFER_CACHE_TTL_FRAMES`, pruned every 120). With 100 sections changing per frame, `gpu`'s RSS grows about 139 MiB/s over the second half of a run and peaks near 731 MiB, against 171 MiB with 2 glyphs. `integrated`'s changing HUD and name tags will show the same growth, bounded by the TTL, so peak RSS depends on capture length. The growth is reported only.
- **Composition replaces registries.** Manifest composition replaces `TilemapRegistry`, so `integrated` must register generated tilemaps, particle configs and animations in the startup hook, or skip manifests, as `particles` does.
- **The default extract dominates sprite-heavy rows.** String-ID sprites cost about 0.16 µs each in the default extract. `integrated` will be extract-heavy wherever particles, animation and props all draw through it.
- **GPU variance between captures.** The same `gpu` configuration moved about 12% in scene-pass time between captures while each capture's runs agreed within 2%. SMAA cost also depends on scene content. P5's suite A/A may show `noisy` GPU rows, since per-run variance within a capture underestimates the variance between captures.
- **`engine_config` hook.** `integrated` needs it for `aa` (SMAA High through `render.post_aa`) and the bloom mips; its `post` chain is a world resource set in `configure`. The same environment-first rule should apply. Resolution changes stay local to `gpu`: `physics`, `ecs` and `churn` keep 1920×1080.
- **Capacity bounds.** Any key knob whose range starts at 0 now floors the `scale` axis at its first nonzero value. `integrated`'s actors, crates, props, torches and pickups all start at 0.
- **Row notes.** `describe` prints a row's `note`; P6 can carry them into `docs/perf/benchmarks.md`.

### P5: integrated and suite runs (2026-09-30)

Implemented steps 1–3 with no engine or library-crate change:
- `examples/02_bench/src/integrated.rs` (501 lines) with `src/integrated/level.rs` (308), `assets.rs` (291), `scene.rs` (381), `runtime.rs` (295) and `systems.rs` (517), at `workload_version` 1, registered in `main.rs`. Warm-up 120, GPU diagnostic run on. The knobs follow §6 plus `bloom_mips` (1–8, default 6): `engine_config` applies it and `aa` under the environment-first rule, and `configure` sets the `post` chain as a world resource. Presets `min` and `default` as designed.
- Row `integrated` owns `stage.total` p50/p95/p99 and `jitter`, has the guard `bench.view_out <= 0` (`counter_max`), declares `system.physics_step` as its bottleneck, reports `actors`, `crates`, `props`, `torches` and `pickups` as key counts, and carries a note.
- Counters, in line order: `actors`, `projectiles` (live), `hits`, `particles` (live), `lights` (point lights reaching the view, before the extract keeps 16), `camera_x`, `view_out`, `flashing`, `landings`, `shots`, `turns` and `events` (collision events).
- `knobs.rs`: `unless_env` moved there from `gpu.rs`, taking the benchmark name, so `gpu`'s warning text is unchanged.
- `gen.rs`: one `pack` helper serves `register_atlas` and the new `register_lit_atlas` (albedo, normal and emissive pages under one handle); new `walker_textures` and `ridge_rgba`. `register_atlas` output is unchanged: the `just visual` capture is byte-identical to the fixture (SHA-256 `0521a2a0…`).
- `scripts/bench_report.py`:
  - `jitter` is a derived per-run statistic (`DERIVED_STATS`, read by `metric_value`), so the capture README, the run summary, compare and the suite report all show it, and older captures derive it too.
  - Compare judges jitter with p99's threshold (`TAU_FROM`; the decision is below).
  - `assemble_suite`, `suite_readme`, `suite_compare`, `suite_compare_markdown` and `suite_compare_html`; `any_regressed` reads suite reports too.
- `scripts/bench.py`:
  - `suite [--preset] [--scale] [--only ROWS] [--repeat N] [--compare BASELINE]` takes the rows from `describe`, in its order, and resolves every config before the first run. Captures go to `perf-runs/<UTC>-suite[-<preset>][-s<scale>]/<row>/`, then `suite.json` and `README.md`; the exit status is 3 when a row is invalid. Rows run 300 frames; the plan's flag list has no `--frames`.
  - `compare` takes two captures or two suites, each a baseline name or a directory; mixing kinds is an error. A suite compare writes each row's report into `<out>/<row>/`, then the suite-level `compare.json`, `compare.md` and `compare.html`: verdict counts, `total` and peak-RSS bars per row, and each row's owned table. `--fail-on regressed` covers every row.
  - `baseline save` accepts a suite (`baseline.json` gains `kind`, plus `rows` for a suite). `run --compare` rejects a suite baseline and `suite --compare` a single capture.
- `scripts/test-bench.py`, 22 tests (4 new):
  - the suite row list from a synthetic describe: the `--preset` rule, `--only`, unknown rows, a missing preset;
  - `suite.json` and the suite README;
  - suite compare: both outputs with peak RSS, no external references, tag balance, a saved suite baseline, `--fail-on regressed`, the mixed-kind error;
  - jitter: derived values, the README row, p99's τ, and a tail that grows while p50 improves and p99 stays within its τ.
- `scripts/smoke-examples.sh`: 15 Benchmarks rows, adding `integrated` at `min` and `default`; `test-smoke-examples.sh` expects them.

Design details the plan leaves open:
- **Level** (`level.rs`, seeded; 96 rows of 16 px tiles).
  - The ground surface wanders between rows 80 and 90: flat runs of 8–24 columns joined by single steps or staircases of 3–5 steps.
  - The collision layer holds a three-tile crust under the surface; the rendered terrain fills to the bottom row.
  - Full-height walls close both ends. Walls 1–2 columns wide and 3–6 tiles high stand on the ground every 48–96 columns.
  - Three tiers at rows 72, 60 and 48 (±1) carry one-tile platforms 10–36 tiles long with gaps of 3–9 tiles.
  - The engine never draws a collision layer, so the tilemap holds the collision layer, the rendered terrain and two decoration layers (grass on surfaces, moss under platforms and on the ground).
  - At the default: a 24,576 × 1,536 px level, 8,528 tile proxies, 317 walkable spans and 81,776 px of walkable surface. Placement spreads each kind evenly over slots along the spans, in x order.
- **Walkers.** Dynamic 12 × 20 px AABBs drawing a 24 px lit clip (6 frames, 4 color variants, one lit atlas page).
  - The AI reads last frame's contacts from dense per-slot arrays. A wall contact turns a walker, unless it is a one-tile step, which the walker hops. A ledge (no ground within two tiles ahead) turns it.
  - A random hop comes every 1.5–4 s, only where ground 56 px ahead supports the landing, so the tiers keep their walkers and the load stays steady.
  - Touching down after 6 or more frames in the air sends a `SquashEvent` (`OnLand`); the engine's squash trigger and tick systems follow.
  - Every tenth walker is a caster (250 at the default). It fires 3 px circles at 700 px/s, 8–29° above the horizontal.
- **Flash.** The default extract draws a lit sprite without its material (lit wins, M29, with a warning per sprite and frame).
  - A struck walker therefore switches to an unlit copy of its clip (a second atlas page and clip set) drawn with `damage_flash`, an override block (color in `vec4[0]`) and a `UniformScalar` F0 tween from 1 to 0 over 0.3 s, tagged `flash`.
  - The tween stage sends `TweenComplete` after the systems, so the next frame reads it in the previous window, restores the lit clip and removes the block.
  - Each flashing walker has its own override hash and so its own batch: 17.5 at a time at the default.
- **Hit sparks** come from a fresh emitter per hit (`Burst { count: 12, once: true }`), spawned through the `CommandBuffer` and despawned when its `ParticleSystemDrained` arrives. The design uses neither a re-armed latch nor `spawn_particle_via`. A hit also despawns the projectile, knocks a struck crate and wakes it with `physics::wake`, and adds 0.08 trauma when within 64 px of the view.
- **Guard counter.** `view_out` is how far the final view, shake included, leaves the level, rounded up to whole px. `counter_max` with `max: 0` checks it, so no new guard kind was needed.
  - The camera uses `CameraMode::Scripted`: `camera_script` writes the path's base position, then `shake_tick` and `camera_update` add shake and clamp to the level bounds.
  - The path keeps 16 px from every edge, more than the 6 × 4 px shake.
- **Draw order.** The parallax strips (four layers, factors 0.1–0.7, one atlas page) have the lowest z, so the default extract emits their batches first; the extract splices `extract_tilemaps`' batches in after them, ahead of every other sprite. Tiles write `z_norm` 0, so the row relies on the default `cpu_stable`, as its note says.
- **Particles and text.**
  - `ParticleBudget` is torches × 32 + 4,096 (13,696 at the default), so it never clips the scaled workload. The 300 fire emitters keep about 6,200 particles live.
  - Three HUD lines plus `tags` name tags in `mono`: the tags sit on the walkers nearest the view center and show name, hit points and x. HUD and tags change every frame.
- **Bloom.** `game` is bloom, tonemap and vignette. The bloom threshold is 0.8 with intensity 0.5: at `gpu`'s 0.6 the dense frame was veiled. The passes are the same.
- **`tile_collision=off`** spawns the collision cells as 328 merged static boxes: runs along rows, merged downward. The AI reads the level grid in both modes, so behavior matches (88 turns per frame either way).
- **Bottleneck.** `system.physics_step`: 6.66 of 12.03 ms (means) at the default, ahead of the default extract's 2.60 ms. The row note explains this.
- **Suite `--preset` rule** (decided 2026-09-30): `--preset P` replaces the preset of the rows whose own preset is `default`. `physics-sparse` and `gpu-throughput` keep `sparse` and `throughput`, because replacing their preset would measure another row.
  - Before anything runs, the suite checks that P exists for every affected benchmark and that each resolved config still measures its row.
  - `suite.json` records the request, and the README states the rule.
  - Rejecting `--preset` was the alternative; it would have dropped quick suites such as `--preset min`.

Calibrated default, from a quiet `--repeat 3` capture (`perf-runs/20260930T175816Z-integrated`). It replaces the design table's `actors`; the other counts stay as designed.

| Row | Default change | `total` p50 / p95 / p99 per run | Jitter per run | Target |
| --- | --- | --- | --- | --- |
| `integrated` | `actors` 3,000 → 2,500 | 11.97/12.37/12.52, 12.04/12.47/12.53, 12.40/12.90/13.29 | 0.55, 0.49, 0.89 | p95 10–16 ms ✓ |

- The design defaults gave p95 13.71 ms (`perf-runs/20260930T175134Z-integrated`, single run). A single-run sweep (`20260930T175311Z-integrated-sweep-actors`), in ms:

  | `actors` | `total` p50 / p95 / p99 | `physics_step` mean | `extract` mean | Turns per walker per s |
  | --- | --- | --- | --- | --- |
  | 1,500 | 9.61 / 9.99 / 10.42 | 4.72 | 2.41 | 1.4 |
  | 2,000 | 10.96 / 11.36 / 11.64 | 5.75 | 2.56 | 1.8 |
  | 2,500 | 12.01 / 12.38 / 12.53 | 6.57 | 2.63 | 2.1 |
  | 3,000 | 13.35 / 13.91 / 14.77 | 7.76 | 2.72 | 2.4 |
  | 3,500 | 14.63 / 15.10 / 15.64 | 8.71 | 2.85 | 2.6 |

  Each 500 walkers add about 1.3 ms of p95, nearly all in `physics_step`. 2,500 leaves 2.4 ms to the band's floor and 3.6 ms to its ceiling, cuts the jitter to 0.52 ms against 1.42 ms at 3,000, and jams the walkers less.
- Stage means at the default, in ms:
  - `update` 7.46: `physics_step` 6.66, `animate_actors` 0.37, `collision_events` 0.16, `actor_ai` 0.11;
  - `extract` 2.60;
  - `render` 1.44, including `render_encode` 0.96 and a present wait of 0.47;
  - `unattributed` 0.37 and `flush` 0.17.

  The frame is CPU-bound. The GPU diagnostic run puts `render_span` at 6.19 ms p50: SMAA 2.56, scene 1.30, bloom 1.05 (all its passes), tonemap 0.34, present 0.32 and vignette 0.26.
- Interaction counters, means per frame:
  - physics: 13,044 proxies (8,528 tile proxies plus 4,516 dynamic bodies), 951 sleeping crates, 6,677 pairs, 6,282 contacts and 23,110 collision events;
  - combat: 16 live projectiles, 1.26 shots, 1.23 hits and 17.5 flashing walkers;
  - 6,203 live particles;
  - lights: 27.7 point lights reach the view (25–30), and the extract keeps 16;
  - walkers: 23.9 landings and 88.2 turns.
- `tile_collision=off` (`20260930T175536Z-integrated-setc49aa7`, single run): 328 static boxes replace the 8,528 tile proxies. `physics_step` drops from 6.57 to 4.27 ms, p95 from 12.38 to 9.66 ms and events from 23,110 to 16,531; turns stay at 88.4.
- The first functional runs overlapped a NoMachine session: `total` averaged 22 ms against 13.35 ms quiet. Every number here comes from a quiet capture.

Jitter threshold (decided 2026-09-30 and implemented):
- τ_jitter = max(8% × baseline p99, 0.05 ms). Jitter shares p99's threshold, about 1.0 ms for `integrated`.
- **Why.** Jitter is p99 − p50, and p99's noise dominates it. Over 9 runs of one build (3 captures), per-run jitter has a mean of 0.69 ms and an SD of 0.22 ms (CV 32%); p99's SD is 0.39 ms and p50's 0.19 ms.
  - A τ relative to jitter itself, such as 25% of 0.69 ms (0.17 ms), sits below the Welch half-width even at `--repeat 5` (about 0.33 ms), so every A/A would read `noisy`.
  - With p99's τ, the suite A/A reads `unchanged` (Δ +0.04 ms, interval [−0.67, +0.75]).
  - Jitter still catches tail growth p99 misses when p50 moves the other way: p50 −0.6 ms with p99 +0.5 ms is +1.1 ms of jitter.
- `judge_metric` takes jitter's τ from the baseline's per-run p99 mean (`TAU_FROM` in `bench_report.py`); the interval rule stays. The jitter test covers the case above: p50 reads `improved`, p99 `unchanged` and jitter `regressed`.

Checks (Vulkan, `immediate` / latency 1, governor `performance`, tree at `b54b389` plus the P5 diff):
- A scratchpad wrapper, `quiet.sh`, ran every capture, probe, smoke run and the visual test. It refuses to start while `nxcodec.bin` runs and polls for it once a second.
- One NoMachine session connected at 18:05:15Z for about 10 s, during the first second suite (`perf-runs/20260930T180259Z-suite`). That suite is excluded and was rerun.
- `just check`: pass. `just repo-check`: pass. `just script-test`: pass; `test-bench.py` also passes under Python 3.12.13.
- `just smoke`: pass (examples 5/5, M25 matrix 4/4, Benchmarks 15/15, other sections unchanged). New row durations, median of 3, dev build with 3 frames and `cargo run` overhead included: `integrated` at `min` 0.42 s, `default` 0.68 s. No row needs a per-row timeout.
- `just visual`: pass, byte-identical after the `gen.rs` change.
- `just perf run integrated --repeat 3` (`perf-runs/20260930T175816Z-integrated`): valid.
  - The guard passes: `view_out` is 0 in all 299 checked frames.
  - Digests match: `9b2617e4c1ab23f7` in all 3 timing and all 3 GPU diagnostic runs.
  - `timestamp_query` is true, and the GPU rows come from the diagnostic runs.
  - Peak RSS: 283.6, 281.0 and 281.2 MiB.
  - Jitter: 0.55, 0.49 and 0.89 ms, median 0.55.
  - RSS grows 19.1–19.8 MiB/s: the text buffer cache, reported only.
- After those captures, `assets.rs` and `runtime.rs` were split out of `scene.rs` and `systems.rs` to keep every module under 600 lines. The move changed no code: `just check`, `just smoke`, `just script-test` and `just repo-check` pass again, and one run (`perf-runs/20260930T182628Z-integrated`) reproduces digest `9b2617e4c1ab23f7` with p95 12.42 ms.

Suites. `just perf suite --repeat 3` wrote `perf-runs/20260930T175915Z-suite` and, as the rerun, `perf-runs/20260930T180644Z-suite`.
- Each took 3 min 30 s; all eight captures are valid, and `suite.json` and `README.md` are written.
- Every row reproduces its recorded digest, so the shared harness changes alter no other benchmark's work and no `workload_version` changes:

  | Row | Digest |
  | --- | --- |
  | `physics` | `c972b2818d486617` |
  | `physics-sparse` | `c95fadc938a3689e` |
  | `ecs` | `c785506ced04de4c` |
  | `churn` | `86014148b7a94681` |
  | `gpu` | `f45f26ebdf706840` |
  | `gpu-throughput` | `121f8ef1503d8697` |
  | `particles` | `08a3c7c126a0d43a` |
  | `integrated` | `9b2617e4c1ab23f7` |

`just perf compare` of the two suites wrote the suite-level `compare.md` and `compare.html` with peak RSS, plus each row's report. With jitter judged (`perf-runs/20260930T183559Z-compare-suite`), the owned verdicts are 0 regressed, 0 improved, 25 unchanged and 30 noisy. The first report (`perf-runs/20260930T181022Z-compare-suite`) predates the decision and left jitter without a verdict. Headless Chromium with a dead proxy renders the page offline in the light and dark schemes.

| Row | Owned verdicts | `total` p95, suite 1 → 2 (ms) | Jitter (ms) | Peak RSS (MiB) | Peak RSS verdict |
| --- | --- | --- | --- | --- | --- |
| `physics` | 3 unchanged | 13.98 → 14.06 | 0.87 → 0.68 | 132.5 → 132.6 | unchanged |
| `physics-sparse` | 2 unchanged | 10.06 → 10.07 | 2.12 → 1.90 | 128.2 → 127.9 | unchanged |
| `ecs` | 4 unchanged, 12 noisy | 11.53 → 11.54 | 0.86 → 0.65 | 163.1 → 163.1 | unchanged |
| `churn` | 2 unchanged, 4 noisy | 10.21 → 10.27 | 1.13 → 1.30 | 143.9 → 143.9 | unchanged |
| `gpu` | 10 unchanged, 6 noisy | 12.60 → 12.51 | 2.80 → 2.49 | 731.7 → 732.7 | unchanged |
| `gpu-throughput` | 2 unchanged, 2 noisy | 26.26 → 26.02 | 1.05 → 1.22 | 251.4 → 255.4 | noisy |
| `particles` | 1 unchanged, 3 noisy | 11.79 → 11.52 | 1.16 → 1.12 | 142.1 → 141.7 | unchanged |
| `integrated` | 1 unchanged (jitter), 3 noisy | 12.50 → 12.51 | 0.52 → 0.64 | 280.4 → 281.7 | unchanged |

Where the noisy verdicts come from, at n = 3:
- `ecs`: 12 of its 14 per-system p50 rows. Most cost 0.1–0.7 ms and sit on the 0.02 ms floor with intervals of about ±0.03 ms; `follow` (1.9 ms) and `stats_decay` spread ±0.3 ms.
- `churn`: `flush` p50 and p95 (intervals of about ±0.5 ms), plus `churn_toggle` and `churn_spawn` on the floor.
- `gpu`: the scene pass and `render_span` p50, with intervals of about ±1.1 ms (P4's GPU drift), plus `extract` and `render_encode` at p50 and p95.
- `gpu-throughput`: `render_encode` p50 and p95.
- `particles`: `unattributed` p95 and both `animate_sprites` statistics.
- `integrated`: `total` p50/p95/p99, with intervals of ±0.7–1.6 ms.

Jitter columns are the suite README medians; the jitter compare row uses means. No regressed or improved verdict appeared.

Capacity (`perf-runs/20260930T181108Z-capacity-60hz` and `perf-runs/20260930T181709Z-capacity-144hz`). No probe failed a guard or a child. p95, p99, the limiting stage and peak RSS are medians over the confirmation probes; for an unreachable budget they come from the last probe.

| Row | Budget | Max scale | Key counts | p95 / p99 (ms) | Limiting stage | Peak RSS | Probes | Wall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `physics` | 60hz | 1.189 | balls 9,514 | 16.48 / 16.82 | `system.physics_step` ✓ 15.69 ms | 133.9 MiB | 9 | 50 s |
| `physics-sparse` | 60hz | 1.542 | bodies 12,338 | 16.20 / 16.42 | `system.physics_step` ✓ 13.64 ms | 130.5 MiB | 9 | 39 s |
| `ecs` | 60hz | 1.477 | entities 369,208 | 16.70 / 17.29 | `system.heading` ✓ 3.74 ms (declared `update`) | 182.7 MiB | 9 | 48 s |
| `churn` | 60hz | 1.414 | population 176,776 | 15.92 / 16.47 | `stage.flush` ✓ 12.58 ms | 156.2 MiB | 9 | 35 s |
| `gpu` | 60hz | 1.534 | sprites 3,068 | 16.43 / 19.99 | `present` ✓ 7.63 ms | 746.0 MiB | 12 | 47 s |
| `gpu-throughput` | 60hz | 0.569 | sprites 227,758 | 16.20 / 16.63 | `stage.extract` ✓ 14.70 ms | 210.6 MiB | 9 | 42 s |
| `particles` | 60hz | 1.242 | emitters 348, animated 9,935 | 15.68 / 16.11 | `stage.extract` ✗ 9.49 ms (declared `stage.unattributed`) | 145.0 MiB | 9 | 47 s |
| `integrated` | 60hz | 1.297 | actors 3,242, crates 2,594, props 7,781, torches 389, pickups 1,297 | 16.23 / 16.95 | `system.physics_step` ✓ 9.14 ms | 251.9 MiB | 9 | 46 s |
| `physics` | 144hz | 0.545 | balls 4,362 | 6.81 / 6.86 | `system.physics_step` ✓ 6.49 ms | 128.5 MiB | 9 | 22 s |
| `physics-sparse` | 144hz | 0.677 | bodies 5,417 | 6.54 / 6.62 | `system.physics_step` ✓ 5.39 ms | 127.0 MiB | 9 | 17 s |
| `ecs` | 144hz | 0.545 | entities 136,314 | 6.66 / 7.07 | `system.heading` ✓ 1.42 ms (declared `update`) | 145.6 MiB | 9 | 21 s |
| `churn` | 144hz | 0.673 | population 84,179 | 6.39 / 6.79 | `stage.flush` ✓ 5.00 ms | 138.6 MiB | 12 | 21 s |
| `gpu` | 144hz | < 0.0005 (budget not reachable) | sprites 1 | 8.75 / 8.93 | `present` ✓ 6.45 ms | 170.9 MiB | 12 | 26 s |
| `gpu-throughput` | 144hz | 0.310 | sprites 124,186 | 6.83 / 7.29 | `stage.extract` ✓ 5.50 ms | 178.1 MiB | 10 | 26 s |
| `particles` | 144hz | 0.648 | emitters 182, animated 5,187 | 6.50 / 7.01 | `stage.extract` ✗ 3.92 ms (declared `stage.unattributed`) | 137.5 MiB | 9 | 20 s |
| `integrated` | 144hz | 0.092 | actors 229, crates 184, props 551, torches 28, pickups 92 | 6.90 / 7.10 | `present` ✗ 3.17 ms (declared `system.physics_step`) | 211.4 MiB | 15 | 31 s |

- **`integrated` at 144hz is present-bound, a finding.** The scale moves the scene counts but not the level or the 1080p post chain. At scale 0.092, `physics_step` still costs 1.2 ms (the 8,528 tile proxies), and the GPU span (SMAA, bloom, tonemap, vignette; about 6 ms) sets a floor near 6.9 ms. The declaration stays `system.physics_step`.
- Step-downs: `gpu` at 60hz, and `churn` and `integrated` at 144hz, each failed a first confirmation (medians 16.75, 6.91 and 6.91 ms) and held one tolerance step lower.
- `particles` stays `extract`-limited at both budgets, as in P4.

For P6:
- **Owned verdicts at `--repeat 3`.** The suite A/A shows no `regressed` or `improved` verdict, but 30 of 55 judged pairs read `noisy`. The repeat-5 A/A decides whether the 0.02 ms floor on `ecs`'s per-system rows, `churn`'s `flush` and the GPU scene pass stay quiet. `integrated`'s `total` statistics were `noisy` at n = 3: per-run p95 spread 12.37–13.10 ms across captures, with one slower run in most captures.
- **Decided.** Jitter takes p99's τ, and `suite --preset` applies only to rows whose preset is `default` (both above, and in the Runner and Compare sections); the docs should describe both.
- **Docs.**
  - The `integrated` section of `docs/perf/benchmarks.md` needs:
    - the `bloom_mips` knob;
    - the counters and the `view_out` guard;
    - the bottleneck choice and the row note;
    - the flash and spark mechanisms;
    - the calibration above and `workload_version` 1.
  - "Where the benchmarks live" lists only `integrated.rs` and `level.rs`; the module now also has `assets.rs`, `scene.rs`, `runtime.rs` and `systems.rs`.
  - The tracked-rows table's guard for `integrated` is `bench.view_out <= 0`.
  - `profiling-workflow.md` needs `suite`, suite compare, suite baselines, `suite.json` and the `--preset` rule.
- **Suite timing.** A suite at `--repeat 3` takes about 3.5 minutes on this machine, so expect about 6 minutes at `--repeat 5`.
- **Text RSS.** `integrated`'s RSS grows about 19.5 MiB/s from the name tags and HUD, and its peak RSS (about 281 MiB) depends on capture length until the 360-frame TTL saturates.
- **Engine observations, not fixed here:**
  - The default extract warns once per sprite and frame when a lit sprite carries a material, so lit-plus-material costs a log call per sprite.
  - Tiles still write `z_norm` 0, which keeps `integrated` on `cpu_stable`.
- **NoMachine.** `quiet.sh` caught a 10-second session mid-suite; the before-and-after checks alone would have missed it. P6 could make the runner record `nxcodec.bin` sightings in the provenance.

### P6: retire the old suite, docs and decision (2026-09-30)

Implemented steps 1–4. No benchmark work and no engine code changed beyond the planned comments: every suite capture below reproduces its recorded digest.
- Step 1: deleted `examples/02_sprite_stress/` (its visual test and fixture included), `scripts/perf-capture.sh` and `scripts/test-perf-capture.sh`, and dropped the workspace member. One plain `cargo build` refreshed `Cargo.lock`, which lost only the `example-02-sprite-stress` package block.
  - `justfile`: `perf-legacy` is gone and `perf-test` runs `scripts/test-bench.py` only. `.claude/settings.json` drops the old allow.
  - `test-smoke-examples.sh`: the stub metadata drops the package and the `scene=` field; the generic loop expects 4/4, and 3/4 in the child-failure and timeout cases.
  - `scripts/bench.py`: `LEGACY_SCENES` and the `perf-legacy` redirect are gone; the docstrings and two errors no longer name the old script (wording only).
- Step 2:
  - `docs/perf/profiling-workflow.md` is rewritten as "Profiling Workflow" with the 15 sections; `docs/perf/benchmarks.md` is new and in `check-repo.py`'s `DOCS`.
  - `docs/LLM_INDEX.md`: the telemetry row, the runner row and the two benchmark rows. `AGENTS.md`: the three edits.
  - `README.md`: the run line, the perf block and a documents-table row for `benchmarks.md`. Its tools note now says the smoke scripts need `jq` and GNU `timeout`; the runner needs neither.
  - `DESIGN.md`: the scene and tooling bullets become one pointer bullet; the telemetry bullets keep their wording.
  - The `tungsten-perf` skill is rewritten; its "do not" list keeps its three items, the pacing one pointing at `--present-mode` / `--max-frame-latency`, and adds a fourth against loosening thresholds or owned metrics.
  - Comments: `app.rs` (both parser references) and `.cargo/config.toml` (the RUSTFLAGS note). The April 2026 pacing matrix moved into `D-078` (A8).
  - The plan moves to the archive, so the comments that cited it now cite the perf docs: `examples/02_bench/src/main.rs` and `knobs.rs`, the fixture README's description (its provenance line names the archived path), and the smoke Benchmarks comment. That comment also still said "full post chain" for the `gpu` default row; it now says bloom and vignette.
- Step 3: `D-078` and its index row. The `D-044` index row gains "Narrowed by `D-078`", as `D-002`'s row does for `D-070`; the `D-044` entry itself is unchanged. The `[Unreleased]` changelog entry covers P1–P6.
- Overlap: the draft `docs/plans/debug-cleanup-docs-pass.md` also touches `smoke-examples.sh`, `app.rs`, `check-repo.py`, `DESIGN.md`, `README.md`, `CHANGELOG.md`, `LLM_INDEX.md`, `profiling-workflow.md` and the `tungsten-perf` skill. P6 lands first and leaves that plan unedited, so it rebases on this one. Its `profiling-workflow.md` items (the "M12" title, the duplicated scene-only statement, the M17 wording) and its skill trim target text P6 replaced, and DESIGN's perf section already ends in a pointer.

Checks (Vulkan, `immediate` / latency 1, governor `performance`, tree at `a77e204` plus the P6 diff). A scratchpad wrapper ran every capture, probe, smoke run and visual run with `WGPU_BACKEND=vulkan`. It refuses to start while `nxcodec.bin` runs, polls for it once a second and checks again at the end.
1. `just smoke`: pass. Examples 4/4, M25 matrix 4/4, Post-stack 2/2, Post-AA 1/1, Bloom 1/1, Lighting 1/1, Game-feel 2/2, Benchmarks 15/15. No benchmark row changed, so row durations weren't remeasured; the whole run took 30 s.
2. Capacity: every tracked row reports a maximum scale at both budgets, or the explicit bound for `gpu` at 144hz (below).
3. `just perf compare` of two `integrated` captures (`perf-runs/20260930T194826Z-compare-integrated`) and the suite compare wrote `compare.md` and `compare.html`, both with peak RSS. `just script-test` passes, `test-bench.py` with 22 tests.
4. `rg -n 'sprite-stress|ecs-high-load|physics-stress|render-features|perf-capture|STRESS_' docs/perf docs/LLM_INDEX.md`: no output.
5. `rg -l 'example-02-sprite-stress|02_sprite_stress|perf-capture|STRESS_(SCENE|COUNT)' AGENTS.md README.md DESIGN.md justfile Cargo.toml .cargo .claude docs/perf docs/LLM_INDEX.md scripts examples crates`: no output.
6. `just check`: pass.
7. `just repo-check` and `just ctx`: pass. Byte budgets: `AGENTS.md` 6,128 B (limit 6,144; −16), `docs/LLM_INDEX.md` 6,523 B (limit 8,192; +64), the `tungsten-perf` skill body 7,387 B (limit 8,192) with a 278-character description (limit 300).
8. `just visual`: passed twice in a row (19:35:20Z and 19:35:21Z).
9. Suite A/A: pass, below.
10. Calibrated defaults: in `docs/perf/benchmarks.md`, and under P1–P5 here.

Suite A/A. `just perf suite --repeat 5` wrote `perf-runs/20260930T193606Z-suite` (19:35:32–19:41:54Z, build included) and `perf-runs/20260930T194202Z-suite` (19:42:02–19:47:48Z, same binary), with no session during either. All 16 captures are valid, and every row reproduces its recorded digest. `just perf compare` of the two (`perf-runs/20260930T194756Z-compare-suite`) gives 0 regressed, 0 improved, 45 unchanged and 10 noisy of the 55 owned pairs, against 25 unchanged and 30 noisy at `--repeat 3` in P5. No workload drift.

| Row | Owned verdicts | `total` p95, suite 1 → 2 (ms) | Jitter (ms) | Peak RSS (MiB) | Peak RSS verdict |
| --- | --- | --- | --- | --- | --- |
| `physics` | 3 unchanged | 14.22 → 13.94 | 0.93 → 0.66 | 132.1 → 132.3 | unchanged |
| `physics-sparse` | 2 unchanged | 10.24 → 10.08 | 2.08 → 1.88 | 127.8 → 127.9 | unchanged |
| `ecs` | 14 unchanged, 2 noisy | 11.54 → 11.52 | 0.71 → 0.71 | 163.3 → 163.1 | unchanged |
| `churn` | 5 unchanged, 1 noisy | 10.33 → 10.41 | 1.06 → 1.30 | 143.6 → 143.6 | unchanged |
| `gpu` | 11 unchanged, 5 noisy | 12.53 → 12.66 | 2.62 → 2.66 | 731.7 → 732.6 | unchanged |
| `gpu-throughput` | 2 unchanged, 2 noisy | 27.61 → 26.29 | 1.08 → 0.75 | 252.7 → 252.7 | unchanged |
| `particles` | 4 unchanged | 11.57 → 11.73 | 1.04 → 1.07 | 142.5 → 141.5 | unchanged |
| `integrated` | 4 unchanged | 12.47 → 12.53 | 0.64 → 0.77 | 281.8 → 281.5 | unchanged |

The columns after the verdicts are suite README medians. The noisy pairs, as Δ [95% interval] against τ, in ms:
- `ecs`: `update` p95 +0.05 [−0.51, +0.60] against 0.56; `stats_decay` p50 +0.014 [−0.026, +0.054] on the 0.02 floor.
- `churn`: `flush` p50 −0.18 [−0.64, +0.29] against 0.25.
- `gpu`: the scene pass p50 +0.18 [−0.40, +0.76] against 0.22 and `render_span` p50 +0.18 [−0.40, +0.75] against 0.33, P4's between-capture GPU drift; `extract` p50 −0.04 [−0.08, +0.00] and p95 −0.09 [−0.19, +0.01]; `render_encode` p95 +0.02 [−0.15, +0.20].
- `gpu-throughput`: `render_encode` p50 −0.28 [−0.57, +0.00] and p95 −0.75 [−1.56, +0.06]. Three of suite 1's five runs had a `render_encode` p95 of 2.55–2.83 ms, against 1.41–1.86 ms in the other seven runs.
- `integrated`'s `total` statistics, `noisy` at n = 3 in P5, read `unchanged` at n = 5.

RSS growth over both suites, reported only, in KiB/s: `physics` 2.0–7.9; `physics-sparse`, `ecs` and `churn` 0.0 in every run; `particles` 4.2–10.8; `gpu` about 140,000 and `integrated` about 19,600 (the text cache); `gpu-throughput` −1,795 to +732, falling in 9 of 10 runs.

Capacity. No probe failed a guard or a child. p95, p99, the limiting stage and peak RSS are medians over the confirmation probes; for an unreachable budget they come from the last probe.
- Excluded: `perf-runs/20260930T194831Z-capacity-60hz`. A NoMachine session from 19:48:58Z to 19:49:06Z overlapped `physics`' bisection, whose probes at 1.297 and 1.242 read 24.01 and 23.22 ms. The whole search was redone.
- The redo, `perf-runs/20260930T195439Z-capacity-60hz`, met a second session from 19:57:33Z to 19:57:38Z. By their log timestamps only `churn`'s confirmation probes at 1.4065 overlapped it (18.92 and 20.53 ms); the other seven rows ran before or after it and stand. `churn` was redone alone in `perf-runs/20260930T200109Z-capacity-60hz`, with no session.
- 144hz: `perf-runs/20260930T200200Z-capacity-144hz`, with no session.

| Row | Budget | Max scale | Key counts | p95 / p99 (ms) | Limiting stage | Peak RSS | Probes | Wall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `physics` | 60hz | 1.139 | balls 9,110 | 16.06 / 16.42 | `system.physics_step` ✓ 15.47 ms | 134.7 MiB | 9 | 48 s |
| `physics-sparse` | 60hz | 1.542 | bodies 12,338 | 16.24 / 16.40 | `system.physics_step` ✓ 13.67 ms | 130.3 MiB | 9 | 40 s |
| `ecs` | 60hz | 1.414 | entities 353,553 | 15.96 / 16.20 | `system.heading` ✓ 3.59 ms (declared `update`) | 179.1 MiB | 9 | 47 s |
| `churn` | 60hz | 1.407 | population 175,813 | 16.03 / 16.75 | `stage.flush` ✓ 12.81 ms | 155.9 MiB | 12 | 46 s |
| `gpu` | 60hz | 1.542 | sprites 3,084 | 16.38 / 21.46 | `present` ✓ 7.61 ms | 749.2 MiB | 9 | 35 s |
| `gpu-throughput` | 60hz | 0.569 | sprites 227,758 | 16.17 / 16.47 | `stage.extract` ✓ 14.78 ms | 211.4 MiB | 9 | 42 s |
| `particles` | 60hz | 1.242 | emitters 348, animated 9,935 | 16.45 / 16.78 | `stage.extract` ✗ 9.85 ms (declared `stage.unattributed`) | 142.9 MiB | 9 | 48 s |
| `integrated` | 60hz | 1.297 | actors 3,242, crates 2,594, props 7,781, torches 389, pickups 1,297 | 16.07 / 16.38 | `system.physics_step` ✓ 9.27 ms | 251.7 MiB | 9 | 46 s |
| `physics` | 144hz | 0.545 | balls 4,362 | 6.75 / 6.82 | `system.physics_step` ✓ 6.40 ms | 128.0 MiB | 9 | 22 s |
| `physics-sparse` | 144hz | 0.677 | bodies 5,417 | 6.62 / 6.70 | `system.physics_step` ✓ 5.42 ms | 126.7 MiB | 9 | 17 s |
| `ecs` | 144hz | 0.542 | entities 135,570 | 6.68 / 6.91 | `system.heading` ✓ 1.40 ms (declared `update`) | 145.4 MiB | 12 | 28 s |
| `churn` | 144hz | 0.677 | population 84,641 | 6.59 / 6.97 | `stage.flush` ✓ 5.12 ms | 138.2 MiB | 9 | 15 s |
| `gpu` | 144hz | < 0.0005 (budget not reachable) | sprites 1 | 8.69 / 8.99 | `present` ✓ 6.42 ms | 169.2 MiB | 12 | 26 s |
| `gpu-throughput` | 144hz | 0.310 | sprites 124,186 | 6.78 / 7.03 | `stage.extract` ✓ 5.53 ms | 176.8 MiB | 10 | 26 s |
| `particles` | 144hz | 0.648 | emitters 182, animated 5,187 | 6.85 / 7.21 | `stage.extract` ✗ 3.97 ms (declared `stage.unattributed`) | 137.4 MiB | 9 | 20 s |
| `integrated` | 144hz | 0.092 | actors 229, crates 184, props 551, torches 28, pickups 92 | 6.89 / 7.10 | `present` ✗ 3.14 ms (declared `system.physics_step`) | 209.6 MiB | 15 | 31 s |

- `physics` at 60hz lands one bisection step below P5 (1.139 against 1.189; 9,110 against 9,514 balls): its bisect probe at 1.189 read 16.83 ms, just over the budget, where P5's confirmations had a 16.48 ms median. One 180-frame probe decides that boundary, and capacity stays informational.
- Step-downs: `churn` at 60hz (1.477 passed its bisect probe and failed all three confirmations, median 17.02 ms), `ecs` at 144hz (0.569 read 6.95–6.99 ms) and `integrated` at 144hz (0.0964 failed two of three). `integrated` at 60hz lost one of three confirmations (17.37 ms), and the median held.
- As in P4 and P5, `gpu` can't reach 144hz (8.69 ms at the scale floor, from the fixed post chain), `particles` is `extract`-limited at both budgets, and `integrated` is present-bound at 144hz.
- The 60hz redo took 6 min 04 s and the 144hz search 3 min 04 s.

Stays open:
- **RSS growth threshold.** P3's proposal for `churn` is listed under "Open proposals" in `docs/perf/benchmarks.md`. Growth is reported for every row and judged for none; nothing implements a verdict.
- **Gaps** G1–G4, T1, T2 and M1 stay unapproved and are listed in the same section; `particles` keeps owning `unattributed`.
- **Engine findings,** recorded in `docs/perf/benchmarks.md`: the burst latch, tiles at `z_norm` 0 under `gpu_depth`, the per-sprite lit-plus-material warning and text-cache RSS, plus the default extract's cost, the per-frame tile-proxy scan, the missing bundle insert and string sprite IDs.
- **Calibration findings:** `particles` fails its 50% share check by construction (accepted), and at 144hz `integrated` is present-bound and `gpu` can't reach the budget.
- **Follow-up, not P6:** the runner could record `nxcodec.bin` sightings in its provenance. Two short sessions hit this phase's capacity runs, and only the wrapper's polling caught them.
- **Python floor.** `README.md` and `docs/agent-setup.md` still say Python 3.9+, but `bench.py` and `bench_report.py`, which `just script-test` exercises, need 3.11 (`datetime.UTC`, PEP 604 annotations at runtime) and target 3.12. `docs/agent-setup.md` is outside P6's files, so both are left for the owner.
- **Out of scope, unchanged:** the `crates/*/benches/*` comments, `docs/plans/phase4.md` and `docs/repo-review-2026-09-25.md` still name the retired scenes; none of them matches done-when #5's patterns.
