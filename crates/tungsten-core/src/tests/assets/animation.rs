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
                sound: None,
            },
            AnimationFrame {
                sprite: walk(1),
                duration_ms: 100,
                sound: None,
            },
            AnimationFrame {
                sprite: walk(2),
                duration_ms: 100,
                sound: None,
            },
            AnimationFrame {
                sprite: walk(3),
                duration_ms: 100,
                sound: None,
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
                    sound: None,
                },
                AnimationFrame {
                    sprite: SpriteAssetId::new(1),
                    duration_ms: 0,
                    sound: None,
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

#[test]
fn total_duration_saturates_instead_of_overflowing() {
    // The loader logs every animation's total; two frames of 3e9 ms
    // overflowed `u32` and panicked under debug overflow checks.
    let long = |sprite| AnimationFrame {
        sprite,
        duration_ms: 3_000_000_000,
        sound: None,
    };
    let data = AnimationData {
        looping: false,
        frames: vec![long(walk(0)), long(walk(1))],
    };
    assert_eq!(data.total_duration_ms(), u32::MAX);
    assert_eq!(test_anim().total_duration_ms(), 400);
}

#[test]
fn a_frames_sound_reads_from_the_file() {
    let dir = std::env::temp_dir().join(format!("tungsten_anim_sound_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("walk.json");
    std::fs::write(
        &path,
        r#"{"looping": true, "frames": [
            {"sprite": "walk_0", "duration_ms": 100},
            {"sprite": "walk_1", "duration_ms": 100, "sound": "step"}
        ]}"#,
    )
    .unwrap();
    let mut sprites = AssetRegistry::new();
    let anim = AnimationData::load(&path, &mut sprites).unwrap();
    let sounds: Vec<Option<&str>> = anim.frames.iter().map(|f| f.sound.as_deref()).collect();
    assert_eq!(sounds, [None, Some("step")]);
    let _ = std::fs::remove_dir_all(&dir);

    // Playback enters the sounded frame, and reports it once.
    let mut registry = AnimationRegistry::new();
    registry.insert("walk".into(), anim);
    let mut state = AnimationState::new("walk");
    assert_eq!(state.advance_entering(50.0, &registry).sound_frame, None);
    assert_eq!(state.advance_entering(60.0, &registry).sound_frame, Some(1));
    assert_eq!(state.advance_entering(10.0, &registry).sound_frame, None);
    // Speed and the end's despawn share two bytes; the component stays 40.
    state.set_speed(0.5);
    state.set_despawn_at_end(true);
    assert_eq!((state.speed(), state.despawn_at_end()), (0.5, true));
    state.set_speed(f32::NAN);
    assert_eq!(state.speed(), 1.0, "a speed that is not finite plays at 1");
    state.set_speed(-2.0);
    assert_eq!(state.speed(), 1.0);
    state.set_speed(100.0);
    assert_eq!(state.speed(), 32.767);
    assert!(state.despawn_at_end(), "the speed leaves the despawn bit");
    assert_eq!(std::mem::size_of::<AnimationState>(), 40);
}

#[test]
fn advancing_in_a_clip_is_advancing_through_the_registry() {
    // `advance_in` is `advance_entering` with the clip already found: the
    // same state and the same result, step for step, a paused state and a
    // finished one-shot included.
    let mut registry = AnimationRegistry::new();
    registry.insert("walk".to_string(), test_anim());
    let mut once = test_anim();
    once.looping = false;
    once.frames[2].sound = Some("step".to_string());
    registry.insert("once".to_string(), once);
    for id in ["walk", "once"] {
        let clip = registry.get(id).unwrap();
        let mut through = AnimationState::new(id).with_speed(1.5);
        let mut within = through.clone();
        for step in 0..40 {
            if step == 12 {
                through.playing = false;
                within.playing = false;
            }
            if step == 15 {
                through.playing = true;
                within.playing = true;
            }
            let dt_ms = 7.0 + 13.0 * (step % 5) as f32;
            assert_eq!(
                within.advance_in(dt_ms, clip),
                through.advance_entering(dt_ms, &registry),
                "{id}, step {step}"
            );
            assert_eq!(
                (within.frame_index, within.accumulated_ms, within.finished),
                (
                    through.frame_index,
                    through.accumulated_ms,
                    through.finished
                ),
                "{id}, step {step}"
            );
        }
    }
    // A clip without frames advances nothing.
    let empty = AnimationData {
        looping: true,
        frames: Vec::new(),
    };
    let mut state = AnimationState::new("empty");
    assert_eq!(state.advance_in(500.0, &empty), Advanced::default());
    assert_eq!((state.frame_index, state.accumulated_ms), (0, 0.0));
}

/// A seeded stream for the slot tests.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        crate::rng::splitmix64(self.0)
    }

    /// Uniform below `n`.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Names the slot tests register, reassign and rewrite: lengths under, at
