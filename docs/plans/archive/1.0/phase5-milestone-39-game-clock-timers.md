# Phase 5 milestone 39: game clock and timers (W3a) — draft

- **status:** done
- **goal:** A `Time` resource in `tungsten-core` with a real clock and a game clock, time scale, pause, elapsed time on both clocks and a frame index, and a plain `Timer`, as `D-129` fixes them. The app advances the clock once a frame; every engine, example and template reader of `DeltaTime` reads `Time` instead, state transitions run on the real clock, and `DeltaTime` is deprecated for W4b to remove. At scale 1 the physics hashes, smoke and every benchmark digest are unchanged.
- **non-goals:** W3b's fixed-step accumulator, `alpha`, the fixed input edge view, `PrevPosition`, interpolation, `FrameTimings::fixed_steps` and the `[time]` section (`D-129`): `fixed_update` still runs once a frame (`D-128`). A timer system or event queue. Example 01's hand-counted timers on `Timer` (Q5) and the template's pause on the clock (Q6). An audio clock (`D-132`'s audio pause row), UI clocks (W1 M3, M4), removing `DeltaTime` (W4b).
- **files to touch:** Step 1: `crates/tungsten-core/src/time.rs`, `crates/tungsten-core/src/tests/time.rs` (new), `crates/tungsten-core/src/lib.rs`. Step 2: in `crates/tungsten/src/`, `app.rs`, `state.rs`, `tweens.rs`, `particles.rs`, `game_feel.rs`, `camera.rs`, `testing.rs`, `tests/app.rs`, `tests/testing.rs`, `tests/state.rs`, `tests/tweens.rs`, `tests/game_feel.rs`, `tests/camera.rs`; `crates/tungsten/tests/camera.rs`, `crates/tungsten/tests/particles.rs`, `crates/tungsten/benches/particle_tick.rs`; in `crates/tungsten-core/`, `src/time.rs`, `src/lib.rs`, `src/physics/step/mod.rs`, `src/tests/physics/step.rs`, `src/tests/ecs/world.rs`, `tests/physics_determinism.rs`, `tests/physics_containment.rs`, `tests/physics_tunneling.rs`, `benches/physics_bench.rs`; in `examples/01_platformer/src/`, `systems.rs`, `gameplay.rs`, `fireball.rs`, `burning.rs`, `tests/main.rs`, `tests/spawning.rs`, `tests/burning.rs`; in `examples/02_bench/`, `src/physics.rs`, `src/particles.rs`, `src/gpu.rs`, `src/ecs/systems.rs`, `src/integrated/systems.rs`, `tests/fixtures/README.md`; `examples/03_scene_state/src/main.rs`; `examples/04_shader_playground/src/main.rs`; `templates/basic/src/game.rs`, `templates/basic/tests/game.rs`, `templates/basic/AGENTS.md`; `docs/getting-started.md`. Step 3: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w03-frame-loop.md`, `api/tungsten-core.txt` and any other `api/` file `just api` rewrites. Step 4: nothing tracked; evidence in `perf-runs/<YYYYMMDD>-m39-clock/`. Step 5: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, `docs/plans/1.0/implementation-plan.md` (the register row and a revisions line), `docs/plans/1.0/README.md`, `docs/plans/1.0/w03-frame-loop.md`, this plan (moved to `docs/plans/archive/1.0/`).
- **ordered steps:** (1) `Time` and `Timer` in core. (2) The clock in the frame and every reader on `Time`; `DeltaTime` deprecated; the template and the guide. (3) Decision, design, route, ledger and API snapshot. (4) The capture sitting and the GPU gates. (5) Release 0.53.
- **done-when:** every step's evidence row quoted; workflow §5 closed (§8); the release checks pass after the cut, with `just physics-release` and `just visual` from step 4; the preflight prints the command block and the post-merge block.

Candidate W3a ([implementation plan](../../1.0/implementation-plan.md) §3 card, level B), milestone M39, release 0.53. Written 2026-10-06 at `36b766d` (0.52, branch `0.53`). Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. Scoping source: [w03](../../1.0/w03-frame-loop.md) step 1 and its done-when.

## Context digest

- Today one `DeltaTime { dt }` resource carries the frame's dt: elapsed wall time capped at 0.1 s (`D-088`), 1/60 s in smoke runs, the harness's dt as given (`D-110`). `physics_step`, tweens, particles, game feel, the camera and the state dispatcher's transitions read it, and so do the examples, the bench rows, the template and seventeen test and bench files, most of them building worlds by hand. There is no pause, time scale, elapsed time, frame index or timer type.
- The frame-loop gate settled the shape (`D-129`), on the W3 spike that prototyped it over M38's stage map (`perf-runs/20261005-w3-stage-map/`, machine-local): a `Time` resource the app advances once a frame; a real clock and a game clock (real × scale, zero while paused); `Time::delta()` as the dt a system reads; transitions, the HUD and UI on real time; a plain `Timer` whose `tick(dt)` returns how many times it finished; `DeltaTime` deprecated at W3a and removed at W4b.
- W3a is the clock half of the spike's `Time`. W3b adds the accumulator: `fixed_update` zero or more times a frame, `Time::delta()` the step inside it, the fixed edge view, `PrevPosition` and interpolation. Until then `fixed_update` runs once a frame and `delta()` is the game dt everywhere.
- At scale 1 the game dt equals the real dt bit for bit (`x × 1.0` is exact), so every reader sees the value it read from `DeltaTime`: the physics hashes, the eight row digests and the pixel fixtures should not move, and the clock adds a few float operations a frame. The sitting (step 4) is there for code placement and to prove the digests.
- Later candidates expect: W3b a clock to extend; W13 `Timer` for lifetimes; W1 M3 and M4 the real clock; the acceptance game `Time::pause` for its pause and level-up screens. W15b, next in Track C, needs nothing from it.

## 1. Audit

Read at `36b766d`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `DeltaTime` is the whole time API: `pub dt: f32`, `new()` (zero) and `seconds()`, re-exported at the crate root. `dt`, `new` and `seconds` have no rustdoc: `-W missing-docs` flags `time.rs` lines 4, 9 and 14 | `crates/tungsten-core/src/time.rs:1-23`, `crates/tungsten-core/src/lib.rs:73` |
| A2 | `App::new` inserts `DeltaTime::new()`. The frame's first step, `stage_delta_time`, writes a pinned dt as given, or a wall frame's `frame_dt_secs`: elapsed time capped at `MAX_DT_SECS` (0.1 s), or `SMOKE_MODE_FIXED_DT_SECS` (1/60 s) in smoke runs | `crates/tungsten/src/app.rs:195`, `:801-819`, `:1151`, `:1322-1328`, `:50`, `:57` |
| A3 | The first frame's dt is zero only for a wall frame with no previous frame time, where `stage_delta_time` writes nothing. The window loop sets that time in `resumed`, after the startup hook, so its first drawn frame gets the time since then (1/60 s in smoke runs); a pinned frame gets its dt from the first frame on. w03 step 1's "the first frame advances both clocks by zero, as `DeltaTime` stays zero today" holds only for the first case (Q1) | `app.rs:809`, `:1727`; `:845-847` resets the time after `Startup`; `crates/tungsten/src/tests/app.rs:329-349` runs the first case |
| A4 | Engine readers, each `get_resource::<DeltaTime>().map_or(0.0, …)`. At dt 0: `physics_step` returns before any work; `tween_tick_system`, `particle_tick_system` and `squash_stretch_tick_system` return; `shake_tick_system` still adds the frame's trauma but decays nothing. `camera_update_system` draws the shake offset for the phase it entered with and then advances the phase by dt, so at dt 0 the offset holds from the frame after the last advance: the first paused frame still shows a new one. Its follow blend holds at dt 0, except that smoothing 0 and 1 hold and snap whatever the dt (`D-100`). `advance_transition` advances the active screen transition by the same dt | `crates/tungsten-core/src/physics/step/mod.rs:321-326`; `crates/tungsten/src/tweens.rs:16-21`, `particles.rs:222-227`, `game_feel.rs:111-117`, `:27-46`, `camera.rs:40-48`, `:65-67`, `:108-114`, `state.rs:236-255`; `D-100` |
| A5 | `particle_emit_system` has no zero-dt return: continuous emission is `rate × dt`, so nothing at 0, but a pending `Burst` fires on its first frame whatever the dt, and a `Pulse` whose timer holds more than one interval fires one pulse a frame until the backlog drains, dt 0 included ("at most one pulse per tick; overflow waits") | `particles.rs:79-82`, `:317-326`, `:339-351` |
| A6 | Engine tests and benches build worlds by hand with `DeltaTime`, inserted or set between direct system calls: the umbrella's `src/tests/{app,testing,state,tweens,game_feel,camera}.rs`, `tests/camera.rs`, `tests/particles.rs` and `benches/particle_tick.rs`; core's `src/tests/physics/step.rs` (9 lines), `tests/physics_{determinism,containment,tunneling}.rs` and `benches/physics_bench.rs` | `rg -c DeltaTime crates` |
| A7 | Example and template readers: example 01's `systems.rs` (eight systems, `animation_system` with `.unwrap()`), `gameplay.rs`, `fireball.rs`, `burning.rs`; each bench row through one helper; examples 03 and 04 with a 1/60 s fallback; the template's `player_movement` | `examples/01_platformer/src/systems.rs:207`; `examples/02_bench/src/physics.rs:801`, `ecs/systems.rs:32`, `integrated/systems.rs:48`, `particles.rs:456`, `gpu.rs:467`; `templates/basic/src/game.rs:74-77` |
| A8 | Example 01's tests call engine systems on hand-built worlds (`physics_step` among them), seed `DeltaTime { dt: 1/60 }`, and keep a `set_dt` helper that writes both the harness dt and `DeltaTime` for direct calls between frames | `examples/01_platformer/src/tests/main.rs:48-49`, `:104-112`; `tests/level.rs:315` |
| A9 | `tests/ecs/world.rs` declares its own `struct DeltaTime(f32)` for the resource tests: a name match for `rg`, not a reader | `crates/tungsten-core/src/tests/ecs/world.rs:97-108` |
| A10 | The harness defaults to 1/60 s; `set_dt` takes any finite, non-negative dt, uncapped (`D-110`); `step` runs `run_frame` with `FrameClock::Pinned`; `Harness::frame()` counts completed frames | `crates/tungsten/src/testing.rs:39-41`, `:86-92`, `:98-123`, `:221` |
| A11 | The API snapshot prints no attributes: M38's three `#[deprecated]` `InspectorState` functions are plain lines in `api/tungsten.txt`, so `just api` cannot show the `DeltaTime` deprecation w03's done-when expects from it; the source shows it. The snapshot lists items by module path (`tungsten_core::time::DeltaTime`), not by the root re-export | `crates/tungsten/src/inspector.rs:75`, `:93`, `:102`; `rg -i deprecated api/` finds nothing; `api/tungsten-core.txt:6227` |
| A12 | `fixed_update` runs once a frame, so a paused clock hands `physics_step` dt 0 and it returns before touching a body: the state a skipped stage leaves. W3b's accumulator skips the stage instead | `app.rs:850`; `step/mod.rs:324` |
| A13 | The template gates movement on the gameplay state; its pause state's hooks are empty, and no clock is involved. Its movement test assumes 1/60 s | `templates/basic/src/game.rs:56-58`, `states.rs:66-68`, `tests/game.rs:88-107` |
| A14 | Example 01 counts five timers by hand: the small-ball impact cooldown, the text refresh interval, the extinguish-sound cooldown, the black hole's lifetime and the burn. Example 03 sums two clocks by hand | `gameplay.rs:361`; `systems.rs:516`, `:886`, `:945`; `burning.rs:44`; `examples/03_scene_state/src/main.rs:136`, `:178` |
| A15 | Docs that name the old clock: `DESIGN.md`'s frame loop, resources, physics, transitions and tweens; the bench fixtures README. The guide and the template's `AGENTS.md` say nothing about clocks, and the ledger's `DeltaTime` row is "Planned" | `DESIGN.md:66`, `:77`, `:89`, `:251`, `:278`, `:284`; `examples/02_bench/tests/fixtures/README.md:31`; `docs/getting-started.md:76-91`; `templates/basic/AGENTS.md:15`; `docs/plans/1.0/w04-api-freeze.md:35` |
| A16 | `docs/LLM_INDEX.md` holds 10,247 of its 12,288 B and has no row for `time.rs` | `just ctx` |
| A17 | No name collides: no `Time`, `Timer` or `TimerMode` type, no glob import of `tungsten_core` and no test named `clock_…` in the workspace | `rg -n '\b(struct\|enum\|type\|trait) (Time\|Timer\|TimerMode)\b'`, `rg -n 'use (tungsten::core\|tungsten_core)::\*'`, `rg -n 'fn \w*clock_'`: nothing |

