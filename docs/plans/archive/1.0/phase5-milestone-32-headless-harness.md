# Phase 5 milestone 32: headless test harness (W14a) — draft

- **status:** done
- **goal:** A frame body in `App` that the window loop and a headless harness both run, and `tungsten::testing::Harness` behind the umbrella's `testing` feature: `step(n)` at a pinned dt, injected actions and cursor positions, events and resources readable between steps, and the CPU extract of each frame visible to the test. Example 01's tests that copy the frame loop by hand move onto it, and its 2,366-line test file splits by topic.
- **non-goals:** A window, surface or audio device in the harness; the startup hook and manifest loading headless (Q2); the physics determinism, containment and tunneling tests, which stay at the core level ([w14](../../1.0/w14-tooling.md)); hand-written loops outside example 01 (A16, a follow-up); W1 M1's `UiHarness`; schedules or plugins (W15a); the CLI (W14b); any change to the stage order.
- **files to touch:** Step 2: `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs`. Step 3: `crates/tungsten/Cargo.toml`, `crates/tungsten/src/lib.rs`, `crates/tungsten/src/testing.rs` (new), `crates/tungsten/src/tests/testing.rs` (new), `crates/tungsten/src/app.rs` (visibility only), `scripts/public-api.sh`, `api/tungsten.txt`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`. Step 4: `examples/01_platformer/Cargo.toml`, `examples/01_platformer/src/tests/main.rs` and new topic modules beside it in `examples/01_platformer/src/tests/`. Step 5: `examples/01_platformer/src/tests/ball_pit.rs`, `burning.rs`, `spells.rs`, `main.rs`, `docs/known-issues.md`. Step 7: `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, `docs/plans/1.0/implementation-plan.md` (register row), `docs/plans/1.0/README.md` ("Now" lines), `docs/plans/1.0/w14-tooling.md` (W14a's status), this plan (moved to `docs/plans/archive/1.0/`). Steps 1 and 6 write only under `perf-runs/` (ignored).
- **ordered steps:** (1) Baseline and A/A suites on the untouched code. (2) The frame body: `App::run_frame`, shared by the window loop. (3) The harness behind `testing`, `D-110`, its tests, the snapshot, the design and index rows. (4) Example 01: the dev-dependency, the test file split, `main.rs`'s tests on the harness. (5) `ball_pit`, `burning` and `spells` on the harness, the hand-loop helpers removed. (6) Candidate suite, visual and physics-release. (7) Release 0.43.
- **done-when:** Every step's evidence row quoted; workflow §5 closed (§8 below); the release checks pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W14a ([implementation plan](../../1.0/implementation-plan.md) §3 card, level B), milestone M32, release 0.43. Written 2026-10-04 at `28abcce` (0.42 merged; the tree held only documentation edits). Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

- W14a is first in Phase 5 Track A: every later test stands on it, and W1 M1's `UiHarness` builds on it, so M1 cannot start before it ([implementation plan](../../1.0/implementation-plan.md) §3; [w14](../../1.0/w14-tooling.md) step 1, amendment 5).
- Today the frame exists only inside `window_event`'s `RedrawRequested` arm (A1). Tests copy parts of it by hand and skip particles, tweens, most event queues and the engine systems (A11; criteria §2.2). Most stages already run without a window, renderer or audio device (A2), so the split is a move, not a rewrite.
- Seam (`D-007`, `D-016`, `D-018`): the harness lives in the umbrella and reads the extract's plain render data; core and render are untouched. `D-043` (display changes at a frame boundary) and `D-088` (the dt cap) hold for the window loop; Q1 settles the harness's pinned dt. No dependency is added (`D-015` n/a).
- The harness is new public surface the freeze covers ([w04](../../1.0/w04-api-freeze.md) scope); it breaks nothing, so the break ledger takes no row. `D-107`'s snapshot lists default features only (A10), so Q4 adds the `testing` feature to the umbrella's snapshot.
- It changes the frame loop, so every CPU row is captured ([workflow](../../1.0/workflow.md) §5 line 2), and digests must not move: the split changes no behavior.

## 1. Audit

Read at `28abcce`. `app.rs` is 1,693 lines; example 01 has 69 tests in five files (68 under `src/tests/`), passing in 9.50 s in a debug build (`cargo test -p example-01-platformer`, 2026-10-04).

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The frame body is the `RedrawRequested` arm; nothing else can run a frame | `crates/tungsten/src/app.rs:1576-1669` |
| A2 | Stages already tolerate a missing window, renderer or device: render no-ops without a renderer, audio drains its commands without a device (tested), a display request without a window syncs state and returns, hot reload returns without a watcher | `app.rs:907`, `app.rs:1013-1023`, `crates/tungsten/src/tests/app.rs:214`, `app.rs:581-584`, `app.rs:424-434` |
| A3 | Only three parts need the event loop: the engine-exit return, pacing and the smoke countdown | `app.rs:1591-1594`, `app.rs:1044-1060`, `app.rs:1063-1071` |
| A4 | dt comes only from the wall clock, capped at `MAX_DT_SECS` unless smoke-pinned; a test cannot pin it through `App` | `app.rs:743-755`, `app.rs:1124`, `app.rs:52`, `app.rs:59` |
| A5 | The extract output `FrameExtract` is private and lives only between `stage_extract`, `stage_render` and `stage_recycle` | `app.rs:698-711`, `app.rs:1603-1607`, `app.rs:1006-1010` |
| A6 | Stages are `#[inline(always)]` to keep the frame flat in debug builds; a split must keep the window path flat | `app.rs:740-742` |
| A7 | `App::new` seeds the engine resources, six engine event queues and seven engine systems, and reads `input.json` from the working directory and capture variables from the environment; example 01 has no `input.json` in its package root, so its tests get the default map | `app.rs:156-282`, `app.rs:158-159`, `app.rs:254-266` |
| A8 | Startup needs a window: `run` installs the default extract and the watcher, `resumed` creates the window and renderer, loads manifests through the renderer, runs the startup hook, whose type takes `&mut Renderer`, and starts audio | `app.rs:376-406`, `app.rs:81`, `app.rs:1392-1516` |
| A9 | The umbrella has no `[features]`; example 01 has no dev-dependencies | `crates/tungsten/Cargo.toml`, `examples/01_platformer/Cargo.toml` |
| A10 | The API snapshot builds rustdoc JSON with default features, so a `testing`-gated module would be missing from `api/tungsten.txt` | `scripts/public-api.sh:47-48` |
| A11 | Example 01's frame helpers: `physics_frame` (player input, physics, ground detection, the collision queue flush, `begin_frame`), `particle_frame` (transient cleanup, three particle stages, a command flush) and `flush_commands`; each skips tweens, the other queues and the engine systems | `examples/01_platformer/src/tests/main.rs:853-866`, `main.rs:1094-1106`, `ball_pit.rs:12-16` |
| A12 | Tests that run an engine stage by hand (command or event flush, `begin_frame`, particle stages, or the A11 helpers): in `main.rs` the tests at 199, 358, 541, 668, 696, 868, 986 (through `route_jump_possible`, 899), 1108, 1208, 1263, 1577, 1677, 1773, 1935, 1998, 2028, 2272; in `ball_pit.rs` 19, 129, 267, 315, 373; in `burning.rs` 64, 145 | the lines given (the `fn` line of each test) |
| A13 | Loops that run two or more systems per iteration with no stage: presentation and animation; input, obstacles, physics and ground detection | `main.rs:1551-1554`, `main.rs:1806-1810` |
| A14 | Loops that repeat one system to test that system's own state stay unit tests | e.g. `main.rs:1661`, `main.rs:1916`, `main.rs:2245`, `spells.rs:67`, `burning.rs:117` |
| A15 | Moving tests that set a dt above `MAX_DT_SECS` | `ball_pit.rs:24` (0.161), `ball_pit.rs:148` (0.161), `main.rs:386` (0.170), `main.rs:671` (`BLACK_HOLE_LIFETIME + 0.1`) |
| A16 | Hand loops outside example 01 | `crates/tungsten/tests/particles.rs`, `crates/tungsten/src/tests/tweens.rs`, `state.rs`, `game_feel.rs`, `examples/03_scene_state/src/states.rs` |
| A17 | The index routes the frame loop to `app.rs` and `lib.rs`; no harness row | `docs/LLM_INDEX.md:12` |
| A18 | `DESIGN.md`'s frame loop lists the redraw order and no headless path | `DESIGN.md:55-72` |
| A19 | Known issues record the test file's length (2,372 then; 2,366 now) | `docs/known-issues.md:45` |
| A20 | `pgrep -af scripts/bench.py`, as workflow §6 and the skill write it, matches the agent's own `bash -c` line, so it always reports a capture; `pgrep -af '[s]cripts/bench.py'` does not | run 2026-10-04 in this session |
| A21 | The A12 audio test asserts on the commands `audio_input_system` queued, but a frame's audio stage drains them whether or not a device exists, so they are gone after a frame | `main.rs:1597-1600`, `app.rs:1015-1020` |

## 2. Design

**Frame body (step 2).** One method runs a frame for both callers:

```rust
/// Where a frame's dt comes from.
pub(crate) enum FrameClock { Wall, Pinned(f32) }
/// How a frame ended.
pub(crate) enum FrameEnd { Completed, ExitRequested }

#[inline(always)]
pub(crate) fn run_frame(&mut self, frame_start: Instant, clock: FrameClock,
                        inspect: impl FnOnce(&FrameExtract)) -> FrameEnd
```

It holds the arm's body from the interval measurement through the perf log, in today's order (`DESIGN.md:65-70`). `FrameClock::Wall` keeps `stage_delta_time` as it is; `Pinned(dt)` writes `dt` to `DeltaTime` at the same point (Q1). The engine-exit check returns `ExitRequested` where the arm returns today. `inspect` runs between render and recycle. The arm becomes: `run_frame(now, Wall, |_| {})`, then `event_loop.exit()` on `ExitRequested`, else pacing and the smoke countdown. `#[inline(always)]` keeps the window path as flat as today (A6) and avoids a second caller outlining it (the shipped build has one caller; tests have a second instantiation). `stage_audio` forwards commands to the device as today; with no device and a `pub(crate)` capture list set, which only the harness sets, it appends them there instead of dropping them (A2, A21), so a test can read what a frame would have played. No public item changes in this step.

**Harness (step 3).** `crates/tungsten/src/testing.rs`, compiled under `#[cfg(any(test, feature = "testing"))]`, with `#![deny(missing_docs)]` so `just check` enforces its rustdoc:

```rust
pub struct Harness { /* App, dt, last draw, frames, exit flag */ }
impl Harness {
    pub fn new(app: App) -> Self;               // default extracts installed; no window, renderer,
                                                // audio, watcher, startup hook or manifests (Q2)
    pub fn set_dt(&mut self, dt: f32);          // default 1/60 s, as given (Q1); panics on a
                                                // NaN, infinite or negative dt
    pub fn step(&mut self, frames: u32) -> u32; // frames completed; stops at an exit request
    pub fn press_action(&mut self, action: &str);   // presses the action's first binding
    pub fn release_action(&mut self, action: &str);
    pub fn set_cursor(&mut self, x: f32, y: f32);
    pub fn world(&self) -> &World;
    pub fn world_mut(&mut self) -> &mut World;
    pub fn events<T: 'static>(&self) -> impl Iterator<Item = &T>; // the last frame's, after rotation
    pub fn draw(&self) -> Option<&FrameDraw>;   // the last completed frame's extract
    pub fn audio(&self) -> &[AudioCommand];     // the commands the last frame would have played
    pub fn exit_requested(&self) -> bool;
    pub fn frame(&self) -> u64;
}
#[non_exhaustive]
pub struct FrameDraw { pub quads, pub sprites, pub text, pub debug_quads, pub debug_lines,
                       pub light_ubo, pub mesh_particles }   // render's plain data types
```

Input goes in between steps, as winit events do between redraws, and the frame's `begin_frame` clears the edges as in the window loop. `App` gains no public item; the harness reaches it through `pub(crate)` (`run_frame`, `install_default_extracts`, a `world()` reader). The feature is off by default; example 01 turns it on as a dev-dependency feature, so `cargo test --workspace` unifies it in and shipped and perf builds never compile the module. Names are Q6's.

**Seam.** Unchanged: the extract still reads `&World` and hands plain data on (`D-018`); `FrameDraw` holds the same `tungsten_render` types the `set_extract_*` setters already expose.

**Test moves (steps 4–5).** A test moves when it runs an engine stage by hand or loops two or more systems per frame (A12, A13; Q3). It builds `App::new(Config::default())`, seeds the world as `seed_world` does today, registers the systems its hand loop ran with `add_system_named`, wraps it in a `Harness` and steps it. A test helper in `tests/main.rs` builds that harness. A moved test keeps every assertion it had, reading the same state through `world()`, `events()`, `draw()` or `audio()`. A changed expectation is a changed value, never a dropped assertion, and is legitimate only when a stage the hand loop skipped explains it; the evidence row lists each with that stage. A manual flush that only ages an event for the test's setup becomes a step taken before the consumer has anything to act on: `contact_spread_is_symmetric_and_only_uses_current_events` sends the stale contact, steps once while no ball burns (`spread_ball_fire` returns early without a burning source, `examples/01_platformer/src/burning.rs:63-65`), then ignites the source, sends the current contacts and steps again. Single-system tests stay (A14). `tests/main.rs` splits by topic into modules of at most 800 lines each, with the shared helpers left in `main.rs` (Q5).

## 3. Steps

### Step 1: Baseline and A/A suites

- **Files:** none in the tree; captures under `perf-runs/`.
- **Change:** before any code edit, wait for `pgrep -x nxcodec.bin` to find nothing and check that `pgrep -af '[s]cripts/bench.py'` and `pgrep -x 'cargo|rustc'` find nothing (A20). Capture the suite, save it as `w14a-pre`, then capture it again on the same build and compare. Each suite is one blocking foreground command. Record example 01's test names (`cargo test -p example-01-platformer -q -- --list`, last path segment, sorted) to the scratchpad for steps 4–5.
- **Done-when:** `WGPU_BACKEND=vulkan just perf suite --repeat 5` → exit 0, every row valid; `just perf baseline save <suite> w14a-pre` → saved; `WGPU_BACKEND=vulkan just perf suite --repeat 5 --compare w14a-pre` → exit 0, no owned metric `regressed` or `improved` beyond the A/A exceptions the [compare rules](../../../perf/profiling-workflow.md#compare) list (`gpu-throughput` `render_encode`, the `ecs` per-run-mode rows), every row's first-run digest matching.
- **Moves:** none.

### Step 2: The frame body

- **Files:** `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs`.
- **Change:** add `FrameClock`, `FrameEnd` and `run_frame` (§2), make `FrameExtract` `pub(crate)` so the signature passes the `private_interfaces` lint, move the arm's body into `run_frame` unchanged in order, and reduce the arm to the call, the exit and pacing. Add the audio capture list to `stage_audio` (§2). Add tests in `tests/app.rs` that call `run_frame` on an `App` with no window: a pinned dt reaches a system, a command queued by a system is applied after the frame, a registered event rotates out after the next frame, `inspect` receives a user quad extract's output, an audio command queued by a system lands in the capture list when one is set and is dropped as today when none is, and a pressed `engine_exit` binding returns `ExitRequested`.
- **Done-when:** `cargo test -p tungsten --locked -q app::tests` → passes, the new tests included; `just check` → passes; `just smoke` → passes (the window path); `rg -n 'fn run_frame' crates/tungsten/src/app.rs` → one match, and the `RedrawRequested` arm calls it.
- **Moves:** none.

### Step 3: The harness, its decision and its records

- **Files:** `crates/tungsten/Cargo.toml` (`[features] testing = []`), `crates/tungsten/src/lib.rs`, `crates/tungsten/src/testing.rs`, `crates/tungsten/src/tests/testing.rs`, `crates/tungsten/src/app.rs` (`pub(crate)` only), `DECISIONS.md`, `docs/DECISION_INDEX.md`, `scripts/public-api.sh`, `api/tungsten.txt`, `DESIGN.md`, `docs/LLM_INDEX.md`.
- **Change:** write `D-110` with the tungsten-decision skill first (§5), with its index row and the marker lines on `D-088` and `D-107`. Add the harness (§2) and its tests: (a) frames observe particles, tweens, the command flush and event rotation, which the hand loops skipped; (b) `draw()` holds what the frame would draw (a sprite at its transform, a user extract's quads and text); (c) the pinned dt, including 0.25 s, reaches systems as given; (d) `press_action` gives one `just_pressed` frame and `set_cursor` reaches `InputState`; (e) an `engine_exit` press stops `step`; (f) `audio()` holds the commands a frame's systems queued, and the next frame clears them; (g) `set_dt` panics on a NaN, infinite or negative dt and accepts zero. Pass `--features testing` to the umbrella's `cargo rustdoc` call in `scripts/public-api.sh` and run `just api`. Add the harness sentence to `DESIGN.md`'s frame loop (the frame body, its two callers and `D-110`) and a `docs/LLM_INDEX.md` row (harness, `D-110`: `tungsten/testing.rs`, `tungsten/app.rs`, `crates/tungsten/src/tests/testing.rs`).
- **Done-when:** `cargo test -p tungsten --locked -q testing` → tests (a)–(g) pass; `just check` → passes (clippy enforces the module's `deny(missing_docs)`); `just api && git --no-pager diff --unified=0 api/ | rg '^-[^-]'` → no output, and `rg -c 'tungsten::testing::Harness' api/tungsten.txt` → at least 1; `git diff --quiet HEAD -- api/tungsten-core.txt api/tungsten-render.txt` → exit 0; `just script-test`, `just ctx` and `just repo-check` → pass; `rg -c 'D-110' docs/DECISION_INDEX.md` → 1.
- **Moves:** `api/tungsten.txt` gains the `testing` module; nothing else in `api/`.

### Step 4: Example 01's `main.rs` tests on the harness

- **Files:** `examples/01_platformer/Cargo.toml` (`[dev-dependencies] tungsten = { workspace = true, features = ["testing"] }`), `examples/01_platformer/src/tests/main.rs`, new topic modules in `examples/01_platformer/src/tests/`.
- **Change:** add the harness builder helper to `tests/main.rs`; move the A12 and A13 tests of `main.rs` onto it; split `main.rs` by topic (Q5). Keep `physics_frame` and `particle_frame` until step 5, since `ball_pit.rs` and `burning.rs` still call them.
- **Done-when:** `cargo test -p example-01-platformer --locked -q` → passes, with its wall time recorded; the test names from step 1, by last path segment, are all still listed (`comm -23 <before> <after>` → empty); `wc -l examples/01_platformer/src/tests/*.rs` → none over 800; each A12 and A13 test of `main.rs` builds a `Harness` (listed in the evidence row with every changed expectation and the stage that explains it); `cargo tree -p example-01-platformer -e features,no-dev -i tungsten | rg -c '"testing"'` → 0 and `cargo tree -p example-01-platformer -e features -i tungsten | rg -c '"testing"'` → at least 1; `git diff --quiet HEAD -- Cargo.lock crates/tungsten-core/tests` → exit 0; `just check` → passes.
- **Moves:** none; test expectations only as recorded.

### Step 5: The remaining tests and the helpers

- **Files:** `examples/01_platformer/src/tests/ball_pit.rs`, `burning.rs`, `spells.rs`, `main.rs`, `docs/known-issues.md`.
- **Change:** move the A12 tests of `ball_pit.rs` and `burning.rs` onto the harness, the stale-contact test as §2 describes; delete `physics_frame`, `particle_frame` and `flush_commands`. `spells.rs` keeps its single-system tests (A14) and is listed only for the helpers it may import. In known issues, drop the test file's length from line 45 and add the A16 hand loops, with W15a or the Phase 5 QA pass as their home.
- **Done-when:** `rg -n 'world\.flush\(|\.flush\(\)|begin_frame\(\)|particle_(count_refresh|emit|tick)_system|tween_tick_system|fn (physics_frame|particle_frame|flush_commands)' examples/01_platformer/src/tests` → no output; `cargo test -p example-01-platformer --locked -q` → passes, wall time recorded; the step 1 name check → empty; `wc -l examples/01_platformer/src/tests/*.rs` → none over 800; `git diff --quiet HEAD -- crates/tungsten-core/tests` → exit 0; `just check` → passes.
- **Moves:** none; test expectations only as recorded.

### Step 6: Candidate suite and invariants

- **Files:** none in the tree; captures under `perf-runs/`.
- **Change:** with the encoder gone and nothing else running (step 1's checks), capture the suite against `w14a-pre` in one blocking foreground command; then, after it, the pixel tests and the physics release tests, which build.
- **Done-when:** `WGPU_BACKEND=vulkan just perf suite --repeat 5 --compare w14a-pre` → exit 0; no owned metric of `physics`, `physics-sparse`, `ecs`, `churn`, `particles`, `integrated` and `gpu-throughput`, nor `gpu`'s `extract` and `render_encode`, reads `regressed`; the suite compare lists no row whose first-run digest differs; `just visual` → passes; `just physics-release` → passes. A metric the compare rules list as an A/A exception (`gpu-throughput` `render_encode`, the `ecs` per-run-mode rows) that reads `regressed` is captured again with `--only <row> --repeat 5 --compare w14a-pre` before the stop condition applies.
- **Moves:** none.

### Step 7: Release 0.43

- **Files:** `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README "Now" lines, W14a's status in `docs/plans/1.0/w14-tooling.md`, this plan (moved to `docs/plans/archive/1.0/`, its relative links fixed for the new depth)
- **Change:** close workflow §5 (§8 below) and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.43.0` once, every release check once after it (steps 2, 3 and 6 ran smoke, script-test, visual and physics-release, so no extra gate), then the preflight with `--message`
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.43.0 --repo JosephGibson/Tungsten --message 'Update 0.43: headless test harness (W14a, D-110)'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt, the plan prompt for W2 R0: `Use the tungsten-milestone skill. Write the milestone plan for W2 R0: read docs/plans/1.0/workflow.md, its card in docs/plans/1.0/implementation-plan.md and docs/plans/1.0/criteria.md §5 (W2); audit only the code it touches; name it docs/plans/1.0/phase5-milestone-33-interned-asset-ids.md; critique it last as the skill's step 7 says (--force is confirmed for the plan file); stop for my approval and end with the approval prompt. Don't commit: leave the changes in the tree for the release commit.`

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Is the harness's pinned dt capped at `MAX_DT_SECS` (0.1 s)? | No: written as given, like the smoke pin, so the A15 tests keep their dt; `D-110` amends `D-088` to say so. Capping it would make a harness frame match the window loop exactly, and the A15 tests would change their dt or expectations |
| Q2 | Does the harness run the startup hook and load manifest roots? | No: the hook's type takes `&mut Renderer` and manifest loading uploads through it (A8). Tests seed resources through `world_mut()`; a headless asset path is a follow-up for W15a's plugins or W12a |
| Q3 | Which example 01 tests move? | Those that run an engine stage by hand or loop two or more systems per frame (A12, A13); single-system tests stay unit tests (A14) |
| Q4 | Does the API snapshot include the `testing` feature? | Yes: the umbrella's `cargo rustdoc` call gets `--features testing`, so the harness shows in `api/tungsten.txt` and the freeze sees it; `D-110` amends `D-107` |
| Q5 | How does `tests/main.rs` split? | By topic (player and physics, spawning and black holes, camera, level routes and platforms, presentation and particles, hazards and fire), each module at most 800 lines, the shared helpers in `main.rs` |
| Q6 | The harness's names | `tungsten::testing::{Harness, FrameDraw}` with the methods in §2; W4b may rename them before the freeze |
| Q7 | Who enables `testing`? | Example 01 only, as a dev-dependency feature; other examples when their tests move (A16) |
| Q8 | Run mode | Level B, one unattended run: steps 1 and 6 wait for the encoder to exit, and each suite is one blocking foreground command |

Approved 2026-10-04 with the stated defaults.

## 5. Decisions expected

`D-110`: Headless harness behind the umbrella's `testing` feature: `tungsten::testing::Harness` runs `App::run_frame`, the frame body the window loop runs, with no window, renderer, audio device, watcher, startup hook or manifests; a pinned dt written as given (amends `D-088`, Q1); the feature off by default and enabled by examples as a dev-dependency feature; the umbrella's API snapshot lists it (amends `D-107`, Q4). No new dependency. Next free ID on 2026-10-04; checked again in step 3.

## 6. Invariants

- **May move:** `api/tungsten.txt`, by additions only (step 3); example 01's test expectations, each recorded with the stage that explains it (steps 4–5).
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post and transition regressions; the stage order in `DESIGN.md`'s frame loop; `api/tungsten-core.txt` and `api/tungsten-render.txt`; the core physics tests (`crates/tungsten-core/tests/`); example 01's test names; `Cargo.lock` until the release cut, whose `cargo update --workspace --offline` changes only the workspace packages' versions (`justfile:104-106`).

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Step 3 needs 2; step 4 needs 3; step 5 needs 4; step 6 needs 2 and runs whether or not 3–5 did. Step 7 needs all of 2–6: the harness and the moved tests are W14a's deliverable ([w14](../../1.0/w14-tooling.md) step 1), so after any skipped step the run stops before the release and leaves it to a release session once the owner has seen why ([workflow](../../1.0/workflow.md) §2).
- An owned CPU metric reads `regressed` in step 6. The placement check of the [regression policy](../../../perf/profiling-workflow.md#tracked-rows-suites-and-regression-policy) (the baseline tree plus a function no frame calls, run once) applies only to metrics of code this milestone does not change, as that policy says: the system rows (`physics_step`, the `ecs` and `churn` system rows, `animate_sprites`) and `churn` `flush`, which it names. If it reproduces the verdict, record both compares as the justification and go on. Any other owned metric (`update`, `unattributed`, `extract`, `render_encode`, `total`, jitter) measures the frame this milestone moved: stop before the release.
- A row digest changes, or `just smoke`, `just visual` or `just physics-release` fails: stop before the release.
- A moved test fails for a reason no skipped stage explains (an engine finding): restore that test file, record the finding, and skip the step.
- The harness needs a public change to `App`, core or render, or a file outside §files to touch: stop, and add an open question.
- `cargo test -p example-01-platformer` takes more than 30 s (about three times today's 9.5 s): stop after the step and ask whether the harness should run the extract only on request.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 2, 3, 6, release | Smoke in 2; script-test in 3; the suite against `w14a-pre`, visual and physics-release in 6; the release checks after the cut |
| 3 Invariants | 3, 4, 5, 6 | Snapshot diffs in 3; core tests and test names in 4–5; digests, visual and physics hashes in 6 |
| 4 Decisions | 3 | `D-110` and its marker lines before `DESIGN.md` and the index cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 3 | No break, so no ledger row; the harness is the new surface w04's scope already names; `deny(missing_docs)` on the module; `just api` |
| 6 Template and guide | — | n/a before W12a |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 3 | The harness row |
| 8 Design | 3 | `DESIGN.md` frame loop: the frame body's two callers |
| 9 Records (known issues, backlog, gap log, register, workstream file, README "Now") | 5, release | Known issues in 5; the register, w14's W14a status and the README in the release; no backlog cut; no gap log before the game |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo` (gpt-6.1-sol through the Codex CLI, OpenAI family), 2026-10-04, reading the tree at `28abcce` plus this session's uncommitted documentation edits.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Failed harness implementation can still be marked complete and released | §7 as written; `w14-tooling.md:48`; workflow §2 (execution sessions) | Accepted | §7: step 7 needs all of 2–6, and a skipped step stops the run before the release |
| 2 | [MEDIUM] The audio-test migration has no way to preserve its command assertion between steps | `main.rs:1597-1600`, `app.rs:1015-1020` | Accepted | A21; §2: `stage_audio` appends to a capture list the harness sets, and `Harness::audio()` reads it; moved tests keep every assertion; step 2 and step 3 gain a test each |
| 3 | [MEDIUM] The placement waiver extends beyond the regression policy it cites | `docs/perf/profiling-workflow.md:210` | Accepted | §7: the check applies to the system rows and `churn` `flush` only; any frame-level metric that regresses stops the release |
| 4 | [MEDIUM] The release necessarily violates the stated lockfile invariant | `justfile:104-106`, `Cargo.lock:2961` | Accepted | §6: unchanged until the cut, which moves only the workspace versions |

Round 2, required because round 1 changed the done-when checks of steps 2 and 3: the same reviewer, 2026-10-04, on the revised plan. There is no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] “Pinned dt written as given” admits values that can break frame execution | `examples/01_platformer/src/systems.rs:624-628`, `crates/tungsten/src/tweens.rs:19-21` | Accepted | §2: `set_dt` panics on a NaN, infinite or negative dt; step 3 gains test (g) |
| 2.2 | [MEDIUM] The stale-contact test cannot migrate by simply replacing its flush with a harness step | `examples/01_platformer/src/tests/burning.rs:64-90`, `examples/01_platformer/src/burning.rs:60-75` | Accepted | §2 and step 5: the stale contact ages in a first step taken before any ball burns, so the test moves without a stop |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-04 | Skipped: the A/A check failed. The baseline is valid and saved as `w14a-pre` | Three sittings before it went invalid: the remote-desktop encoder came back for 3–31 s at 04:22, 04:25 and 04:32 (`particles` runs 3–5, `gpu` run 3, a `gpu-throughput` diagnostic run). The fourth, `suite` → exit 0, all 8 rows valid; `baseline save` → "Saved baseline w14a-pre". A/A `suite --repeat 5 --compare w14a-pre` → exit 0, valid, "Owned verdicts, all rows: 3 regressed, 0 improved, 38 unchanged, 13 noisy", "First-run digests match in 8 of 8 rows". Regressed beyond the listed exceptions: `gpu` `extract` p95 0.83 → 0.94 ms (+13.0%, [+0.01, +0.21], τ 0.050); `particles` `unattributed` p50 2.10 → 2.17 ms (+3.5%, [+0.01, +0.14], τ 0.063); `particles` `animate_sprites` p95 0.43 → 0.46 ms (+6.5%, [+0.00, +0.05], τ 0.022). Every A/A run sits above the baseline's runs, so a sitting-level shift, not one outlier (`animate_sprites` p95 0.42–0.46 → 0.45–0.49). The encoder ran 04:43:32–04:44:03, ending 2 s before the A/A sitting began. 69 test names recorded | `perf-runs/20261004T073936Z-suite` (baseline), `perf-runs/20261004T074405Z-suite` (A/A), `perf-runs/20261004T074756Z-compare-suite`; invalid: `20261004T071938Z-suite`, `20261004T072346Z-suite`, `20261004T073014Z-suite` |
| 2 | 2026-10-04 | Done | `cargo test -p tungsten --locked -q app::tests` → "27 passed; 0 failed", the six new tests included (`a_pinned_dt_reaches_systems_as_given`, `a_command_a_system_queues_is_applied_by_the_end_of_its_frame`, `a_registered_event_rotates_out_after_the_next_frame`, `inspect_sees_the_frames_user_quad_extract`, `audio_a_frame_queues_lands_in_the_capture_list_only_when_one_is_set`, `a_pressed_engine_exit_binding_ends_the_frame_after_update`); `just check` → exit 0, 931 passed, 0 failed, 5 ignored; `just smoke` → exit 0: examples 4/4, matrix 4/4, post-stack 2/2, post-AA 1/1, bloom 1/1, lighting 1/1, game-feel 2/2, mesh/transition 5/5, benchmarks 15/15, frame cap 1/1; `rg -n 'fn run_frame'` → one match, `app.rs:1082`, and the `RedrawRequested` arm calls it at `app.rs:1715`. `FrameClock::Pinned` carries `cfg_attr(not(test), allow(dead_code))` until step 3's harness is its caller | `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs` |
| 3 | 2026-10-04 | Done, one check mis-specified | `D-110` written with the tungsten-decision skill (next free ID on 2026-10-04, after `D-109`), marker lines on `D-088` and `D-107`, index rows, then the code and docs. `cargo test -p tungsten --locked -q testing` → "10 passed; 0 failed" ((a) `frames_run_particles_tweens_the_command_flush_and_event_rotation`, (b) `draw_holds_what_the_frame_would_draw`, (c) `a_pinned_dt_reaches_systems_as_given_above_the_window_cap`, (d) `press_action_gives_one_just_pressed_frame_and_set_cursor_reaches_input`, (e) `an_engine_exit_press_stops_step`, (f) `audio_holds_the_last_frames_commands`, (g) `set_dt_accepts_zero` and three `set_dt_rejects_*`); `just check` → exit 0, 941 passed, 0 failed, 5 ignored; a scratch probe that drops one method's rustdoc fails clippy with "missing documentation for a method"; `just api && git --no-pager diff --unified=0 api/ \| rg '^-[^-]'` → no output (38 lines added to `api/tungsten.txt`); `rg -c 'tungsten::testing::Harness' api/tungsten.txt` → 22; `git diff --quiet HEAD -- api/tungsten-core.txt api/tungsten-render.txt` → exit 0; `just script-test` → exit 0; `just ctx` → "Agent context checks: OK" (`LLM_INDEX.md` 8,619 B); `just repo-check` → "Repository QA: 0 error(s)". `rg -c 'D-110' docs/DECISION_INDEX.md` → 3, not the 1 written: the skill this step follows adds "Amended by `D-110`" to the `D-088` and `D-107` rows, as `D-107` did to `D-104`'s, so no correct index prints 1; `rg -c '^\| `D-110` ' docs/DECISION_INDEX.md` → 1. Kept the skill's index and went on rather than restoring the step; the owner can overrule from the step 3 copies | `crates/tungsten/Cargo.toml`, `crates/tungsten/src/{lib,testing,app}.rs`, `crates/tungsten/src/tests/testing.rs`, `scripts/public-api.sh`, `api/tungsten.txt`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md` |
| 4 | 2026-10-04 | Done | `cargo test -p example-01-platformer --locked -q` → "69 passed; 0 failed", wall 9.88 s (the test binary reports 9.71 s; 9.50 s before); step 1's names by last segment → `comm -23` empty, 69 of 69; `wc -l` → largest `presentation.rs` 681, then `spawning.rs` 432, `ball_pit.rs` 409; `cargo tree … -e features,no-dev -i tungsten \| rg -c '"testing"'` → no output (0), with dev-dependencies → 1; `git diff --quiet HEAD -- Cargo.lock crates/tungsten-core/tests` → exit 0; `just check` → exit 0, 941 passed. The 19 A12/A13 tests of `main.rs` each build a `Harness` through `platformer_harness` (checked by script): player `player_becomes_grounded_after_falling_onto_tilemap`, `real_map_dimensions_spawn_and_fall_reset`, `double_jump_needs_a_fresh_press_emits_once_and_refills_on_landing`; spawning `spawn_ball_system_spawns_at_fixed_rate_while_held`, `spawn_black_hole_system_drags_active_hole_to_cursor_while_held`, `black_hole_lifetime_system_despawns_expired_hole`, `despawn_out_of_bounds_culls_escaped_balls_and_keeps_in_bounds_balls`, `only_small_marbles_shift_hue_while_orbs_render_untinted`; camera `zoom_input_reaches_and_clamps_expanded_range`; level `authored_routes_and_recovery_shelves_traverse_with_real_physics` (through `route_jump_possible`), `platforms_support_riders_through_a_complete_cycle_with_real_physics`; presentation `animation_transitions_hold_finish_face_and_interrupt_landing`, `jump_landing_bursts_fire_once_stay_at_event_and_cleanup`, `particle_caps_and_ambient_placements_remain_bounded`, `landing_clip_completes_without_resetting_idle_and_props_stay_synchronized`, `player_lantern_toggle_controls_halo_and_native_light_and_tracks_facing`; hazards `audio_controls_and_damage_flash_shake_remain_wired`, `moving_fire_sweeps_fast_balls_once_and_explosions_do_not_hurt_players`, `moving_fire_emits_a_bounded_particle_trail`. Changed expectations: none; every assertion and value kept. Assertions the hand loops made in mid-frame read a probe system registered at that point: `record_after_input` after `player_input` (the double jump's `air_jump_used`, pending effect and exact jump velocity) and `record_queued_commands` after `despawn_out_of_bounds` (the one queued despawn). The audio test reads `Harness::audio()` (A21). A single-system call the original made outside its loop stays a direct call on the harness's world: `despawn_out_of_bounds` after the fall, the last `player_presentation_system` of the landing clip, `scene_effects` beside the lantern toggles, and the damage half of the audio test | `examples/01_platformer/Cargo.toml`, `examples/01_platformer/src/tests/{main,player,spawning,camera,level,presentation,hazards}.rs` |
| 5 | 2026-10-04 | Done | The helper `rg` → no output (exit 1); `cargo test -p example-01-platformer --locked -q` → "69 passed; 0 failed", wall 9.81 s (the binary reports 9.64 s); names → `comm -23` empty; `wc -l` → largest `presentation.rs` 681; `git diff --quiet HEAD -- crates/tungsten-core/tests` → exit 0; `just check` → exit 0, 941 passed; `just repo-check` → 0 errors; `just ctx` → OK. Moved: `ball_pit.rs` `middle_mouse_has_five_times_the_rate_half_size_and_dedicated_animation`, `spawning_stops_at_the_ball_cap`, `relative_body_impacts_work_on_b_side_and_dense_contacts_remain_bounded`, `impact_particles_span_rainbow_and_vortex_births_spiral_inward`, `pit_contains_two_thousand_mixed_balls_and_fast_wall_impacts`; `burning.rs` `contact_spread_is_symmetric_and_only_uses_current_events` (the stale contact ages in a first step before the source ignites, as §2 says), `burning_particles_use_a_rotating_bounded_pool_and_drain_after_burnout`. `physics_frame`, `particle_frame` and `flush_commands` deleted. Changed expectations: none. The 0.161 s cases keep their dt (Q1). Direct `scene_effects`, `ball_fire_particles` and `tick_ball_fire` calls between frames stay as the originals made them, with a `set_dt` helper writing `DeltaTime` for them as the originals did. `spells.rs` unchanged (A14). Known issues: the 2,372-line note dropped, the A16 hand loops added (home W15a or the Phase 5 QA pass), and two `app.rs` line citations moved with the code (`:443` → `:454`, `:1631` → `:1147`) | `examples/01_platformer/src/tests/{ball_pit,burning,main}.rs`, `docs/known-issues.md` |
| 6 | 2026-10-04 | Done | Encoder quiet from 04:44:03 through the sitting (04:56:50–05:00:41). `suite --repeat 5 --compare w14a-pre` → exit 0, valid, "Owned verdicts, all rows: 0 regressed, 0 improved, 41 unchanged, 13 noisy", "First-run digests match in 8 of 8 rows". The A/A's three: `gpu` `extract` p95 0.83 → 0.89 ms (noisy, [-0.07, +0.19]), `particles` `unattributed` p50 2.10 → 2.11 (noisy), `animate_sprites` p95 0.43 → 0.43 (noisy). Frame rows: `integrated` `total` p50 8.58 → 8.54, p95 9.13 → 9.14 (unchanged), p99 9.49 → 9.72 and jitter 0.91 → 1.17 (noisy); `gpu` `render_encode` p50 2.11 → 2.12 (unchanged); `churn` `flush` p95 2.67 → 2.72 (unchanged); `physics_step` p50 6.07 → 6.07. Against the A/A sitting, for context: 0 regressed, 0 improved, 43 unchanged, 11 noisy, digests 8 of 8. No re-capture was needed. `just visual` → exit 0 (2, 5 and 1 passed); `just physics-release` → exit 0 (4, 1 and 7 passed) | `perf-runs/20261004T075650Z-suite`, `perf-runs/20261004T080041Z-compare-suite` (against `w14a-pre`), `perf-runs/20261004T080046Z-compare-suite` (against the A/A sitting) |
| 7 | 2026-10-04 | Done, in a release session | The run session stopped before the release under §7 ("after any skipped step the run stops before the release and leaves it to a release session once the owner has seen why"): step 1 was skipped for its A/A check, steps 2–6 were done, nothing was cut. The owner then asked for the release (`release 0.43`). Workflow §5 closed (§8): evidence for steps 1–6 above, step 1 skipped with its reason; gates by change in steps 2, 3 and 6; invariants held in step 6 (digests 8 of 8, `just visual`, `just physics-release`) and step 3 (`api/tungsten-core.txt`, `api/tungsten-render.txt` unchanged); `D-110` and its index rows before the docs that cite it; no API break, so no ledger row; the `DESIGN.md` frame-loop paragraph and the `LLM_INDEX.md` row; known issues in step 5; the register's W14a row, W14a's status in `w14-tooling.md` and the README "Now" lines; one `CHANGELOG.md` line. `status: done` and this plan archived before `just release-cut 0.43.0`; the cut, the release checks and the preflight are quoted in the session's report, as this step's done-when says | `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/README.md`, `docs/plans/1.0/w14-tooling.md`, `CHANGELOG.md`, `DESIGN.md` |

## Follow-ups

1. **`pgrep` matches its own shell (A20).** Workflow §6 rule 2 and the tungsten-milestone skill write `pgrep -af scripts/bench.py`, which finds the agent's `bash -c` line; this plan uses `'[s]cripts/bench.py'`. Home: the next docs change to workflow §6 and the skill.
2. **Hand loops outside example 01 (A16).** Known issues gain them in step 5; home: W15a, which steps the harness, or the Phase 5 QA pass.
3. **Headless assets (Q2).** The harness loads no manifest; home: W15a's plugins or W12a.
4. **Step 3's index check (found in the run).** `rg -c 'D-110' docs/DECISION_INDEX.md` → 1 cannot hold while the tungsten-decision skill adds "Amended by `D-110`" to the `D-088` and `D-107` rows; a check of the row itself is `rg -c '^\| `D-110` '` → 1. Home: the skeleton's done-when guidance, when a plan next cites an amendment.
5. **A/A drift on this machine (found in the run).** In two sittings 4.5 minutes apart on one build, `gpu` `extract` p95, `particles` `unattributed` p50 and `particles` `animate_sprites` p95 read `regressed`, every run shifted. None is among the compare rules' A/A exceptions. Home: the profiling workflow's Compare §8 if a later A/A repeats it.
6. **Encoder blips (found in the run).** After the owner disconnected, `nxcodec.bin` came back for 3–31 s at 04:22, 04:25, 04:32 and 04:43, and three suite sittings went invalid. Waiting for a blip to end, or for 300 s of quiet, then starting the sitting, gave two clean ones. Home: workflow §6 rule 3.