/// and over the compare's eight-byte words, at least two of each.
const NAMES: &[&str] = &[
    "",
    "a",
    "b",
    "run",
    "hop",
    "idle",
    "walk",
    "walk_a",
    "walk_b",
    "walk_c",
    "walk_ab1",
    "walk_ab2",
    "walk_abc1",
    "walk_abc2",
    "jump_start_00",
    "jump_start_01",
    "sixteen_bytes_01",
    "sixteen_bytes_02",
    "a_troll_walking_left_under_a_long_name_01",
    "a_troll_walking_left_under_a_long_name_02",
];

/// Names no registry ever holds, each the length of one that does.
const UNREGISTERED: &[&str] = &["x", "ghost", "walk_z", "walk_ab9", "sixteen_bytes_99"];

/// A clip of up to six frames, none at times, every frame a sprite of its
/// own, so two clips never answer alike.
fn seeded_clip(rng: &mut Seeded, sprites: &mut u32) -> AnimationData {
    let frames = (0..rng.below(7))
        .map(|_| {
            *sprites += 1;
            AnimationFrame {
                sprite: SpriteAssetId::new(*sprites),
                duration_ms: [0, 1, 40, 90, 130][rng.below(5)],
                sound: (rng.below(4) == 0).then(|| "step".to_string()),
            }
        })
        .collect();
    AnimationData {
        looping: rng.below(3) != 0,
        frames,
    }
}

/// `advance_entering` as it stood before a state kept its clip's slot: the
/// clip looked up by name at every call.
fn advance_by_name(
    state: &mut AnimationState,
    dt_ms: f32,
    registry: &AnimationRegistry,
) -> Advanced {
    if !state.playing || state.finished {
        return Advanced::default();
    }
    match registry.get(&state.animation_id) {
        Some(clip) => state.advance_in(dt_ms, clip),
        None => Advanced::default(),
    }
}

/// `current_sprite` as it stood.
fn sprite_by_name(state: &AnimationState, registry: &AnimationRegistry) -> Option<SpriteAssetId> {
    let clip = registry.get(&state.animation_id)?;
    Some(clip.frames.get(state.frame_index as usize)?.sprite)
}

/// Every field of a state but its slot.
fn fields(state: &AnimationState) -> (&str, u32, u32, bool, bool, u16) {
    (
        state.animation_id.as_str(),
        state.frame_index,
        state.accumulated_ms.to_bits(),
        state.playing,
        state.finished,
        state.rate,
    )
}