## 2. Design

### 2.1 API sketch

```rust
// tungsten_core::time; the crate root re-exports Time, Timer and TimerMode.

/// The frame's clocks: a World resource App::new inserts and advances once a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Time { /* private: real and game dt, both elapsed (f64), frame, scale, paused */ }
impl Time {
    pub fn new() -> Self;                          // zero clocks, scale 1, running, frame 0
    pub fn advance_frame(&mut self, real_dt: f32); // the app's clock stage; a world driven by hand
                                                   // calls it in its place; negative or non-finite = 0;
                                                   // the game dt saturates at f32::MAX
    pub fn delta(&self) -> f32;                    // what a system reads: the game dt
                                                   // (W3b: the step inside fixed_update)
    pub fn game_delta(&self) -> f32;               // real dt × scale, 0 while paused
    pub fn real_delta(&self) -> f32;               // the dt the loop gave: capped, pinned or the harness's
    pub fn elapsed(&self) -> f64;                  // game seconds over every frame so far
    pub fn real_elapsed(&self) -> f64;
    pub fn frame(&self) -> u64;                    // frames started, this one included; 0 before any
    pub fn scale(&self) -> f32;
    pub fn set_scale(&mut self, scale: f32);       // negative or non-finite = 0; from the next frame
    pub fn is_paused(&self) -> bool;
    pub fn pause(&mut self);                       // from the next frame; real time keeps running
    pub fn resume(&mut self);
    pub fn set_paused(&mut self, paused: bool);
}
impl Default for Time;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode { Once, Repeating }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timer { /* private: duration, elapsed, mode, finished, finished this tick */ }
impl Timer {
    pub fn new(duration: f32, mode: TimerMode) -> Self; // panics: negative or non-finite, or 0 and Repeating
    pub fn once(duration: f32) -> Self;
    pub fn repeating(duration: f32) -> Self;
    pub fn tick(&mut self, dt: f32) -> u32;        // times it finished, saturating at u32::MAX; a negative
                                                   // or NaN dt counts as 0; elapsed stays finite
    pub fn finished(&self) -> bool;                // Once: at any point; Repeating: during the last tick
    pub fn just_finished(&self) -> bool;
    pub fn times_finished_this_tick(&self) -> u32;
    pub fn elapsed(&self) -> f32;
    pub fn remaining(&self) -> f32;
    pub fn fraction(&self) -> f32;                 // 1.0 for a zero duration
    pub fn duration(&self) -> f32;
    pub fn mode(&self) -> TimerMode;
    pub fn reset(&mut self);
    pub fn set_duration(&mut self, duration: f32); // keeps elapsed, clamped to the new duration;
                                                   // panics as new does
}

#[deprecated(since = "0.53.0", note = "read `Time::delta()`, or `Time::real_delta()` for real time; \
    App still writes the game dt here each frame. Removed at W4b")]
pub struct DeltaTime { pub dt: f32 }               // unchanged otherwise
```

