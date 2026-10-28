use super::*;

#[test]
fn an_unknown_mesh_name_is_warned_about_once() {
    let mut world = World::new();
    assert!(first_unknown_mesh(&mut world, "tri"));
    assert!(!first_unknown_mesh(&mut world, "tri"));
    assert!(first_unknown_mesh(&mut world, "quad"));
}
