//! D-053 headless hot-reload tests; GPU upload paths covered by smoke suite.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use tungsten_core::assets::{
    AnimationData, AnimationRegistry, FilterMode, ParticleConfig, ParticleConfigRegistry,
    ParticleMesh, ParticleMeshRegistry, ParticleRender, ResolvedManifest, ResolvedParticleMesh,
    TilemapData, TilemapLayer, TilemapRegistry, UvRect,
};
use tungsten_core::ecs::{Entity, World};
use tungsten_core::ui::{Theme, UiThemeRegistry, UiTree};
use tungsten_core::{AssetRegistry, TextureHandle};

use super::reload::apply_nine_slice;
use super::*;
use tungsten_render::sprite::TextureHandles;

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn tempdir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_reload_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut f = fs::File::create(path).unwrap();
    f.write_all(contents.as_bytes()).unwrap();
}

fn seed_sprite(world: &mut World, id: &str) {
    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .register_sprite(
            id.to_string(),
            FilterMode::Nearest,
            16,
            16,
            PathBuf::from(format!("__test__/{id}.png")),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        );
}

fn seed_world() -> World {
    let mut world = World::new();
    world.insert_resource(AssetRegistry::new());
    world.insert_resource(AnimationRegistry::new());
    world.insert_resource(TilemapRegistry::new());
    world.insert_resource(ParticleConfigRegistry::new());
    world.insert_resource(tungsten_core::assets::ShaderRegistry::new());
    world
}

fn build_minimal_tmj(sprite_id: &str) -> String {
    format!(
        r#"{{
            "tilewidth": 16, "tileheight": 16,
            "width": 1, "height": 1,
            "tilesets": [{{"firstgid": 1, "tiles": [
                {{"id": 0, "properties": [{{"name": "sprite_id", "value": "{sprite_id}"}}]}}
            ]}}],
            "layers": [{{"type": "tilelayer", "name": "bg", "data": [1]}}]
        }}"#
    )
}

fn build_minimal_particle_json(sprite_id: &str) -> String {
    format!(
        r#"{{
            "sprite": "{sprite_id}",
            "max_alive": 100,
            "seed": 1,
            "blend": "alpha",
            "emission": {{"kind": "continuous", "rate_hz": 10.0}},
            "lifetime": {{"min": 0.5, "max": 1.0}},
            "initial_velocity": {{
                "kind": "radial",
                "speed": {{"min": 10.0, "max": 20.0}}
            }},
            "gravity": [0.0, 0.0],
            "drag_per_sec": 0.0,
            "angular_velocity": {{"min": 0.0, "max": 0.0}},
            "start_scale": {{"min": 1.0, "max": 1.0}},
            "scale_over_life": [[0.0, 1.0], [1.0, 1.0]],
            "color_over_life": [[0.0, [1.0, 1.0, 1.0, 1.0]], [1.0, [1.0, 1.0, 1.0, 1.0]]],
            "alpha_over_life": [[0.0, 1.0], [1.0, 1.0]],
            "tint": [1.0, 1.0, 1.0, 1.0]
        }}"#
    )
}

#[test]
fn reload_animation_replaces_registry_entry() {
    let dir = tempdir();
    let anim_path = dir.join("walk.json");
    write(
        &anim_path,
        r#"{"looping": true, "frames": [{"sprite": "walk_0", "duration_ms": 100}]}"#,
    );

    let mut world = seed_world();
    let initial = AnimationData::load(&anim_path, world.get_resource_mut().unwrap())
        .expect("initial animation parse should succeed");
    world
        .get_resource_mut::<AnimationRegistry>()
        .unwrap()
        .insert_with_path("walk".into(), initial, anim_path.clone());

    write(
        &anim_path,
        r#"{"looping": false, "frames": [{"sprite": "walk_1", "duration_ms": 250}]}"#,
    );
    reload_animation("walk", &anim_path, &mut world).unwrap();

    let reg = world.get_resource::<AnimationRegistry>().unwrap();
    let data = reg.get("walk").expect("animation should still exist");
    assert!(!data.looping, "reload must pick up the new `looping` field");
    assert_eq!(data.frames.len(), 1);
    assert_eq!(data.frames[0].duration_ms, 250);
}

#[test]
fn reload_animation_preserves_previous_on_parse_error() {
    let dir = tempdir();
    let anim_path = dir.join("walk.json");
    write(
        &anim_path,
        r#"{"looping": true, "frames": [{"sprite": "walk_0", "duration_ms": 100}]}"#,
    );

    let mut world = seed_world();
    let initial = AnimationData::load(&anim_path, world.get_resource_mut().unwrap()).unwrap();
    world
        .get_resource_mut::<AnimationRegistry>()
        .unwrap()
        .insert_with_path("walk".into(), initial, anim_path.clone());

    write(&anim_path, "not valid json!");
    reload_animation("walk", &anim_path, &mut world).unwrap();

    let reg = world.get_resource::<AnimationRegistry>().unwrap();
    let data = reg.get("walk").expect("last-known-good must be preserved");
    assert_eq!(data.frames[0].duration_ms, 100);
}

#[test]
fn reload_animation_resolves_frames_to_the_registry_ids() {
    let dir = tempdir();
    let anim_path = dir.join("run.json");
    write(
        &anim_path,
        r#"{"looping": true, "frames": [
            {"sprite": "run_0", "duration_ms": 100},
            {"sprite": "run_1", "duration_ms": 100}
        ]}"#,
    );

    let mut world = seed_world();
    let (run_0, run_1) = {
        let sprites = world.get_resource_mut::<AssetRegistry>().unwrap();
        (
            sprites.intern_sprite("run_0"),
            sprites.intern_sprite("run_1"),
        )
    };
    let initial = AnimationData::load(&anim_path, world.get_resource_mut().unwrap()).unwrap();
    let ids = |data: &AnimationData| data.frames.iter().map(|f| f.sprite).collect::<Vec<_>>();
    assert_eq!(ids(&initial), [run_0, run_1]);
    world
        .get_resource_mut::<AnimationRegistry>()
        .unwrap()
        .insert_with_path("run".into(), initial, anim_path.clone());

    write(
        &anim_path,
        r#"{"looping": true, "frames": [
            {"sprite": "run_1", "duration_ms": 80},
            {"sprite": "run_0", "duration_ms": 80},
            {"sprite": "run_1", "duration_ms": 80}
        ]}"#,
    );
    reload_animation("run", &anim_path, &mut world).unwrap();

    let reg = world.get_resource::<AnimationRegistry>().unwrap();
    assert_eq!(ids(reg.get("run").unwrap()), [run_1, run_0, run_1]);
    assert_eq!(
        world
            .get_resource::<AssetRegistry>()
            .unwrap()
            .sprite_id("run_1"),
        Some(run_1),
        "reload interned no second ID"
    );
}

#[test]
fn scene_sprite_named_before_registration_draws_once_registered() {
    use tungsten_core::assets::{SceneData, SceneEntry, SceneSprite, SceneTransform};
    use tungsten_core::{CommandBuffer, Sprite};

    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());
    let data = SceneData {
        entities: vec![SceneEntry {
            transform: SceneTransform {
                position: [4.0, 8.0],
                rotation: 0.0,
                scale: [1.0, 1.0],
            },
            sprite: Some(SceneSprite {
                asset_id: "late".into(),
                color: [255; 4],
                z_order: 0,
            }),
            visible: true,
            tag: None,
            tweens: Vec::new(),
            components: std::collections::BTreeMap::new(),
        }],
    };
    spawn_scene(&mut world, &data, "gameplay");
    let buf = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(buf);

    let late = world
        .get_resource::<AssetRegistry>()
        .unwrap()
        .sprite_id("late");
    assert!(late.is_some(), "spawn interned the scene's name");
    let spawned: Vec<_> = world
        .query::<(Entity, &Sprite)>()
        .map(|(_, s)| s.asset_id)
        .collect();
    assert_eq!(spawned, [late.unwrap()]);
    assert!(crate::sprite_extract::extract_sprites_default(&world).is_empty());

    let registered = world
        .get_resource_mut::<AssetRegistry>()
        .unwrap()
        .register_sprite(
            "late".into(),
            FilterMode::Nearest,
            16,
            16,
            PathBuf::from("test/late.png"),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        );
    assert_eq!(Some(registered), late);
    let batches = crate::sprite_extract::extract_sprites_default(&world);
    let positions: Vec<[f32; 2]> = batches
        .iter()
        .flat_map(|b| b.instances.iter().map(|i| i.position))
        .collect();
    assert_eq!(positions, [[4.0, 8.0]]);
}