Taken from the spike's `time.rs` (`perf-runs/20261005-w3-stage-map/w3-stage-map.patch`, the `Time` and `Timer` hunks), less the accumulator: `fixed_step`, `max_steps_per_frame`, `fixed_steps_this_frame`, `fixed_step_count`, `fixed_elapsed`, `dropped_this_frame`, `alpha`, `in_fixed_update`, `interpolate`, `enter_fixed_step`, `leave_fixed_steps` and the two default constants are W3b's. Five changes from the spike. `advance_frame` counts a negative or non-finite dt as zero where the spike had a `debug_assert!` (the harness refuses one already, so only a direct caller can pass it), and saturates the game dt at `f32::MAX`, since a finite real dt times a finite scale can overflow (`f32::MAX × 2`) and the spike's `f64` sums would then stay infinite. `Timer::tick` saturates its count at `u32::MAX` and keeps `elapsed()` finite within `[0, duration]` for every accepted duration and dt: the spike cast `floor(elapsed / duration)` to `u32` and subtracted `wraps × duration`, which for a tiny duration overflows the quotient to infinity and leaves the remainder at −∞ (patch lines 1704–1712). `set_duration` clamps `elapsed()` to the new duration, where the spike kept it whole (patch line 1792), so a finished `Once` timer shortened below its elapsed time stays within its duration, and an unfinished timer that the new duration has overtaken finishes on its next tick. `delta()` returns the game dt, there being no fixed-step state yet. The fields are private, so W3b adds without a break.

### 2.2 The clock stage

`stage_delta_time` becomes `stage_time` and stays the first step of `run_frame` (A2):

```text
redraw:  apply pending display settings
         → advance Time: real dt = the pinned dt as given, or a wall frame's frame_dt_secs
           (0 with no previous frame time, A3); game dt = real dt × scale, 0 while paused;
           elapsed on both clocks; frame + 1
         → DeltaTime.dt = the game dt (deprecated; the real dt if a game removed Time)
         → startup (first frame) → pre_update → fixed_update (once) → update → exit check → post_update → …
```

The cap and its constant, the smoke pin and the harness's as-given dt stay where they are (`D-088`, `D-110`): `Time` takes the dt the loop gives it. The first-frame sequence is today's (A3, Q1), so a smoke run's frames see the dts they saw before.

### 2.3 Which clock each reader takes

| Reader | Reads | So |
| --- | --- | --- |
| `physics_step`, `tween_tick`, `particle_emit`, `particle_tick`, `shake_tick`, `squash_stretch_tick`, `camera_update` | `Time::delta()` | A pause freezes them and a scale slows or speeds them (A4, A12); W3b makes `delta()` the step inside `fixed_update` without touching them |
| `advance_transition`, in the state dispatcher | `Time::real_delta()` | A fade finishes over a paused game (`D-093` as `D-129` amends it) |
| Gameplay in the examples and the template | `Time::delta()` | — |
| The HUD, overlays, the inspector | `FrameTimings`, as today | They read no dt |
| `DeltaTime.dt` | the app's write | The game dt, for games until W4b |

Two readers do not freeze outright (A4): the camera's shake changes once on the first paused frame, which draws the phase the last running frame advanced to, and a camera at smoothing 1 still snaps to a target that moves under a pause (`D-100`). A burst emitter spawned under a pause fires at once, and a pulse emitter with a backlog fires one pulse a frame until it drains (A5, Q8). No engine system falls back to `DeltaTime` (Q2). Above scale 2 at 60 Hz the game dt exceeds `max_step_dt` (1/30 s), so `physics_step` advances 1/30 s a call and physics runs slow against the game clock: `D-094`'s behaviour, which W3b's bound of two steps keeps.

### 2.4 Worlds driven by hand

`world.insert_resource(DeltaTime { dt })` becomes `let mut time = Time::new(); time.advance_frame(dt); world.insert_resource(time);`, and a helper that set `DeltaTime.dt` between direct system calls calls `advance_frame(dt)` on the resource instead. It stands in for the app's clock stage; no existing test reads the frame index it advances. At scale 1 each such test sees the dt it set, so the core physics tests keep their hashes.

### 2.5 Template and guide

- `templates/basic/src/game.rs`: `player_movement` reads `Time::delta()`. `tests/game.rs`: a `clock_` test sets the scale to 0.5 through `harness.world_mut()` and sees the player move half as far in 30 frames as at scale 1.
- `templates/basic/AGENTS.md`: one rule: read time from `Time`, `delta()` (the game clock: scaled, zero while paused) for movement and animation and `real_delta()` for anything that runs over a pause; count cooldowns with `Timer`; `DeltaTime` is deprecated.
- `docs/getting-started.md`: a "Time and timers" section after "Systems and stages": the two clocks, `pause`, `resume` and `set_scale` taking effect from the next frame, a pause freezing motion and not input, `Timer` in a short example, and `fixed_update` still running once a frame.

