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
fn sprite_name_for_path_reverse_lookup() {
    let mut reg = AssetRegistry::new();
    let path = "/assets/sprites/foo.png";
    register(&mut reg, "foo", FilterMode::Nearest, 32, 32, path);
    assert_eq!(reg.sprite_name_for_path(Path::new(path)), Some("foo"));
    assert_eq!(reg.sprite_name_for_path(Path::new("/other.png")), None);
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

#[test]
fn intern_gives_one_id_per_name_registered_or_not() {
    let mut reg = AssetRegistry::new();
    let a = reg.intern_sprite("a");
    let b = reg.intern_sprite("b");
    assert_ne!(a, b);
    assert_eq!(reg.intern_sprite("a"), a);
    assert!(reg.sprite(a).is_none(), "interned, not registered");
    assert_eq!(reg.sprite_name(a), Some("a"));
    register(&mut reg, "a", FilterMode::Nearest, 16, 16, "a.png");
    assert_eq!(reg.intern_sprite("a"), a);
    assert_eq!(reg.sprite_id("a"), Some(a));
}

#[test]
fn sprite_id_never_interns() {
    let mut reg = AssetRegistry::new();
    assert_eq!(reg.sprite_id("missing"), None);
    assert_eq!(reg.sprite_id("missing"), None);
    let first = reg.intern_sprite("first");
    assert_eq!(first.index(), 0, "the lookups minted no ID");
}

#[test]
fn register_fills_interned_slot_and_returns_its_id() {
    let mut reg = AssetRegistry::new();
    let id = reg.intern_sprite("hero");
    assert!(reg.get_sprite("hero").is_none());
    let registered = reg.register_sprite(
        "hero".to_string(),
        FilterMode::Linear,
        24,
        48,
        PathBuf::from("hero.png"),
        TextureHandle(3),
        UvRect::FULL,
        None,
        None,
        None,
    );
    assert_eq!(registered, id);
    let asset = reg.sprite(id).unwrap();
    assert_eq!(asset.atlas, TextureHandle(3));
    assert_eq!((asset.width, asset.height), (24, 48));
    assert_eq!(
        reg.get_sprite("hero").unwrap().path,
        PathBuf::from("hero.png")
    );
    assert_eq!(reg.sprite_names().collect::<Vec<_>>(), ["hero"]);
}

#[test]
#[should_panic(expected = "duplicate sprite ID")]
fn duplicate_registration_of_interned_name_panics() {
    let mut reg = AssetRegistry::new();
    reg.intern_sprite("same");
    register(&mut reg, "same", FilterMode::Nearest, 16, 16, "same.png");
    register(&mut reg, "same", FilterMode::Nearest, 16, 16, "same2.png");
}

#[test]
fn sprite_names_lists_registered_names_only() {
    let mut reg = AssetRegistry::new();
    reg.intern_sprite("pending");
    register(
        &mut reg,
        "loaded",
        FilterMode::Nearest,
        16,
        16,
        "loaded.png",
    );
    assert_eq!(reg.sprite_names().collect::<Vec<_>>(), ["loaded"]);
}

#[test]
fn intern_sprites_ids_do_not_depend_on_input_order() {
    let names = ["walk_2", "idle", "walk_10", "jump", "idle"];
    let mut forward = AssetRegistry::new();
    forward.intern_sprites(names);
    let mut reversed = AssetRegistry::new();
    reversed.intern_sprites(names.iter().rev().copied());
    for name in names {
        assert_eq!(forward.sprite_id(name), reversed.sprite_id(name), "{name}");
    }
    let ids: Vec<u32> = ["idle", "jump", "walk_10", "walk_2"]
        .iter()
        .map(|n| forward.sprite_id(n).unwrap().index())
        .collect();
    assert_eq!(ids, [0, 1, 2, 3], "sorted order");
}

#[test]
fn intern_sprites_keeps_existing_ids() {
    let mut reg = AssetRegistry::new();
    let z = reg.intern_sprite("z");
    reg.intern_sprites(["b", "z", "a"]);
    assert_eq!(reg.sprite_id("z"), Some(z));
    assert_eq!(reg.sprite_id("a").unwrap().index(), 1);
    assert_eq!(reg.sprite_id("b").unwrap().index(), 2);
}

#[test]
fn sprite_and_name_read_none_past_the_end() {
    let mut reg = AssetRegistry::new();
    reg.intern_sprite("only");
    let past = SpriteAssetId::new(1);
    assert!(reg.sprite(past).is_none());
    assert_eq!(reg.sprite_name(past), None);
    assert!(reg.sprite(SpriteAssetId::new(u32::MAX)).is_none());
}

#[test]
fn path_lookup_survives_interning_before_registration() {
    let mut reg = AssetRegistry::new();
    reg.intern_sprite("other");
    register(&mut reg, "foo", FilterMode::Nearest, 32, 32, "foo.png");
    assert_eq!(reg.sprite_name_for_path(Path::new("foo.png")), Some("foo"));
}