#[test]
fn reload_tilemap_replaces_registry_entry() {
    let dir = tempdir();
    let tmj = dir.join("map.tmj");
    write(&tmj, &build_minimal_tmj("ground"));

    let mut world = seed_world();
    seed_sprite(&mut world, "ground");
    seed_sprite(&mut world, "water");

    let initial = TilemapData::load(&tmj).unwrap();
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert_with_path("level".into(), initial, tmj.clone());

    write(&tmj, &build_minimal_tmj("water"));
    reload_tilemap("level", &tmj, &mut world).unwrap();

    let reg = world.get_resource::<TilemapRegistry>().unwrap();
    let data = reg.get("level").expect("tilemap should still exist");
    assert_eq!(data.tileset, vec!["water".to_string()]);
}

#[test]
fn reload_tilemap_rejects_unknown_sprite_id() {
    let dir = tempdir();
    let tmj = dir.join("map.tmj");
    write(&tmj, &build_minimal_tmj("ground"));

    let mut world = seed_world();
    seed_sprite(&mut world, "ground");

    let initial = TilemapData::load(&tmj).unwrap();
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert_with_path("level".into(), initial, tmj.clone());

    write(&tmj, &build_minimal_tmj("not_a_real_sprite"));
    reload_tilemap("level", &tmj, &mut world).unwrap();

    let reg = world.get_resource::<TilemapRegistry>().unwrap();
    let data = reg.get("level").expect("stale data must be kept");
    assert_eq!(
        data.tileset,
        vec!["ground".to_string()],
        "unknown sprite-id reload must be rejected and leave last-known-good"
    );
}

#[test]
fn reload_particle_swaps_arc_under_same_asset_id() {
    let dir = tempdir();
    let cfg_path = dir.join("spark.json");
    write(&cfg_path, &build_minimal_particle_json("ex10_spark"));

    let mut world = seed_world();
    seed_sprite(&mut world, "ex10_spark");

    let initial = ParticleConfig::load(&cfg_path).unwrap();
    let initial_id = world
        .get_resource_mut::<ParticleConfigRegistry>()
        .unwrap()
        .register("spark".into(), cfg_path.clone(), initial);

    // D-050: stable AssetId across particle reloads.
    let bumped = build_minimal_particle_json("ex10_spark")
        .replace("\"max_alive\": 100", "\"max_alive\": 250");
    write(&cfg_path, &bumped);
    reload_particle("spark", &cfg_path, &mut world).unwrap();

    let reg = world.get_resource::<ParticleConfigRegistry>().unwrap();
    let id_after = reg.id_for_name("spark").expect("particle still registered");
    assert_eq!(
        initial_id, id_after,
        "asset id must stay stable across reloads"
    );
    assert_eq!(reg.get(id_after).unwrap().max_alive, 250);
}

#[test]
fn reload_particle_preserves_previous_on_unknown_sprite() {
    let dir = tempdir();
    let cfg_path = dir.join("spark.json");
    write(&cfg_path, &build_minimal_particle_json("ex10_spark"));

    let mut world = seed_world();
    seed_sprite(&mut world, "ex10_spark");

    let initial = ParticleConfig::load(&cfg_path).unwrap();
    world
        .get_resource_mut::<ParticleConfigRegistry>()
        .unwrap()
        .register("spark".into(), cfg_path.clone(), initial);

    write(&cfg_path, &build_minimal_particle_json("ghost_sprite"));
    reload_particle("spark", &cfg_path, &mut world).unwrap();

    let reg = world.get_resource::<ParticleConfigRegistry>().unwrap();
    let id = reg.id_for_name("spark").unwrap();
    assert_eq!(
        reg.get(id).unwrap().sprite,
        "ex10_spark",
        "unknown-sprite reload must be rejected and leave last-known-good"
    );
}

fn build_minimal_mesh_particle_json(mesh_id: &str) -> String {
    format!(
        r#"{{
            "render": {{"kind": "mesh", "mesh": "{mesh_id}"}},
            "max_alive": 100,
            "emission": {{"kind": "continuous", "rate_hz": 10.0}},
            "lifetime": {{"min": 0.5, "max": 1.0}},
            "initial_velocity": {{
                "kind": "radial",
                "speed": {{"min": 10.0, "max": 20.0}}
            }}
        }}"#
    )
}

fn triangle(width: f32) -> ParticleMesh {
    ParticleMesh {
        vertices: vec![[0.0, -7.0], [width, 7.0], [-width, 7.0]],
        indices: vec![0, 1, 2],
    }
}

fn manifest_with_meshes(meshes: &[(&str, ParticleMesh)]) -> ResolvedManifest {
    let mut manifest = ResolvedManifest::default();
    for (id, mesh) in meshes {
        manifest.particle_meshes.insert(
            (*id).to_string(),
            ResolvedParticleMesh {
                source_manifest: PathBuf::from("manifest.json"),
                mesh: mesh.clone(),
            },
        );
    }
    manifest
}

#[test]
fn diff_particle_meshes_reports_added_changed_removed() {
    let mut registry = ParticleMeshRegistry::new();
    registry.insert("keep", triangle(6.0));
    registry.insert("edit", triangle(6.0));
    registry.insert("gone_b", triangle(6.0));
    registry.insert("gone_a", triangle(6.0));

    let manifest = manifest_with_meshes(&[
        ("keep", triangle(6.0)),
        ("edit", triangle(9.0)),
        ("new_b", triangle(6.0)),
        ("new_a", triangle(6.0)),
    ]);
    let diff = diff_particle_meshes(&registry, &manifest);
    assert_eq!(diff.added, ["new_a", "new_b"]);
    assert_eq!(diff.changed, ["edit"]);
    assert_eq!(diff.removed, ["gone_a", "gone_b"]);

    let same = manifest_with_meshes(&[
        ("keep", triangle(6.0)),
        ("edit", triangle(6.0)),
        ("gone_a", triangle(6.0)),
        ("gone_b", triangle(6.0)),
    ]);
    assert_eq!(
        diff_particle_meshes(&registry, &same),
        ParticleMeshDiff::default()
    );
}

#[test]
fn reload_particle_accepts_registered_mesh() {
    let dir = tempdir();
    let cfg_path = dir.join("trail.json");
    write(&cfg_path, &build_minimal_particle_json("ex10_spark"));

    let mut world = seed_world();
    seed_sprite(&mut world, "ex10_spark");
    let mut meshes = ParticleMeshRegistry::new();
    meshes.insert("tri", triangle(6.0));
    world.insert_resource(meshes);

    let initial = ParticleConfig::load(&cfg_path).unwrap();
    let initial_id = world
        .get_resource_mut::<ParticleConfigRegistry>()
        .unwrap()
        .register("trail".into(), cfg_path.clone(), initial);

    write(&cfg_path, &build_minimal_mesh_particle_json("tri"));
    reload_particle("trail", &cfg_path, &mut world).unwrap();

    let reg = world.get_resource::<ParticleConfigRegistry>().unwrap();
    assert_eq!(reg.id_for_name("trail"), Some(initial_id));
    assert_eq!(
        reg.get(initial_id).unwrap().render,
        ParticleRender::Mesh { mesh: "tri".into() },
        "a mesh config naming a registered mesh must swap in"
    );
}

#[test]
fn reload_particle_preserves_previous_on_unknown_mesh() {
    let dir = tempdir();
    let cfg_path = dir.join("trail.json");
    write(&cfg_path, &build_minimal_particle_json("ex10_spark"));

    let mut world = seed_world();
    seed_sprite(&mut world, "ex10_spark");
    let mut meshes = ParticleMeshRegistry::new();
    meshes.insert("tri", triangle(6.0));
    world.insert_resource(meshes);

    let initial = ParticleConfig::load(&cfg_path).unwrap();
    let id = world
        .get_resource_mut::<ParticleConfigRegistry>()
        .unwrap()
        .register("trail".into(), cfg_path.clone(), initial);

    write(&cfg_path, &build_minimal_mesh_particle_json("ghost_mesh"));
    reload_particle("trail", &cfg_path, &mut world).unwrap();

    let reg = world.get_resource::<ParticleConfigRegistry>().unwrap();
    let cfg = reg.get(id).unwrap();
    assert_eq!(
        cfg.render,
        ParticleRender::Quad,
        "unknown-mesh reload must be rejected and leave last-known-good"
    );
    assert_eq!(cfg.sprite, "ex10_spark");
}

#[test]
fn load_all_merged_populates_loaded_manifest_resource() {
    // Merge step is renderer-free; end-to-end composition lives in core tests.
    let empty: &[PathBuf] = &[];
    let merged = tungsten_core::assets::ResolvedManifest::load_and_merge_many(empty)
        .expect("empty merge should succeed");
    let mut world = seed_world();
    world.insert_resource(tungsten_core::assets::LoadedManifest::new(merged));

    let resource = world
        .get_resource::<tungsten_core::assets::LoadedManifest>()
        .expect("LoadedManifest resource missing");
    assert!(resource.as_resolved().sprites.is_empty());
}