## 3. Steps

Checks pass or fail on their exit codes, except where the expected result is the output itself. Output that a check reads or the evidence log quotes comes from the raw command: `rtk proxy <command>` for `cargo` and `git`, whose output the rtk hook compresses.

### Step 1: `Time` and `Timer` in core

- **Files:** `crates/tungsten-core/src/time.rs`, `crates/tungsten-core/src/tests/time.rs` (new), `crates/tungsten-core/src/lib.rs`
- **Change:** `Time`, `TimerMode` and `Timer` as §2.1 sketches them, with module rustdoc and rustdoc on every public item, `DeltaTime`'s field and methods included (A1). `time.rs` includes `tests/time.rs` as its `#[cfg(test)]` module, the crate's convention; the root re-exports `Time`, `Timer` and `TimerMode`. Nothing reads the clock yet. Tests, all in `time::tests`:
  - 600 frames of `advance_frame(1.0 / 60.0)`: `frame()` 600; `real_delta()`, `game_delta()` and `delta()` equal to `1.0 / 60.0` bit for bit; `elapsed()` equal to `real_elapsed()` bit for bit and within 1 µs of 10 s.
  - At scale 0.5 and 2 the game dt and game elapsed time are exactly half and double the real ones (a power-of-two scale is exact), and the real ones equal scale 1's.
  - Paused frames hold `game_delta()` at 0 and `elapsed()` still while `real_elapsed()` and `frame()` count; `resume()` restores the game clock.
  - `set_scale` turns −1, NaN and infinity into 0; `advance_frame` counts −1 and NaN as zero real time and still advances `frame()`; a new `Time` reads zero everywhere, scale 1, not paused.
  - `set_scale(2.0)` then `advance_frame(f32::MAX)`: `game_delta()` is `f32::MAX`, and `elapsed()` and `real_elapsed()` are finite.
  - `Timer::repeating(0.1).tick(0.35)` returns 3, with `elapsed()` within 1e-6 of 0.05 and `just_finished()` true.
  - `Timer::once(0.2)` returns 0 on `tick(0.1)`, 1 on `tick(0.15)`, then 0 on `tick(1.0)` with `finished()` still true and `elapsed()` at the duration; `Timer::once(0.0)` finishes on its first tick.
  - `tick(-1.0)` and `tick(f32::NAN)` return 0 and leave `elapsed()` unchanged; `reset()` clears `finished()` and `elapsed()`.
  - `Timer::repeating(1e-10).tick(1.0)` returns `u32::MAX` with `elapsed()` in `[0, 1e-10)`; `Timer::repeating(1.0).tick(f32::MAX)` returns `u32::MAX` with `elapsed()` in `[0, 1)`; `Timer::repeating(f32::MAX)` after `tick(f32::MAX / 2.0)` and `tick(f32::MAX)` has a finite `elapsed()` within its duration.
  - `set_duration(0.5)` on a finished `Timer::once(1.0)` leaves `elapsed()` 0.5 and `finished()` true; on an unfinished one at 0.8 s it leaves `elapsed()` 0.5, and `tick(0.0)` then returns 1; on a `Timer::repeating(1.0)` at 0.8 s, `tick(0.0)` then returns 1 and leaves `elapsed()` 0.
  - `Timer::repeating(0.0)` and `Timer::once(-1.0)` panic (`#[should_panic]`).
- **Done-when:** `cargo test -p tungsten-core --lib time::tests` → every test passes, and the evidence row names the test that covers each case above; `rtk proxy cargo rustc -p tungsten-core --lib -- -W missing-docs 2>&1 | rg -A1 'missing documentation' | rg -c 'src/time\.rs'` → prints nothing (at `36b766d` it prints 3); `just check` → passes.
- **Moves:** nothing.

### Step 2: The clock in the frame and every reader on `Time`

- **Files:** step 2's list in the header
- **Change**, in order:
  1. `app.rs`: `App::new` inserts `Time::new()` beside `DeltaTime`; `stage_time` as §2.2; `FrameClock`'s rustdoc names the real clock. `testing.rs`: the module's and `set_dt`'s rustdoc say the dt is the real clock's, which systems read through `Time` (times the scale, zero while paused).
  2. The engine readers as §2.3, `physics_step` in core among them; `state.rs`'s rustdoc says transitions advance on the real clock.
  3. Every hand-built world in the tests and benches as §2.4 (A6, A8), the platformer's `seed` and `set_dt` included; the core resource test's local struct renamed (A9).
  4. The examples' and the template's readers as §2.3 (A7); `animation_system` keeps its `.unwrap()`, now on `Time`. The bench fixtures README says the smoke pin sets the real clock.
  5. `#[deprecated(since = "0.53.0", …)]` on `DeltaTime` as §2.1, with `#[allow(deprecated)]` at the narrowest scope on its two `impl` blocks, the root re-export, the app's import, insert and write, and the app test that pins the write: on the pinned 1.98.1 the lint fires on a `pub use`, on each `impl` header, on the field inside them and at every use, and not on derives (a scratch probe at planning time).
  6. New tests, each named `clock_…`: in `tests/app.rs`, a wall frame whose previous frame time the test sets 1 s back reads `real_delta()` 0.1 (`D-088`, no sleep), a pinned 0.25 s frame reads 0.25 on both clocks and in `DeltaTime` (`D-110`), and a paused pinned frame reads real 0.25, game 0 and `DeltaTime` 0. In `tests/testing.rs`, on the harness, after running frames and then with the clock paused while a short fade (`Transition::new`) pushes a second state: a tween's target, a continuous emitter's live count and its particles' positions, and a dynamic body's `Position` under gravity keep the last running frame's values for 30 paused frames, and the camera, following that body at smoothing 1 under shake trauma, keeps its first paused frame's position from then on (A4), while the transition finishes (the second state on top, none active); and at `set_dt(1.0 / 64.0)` a 0.25 s tween sends `TweenComplete` on frame 16 at scale 1 and on frame 32 at scale 0.5, the dts being exact in binary. In `tests/state.rs`, the dispatcher finishes a transition over a paused clock. The template's test (§2.5).
  7. The template's `AGENTS.md` and the guide as §2.5: the step that changes how a game reads time changes the template and the guide with it (implementation plan amendment 16, workflow §5 item 6).
