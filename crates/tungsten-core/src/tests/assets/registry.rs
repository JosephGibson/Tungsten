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

#[test]
fn a_sprites_alpha_measures_as_opaque_cutout_or_blend() {
    let image =
        |alphas: &[u8]| -> Vec<u8> { alphas.iter().flat_map(|&a| [10, 20, 30, a]).collect() };
    let measure = |alphas: &[u8]| SpriteAlpha::measure(&image(alphas));
    assert_eq!(measure(&[255, 255, 255]), SpriteAlpha::Opaque);
    assert_eq!(measure(&[255, 0, 255]), SpriteAlpha::Cutout);
    // A wholly clear image holds no texel between the two levels.
    assert_eq!(measure(&[0, 0]), SpriteAlpha::Cutout);
    // One texel between them is enough, wherever it sits.
    for between in 1..255u8 {
        assert_eq!(measure(&[255, between, 0]), SpriteAlpha::Blend, "{between}");
        assert_eq!(measure(&[between, 255]), SpriteAlpha::Blend, "{between}");
        assert_eq!(measure(&[0, 255, between]), SpriteAlpha::Blend, "{between}");
    }
    // Colour never counts, and no pixels measure as unmeasured.
    assert_eq!(
        SpriteAlpha::measure(&[0, 0, 0, 255, 255, 255, 255, 255]),
        SpriteAlpha::Opaque
    );
    assert_eq!(SpriteAlpha::measure(&[]), SpriteAlpha::Blend);
    assert_eq!(SpriteAlpha::default(), SpriteAlpha::Blend);
}

#[test]
fn only_a_two_level_sprite_under_a_nearest_sampler_is_solid() {
    for alpha in [SpriteAlpha::Opaque, SpriteAlpha::Cutout] {
        assert!(alpha.solid(FilterMode::Nearest), "{alpha:?}");
        // A linear sampler mixes a cutout's edge, and the rule is one rule.
        assert!(!alpha.solid(FilterMode::Linear), "{alpha:?}");
    }
    assert!(!SpriteAlpha::Blend.solid(FilterMode::Nearest));
    assert!(!SpriteAlpha::Blend.solid(FilterMode::Linear));
}

#[test]
fn a_sprite_is_blend_until_its_alpha_is_set_and_a_change_moves_the_epoch() {
    let mut reg = AssetRegistry::new();
    register(&mut reg, "hill", FilterMode::Nearest, 16, 16, "hill.png");
    assert_eq!(reg.get_sprite("hill").unwrap().alpha, SpriteAlpha::Blend);
    let epoch = reg.sprite_epoch();
    reg.set_sprite_alpha("hill", SpriteAlpha::Cutout);
    assert_eq!(reg.get_sprite("hill").unwrap().alpha, SpriteAlpha::Cutout);
    assert_eq!(reg.sprite_epoch(), epoch + 1);
    // The class it holds, an interned name and an unknown one change nothing.
    reg.set_sprite_alpha("hill", SpriteAlpha::Cutout);
    reg.intern_sprite("later");
    reg.set_sprite_alpha("later", SpriteAlpha::Opaque);
    reg.set_sprite_alpha("missing", SpriteAlpha::Opaque);
    assert_eq!(reg.sprite_epoch(), epoch + 1);
    reg.set_sprite_alpha("hill", SpriteAlpha::Opaque);
    assert_eq!(reg.sprite_epoch(), epoch + 2);
    // New pixels under the entry: what the old ones measured as is gone, so
    // a caller that replaces an entry and measures nothing marks nothing.
    reg.update_sprite_entry("hill", TextureHandle(0), UvRect::FULL, 16, 16);
    assert_eq!(reg.get_sprite("hill").unwrap().alpha, SpriteAlpha::Blend);
    assert!(
        !reg.get_sprite("hill")
            .unwrap()
            .alpha
            .solid(FilterMode::Nearest)
    );
}

/// A `width` x `height` RGBA image whose texels in `drawn` (`[x, y, w, h]`)
/// hold alpha 255 and the rest 0.
fn boxed_image(width: u32, height: u32, drawn: [u32; 4]) -> Vec<u8> {
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let inside = (drawn[0]..drawn[0] + drawn[2]).contains(&x)
                && (drawn[1]..drawn[1] + drawn[3]).contains(&y);
            rgba.extend([1, 2, 3, if inside { 255 } else { 0 }]);
        }
    }
    rgba
}

