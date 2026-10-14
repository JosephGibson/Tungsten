use super::*;
use crate::ecs::World;

const MINIMAL: &str = r#"{
    "entities": [
        {
            "transform": { "position": [10.0, -4.0], "rotation": 0.5, "scale": [2.0, 3.0] },
            "sprite": { "asset_id": "hero", "color": [10, 20, 30, 255], "z_order": 5 },
            "visible": false,
            "tag": "player"
        }
    ]
}"#;

#[test]
fn load_parses_minimal_fixture() {
    let data: SceneData = serde_json::from_str(MINIMAL).expect("parse minimal");
    assert_eq!(data.entities.len(), 1);
    let entry = &data.entities[0];
    assert_eq!(entry.transform.position, [10.0, -4.0]);
    assert_eq!(entry.transform.rotation, 0.5);
    assert_eq!(entry.transform.scale, [2.0, 3.0]);
    let sprite = entry.sprite.as_ref().expect("sprite present");
    assert_eq!(sprite.asset_id, "hero");
    assert_eq!(sprite.color, [10, 20, 30, 255]);
    assert_eq!(sprite.z_order, 5);
    assert!(!entry.visible);
    assert_eq!(entry.tag.as_deref(), Some("player"));
}

#[test]
fn defaults_fill_missing_fields() {
    let src = r#"{
        "entities": [
            {
                "transform": { "position": [0.0, 0.0] },
                "sprite": { "asset_id": "s" }
            }
        ]
    }"#;
    let data: SceneData = serde_json::from_str(src).expect("parse defaults");
    let entry = &data.entities[0];
    assert_eq!(entry.transform.rotation, 0.0);
    assert_eq!(entry.transform.scale, [1.0, 1.0]);
    let sprite = entry.sprite.as_ref().unwrap();
    assert_eq!(sprite.color, [255, 255, 255, 255]);
    assert_eq!(sprite.z_order, 0);
    assert!(entry.visible);
    assert!(entry.tag.is_none());
}

#[test]
fn empty_entities_list_is_valid() {
    let data: SceneData = serde_json::from_str(r#"{ "entities": [] }"#).unwrap();
    assert!(data.entities.is_empty());
}

#[test]
fn round_trip_preserves_fields() {
    let data: SceneData = serde_json::from_str(MINIMAL).unwrap();
    let encoded = serde_json::to_string(&data).unwrap();
    let reparsed: SceneData = serde_json::from_str(&encoded).unwrap();
    assert_eq!(reparsed.entities.len(), data.entities.len());
    assert_eq!(
        reparsed.entities[0].transform.position,
        data.entities[0].transform.position
    );
}

const WITH_TWEEN: &str = r#"{
    "entities": [
        {
            "transform": { "position": [0.0, 0.0] },
            "sprite": { "asset_id": "overlay" },
            "tweens": [
                {
                    "duration": 0.4,
                    "easing": "cubic_out",
                    "repeat": "once",
                    "tag": "scene_fade_in",
                    "channels": [
                        { "kind": "color_a", "from": 0, "to": 255 },
                        { "kind": "position_x", "from": -100.0, "to": 0.0 }
                    ]
                }
            ]
        }
    ]
}"#;

#[test]
fn parses_scene_tween_entry() {
    let data: SceneData = serde_json::from_str(WITH_TWEEN).expect("parse");
    let entry = &data.entities[0];
    assert_eq!(entry.tweens.len(), 1);
    let t = &entry.tweens[0];
    assert_eq!(t.duration, 0.4);
    assert_eq!(t.easing, Easing::CubicOut);
    assert_eq!(t.channels.len(), 2);
    assert_eq!(t.tag.as_deref(), Some("scene_fade_in"));
}

#[test]
fn scene_tween_round_trips_through_json() {
    let data: SceneData = serde_json::from_str(WITH_TWEEN).unwrap();
    let encoded = serde_json::to_string(&data).unwrap();
    let reparsed: SceneData = serde_json::from_str(&encoded).unwrap();
    assert_eq!(reparsed.entities[0].tweens.len(), 1);
    assert_eq!(
        reparsed.entities[0].tweens[0].duration,
        data.entities[0].tweens[0].duration
    );
}

