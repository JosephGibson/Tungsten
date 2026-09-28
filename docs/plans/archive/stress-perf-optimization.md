---
status: done
goal: "Cut measured CPU/GPU frame cost in the three stress scenes (ecs-high-load, physics-stress, sprite-stress) by attacking the profiled hotspots in ranked order, after closing the capture-procedure gaps that currently hide or blur them."
non-goals:
  - "No threads or job system: physics stays serial (D-067); only the cpal and notify threads exist (AGENTS.md hard rules)."
  - "No new runtime dependency. Every hasher, bench or query shape named here is in-repo (D-015)."
  - "No shipped pacing change: tungsten.json present-mode and max-frame-latency defaults stay (profiling-workflow frame pacing policy)."
  - "No GPU-side optimization (culling, compute, render bundles, shader work) until step T5 produces per-pass GPU evidence; no current capture shows a GPU-bound canonical scene."
  - "No unlabeled stress workload changes: R9 establishes ECS v2; T7 adds opt-in density sweeps; T5 adds its own render-features baseline."
  - "Execution started 2026-09-27 (T0 from the prior session; all steps remain uncommitted)."
files to touch:
  - "scripts/perf-capture.sh, scripts/test-perf-capture.sh (T0 done; T1-T4, T6)"
  - "docs/perf/profiling-workflow.md, .claude/skills/tungsten-perf/SKILL.md (T0 done; T1-T7 docs)"
  - "crates/tungsten/src/app.rs, crates/tungsten/src/tests/app.rs (T0 done; T3 physics line)"
  - "crates/tungsten-core/src/physics/broadphase.rs (R4)"
  - "crates/tungsten-core/src/physics/step.rs (R2, R3, R5, R6)"
  - "crates/tungsten-core/src/tests/physics/step.rs (R5 contact equivalence, wake/floor and budget-trip regressions)"
  - "crates/tungsten-core/src/ecs/world.rs, storage.rs, archetype.rs (R3 query shape, R8 hasher)"
  - "crates/tungsten-render/src/sprite.rs (R1)"
  - "crates/tungsten-render/src/{timing.rs,renderer.rs,passes/recorder.rs,post/mod.rs,post/bloom.rs} (T5)"
  - "examples/02_sprite_stress/src/render_features.rs, scripts/smoke-examples.sh, scripts/test-smoke-examples.sh (T5)"
  - "crates/tungsten/src/sprite_extract.rs (R7)"
  - "crates/tungsten-core/benches/physics_bench.rs, ecs_bench.rs (new benches for R2-R6, R8)"
  - "examples/02_sprite_stress/src/{main.rs,ecs_high_load.rs,physics_stress.rs} (T3 knob, R4a, R9)"
  - "DECISIONS.md + docs/DECISION_INDEX.md (R4 if the SpatialGrid contract changes; R5 always)"
ordered steps:
  - "T1 Record git commit, dirty flag, CPU governor and platform profile in each per-run README."
  - "T2 Add --repeat N to perf-capture.sh with a summary.md of per-metric medians (avg, p95) across runs."
  - "T3 Make physics-stress measure the awake solve: add a physics: perf line, awake-phase README stats, a sleep-off scene knob and an awake 3k bench."
  - "T4 Track sprite-stress --stress-count 100000 as the render-throughput row; add encode/submit/gpu percentiles."
  - "R1 [score 20] Sprite instance upload: replace the per-element flatten with slice copies via Queue::write_buffer_with (sprite.rs:846-857; 21.1% of samples at 100k sprites; sprite-stress)."
  - "R4 [score 11] SpatialGrid query path: point insertion for steering, visitor query, fewer per-entry loads (broadphase.rs:104-143; SpatialGrid::query 63.8% self in ecs-high-load, 26.9% in physics-stress)."
  - "R2 [score 10] Cache per-substep travel AABBs instead of recomputing per candidate (step.rs:884-895; Proxy::travel_aabb 10.7% + Aabb::overlaps 6.9%; physics-stress)."
  - "R5 [score 7.5] Reuse the pair list across substeps (step.rs:534, 878-897; per-substep pair finding is 46.7% of physics-stress samples; D-067 lever 1)."
  - "R6 [score 6] Carry warm-start impulses across substeps; probe ImpulseMap once per frame (step.rs:962, 1036-1041; ImpulseMap get/insert/reset/resize 17.9%; physics-stress; after R5)."
  - "R3 [score 1.35] Columnar integrate_loose_bodies (step.rs:741-758; physics_step system 2.02 ms in ecs-high-load, SipHash TypeId lookups; ecs-high-load)."
  - "R7 [score 0.85] Default sprite extract without per-sprite hashing (sprite_extract.rs:59-130; 2.8% of physics-stress; linear in sprite count)."
  - "R8 [score 0.8] No-op hasher for Archetype::columns TypeId keys (archetype.rs:80, storage.rs:241-257; RandomState::hash_one<&TypeId> 0.86% + Sip13 0.36% in ecs-high-load)."
  - "R9 [unscored, workload change] Example steering and tint math (ecs_high_load.rs:283-340; steer self 26.9%, sqrt 8.3%, tint 0.99 ms); only with an ecs-high-load re-baseline."
  - "T5-T7 (lower priority) Per-pass GPU timestamps plus a render-features scene; cheaper perf-record settings; density-preserving ecs-high-load sweeps."
done-when:
  - "just repo-check passes with this plan; each executed step has before/after medians from `--repeat 3` captures (T2) on every scene it cites, plus the named criterion bench, recorded in the step's section."
  - "A step that fails to improve its evidence metric is reverted and its numbers are recorded here (D-067 precedent); it doesn't block the others."
  - "ecs-high-load: update median <= 50 ms after R4 + R3 (baseline 73.11 ms; steer_agents_system 69.53 ms)."
  - "physics-stress: awake-phase physics_step mean <= 10 ms after R2 + R5 + R6 (baseline 14.10 ms), and whole-window p95 total <= 15 ms (baseline 16.54 ms)."
  - "sprite-stress --stress-count 100000: render_encode avg <= 0.6 ms after R1 (baseline 1.65-1.94 ms); extract stays within 10%; original GPU-unchanged criterion failed and is replaced by the measured end-to-end tradeoff acceptance below."
  - "R5: QA-fixed pre/post just check and release physics_determinism pass; three new contact-equivalence/wake-floor/budget-trip tests pass; dense_pile_awake/3000 improves and every pile_plus_bullet/projectile_stream size stays within the 10% regression gate; D-075 and index are recorded. Closed 2026-09-28."
  - "just check, just script-test, just smoke green; tests/physics_determinism.rs, physics_tunneling.rs and physics_containment.rs green for R2-R6; the visual regression test green for R1 and R7."
---

## Context digest

Tungsten is a hand-rolled-ECS + wgpu 2D engine. Every stress scene runs `example-02-sprite-stress`, captured by `scripts/perf-capture.sh`: release build, Vulkan, 1920x1080, `Immediate`, max frame latency 1, 60 warm-up + 300 measured frames. Smoke mode pins dt to 1/60, so the simulation is deterministic.

Execution is measured against the **Execution baseline** below (2026-09-27, T1–T4 in place). The **Reference numbers** that motivated the ranking came from an uncommitted tree; keep them as context, not as comparison points.

Constraints: `D-005`, `D-015`, `D-038`, `D-042`, `D-062`–`D-067` (threading rejected).

## Execution status (2026-09-28, remaining-work continuation)

Commits: the user requested no staging or commits. Each continuation step exists as an ordered patch plus proposed commit message in `/home/joker/Projects/Tungsten-handoff-remaining-20260928/`; the index is unchanged. The incoming uncommitted work is the patch baseline, not HEAD.

| Step | Outcome | Headline (baseline → now, `--repeat 3` medians) |
| --- | --- | --- |
| T0 | landed (prior session) | per-system telemetry |
| T1 | landed | provenance rows plus dirty-tree fingerprint |
| T2 | landed | `--repeat N`, `summary.md` |
| T3 | landed | `physics:` line, awake-phase stats, `--physics-sleep off`, `dense_pile_awake/3000` |
| T4 | landed | encode/submit/GPU p95, Tracked Rows |
| R1 | landed; GPU tradeoff accepted | 100k encode 0.91 → 0.31 ms; measured GPU +0.19 ms; total flat and p95 improves in fresh controls |
| R4a | landed | ecs update 65.58 → 35.28 ms |
| R4b | landed | steer 31.73 → 27.51 ms; physics awake 9.56 → 8.35 ms |
| R4c | (a) landed; (b)/(c) measured and dropped | steer 27.51 → 24.65 ms |
| R2 | landed | physics awake 8.44 → 7.82 ms, bit-identical |
| R3 | landed | ecs `physics_step` 2.02 → 0.06 ms |
| R7 | landed | physics-stress extract 0.27 → 0.09 ms, byte-identical frames |
| R8 | landed | `spawn_insert_3_components_10k` −50%, `position_integration_50k` −12%; scenes flat |
| R5 | **landed** | awake physics 7.50 → 2.68 ms; `dense_pile_awake/3000` −69.6%; all fast-body bench point estimates within +4.7% |
| R6 | landed | awake bench −8.7%; matched physics 2.76 → 2.51 ms; fast-body gates pass |
| T6 | landed | FP records 52× smaller at comparable sample count; optional sampling frequency |
| R9 | landed, scene workload v2 | tint 0.97 → 0.18 ms; new baseline below |
| T7 | landed | density-preserving sweeps plus representative 50k iteration baselines |
| T5 | landed | 19 render-feature passes measured; timed/untimed pixels match |

Through R8: ecs-high-load total 66.51 → **26.79** ms (update 65.58 → 26.06). physics-stress awake `physics_step` 9.57 → **7.68** ms (whole-window total 7.85 / 10.45 → 6.30 / 8.38). sprite 100k encode 0.91 → **0.31–0.33** ms.

Done-when checks:
- **Pass:** `just repo-check`; before/after medians and benches recorded for every executed step; the reverted-step rule (R4c (b)/(c) prototypes reverted with evidence).
- **Pass:** ecs-high-load update ≤ 50 ms after R4 + R3 (26.06).
- **Pass:** R2/R5/R6 are measured and landed; awake physics 2.51 ms and total p95 3.65 ms satisfy the numerical thresholds. Those thresholds already passed at the execution baseline, so the independent R5 and R6 benchmark gates below establish the actual benefit.
- **Resolved tradeoff:** sprite 100k `render_encode` ≤ 0.6 ms after R1 (0.31–0.33) and extract unchanged (2.07) pass. "GPU unchanged" fails as measured (2.78 → 2.95–3.02). Historical identical-command-stream controls support a pacing/submission explanation. Fresh controls below establish no end-to-end regression; they do not prove a unique driver-level cause.
- **Pass:** final continuation `just check`, strict clippy, all 29 release physics-step tests (including R5/R6), release determinism/tunneling/containment, full script suite, repository checks, all 16 Vulkan smoke rows and reference visual regression. Historical validation: QA-fixed pre-R5 and post-R5 `just check` and release determinism/tunneling/containment; script suite; all 15 Vulkan smoke rows; visual regression. R5 adds three passing regression tests and passes all named benchmark gates, including every configured fast-body size. Ordered patch replay is verified and the real index is unchanged.

