use std::panic::{AssertUnwindSafe, catch_unwind};

use glam::Vec2;
use serde::Deserialize;
use serde_json::json;

use super::*;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Health {
    max: u32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Armor {
    inner: Health,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Player;

#[derive(Debug, PartialEq)]
struct Spawned {
    at: Vec2,
    bonus: u32,
}

#[derive(Deserialize)]
struct SpawnedDef {
    bonus: u32,
}

/// A resource a definition reads through `SpawnContext::world`.
struct Difficulty(u32);

fn panic_message(f: impl FnOnce()) -> String {
    let payload = catch_unwind(AssertUnwindSafe(f)).expect_err("must panic");
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_default()
}

fn invalid(err: ComponentError) -> (String, String, String) {
    match err {
        ComponentError::Invalid {
            name,
            field,
            source,
        } => (name, field, source.to_string()),
        other => panic!("expected Invalid, got {other}"),
    }
}

#[test]
fn one_registration_per_name_and_type() {
    let mut registry = ComponentRegistry::new();
    assert!(registry.is_empty());
    assert!(registry.register::<Health>("health"));
    assert!(
        !registry.register::<Health>("health"),
        "the same pair again"
    );
    assert_eq!(registry.len(), 1);
    assert!(registry.contains("health"));
    assert_eq!(registry.name_of::<Health>(), Some("health"));
    assert_eq!(registry.name_of::<Player>(), None);

    assert!(registry.register::<Player>("player"));
    assert!(registry.register::<Armor>("armor"));
    assert_eq!(
        registry.names().collect::<Vec<_>>(),
        ["armor", "health", "player"]
    );
    assert_eq!(
        format!("{registry:?}"),
        r#"ComponentRegistry { names: ["armor", "health", "player"] }"#
    );
}

#[test]
fn a_name_taken_by_another_type_panics_naming_both() {
    let mut registry = ComponentRegistry::new();
    registry.register::<Health>("health");
    let message = panic_message(|| {
        registry.register::<Player>("health");
    });
    assert!(message.contains("`health`"), "{message}");
    assert!(message.contains(type_name::<Health>()), "{message}");
    assert!(message.contains(type_name::<Player>()), "{message}");
    assert_eq!(
        registry.name_of::<Player>(),
        None,
        "the registry is unchanged"
    );
}

#[test]
fn a_type_under_a_second_name_panics_naming_both_names() {
    let mut registry = ComponentRegistry::new();
    registry.register::<Health>("health");
    let message = panic_message(|| {
        registry.register::<Health>("hp");
    });
    assert!(message.contains(type_name::<Health>()), "{message}");
    assert!(message.contains("`health`"), "{message}");
    assert!(message.contains("`hp`"), "{message}");
    assert!(!registry.contains("hp"));
}

#[test]
fn a_name_outside_the_pattern_panics() {
    for name in ["", "Health", "9lives", "_hidden", "max-hp", "hp!", "héalth"] {
        let mut registry = ComponentRegistry::new();
        let message = panic_message(|| {
            registry.register::<Health>(name);
        });
        assert!(message.contains("[a-z][a-z0-9_]*"), "{name}: {message}");
        assert!(message.contains(type_name::<Health>()), "{name}: {message}");
        assert!(registry.is_empty());
    }
    let mut registry = ComponentRegistry::new();
    assert!(registry.register::<Health>("hp_2"));
}

#[test]
fn check_names_an_unknown_component() {
    let registry = ComponentRegistry::new();
    let err = registry.check("helth", &json!({})).unwrap_err();
    assert!(matches!(&err, ComponentError::Unknown { name } if name == "helth"));
    assert_eq!(err.to_string(), "unknown component `helth`");
}

#[test]
fn check_reports_a_missing_field_at_the_top() {
    let mut registry = ComponentRegistry::new();
    registry.register::<Health>("health");
    let err = registry.check("health", &json!({})).unwrap_err();
    assert_eq!(err.to_string(), "component `health`: missing field `max`");
    let (name, field, source) = invalid(err);
    assert_eq!((name.as_str(), field.as_str()), ("health", ""));
    assert_eq!(source, "missing field `max`");
}

#[test]
fn check_reports_an_unknown_field_under_deny_unknown_fields() {
    let mut registry = ComponentRegistry::new();
    registry.register::<Health>("health");
    let err = registry
        .check("health", &json!({ "max": 3, "regen": 1 }))
        .unwrap_err();
    let (_, field, source) = invalid(err);
    assert_eq!(field, "regen");
    assert!(source.starts_with("unknown field `regen`"), "{source}");
}

#[test]
fn check_names_the_field_of_the_wrong_type() {
    let mut registry = ComponentRegistry::new();
    registry.register::<Health>("health");
    let err = registry
        .check("health", &json!({ "max": "ten" }))
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "component `health`, field `max`: invalid type: string \"ten\", expected u32"
    );
    let (_, field, _) = invalid(err);
    assert_eq!(field, "max");
}

#[test]
fn check_names_a_wrong_type_one_struct_down() {
    let mut registry = ComponentRegistry::new();
    registry.register::<Armor>("armor");
    let err = registry
        .check("armor", &json!({ "inner": { "max": -1 } }))
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "component `armor`, field `inner.max`: invalid value: integer `-1`, expected u32"
    );
    assert!(
        registry
            .check("armor", &json!({ "inner": { "max": 1 } }))
            .is_ok()
    );
}

