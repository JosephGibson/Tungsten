//! Frame time: the [`Time`] resource with the real and game clocks and the
//! fixed-step accumulator, the [`Timer`] countdown, and [`DeltaTime`], the
//! single dt `Time` replaces (`D-129`).
//!
//! The app advances `Time` once a frame, before any stage runs. The real
//! clock takes the dt the loop gives it: elapsed wall time capped at 0.1 s
//! (`D-088`), 1/60 s in smoke runs, or the test harness's dt as given
//! (`D-110`). The game clock is the real dt times the [scale](Time::scale),
//! zero while [paused](Time::is_paused). Systems read [`Time::delta`];
//! anything that must run over a pause, such as a screen transition, reads
//! [`Time::real_delta`].
//!
//! The accumulator spends game time in whole [fixed steps](Time::fixed_step),
//! 1/60 s unless set: the app runs `fixed_update` once per step, zero or more
//! times a frame and at most [`max_steps_per_frame`](Time::max_steps_per_frame).
//! Whole steps past that bound are dropped, and the fraction of a step left
//! over is [`alpha`](Time::alpha), by which bodies with history are drawn
//! between their last two steps. Inside the steps `delta` is the step. At
//! the smoke pin every frame runs one step with `alpha` 0.
//!
//! A world driven by hand, without the app, calls [`Time::advance_frame`]
//! in the clock stage's place. It reads the game dt everywhere unless it
//! brackets its fixed systems with [`Time::enter_fixed_step`] and
//! [`Time::leave_fixed_steps`], running them
//! [`fixed_steps_this_frame`](Time::fixed_steps_this_frame) times.
//!
//! A [`Timer`] is a plain value that a component or resource owns and ticks
//! with whichever clock its owner picks; there is no timer system.

/// Elapsed seconds between frames: the frame's single dt before [`Time`].
///
/// Systems read [`Time::delta`], or [`Time::real_delta`] for real time. The
/// app still writes the dt here, the step inside `fixed_update` and the game
/// dt everywhere else, so a game that reads it keeps working until W4b
/// removes the type; a world built by hand inserts and advances `Time`
/// instead, since engine systems no longer read this.
#[deprecated(
    since = "0.53.0",
    note = "read `Time::delta()`, or `Time::real_delta()` for real time; \
            App still writes the game dt here each frame. Removed at W4b"
)]
#[derive(Debug, Clone, Copy)]
pub struct DeltaTime {
    /// This frame's dt in seconds.
    pub dt: f32,
}

#[allow(deprecated)]
impl DeltaTime {
    /// A zero dt.
    #[must_use]
    pub fn new() -> Self {
        Self { dt: 0.0 }
    }

    /// This frame's dt in seconds, [`dt`](Self::dt).
    #[must_use]
    pub fn seconds(&self) -> f32 {
        self.dt
    }
}

#[allow(deprecated)]
impl Default for DeltaTime {
    fn default() -> Self {
        Self::new()
    }
}

/// The frame's clocks and the fixed-step accumulator: a `World` resource the
/// app inserts and advances once a frame, before any stage runs.
///
/// - **Real** time is the dt the loop gives the frame
///   ([`real_delta`](Self::real_delta)); it keeps running while the game is
///   paused.
/// - **Game** time is real time times [`scale`](Self::scale), zero while
///   [paused](Self::is_paused) ([`game_delta`](Self::game_delta)).
/// - **Fixed** time is game time spent in whole steps of
///   [`fixed_step`](Self::fixed_step): the frame runs
///   [`fixed_steps_this_frame`](Self::fixed_steps_this_frame) steps, at most
///   [`max_steps_per_frame`](Self::max_steps_per_frame), and keeps the rest
///   of a step as [`alpha`](Self::alpha).
///
/// [`delta`](Self::delta) is the dt a system reads: the step inside
/// `fixed_update`, the game dt everywhere else, so a pause freezes and a
/// scale slows or speeds whatever moves by it. A paused frame runs no step
/// and holds `alpha`. Scale, pause and the step settings take effect from
/// the next frame, so a system that changes them leaves the current frame's
/// dt and steps as they were. Elapsed time on both clocks sums every frame's
/// dt in `f64`; [`frame`](Self::frame) counts frames started.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Time {
    real_delta: f32,
    game_delta: f32,
    real_elapsed: f64,
    elapsed: f64,
    frame: u64,
    scale: f32,
    paused: bool,
    /// The step setting; `frame_step` takes it at each `advance_frame`.
    fixed_step: f32,
    max_steps_per_frame: u32,
    interpolate: bool,
    /// Game time not yet stepped: below `frame_step` after a frame starts.
    accumulator: f32,
    /// The step this frame's steps take.
    frame_step: f32,
    steps_this_frame: u32,
    dropped_this_frame: f32,
    alpha: f32,
    in_fixed_step: bool,
}