#[test]
fn scene_tween_into_tween_maps_fields() {
    let data: SceneData = serde_json::from_str(WITH_TWEEN).unwrap();
    let runtime = data.entities[0].tweens[0].into_tween();
    assert_eq!(runtime.channels.len(), 2);
    assert_eq!(runtime.duration, 0.4);
    assert_eq!(runtime.easing, Easing::CubicOut);
    assert_eq!(runtime.repeat, TweenRepeat::Once);
    assert_eq!(runtime.on_complete_tag.as_deref(), Some("scene_fade_in"));
}

#[test]
fn scene_tween_times_repeat_parses() {
    let src = r#"{ "entities": [ {
        "transform": { "position": [0.0, 0.0] },
        "tweens": [ {
            "duration": 0.5,
            "repeat": { "times": 4 },
            "channels": [ { "kind": "rotation", "from": 0.0, "to": 6.28 } ]
        } ]
    } ] }"#;
    let data: SceneData = serde_json::from_str(src).expect("parse times");
    let t = data.entities[0].tweens[0].into_tween();
    assert_eq!(t.repeat, TweenRepeat::Times(4));
}

#[test]
fn load_rejects_non_finite_duration() {
    let file = SceneFile::new(
        r#"{ "entities": [ {
        "transform": { "position": [0.0, 0.0] },
        "tweens": [ {
            "duration": 0.0,
            "channels": [ { "kind": "color_a", "from": 0, "to": 255 } ]
        } ]
    } ] }"#,
    );
    let err = SceneData::load(&file.path, &World::new()).expect_err("must reject zero duration");
    assert!(matches!(err, SceneError::Validation { .. }));
}

#[test]
fn load_rejects_empty_channels() {
    let file = SceneFile::new(
        r#"{ "entities": [ {
        "transform": { "position": [0.0, 0.0] },
        "tweens": [ { "duration": 0.5, "channels": [] } ]
    } ] }"#,
    );
    let err = SceneData::load(&file.path, &World::new()).expect_err("must reject empty channels");
    assert!(matches!(err, SceneError::Validation { .. }));
}

#[test]
fn scene_rejects_vec4_lane_out_of_range() {
    // B6: a vec4 lane above 3 must fail validation instead of silently
    // driving lane 3 through the runtime clamp.
    let file = SceneFile::new(
        r#"{ "entities": [ {
        "transform": { "position": [0.0, 0.0] },
        "tweens": [ {
            "duration": 0.5,
            "channels": [ { "kind": "uniform_vec4_lane", "slot": "v0", "lane": 4, "from": 0.0, "to": 1.0 } ]
        } ]
    } ] }"#,
    );
    let err = SceneData::load(&file.path, &World::new()).expect_err("must reject lane 4");
    assert!(matches!(err, SceneError::Validation { .. }), "{err}");
}

// Only deserialized: the load checks the value and drops it.
#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Health {
    max: u32,
}

#[derive(Debug, serde::Deserialize)]
struct Player;

const WITH_COMPONENTS: &str = r#"{
    "entities": [
        {
            "transform": { "position": [1.0, 2.0] },
            "components": { "player": null, "health": { "max": 10 } }
        },
        { "transform": { "position": [0.0, 0.0] } }
    ]
}"#;

#[test]
fn components_parse_and_default_to_empty() {
    let data: SceneData = serde_json::from_str(WITH_COMPONENTS).unwrap();
    let names: Vec<_> = data.entities[0].components.keys().collect();
    assert_eq!(names, ["health", "player"], "in name order");
    assert_eq!(
        data.entities[0].components["health"],
        serde_json::json!({ "max": 10 })
    );
    assert_eq!(
        data.entities[0].components["player"],
        serde_json::Value::Null
    );
    assert!(data.entities[1].components.is_empty());
}