#[test]
fn a_unit_struct_reads_null_and_refuses_an_object() {
    let mut registry = ComponentRegistry::new();
    registry.register::<Player>("player");
    assert!(registry.check("player", &json!(null)).is_ok());
    let err = registry.check("player", &json!({})).unwrap_err();
    let (_, field, source) = invalid(err);
    assert_eq!(field, "");
    assert!(source.contains("expected unit struct Player"), "{source}");
}

#[test]
fn register_with_builds_from_the_transform_and_the_world() {
    let mut world = World::new();
    world.insert_resource(Difficulty(5));
    world.register_component_with("spawned", |def: SpawnedDef, at: &SpawnContext<'_>| {
        let difficulty = at.world().resource::<Difficulty>().0;
        (Spawned {
            at: at.transform().position,
            bonus: def.bonus * difficulty,
        },)
    });
    let at = SpawnContext::new(&world, Transform::from_position(Vec2::new(3.0, -4.0)));
    let prepared = world
        .resource::<ComponentRegistry>()
        .prepare("spawned", &json!({ "bonus": 2 }), &at)
        .unwrap();

    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();
    prepared.insert_pending(&mut buffer, pending);
    world.flush(buffer);
    let spawned: Vec<_> = world.query::<&Spawned>().collect();
    assert_eq!(
        spawned,
        [&Spawned {
            at: Vec2::new(3.0, -4.0),
            bonus: 10
        }]
    );
    assert_eq!(
        world
            .resource::<ComponentRegistry>()
            .name_of::<SpawnedDef>(),
        Some("spawned"),
        "keyed by the definition's type"
    );
}

