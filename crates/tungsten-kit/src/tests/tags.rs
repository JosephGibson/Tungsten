use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, PrefabData, PrefabRegistry};

#[test]
fn tags_sort_deduplicate_and_carry_their_ancestors() {
    let tags = Tags::new(["team.enemy", "flying", "team.enemy", "boss"]);
    assert_eq!(
        tags.iter().collect::<Vec<_>>(),
        ["boss", "flying", "team.enemy"]
    );
    assert_eq!(tags.len(), 3);
    assert!(tags.has("team") && tags.has("team.enemy") && tags.has("boss"));
    assert!(!tags.has("team.enemy.red") && !tags.has("tea") && !tags.has("fly"));
    assert!(Tags::default().is_empty());
}

#[test]
fn insert_and_remove_keep_the_order() {
    let mut tags = Tags::new(["flying"]);
    assert!(tags.insert("boss"));
    assert!(!tags.insert("boss"));
    assert_eq!(tags.iter().collect::<Vec<_>>(), ["boss", "flying"]);
    assert!(tags.remove("boss"));
    assert!(!tags.remove("boss"));
    assert!(!tags.remove("fly"), "an exact name, not an ancestor");
    assert_eq!(tags.iter().collect::<Vec<_>>(), ["flying"]);
}

#[test]
#[should_panic(expected = "is not a tag name")]
fn an_invalid_name_panics() {
    let _ = Tags::new(["Team.Player"]);
}

#[test]
#[should_panic(expected = "is not a tag name")]
fn an_empty_segment_panics_on_insert() {
    let mut tags = Tags::default();
    tags.insert("team..player");
}

#[test]
fn tags_are_data() {
    let tags: Tags = serde_json::from_value(json!(["team.player", "flying"])).unwrap();
    assert_eq!(tags, Tags::new(["flying", "team.player"]));
    assert_eq!(
        serde_json::to_value(&tags).unwrap(),
        json!(["flying", "team.player"])
    );
    let error = serde_json::from_value::<Tags>(json!(["1up"])).unwrap_err();
    assert!(error.to_string().contains("1up"), "{error}");
}

#[test]
fn a_filter_needs_all_takes_any_and_refuses_none() {
    let grunt = Tags::new(["team.enemy"]);
    let boss = Tags::new(["team.enemy", "boss", "flying"]);
    let hero = Tags::new(["team.player"]);

    let enemies = TagFilter::new().all(["team.enemy"]);
    assert!(enemies.matches(Some(&grunt)) && enemies.matches(Some(&boss)));
    assert!(!enemies.matches(Some(&hero)) && !enemies.matches(None));

    let airborne_or_boss = TagFilter::new().any(["flying", "boss"]);
    assert!(airborne_or_boss.matches(Some(&boss)));
    assert!(!airborne_or_boss.matches(Some(&grunt)) && !airborne_or_boss.matches(None));

    let not_bosses = TagFilter::new().none(["boss"]);
    assert!(not_bosses.matches(Some(&grunt)) && not_bosses.matches(Some(&hero)));
    assert!(
        not_bosses.matches(None),
        "no `all` and no `any`: untagged passes"
    );
    assert!(!not_bosses.matches(Some(&boss)));

    let everything = TagFilter::new();
    assert!(everything.is_everything() && everything.matches(None));
    assert!(!not_bosses.is_everything());

    let team = TagFilter::new().all(["team"]);
    assert!(team.matches(Some(&hero)), "an ancestor matches");
}

#[test]
fn a_filter_is_data_with_optional_fields() {
    let filter: TagFilter = serde_json::from_value(json!({ "all": ["team.player"] })).unwrap();
    assert_eq!(filter, TagFilter::new().all(["team.player"]));
    assert_eq!(
        serde_json::to_value(&filter).unwrap(),
        json!({ "all": ["team.player"] })
    );
    let empty: TagFilter = serde_json::from_value(json!({})).unwrap();
    assert!(empty.is_everything());
    assert_eq!(serde_json::to_value(&empty).unwrap(), json!({}));
    assert!(serde_json::from_value::<TagFilter>(json!({ "every": [] })).is_err());
    let error = serde_json::from_value::<TagFilter>(json!({ "none": ["Boss"] })).unwrap_err();
    assert!(error.to_string().starts_with("none: "), "{error}");
}