Remaining: none in the requested continuation scope. R4c(b)/(c) are explicitly rejected, and the R1 GPU-unchanged criterion is explicitly replaced by the measured tradeoff acceptance. Unranked and alternative solver ideas below remain outside this task.

## Close-out (2026-09-28)

All seven requested continuation items are resolved. Final `just check`, `just script-test`, `just repo-check`, release physics integration and all 29 physics-step tests pass. Vulkan smoke passes 16 rows and the reference visual regression passes; timed/untimed render-feature captures match pixel-for-pixel. The complete ordered eight-patch handoff starts from the preserved incoming working tree, replays without staging, and reproduces the final source. No code or assets were removed from the incoming changes. Plan archived after completion per `docs/plans/README.md`.

Handoff: `/home/joker/Projects/Tungsten-handoff-remaining-20260928/README.md`; full raw evidence remains alongside it and in the named `perf-runs/` directories. Limitations: measurements cover this Vulkan/RADV machine; other backends and unsupported timestamp adapters are not hardware-tested. GPU diagnostics intentionally block and are not normal frame-time claims. No remaining acceptance criterion is silently marked passed.

## Continuation baseline (2026-09-28)

Incoming branch `0.30`, HEAD `1f9c183`, dirty fingerprint `bde9c02d673a`; all prior uncommitted work preserved. Fresh `just check` and release determinism/tunneling/containment pass. Same reference hardware, performance governor and canonical flags/window. Three-repeat medians (avg / p95, ms):

| Scene | Primary metric | Total | Evidence under `perf-runs/` |
| --- | --- | --- | --- |
| physics-stress | awake physics 2.67 / 3.19 | 2.93 / 3.52 | `20260928T135400Z-physics-stress` (full, 300 awake frames per run) |
| ecs-high-load | update 25.74 / 27.09; steering 24.17 / 25.64 | 26.40 / 27.80 | `20260928T135508Z-ecs-high-load` |
| sprite 100k | encode 0.29 / 0.39; GPU 3.10 / 3.95 | 3.98 / 5.28 | `20260928T135611Z-sprite-stress-count100000` |

Incoming source snapshot, binaries, validation logs, benchmark logs and incremental patches live at `/home/joker/Projects/Tungsten-handoff-remaining-20260928/`. No staging or commits. Execution order: R6; T6; R4c(c); R1 matched pacing controls; R9; T7; T5; final validation and ordered patch replay. Workload edits remain separate from engine claims.

## Execution baseline (2026-09-27, T1–T4 in place)

Machine: Ryzen 5 6600H + Radeon 660M (RADV), rustc 1.98.1, `-C force-frame-pointers=yes`, Vulkan `immediate`, latency 1.

Provenance: commit `1f9c183`, dirty tree `1f9c51959201` (the T0–T4 working tree), governor `performance`, platform profile n/a, load average about 0.7.

Values are `summary.md` medians of `--repeat 3 --telemetry-only`, in ms (avg / p95). Every later step's before/after is measured against these rows.

| Scene | total | update | extract | render_encode | GPU | Key rows |
| --- | --- | --- | --- | --- | --- | --- |
| ecs-high-load (50k) | 66.51 / 70.33 | 65.58 / 69.46 | 0.36 / 0.45 | 0.32 / 0.40 | 1.03 / 1.18 | `steer_agents_system` 62.03 / 66.03, `physics_step` 2.02 / 2.04, `tint` 0.97, `orient` 0.44 |
| physics-stress (3k) | 7.85 / 10.45 | 7.27 / 9.80 | 0.27 / 0.29 | 0.07 / 0.12 | 0.18 / 0.30 | awake (225 of 300 frames, every run): `physics_step` 9.57 / 9.82, total 10.09 / 10.49 |
| sprite-stress --stress-count 100000 | 4.17 / 6.14 | 0.43 / 0.55 | 2.08 / 2.18 | 0.91 / 1.35 | 2.79 / 3.59 | render 1.65, acquire 0.53 / 2.38 |

Captures (under `perf-runs/`):

| Capture | Kind |
| --- | --- |
| `20260927T214402Z-ecs-high-load` | repeat 3, telemetry-only |
| `20260927T214633Z-physics-stress` | repeat 3, telemetry-only |
| `20260927T214704Z-sprite-stress-count100000` | repeat 3, telemetry-only |
| `20260927T214724Z-ecs-high-load` | full |
| `20260927T215003Z-physics-stress` | full |
| `20260927T215104Z-sprite-stress-count100000` | full |

Each full capture matches its telemetry medians within 0.1 ms. Its `perf-report-self.txt` and `perf-report-children.txt` sit next to the recording.

Full-capture profiles (percent of all samples):

- **ecs-high-load (self):** `SpatialGrid::query` 62.02%, `steer_agents_system` 28.18%, `physics_step` 1.06%, `hash_one<&TypeId>` 0.92%, `tint_agents_system` 0.67%, `get_mut<Velocity>` 0.50%, `Sip13Rounds::write` 0.42%. Same shape as the reference.
- **physics-stress (inclusive):**
  - `physics_step` 93.0% (self 58.3%, `substep` inlined) and `SpatialGrid::query` 32.9% (self 32.8%).
  - `travel_aabb` 14.5%, `overlaps` 9.1%, `union` 7.1%, `narrow_phase` 5.3%, `solve_contacts` 5.1%, `circle_vs_circle_speculative` 4.9%.
  - `ImpulseMap::get` 3.5%, `ImpulseMap::insert` 1.3%, pair `push` 2.9%, `extract_sprites_default` 3.3%.
  - **Shift from the reference:** pair finding (query + travel_aabb + overlaps + push) is now about 59%, not 46.7%. The impulse map is about 5%, not 17.9%, which weakens R6's case.
- **sprite 100k (inclusive):**
  - Example extract `extract_baseline_sprites` 57.0%, with `rgb_wheel_color` at 48.5% and `sin` at 27.8%.
  - `SpritePipeline::draw` 21.7% (self 14.7%), of which the `extend_desugared<FlatMap>` flatten is 14.7%.

Benches (criterion, `target-cpu=native` from `.cargo/config.toml`): `physics_step/dense_pile_awake/3000` 9.19 ms, `physics_step/dense_pile/3000` 187 µs.

**Against the reference below:**
- ecs-high-load update is 65.58 ms, not 73.11. physics-stress awake `physics_step` is 9.57 ms, not 14.10. The 100k encode is 0.91 ms, not 1.65–1.94.
- The reference came from an uncommitted tree with no commit, governor or profile recorded, so the drift can't be attributed. It matches the 2026-09-25 `cpu-levels` numbers (64.5 ms, about 9.6 ms awake) far better than the reference does.
- **Done-when consequence:** the physics-stress thresholds (awake ≤ 10 ms, p95 total ≤ 15 ms) already pass at this baseline, so they no longer measure R2/R5/R6. The same 29% cut from this baseline would be about 6.8 ms awake. The ecs-high-load (≤ 50 ms) and sprite 100k encode (≤ 0.6 ms) thresholds still discriminate.

## Reference numbers (pre-execution tree; reference only)

Taken 2026-09-27 before T1 existed, on an uncommitted tree. Ryzen 5 6600H + Radeon 660M (RADV), rustc 1.98.1, `-C force-frame-pointers=yes`, median of 3 runs:

| Scene | total avg / p95 | update | extract | render | GPU (scene pass) | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| ecs-high-load (50k agents) | 74.06 / 83.29 ms | 73.11 | 0.34 | 0.60 | 1.08 | CPU-bound, 4.4x over budget |
| physics-stress (3k bodies) | 11.92 / 16.54 ms | 10.75 | 0.34 | 0.81 | 0.17 | CPU-bound, p95 at the 16.7 ms budget |
| sprite-stress (2k sprites) | 1.38 / 3.48 ms | 0.01 | 0.05 | 1.31 | 0.14 | Present-bound (acquire 0.98 ms) |
| sprite-stress --stress-count 100000 | 5.06 / 7.70 ms | 0.03 | 2.41 | 2.07 | 2.78 | extract and encode scale linearly |

- **ecs-high-load:** one example system, `steer_agents_system`, takes 69.53 of the 73.11 ms update. Inside it, the engine's `SpatialGrid::query` is 63.75% of all samples (self time).
- **physics-stress:** per-substep pair finding plus the warm-start map dominate. The contact solve itself is under 4% (`D-067` saw the same at 25k).
- **Rendering:** it only shows up at scale. At 100k sprites the instance flatten in `SpritePipeline::draw` takes 21% of the process.

## Evidence base

Capture directories live under `perf-runs/`, which is gitignored and exists only on this machine. The numbers are copied here so the evidence outlives them.

| Capture (perf-runs/…) | Kind | Used for |
| --- | --- | --- |
| `20260927T060621Z-ecs-high-load` | full (flamegraph, perf record 859 MB, perf stat) | ecs hotspots |
| `20260927T060936Z-physics-stress` | full | physics hotspots |
| `20260927T060911Z-sprite-stress` | full | canonical render/present |
| `20260927T061925Z-sprite-stress-count100000` | full | render path at scale |
| `20260927T0615–0619Z` × 9 | telemetry-only, 3 per scene, with `systems:` lines | baseline medians, per-system ms |
| `…-sprite-stress-count{20000,100000,400000}`, `…-ecs-high-load-count{12500,25000}` | telemetry-only sweeps | scaling |
| `20260927T0620–0624Z` × 6 | telemetry-only, `TUNGSTEN_PERF_RUSTFLAGS="-C target-cpu=x86-64"` | frame-pointer cost check |

Flamegraph percentages are shares of all process samples in that capture (perf record `--call-graph dwarf`, user space, `perf_event_paranoid=2`). "Self" means `perf report --no-children`.

**ecs-high-load, per-system mean ms (median run):** `steer_agents_system` 69.53 (p95 78.95), `physics_step` 2.02, `tint_agents_system` 0.99, `orient_agents_system` 0.46, `confine_agents_system` 0.05, `sync_position_to_transform` 0.05, camera and telemetry systems ~0.

**ecs-high-load, self time:**

| Symbol | Self |
| --- | --- |
| `SpatialGrid::query` | 63.75% |
| `steer_agents_system` | 26.87% |
| `physics_step` | 0.99% |
| `RandomState::hash_one<&TypeId>` | 0.86% |
| `tint_agents_system` | 0.62% |
| `atan2f` | 0.50% |
| `Archetypes::get_mut<Velocity>` | 0.49% |
| `floorf` | 0.45% |
| `extract_high_load_sprites` | 0.39% |
| `Sip13Rounds::write` | 0.36% |

