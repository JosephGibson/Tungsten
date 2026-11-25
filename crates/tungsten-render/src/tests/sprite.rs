use super::*;

#[test]
fn sprite_instance_stride_matches_attribute_layout() {
    // M25: 40-byte base layout + `z_norm: f32` + explicit `_pad: f32`
    // for 16-byte alignment on the GPU side.
    assert_eq!(std::mem::size_of::<SpriteInstance>(), 48);
}

#[test]
fn whole_instance_fills_full_uv() {
    let inst = SpriteInstance::whole([0.0, 0.0], [16.0, 16.0], 0.0, [255; 4]);
    assert_eq!(inst.uv_min, [0.0, 0.0]);
    assert_eq!(inst.uv_size, [1.0, 1.0]);
}

#[test]
fn instance_capacity_stays_within_one_buffer() {
    // 4,194,305 sprites rounded up to 2^23 instances of 48 bytes is 384 MiB,
    // past wgpu's default 256 MiB buffer limit: `create_buffer` panicked.
    const MAX: u64 = 256 << 20;
    let stride = std::mem::size_of::<SpriteInstance>();
    assert_eq!(instance_capacity(4_194_305, stride, MAX), 5_592_405);
    assert!(5_592_405 * stride as u64 <= MAX);
    assert_eq!(instance_capacity(100, stride, MAX), 128);
    assert_eq!(instance_capacity(0, stride, MAX), 1);
    assert_eq!(instance_capacity(usize::MAX, stride, MAX), 5_592_405);
    assert_eq!(instance_capacity(10, 48, 100), 2);
    assert_eq!(instance_capacity(10, 48, 10), 1);
}

#[test]
fn a_reserved_handle_never_comes_from_the_counter() {
    let mut handles = TextureHandles::new();
    let counted: Vec<u32> = (0..4).map(|_| handles.allocate().0).collect();
    let reserved: Vec<u32> = (0..3).map(|_| handles.allocate_reserved().0).collect();
    assert_eq!(counted, [0, 1, 2, 3]);
    assert_eq!(reserved, [u32::MAX, u32::MAX - 1, u32::MAX - 2]);
    // Reserved handles leave the counter where it was.
    assert_eq!(handles.allocate().0, 4);
    assert!(
        reserved
            .iter()
            .all(|&h| h >= TextureHandles::RESERVED_FLOOR)
    );

    // The counter stops below the range, and the range stops at its floor.
    let mut full = TextureHandles {
        next: TextureHandles::RESERVED_FLOOR - 1,
        next_reserved: TextureHandles::RESERVED_FLOOR,
    };
    assert_eq!(full.allocate().0, TextureHandles::RESERVED_FLOOR - 1);
    assert_eq!(full.allocate_reserved().0, TextureHandles::RESERVED_FLOOR);
    let mut counter = full.clone();
    let counter = std::panic::catch_unwind(move || counter.allocate());
    assert!(counter.is_err(), "the counter never reaches the range");
    let range = std::panic::catch_unwind(move || full.allocate_reserved());
    assert!(range.is_err(), "the range ends at its floor");
}

/// A batch on `page`, solid or not, lit or not, with one instance.
fn turn_batch(page: u32, solid: bool, lit: bool, material: Option<u32>) -> SpriteBatch {
    let mut batch = SpriteBatch::new(TextureHandle(page), FilterMode::Nearest);
    batch.solid = solid;
    batch.lit = lit;
    batch.material_id = material.map(MaterialAssetId);
    batch.instances.push(SpriteInstance::default());
    batch
}

fn turns(batches: &[SpriteBatch], depth: bool) -> Vec<(u32, bool)> {
    let mut out = vec![Turn {
        batch: 99,
        solid: true,
    }];
    draw_turns(batches, depth, &mut out);
    out.iter().map(|turn| (turn.batch, turn.solid)).collect()
}

#[test]
fn without_the_depth_test_the_draw_keeps_the_lists_order() {
    let batches = [
        turn_batch(0, true, false, None),
        turn_batch(1, false, false, None),
        turn_batch(2, true, true, None),
    ];
    // A solid mark means nothing without the test.
    assert_eq!(turns(&batches, false), [(0, false), (1, false), (2, false)]);
    assert_eq!(turns(&[], true), []);
}

#[test]
fn under_the_depth_test_solid_batches_draw_first_and_the_last_of_them_first() {
    let batches = [
        turn_batch(0, true, false, None),
        turn_batch(1, false, false, None),
        turn_batch(2, true, true, None),
        turn_batch(3, false, true, None),
        turn_batch(4, true, false, None),
    ];
    assert_eq!(
        turns(&batches, true),
        [(4, true), (2, true), (0, true), (1, false), (3, false)]
    );
    // Every batch takes one turn.
    let mut seen: Vec<u32> = turns(&batches, true).iter().map(|turn| turn.0).collect();
    seen.sort_unstable();
    assert_eq!(seen, [0, 1, 2, 3, 4]);
}

#[test]
fn a_material_batch_is_never_solid_and_a_lit_one_with_a_material_may_be() {
    let batches = [
        // Its material's shader draws it: no promise holds.
        turn_batch(0, true, false, Some(7)),
        // The lit shader draws it whatever material it names (`D-061`).
        turn_batch(1, true, true, Some(7)),
    ];
    assert_eq!(turns(&batches, true), [(1, true), (0, false)]);
}

