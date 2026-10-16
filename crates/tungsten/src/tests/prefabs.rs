//! Prefab loading, hot reload and the manifest reload's prefab block (M51);
//! the `particle_emitter` and `tilemap` components through prefab files,
//! scene files and the harness.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use glam::Vec2;
use tungsten_core::assets::{
    BlendMode, EmissionKind, FilterMode, InitialVelocity, ParticleConfig, ParticleConfigRegistry,
    ParticleRender, PrefabError, Range, ResolvedPrefab, ResolvedTilemap, TilemapData,
    TilemapInstance, UvRect,
};
use tungsten_core::{
    AssetRegistry, CommandBuffer, Config, Entity, LayerKind, Particle, ParticleEmitter,
    PrefabRegistry, Tag, TextureHandle, Transform, With,
};

use super::*;
use crate::App;
use crate::asset_loader::{load_scene, reload_tilemap};
use crate::state::{GameState, SceneEntity, StateContext, StateId, StateStack};
use crate::testing::Harness;

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn tempdir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_prefabs_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

/// A burst of eight sparks, once.
fn burst_config() -> ParticleConfig {
    ParticleConfig {
        sprite: "spark".into(),
        render: ParticleRender::Quad,
        max_alive: 64,
        seed: Some(42),
        blend: BlendMode::Alpha,
        emission: EmissionKind::Burst {
            count: 8,
            once: true,
        },
        lifetime: Range { min: 0.5, max: 0.5 },
        initial_velocity: InitialVelocity::Radial {
            speed: Range::single(10.0),
        },
        gravity: [0.0, 0.0],
        drag_per_sec: 0.0,
        angular_velocity: Range::single(0.0),
        start_scale: Range::single(1.0),
        scale_over_life: None,
        color_over_life: None,
        alpha_over_life: None,
        tint: [1.0, 1.0, 1.0, 1.0],
    }
}

/// An app's World with the particle config `embers` and the sprite
/// `ground`, registered.
fn app_with_assets() -> App {
    let mut app = App::new(Config::default()).unwrap();
    let world = app.world_mut();
    world.resource_mut::<ParticleConfigRegistry>().register(
        "embers".into(),
        PathBuf::from("/tmp/embers.json"),
        Arc::new(burst_config()),
    );
    world.resource_mut::<AssetRegistry>().register_sprite(
        "ground".to_string(),
        FilterMode::Nearest,
        16,
        16,
        PathBuf::from("__test__/ground.png"),
        TextureHandle(0),
        UvRect::FULL,
        None,
        None,
        None,
    );
    app
}

/// A 2 × 1 map of `ground` tiles with `objects` in an object layer `things`.
fn tmj(objects: &str) -> String {
    format!(
        r#"{{
            "tilewidth": 16, "tileheight": 16, "width": 2, "height": 1,
            "tilesets": [{{ "firstgid": 1, "tiles": [
                {{ "id": 0, "properties": [{{ "name": "sprite_id", "value": "ground" }}] }}
            ] }}],
            "layers": [
                {{ "type": "tilelayer", "name": "ground", "data": [1, 1] }},
                {{ "type": "objectgroup", "name": "things", "objects": [{objects}] }}
            ]
        }}"#
    )
}

/// A manifest of `prefabs` and `tilemaps`, each `(id, path)`.
fn manifest(prefabs: &[(&str, &Path)], tilemaps: &[(&str, &Path)]) -> ResolvedManifest {
    let mut manifest = ResolvedManifest::default();
    for (id, path) in prefabs {
        manifest
            .prefabs
            .insert((*id).to_string(), ResolvedPrefab::new(path.to_path_buf()));
    }
    for (id, path) in tilemaps {
        manifest.tilemaps.insert(
            (*id).to_string(),
            ResolvedTilemap {
                path: path.to_path_buf(),
            },
        );
    }
    manifest
}

