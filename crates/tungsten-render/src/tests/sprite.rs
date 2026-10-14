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
