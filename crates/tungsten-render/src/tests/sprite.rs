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
