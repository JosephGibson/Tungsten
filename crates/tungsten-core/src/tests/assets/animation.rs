use super::*;

/// Stand-in sprite ID for frame `walk_<frame>`.
fn walk(frame: u32) -> SpriteAssetId {
    SpriteAssetId::new(frame)
}

fn test_anim() -> AnimationData {
    AnimationData {
        looping: true,
        frames: vec![
            AnimationFrame {
                sprite: walk(0),
                duration_ms: 100,
            },
            AnimationFrame {
                sprite: walk(1),
                duration_ms: 100,
            },
            AnimationFrame {
                sprite: walk(2),
                duration_ms: 100,
            },
            AnimationFrame {
                sprite: walk(3),
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
    assert_eq!(state.current_sprite(&registry), Some(walk(0)));

    let new = state.advance(150.0, &registry);
    assert_eq!(new, Some(walk(1)));
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
    assert_eq!(new, Some(walk(0)));
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
    state.advance(100.0, &registry);
    state.advance(100.0, &registry);
    state.advance(100.0, &registry);
    let new = state.advance(100.0, &registry);
    assert_eq!(state.frame_index, 3);
    assert!(state.finished);
    assert_eq!(new, None);
}

#[test]
fn skip_multiple_frames() {
    let mut registry = AnimationRegistry::new();
    registry.insert("walk".into(), test_anim());

    let mut state = AnimationState::new("walk");
    let new = state.advance(250.0, &registry);
    assert_eq!(new, Some(walk(2)));
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
                    sprite: SpriteAssetId::new(0),
                    duration_ms: 0,
                },
                AnimationFrame {
                    sprite: SpriteAssetId::new(1),
                    duration_ms: 0,
                },
            ],
        },
    );

    let mut state = AnimationState::new("zeros");
    let _ = state.advance(100.0, &registry);
}

#[test]
fn playback_survives_a_shorter_hot_reloaded_clip() {
    for looping in [false, true] {
        let mut registry = AnimationRegistry::new();
        registry.insert("walk".into(), test_anim());
        let mut state = AnimationState::new("walk");
        state.advance(350.0, &registry);
        assert_eq!(state.frame_index, 3);

        let mut replacement = test_anim();
        replacement.looping = looping;
        replacement.frames.truncate(2);
        registry.insert("walk".into(), replacement);
        assert_eq!(state.advance(0.0, &registry), Some(walk(1)));
        state.advance(100.0, &registry);
        assert_eq!(state.finished, !looping);
        assert!(state.current_sprite(&registry).is_some());
    }
}

#[test]
fn load_interns_frame_names_in_the_sprite_registry() {
    let path = std::env::temp_dir().join(format!("tungsten_anim_{}.json", std::process::id()));
    std::fs::write(
        &path,
        r#"{"looping": true, "frames": [
            {"sprite": "walk_1", "duration_ms": 80},
            {"sprite": "walk_0", "duration_ms": 90},
            {"sprite": "walk_1", "duration_ms": 100}
        ]}"#,
    )
    .unwrap();
    let mut sprites = AssetRegistry::new();
    let walk_0 = sprites.intern_sprite("walk_0");
    let anim = AnimationData::load(&path, &mut sprites).unwrap();
    std::fs::remove_file(&path).ok();

    let walk_1 = sprites.sprite_id("walk_1").expect("load interns new names");
    let frames: Vec<_> = anim
        .frames
        .iter()
        .map(|f| (f.sprite, f.duration_ms))
        .collect();
    assert_eq!(frames, [(walk_1, 80), (walk_0, 90), (walk_1, 100)]);
    assert!(anim.looping);
}