#[test]
fn a_kept_slot_finds_the_clip_its_name_finds() {
    // Twin states through 2,000 seeded operations a seed, one on the kept
    // slot and one looking its clip up by name at every call, as the code
    // stood: every result and every field stays equal while IDs are
    // reassigned and rewritten in place, states cloned, clips replaced and
    // added, and each state advanced against two registries whose slots
    // hold other IDs.
    let (mut kept, mut stale, mut unset) = (0u32, 0u32, 0u32);
    let mut count = |state: &AnimationState, registry: &AnimationRegistry| match registry
        .clips
        .get(state.slot as usize)
    {
        Some(clip) if clip.id == state.animation_id => kept += 1,
        Some(_) => stale += 1,
        None => unset += 1,
    };
    for seed in 1..=8u64 {
        let mut rng = Seeded(seed);
        let mut sprites = 0;
        let mut registries = [AnimationRegistry::new(), AnimationRegistry::new()];
        for (which, registry) in registries.iter_mut().enumerate() {
            // The second registers in reverse, so a slot holds another ID.
            let mut order = NAMES.to_vec();
            if which == 1 {
                order.reverse();
            }
            for name in order {
                if rng.below(5) < 3 {
                    registry.insert(name.to_string(), seeded_clip(&mut rng, &mut sprites));
                }
            }
        }
        let any_name = |rng: &mut Seeded| {
            let at = rng.below(NAMES.len() + UNREGISTERED.len());
            NAMES.iter().chain(UNREGISTERED).copied().nth(at).unwrap()
        };
        let mut slotted: Vec<AnimationState> = (0..8)
            .map(|_| AnimationState::new(any_name(&mut rng)))
            .collect();
        let mut named = slotted.clone();
        for step in 0..2_000 {
            let at = rng.below(slotted.len());
            let registry = rng.below(2);
            match rng.below(100) {
                0..=44 => {
                    let dt_ms = [0.0, 16.6, 33.3, 100.0, 250.0, 1000.0][rng.below(6)];
                    count(&slotted[at], &registries[registry]);
                    assert_eq!(
                        slotted[at].advance_entering(dt_ms, &registries[registry]),
                        advance_by_name(&mut named[at], dt_ms, &registries[registry]),
                        "seed {seed}, step {step}: an advance"
                    );
                }
                45..=59 => {
                    count(&slotted[at], &registries[registry]);
                    assert_eq!(
                        slotted[at].current_sprite(&registries[registry]),
                        sprite_by_name(&named[at], &registries[registry]),
                        "seed {seed}, step {step}: the current sprite"
                    );
                }
                60..=67 => {
                    // Reassigned, to a registered clip or to none.
                    let name = any_name(&mut rng);
                    slotted[at].animation_id = name.to_string();
                    named[at].animation_id = name.to_string();
                }
                68..=75 => {
                    // Rewritten in place, to another name of its length.
                    let len = slotted[at].animation_id.len();
                    let same: Vec<&str> = NAMES
                        .iter()
                        .chain(UNREGISTERED)
                        .copied()
                        .filter(|name| name.len() == len)
                        .collect();
                    let name = same[rng.below(same.len())];
                    slotted[at].animation_id.replace_range(.., name);
                    named[at].animation_id.replace_range(.., name);
                }
                76..=80 => {
                    let from = rng.below(slotted.len());
                    slotted[at] = slotted[from].clone();
                    named[at] = named[from].clone();
                }
                81..=86 => {
                    // A clip replaced where the registry holds the name,
                    // added where it does not.
                    let name = NAMES[rng.below(NAMES.len())];
                    let clip = seeded_clip(&mut rng, &mut sprites);
                    registries[registry].insert(name.to_string(), clip);
                }
                _ => {
                    // Played again, from a frame that may lie past its
                    // clip's end.
                    let frame = rng.below(8) as u32;
                    let playing = rng.below(4) != 0;
                    let speed = [0.0, 0.5, 1.0, 2.0][rng.below(4)];
                    for state in [&mut slotted[at], &mut named[at]] {
                        state.frame_index = frame;
                        state.finished = false;
                        state.playing = playing;
                        state.set_speed(speed);
                    }
                }
            }
            for (slotted, named) in slotted.iter().zip(&named) {
                assert_eq!(fields(slotted), fields(named), "seed {seed}, step {step}");
            }
        }
    }
    // The run met each case often: a slot that held its clip, one that held
    // another's, and none.
    assert!(
        kept > 2_000 && stale > 500 && unset > 100,
        "lookups on a kept slot {kept}, on another clip's {stale}, on none {unset}"
    );
}

#[test]
fn the_word_compare_is_string_equality() {
    let mut rng = Seeded(2);
    let mut letters = |len: usize, of: usize| -> String {
        (0..len)
            .map(|_| char::from(b'a' + rng.below(of) as u8))
            .collect()
    };
    for len in 0..=40usize {
        for _ in 0..8 {
            let a = letters(len, 26);
            assert!(same_id(&a, &a.clone()), "{a:?} against its copy");
            // One byte apart, at each position.
            for at in 0..len {
                let mut bytes = a.clone().into_bytes();
                bytes[at] = if bytes[at] == b'z' {
                    b'A'
                } else {
                    bytes[at] + 1
                };
                let b = String::from_utf8(bytes).unwrap();
                assert!(!same_id(&a, &b) && !same_id(&b, &a), "{a:?} and {b:?}");
            }
            // One byte longer and one shorter.
            let longer = format!("{a}x");
            assert!(!same_id(&a, &longer) && !same_id(&longer, &a), "{a:?}");
            if let Some(shorter) = a.get(..len.wrapping_sub(1)) {
                assert!(!same_id(&a, shorter) && !same_id(shorter, &a), "{a:?}");
            }
        }
    }
    // Seeded pairs over two letters, where short pairs are often equal.
    let (mut equal, mut unequal) = (0u32, 0u32);
    for round in 0..40_000usize {
        let len = round % 41;
        let a = letters(len, 2);
        let b = if round % 3 == 0 {
            a.clone()
        } else {
            letters(len.min(round % 7), 2)
        };
        assert_eq!(same_id(&a, &b), a == b, "{a:?} and {b:?}");
        if a == b {
            equal += 1;
        } else {
            unequal += 1;
        }
    }
    assert!(
        equal > 10_000 && unequal > 10_000,
        "{equal} equal, {unequal} not"
    );
}

