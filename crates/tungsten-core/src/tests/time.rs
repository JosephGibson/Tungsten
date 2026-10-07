use super::*;

const DT: f32 = 1.0 / 60.0;

#[test]
fn six_hundred_frames_at_scale_one_keep_real_and_game_time_equal() {
    let mut time = Time::new();
    for _ in 0..600 {
        time.advance_frame(DT);
    }
    assert_eq!(time.frame(), 600);
    assert_eq!(time.real_delta().to_bits(), DT.to_bits());
    assert_eq!(time.game_delta().to_bits(), DT.to_bits());
    assert_eq!(time.delta().to_bits(), DT.to_bits());
    assert_eq!(time.elapsed().to_bits(), time.real_elapsed().to_bits());
    assert!(
        (time.elapsed() - 10.0).abs() < 1e-6,
        "elapsed {}",
        time.elapsed()
    );
}

#[test]
fn power_of_two_scales_halve_and_double_game_time_exactly() {
    let mut unit = Time::new();
    let mut half = Time::new();
    half.set_scale(0.5);
    let mut double = Time::new();
    double.set_scale(2.0);
    for _ in 0..600 {
        for time in [&mut unit, &mut half, &mut double] {
            time.advance_frame(DT);
        }
    }
    assert_eq!(half.game_delta(), half.real_delta() * 0.5);
    assert_eq!(half.delta(), half.game_delta());
    assert_eq!(half.elapsed(), half.real_elapsed() * 0.5);
    assert_eq!(double.game_delta(), double.real_delta() * 2.0);
    assert_eq!(double.elapsed(), double.real_elapsed() * 2.0);
    for time in [&half, &double] {
        assert_eq!(time.real_delta(), unit.real_delta());
        assert_eq!(time.real_elapsed(), unit.real_elapsed());
    }
}

#[test]
fn a_pause_holds_game_time_while_real_time_and_frames_count() {
    let mut time = Time::new();
    time.advance_frame(DT);
    time.pause();
    assert!(time.is_paused());
    assert_eq!(time.game_delta(), DT, "a pause starts from the next frame");
    let held = time.elapsed();
    for _ in 0..10 {
        time.advance_frame(DT);
        assert_eq!(time.game_delta(), 0.0);
        assert_eq!(time.delta(), 0.0);
        assert_eq!(time.real_delta(), DT);
        assert_eq!(time.elapsed(), held);
    }
    assert_eq!(time.frame(), 11);
    assert!((time.real_elapsed() - 11.0 * f64::from(DT)).abs() < 1e-9);

    time.resume();
    assert!(!time.is_paused());
    time.advance_frame(DT);
    assert_eq!(time.game_delta(), DT);
    assert_eq!(time.elapsed(), held + f64::from(DT));

    time.set_paused(true);
    assert!(time.is_paused());
    time.set_paused(false);
    assert!(!time.is_paused());
}

#[test]
fn a_new_time_reads_zero_at_scale_one_running() {
    let time = Time::new();
    assert_eq!(time.frame(), 0);
    assert_eq!(time.real_delta(), 0.0);
    assert_eq!(time.game_delta(), 0.0);
    assert_eq!(time.delta(), 0.0);
    assert_eq!(time.real_elapsed(), 0.0);
    assert_eq!(time.elapsed(), 0.0);
    assert_eq!(time.scale(), 1.0);
    assert!(!time.is_paused());
    assert_eq!(Time::default(), time);
}

#[test]
fn set_scale_turns_negative_and_non_finite_scales_into_zero() {
    let mut time = Time::new();
    for scale in [-1.0, f32::NAN, f32::INFINITY] {
        time.set_scale(2.0);
        time.set_scale(scale);
        assert_eq!(time.scale(), 0.0, "scale {scale}");
    }
}