#[test]
fn in_place_shrink_uv_spans_the_new_size() {
    // B3: a 64x64 cell reloaded in place with a 32x32 image keeps the image at
    // the cell's top-left. The entry's UV must span that image, (new_w - 1,
    // new_h - 1) texels inside the half-texel inset, as a fresh pack produces;
    // the UV of the whole cell draws it squeezed into the smaller quad.
    let page = tungsten_core::assets::AtlasPage {
        width: 512,
        height: 256,
    };
    let cell = tungsten_core::assets::PackedSprite {
        id: "s".to_string(),
        page: 0,
        x: 128,
        y: 64,
        width: 64,
        height: 64,
    };
    let (uv, width, height) = super::atlas::shrink_entry(&cell, page, 32, 32);
    assert_eq!((width, height), (32, 32));
    let texels = |a: f32, b: f32, size: u32| (b - a) * size as f32;
    let span = [
        texels(uv.min[0], uv.max[0], page.width),
        texels(uv.min[1], uv.max[1], page.height),
    ];
    assert!(
        (span[0] - 31.0).abs() < 1e-3 && (span[1] - 31.0).abs() < 1e-3,
        "UV spans {span:?} texels; a fresh 32x32 pack spans [31.0, 31.0]"
    );
    let origin = [uv.min[0] * 512.0, uv.min[1] * 256.0];
    assert!(
        (origin[0] - 128.5).abs() < 1e-3 && (origin[1] - 64.5).abs() < 1e-3,
        "UV starts at texel {origin:?}, not the cell's inset corner"
    );
}

#[allow(dead_code)]
fn _touch_imports(_layer: TilemapLayer) {}

#[test]
fn fonts_load_in_sorted_id_order() {
    let mut manifest = ResolvedManifest::default();
    let ids = ["sans_bold", "mono", "zeta", "sans", "alpha", "mono_bold"];
    for id in ids {
        manifest.fonts.insert(
            id.to_string(),
            tungsten_core::assets::ResolvedFont {
                path: PathBuf::from(format!("{id}.ttf")),
            },
        );
    }
    let mut sorted = ids;
    sorted.sort_unstable();
    let order: Vec<&str> = font_load_order(&manifest)
        .into_iter()
        .map(String::as_str)
        .collect();
    assert_eq!(order, sorted);
}

// ---- M46: ui themes and nine-slice insets ----

fn theme_json(text_color: &str) -> String {
    format!(r#"{{ "tokens": {{ "colors": {{ "text": "{text_color}" }} }} }}"#)
}

/// A manifest whose `ui_themes` are `themes` (ID, file text), written under
/// `dir`, with `sans` and `mono` families declared.
fn theme_manifest(dir: &Path, themes: &[(&str, &str)]) -> ResolvedManifest {
    let mut manifest = ResolvedManifest::default();
    for family in ["sans", "mono"] {
        manifest.font_families.insert(
            family.to_string(),
            tungsten_core::assets::ResolvedFontFamily::new(vec![family.to_string()]),
        );
    }
    for (id, text) in themes {
        let path = dir.join("ui").join("themes").join(format!("{id}.json"));
        write(&path, text);
        manifest.ui_themes.insert(
            (*id).to_string(),
            tungsten_core::assets::ResolvedUiTheme { path },
        );
    }
    manifest
}

#[test]
fn the_theme_token_check_accepts_the_engine_family() {
    let dir = tempdir();
    let manifest = theme_manifest(&dir, &[]);
    let mut theme = Theme::default();
    theme.tokens.fonts.mono = crate::ENGINE_FONT_ID.to_string();
    theme.tokens.fonts.heading = "display".to_string();
    assert_eq!(
        unknown_font_tokens(&theme, &manifest),
        [("heading", "display")],
        "engine_mono is the engine's; sans is the manifest's"
    );
}

#[test]
fn ui_themes_load_into_the_registry_with_their_paths() {
    let dir = tempdir();
    let manifest = theme_manifest(&dir, &[("dark", &theme_json("#ff0000")), ("light", "{}")]);
    let mut world = World::new();
    load_ui_themes(&manifest, &mut world).unwrap();
    let registry = world.get_resource::<UiThemeRegistry>().unwrap();
    assert_eq!(registry.ids(), ["dark", "light"]);
    assert_eq!(
        registry.get("dark").unwrap().tokens.colors.text,
        [255, 0, 0, 255]
    );
    assert_eq!(registry.get("light"), Some(&Theme::default()));
    let paths = world.get_resource::<UiThemePaths>().unwrap();
    assert_eq!(
        paths.id_for_path(&manifest.ui_themes["dark"].path),
        Some("dark")
    );
    assert_eq!(paths.id_for_path(Path::new("elsewhere.json")), None);

    let bad = theme_manifest(&dir, &[("broken", "{ \"kinds\": { \"progress\": {} } }")]);
    let err = load_ui_themes(&bad, &mut world).unwrap_err();
    assert!(err.to_string().contains("ui theme 'broken'"), "{err}");
}

#[test]
fn reloading_the_applied_theme_restyles_the_tree_and_another_does_not() {
    let dir = tempdir();
    let manifest = theme_manifest(
        &dir,
        &[
            ("dark", &theme_json("#ff0000")),
            ("light", &theme_json("#000000")),
        ],
    );
    let mut world = World::new();
    load_ui_themes(&manifest, &mut world).unwrap();
    let dark = world
        .get_resource::<UiThemeRegistry>()
        .unwrap()
        .get("dark")
        .unwrap()
        .clone();
    let mut tree = UiTree::new();
    tree.add_root(
        tungsten_core::ui::UiLayer::Game,
        tungsten_core::ui::LayoutStyle::default(),
    );
    tree.set_theme_asset("dark", dark);
    tree.clear_paint_dirty();
    let epoch = tree.theme_epoch();
    world.insert_resource(tree);

    // The applied theme's file changes: the registry and the tree follow.
    let dark_path = manifest.ui_themes["dark"].path.clone();
    write(&dark_path, &theme_json("#00ff00"));
    reload_ui_theme("dark", &dark_path, &mut world).unwrap();
    let tree = world.get_resource::<UiTree>().unwrap();
    assert_eq!(tree.theme_epoch(), epoch.next());
    assert!(tree.is_paint_dirty());
    assert!(tree.is_layout_dirty());
    assert_eq!(tree.theme().tokens.colors.text, [0, 255, 0, 255]);
    assert_eq!(tree.theme_id(), Some("dark"));
    assert_eq!(
        world
            .get_resource::<UiThemeRegistry>()
            .unwrap()
            .get("dark")
            .unwrap()
            .tokens
            .colors
            .text,
        [0, 255, 0, 255]
    );

    // Another theme's file changes: the registry follows, the tree does not.
    world
        .get_resource_mut::<UiTree>()
        .unwrap()
        .clear_paint_dirty();
    let light_path = manifest.ui_themes["light"].path.clone();
    write(&light_path, &theme_json("#0000ff"));
    reload_ui_theme("light", &light_path, &mut world).unwrap();
    let tree = world.get_resource::<UiTree>().unwrap();
    assert_eq!(tree.theme_epoch(), epoch.next());
    assert!(!tree.is_paint_dirty());
    assert_eq!(
        world
            .get_resource::<UiThemeRegistry>()
            .unwrap()
            .get("light")
            .unwrap()
            .tokens
            .colors
            .text,
        [0, 0, 255, 255]
    );

    // A bad file keeps the previous theme, in the registry and on the tree.
    write(&dark_path, "{ not json");
    reload_ui_theme("dark", &dark_path, &mut world).unwrap();
    let tree = world.get_resource::<UiTree>().unwrap();
    assert_eq!(tree.theme_epoch(), epoch.next());
    assert_eq!(tree.theme().tokens.colors.text, [0, 255, 0, 255]);
    assert_eq!(
        world
            .get_resource::<UiThemeRegistry>()
            .unwrap()
            .get("dark")
            .unwrap()
            .tokens
            .colors
            .text,
        [0, 255, 0, 255]
    );
}

#[test]
fn a_manifest_reload_adds_a_new_theme_and_keeps_a_removed_one() {
    let dir = tempdir();
    let first = theme_manifest(&dir, &[("dark", &theme_json("#ff0000"))]);
    let mut world = World::new();
    load_ui_themes(&first, &mut world).unwrap();

    let second = theme_manifest(&dir, &[("light", "{}"), ("broken", "[]")]);
    reload_ui_themes_from_manifest(&second, &mut world);
    let registry = world.get_resource::<UiThemeRegistry>().unwrap();
    assert_eq!(
        registry.ids(),
        ["dark", "light"],
        "the removed theme stays, the new one loads, the broken one does not"
    );
    assert_eq!(
        registry.get("dark").unwrap().tokens.colors.text,
        [255, 0, 0, 255],
        "an existing entry keeps its theme"
    );
    let paths = world.get_resource::<UiThemePaths>().unwrap();
    assert_eq!(
        paths.id_for_path(&second.ui_themes["light"].path),
        Some("light")
    );
}

#[test]
fn nine_slice_insets_apply_once_per_change_and_move_the_sprite_epoch() {
    let mut world = seed_world();
    seed_sprite(&mut world, "frame");
    let registry = world.get_resource_mut::<AssetRegistry>().unwrap();
    let epoch = registry.sprite_epoch();
    assert!(apply_nine_slice(registry, "frame", Some([4, 4, 4, 4])));
    assert_eq!(registry.sprite_epoch(), epoch + 1);
    assert_eq!(
        registry.get_sprite("frame").unwrap().nine_slice,
        Some([4, 4, 4, 4])
    );
    assert!(
        !apply_nine_slice(registry, "frame", Some([4, 4, 4, 4])),
        "equal insets change nothing"
    );
    assert_eq!(registry.sprite_epoch(), epoch + 1);
    assert!(apply_nine_slice(registry, "frame", None));
    assert_eq!(registry.sprite_epoch(), epoch + 2);
    assert!(!apply_nine_slice(registry, "missing", Some([1, 1, 1, 1])));
    assert_eq!(registry.sprite_epoch(), epoch + 2);
}

/// A game component the scene tests register.
#[derive(Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Health {
    max: u32,
}

