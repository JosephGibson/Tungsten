# Fixed-step health pass

- **status:** done
- **goal:** Fix what playing example 01 at uncapped 200–1000 fps shows since 0.55's fixed step (`D-137`): collision events lost between steps (finding 1), the player stuck at slab joins (finding 2), and a measured, decided windowed step policy for the two-step cost under load and the remaining unsmoothness (findings 3–4). Released as 0.57, outside the register.
- **non-goals:** the Phase 5 QA audit and pass; `spread_ball_fire`'s cost (a known-issues line with the measured number only); perf captures beyond step 3's; example rewrites; the solver's ghost-collision fix (a known-issues row only); the player's two debug boxes unless step 1 reproduces them; any 1.0 register row, `docs/plans/1.0/README.md` "Now" line, `docs/plans/1.0/roadmap.json` or the Phase 5 QA plan.
- **level:** A, unattended (owner away; implicit approval of the recommended defaults at steps 2 and 3, given 2026-10-07). No commits (`D-105`).

## Context digest

`EventQueue<T>` (`core/ecs/event_queue.rs`) keeps `previous` and `current` and rotates once a frame (`D-040`). Since `D-137` a fixed step has a view: the frame's first step reads both windows, a later step only the events sent since the step before it ended. At 60 Hz steps and 144–1000 fps most frames run no step, so a reader that runs before the sender in a step (or any reader of events sent outside the steps) loses them when a frame without a step rotates them out. Example 01's only such reader is the HUD's `update_text_display` (`Slot::BeforeStep`, `EventQueue::len`); its contact readers (`ground_detection`, `small_ball_impacts`, `spread_ball_fire`) read `iter_current` after `physics_step` and already see each step's events once. Collision events fire once per penetrating contact per substep.

The player (`PLAYER_HALF` 20×28) sinks about 0.27 px into a slab; at a join the next 64×23 slab (`SLAB_COLLIDERS`, `level_layout.rs:1618`, one static AABB each via `gameplay::spawn_platform_colliders`) overlaps it by 0.25 px sideways, the smaller axis, so the solver pushes it back as from a wall. At 30–144 Hz steps the player stops at x = 1580.25; at 240 and 480 Hz it crosses.

`Time` runs at most `max_steps_per_frame` (2) steps of `fixed_step` (1/60 s, `TimeConfig` 60 Hz) a frame and drops whole steps past the bound (`D-129`, `D-137`); smoke runs pin dt to 1/60 s, so they run one step with `alpha` 0. `02_bench` registers no `fixed_update` system and steps physics in `update`, so its rows and digests do not run the fixed loop. The physics hash tests call `physics_step` directly.

## Files to touch