/// The default step, the smoke pin: a pinned frame runs exactly one step.
const DEFAULT_FIXED_STEP: f32 = 1.0 / 60.0;

/// The default bound: 1/30 s of game time a frame at the default step,
/// `D-094`'s bound on one physics call.
const DEFAULT_MAX_STEPS_PER_FRAME: u32 = 2;

impl Time {
    /// Both clocks at zero, scale 1, running, before the first frame, with a
    /// 1/60 s step, two steps a frame at most and interpolation on.
    #[must_use]
    pub fn new() -> Self {
        Self {
            real_delta: 0.0,
            game_delta: 0.0,
            real_elapsed: 0.0,
            elapsed: 0.0,
            frame: 0,
            scale: 1.0,
            paused: false,
            fixed_step: DEFAULT_FIXED_STEP,
            max_steps_per_frame: DEFAULT_MAX_STEPS_PER_FRAME,
            interpolate: true,
            accumulator: 0.0,
            frame_step: DEFAULT_FIXED_STEP,
            steps_this_frame: 0,
            dropped_this_frame: 0.0,
            alpha: 0.0,
            in_fixed_step: false,
        }
    }

    /// Starts a frame of `real_dt` seconds: sets both clocks' dt, adds them
    /// to their elapsed times, counts the frame and decides its fixed steps.
    ///
    /// The app's clock stage calls it once a frame with the loop's dt
    /// (capped, pinned, or the harness's); a world driven by hand calls it in
    /// that stage's place. A negative or non-finite `real_dt` counts as zero.
    /// The game dt is `real_dt` times the scale, zero while paused, and
    /// saturates at `f32::MAX`.
    ///
    /// The accumulator adds the game dt, saturating at `f32::MAX`, then gives
    /// up one step at a time while it holds a whole step, until the bound's
    /// steps have run or a step no longer changes it. Whole steps still left
    /// are dropped and the remainder kept, so `alpha` stays in `[0, 1)`. A
    /// paused frame adds nothing, runs no step and holds `alpha`.
    pub fn advance_frame(&mut self, real_dt: f32) {
        let real_dt = if real_dt.is_finite() && real_dt > 0.0 {
            real_dt
        } else {
            0.0
        };
        self.real_delta = real_dt;
        self.game_delta = if self.paused {
            0.0
        } else {
            (real_dt * self.scale).min(f32::MAX)
        };
        self.real_elapsed += f64::from(real_dt);
        self.elapsed += f64::from(self.game_delta);
        self.frame += 1;
        self.in_fixed_step = false;
        self.frame_step = self.fixed_step;
        self.steps_this_frame = 0;
        self.dropped_this_frame = 0.0;
        if !self.paused {
            self.step_accumulator();
        }
    }

    /// Spends the accumulator in whole steps (the spike's loop, `D-129`).
    fn step_accumulator(&mut self) {
        let step = self.frame_step;
        self.accumulator = (self.accumulator + self.game_delta).min(f32::MAX);
        while self.steps_this_frame < self.max_steps_per_frame && self.accumulator >= step {
            let rest = self.accumulator - step;
            if rest == self.accumulator {
                // The step is below the sum's precision: drop it whole below.
                break;
            }
            self.accumulator = rest;
            self.steps_this_frame += 1;
        }
        if self.accumulator >= step {
            // `%` is exact, so the remainder is a finite value below the step.
            let rest = self.accumulator % step;
            self.dropped_this_frame = self.accumulator - rest;
            self.accumulator = rest;
        }
        self.alpha = self.accumulator / step;
    }

    /// The dt a system reads: the step inside the frame's fixed steps
    /// (between [`enter_fixed_step`](Self::enter_fixed_step) and
    /// [`leave_fixed_steps`](Self::leave_fixed_steps)), this frame's game dt
    /// everywhere else.
    #[must_use]
    pub fn delta(&self) -> f32 {
        if self.in_fixed_step {
            self.frame_step
        } else {
            self.game_delta
        }
    }

    /// This frame's game dt: the real dt times the scale, zero while paused.
    #[must_use]
    pub fn game_delta(&self) -> f32 {
        self.game_delta
    }

    /// This frame's real dt, as the loop gave it: wall time capped at 0.1 s,
    /// the smoke pin, or the harness's dt.
    #[must_use]
    pub fn real_delta(&self) -> f32 {
        self.real_delta
    }

    /// Game seconds summed over every frame so far.
    #[must_use]
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    /// Real seconds summed over every frame so far.
    #[must_use]
    pub fn real_elapsed(&self) -> f64 {
        self.real_elapsed
    }