/// A marker whose definition hides the entity, overriding the entry's
/// `visible`.
#[derive(serde::Deserialize)]
struct Hidden;

fn scene_world() -> World {
    let mut world = seed_world();
    world.insert_resource(tungsten_core::CommandBuffer::new());
    world.register_component::<Health>("health");
    world.register_component_with(
        "hidden",
        |_: Hidden, _: &tungsten_core::SpawnContext<'_>| {
            (tungsten_core::Visibility { visible: false },)
        },
    );
    world
}

fn scene_from(world: &World, src: &str) -> tungsten_core::assets::SceneData {
    let dir = tempdir();
    let path = dir.join("scene.json");
    write(&path, src);
    let data = load_scene(world, &path);
    fs::remove_dir_all(&dir).unwrap();
    data.unwrap()
}

fn flush(world: &mut World) {
    let buf = world
        .remove_resource::<tungsten_core::CommandBuffer>()
        .unwrap();
    world.flush(buf);
    world.insert_resource(tungsten_core::CommandBuffer::new());
}

#[test]
fn a_scene_entry_spawns_its_components_beside_its_fields() {
    use crate::state::SceneEntity;
    use tungsten_core::{Transform, Visibility};

    let mut world = scene_world();
    let data = scene_from(
        &world,
        r#"{ "entities": [ {
            "transform": { "position": [3.0, 4.0] },
            "components": { "health": { "max": 12 } }
        } ] }"#,
    );
    spawn_scene(&mut world, &data, "gameplay");
    flush(&mut world);

    let spawned: Vec<_> = world
        .query::<(&Health, &Transform, &Visibility, &SceneEntity)>()
        .map(|(health, transform, visibility, scene)| {
            (
                health.max,
                transform.position,
                visibility.visible,
                scene.state_id,
            )
        })
        .collect();
    assert_eq!(spawned, [(12, glam::Vec2::new(3.0, 4.0), true, "gameplay")]);
}

#[test]
fn a_component_overrides_a_field_of_the_same_type() {
    use tungsten_core::Visibility;

    let mut world = scene_world();
    let data = scene_from(
        &world,
        r#"{ "entities": [ {
            "transform": { "position": [0.0, 0.0] },
            "visible": true,
            "components": { "hidden": null }
        } ] }"#,
    );
    spawn_scene(&mut world, &data, "gameplay");
    flush(&mut world);
    let visible: Vec<_> = world.query::<&Visibility>().map(|v| v.visible).collect();
    assert_eq!(visible, [false]);
}

#[test]
fn an_unregistered_component_panics_before_anything_is_queued() {
    let mut world = scene_world();
    // Checked against another World's registry, so it skipped this one's.
    let mut other = World::new();
    other.register_component::<Health>("helth");
    let data = scene_from(
        &other,
        r#"{ "entities": [ {
            "transform": { "position": [0.0, 0.0] },
            "sprite": { "asset_id": "never" },
            "components": { "helth": { "max": 1 } }
        } ] }"#,
    );
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        spawn_scene(&mut world, &data, "gameplay");
    }))
    .expect_err("an unknown component must panic");
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_default();
    assert_eq!(message, "scene entities[0]: unknown component `helth`");
    assert!(
        world
            .get_resource::<tungsten_core::CommandBuffer>()
            .unwrap()
            .is_empty()
    );
    assert!(
        world
            .get_resource::<AssetRegistry>()
            .unwrap()
            .sprite_id("never")
            .is_none(),
        "no sprite name interned"
    );
}

#[test]
fn load_scene_checks_against_the_worlds_registry() {
    let dir = tempdir();
    let path = dir.join("scene.json");
    write(
        &path,
        r#"{ "entities": [ {
            "transform": { "position": [0.0, 0.0] },
            "components": { "health": { "max": 1 } }
        } ] }"#,
    );
    let unregistered = load_scene(&World::new(), &path).unwrap_err().to_string();
    assert!(
        unregistered.ends_with("entities[0]: unknown component `health`"),
        "{unregistered}"
    );
    assert!(load_scene(&scene_world(), &path).is_ok());
    fs::remove_dir_all(&dir).unwrap();
}

/// A state that loads and spawns a scene file on enter.
struct SceneState(PathBuf);

impl crate::state::GameState for SceneState {
    fn id(&self) -> crate::state::StateId {
        "scene"
    }

    fn on_enter(&mut self, ctx: &mut crate::state::StateContext) {
        let data = load_scene(ctx.world, &self.0).unwrap();
        spawn_scene(ctx.world, &data, "scene");
    }

    fn on_exit(&mut self, _ctx: &mut crate::state::StateContext) {}

    fn update(&mut self, _world: &mut World) {}
}

#[test]
fn an_app_registered_component_spawns_from_a_scene_and_leaves_with_its_state() {
    use crate::state::StateStack;

    let dir = tempdir();
    let path = dir.join("scene.json");
    write(
        &path,
        r#"{ "entities": [ {
            "transform": { "position": [0.0, 0.0] },
            "components": { "health": { "max": 30 } }
        } ] }"#,
    );
    let mut app = crate::App::new(tungsten_core::Config::default()).unwrap();
    app.register_component::<Health>("health");
    app.world_mut()
        .resource_mut::<StateStack>()
        .request_push(SceneState(path));
    let mut harness = crate::testing::Harness::new(app);
    harness.step(1);
    let healths: Vec<_> = harness.world().query::<&Health>().map(|h| h.max).collect();
    assert_eq!(healths, [30]);

    harness
        .world_mut()
        .resource_mut::<StateStack>()
        .request_pop();
    harness.step(1);
    assert_eq!(harness.world().query::<&Health>().count(), 0);
    fs::remove_dir_all(&dir).unwrap();
}

/// A world with the umbrella's scene components.
fn umbrella_components_world() -> World {
    let mut world = seed_world();
    world.insert_resource(tungsten_core::CommandBuffer::new());
    register_extract_components(&mut world);
    register_squash_stretch(&mut world);
    register_particle_emitter(&mut world);
    world
}