**ecs-high-load, inclusive inlined frames:**

| Frame | Inclusive |
| --- | --- |
| `IVec2::eq` (exact-cell filter, broadphase.rs:127) | 11.08% |
| bucket `Range::next` | 9.19% |
| `f32::sqrt` (ecs_high_load.rs:305) | 8.34% |
| `Vec<u32>::push` (broadphase.rs:139) | 6.86% |
| `integrate_loose_bodies` | 2.66% |

perf stat: IPC 1.4, L1d miss rate 4.5%, branch miss rate 4.0%.

**ecs-high-load, density scaling:** `steer_agents_system` takes 7.85, 22.71 and 69.53 ms at 12.5k, 25k and 50k agents, about 2.9x per doubling. The world size is fixed, so neighbors per agent grow with the count. `physics_step` scales linearly: 0.59, 1.09, 2.02 ms.

**physics-stress, inclusive frames:**

| Frame | Inclusive |
| --- | --- |
| `physics_step` | 90.74% |
| `substep` | 84.76% |
| `SpatialGrid::query` (self 26.90%, all from the substep pair loop) | 27.04% |
| `ImpulseMap::get` | 14.07% |
| `Proxy::travel_aabb` | 10.65% |
| `Aabb::overlaps` | 6.93% |
| `Aabb::union` | 5.61% |
| `solve_contacts` | 3.88% |
| `narrow_phase` | 3.70% |
| `circle_vs_circle_speculative` | 3.17% |
| `extract_sprites_default` | 2.80% |
| `Vec<(u32,u32)>::push` | 2.24% |
| `sleep_frame_start` | 2.24% |
| `ImpulseMap::insert` | 1.52% |
| `ImpulseMap::reset` | 1.28% |
| `Vec<u128>::resize` | 1.03% |

**physics-stress, sleep onset:** the pile falls asleep at frame 287 in every run (deterministic dt). `physics_step` drops from about 14.9 to 0.2–0.4 ms. So 74 of the 300 measured frames are asleep, and the whole-window average (10.75 ms) blends two regimes. Over the 226 awake frames, `physics_step` means 13.54, 14.10 and 14.23 ms (p95 about 15.7 ms). Historical captures show the same step: `20260925T191112Z`, `20260925T190849Z`, `20260713T184012Z`.

**sprite-stress, canonical 2k:** acquire is 0.98 of 1.38 ms. Total p95 swings between 3.19 and 8.54 ms across identical runs, and the swing is all acquire. The CPU profile is mostly driver and wgpu bookkeeping: `Queue::maintain` 11.1%, `EncoderInFlight` drop 10.8%, `InnerCommandEncoder` drop 8.9%.

**sprite-stress, 100k:** the example extract (`rgb_wheel_color`) is 51.8%, with `sin` alone at 35.6%. `SpritePipeline::draw` is 21.5%, of which the `Vec::extend` over `FlatMap<Copied>` is 21.08%. `Queue::write_buffer` itself is 1.23%.

**sprite-stress sweep:**

| Sprites | extract | encode | GPU | total |
| --- | --- | --- | --- | --- |
| 20k | 0.47 | 0.30 | 0.55 | 1.91 |
| 100k | 2.41 | 1.65 | 2.78 | 5.06 |
| 400k | 9.18 | 5.43 | 10.63 | 17.28 |

**Frame-pointer check:** the flag makes no measurable difference. `ecs-high-load` total avg is 74.06 ms with frame pointers vs 74.57 ms without; `physics-stress` is 11.92 vs 11.64 ms (medians of 3). The 2026-09-25 `cpu-levels` captures (ecs update 64.5 ms, physics awake about 9.6 ms, same no-frame-pointer flags) predate the squash-merge `523fd98` and record no commit, so that drift can't be attributed. T1 exists to prevent that.

## Ordered steps (ranked, with evidence)

Score = expected share of the affected scene's frame saved ÷ effort points (XS=1, S=2, M=3, L=4). The expected share uses the profile numbers above. Tooling steps come first because the R-steps' done-when checks depend on them.

### T0 — Per-system telemetry (landed this session)

**Gap:** update was 98% of ecs-high-load and 90% of physics-stress frames, yet the per-run README couldn't attribute it. `FrameTimings::system_timings` existed but was never logged, extract was a budget line with no reported percentile, and render had only an average.

**Change:**
- `app.rs` emits a `systems: name=ms …` debug line after each `frame:` line (`format_perf_systems_line`, unit-tested).
- `perf-capture.sh` gains tag-aware `metric_samples`, `system_names` and `system_timing_rows`. The README gains extract and render avg/p50/p95/p99 plus a per-system table.
- `test-perf-capture.sh` covers all of the above; the workflow doc and skill are updated.

**Verified:** `bash scripts/test-perf-capture.sh`, shellcheck, `cargo test -p tungsten --lib perf_systems`, `cargo clippy -p tungsten --all-targets -D warnings`, `cargo fmt --check`. The per-system tables quoted above come from this tooling.

### T1 — Capture provenance

**Evidence:** today's `ecs-high-load` (73.1 ms update) and `physics-stress` (about 14 ms awake) can't be matched against the 2026-09-25 captures (64.5 ms, about 9.6 ms awake), because READMEs record flags but no commit.

**Change:** add rows for `git rev-parse --short HEAD`, dirty yes/no, `/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor` and `/sys/firmware/acpi/platform_profile` (n/a when absent). Extend the parser test for the n/a path. Effort XS.

**Landed 2026-09-27.** `file_value_or_na`, `git_commit_label` and `git_dirty_label` in `perf-capture.sh`, read before the build. Dirty rows carry a 12-hex fingerprint of `git diff HEAD` plus untracked files, because every capture in this session runs on uncommitted work and a bare `yes` couldn't tell two trees apart. Tests cover the value, absent and empty paths and both git label shapes. This machine reports governor `performance` and platform profile `n/a`.

### T2 — Repeat runs and medians

**Evidence:** the workflow's comparison rule ("at least three sequential captures and the median of their p95 values") is manual today; `cpu-levels-20260925/run.sh` hand-rolled it. Run-to-run spread is large:

| Scene | p95 spread across 3 runs |
| --- | --- |
| sprite-stress | 3.19–8.54 ms |
| ecs-high-load | 82.22–86.75 ms |
| physics-stress | 16.40–16.78 ms |

**Change:** add `--repeat N` (default 1). It runs N captures into `…/run-K/` and writes `summary.md` with the median across runs of avg and p95 for `total`, `update`, `extract`, `render`, `render_acquire`, `render_encode` and each system. Keep the full-capture profilers on run 1 only. Effort S.

**Landed 2026-09-27.**
- `main` loops over run directories. Measured runs go back to back, and the profilers (`capture_profiles`) run afterwards into `run-1/`.
- Each run writes `README.md` (`write_run_readme`) and a `metrics.tsv` of `metric, avg, p95` (`write_run_metrics`). `summary_rows` and `write_summary` take medians across runs and show the per-run values next to them.
- `--repeat 1` keeps the flat single-run layout and also writes `metrics.tsv`.
- Tests cover odd and even medians, n/a skipping, the per-run rows, and a summary row where one run lacks the metric.
- An end-to-end `sprite-stress 30 --repeat 2 --telemetry-only` run produced the expected tree. It was deleted afterwards.

### T3 — An awake-phase physics metric

**Evidence:** the frame-287 sleep onset puts 74 of 300 measured frames at 0.2–0.4 ms. Any change that shifts settling time moves the whole-window average by about 0.45 ms per 10 frames, independent of solver cost. The existing criterion suite can't stand in either: `physics_step/dense_pile/3000` (190 µs, `cpu-levels-20260925`) is about 70x below the in-scene awake cost. The bench pre-settles for 120 steps with sleeping on (the default), so its iterations are dominated by the slept state, not the awake 3k solve the scene spends 226 frames in.

**Change:**
- Emit `physics: proxies=… sleeping=… pairs=… contacts=…` from `PhysicsBuffers` when the resource exists. `sleeping_count()` is already public; add read-only counts for pairs and contacts.
- Have the capture README report `physics_step` stats over frames with `sleeping < proxies`.
- Add a child-only `STRESS_PHYSICS_SLEEP=0` knob (`--physics-sleep off`) for solver-throughput rows.
- Add a sleep-disabled `physics_step/dense_pile_awake/3000` bench.

Effort S.

**Landed 2026-09-27.**
- `PhysicsBuffers` gains `proxy_count`, `dynamic_count`, `pair_count` and `contact_count`, covered by a core test.
- `app.rs` emits `physics: proxies=… dynamic=… sleeping=… pairs=… contacts=…` after the `systems:` line (`format_perf_physics_line`, unit-tested).
- **Deviation from the plan:** the awake filter is `sleeping < dynamic`, not `sleeping < proxies`. Proxies include the scene's 3 static walls, so `sleeping` never reaches `proxies` and every frame would count as awake.
- The README gains a Physics Awake Phase section: `physics_step`, `update`, `total`, pairs and contacts over awake frames. `metrics.tsv` and `summary.md` gain the `awake:*` rows and the awake frame count.
- `--physics-sleep on|off` works for physics-stress only. It injects a child-only `STRESS_PHYSICS_SLEEP` and adds a `sleepoff` suffix. The example parses the value (unit-tested) and sets `sleep_threshold = 0`.
- The `dense_pile_awake/3000` bench settles the dense pile with sleeping off.
- In a check run on this machine, the first all-asleep frame is 286 of 360, which leaves 225 awake measured frames (the transition frame ends asleep and is excluded). Awake `physics_step` was 9.56 ms, not the 14.10 ms reference; see the fresh baseline below.
- Gates: `just check`, `just script-test` and `just smoke` all green.

### T4 — Render-throughput row

**Evidence:** canonical sprite-stress is present-bound (acquire 0.98 of 1.38 ms, GPU 0.14 ms) and its p95 is acquire noise. Render-path costs only appear with `--stress-count 100000`: extract 2.41, encode 1.65, GPU 2.78 ms.

**Change:** add the 100k row to the workflow doc's tracked rows. Add p95 for `render_encode`, `render_submit_present` and `gpu` to the README. Render steps (R1, R7) are judged on encode, extract and GPU, not on total p95. Effort XS.

**Landed 2026-09-27.**
- The README gains p95 rows for `render_encode`, `render_submit_present` and `gpu`.
- `metrics.tsv` and `summary.md` gain `render_submit_present` and `gpu` rows. `gpu` comes from the run's GPU-timing log, passed as `write_run_metrics`' optional third argument.
- The workflow doc gains a Tracked Rows table. It lists the 100k render-throughput row and what each row is judged on. The skill's scene list names that row too.
- Tests cover the gpu and submit rows and the no-GPU-log case. `just script-test` green.

### R1 [score 20] — Sprite instance upload without the per-element flatten

