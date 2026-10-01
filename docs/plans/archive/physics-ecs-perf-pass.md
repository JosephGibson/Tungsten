# Physics and ECS performance pass 2

status: done
goal: cut the owned metrics of the `physics`, `physics-sparse`, `churn` and `ecs` benchmark rows by removing the hotspots measured on 2026-10-01: the pair list rebuilt every substep, the safety-net sweep that queries the full grid, boxed per-value archetype moves, boxed commands and the slow `World::get` path. Every change is judged by `just perf compare` against the saved baseline `pre-physics-ecs-pass`.
non-goals: render and GPU work (`gpu`, `gpu-throughput` and the extract cost in `particles` are reported only); any change to a benchmark's work or knobs (no `workload_version` bump); threads in the physics step (`D-067`); an external ECS crate (`D-005`); `unsafe` code or a new dependency; a public bundle-insert API; software prefetch and a dynamic-only pair restage (both under "Not proposed"); looser verdict thresholds.
files to touch: `crates/tungsten-core/src/physics/step.rs` and `broadphase.rs`; `crates/tungsten-core/src/ecs/archetype.rs`, `storage.rs`, `world.rs` and `command_buffer.rs`; `crates/tungsten/src/app.rs` (`stage_flush_commands`); the matching tests under `crates/tungsten-core/src/tests/physics/` and `src/tests/ecs/`; `crates/tungsten-core/benches/physics_bench.rs` and `ecs_bench.rs`; `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md` (ECS section), `docs/perf/benchmarks.md` (digests, findings) and `CHANGELOG.md`.
ordered steps: 0 preflight; 1 static sweep grid (P2); 2 grid walk costs (P3); 3 pair repair (P1); 4 unboxed column moves (C1); 5 batched inserts at flush (C3); 6 command buffer without boxes (C2); 7 lean `World::get` (E1); 8 gated prototypes (P4, E2); 9 re-profile, full suite compare, decisions and docs. Two sessions: A, physics (0–3, P4) and B, ECS (4–7, E2, 9). Details under "Sessions" and "Ordered steps"; execution state under "Progress".
done-when: `just perf compare pre-physics-ecs-pass <final suite>` reads `improved` on the owned metrics listed under "Done when" and `regressed` on none; `just check`, `just smoke` and `just repo-check` pass.

## Context digest

- Branch `0.33`, commit `7276c8a` (workspace 0.32.0). The first pass was 0.30 (`D-075` pair persistence, `D-076` warm-start carry, the `TypeId` pass-through hasher). It was measured on the retired stress scenes, which mostly slept.
- Baseline: `pre-physics-ecs-pass`, a copy of `perf-runs/20261001T002643Z-suite` (8 rows, 5 runs each, valid). An A/A re-capture of the four owned rows read 0 regressed, 0 improved, 24 unchanged, 3 noisy.
- `physics`: the pair list is rebuilt 3.99 times per frame, not once. `build_pairs` is 70% of all samples; narrow phase and solver are under 4% each.
- `physics-sparse`: 3.2 pair builds per frame (54% of samples), plus 31,517 sweep queries per frame against a grid that holds 8,000 dynamic bodies and 4 walls (29%).
- `churn`: 293,960 `malloc` calls per frame, 85% of them one `Box` per moved component value. Spawning with k components moves k(k−1)/2 values: `flush` p50 is 1.99, 8.37 and 28.86 ms at 2, 6 and 12 components.
- `ecs`: 48% of `update` is benchmark work the engine can't touch (`heading`'s `atan2f`, `brain`, the digest). `follow` (18%) waits on cache misses in `World::get`. About 1.7 ms is overhead per archetype (setup and cold cache lines) across 488 archetypes.
- Evidence beyond the captures (exact call counts, the budget replay, profile tables, scripts): `perf-runs/20261001-physics-ecs-pass-evidence/`, with a README.
- Captures need a quiet machine. Besides `nxcodec.bin`, a second agent session running `cargo` invalidated the first suite of this session (see "Capture hygiene").

## Progress

The executing session fills this in at the end of every step, before it starts the next one. A step is not finished until its row is complete. "Step capture" is the capture directory; the two verdict columns quote the owned metrics from the compare reports.