#[test]
#[should_panic(expected = "TagFilter::any:")]
fn a_filter_with_an_invalid_name_panics() {
    let _ = TagFilter::new().any(["no spaces"]);
}

/// A World with the plugin built, declaring `declared` at build.
fn rig(declared: &[&str]) -> World {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(declared.iter().copied())
        .build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    world
}

fn check(world: &World, value: &serde_json::Value) -> Result<(), String> {
    world
        .resource::<ComponentRegistry>()
        .check_in(TAGS, value, world)
        .map_err(|error| error.to_string())
}

#[test]
fn the_load_checks_tags_against_the_build_declarations() {
    let world = rig(&["team.player", "boss"]);
    assert_eq!(check(&world, &json!(["team.player", "boss"])), Ok(()));
    assert_eq!(check(&world, &json!(["team"])), Ok(()), "an ancestor");
    let error = check(&world, &json!(["team.enemy", "boss"])).unwrap_err();
    assert!(
        error.contains("team.enemy") && error.contains("tag"),
        "{error}"
    );
    assert!(error.contains("`0`"), "the index as written: {error}");
    assert!(check(&world, &json!(["Boss"])).is_err(), "checked names");
}

#[test]
fn the_kit_declares_its_tracks_states_and_sides() {
    let world = rig(&[]);
    for name in [
        "delivery.hit",
        "delivery.dot",
        "delivery.pierced",
        "delivery.forked",
        "delivery.chained",
        "delivery.repeated",
        "side.front",
        "side.rear",
        "delivery",
        "side",
    ] {
        assert_eq!(check(&world, &json!([name])), Ok(()), "{name}");
    }
    assert!(check(&world, &json!(["side.left"])).is_err());
    assert!(check(&world, &json!(["delivery.hit.crit"])).is_err());
    assert_eq!(KIT_TAGS.len(), 8);
}

#[test]
fn the_tags_data_kind_declares_the_rest() {
    let mut world = rig(&[]);
    assert!(check(&world, &json!(["team.enemy"])).is_err());
    world
        .resource_mut::<DataRegistry>()
        .insert("game", TagList(Tags::new(["team.enemy", "flying"])));
    assert_eq!(check(&world, &json!(["team.enemy", "team"])), Ok(()));
    assert!(check(&world, &json!(["boss"])).is_err());
    assert!(world.resource::<DataRegistry>().contains_kind(TAGS));
}

#[test]
fn the_scene_component_spawns_the_tags() {
    let mut world = rig(&["team.player"]);
    let hero: PrefabData =
        serde_json::from_value(json!({ "components": { "tags": ["team.player"] } })).unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("hero".to_string(), hero);
    world.insert_resource(prefabs);
    let entity = world
        .prepare_prefab("hero", [0.0, 0.0])
        .unwrap()
        .spawn(&mut world);
    assert_eq!(world.get::<Tags>(entity), Some(&Tags::new(["team.player"])));
}

#[test]
fn a_second_plugin_adds_its_declarations() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.player"])
        .build(&mut schedule, &mut world);
    TagsPlugin::new()
        .with_tags(["boss"])
        .build(&mut schedule, &mut world);
    assert_eq!(check(&world, &json!(["team.player", "boss"])), Ok(()));
}

/// A World with the tags plugin built, its hooks registered.
fn hooked() -> World {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.player", "team.enemy"])
        .build(&mut schedule, &mut world);
    world
}