#[test]
fn the_drawn_box_bounds_the_texels_that_are_not_clear() {
    assert_eq!(drawn_box(&boxed_image(8, 6, [2, 1, 4, 3]), 8), [2, 1, 4, 3]);
    assert_eq!(drawn_box(&boxed_image(8, 6, [0, 0, 8, 6]), 8), [0, 0, 8, 6]);
    assert_eq!(drawn_box(&boxed_image(8, 6, [7, 5, 1, 1]), 8), [7, 5, 1, 1]);
    // Two texels in opposite corners span the image; a faint one counts.
    let mut corners = boxed_image(8, 6, [0, 0, 1, 1]);
    corners[(5 * 8 + 7) * 4 + 3] = 1;
    assert_eq!(drawn_box(&corners, 8), [0, 0, 8, 6]);
    // Nothing drawn, and no pixels.
    assert_eq!(drawn_box(&boxed_image(8, 6, [0, 0, 0, 0]), 8), [0; 4]);
    assert_eq!(drawn_box(&[], 8), [0; 4]);
}

#[test]
fn the_drawn_span_covers_every_texel_of_the_box_under_the_registrys_uvs() {
    let mut reg = AssetRegistry::new();
    register(&mut reg, "strip", FilterMode::Nearest, 8, 256, "strip.png");
    // Unmeasured: the whole quad.
    assert_eq!(
        reg.get_sprite("strip").unwrap().drawn_span(),
        ([0.0, 0.0], [1.0, 1.0])
    );
    // Rows 57..256 drawn. The UVs run from the first texel's centre to the
    // last's, so texel row `r` covers the quad from (r - 0.5) / 255.
    reg.set_sprite_drawn("strip", [0, 57, 8, 199]);
    let (low, high) = reg.get_sprite("strip").unwrap().drawn_span();
    assert_eq!((low[0], high[0]), (0.0, 1.0));
    assert_eq!(low[1], 56.5 / 255.0);
    assert_eq!(high[1], 1.0);
    // Every texel of the box, sampled where a quad samples it, lies inside:
    // quad fraction t samples texel floor(0.5 + t * 255).
    for row in 57..256u32 {
        for t in [(row as f32 - 0.5) / 255.0, (row as f32 + 0.499) / 255.0] {
            let t = t.clamp(0.0, 1.0);
            assert!(t >= low[1] && t <= high[1], "row {row} at {t}");
        }
    }
    // A band in the middle, and a linear sampler's extra texel each way.
    reg.set_sprite_drawn("strip", [2, 100, 3, 50]);
    let (low, high) = reg.get_sprite("strip").unwrap().drawn_span();
    assert_eq!((low[1], high[1]), (99.5 / 255.0, 149.5 / 255.0));
    assert_eq!((low[0], high[0]), (1.5 / 7.0, 4.5 / 7.0));
    register(&mut reg, "soft", FilterMode::Linear, 8, 256, "soft.png");
    reg.set_sprite_drawn("soft", [2, 100, 3, 50]);
    let (low, high) = reg.get_sprite("soft").unwrap().drawn_span();
    assert_eq!((low[1], high[1]), (98.5 / 255.0, 150.5 / 255.0));
    // Nothing drawn: an empty part. One texel across: the whole quad.
    reg.set_sprite_drawn("strip", [0; 4]);
    let (low, high) = reg.get_sprite("strip").unwrap().drawn_span();
    assert_eq!((low, high), ([0.0, 0.0], [0.0, 0.0]));
    register(&mut reg, "line", FilterMode::Nearest, 1, 1, "line.png");
    assert_eq!(
        reg.get_sprite("line").unwrap().drawn_span(),
        ([0.0, 0.0], [1.0, 1.0])
    );
}

#[test]
fn a_sprites_drawn_box_is_its_whole_until_set_and_again_after_new_pixels() {
    let mut reg = AssetRegistry::new();
    register(&mut reg, "hill", FilterMode::Nearest, 16, 12, "hill.png");
    assert_eq!(reg.get_sprite("hill").unwrap().drawn, [0, 0, 16, 12]);
    let epoch = reg.sprite_epoch();
    reg.set_sprite_drawn("hill", [1, 2, 3, 4]);
    reg.set_sprite_drawn("hill", [1, 2, 3, 4]);
    reg.set_sprite_drawn("missing", [1, 2, 3, 4]);
    assert_eq!(reg.sprite_epoch(), epoch + 1, "a change, once");
    // New pixels of another size: unmeasured again.
    reg.update_sprite_entry("hill", TextureHandle(0), UvRect::FULL, 20, 10);
    assert_eq!(reg.get_sprite("hill").unwrap().drawn, [0, 0, 20, 10]);
}