#[test]
fn the_registry_answers_as_its_maps_did() {
    // The registry as it stood: an ID's clip in one map and a path's ID in
    // another. A clip's first sprite tells clips apart.
    let clip = |sprite: u32| AnimationData {
        looping: true,
        frames: vec![AnimationFrame {
            sprite: SpriteAssetId::new(sprite),
            duration_ms: 100,
            sound: None,
        }],
    };
    let tag = |data: &AnimationData| data.frames[0].sprite;
    let mut registry = AnimationRegistry::new();
    let mut clips: HashMap<String, SpriteAssetId> = HashMap::new();
    let mut paths: HashMap<PathBuf, String> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let all_paths: Vec<PathBuf> = (0..6).map(|n| PathBuf::from(format!("{n}.json"))).collect();
    let check = |registry: &AnimationRegistry,
                 clips: &HashMap<String, SpriteAssetId>,
                 paths: &HashMap<PathBuf, String>,
                 order: &[String],
                 when: &str| {
        for name in NAMES.iter().chain(UNREGISTERED) {
            assert_eq!(
                registry.get(name).map(tag),
                clips.get(*name).copied(),
                "{when}: get {name:?}"
            );
        }
        for path in &all_paths {
            assert_eq!(
                registry.id_for_path(path),
                paths.get(path).map(String::as_str),
                "{when}: id_for_path {path:?}"
            );
        }
        // `ids` and `iter` go in the order the IDs first registered.
        assert_eq!(registry.ids().collect::<Vec<_>>(), order, "{when}: ids");
        let listed: Vec<(&str, SpriteAssetId)> =
            registry.iter().map(|(id, data)| (id, tag(data))).collect();
        let expected: Vec<(&str, SpriteAssetId)> =
            order.iter().map(|id| (id.as_str(), clips[id])).collect();
        assert_eq!(listed, expected, "{when}: iter");
    };
    let mut sprite = 0;
    let mut insert = |registry: &mut AnimationRegistry,
                      clips: &mut HashMap<String, SpriteAssetId>,
                      paths: &mut HashMap<PathBuf, String>,
                      order: &mut Vec<String>,
                      id: &str,
                      path: Option<&PathBuf>| {
        sprite += 1;
        match path {
            Some(path) => {
                registry.insert_with_path(id.to_string(), clip(sprite), path.clone());
                paths.insert(path.clone(), id.to_string());
            }
            None => registry.insert(id.to_string(), clip(sprite)),
        }
        if clips
            .insert(id.to_string(), SpriteAssetId::new(sprite))
            .is_none()
        {
            order.push(id.to_string());
        }
    };
    check(&registry, &clips, &paths, &order, "empty");

    // A path given to a second ID, then the first inserted again with it.
    let shared = &all_paths[0];
    for (id, when) in [
        ("walk", "one ID on the path"),
        ("idle", "the path given to a second ID"),
        ("walk", "the first inserted again with it"),
    ] {
        insert(
            &mut registry,
            &mut clips,
            &mut paths,
            &mut order,
            id,
            Some(shared),
        );
        check(&registry, &clips, &paths, &order, when);
    }
    assert_eq!(registry.id_for_path(shared), Some("walk"));
    assert_eq!(registry.ids().collect::<Vec<_>>(), ["walk", "idle"]);
    // A plain insert of a held ID keeps its paths and its place.
    insert(
        &mut registry,
        &mut clips,
        &mut paths,
        &mut order,
        "walk",
        None,
    );
    check(
        &registry,
        &clips,
        &paths,
        &order,
        "a held ID inserted again",
    );

    // Seeded inserts, with paths and without.
    let mut rng = Seeded(3);
    for step in 0..600 {
        let id = NAMES[rng.below(NAMES.len())];
        let path = (rng.below(3) != 0).then(|| &all_paths[rng.below(all_paths.len())]);
        insert(&mut registry, &mut clips, &mut paths, &mut order, id, path);
        check(&registry, &clips, &paths, &order, &format!("step {step}"));
    }
    assert_eq!(order.len(), NAMES.len(), "every name registered");
}