    /// Frames started, this one included; 0 before the first.
    #[must_use]
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// The game clock's rate against real time; 1 unless set.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Sets the game clock's rate from the next frame on. A negative or
    /// non-finite scale counts as zero.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            0.0
        };
    }

    /// Whether the game clock is paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Stops the game clock from the next frame on: its dt is zero and its
    /// elapsed time holds. Real time keeps running.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Restarts the game clock from the next frame on.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Pauses or resumes the game clock from the next frame on.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// The fixed step in game seconds: 1/60 s, the smoke pin, unless set.
    #[must_use]
    pub fn fixed_step(&self) -> f32 {
        self.fixed_step
    }

    /// Sets the fixed step in game seconds from the next frame on.
    ///
    /// `physics_step` clamps each call to
    /// [`PhysicsConfig::max_step_dt`](crate::physics::PhysicsConfig::max_step_dt),
    /// 1/30 s by default (`D-094`), so a longer step makes physics run slow
    /// against the game unless that bound rises with it. `tungsten.json`'s
    /// `time` section refuses a step longer than 1/30 s.
    ///
    /// # Panics
    /// When `secs` is not finite or not positive.
    pub fn set_fixed_step(&mut self, secs: f32) {
        assert!(
            secs.is_finite() && secs > 0.0,
            "Time::set_fixed_step: step must be finite and positive, got {secs}"
        );
        self.fixed_step = secs;
    }

    /// The most fixed steps a frame runs: 2 unless set.
    #[must_use]
    pub fn max_steps_per_frame(&self) -> u32 {
        self.max_steps_per_frame
    }

    /// Sets the most fixed steps a frame runs, from the next frame on. A
    /// frame that owes more drops the whole steps past the bound, so the game
    /// runs slow on a slow frame instead of falling further behind.
    ///
    /// # Panics
    /// When `steps` is 0.
    pub fn set_max_steps_per_frame(&mut self, steps: u32) {
        assert!(
            steps > 0,
            "Time::set_max_steps_per_frame: a frame needs at least one step"
        );
        self.max_steps_per_frame = steps;
    }

    /// How many fixed steps this frame runs: zero or more, at most
    /// [`max_steps_per_frame`](Self::max_steps_per_frame); none while paused.
    #[must_use]
    pub fn fixed_steps_this_frame(&self) -> u32 {
        self.steps_this_frame
    }

    /// The game seconds this frame dropped as whole steps past the bound; 0
    /// unless the frame owed more steps than the bound allows.
    #[must_use]
    pub fn dropped_this_frame(&self) -> f32 {
        self.dropped_this_frame
    }

    /// The game time left over after this frame's steps, as a fraction of
    /// the step, in `[0, 1)`: where the frame sits between the last two
    /// steps. Interpolation draws a body at `prev + (cur - prev) * alpha`.
    #[must_use]
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Whether bodies that carry a previous position are drawn interpolated
    /// by [`alpha`](Self::alpha): on unless set.
    #[must_use]
    pub fn interpolate(&self) -> bool {
        self.interpolate
    }

    /// Turns render interpolation on or off. Off, `PhysicsPlugin` draws every
    /// body at its `Position`, as a body without history is drawn.
    pub fn set_interpolate(&mut self, on: bool) {
        self.interpolate = on;
    }

    /// Starts one of the frame's fixed steps: [`delta`](Self::delta) reads
    /// the step until [`leave_fixed_steps`](Self::leave_fixed_steps). The app
    /// calls it before each step; a world driven by hand that never calls it
    /// reads the game dt throughout.
    pub fn enter_fixed_step(&mut self) {
        self.in_fixed_step = true;
    }

    /// Ends the frame's fixed steps: [`delta`](Self::delta) reads the game dt
    /// again. The app calls it once, after the last step.
    pub fn leave_fixed_steps(&mut self) {
        self.in_fixed_step = false;
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a [`Timer`] stops at its duration or starts over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    /// Finishes once; later ticks leave it finished until
    /// [`reset`](Timer::reset).
    Once,
    /// Finishes every `duration` seconds; one long tick can finish it
    /// several times.
    Repeating,
}

/// A countdown that a component or resource owns and ticks with whichever
/// clock its owner picks: [`Time::delta`] for gameplay, [`Time::real_delta`]
/// for anything that runs over a pause. There is no timer system or event
/// queue: [`tick`](Self::tick) returns how many times the timer finished.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timer {
    duration: f32,
    elapsed: f32,
    mode: TimerMode,
    finished: bool,
    finished_this_tick: u32,
}

impl Timer {
    /// A timer of `duration` seconds at zero elapsed time. A `Once` timer may
    /// last zero seconds; it finishes on its first tick.
    ///
    /// # Panics
    /// When `duration` is negative or not finite, or zero for a repeating
    /// timer.
    #[must_use]
    pub fn new(duration: f32, mode: TimerMode) -> Self {
        assert_duration("Timer::new", duration, mode);
        Self {
            duration,
            elapsed: 0.0,
            mode,
            finished: false,
            finished_this_tick: 0,
        }
    }