#[test]
fn the_umbrella_components_spawn_with_their_values() {
    use tungsten_core::{
        Easing, Light, LightKind, ParallaxLayer, ParticleEmitter, ParticleEmitterState,
        SpriteSquashStretch, SquashTrigger, TilemapInstance,
    };

    let mut world = umbrella_components_world();
    let dir = tempdir();
    let config_path = dir.join("embers.json");
    write(&config_path, &build_minimal_particle_json("spark"));
    let config = ParticleConfig::load(&config_path).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    let embers = world.resource_mut::<ParticleConfigRegistry>().register(
        "embers".into(),
        config_path,
        config,
    );
    world
        .resource_mut::<TilemapRegistry>()
        .insert("arena".into(), TilemapData::new(16, 16, 1, 1));
    let data = scene_from(
        &world,
        r#"{ "entities": [
            { "transform": { "position": [7.0, 9.0] }, "components": {
                "particle_emitter": { "config": "embers", "seed": 9 },
                "tilemap": { "id": "arena" }
            } },
            { "transform": { "position": [0.0, 0.0] }, "components": {
                "particle_emitter": { "config": "embers" }
            } },
            { "transform": { "position": [0.0, 0.0] }, "components": {
                "light": { "kind": "point", "radius": 40.0, "color": [1.0, 0.5, 0.0], "intensity": 2.0 },
                "parallax_layer": { "scroll_factor": [0.5, 0.25] },
                "squash_stretch": { "on": "on_land", "amount": [1.3, 0.7], "duration": 0.2, "easing": "cubic_out" }
            } },
            { "transform": { "position": [0.0, 0.0] }, "components": {
                "light": { "kind": "directional", "angle": 1.5, "color": [0.2, 0.2, 0.2] },
                "squash_stretch": { "on": "manual", "amount": [1.1, 1.1], "duration": 0.5 }
            } }
        ] }"#,
    );
    spawn_scene(&mut world, &data, "gameplay");
    flush(&mut world);

    let mut emitters: Vec<_> = world
        .query::<(&ParticleEmitter, &ParticleEmitterState)>()
        .map(|(emitter, state)| (emitter.config, emitter.seed_override, state.first_tick_done))
        .collect();
    emitters.sort_by_key(|emitter| emitter.1);
    assert_eq!(emitters, [(embers, None, false), (embers, Some(9), false)]);
    let maps: Vec<_> = world
        .query::<&TilemapInstance>()
        .map(|map| (map.id.clone(), map.origin))
        .collect();
    assert_eq!(
        maps,
        [("arena".to_owned(), glam::Vec2::new(7.0, 9.0))],
        "the entry's position is the map's top-left"
    );

    let mut lights: Vec<Light> = world.query::<&Light>().copied().collect();
    lights.sort_by(|a, b| a.intensity.total_cmp(&b.intensity));
    assert_eq!(
        lights,
        [
            Light {
                kind: LightKind::Directional { angle: 1.5 },
                color: glam::Vec3::new(0.2, 0.2, 0.2),
                intensity: 1.0,
            },
            Light {
                kind: LightKind::Point { radius: 40.0 },
                color: glam::Vec3::new(1.0, 0.5, 0.0),
                intensity: 2.0,
            },
        ]
    );
    let layers: Vec<ParallaxLayer> = world.query::<&ParallaxLayer>().copied().collect();
    assert_eq!(layers, [ParallaxLayer::new(glam::Vec2::new(0.5, 0.25))]);
    let mut squashes: Vec<SpriteSquashStretch> =
        world.query::<&SpriteSquashStretch>().copied().collect();
    squashes.sort_by(|a, b| a.duration.total_cmp(&b.duration));
    assert_eq!(
        squashes,
        [
            SpriteSquashStretch {
                on: SquashTrigger::OnLand,
                amount: glam::Vec2::new(1.3, 0.7),
                duration: 0.2,
                easing: Easing::CubicOut,
            },
            SpriteSquashStretch {
                on: SquashTrigger::Manual,
                amount: glam::Vec2::new(1.1, 1.1),
                duration: 0.5,
                easing: Easing::Linear,
            },
        ]
    );
}

#[test]
fn the_umbrella_components_reject_bad_values() {
    let world = umbrella_components_world();
    let cases = [
        (
            "light",
            r#"{ "kind": "point", "radius": 0.0, "color": [1.0, 1.0, 1.0] }"#,
            "`radius` must be finite and > 0 (got 0)",
        ),
        (
            "light",
            r#"{ "kind": "point", "radius": 4.0, "color": [1.0, -0.1, 1.0] }"#,
            "`color` channels must be finite and >= 0 (got [1, -0.1, 1])",
        ),
        (
            "light",
            r#"{ "kind": "directional", "angle": 0.0, "color": [1.0, 1.0, 1.0], "intensity": -1.0 }"#,
            "`intensity` must be finite and >= 0 (got -1)",
        ),
        (
            "light",
            r#"{ "kind": "directional", "angle": 1e40, "color": [1.0, 1.0, 1.0] }"#,
            "`angle` must be finite (got inf)",
        ),
        (
            "light",
            r#"{ "kind": "spot", "radius": 4.0, "color": [1.0, 1.0, 1.0] }"#,
            "unknown variant `spot`",
        ),
        (
            "light",
            r#"{ "kind": "point", "radius": 4.0, "color": [1.0, 1.0, 1.0], "falloff": 2 }"#,
            "unknown field `falloff`",
        ),
        (
            "light",
            r#"{ "kind": "point", "radius": 4.0 }"#,
            "missing field `color`",
        ),
        (
            "parallax_layer",
            r#"{ "scroll_factor": [1e40, 0.0] }"#,
            "`scroll_factor` must be finite (got [inf, 0])",
        ),
        (
            "parallax_layer",
            r#"{ "scroll_factor": [1.0, 1.0], "depth": 2 }"#,
            "unknown field `depth`",
        ),
        (
            "squash_stretch",
            r#"{ "on": "on_land", "amount": [1.0, 0.0], "duration": 0.2 }"#,
            "`amount` must be finite and > 0 on both axes (got [1, 0])",
        ),
        (
            "squash_stretch",
            r#"{ "on": "on_land", "amount": [1.2, 0.8], "duration": 0.0 }"#,
            "`duration` must be finite and > 0 (got 0)",
        ),
        (
            "squash_stretch",
            r#"{ "on": "on_jump", "amount": [1.2, 0.8], "duration": 0.2 }"#,
            "unknown variant `on_jump`",
        ),
        (
            "squash_stretch",
            r#"{ "on": "on_land", "amount": [1.2, 0.8], "duration": 0.2, "easing": "wobbly" }"#,
            "unknown variant `wobbly`",
        ),
        (
            "particle_emitter",
            r#"{ "config": "embers" }"#,
            "field `config`: unknown particle config `embers`",
        ),
        (
            "particle_emitter",
            r#"{ "config": "embers", "rate": 2 }"#,
            "unknown field `rate`",
        ),
        (
            "particle_emitter",
            r#"{ "seed": 2 }"#,
            "missing field `config`",
        ),
        (
            "particle_emitter",
            r#"{ "config": "embers", "seed": -1 }"#,
            "field `seed`: invalid value: integer `-1`",
        ),
        (
            "tilemap",
            r#"{ "id": "cave" }"#,
            "field `id`: unknown tilemap `cave`",
        ),
        (
            "tilemap",
            r#"{ "id": "cave", "layer": 1 }"#,
            "unknown field `layer`",
        ),
    ];
    for (name, value, expected) in cases {
        let src = format!(
            r#"{{ "entities": [ {{ "transform": {{ "position": [0.0, 0.0] }}, "components": {{ "{name}": {value} }} }} ] }}"#
        );
        let dir = tempdir();
        let path = dir.join("scene.json");
        write(&path, &src);
        let err = load_scene(&world, &path).unwrap_err().to_string();
        fs::remove_dir_all(&dir).unwrap();
        assert!(
            err.contains(&format!("entities[0]: component `{name}`")) && err.contains(expected),
            "{name} {value}: {err}"
        );
    }
}