#[test]
fn load_prefabs_loads_each_prefab_with_its_path() {
    let dir = tempdir();
    let (bat, torch) = (dir.join("prefabs/bat.json"), dir.join("prefabs/torch.json"));
    write(&bat, r#"{ "tag": "bat" }"#);
    write(
        &torch,
        r#"{ "components": { "particle_emitter": { "config": "embers", "seed": 7 } } }"#,
    );
    let mut app = app_with_assets();
    let world = app.world_mut();
    load_prefabs(&manifest(&[("torch", &torch), ("bat", &bat)], &[]), world).unwrap();
    let prefabs = world.resource::<PrefabRegistry>();
    assert_eq!(prefabs.ids().collect::<Vec<_>>(), ["bat", "torch"]);
    assert_eq!(prefabs.get("bat").unwrap().tag.as_deref(), Some("bat"));
    assert_eq!(prefabs.id_for_path(&torch), Some("torch"));

    let entity = world.spawn_prefab("torch", [4.0, 2.0]).unwrap();
    let emitter = world.get::<ParticleEmitter>(entity).unwrap();
    let embers = world
        .resource::<ParticleConfigRegistry>()
        .id_for_name("embers")
        .unwrap();
    assert_eq!((emitter.config, emitter.seed_override), (embers, Some(7)));
    assert!(world.has::<tungsten_core::ParticleEmitterState>(entity));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_malformed_prefab_fails_the_load_naming_its_id_file_and_field() {
    let dir = tempdir();
    let torch = dir.join("prefabs/torch.json");
    write(
        &torch,
        r#"{ "components": { "particle_emitter": { "config": "embers", "sed": 7 } } }"#,
    );
    let mut app = app_with_assets();
    let err = load_prefabs(&manifest(&[("torch", &torch)], &[]), app.world_mut())
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("Failed to load prefab 'torch': "), "{err}");
    assert!(err.contains(&torch.display().to_string()), "{err}");
    assert!(
        err.contains("component `particle_emitter`, field `sed`: unknown field `sed`"),
        "{err}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_prefab_naming_a_missing_particle_config_fails_its_load() {
    let dir = tempdir();
    let smoke = dir.join("prefabs/smoke.json");
    write(
        &smoke,
        r#"{ "components": { "particle_emitter": { "config": "smoke" } } }"#,
    );
    let mut app = app_with_assets();
    let err = load_prefabs(&manifest(&[("smoke", &smoke)], &[]), app.world_mut())
        .unwrap_err()
        .to_string();
    assert!(
        err.ends_with(
            "component `particle_emitter`, field `config`: unknown particle config `smoke`"
        ),
        "{err}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn reload_prefab_replaces_it_and_a_bad_edit_keeps_the_previous() {
    let dir = tempdir();
    let bat = dir.join("prefabs/bat.json");
    write(&bat, r#"{ "tag": "bat" }"#);
    let mut app = app_with_assets();
    let world = app.world_mut();
    load_prefabs(&manifest(&[("bat", &bat)], &[]), world).unwrap();
    let live = world.spawn_prefab("bat", [0.0, 0.0]).unwrap();

    write(&bat, r#"{ "tag": "vampire" }"#);
    reload_prefab("bat", &bat, world).unwrap();
    let tag = |world: &World| {
        world
            .resource::<PrefabRegistry>()
            .get("bat")
            .unwrap()
            .tag
            .clone()
    };
    assert_eq!(tag(world).as_deref(), Some("vampire"));
    assert_eq!(
        world.get::<tungsten_core::Tag>(live).unwrap().name,
        "bat",
        "a live entity stays as it was"
    );

    write(&bat, r#"{ "tag": "vampire", "wings": 2 }"#);
    reload_prefab("bat", &bat, world).unwrap();
    assert_eq!(
        tag(world).as_deref(),
        Some("vampire"),
        "the bad edit is refused"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_manifest_reload_adds_maps_and_prefabs_that_name_each_other() {
    // One edit adds the map `arena`, the prefab `arena_map` whose `tilemap`
    // names it, and the map `cave` whose object's class names `arena_map`.
    let dir = tempdir();
    let (arena, cave) = (dir.join("maps/arena.tmj"), dir.join("maps/cave.tmj"));
    write(&arena, &tmj(""));
    write(
        &cave,
        &tmj(r#"{ "id": 1, "type": "arena_map", "x": 32, "y": 0 }"#),
    );
    let arena_map = dir.join("prefabs/arena_map.json");
    write(
        &arena_map,
        r#"{ "components": { "tilemap": { "id": "arena" } } }"#,
    );
    let mut app = app_with_assets();
    let world = app.world_mut();
    let edit = manifest(
        &[("arena_map", &arena_map)],
        &[("arena", &arena), ("cave", &cave)],
    );

    let mut new_maps = super::super::reload::reload_new_tilemaps(&edit, world);
    new_maps.sort();
    assert_eq!(new_maps, ["arena", "cave"]);
    reload_new_prefabs(&edit, world, &new_maps);
    assert!(
        world
            .resource::<PrefabRegistry>()
            .get("arena_map")
            .is_some()
    );
    assert!(check_all_map_objects(world).is_ok());

    let spawned = world.spawn_map_objects("cave", [0.0, 100.0]).unwrap();
    let instance = world.get::<TilemapInstance>(spawned[0]).unwrap();
    assert_eq!(
        (instance.id.as_str(), instance.origin),
        ("arena", Vec2::new(32.0, 100.0))
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_new_map_whose_objects_fail_keeps_its_tiles_and_spawn_returns_the_error() {
    let dir = tempdir();
    let cave = dir.join("maps/cave.tmj");
    write(
        &cave,
        &tmj(r#"{ "id": 4, "type": "ghost", "x": 0, "y": 0 }"#),
    );
    let mut app = app_with_assets();
    let world = app.world_mut();
    let edit = manifest(&[], &[("cave", &cave)]);
    let new_maps = super::super::reload::reload_new_tilemaps(&edit, world);
    reload_new_prefabs(&edit, world, &new_maps);

    let maps = world.resource::<tungsten_core::TilemapRegistry>();
    assert_eq!(
        maps.get("cave").unwrap().layers.len(),
        1,
        "the tiles loaded"
    );
    let err = world.spawn_map_objects("cave", Vec2::ZERO).unwrap_err();
    assert!(
        matches!(err, PrefabError::Object { object: 4, .. }),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "tilemap `cave`, layer `things`, object 4: unknown prefab `ghost`"
    );
    assert_eq!(
        check_all_map_objects(world).unwrap_err().to_string(),
        err.to_string(),
        "a startup load fails on the same map"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn reload_tilemap_keeps_the_last_good_map_on_a_bad_object() {
    let dir = tempdir();
    let (bat, cave) = (dir.join("prefabs/bat.json"), dir.join("maps/cave.tmj"));
    write(&bat, r#"{ "tag": "bat" }"#);
    write(&cave, &tmj(r#"{ "id": 1, "type": "bat", "x": 0, "y": 0 }"#));
    let mut app = app_with_assets();
    let world = app.world_mut();
    load_prefabs(&manifest(&[("bat", &bat)], &[]), world).unwrap();
    reload_tilemap("cave", &cave, world).unwrap();
    let class = |world: &World| {
        world
            .resource::<tungsten_core::TilemapRegistry>()
            .get("cave")
            .unwrap()
            .object_layers[0]
            .objects[0]
            .class
            .clone()
    };
    assert_eq!(class(world), "bat");

    write(
        &cave,
        &tmj(r#"{ "id": 1, "type": "ghost", "x": 0, "y": 0 }"#),
    );
    reload_tilemap("cave", &cave, world).unwrap();
    assert_eq!(
        class(world),
        "bat",
        "the map with the unknown class is refused"
    );
    write(&cave, &tmj(r#"{ "id": 1, "type": "bat", "x": 8, "y": 0 }"#));
    reload_tilemap("cave", &cave, world).unwrap();
    let maps = world.resource::<tungsten_core::TilemapRegistry>();
    assert_eq!(
        maps.get("cave").unwrap().object_layers[0].objects[0].position,
        Vec2::new(8.0, 0.0)
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_scene_naming_an_unknown_particle_config_fails_load_scene() {
    let dir = tempdir();
    let scene = dir.join("scene.json");
    write(
        &scene,
        r#"{ "entities": [ { "transform": { "position": [0.0, 0.0] },
            "components": { "particle_emitter": { "config": "smoke" } } } ] }"#,
    );
    let app = app_with_assets();
    let err = load_scene(app.world(), &scene).unwrap_err().to_string();
    assert!(
        err.ends_with(
            "entities[0]: component `particle_emitter`, field `config`: unknown particle config `smoke`"
        ),
        "{err}"
    );
    write(
        &scene,
        r#"{ "entities": [ { "transform": { "position": [0.0, 0.0] },
            "components": { "tilemap": { "id": "nowhere" } } } ] }"#,
    );
    let err = load_scene(app.world(), &scene).unwrap_err().to_string();
    assert!(
        err.ends_with("entities[0]: component `tilemap`, field `id`: unknown tilemap `nowhere`"),
        "{err}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_particle_emitter_prefab_emits_through_the_harness() {
    let dir = tempdir();
    let torch = dir.join("prefabs/torch.json");
    write(
        &torch,
        r#"{ "components": { "particle_emitter": { "config": "embers" } } }"#,
    );
    let mut app = app_with_assets();
    load_prefabs(&manifest(&[("torch", &torch)], &[]), app.world_mut()).unwrap();
    app.world_mut().spawn_prefab("torch", [10.0, 20.0]).unwrap();
    let mut harness = Harness::new(app);
    harness.set_dt(0.05);
    harness.step(1);
    let particles = harness
        .world()
        .query_filtered::<Entity, With<Particle>>()
        .count();
    assert_eq!(particles, 8, "the burst config's count");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_tilemap_prefab_draws_its_tiles_through_the_harness() {
    let dir = tempdir();
    let arena_map = dir.join("prefabs/arena_map.json");
    write(
        &arena_map,
        r#"{ "components": { "tilemap": { "id": "arena" } } }"#,
    );
    let mut map = TilemapData::new(16, 16, 3, 2);
    let ground = map.add_tile("ground");
    let layer = map.add_layer("ground", LayerKind::Render);
    map.fill(layer, ground);
    map.set_tile(layer, 1, 1, -1);

    // What the map draws from a prefab and from a `TilemapInstance` at the
    // same origin.
    let draw = |by_prefab: bool| {
        let mut app = app_with_assets();
        let world = app.world_mut();
        world
            .resource_mut::<tungsten_core::TilemapRegistry>()
            .insert("arena".to_string(), map.clone());
        load_prefabs(&manifest(&[("arena_map", &arena_map)], &[]), world).unwrap();
        if by_prefab {
            world.spawn_prefab("arena_map", [32.0, 48.0]).unwrap();
        } else {
            world.spawn_with((TilemapInstance::new("arena", Vec2::new(32.0, 48.0)),));
        }
        let mut harness = Harness::new(app);
        harness.step(1);
        harness
            .draw()
            .unwrap()
            .sprites
            .iter()
            .flat_map(|batch| batch.instances.iter().map(|i| (i.position, i.size)))
            .collect::<Vec<_>>()
    };
    let drawn = draw(true);
    assert_eq!(drawn.len(), 5, "six cells, one cleared");
    assert_eq!(drawn, draw(false));
    fs::remove_dir_all(&dir).unwrap();
}

/// A state with one entity of its own, whose `on_exit` queues prefab
/// `spark` from that entity.
struct Sparkler;

impl GameState for Sparkler {
    fn id(&self) -> StateId {
        "sparkler"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        ctx.world.spawn_with((
            Transform::default(),
            SceneEntity {
                state_id: ctx.state_id,
            },
        ));
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        let owner = ctx
            .world
            .query::<(Entity, &SceneEntity)>()
            .find(|(_, marker)| marker.state_id == ctx.state_id)
            .map(|(entity, _)| entity)
            .unwrap();
        let prepared = ctx.world.prepare_prefab("spark", [0.0, 0.0]).unwrap();
        ctx.world
            .resource_mut::<CommandBuffer>()
            .spawn_prefab_from(owner, prepared);
    }

    fn update(&mut self, _world: &mut World) {}
}

#[test]
fn a_spawn_queued_in_on_exit_from_a_states_entity_leaves_nothing() {
    let dir = tempdir();
    let spark = dir.join("prefabs/spark.json");
    write(&spark, r#"{ "tag": "spark" }"#);
    let mut app = app_with_assets();
    load_prefabs(&manifest(&[("spark", &spark)], &[]), app.world_mut()).unwrap();
    let mut harness = Harness::new(app);
    harness.step(1);
    let before = harness.world().entity_count();
    let request = |harness: &mut Harness, push: bool| {
        let stack = harness.world_mut().resource_mut::<StateStack>();
        if push {
            stack.request_push(Sparkler);
        } else {
            stack.request_pop();
        }
    };
    request(&mut harness, true);
    harness.step(1);
    assert_eq!(harness.world().entity_count(), before + 1, "its entity");
    request(&mut harness, false);
    harness.step(1);
    let sparkler = |marker: &&SceneEntity| marker.state_id == "sparkler";
    assert_eq!(
        harness
            .world()
            .query::<&SceneEntity>()
            .filter(sparkler)
            .count(),
        0
    );
    let sparks = harness
        .world()
        .query::<&Tag>()
        .filter(|tag| tag.name == "spark")
        .count();
    assert_eq!(sparks, 0, "the spawn followed its source's despawn");
    assert_eq!(harness.world().entity_count(), before);
    fs::remove_dir_all(&dir).unwrap();
}
