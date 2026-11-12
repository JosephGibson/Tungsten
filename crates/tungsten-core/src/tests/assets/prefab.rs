//! Prefab files, the registry, spawning live and queued, and a map's
//! objects with their properties over the prefabs' components.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;

use super::*;
use crate::assets::data::{StagedLoad, StagedLoadError};
use crate::assets::tilemap::{LayerKind, MapObject, ObjectLayer};
use crate::component_registry::MissingAsset;
use crate::physics::{BodyKind, Collider, PhysicsPlugin, RigidBody, Shape};
use crate::schedule::{Plugin, Schedule};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Health {
    max: u32,
}

/// Unknown fields pass: the type denies none.
#[derive(Debug, Deserialize, PartialEq)]
struct Loose {
    #[allow(dead_code)]
    a: u32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Player;

/// A component that names an asset: `config`, checked against `Configs`.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Emitter {
    config: String,
}

/// A component holding an array a property can index.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Route {
    points: Vec<[f32; 2]>,
}

/// The particle configs a test World holds, by ID.
struct Configs(Vec<String>);

/// A component that spawns `Visibility`, to see the order of the run.
#[derive(Deserialize)]
struct HiddenDef;

/// A World with the asset registry, the test components, `rigid_body`, and
/// the configs `embers`.
fn world() -> World {
    let mut world = World::new();
    world.insert_resource(AssetRegistry::new());
    world.insert_resource(Configs(vec!["embers".to_owned()]));
    world.register_component::<Health>("health");
    world.register_component::<Loose>("loose");
    world.register_component::<Player>("player");
    world.register_component::<Route>("route");
    world.register_component_with("hidden", |_: HiddenDef, _: &SpawnContext<'_>| {
        (Visibility { visible: false },)
    });
    world.register_component_with_assets(
        "emitter",
        |emitter: &Emitter, world: &World| {
            if world.resource::<Configs>().0.contains(&emitter.config) {
                Ok(())
            } else {
                Err(MissingAsset::new(
                    "config",
                    "particle config",
                    &emitter.config,
                ))
            }
        },
        |emitter: Emitter, _: &SpawnContext<'_>| (emitter,),
    );
    PhysicsPlugin.build(&mut Schedule::new(), &mut world);
    world
}

fn prefab(value: serde_json::Value) -> PrefabData {
    serde_json::from_value(value).expect("a valid prefab")
}

/// `world` with `prefabs` registered.
fn with_prefabs(mut world: World, prefabs: &[(&str, serde_json::Value)]) -> World {
    let mut registry = PrefabRegistry::new();
    for (id, value) in prefabs {
        registry.insert((*id).to_owned(), prefab(value.clone()));
    }
    world.insert_resource(registry);
    world
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A prefab file in a temp directory of its own, removed on drop.
struct PrefabFile {
    dir: PathBuf,
    path: PathBuf,
}

impl PrefabFile {
    fn new(contents: Option<&str>) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tungsten_prefab_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("thing.json");
        if let Some(contents) = contents {
            std::fs::write(&path, contents).unwrap();
        }
        Self { dir, path }
    }
}

impl Drop for PrefabFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Io,
    Parse,
    Validation,
}