#[test]
fn a_scene_naming_the_four_engine_components_spawns_them_and_steps_the_bodies() {
    use crate::state::StateStack;
    use tungsten_core::{Light, ParallaxLayer, Position, RigidBody, SpriteSquashStretch};

    let dir = tempdir();
    let path = dir.join("scene.json");
    write(
        &path,
        r#"{ "entities": [
            { "transform": { "position": [100.0, 50.0] }, "components": {
                "rigid_body": { "kind": "dynamic", "shape": { "kind": "circle", "radius": 4.0 },
                                "velocity": [60.0, 0.0] },
                "light": { "kind": "point", "radius": 30.0, "color": [1.0, 1.0, 1.0] },
                "parallax_layer": { "scroll_factor": [1.0, 1.0] },
                "squash_stretch": { "on": "on_land", "amount": [1.2, 0.8], "duration": 0.1 }
            } },
            { "transform": { "position": [-200.0, 50.0] }, "components": {
                "rigid_body": { "kind": "static", "shape": { "kind": "aabb", "half_extents": [16.0, 4.0] } }
            } }
        ] }"#,
    );
    let mut app = crate::App::new(tungsten_core::Config::default()).unwrap();
    app.world_mut()
        .resource_mut::<StateStack>()
        .request_push(SceneState(path));
    let mut harness = crate::testing::Harness::new(app);
    harness.step(1);
    let world = harness.world();
    assert_eq!(world.query::<&Light>().count(), 1);
    assert_eq!(world.query::<&ParallaxLayer>().count(), 1);
    assert_eq!(world.query::<&SpriteSquashStretch>().count(), 1);
    let bodies = |world: &World| {
        let mut bodies: Vec<(bool, glam::Vec2)> = world
            .query::<(&RigidBody, &Position)>()
            .map(|(body, position)| (body.inv_mass > 0.0, position.0))
            .collect();
        bodies.sort_by_key(|body| body.0);
        bodies
    };
    let spawned = bodies(world);
    assert_eq!(spawned.len(), 2);

    harness.step(1);
    let stepped = bodies(harness.world());
    let (still, moving) = (stepped[0], stepped[1]);
    assert_eq!(still, spawned[0], "the static body stays");
    assert!(
        moving.1.x > spawned[1].1.x,
        "{moving:?} after {:?}",
        spawned[1]
    );
    assert_eq!(moving.1.y, spawned[1].1.y);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_sounds_gives_a_sound_its_manifest_bus_and_voice_limit() {
    let dir = tempdir();
    let blip =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/kit/shmup/sounds/blip.ogg");
    fs::copy(&blip, dir.join("blip.ogg")).unwrap();
    write(
        &dir.join("manifest.json"),
        r#"{"sounds": {
            "theme": {"path": "blip.ogg", "looping": true, "volume": 0.4, "bus": "music"},
            "hit": {"path": "blip.ogg", "volume": 0.8, "max_voices": 3}
        }}"#,
    );
    let manifest = ResolvedManifest::load(dir.join("manifest.json")).unwrap();
    let mut world = World::new();
    load_sounds(&manifest, &mut world).unwrap();
    let sounds = world.resource::<SoundRegistry>();
    let theme = sounds.get_by_id("theme").unwrap();
    let hit = sounds.get_by_id("hit").unwrap();
    assert_eq!(
        (sounds.get_bus(theme), sounds.get_max_voices(theme)),
        (tungsten_core::AudioBus::Music, 64)
    );
    assert_eq!(
        (sounds.get_bus(hit), sounds.get_max_voices(hit)),
        (tungsten_core::AudioBus::Effects, 3)
    );
    assert!(sounds.get_looping(theme) && (sounds.get_volume(hit) - 0.8).abs() < 1e-6);
}

#[test]
fn the_loader_inserts_physics_names_and_a_reload_replaces_them() {
    use glam::Vec2;
    use tungsten_core::physics::{PhysicsLayers, PhysicsMaterial, PhysicsMaterials, RigidBody};
    use tungsten_core::schedule::{Plugin, Schedule};

    // A manifest naming a layer and a material, and a prefab naming both.
    let dir = tempdir();
    write(
        &dir.join("prefabs/crate.json"),
        r#"{ "components": { "rigid_body": { "kind": "dynamic",
            "shape": { "kind": "aabb", "half_extents": [8.0, 8.0] },
            "layers": ["walls"], "material": "ice" } } }"#,
    );
    let root = dir.join("manifest.json");
    write(
        &root,
        r#"{ "physics_layers": { "walls": 3 },
             "physics_materials": { "ice": { "friction": 0.05, "restitution": 0.25 } },
             "prefabs": { "crate": { "path": "prefabs/crate.json" } } }"#,
    );
    let manifest = ResolvedManifest::load(&root).unwrap();
    let mut world = seed_world();
    tungsten_core::physics::PhysicsPlugin.build(&mut Schedule::new(), &mut world);
    // Before the names load, the prefab names what the World lacks.
    assert!(load_prefabs(&manifest, &mut world).is_err());
    load_physics_names(&manifest, &mut world);
    load_prefabs(&manifest, &mut world).unwrap();
    let ice = PhysicsMaterial {
        friction: 0.05,
        restitution: 0.25,
        rolling: 0.0,
    };
    assert_eq!(world.resource::<PhysicsLayers>().bit("walls"), Some(1 << 3));
    let materials = world.resource::<PhysicsMaterials>();
    assert_eq!((materials.get("ice"), materials.version()), (Some(ice), 0));
    let spawned = world.spawn_prefab("crate", Vec2::ZERO).unwrap();
    let body = *world.get::<RigidBody>(spawned).unwrap();
    assert_eq!(
        (body.layers, body.friction, body.restitution),
        (1 << 3, 0.05, 0.25)
    );

    // A reload replaces both, the materials' version moving on.
    let mut next = ResolvedManifest::default();
    next.physics_layers.insert("water".to_owned(), 5);
    next.physics_materials.insert(
        "mud".to_owned(),
        PhysicsMaterial {
            friction: 0.9,
            ..PhysicsMaterial::default()
        },
    );
    load_physics_names(&next, &mut world);
    let layers = world.resource::<PhysicsLayers>();
    assert_eq!(
        (layers.bit("walls"), layers.bit("water")),
        (None, Some(1 << 5))
    );
    let materials = world.resource::<PhysicsMaterials>();
    assert_eq!(materials.get("ice"), None);
    assert_eq!(materials.get("mud").map(|mud| mud.friction), Some(0.9));
    assert_eq!(materials.version(), 1);
}

/// Four 16 × 16 cells, each one colour, as a 32 × 32 sheet at `dir/walk.png`
/// and as four files `dir/walk_<n>.png`; the colour of cell `n`.
fn write_walk_frames(dir: &Path, shade: u8) -> [[u8; 4]; 4] {
    let colours = [
        [shade, 0, 0, 255],
        [0, shade, 0, 255],
        [0, 0, shade, 255],
        [shade, shade, 0, 128],
    ];
    let mut sheet = image::RgbaImage::new(32, 32);
    for (n, colour) in colours.iter().enumerate() {
        let (cx, cy) = (n as u32 % 2 * 16, n as u32 / 2 * 16);
        for y in 0..16 {
            for x in 0..16 {
                sheet.put_pixel(cx + x, cy + y, image::Rgba(*colour));
            }
        }
        image::RgbaImage::from_pixel(16, 16, image::Rgba(*colour))
            .save(dir.join(format!("walk_{n}.png")))
            .unwrap();
    }
    sheet.save(dir.join("walk.png")).unwrap();
    colours
}

#[test]
fn a_sheets_frames_pack_as_their_own_images_would() {
    let dir = tempdir();
    write_walk_frames(&dir, 200);
    let sheet = dir.join("sheet.json");
    write(
        &sheet,
        r#"{"sheets": {"walk": {"path": "walk.png", "frame": [16, 16]}}}"#,
    );
    let files = dir.join("files.json");
    write(
        &files,
        r#"{"sprites": {
            "walk_0": {"path": "walk_0.png"}, "walk_1": {"path": "walk_1.png"},
            "walk_2": {"path": "walk_2.png"}, "walk_3": {"path": "walk_3.png"}
        }}"#,
    );
    let pack = |manifest: &Path| {
        let manifest = ResolvedManifest::load(manifest).unwrap();
        let [nearest, linear] = super::atlas::decode_sprites(&manifest).unwrap();
        assert!(linear.is_empty());
        let (pack, canvases) = super::atlas::pack_pages(&nearest, 256);
        let mut placed: Vec<_> = pack
            .sprites
            .iter()
            .map(|p| (p.id.clone(), p.page, p.x, p.y, p.width, p.height))
            .collect();
        placed.sort();
        (pack.pages, placed, canvases.albedo)
    };
    let (sheet_pages, sheet_placed, sheet_albedo) = pack(&sheet);
    let (file_pages, file_placed, file_albedo) = pack(&files);
    assert_eq!(sheet_placed.len(), 4);
    assert_eq!(sheet_pages, file_pages);
    assert_eq!(sheet_placed, file_placed, "the same cells, so the same UVs");
    assert!(sheet_albedo == file_albedo, "the same pixels on the pages");
}

#[test]
fn a_sheet_edit_crops_every_frame_from_one_decode() {
    let dir = tempdir();
    write_walk_frames(&dir, 200);
    // The edit: every cell a new shade, cut from one decode of the file.
    let colours = write_walk_frames(&dir, 90);
    let rects: Vec<_> = (0..4)
        .map(|n| Some([n % 2 * 16, n / 2 * 16, 16, 16]))
        .chain([Some([24, 24, 16, 16])])
        .collect();
    let cuts = super::atlas::reload_cuts(&dir.join("walk.png"), None, None, &rects, |_, why| {
        panic!("no sibling: {why}")
    })
    .unwrap();
    for (n, colour) in colours.iter().enumerate() {
        let cut = cuts[n].as_ref().expect("a cell inside the image");
        assert_eq!((cut.width, cut.height), (16, 16));
        assert!(cut.rgba.chunks(4).all(|texel| texel == colour), "frame {n}");
        assert!(cut.normal.is_none() && cut.emissive.is_none());
    }
    assert!(
        cuts[4].is_none(),
        "a frame that leaves the image cuts nothing"
    );
}