    /// A [`TimerMode::Once`] timer of `duration` seconds.
    ///
    /// # Panics
    /// As [`Timer::new`].
    #[must_use]
    pub fn once(duration: f32) -> Self {
        Self::new(duration, TimerMode::Once)
    }

    /// A [`TimerMode::Repeating`] timer of `duration` seconds.
    ///
    /// # Panics
    /// As [`Timer::new`].
    #[must_use]
    pub fn repeating(duration: f32) -> Self {
        Self::new(duration, TimerMode::Repeating)
    }

    /// Advances the timer by `dt` seconds and returns how many times it
    /// finished during this tick, so a long frame cannot swallow a repeat. A
    /// negative or non-finite `dt` counts as zero.
    ///
    /// A `Once` timer returns 1 on the tick its elapsed time reaches the
    /// duration and 0 on every later one. A `Repeating` timer keeps what is
    /// left past its last finished period: the count saturates at
    /// `u32::MAX`, and [`elapsed`](Self::elapsed) stays within the duration
    /// whatever `dt` is.
    pub fn tick(&mut self, dt: f32) -> u32 {
        let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
        self.finished_this_tick = match self.mode {
            TimerMode::Once if self.finished => 0,
            TimerMode::Once => {
                self.elapsed += dt;
                if self.elapsed >= self.duration {
                    self.elapsed = self.duration;
                    self.finished = true;
                    1
                } else {
                    0
                }
            }
            TimerMode::Repeating => {
                let (count, rest) = whole_periods(self.elapsed, dt, self.duration);
                self.elapsed = rest;
                self.finished = count > 0;
                count
            }
        };
        self.finished_this_tick
    }

    /// `Once`: whether the timer has finished since it started or was reset.
    /// `Repeating`: whether it finished during the last tick.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Whether the last tick finished the timer at least once.
    #[must_use]
    pub fn just_finished(&self) -> bool {
        self.finished_this_tick > 0
    }

    /// How many times the last tick finished the timer.
    #[must_use]
    pub fn times_finished_this_tick(&self) -> u32 {
        self.finished_this_tick
    }

    /// Seconds into the current period, within `[0, duration]`.
    #[must_use]
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    /// Seconds left in the current period.
    #[must_use]
    pub fn remaining(&self) -> f32 {
        (self.duration - self.elapsed).max(0.0)
    }

    /// Elapsed time over the duration, within `[0, 1]`; 1 for a zero
    /// duration.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        if self.duration > 0.0 {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// The period in seconds.
    #[must_use]
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Whether the timer stops at its duration or starts over.
    #[must_use]
    pub fn mode(&self) -> TimerMode {
        self.mode
    }

    /// Back to zero elapsed time, not finished.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.finished = false;
        self.finished_this_tick = 0;
    }

    /// Changes the period, keeping the elapsed time clamped to the new
    /// duration: a timer that the new duration has overtaken finishes on its
    /// next tick, and a finished `Once` timer stays finished.
    ///
    /// # Panics
    /// As [`Timer::new`].
    pub fn set_duration(&mut self, duration: f32) {
        assert_duration("Timer::set_duration", duration, self.mode);
        self.duration = duration;
        self.elapsed = self.elapsed.min(duration);
    }
}

fn assert_duration(caller: &str, duration: f32, mode: TimerMode) {
    assert!(
        duration.is_finite() && duration >= 0.0,
        "{caller}: duration must be finite and not negative, got {duration}"
    );
    assert!(
        mode == TimerMode::Once || duration > 0.0,
        "{caller}: a repeating timer needs a positive duration"
    );
}

/// Splits `elapsed + dt` into whole periods of `duration` and the rest: the
/// count, saturating at `u32::MAX`, and the rest, within `[0, duration)`.
/// Works in `f64`, where neither the sum nor the quotient of two finite
/// `f32` values overflows.
fn whole_periods(elapsed: f32, dt: f32, duration: f32) -> (u32, f32) {
    let period = f64::from(duration);
    let total = f64::from(elapsed) + f64::from(dt);
    // `%` is exact, so the rest is within [0, period) whatever the quotient.
    let rest = total % period;
    // A whole number up to rounding, which `round` removes below 2^52.
    let periods = ((total - rest) / period).round();
    let mut count = if periods < f64::from(u32::MAX) {
        periods as u32
    } else {
        u32::MAX
    };
    let mut rest = rest as f32;
    if rest >= duration {
        // Rounding to `f32` landed on the duration: one more period.
        rest = 0.0;
        count = count.saturating_add(1);
    }
    (count, rest)
}

#[cfg(test)]
#[path = "tests/time.rs"]
mod tests;
