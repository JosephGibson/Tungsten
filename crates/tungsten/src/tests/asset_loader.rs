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
use tungsten_core::ecs::World;
use tungsten_core::{AssetRegistry, TextureHandle};

use super::*;

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
            "blend": "premultiplied",
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
    let spawned: Vec<_> = world.query::<Sprite>().map(|(_, s)| s.asset_id).collect();
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