**Scenes:** sprite-stress at scale; every scene marginally. Effort XS. Expected saving about 1.2 ms of about 5.8 ms at 100k (21%); about 0 at the canonical 2k.

**Evidence:**
- `crates/tungsten-render/src/sprite.rs:846-857` flattens all batches into `instance_upload` with `extend(batches.iter().flat_map(|b| b.instances.iter().copied()))`, then `queue.write_buffer` copies it again.
- In `20260927T061925Z-sprite-stress-count100000`, `Vec<SpriteInstance>::extend_desugared<FlatMap<…Copied…>>` is 21.08% of samples, an element-wise copy that does not become a `memcpy`. `Queue::write_buffer` itself is 1.23%.
- `render_encode` averages 1.65–1.94 ms at 100k.

**Change:**
- Per contiguous batch run, write `bytemuck::cast_slice(&batch.instances)` directly into the view from `queue.write_buffer_with(&instance_buffer, offset, size)` (wgpu 30.0.1).
- Drop `instance_upload`. Draw ranges are unchanged.
- `StagingBelt` is the alternative only if the per-call allocation shows up afterwards.

**Checks:** encode at 100k ≤ 0.6 ms; visual regression; smoke.

**Result 2026-09-27: landed.**

`draw` now copies each batch with `bytemuck::cast_slice(&batch.instances)` into one `queue.write_buffer_with` view, in batch order. `instance_upload` is gone. A rejected upload logs an error and skips the draw.

Row: sprite-stress --stress-count 100000, `--repeat 3` medians in ms (avg / p95). A = baseline code, B = R1. Runs are listed in the order taken, so A and B alternate:

| Run | Capture | total | extract | render_encode | render_acquire | GPU |
| --- | --- | --- | --- | --- | --- | --- |
| A1 baseline | `20260927T214704Z` | 4.17 / 6.14 | 2.08 / 2.18 | 0.91 / 1.35 | 0.53 / 2.38 | 2.79 / 3.59 |
| B1 R1 | `20260927T215456Z` | 4.16 / 6.02 | 2.07 / 2.28 | **0.31 / 0.47** | 1.15 / 3.09 | 2.95 / 3.90 |
| A2 baseline rerun | `20260927T215604Z` | 4.18 / 6.12 | 2.07 / 2.13 | 0.90 / 1.32 | 0.55 / 2.40 | 2.78 / 3.58 |
| B2 R1 rerun | `20260927T215707Z` | 4.37 / 7.23 | 2.07 / 2.25 | **0.33 / 0.59** | 1.29 / 4.13 | 3.02 / 3.96 |
| C probe (not landed) | `20260927T220007Z` | 4.35 / 7.08 | 2.09 / 2.25 | 0.33 / 0.54 | 1.25 / 3.88 | 3.01 / 3.93 |

- **Encode:** down about 65%, from 0.90–0.91 to 0.31–0.33 avg. The ≤ 0.6 ms target is met. Extract is unchanged.
- **GPU not unchanged as measured:** the scene pass reads about 0.2 ms higher (2.78–2.79 → 2.95–3.02) in both R1 runs. Acquire rises by the encode saving, so at 100k this machine is GPU/present-bound and the CPU saving becomes acquire wait.
- **Control:** the C probe used a per-batch `queue.write_buffer` with no flatten. Its GPU command stream is identical to the baseline's (wgpu-core 30.0.1 `write_buffer` and `write_buffer_with` share `StagingBuffer::new` → flush → `write_staging_buffer_impl`), and it shows the same GPU 3.01. So the shift follows from encode finishing earlier, not from the upload API or extra GPU work. A likely cause is CPU/GPU contention for shared memory and power on this APU.
- **Total** is flat to +0.2 ms (B1 4.16, B2 4.37 against 4.17/4.18). The evidence metric improved, so R1 stays under the D-067 rule.
- **Revisit** if a GPU-bound 100k frame matters more than CPU encode cost. `07-R1` is its own patch.
- **Gates:** `just check`, `WGPU_BACKEND=vulkan just smoke` (15 OK) and the visual regression test all green.

**R1 continuation result 2026-09-28: accept the measured tradeoff.** Fresh isolated pre-R1 upload and current-R1 binaries use identical R6 physics, example workload, compiler flags and capture windows. Three-repeat medians (avg / p95 ms):

| Pacing / implementation | Encode | Acquire | Scene GPU (separate diagnostic) | Total (GPU timer off) |
| --- | --- | --- | --- | --- |
| Immediate / latency 1, old flatten | 0.89 / 1.16 | 0.41 / 1.70 | 2.92 / 3.62 | 3.99 / 5.42 |
| Immediate / latency 1, R1 staging slices | 0.27 / 0.30 | 1.03 / 2.39 | 3.11 / 3.95 | 3.98 / 5.28 |
| Mailbox / latency 3, old flatten | 0.88 / 1.14 | 0.43 / 1.79 | 2.92 / 3.63 | 4.00 / 5.44 |
| Mailbox / latency 3, R1 staging slices | 0.27 / 0.30 | 1.02 / 2.39 | 3.11 / 3.96 | 3.98 / 5.28 |

Extract average **2.06 → 2.09 ms (+1.5%)**; p95 **2.11 → 2.28 ms (+8.1%)**. The extra acquire wait absorbs the CPU encode saving. GPU diagnostic mean rises **6.5%**, p95 about **9%**; total mean stays flat and p95 improves **2.6–2.9%**. Both pacing settings pass the 10% end-to-end regression gate. The original “GPU unchanged” criterion **fails**, explicitly; it is replaced by acceptance of this bounded diagnostic increase for the CPU encode reduction with no total-frame regression. Shipped pacing defaults stay unchanged.

An additional fresh control keeps `queue.write_buffer` and its GPU command stream, replacing only elementwise flattening with `extend_from_slice`: encode **0.83 / 1.08**, GPU **2.92 / 3.66**, total **4.04 / 5.55 ms**. It does not achieve R1's CPU speedup, so this control alone cannot establish the cause of R1's higher GPU timestamps. Local wgpu-core 30.0.1 queue code routes both upload APIs through staging-buffer copies; render draws/instance bytes are unchanged. Combined with the prior faster identical-command-stream experiment, evidence supports submission timing/pacing rather than additional rendering work, but does not isolate GPU clock or driver scheduling effects. No GPU optimization is justified by these measurements.

Evidence in handoff `capture-root/perf-runs/`: old/current immediate `20260928T143328Z` / `20260928T143343Z`, old/current mailbox-lat3 `20260928T143359Z` / `20260928T143414Z`, slice control `20260928T143737Z` (all sprite-stress-count100000 suffixes). `evidence/r1-build-control-forced.log` and `r1-build-slice.log` show recompilation of all four local crates. Earlier `invalid-cached-r1-*.log` captures reused an artifact despite the alternate manifest and are excluded; the controls were rebuilt after explicitly touching the isolated source tree. Saved binaries and SHA256 provenance preserve the valid comparisons.

### R4 [score 11] — SpatialGrid query path

**Scenes:** ecs-high-load (primary), physics-stress. Effort M. Expected saving 20–30 ms of 74 ms in ecs-high-load, 1–2 ms awake in physics-stress.

**Evidence:**
- `SpatialGrid::query` self time is 63.75% in ecs-high-load and 26.90% in physics-stress.
- Inside it: the exact-cell filter `entry_cells[i] != cell` (broadphase.rs:127) is 11.08% and 4.39%; bucket iteration is 9.19%; the output `Vec<u32>::push` (broadphase.rs:139) is 6.86% and 2.71%; dedupe marks (broadphase.rs:134) are also in this loop.
- Steering inserts 12×12 AABBs (ecs_high_load.rs:277-279), so a typical agent lands in about 1.9 cells. It then queries a 60×60 box (ecs_high_load.rs:289-295), so each query scans duplicate entries that the mark array has to remove.
- The count sweep above shows the query cost grows with neighbor density.

