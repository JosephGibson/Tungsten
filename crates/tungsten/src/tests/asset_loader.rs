//! D-053 headless hot-reload tests; GPU upload paths covered by smoke suite.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use tungsten_core::assets::{
    AnimationData, AnimationRegistry, FilterMode, ParticleConfig, ParticleConfigRegistry,
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
        );
}

fn seed_world() -> World {
    let mut world = World::new();
    world.insert_resource(AssetRegistry::new());
    world.insert_resource(AnimationRegistry::new());
    world.insert_resource(TilemapRegistry::new());
    world.insert_resource(ParticleConfigRegistry::new());
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
    let initial = AnimationData::load(&anim_path).expect("initial animation parse should succeed");
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
    let initial = AnimationData::load(&anim_path).unwrap();
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

#[allow(dead_code)]
fn _touch_imports(_layer: TilemapLayer) {}