/// A 16-bit mono WAV of `samples` samples at 8 kHz.
fn write_wav(path: &Path, samples: u32) {
    let data = samples * 2;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&[1, 0, 1, 0]);
    bytes.extend_from_slice(&8000u32.to_le_bytes());
    bytes.extend_from_slice(&16000u32.to_le_bytes());
    bytes.extend_from_slice(&[2, 0, 16, 0]);
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for i in 0..samples {
        bytes.extend_from_slice(&((i % 64) as i16 * 100).to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

/// A game folder with a manifest of every kind a plan has a unit for, a
/// sheet among its sprites and a boot group; the manifest's path.
fn plan_game(boot: &str) -> PathBuf {
    let dir = tempdir();
    write_walk_frames(&dir, 200);
    image::RgbaImage::from_pixel(8, 12, image::Rgba([9, 9, 9, 255]))
        .save(dir.join("hero.png"))
        .unwrap();
    image::RgbaImage::new(24, 6)
        .save(dir.join("panel.png"))
        .unwrap();
    write(
        &dir.join("run.json"),
        r#"{"looping": true, "frames": [{"sprite": "walk_0", "duration_ms": 100}, {"sprite": "walk_3", "duration_ms": 100}]}"#,
    );
    write(
        &dir.join("idle.json"),
        r#"{"looping": false, "frames": [{"sprite": "hero", "duration_ms": 50}]}"#,
    );
    write(&dir.join("sans.ttf"), "not parsed headless");
    write_wav(&dir.join("step.wav"), 400);
    write_wav(&dir.join("hum.wav"), 900);
    write(&dir.join("level.tmj"), &build_minimal_tmj("walk_1"));
    write(
        &dir.join("spark.json"),
        &build_minimal_particle_json("hero"),
    );
    let manifest = dir.join("manifest.json");
    write(
        &manifest,
        &format!(
            r#"{{
                "sprites": {{"hero": {{"path": "hero.png"}}, "panel": {{"path": "panel.png", "filter": "linear"}}}},
                "sheets": {{"walk": {{"path": "walk.png", "frame": [16, 16], "anchor": [8, 15]}}}},
                "animations": {{"run": {{"path": "run.json"}}, "idle": {{"path": "idle.json"}}}},
                "fonts": {{"sans": {{"path": "sans.ttf"}}}},
                "sounds": {{"step": {{"path": "step.wav"}}, "hum": {{"path": "hum.wav", "looping": true}}}},
                "tilemaps": {{"level": {{"path": "level.tmj"}}}},
                "particles": {{"spark": {{"path": "spark.json"}}}}
                {boot}
            }}"#
        ),
    );
    manifest
}

/// What a load leaves in the World, as text that does not hang on map
/// order: every sprite and page, animation, sound, map, particle config and
/// font.
fn loaded(world: &World, manifest: &ResolvedManifest) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let sprites = world.resource::<AssetRegistry>();
    let mut names: Vec<&str> = sprites.sprite_names().collect();
    names.sort_unstable();
    for name in names {
        let s = sprites.get_sprite(name).unwrap();
        let id = sprites.sprite_id(name).unwrap().index();
        let _ = writeln!(
            out,
            "sprite {name} #{id} {:?} {:?} {}x{} {:?} {:?} {:?} {:?}",
            s.atlas, s.uv, s.width, s.height, s.filter, s.lit_atlas, s.rect, s.anchor
        );
    }
    let atlas = world.resource::<AtlasRegistry>();
    let mut sizes: Vec<_> = atlas
        .page_sizes
        .iter()
        .map(|(h, p)| (h.0, p.width, p.height))
        .collect();
    sizes.sort_unstable();
    let mut packed: Vec<_> = atlas
        .packed
        .values()
        .map(|p| (p.id.clone(), p.page, p.x, p.y, p.width, p.height))
        .collect();
    packed.sort();
    let _ = writeln!(
        out,
        "pages {:?} {:?} {sizes:?}\npacked {packed:?}",
        atlas.nearest_pages, atlas.linear_pages
    );
    let animations = world.resource::<AnimationRegistry>();
    let mut ids: Vec<&str> = animations.ids().collect();
    ids.sort_unstable();
    for id in ids {
        let _ = writeln!(
            out,
            "animation {id} {}",
            animations.get(id).unwrap().frames.len()
        );
    }
    let sounds = world.resource::<SoundRegistry>();
    let mut ids: Vec<&String> = manifest.sounds.keys().collect();
    ids.sort();
    for id in ids {
        let handle = sounds.get_by_id(id).unwrap();
        let _ = writeln!(
            out,
            "sound {id} {} {} {}",
            sounds.get(handle).unwrap().samples.len(),
            sounds.get_volume(handle),
            sounds.get_looping(handle)
        );
    }
    let maps = world.resource::<TilemapRegistry>();
    let mut ids: Vec<&str> = maps.ids().collect();
    ids.sort_unstable();
    let particles = world.resource::<ParticleConfigRegistry>();
    let mut configs: Vec<&str> = particles.names().collect();
    configs.sort_unstable();
    let fonts = world.resource::<FontRegistry>();
    let _ = writeln!(
        out,
        "maps {ids:?} particles {configs:?} sans {}",
        fonts.contains_id("sans")
    );
    out
}

/// A World as a window's start builds it, for a load to land in.
fn load_world() -> World {
    let mut app = crate::App::new(tungsten_core::Config::default()).unwrap();
    std::mem::take(app.world_mut())
}

#[test]
fn a_plan_lists_load_alls_units_in_order() {
    let manifest = ResolvedManifest::load(plan_game("")).unwrap();
    let path = |name: &str| manifest.sprites[name].path.clone();
    let mut world = load_world();
    let plan = LoadPlan::new(manifest.clone(), &mut world);
    let mut files = vec![path("hero"), path("panel"), path("walk_0")];
    files.sort();
    let mut expected: Vec<LoadUnit> = files.into_iter().map(LoadUnit::Sprites).collect();
    expected.extend([
        LoadUnit::Pack(FilterMode::Nearest),
        LoadUnit::Pack(FilterMode::Linear),
        LoadUnit::Animation("idle".into()),
        LoadUnit::Animation("run".into()),
        LoadUnit::Font("sans".into()),
        LoadUnit::FontFamilies,
        LoadUnit::Themes,
        LoadUnit::Shaders,
        LoadUnit::Materials,
        LoadUnit::Sound("hum".into()),
        LoadUnit::Sound("step".into()),
        LoadUnit::Tilemap("level".into()),
        LoadUnit::ParticleMeshes,
        LoadUnit::Particles,
        LoadUnit::PhysicsNames,
        LoadUnit::Prefabs,
        LoadUnit::CrossChecks,
    ]);
    assert_eq!(plan.units(), expected.as_slice());
    assert_eq!(plan.done(), 0);
    // Every name, sheet frames included, is interned before any unit, in
    // sorted order.
    let sprites = world.resource::<AssetRegistry>();
    let ids: Vec<u32> = ["hero", "panel", "walk_0", "walk_3"]
        .iter()
        .map(|name| sprites.sprite_id(name).unwrap().index())
        .collect();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]), "{ids:?}");
}

#[test]
fn a_plan_run_in_slices_registers_and_packs_what_load_all_does() {
    let manifest = ResolvedManifest::load(plan_game("")).unwrap();
    // All at once, as `load_all` runs them.
    let mut at_once = load_world();
    let mut target = HeadlessTarget::default();
    let mut plan = LoadPlan::new(manifest.clone(), &mut at_once);
    while plan.run_next_into(&mut at_once, &mut target).unwrap() {}
    // A slice of one unit each, the World handed back between them.
    let mut sliced = load_world();
    let mut sliced_target = HeadlessTarget::default();
    let mut plan = LoadPlan::new(manifest.clone(), &mut sliced);
    let mut slices = 0;
    while !plan.is_finished() {
        assert!(plan.run_next_into(&mut sliced, &mut sliced_target).unwrap());
        slices += 1;
    }
    assert!(!plan.run_next_into(&mut sliced, &mut sliced_target).unwrap());
    assert_eq!(slices, plan.units().len());
    let text = loaded(&at_once, &manifest);
    assert_eq!(text, loaded(&sliced, &manifest));
    assert_eq!(target.uploads, sliced_target.uploads);
    // The pack's pages are the counter's from 0, the sheet's frames on them.
    assert!(text.contains("sprite walk_2 "), "{text}");
    assert_eq!(target.uploads[0], TextureHandle(0));
    let timings = plan.timings();
    assert!(timings.units >= timings.sprites + timings.sounds + timings.pack);
}

