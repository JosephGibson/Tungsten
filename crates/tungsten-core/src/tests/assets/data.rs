//! Data kinds, `DataRef`, the two-phase `StagedLoad` and the nearest name.

use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;

use super::*;
use crate::component_registry::SpawnContext;

/// An effect: how long it lasts, and the prefab its trail names.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Effect {
    duration: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trail: Option<String>,
}

/// An item, which names an effect.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    effect: DataRef<Effect>,
}

/// The declared tags: one entry that the `filter` component reads.
#[derive(Debug, Deserialize)]
struct Tags {
    names: Vec<String>,
}

/// A component that names an effect and, optionally, a prefab.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Projectile {
    on_hit: DataRef<Effect>,
    #[serde(default)]
    spark: Option<String>,
}

/// A component that names a tag the `tags` entry `all` declares.
#[derive(Debug, Deserialize)]
struct Filter {
    tag: String,
}

/// A World with the kinds `effects` (its trail checked as a prefab),
/// `items` (its effect checked) and `tags`, and the components
/// `projectile` and `filter`.
fn world() -> World {
    let mut world = World::new();
    world.register_data_kind_with_assets::<Effect>("effects", |effect, world| {
        match &effect.trail {
            Some(trail) => world.check_prefab_id("trail", trail),
            None => Ok(()),
        }
    });
    world.register_data_kind_with_assets::<Item>("items", |item, world| {
        item.effect.check(world, "effect")
    });
    world.register_data_kind::<Tags>("tags");
    world.register_component_with_assets(
        "projectile",
        |projectile: &Projectile, world: &World| {
            projectile.on_hit.check(world, "on_hit")?;
            match &projectile.spark {
                Some(spark) => world.check_prefab_id("spark", spark),
                None => Ok(()),
            }
        },
        |projectile: Projectile, _: &SpawnContext<'_>| (projectile,),
    );
    world.register_component_with_assets(
        "filter",
        |filter: &Filter, world: &World| {
            let declared = world
                .get_resource::<DataRegistry>()
                .and_then(|data| data.get::<Tags>("all"))
                .is_some_and(|tags| tags.names.contains(&filter.tag));
            if declared {
                Ok(())
            } else {
                Err(MissingAsset::new("tag", "tag", &filter.tag))
            }
        },
        |filter: Filter, _: &SpawnContext<'_>| (filter,),
    );
    world
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A temp directory of its own, removed on drop.
struct Files {
    dir: PathBuf,
}

impl Files {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tungsten_data_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    /// Writes `name`, returning its canonical path: a commit keeps the
    /// canonical path, as hot reload looks it up.
    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.dir.join(name);
        std::fs::write(&path, contents).unwrap();
        path.canonicalize().unwrap()
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn duration(world: &World, id: &str) -> Option<f32> {
    world
        .resource::<DataRegistry>()
        .get::<Effect>(id)
        .map(|effect| effect.duration)
}

fn prefab_route(world: &World, path: &Path) -> Option<String> {
    world
        .resource::<PrefabRegistry>()
        .id_for_path(path)
        .map(str::to_owned)
}

fn data_route(world: &World, path: &Path) -> Option<(&'static str, String)> {
    world
        .resource::<DataRegistry>()
        .entry_for_path(path)
        .map(|(kind, id)| (kind, id.to_owned()))
}

#[test]
fn registration_is_one_kind_per_type_and_one_type_per_kind() {
    let mut data = DataRegistry::new();
    assert!(data.register::<Effect>("effects"));
    assert!(
        !data.register::<Effect>("effects"),
        "the same pair is a no-op"
    );
    assert!(data.register_with_assets::<Item>("items", |_, _| Ok(())));
    assert!(data.contains_kind("effects") && !data.contains_kind("item"));
    assert_eq!(data.kinds().collect::<Vec<_>>(), ["effects", "items"]);
    assert_eq!(data.kind_of::<Item>(), Some("items"));
    assert_eq!(data.kind_of::<Tags>(), None);
    assert!(format!("{data:?}").contains("effects"));

    let mut world = World::new();
    assert!(world.register_data_kind::<Tags>("tags"));
    assert!(!world.register_data_kind::<Tags>("tags"));
    assert!(world.resource::<DataRegistry>().contains_kind("tags"));
}

#[test]
#[should_panic(expected = "data kind `Effects` for")]
fn a_kind_name_outside_the_pattern_panics() {
    DataRegistry::new().register::<Effect>("Effects");
}

#[test]
#[should_panic(expected = "data kind `effects` already holds")]
fn a_kind_taken_by_another_type_panics() {
    let mut data = DataRegistry::new();
    data.register::<Effect>("effects");
    data.register::<Tags>("effects");
}

#[test]
#[should_panic(expected = "is already registered as data kind `effects`; it cannot also be `fx`")]
fn a_type_under_a_second_kind_panics() {
    let mut data = DataRegistry::new();
    data.register::<Effect>("effects");
    data.register::<Effect>("fx");
}

#[test]
#[should_panic(expected = "is not a registered data kind's type")]
fn inserting_an_unregistered_type_panics() {
    DataRegistry::new().insert(
        "burning",
        Effect {
            duration: 1.0,
            trail: None,
        },
    );
}

#[test]
fn the_registry_reads_entries_typed_sorted_and_by_file() {
    let mut data = DataRegistry::new();
    data.register::<Effect>("effects");
    data.register::<Tags>("tags");
    assert!(data.is_empty());
    let effect = |duration| Effect {
        duration,
        trail: None,
    };
    data.insert("frozen", effect(3.0));
    data.insert("burning", effect(1.0));
    data.insert("burning".to_owned(), effect(2.0));
    assert_eq!(data.len(), 2);
    assert_eq!(
        data.ids("effects").collect::<Vec<_>>(),
        ["burning", "frozen"]
    );
    assert_eq!(data.ids("tags").count(), 0);
    assert_eq!(data.ids("unknown").count(), 0);
    assert_eq!(data.get::<Effect>("burning"), Some(&effect(2.0)));
    assert!(data.get::<Effect>("rotten").is_none());
    assert!(data.get::<Tags>("burning").is_none(), "another kind's ID");
    assert!(data.get::<u8>("burning").is_none(), "an unregistered type");

    let files = Files::new();
    let path = files.write("burning.json", r#"{ "duration": 4.0 }"#);
    let mut world = World::new();
    world.insert_resource(data);
    let mut load = StagedLoad::new();
    load.add_data(&world, "effects", "burning", &path).unwrap();
    load.commit(&mut world).unwrap();
    assert_eq!(
        data_route(&world, &path),
        Some(("effects", "burning".into()))
    );
    assert_eq!(duration(&world, "burning"), Some(4.0));
    // `insert` drops the file the ID had.
    world
        .resource_mut::<DataRegistry>()
        .insert("burning", effect(5.0));
    assert_eq!(data_route(&world, &path), None);
}

#[test]
fn a_data_ref_reads_an_id_an_object_or_an_array() {
    let by_id: DataRef<Effect> = serde_json::from_value(json!("burning")).unwrap();
    assert_eq!(by_id.id(), Some("burning"));
    assert!(by_id.as_inline().is_none());
    assert_eq!(by_id, DataRef::by_id("burning"));

    let inline: DataRef<Effect> = serde_json::from_value(json!({ "duration": 0.5 })).unwrap();
    assert_eq!(inline.id(), None);
    assert_eq!(
        inline.as_inline(),
        Some(&Effect {
            duration: 0.5,
            trail: None
        })
    );

    let array: DataRef<Vec<u32>> = serde_json::from_value(json!([1, 2])).unwrap();
    assert_eq!(array.as_inline(), Some(&vec![1, 2]));
    let from_text: DataRef<Effect> = serde_json::from_str(r#"{ "duration": 1 }"#).unwrap();
    assert!(from_text.as_inline().is_some());

    for value in [json!(5), json!(null), json!(true)] {
        let err = serde_json::from_value::<DataRef<Effect>>(value.clone()).unwrap_err();
        assert!(
            err.to_string()
                .contains("expected a data entry's ID or its definition"),
            "{value}: {err}"
        );
    }
}

#[test]
fn an_inline_mistake_reports_its_own_field_path() {
    for (value, field, needle) in [
        (
            json!({ "effect": { "duration": "long" } }),
            "effect.duration",
            "invalid type: string \"long\", expected f32",
        ),
        (
            json!({ "effect": { "duration": 1.0, "trial": "spark" } }),
            "effect.trial",
            "unknown field `trial`",
        ),
    ] {
        let err = serde_path_to_error::deserialize::<_, Item>(value).unwrap_err();
        assert_eq!(err.path().to_string(), field);
        let message = err.into_inner().to_string();
        assert!(message.contains(needle), "{message}");
        assert!(!message.contains("variant"), "{message}");
    }
}

#[test]
fn both_forms_round_trip_and_an_inline_scalar_does_not_serialize() {
    let by_id = DataRef::<Effect>::by_id("burning");
    let saved = serde_json::to_value(&by_id).unwrap();
    assert_eq!(saved, json!("burning"));
    assert_eq!(
        serde_json::from_value::<DataRef<Effect>>(saved).unwrap(),
        by_id
    );

    let inline = DataRef::inline(Effect {
        duration: 2.0,
        trail: Some("spark".into()),
    });
    let saved = serde_json::to_value(&inline).unwrap();
    assert_eq!(saved, json!({ "duration": 2.0, "trail": "spark" }));
    assert_eq!(
        serde_json::from_value::<DataRef<Effect>>(saved).unwrap(),
        inline
    );

    let list = DataRef::inline(vec![1_u32, 2]);
    let saved = serde_json::to_string(&list).unwrap();
    assert_eq!(
        serde_json::from_str::<DataRef<Vec<u32>>>(&saved).unwrap(),
        list
    );

    let text = serde_json::to_value(DataRef::inline("burning".to_owned())).unwrap_err();
    assert!(text.to_string().contains("reads back as an ID"), "{text}");
    assert!(serde_json::to_value(DataRef::inline(5_u32)).is_err());
}

#[test]
fn resolve_reads_the_definition_in_place_or_the_entry_now() {
    let mut data = DataRegistry::new();
    data.register::<Effect>("effects");
    let effect = |duration| Effect {
        duration,
        trail: None,
    };
    data.insert("burning", effect(1.0));
    let inline = DataRef::inline(effect(9.0));
    assert_eq!(inline.resolve(&data), Some(&effect(9.0)));
    let by_id = DataRef::<Effect>::by_id("burning");
    assert_eq!(by_id.resolve(&data), Some(&effect(1.0)));
    assert_eq!(DataRef::<Effect>::by_id("rotten").resolve(&data), None);
    data.insert("burning", effect(2.0));
    assert_eq!(by_id.resolve(&data), Some(&effect(2.0)), "a replace shows");
}

#[test]
fn check_names_a_missing_id_an_inline_field_or_an_unregistered_type() {
    let mut world = world();
    world.resource_mut::<DataRegistry>().insert(
        "burning",
        Effect {
            duration: 1.0,
            trail: None,
        },
    );
    assert_eq!(
        DataRef::<Effect>::by_id("burning").check(&world, "on_hit"),
        Ok(())
    );
    assert_eq!(
        DataRef::<Effect>::by_id("burnin").check(&world, "on_hit"),
        Err(MissingAsset::new("on_hit", "effects", "burnin"))
    );
    let trail = DataRef::inline(Effect {
        duration: 1.0,
        trail: Some("spark".into()),
    });
    assert_eq!(
        trail.check(&world, "on_hit"),
        Err(MissingAsset::new("on_hit.trail", "prefab", "spark"))
    );
    assert_eq!(
        trail.check(&world, ""),
        Err(MissingAsset::new("trail", "prefab", "spark"))
    );
    let unregistered = DataRef::<u8>::by_id("one").check(&world, "count");
    assert_eq!(
        unregistered,
        Err(MissingAsset::new("count", "data kind", "u8"))
    );
    let bare = DataRef::<Effect>::inline(Effect {
        duration: 1.0,
        trail: None,
    });
    assert!(bare.check(&World::new(), "on_hit").is_err(), "no registry");
}

#[test]
fn check_prefab_id_reads_the_prefab_registry() {
    let mut world = World::new();
    assert_eq!(
        world.check_prefab_id("spark", "spark"),
        Err(MissingAsset::new("spark", "prefab", "spark")),
        "no registry"
    );
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert(
        "spark".into(),
        serde_json::from_value(json!({ "tag": "spark" })).unwrap(),
    );
    world.insert_resource(prefabs);
    assert_eq!(world.check_prefab_id("spark", "spark"), Ok(()));
    assert_eq!(
        world.check_prefab_id("trail", "sparks"),
        Err(MissingAsset::new("trail", "prefab", "sparks"))
    );
}

#[test]
fn a_prefab_and_an_entry_naming_each_other_load_in_either_order() {
    let files = Files::new();
    let effect = files.write("burning.json", r#"{ "duration": 1.0, "trail": "spark" }"#);
    let prefab = files.write(
        "spark.json",
        r#"{ "components": { "projectile": { "on_hit": "burning" } } }"#,
    );
    for data_first in [true, false] {
        let mut world = world();
        let mut load = StagedLoad::new();
        if data_first {
            load.add_data(&world, "effects", "burning", &effect)
                .unwrap();
            load.add_prefab("spark", &prefab).unwrap();
        } else {
            load.add_prefab("spark", &prefab).unwrap();
            load.add_data(&world, "effects", "burning", &effect)
                .unwrap();
        }
        assert_eq!(load.len(), 2);
        assert!(format!("{load:?}").contains("effects.burning"));
        load.commit(&mut world).unwrap();
        assert_eq!(duration(&world, "burning"), Some(1.0));
        assert!(world.resource::<PrefabRegistry>().get("spark").is_some());
        assert_eq!(prefab_route(&world, &prefab).as_deref(), Some("spark"));
    }
}

#[test]
fn a_failed_commit_restores_every_staged_entry_and_names_the_first() {
    let files = Files::new();
    let burning = files.write("burning.json", r#"{ "duration": 1.0 }"#);
    let bolt = files.write(
        "bolt.json",
        r#"{ "components": { "projectile": { "on_hit": "burning" } } }"#,
    );
    let mut world = world();
    let mut load = StagedLoad::new();
    load.add_data(&world, "effects", "burning", &burning)
        .unwrap();
    load.add_prefab("bolt", &bolt).unwrap();
    load.commit(&mut world).unwrap();

    // A replaced entry that fails, an added one that passes, a failing item
    // of a later kind and a failing prefab: the effect is named first.
    let burning2 = files.write("burning2.json", r#"{ "duration": 2.0, "trail": "nope" }"#);
    let frozen = files.write("frozen.json", r#"{ "duration": 3.0 }"#);
    let axe = files.write("axe.json", r#"{ "effect": "rotten" }"#);
    let zap = files.write(
        "zap.json",
        r#"{ "components": { "projectile": { "on_hit": "missing" } } }"#,
    );
    let mut load = StagedLoad::new();
    load.add_data(&world, "items", "axe", &axe).unwrap();
    load.add_data(&world, "effects", "frozen", &frozen).unwrap();
    load.add_data(&world, "effects", "burning", &burning2)
        .unwrap();
    load.add_prefab("zap", &zap).unwrap();
    let err = load.commit(&mut world).unwrap_err();
    assert!(
        matches!(&err, StagedLoadError::Data { kind, id, .. } if kind == "effects" && id == "burning"),
        "{err}"
    );
    let message = err.to_string();
    assert!(message.starts_with("data `effects` entry `burning`: invalid data file '"));
    assert!(
        message.contains(&burning2.display().to_string()),
        "{message}"
    );
    assert!(
        message.ends_with(", field `trail`: unknown prefab `nope`"),
        "{message}"
    );

    assert_eq!(duration(&world, "burning"), Some(1.0), "the replaced entry");
    assert_eq!(
        data_route(&world, &burning),
        Some(("effects", "burning".into()))
    );
    assert_eq!(data_route(&world, &burning2), None);
    assert_eq!(duration(&world, "frozen"), None, "the added entry is gone");
    assert_eq!(data_route(&world, &frozen), None);
    assert!(
        world
            .resource::<DataRegistry>()
            .ids("items")
            .next()
            .is_none()
    );
    assert!(world.resource::<PrefabRegistry>().get("zap").is_none());
    assert_eq!(prefab_route(&world, &zap), None);
    assert_eq!(prefab_route(&world, &bolt).as_deref(), Some("bolt"));

    // Data entries are checked before prefabs.
    let mut load = StagedLoad::new();
    load.add_data(&world, "items", "axe", &axe).unwrap();
    load.add_prefab("aaa", &zap).unwrap();
    let err = load.commit(&mut world).unwrap_err();
    assert!(
        matches!(&err, StagedLoadError::Data { kind, id, .. } if kind == "items" && id == "axe"),
        "{err}"
    );
    assert!(
        err.to_string()
            .ends_with("field `effect`: unknown effects `rotten`")
    );
    let mut load = StagedLoad::new();
    load.add_prefab("aaa", &zap).unwrap();
    let err = load.commit(&mut world).unwrap_err();
    assert!(matches!(&err, StagedLoadError::Prefab { id, .. } if id == "aaa"));
    assert!(
        err.to_string()
            .starts_with("prefab `aaa`: invalid prefab '")
    );
}

#[test]
fn a_staged_entry_that_breaks_an_unchanged_one_is_refused() {
    let files = Files::new();
    let tags = files.write("tags.json", r#"{ "names": ["enemy", "ally"] }"#);
    let slime = files.write(
        "slime.json",
        r#"{ "components": { "filter": { "tag": "enemy" } } }"#,
    );
    let mut world = world();
    let mut load = StagedLoad::new();
    load.add_data(&world, "tags", "all", &tags).unwrap();
    load.add_prefab("slime", &slime).unwrap();
    load.commit(&mut world).unwrap();

    let renamed = files.write("tags2.json", r#"{ "names": ["foe", "ally"] }"#);
    let mut load = StagedLoad::new();
    load.add_data(&world, "tags", "all", &renamed).unwrap();
    let err = load.commit(&mut world).unwrap_err();
    assert!(
        matches!(&err, StagedLoadError::Prefab { id, .. } if id == "slime"),
        "the unchanged prefab is named: {err}"
    );
    assert!(err.to_string().contains(&slime.display().to_string()));
    assert!(
        err.to_string()
            .ends_with("field `tag`: unknown tag `enemy`"),
        "{err}"
    );
    let declared = world.resource::<DataRegistry>().get::<Tags>("all").unwrap();
    assert_eq!(
        declared.names,
        ["enemy", "ally"],
        "the previous entry stays"
    );
}

#[test]
fn entries_without_a_file_are_taken_as_given() {
    let files = Files::new();
    let bolt = files.write(
        "bolt.json",
        r#"{ "components": { "projectile": { "on_hit": { "duration": 1.0 } } } }"#,
    );
    let mut world = world();
    let mut load = StagedLoad::new();
    load.add_prefab("bolt", &bolt).unwrap();
    load.commit(&mut world).unwrap();

    // An entry inserted in code that would fail, and a loaded prefab replaced
    // in code by one that would fail: neither is checked or routed.
    world.resource_mut::<DataRegistry>().insert(
        "ghost",
        Effect {
            duration: 1.0,
            trail: Some("nowhere".into()),
        },
    );
    let broken: PrefabData =
        serde_json::from_value(json!({ "components": { "projectile": { "on_hit": "rotten" } } }))
            .unwrap();
    world
        .resource_mut::<PrefabRegistry>()
        .insert("bolt".into(), broken);
    assert_eq!(prefab_route(&world, &bolt), None);

    let frozen = files.write("frozen.json", r#"{ "duration": 3.0 }"#);
    let mut load = StagedLoad::new();
    load.add_data(&world, "effects", "frozen", &frozen).unwrap();
    load.commit(&mut world).unwrap();
    assert_eq!(duration(&world, "frozen"), Some(3.0));
    assert_eq!(duration(&world, "ghost"), Some(1.0));
}

#[test]
fn a_staged_entry_takes_its_file_and_a_failure_gives_it_back() {
    for passes in [true, false] {
        let files = Files::new();
        let file = files.write("shared.json", r#"{ "tag": "old" }"#);
        let mut world = world();
        let mut load = StagedLoad::new();
        load.add_prefab("old", &file).unwrap();
        load.commit(&mut world).unwrap();
        assert_eq!(prefab_route(&world, &file).as_deref(), Some("old"));

        // The manifest drops the prefab and gives its file to a data entry.
        let trail = if passes { "old" } else { "gone" };
        std::fs::write(
            &file,
            format!(r#"{{ "duration": 1.0, "trail": "{trail}" }}"#),
        )
        .unwrap();
        let mut load = StagedLoad::new();
        load.add_data(&world, "effects", "new", &file).unwrap();
        let committed = load.commit(&mut world);
        assert_eq!(committed.is_ok(), passes, "{committed:?}");
        assert!(world.resource::<PrefabRegistry>().get("old").is_some());
        if passes {
            assert_eq!(data_route(&world, &file), Some(("effects", "new".into())));
            assert_eq!(
                prefab_route(&world, &file),
                None,
                "the file routes to one entry"
            );
        } else {
            assert_eq!(data_route(&world, &file), None);
            assert_eq!(prefab_route(&world, &file).as_deref(), Some("old"));
            assert_eq!(duration(&world, "new"), None);
        }
    }
}

#[test]
fn phase_one_errors_name_the_file_and_the_field() {
    let files = Files::new();
    let world = world();
    let mut load = StagedLoad::new();
    let cases = [
        (
            "repeat.json",
            r#"{ "duration": 1.0, "duration": 2.0 }"#,
            "duplicate field `duration`",
        ),
        (
            "deep.json",
            r#"{ "effect": { "duration": 1, "duration": 2 } }"#,
            "duplicate field `duration`",
        ),
        ("text.json", r#"{ "duration": "#, "EOF"),
    ];
    for (name, contents, needle) in cases {
        let path = files.write(name, contents);
        let kind = if name == "deep.json" {
            "items"
        } else {
            "effects"
        };
        let err = load.add_data(&world, kind, "x", &path).unwrap_err();
        assert!(matches!(err, DataError::Parse { .. }), "{name}: {err}");
        let message = err.to_string();
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(message.contains(needle), "{name}: {message}");
    }

    let path = files.write("long.json", r#"{ "duration": "long" }"#);
    let err = load.add_data(&world, "effects", "x", &path).unwrap_err();
    assert!(matches!(&err, DataError::Invalid { field, .. } if field == "duration"));
    assert!(err.to_string().contains(&format!(
        "invalid data file '{}', field `duration`: invalid type",
        path.display()
    )));
    let path = files.write(
        "inline.json",
        r#"{ "effect": { "duration": 1, "trial": "x" } }"#,
    );
    let err = load.add_data(&world, "items", "x", &path).unwrap_err();
    assert!(
        matches!(&err, DataError::Invalid { field, .. } if field == "effect.trial"),
        "{err}"
    );

    let missing = files.dir.join("missing.json");
    let err = load.add_data(&world, "effects", "x", &missing).unwrap_err();
    assert!(matches!(err, DataError::Io { .. }));
    assert!(err.to_string().starts_with("failed to read data file '"));

    let err = load.add_data(&world, "efects", "x", &missing).unwrap_err();
    assert!(
        matches!(&err, DataError::UnknownKind { suggestion: Some(near), .. } if near == "effects")
    );
    assert_eq!(
        err.to_string(),
        format!(
            "data file '{}': unknown data kind `efects` (registered: `effects`, `items`, `tags`); did you mean `effects`?",
            missing.display()
        )
    );
    let err = load
        .add_data(&World::new(), "effects", "x", &missing)
        .unwrap_err();
    assert!(err.to_string().contains("(registered: none)"), "{err}");
    assert!(load.is_empty(), "nothing failed staged");
}

#[test]
fn a_second_add_replaces_the_first() {
    let files = Files::new();
    let one = files.write("one.json", r#"{ "duration": 1.0 }"#);
    let two = files.write("two.json", r#"{ "duration": 2.0 }"#);
    let tag_a = files.write("a.json", r#"{ "tag": "a" }"#);
    let tag_b = files.write("b.json", r#"{ "tag": "b" }"#);
    let mut world = world();
    let mut load = StagedLoad::new();
    load.add_data(&world, "effects", "burning", &one).unwrap();
    load.add_data(&world, "effects", "burning", &two).unwrap();
    load.add_prefab("bat", &tag_a).unwrap();
    load.add_prefab("bat", &tag_b).unwrap();
    assert_eq!(load.len(), 2);
    load.commit(&mut world).unwrap();
    assert_eq!(duration(&world, "burning"), Some(2.0));
    assert_eq!(data_route(&world, &one), None);
    assert_eq!(
        data_route(&world, &two),
        Some(("effects", "burning".into()))
    );
    let prefabs = world.resource::<PrefabRegistry>();
    assert_eq!(prefabs.get("bat").unwrap().tag.as_deref(), Some("b"));
}

#[test]
fn a_commit_on_a_bare_world_inserts_both_registries() {
    let mut world = World::new();
    StagedLoad::new().commit(&mut world).unwrap();
    assert!(world.has_resource::<DataRegistry>() && world.has_resource::<PrefabRegistry>());

    let files = Files::new();
    let path = files.write("bat.json", r#"{ "tag": "bat" }"#);
    let mut world = World::new();
    let mut load = StagedLoad::new();
    load.add_prefab("bat", &path).unwrap();
    load.commit(&mut world).unwrap();
    assert_eq!(prefab_route(&world, &path).as_deref(), Some("bat"));
    assert!(world.resource::<DataRegistry>().is_empty());

    // A prefab naming an unregistered component fails on a bare World.
    let path = files.write(
        "rat.json",
        r#"{ "components": { "health": { "max": 1 } } }"#,
    );
    let mut load = StagedLoad::new();
    load.add_prefab("rat", &path).unwrap();
    let err = load.commit(&mut world).unwrap_err();
    assert!(
        err.to_string().ends_with("unknown component `health`"),
        "{err}"
    );
}

#[test]
fn a_missing_prefab_or_data_id_suggests_the_nearest() {
    let files = Files::new();
    let burning = files.write("burning.json", r#"{ "duration": 1.0, "trail": "sparks" }"#);
    let spark = files.write(
        "spark.json",
        r#"{ "components": { "projectile": { "on_hit": "burnin" } } }"#,
    );
    let mut world = world();
    let mut load = StagedLoad::new();
    load.add_data(&world, "effects", "burning", &burning)
        .unwrap();
    load.add_prefab("spark", &spark).unwrap();
    let err = load.commit(&mut world).unwrap_err();
    assert!(
        err.to_string()
            .ends_with("field `trail`: unknown prefab `sparks`; did you mean `spark`?"),
        "{err}"
    );
    let fixed = files.write("fixed.json", r#"{ "duration": 1.0, "trail": "spark" }"#);
    let mut load = StagedLoad::new();
    load.add_data(&world, "effects", "burning", &fixed).unwrap();
    load.add_prefab("spark", &spark).unwrap();
    let err = load.commit(&mut world).unwrap_err();
    assert!(
        err.to_string().ends_with(
            "component `projectile`, field `on_hit`: unknown effects `burnin`; did you mean `burning`?"
        ),
        "{err}"
    );
}

#[test]
fn the_nearest_name_is_within_its_bound_and_ties_go_first() {
    let names = ["burning", "turning", "zz"];
    assert_eq!(nearest("burnin", names.into_iter()), Some("burning"));
    assert_eq!(
        nearest("xurning", names.into_iter()),
        Some("burning"),
        "a tie"
    );
    assert_eq!(
        nearest("burning", names.into_iter()),
        Some("turning"),
        "not itself"
    );
    assert_eq!(
        nearest("z", names.into_iter()),
        Some("zz"),
        "a bound of at least 1"
    );
    assert_eq!(nearest("abc", ["abd"].into_iter()), Some("abd"));
    assert_eq!(nearest("abc", ["axy"].into_iter()), None, "2 edits past 1");
    assert_eq!(
        nearest("abcdefghi", ["abcxyzghi"].into_iter()),
        Some("abcxyzghi")
    );
    assert_eq!(
        nearest("abcdefghi", ["wxyzefghi"].into_iter()),
        None,
        "4 past 3"
    );
    assert_eq!(
        nearest("éa", ["ea"].into_iter()),
        Some("ea"),
        "chars, not bytes"
    );
    assert_eq!(nearest("a", std::iter::empty()), None);
}

#[test]
fn a_commit_refuses_a_kind_the_world_lacks_or_holds_for_another_type() {
    let files = Files::new();
    let burning = files.write("burning.json", r#"{ "duration": 1.0 }"#);
    let mut load = StagedLoad::new();
    load.add_data(&world(), "effects", "burning", &burning)
        .unwrap();

    // Committed to a bare World: nothing changes, not even the registries.
    let mut bare = World::new();
    let err = load.commit(&mut bare).unwrap_err();
    assert!(
        matches!(&err, StagedLoadError::Data { kind, id, source }
            if kind == "effects" && id == "burning" && matches!(**source, DataError::UnknownKind { .. })),
        "{err}"
    );
    assert!(
        err.to_string()
            .contains("unknown data kind `effects` (registered: none)")
    );
    assert!(!bare.has_resource::<DataRegistry>() && !bare.has_resource::<PrefabRegistry>());

    // Committed to a World whose `effects` holds another type.
    let mut load = StagedLoad::new();
    load.add_data(&world(), "effects", "burning", &burning)
        .unwrap();
    let mut other = World::new();
    other.register_data_kind::<Tags>("effects");
    let err = load.commit(&mut other).unwrap_err();
    let message = err.to_string();
    assert!(
        matches!(&err, StagedLoadError::Data { source, .. } if matches!(**source, DataError::MismatchedKind { .. })),
        "{message}"
    );
    assert!(message.contains("data kind `effects` holds `"), "{message}");
    assert!(
        message.contains("Tags`") && message.contains("Effect`"),
        "{message}"
    );
    assert!(other.resource::<DataRegistry>().is_empty());
}

#[test]
fn a_staged_file_routes_by_its_canonical_path() {
    let files = Files::new();
    files.write("burning.json", r#"{ "duration": 1.0 }"#);
    files.write("bat.json", r#"{ "tag": "bat" }"#);
    std::fs::create_dir_all(files.dir.join("sub")).unwrap();
    let (data, prefab) = (
        files.dir.join("sub/../burning.json"),
        files.dir.join("sub/../bat.json"),
    );
    let mut world = world();
    let mut load = StagedLoad::new();
    load.add_data(&world, "effects", "burning", &data).unwrap();
    load.add_prefab("bat", &prefab).unwrap();
    load.commit(&mut world).unwrap();
    let canonical = data.canonicalize().unwrap();
    assert_ne!(canonical, data);
    assert_eq!(
        data_route(&world, &canonical),
        Some(("effects", "burning".into()))
    );
    assert_eq!(data_route(&world, &data), None);
    assert_eq!(
        prefab_route(&world, &prefab.canonicalize().unwrap()).as_deref(),
        Some("bat")
    );
}

#[test]
fn commit_for_check_returns_every_failure_in_commits_order_and_keeps_them_in_place() {
    let files = Files::new();
    let burning = files.write("burning.json", r#"{ "duration": 1.0, "trail": "nope" }"#);
    let zap = files.write(
        "zap.json",
        r#"{ "components": { "projectile": { "on_hit": "rotten" } } }"#,
    );
    let bolt = files.write(
        "bolt.json",
        r#"{ "components": { "projectile": { "on_hit": "stale" } } }"#,
    );
    let mut world = world();
    let mut load = StagedLoad::new();
    load.add_prefab("zap", &zap).unwrap();
    load.add_prefab("bolt", &bolt).unwrap();
    load.add_data(&world, "effects", "burning", &burning)
        .unwrap();
    let failures = load.commit_for_check(&mut world);
    let names: Vec<String> = failures
        .iter()
        .map(|failure| match failure {
            StagedLoadError::Data { kind, id, .. } => format!("{kind}.{id}"),
            StagedLoadError::Prefab { id, .. } => format!("prefabs.{id}"),
        })
        .collect();
    assert_eq!(names, ["effects.burning", "prefabs.bolt", "prefabs.zap"]);
    assert!(failures[0].to_string().ends_with("unknown prefab `nope`"));
    assert!(failures[1].to_string().contains("stale"), "{}", failures[1]);
    // Nothing is taken back.
    assert_eq!(duration(&world, "burning"), Some(1.0));
    assert!(world.resource::<PrefabRegistry>().get("zap").is_some());
    assert!(world.resource::<PrefabRegistry>().get("bolt").is_some());
}

#[test]
fn commit_for_check_keeps_out_a_kind_the_world_holds_for_another_type() {
    let files = Files::new();
    let burning = files.write("burning.json", r#"{ "duration": 1.0 }"#);
    let staged_on = world();
    let mut load = StagedLoad::new();
    load.add_data(&staged_on, "effects", "burning", &burning)
        .unwrap();
    let mut other = World::new();
    other.register_data_kind::<Tags>("effects");
    let failures = load.commit_for_check(&mut other);
    assert_eq!(failures.len(), 1);
    assert!(
        matches!(&failures[0], StagedLoadError::Data { source, .. }
            if matches!(**source, DataError::MismatchedKind { .. })),
        "{}",
        failures[0]
    );
    assert!(
        other
            .resource::<DataRegistry>()
            .ids("effects")
            .next()
            .is_none()
    );
}