#[test]
fn prefab_file_errors() {
    let cases: [(&str, Option<&str>, Kind, &str); 15] = [
        (
            "the file does not read",
            None,
            Kind::Io,
            "failed to read prefab",
        ),
        ("not JSON", Some("{ \"tag\": "), Kind::Parse, "EOF"),
        (
            "an unknown key at the top",
            Some(r#"{ "sprit": { "asset_id": "a" } }"#),
            Kind::Parse,
            "unknown field `sprit`",
        ),
        (
            "an unknown key in the sprite",
            Some(r#"{ "sprite": { "asset_id": "a", "colour": [0, 0, 0, 0] } }"#),
            Kind::Parse,
            "unknown field `colour`",
        ),
        (
            "an unknown key in a tween",
            Some(r#"{ "tweens": [ { "duration": 1.0, "loop": true, "channels": [] } ] }"#),
            Kind::Parse,
            "unknown field `loop`",
        ),
        (
            "a component named twice",
            Some(r#"{ "components": { "health": { "max": 1 }, "health": { "max": 2 } } }"#),
            Kind::Parse,
            "duplicate component `health`",
        ),
        (
            "a key repeated inside a value",
            Some(r#"{ "components": { "health": { "max": "ten", "max": 10 } } }"#),
            Kind::Parse,
            "duplicate field `max`",
        ),
        (
            "two tweens",
            Some(
                r#"{ "tweens": [
                    { "duration": 1.0, "channels": [ { "kind": "rotation", "from": 0.0, "to": 1.0 } ] },
                    { "duration": 1.0, "channels": [ { "kind": "rotation", "from": 0.0, "to": 1.0 } ] }
                ] }"#,
            ),
            Kind::Validation,
            "tweens: a prefab holds at most one tween (got 2)",
        ),
        (
            "a tween that fails its checks",
            Some(
                r#"{ "tweens": [ { "duration": 0.0, "channels": [ { "kind": "rotation", "from": 0.0, "to": 1.0 } ] } ] }"#,
            ),
            Kind::Validation,
            "tweens[0]: tween duration must be finite and > 0",
        ),
        (
            "an unknown component",
            Some(r#"{ "components": { "helth": { "max": 1 } } }"#),
            Kind::Validation,
            "unknown component `helth`",
        ),
        (
            "a value of the wrong type",
            Some(r#"{ "components": { "health": { "max": "ten" } } }"#),
            Kind::Validation,
            "component `health`, field `max`: invalid type: string \"ten\", expected u32",
        ),
        (
            "an unknown field under deny_unknown_fields",
            Some(r#"{ "components": { "health": { "max": 1, "regen": 2 } } }"#),
            Kind::Validation,
            "component `health`, field `regen`: unknown field `regen`",
        ),
        (
            "an asset ID the World lacks",
            Some(r#"{ "components": { "emitter": { "config": "smoke" } } }"#),
            Kind::Validation,
            "component `emitter`, field `config`: unknown particle config `smoke`",
        ),
        (
            "a rigid_body field its type denies",
            Some(
                r#"{ "components": { "rigid_body": { "kind": "dynamic", "shape": { "kind": "circle", "radius": 4 }, "spin": 1 } } }"#,
            ),
            Kind::Validation,
            "component `rigid_body`, field `spin`: unknown field `spin`",
        ),
        (
            "a sprite without its asset_id",
            Some(r#"{ "sprite": { "z_order": 1 } }"#),
            Kind::Parse,
            "missing field `asset_id`",
        ),
    ];
    let world = world();
    for (case, contents, kind, needle) in cases {
        let file = PrefabFile::new(contents);
        let err = PrefabData::load(&file.path, &world).expect_err(case);
        let got = match &err {
            PrefabError::Io { .. } => Kind::Io,
            PrefabError::Parse { .. } => Kind::Parse,
            PrefabError::Validation { .. } => Kind::Validation,
            other => panic!("{case}: unexpected {other:?}"),
        };
        assert_eq!(got, kind, "{case}: {err}");
        let message = err.to_string();
        assert!(
            message.contains(&file.path.display().to_string()),
            "{case}: {message} must name the file"
        );
        assert!(message.contains(needle), "{case}: {message}");
    }
}

#[test]
fn the_rules_kept_from_scenes_pass_the_load() {
    let world = world();
    // A tween channel accepts unknown keys, and a type without
    // `deny_unknown_fields` ignores an unknown field.
    let file = PrefabFile::new(Some(
        r#"{
            "sprite": { "asset_id": "slime", "z_order": 2 },
            "visible": false,
            "tag": "enemy",
            "tweens": [ { "duration": 0.5, "repeat": "loop",
                "channels": [ { "kind": "rotation", "from": 0.0, "to": 1.0, "ease": 3 } ] } ],
            "components": {
                "player": null,
                "loose": { "a": 1, "b": 2 },
                "emitter": { "config": "embers" }
            }
        }"#,
    ));
    let data = PrefabData::load(&file.path, &world).unwrap();
    assert_eq!(data.sprite.as_ref().unwrap().asset_id, "slime");
    assert_eq!(data.sprite.as_ref().unwrap().z_order, 2);
    assert!(!data.visible);
    assert_eq!(data.tag.as_deref(), Some("enemy"));
    assert_eq!(data.tweens.len(), 1);
    assert_eq!(
        data.components.keys().collect::<Vec<_>>(),
        ["emitter", "loose", "player"]
    );
    let empty = prefab(json!({}));
    assert!(empty.visible, "visible defaults to true");
    assert!(empty.sprite.is_none() && empty.tag.is_none() && empty.components.is_empty());
}

#[test]
fn the_registry_sorts_ids_replaces_and_maps_paths() {
    let mut registry = PrefabRegistry::new();
    assert!(registry.is_empty());
    registry.insert("slime".to_owned(), prefab(json!({ "tag": "a" })));
    registry.insert_with_path(
        "bat".to_owned(),
        prefab(json!({})),
        PathBuf::from("/game/prefabs/bat.json"),
    );
    registry.insert("slime".to_owned(), prefab(json!({ "tag": "b" })));
    assert_eq!(registry.len(), 2);
    assert_eq!(registry.ids().collect::<Vec<_>>(), ["bat", "slime"]);
    assert_eq!(registry.get("slime").unwrap().tag.as_deref(), Some("b"));
    assert_eq!(
        registry.id_for_path(Path::new("/game/prefabs/bat.json")),
        Some("bat")
    );
    assert_eq!(registry.id_for_path(Path::new("/game/slime.json")), None);
    assert!(registry.get("rat").is_none());
}

#[test]
fn a_replaced_path_no_longer_routes() {
    let mut registry = PrefabRegistry::new();
    let (old, new) = (
        PathBuf::from("/game/bat.json"),
        PathBuf::from("/game/bat2.json"),
    );
    registry.insert_with_path("bat".to_owned(), prefab(json!({})), old.clone());
    registry.insert_with_path("bat".to_owned(), prefab(json!({})), new.clone());
    assert_eq!(
        registry.id_for_path(&old),
        None,
        "the moved file's old path"
    );
    assert_eq!(registry.id_for_path(&new), Some("bat"));
    // A prefab given a held file takes it; one replaced in code drops its own.
    registry.insert_with_path("rat".to_owned(), prefab(json!({})), new.clone());
    assert_eq!(registry.id_for_path(&new), Some("rat"));
    registry.insert("rat".to_owned(), prefab(json!({})));
    assert_eq!(registry.id_for_path(&new), None);
    assert_eq!(registry.len(), 2);
}

#[test]
fn a_validation_message_suggests_the_nearest_prefab_id() {
    #[derive(Deserialize)]
    struct Spawns {
        prefab: String,
    }
    let mut world = with_prefabs(world(), &[("slime", json!({})), ("slug", json!({}))]);
    world.register_component_with_assets(
        "spawns",
        |spawns: &Spawns, world: &World| world.check_prefab_id("prefab", &spawns.prefab),
        |spawns: Spawns, _: &SpawnContext<'_>| (spawns,),
    );
    let near = PrefabFile::new(Some(
        r#"{ "components": { "spawns": { "prefab": "slme" } } }"#,
    ));
    let err = PrefabData::load(&near.path, &world).unwrap_err();
    assert!(matches!(err, PrefabError::Validation { .. }));
    assert!(
        err.to_string().ends_with(
            "component `spawns`, field `prefab`: unknown prefab `slme`; did you mean `slime`?"
        ),
        "{err}"
    );
    let far = PrefabFile::new(Some(
        r#"{ "components": { "spawns": { "prefab": "dragon" } } }"#,
    ));
    let err = PrefabData::load(&far.path, &world).unwrap_err();
    assert!(
        err.to_string().ends_with("unknown prefab `dragon`"),
        "{err}"
    );
    let ok = PrefabFile::new(Some(
        r#"{ "components": { "spawns": { "prefab": "slug" } } }"#,
    ));
    PrefabData::load(&ok.path, &world).unwrap();
}

#[test]
fn a_spawn_gives_the_fields_then_the_components_then_with() {
    let mut world = with_prefabs(
        world(),
        &[(
            "slime",
            json!({
                "sprite": { "asset_id": "slime", "color": [1, 2, 3, 4], "z_order": 2 },
                "tag": "enemy",
                "tweens": [ { "duration": 1.0,
                    "channels": [ { "kind": "rotation", "from": 0.0, "to": 1.0 } ] } ],
                "components": { "health": { "max": 3 }, "hidden": null }
            }),
        )],
    );
    let entity = world.spawn_prefab("slime", [10.0, -4.0]).unwrap();
    let transform = world.get::<Transform>(entity).unwrap();
    assert_eq!(*transform, Transform::from_position(Vec2::new(10.0, -4.0)));
    let sprite = *world.get::<Sprite>(entity).unwrap();
    let assets = world.resource::<AssetRegistry>();
    assert_eq!(assets.sprite_name(sprite.asset_id), Some("slime"));
    assert_eq!((sprite.color, sprite.z_order), ([1, 2, 3, 4], 2));
    assert_eq!(world.get::<Tag>(entity).unwrap().name, "enemy");
    assert!(world.has::<Tween>(entity));
    assert_eq!(world.get::<Health>(entity), Some(&Health { max: 3 }));
    assert_eq!(
        world.get::<Visibility>(entity),
        Some(&Visibility { visible: false }),
        "a component wins over the prefab's visible field"
    );

    let shown = world
        .prepare_prefab("slime", [0.0, 0.0])
        .unwrap()
        .with((Visibility { visible: true }, Player))
        .spawn(&mut world);
    assert_eq!(
        world.get::<Visibility>(shown),
        Some(&Visibility { visible: true }),
        "a `with` bundle wins over a component"
    );
    assert!(world.has::<Player>(shown));
    assert_eq!(world.entity_count(), 2);
}

#[test]
fn the_queued_form_records_one_spawn_and_one_run_for_the_flush() {
    let mut world = with_prefabs(
        world(),
        &[(
            "slime",
            json!({ "sprite": { "asset_id": "slime" }, "tag": "enemy",
                    "components": { "health": { "max": 3 }, "player": null } }),
        )],
    );
    let prepared = world
        .prepare_prefab("slime", Vec2::new(4.0, 8.0))
        .unwrap()
        .with((Route { points: Vec::new() },));
    assert_eq!(
        format!("{prepared:?}"),
        r#"PreparedPrefab { id: "slime", .. }"#
    );
    assert_eq!(prepared.id(), "slime");
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn_prefab(prepared);
    assert_eq!(world.entity_count(), 0, "nothing spawns before the flush");
    // The spawn, then Transform, Sprite, Visibility, Tag, two components and
    // the `with` bundle on one target, back to back: one run.
    assert_eq!(buffer.len(), 8);
    assert_eq!(pending, PendingEntity { index: 0, epoch: 0 });
    world.flush(buffer);
    let found: Vec<_> = world
        .query::<(&Transform, &Health, &Player, &Route, &Tag)>()
        .map(|(transform, health, _, _, tag)| (transform.position, health.max, tag.name.clone()))
        .collect();
    assert_eq!(found, [(Vec2::new(4.0, 8.0), 3, "enemy".to_owned())]);
}

#[test]
fn an_unknown_prefab_is_an_error() {
    let mut bare = World::new();
    let err = bare.spawn_prefab("slime", [0.0, 0.0]).unwrap_err();
    assert!(matches!(&err, PrefabError::Unknown { id } if id == "slime"));
    assert_eq!(err.to_string(), "unknown prefab `slime`");
    let mut world = with_prefabs(world(), &[("bat", json!({}))]);
    assert!(matches!(
        world.prepare_prefab("slime", [0.0, 0.0]),
        Err(PrefabError::Unknown { .. })
    ));
}

#[test]
fn a_vanished_asset_fails_the_spawn_and_leaves_the_world_as_it_was() {
    let mut world = with_prefabs(
        world(),
        &[(
            "torch",
            json!({ "sprite": { "asset_id": "torch" },
                    "components": { "emitter": { "config": "embers" } } }),
        )],
    );
    world.resource_mut::<Configs>().0.clear();
    let err = world.spawn_prefab("torch", [0.0, 0.0]).unwrap_err();
    assert!(
        matches!(
            &err,
            PrefabError::Component { id, source: ComponentError::MissingAsset { .. } } if id == "torch"
        ),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "prefab `torch`: component `emitter`, field `config`: unknown particle config `embers`"
    );
    assert_eq!(world.entity_count(), 0);
    assert_eq!(
        world.resource::<AssetRegistry>().sprite_id("torch"),
        None,
        "the sprite was not interned"
    );
}

#[test]
fn a_position_that_is_not_finite_or_too_far_is_an_error() {
    let mut world = with_prefabs(world(), &[("bat", json!({}))]);
    for position in [
        Vec2::new(f32::INFINITY, 0.0),
        Vec2::new(0.0, f32::NAN),
        Vec2::new(16_777_218.0, 0.0),
        Vec2::new(0.0, -16_777_218.0),
    ] {
        let err = world.spawn_prefab("bat", position).unwrap_err();
        assert!(matches!(err, PrefabError::Position { .. }), "{err:?}");
        assert!(
            err.to_string()
                .ends_with("must be finite and within ±16777216"),
            "{err}"
        );
    }
    assert!(
        world
            .spawn_prefab("bat", [16_777_216.0, -16_777_216.0])
            .is_ok()
    );
    assert_eq!(
        world
            .spawn_prefab("bat", [1e40_f64 as f32, 0.0])
            .unwrap_err()
            .to_string(),
        "prefab `bat`: position [inf, 0] must be finite and within ±16777216"
    );
}

/// A 4 × 4 map of 16-pixel tiles holding `layers`, registered as `level`.
fn with_map(mut world: World, layers: Vec<ObjectLayer>) -> World {
    let mut map = TilemapData::new(16, 16, 4, 4);
    map.add_tile("ground");
    let ground = map.add_layer("ground", LayerKind::Render);
    map.fill(ground, 0);
    map.object_layers = layers;
    let mut maps = TilemapRegistry::new();
    maps.insert("level".to_owned(), map);
    world.insert_resource(maps);
    world
}

fn object(id: u32, class: &str, at: [f32; 2], properties: serde_json::Value) -> MapObject {
    MapObject {
        id,
        name: format!("object{id}"),
        class: class.to_owned(),
        position: Vec2::from(at),
        size: Vec2::ZERO,
        properties: serde_json::from_value(properties).unwrap(),
    }
}

fn layer(name: &str, objects: Vec<MapObject>) -> ObjectLayer {
    ObjectLayer {
        name: name.to_owned(),
        objects,
    }
}

#[test]
fn map_objects_spawn_their_prefabs_in_order_with_their_properties() {
    let world = with_prefabs(
        world(),
        &[
            (
                "slime",
                json!({ "tag": "slime", "components": { "health": { "max": 3 } } }),
            ),
            ("torch", json!({ "tag": "torch" })),
        ],
    );
    let mut world = with_map(
        world,
        vec![
            layer(
                "enemies",
                vec![
                    object(7, "slime", [16.0, 32.0], json!({ "health.max": 9 })),
                    object(2, "", [0.0, 0.0], json!({})),
                    object(3, "slime", [48.0, 0.0], json!({})),
                ],
            ),
            layer(
                "props",
                vec![object(
                    1,
                    "torch",
                    [8.0, 8.0],
                    json!({ "emitter.config": "embers" }),
                )],
            ),
        ],
    );
    world
        .check_map_objects(
            "level",
            world.resource::<TilemapRegistry>().get("level").unwrap(),
        )
        .unwrap();
    let entities = world.spawn_map_objects("level", [100.0, 200.0]).unwrap();
    assert_eq!(entities.len(), 3, "the marker spawns nothing");
    let spawned: Vec<_> = entities
        .iter()
        .map(|&e| {
            (
                world.get::<Tag>(e).unwrap().name.clone(),
                world.get::<Transform>(e).unwrap().position,
                world.get::<Health>(e).map(|health| health.max),
                world
                    .get::<Emitter>(e)
                    .map(|emitter| emitter.config.clone()),
            )
        })
        .collect();
    assert_eq!(
        spawned,
        [
            ("slime".to_owned(), Vec2::new(116.0, 232.0), Some(9), None),
            ("slime".to_owned(), Vec2::new(148.0, 200.0), Some(3), None),
            (
                "torch".to_owned(),
                Vec2::new(108.0, 208.0),
                None,
                Some("embers".to_owned())
            ),
        ],
        "layer and file order, the origin added, the properties over the prefab"
    );
    let prepared = world.prepare_map_objects("level", Vec2::ZERO).unwrap();
    assert_eq!(
        prepared.iter().map(PreparedPrefab::id).collect::<Vec<_>>(),
        ["slime", "slime", "torch"]
    );
}

#[test]
fn a_class_property_merges_and_keeps_the_prefabs_shape() {
    let world = with_prefabs(
        world(),
        &[(
            "crate",
            json!({ "components": { "rigid_body": {
                "kind": "dynamic", "shape": { "kind": "circle", "radius": 6 } } } }),
        )],
    );
    let mut world = with_map(
        world,
        vec![layer(
            "props",
            vec![
                object(
                    1,
                    "crate",
                    [0.0, 0.0],
                    json!({ "rigid_body": { "kind": "kinematic" } }),
                ),
                object(
                    2,
                    "crate",
                    [32.0, 0.0],
                    json!({ "rigid_body": { "kind": "static" }, "rigid_body.shape.radius": 2 }),
                ),
            ],
        )],
    );
    let entities = world.spawn_map_objects("level", Vec2::ZERO).unwrap();
    let bodies: Vec<_> = entities
        .iter()
        .map(|&e| {
            (
                world.get::<RigidBody>(e).unwrap().kind,
                world.get::<Collider>(e).unwrap().shape,
            )
        })
        .collect();
    assert_eq!(
        bodies,
        [
            (BodyKind::Kinematic, Shape::Circle { radius: 6.0 }),
            (BodyKind::Static, Shape::Circle { radius: 2.0 }),
        ]
    );
}

#[test]
fn a_shape_switch_fails_the_check_at_the_leftover_field() {
    let world = with_prefabs(
        world(),
        &[(
            "crate",
            json!({ "components": { "rigid_body": {
                "kind": "dynamic", "shape": { "kind": "circle", "radius": 6 } } } }),
        )],
    );
    let mut world = with_map(
        world,
        vec![layer(
            "props",
            vec![object(
                4,
                "crate",
                [0.0, 0.0],
                json!({ "rigid_body.shape": { "kind": "aabb", "half_extents": [4, 4] } }),
            )],
        )],
    );
    let map = world
        .resource::<TilemapRegistry>()
        .get("level")
        .unwrap()
        .clone();
    let err = world.check_map_objects("level", &map).unwrap_err();
    let message = err.to_string();
    assert!(
        message.starts_with(
            "tilemap `level`, layer `props`, object 4: prefab `crate`: component `rigid_body`"
        ),
        "{message}"
    );
    assert!(message.contains("`radius`"), "{message}");
    assert!(matches!(
        world.spawn_map_objects("level", Vec2::ZERO),
        Err(PrefabError::Object { object: 4, .. })
    ));
}

#[test]
fn each_object_rule_has_its_error() {
    let prefabs = [
        (
            "slime",
            json!({ "components": { "health": { "max": 3 }, "player": null } }),
        ),
        (
            "walker",
            json!({ "components": { "route": { "points": [[0, 0], [1, 1]] } } }),
        ),
    ];
    let cases: [(&str, serde_json::Value, &str); 8] = [
        ("ghost", json!({}), "unknown prefab `ghost`"),
        (
            "slime",
            json!({ "health": 4 }),
            "property `health`: a value that is not a class must name a field: `health.<field>`",
        ),
        (
            "slime",
            json!({ "health.max.x": 4 }),
            "property `health.max.x`: `health.max` is a number, which `x` cannot step into",
        ),
        (
            "slime",
            json!({ "player.x": 1 }),
            "property `player.x`: `player` is null, which `x` cannot step into",
        ),
        (
            "slime",
            json!({ "health..max": 1 }),
            "property `health..max`: a path segment is empty",
        ),
        (
            "walker",
            json!({ "route.points.5": [2, 2] }),
            "property `route.points.5`: `route.points` has no element 5 (it has 2)",
        ),
        (
            "walker",
            json!({ "route.points.first": [2, 2] }),
            "property `route.points.first`: `route.points` is an array, which `first` cannot step into",
        ),
        (
            "slime",
            json!({ "health.max": "ten" }),
            "prefab `slime`: component `health`, field `max`: invalid type: string \"ten\", expected u32",
        ),
    ];
    for (class, properties, expected) in cases {
        let mut world = with_map(
            with_prefabs(world(), &prefabs),
            vec![layer(
                "things",
                vec![object(5, class, [0.0, 0.0], properties.clone())],
            )],
        );
        let err = world.spawn_map_objects("level", Vec2::ZERO).unwrap_err();
        assert!(
            matches!(&err, PrefabError::Object { map, layer, object: 5, .. } if map == "level" && layer == "things"),
            "{err:?}"
        );
        assert_eq!(
            err.to_string(),
            format!("tilemap `level`, layer `things`, object 5: {expected}"),
            "{properties}"
        );
        let map = world
            .resource::<TilemapRegistry>()
            .get("level")
            .unwrap()
            .clone();
        assert_eq!(
            world
                .check_map_objects("level", &map)
                .unwrap_err()
                .to_string(),
            err.to_string(),
            "the check agrees with the spawn"
        );
    }

    // An existing element takes its replacement.
    let mut world = with_map(
        with_prefabs(world(), &prefabs),
        vec![layer(
            "things",
            vec![object(
                6,
                "walker",
                [0.0, 0.0],
                json!({ "route.points.1": [5, 6] }),
            )],
        )],
    );
    let walker = world.spawn_map_objects("level", Vec2::ZERO).unwrap()[0];
    assert_eq!(
        world.get::<Route>(walker).unwrap().points,
        [[0.0, 0.0], [5.0, 6.0]]
    );
}

#[test]
fn map_objects_prepare_all_or_nothing_and_name_an_unknown_map() {
    let world = with_prefabs(
        world(),
        &[
            (
                "torch",
                json!({ "sprite": { "asset_id": "torch" }, "tag": "torch" }),
            ),
            ("slime", json!({ "components": { "health": { "max": 3 } } })),
        ],
    );
    let mut world = with_map(
        world,
        vec![layer(
            "things",
            vec![
                object(1, "torch", [0.0, 0.0], json!({})),
                object(2, "slime", [0.0, 0.0], json!({ "health.max": -1 })),
            ],
        )],
    );
    assert!(matches!(
        world.prepare_map_objects("level", Vec2::ZERO),
        Err(PrefabError::Object { object: 2, .. })
    ));
    assert_eq!(world.entity_count(), 0);
    assert_eq!(world.resource::<AssetRegistry>().sprite_id("torch"), None);

    let err = world.spawn_map_objects("cave", Vec2::ZERO).unwrap_err();
    assert!(matches!(&err, PrefabError::UnknownMap { map } if map == "cave"));
    assert_eq!(err.to_string(), "unknown tilemap `cave`");
}

#[test]
fn the_bound_holds_through_a_maps_origin() {
    let world = with_prefabs(world(), &[("bat", json!({}))]);
    let mut world = with_map(
        world,
        vec![layer(
            "air",
            vec![object(9, "bat", [10_000_000.0, 0.0], json!({}))],
        )],
    );
    assert_eq!(
        world.spawn_map_objects("level", Vec2::ZERO).unwrap().len(),
        1
    );
    let err = world
        .spawn_map_objects("level", [10_000_000.0, 0.0])
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "tilemap `level`, layer `air`, object 9: prefab `bat`: position [20000000, 0] must be finite and within ±16777216"
    );
    assert_eq!(world.entity_count(), 1);
}

#[test]
fn a_prefab_inserted_in_code_is_checked_at_its_spawn() {
    // Deserialized in code and inserted, the data skipped `PrefabData::load`;
    // the spawn runs the load's checks of the prefab's own fields.
    let rotation = json!({ "kind": "rotation", "from": 0.0, "to": 1.0 });
    let mut world = with_prefabs(
        world(),
        &[
            (
                "twice",
                json!({ "tweens": [
                    { "duration": 1.0, "channels": [rotation] },
                    { "duration": 2.0, "channels": [rotation] }
                ] }),
            ),
            (
                "still",
                json!({ "tweens": [ { "duration": 0.0, "channels": [rotation] } ] }),
            ),
        ],
    );
    let err = world.spawn_prefab("twice", [0.0, 0.0]).unwrap_err();
    assert!(
        matches!(&err, PrefabError::Invalid { id, .. } if id == "twice"),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "prefab `twice`: tweens: a prefab holds at most one tween (got 2)"
    );
    let err = world.spawn_prefab("still", [0.0, 0.0]).unwrap_err();
    assert_eq!(
        err.to_string(),
        "prefab `still`: tweens[0]: tween duration must be finite and > 0 (got 0)"
    );
    assert_eq!(world.entity_count(), 0);

    let mut world = with_map(
        world,
        vec![layer(
            "things",
            vec![object(3, "still", [0.0, 0.0], json!({}))],
        )],
    );
    let expected = "tilemap `level`, layer `things`, object 3: prefab `still`: tweens[0]: tween duration must be finite and > 0 (got 0)";
    let map = world
        .resource::<TilemapRegistry>()
        .get("level")
        .unwrap()
        .clone();
    assert_eq!(
        world
            .check_map_objects("level", &map)
            .unwrap_err()
            .to_string(),
        expected
    );
    assert_eq!(
        world
            .spawn_map_objects("level", Vec2::ZERO)
            .unwrap_err()
            .to_string(),
        expected
    );
}

/// A component a spawn takes from its source in the inheritance tests.
#[derive(Debug, Clone, PartialEq)]
struct Team(u8);

/// A second inherited component.
#[derive(Debug, Clone, PartialEq)]
struct Rank(u8);

/// A component no test registers for inheritance.
#[derive(Debug, Clone, PartialEq)]
struct Secret(u8);

/// F3's `Source` shape: the first entity of a spawn chain.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Origin(Entity);

/// The test World with the prefab `bolt`, tagged `bolt`.
fn bolt_world() -> World {
    with_prefabs(world(), &[("bolt", json!({ "tag": "bolt" }))])
}

/// `bolt` prepared at `x`, which tells the spawns apart.
fn bolt_at(world: &mut World, x: f32) -> PreparedPrefab {
    world.prepare_prefab("bolt", [x, 0.0]).unwrap()
}

/// A spawned bolt's x, `Team`, `Rank` and `Secret`.
type Bolt = (f32, Option<Team>, Option<Rank>, Option<Secret>);

/// Each spawned bolt, by x.
fn bolts(world: &World) -> Vec<Bolt> {
    let mut found: Vec<_> = world
        .query::<(
            &Transform,
            &Tag,
            Option<&Team>,
            Option<&Rank>,
            Option<&Secret>,
        )>()
        .filter(|(_, tag, ..)| tag.name == "bolt")
        .map(|(transform, _, team, rank, secret)| {
            (
                transform.position.x,
                team.cloned(),
                rank.cloned(),
                secret.cloned(),
            )
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found
}

#[test]
fn a_spawn_from_a_source_takes_what_it_inherits_and_nothing_else() {
    let mut world = bolt_world();
    assert!(world.inherit_from_source::<Team>());
    let armed = world.spawn_with((Team(2), Secret(9)));
    let bare = world.spawn_with((Secret(1),));
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(armed, bolt_at(&mut world, 1.0));
    buffer.spawn_prefab_from(bare, bolt_at(&mut world, 2.0));
    buffer.spawn_prefab(bolt_at(&mut world, 3.0));
    world.flush(buffer);
    assert_eq!(
        bolts(&world),
        [
            (1.0, Some(Team(2)), None, None),
            (2.0, None, None, None),
            (3.0, None, None, None),
        ],
        "the registered type from the source that has it; `Secret` from neither"
    );
}

#[test]
fn spawn_prefab_from_records_one_derive_after_the_run() {
    let mut world = with_prefabs(
        world(),
        &[(
            "slime",
            json!({ "sprite": { "asset_id": "slime" }, "tag": "enemy",
                    "components": { "health": { "max": 3 }, "player": null } }),
        )],
    );
    world.inherit_from_source::<Team>();
    let source = world.spawn_with((Team(4),));
    let prepare = |world: &mut World| {
        world
            .prepare_prefab("slime", Vec2::new(4.0, 8.0))
            .unwrap()
            .with((Route { points: Vec::new() },))
    };
    let mut plain = CommandBuffer::new();
    plain.spawn_prefab(prepare(&mut world));
    assert_eq!(plain.len(), 8);
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn_prefab_from(source, prepare(&mut world));
    assert_eq!(buffer.len(), 9, "the spawn, its run of 7 and one derive");
    assert_eq!(pending, PendingEntity { index: 0, epoch: 0 });
    world.flush(buffer);
    let found: Vec<_> = world
        .query::<(&Health, &Player, &Route, &Tag, &Team)>()
        .map(|(health, _, _, tag, team)| (health.max, tag.name.clone(), team.clone()))
        .collect();
    assert_eq!(found, [(3, "enemy".to_owned(), Team(4))]);
}

#[test]
fn a_queued_spawn_derives_from_the_source_as_the_flush_finds_it() {
    let mut world = bolt_world();
    world.inherit_from_source::<Team>();
    world.inherit_from_source::<Rank>();
    let source = world.spawn_with((Team(1),));
    let mut buffer = CommandBuffer::new();
    buffer.insert(source, Rank(4));
    buffer.remove_component::<Team>(source);
    buffer.spawn_prefab_from(source, bolt_at(&mut world, 1.0));
    world.flush(buffer);
    assert_eq!(bolts(&world), [(1.0, None, Some(Rank(4)), None)]);
}

#[test]
fn a_source_despawned_by_an_earlier_command_drops_the_spawn() {
    let mut world = bolt_world();
    world.inherit_from_source::<Team>();
    let gone = world.spawn_with((Team(1),));
    let kept = world.spawn_with((Team(2),));
    let count = world.entity_count();
    let mut buffer = CommandBuffer::new();
    buffer.despawn(gone);
    buffer.spawn_prefab_from(gone, bolt_at(&mut world, 1.0).with((Secret(1),)));
    buffer.spawn_prefab_from(kept, bolt_at(&mut world, 2.0).with((Secret(2),)));
    world.flush(buffer);
    assert_eq!(
        bolts(&world),
        [(2.0, Some(Team(2)), None, Some(Secret(2)))],
        "the next spawn keeps its own `with` value"
    );
    assert_eq!(
        world.entity_count(),
        count,
        "the source and its spawn gone, the other spawn added"
    );
    assert!(!world.is_alive(gone));
    assert_eq!(world.query::<&Secret>().count(), 1, "nothing of it remains");
}

#[test]
fn a_source_despawned_by_a_later_command_still_spawns() {
    let mut world = bolt_world();
    world.inherit_from_source::<Team>();
    let source = world.spawn_with((Team(3),));
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(source, bolt_at(&mut world, 1.0));
    buffer.despawn(source);
    world.flush(buffer);
    assert_eq!(bolts(&world), [(1.0, Some(Team(3)), None, None)]);
    assert!(!world.is_alive(source));
    assert_eq!(world.entity_count(), 1);
}

#[test]
fn the_prefab_and_a_with_bundle_keep_their_own_values() {
    let mut world = bolt_world();
    world.inherit_from_source::<Tag>();
    world.inherit_from_source::<Team>();
    world.inherit_from_source::<Rank>();
    let source = world.spawn_with((Tag::new("turret"), Team(1), Rank(1)));
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(source, bolt_at(&mut world, 1.0).with((Team(5),)));
    world.flush(buffer);
    assert_eq!(
        bolts(&world),
        [(1.0, Some(Team(5)), Some(Rank(1)), None)],
        "the prefab's tag and the bundle's team win; the rank is inherited"
    );
}

#[test]
fn derive_from_source_keeps_a_spawn_chains_first_entity() {
    let mut world = bolt_world();
    assert!(world.derive_from_source::<Origin>(|world, source| {
        Some(
            world
                .get::<Origin>(source)
                .copied()
                .unwrap_or(Origin(source)),
        )
    }));
    let a = world.spawn();
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(a, bolt_at(&mut world, 1.0));
    world.flush(buffer);
    let b: Vec<(Entity, Origin)> = world
        .query::<(Entity, &Origin)>()
        .map(|(entity, origin)| (entity, *origin))
        .collect();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].1, Origin(a), "A spawns B");
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(b[0].0, bolt_at(&mut world, 2.0));
    world.flush(buffer);
    let origins: Vec<Origin> = world.query::<&Origin>().copied().collect();
    assert_eq!(origins, [Origin(a), Origin(a)], "B spawns C: both name A");
}

#[test]
fn the_first_registration_of_a_type_stands() {
    let mut world = bolt_world();
    assert!(world.derive_from_source::<Team>(|_, _| Some(Team(7))));
    assert!(!world.inherit_from_source::<Team>());
    assert!(!world.derive_from_source::<Team>(|_, _| Some(Team(8))));
    assert!(world.inherit_from_source::<Rank>());
    assert!(!world.inherit_from_source::<Rank>());
    assert!(!world.derive_from_source::<Rank>(|_, _| Some(Rank(9))));
    let source = world.spawn_with((Team(1), Rank(2)));
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(source, bolt_at(&mut world, 1.0));
    world.flush(buffer);
    assert_eq!(bolts(&world), [(1.0, Some(Team(7)), Some(Rank(2)), None)]);
}

#[test]
fn a_spawn_from_a_saved_source_takes_the_next_id_its_area_and_its_prefab() {
    use crate::save::{SaveId, SaveState, Saved, name};

    let mut world = bolt_world();
    let turret = world.spawn();
    name(&mut world, turret, Some("cave"), "turret").unwrap();
    let global = world.spawn();
    name(&mut world, global, None, "player").unwrap();
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(turret, bolt_at(&mut world, 1.0));
    buffer.spawn_prefab_from(global, bolt_at(&mut world, 2.0));
    buffer.spawn_prefab_from(turret, bolt_at(&mut world, 3.0));
    world.flush(buffer);

    let mut spawned: Vec<(f32, SaveId, Option<String>, Option<String>)> = world
        .query::<(&Transform, &Saved)>()
        .filter(|(_, saved)| saved.prefab().is_some())
        .map(|(transform, saved)| {
            (
                transform.position.x,
                saved.id(),
                saved.area().map(str::to_owned),
                saved.prefab().map(str::to_owned),
            )
        })
        .collect();
    spawned.sort_by(|a, b| a.0.total_cmp(&b.0));
    let bolt = Some("bolt".to_owned());
    assert_eq!(
        spawned,
        [
            (1.0, SaveId(0), Some("cave".to_owned()), bolt.clone()),
            (2.0, SaveId(1), None, bolt.clone()),
            (3.0, SaveId(2), Some("cave".to_owned()), bolt),
        ],
        "IDs in queue order, each with its source's area"
    );
    assert_eq!(world.resource::<SaveState>().next_id(), 3);
}

#[test]
fn an_unsaved_prefab_spawns_unsaved_from_a_saved_source() {
    use crate::save::{Saved, name};

    let mut world = bolt_world();
    world.inherit_from_source::<Team>();
    let turret = world.spawn_with((Team(3),));
    name(&mut world, turret, Some("cave"), "turret").unwrap();
    let plain = world.spawn_with((Team(4),));
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(turret, bolt_at(&mut world, 1.0).unsaved());
    buffer.spawn_prefab_from(plain, bolt_at(&mut world, 2.0));
    buffer.spawn_prefab(bolt_at(&mut world, 3.0));
    world.flush(buffer);
    assert_eq!(
        bolts(&world),
        [
            (1.0, Some(Team(3)), None, None),
            (2.0, Some(Team(4)), None, None),
            (3.0, None, None, None),
        ],
        "each still inherits"
    );
    assert_eq!(
        world.query::<&Saved>().count(),
        1,
        "the turret alone: an unsaved prefab, an unsaved source and no source take none"
    );
}

#[test]
fn a_world_that_never_named_spawns_as_before() {
    use crate::save::Saved;

    let mut world = bolt_world();
    world.inherit_from_source::<Team>();
    world.register_saved_resource::<u32>("counter");
    let source = world.spawn_with((Team(1),));
    let types = |world: &World| -> Vec<Vec<&'static str>> {
        let mut entities: Vec<Entity> = world
            .query::<(Entity, &Tag)>()
            .map(|(entity, _)| entity)
            .collect();
        entities.sort_by_key(|entity| entity.id());
        entities
            .into_iter()
            .map(|entity| {
                world
                    .component_types(entity)
                    .into_iter()
                    .map(|(_, name)| name)
                    .collect()
            })
            .collect()
    };
    let before = world.counters();
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(source, bolt_at(&mut world, 1.0));
    buffer.spawn_prefab(bolt_at(&mut world, 2.0));
    world.flush(buffer);
    assert!(!crate::save::saving(&world));
    assert_eq!(world.query::<&Saved>().count(), 0);
    let found = types(&world);
    assert_eq!(found.len(), 2);
    for spawned in &found {
        assert!(
            !spawned.iter().any(|name| name.ends_with("::Saved")),
            "{spawned:?}"
        );
    }
    assert_eq!(
        found[0].len(),
        found[1].len() + 1,
        "the derived `Team` alone"
    );
    let after = world.counters();
    assert_eq!(
        after.archetype_moves - before.archetype_moves,
        3,
        "one move a spawn, one more for the derived `Team`, as before"
    );
}

#[test]
fn map_objects_spawn_with_their_names() {
    let world = with_prefabs(
        world(),
        &[(
            "slime",
            json!({ "tag": "slime", "components": { "health": { "max": 3 } } }),
        )],
    );
    let mut unnamed = object(3, "slime", [8.0, 8.0], json!({}));
    unnamed.name = String::new();
    let mut world = with_map(
        world,
        vec![
            layer(
                "enemies",
                vec![object(1, "slime", [0.0, 0.0], json!({})), unnamed],
            ),
            layer("more", vec![object(2, "slime", [16.0, 0.0], json!({}))]),
        ],
    );
    let prepared = world
        .prepare_map_objects_named("level", Vec2::ZERO)
        .unwrap();
    let names: Vec<&str> = prepared.iter().map(|(_, name)| name.as_str()).collect();
    assert_eq!(names, ["object1", "", "object2"], "in layer and file order");
    assert!(prepared.iter().all(|(prefab, _)| prefab.id() == "slime"));
}

/// Prefab files `<id>.json` in a temp directory of their own, removed on
/// drop.
struct PrefabDir(PathBuf);

impl PrefabDir {
    fn new(files: &[(&str, serde_json::Value)]) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "tungsten_prefab_variants_{}_{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (id, contents) in files {
            std::fs::write(dir.join(format!("{id}.json")), contents.to_string()).unwrap();
        }
        Self(dir)
    }

    /// One staged load of every file, each as prefab `<id>`.
    fn load(&self, ids: &[&str]) -> StagedLoad {
        let mut load = StagedLoad::new();
        for id in ids {
            load.add_prefab(*id, self.0.join(format!("{id}.json")))
                .unwrap();
        }
        load
    }
}

impl Drop for PrefabDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_variant_takes_its_bases_fields_and_merges_components_by_member() {
    let dir = PrefabDir::new(&[
        (
            "slime",
            json!({
                "sprite": { "asset_id": "slime", "z_order": 2 },
                "tag": "enemy",
                "components": {
                    "health": { "max": 10 },
                    "route": { "points": [[0, 0], [1, 1]] },
                    "rigid_body": { "kind": "dynamic", "shape": { "kind": "circle", "radius": 6 } }
                }
            }),
        ),
        (
            "slime_red",
            json!({
                "extends": "slime",
                "sprite": { "asset_id": "slime_red" },
                "components": {
                    "health": { "max": 30 },
                    "route": { "points": [[5, 5]] },
                    "rigid_body": { "shape": { "radius": 8 } },
                    "player": null
                }
            }),
        ),
    ]);
    let mut world = world();
    dir.load(&["slime", "slime_red"])
        .commit(&mut world)
        .unwrap();
    let prefabs = world.resource::<PrefabRegistry>();
    let red = prefabs.get("slime_red").unwrap();
    assert_eq!(red.extends.as_deref(), Some("slime"));
    assert_eq!(red.tag.as_deref(), Some("enemy"), "the base's field");
    let sprite = red.sprite.as_ref().unwrap();
    assert_eq!(
        (sprite.asset_id.as_str(), sprite.z_order),
        ("slime_red", 0),
        "a top-level field replaces the base's whole"
    );
    assert_eq!(
        red.components,
        BTreeMap::from([
            ("health".to_owned(), json!({ "max": 30 })),
            ("player".to_owned(), json!(null)),
            (
                "rigid_body".to_owned(),
                json!({ "kind": "dynamic", "shape": { "kind": "circle", "radius": 8 } })
            ),
            ("route".to_owned(), json!({ "points": [[5, 5]] })),
        ]),
        "components merge by member, arrays replaced"
    );
    let base = prefabs.get("slime").unwrap();
    assert_eq!(base.extends, None);
    assert_eq!(
        base.components["health"],
        json!({ "max": 10 }),
        "the base keeps its own"
    );

    let entity = world.spawn_prefab("slime_red", [0.0, 0.0]).unwrap();
    assert_eq!(world.get::<Health>(entity), Some(&Health { max: 30 }));
    assert_eq!(
        world.get::<Collider>(entity).unwrap().shape,
        Shape::Circle { radius: 8.0 }
    );
}

#[test]
fn a_variant_chain_resolves_from_the_root() {
    let dir = PrefabDir::new(&[
        (
            "a",
            json!({ "extends": "b", "components": { "health": { "max": 3 } } }),
        ),
        (
            "b",
            json!({ "extends": "c", "tag": "b", "components": { "route": { "points": [[1, 2]] } } }),
        ),
        (
            "c",
            json!({ "tag": "c", "visible": false, "components": { "health": { "max": 1 } } }),
        ),
    ]);
    let mut world = world();
    dir.load(&["a", "b", "c"]).commit(&mut world).unwrap();
    let prefabs = world.resource::<PrefabRegistry>();
    let summary = |id: &str| {
        let data = prefabs.get(id).unwrap();
        (
            data.extends.clone(),
            data.tag.clone(),
            data.visible,
            data.components.clone(),
        )
    };
    let route = json!({ "points": [[1, 2]] });
    assert_eq!(
        summary("a"),
        (
            Some("b".to_owned()),
            Some("b".to_owned()),
            false,
            BTreeMap::from([
                ("health".to_owned(), json!({ "max": 3 })),
                ("route".to_owned(), route.clone()),
            ])
        ),
        "the root's fields, then b's over them, then a's"
    );
    assert_eq!(
        summary("b"),
        (
            Some("c".to_owned()),
            Some("b".to_owned()),
            false,
            BTreeMap::from([
                ("health".to_owned(), json!({ "max": 1 })),
                ("route".to_owned(), route),
            ])
        )
    );
}

#[test]
fn an_extends_cycle_fails_naming_the_chain() {
    let dir = PrefabDir::new(&[
        ("a", json!({ "extends": "b" })),
        ("b", json!({ "extends": "a", "tag": "b" })),
        ("solo", json!({ "extends": "solo" })),
    ]);
    let mut world = world();
    let err = dir.load(&["a", "b"]).commit(&mut world).unwrap_err();
    let StagedLoadError::Prefab { id, .. } = &err else {
        panic!("a prefab's error: {err}");
    };
    assert_eq!(id, "a");
    assert!(
        err.to_string()
            .ends_with("extends: a cycle: `a` \u{2192} `b` \u{2192} `a`"),
        "{err}"
    );
    assert!(
        world.resource::<PrefabRegistry>().is_empty(),
        "the commit put everything back"
    );
    let err = dir.load(&["solo"]).commit(&mut world).unwrap_err();
    assert!(
        err.to_string()
            .ends_with("extends: a cycle: `solo` \u{2192} `solo`"),
        "{err}"
    );

    // A check's commit names each variant of the cycle.
    let failures = dir.load(&["a", "b"]).commit_for_check(&mut world);
    let messages: Vec<String> = failures.iter().map(ToString::to_string).collect();
    assert_eq!(messages.len(), 2, "{messages:?}");
    assert!(
        messages[1].ends_with("`b` \u{2192} `a` \u{2192} `b`"),
        "{messages:?}"
    );
}

#[test]
fn a_missing_base_fails_with_the_nearest_id() {
    let dir = PrefabDir::new(&[
        ("goblin", json!({ "tag": "goblin" })),
        ("goblin_red", json!({ "extends": "gobin" })),
        ("goblin_dark", json!({ "extends": "goblin_red" })),
    ]);
    let mut world = world();
    let err = dir
        .load(&["goblin", "goblin_red"])
        .commit(&mut world)
        .unwrap_err();
    assert!(
        err.to_string()
            .ends_with("extends: unknown prefab `gobin`; did you mean `goblin`?"),
        "{err}"
    );
    assert!(world.resource::<PrefabRegistry>().is_empty());

    // Past a variant of its own, the error names the link that breaks.
    let err = dir
        .load(&["goblin", "goblin_dark", "goblin_red"])
        .commit(&mut world)
        .unwrap_err();
    assert!(
        err.to_string().ends_with(
            "extends: `goblin_red` extends unknown prefab `gobin`; did you mean `goblin`?"
        ),
        "{err}"
    );
}

#[test]
fn a_prefab_without_extends_keeps_no_file_json() {
    let dir = PrefabDir::new(&[
        ("bat", json!({ "tag": "bat" })),
        ("vampire", json!({ "extends": "bat", "visible": false })),
    ]);
    let mut world = world();
    dir.load(&["bat", "vampire"]).commit(&mut world).unwrap();
    let prefabs = world.resource::<PrefabRegistry>();
    assert_eq!(
        prefabs.variants.keys().collect::<Vec<_>>(),
        ["vampire"],
        "the variant's file JSON alone"
    );
    assert_eq!(
        prefabs.variants["vampire"],
        json!({ "extends": "bat", "visible": false }),
        "as the file gave it"
    );
    let vampire = prefabs.get("vampire").unwrap();
    assert_eq!(
        (vampire.tag.as_deref(), vampire.visible),
        (Some("bat"), false)
    );

    // A code insert over the variant ends its resolving.
    world
        .resource_mut::<PrefabRegistry>()
        .insert("vampire".to_owned(), prefab(json!({ "tag": "code" })));
    assert!(world.resource::<PrefabRegistry>().variants.is_empty());
}

#[test]
fn a_prefabs_scale_sets_the_spawns_transform() {
    let mut world = with_prefabs(
        world(),
        &[
            ("big", json!({ "scale": [2.0, 0.5], "tag": "big" })),
            ("plain", json!({ "tag": "plain" })),
        ],
    );
    let big = world.spawn_prefab("big", [10.0, 4.0]).unwrap();
    let transform = world.get::<Transform>(big).unwrap();
    assert_eq!(transform.scale, Vec2::new(2.0, 0.5));
    assert_eq!(transform.position, Vec2::new(10.0, 4.0));
    let plain = world.spawn_prefab("plain", [0.0, 0.0]).unwrap();
    assert_eq!(world.get::<Transform>(plain).unwrap().scale, Vec2::ONE);
    // The default writes nothing back.
    let written = serde_json::to_value(prefab(json!({ "tag": "plain" }))).unwrap();
    assert!(written.get("scale").is_none(), "{written}");
}

#[test]
fn a_scale_not_above_zero_fails_the_load() {
    for scale in ["[0, 1]", "[1, -2]"] {
        let file = PrefabFile::new(Some(&format!(r#"{{ "scale": {scale} }}"#)));
        let err = PrefabData::load(&file.path, &world()).unwrap_err();
        assert!(matches!(err, PrefabError::Validation { .. }), "{err}");
        assert!(
            err.to_string()
                .contains("scale: each axis is finite and above 0"),
            "{err}"
        );
    }
    let file = PrefabFile::new(Some(r#"{ "scale": [1] }"#));
    assert!(matches!(
        PrefabData::load(&file.path, &world()),
        Err(PrefabError::Parse { .. })
    ));
    // Inserted in code, the spawn refuses it.
    let mut flat = prefab(json!({}));
    flat.scale = [f32::INFINITY, 1.0];
    let mut world = world();
    let mut registry = PrefabRegistry::new();
    registry.insert("flat".into(), flat);
    world.insert_resource(registry);
    assert!(matches!(
        world.prepare_prefab("flat", [0.0, 0.0]),
        Err(PrefabError::Invalid { .. })
    ));
}

/// `world()` with the sprites `crate`, `crate_b` and `crate_c` registered and
/// the prefabs `box`, `crate` with both as variants, and `bat`.
fn variant_world() -> World {
    let mut world = with_prefabs(
        world(),
        &[
            (
                "box",
                json!({ "sprite": { "asset_id": "crate", "variants": ["crate_b", "crate_c"] }, "tag": "box" }),
            ),
            ("bat", json!({ "tag": "bat" })),
        ],
    );
    let assets = world.resource_mut::<AssetRegistry>();
    for name in ["crate", "crate_b", "crate_c"] {
        assets.register_sprite(
            name.to_owned(),
            crate::assets::FilterMode::Nearest,
            16,
            16,
            PathBuf::from(format!("__test__/{name}.png")),
            crate::assets::registry::TextureHandle(0),
            crate::assets::UvRect::FULL,
            None,
            None,
            None,
        );
    }
    world
}

/// The sprite `entity` shows, by name.
fn shown(world: &World, entity: Entity) -> String {
    let id = world.get::<Sprite>(entity).unwrap().asset_id;
    world
        .resource::<AssetRegistry>()
        .sprite_name(id)
        .unwrap()
        .to_owned()
}

#[test]
fn a_prefabs_variant_comes_from_its_save_id() {
    use crate::save::{SaveId, Saved, name};
    let mut world = variant_world();
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..16 {
        let entity = world.spawn_prefab("box", [0.0, 0.0]).unwrap();
        let label = format!("box_{i}");
        let id = name(&mut world, entity, Some("cave"), &label).unwrap();
        let pool = world.get::<SpriteVariants>(entity).unwrap();
        assert_eq!(pool.pool().len(), 3);
        assert_eq!(
            world.get::<Sprite>(entity).unwrap().asset_id,
            pool.by_save_id(id)
        );
        assert_eq!(id, SaveId::authored(&label));
        seen.insert(shown(&world, entity));
    }
    assert_eq!(seen.len(), 3, "the names reach the whole pool: {seen:?}");
    // A run's spawn from a saved source: the run's next ID picks it, at the
    // flush.
    let source = world.spawn_prefab("bat", [0.0, 0.0]).unwrap();
    name(&mut world, source, Some("cave"), "bat_1").unwrap();
    let mut buffer = CommandBuffer::new();
    for _ in 0..6 {
        let spawn = world.prepare_prefab("box", [5.0, 5.0]).unwrap();
        buffer.spawn_prefab_from(source, spawn);
    }
    world.flush(buffer);
    let spawned: Vec<(Entity, SaveId)> = world
        .query::<(Entity, &Saved)>()
        .filter(|(_, saved)| saved.prefab() == Some("box"))
        .map(|(entity, saved)| (entity, saved.id()))
        .collect();
    assert_eq!(spawned.len(), 6);
    for (entity, id) in spawned {
        let pool = world.get::<SpriteVariants>(entity).unwrap();
        assert_eq!(
            world.get::<Sprite>(entity).unwrap().asset_id,
            pool.by_save_id(id)
        );
    }
    assert!(!world.has::<SpriteVariants>(source), "no variants, no pool");
}

#[test]
fn an_unnamed_spawn_picks_by_its_position() {
    let mut world = variant_world();
    let mut seen = std::collections::BTreeSet::new();
    for x in 0..16 {
        let at = Vec2::new(x as f32 * 16.0 + 0.75, 3.25);
        let entity = world.spawn_prefab("box", at).unwrap();
        let pool = world.get::<SpriteVariants>(entity).unwrap().clone();
        assert_eq!(
            world.get::<Sprite>(entity).unwrap().asset_id,
            pool.by_position(at)
        );
        let again = world.spawn_prefab("box", at.floor()).unwrap();
        assert_eq!(
            shown(&world, again),
            shown(&world, entity),
            "by its whole pixels"
        );
        seen.insert(shown(&world, entity));
    }
    assert_eq!(seen.len(), 3, "positions reach the whole pool: {seen:?}");
    assert!(!crate::save::saving(&world), "nothing saved for it");
}

#[test]
fn an_unknown_variant_fails_the_load() {
    let world = variant_world();
    let load = |sprite: &str| {
        let file = PrefabFile::new(Some(&format!(r#"{{ "sprite": {sprite} }}"#)));
        PrefabData::load(&file.path, &world)
    };
    let err = load(r#"{ "asset_id": "crate", "variants": ["crate_b", "crate_bb"] }"#).unwrap_err();
    assert!(matches!(err, PrefabError::Validation { .. }), "{err}");
    assert!(
        err.to_string()
            .ends_with("sprite.variants[1]: unknown sprite ID `crate_bb`; did you mean `crate_b`?"),
        "{err}"
    );
    for variants in [r#"["crate_b", "crate_b"]"#, r#"["crate"]"#, r#"[""]"#] {
        let err = load(&format!(
            r#"{{ "asset_id": "crate", "variants": {variants} }}"#
        ))
        .unwrap_err();
        assert!(
            err.to_string().contains("is empty or already in the pool"),
            "{variants}: {err}"
        );
    }
    assert!(load(r#"{ "asset_id": "crate", "variants": ["crate_c", "crate_b"] }"#).is_ok());
    // A scene entry's sprite takes none.
    let dir = std::env::temp_dir().join(format!("tungsten_variant_scene_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let scene = dir.join("scene.json");
    std::fs::write(
        &scene,
        r#"{ "entities": [{ "transform": { "position": [0, 0] }, "sprite": { "asset_id": "crate", "variants": ["crate_b"] } }] }"#,
    )
    .unwrap();
    let err = crate::assets::SceneData::load(&scene, &world).unwrap_err();
    assert!(
        err.to_string()
            .contains("entities[0].sprite.variants: a scene entry's sprite takes none"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn take_id_and_the_flush_count_on_one_counter() {
    use crate::save::{SaveId, SaveState, Saved, name, take_id};

    let mut world = bolt_world();
    assert_eq!(take_id(&mut world), None, "a World that names nothing");
    let turret = world.spawn();
    name(&mut world, turret, Some("cave"), "turret").unwrap();
    assert_eq!(take_id(&mut world), Some(SaveId(0)));
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(turret, bolt_at(&mut world, 1.0));
    world.flush(buffer);
    let bolt: Vec<SaveId> = world
        .query::<&Saved>()
        .filter(|saved| saved.prefab().is_some())
        .map(Saved::id)
        .collect();
    assert_eq!(bolt, [SaveId(1)], "the flush takes the next one");
    assert_eq!(take_id(&mut world), Some(SaveId(2)));
    assert_eq!(world.resource::<SaveState>().next_id(), 3);
}

#[test]
fn a_prepared_saved_spawn_takes_the_next_id_now_and_gives_it_in_its_one_move() {
    use crate::save::{SaveId, SaveState, Saved, name, saving};

    let mut world = variant_world();
    let unsaving = world
        .prepare_prefab("box", [5.0, 5.0])
        .unwrap()
        .saved(&mut world, Some("cave"));
    assert_eq!(unsaving.save_id(), None, "without save state, nothing");
    let plain = unsaving.spawn(&mut world);
    assert!(!world.has::<Saved>(plain));
    assert!(!saving(&world));

    let source = world.spawn_prefab("bat", [0.0, 0.0]).unwrap();
    name(&mut world, source, Some("cave"), "bat_1").unwrap();
    let now = world
        .prepare_prefab("box", [5.0, 5.0])
        .unwrap()
        .saved(&mut world, Some("cave"));
    assert_eq!(now.save_id(), Some(SaveId(0)));
    assert_eq!(world.resource::<SaveState>().next_id(), 1, "taken at once");
    let again = now.saved(&mut world, None);
    assert_eq!(again.save_id(), Some(SaveId(0)), "one ID a spawn");
    let moves = world.counters().archetype_moves;
    let entity = again.spawn(&mut world);
    assert_eq!(world.counters().archetype_moves - moves, 1, "one move");
    let saved = world.get::<Saved>(entity).unwrap();
    assert_eq!(
        (saved.id(), saved.area(), saved.prefab()),
        (SaveId(0), Some("cave"), Some("box"))
    );

    let mut buffer = CommandBuffer::new();
    let queued = world
        .prepare_prefab("box", [5.0, 5.0])
        .unwrap()
        .saved(&mut world, None);
    buffer.spawn_prefab(queued);
    let from = world
        .prepare_prefab("box", [5.0, 5.0])
        .unwrap()
        .saved(&mut world, Some("cave"));
    buffer.spawn_prefab_from(source, from);
    let moves = world.counters().archetype_moves;
    world.flush(buffer);
    assert_eq!(world.counters().archetype_moves - moves, 2, "one move each");
    let mut spawned: Vec<(SaveId, Option<String>, Entity)> = world
        .query::<(Entity, &Saved)>()
        .filter(|(_, saved)| saved.prefab() == Some("box"))
        .map(|(entity, saved)| (saved.id(), saved.area().map(str::to_owned), entity))
        .collect();
    spawned.sort_by_key(|&(id, ..)| id);
    let areas: Vec<_> = spawned
        .iter()
        .map(|(id, area, _)| (*id, area.clone()))
        .collect();
    assert_eq!(
        areas,
        [
            (SaveId(0), Some("cave".to_owned())),
            (SaveId(1), None),
            (SaveId(2), Some("cave".to_owned())),
        ],
        "the spawn from a source keeps the identity its run gave"
    );
    assert_eq!(
        world.resource::<SaveState>().next_id(),
        3,
        "the flush took none"
    );

    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..12 {
        let prepared = world
            .prepare_prefab("box", [5.0, 5.0])
            .unwrap()
            .saved(&mut world, None);
        spawned.push((
            prepared.save_id().unwrap(),
            None,
            prepared.spawn(&mut world),
        ));
    }
    for (id, _, entity) in spawned {
        let pool = world.get::<SpriteVariants>(entity).unwrap();
        assert_eq!(
            world.get::<Sprite>(entity).unwrap().asset_id,
            pool.by_save_id(id),
            "the variant its ID picks"
        );
        seen.insert(shown(&world, entity));
    }
    assert_eq!(seen.len(), 3, "{seen:?}");
}