/// A body tagged `names` at `at`, moving.
fn body(world: &mut World, names: &[&str], at: [f32; 2]) -> Entity {
    use glam::Vec2;
    use tungsten_core::{Position, PrevPosition, Velocity};
    world.spawn_with((
        Tags::new(names.iter().copied()),
        Position(Vec2::from(at)),
        PrevPosition(Vec2::from(at)),
        Velocity(Vec2::new(30.0, -5.0)),
    ))
}

#[test]
fn place_sets_the_first_tagged_entity_at_rest() {
    use glam::Vec2;
    use tungsten_core::script::ScriptHooks;
    use tungsten_core::{Position, PrevPosition, Velocity};
    let mut world = hooked();
    let enemy = body(&mut world, &["team.enemy"], [0.0, 0.0]);
    let first = body(&mut world, &["team.player"], [10.0, 10.0]);
    let second = body(&mut world, &["team.player"], [20.0, 20.0]);
    let args = json!({ "tag": "team.player", "at": [900.0, 400.0] });
    ScriptHooks::call_action(&mut world, PLACE_ACTION, &args).unwrap();
    let at = Vec2::new(900.0, 400.0);
    assert_eq!(world.get::<Position>(first).unwrap().0, at);
    assert_eq!(world.get::<PrevPosition>(first).unwrap().0, at, "no slide");
    assert_eq!(world.get::<Velocity>(first).unwrap().0, Vec2::ZERO);
    assert_eq!(
        world.get::<Position>(second).unwrap().0,
        Vec2::new(20.0, 20.0)
    );
    assert_eq!(world.get::<Position>(enemy).unwrap().0, Vec2::ZERO);
    // An ancestor names the team: the first entity holding any team.
    let args = json!({ "tag": "team", "at": [1.0, 2.0] });
    ScriptHooks::call_action(&mut world, PLACE_ACTION, &args).unwrap();
    assert_eq!(world.get::<Position>(enemy).unwrap().0, Vec2::new(1.0, 2.0));
    let doc = world
        .resource::<ScriptHooks>()
        .actions()
        .find(|(name, _)| *name == PLACE_ACTION)
        .map(|(_, doc)| doc);
    assert!(doc.is_some_and(|doc| !doc.is_empty()), "described");
}

#[test]
fn place_fails_with_no_tagged_entity() {
    use tungsten_core::script::ScriptHooks;
    let mut world = hooked();
    let args = json!({ "tag": "team.player", "at": [1.0, 2.0] });
    let error = ScriptHooks::call_action(&mut world, PLACE_ACTION, &args).unwrap_err();
    assert!(error.contains("no entity holds `team.player`"), "{error}");
    world.spawn_with((Tags::new(["team.player"]),));
    let error = ScriptHooks::call_action(&mut world, PLACE_ACTION, &args).unwrap_err();
    assert!(error.contains("no position"), "{error}");
    let error =
        ScriptHooks::call_action(&mut world, PLACE_ACTION, &json!({ "tag": "team" })).unwrap_err();
    assert!(error.contains("`at`"), "{error}");
    // A second plugin keeps the hooks it finds.
    TagsPlugin::new().build(&mut Schedule::new(), &mut world);
}

#[test]
fn tagged_reads_position_and_health() {
    use tungsten_core::script::ScriptHooks;
    let mut world = hooked();
    let read = |world: &World, tag: &str| {
        ScriptHooks::read_probe(world, TAGGED_PROBE, &json!({ "tag": tag })).unwrap()
    };
    assert_eq!(read(&world, "team.player"), serde_json::Value::Null, "none");
    let player = body(&mut world, &["team.player"], [3.0, 4.0]);
    assert_eq!(
        read(&world, "team.player"),
        json!({ "position": [3.0, 4.0], "health": null })
    );
    world.insert(player, crate::Health::new(100.0));
    assert_eq!(
        read(&world, "team.player"),
        json!({ "position": [3.0, 4.0], "health": [100.0, 100.0] })
    );
    let error =
        ScriptHooks::read_probe(&world, TAGGED_PROBE, &serde_json::Value::Null).unwrap_err();
    assert!(error.contains("probe `tagged`"), "{error}");
}
