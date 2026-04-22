use super::*;

fn test_anim() -> AnimationData {
    AnimationData {
        looping: true,
        frames: vec![
            AnimationFrame {
                sprite: "walk_0".into(),
                duration_ms: 100,
            },
            AnimationFrame {
                sprite: "walk_1".into(),
                duration_ms: 100,
            },
            AnimationFrame {
                sprite: "walk_2".into(),
                duration_ms: 100,
            },
            AnimationFrame {
                sprite: "walk_3".into(),
                duration_ms: 100,
            },
        ],
    }
}

#[test]
fn animation_advances_frames() {
    let mut registry = AnimationRegistry::new();
    registry.insert("walk".into(), test_anim());

    let mut state = AnimationState::new("walk");
    assert_eq!(state.current_sprite(&registry), Some("walk_0"));

    let new = state.advance(150.0, &registry);
    assert_eq!(new, Some("walk_1".into()));
    assert_eq!(state.frame_index, 1);
}

#[test]
fn no_change_within_frame() {
    let mut registry = AnimationRegistry::new();
    registry.insert("walk".into(), test_anim());

    let mut state = AnimationState::new("walk");
    let new = state.advance(50.0, &registry);
    assert_eq!(new, None);
    assert_eq!(state.frame_index, 0);
}

#[test]
fn looping_animation_wraps() {
    let mut registry = AnimationRegistry::new();
    registry.insert("walk".into(), test_anim());

    let mut state = AnimationState::new("walk");
    state.frame_index = 3;
    state.accumulated_ms = 0.0;

    let new = state.advance(150.0, &registry);
    assert_eq!(new, Some("walk_0".into()));
    assert_eq!(state.frame_index, 0);
    assert!(!state.finished);
}

#[test]
fn non_looping_animation_finishes() {
    let mut registry = AnimationRegistry::new();
    let mut anim = test_anim();
    anim.looping = false;
    registry.insert("once".into(), anim);

    let mut state = AnimationState::new("once");
    // Advance through all frames
    state.advance(100.0, &registry); // -> frame 1
    state.advance(100.0, &registry); // -> frame 2
    state.advance(100.0, &registry); // -> frame 3
    let new = state.advance(100.0, &registry); // should finish
    assert_eq!(state.frame_index, 3);
    assert!(state.finished);
    assert_eq!(new, None); // already finished, no change
}

#[test]
fn skip_multiple_frames() {
    let mut registry = AnimationRegistry::new();
    registry.insert("walk".into(), test_anim());

    let mut state = AnimationState::new("walk");
    let new = state.advance(250.0, &registry);
    assert_eq!(new, Some("walk_2".into()));
    assert_eq!(state.frame_index, 2);
}

#[test]
fn zero_duration_does_not_infinite_loop() {
    let mut registry = AnimationRegistry::new();
    registry.insert(
        "zeros".into(),
        AnimationData {
            looping: true,
            frames: vec![
                AnimationFrame {
                    sprite: "a".into(),
                    duration_ms: 0,
                },
                AnimationFrame {
                    sprite: "b".into(),
                    duration_ms: 0,
                },
            ],
        },
    );

    let mut state = AnimationState::new("zeros");
    // Must terminate rather than spin forever.
    let _ = state.advance(100.0, &registry);
}
