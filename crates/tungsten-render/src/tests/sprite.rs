use super::*;

#[test]
fn sprite_instance_stride_matches_attribute_layout() {
    assert_eq!(std::mem::size_of::<SpriteInstance>(), 40);
}

#[test]
fn whole_instance_fills_full_uv() {
    let inst = SpriteInstance::whole([0.0, 0.0], [16.0, 16.0], 0.0, [255; 4]);
    assert_eq!(inst.uv_min, [0.0, 0.0]);
    assert_eq!(inst.uv_size, [1.0, 1.0]);
}