#[test]
fn advance_frame_counts_negative_and_nan_dts_as_zero() {
    let mut time = Time::new();
    for (frame, dt) in (1..).zip([-1.0, f32::NAN]) {
        time.advance_frame(dt);
        assert_eq!(time.real_delta(), 0.0, "dt {dt}");
        assert_eq!(time.game_delta(), 0.0, "dt {dt}");
        assert_eq!(time.real_elapsed(), 0.0, "dt {dt}");
        assert_eq!(time.elapsed(), 0.0, "dt {dt}");
        assert_eq!(time.frame(), frame);
    }
}

#[test]
fn the_game_dt_saturates_at_f32_max() {
    let mut time = Time::new();
    time.set_scale(2.0);
    time.advance_frame(f32::MAX);
    assert_eq!(time.game_delta(), f32::MAX);
    assert!(time.elapsed().is_finite());
    assert!(time.real_elapsed().is_finite());
}

#[test]
fn a_new_time_steps_at_the_smoke_pin_two_a_frame_interpolating() {
    let time = Time::new();
    assert_eq!(time.fixed_step().to_bits(), (1.0_f32 / 60.0).to_bits());
    assert_eq!(time.max_steps_per_frame(), 2);
    assert!(time.interpolate());
    assert_eq!(time.fixed_steps_this_frame(), 0);
    assert_eq!(time.dropped_this_frame(), 0.0);
    assert_eq!(time.alpha(), 0.0);
}

#[test]
fn six_hundred_pinned_frames_run_one_step_each_at_alpha_zero() {
    let mut time = Time::new();
    for frame in 0..600 {
        time.advance_frame(DT);
        assert_eq!(time.fixed_steps_this_frame(), 1, "frame {frame}");
        assert_eq!(time.alpha(), 0.0, "frame {frame}");
        assert_eq!(time.dropped_this_frame(), 0.0, "frame {frame}");
    }
}

#[test]
fn half_a_step_a_frame_alternates_none_and_one() {
    let mut time = Time::new();
    for frame in 0..600 {
        time.advance_frame(1.0 / 120.0);
        let (steps, alpha) = if frame % 2 == 0 { (0, 0.5) } else { (1, 0.0) };
        assert_eq!(time.fixed_steps_this_frame(), steps, "frame {frame}");
        assert_eq!(time.alpha(), alpha, "frame {frame}");
    }
}

#[test]
fn two_steps_a_frame_run_at_1_30_s_and_at_scale_two() {
    let mut slow = Time::new();
    let mut scaled = Time::new();
    scaled.set_scale(2.0);
    for frame in 0..60 {
        slow.advance_frame(1.0 / 30.0);
        scaled.advance_frame(DT);
        for time in [&slow, &scaled] {
            assert_eq!(time.fixed_steps_this_frame(), 2, "frame {frame}");
            assert_eq!(time.alpha(), 0.0, "frame {frame}");
            assert_eq!(time.dropped_this_frame(), 0.0, "frame {frame}");
        }
    }
}

#[test]
fn a_stall_runs_the_bound_and_drops_the_whole_steps_left() {
    let mut time = Time::new();
    time.advance_frame(0.1);
    assert_eq!(time.fixed_steps_this_frame(), 2);
    let dropped = time.dropped_this_frame();
    assert!((dropped - 4.0 * DT).abs() < 1e-6, "dropped {dropped}");
    assert!(time.alpha() < 1e-6, "alpha {}", time.alpha());
    time.advance_frame(DT);
    assert_eq!(time.fixed_steps_this_frame(), 1);
    assert_eq!(time.dropped_this_frame(), 0.0);

    let mut wide = Time::new();
    wide.set_max_steps_per_frame(6);
    wide.advance_frame(0.1);
    assert_eq!(wide.fixed_steps_this_frame(), 6);
    assert_eq!(wide.dropped_this_frame(), 0.0);
}