#[test]
fn the_boot_group_loads_first() {
    let with_group = ResolvedManifest::load(plan_game(
        r#", "boot": {"logo": "hero", "bar": "walk_1", "font": "sans"}"#,
    ))
    .unwrap();
    let mut world = load_world();
    let mut target = HeadlessTarget::default();
    let plan = LoadPlan::new(with_group.clone(), &mut world);
    let assets = load_boot_group(plan.manifest(), &mut target, &mut TextureHandles::new()).unwrap();
    // Before any unit: the logo and the bar each on a reserved page of its
    // own, the bar cut from its sheet.
    assert_eq!(plan.done(), 0);
    let logo = assets.logo.unwrap();
    let bar = assets.bar.unwrap();
    assert_eq!(
        (logo.texture, logo.size),
        (TextureHandle(u32::MAX), [8, 12])
    );
    assert_eq!(
        (bar.texture, bar.size),
        (TextureHandle(u32::MAX - 1), [16, 16])
    );
    assert_eq!(assets.font.as_deref(), Some("sans"));
    assert_eq!(assets.pages(), [logo.texture, bar.texture]);
    assert_eq!(target.uploads, assets.pages());

    // The full load then registers and packs what a load without the group
    // does, under the counter's handles.
    let mut plan = plan;
    while plan.run_next_into(&mut world, &mut target).unwrap() {}
    let plain = ResolvedManifest::load(plan_game("")).unwrap();
    let mut plain_world = load_world();
    let mut plain_target = HeadlessTarget::default();
    let mut plain_plan = LoadPlan::new(plain.clone(), &mut plain_world);
    while plain_plan
        .run_next_into(&mut plain_world, &mut plain_target)
        .unwrap()
    {}
    // The two folders differ, so compare without the paths.
    assert_eq!(loaded(&world, &with_group), loaded(&plain_world, &plain));
    assert_eq!(target.uploads[2..], plain_target.uploads[..]);
}

#[test]
fn a_unit_that_fails_fails_the_plan() {
    let manifest_path = plan_game("");
    write(
        &manifest_path.with_file_name("run.json"),
        "{ not an animation",
    );
    let manifest = ResolvedManifest::load(&manifest_path).unwrap();
    let mut world = load_world();
    let mut target = HeadlessTarget::default();
    let mut plan = LoadPlan::new(manifest, &mut world);
    let err = loop {
        match plan.run_next_into(&mut world, &mut target) {
            Ok(true) => {}
            Ok(false) => panic!("the plan ended without the broken animation's error"),
            Err(err) => break err,
        }
    };
    assert!(
        matches!(&plan.units()[plan.done()], LoadUnit::Animation(id) if id == "run"),
        "the failing unit is not counted done: {:?}",
        plan.units()[plan.done()]
    );
    assert!(format!("{err:#}").contains("run.json"), "{err:#}");
}

/// Two sounds loaded from a manifest at `dir`, as a window's load leaves
/// them: `step` at 0.8 on effects, `hum` looping on music with 2 voices.
fn sound_world(dir: &Path) -> World {
    write_wav(&dir.join("step.wav"), 400);
    write_wav(&dir.join("hum.wav"), 900);
    let manifest = dir.join("sounds.json");
    write(
        &manifest,
        r#"{"sounds": {"step": {"path": "step.wav", "volume": 0.8},
            "hum": {"path": "hum.wav", "looping": true, "bus": "music", "max_voices": 2}}}"#,
    );
    let mut world = load_world();
    load_sounds(&ResolvedManifest::load(&manifest).unwrap(), &mut world).unwrap();
    world
}

#[test]
fn a_manifest_reload_adds_sounds_and_refreshes_their_defaults() {
    let dir = tempdir();
    let mut world = sound_world(&dir);
    let sounds = world.resource::<SoundRegistry>();
    let (step, hum) = (
        sounds.get_by_id("step").unwrap(),
        sounds.get_by_id("hum").unwrap(),
    );
    let revision = sounds.revision();
    write_wav(&dir.join("chime.wav"), 300);
    let edited = dir.join("edited.json");
    write(
        &edited,
        r#"{"sounds": {"step": {"path": "step.wav", "volume": 0.5, "max_voices": 4},
            "hum": {"path": "hum.wav", "looping": false, "bus": "effects"},
            "chime": {"path": "chime.wav", "bus": "music"}}}"#,
    );
    let removed =
        reload::reload_manifest_sounds(&ResolvedManifest::load(&edited).unwrap(), &mut world);
    assert!(removed.is_empty());
    let sounds = world.resource::<SoundRegistry>();
    // The kept sounds keep their handles and take their entries.
    assert_eq!(sounds.get_by_id("step"), Some(step));
    assert_eq!(
        (sounds.get_volume(step), sounds.get_max_voices(step)),
        (0.5, 4)
    );
    assert_eq!(sounds.get_by_id("hum"), Some(hum));
    assert!(!sounds.get_looping(hum));
    assert_eq!(sounds.get_bus(hum), tungsten_core::AudioBus::Effects);
    assert_eq!(sounds.get_max_voices(hum), 64);
    // The new one registers, on its path, with its entry.
    let chime = sounds.get_by_id("chime").expect("a new sound registers");
    assert_eq!(sounds.get(chime).unwrap().samples.len(), 300);
    assert_eq!(sounds.get_bus(chime), tungsten_core::AudioBus::Music);
    let chime_path = dir.join("chime.wav").canonicalize().unwrap();
    assert_eq!(sounds.id_for_path(&chime_path), Some("chime"));
    assert!(sounds.revision() > revision, "the app restarts the audio");

    // An edit to a sound's file decodes it again under its handle.
    write_wav(&dir.join("step.wav"), 1_000);
    let step_path = dir.join("step.wav").canonicalize().unwrap();
    assert!(reload::reload_sound(&step_path, &mut world));
    let sounds = world.resource::<SoundRegistry>();
    assert_eq!(sounds.get(step).unwrap().samples.len(), 1_000);
    assert_eq!(sounds.get_volume(step), 0.5);
    assert!(!reload::reload_sound(&dir.join("none.wav"), &mut world));
}

#[test]
fn a_removed_sound_stays_and_warns() {
    let dir = tempdir();
    let mut world = sound_world(&dir);
    let hum = world.resource::<SoundRegistry>().get_by_id("hum").unwrap();
    let edited = dir.join("edited.json");
    write(
        &edited,
        r#"{"sounds": {"step": {"path": "step.wav", "volume": 0.8}}}"#,
    );
    let removed =
        reload::reload_manifest_sounds(&ResolvedManifest::load(&edited).unwrap(), &mut world);
    assert_eq!(removed, ["hum"], "warned once, by ID");
    let sounds = world.resource::<SoundRegistry>();
    assert_eq!(
        sounds.get_by_id("hum"),
        Some(hum),
        "it stays, as a removed sprite does"
    );
    assert_eq!(sounds.get(hum).unwrap().samples.len(), 900);
    assert!(sounds.get_looping(hum));
}

#[test]
fn an_unknown_frame_sound_fails_the_load() {
    let manifest_path = plan_game("");
    write(
        &manifest_path.with_file_name("run.json"),
        r#"{"looping": true, "frames": [
            {"sprite": "walk_0", "duration_ms": 100, "sound": "step"},
            {"sprite": "walk_3", "duration_ms": 100, "sound": "squeak"}
        ]}"#,
    );
    let manifest = ResolvedManifest::load(&manifest_path).unwrap();
    let mut world = load_world();
    let mut target = HeadlessTarget::default();
    let mut plan = LoadPlan::new(manifest, &mut world);
    let err = loop {
        match plan.run_next_into(&mut world, &mut target) {
            Ok(true) => {}
            Ok(false) => panic!("the load took a frame naming an unknown sound"),
            Err(err) => break err,
        }
    };
    assert_eq!(
        plan.units()[plan.done()],
        LoadUnit::CrossChecks,
        "checked once the sounds load"
    );
    assert!(
        err.to_string()
            .contains("frame 1 references unknown sound ID 'squeak'"),
        "{err}"
    );

    // A reload naming an unknown sound keeps the previous clip.
    let path = manifest_path.with_file_name("idle.json");
    write(
        &path,
        r#"{"looping": false, "frames": [{"sprite": "hero", "duration_ms": 50, "sound": "nope"}]}"#,
    );
    let before = world
        .resource::<AnimationRegistry>()
        .get("idle")
        .unwrap()
        .frames
        .len();
    reload::reload_animation("idle", &path, &mut world).unwrap();
    let idle = world.resource::<AnimationRegistry>().get("idle").unwrap();
    assert_eq!(idle.frames.len(), before);
    assert!(idle.frames[0].sound.is_none());
}