#[test]
fn components_round_trip_and_an_empty_map_writes_no_key() {
    let data: SceneData = serde_json::from_str(WITH_COMPONENTS).unwrap();
    let encoded = serde_json::to_value(&data).unwrap();
    assert_eq!(
        encoded["entities"][0]["components"],
        serde_json::json!({ "health": { "max": 10 }, "player": null })
    );
    assert!(encoded["entities"][1].get("components").is_none());
    let reparsed: SceneData = serde_json::from_value(encoded).unwrap();
    assert_eq!(reparsed.entities[0].components, data.entities[0].components);
}

fn parse_error(src: &str) -> String {
    serde_json::from_str::<SceneData>(src)
        .expect_err("must not parse")
        .to_string()
}

#[test]
fn a_component_named_twice_fails_the_parse() {
    let err = parse_error(
        r#"{ "entities": [ { "transform": { "position": [0.0, 0.0] },
        "components": { "health": { "max": 1 }, "health": { "max": 2 } } } ] }"#,
    );
    assert!(
        err.starts_with("duplicate component `health` at line"),
        "{err}"
    );
}

#[test]
fn a_key_repeated_inside_a_value_fails_the_parse() {
    // A plain `Value` keeps the last `max`, so the valid 10 would mask "ten".
    let err = parse_error(
        r#"{ "entities": [ { "transform": { "position": [0.0, 0.0] },
        "components": { "health": { "max": "ten", "max": 10 } } } ] }"#,
    );
    assert!(err.starts_with("duplicate field `max` at line 2"), "{err}");
    let deep = parse_error(
        r#"{ "entities": [ { "transform": { "position": [0.0, 0.0] },
        "components": { "armor": { "list": [ { "a": 1, "a": 2 } ] } } } ] }"#,
    );
    assert!(deep.starts_with("duplicate field `a`"), "{deep}");
}

#[test]
fn unknown_keys_fail_the_parse_at_every_level() {
    let cases = [
        (
            r#"{ "entities": [], "entites": [] }"#,
            "unknown field `entites`",
        ),
        (
            r#"{ "entities": [ { "transform": { "position": [0.0, 0.0] }, "componets": {} } ] }"#,
            "unknown field `componets`",
        ),
        (
            r#"{ "entities": [ { "transform": { "position": [0.0, 0.0], "z": 1 } } ] }"#,
            "unknown field `z`",
        ),
        (
            r#"{ "entities": [ { "transform": { "position": [0.0, 0.0] },
                "sprite": { "asset_id": "s", "colour": [0, 0, 0, 0] } } ] }"#,
            "unknown field `colour`",
        ),
        (
            r#"{ "entities": [ { "transform": { "position": [0.0, 0.0] },
                "tweens": [ { "duration": 1.0, "loop": true, "channels": [] } ] } ] }"#,
            "unknown field `loop`",
        ),
    ];
    for (src, expected) in cases {
        let err = parse_error(src);
        assert!(err.starts_with(expected), "{err}");
    }
}

#[test]
fn load_rejects_a_transform_that_is_not_finite_or_too_far() {
    for (position, got) in [
        ("[1e40, 0.0]", "[inf, 0]"),
        ("[0.0, -16777218.0]", "[0, -16777218]"),
    ] {
        let file = SceneFile::new(&format!(
            r#"{{ "entities": [ {{ "transform": {{ "position": {position} }} }} ] }}"#
        ));
        let err = SceneData::load(&file.path, &World::new()).unwrap_err();
        assert!(matches!(err, SceneError::Validation { .. }), "{err}");
        assert_eq!(
            err.to_string(),
            format!(
                "invalid scene '{}': entities[0].transform: position must be finite and within ±16777216 (got {got})",
                file.path.display()
            )
        );
    }
    let edge = SceneFile::new(
        r#"{ "entities": [ { "transform": { "position": [16777216.0, -16777216.0] } } ] }"#,
    );
    assert!(SceneData::load(&edge.path, &World::new()).is_ok());
    let rotation = SceneFile::new(
        r#"{ "entities": [ { "transform": { "position": [0.0, 0.0], "rotation": 1e40 } } ] }"#,
    );
    let err = SceneData::load(&rotation.path, &World::new()).unwrap_err();
    assert!(
        err.to_string()
            .ends_with("entities[0].transform: rotation must be finite (got inf)"),
        "{err}"
    );
    let scale = SceneFile::new(
        r#"{ "entities": [ { "transform": { "position": [0.0, 0.0], "scale": [1.0, -1e40] } } ] }"#,
    );
    let err = SceneData::load(&scale.path, &World::new()).unwrap_err();
    assert!(
        err.to_string()
            .ends_with("entities[0].transform: scale must be finite (got [1, -inf])"),
        "{err}"
    );
}

