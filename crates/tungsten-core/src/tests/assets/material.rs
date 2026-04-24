use super::*;
use std::path::PathBuf;

fn defaults() -> MaterialUniformDefaults {
    MaterialUniformDefaults::default()
}

#[test]
fn allocate_returns_stable_id_for_same_name() {
    let mut reg = MaterialRegistry::new();
    let a = reg.allocate(
        "damage_flash",
        PathBuf::from("manifest.json"),
        "damage_flash".to_string(),
        defaults(),
    );
    let b = reg.allocate(
        "damage_flash",
        PathBuf::from("manifest.json"),
        "damage_flash".to_string(),
        defaults(),
    );
    assert_eq!(a, b);
}

#[test]
fn allocate_mints_distinct_ids_for_distinct_names() {
    let mut reg = MaterialRegistry::new();
    let a = reg.allocate(
        "flash",
        PathBuf::from("a.json"),
        "flash".to_string(),
        defaults(),
    );
    let b = reg.allocate(
        "vignette",
        PathBuf::from("b.json"),
        "vignette".to_string(),
        defaults(),
    );
    assert_ne!(a, b);
}

#[test]
fn path_reverse_lookup_round_trips() {
    let mut reg = MaterialRegistry::new();
    let path = PathBuf::from("manifest.json");
    let id = reg.allocate(
        "flash",
        path.clone(),
        "flash_shader".to_string(),
        defaults(),
    );
    assert_eq!(reg.id_for_path(&path), Some(id));
    assert_eq!(reg.get("flash"), Some(id));
    assert_eq!(reg.name_for_id(id), Some("flash"));
    assert_eq!(reg.shader_name_for_id(id), Some("flash_shader"));
    assert_eq!(reg.path_for_id(id), Some(path.as_path()));
}

#[test]
fn allocate_rebinds_path_without_dropping_id() {
    let mut reg = MaterialRegistry::new();
    let old_path = PathBuf::from("old.json");
    let new_path = PathBuf::from("new.json");
    let id = reg.allocate("flash", old_path.clone(), "flash".to_string(), defaults());
    let id2 = reg.allocate("flash", new_path.clone(), "flash".to_string(), defaults());
    assert_eq!(id, id2);
    assert_eq!(reg.id_for_path(&new_path), Some(id));
    assert_eq!(reg.id_for_path(&old_path), None);
}

#[test]
fn unknown_name_and_path_return_none() {
    let reg = MaterialRegistry::new();
    assert!(reg.get("flash").is_none());
    assert!(reg.id_for_path(&PathBuf::from("nothing")).is_none());
}

#[test]
fn material_uniform_defaults_round_trip_through_json() {
    let mut d = MaterialUniformDefaults::default();
    d.vec4[0] = [1.0, 0.5, 0.25, 1.0];
    d.f32s[1] = 0.75;
    d.i32s[2] = 7;
    let json = serde_json::to_string(&d).expect("serialize");
    let back: MaterialUniformDefaults = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(d.vec4, back.vec4);
    assert_eq!(d.f32s, back.f32s);
    assert_eq!(d.i32s, back.i32s);
}

#[test]
fn material_uniform_defaults_project_onto_override_block() {
    let mut d = MaterialUniformDefaults::default();
    d.vec4[2] = [1.0, 2.0, 3.0, 4.0];
    d.f32s[0] = 0.5;
    d.i32s[3] = -1;
    let block = d.to_override_block();
    assert_eq!(block.vec4[2], [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(block.f32s[0], 0.5);
    assert_eq!(block.i32s[3], -1);
}