**Sub-steps**, each gated by a new criterion bench `spatial_grid_query_50k_dense` (50k points in 3200×1800, radius 24) and by the ecs-high-load per-system row:
- **R4a (example, XS):** insert agent centers as zero-extent AABBs and query with the true neighbor radius (24 px). Each id then sits in one cell, removing duplicates. Müller's dense spatial hash stores each particle once at its center and checks only the surrounding cells. The repulsion result is unchanged apart from summation order.
- **R4b (engine, S):** add a visitor `SpatialGrid::for_each_in(&Aabb, exclude, FnMut(ProxyId))` to drop the `out` Vec write and re-read. Keep `query` for existing callers.
- **R4c (engine, M, prototype):**
  - Skip dedupe marks when every staged entry occupies one cell (track this at `insert`).
  - Measure whether dropping the per-entry `entry_cells` load (Müller's "hash collision OK", with callers filtering by distance or AABB) beats keeping it. Physics already AABB-filters candidates (step.rs:895).
  - Keep only what the bench and captures show.

`D-062`'s drift-budget staging and determinism must hold. Candidate order may change, so pair order changes too. `physics_determinism.rs` compares runs, not golden values. Add a `DECISIONS.md` entry if the `SpatialGrid` public contract changes.

**Bench (added before any R4 code), `spatial_grid_query_50k_dense`:**
- Setup: 50k uniform points in 3200×1800 (`Pcg32` seed `0x0DE5E0B5`). Each iteration does a full clear, insert and build, then one query per agent plus the caller's 24 px distance test.
- `boxes` is the original example shape: 12 px inserts, 30 px query half-extent. `points` is the R4a shape: center inserts, 24 px query.
- Baseline: `boxes` 51.79 ms, `points` 21.91 ms.

**R4a result 2026-09-27: landed.**
- `steer_agents_system` inserts centers as zero-extent AABBs and queries the 24 px radius. Every agent within the radius sits in a cell the box touches, and the existing distance test filters exactly, so only summation order changes.
- The bench is engine-side and doesn't move; `points` vs `boxes` (21.91 vs 51.79 ms) is the R4a shape change in isolation.

| Row (`--repeat 3` median avg / p95, ms) | Before (baseline) | After R4a |
| --- | --- | --- |
| ecs-high-load total | 66.51 / 70.33 | **36.05 / 37.60** |
| ecs-high-load update | 65.58 / 69.46 | **35.28 / 36.90** |
| `steer_agents_system` | 62.03 / 66.03 | **31.73 / 33.46** |
| `physics_step` (ecs-high-load) | 2.02 / 2.04 | 2.02 / 2.04 |
| physics-stress awake `physics_step` (control; engine unchanged) | 9.57 / 9.82 | 9.56 / 9.84 |

- Captures: `20260927T220523Z-ecs-high-load` and `20260927T220720Z-physics-stress`.
- The ecs-high-load update target (≤ 50 ms) already passes after R4a alone.
- Gates: `just check` and `just smoke` (15 OK) green.

**R4b result 2026-09-27: landed.**
- `SpatialGrid::for_each_in(&Aabb, exclude, impl FnMut(ProxyId))` visits in `query`'s order, and `query` now wraps it. `steer_agents_system` and the physics substep pair loop use the visitor; `speculative_pass` keeps `query`.
- Additive API with unchanged `query` semantics, so no `DECISIONS.md` entry. A new test pins visitor order = `query` order.
- Pair order is unchanged, so physics output is bit-identical: 225 awake frames every run, and `physics_determinism.rs` passes.

| Metric | Before (after R4a) | After R4b |
| --- | --- | --- |
| ecs-high-load total | 36.05 / 37.60 | **31.83 / 33.51** |
| ecs-high-load update | 35.28 / 36.90 | **31.03 / 32.79** |
| `steer_agents_system` | 31.73 / 33.46 | **27.51 / 29.40** |
| physics-stress awake `physics_step` | 9.56 / 9.84 | **8.35 / 8.65** |
| physics-stress total (whole window) | 7.86 / 10.46 | **6.92 / 9.21** |
| bench `physics_step/dense_pile_awake/3000` | 9.19 ms | **8.52 ms** (−7.4%) |
| bench `spatial_grid_query_50k_dense/points` (`query`) | 21.91 ms | 20.66 ms |
| bench `…/points_visit` (visitor) | n/a | 25.53 ms |
| bench `…/boxes` | 51.79 ms | 52.82 ms |

- Rows are `--repeat 3` medians (avg / p95, ms); captures `20260927T221154Z-ecs-high-load` and `20260927T221338Z-physics-stress`.
- **Bench caveat:** the grid bench runs slower through the visitor (25.5 vs 20.7 ms) while both scene rows improve 13%. Its visitor closure captures `&mut neighbors`, which the scene closures don't. The in-scene rows are the gate, and the bench shape is recorded rather than tuned.
- Gates: `just check`, `just smoke` (15 OK), and `physics_determinism.rs`, `physics_tunneling.rs` and `physics_containment.rs` in release all green.

**R4c result 2026-09-27: (a) landed; (b) measured and dropped; (c) measured, not landed (needs a decision).**

Three prototypes, each benched, then captured when the bench showed a gain:
- **(a) Skip dedupe marks for single-cell grids.** `insert` tracks `single_cell_ids`: every entry spans one cell under a distinct id, checked with per-id generation marks and reset by `clear`. A query then meets each id at most once, so it skips `begin_query` and the marks. The first multi-cell insert turns the check off, so physics grids (walls, inflated bodies) pay one flag test per insert. Tests cover the duplicate-id fallback and point grids re-inserted after `clear`.
- **(b) Per-slot owner cell.** Built at scatter time with a `MIXED_SLOT` sentinel, so pure slots skip the per-entry `entry_cells` compare with exact semantics kept.
- **(c) No exact-cell compare at all** (Müller's "hash collision OK"). Probe only. It fails `hash_aliasing_never_produces_false_candidates`, which pins `D-062`'s "each entry stores its exact cell so hash aliasing … without producing false candidates".

| Variant | bench `points` | `points_visit` | `boxes` | `dense_pile_awake/3000` | `steer_agents_system` | physics awake `physics_step` |
| --- | --- | --- | --- | --- | --- | --- |
| R4b (before) | 20.66 | 25.53 | 52.82 | 8.52 | 27.51 / 29.40 | 8.35 / 8.65 |
| (a) **landed** | 15.14 | 22.05 | 51.96 | 8.58 | **24.65 / 26.31** | 8.44 / 8.70 |
| (a)+(b) | 14.01 | 21.25 | 49.79 | 8.74 | 24.59 / 26.20 | 8.38 / 8.67 |
| (a)+(c) probe | 12.02 | 17.87 | 47.31 | 8.26 | not captured | not captured |

Units: benches in ms; scene rows are `--repeat 3` median avg / p95 in ms.

- (a) captures: `20260927T222605Z-ecs-high-load` and `20260927T222744Z-physics-stress`; (a)+(b): `20260927T222400Z` and `20260927T222540Z`.
- **Why (a) lands:** it cuts `steer_agents_system` 10.4%. Physics awake is flat within run spread (8.38–8.47 across the (a) runs; physics grids are multi-cell).
- **Why (b) is dropped:** it adds 0.06 ms of steering gain (noise) plus a 512 KB slot array, and physics is flat.
- **(c) as a follow-up:** it is the fastest grid (−20% `points` and −4% `dense_pile_awake` vs (a)), but it reverses a `D-062` clause. It would need a new decision superseding that clause, slot-level dedupe (two aliased query cells would otherwise visit an id twice on the no-marks path), rewritten aliasing tests, and a check that `speculative_pass` tolerates false candidates. That was the original follow-up proposal; the continuation rejection below now closes it.

**R4c(c) continuation result 2026-09-28: rejected after a complete prototype.** Removed exact-cell loads and deduplicated queried hash slots, retaining ID marks for multi-cell/repeated-ID insertion. Tests cover aliased query cells, generation wrap, uniqueness and true-hit retention. All 65 physics unit tests and release determinism/tunneling/containment pass. Caller audit: steering rejects by squared distance; persistent pair building rejects by inflated AABB overlap; speculative static sweeps reject through the existing swept-shape test. Candidate ordering remains deterministic but changes solver and floating-point accumulation order, including equal-time sweep tie selection. D-062 stays unchanged because this prototype is reverted.

Native `spatial_grid_query_50k_dense` Criterion: boxes **51.222 → 44.244 ms (−13.6%)**, points **14.821 → 12.084 ms (−18.5%)**, visitor **21.778 → 17.409 ms (−20.1%)**. Canonical ECS three-repeat medians: update **26.03 / 27.59 → 25.21 / 28.52 ms**, steering **24.46 / 26.14 → 23.52 / 26.84 ms**, total **26.67 / 28.29 → 26.20 / 29.59 ms** (avg / p95). Post steering means span 21.69–23.94 ms: the small scene gain is not stable enough to justify weakening the public contract, and p95 worsens. Physics now sleeps during the capture (210 awake frames versus 300), so its whole-window improvement is not an isolated throughput result. Awake physics **2.48 / 3.53 ms** also fails to improve the prior matched R6 p95 of 3.21 ms. This is a conservative rejection, not proof that false candidates can never help. R4c(b) remains dropped.

Evidence: handoff `evidence/r4c-{bench-before,bench-after,ecs-before,ecs-after,physics-after,release-physics}.log` and saved prototype source/tests; pre ECS `capture-root/perf-runs/20260928T142111Z-ecs-high-load`, post ECS `perf-runs/20260928T142214Z-ecs-high-load`, physics `perf-runs/20260928T142346Z-physics-stress`.

**R4 overall** (baseline → after R4c(a)):
- ecs-high-load: update 65.58 → **28.20** ms (−57%), total 66.51 → **28.98**, `steer_agents_system` 62.03 → **24.65**.
- physics-stress: awake `physics_step` 9.57 → **8.44** (−12%).
- Gates for R4c: `just check`, `just smoke` (15 OK) and the three release physics tests all green.

### R2 [score 10] — Cache per-substep travel AABBs

**Scene:** physics-stress. Effort XS. Expected saving about 1.5 ms of 14.1 ms awake (about 10%). The result is bit-identical because it's the same float operations.

**Evidence:**
- The pair loop recomputes `proxies[b_idx].travel_aabb(sub_dt)` for every grid candidate (step.rs:895), and `travel_aabb` for each initiator (step.rs:884).
- `Proxy::travel_aabb` is 10.65% inclusive, including `Aabb::union` at 5.61% and `Aabb::min` inside union at 2.37%. `Aabb::overlaps` is 6.93%.

**Change:** fill a `Vec<Aabb>` of travel AABBs once per substep in `PhysicsBuffers` (O(N)) and index it in the loop (O(N·k) lookups instead of recomputes).

**Checks:** awake `physics_step` and `dense_pile_awake/3000` improve; determinism, tunneling and containment tests green.

**Result 2026-09-27: landed.**
- `PhysicsBuffers::travel_aabbs` is filled once per substep, right before the pair loop. The initiator AABB and every candidate's overlap test index it, so the float ops are unchanged.
- Bit-identical: the per-frame `physics:` lines (proxies, sleeping, pairs, contacts over all 360 frames) hash the same before and after, and the awake frame count stays 225.

| Metric | Before (after R4c) | After R2 |
| --- | --- | --- |
| physics-stress awake `physics_step` | 8.44 / 8.70 | **7.82 / 8.17** (−7.3%) |
| physics-stress total (whole window) | 7.01 / 9.30 | **6.54 / 8.72** |
| bench `physics_step/dense_pile_awake/3000` | 8.58 ms | **7.83 ms** (−8.7%) |

- Scene rows are `--repeat 3` medians (avg / p95, ms); capture `20260927T223305Z-physics-stress`.
- A first capture (`20260927T223147Z`) ran during a VS Code CPU spike (164%, load 2.8), so its three runs spread from 7.85 to 11.35 ms. It was discarded and retaken on a quiet machine.
- Gates: `just check`, `just smoke` (15 OK) and the three release physics tests all green.

### R5 [score 7.5] — Reuse the pair list across substeps

**Scene:** physics-stress; also physics at 10k–25k. Effort L. Expected saving 4–5 ms of 14.1 ms awake (about 30%).

**Evidence:**
- `substep` runs the full grid query, AABB prefilter and pair push for every one of the 4 substeps (`for _ in 0..substeps`, step.rs:534; pair loop step.rs:878-897).
- That work is 46.7% of physics-stress samples: `SpatialGrid::query` 26.9% + `travel_aabb` 10.65% + `overlaps` 6.93% + pair push 2.24%.
- `D-067` independently named "pair-list reuse across substeps" as the next lever after its 25k profile.

**Change:**
- Build pairs once per frame from AABBs inflated by the full frame's travel plus the `D-062` margin. Keep the narrow phase per substep over that list.
- Or go further, in Box2D v3 Solver2D style: compute contacts once per step and update separation within substeps from body deltas via local anchors.
- Wake (`D-065`), event (`D-064`) and speculative-margin semantics must be re-derived. This needs a `DECISIONS.md` entry that supersedes the relevant clauses.

**Checks:** awake `physics_step` improvement; all physics tests; `pile_plus_bullet` and `projectile_stream` benches must not regress more than 10%.

**Prior checkpoint (2026-09-27):** stopped for review before code. The subsequent QA/R5 request approved the recommended variant and its event-order change.

Current evidence, from the full capture `20260927T230122Z-physics-stress` after R2/R4/R7/R8:
- Awake `physics_step` is 7.67 ms.
- The substep pair loop (`SpatialGrid::for_each_in` plus its filter and push) is **59.6%** of all samples, about 4.6 ms awake. `narrow_phase` is 6.8%, `solve_contacts` 6.6%, and `ImpulseMap` get + insert 6.2% (about 0.5 ms, so R6 is now worth about 0.5 ms, not the 2.5 ms first estimated).
- Each substep narrow-phases about **31.0k pairs**, of which only about **8.6k** become contacts. The pair prefilter reuses `D-062`'s 16 px half-cell grid-staleness margin on the initiator AABB, so 72% of the pairs it passes are beyond any possible speculative admission (`|v_rel|·sub_dt + 4·linear_slop` ≈ 1–3 px here).

#### D-075 implementation

The approved draft now lives in `DECISIONS.md` as **D-075**, with its index row and explicit partial-supersession notices on D-062/D-066. The recommended pair-list variant is implemented; the alternative below remains deferred.

- `PhysicsBuffers` keeps build-time inflated AABBs, per-proxy radius and accumulated travel, and an invalidation flag. Gather invalidates every frame, including proxy identity/order changes with the same count. A contact wake invalidates before the next substep; budget exhaustion checks run in proxy order before each narrow phase.
- Each build uses the remaining frame time and restages the grid without a half-cell query margin. Travel is accumulated from actual displacement after integration and the static sweep clamp. Solver, impulses, wake thresholds, islands, event gate and per-substep narrow phase retain their existing paths.
- Three new tests cover 1,200 substeps of randomized mixed-shape piles/bullets/wakes against fresh pair finding and an exhaustive contact oracle; floor pairing in the contact-waking frame; and a fast-body velocity spike that forces a budget rebuild before admission. The test instrumentation is compiled out of production builds.

**QA before R5:** read the complete incoming diff and fixed two capture bugs: inherited `TUNGSTEN_GPU_TIMING` could contaminate CPU measurements with a blocking readback, and regex punctuation in a system name could corrupt sample parsing. Added regression coverage for both. `just check`, `just script-test`, all 15 Vulkan smoke rows, the visual regression, and release determinism/tunneling/containment passed on the pre-R5 engine. Determinism hash: `ae0818a3f4564331` in both runs; containment: 0/3000 escapes over 2400 steps.

**R5 validation:** `just check` and the three new tests pass. Release determinism gives `fb625e0eb4026424` in both runs, with no tunneling misses and 0/3000 containment escapes over 2400 steps. Different hashes across implementations are expected under D-075's contact-order change. The post-R5 script suite, 15 Vulkan smoke rows, explicit ecs-high-load smoke run, and visual regression also pass.

Criterion, identical `target-cpu=native` flags, performance governor, 10 samples, 2 s warm-up, 12 s requested measurement; time point estimates:

| Bench | QA-fixed pre-R5 | R5 | Change |
| --- | --- | --- | --- |
| `dense_pile_awake/3000` | 7.799 ms | 2.373 ms | −69.6% |
| `projectile_stream/3000` | 3.347 ms | 2.800 ms | −16.3% |
| `projectile_stream/10000` | 13.309 ms | 10.473 ms | −21.3% |
| `projectile_stream/25000` | 39.808 ms | 41.660 ms | +4.7%; median of three paired repeats |
| `pile_plus_bullet/10000` | 5.815 ms | 0.731 ms | −87.4% |
| `pile_plus_bullet/25000` | 106.27 ms | 47.367 ms | −55.4% |

The pile-plus-bullet benchmark evolves through settling and sleep while Criterion samples it: the 10k intervals were wide (1.805–15.604 ms before, 0.566–1.204 ms after). Its improvement includes the simulation's changed settling trajectory, rather than isolating equal contact work. The sleep-disabled 3k bench is the stable solver-throughput gate. The first 25k projectile comparison was inconclusive by Criterion's change statistic. Three paired repeats reproduce the point estimates: pre 39.888 / 39.808 / 39.710 ms, post 41.660 / 41.691 / 41.390 ms; median +4.7%. **The gate passes**, and an equal-frame control corroborates it: the unchanged `build_projectile_stream(25_000)` builder, 240 warm-up steps then 300 timed steps, three runs per implementation. Pre mean / p95: 38.865 / 40.911, 38.699 / 40.375, 39.561 / 42.035 ms; post: 29.452 / 33.240, 30.464 / 34.833, 29.819 / 33.712 ms. Median mean falls **23.3%**, median p95 falls **17.6%**. This probe used the same native build flags and scene builder in an isolated copy; no benchmark or scene workload changes land. The Criterion intervals remain recorded rather than presented as a precise 4.7% slowdown.

Canonical physics-stress captures use identical frame-pointer builds, Vulkan immediate/latency 1, 60 warm-up + 300 measured frames, and three sequential runs per tree. Values are medians of per-run avg / p95 in ms:

| Metric | QA-fixed pre-R5 | R5 |
| --- | --- | --- |
| Awake `physics_step` | 7.50 / 7.58 | 2.68 / 3.39 |
| Whole-window total | 6.10 / 7.86 | 2.96 / 3.76 |
| Awake measured frames | 225 / 225 / 225 | 300 / 300 / 300 |

Captures: `perf-runs/20260928T023103Z-physics-stress` (dirty fingerprint `62481cf0f606`) and `perf-runs/20260928T023612Z-physics-stress` (dirty fingerprint `5de121f3b516`). R5 changes pair order and therefore settling: the 360-frame capture now remains awake throughout. An 1800-frame follow-up confirms the first fully asleep frame is **466**, versus **286** pre-R5; the rested tail returns to about 0.16 ms. The post-R5 full profile is `perf-runs/20260928T133928Z-physics-stress` (perf stat, perf record and flamegraph). The awake-phase and sleep-disabled bench gains are the relevant comparison; whole-window timing blends different sleep windows.

**Alternative, not recommended now: Box2D v3 Solver2D (contacts once per frame, separation relaxed from body deltas).** It would also remove 3 of 4 narrow phases (6.8%), but it rewrites semantics the variant above keeps:
- `D-063`'s narrow phase per substep would become once per frame.
- `D-064`'s admission margin would become one frame of relative travel, with separation updated as `s + n·(Δp_b − Δp_a)`.
- `D-065`'s wake test would run once per frame, so wake waves would hop one frame per ring instead of one substep, and islands would form from frame contacts.
- Events would emit once per frame.

Measure the variant above first.

**Follow-on implemented by R6 below (separate entry):** between rebuilds a pair's list index is stable. Its accumulated impulse can live in the pair record and be read directly in substeps 2–4. The `ImpulseMap` would then be probed only for pairs new at a build and rebuilt once at frame end, superseding `D-063`'s "map rebuilt from live contacts each substep". Expected gain is about 0.5 ms awake at 3k (6.2% of samples).

### R6 [score 6] — Carry warm-start impulses across substeps

**Scene:** physics-stress. Effort S after R5 (M standalone). Expected saving about 2.5 ms awake (about 18%).

**Evidence:**
- The contact build calls `impulses.get(key)` (step.rs:962) on a 16-byte-key open-addressing map that is rebuilt every substep (`impulses_next.reset/insert`, step.rs:1036-1041).
- `ImpulseMap::get` is 14.07%, `insert` 1.52%, `reset` 1.28% and `Vec<u128>::resize` 1.03%: 17.9% of samples, about 19.7% of `physics_step`.
- `D-067` also measured about 12% impulse-map probing at 25k.

**Change:**
- With R5's frame-stable pair list, store the accumulated impulse in the pair or contact record and read it directly in substeps 2–4.
- Probe the map only once per frame, against last frame's entries, and rebuild it only at frame end.

**Checks:** as R5; warm-start behavior pinned by the existing solver tests.

**Result 2026-09-28: landed, D-076.** Indexed impulses are cleared on contact disappearance and transferred by key across rebuilds. Only the first substep after a pair build probes the map; final live contacts synchronize at frame end, with a correctness fallback before mid-frame rebuilds. This narrows the original once-per-frame claim on wake/budget-trip frames. The randomized R5 oracle now also compares every warm-start value with a separate D-063 per-substep map; a dedicated test covers disappearance, return, invalidation and empty final synchronization.

Fresh Criterion point estimates, identical native builds, 10 samples / 2 s warm-up / 12 s requested measurement:

| Bench | Incoming R5 | R6 | Change |
| --- | --- | --- | --- |
| dense_pile_awake/3000 | 2.3786 ms | 2.1707 ms | −8.7% |
| projectile_stream/3000 | 2.7869 ms | 2.8199 ms | +1.2% |
| projectile_stream/10000 | 10.567 ms | 11.162 ms | +5.6% |
| projectile_stream/25000, initial | 42.910 ms | 46.056 ms | +7.3%, wide intervals |
| pile_plus_bullet/10000 | 0.72587 ms | 0.71739 ms | −1.2%, evolving sleep state |
| pile_plus_bullet/25000 | 46.092 ms | 45.970 ms | −0.3% |

The 25k projectile paired repeats were incoming 42.910 / 47.307 / 98.883 ms versus R6 46.056 / 44.787 / 41.562 ms. The large third incoming value sampled a different evolving window (110 iterations versus 165), so the median ratio is not a reliable isolated throughput claim. A fixed-window control uses the unchanged builder, 240 warm-up + 300 measured steps, three runs per version: median mean **26.7384 → 26.8058 ms (+0.25%)**, median p95 **29.8465 → 30.0786 ms (+0.78%)**. All configured fast-body sizes satisfy the 10% gate.

Canonical three-repeat comparison initially gave awake physics **2.67 / 3.19 → 2.57 / 3.38 ms** (avg / p95); a matched archived-binary repeat reproduced the average benefit and resolved the p95 drift: **2.76 / 3.56 → 2.51 / 3.21 ms**, total **3.08 / 3.95 → 2.83 / 3.65 ms**. All 360 per-frame physics count lines match exactly across implementations. The first result is retained rather than discarded.

Evidence: initial post capture `perf-runs/20260928T140712Z-physics-stress`; matched repeat captures `capture-root/perf-runs/20260928T140956Z-physics-stress` and `20260928T141008Z-physics-stress` in the handoff directory. `evidence/{incoming,r6}-physics-bench.log`, `r6-projectile-{before,after}-{2,3}.log`, and `r6-window-{before,after}.log` preserve the raw results; `r6_projectile_window.rs` preserves the control. All 29 physics step unit tests (including the three R5 regressions) and release determinism/tunneling/containment pass; final workspace/GPU gates remain due at close-out.

### R3 [score 1.35] — Columnar `integrate_loose_bodies`

**Scene:** ecs-high-load; any scene with collider-less dynamic bodies. Effort S. Expected saving about 1.9 ms of 74 ms (2.6%).

**Evidence:**
- `integrate_loose_bodies` (step.rs:741-758) gathers entities, then does `world.get::<Collider>` (744), `get_mut::<Velocity>` (749) and `get_mut::<Position>` (754) per entity.
- Each call hashes a `TypeId` with SipHash into `Archetype::columns` (archetype.rs:80).
- Evidence: `physics_step` system 2.02 ms at 50k; `integrate_loose_bodies` 2.66%; `hash_one<&TypeId>` 0.86% + `Sip13Rounds::write` 0.36% + `get_mut<Velocity>` 0.49% self.
- The existing `position_integration_50k` bench (1.42 ms) against `query2_mut_10k` (3.45 µs per 10k) shows the gap between random lookups and a columnar pass.

**Change:**
- Add an archetype-filtered mutable query that excludes `Collider`, e.g. `World::query3_mut_without::<Velocity, Position, RigidBody, Collider>`, following the `query2_opt2` shape from `D-066`.
- Integrate in place.

**Checks:** `physics_step` row ≤ 0.2 ms at 50k; `position_integration_50k` rewritten to the new shape.

**Result 2026-09-27: landed.**
- `World::query3_mut_without::<A, B, C, X>()` filters archetypes lacking `X` once per archetype. It shares a private `query3_mut_excluding` body with `query3_mut` and panics if `X` is a queried type; tests cover both.
- `integrate_loose_bodies` is one columnar pass over `<Velocity, Position, RigidBody, Collider>` with the same float ops. The `loose_bodies` entity scratch is removed.
- `position_integration_50k` now runs this shape.

| Metric | Before (after R4c/R2) | After R3 |
| --- | --- | --- |
| ecs-high-load `physics_step` | 2.02 / 2.04 | **0.06 / 0.08** (target ≤ 0.2 met) |
| ecs-high-load update | 28.20 / 29.72 | **26.18 / 27.75** |
| ecs-high-load total | 28.98 / 30.47 | **26.88 / 28.54** |
| bench `position_integration_50k` | 1.84 ms (old lookup shape) | **35.5 µs** (new shape) |

- Scene rows are `--repeat 3` medians (avg / p95, ms); capture `20260927T223810Z-ecs-high-load`. physics-stress isn't cited: every body there has a collider, so the pass visits no archetype.
- Gates: `just check`, `just smoke` (15 OK) and the three release physics tests all green.

### R7 [score 0.85] — Default sprite extract without per-sprite hashing

**Scenes:** physics-stress, the platformer, any default-extract game. Effort S. Expected saving about 0.2 ms at 3k sprites; linear in count, so about 3 ms at 50k (projection from 0.34 ms per 3k).

**Evidence:**
- `extract_sprites_default` is 2.80% (extract avg 0.34 ms, p95 0.57 ms). Per sprite it does:
  - a `String`-keyed registry lookup (sprite_extract.rs:59);
  - a SipHash `HashMap<BatchKey>` lookup (74-100): 0.89%, of which `make_hash` is 0.71%;
  - random `get::<UniformOverrideBlock>` (80) and `get::<ParallaxLayer>` (130): 0.35% + 0.25%;
  - un-reserved instance pushes: 0.17%.

**Change:**
- Memoize the last `(asset_id → &SpriteAsset)` and the last `BatchKey → index` before the map lookup.
- Resolve the optional `UniformOverrideBlock` and `ParallaxLayer` columns per archetype (an optional-column query like `query2_opt2`).
- Reserve instances. Output must stay byte-identical (`D-042`, M26/M30 notes in the file header).

**Checks:** extract p95 in physics-stress; `crates/tungsten/src/tests/sprite_extract.rs`; visual regression.

**Result 2026-09-27: landed (without the reservation).**
- New `World::query3_opt2::<A, B, C, D, E>()`: three required and two optional columns resolved once per archetype, in `query3` order. A test pins order and optional presence.
- `extract_sprites_default` collects `(Entity, &Transform, &Sprite, &SpriteAsset, Option<&UniformOverrideBlock>, Option<&ParallaxLayer>)` through it. A last-seen `(asset_id, &SpriteAsset)` memo skips the String-keyed registry lookup, and a last-seen `(BatchKey, index)` memo (reset per z-run) skips the `HashMap<BatchKey>` probe.
- **Not done:** instance reservation. Per-batch sizes aren't known without a second pass, and reserving the remaining z-run per new batch would over-allocate in multi-key runs. The profile share was 0.17%.

| Metric (physics-stress) | Before (after R2) | After R7 |
| --- | --- | --- |
| extract | 0.27 / 0.29 | **0.09 / 0.11** (−67%) |
| total (whole window) | 6.54 / 8.72 | **6.33 / 8.42** |
| render_acquire | 0.11 / 0.29 | 0.14 / 0.51 |
| render_encode / GPU | 0.06 / 0.19 | 0.06 / 0.19 |

- Scene rows are `--repeat 3` medians (avg / p95, ms); capture `20260927T224257Z-physics-stress`.
- Render p95 moved 0.44 → 0.65 ms, entirely in acquire: the CPU reaches the next present sooner, the same pacing effect as R1.

**Byte-identical output:**
- The visual regression scene uses the example's own extract, so it doesn't exercise this path.
- `extract_sprites_default` was compared directly: frames 5 and 40 of physics-stress (1920×1080) and of example-04 (1280×720, `ParallaxLayer` plus material sprites) are byte-equal before and after R7. The frames differ from each other, so the content is live.
- A first physics comparison at 1280×720 was void (the pile sits below the viewport) and was redone at 1920×1080.

Gates: `just check` (includes the 18 `sprite_extract` tests), `just smoke` (15 OK) and the visual regression test all green.

### R8 [score 0.8] — No-op hasher for column lookup

**Scenes:** all; ECS-wide. Effort XS. Expected saving under 1% on the canonical scenes; it shrinks further once R3 and R7 remove the main callers.

**Evidence:** `Archetype::columns: HashMap<TypeId, Box<dyn AnyColumn>>` (archetype.rs:80) is hit by every `get`/`get_mut`/`has` (storage.rs:241-257) and by per-archetype query setup. Self time: `RandomState::hash_one<&TypeId>` 0.86% + `Sip13Rounds::write` 0.36% in ecs-high-load; the `HashMap<TypeId>` frame is 0.56% under extract in physics-stress.

**Change:** an in-repo `BuildHasher` that passes `TypeId`'s `write_u64` through, as Bevy's `TypeIdMap` does with `NoOpHash`. Also apply it to `add_edges` and `remove_edges`.

**Checks:** `position_integration_50k` and `naive_query2_via_entities_10k` bench deltas.

**Result 2026-09-27: landed.**
- `ecs::archetype::TypeIdMap<V>` = `HashMap<TypeId, V, BuildHasherDefault<TypeIdHasher>>` now backs `Archetype::columns`, `add_edges` and `remove_edges`. The `split*_columns_mut` helpers take it.
- `TypeIdHasher::write_u64` passes through. `write` is a full byte mix, kept only as a fallback.
- Tests: distinct `TypeId`s key distinctly, and `TypeId::hash` calls `write_u64` exactly once on rustc 1.98.1, which pins the fast-path assumption.
- **Plan error:** `naive_query2_via_entities_10k` (and `naive_query_single_10k`) run on a bench-local `NaiveWorld`, not `World`, so R8 can't move them. Their runs drifted 613 → 627 → 699 µs with no code on their path. `spawn_insert_3_components_10k` (archetype migration through the edge and column maps) replaces it as the second check.
- Resources (`resource.rs`, also `TypeId`-keyed) are out of this step's scope.

| Metric | Before (after R7) | After R8 |
| --- | --- | --- |
| bench `position_integration_50k` | 38.1 µs | **34.5 µs** (−12%) |
| bench `spawn_insert_3_components_10k` | 3.56 ms | **1.78 ms** (−50%) |
| bench `naive_query2_via_entities_10k` (not engine code) | 613 µs | 627 / 699 µs (noise) |
| ecs-high-load update | 26.18 / 27.75 | 26.06 / 27.63 |
| ecs-high-load total | 26.88 / 28.54 | 26.79 / 28.47 |
| physics-stress awake `physics_step` | 7.75 / 8.06 | 7.68 / 7.96 |
| physics-stress total | 6.33 / 8.42 | 6.30 / 8.38 |

- Scene rows are `--repeat 3` medians (avg / p95, ms); captures `20260927T225615Z-ecs-high-load` and `20260927T225547Z-physics-stress`.
- Scene rows are flat within noise, as predicted once R3 and R7 removed the per-entity lookups. The gain is ECS-wide structural cost (spawn/insert) and per-archetype setup.
- A first ecs-high-load capture (`20260927T225406Z`) started at load 2.6, and its run 1 was 22% slow. It was retaken on a quiet machine.
- Gates: `just check`, `just smoke` (15 OK) and the three release physics tests all green.

### R9 [unscored] — Example steering and tint math (workload change)

**Scene:** ecs-high-load.

**Evidence:**
- `steer_agents_system` self time 26.87% (about 20 ms) beyond the grid query: `sqrt` 8.34% (ecs_high_load.rs:305); per-agent `sin`/`cos` flow field (311-314); per-agent drift `cos`/`sin` of a constant seed recomputed every frame (316-319); `Vec2` math.
- `tint_agents_system` 0.99 ms (three `sin` per agent via `rgb_wheel_color`). `orient_agents_system` 0.46 ms (`atan2f` 0.50%).

**Change:** only if the scene's purpose calls for it, e.g. store the drift vector in `StressAgent` at spawn, or use an inverse-sqrt form. Record a new ecs-high-load baseline in the same change, because this edits the benchmark itself, not the engine.

**R9 result 2026-09-28: landed as ECS scene workload v2.** Cache constant drift and each color channel's phase sine/cosine at spawn; compute six carrier trig values per frame rather than three per sprite. Rewrite radial repulsion as `delta * (1 / distance - 1 / radius)`. The spatial grid, neighbor admission and dynamic flow remain unchanged. Tests compare colors within one byte and radial repulsion within 1e-6, and retain deterministic same-version steering. Floating-point operation order and the agent component size change, so these are workload savings, not engine improvements.

Canonical 50k fixed-world three-repeat medians, avg / p95 ms:

| Metric | Scene v1 after R6 | Scene v2 baseline |
| --- | --- | --- |
| update | 26.03 / 27.59 | 24.63 / 26.03 |
| steering | 24.46 / 26.14 | 23.85 / 25.25 |
| tint | 0.97 / 1.13 | 0.18 / 0.19 |
| total | 26.67 / 28.29 | 25.32 / 26.79 |

Evidence: handoff `capture-root/perf-runs/20260928T142111Z-ecs-high-load` and repo `perf-runs/20260928T143436Z-ecs-high-load`. All example unit tests pass. The modest steering gain and substantial tint reduction justify retaining the changes; future ECS captures must identify v2 and density mode.

A second matched archived-binary v1/v2 comparison confirms the new workload baseline: update **25.91 / 27.24 → 24.61 / 26.08**, steering **24.33 / 25.78 → 23.84 / 25.28**, tint **0.97 / 1.13 → 0.17 / 0.18**, total **26.56 / 27.95 → 25.29 / 26.82 ms**. Evidence: handoff `capture-root/perf-runs/20260928T143753Z-ecs-high-load` and `20260928T143855Z-ecs-high-load`.

### T5–T7 — Lower-priority tooling (propose, do when a step needs it)

**T5 — GPU attribution:**
- `GpuFrameTimings::frame_gpu_ms` covers only the scene pass.
- None of the three scenes enables the post stack, lighting or materials, and each yields a single sprite batch (baseline.rs:151, `extract_high_load_sprites`, and one sprite id in physics_stress.rs).
- GPU scene time reaches 10.63 ms at 400k sprites with no pass breakdown.
- Add per-pass timestamp queries and a `render-features` scene (several textures and batches, a material, lights, a post stack) before any GPU-side work.

**T5 result 2026-09-28: landed.** `GpuFrameTimings` adds ordered per-pass durations and a first-scene-to-present-blit span. The historical `frame_gpu_ms` / `gpu=` value stays scene-only. Timestamp writes cover all render passes, including each bloom mip stage; post slot indices prevent duplicate labels. Allocation follows the actual stack/mip count and falls back to untimed rendering on query-count overflow. Timing values clear before acquisition, including disabled/unsupported/skipped paths. One resolve/readback handles the entire diagnostic frame; untimed frames allocate no query resources or pass labels. A shared pass recorder replaces the duplicate scene timestamp helper.

The new 4k `render-features` scene uses **15 actual batches** (3 lit, 6 material, 6 stock), nearest/linear root-manifest textures, two uniform variants, three z layers, two point lights and a directional light, bloom → vignette → SMAA High → text → present. No new assets or dependencies. Unit tests cover the intended batch keys, query-count limits, timestamp decoding/wrap, scene compatibility, stale-value clearing and parser warm-up/missing-pass handling. The smoke matrix now includes this timed scene.

Canonical Vulkan, 60 warm-up + 300 measured, three-repeat medians (avg / p95 ms), new attribution baseline:

| Metric | render-features |
| --- | --- |
| Total, GPU timer off | 6.95 / 8.27 |
| Extract / encode means | 0.84 / 0.31 |
| Scene GPU | 1.41 / 1.42 |
| Bloom threshold / composite | 0.13 / 0.13; 0.37 / 0.38 |
| Largest bloom upsample (mip 0) | 0.34 / 0.34 |
| Vignette | 0.29 / 0.30 |
| SMAA edges / blend weights / neighborhood | 0.70 / 0.73; 1.38 / 1.50; 0.70 / 0.73 |
| Text / present | 0.01 / 0.01; 0.33 / 0.33 |
| Render span, includes inter-pass gaps | 6.21 / 6.36 |

All **19 actual passes** appear in each diagnostic frame and summary; the full table includes all 12 bloom stages. These are attribution measurements, not a claimed GPU optimization. Capture `perf-runs/20260928T145129Z-render-features`. At sprite 100k, T5's untimed control remains flat versus the valid R1/R6 binary: total **3.98 / 5.28 → 3.97 / 5.27**, encode **0.27 / 0.30 → 0.27 / 0.32**, extract **2.09 / 2.28 → 2.07 / 2.25**, scene GPU **3.11 / 3.95 → 3.10 / 3.96 ms**; capture `perf-runs/20260928T145222Z-sprite-stress-count100000`.

GPU validation: timed/untimed render-features PNGs are byte-identical at frames **5 and 40**, and those two frames differ from each other (live animation). Vulkan smoke passes all **16** example/fixture rows, including per-pass bloom/SMAA readback. The reference sprite visual test passes. Evidence: handoff `evidence/render-features-{5,40}-{off,on}.{png,log}`, `t5-tests.log`, `final-smoke.log`, `final-visual.log`, `t5-render-features.log`, `t5-sprite-100k.log`. Unsupported adapters and other backends are covered by fallback logic/unit checks, not claimed as hardware-tested.

**T6 — Cheaper full captures:**
- The ecs-high-load `perf-record.data` is 859 MB for a 27 s run.
- Frame pointers cost nothing measurable (see above), so offer `--call-graph fp` or a `-F` knob, keeping dwarf for inline attribution.

**T6 result 2026-09-28: landed.** `--call-graph dwarf|fp` and `--sample-frequency <positive Hz>` reach `perf record`, with requested settings in both READMEs and repeat summaries. DWARF remains default; FP requires an effective `force-frame-pointers=yes` build flag (the last override wins). `perf-record.log` retains recording failures/throttling diagnostics. Regression tests exercise argument validation, flag overrides, actual mock-perf invocation and GPU-readback isolation; shellcheck passes.

Matched 360-frame physics captures of the same archived R6 binary, canonical flags/backend, with flamegraphs generated successfully:

| Mode | Recording bytes | Samples | Capture under handoff `capture-root/perf-runs/` |
| --- | --- | --- | --- |
| DWARF, perf default Hz | 39,376,980 | 4,652 | `20260928T141642Z-physics-stress` |
| FP, perf default Hz | 755,492 | 4,613 | `20260928T141705Z-physics-stress` |
| FP, 499 Hz | 130,764 | 699 | `20260928T141728Z-physics-stress` |

FP shrinks the recording **52×** at comparable sample count; FP/499 Hz is **301×** smaller with correspondingly fewer samples. These are capture-storage gains, not engine speedups. Keep DWARF for ambiguous/inline attribution and libraries without frame pointers; 499 Hz trades detail for cost and remains opt-in.

**T7 — ecs-high-load realism:**
- One example system is 95% of update, so the scene measures neighbor search more than ECS iteration.
- `--stress-count` also changes density.
- Add a density-preserving sweep (world area proportional to count) and an ECS-iteration bench at 50k matching the scene's archetype.

**T7 result 2026-09-28: landed.** `--ecs-density fixed|preserve` controls a child-only setting, records provenance and scales world dimensions, spawn/confinement bounds, steering center and camera bounds. Fixed remains default; 50k is identical in both modes. Tests pin area scaling, 50k equality, small-count bounds, option validation and inherited-environment isolation. All scene tests, parser tests and shellcheck pass.

Scene v2, canonical repeat-three medians, avg / p95 ms:

| Count | Density mode | Update | Steering |
| --- | --- | --- | --- |
| 12,500 | fixed | 3.19 / 3.30 | 3.00 / 3.12 |
| 12,500 | preserve | 5.89 / 6.26 | 5.70 / 6.07 |
| 25,000 | fixed | 8.08 / 8.56 | 7.71 / 8.18 |
| 25,000 | preserve | 11.67 / 12.59 | 11.28 / 12.20 |
| 50,000 | fixed | 24.61 / 26.07 | 23.83 / 25.31 |
| 50,000 | preserve | 24.71 / 26.59 | 23.91 / 25.79 |

Preserving density makes the 12.5k→25k→50k steering series much closer to linear than the fixed-world series. These measure entity scaling at approximately constant neighbor density, versus simultaneous entity/density growth. Bounds, flow, initial layout and visible population still vary; the sweep is not a pure ECS iteration benchmark. The 50k modes are an identical-workload control, with timing differences treated as run noise.

New native Criterion baselines with the full scene-v2 seven-component archetype: `high_load_query3_50k` **133.20 µs** (read/accumulate) and `high_load_query3_mut_50k` **29.143 µs** (bounded velocity writes), 10 samples, 2 s warm-up, 5 s measurement. No grid search or per-iteration allocation is included. These new measurements characterize iteration cost; they are not before/after optimization claims.

Evidence: handoff `evidence/t7-ecs-bench.log`, `t7-tests.log`, `t7-script-test.log` and `t7-<count>-<mode>.log`; captures:

- `perf-runs/20260928T144228Z-ecs-high-load-count12500-densityfixed/summary.md`
- `/home/joker/Projects/Tungsten-handoff-remaining-20260928/capture-root/perf-runs/20260928T144311Z-ecs-high-load-count12500-densitypreserve/summary.md`
- `/home/joker/Projects/Tungsten-handoff-remaining-20260928/capture-root/perf-runs/20260928T144330Z-ecs-high-load-count25000-densityfixed/summary.md`
- `/home/joker/Projects/Tungsten-handoff-remaining-20260928/capture-root/perf-runs/20260928T144354Z-ecs-high-load-count25000-densitypreserve/summary.md`
- `/home/joker/Projects/Tungsten-handoff-remaining-20260928/capture-root/perf-runs/20260928T144426Z-ecs-high-load-count50000-densityfixed/summary.md`
- `/home/joker/Projects/Tungsten-handoff-remaining-20260928/capture-root/perf-runs/20260928T144526Z-ecs-high-load-count50000-densitypreserve/summary.md`

## Not ranked (profiled, no actionable evidence)

- **wgpu per-submit bookkeeping** (`Queue::maintain`/`triage_submissions` 11.1%, `EncoderInFlight` drop 10.8% of canonical sprite-stress CPU samples): internal to wgpu, and the scene is present-bound. Revisit only if a scene becomes encode-bound.
- **Bind-group churn and render bundles:** one per-frame bind group (the present blit) and one sprite batch per stress scene. Toji's guidance treats 5–10 bind groups per frame as fine. No capture shows state-change cost.
- **View culling in the default extract:** no scene has off-screen default-extract sprites. ecs-high-load culls in its own extract (0.34 ms). Needs a T5-style scene before any claim.
- **Threading or SIMD graph colouring** (Box2D v3 style): `D-067` measured the solve at about 2% and rejected it. The physics-stress profile agrees (`solve_contacts` 3.88%).

## Best-practice references

- **Box2D v3 Solver2D:** contact points are computed once per step and "maintained across sub-steps" via local anchors, because recomputing every substep "would be very expensive". Supports R5 and R6. https://box2d.org/posts/2024/02/solver2d/
- **Box2D, "SIMD Matters":** graph colouring plus SIMD for the solver. Context for why `D-067`'s rejection still stands. https://box2d.org/posts/2024/08/simd-matters/
- **Müller, "Blazing Fast Neighbor Search with Spatial Hashing":** store each particle once at its center cell, check the surrounding 3×3 cells, dense count/partial-sum layout, "hash collision OK". Supports R4a and R4c. https://matthias-research.github.io/pages/tenMinutePhysics/11-hashing.pdf
- **The Rust Performance Book, Hashing:** the default SipHash is slow for small keys. Supports R7 and R8. https://nnethercote.github.io/perf-book/hashing.html
- **Bevy `TypeIdMap` = `HashMap<TypeId, V, NoOpHash>`:** `TypeId` is already a hash. Supports R8. https://docs.rs/bevy/latest/bevy/utils/type.TypeIdMap.html
- **wgpu `Queue::write_buffer_with` and `util::StagingBelt`:** write directly into staging memory, avoiding the intermediate `Vec`. Supports R1. https://wgpu.rs/doc/wgpu/struct.Queue.html, https://docs.rs/wgpu/latest/wgpu/util/struct.StagingBelt.html
- **Toji, WebGPU bind-group and render-bundle best practices:** why neither is ranked here. https://toji.dev/webgpu-best-practices/bind-groups.html, https://toji.dev/webgpu-best-practices/render-bundles.html
