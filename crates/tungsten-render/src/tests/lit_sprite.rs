use super::*;
use crate::sprite::SpriteBatch;
use tungsten_core::assets::{FilterMode, TextureHandle};

#[test]
fn lit_sprite_shader_name_constant() {
    assert_eq!(LIT_SPRITE_SHADER_NAME, "lit_sprite");
    assert_eq!(EMISSIVE_MASK_SHADER_NAME, "emissive_mask");
    assert_eq!(RIM_LIGHT_SHADER_NAME, "rim_light");
}

#[test]
fn sprite_batch_default_lit_false() {
    let b = SpriteBatch::new(TextureHandle(0), FilterMode::Nearest);
    assert!(!b.lit);
}

#[test]
fn lit_shader_source_includes_light_ubo_struct() {
    // Sanity: the manifest mirror and compile-time include must match.
    assert!(LIT_SPRITE_SHADER_SOURCE.contains("struct LightUbo"));
    assert!(LIT_SPRITE_SHADER_SOURCE.contains("@group(2) @binding(0)"));
}
