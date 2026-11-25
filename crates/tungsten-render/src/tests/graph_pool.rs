//! The pool's rules, on textures that are plain numbers.

use std::time::Duration;

use super::*;
use wgpu::TextureFormat;

const COLOR: Spec = Spec {
    size: (64, 32),
    format: TextureFormat::Rgba8UnormSrgb,
    samples: 1,
    mips: 1,
    class: Class::Color,
    twin: None,
};
const WORK: Spec = Spec {
    class: Class::Work,
    ..COLOR
};

/// Binds `slots`, counting the textures made.
fn bind(pool: &mut Pool<u64>, slots: &[Spec], made: &mut u32) -> Vec<u64> {
    pool.bind(slots, |_, id| {
        *made += 1;
        id
    })
}

#[test]
fn a_slot_gets_a_texture_of_its_own() {
    let mut pool = Pool::default();
    let mut made = 0;
    let bound = bind(&mut pool, &[COLOR, COLOR, WORK], &mut made);
    assert_eq!(made, 3);
    assert_eq!(bound.len(), 3);
    assert!(bound[0] != bound[1] && bound[1] != bound[2] && bound[0] != bound[2]);
    for id in &bound {
        assert_eq!(pool.get(*id), id, "each texture is found by its identity");
    }
    assert_eq!(
        pool.held().copied().collect::<Vec<_>>(),
        [COLOR, COLOR, WORK]
    );
}

#[test]
fn the_same_slots_bind_the_same_textures() {
    let mut pool = Pool::default();
    let mut made = 0;
    let first = bind(&mut pool, &[COLOR, WORK, COLOR], &mut made);
    let again = bind(&mut pool, &[COLOR, WORK, COLOR], &mut made);
    assert_eq!(first, again);
    assert_eq!(made, 3, "nothing new was made");

    // A frame of another shape takes what fits and adds the rest; the first
    // shape then finds its own again.
    let other = bind(&mut pool, &[COLOR], &mut made);
    assert_eq!(other, [first[0]]);
    assert_eq!(made, 3);
    let wider = bind(&mut pool, &[COLOR, COLOR, COLOR], &mut made);
    assert_eq!(wider[..2], [first[0], first[2]]);
    assert_eq!(made, 4);
    assert_eq!(bind(&mut pool, &[COLOR, WORK, COLOR], &mut made), first);
}

/// A frame at 60 frames a second, in nanoseconds: 240 of them are the keep
/// time to the nanosecond.
const FRAME_60: Duration = Duration::from_nanos(16_666_667);

#[test]
fn a_texture_unused_for_the_keep_frames_is_freed() {
    assert_eq!(FRAME_60 * 240, KEEP_TIME + Duration::from_nanos(80));
    let mut pool = Pool::default();
    let mut made = 0;
    let both = bind(&mut pool, &[COLOR, WORK], &mut made);
    // Every frame uses the first alone, at 60 frames a second.
    let mut now = Duration::ZERO;
    for frame in 0..KEEP_FRAMES {
        pool.touch(&both[..1]);
        now += FRAME_60;
        assert_eq!(pool.end_frame(now), [] as [u64; 0], "frame {frame}");
    }
    pool.touch(&both[..1]);
    now += FRAME_60;
    assert_eq!(pool.end_frame(now), [both[1]], "the unused one goes");
    assert_eq!(pool.held().copied().collect::<Vec<_>>(), [COLOR]);
    // The used one stays however long.
    for _ in 0..KEEP_FRAMES * 2 {
        pool.touch(&both[..1]);
        now += FRAME_60;
        assert!(pool.end_frame(now).is_empty());
    }
}

#[test]
fn the_keep_rule_waits_for_the_frames_and_for_the_time() {
    let unused = |frame_time: Duration, frames: u64| -> bool {
        let mut pool = Pool::default();
        let mut made = 0;
        let both = bind(&mut pool, &[COLOR, WORK], &mut made);
        let mut freed = Vec::new();
        let mut now = Duration::ZERO;
        for _ in 0..frames {
            pool.touch(&both[..1]);
            now += frame_time;
            freed.extend(pool.end_frame(now));
        }
        freed == [both[1]]
    };
    // Uncapped, a millisecond a frame: 241 frames are a quarter of a
    // second, and the texture waits out its four seconds.
    let fast = Duration::from_millis(1);
    assert!(!unused(fast, KEEP_FRAMES + 1));
    assert!(!unused(fast, 4_000));
    assert!(unused(fast, 4_001));
    // Ten frames a second: four seconds are 40 frames, and the texture
    // waits out its 240.
    let slow = Duration::from_millis(100);
    assert!(!unused(slow, 41));
    assert!(!unused(slow, KEEP_FRAMES));
    assert!(unused(slow, KEEP_FRAMES + 1));
}

#[test]
fn an_identity_is_never_given_again() {
    let mut pool = Pool::default();
    let mut made = 0;
    let first = bind(&mut pool, &[COLOR], &mut made);
    assert_eq!(pool.keep_only(&[]), first);
    let second = bind(&mut pool, &[COLOR], &mut made);
    assert_ne!(first, second);
    assert_eq!(made, 2);
}

#[test]
fn keep_only_frees_every_other_texture() {
    let mut pool = Pool::default();
    let mut made = 0;
    let bound = bind(&mut pool, &[COLOR, COLOR, WORK], &mut made);
    let freed = pool.keep_only(&[bound[1]]);
    assert_eq!(freed, [bound[0], bound[2]]);
    assert_eq!(pool.held().count(), 1);
    assert_eq!(pool.get(bound[1]), &bound[1]);
}
