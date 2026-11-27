use super::*;
use std::path::PathBuf;

#[test]
fn register_keeps_the_given_id_name_and_path() {
    // The renderer seeds its engine shaders first, so a manifest shader's ID
    // need not follow manifest order; the registry mints none of its own.
    for (case, name, id, path) in [
        (
            "the renderer's ID",
            "glow",
            ShaderAssetId(23),
            "assets/shaders/glow.wgsl",
        ),
        (
            "ID zero",
            "sprite",
            ShaderAssetId(0),
            "assets/shaders/sprite.wgsl",
        ),
    ] {
        let mut reg = ShaderRegistry::new();
        let path = PathBuf::from(path);
        reg.register(name, id, path.clone());
        assert_eq!(reg.get(name), Some(id), "{case}: get");
        assert_eq!(reg.id_for_path(&path), Some(id), "{case}: id_for_path");
        assert_eq!(reg.name_for_id(id), Some(name), "{case}: name_for_id");
        assert_eq!(
            reg.path_for_id(id),
            Some(path.as_path()),
            "{case}: path_for_id"
        );
    }
}

#[test]
fn register_rebinds_path_without_dropping_id() {
    let mut reg = ShaderRegistry::new();
    let old_path = PathBuf::from("old.wgsl");
    let new_path = PathBuf::from("new.wgsl");
    let id = ShaderAssetId(4);
    reg.register("sprite", id, old_path.clone());
    reg.register("sprite", id, new_path.clone());
    assert_eq!(reg.get("sprite"), Some(id));
    assert_eq!(reg.id_for_path(&new_path), Some(id));
    assert_eq!(reg.id_for_path(&old_path), None);
}

#[test]
fn register_under_a_new_id_drops_the_old_one() {
    let mut reg = ShaderRegistry::new();
    let path = PathBuf::from("glow.wgsl");
    reg.register("glow", ShaderAssetId(3), path.clone());
    reg.register("glow", ShaderAssetId(9), path.clone());
    assert_eq!(reg.get("glow"), Some(ShaderAssetId(9)));
    assert_eq!(reg.id_for_path(&path), Some(ShaderAssetId(9)));
    assert_eq!(reg.name_for_id(ShaderAssetId(3)), None);
    assert_eq!(reg.path_for_id(ShaderAssetId(3)), None);
}

#[test]
fn unknown_name_and_path_return_none() {
    let reg = ShaderRegistry::new();
    assert!(reg.get("sprite").is_none());
    assert!(reg.id_for_path(&PathBuf::from("nothing")).is_none());
}
