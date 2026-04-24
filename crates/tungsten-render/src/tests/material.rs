use tungsten_core::assets::MaterialUniformDefaults;
use tungsten_core::tween::{IntSlot, ScalarSlot, UniformOverrideBlock, Vec4Slot};

#[test]
fn override_block_packs_into_first_96_bytes_in_slot_order() {
    let mut block = UniformOverrideBlock::default();
    block.vec4[Vec4Slot::V0.index()] = [1.0, 2.0, 3.0, 4.0];
    block.f32s[ScalarSlot::F1.index()] = 0.5;
    block.i32s[IntSlot::I2.index()] = 7;

    let bytes = block.to_bytes();

    // vec4 region: 0..64. V0 occupies 0..16.
    assert_eq!(
        &bytes[0..16],
        bytemuck::cast_slice::<f32, u8>(&[1.0_f32, 2.0, 3.0, 4.0])
    );
    // f32s start at 64; F1 at 64+4.
    assert_eq!(&bytes[64 + 4..64 + 8], &0.5_f32.to_le_bytes());
    // i32s start at 80; I2 at 80+8.
    assert_eq!(&bytes[80 + 8..80 + 12], &7_i32.to_le_bytes());
    // Reserved tail is zero.
    assert!(bytes[96..].iter().all(|&b| b == 0));
}

#[test]
fn defaults_projection_preserves_payload() {
    let mut d = MaterialUniformDefaults::default();
    d.vec4[3] = [9.0, 8.0, 7.0, 6.0];
    d.f32s[0] = -0.125;
    d.i32s[1] = -3;
    let block = d.to_override_block();
    let bytes = block.to_bytes();
    assert_eq!(
        &bytes[48..64],
        bytemuck::cast_slice::<f32, u8>(&[9.0_f32, 8.0, 7.0, 6.0])
    );
    assert_eq!(&bytes[64..68], &(-0.125_f32).to_le_bytes());
    assert_eq!(&bytes[84..88], &(-3_i32).to_le_bytes());
}