#[test]
fn the_sprite_epoch_rises_on_every_change_and_never_on_interning() {
    let mut reg = AssetRegistry::new();
    assert_eq!(reg.sprite_epoch(), 0);
    reg.intern_sprite("later");
    reg.intern_sprites(["a", "b"]);
    assert_eq!(reg.sprite_epoch(), 0, "interning changes no sprite");
    register(&mut reg, "later", FilterMode::Nearest, 16, 16, "later.png");
    assert_eq!(reg.sprite_epoch(), 1);
    assert_eq!(reg.get_sprite("later").unwrap().nine_slice, None);

    reg.update_sprite_entry("later", TextureHandle(2), UvRect::FULL, 32, 32);
    assert_eq!(reg.sprite_epoch(), 2);
    reg.update_sprite_lit_atlas("later", Some(TextureHandle(2)));
    assert_eq!(reg.sprite_epoch(), 3);
    reg.set_sprite_nine_slice("later", Some([4, 4, 4, 4]));
    assert_eq!(reg.sprite_epoch(), 4);
    assert_eq!(
        reg.get_sprite("later").unwrap().nine_slice,
        Some([4, 4, 4, 4])
    );
    reg.set_sprite_nine_slice("later", None);
    assert_eq!(reg.sprite_epoch(), 5);
    assert_eq!(reg.get_sprite("later").unwrap().nine_slice, None);

    // An unregistered ID changes nothing.
    reg.update_sprite_entry("a", TextureHandle(2), UvRect::FULL, 32, 32);
    reg.update_sprite_lit_atlas("missing", None);
    reg.set_sprite_nine_slice("a", Some([1, 1, 1, 1]));
    assert_eq!(reg.sprite_epoch(), 5);
    assert!(reg.sprite(reg.sprite_id("a").unwrap()).is_none());
}

#[test]
fn a_sheets_path_maps_to_every_frame() {
    let mut reg = AssetRegistry::new();
    for n in 0..3 {
        register(
            &mut reg,
            &format!("walk_{n}"),
            FilterMode::Nearest,
            16,
            16,
            "walk.png",
        );
    }
    register(&mut reg, "rock", FilterMode::Nearest, 8, 8, "rock.png");
    let names: Vec<&str> = reg
        .sprite_ids_for_path(Path::new("walk.png"))
        .iter()
        .map(|&id| reg.sprite_name(id).unwrap())
        .collect();
    assert_eq!(names, ["walk_0", "walk_1", "walk_2"]);
    // The first registered still answers the one-name lookup.
    assert_eq!(
        reg.sprite_name_for_path(Path::new("walk.png")),
        Some("walk_0")
    );
    assert_eq!(reg.sprite_ids_for_path(Path::new("rock.png")).len(), 1);
    assert!(reg.sprite_ids_for_path(Path::new("none.png")).is_empty());

    reg.set_sprite_frame("walk_1", Some([16, 0, 16, 16]), Some([8.0, 15.0]));
    let frame = reg.get_sprite("walk_1").unwrap();
    assert_eq!(
        (frame.rect, frame.anchor),
        (Some([16, 0, 16, 16]), Some([8.0, 15.0]))
    );
}

#[test]
fn a_noted_sprite_size_reads_until_the_sprite_registers() {
    let mut reg = AssetRegistry::new();
    let epoch = reg.sprite_epoch();
    let id = reg.note_sprite_size("banner", 64, 32, Some([8.0, 30.0]));
    assert_eq!(reg.sprite_id("banner"), Some(id), "the name interned");
    assert_eq!(reg.sprite_size(id), Some((64, 32)));
    assert_eq!(reg.sprite_anchor(id), Some([8.0, 30.0]));
    assert!(reg.sprite(id).is_none() && reg.get_sprite("banner").is_none());
    assert_eq!(reg.sprite_names().count(), 0, "noted, not registered");
    assert_eq!(
        reg.sprite_epoch(),
        epoch,
        "noting changes no registered sprite"
    );
    // Registered at another size, its own size and anchor win.
    register(
        &mut reg,
        "banner",
        FilterMode::Nearest,
        48,
        48,
        "banner.png",
    );
    assert_eq!(reg.sprite_size(id), Some((48, 48)));
    assert_eq!(reg.sprite_anchor(id), None, "registered with no anchor");
    reg.set_sprite_frame("banner", None, Some([24.0, 47.0]));
    assert_eq!(reg.sprite_anchor(id), Some([24.0, 47.0]));
    // An interned name noted nothing, and an ID past the end, read none.
    let bare = reg.intern_sprite("bare");
    assert_eq!(
        (reg.sprite_size(bare), reg.sprite_anchor(bare)),
        (None, None)
    );
    let past = SpriteAssetId::new(99);
    assert_eq!(
        (reg.sprite_size(past), reg.sprite_anchor(past)),
        (None, None)
    );
}
