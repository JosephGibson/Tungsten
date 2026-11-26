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
fn easings_at_one_half_hit_their_known_values() {
    for (easing, want) in [
        (Easing::QuadIn, 0.25),
        (Easing::CubicIn, 0.125),
        (Easing::QuartIn, 0.0625),
        (Easing::SineInOut, 0.5),
    ] {
        let got = easing.apply(0.5);
        assert!(
            (got - want).abs() < EPS,
            "{easing:?} at 0.5: expected {want}, got {got}"
        );
    }
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
fn lerp_u8_rounds_and_clamps() {
    for (case, a, b, t, want) in [
        ("the start is exact", 0, 255, 0.0, 0),
        ("the end is exact", 0, 255, 1.0, 255),
        ("the midpoint rounds", 0, 255, 0.5, 128),
        ("a narrow midpoint", 10, 30, 0.5, 20),
        ("a negative t clamps", 100, 200, -1.0, 0),
        ("a t past one clamps", 100, 200, 5.0, 255),
    ] {
        assert_eq!(lerp_u8(a, b, t), want, "{case}: lerp_u8({a}, {b}, {t})");
    }
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
fn uniform_override_block_default_is_all_zero_bytes() {
    let block = UniformOverrideBlock::default();
    assert_eq!(block.to_bytes(), [0u8; 256]);
}

#[test]
fn uniform_override_block_writes_land_in_expected_bytes() {
    // vec4[0] is the first 16 bytes; lane 2 is bytes 8..12.
    let mut block = UniformOverrideBlock::default();
    block.vec4[0][2] = 1.0_f32;
    let bytes = block.to_bytes();
    assert_eq!(&bytes[8..12], &1.0_f32.to_le_bytes());
    // vec4[0][3] stays zero
    assert_eq!(&bytes[12..16], &0.0_f32.to_le_bytes());
    // f32s start at offset 64 (4 vec4s of 16 bytes)
    let mut block = UniformOverrideBlock::default();
    block.f32s[1] = 0.5;
    let bytes = block.to_bytes();
    assert_eq!(&bytes[64 + 4..64 + 8], &0.5_f32.to_le_bytes());
    // i32s start at offset 80
    let mut block = UniformOverrideBlock::default();
    block.i32s[2] = 7;
    let bytes = block.to_bytes();
    assert_eq!(&bytes[80 + 8..80 + 12], &7_i32.to_le_bytes());
}

#[test]
fn slot_enum_indices_are_dense() {
    assert_eq!(Vec4Slot::V0.index(), 0);
    assert_eq!(Vec4Slot::V3.index(), 3);
    assert_eq!(ScalarSlot::F2.index(), 2);
    assert_eq!(IntSlot::I1.index(), 1);
}

#[test]
fn light_intensity_reads_from_a_scene() {
    let tween: crate::assets::SceneTween = serde_json::from_str(
        r#"{ "duration": 0.25, "channels": [{ "kind": "light_intensity", "from": 2.0, "to": 0.0 }] }"#,
    )
    .unwrap();
    tween.validate().unwrap();
    let channels: Vec<TweenChannel> = tween
        .into_tween()
        .channels()
        .iter()
        .map(|eased| eased.channel)
        .collect();
    assert_eq!(
        channels,
        [TweenChannel::LightIntensity { from: 2.0, to: 0.0 }]
    );
    let dimmer: crate::assets::SceneTween = serde_json::from_str(
        r#"{ "duration": 0.25, "channels": [{ "kind": "light_intensity", "from": -1.0, "to": 1.0 }] }"#,
    )
    .unwrap();
    let error = dimmer.validate().unwrap_err();
    assert!(error.contains("light_intensity"), "{error}");
}

#[test]
fn a_channel_takes_its_own_easing() {
    let t = Tween::new(1.0, Easing::Linear)
        .with_channel(TweenChannel::PositionX { from: 0.0, to: 1.0 })
        .with_channel_eased(
            TweenChannel::ScaleX { from: 1.0, to: 2.0 },
            Easing::CubicOut,
        )
        .with_channel(TweenChannel::ColorA { from: 255, to: 0 })
        .with_channel_eased(
            TweenChannel::Rotation { from: 0.0, to: 1.0 },
            Easing::QuadOut,
        );
    let read: Vec<(TweenChannel, Option<Easing>)> = t
        .channels()
        .iter()
        .map(|eased| (eased.channel, eased.easing))
        .collect();
    assert_eq!(
        read,
        [
            (TweenChannel::PositionX { from: 0.0, to: 1.0 }, None),
            (
                TweenChannel::ScaleX { from: 1.0, to: 2.0 },
                Some(Easing::CubicOut)
            ),
            (TweenChannel::ColorA { from: 255, to: 0 }, None),
            (
                TweenChannel::Rotation { from: 0.0, to: 1.0 },
                Some(Easing::QuadOut)
            ),
        ],
        "in the order added, each with its own easing, the plain ones none"
    );
    let eased: Vec<Easing> = t
        .channels()
        .iter()
        .map(|eased| eased.easing_in(t.easing))
        .collect();
    assert_eq!(
        eased,
        [
            Easing::Linear,
            Easing::CubicOut,
            Easing::Linear,
            Easing::QuadOut
        ]
    );
    assert!(!t.despawn_at_end);
    assert!(t.with_despawn_at_end().despawn_at_end);
}