- **Done-when:** `rg -l --type rust 'DeltaTime' crates examples templates | sort` → exactly `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/time.rs`, `crates/tungsten/src/app.rs` and `crates/tungsten/src/tests/app.rs` (Markdown such as the template's `AGENTS.md` may name the deprecated type); `rg -c '#\[deprecated' crates/tungsten-core/src/time.rs` → 1, and `rg -n -B8 'pub struct DeltaTime' crates/tungsten-core/src/time.rs` shows it there with `since = "0.53.0"`; `rg -l 'allow\(deprecated\)' crates examples templates | sort` → those four files and `crates/tungsten/src/tests/inspector.rs` (M38's shims, there at `36b766d`), nothing else; `cargo test --workspace clock_` → exit 0, its per-binary results summing to 7 tests run and passed (three app tests, two harness tests, the dispatcher's and the template's); `just physics-release` → passes, its determinism and pinned-step tests asserting `0x088ec07a73c1b168` and `0xaee272e01ffc4e4c`; `just bench-build` → compiles; `just check` → passes.
- **Moves:** nothing; a hash, a schedule snapshot or a pixel test that moves is a stop (§7).

### Step 3: Decision, design, route, ledger and API snapshot

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w03-frame-loop.md`, `api/tungsten-core.txt` and any other `api/` file `just api` rewrites
- **Change**, in order:
  1. `D-134` with the tungsten-decision skill, its index row in the same edit (§5), before any doc cites it.
  2. `DESIGN.md`: the frame-loop block and paragraph as §2.2 ("advance Time" for "update DeltaTime"; the smoke run pins the real clock); `Time` for `DeltaTime` in the resources line; the physics, transition and tween paragraphs name their clocks (A15); a "Game Clock and Timers — M39" section after M38's: the two clocks, scale and pause from the next frame, the frame index, `Timer`, the clock table of §2.3, worlds driven by hand, the deprecation and what W3b adds.
  3. `docs/LLM_INDEX.md`: a Runtime and ECS row, "Game clock, `Time`, `Timer`, `DeltaTime` (`D-088`, `D-129`, `D-134`)" → `core/time.rs`, `core/tests/time.rs`, `tungsten/app.rs`, `tungsten/tests/testing.rs`.
  4. The ledger's `DeltaTime` row ([w04](../../1.0/w04-api-freeze.md#break-ledger)): status "Deprecated at 0.53 (`D-134`): `App` still writes the game dt into it each frame; engine systems read `Time`, so a world built by hand inserts and advances `Time` for them to move; W4b removes it", landed in "0.53 (M39) deprecated; removal W4b".
  5. w03 step 1: an italic note after its paragraph on A3's first-frame sequence (Q1) and A11's deprecation check.
  6. `just api`, then read its diff.
- **Done-when:** `rg -n '^## D-134' DECISIONS.md` → one heading; ``rg -n '^\| `D-134`' docs/DECISION_INDEX.md`` → one row; `just repo-check` → passes; after `just api`, `for f in api/*.txt; do comm -23 <(git cat-file -p HEAD:$f | sort) <(sort $f) | wc -l; done` → `0` three times (no line removed; not a piped `git diff`, which the rtk hook rewrites) and `rg -n '^pub (struct|enum) tungsten_core::time::' api/tungsten-core.txt` → `DeltaTime`, `Time`, `Timer`, `TimerMode`; `rg -n 'Game Clock and Timers — M39' DESIGN.md` → one heading; `rg -n 'core/time\.rs' docs/LLM_INDEX.md` → the new row; `just ctx` → passes with the index under 12,288 B; `rg -n 'Deprecated at 0.53' docs/plans/1.0/w04-api-freeze.md` → the `DeltaTime` row.
- **Moves:** nothing; `api/tungsten-core.txt` gains lines and loses none.

### Step 4: The capture sitting and the GPU gates

- **Files:** nothing tracked. Evidence in `perf-runs/<YYYYMMDD>-m39-clock/`: a README with the trees and provenance, `scripts/` (M38's `sitting.sh`, `compares.sh`, `placement.sh`, `summarize.py` and `gates.sh` from `perf-runs/20261006-m38-schedule/`, renamed), `captures/`, `compares.log`, `summary.md`, `gates.log`.
- **Change:** M38's sitting, whose README is the recipe:
  1. The baseline tree: `git archive 36b766d | tar -x -C target/m39-base` (the 0.52 release). A copy without `.git` records the enclosing repository's provenance, so the README names each tree by hand.
  2. Four binaries prebuilt with the runner's command: the baseline and the tree, plain, and both again with `TUNGSTEN_PERF_RUSTFLAGS="-C force-frame-pointers=yes -C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=6"`, each in its own `CARGO_TARGET_DIR`. Each is rebuilt once to show it fresh; `cmp` shows the plain pair differs; `placement.sh` lists the hot functions' offsets modulo 64 in the plain pair.
  3. From the first build to the end of the gates nothing writes to the tree (`D-095`, workflow §6).
  4. One background job once the remote-desktop encoder has been gone 60 s: six suites of every tracked row at `--repeat 5` (baseline, tree, baseline again, tree again, baseline aligned, tree aligned), then the compares (`compare-direct`, `compare-direct-again`, both A/A pairs, both cross pairs, `compare-aligned`) and `summarize.py`.
  5. The gates as a second job, with an encoder check before and after each: `just smoke`, `just visual` (judged on its exit code; its `load_shaders 'fade'` ERROR line comes from a deliberate test) and `just physics-release`.
  6. The scratch copies and target directories deleted.
- **Done-when:** all 48 captures valid, and the tree's dirty-file count the same before the first capture and after the last compare; every compare matches digests 8 of 8 with `only_baseline` and `only_candidate` empty; both direct pairs read 0 `regressed` on owned metrics, apart from Q7's documented exception; both A/A pairs read 0 `regressed` and 0 `improved` on owned metrics outside Q7's list; `just smoke` → exit 0; `just visual` → exit 0; `just physics-release` → passes with both hashes; `fd -I --max-depth 1 '^m39-' target` → prints nothing.
- **Moves:** nothing.

### Step 5: Release 0.53

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row and a revisions line in `docs/plans/1.0/implementation-plan.md`, the README "Now" lines, `docs/plans/1.0/w03-frame-loop.md`, this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 (§8) and this step's evidence row with `status: done`: the `[Unreleased]` entry (Added: `Time`, `Timer`, `TimerMode`; Changed: the engine readers, transitions on the real clock, `DeltaTime` deprecated, the template and docs; the hashes, digests and pixel fixtures unchanged), `DESIGN.md`'s status line (its head, the old head prepended to the chain), the register row's release and status, the README's Now lines, a note under w03 step 1 that W3a landed in 0.53. Then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.53.0` once, the release checks once after it, then the preflight with `--message`. No gate beyond the chain: step 4 ran `just physics-release` and `just visual` on the same code, and the cut changes versions and docs only.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.53.0 --repo JosephGibson/Tungsten --message 'Update 0.53: game clock and timers (W3a, D-134)'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: [tungsten-next's](../../../../.claude/skills/tungsten-next/prompts.md) `plan` prompt for W15b, `docs/plans/1.0/phase5-milestone-40-tuple-queries-bundles.md`, with `docs/plans/1.0/w15-authoring-api.md` as its source (`/tungsten-next` adds the records line once 0.53 merges).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | w03 step 1 says both clocks advance by zero on the first frame, "as `DeltaTime` stays zero today"; the tree gives the window loop's first frame the time since `resumed` (1/60 s in smoke runs) and a harness frame its dt (A3). Which sequence? | Today's, which the digests and the harness tests depend on: zero only for a wall frame with no previous frame time. Step 3 notes it under w03 step 1 |
| Q2 | Should engine systems fall back to `DeltaTime` when a world has no `Time`? | No: they read `Time` alone, and a world built by hand inserts it (§2.4). The ledger row says so, since a game's hand-built test world that sets only `DeltaTime` sees engine systems stand still |
| Q3 | `Time::advance_frame` public, or `#[doc(hidden)]`? | Public, its rustdoc naming its callers (the app once a frame; a world driven by hand in its place), since the examples' tests call it and examples teach |
| Q4 | `TimerMode` `#[non_exhaustive]`? | No: one-shot and repeating are the whole set (Bevy's and Godot's timers have the same two), and games match on it. `Time` and `Timer` have private fields, so they can grow |
| Q5 | Example 01's five hand-counted timers (A14) on `Timer`? | No: W3a moves readers only, and each change would alter the example's float sequence for no engine gain. A follow-up for the Phase 5 QA pass; W13's lifetimes are `Timer`'s first engine users |
| Q6 | Should the template's pause state pause the game clock? | No: its pause gates movement on the state (A13) and has nothing else to freeze. The guide shows `Time::pause()` for a pause over a frozen world, and the acceptance game's pause and level-up screens (W1 M3, M4) use it |
| Q7 | What may step 4 pass without the owner? | Only what its done-when lists. A direct-pair `regressed` passes only on one of the two readings the profiling workflow's A/A check documents as moving with nothing changed (`gpu-throughput` `render_encode`, the `ecs` per-run-mode rows), and only when the same sitting's A/A pairs move that metric at least as far; the evidence row quotes both. The A/A pairs may read those two and the drifts past A/A pairs showed on this machine (`particles` `animate_sprites` p95, `gpu` `extract` p95). Any other owned `regressed`, on either direct pair, stops before the release (§7) with its evidence: the cross pairs', the aligned pair's and the A/A pairs' readings of it and its function's offset modulo 64 in both plain binaries. The owner then fixes it, accepts it or asks for the regression policy's placebo build; the run classifies nothing as noise or placement itself |
| Q8 | Should pending burst and pulse emissions wait for the clock to resume (A5)? | Not in W3a: emission semantics are W8a's (burst and pulse semantics, the burst latch), and W3a moves readers only. Its pause test uses a continuous emitter; follow-up 2 hands both cases to W8a |

Approved 2026-10-06 with the stated defaults.

Q7, 2026-10-06: the owner accepts `compare-direct`'s `ecs` `system.stats_decay` p50 `regressed` reading (step 4: 0.180 → 0.310 ms, +0.130 [+0.047, +0.213], τ 0.020) as the per-run mode, and this is the regression policy's justification for it. The first tree suite held five mode runs of five (`stats_decay` 0.31 ms in each), where every other suite held one or two, both builds included; `compare-direct-again` (−0.004 ms), the cross pair without that suite (`compare-cross-baseline-vs-tree-again`, +0.002 ms) and `compare-aligned` (+0.034 [−0.089, +0.157] ms) read it `noisy`. Step 4's stop is resolved, and step 5 runs as written.

## 5. Decisions expected

- `D-134`: W3a landed (M39). `Time`'s API (§2.1): `advance_frame` public, a negative or non-finite dt counted as zero, the game dt saturating at `f32::MAX`, `frame()` counting frames started, the scale clamped at zero, scale and pause from the next frame, today's first-frame sequence (Q1). `Timer`'s edge rules: a negative, non-finite, or zero repeating duration panics; `tick` returns the count, saturating at `u32::MAX`, and keeps `elapsed()` finite within the duration; `set_duration` clamps `elapsed()` to the new duration; a negative or NaN dt counts as zero. Engine systems read `Time` alone (Q2). `DeltaTime` deprecated since 0.53.0 and written by the app as the game dt until W4b. Amends nothing: `D-129` left these to W3a. The next free ID at `36b766d`; checked again when step 3 runs.

## 6. Invariants

- **May move:** nothing. `api/tungsten-core.txt` gains `Time`, `Timer` and `TimerMode` and loses nothing.
- **Must not move:** the determinism hash `0x088ec07a73c1b168`, the pinned containment hash `0xaee272e01ffc4e4c`, the eight row digests, `gpu-visual.png`, the post and transition regressions, every schedule snapshot (`DefaultPlugins` in `tests/plugins.rs`, example 01's `runtime_schedule_matches_expected_pipeline`, the template's and examples 03 and 04's), the `frame:` and `systems:` perf lines and every benchmark's `workload_version`.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Step 2 depends on step 1, steps 3 and 4 on step 2, and the release on every step.
- A physics hash, a row digest, a schedule snapshot or a pixel test moves: the change was meant to be invisible at scale 1, so restore step 2's files and stop before the release with the diff for the owner.
- Step 4: an owned metric `regressed` on either direct pair beyond Q7's exception, or an A/A reading outside Q7's list: no recapture; record Q7's evidence, skip the release and give the owner the compares.
- An invalid capture or a background-load flag: rerun that suite once in the same sitting; a second one skips step 4.
- A file outside the files to touch needs a change: one that `rg -l --type rust DeltaTime crates examples templates` lists is in scope, named in the evidence row; any other stops the step for the owner.
- `just api` removes a line: stop.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–4, release | `just check` in steps 1–3; `just physics-release` and `just bench-build` in 2; every CPU row, `just smoke`, `just visual` and `just physics-release` in 4; the release chain, `just template-check` among it, after the cut |
| 3 Invariants | 2, 4 | The hashes in 2 and 4; the digests and pixel tests in 4 |
| 4 Decisions | 3 | `D-134` and its row before `DESIGN.md` and the ledger cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 3 | Rustdoc in 1, checked by the missing-docs count; the ledger row, `just api` and its diff in 3. New surface for the freeze: `Time`, `Timer`, `TimerMode` |
| 6 Template and guide | 2 | `game.rs`, its test, `AGENTS.md`; the guide's "Time and timers" |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 3 | The `time.rs` row |
| 8 Design | 3 | The frame loop and the M39 section |
| 9 Records (known issues, backlog, gap log, register, workstream file, README "Now") | 3, release | Ledger and w03 in 3; register, README and w03's landing note in the release. Known issues: nothing fixed or found (A5 is a follow-up for W8a); backlog: nothing cut; gap log: empty |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Reviewer: the `critique` skill with `--repo`, gpt-6.1-sol through the Codex CLI (OpenAI, another model family), 2026-10-06, reading the tree at `36b766d` with this plan and the session's record edits uncommitted. Round 1: model gpt-6.1-sol, effort xhigh, 282.9 s, tokens in 375,082 (cached 292,480), out 8,031 (reasoning 5,141). It changed done-when checks in steps 1, 2 and 4, so a second round ran on the revised plan at the same effort.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Changing the camera's dt reader does not freeze its output on pause | `crates/tungsten/src/camera.rs:108-114`, `:40-48`; `D-100` (`DECISIONS.md:1183-1190`) | accepted | A4 and §2.3 state both; step 2's harness test holds the camera from its first paused frame, following a still body at smoothing 1; follow-up 4 |
| 2 | [MEDIUM] Accepted finite inputs can permanently overflow the game clock | The spike patch, lines 1424–1432 (`real_dt * self.scale`, `elapsed += f64::from(self.game_delta)`) | accepted | §2.1: the game dt saturates at `f32::MAX`; a step 1 test; `D-134`'s expected text |
| 3 | [MEDIUM] The timer cannot return an exact finish count across its accepted input range | The spike patch, lines 1701–1713 (`floor`, `wraps * duration`, `wraps as u32`) | accepted | §2.1: the count saturates at `u32::MAX` and `elapsed()` stays finite within the duration; three step 1 tests; `D-134`'s expected text |
| 4 | [MEDIUM] Q7 automatically dismisses regressions without establishing their cause | `docs/perf/profiling-workflow.md:209-210` (a `regressed` needs a fix or a recorded justification; placement needs the padded baseline); M38's direct pairs read 0 `regressed` (`perf-runs/20261006-m38-schedule/README.md:30-31`) | accepted | Q7, step 4's done-when and §7: a direct-pair `regressed` passes only on the two documented two-valued readings, when the sitting's A/A pairs move the metric as far; any other stops before the release with its evidence for the owner |
| 5 | [MEDIUM] Step 2's reader-scan check rejects documentation the plan requires | §2.5 and step 2's done-when | accepted | Step 2's scan and §7 read Rust sources only (`--type rust`). Checking it also showed `crates/tungsten/src/tests/inspector.rs` already holds `allow(deprecated)` (M38's shims), so the `allow` check names that file |

Round 2, on the revised plan: model gpt-6.1-sol, effort xhigh, 233.9 s, tokens in 623,594 (cached 533,376), out 6,597 (reasoning 4,088). There is no third round (skill step 7).

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Pulse emitters can continue emitting while the game is paused | `crates/tungsten/src/particles.rs:339-351` (`pulse_timer += dt`; at most one pulse per tick, overflow waits) | accepted, as Q8 | A5 and §2.3 name it, and Q8's default leaves emission semantics to W8a (follow-up 2). The pause test keeps a continuous emitter: a test of the backlog would pin what W8a may change |
| 2.2 | [MEDIUM] Duration changes can defeat the timer's bounded-elapsed guarantee | The spike patch, lines 1686–1699 (a finished `Once` timer returns at once) and 1779–1793 (`set_duration` keeps `elapsed`) | accepted | §2.1: `set_duration` clamps `elapsed()` to the new duration; three step 1 cases; `D-134`'s expected text |

Outside the critique, after round 2: a planning-time test showed the rtk hook rewriting a piped `git diff` (four removed-side lines counted where the raw diff had three), so step 3's removed-line check reads `git cat-file` through `comm`, and §3 asks for raw output wherever a check is quoted.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-06 | pass | `cargo test -p tungsten-core --lib time::tests`: "test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 587 filtered out". Cases to tests: 600 frames `six_hundred_frames_at_scale_one_keep_real_and_game_time_equal`; scales 0.5 and 2 `power_of_two_scales_halve_and_double_game_time_exactly`; pause `a_pause_holds_game_time_while_real_time_and_frames_count`; bad scales `set_scale_turns_negative_and_non_finite_scales_into_zero`, bad dts `advance_frame_counts_negative_and_nan_dts_as_zero`, a new `Time` `a_new_time_reads_zero_at_scale_one_running`; `f32::MAX` `the_game_dt_saturates_at_f32_max`; `repeating(0.1).tick(0.35)` `a_repeating_timer_counts_every_period_of_a_long_tick`; `once(0.2)` `a_once_timer_finishes_once_and_stays_finished`, `once(0.0)` `a_zero_duration_once_timer_finishes_on_its_first_tick`; negative and NaN ticks `negative_and_nan_ticks_count_as_zero`, `reset` `reset_clears_finished_and_elapsed`; saturation `a_repeating_count_saturates_with_elapsed_within_the_duration`; `set_duration` `set_duration_clamps_elapsed_to_the_new_duration`; panics `a_zero_duration_repeating_timer_panics`, `a_negative_duration_panics`. A repeating `tick` splits periods in `f64` with an exact `%`; `tick` counts +∞ as zero too (the spike's `is_finite`), which its rustdoc and the NaN test state. Missing-docs count for `src/time.rs`: prints nothing (`rg` exit 1; 897 elsewhere in the crate). `just check`: exit 0, 33 `test result: ok` lines, 1,181 passed, 0 failed, 5 ignored | `crates/tungsten-core/src/time.rs`, `src/tests/time.rs` (new), `src/lib.rs` |
| 2 | 2026-10-06 | pass | `rg -l --type rust 'DeltaTime' crates examples templates \| sort`: `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/time.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs`. `rg -c '#\[deprecated' …/time.rs`: 1; `-B8` shows `since = "0.53.0",` at line 24, `pub struct DeltaTime {` at 29. `allow(deprecated)` files: those four and `crates/tungsten/src/tests/inspector.rs`. `cargo test --workspace clock_`: exit 0, 7 passed, 0 failed over 29 binaries (6 in the umbrella's lib: three `app::tests::clock_…`, two `testing::tests::clock_…`, `state::tests::clock_a_transition_finishes_over_a_paused_clock`; 1 in the template's `tests/game.rs`, `clock_half_scale_moves_the_player_half_as_far`). `just physics-release`: exit 0, 4 + 1 + 7 passed; `physics_step_is_bit_identical_across_runs` asserts `0x088e_c07a_73c1_b168` and `pinned_step_state_is_unchanged` `0xaee2_72e0_1ffc_4e4c`, neither line touched. `just bench-build`: exit 0. `just check`: the first run exited 101 on clippy's `unchecked_time_subtraction` in the new wall-frame test (`Instant::now() - Duration`); with `checked_sub` the rerun exited 0, 1,188 passed, 0 failed, 5 ignored, 33 result lines, every schedule snapshot among them. No file outside the list changed | the 44 step 2 files in the header; `just ctx` passes with the template's new rule |
| 3 | 2026-10-06 | pass | `D-134` the next free ID (the skill's next-ID command printed `D-134`). `rg -n '^## D-134' DECISIONS.md`: one heading, line 1631; ``rg -n '^\| `D-134`' docs/DECISION_INDEX.md``: one row, line 157. `just repo-check`: exit 0 (its `manifests` and `decision_index` tests 2 + 3 passed). `just api`: exit 0, wrote the three files; the `comm -23` loop printed `0` for `api/tungsten-core.txt`, `api/tungsten-render.txt` and `api/tungsten.txt`; `comm -13` counts 166 added lines, all in `tungsten-core.txt` and all `Time`, `Timer` or `TimerMode` items (14 `Time` methods, 14 `Timer` methods, the two variants, their derives and auto traits), as §2.1 sketches them. `rg -n '^pub (struct|enum) tungsten_core::time::' api/tungsten-core.txt`: `TimerMode` (6227), `DeltaTime` (6246), `Time` (6265), `Timer` (6298). `rg -n 'Game Clock and Timers — M39' DESIGN.md`: line 392. `rg -n 'core/time\.rs' docs/LLM_INDEX.md`: the new row, line 14. `just ctx`: exit 0, `LLM_INDEX.md` 10,407 B. `rg -n 'Deprecated at 0.53' …/w04-api-freeze.md`: the `DeltaTime` row, line 35; the scope's freeze list gains the game clock. `just check` (§8): exit 0, 1,188 passed, 0 failed, 5 ignored. `just api` printed two pre-existing rustdoc warnings in `sprite_extract.rs` (follow-up 5) | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w03-frame-loop.md`, `api/tungsten-core.txt` |
| 4 | 2026-10-06 | stop (§7) | Sitting 14:27:53–14:51:03 UTC: six suites of every tracked row at `--repeat 5` on prebuilt binaries (the runner's build checks 0.11–0.17 s), no `nxcodec.bin`, no other session in the tree. All 48 captures valid on the first attempt; dirty count 55 before the first capture, after the suites and after the last compare. Digests 8 of 8 with `digests_differ`, `only_baseline` and `only_candidate` empty in all seven compares. `compare-direct-again`: 0 regressed, 0 improved, 47 unchanged, 11 noisy. `compare-direct`: 2 regressed, 2 improved, 41 unchanged, 13 noisy: `ecs` `buffs` p50 0.638 → 0.660 (+0.022 [+0.005, +0.039], τ 0.020) and `stats_decay` p50 0.180 → 0.310 (+0.130 [+0.047, +0.213], τ 0.020) `regressed`, `follow` and `team_bags` `improved`. A/A: `compare-baseline-aa` 0 regressed, 0 improved; `compare-tree-aa` `follow` +0.200 and `team_bags` +0.020 `regressed`, `buffs` −0.022 and `stats_decay` −0.128 `improved`, all four on Q7's list. Q7 on the direct pair: `buffs` passes (an A/A pair moved it 0.022, as far); **`stats_decay` does not**, the A/A pairs having moved it at most 0.128 against the direct pair's 0.130. Its other readings: `compare-direct-again` −0.004 `noisy`, the cross pairs +0.002 `noisy` and +0.124 `regressed` (the one holding the first tree suite), `compare-aligned` +0.034 [−0.089, +0.157] `noisy`; `ecs::systems::stats_decay` sits at offset 32 modulo 64 in the plain baseline binary and 16 in the plain tree binary. Per-run p50: the first tree suite held five mode runs of five (`stats_decay` 0.31 ms each), every other suite one or two, both builds included. Gates 14:53:27–14:53:59 UTC, no encoder: `just smoke` exit 0 (every example OK, Benchmarks 15/15); `just visual` exit 0 (2 + 5 + 1 passed, the deliberate `fade` ERROR only); `just physics-release` exit 0, 4 + 1 + 7 passed with `0x088ec07a73c1b168` and `0xaee272e01ffc4e4c`. Scratch copies deleted; `fd -I --max-depth 1 '^m39-' target` prints nothing. No recapture (§7) | `perf-runs/20261006-m39-clock/` (machine-local): `README.md`, `summary.md`, `compares.log`, `sitting.log`, `gates.log`, `placement.txt`, `scripts/` |
| 5 | 2026-10-06 | skipped | §7: step 4's `stats_decay` reading is past Q7's exception, so the release waits for the owner: fix it, accept it in this plan (the regression policy's justification), or ask for the policy's placebo build. Nothing of step 5 ran: no `CHANGELOG.md` line, no cut, no release checks, no preflight; the plan stays here, `in progress` | — |
| 5 | 2026-10-06 | pass | Run after the owner's Q7 ruling (§4), which resolves step 4's stop. §8 closed: 1, a row here for every step; 2, the release chain once after the cut, quoted in the session report since the plan is archived before it, with `just physics-release`, `just smoke` and `just visual` from step 4; 3, the hashes in steps 2 and 4, the digests 8 of 8 and the pixel tests in step 4; 4, `D-134` and its row in step 3; 5, rustdoc in step 1, the ledger row and `just api` in step 3; 6, the template and the guide in step 2; 7, the `time.rs` route and `just ctx` in step 3; 8, the M39 section in step 3; 9, the register's W3a row (release 0.53, cut) and a revisions line, the README's Now lines and w03 step 1's landing note, with known issues, the backlog and the gap log unchanged (nothing fixed, cut or closed; A5 is follow-up 2); 10, the `[Unreleased]` entry, `status: done` and this plan archived with its links fixed for the new depth and the implementation plan's two incoming links repointed. `DESIGN.md`'s status line: the M39 head, the M38 head prepended to the chain (`Before that came` 13 → 14). The cut, the release checks and the preflight are in the session report | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/README.md`, `docs/plans/1.0/w03-frame-loop.md`, `docs/plans/archive/1.0/phase5-milestone-39-game-clock-timers.md` |

## Follow-ups

1. Example 01's hand-counted timers on `Timer` (A14, Q5): the Phase 5 QA pass.
2. A `Burst` emitter spawned under a pause fires at once, and a `Pulse` emitter with a backlog fires one pulse a frame until it drains; their particles hang until the clock resumes (A5, Q8): W8a's burst and pulse semantics, beside the burst latch.
3. The template's pause on the game clock (Q6): the acceptance game's pause screens (W1 M3, M4), or W12b.
4. The camera's shake moves once on the first paused frame, since `camera_update_system` draws the offset before it advances the phase (A4): the Phase 5 QA pass, if a game shows it; reordering changes every shake sequence.
5. `just api` prints two `rustdoc::private_intra_doc_links` warnings: `crates/tungsten/src/sprite_extract.rs`'s module docs link the private `ExtractScratch` (lines 28 and 33 at `36b766d`; M39 does not touch the file). W15c, which edits the file, or W4b's documentation sweep.