fn registered_world() -> World {
    let mut world = World::new();
    world.register_component::<Health>("health");
    world.register_component::<Player>("player");
    world
}

fn load_message(src: &str, world: &World) -> String {
    let file = SceneFile::new(src);
    let err = SceneData::load(&file.path, world).expect_err("must fail");
    assert!(matches!(err, SceneError::Validation { .. }), "{err}");
    let prefix = format!("invalid scene '{}': ", file.path.display());
    err.to_string()
        .strip_prefix(&prefix)
        .unwrap_or_else(|| panic!("{err} must name the path"))
        .to_owned()
}

#[test]
fn load_names_the_path_the_entry_and_the_component() {
    let world = registered_world();
    assert_eq!(
        load_message(
            r#"{ "entities": [
                { "transform": { "position": [0.0, 0.0] }, "components": { "player": null } },
                { "transform": { "position": [0.0, 0.0] }, "components": { "helth": { "max": 1 } } }
            ] }"#,
            &world
        ),
        "entities[1]: unknown component `helth`"
    );
    assert_eq!(
        load_message(
            r#"{ "entities": [ { "transform": { "position": [0.0, 0.0] },
                "components": { "health": { "max": "ten" } } } ] }"#,
            &world
        ),
        "entities[0]: component `health`, field `max`: invalid type: string \"ten\", expected u32"
    );
    assert_eq!(
        load_message(
            r#"{ "entities": [ { "transform": { "position": [0.0, 0.0] },
                "components": { "health": {} } } ] }"#,
            &world
        ),
        "entities[0]: component `health`: missing field `max`"
    );
}

#[test]
fn a_world_without_a_registry_finds_every_component_unknown() {
    assert_eq!(
        load_message(WITH_COMPONENTS, &World::new()),
        "entities[0]: unknown component `health`"
    );
}

#[test]
fn registered_components_pass_the_load() {
    let file = SceneFile::new(WITH_COMPONENTS);
    let data = SceneData::load(&file.path, &registered_world()).unwrap();
    assert_eq!(data.entities[0].components.len(), 2);
}

use std::sync::atomic::{AtomicU32, Ordering};
static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A scene file in a temp directory of its own, removed on drop.
struct SceneFile {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl SceneFile {
    fn new(contents: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tungsten_scene_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scene.json");
        std::fs::write(&path, contents).unwrap();
        Self { dir, path }
    }
}

impl Drop for SceneFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn the_load_checks_the_asset_ids_a_component_names() {
    // `SceneData::load` runs `check_in`: a value naming an asset the World
    // lacks fails, naming the entry, the component, its field and the ID.
    #[derive(Debug, serde::Deserialize)]
    struct Emitter {
        config: String,
    }
    struct Configs(Vec<&'static str>);
    let mut world = World::new();
    world.insert_resource(Configs(vec!["embers"]));
    world.register_component_with_assets(
        "emitter",
        |emitter: &Emitter, world: &World| {
            if world
                .resource::<Configs>()
                .0
                .contains(&emitter.config.as_str())
            {
                Ok(())
            } else {
                Err(crate::component_registry::MissingAsset::new(
                    "config",
                    "particle config",
                    &emitter.config,
                ))
            }
        },
        |emitter: Emitter, _: &crate::component_registry::SpawnContext<'_>| (emitter,),
    );
    assert_eq!(
        load_message(
            r#"{ "entities": [
                { "transform": { "position": [0.0, 0.0] }, "components": { "emitter": { "config": "embers" } } },
                { "transform": { "position": [0.0, 0.0] }, "components": { "emitter": { "config": "smoke" } } }
            ] }"#,
            &world
        ),
        "entities[1]: component `emitter`, field `config`: unknown particle config `smoke`"
    );
}
