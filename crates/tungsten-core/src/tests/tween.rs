use super::*;

const EPS: f32 = 1e-5;

fn approx(a: f32, b: f32, eps: f32) {
    assert!((a - b).abs() < eps, "expected {b}, got {a}");
}

#[test]
fn linear_is_identity() {
    assert_eq!(Easing::Linear.apply(0.0), 0.0);
    assert_eq!(Easing::Linear.apply(0.25), 0.25);
    assert_eq!(Easing::Linear.apply(1.0), 1.0);
}

#[test]
fn standard_endpoints_are_zero_and_one() {
    for e in [
        Easing::Linear,
        Easing::QuadIn,
        Easing::QuadOut,
        Easing::QuadInOut,
        Easing::CubicIn,
        Easing::CubicOut,
        Easing::CubicInOut,
        Easing::QuartIn,
        Easing::QuartOut,
        Easing::QuartInOut,
        Easing::SineIn,
        Easing::SineOut,
        Easing::SineInOut,
        Easing::ExpoIn,
        Easing::ExpoOut,
        Easing::ExpoInOut,
    ] {
        approx(e.apply(0.0), 0.0, EPS);
        approx(e.apply(1.0), 1.0, EPS);
    }
}

#[test]
fn back_endpoints_are_zero_and_one() {
    for e in [Easing::BackIn, Easing::BackOut, Easing::BackInOut] {
        approx(e.apply(0.0), 0.0, EPS);
        approx(e.apply(1.0), 1.0, EPS);
    }
}

#[test]
fn back_out_overshoots_above_one_mid_range() {
    let sample = Easing::BackOut.apply(0.7);
    assert!(sample > 1.0, "expected overshoot > 1, got {sample}");
}

#[test]
fn quad_in_half_is_quarter() {
    approx(Easing::QuadIn.apply(0.5), 0.25, EPS);
}

#[test]
fn cubic_in_half_is_eighth() {
    approx(Easing::CubicIn.apply(0.5), 0.125, EPS);
}

#[test]
fn quart_in_half_is_sixteenth() {
    approx(Easing::QuartIn.apply(0.5), 0.0625, EPS);
}

#[test]
fn sine_in_out_half_is_half() {
    approx(Easing::SineInOut.apply(0.5), 0.5, EPS);
}

#[test]
fn bounce_out_known_values() {
    approx(Easing::BounceOut.apply(0.0), 0.0, EPS);
    approx(Easing::BounceOut.apply(1.0), 1.0, EPS);
    approx(Easing::BounceOut.apply(1.0 / 2.75), 1.0, EPS);
}

#[test]
fn bounce_in_is_reflected_bounce_out() {
    for t in [0.0_f32, 0.15, 0.37, 0.5, 0.72, 0.91, 1.0] {
        approx(
            Easing::BounceIn.apply(t),
            1.0 - Easing::BounceOut.apply(1.0 - t),
            EPS,
        );
    }
}

#[test]
fn expo_in_zero_stays_zero() {
    assert_eq!(Easing::ExpoIn.apply(0.0), 0.0);
    approx(Easing::ExpoIn.apply(1.0), 1.0, EPS);
}

#[test]
fn lerp_u8_endpoints_are_exact() {
    assert_eq!(lerp_u8(0, 255, 0.0), 0);
    assert_eq!(lerp_u8(0, 255, 1.0), 255);
}

#[test]
fn lerp_u8_midpoint_rounds() {
    assert_eq!(lerp_u8(0, 255, 0.5), 128);
    assert_eq!(lerp_u8(10, 30, 0.5), 20);
}

#[test]
fn lerp_u8_clamps_negative_and_over() {
    assert_eq!(lerp_u8(100, 200, -1.0), 0);
    assert_eq!(lerp_u8(100, 200, 5.0), 255);
}

#[test]
fn lerp_f32_linear() {
    approx(lerp_f32(10.0, 30.0, 0.25), 15.0, EPS);
    approx(lerp_f32(-4.0, 4.0, 0.5), 0.0, EPS);
}

#[test]
fn tween_builder_accumulates_channels_and_tag() {
    let t = Tween::new(0.5, Easing::CubicOut)
        .with_channel(TweenChannel::ColorA { from: 0, to: 255 })
        .with_channel(TweenChannel::PositionX {
            from: 0.0,
            to: 10.0,
        })
        .with_repeat(TweenRepeat::Times(3))
        .with_tag("state_exit");
    assert_eq!(t.channels.len(), 2);
    assert_eq!(t.duration, 0.5);
    assert_eq!(t.easing, Easing::CubicOut);
    assert_eq!(t.repeat, TweenRepeat::Times(3));
    assert_eq!(t.on_complete_tag.as_deref(), Some("state_exit"));
    assert_eq!(t.direction, TweenDirection::Forward);
}

#[test]
fn tween_new_positive_duration_is_preserved() {
    let t = Tween::new(0.1, Easing::Linear);
    assert!(t.duration.is_finite() && t.duration > 0.0);
    assert_eq!(t.duration, 0.1);
}
