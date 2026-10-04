use crate::lighting::{AmbientLight, LIGHT_CAP};
use glam::Vec3;

#[test]
fn ambient_default_is_one() {
    assert_eq!(AmbientLight::default().0, Vec3::ONE);
}

#[test]
fn light_cap_is_sixteen() {
    assert_eq!(LIGHT_CAP, 16);
    let wgsl = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/shaders/lit_sprite.wgsl"
    ));
    let len: usize = wgsl
        .split_once("array<GpuLight,")
        .and_then(|(_, rest)| rest.split_once('>'))
        .expect("lit_sprite.wgsl declares array<GpuLight, N>")
        .0
        .trim()
        .parse()
        .expect("array<GpuLight, N> has a literal N");
    assert_eq!(len, LIGHT_CAP);
}