#[test]
fn prepare_leaves_the_buffer_and_insert_pending_queues_one_bundle() {
    let mut world = World::new();
    world.register_component_with("pair", |def: Health, _: &SpawnContext<'_>| (def, Player));
    let at = SpawnContext::new(&world, Transform::default());
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();
    let prepared = world
        .resource::<ComponentRegistry>()
        .prepare("pair", &json!({ "max": 7 }), &at)
        .unwrap();
    assert_eq!(buffer.len(), 1, "prepare queued nothing");
    assert_eq!(
        format!("{prepared:?}"),
        r#"PreparedComponent { name: "pair", .. }"#
    );
    prepared.insert_pending(&mut buffer, pending);
    assert_eq!(buffer.len(), 3, "the spawn, then one insert per component");

    world.flush(buffer);
    let found: Vec<_> = world.query::<(&Health, &Player)>().collect();
    assert_eq!(found, [(&Health { max: 7 }, &Player)]);
}

#[test]
fn prepare_fails_as_check_does() {
    let mut registry = ComponentRegistry::new();
    registry.register::<Health>("health");
    let world = World::new();
    let at = SpawnContext::new(&world, Transform::default());
    for value in [
        json!({}),
        json!({ "max": "ten" }),
        json!({ "max": 1, "x": 0 }),
    ] {
        let checked = registry.check("health", &value).unwrap_err().to_string();
        let prepared = registry
            .prepare("health", &value, &at)
            .unwrap_err()
            .to_string();
        assert_eq!(checked, prepared);
    }
    assert!(matches!(
        registry.prepare("helth", &json!({}), &at),
        Err(ComponentError::Unknown { .. })
    ));
}

#[test]
fn the_world_door_inserts_the_registry_and_shares_it() {
    let mut world = World::new();
    assert!(!world.has_resource::<ComponentRegistry>());
    assert!(world.register_component::<Health>("health"));
    assert!(world.has_resource::<ComponentRegistry>());
    assert!(!world.register_component::<Health>("health"));

    assert!(
        world
            .resource_mut::<ComponentRegistry>()
            .register::<Player>("player")
    );
    assert!(world.register_component_with("armor", |def: Armor, _: &SpawnContext<'_>| (def,)));
    assert_eq!(
        world
            .resource::<ComponentRegistry>()
            .names()
            .collect::<Vec<_>>(),
        ["armor", "health", "player"]
    );
}

/// The configs a test World holds, by ID.
struct Configs(Vec<&'static str>);

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Emitter {
    config: String,
}

fn config_known(emitter: &Emitter, world: &World) -> Result<(), MissingAsset> {
    let known = world
        .get_resource::<Configs>()
        .is_some_and(|configs| configs.0.contains(&emitter.config.as_str()));
    if known {
        Ok(())
    } else {
        Err(MissingAsset::new(
            "config",
            "particle config",
            &emitter.config,
        ))
    }
}

#[test]
fn check_in_runs_the_asset_check_and_check_skips_it() {
    let mut world = World::new();
    world.insert_resource(Configs(vec!["embers"]));
    world.register_component_with_assets(
        "emitter",
        config_known,
        |def: Emitter, _: &SpawnContext<'_>| (def,),
    );
    world.register_component::<Health>("health");
    let registry = world.resource::<ComponentRegistry>();

    assert!(
        registry
            .check_in("emitter", &json!({ "config": "embers" }), &world)
            .is_ok()
    );
    assert!(
        registry
            .check("emitter", &json!({ "config": "smoke" }))
            .is_ok(),
        "the shape alone"
    );
    let err = registry
        .check_in("emitter", &json!({ "config": "smoke" }), &world)
        .unwrap_err();
    assert!(
        matches!(
            &err,
            ComponentError::MissingAsset { name, field, kind, id }
                if name == "emitter" && field == "config" && *kind == "particle config" && id == "smoke"
        ),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "component `emitter`, field `config`: unknown particle config `smoke`"
    );
    // A bad shape fails before the asset check, and a plain registration
    // has none.
    let (_, field, _) = invalid(
        registry
            .check_in("emitter", &json!({ "config": 3 }), &world)
            .unwrap_err(),
    );
    assert_eq!(field, "config");
    assert!(
        registry
            .check_in("health", &json!({ "max": 1 }), &world)
            .is_ok()
    );
    assert!(matches!(
        registry.check_in("helth", &json!({}), &world),
        Err(ComponentError::Unknown { .. })
    ));
}

#[test]
fn prepare_runs_the_asset_check_against_its_world_before_build() {
    let mut world = World::new();
    world.insert_resource(Configs(vec!["embers"]));
    world.register_component_with_assets(
        "emitter",
        config_known,
        |def: Emitter, at: &SpawnContext<'_>| {
            // The check passed against this World, so the ID resolves.
            assert!(
                at.world()
                    .resource::<Configs>()
                    .0
                    .contains(&def.config.as_str())
            );
            (def,)
        },
    );
    let at = SpawnContext::new(&world, Transform::default());
    let registry = world.resource::<ComponentRegistry>();
    let err = registry
        .prepare("emitter", &json!({ "config": "smoke" }), &at)
        .unwrap_err();
    assert!(
        matches!(err, ComponentError::MissingAsset { .. }),
        "{err:?}"
    );
    let prepared = registry
        .prepare("emitter", &json!({ "config": "embers" }), &at)
        .unwrap();

    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();
    prepared.insert_pending(&mut buffer, pending);
    world.flush(buffer);
    let found: Vec<_> = world.query::<&Emitter>().collect();
    assert_eq!(
        found,
        [&Emitter {
            config: "embers".to_owned()
        }]
    );

    // An asset that went between a check and the spawn fails the prepare.
    let checked = world.resource::<ComponentRegistry>().check_in(
        "emitter",
        &json!({ "config": "embers" }),
        &world,
    );
    assert!(checked.is_ok());
    world.resource_mut::<Configs>().0.clear();
    let at = SpawnContext::new(&world, Transform::default());
    assert!(matches!(
        world.resource::<ComponentRegistry>().prepare(
            "emitter",
            &json!({ "config": "embers" }),
            &at
        ),
        Err(ComponentError::MissingAsset { .. })
    ));
}

#[test]
fn a_missing_asset_at_the_values_top_prints_no_field() {
    let err = ComponentError::MissingAsset {
        name: "tilemap".to_owned(),
        field: String::new(),
        kind: "tilemap",
        id: "cave".to_owned(),
    };
    assert_eq!(
        err.to_string(),
        "component `tilemap`: unknown tilemap `cave`"
    );
    let missing = MissingAsset::new("id", "tilemap", "cave");
    assert_eq!(
        (missing.field.as_str(), missing.kind, missing.id.as_str()),
        ("id", "tilemap", "cave")
    );
}

#[test]
fn the_asset_form_takes_names_and_types_as_register_with_does() {
    let mut world = World::new();
    assert!(world.register_component_with_assets(
        "emitter",
        config_known,
        |def: Emitter, _: &SpawnContext<'_>| (def,)
    ));
    assert!(!world.register_component_with_assets(
        "emitter",
        config_known,
        |def: Emitter, _: &SpawnContext<'_>| (def,)
    ));
    assert_eq!(
        world.resource::<ComponentRegistry>().name_of::<Emitter>(),
        Some("emitter")
    );
    let message = panic_message(|| {
        world.register_component::<Health>("emitter");
    });
    assert!(message.contains("`emitter`"), "{message}");
    assert!(message.contains(type_name::<Emitter>()), "{message}");
    let message = panic_message(|| {
        world.register_component_with_assets(
            "smoke",
            config_known,
            |def: Emitter, _: &SpawnContext<'_>| (def,),
        );
    });
    assert!(message.contains("`smoke`"), "{message}");
}