| Step | State | Step capture | Against previous step | Against baseline | Digests | Kept |
| --- | --- | --- | --- | --- | --- | --- |
| 0 preflight (session A) | done | `perf-runs/20261001T014449Z-suite` (`physics`, `physics-sparse`, `ecs`, `churn`) | n/a | 0 regressed, 0 improved, 25 unchanged, 2 noisy (`follow`, `churn_spawn`, as in the A/A). `physics_step` p50 +0.3% in `physics` and +0.2% in sparse. Report: `perf-runs/20261001T014621Z-compare-suite` | The four equal the baseline's | n/a |
| 1 P2 | done | `perf-runs/20261001T023436Z-suite`; `integrated` rerun `perf-runs/20261001T023713Z-suite` | Against the preflight (`perf-runs/20261001T023829Z-compare-suite`): `physics-sparse` `physics_step` p50 8.07 → 6.57 (−18.7%) and p95 9.83 → 8.37 (−14.8%), both `improved`; `physics` `physics_step` p50, p95 and `update` p95 `unchanged` | `perf-runs/20261001T023630Z-compare-suite`: sparse `physics_step` p50 −18.5% and p95 −14.7%, `improved`; `physics` `unchanged`. `integrated` `total` read `noisy` there (two of its five runs were about 0.4 ms slow in every stage, extract included) and `unchanged` on all four owned metrics in the rerun (`perf-runs/20261001T023821Z-compare-suite`, p50 12.12 → 12.09) | The three equal the baseline's, in both captures | kept |
| 2 P3 | done | `perf-runs/20261001T025007Z-suite` (all three parts). Parts: floor alone `perf-runs/20261001T024002Z-suite`; plus direct table `perf-runs/20261001T024739Z-suite`; plus flag array, the step capture | Against step 1 (`perf-runs/20261001T025200Z-compare-suite`): `physics` `physics_step` p50 13.28 → 10.04 (−24.5%), p95 −24.2%, `update` p95 −24.1%; sparse p50 6.57 → 4.77 (−27.4%), p95 8.37 → 5.84 (−30.3%); `integrated` `total` p50 12.24 → 10.65 (−13.0%), p95 −12.7%, p99 −13.1%; all `improved`, jitter `unchanged`. By part, `physics_step` p50 of `physics` / sparse: floor −1.1% / −0.8% (`unchanged`), direct table −18.5% / −24.6% (`improved`), flag array −6.2% (`improved`) / −2.9% (`noisy`) | `perf-runs/20261001T025143Z-compare-suite`: `physics` p50 13.25 → 10.04 (−24.2%), p95 −24.4%, `update` p95 −24.3%; sparse p50 8.05 → 4.77 (−40.8%), p95 −40.5%; `integrated` `total` p50 12.12 → 10.65 (−12.1%), p95 −11.2%, p99 −11.1%; all `improved` | The three equal the baseline's, in every capture | kept, all three parts |
| 3 P1 | done | `perf-runs/20261001T030829Z-suite` (margin 0.5 px) | Against step 2 (`perf-runs/20261001T031002Z-compare-suite`): `physics` `physics_step` p50 10.04 → 5.86 (−41.6%), p95 −41.4%, `update` p95 −41.2%, `improved`; sparse p50 4.77 → 3.67 (−23.0%), p95 5.84 → 3.75 (−35.8%), `improved`; `integrated` `total` p50 10.65 → 10.75, p95, p99 and jitter all `unchanged`, not the expected `improved` (see notes) | `perf-runs/20261001T030953Z-compare-suite`: `physics` p50 13.25 → 5.86 (−55.7%), p95 −55.7%, `update` p95 −55.5%; sparse p50 8.05 → 3.67 (−54.4%), p95 −61.8%; `integrated` `total` p50 12.12 → 10.75 (−11.3%), p95 −9.2%, p99 −8.0%; all `improved`, jitter `unchanged`. Workload drift on `physics.pairs`, as expected | New: `physics` `86ffcabcdb15eed1`, `physics-sparse` `5899f9c8a69d79b1`, `integrated` `5f031f4d947964cb`; each row's five runs agree, and two captures of the state agree | kept |
| 8 P4 | done | `perf-runs/20261001T031601Z-suite` | Against step 3 (`perf-runs/20261001T031729Z-compare-suite`): `physics` `physics_step` p50 5.86 → 5.60 (−4.6%), p95 −5.6%, `update` p95 −5.6%, `improved`; sparse p50 3.67 → 3.43 (−6.7%), p95 −7.3%, `improved`; `integrated` `total` p50 −2.8% and p95 −3.6% `noisy`, p99 and jitter `unchanged` | `perf-runs/20261001T031723Z-compare-suite`: `physics` p50 13.25 → 5.60 (−57.8%), p95 −58.2%, `update` p95 −58.0%; sparse p50 8.05 → 3.43 (−57.4%), p95 −64.6%; `integrated` `total` p50 12.12 → 10.45 (−13.8%), p95 −12.4%, p99 −11.9%; all `improved` | The three equal step 3's | kept |
| Session A close-out | done | `perf-runs/20261001T031742Z-suite` (all eight rows, valid). Profiles: `perf-runs/20261001T032837Z-physics`, `perf-runs/20261001T032908Z-physics-sparse` | n/a | `perf-runs/20261001T032255Z-compare-suite`: `physics` `physics_step` p50 13.25 → 5.58 (−57.9%), p95 −58.2%, `update` p95 −58.0%; sparse p50 8.05 → 3.42 (−57.5%), p95 −64.6%; `integrated` `total` p50 12.12 → 10.50 (−13.4%), p95 −12.2%, p99 −12.6%; all `improved`, jitter `unchanged`. `ecs`, `churn`, `gpu-throughput` and `particles`: no `regressed`. `gpu`: `stage.extract` p95 `regressed`, which is machine drift (see notes). Peak RSS `unchanged` in all eight rows | `ecs`, `churn`, `gpu`, `gpu-throughput` and `particles` equal the baseline's; the three physics-bearing ones equal step 3's | n/a |
| 0 preflight (session B) | done | `perf-runs/20261001T044518Z-suite` (`ecs`, `churn`). GPU reference on the starting tree: `gpu` in `perf-runs/20261001T044916Z-suite`, `gpu-throughput` in `perf-runs/20261001T045216Z-suite` | n/a | `perf-runs/20261001T044602Z-compare-suite`: 0 regressed, 0 improved, 15 unchanged, 7 noisy. `update` p50 10.74 → 10.74 and p95 11.22 → 11.17; `flush` p50 8.48 → 8.58 (+1.2%, `noisy`) and p95 9.41 → 9.39; `follow` 1.95 → 1.91 (`noisy`). `bounds_wrap` 0.28 → 0.26 reads `noisy` here, not `improved`. GPU reference against the baseline: `gpu` `extract` p50 1.95 → 2.02 and p95 2.30 → 2.44 `regressed` (the drift session A found), `gpu-throughput` no `regressed` | `ecs`, `churn`, `gpu` and `gpu-throughput` equal the baseline's | n/a |
| 4 C1 | done | `perf-runs/20261001T053131Z-suite` (`churn`, `ecs`, `particles`, `integrated`), the fourth variant (see notes). One pass of the other four rows for their digests: `perf-runs/20261001T053354Z-suite` | Against the preflight (`perf-runs/20261001T053330Z-compare-suite`): `churn` `flush` p50 8.58 → 3.82 (−55.5%) and p95 9.39 → 4.16 (−55.7%), both `improved`. `ecs`: no `regressed`; `update` p50 10.74 → 10.66 and p95 11.17 → 11.01 `unchanged`, `cooldowns` and `buffs` `improved`, `follow` 1.91 → 1.98 `noisy`. `particles` and `integrated` against session A's close-out (`perf-runs/20261001T053540Z-compare-suite`): no `regressed`, `integrated` `total` `unchanged` | `perf-runs/20261001T053329Z-compare-suite`: `churn` `flush` p50 8.48 → 3.82 (−54.9%) and p95 9.41 → 4.16 (−55.8%), `improved`; `churn_spawn` 0.35 → 0.37 `regressed` (recording is untouched by this step; see notes). `ecs`: 0 regressed, `buffs`, `integrate` and `bounds_wrap` `improved`, `follow` 1.95 → 1.98 `noisy`. `particles`: 0 regressed. `integrated` `total` p50 −14.0% `improved` (the physics steps) | All eight equal the expected ones: the four captured rows in all five runs, the other four in one pass | kept |
| 5 C3 | done | `perf-runs/20261001T053929Z-suite` (`churn`, `particles`, `integrated`) | Against step 4 (`perf-runs/20261001T054105Z-compare-suite`): `churn` `flush` p50 3.82 → 3.48 (−9.0%) and p95 4.16 → 3.86 (−7.2%), both `improved`; churn system rows `unchanged` or `noisy`. `integrated` `total` `unchanged`. `particles`: `unattributed` p50 2.43 → 2.54 (+4.7%) and p95 3.02 → 3.21, `animate_sprites` p95 0.35 → 0.39, `regressed` (a memory-layout effect, not flush work; see notes) | `perf-runs/20261001T054104Z-compare-suite`: `churn` `flush` p50 8.48 → 3.48 (−59.0%) and p95 9.41 → 3.86 (−58.9%), `improved`; no churn system row `regressed`. `particles`: `unattributed` p50 2.37 → 2.54 and p95, `animate_sprites` p95 `regressed`. `integrated` `total` `improved` | `churn`, `particles` and `integrated` equal the expected ones | kept |
| 6 C2 | done | `perf-runs/20261001T055438Z-suite` (`churn`, `ecs`, `particles`, `integrated`; `ecs` is there as the previous capture for step 7) | Against step 5 (`perf-runs/20261001T055635Z-compare-suite`): `churn` `flush` p50 3.48 → 2.68 (−22.9%) and p95 3.86 → 3.07 (−20.6%), `improved`; `churn_spawn` 0.36 → 0.33, `churn_toggle` 0.08 → 0.04 and `churn_scan` `improved`. `particles` `unattributed` p50 2.54 → 2.22 and p95 `improved`, `animate_sprites` `unchanged`. `integrated` `total` p50 and p95 `noisy` (10.40 → 10.68), p99 and jitter `unchanged` | `perf-runs/20261001T055634Z-compare-suite`: `churn` `flush` p50 8.48 → 2.68 (−68.4%) and p95 9.41 → 3.07 (−67.4%), `improved`; no churn system row `regressed` (`churn_spawn` 0.35 → 0.33 `noisy`, so step 4's reading is gone). `ecs`: 0 regressed, `follow` 1.95 → 1.98 `noisy`. `particles`: `unattributed` p50 2.37 → 2.22 `improved`, `animate_sprites` p95 0.35 → 0.38 `regressed` (see notes). `integrated` `total` p50 12.12 → 10.68 `improved` | The four equal the expected ones | kept |
| 7 E1 | done | `perf-runs/20261001T065855Z-suite` (`ecs`, `churn`, `particles`, `integrated`), the amended state (see notes). The state first captured: `perf-runs/20261001T062938Z-suite` (`ecs`); the capture before that one, `perf-runs/20261001T062905Z-suite`, had a NoMachine session inside timing runs 1 and 2 and is not used | Against step 6 (`perf-runs/20261001T070051Z-compare-suite`): `follow` p50 1.98 → 1.75 (−11.7%), `improved`; `update` p50 10.65 → 10.44 (−1.9%) `noisy` and p95 `unchanged`; `brain` 2.18 → 2.26 (+3.6%) `regressed` (untouched by this step; see notes); the other system rows `unchanged` or `noisy`. `churn` `flush` p50 2.68 → 2.69 `noisy` and p95 `unchanged`, no churn row `regressed`. `particles` `unattributed` p50 2.22 → 2.10 `improved`. `integrated` `total` `unchanged` | `perf-runs/20261001T070050Z-compare-suite`: `follow` p50 1.95 → 1.75 (−10.1%) `improved`; `update` p50 10.74 → 10.44 (−2.8%) `noisy`; no `ecs` row `regressed` (`brain` 2.21 → 2.26 `noisy`). `churn` `flush` p50 −68.3% and p95 −67.6% `improved`. `particles`: `unattributed` `improved`, `animate_sprites` p95 0.35 → 0.37 `regressed`. `integrated` `total` `improved` | All eight equal the expected ones: these four rows in all five runs, and all eight in the final suite, which is the same state | kept |
| 8 E2 | done | `perf-runs/20261001T063651Z-suite` and its rerun `perf-runs/20261001T063726Z-suite` (`ecs`). Sweeps of `fragmentation` 1, 8, 64: before `perf-runs/20261001T063314Z-ecs-sweep-fragmentation`, after `perf-runs/20261001T063749Z-ecs-sweep-fragmentation` | Against step 7: `update` p50 10.51 → 10.38 (−1.3%) and, in the rerun, 10.44 (−0.7%), `unchanged` both times; p95 `unchanged`; `team_bags` `improved` in the first capture only; no `regressed` (`perf-runs/20261001T063713Z-compare-suite-2`, `perf-runs/20261001T063749Z-compare-suite`). Sweep, `update` p50 before → after: 9.14 → 9.13 at 64 archetypes, 10.52 → 10.37 at 488, 19.15 → 18.74 (−2.1%) at 3,189 | `perf-runs/20261001T063713Z-compare-suite`: no `regressed`; `update` p50 10.74 → 10.38 | `ecs` equals the baseline's in both captures | reverted: `unchanged` after one rerun |
| 9 close-out | done on 2026-10-01 by a close-out session (see the closing note). The final suite misses two final checks, and the owner accepted both readings as they read on 2026-10-01, with no capture taken and no code changed to move either label: `ecs` `stage.update` p50 `noisy` (recorded in `D-083`) and `particles` `animate_sprites` p95 `regressed` (an accepted regression, justified in `D-084`). Profiles, the full suite, `D-083` and `D-084` with their index rows, `DESIGN.md` §ECS, `CHANGELOG.md`, `docs/perf/benchmarks.md`, `status: done` and the archive move are done | Final suite `perf-runs/20261001T070637Z-suite` (all eight rows, valid, no encoder sighting). Earlier ones: `perf-runs/20261001T063945Z-suite` on step 7's first state, and `perf-runs/20261001T070121Z-suite`, which had sightings inside `churn` runs 4 and 5 and `gpu` run 1 and is not used. Profiles: `perf-runs/20261001T071448Z-ecs` (DWARF), `perf-runs/20261001T071540Z-ecs` (frame pointers), `perf-runs/20261001T071630Z-churn` (frame pointers), `perf-runs/20261001T071658Z-churn` (DWARF) | n/a | `perf-runs/20261001T071134Z-compare-suite`: `physics` `physics_step` p50 13.25 → 5.56 (−58.0%), p95 −58.1%, `update` p95 −57.9%; `physics-sparse` p50 8.05 → 3.42 (−57.6%), p95 −64.6%; `churn` `flush` p50 8.48 → 2.69 (−68.2%), p95 9.41 → 3.08 (−67.3%), no churn system row `regressed`; `ecs` `follow` p50 1.95 → 1.74 (−10.8%); `integrated` `total` p50 12.12 → 10.55 (−13.0%), p95 −11.7%, p99 −11.7%: all `improved`. Not met, and accepted by the owner on 2026-10-01: `ecs` `update` p50 10.74 → 10.46 (−2.6%) reads `noisy`, not `improved` or `unchanged`; `particles` `animate_sprites` p95 0.35 → 0.38 reads `regressed`. `gpu` and `gpu-throughput`: no `regressed`, against the baseline and against the starting tree's reference captures (`perf-runs/20261001T071856Z-compare-suite` and `-2`). Peak RSS `unchanged` in all eight rows | `ecs`, `churn`, `gpu`, `gpu-throughput` and `particles` equal the baseline's; the three physics-bearing ones equal step 3's; each row's five runs agree | n/a |

Values later steps need:

- Margin chosen in step 3: 0.5 px, written as `PAIR_MARGIN_SLOPS = 2.0` (2·`linear_slop`). Sweep results (sum of the two rows' `physics_step` p50, `physics` + sparse): 0 px 10.46 ms (6.33 + 4.13, `perf-runs/20261001T025724Z-suite`); 0.5 px 9.55 ms (5.93 + 3.62, `perf-runs/20261001T025942Z-suite`); 1 px 9.67 ms (5.99 + 3.68, `perf-runs/20261001T030610Z-suite`). The step capture, a second capture of the 0.5 px build, reads 9.53 ms (5.85 + 3.68).
- Digests after step 3: `physics` `86ffcabcdb15eed1`, `physics-sparse` `5899f9c8a69d79b1`, `integrated` `5f031f4d947964cb`. P4 leaves them unchanged. Step 4's `integrated` digest check is against `5f031f4d947964cb`.
- Decision IDs written: `D-080` (broadphase layout: P2 and P3), `D-081` (pair repair: P1), `D-082` (sleep table: P4). Session B's column-storage and command-buffer decisions take `D-083` and `D-084`.
- Session log folders: `perf-runs/20261001-physics-ecs-pass-session-a/` (session A: `load.log`, `guard.log`, each capture's console output and load check, and `checkpoints/` with the per-step file copies, patches and commit messages).
- Session A's commits, not yet made: `checkpoints/01-p2.patch`, `02-p3.patch`, `03-p1.patch`, `08-p4.patch` and `09-plan.patch`, each with a `.msg`, apply in that order on `a7cc105`; `checkpoints/commit-series.sh` commits them through the index. The working tree already holds their result, so session B starts from this tree whether or not they are committed.
- Session B started on `HEAD` `e27bdb8` ("Physics Improvement Session A") with a clean tree: session A's work is committed as one commit, so session B's patches apply on `e27bdb8`. Its folder is `perf-runs/20261001-physics-ecs-pass-session-b/`: `load.log`, `guard.log`, each capture's console output (`NN-<id>.out`) and load check (`.load.txt`), and `checkpoints/` with one full copy of the ECS file set per state (`00-base`, `04-c1`, `05-c3`, `06-c2`, `07-e1`, `08-e2`; `checkpoints/use-state.sh <state>` puts one into the working tree).
- GPU rows in step 9 are judged against the starting tree's captures of this session: `gpu` in `perf-runs/20261001T044916Z-suite` and `gpu-throughput` in `perf-runs/20261001T045216Z-suite`.

Notes (reverts and their reason, stops, anything the next session must know):

- Session A started on `HEAD` `a7cc105`, not `7276c8a`: the release-tooling work this plan calls foreign was committed there, `D-079` included. `git diff 7276c8a a7cc105` is empty for `crates/`, `examples/`, `assets/`, `Cargo.*` and `.cargo/`, and `cargo build` with the runner's flags compiled nothing, so the binary is still the baseline's. The only uncommitted file at the start was this plan. `DECISIONS.md`, `docs/DECISION_INDEX.md` and `CHANGELOG.md` carry no foreign changes any more; the next free decision ID is `D-080`.
- Captures in this session export `WGPU_BACKEND=vulkan`, as the baseline did (`provenance.wgpu_backend_env`).
- Step 1, first capture (`perf-runs/20261001T015602Z-suite`, compare `perf-runs/20261001T015757Z-compare-suite`): the sweep staged the statics-only grid on a frame's first sweep query. `physics-sparse` `physics_step` p50 8.05 → 6.69 (`improved`, p95 too) and `physics` `unchanged`, digests equal. `integrated` paid for it: `stage.total` p50 +0.30 ms and p95 +0.43 ms (`noisy`), and its `physics_step` p95 6.98 → 7.38 (`regressed`), because a frame with 3.8 sweep queries staged 8,528 tile proxies. Reworked before recapturing: the sweep keeps querying the pair grid until the frame's sweep queries reach the static count, then stages the statics-only grid (both grids return the same statics in the same order, pinned by `sweep_grid_returns_the_pair_grids_statics_in_order`). That first variant is kept in `checkpoints/01-p2-first-capture/`.
- `tests/physics_determinism.rs` and `physics_containment.rs` are `#[ignore]`d in debug builds, so `just check` does not run them. Run them with `RUSTFLAGS="-C force-frame-pointers=yes" cargo test --release -p tungsten-core --test physics_determinism --test physics_tunneling --test physics_containment` (the runner's flags, so the capture build's dependencies are reused).
- Step 2, first capture of the direct table (`perf-runs/20261001T024244Z-suite`): sparse and `integrated` improved, but `physics` `physics_step` p50 rose 13.13 → 13.52. A profile (`profiles/02b-physics.perf.data` in the session folder) showed why: the per-entry body shared by the two layouts' loops was a closure, and the compiler emitted it out of line (54.8% self time, one call per grid entry). It is now an `#[inline(always)]` function (`broadphase.rs`, `visit_entry`), and the recapture is the one in the table. The first variant is in `checkpoints/02b-direct-first-capture/`.
- Step 3, `integrated`: P1 does not move it, because its rebuilds are not budget trips. A count on an instrumented build of the 0.5 px state (`03-p1-rebuild-causes.txt` in the session folder; 400 frames) gives 1,403 builds from `pairs_invalidated`, which is one per frame after gather plus 1,003 after a contact wake (`D-075` rule b), against 37 repairs and no fall-back. The same count reads 1 build and 3.0 repairs per frame in `physics` (456 tripped proxies per frame) and 1 build and 2.1 repairs in sparse (350). The plan kept the rebuild on a wake, so this was left alone. Proposed next move, as its own task: re-pair a woken proxy the way a tripped one is re-paired (it becomes an initiator, so it needs its sleeping and static neighbours), which would remove about 2.5 of `integrated`'s 3.5 builds per frame. `integrated` still reads `improved` against the baseline, from P3.
- Step 3, sweep captures: a 7 s NoMachine session fell inside the 0 px capture (`integrated` runs 2 and 3) and a 3 s one inside the first 0.5 px capture (`integrated` run 5's GPU diagnostic run only). The `physics` and `physics-sparse` runs the sweep is judged on were clean in all three. The step capture is a second, clean capture of the 0.5 px build with the same three digests.
- Step 3, `D-075`'s gate (`cargo bench -p tungsten-core --bench physics_bench`, with `CARGO_TARGET_DIR=target/criterion-native` so the capture build is left alone; outputs `03-bench-before.out`, `03-bench-after.out` and `03-bench-after-rerun.out`): `projectile_stream` 1.94 → 1.17 ms at 3,000, 6.67 → 3.01 ms at 10,000 and 21.4 → 11.5 ms at 25,000; `pile_plus_bullet` 0.77 → 0.69 ms at 10,000 (no change detected) and 36.1 → 28.1 ms at 25,000. Nothing slowed.
- Final suite, `gpu`: `stage.extract` p95 reads `regressed` against the baseline (2.30 → 2.52 ms). The machine drifted, not the code: the untouched tree, captured right after that suite (`perf-runs/20261001T032411Z-suite`), reads `regressed` against the baseline on the same metric (p50 1.95 → 2.05, p95 2.30 → 2.48), and the final suite reads no `regressed` against that capture (`perf-runs/20261001T032707Z-compare-suite`). The physics code does not run in that row. Session B should expect the same when it compares `gpu` with `pre-physics-ecs-pass`, and can judge it against `perf-runs/20261001T032411Z-suite` instead.
- Close-out checks on the final tree: `just check`, `just smoke` (15/15 benchmark rows, every example and fixture row OK) and `just repo-check` pass, and so do the three release-scale tests named above. Outputs are in the session folder (`09-*.out`).
- What leads each physics row now (inclusive shares of all samples, `profiles/*.inclusive.txt` in the session folder):
  - `physics` (`physics_step` 92.4%): the one pair build per frame 37.4% (its grid query 35.1%), narrow phase 13.0%, `solve_contacts` 8.8%, `repair_pairs` 7.4%, `apply_restitution` 2.6%, grid build and insert 4.2%, `ImpulseMap::get` 2.3%, `sleep_frame_end` 1.7%.
  - `physics-sparse` (`physics_step` 89.4%): the pair build 38.1% (query 34.2%), the safety-net sweep 30.7%, of which `SpatialGrid::query` is 23.7% and `cell_range` 9.8% (four long walls do not fill compact bounds, so the statics-only grid uses the hashed table), `repair_pairs` 4.4%, grid build and insert 7.0%, narrow phase 3.9%, solver 1.7%, `collect_tripped` 1.2%.
- For session B's pass over `docs/perf/benchmarks.md`: session A changed only the two digest lines. These are stale after the physics changes: the `physics` calibrated-defaults table (13.37 / 13.99 ms and 8.06 / 9.83 ms) and the profile split under it (`build_pairs` 54.5%, `speculative_pass` 29.9%); `integrated`'s stage means (`physics_step` 6.66 ms), its `actors` sweep and `tile_collision=off` numbers, and its counters (6,677 pairs is now 7,034, 6,282 contacts 6,319, 951 sleeping crates 929); the "Tile collision proxies" finding (2.3 ms).
- A NoMachine client was connected from 02:00:55Z, so no capture could run. The code for steps 2, 3 and P4 was written and unit-tested ahead in that time, one state per folder under `checkpoints/` (`02a-floor`, `02b-direct`, `02c-flags`, `03-p1-margin2`, `08-p4`); `checkpoints/use-state.sh <state>` puts a state into the working tree. Each is still captured and judged in plan order, on its own state.
- Session B, before step 0: a NoMachine client was connected from the session's start (04:10Z) to 04:44:46Z. No file in the working tree was edited in that time, so the preflight and the GPU reference ran on the untouched tree. The code for steps 4 to 7 and the E2 prototype was written and unit-tested in a copy of the workspace outside the repository, one state per folder under the session's `checkpoints/`. Two scratch-only probes ran on those states and are in the session folder, in no patch: `order-probe/` hashes what every query kind yields after 16,000 immediate and 16,000 deferred structural commands and reads the same value on all six states; `churn-probe/` replays the `churn` default frame headless and counts 97.4 million instructions per frame on the base state (the baseline binary measured about 100 million), 49.5 after C1, 40.4 after C3 and 26.0 after C2.
- Step 0, GPU reference: `gpu-throughput` was captured three times. A 4 s NoMachine session fell inside timing run 2 of `perf-runs/20261001T044617Z-suite` and a 13 s one inside run 1 of `perf-runs/20261001T044916Z-suite`; the third capture, `perf-runs/20261001T045216Z-suite`, is clean. `gpu` was clean in the first two; an A/A of them reads 0 regressed, 0 improved, 13 unchanged, 3 noisy (`perf-runs/20261001T045414Z-compare-suite`).
- `docs/plans/debug-cleanup-docs-pass.md` was still `draft` before step 4, so C1 lands first and leaves `AnyColumn::len`, `AnyColumn::type_id` and `Archetype::id` as they were. None of them became dead through this plan: `len` is still read by one test, the other two were unread before.
- Step 4 took four variants. Each first capture contradicted an expectation, so each was followed by a profile or a disassembly before the next variant; the three dropped ones are in `checkpoints/04-c1-first-capture/`, `-second-capture/` and `-third-capture/`.
  1. Every lookup by type scans the sorted key, as C1 and E1 are written above (`perf-runs/20261001T045907Z-suite`). `flush` p50 8.58 → 3.82, but `follow` 1.91 → 2.84 ms and `update` p50 10.74 → 11.59, `regressed`. The profile (`perf-runs/20261001T050157Z-ecs`) shows `follow` still waiting on the same two loads, 29.6% on the entity metadata and 43.4% on the leader's `Position`, with the scan between them. A branch-miss recording (`profiles/` in the session folder) counts 0.88 mispredictions per lookup in `follow`, against 0.06 on the base build: the scan leaves the key at an iteration that depends on the archetype, and that branch resolves only after the metadata miss, so the next lookup's miss no longer starts early. E1's premise, a scan in place of the map probe, is therefore wrong for random access.
  2. A `TypeId` → index map answers every lookup by type (`perf-runs/20261001T050832Z-suite`). `follow` 1.92, but `cooldowns` +0.03, `team_bags` +0.02 and `faction_histogram` +0.03 ms against the preflight, `regressed`: a query's per-archetype setup now reads the map and the column list, one cache line more than before.
  3. Queries resolve their columns in one pass over the key and only `World::get` probes the map (`perf-runs/20261001T052110Z-suite`). Setup-bound rows improved, but `follow` read +0.07 ms against the baseline (`Archetypes::get` out of line and calling the probe out of line) and `particles` `unattributed` 2.37 → 2.47, both `regressed`: the new iterator shape changed how the particle systems' row loops compiled.
  4. Kept (`checkpoints/04-c1/`): the original query shapes. A one-type query finds its column in the scan that filters the archetype; the scan (`Archetype::column_index`) is `#[inline(never)]`; `World::get` and `get_mut` probe the map through `Archetype::probe_column_index`.
- Step 4, the 0.02–0.05 ms shifts of the small `ecs` system rows between variants are mostly row-loop code generation, not setup work. With the key scan inlined beside it, `bounds_wrap`'s row loop rebuilt its zero constant on every row (+0.05 ms in variant 1, and again in a build of variant 4 without the attribute); with the scan out of line the loop hoists its constants as the baseline's does. The disassemblies are from scratch builds of each variant.
- Step 4, `churn_spawn` reads +0.02 ms against the baseline in every variant (0.35 → 0.37, on the 0.02 ms floor, `regressed` in the kept one). That system only records commands, and step 4 does not touch recording. What changed is the allocator's state: `flush` no longer allocates and frees 250,000 boxes a frame, so the 43,750 command boxes `churn_spawn` and its siblings allocate come from colder memory. C2 (step 6) removes those boxes; the row is checked again there.
- Step 4, quick checks (`ecs_bench`, native flags, `04-bench-before.out` and `04-bench-after.out`): `spawn_insert_3_components_10k` 1.72 → 1.47 ms, `spawn_despawn_1k` 46.0 → 32.7 µs, `command_buffer_flush_1k_spawns` 127.6 → 100.4 µs. The three release-scale physics tests pass on the step's state.
- Step 5, `particles`: every stage of the row read about 5% slower on the C3 state, the default extract included (7.03 → 7.5 ms), which C3 does not touch. It is reproducible (the five capture runs, a profile run and three more runs of a variant) and it is not flush work: `flush` in that row stayed at 0.56 ms. Profiles of the row on both states (`perf-runs/20261001T054347Z-particles` for step 4, `perf-runs/20261001T054207Z-particles` for step 5) show fewer instructions on C3 (33.98 → 33.57 billion) but more cycles (21.47 → 22.83 billion) and more L1 data misses (969 → 1,033 million); a recording of the misses by symbol (`profiles/particles-l1miss-*.perf.data`) puts the increase in the extract's sort and the particle tick, which walk the same sprite columns. So the data is laid out less favourably, with the same rows in the same order. One suspect was ruled out: writing a run's values in column order, the order one-by-one inserts push them in, changed nothing (`05-exp-column-order-particles.out`, `checkpoints/05-c3-exp-column-order/`). Transparent huge pages are `always` on this machine, so where a multi-megabyte column lands decides how it is mapped; that is the remaining suspect and was not tested. Step 6 changes the allocation pattern again and rechecks the row.
- Step 5, quick checks against the base (`05-bench-after.out`): `spawn_despawn_1k` 31.9 µs, `spawn_insert_3_components_10k` 1.52 ms and `command_buffer_flush_1k_spawns` 105.1 µs, against 1.47 ms and 100.4 µs after step 4. That last bench flushes runs of two inserts, where the run bookkeeping on boxed commands costs more than the one saved move; `churn` flushes runs of six.
- Step 6, `particles`: `unattributed` is below the baseline again (2.37 → 2.22 ms, `improved`), because recording a particle no longer boxes four commands. `animate_sprites` p95 stays at 0.38 ms (0.35 at the baseline, `regressed`; p50 `unchanged`), and the reported-only extract reads 7.7 ms against 7.03. What was checked after step 5's note, all on scratch builds of steps 4 and 5:
  - The allocation traffic is the same: 5,398.6 `malloc` calls per frame after step 4 and 5,399.4 after step 5 (`counts/particles-alloc-*`, exact counts), one scratch buffer per flush more.
  - The extract's collect loop is byte-identical and at the same alignment in both builds, and the particle sprites' asset-id strings are equally scattered (0.2% of consecutive rows share a page in steps 4, 5 and 6).
  - Disabling transparent huge pages for the process changes nothing, which clears that suspect.
  - In cycle profiles (`profiles/particles-cycles-*.perf.data`) the step 5 build spends more only in `malloc` and `free` (+20%), in the string compare (+12%) and in the extract's batching loop; the sort is unchanged.
  - Without telemetry logging both builds take the same cycles (21.0–21.5 billion over 480 frames in four `perf stat` runs); with it, as in a capture, step 5 is slower in every run.

  So the row's time follows the allocator's state under its own traffic (a `String` clone per animation frame change, a `String` per particle, the extract's per-frame buffers, the telemetry's formatting), and that state shifts when any allocation is added or removed elsewhere. It is not work the ECS does, and it was not fixed here. The engine finding "String sprite IDs" is the lever: interned IDs would take the strings out of all three stages.
- Step 6, GPU rows on the step's state (`perf-runs/20261001T061342Z-suite`, five runs): no `regressed` against the baseline (`perf-runs/20261001T061624Z-compare-suite`) or against the starting tree's reference captures.
- Step 6, quick checks against the base (`06-bench-after.out`, `06-bench-reused.out`): `command_buffer_flush_1k_spawns` 127.6 → 66.8 µs (105.1 after step 5), the new `command_buffer_reused_flush_1k_spawns` 62.5 µs, `spawn_insert_3_components_10k` 1.52 ms, `spawn_despawn_1k` 33.1 µs.
- Step 7, what E1 became. The plan's E1 is a scan of the type key plus `#[inline]`; step 4 showed the scan is wrong behind a cache miss. E1 as built keeps the plan's shape (find the column, downcast it once, index the row, all inlined into the caller) and changes how the column is found. It was worked out on a headless replica of the benchmark's population and `follow` system (`follow-probe/` in the session folder, in no patch), which reproduces the benchmark's ordering of the variants: base 1.34 ms, step 4's scan 2.84, step 6 1.39.
  - Step 6 plus `#[inline]` and the typed downcast, still probing the hash index: 1.38 ms, 1% better. One iteration of `follow` is about 160 instructions with either lookup, so an iteration is too long for a third lookup to overlap; the plan's ceiling (60 instructions, three or four lookups in flight) is out of reach from the engine's side.
  - A slot table in place of the hash index, with the type key compared afterwards: 1.27 ms.
  - The same without the compare, leaving the check to the downcast the lookup does anyway: 1.17 ms, 16% better than step 6. This is what step 7 captures.

  What moved the number is the loads between the two misses, not the instruction count: each archetype's hash index is its own allocation (two cache lines per lookup, outside L2 with 488 archetypes), and the key compare reads a third. The slot table sits inside the archetype.
- Step 7, `brain` reads `regressed` against step 6 (2.18 → 2.26 ms) and `noisy` against the baseline (2.21). E1 does not touch `query3_mut` or the benchmark's state machine. The row has read 2.17 to 2.26 across this session's builds, 2.25 already in the preflight on the untouched tree, and each capture's five runs agree to 0.01 ms: it moves with the build, not with load. No fix was attempted.
- Step 8, what the E2 prototype was. With step 7's slot table in every archetype, the cheaper setup is to let the slot name a type's column and the key confirm it, for `has` as well: one key entry read where a query's per-archetype setup and a structural change scanned the key, and no 64-bit mask, because an absent type already costs one probe. It is `checkpoints/08-e2/` and `08-e2-reverted.patch` (two files). The gain is real and small: 0.07 to 0.13 ms of `update` at the default, against a threshold of 0.32 ms, and 0.41 ms of 19.15 at 3,189 archetypes. So setup work is not what the 488 archetypes cost; the rest is the cold first lines of each archetype's columns (H17), which this pass does not reach. A mask-and-index version written before step 4 is in `checkpoints/08-e2-mask-prototype/`; it was never captured as E2, and its query shape is the one step 4's third variant measured.
- Step 7 was amended after the first full suite. That suite (`perf-runs/20261001T063945Z-suite`, on the state first captured for step 7) read `churn` `flush` p50 at 3.12 ms, against 2.68 at step 6, a row step 7's plan did not capture. Back-to-back captures confirmed it (`perf-runs/20261001T064648Z-suite` on step 6: 2.65 to 2.81; `perf-runs/20261001T064732Z-suite` on step 7: 2.95 to 3.13). E1 does not touch the flush, but it changes `Archetype`'s layout and the column vtable, and the flush compiled differently: a profile of each state (`perf-runs/20261001T065049Z-churn`, `perf-runs/20261001T064755Z-churn`) puts 25% of `flush_reusing`'s own time on the return from the per-value `write_next` call in step 7's build, about 25 cycles per inserted value, where step 6's build shows none. Branch mispredictions are equal in both (`profiles/churn-brmiss-*.perf.data`), and the headless churn replica does not show the difference at all, so the cause below the call was not found. What removed it: the write loop of `insert_run` now finds each destination column through the slot table in place of the out-of-line key scan, which is less work per value in any case (`checkpoints/07-e1/`; the state first captured is `checkpoints/07-e1-first-capture/`). `flush` p50 reads 2.69 on the amended state. E2 had been judged on the first state; its change is to queries and to `column_index`, which the amendment leaves alone.
- Step 9, stopped. The final suite misses two of the final checks, in three clean captures of the last two states:
  - `ecs` `stage.update` p50 reads `noisy`: 10.74 → 10.46 ms, −0.28 with an interval of −0.37 to −0.20 against a threshold of 0.32. It read −0.26 (`perf-runs/20261001T063945Z-suite`), −0.30 (`perf-runs/20261001T065855Z-suite`) and −0.28. The ECS steps take a little under 3% off `update`, so the interval reaches past the threshold (not `unchanged`) while the mean stays short of it (not `improved`). More runs would narrow the interval to `unchanged`; the plan allows no third run.
  - `particles` `animate_sprites` p95 reads `regressed`: 0.35 → 0.38 ms (0.34 to 0.36 per run at step 4, 0.36 to 0.41 from step 5 on; p50 `unchanged`). The row's other owned metric, `unattributed`, improved by 0.26 ms. Step 6's note has what was checked; the mechanism behind this one metric was not found.

  Every other final check holds. Proposed next move, for the owner to decide: accept both readings and close the plan out (decisions `D-083` and `D-084`, `DESIGN.md` §ECS, `CHANGELOG.md`, `docs/perf/benchmarks.md`, then `status: done` and the archive move), with the `animate_sprites` reading recorded as accepted. Drafts of the two decisions, the index rows, the `DESIGN.md` paragraphs and the changelog entry are in the session folder (`checkpoints/docs-draft/`), not in the tree.
- Step 9, what leads each ECS row now (inclusive shares of all samples, frame-pointer profiles; tables in `profiles/09-*.txt` of the session folder):
  - `ecs` (`stage_update` 87.2%; startup is 8.3% of the profile run, 17.6% before): `heading` 19.7% (`atan2f` 15.0%) and `brain` 18.5%, both benchmark work; `follow` 16.5%, of which `Archetypes::get::<Position>` is 4.9% and the rest waits on the two loads; `buffs` 5.7%, `cooldowns` 4.3%, `age_phase` 4.0%, `tint` 3.3%; the other six systems 1.4 to 2.2% each; `Archetype::column_index`, the query setup scan, 1.1%.
  - `churn` (`flush_reusing` 68.9%, 22.4% of it its own; no `malloc` or `free` in the table): `insert_run` 39.8%, made of `move_row` 24.4% (the toggles' row moves: `move_components_to` 18.2%, waiting on the moved row in the first column), `write_next` 6.1%, the edge walk (`add_edge`) 5.9% and `has` 2.8%; `swap_remove_drop` on the first column 6.7% and `World::despawn` 4.3% (despawns); `Archetypes::remove` 7.4% (status losses); `churn_scan` 10.5% and `churn_spawn` 8.0% (the benchmark's own systems). The run is 9.76 billion instructions over 420 frames, where the baseline's frame alone was about 100 million. Exact counts on the final state (`counts/churn-alloc-final-*` in the session folder): 208 `malloc`, 220 `free` and 8 `realloc` calls per frame, against 293,960, 293,972 and 12,522.
- Closing note (2026-10-01, a third session that did only the paperwork of step 9, from 07:48Z; its folder is `perf-runs/20261001-physics-ecs-pass-closeout/`, with a README).
  - The owner's decisions, which settle the stop above:
    1. Both missed final checks are accepted as they read. No capture was taken and no code changed to move either label. `D-083` records the `ecs` `update` p50 reading, and `D-084` carries the written justification the regression policy asks for the `particles` `animate_sprites` p95 regression.
    2. The knob captures for `docs/perf/benchmarks.md` were wanted, not required. They were taken (below).
    3. No source file changed: the tree is `19c24ee` plus documents, so the final suite still describes the binary. The `components` knob note "one archetype move each" (`examples/02_bench/src/churn.rs` and the knob table) is true only for `mode=immediate` now; the note and the table row were left alone, and the prose of the `churn` section says so.
    4. Session A's work stays as written. `D-080` to `D-082` cite this plan at the path it had before the archive move.
    5. Nothing a later session needs lives only in this file. `D-083`, `D-084` and `docs/perf/benchmarks.md` carry the two accepted readings, the rows now below their calibration band, the reported-only extract readings, what the E2 prototype showed about many archetypes, the contact-wake rebuilds of `integrated` and the open items ("Engine findings" and "Open proposals").
    6. No release work, and no open item was started.
  - Preflight: `HEAD` `19c24ee` on `0.33`, clean tree, and the ten files of the session B folder's `checkpoints/files.txt` equal `checkpoints/07-e1/`, so the final suite measured the sources at `HEAD`. The suite was not captured again.
  - Knob captures, on the unchanged binary, three runs each, through `guard.sh` with the load log running; no encoder sighting in any. Each table's captures were taken in one sitting, and each sitting twice: during the first take the session ran shell commands beside the captures, and the tails read fatter than the final suite's (`churn` `flush` p95 3.14 to 3.46 ms per run against 2.99 to 3.15; `integrated` jitter 0.8 to 2.6 ms against 0.7 to 0.9), so both sittings were redone with nothing else running. The tables use the second take, which reads 3.07 to 3.12 ms and 0.7 to 1.4 ms on the same two metrics.
    - `churn`: default `perf-runs/20261001T075651Z-churn`, `mode=immediate` `perf-runs/20261001T075656Z-churn-setaa3d97`, `toggles=0` `perf-runs/20261001T075701Z-churn-setbf6388`. First take, not used: `perf-runs/20261001T075103Z-churn`, `…T075108Z-churn-setaa3d97`, `…T075113Z-churn-setbf6388`.
    - `integrated`: default `perf-runs/20261001T075709Z-integrated`, the `actors` sweep `perf-runs/20261001T075746Z-integrated-sweep-actors`, `tile_collision=off` `perf-runs/20261001T080049Z-integrated-setc49aa7`. First take, not used: `perf-runs/20261001T075117Z-integrated`, `…T075154Z-integrated-sweep-actors`, `…T075458Z-integrated-setc49aa7`.
    - Readings: the `CommandBuffer` now costs 0.23 ms per `churn` frame over the direct calls (structural totals 3.52 and 3.29 ms; 2.52 ms before the pass); `integrated` gains about 1.1 ms of `total` p95 per 500 walkers; merged boxes in place of the tile proxies save 1.9 ms of `physics_step` (2.3 before).
  - Written: `D-083` and `D-084` in `DECISIONS.md`, with `D-036` and `D-039` marked amended, and their rows in `docs/DECISION_INDEX.md`; `DESIGN.md` §ECS (storage, queries, command buffer); the `CHANGELOG.md` `[Unreleased]` entry for the whole pass, `D-080` to `D-084`; `docs/perf/benchmarks.md` (dated numbers for the six rows the pass moved, the knob tables, findings and open items); the hotspot line in `docs/perf/profiling-workflow.md`.
  - Found while checking every row's section against the final suite, and recorded in `docs/perf/benchmarks.md`:
    - `physics` and `physics-sparse` run below the 8–16 ms calibration band as `churn` does (`total` p95 6.07, 3.73 and 4.06 ms).
    - `integrated`'s default extract reads `regressed` against the baseline (p50 2.61 → 2.80 ms), reported only like the `particles` one. It moved at step 6 (2.60 → 2.85), where the `particles` extract moved at step 5 and again at step 6.
    - `ecs` has a per-run mode: in 5 of the 40 runs of eight captures, the untouched tree's included, `stats_decay` reads 0.31–0.32 ms instead of 0.15–0.16 and `follow` about 0.2 ms less. Two of the final suite's five runs are in it.
  - `just check`, `just smoke` and `just repo-check` run on the closed-out tree after this file is archived, so their results are not here: they are `07-*.out` in the close-out folder and in its README.
  - The close-out is one patch on `19c24ee` with a one-line commit message, both in the close-out folder; the owner commits.

## Measured data

| Field | Value |
| --- | --- |
| Baseline name | `pre-physics-ecs-pass` (in `just perf baseline list`: suite, 8 rows, valid) |
| Capture directory | `perf-runs/20261001T002643Z-suite/`, one capture per row under `<row>/` |
| Commit | `7276c8a`, dirty `yes (diff 72977ae30637)`. The dirty files are 15 release-tooling and documentation files another session was editing. Nothing under `crates/`, `examples/`, `assets/`, `Cargo.*` or `.cargo/` differs from `7276c8a`, so the binary is that commit's |
| Machine | Ryzen 5 6600H, Radeon 660M (RADV REMBRANDT), Vulkan, `immediate` present mode, latency 1; Linux 7.2.7-arch1-1; rustc 1.98.1; governor `performance`; fingerprint `f3b1bfed09f8` |
| Build | release, `RUSTFLAGS="-C force-frame-pointers=yes"` (generic x86-64, the runner's default) |
| Runs | 5 per row, 300 measured frames after each benchmark's warm-up |

All values below are medians of the five per-run values, in ms.

### `physics`

| Metric | p50 | p95 |
| --- | --- | --- |
| `system.physics_step` (owned) | 13.26 | 13.64 |
| `stage.update` (owned at p95) | 13.31 | 13.73 |
| `stage.total` | 13.61 | 14.08 |
| `stage.render` | 0.24 | 0.32 |
| `stage.extract` | 0.05 | 0.06 |

- Other systems at p50: `pachinko_events` 0.03, `bench_counters` 0.01, `stall_guard` 0.01.
- Counters: 8,521 proxies (8,000 dynamic), 0 sleeping, 15,150 pairs and 8,861 contacts in the final substep, 13,488 collision events per frame.
- Peak RSS 132.1 MiB, growth 2–6 KiB/s. Digest `c972b2818d486617`.

### `physics-sparse`

| Metric | p50 | p95 |
| --- | --- | --- |
| `system.physics_step` (owned) | 8.06 | 9.82 |
| `stage.update` | 8.09 | 9.85 |
| `stage.total` | 8.31 | 10.07 |
| `stage.render` | 0.17 | 0.22 |
| `stage.extract` | 0.04 | 0.05 |

- Other systems: `renormalize_speeds` 0.02.
- Counters: 8,004 proxies (8,000 dynamic, 4 static walls), 0 sleeping, 1,831 pairs, 942 contacts, 269 events per frame.
- The p95 is 22% above the p50 because about one frame in five builds the pair list four times instead of three: in run 1, 79% of the frames take 7.5–8.5 ms and 20% take 9.5–10.5 ms.
- Peak RSS 128.1 MiB, no growth. Digest `c95fadc938a3689e`.

### `ecs`

| Metric | p50 | p95 |
| --- | --- | --- |
| `stage.update` (owned) | 10.75 | 11.14 |
| `stage.total` | 11.24 | 11.62 |
| `stage.render` | 0.36 | 0.41 |
| `stage.extract` | 0.08 | 0.09 |

- Owned system rows at p50: `heading` 2.55, `brain` 2.21, `follow` 1.93, `buffs` 0.65, `age_phase` 0.47, `cooldowns` 0.44, `tint` 0.41, `bounds_wrap` 0.28, `integrate` 0.27, `accelerate` 0.26, `team_bags` 0.24, `faction_histogram` 0.24, `stats_decay` 0.16, `regen` 0.09.
- Not owned: `bench_counters` 0.39 (the digest pass).
- Counters: 250,000 entities, 488 archetypes, 24,967 lookups per frame, 0 structural changes.
- Peak RSS 162.9 MiB, no growth. Digest `c785506ced04de4c`.

### `churn`

| Metric | p50 | p95 |
| --- | --- | --- |
| `stage.flush` (owned) | 8.50 | 9.40 |
| `stage.update` | 0.89 | 1.00 |
| `stage.total` | 9.66 | 10.61 |
| `stage.render` | 0.27 | 0.32 |

- Owned system rows at p50: `churn_scan` 0.41, `churn_spawn` 0.35, `churn_toggle` 0.09, `churn_despawn` 0.02.
- Counters per frame: 6,250 spawns, 6,250 despawns, 40,625 inserts, 3,125 removals, 56,250 commands; population 125,000.
- Peak RSS 143.6 MiB, no growth. Digest `86014148b7a94681`.

### Reported only

| Row | Owned metrics, p50 / p95 | `total` p50 / p95 / p99 | Peak RSS | Digest |
| --- | --- | --- | --- | --- |
| `gpu` | scene pass 7.52 / 8.16; `render_span` 11.18 / 11.81; `extract` 1.95 / 2.27; `render_encode` 2.68 / 3.06 | 11.08 / 12.74 / 14.72 | 732.3 | `f45f26ebdf706840` |
| `gpu-throughput` | `extract` 24.22 / 24.82; `render_encode` 1.45 / 1.59 | 26.14 / 26.74 / 27.15 | 251.8 | `121f8ef1503d8697` |
| `particles` | `unattributed` 2.36 / 2.92; `animate_sprites` 0.30 / 0.35 | 10.87 / 11.66 / 12.17 | 143.7 | `08a3c7c126a0d43a` |
| `integrated` | `total` 12.12 / 12.54 / 12.76 (p99); jitter 0.68 | 12.12 / 12.54 / 12.76 | 280.8 | `9b2617e4c1ab23f7` |

`integrated` runs the same physics code: `physics_step` p50 6.76 of `update` 7.54, with 13,044 proxies (8,528 of them tile proxies), 951 sleepers, 6,677 pairs and 6,282 contacts. `flush` is 0.17 there and 0.70 in `particles`.

### A/A check

`perf-runs/20261001T003527Z-compare-suite/compare.md` compares the baseline with a second capture of the four owned rows (`perf-runs/20261001T003355Z-suite/`): 0 regressed, 0 improved, 24 unchanged, 3 noisy. The noisy rows are `follow` (−1.0%) and `churn_scan` and `churn_spawn`, which sit on the 0.02 ms floor. No owned stage metric moved by more than 1.5% (`ecs` `update` p95), `physics_step` p50 moved by +0.4% and +0.1%, and no system row moved by more than 0.02 ms.

### Knob experiments on the unmodified build

Three runs each; values are p50 medians.

| Experiment | Capture | Result |
| --- | --- | --- |
| `physics`, `sleep=off` | `perf-runs/20261001T003528Z-physics-set03cbc7/` | `physics_step` 12.84: sleep bookkeeping costs 0.42 ms (3.2%) with no sleeper. Digest unchanged |
| `physics-sparse`, `sleep=off` | `perf-runs/20261001T003545Z-physics-sparse-set03cbc7/` | 7.79: 0.27 ms (3.3%) |
| `physics`, `cell` 16, 24, 32, 48 | `perf-runs/20261001T004006Z-physics-sweep-cell/` | 14.82, 13.79, 13.29, 15.63: 32 px is already the best cell size |
| `physics-sparse`, `cell` 16, 32, 64 | `perf-runs/20261001T004123Z-physics-sparse-sweep-cell/` | 10.29, 8.10, 7.98: nothing to gain |
| `churn`, `mode=immediate` | `perf-runs/20261001T003557Z-churn-setaa3d97/` | systems 0.33 + 1.29 + 2.05 + 3.03 = 6.70, against 9.37 deferred (`flush` 8.50 plus systems 0.87): the command buffer costs 2.67 ms, 47 ns per command |
| `churn`, `toggles=0` | `perf-runs/20261001T003606Z-churn-setbf6388/` | `flush` 4.55 |
| `churn`, `components` 2, 6, 12 | `perf-runs/20261001T003614Z-churn-sweep-components/` | `flush` 1.99, 8.37, 28.86 |
| `ecs`, `followers` 0, 0.1, 0.3 | `perf-runs/20261001T004221Z-ecs-sweep-followers/` | `follow` 0.01, 1.91, 4.49 for 0, 24,967 and 74,924 lookups: 52–76 ns per lookup |
| `ecs`, `fragmentation` 1, 8, 64, 256 | `perf-runs/20261001T003752Z-ecs-sweep-fragmentation/` | 64, 488, 3,189 and 10,252 archetypes give `update` 9.22, 10.72, 21.31 and 61.27 with the same 250,000 entities |

### Exact call counts

Counted with hardware execute breakpoints (`perf stat -e mem:ADDR:x`, ASLR off) on the same binary. The workload is deterministic, so these are exact. Outputs are in `perf-runs/20261001-physics-ecs-pass-evidence/counts/`.

| Row | Count | Per frame |
| --- | --- | --- |
| `physics` | `SpatialGrid::build` 1,677 in 420 frames | 3.99 pair builds |
| `physics` | `SpatialGrid::query` 13,848 | 33 sweep queries |
| `physics-sparse` | `SpatialGrid::build` 1,342 | 3.20 pair builds |
| `physics-sparse` | `SpatialGrid::query` 13,237,348 | 31,517 sweep queries: 98.5% of the bodies in every substep |
| `integrated` | `SpatialGrid::build` 1,199 in frames 120–420 | 4.00 pair builds; 3.8 sweep queries |
| `churn` | `malloc` 12,115,980 at 30 frames and 29,753,576 at 90 | 293,960 `malloc`, 293,972 `free`, 12,522 `realloc` |

`sync_impulses` ran once per pair build plus once per frame in every row, as the code says.

### Pair-budget replay

`dump_substeps.py` read the proxy array from the unmodified binary at the start of every substep for frames 150–209, and `analyze_budgets.py` replayed the budget rule over those snapshots. The replay reproduces the binary's build counts (4.00 and 3.18 against the counted 3.99 and 3.20) and its final-substep pair counts (15,117 and 1,837 against the telemetry's 15,150 and 1,831). Full output: `tables/budget-replay-physics.txt` and `tables/budget-replay-physics-sparse.txt` in the evidence folder.

Proxies that break their budget, read from the engine's own budget arrays at the start of each substep (mean; share of substeps with at least one):

| Row | Substep 1 | Substep 2 | Substep 3 |
| --- | --- | --- | --- |
| `physics` | 31 (100%) | 83 (100%) | 1,187 (100%) |
| `physics-sparse` | 0.2 (18%) | 53 (100%) | 3,970 (100%) |

Replay of one build at substep 0, by radius formula. "Builds" is under today's rule (any trip rebuilds everything). "Trips" is the mean number of proxies that trip per later substep when only those proxies are re-inflated. "Pairs" is the list size from the build at substep 0.

| Formula | `physics` builds | `physics` trips | `physics` pairs | sparse builds | sparse trips | sparse pairs |
| --- | --- | --- | --- | --- | --- | --- |
| Today's | 4.00 | 1,499 | 33,551 | 3.18 | 1,328 | 14,910 |
| Exact gravity term, no margin | 4.00 | 473 | 33,680 | 3.18 | 134 | 14,934 |
| Exact, margin 0.5 px | 4.00 | 155 | 36,756 | 3.15 | 115 | 15,801 |
| Exact, margin 1 px | 3.92 | 65 | 39,955 | 3.05 | 97 | 16,687 |
| Exact, margin 2 px | 2.75 | 14 | 46,800 | 3.00 | 67 | 18,536 |
| Exact, margin 4 px | 1.87 | 0.9 | 61,942 | 2.88 | 31 | 22,549 |
| Exact, margin 8 px | 1.00 | 0 | 98,958 | 2.00 | 5 | 31,746 |

Trips grow through the frame, because deviations from the predicted travel add up. With the exact term and a 0.5 px margin `physics` sees 10, 58 and 398 trips in substeps 1, 2 and 3, and sparse 0.1, 47 and 298 (`tables/budget-replay-by-substep.txt`).

Today the four per-substep builds admit 33,551, 26,556, 20,411 and 15,117 pairs in `physics` (mean 23,909) and 14,910, 9,197, 4,839 and 1,837 in sparse (mean 7,696), because each later build inflates for less remaining time.

### Capture hygiene

- The first suite of this session, `perf-runs/20261001T001042Z-suite/`, is contaminated: another agent session ran `cargo` at 00:15:16Z, and `particles` run 3 shows a `total` p95 of 41.9 ms against about 12.5. `particles` and `integrated` read 3–9% high throughout. Every guard passed and the digests matched. It is not the baseline.
- From then on a per-second load log ran beside every capture. During the baseline no run saw more than 1.61 busy CPUs in any second (the CPU-bound rows averaged 1.12–1.30), and no encoder ran (`tables/load-baseline-suite.txt`).
- One sweep overlapped a 5 s NoMachine session and was redone; the overlapped capture is named in the evidence README.
- For the implementation sessions: the hygiene scripts are in `perf-runs/20261001-physics-ecs-pass-evidence/scripts/`, not in the repository's `scripts/`. `perf-runs/` is gitignored and listed in `.ignore`, so the scripts exist only on this machine and `rg` and `fd` skip them; open them by path. Start `loadlog.sh <log>` before the first capture, run each capture as `guard.sh <log> <command…>`, check it with `capture_load.py <load.log> <capture-or-suite>…`, and keep other agent sessions idle while a capture runs. The commands are under "Ordered steps".

## Hotspots

Shares are inclusive percentages of all samples in the named profile. `step.rs`, `broadphase.rs` and `collision.rs` are under `crates/tungsten-core/src/physics/`; `archetype.rs`, `storage.rs`, `world.rs`, `entity.rs` and `command_buffer.rs` are under `crates/tungsten-core/src/ecs/`.

### `physics`: `perf-runs/20261001T001710Z-physics/profile/` (24,408 samples)

`physics_step` holds 94.5% of the samples. `perf stat`: 1.5 instructions per cycle, 7.2% L1 data misses, 4.1% branch misses.

| ID | Symbol | Location | Share |
| --- | --- | --- | --- |
| H1 | `build_pairs`, run 3.99 times per frame | `step.rs:804` | 70.1% |
| H1a | `SpatialGrid::for_each_in`, the pair query | `broadphase.rs:132` | 66.0% |
| H1b | the candidate closure: flags of `proxies[b]`, inflated-AABB overlap, pair push | `step.rs:846` | 25.7% |
| H1c | exact-cell compare per grid entry (`IVec2::eq`) | `broadphase.rs:160` | 7.1% |
| H1d | `SpatialGrid::build` | `broadphase.rs:181` | 4.9% |
| H1e | `SpatialGrid::cell_range`, of which `floorf` is 2.4% | `broadphase.rs:228` | 3.2% |
| H2 | `narrow_phase`, nearly all `circle_vs_circle_speculative` | `step.rs:1330`, `collision.rs:189` | 3.8% |
| H3 | `ImpulseMap::get`, one probe per contact after every build | `step.rs:230`, called at `step.rs:1029` | 3.8% |
| H4 | `solve_contacts` | `step.rs:1172` | 3.7% |
| H5 | `sleep_frame_start` plus `sleep_frame_end` | `step.rs:635`, `step.rs:690` | 4.3% |

Under 1% each: `apply_restitution`, `sync_impulses`, `gather_proxies` (0.40%), `write_back` (0.09%), `EventQueue::send` (0.15%). `event_queue.rs` holds no hotspot in any row.

### `physics-sparse`: `perf-runs/20261001T001753Z-physics-sparse/profile/` (16,166 samples)

`physics_step` holds 93.1%. `perf stat`: 1.5 instructions per cycle, 7.0% L1 data misses, 4.7% branch misses.

| ID | Symbol | Location | Share |
| --- | --- | --- | --- |
| H1 | `build_pairs`, run 3.2 times per frame | `step.rs:804` | 53.6% |
| H1a | `SpatialGrid::for_each_in` from `build_pairs` | `broadphase.rs:132` | 49.4% |
| H1d | `SpatialGrid::build` | `broadphase.rs:181` | 7.3% |
| H6 | `speculative_pass`, the safety-net sweep | `step.rs:1237` | 29.0% |
| H6a | `SpatialGrid::query` from the sweep | `broadphase.rs:125`, called at `step.rs:1261` | 22.0% |
| H1c | exact-cell compare, both callers | `broadphase.rs:160` | 12.9% |
| H1e | `SpatialGrid::cell_range`, of which `floorf` is 4.0% | `broadphase.rs:228` | 6.6% |
| H5 | `sleep_frame_start` plus `sleep_frame_end` | `step.rs:635`, `step.rs:690` | 4.4% |

Narrow phase is 1.1% and the solver 0.7%.

### `integrated`: `perf-runs/20261001T005327Z-integrated/profile/` (21,595 samples, reported only)

`physics_step` holds 54.7%: `build_pairs` 32.2% at 4.00 builds per frame, `SpatialGrid::build` 9.6%, `solve_contacts` 4.4%, `ImpulseMap::get` 4.1%, `cell_range` 3.4%, `SpatialGrid::insert` 3.0%, `sleep_frame_start` 2.9%, `gather_proxies` 2.5% (`gather_tilemap_proxies`, `step.rs:1373`, 1.9%).

### `churn`: `perf-runs/20261001T001848Z-churn/profile/` (DWARF, 14,707 samples) and `perf-runs/20261001T005555Z-churn/profile/` (frame pointers)

`World::flush` holds 84.3% inclusive. `perf stat`: 2.3 instructions per cycle, 1.4% L1 data misses, 0.3% branch misses, about 100 million instructions per frame, roughly 1,800 per command. The row is bound by instruction count and by latency on scattered rows, not by bandwidth.

Self time, grouped (`tables/churn-self-dwarf.txt`):

| ID | Symbols | Location | Share |
| --- | --- | --- | --- |
| H7 | `TypedVec<T>::swap_remove_erased`; 88% of its samples wait on the load of the row being moved | `archetype.rs:60` | 28.3% |
| H8 | `malloc`, `free`, `realloc` and their internals in `libc.so.6` | — | 22.4% |
| H9 | `Archetypes::insert<T>` | `storage.rs:83` | 11.3% |
| H10 | `Archetype::move_components_to` | `archetype.rs:153` | 10.8% |
| H11 | `TypedVec<T>::push_erased` | `archetype.rs:55` | 5.7% |
| H12 | `Any::type_id` behind every downcast | `archetype.rs:57` | 3.4% |
| H13 | `World::flush` itself; 78% of it waits on entity metadata in `is_alive` | `world.rs:430`, `entity.rs:109` | 3.3% |

By command kind, inclusive, from the frame-pointer profile: inserts 48.1% (`InsertSetter::apply`), despawns 19.7% (`World::despawn`, of which `Archetype::swap_remove_row`, `archetype.rs:139`, is 17.9%), removals 8.2%. The benchmark's own systems are 5.6%.

### `ecs`: `perf-runs/20261001T001827Z-ecs/profile/` (DWARF, 20,276 samples) and `perf-runs/20261001T005454Z-ecs/profile/` (frame pointers)

`perf stat`: 2.2 instructions per cycle, 3.8% L1 data misses, 1.3% branch misses. The DWARF flamegraph of this row roots most stacks under `atan2f` and is wrong; use the DWARF self-time table (`tables/ecs-self-dwarf.txt`) and the frame-pointer flamegraph. Startup (`spawn_population`) is 17.6% of the profile run and lies outside the measured frames.

| ID | Symbol | Location | Share |
| --- | --- | --- | --- |
| H14 | `follow` (10.6% self) plus `Archetypes::get::<Position>` (4.0%), `get_erased` and `type_id` (0.6%) | `examples/02_bench/src/ecs/systems.rs:154`, `storage.rs:241` | 15.2% |
| H15 | `heading` (6.2% self) plus `atan2f` (13.3%): benchmark work | `examples/02_bench/src/ecs/systems.rs:201` | 19.4% |
| H16 | `brain`: benchmark work, a branching state machine | `examples/02_bench/src/ecs/systems.rs:55` | 15.9% |
| H17 | the other eleven systems, 0.7–5.2% each | `world.rs:314` (`query2_mut`) and siblings | 27.7% |

## Why each hotspot costs what it does

### H1: the pair list never survives a substep

`substep` rebuilds the pair list when `pair_budget_exhausted` finds one awake body with `travel + |v|·sub_dt + 2·linear_slop > radius` (`step.rs:869`, `step.rs:940`). With 8,000 moving bodies one always trips, for three reasons.

1. **The gravity allowance is short by one `g·h²`.** `build_pairs` adds `½·g·t_left²` (`step.rs:824`). The integrator adds `g·h` to the velocity before it moves the body, so a body falling along gravity has travelled `k·|v|·h + g·h²·k(k+1)/2` after k substeps, and the check adds `(|v| + k·g·h)·h` on top. At the last check of a 4-substep frame the left side is `4|v|h + 9·g·h²` and the radius holds `4|v|h + 8·g·h²`. The shortfall, 0.0156 px at 900 px/s² and 240 Hz, trips every falling body. The replay shows 4,328 trips at the last substep with today's formula and 1,256 with the exact term.
2. **With constant velocity the two sides are equal.** Without gravity, or after a build with two substeps left, `travel + |v|·h` equals the predicted travel exactly, so `f32` rounding decides. In `physics-sparse` 3,970 of 8,000 bodies trip at the last substep.
3. **Contacts change speeds.** A body that a collision, a bumper or the solver's push-out speeds up outruns the travel predicted at build time. That is 20–100 bodies per substep in `physics` and up to 89 in sparse. A slow body has a radius near `2·linear_slop` = 0.5 px, so a push of about 30 px/s is enough.

Each rebuild restages 8,521 proxies, rebuilds the grid and runs 8,000 queries: 9.85 ms per frame in `physics`, or 2.47 ms per build. Rebuilding every substep also sets `seed_pair_impulses` each time, so `D-076`'s index carry never applies and every contact probes the keyed map (H3).

One query costs about 290 ns in `physics` because the board is dense: 8,521 proxies stage 17,000–23,000 cell references over 2,356 cells of 32 px, which is 2 to 2.7 cells per proxy and 7 to 10 entries per cell. Multi-cell entries disable the single-cell fast path, so each candidate pays the dedupe mark, a read of the 80-byte `Proxy` for two flags, and an overlap test.

### H1c, H1e: hashing and `floorf`

Every entry in a slot is compared with the queried cell (`broadphase.rs:160`) because the slot table is hashed. All three physics worlds are small and bounded: 38 × 62 cells in `physics` and 137 × 79 in sparse (measured from the replay snapshots), and about 768 × 48 in `integrated` (from its level size). A build stages 17,000–36,000 cell references, so the hashed table has 32,768 or 65,536 slots (128–256 KiB of `slot_starts`) plus 8 bytes of `entry_cells` per reference. A direct table would have 2,356, 10,823 and about 37,000 slots and no `entry_cells`.

`cell_range` calls `f32::floor` four times (`broadphase.rs:233`). The generic x86-64 build has no `roundss`, so each call lands in `compiler_builtins`' software `floorf`: 2.4–4.0% of samples. An `x86-64-v3` build inlines it.

### H6: the sweep searches the wrong grid

`speculative_pass` handles every dynamic body that moved more than its smallest half extent in the substep (`step.rs:1254`). A 2 px body at 600–1,400 px/s moves 2.5–5.8 px per substep, so 98.5% of the sparse bodies qualify. Each one queries the pair grid, which holds every proxy with its inflated AABB, and then discards the dynamic candidates (`step.rs:1271`). Only statics can be hit, and sparse has four.

### H5: sleep bookkeeping runs when nothing sleeps

`sleep_frame_start` rebuilds the sleep map every frame: one clear of a table sized for twice the proxies, then three probes per body (`step.rs:645`–`680`). `sleep_frame_end` probes once more per body (`step.rs:716`).

### H7–H13: one allocation per moved value

- `swap_remove_erased` returns `Box<dyn Any>` (`archetype.rs:60`) and `push_erased` takes one (`archetype.rs:55`). Moving a row therefore allocates and frees one box per component, and a despawn allocates one per component only to drop it (`archetype.rs:142`).
- `Archetypes::insert` boxes the inserted value as well (`storage.rs:133`).
- `move_components_to` collects a `Vec<TypeId>` per move (`archetype.rs:154`), scans `dest.component_types` once per type, and does three hash lookups per moved column (`archetype.rs:163`, `archetype.rs:174`, `archetype.rs:178`).
- A spawn with k components added one at a time passes through k archetypes and moves k(k−1)/2 values: 15 at the default of 6.
- `CommandBuffer` boxes every insert and every removal (`command_buffer.rs:76`, `command_buffer.rs:84`, `command_buffer.rs:91`), and `stage_flush_commands` drops the buffer and creates a new one every frame (`crates/tungsten/src/app.rs:730`).

Counting those sites for one default frame gives 293,750 allocations; the measured number is 293,960:

| Source | Allocations per frame |
| --- | --- |
| One box per value moved along the spawn chain (6,250 × 15) | 93,750 |
| One box per value moved by a status toggle, plus the removed status | 40,625 |
| One box per component of a despawned entity | 37,500 |
| One box per inserted value | 40,625 |
| The `Vec<TypeId>` of each move | 37,500 |
| One box per insert or removal command | 43,750 |

What remains after the allocations is latency. Despawns and toggles touch six or seven columns at a scattered row, about 78,000 loads per frame, and several hundred instructions separate one load from the next, so the misses can't overlap. The `is_alive` check in `flush` misses on entity metadata the same way.

### H14: `World::get` is a dependent chain

Each lookup loads the entity's metadata (69% of `Archetypes::get`'s samples wait there, `entity.rs:92`), probes the archetype's column map, makes two virtual calls (`get_erased`, then `type_id` for the downcast, `storage.rs:244`–`247`) and loads the leader's `Position`. In `follow`, 74.5% of the samples sit on the first use of that value. The metadata table is 4 MiB and fits the 16 MiB L3; the component columns together do not, so the first load usually comes from L3 and the second from memory. One iteration is an estimated 150 instructions, so at most two lookups fit in the reorder window together.

### H17: every archetype starts cold

With the same entities and the same work per entity, `update` grows by about 3.5 µs per archetype per frame between 64 and 488 archetypes, which is about 220 ns per archetype per query pass. `integrate` alone goes from 0.15 ms at 64 archetypes to 0.26 at 488 and 3.82 at 10,252. In its annotated profile the row loop is about 13 instructions per entity, and setup (the `has` scans at `archetype.rs:129`, the walk over the column map at `world.rs:541`, the downcasts) takes about 10% of the samples, roughly 55 ns per archetype. The remaining 165 ns is consistent with the first cache lines of each column missing: a hardware prefetcher needs two or more misses in a pattern before it starts [S7], and every column of every archetype is a separate allocation.

## Candidates, ranked

Expected gains are for the owned metric of the judging row against the baseline.

| Rank | ID | Change | Judged by | Expected |
| --- | --- | --- | --- | --- |
| 1 | P1 | Repair the pair list for tripped proxies instead of rebuilding it | `physics`, `physics-sparse` `physics_step`; `integrated` `total` | −45% to −55%; sparse −25% to −30%; `integrated` −17% to −25% |
| 2 | C1 | Move rows between columns without boxing; columns in a sorted `Vec` | `churn` `flush` | −30% to −40% |
| 3 | P2 | A static-only grid for the safety-net sweep | `physics-sparse` `physics_step` | −22% to −26% |
| 4 | C3 | Apply consecutive inserts on one entity as one archetype move | `churn` `flush` | −25% to −40% alone; −10% to −15% after C1 |
| 5 | C2 | Commands without a box each; reuse the buffer | `churn` `flush` | −5% to −10% |
| 6 | P3 | Cheaper grid walk: inline floor, direct cell table, compact flags | both physics rows `physics_step` | −6% to −12% before P1 |
| 7 | E1 | A short, inlinable `World::get` | `ecs` `follow`, `update` | `follow` −10% to −45% |
| 8 | E2 | Cheaper archetype setup | `ecs` `update` | −2% to −4%, unproven |
| 9 | P4 | Sleep table by proxy index | both physics rows | −3% |

C4 (prefetch in `flush`) and the prefetch half of E2 were dropped: they need `unsafe` (see "Not proposed").

### P1. Repair the pair list instead of rebuilding it

- **Change.** Keep the build at frame start and on `pairs_invalidated`. Replace "one trip rebuilds everything" with:
  1. Budget: `radius = |v|·t_left + |g|·h²·(n−1)(n+2)/2 + margin + 2·linear_slop`, where n is the number of substeps left. Start with a margin of `2·linear_slop` (0.5 px); it also absorbs the rounding case.
  2. Before each substep, collect the tripped proxies E in proxy order. If E is empty, reuse the list.
  3. If E holds more than a quarter of the awake bodies, rebuild as today. The quarter is a starting value, not a measured one: with the exact gravity term the replay's largest count is 1,256 of 8,000 (`physics`, last substep, no margin), so it should not fire in the owned rows. Tune it only if a capture shows repairs costing more than the rebuild they replace.
  4. Otherwise repair:
     - drop every pair that involves a member of E, compacting `pairs` and `pair_impulses` together, and keep each dropped pair's nonzero carried impulse in a scratch map under its pair key;
     - give each member a fresh radius and inflated AABB from its current state and reset its travel;
     - add E to the list R of proxies repaired since the last build, and restage a second small grid from R;
     - for each member e in index order, query the main grid, skipping candidates in R because their staged cells are stale, and then the repair grid. Apply the overlap test and the repair pairing rule in item 5, and append each admitted pair with its impulse from the scratch map, or zero.
  5. The repair pairing rule is not the build's initiator rule. A build skips an awake dynamic candidate whose index is at or below the querying proxy's (`step.rs:848`), because that candidate runs its own query. In a repair only the members of E query, so the build's rule would lose every pair between a tripped proxy and an untripped awake dynamic body of lower index. For a member e and a candidate c other than e:
     - c is static or sleeping: admit, stored as (e, c);
     - c is an awake dynamic body outside E, whether or not it is in R: admit whatever its index, stored as (lower index, higher index), the orientation a build stores, so nothing downstream sees an order a build can't produce;
     - c is in E: admit only when c > e, so a pair between two members is added once, stored as (e, c).
  6. A repair leaves the keyed impulse map and `seed_pair_impulses` alone. `pair_impulses` already holds zero for a pair whose contact was absent in the previous substep, so the carry keeps `D-076`'s rule.
- **Hotspot.** H1 and H3.
- **Expected gain.**
  - `physics`: `physics_step` p50 13.26 → 6.0–7.3 ms. Arithmetic: remove 9.85 ms of builds; add one build at substep 0, about 3.0 ms (2.47 ms average, scaled for the larger inflation); add repairs, about 470 proxies per frame at an estimated 0.4 µs each; add about 0.45 ms of narrow phase for 36,756 pairs per substep against today's mean of 23,909; remove about 0.45 ms of map probes and syncs.
  - `physics-sparse`: p50 8.06 → 5.7–6.1 ms, and p95 falls to about the same value because the four-build frames disappear.
  - `integrated`: `total` p50 12.12 → about 9–10 ms. `build_pairs` is 3.95 ms of its `physics_step`; trips were not replayed for this row.
- **Evidence.** The call counts, the replay tables and the profile shares above. The same technique elsewhere: Box2D fattens proxy AABBs so they "move by a small amount without triggering a tree adjustment" and re-queries only the proxies in its move buffer [S1, S2]; its margin is 20 times its linear slop. Checkaraou et al. size a Verlet skin per particle from its own displacement, because one global skin is suboptimal when flow regimes coexist [S3].
- **Why a margin alone is not enough.** Under today's rule a margin must reach 8 px before `physics` builds once per frame, and the list then holds 98,958 pairs, four times today's mean. At 4 px it still builds 1.87 times.
- **Risk to determinism and tests.**
  - The step stays serial and ordered by proxy index, so two runs of one build still match (`tests/physics_determinism.rs`).
  - Pair order differs from a fresh build's, so Gauss–Seidel order and trajectories differ from today's. The digests of `physics`, `physics-sparse` and `integrated` move, and `physics.pairs` roughly doubles, which compare reports as workload drift.
  - The contact set per substep must stay equal to a fresh build's: `assert_pair_contacts` and `persistent_pairs_match_fresh_contacts_on_randomized_piles_bullets_and_wakes` (`src/tests/physics/step.rs:1122`, `:1184`) check that every substep, and the `reference_impulses` oracle checks every warm start bit for bit.
  - `fast_body_trips_pair_budget_and_rebuilds_before_narrow_phase` (`:1336`) asserts a second build and the exact old radius. It needs rewriting for the new formula and for repair.
  - Add: a pile where a few bodies speed up mid-frame (a repair, with `pair_builds` unchanged); a tripped body whose untripped awake neighbour has a lower index (the pair the build's initiator rule would drop); a body repaired twice in one frame; a pair between two bodies repaired in different substeps; the fall-back to a rebuild.
  - `pile_plus_bullet` and `projectile_stream` in `physics_bench.rs` must not regress, as `D-075` required.
- **Decisions.** A new entry superseding `D-075`'s rule (a) ("any of these rebuilds from current state") and its radius formula, and amending `D-076` (a repair carries impulses by pair key through a scratch map, without a keyed-map sync).

### C1. Move rows between columns without boxing

- **Change.**
  1. `AnyColumn` gains `move_row_to(&mut self, row, dest: &mut dyn AnyColumn)`, which downcasts `dest` once and does `dest.push(self.swap_remove(row))`, and `swap_remove_drop(row)` for despawn.
  2. `Archetypes::insert<T>` and `remove<T>` push and swap-remove through the typed column instead of `Box<dyn Any>`.
  3. `Archetype::columns` becomes a `Vec<Box<dyn AnyColumn>>` in `component_types` order, created with the archetype. A move walks the two sorted type lists once: no `Vec<TypeId>`, no hash lookups.
  4. Downcasts use trait upcasting to `dyn Any` in place of `as_any` [S8].
- **Hotspot.** H8–H12; the allocations inside H7.
- **Expected gain.** `flush` p50 8.50 → 5.1–6.0 ms. 250,000 of the 293,960 allocations per frame disappear (85% of H8, about 19% of samples), and about half of H9–H12 (about 15%). The cache misses in H7 stay. Immediate-mode structural calls and startup gain too: `spawn_population` is 17.6% of the `ecs` profile run.
- **Evidence.** The allocation count and its per-site breakdown, the self-time table, and hecs' note that `insert` costs in proportion to the entity's component count [S6].
- **Risk.** Rows, swap-remove order and archetype creation order must not change, so iteration order and every digest stay the same. `src/tests/ecs/archetype.rs`, `storage.rs` and `world.rs` pin the behavior; `query2_opt2_matches_query2_order` and `query_mut_matches_query_order` pin the order. `docs/plans/debug-cleanup-docs-pass.md` removes `AnyColumn::len`, `AnyColumn::type_id` and `Archetype::id` from the same file; "Checkpoints, reverts and shared files" says how the two plans are ordered.
- **Decisions.** A new entry amending `D-036`'s storage description (`AnyColumn` over a hashed column map); `DESIGN.md` §ECS follows.

### P2. A static-only grid for the safety-net sweep

- **Change.** Stage the static proxies once per frame into a second `SpatialGrid`, with the same `2·linear_slop` inflation they get in the pair grid, and let `speculative_pass` query it.
- **Hotspot.** H6.
- **Expected gain.** `physics-sparse` p50 8.06 → 6.0–6.3 ms: the sweep is 2.60 ms per frame, 82 ns per query, and a query against four walls should cost 15–20 ns. `physics` has 33 sweep queries per frame and does not move. In `integrated` the same grid could later let pair builds restage 4,516 dynamic proxies instead of 13,044. That follow-up changes pair order and is not part of this pass (see "Not proposed").
- **Evidence.** The sweep-query count, the profile, and Box2D's separate tree per body type [S1].
- **Risk.** None to results: the static candidates and their order are the same, so the first hit is the same. The `physics-sparse`, `physics` and `integrated` digests must not change. `sweep_net_catches_solver_injected_velocity_through_static_wall` and `sweep_net_covers_static_circles_via_bounding_square` cover the pass.
- **Decisions.** A new entry amending `D-062` (one grid) and `D-075`'s note that the sweep keeps querying the pair grid.

### C3. Apply consecutive inserts on one entity as one move

- **Change.** In `World::flush`, treat a run of consecutive `Insert` commands with the same target as one operation: walk the add edges for each new type (creating the intermediate archetypes in today's order, with no rows), move the row once, then push or overwrite each value in command order.
- **Hotspot.** The spawn chain inside H7–H11: 93,750 moved values per frame.
- **Expected gain.** Alone, `flush` p50 8.50 → 5.1–6.4 ms. A spawn costs 485 ns in immediate mode (measured) and an estimated 630 ns inside `flush`, where inserts are 4.85 ms inclusive and toggle gains about 0.9 ms of that; one move plus six pushes should cost about 100 ns. After C1 the chain is already cheaper, so the step adds about 1 ms. At 12 components the chain is 66 values per spawn, which is what takes `flush` to 28.86 ms.
- **Evidence.** The `components` sweep, the immediate-mode capture (`churn_spawn` 3.03 ms for 6,250 spawns), and flecs' command batching, "which reduces archetype moves for entities when doing deferred operations" [S5].
- **Risk.** The result must equal sequential application: last write wins for a type inserted twice, an insert of a type the entity already has overwrites, commands for other entities are never reordered, and archetype IDs come out in the same order. `flush_command_order_preserved`, `flush_insert_skips_entity_despawned_earlier_in_same_buffer`, `flush_multiple_pending_entities` and `flush_spawn_insert_pending_components_visible` pin the semantics. Digests must not change.
- **Decisions.** A new entry amending `D-039` (replay in registration order stays; a run on one entity becomes one move).

### C2. Commands without a box each

- **Change.** Store inserted values in one typed queue per component type inside `CommandBuffer`; an insert command holds the queue's slot, and `flush` takes values in recording order. A removal holds the entity and a plain `fn(&mut World, Entity)`. `flush` drains a buffer that the app keeps across frames.
- **Hotspot.** The 43,750 command boxes in H8, the dispatch in `flush`, and the buffer that regrows every frame.
- **Expected gain.** `flush` p50 −0.4 to −0.9 ms. The whole deferred overhead is 2.67 ms, but part of it is the second pass over the data, which stays. `churn_spawn` and `churn_toggle` should not move by more than their 0.02 ms floor.
- **Evidence.** The deferred and immediate captures, the allocation breakdown, and Bevy's two rewrites of its command queue, which removed the `Box<dyn Command>` per command for 1.6× faster spawn commands and later stored commands "densely within the same buffer" [S4].
- **Risk.** Command order and the dead-entity guards must stay. `CommandBuffer::len` must keep counting one per recorded command: `src/tests/ecs/command_buffer.rs` and the `churn` and `ecs` benchmarks read it. Bevy's packed byte queue needs `unsafe`; typed queues do not.
- **Decisions.** The same entry as C3: `D-039` names `Vec<Command>`, a private `ComponentSetter` trait object and a boxed closure for removals.

### P3. Cheaper grid walk

- **Change.**
  1. Compute the cell index by truncation with a correction for negatives (saturating), in place of `floor`.
  2. When the staged cell bounds are compact (width × height no more than four times the cell references), index slots directly by cell and drop `entry_cells`; keep the hashed table for worlds that are large or sparse.
  3. Read the "awake dynamic" flag in the pair query from a one-byte-per-proxy array filled in `build_pairs`, not from the 80-byte `Proxy`.
- **Hotspot.** H1b, H1c, H1e.
- **Expected gain.** Before P1, `physics_step` −6% to −12% in both physics rows: `floorf` is 2.4–4.0% of samples and the exact-cell compare 7.1–12.9%. The direct table is 9–42 KiB in the two physics rows where the hashed one is 128–256 KiB, and 140–290 KiB of `entry_cells` go away. After P1 the builds are a quarter as frequent, so the same change is worth about a third of that.
- **Evidence.** The profile shares, and Ericson's treatment of grids stored as dense arrays against hashed storage [S9].
- **Risk.** None to results if the visit order is kept: cells in the same y-then-x order, entries in staging order. Digests must not change. `hash_aliasing_never_produces_false_candidates` stays for the hashed path; add twins of the broadphase tests for the direct path, a test that forces the fall-back, and boundary tests for the floor (negative coordinates, exact cell edges, values beyond `i32`).
- **Decisions.** A new entry amending `D-062` ("unbounded cell coordinates map into a power-of-two slot table … each entry stores its exact cell" becomes the fall-back).

### E1. A short, inlinable `World::get`

- **Change.** On C1's sorted column `Vec`: find the column by a scan of at most about 20 `TypeId`s, downcast through upcasting, index the row. Mark `World::get`, `get_mut` and `Archetypes::get` `#[inline]`.
- **Hotspot.** H14.
- **Expected gain.** `follow` p50 1.93 → 1.1–1.7 ms, and `update` p50 −2% to −8%. The floor is the removed work, about 10 ns of the 76 ns per lookup (the map probe and the two virtual calls). The ceiling assumes three or four lookups overlap once an iteration shrinks from about 150 instructions to about 60.
- **Evidence.** The annotated profiles (the two waits), the `followers` sweep, and Drepper on dependent loads [S7].
- **Risk.** None to results. The gain above the floor is a hypothesis; measure before keeping the `#[inline]` attributes.
- **Decisions.** Covered by C1's entry.

### E2. Cheaper archetype setup (prototype first)

- **Change.** Resolve a query's columns by index on C1's sorted `Vec` (no walk over a map, no hashing) and filter archetypes with a 64-bit type mask before the exact check.
- **Hotspot.** The setup part of H17.
- **Expected gain.** Of the 1.7 ms of per-archetype overhead, setup is about 0.4 ms per frame (4% of `update`) and most of it should go. The cold starts, about 1.3 ms, stay: the prefetch that aimed at them needs `unsafe` (see "Not proposed").
- **Evidence.** The `fragmentation` sweep and the annotated `integrate`.
- **Risk.** No semantic risk; iteration order must not change. Gate it with `just perf run ecs --sweep fragmentation=1,8,64 --repeat 3` before and after. At the default (488 archetypes) the whole setup cost is about 0.4 ms against a p50 threshold of 0.32 ms (3% of 10.75), so the default row may read `unchanged`; the gain should show at `fragmentation=64` (3,189 archetypes). Under the keep-or-revert rule an `unchanged` default row means a revert.
- **Decisions.** Covered by C1's entry.

### P4. Sleep table by proxy index

- **Change.** Keep the sleep entries in an array parallel to the proxies and reuse it while the proxy key sequence equals the last frame's (one sequential compare). Fall back to today's keyed rebuild when it differs.
- **Hotspot.** H5.
- **Expected gain.** `physics_step` −0.42 ms in `physics` and −0.27 ms in sparse at most (the `sleep=off` captures): 3% now, about 6% of the step once P1 has landed.
- **Risk.** Every `D-065` wake path must hold: `settled_island_sleeps_and_freezes`, `despawn_in_sleeping_island_wakes_the_rest`, `external_velocity_write_wakes_island`, `external_position_write_wakes_island`, `late_sleeper_adopts_supporting_island_for_despawn_wake`. Digests must not change.
- **Decisions.** A note amending `D-065` ("rebuilt from live proxies each frame").

## Not proposed

- **A parallel solver.** `D-067` measured it and dropped it. `solve_contacts` is still 3.7%.
- **A different cell size.** The `cell` sweeps put 32 px at the optimum for `physics` and within 2% of it for sparse.
- **`heading` and `brain`.** `atan2f` and the state machine are the benchmark's work; changing them would bump `workload_version`.
- **A wide narrow phase or solver.** 3.8% and 3.7% today. Look again after P1.
- **A bundle-insert API.** C3 gives the same single move without a change to the benchmark.
- **A packed byte command queue or `BlobVec`-style columns.** Both need `unsafe`; the crate has none outside tests.
- **Cached tile proxies.** `gather_tilemap_proxies` is 1.9% of `integrated` and nothing in the owned rows. It stays an engine finding in `docs/perf/benchmarks.md`.
- **Software prefetch in `flush` and in query iteration (was C4 and half of E2).** `core::arch::x86_64::_mm_prefetch` is declared safe, but it carries `#[target_feature(enable = "sse")]`, and rustc 1.98.1 rejects a call from a function without that attribute (E0133: "the sse target feature being enabled in the build configuration does not remove the requirement"). A wrapper with the attribute only moves the error to its caller. Both were compiled under `#![forbid(unsafe_code)]` on 2026-10-01. So a prefetch needs an `unsafe` block, which is a non-goal. The latency it aimed at is measured: 88% of `swap_remove_erased` and 78% of `flush` wait on one load (about 1.5 ms of today's `flush`), and cold archetype starts cost about 1.3 ms in `ecs`. Reopen it with a decision that allows one `unsafe` block.
- **Pair builds that restage only dynamic proxies.** With P2's static grid, `integrated` could restage 4,516 proxies per build instead of 13,044. It changes pair order a second time, and trips were not replayed for `integrated`. Open it as its own task after P1, from a fresh profile of `integrated`.

## Sessions

The physics steps and the ECS steps touch different files and are judged by different rows, so the plan runs as two sessions. Each starts from this file and continues from "Progress".

| Session | Steps | Close-out it owns |
| --- | --- | --- |
| A, physics | 0, 1, 2, 3, then P4 from step 8 | A profile of `physics` and `physics-sparse` with what now leads each; decisions 1 and 2 (and 5 if P4 is kept) with their index rows; the three physics-bearing digests in `docs/perf/benchmarks.md` |
| B, ECS | 0 on `ecs` and `churn` only, 4, 5, 6, 7, then E2 from step 8, then step 9 | Decisions 3 and 4; the rest of step 9; `status: done` |

Session A sets `status: in progress` when it starts and leaves it there. It is finished when:

- its rows in "Progress" are complete;
- a full `just perf suite --repeat 5 --compare pre-physics-ecs-pass` reads the `physics`, `physics-sparse` and `integrated` lines of "Final checks", with no owned metric `regressed` in the other five rows;
- the `ecs`, `churn`, `gpu`, `gpu-throughput` and `particles` digests equal the baseline's;
- `just check`, `just smoke` and `just repo-check` pass.

## Ordered steps

Run every capture through `guard.sh` with `loadlog.sh` running, with no other session building. Both scripts are machine-local (see "Capture hygiene"). Each session writes its logs to a new folder and records it in "Progress":

```bash
EV=perf-runs/20261001-physics-ecs-pass-evidence/scripts
LOG=perf-runs/<UTC date>-physics-ecs-pass-session-<a|b>
mkdir -p "$LOG"
"$EV/loadlog.sh" "$LOG/load.log"      # once per session, in the background; stop it at the end
"$EV/guard.sh" "$LOG/guard.log" just perf suite --repeat 5 --only <rows> --compare pre-physics-ecs-pass
python3 "$EV/capture_load.py" "$LOG/load.log" perf-runs/<UTC>-suite
```

`guard.sh` exits 97 without running when `nxcodec.bin` is present. Redo a capture when `guard.log` shows a sighting inside it or when `capture_load.py` shows a foreign load spike in a run's window. For reference, the baseline's runs peaked at 1.61 busy CPUs and the CPU-bound rows averaged 1.12–1.30.

Captures use `--repeat 5`. "Digest check" means each run's digest equals the stated one:

```bash
jq -r '[.row, (.runs | map(.digest) | unique | join(","))] | join(" ")' perf-runs/<UTC>-suite/*/capture.json
```

Steps 1 and 2 come before P1 although P1 is worth more: they leave results unchanged, so the digests in `docs/perf/benchmarks.md` verify them, while P1 changes the three physics-bearing digests. The physics steps (1–3) and the ECS steps (4–7) are independent.

0. **Preflight.**
   - `just perf baseline list` shows `pre-physics-ecs-pass` as valid.
   - `just perf suite --repeat 5 --only physics,physics-sparse,ecs,churn --compare pre-physics-ecs-pass` on the untouched tree reads no `regressed` and no `improved`.
   - If the tree no longer builds the same binary (toolchain, flags, engine sources), capture a new baseline first.
1. **P2, static sweep grid.** `just check`; capture `physics`, `physics-sparse`, `integrated`; digest check against the baseline's.
2. **P3, grid walk costs.** The same captures and digest check. Do the floor first and measure it alone.
3. **P1, pair repair.** Write the new decision text first. Sweep the margin over 0, 0.5 and 1 px with a temporary constant and keep the margin with the lowest sum of the two rows' `physics_step` p50; when two sums are within 1%, keep the smaller margin, which holds fewer pairs. Capture `physics`, `physics-sparse`, `integrated`; record the sweep, the chosen margin and the three new digests in "Progress". Run `cargo bench -p tungsten-core --bench physics_bench` before and after: `pile_plus_bullet` and `projectile_stream` must not slow by more than 10% (`D-075`'s gate).
4. **C1, unboxed moves and sorted columns.** Capture `churn`, `ecs`, `particles`, `integrated`; digest check (after step 3 the `integrated` digest to match is step 3's). `spawn_insert_3_components_10k`, `spawn_despawn_1k` and `command_buffer_flush_1k_spawns` in `ecs_bench` are quick checks for steps 4–6; they build with `target-cpu=native`, so they never compare with captures.
5. **C3, batched inserts.** Capture `churn`; digest check on `churn`, `particles`, `integrated`.
6. **C2, command buffer.** Capture `churn`; digest check as in step 5.
7. **E1, `World::get`.** Capture `ecs`; digest check.
8. **Gated prototypes.** P4 in session A after step 3, judged on `physics` and `physics-sparse` against step 3's capture, with a digest check on the three physics-bearing rows. E2 in session B after step 7. Keep each only on an `improved` step verdict.
9. **Close out (session B; session A does its part of this list, see "Sessions").**
   - Profile the four rows again (`just perf run <row> --profile`, plus `--call-graph fp` for `ecs`) and list what now leads each one.
   - `just perf suite --repeat 5 --compare pre-physics-ecs-pass`.
   - Write the decisions and their `docs/DECISION_INDEX.md` rows.
   - In `docs/perf/benchmarks.md`, update the digests, the calibrated numbers and the engine findings.
   - Add the `CHANGELOG.md` entry.
   - Run `just check`, `just smoke` and `just repo-check`.

After each step, judge twice: against the previous step's capture of the same row (did this step help) and against the baseline (cumulative):

```bash
just perf suite --repeat 5 --only <rows> --compare pre-physics-ecs-pass
just perf compare perf-runs/<previous suite> perf-runs/<this suite>
```

Keep or revert: a step whose verdict is `unchanged` or `noisy` after one rerun is reverted, unless it is a prerequisite of a later step (C1 is, for E1 and E2). Record the verdict and the outcome in "Progress". A `regressed` owned metric in any row needs a fix or a written justification (`docs/perf/profiling-workflow.md`, "Regression policy").

### Checkpoints, reverts and shared files

- Sessions run no mutating Git command. Before a step, copy every file it will touch to the session's scratch folder. After the step, write it as one patch with a commit message file (`NN-<id>.patch`, `NN-<id>.msg`) next to those copies. A revert restores the copies and touches no other file.
- On 2026-10-01 the working tree carried 15 uncommitted files from other work (release tooling), among them `DECISIONS.md`, `docs/DECISION_INDEX.md` and `CHANGELOG.md`; `D-079` existed only there. Before editing one of the three, read `git status --short` and `git diff --unified=1` for it. If foreign changes are still there: append after them, take the next free decision ID from the working tree and not from `HEAD`, never renumber or rewrite a foreign entry, and say in the hand-off that this plan's patches for those files apply on top of the foreign changes. If a foreign change edits a decision this plan amends, stop and ask.
- `docs/plans/debug-cleanup-docs-pass.md` (draft) overlaps in two places. In `physics/step.rs` it only repoints a comment path, which survives either order. In `ecs/archetype.rs` and `storage.rs` it removes `AnyColumn::len`, `AnyColumn::type_id` and `Archetype::id`: check that plan's status before step 4. If it has landed, C1 starts from the reduced trait. If not, C1 lands first and leaves those three items alone unless its own change makes one of them dead; in that case note it in "Progress" for that plan to pick up.

### When to stop

The per-step table under "Done when" lists expectations. The gate for a step is the keep-or-revert rule, and the gate for the plan is "Final checks". Stop, record the state in "Progress" and report the measured verdicts, what the profile shows and one proposed next move when:

- a revert leaves a final check out of reach (E1 for `system.follow`, P1 for `integrated`);
- a digest that must not change changes, and the cause is not found in one attempt;
- a determinism, tunnelling or containment test fails and one fix attempt does not clear it;
- the preflight reads `regressed` or `improved`;
- a capture stays `noisy` or invalid after one rerun on a quiet machine.

Do not widen a change, take a candidate from "Not proposed", change a threshold or a benchmark knob, or rerun a third time to get a verdict.

## Done when

Expected verdicts of each step against the previous step's capture. These are expectations, not gates (see "When to stop"):

| Step | Row | Metric | Verdict |
| --- | --- | --- | --- |
| 1 (P2) | `physics-sparse` | `system.physics_step` p50, p95 | `improved` |
| 1 (P2) | `physics` | `system.physics_step` p50, p95 | `unchanged` |
| 2 (P3) | `physics`, `physics-sparse` | `system.physics_step` p50 | `improved` |
| 3 (P1) | `physics` | `system.physics_step` p50, p95; `stage.update` p95 | `improved` |
| 3 (P1) | `physics-sparse` | `system.physics_step` p50, p95 | `improved` |
| 3 (P1) | `integrated` | `stage.total` p50, p95 | `improved` |
| 4 (C1) | `churn` | `stage.flush` p50, p95 | `improved` |
| 5 (C3) | `churn` | `stage.flush` p50, p95 | `improved` |
| 6 (C2) | `churn` | `stage.flush` p50 | `improved` |
| 7 (E1) | `ecs` | `system.follow` p50 | `improved` |

Final checks:

- `just perf compare pre-physics-ecs-pass perf-runs/<final suite>` reads:
  - `physics`: `system.physics_step` p50 and p95 and `stage.update` p95 `improved`;
  - `physics-sparse`: `system.physics_step` p50 and p95 `improved`;
  - `churn`: `stage.flush` p50 and p95 `improved`, and no churn system row `regressed`;
  - `ecs`: `system.follow` p50 `improved`, and `stage.update` p50 `improved` or `unchanged`;
  - `integrated`: `stage.total` p50 and p95 `improved`;
  - no owned metric `regressed` in any of the eight rows (`noisy` on the GPU rows is expected, as in any A/A), and no peak-RSS verdict `regressed`.
- The suite is valid: every guard passes and every row's five digests match each other. The `ecs`, `churn`, `gpu`, `gpu-throughput` and `particles` digests equal the baseline's; the three physics-bearing digests are the ones recorded in step 3.
- Compare's workload-drift warning on `physics.pairs` is expected after P1 and is explained in the decision.
- `just check`, `just smoke` and `just repo-check` pass, and `tests/physics_determinism.rs`, `physics_tunneling.rs` and `physics_containment.rs` are green.
- The decisions exist with their index rows, and `docs/perf/benchmarks.md` carries the new digests and numbers.

## Decisions the candidates need

No `DECISIONS.md` entry is written by this plan. The implementation session adds, under the next free IDs:

1. Pair repair: supersedes `D-075`'s invalidation rule (a) and its radius formula; amends `D-076` (impulse carry across a repair). Covers P1.
2. Broadphase layout: amends `D-062` with the static grid and the direct cell table, and `D-075`'s note on the sweep's grid. Covers P2 and P3.
3. Column storage: amends `D-036` (sorted column `Vec`, unboxed moves, upcast downcasts). Covers C1, E1 and E2's setup part.
4. Command buffer: amends `D-039` (typed queues, function-pointer removals, a reused buffer, one move per run of inserts). Covers C2 and C3.
5. Sleep table: amends `D-065`, only if P4 is kept.

## Sources

- [S1] Box2D v3.0.0, `src/broad_phase.c`: one tree per body type; a move set and move array; `b2FindPairsTask` queries only moved proxies. https://github.com/erincatto/box2d/blob/v3.0.0/src/broad_phase.c
- [S2] Box2D v3.0.0, `src/core.h`: `b2_aabbMargin` (0.1 m) "is used to fatten AABBs in the dynamic tree. This allows proxies to move by a small amount without triggering a tree adjustment"; `b2_linearSlop` 0.005 m; `b2_speculativeDistance` 4 × slop. https://github.com/erincatto/box2d/blob/v3.0.0/src/core.h
- [S3] Checkaraou, Besseron, Rousset, Qi, Peters (2022), "Local Verlet buffer approach for broad-phase interaction detection in Discrete Element Method", arXiv:2208.13770. https://arxiv.org/abs/2208.13770
- [S4] Bevy pull requests #2332, "[ecs] Improve `Commands` performance" (merged 2021-07-16), and #6391, "Speed up `CommandQueue` by storing commands more densely" (merged 2023-01-28). https://github.com/bevyengine/bevy/pull/2332 and https://github.com/bevyengine/bevy/pull/6391
- [S5] Flecs v3.1.0 release notes: "Command batching, which reduces archetype moves for entities when doing deferred operations". https://github.com/SanderMertens/flecs/releases/tag/v3.1.0
- [S6] hecs, `World::insert` ("Computational cost is proportional to the number of components `entity` has") and `World::exchange` ("the intermediate archetype … is skipped"). https://docs.rs/hecs/latest/hecs/struct.World.html
- [S7] Drepper (2007), "What every programmer should know about memory", part 5, §6.3.1 (hardware prefetching starts after "two or more cache misses in a certain pattern" and cannot cross page boundaries) and §6.3.2 (software prefetching). https://lwn.net/Articles/255364/
- [S8] Rust: trait upcasting to `dyn Any`, stable since 1.86 and compiled under `#![forbid(unsafe_code)]` on 1.98.1, https://blog.rust-lang.org/2025/04/03/Rust-1.86.0/. `core::arch::x86_64::_mm_prefetch` is declared safe ("safe to use even though it takes a raw pointer argument") but is not callable without `unsafe` from ordinary code, because of its `#[target_feature]` attribute (see "Not proposed"), https://doc.rust-lang.org/core/arch/x86_64/fn._mm_prefetch.html
- [S9] Ericson (2005), *Real-Time Collision Detection*, chapter 7.1, "Uniform grids" (cell size, dense arrays and hashed storage).
