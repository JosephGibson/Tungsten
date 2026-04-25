use crate::lighting::{AmbientLight, LIGHT_CAP};
use glam::Vec3;

#[test]
fn ambient_default_is_one() {
    assert_eq!(AmbientLight::default().0, Vec3::ONE);
}

#[test]
fn light_cap_is_sixteen() {
    assert_eq!(LIGHT_CAP, 16);
}
