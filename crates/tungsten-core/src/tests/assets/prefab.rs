//! Prefab files, the registry, spawning live and queued, and a map's
//! objects with their properties over the prefabs' components.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;

use super::*;
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
    assert_eq!(pending, PendingEntity(0));
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