#[test]
fn a_paused_frame_runs_no_step_and_holds_alpha_as_real_time_runs() {
    let mut time = Time::new();
    time.advance_frame(1.0 / 120.0);
    assert_eq!(time.alpha(), 0.5);
    time.pause();
    for _ in 0..10 {
        time.advance_frame(DT);
        assert_eq!(time.fixed_steps_this_frame(), 0);
        assert_eq!(time.dropped_this_frame(), 0.0);
        assert_eq!(time.alpha(), 0.5);
        assert_eq!(time.real_delta(), DT);
    }
    assert_eq!(time.frame(), 11);
    assert!((time.real_elapsed() - (10.5 * f64::from(DT))).abs() < 1e-9);

    time.resume();
    time.advance_frame(DT);
    assert_eq!(time.fixed_steps_this_frame(), 1);
    assert!((time.alpha() - 0.5).abs() < 1e-6, "alpha {}", time.alpha());
}

#[test]
fn delta_is_the_step_only_between_enter_and_leave() {
    let mut time = Time::new();
    time.advance_frame(1.0 / 144.0);
    assert_eq!(time.delta(), 1.0 / 144.0);
    time.enter_fixed_step();
    assert_eq!(time.delta(), DT);
    time.enter_fixed_step();
    assert_eq!(time.delta(), DT);
    assert_eq!(time.game_delta(), 1.0 / 144.0);
    time.leave_fixed_steps();
    assert_eq!(time.delta(), 1.0 / 144.0);

    time.enter_fixed_step();
    time.advance_frame(1.0 / 30.0);
    assert_eq!(
        time.delta(),
        1.0 / 30.0,
        "a new frame starts outside the steps"
    );
}

#[test]
fn a_new_step_and_bound_take_effect_at_the_next_frame() {
    let mut time = Time::new();
    time.advance_frame(DT);
    time.set_fixed_step(DT / 2.0);
    time.set_max_steps_per_frame(4);
    time.enter_fixed_step();
    assert_eq!(time.delta(), DT);
    assert_eq!(time.fixed_step(), DT / 2.0);
    time.leave_fixed_steps();
    time.advance_frame(DT);
    assert_eq!(time.fixed_steps_this_frame(), 2);
    time.enter_fixed_step();
    assert_eq!(time.delta(), DT / 2.0);
}

#[test]
fn the_step_setter_panics_on_zero_nan_negative_and_infinite_steps() {
    for secs in [0.0, f32::NAN, -1.0, f32::INFINITY] {
        let result = std::panic::catch_unwind(|| Time::new().set_fixed_step(secs));
        assert!(result.is_err(), "step {secs}");
    }
}

#[test]
#[should_panic(expected = "a frame needs at least one step")]
fn a_zero_step_bound_panics() {
    Time::new().set_max_steps_per_frame(0);
}

#[test]
fn extreme_steps_and_scales_leave_a_finite_accumulator() {
    fn check(time: &Time, case: &str) {
        assert!(time.accumulator.is_finite(), "{case}");
        assert!(
            (0.0..1.0).contains(&time.alpha()),
            "{case}: {}",
            time.alpha()
        );
        assert!(time.fixed_steps_this_frame() <= 2, "{case}");
        assert!(time.dropped_this_frame().is_finite(), "{case}");
    }
    let mut huge = Time::new();
    huge.set_fixed_step(f32::MAX);
    huge.advance_frame(f32::MAX / 2.0);
    check(&huge, "a step of f32::MAX, half of it");
    huge.advance_frame(f32::MAX);
    check(&huge, "a step of f32::MAX, all of it");

    let mut fast = Time::new();
    fast.set_scale(f32::MAX);
    for dt in [DT, f32::MAX, DT] {
        fast.advance_frame(dt);
        check(&fast, &format!("scale f32::MAX at {dt}"));
    }
}

#[test]
fn a_repeating_timer_counts_every_period_of_a_long_tick() {
    let mut timer = Timer::repeating(0.1);
    assert_eq!(timer.mode(), TimerMode::Repeating);
    assert_eq!(timer.tick(0.35), 3);
    assert!(
        (timer.elapsed() - 0.05).abs() < 1e-6,
        "elapsed {}",
        timer.elapsed()
    );
    assert!(timer.just_finished());
    assert!(timer.finished());
    assert_eq!(timer.times_finished_this_tick(), 3);
    assert_eq!(timer.tick(0.01), 0);
    assert!(!timer.finished());
    assert!(!timer.just_finished());
}