- `crates/tungsten-core/src/ecs/event_queue.rs`, `crates/tungsten-core/src/ecs/world.rs`, `crates/tungsten-core/src/tests/ecs/event_queue.rs` (step 2)
- `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/fixed_step.rs` (step 2; step 3 only if a default changes)
- `examples/01_platformer/src/tests/regime.rs` (new), `examples/01_platformer/src/tests/main.rs` (steps 1, 2, 4)
- `examples/01_platformer/src/systems.rs`, `examples/01_platformer/src/setup.rs` (step 2's HUD fix, only if needed)
- `examples/01_platformer/src/gameplay.rs` (step 4), `examples/01_platformer/src/tests/level.rs` (step 4, if a slab test belongs there)
- `api/tungsten-core.txt`, `api/tungsten.txt` (`just api`), `docs/plans/1.0/w04-api-freeze.md` (break-ledger row only)
- `DECISIONS.md`, `docs/DECISION_INDEX.md` (`D-139`), `DESIGN.md` (events paragraph and status line), `docs/getting-started.md` (the fixed-stage event rule), `docs/known-issues.md`, `CHANGELOG.md`
- `crates/tungsten-core/src/tests/ecs/world.rs` (step 2: the hold through `World`)
- Release (step 5): what `just release-cut` and the finalize skill write; this plan moves to `docs/plans/archive/fixed-step-health-pass.md`
- Machine-local evidence: `perf-runs/20261007-fixed-step-health/` (ignored by Git)

## Steps

### 1. Regime test, failing first

Add `tests/regime.rs`: the game as `main` builds it (`App::new(Config::default())`, `configure_app`, the authored map) on the harness at frame dt 1/30, 1/60, 1/144 and 1/1000 s, default 60 Hz step. Three probes: `regime_before` (`fixed_update`, before `PHYSICS_STEP`, `iter()`), `regime_after` (after `PHYSICS_STEP`, `iter_current()`), `regime_sent` (`post_update`, the frame view's `iter_current()`). The player is teleported (`Position` and `PrevPosition`) onto the y = 1920 slab row at x = 1560, settles, then holds `move_right`. Per regime, checks:

- (a) seams: the player's x passes 1860 while it stays on the row (all five joins crossed);
- (b) the after-reader's events, concatenated, equal the sent events: each exactly once, in order;
- (c) the before-reader's events are a prefix of the sent events, missing at most the last stepping frame's: none lost, none twice;
- (d) Contacts: each HUD refresh while on the row reads a nonzero count that equals one step's events, the refresh frame's steps or the step before them.

The test prints one line per regime and fails listing every failed check. Expected before any fix: (a) fails in all four (x stops at 1580.25), (c) fails at 1/144 and 1/1000, (d) fails at 1/30, 1/144 and 1/1000; (b) passes in all four.

Done-when: `cargo test -p example-01-platformer regime -- --nocapture` prints `FAILED` with exactly those failures; `cargo test -p example-01-platformer` otherwise passes (`test result: FAILED. N passed; 1 failed`).

### 2. Event retention (finding 1)

Owner question, settled on the recommended default (implicit approval): **hold, bounded**, not a reader cursor. A frame that runs no fixed step while the game clock advances, in an app whose `fixed_update` has a system, holds each queue's events for the next step: its flush moves `previous` into a held window instead of dropping it, and the next frame's first step reads held, then `previous`, then `current` through `iter`, `len` and `is_empty`. `iter_current`, later steps and the frame view (every reader outside the steps, `Harness::events`) are unchanged, so at the 1/60 s pin nothing moves. A frame with a step, or with no game time (paused, scale 0), drops the held events as before; at most 256 held frames with events, the oldest dropped first (slow motion). New public `World::hold_events_for_fixed_step()`, which `App` calls in a frame with no step. A cursor would need a new public type, a cursor in every reader and the same retention past the flush, so it is out of proportion here.

Then, only if the regime test's (d) still fails (expected at 1/30 s: a refresh in a frame's second step reads 0, one in its first reads two steps' events): move `update_text_display` to `Slot::AfterStep`, after the contact readers, reading `iter_current().count()`, the step's own collisions; update `RUNTIME_SCHEDULE` and drop the known-issues HUD row.

Records: `D-139` (tungsten-decision; amends `D-040`, `D-137`) with its index row; `DESIGN.md`'s events paragraph; `just api`; a break-ledger row (a behaviour change and an addition, not a compile break); the known-issues M41 Q12 follow-up removed; unit tests for hold, drop on a stepped or still frame, the frame view and the cap; one umbrella test through the app at 1/144 s and paused.

Done-when:
- `cargo test -p tungsten-core event_queue` and `cargo test -p tungsten --features testing fixed_step` pass with the new tests (`test result: ok`).
- `cargo test -p example-01-platformer regime -- --nocapture`: (b), (c) and (d) pass in all four regimes; (a) still fails (`FAILED`, seams only).
- `just api` then `git diff --stat api/` shows `api/tungsten-core.txt` changed by the one new method, listed once per path (`World` is listed twice); `rg -n 'hold_events_for_fixed_step' api/tungsten-core.txt` prints those lines.
- `rg -n '^\| `D-139`' docs/DECISION_INDEX.md` prints one row.

### 3. Fixed-step policy (findings 2–4)

Measure with the 3,000-ball pile at 60, 120 and 240 Hz on the harness, in a release build under `CARGO_TARGET_DIR=target/health-probe` (deleted after), with a probe kept outside the tree afterwards (`perf-runs/20261007-fixed-step-health/`): the seam stall per rate on the original slabs (stop x), the summed `fixed_update` system time per game second (three interleaved repeats, medians) and `spread_ball_fire`'s cost. Finding 4: headless, the per-frame drawn motion at 144 and 1000 fps of the player, a ball and everything example 01 animates from its step clock (`SceneTime`); in the window, a short uncapped run of example 01 with `TUNGSTEN_PERF_LOG` for the step and non-step frame costs, only while NoMachine is disconnected (`pgrep -x nxcodec.bin` empty; else list the command for the owner). `just physics-release` for the hash.

Owner question, settled on the recommended default from the numbers (implicit approval): the windowed default `fixed_step_hz` and `max_steps_per_frame`, smoke runs and benchmarks staying at 1/60 s. A change to the physics hash is a stop, not a default.

Done-when: the evidence row gives the stop x and cost per game second per rate, the jitter verdict and the decision; `just physics-release` prints `0x088ec07a73c1b168` and `0xaee272e01ffc4e4c` unchanged (`test result: ok`); `D-139` records the defaults; `target/health-probe` is gone (`fd -td -d1 health-probe target` prints nothing).

### 4. Seams

`spawn_platform_colliders` spawns one static collider per run of slabs that share a row and height and touch end to end, so no join exists inside a platform. A test in the example: the spawned slab colliders cover exactly the slabs' area and no two share an edge. Known issues: a row for the solver's ghost collision at joins between static boxes (finding 2, with the triage's rates), the `spread_ball_fire` line with step 3's cost, the not-reproduced debug boxes.

Done-when: `cargo test -p example-01-platformer` passes (`test result: ok`), the regime test included, its four lines showing every check `ok`.

### 5. Release

Before it, read `docs/releases.md` for a release outside the register: if numbering it needs a register or `roadmap.json` edit, stop (implicit approval does not cover files this pass must not touch); otherwise follow the tungsten-finalize and tungsten-release skills: plan `status: done` and archived, `CHANGELOG.md` line, `just release-cut 0.57.0`, the release checks once after the cut, the read-only preflight, then the changed-file list and the command blocks.

Done-when: the release chain's commands each exit 0 (`just check`, `just ctx`, `just repo-check`, `just api-check`, `just script-test`, `just smoke`, `just notices`, `just template-check`, `git diff --check`), `just physics-release` passes, and preflight prints the command block.

## Stop conditions

A physics hash or row digest change; a benchmark row reading `regressed`; an API break without a ledger row; anything that needs a 1.0 register, README "Now" line or `roadmap.json` edit. A stop restores the step's files and records the step as skipped (workflow §2).

## Evidence log

| Step | Verdict | Numbers and paths |
| --- | --- | --- |
| 1 | done | `regime.rs` fails as expected (`FAILED. 72 passed; 1 failed`, 0.19 s for four regimes): seams fail in all four (`walk to x = 1580.25`); after-reader `ok` in all four (524, 516, 512, 512 events); before-reader `FAIL` at 1/144 and 1/1000 s (`read 0 events of 512 sent`); Contacts `FAIL` at 1/30 s (`24 (steps [12, 12, 12])`, `0 (steps [15, 12, 12])`), 1/144 and 1/1000 s (`0 (steps [12, 12])`), `ok` at 1/60 s. Debug boxes: not reproduced (one player, one collider). |
| 2 | done | Hold, bounded (default taken). `event_queue` 15 passed, `World` hold 1, umbrella `fixed_step` 12 passed; the new app test fails without the app's hold call (`each id once, in order`), checked by swapping `app.rs` back and restoring it (`cmp`). Regime after the hold: before-reader `ok` in all four; Contacts still `FAIL` at 1/30 s, so the HUD moved after the contact readers on `iter_current().count()`: regime `after ok, before ok, contacts ok` in all four, seams `FAIL` only; example suite `72 passed; 1 failed`. `just api`: `api/tungsten-core.txt` +2 (one method, two paths). `D-139` with index row and `D-040`/`D-137` markers; ledger row; `DESIGN.md` events, guide rule; known issues lose M41 Q12 and the HUD row. |
| 3 | done | Release build on the harness, 3,000-ball pile, three interleaved repeats (`perf-runs/20261007-fixed-step-health/probe-run.log`, `probe-run-2.log`, probe `step_rate_probe.rs`): `physics_step` 0.32 ms a step at 60, 120 and 240 Hz; `fixed_update` 26.3 / 52.3 / 104.0 ms per game second settling, 68.9 / 106.5 / 207.2 falling; walk on the old slabs to x = 1580.25 / 1644.25 / 1889.96 (every repeat). 12,000 balls: `physics_step` 10.25 ms a step; `spread_ball_fire` 4.70 ms a step (max 5.15) with 779 burning, 0.15 ms with 320 of 3,000. Finding 4: headless step frames 0.52 ms against 0.07 (1/1000 s, settled pile); `SceneTime` moved in 120 of 2,000 frames at 1/1000 s, 120 of 288 at 1/144 s; window (`window-run.log`, `window-summary.txt`; immediate, Vulkan, Radeon 660M): ~150 fps GPU-bound, step frames total p50 6.63 ms against 6.59, update 0.35 against 0.09, steps every 1–2 frames. Decision (default taken): keep 60 Hz and 2, `D-139` (4). `just physics-release`: `4 passed`, `1 passed`, `7 passed` (hash constants untouched). `target/health-probe` deleted (`fd` prints nothing); NoMachine absent throughout. |
| 4 | done | `slab_runs`: 155 slabs → 27 colliders, sorted by row, then x. `cargo test -p example-01-platformer`: `test result: ok. 74 passed`; regime: all four lines `after ok, before ok, contacts ok (2 refreshes), walk to x = 1896.0059 (seams ok)`; the route test and `thin_platform_underside_and_edges_match_visible_art` pass. Known issues: the ghost-collision row (P3, `gather.rs:69`), `spread_ball_fire`'s cost, `SceneTime` animation, the catch-up bound, the debug boxes not reproduced. |
| 5 | done | Numbering: `docs/releases.md` ships milestone branch `0.NN` as `0.NN.0`, and 0.44, 0.47 and 0.51 took their branch's minor outside the register, so `0.57.0` with no register or `roadmap.json` edit. Before the cut: `just check` exit 0, `just release-check` consistent; `CHANGELOG.md` `[Unreleased]` line, `DESIGN.md` status head, this plan archived. The cut, the release chain and the preflight run after the archival; their results are in the session's hand-off. |

## Follow-ups

In [known issues](../../known-issues.md): the solver's ghost collision at joins between static boxes, example 01's `SceneTime` animation at the interpolated time, a catch-up bound that knows the step's cost, `spread_ball_fire`'s cost and the debug boxes not reproduced. The README "Now" lines and the register were not touched (owner's instruction); they still describe 0.56 as the latest cut.