#[test]
fn a_later_batch_sits_nearer_and_every_depth_lies_inside_the_range() {
    for count in [1usize, 2, 21, 4_096, 100_000] {
        let mut last = 1.0f32;
        for index in 0..count {
            let depth = batch_depth(index, count);
            assert!(depth < last, "{index} of {count}: {depth} against {last}");
            assert!(depth > 0.0, "{index} of {count}: {depth}");
            last = depth;
        }
    }
    assert_eq!(batch_depth(0, 1), 0.5);
}

/// A camera showing world x 100..500 and y 200..400 on a 400 x 200 target,
/// y down: a pixel a world unit.
fn clip_camera() -> glam::Mat4 {
    glam::Mat4::from_cols_array(&[
        2.0 / 400.0,
        0.0,
        0.0,
        0.0,
        0.0,
        -2.0 / 200.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        -(500.0 + 100.0) / 400.0,
        (400.0 + 200.0) / 200.0,
        0.0,
        1.0,
    ])
}

/// Whether `rect` holds pixel columns `xs` and rows `ys` and reaches at
/// most two pixels past them: the margin, and one for a corner that
/// projects a hair short of its pixel's edge.
fn holds(rect: [u32; 4], xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> bool {
    let [x, y, w, h] = rect;
    let within = |low: u32, high: u32, range: &std::ops::Range<u32>| {
        low <= range.start && range.start - low <= 2 && high >= range.end && high - range.end <= 2
    };
    within(x, x + w, &xs) && within(y, y + h, &ys)
}

#[test]
fn a_clip_box_scissors_to_the_pixels_it_reaches_and_a_margin() {
    let camera = clip_camera();
    let target = (400, 200);
    let scissor = |clip| match clip_scissor(&camera, clip, target) {
        Scissor::Rect(rect) => rect,
        other => panic!("{clip:?}: {other:?}"),
    };
    // World (150, 250)..(300, 380) is pixels (50, 50)..(200, 180).
    let rect = scissor([150.0, 250.0, 300.0, 380.0]);
    assert!(holds(rect, 50..200, 50..180), "{rect:?}");
    assert!(rect[0] < 50 && rect[1] < 50, "a margin: {rect:?}");
    // A box given corner for corner the other way reaches the same pixels.
    assert_eq!(scissor([300.0, 380.0, 150.0, 250.0]), rect);
    // An edge inside a pixel row keeps that row.
    let rect = scissor([150.0, 250.5, 300.0, 380.0]);
    assert!(rect[1] <= 50, "{rect:?}");
    // Past the target: clamped to it; around it: the whole of it.
    let rect = scissor([-1.0e6, 300.0, 1.0e6, 1.0e6]);
    assert_eq!((rect[0], rect[2]), (0, 400));
    assert!(holds(rect, 0..400, 100..200), "{rect:?}");
    assert_eq!(scissor([-1.0e9, -1.0e9, 1.0e9, 1.0e9]), [0, 0, 400, 200]);
}

#[test]
fn a_clip_box_off_the_target_draws_nothing_and_one_that_is_no_number_clips_nothing() {
    let camera = clip_camera();
    let target = (400, 200);
    // Wholly above the view, and more than the margin away; then beside it.
    assert_eq!(
        clip_scissor(&camera, [100.0, 0.0, 500.0, 190.0], target),
        Scissor::Nothing
    );
    assert_eq!(
        clip_scissor(&camera, [600.0, 200.0, 700.0, 400.0], target),
        Scissor::Nothing
    );
    // An empty box on the target still spans its margin: the caller's empty
    // box is its own to skip.
    let Scissor::Rect(rect) = clip_scissor(&camera, [200.0, 300.0, 200.0, 300.0], target) else {
        panic!("on the target");
    };
    assert!(holds(rect, 100..100, 100..100), "{rect:?}");
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            clip_scissor(&camera, [100.0, bad, 500.0, 400.0], target),
            Scissor::Whole,
            "{bad}"
        );
    }
}

#[test]
fn a_turned_camera_keeps_every_pixel_the_box_reaches() {
    // A quarter turn about the view's centre: the box's corners still bound
    // it, so its scissor holds each corner's pixel.
    let centre = glam::Vec3::new(300.0, 300.0, 0.0);
    let turned = clip_camera()
        * glam::Mat4::from_translation(centre)
        * glam::Mat4::from_rotation_z(0.5)
        * glam::Mat4::from_translation(-centre);
    let clip = [250.0, 280.0, 350.0, 320.0];
    let Scissor::Rect([x, y, w, h]) = clip_scissor(&turned, clip, (400, 200)) else {
        panic!("on the target");
    };
    for corner in [
        (250.0, 280.0),
        (350.0, 280.0),
        (250.0, 320.0),
        (350.0, 320.0),
    ] {
        let ndc = turned.project_point3(glam::Vec3::new(corner.0, corner.1, 0.0));
        let (px, py) = ((ndc.x * 0.5 + 0.5) * 400.0, (0.5 - ndc.y * 0.5) * 200.0);
        assert!(px >= x as f32 && px <= (x + w) as f32, "{corner:?}: x {px}");
        assert!(py >= y as f32 && py <= (y + h) as f32, "{corner:?}: y {py}");
    }
}
