use tungsten_core::assets::{MaterialAssetId, MaterialUniformDefaults};
use tungsten_core::tween::{IntSlot, ScalarSlot, UniformOverrideBlock, Vec4Slot};

use super::{PendingMaterial, PendingMaterials};

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

#[test]
fn pending_materials_wait_on_their_shader_until_built() {
    // A material whose first upload failed was in no map a shader reload
    // read, so fixing the shader never built it.
    let pending_on = |name: &str, shader: &str| PendingMaterial {
        name: name.to_string(),
        shader_name: shader.to_string(),
        defaults: MaterialUniformDefaults::default(),
    };
    let mut pending = PendingMaterials::default();
    pending.wait(MaterialAssetId(0), pending_on("flash", "flash_shader"));
    pending.wait(MaterialAssetId(1), pending_on("glow", "glow_shader"));
    pending.wait(MaterialAssetId(2), pending_on("burn", "flash_shader"));
    // A second failure replaces the record.
    pending.wait(MaterialAssetId(0), pending_on("flash", "flash_shader"));
    let ids = |list: Vec<(MaterialAssetId, PendingMaterial)>| -> Vec<u32> {
        list.into_iter().map(|(id, _)| id.0).collect()
    };
    assert_eq!(ids(pending.on_shader("flash_shader")), [2, 0]);
    assert_eq!(ids(pending.on_shader("glow_shader")), [1]);
    pending.built(MaterialAssetId(2));
    assert_eq!(ids(pending.on_shader("flash_shader")), [0]);
    assert!(pending.on_shader("other").is_empty());
}