#[test]
fn a_once_timer_finishes_once_and_stays_finished() {
    let mut timer = Timer::once(0.2);
    assert_eq!(timer.mode(), TimerMode::Once);
    assert_eq!(timer.tick(0.1), 0);
    assert!(!timer.finished());
    assert_eq!(timer.tick(0.15), 1);
    assert!(timer.finished());
    assert!(timer.just_finished());
    assert_eq!(timer.tick(1.0), 0);
    assert!(timer.finished());
    assert!(!timer.just_finished());
    assert_eq!(timer.elapsed(), timer.duration());
    assert_eq!(timer.remaining(), 0.0);
    assert_eq!(timer.fraction(), 1.0);
}

#[test]
fn a_zero_duration_once_timer_finishes_on_its_first_tick() {
    let mut timer = Timer::once(0.0);
    assert_eq!(timer.fraction(), 1.0);
    assert_eq!(timer.tick(0.0), 1);
    assert!(timer.finished());
}

#[test]
fn negative_and_nan_ticks_count_as_zero() {
    for mode in [TimerMode::Once, TimerMode::Repeating] {
        let mut timer = Timer::new(1.0, mode);
        timer.tick(0.25);
        assert_eq!(timer.tick(-1.0), 0, "{mode:?}");
        assert_eq!(timer.tick(f32::NAN), 0, "{mode:?}");
        assert_eq!(timer.tick(f32::INFINITY), 0, "{mode:?}");
        assert_eq!(timer.elapsed(), 0.25, "{mode:?}");
    }
}

#[test]
fn reset_clears_finished_and_elapsed() {
    let mut timer = Timer::once(0.5);
    assert_eq!(timer.tick(0.75), 1);
    timer.reset();
    assert!(!timer.finished());
    assert!(!timer.just_finished());
    assert_eq!(timer.elapsed(), 0.0);
    assert_eq!(timer.remaining(), 0.5);
}

#[test]
fn a_repeating_count_saturates_with_elapsed_within_the_duration() {
    let mut tiny = Timer::repeating(1e-10);
    assert_eq!(tiny.tick(1.0), u32::MAX);
    assert!(
        (0.0..1e-10).contains(&tiny.elapsed()),
        "elapsed {}",
        tiny.elapsed()
    );

    let mut unit = Timer::repeating(1.0);
    assert_eq!(unit.tick(f32::MAX), u32::MAX);
    assert!(
        (0.0..1.0).contains(&unit.elapsed()),
        "elapsed {}",
        unit.elapsed()
    );

    let mut huge = Timer::repeating(f32::MAX);
    assert_eq!(huge.tick(f32::MAX / 2.0), 0);
    assert_eq!(huge.tick(f32::MAX), 1);
    assert!(huge.elapsed().is_finite());
    assert!(
        (0.0..=f32::MAX).contains(&huge.elapsed()),
        "elapsed {}",
        huge.elapsed()
    );
}

#[test]
fn set_duration_clamps_elapsed_to_the_new_duration() {
    let mut finished = Timer::once(1.0);
    finished.tick(1.0);
    finished.set_duration(0.5);
    assert_eq!(finished.elapsed(), 0.5);
    assert!(finished.finished());

    let mut running = Timer::once(1.0);
    running.tick(0.8);
    running.set_duration(0.5);
    assert_eq!(running.elapsed(), 0.5);
    assert!(!running.finished());
    assert_eq!(running.tick(0.0), 1);

    let mut repeating = Timer::repeating(1.0);
    repeating.tick(0.8);
    repeating.set_duration(0.5);
    assert_eq!(repeating.tick(0.0), 1);
    assert_eq!(repeating.elapsed(), 0.0);
}

#[test]
#[should_panic(expected = "a repeating timer needs a positive duration")]
fn a_zero_duration_repeating_timer_panics() {
    let _ = Timer::repeating(0.0);
}

#[test]
#[should_panic(expected = "duration must be finite and not negative")]
fn a_negative_duration_panics() {
    let _ = Timer::once(-1.0);
}
