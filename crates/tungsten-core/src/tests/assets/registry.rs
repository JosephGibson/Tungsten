use super::*;

fn register(reg: &mut AssetRegistry, id: &str, filter: FilterMode, w: u32, h: u32, path: &str) {
    reg.register_sprite(
        id.to_string(),
        filter,
        w,
        h,
        PathBuf::from(path),
        TextureHandle(0),
        UvRect::FULL,
        None,
        None,
        None,
    );
}

#[test]
fn register_and_lookup() {
    let mut reg = AssetRegistry::new();
    register(
        &mut reg,
        "player_idle",
        FilterMode::Nearest,
        32,
        32,
        "dummy.png",
    );
    let sprite = reg.get_sprite("player_idle").unwrap();
    assert_eq!(sprite.atlas, TextureHandle(0));
    assert_eq!(sprite.uv, UvRect::FULL);
    assert_eq!(sprite.width, 32);
}

#[test]
fn register_stores_filter_and_path() {
    let mut reg = AssetRegistry::new();
    register(&mut reg, "a", FilterMode::Nearest, 16, 16, "a.png");
    register(&mut reg, "b", FilterMode::Linear, 32, 32, "b.png");
    let a = reg.get_sprite("a").unwrap();
    let b = reg.get_sprite("b").unwrap();
    assert_eq!(a.filter, FilterMode::Nearest);
    assert_eq!(b.filter, FilterMode::Linear);
    assert_eq!(a.path, PathBuf::from("a.png"));
}

#[test]
#[should_panic(expected = "duplicate sprite ID")]
fn duplicate_sprite_id_panics() {
    let mut reg = AssetRegistry::new();
    register(&mut reg, "same", FilterMode::Nearest, 16, 16, "same.png");
    register(&mut reg, "same", FilterMode::Nearest, 16, 16, "same2.png");
}

#[test]
fn sprite_id_for_path_reverse_lookup() {
    let mut reg = AssetRegistry::new();
    let path = "/assets/sprites/foo.png";
    register(&mut reg, "foo", FilterMode::Nearest, 32, 32, path);
    assert_eq!(reg.sprite_id_for_path(Path::new(path)), Some("foo"));
    assert_eq!(reg.sprite_id_for_path(Path::new("/other.png")), None);
}

#[test]
fn update_sprite_entry_changes_stored_size() {
    let mut reg = AssetRegistry::new();
    register(&mut reg, "bar", FilterMode::Nearest, 16, 16, "bar.png");
    let new_uv = UvRect {
        min: [0.25, 0.25],
        max: [0.75, 0.75],
    };
    reg.update_sprite_entry("bar", TextureHandle(7), new_uv, 32, 64);
    let asset = reg.get_sprite("bar").unwrap();
    assert_eq!(asset.atlas, TextureHandle(7));
    assert_eq!(asset.uv, new_uv);
    assert_eq!(asset.width, 32);
    assert_eq!(asset.height, 64);
}
