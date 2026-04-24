use super::*;
use std::path::PathBuf;

#[test]
fn allocate_returns_stable_id_for_same_name() {
    let mut reg = ShaderRegistry::new();
    let a = reg.allocate("sprite", PathBuf::from("assets/shaders/sprite.wgsl"));
    let b = reg.allocate("sprite", PathBuf::from("assets/shaders/sprite.wgsl"));
    assert_eq!(a, b);
}

#[test]
fn allocate_mints_distinct_ids_for_distinct_names() {
    let mut reg = ShaderRegistry::new();
    let a = reg.allocate("sprite", PathBuf::from("a.wgsl"));
    let b = reg.allocate("quad", PathBuf::from("b.wgsl"));
    assert_ne!(a, b);
}

#[test]
fn path_reverse_lookup_round_trips() {
    let mut reg = ShaderRegistry::new();
    let path = PathBuf::from("assets/shaders/sprite.wgsl");
    let id = reg.allocate("sprite", path.clone());
    assert_eq!(reg.id_for_path(&path), Some(id));
    assert_eq!(reg.get("sprite"), Some(id));
    assert_eq!(reg.name_for_id(id), Some("sprite"));
    assert_eq!(reg.path_for_id(id), Some(path.as_path()));
}

#[test]
fn allocate_rebinds_path_without_dropping_id() {
    let mut reg = ShaderRegistry::new();
    let old_path = PathBuf::from("old.wgsl");
    let new_path = PathBuf::from("new.wgsl");
    let id = reg.allocate("sprite", old_path.clone());
    let id2 = reg.allocate("sprite", new_path.clone());
    assert_eq!(id, id2);
    assert_eq!(reg.id_for_path(&new_path), Some(id));
    assert_eq!(reg.id_for_path(&old_path), None);
}

#[test]
fn unknown_name_and_path_return_none() {
    let reg = ShaderRegistry::new();
    assert!(reg.get("sprite").is_none());
    assert!(reg.id_for_path(&PathBuf::from("nothing")).is_none());
}
