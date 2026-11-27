//! Capture and restore of the global record and a code-built area: patches
//! over the fresh spawn, a run's spawns, references, versions and the
//! checks that change nothing.

use serde_json::json;

use super::*;
use crate::assets::{PrefabData, PrefabRegistry};
use crate::components::Tag;
use crate::physics::PhysicsPlugin;
use crate::save::{RefCodec, RefReader, RefWriter, SaveOptions, name};
use crate::schedule::{Plugin, Schedule};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Health {
    current: f32,
    max: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shield: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Brain {
    mood: String,
    #[serde(default)]
    timer: f32,
    #[serde(default)]
    seen: Vec<u32>,
}

/// The entity it chases: a reference its saved form leaves out.
#[derive(Debug, Clone, PartialEq)]
struct Chase {
    target: Entity,
    speed: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChaseForm {
    speed: f32,
}

impl From<&Chase> for ChaseForm {
    fn from(chase: &Chase) -> Self {
        Self { speed: chase.speed }
    }
}

impl From<ChaseForm> for Chase {
    fn from(form: ChaseForm) -> Self {
        Self {
            target: Entity::PLACEHOLDER,
            speed: form.speed,
        }
    }
}

fn save_chase(chase: &Chase, out: &mut RefWriter<'_>) {
    out.put("target", chase.target);
}

fn load_chase(chase: &mut Chase, refs: &RefReader<'_>) {
    if let Some(target) = refs.first("target") {
        chase.target = target;
    }
}

fn chase_keys(_: &Chase) -> Vec<String> {
    vec!["target".into()]
}

/// The shots a turret has left: paid before its spawn queues.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Ammo(u32);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Weather {
    raining: bool,
    wind: f32,
}

/// What a restored spawn takes, as a state's marker.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Scene;

fn prefab(value: Value) -> PrefabData {
    serde_json::from_value(value).unwrap()
}

fn bat() -> PrefabData {
    prefab(json!({ "tag": "bat", "components": {
        "health": { "current": 10.0, "max": 10.0, "shield": 3.0 },
        "brain": { "mood": "calm" } } }))
}

/// A World with the test components saved, `health` with `options`, and
/// the prefabs `hero`, `bat`, `turret` and `bolt`.
fn world_with(options: SaveOptions<Health>) -> World {
    let mut world = World::new();
    PhysicsPlugin.build(&mut Schedule::new(), &mut world);
    world.register_component::<Health>("health");
    world.register_component::<Brain>("brain");
    world.register_component::<Ammo>("ammo");
    world.register_saved_with("health", options);
    world.register_saved::<Brain>("brain");
    world.register_saved::<Ammo>("ammo");
    world.register_saved_with(
        "chase",
        SaveOptions::<Chase>::form::<ChaseForm>().refs(RefCodec {
            save: save_chase,
            load: load_chase,
            keys: chase_keys,
        }),
    );
    world.register_saved_resource::<Weather>("weather");
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert(
        "hero".into(),
        prefab(json!({ "tag": "hero", "components": {
            "health": { "current": 20.0, "max": 20.0 } } })),
    );
    prefabs.insert("bat".into(), bat());
    prefabs.insert(
        "turret".into(),
        prefab(json!({ "tag": "turret", "components": { "ammo": 3 } })),
    );
    prefabs.insert(
        "bolt".into(),
        prefab(json!({ "tag": "bolt", "components": {
            "health": { "current": 1.0, "max": 1.0 }, "brain": { "mood": "wild" } } })),
    );
    world.insert_resource(prefabs);
    world.insert_resource(Weather {
        raining: false,
        wind: 0.0,
    });
    world.insert_resource(CommandBuffer::new());
    world
}

fn world() -> World {
    world_with(SaveOptions::new())
}

/// A run's start.
struct Level {
    hero: Entity,
    bat_1: Entity,
    bat_2: Entity,
    turret: Entity,
}

/// A new run: the hero (global) and, in the code-built area `cave`, a bat
/// chasing the hero, a bat chasing it and a turret.
fn level(world: &mut World) -> Level {
    let hero = world.spawn_prefab("hero", [16.0, 16.0]).unwrap();
    let bat_1 = world.spawn_prefab("bat", [100.0, 40.0]).unwrap();
    let bat_2 = world.spawn_prefab("bat", [140.0, 40.0]).unwrap();
    let turret = world.spawn_prefab("turret", [200.0, 40.0]).unwrap();
    world.insert(
        bat_1,
        Chase {
            target: hero,
            speed: 2.0,
        },
    );
    world.insert(
        bat_2,
        Chase {
            target: bat_1,
            speed: 1.5,
        },
    );
    name(world, hero, None, "hero").unwrap();
    name(world, bat_1, Some("cave"), "bat_1").unwrap();
    name(world, bat_2, Some("cave"), "bat_2").unwrap();
    name(world, turret, Some("cave"), "turret").unwrap();
    Level {
        hero,
        bat_1,
        bat_2,
        turret,
    }
}

/// Queues a bolt from `source` on the World's buffer.
fn shoot(world: &mut World, source: Entity, x: f32) {
    let bolt = world.prepare_prefab("bolt", [x, 40.0]).unwrap();
    world
        .resource_mut::<CommandBuffer>()
        .spawn_prefab_from(source, bolt);
}

/// The turret fires: its ammo paid, then a bolt queued.
fn fire(world: &mut World, turret: Entity, x: f32) {
    world.get_mut::<Ammo>(turret).unwrap().0 -= 1;
    shoot(world, turret, x);
}

fn flush(world: &mut World) {
    let mut buffer = std::mem::take(world.resource_mut::<CommandBuffer>());
    world.flush_reusing(&mut buffer);
    *world.resource_mut::<CommandBuffer>() = buffer;
}

/// Spawns `count` entities and despawns two in three, so later spawns take
/// other indices.
fn churn(world: &mut World, count: usize) {
    let spawned: Vec<Entity> = (0..count).map(|_| world.spawn()).collect();
    for (i, entity) in spawned.into_iter().enumerate() {
        if i % 3 != 1 {
            world.despawn(entity);
        }
    }
}

fn text(save: &WorldSave) -> String {
    serde_json::to_string(save).unwrap()
}

fn value(save: &WorldSave) -> Value {
    serde_json::to_value(save).unwrap()
}

fn from_value(value: Value) -> WorldSave {
    serde_json::from_value(value).unwrap()
}

/// The run's spawns, by save ID.
fn spawns(world: &World) -> Vec<(SaveId, Entity)> {
    let mut found: Vec<(SaveId, Entity)> = world
        .query::<(Entity, &Saved)>()
        .filter(|(_, saved)| saved.prefab().is_some())
        .map(|(entity, saved)| (saved.id(), entity))
        .collect();
    found.sort_by_key(|&(id, _)| id);
    found
}

/// Each chaser's save ID with its target's.
fn chases(world: &World) -> BTreeMap<SaveId, Option<SaveId>> {
    world
        .query::<(&Saved, &Chase)>()
        .map(|(saved, chase)| (saved.id(), world.get::<Saved>(chase.target).map(Saved::id)))
        .collect()
}

/// Every entity's registered components and every resource, as JSON.
fn snapshot(world: &World) -> (u32, Vec<BTreeMap<String, Value>>, BTreeMap<String, Value>) {
    let registry = world.resource::<SaveRegistry>();
    let mut entities: Vec<Entity> = world.query::<Entity>().collect();
    entities.sort_by_key(|entity| entity.id());
    let components = entities
        .into_iter()
        .map(|entity| {
            registry
                .components
                .entries
                .iter()
                .filter_map(|(name, entry)| {
                    let value = entry.saved.to_value(world, Holder::Entity(entity))?;
                    Some((name.clone(), value.unwrap()))
                })
                .collect()
        })
        .collect();
    let resources = registry
        .resources
        .entries
        .iter()
        .filter_map(|(name, entry)| {
            let value = entry.saved.to_value(world, Holder::Resource)?;
            Some((name.clone(), value.unwrap()))
        })
        .collect();
    (world.entity_count(), components, resources)
}

#[test]
fn a_capture_with_commands_queued_fails_and_one_after_the_flush_holds_the_spawn() {
    let mut world = world();
    let level = level(&mut world);
    fire(&mut world, level.turret, 210.0);
    let queued = world.resource::<CommandBuffer>().len();
    assert!(queued > 0);
    assert_eq!(
        capture_world(&world),
        Err(CaptureError::Queued { commands: queued })
    );

    flush(&mut world);
    let saved = value(&capture_world(&world).unwrap());
    let cave = &saved["areas"]["cave"];
    assert_eq!(
        cave["named"]["turret"]["components"],
        json!({ "ammo": 2 }),
        "the paid count"
    );
    assert_eq!(cave["spawns"].as_array().unwrap().len(), 1);
    assert_eq!(cave["spawns"][0]["id"], json!(0));
    assert_eq!(cave["spawns"][0]["prefab"], json!("bolt"));
    assert_eq!(saved["next_id"], json!(1));
}

#[test]
fn a_capture_restored_into_a_world_with_other_allocations_captures_the_same() {
    let mut a = world();
    churn(&mut a, 50);
    let la = level(&mut a);
    for x in [210.0, 220.0, 230.0] {
        fire(&mut a, la.turret, x);
    }
    shoot(&mut a, la.hero, 30.0);
    flush(&mut a);
    let bolts: Vec<Entity> = spawns(&a).into_iter().map(|(_, entity)| entity).collect();
    assert_eq!(bolts.len(), 4);
    a.get_mut::<Health>(la.bat_1).unwrap().current = 4.0;
    a.get_mut::<Brain>(la.bat_1).unwrap().mood = "angry".into();
    a.get_mut::<Chase>(la.bat_1).unwrap().target = bolts[2];
    a.insert(
        bolts[1],
        Chase {
            target: la.hero,
            speed: 9.0,
        },
    );
    a.get_mut::<Health>(bolts[0]).unwrap().current = 0.5;
    a.resource_mut::<Weather>().raining = true;
    let saved = capture_world(&a).unwrap();
    assert_eq!(saved.dropped_refs(), 0);
    assert_eq!(
        value(&saved)["global"]["spawns"][0]["id"],
        json!(3),
        "the hero's bolt, global"
    );

    let mut b = world();
    churn(&mut b, 80);
    let lb = level(&mut b);
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(text(&capture_world(&b).unwrap()), text(&saved));

    assert_eq!(chases(&a).len(), 3);
    assert_eq!(chases(&b), chases(&a), "each reference by save ID");
    assert_eq!(
        b.get::<Chase>(lb.bat_2).unwrap().target,
        lb.bat_1,
        "to B's own entity"
    );
    let ids = |world: &World| spawns(world).into_iter().map(|(id, _)| id);
    assert!(ids(&a).eq(ids(&b)));
    assert_ne!(
        spawns(&a).iter().map(|&(_, e)| e).collect::<Vec<_>>(),
        spawns(&b).iter().map(|&(_, e)| e).collect::<Vec<_>>(),
        "other entities hold the same IDs"
    );
    assert!(
        spawns(&b).iter().all(|&(_, e)| b.has::<Scene>(e)),
        "each spawn takes `with`"
    );
}

#[test]
fn a_named_entity_saves_only_the_fields_its_run_changed() {
    let mut world = world();
    let level = level(&mut world);
    let fresh = value(&capture_world(&world).unwrap());
    assert_eq!(
        fresh["areas"]["cave"]["named"]["bat_1"],
        json!({ "refs": { "chase": { "target": [SaveId::authored("hero").0] } } }),
        "an unchanged entity writes its references alone"
    );
    assert_eq!(fresh["areas"]["cave"]["named"].get("turret"), None);

    world.get_mut::<Brain>(level.bat_1).unwrap().mood = "angry".into();
    world.get_mut::<Health>(level.bat_2).unwrap().current = 7.0;
    let saved = value(&capture_world(&world).unwrap());
    let named = &saved["areas"]["cave"]["named"];
    assert_eq!(
        named["bat_1"]["components"],
        json!({ "brain": { "mood": "angry" } })
    );
    assert_eq!(
        named["bat_2"]["components"],
        json!({ "health": { "current": 7.0 } })
    );
    assert_eq!(saved["global"]["named"].get("hero"), None);
}

#[test]
fn a_retuned_field_reaches_an_old_save_and_a_changed_one_keeps_the_saves() {
    let mut a = world();
    let la = level(&mut a);
    a.get_mut::<Health>(la.bat_1).unwrap().current = 5.0;
    let saved = capture_world(&a).unwrap();

    let mut b = world();
    b.resource_mut::<PrefabRegistry>().insert(
        "bat".into(),
        prefab(json!({ "tag": "bat", "components": {
            "health": { "current": 12.0, "max": 12.0, "shield": 3.0 },
            "brain": { "mood": "calm" } } })),
    );
    let lb = level(&mut b);
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert_eq!(
        b.get::<Health>(lb.bat_1),
        Some(&Health {
            current: 5.0,
            max: 12.0,
            shield: Some(3.0)
        })
    );
    assert_eq!(b.get::<Health>(lb.bat_2).unwrap().current, 12.0);
}

#[test]
fn a_removed_component_and_a_removed_field_stay_removed() {
    let mut a = world();
    let la = level(&mut a);
    a.remove_component::<Brain>(la.bat_1);
    a.get_mut::<Health>(la.bat_1).unwrap().shield = None;
    let saved = capture_world(&a).unwrap();
    let record = &value(&saved)["areas"]["cave"]["named"]["bat_1"];
    assert_eq!(record["removed"], json!(["brain"]));
    assert_eq!(
        record["components"],
        json!({ "health": { "-": ["shield"] } })
    );

    let mut b = world();
    let lb = level(&mut b);
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(!b.has::<Brain>(lb.bat_1));
    assert_eq!(b.get::<Health>(lb.bat_1).unwrap().shield, None);
    assert!(b.has::<Brain>(lb.bat_2));
    assert_eq!(b.get::<Health>(lb.bat_2).unwrap().shield, Some(3.0));
}

#[test]
fn a_spawn_from_a_saved_source_comes_back_from_its_prefab_with_its_id() {
    let mut a = world();
    let la = level(&mut a);
    fire(&mut a, la.turret, 210.0);
    flush(&mut a);
    let bolt = spawns(&a)[0].1;
    a.get_mut::<Health>(bolt).unwrap().current = 0.25;
    let saved = capture_world(&a).unwrap();

    let mut b = world();
    level(&mut b);
    assert!(spawns(&b).is_empty());
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    let restored = spawns(&b);
    assert_eq!(restored.len(), 1);
    let (id, bolt) = restored[0];
    assert_eq!(id, SaveId(0));
    let tagged = b.get::<Saved>(bolt).unwrap();
    assert_eq!(
        (tagged.area(), tagged.prefab()),
        (Some("cave"), Some("bolt"))
    );
    assert_eq!(
        b.get::<Health>(bolt),
        Some(&Health {
            current: 0.25,
            max: 1.0,
            shield: None
        })
    );
    assert_eq!(b.get::<Tag>(bolt).unwrap().name, "bolt", "from its prefab");
    assert!(b.has::<Scene>(bolt));
    assert_eq!(b.resource::<SaveState>().next_id(), 1);
}

#[test]
fn a_spawns_removed_component_stays_removed() {
    let mut a = world();
    let la = level(&mut a);
    fire(&mut a, la.turret, 210.0);
    flush(&mut a);
    let bolt = spawns(&a)[0].1;
    assert!(a.has::<Brain>(bolt), "the prefab gives it");
    a.remove_component::<Brain>(bolt);
    let saved = capture_world(&a).unwrap();

    let mut b = world();
    level(&mut b);
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    let bolt = spawns(&b)[0].1;
    assert!(!b.has::<Brain>(bolt));
    assert!(b.has::<Health>(bolt));
}

#[test]
fn a_spawn_from_an_unsaved_source_is_not_saved() {
    let mut world = world();
    level(&mut world);
    let plain = world.spawn_with((Ammo(1),));
    shoot(&mut world, plain, 10.0);
    flush(&mut world);
    assert_eq!(
        world.query::<&Tag>().filter(|t| t.name == "bolt").count(),
        1
    );
    assert!(spawns(&world).is_empty());
    let saved = value(&capture_world(&world).unwrap());
    assert_eq!(saved["areas"]["cave"]["spawns"], json!([]));
    assert_eq!(saved["global"]["spawns"], json!([]));
    assert_eq!(saved["next_id"], json!(0));
}

#[test]
fn a_reference_to_an_unsaved_entity_is_dropped_and_reported() {
    let mut a = world();
    let la = level(&mut a);
    let plain = a.spawn();
    a.get_mut::<Chase>(la.bat_1).unwrap().target = plain;
    let saved = capture_world(&a).unwrap();
    assert_eq!(saved.dropped_refs(), 1);
    assert_eq!(value(&saved)["areas"]["cave"]["named"].get("bat_1"), None);

    let mut forged = value(&saved);
    forged["areas"]["cave"]["named"]["bat_2"]["refs"]["chase"]["target"] = json!([999]);
    let mut b = world();
    let lb = level(&mut b);
    let report = restore_world(&mut b, &from_value(forged), &(Scene,)).unwrap();
    assert_eq!(
        report.dropped,
        ["`chase`'s reference `target` to save ID 999: no such entity"]
    );
    // The chase has no patch, so it loads onto its saved form's
    // placeholder, not the fresh spawn's target (`RefCodec::load`).
    assert_eq!(
        b.get::<Chase>(lb.bat_2).unwrap().target,
        Entity::PLACEHOLDER,
        "the saved form's placeholder"
    );
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OldHealth {
    hp: f32,
    cap: f32,
}

fn rename_hp(mut value: Value) -> Result<Value, String> {
    let fields = value.as_object_mut().ok_or("not an object")?;
    if let Some(hp) = fields.remove("hp") {
        fields.insert("current".into(), hp);
    }
    if let Some(cap) = fields.remove("cap") {
        fields.insert("max".into(), cap);
    }
    Ok(value)
}

// A migration step's signature.
#[allow(clippy::unnecessary_wraps)]
fn double_current(mut value: Value) -> Result<Value, String> {
    if let Some(current) = value.get("current").and_then(Value::as_f64) {
        value["current"] = json!(current * 2.0);
    }
    Ok(value)
}

#[test]
fn an_older_component_migrates_a_step_at_a_time() {
    let mut a = World::new();
    a.register_component::<OldHealth>("health");
    a.register_saved::<OldHealth>("health");
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert(
        "bolt".into(),
        prefab(json!({ "components": { "health": { "hp": 1.0, "cap": 1.0 } } })),
    );
    a.insert_resource(prefabs);
    let bat = a.spawn_with((OldHealth {
        hp: 10.0,
        cap: 10.0,
    },));
    name(&mut a, bat, Some("cave"), "bat_1").unwrap();
    let mut buffer = CommandBuffer::new();
    let bolt = a.prepare_prefab("bolt", [0.0, 0.0]).unwrap();
    buffer.spawn_prefab_from(bat, bolt);
    a.flush(buffer);
    a.get_mut::<OldHealth>(bat).unwrap().hp = 4.0;
    let saved = capture_world(&a).unwrap();
    assert_eq!(value(&saved)["versions"]["components"]["health"], json!(1));

    let mut b = world_with(
        SaveOptions::new()
            .version(3)
            .step(1, rename_hp)
            .step(2, double_current),
    );
    let lb = level(&mut b);
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(
        b.get::<Health>(lb.bat_1),
        Some(&Health {
            current: 8.0,
            max: 10.0,
            shield: Some(3.0)
        }),
        "the patch migrated, then merged over the fresh spawn"
    );
    assert_eq!(
        b.get::<Health>(spawns(&b)[0].1),
        Some(&Health {
            current: 2.0,
            max: 1.0,
            shield: None
        }),
        "the spawn's whole value"
    );
    assert_eq!(
        value(&capture_world(&b).unwrap())["versions"]["components"]["health"],
        json!(3)
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Count(u32);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Tally(u32);

// A migration step's signature.
#[allow(clippy::needless_pass_by_value)]
fn times_ten(value: Value) -> Result<Value, String> {
    Ok(json!(value.as_u64().ok_or("not a count")? * 10))
}

#[test]
fn a_component_and_a_resource_of_one_name_keep_their_own_versions() {
    let mut a = world();
    a.register_saved_with("count", SaveOptions::<Count>::new().version(2));
    a.register_saved_resource::<Tally>("count");
    let la = level(&mut a);
    a.insert(la.hero, Count(3));
    a.insert_resource(Tally(4));
    let saved = capture_world(&a).unwrap();
    let versions = &value(&saved)["versions"];
    assert_eq!(versions["components"]["count"], json!(2));
    assert_eq!(versions["resources"]["count"], json!(1));

    let mut b = world();
    b.register_saved_with(
        "count",
        SaveOptions::<Count>::new().version(3).step(2, times_ten),
    );
    b.register_saved_resource_with(
        "count",
        SaveOptions::<Tally>::new().version(2).step(1, times_ten),
    );
    let lb = level(&mut b);
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert_eq!(b.get::<Count>(lb.hero), Some(&Count(30)), "from 2 to 3");
    assert_eq!(b.resource::<Tally>(), &Tally(40), "from 1 to 2");
}

#[test]
fn a_newer_version_or_a_value_that_fails_changes_nothing() {
    let mut a = world();
    let la = level(&mut a);
    fire(&mut a, la.turret, 210.0);
    flush(&mut a);
    a.despawn(la.bat_2);
    a.get_mut::<Health>(la.bat_1).unwrap().current = 2.0;
    a.resource_mut::<Weather>().wind = 4.0;
    let saved = value(&capture_world(&a).unwrap());

    let mut b = world();
    level(&mut b);
    let before = snapshot(&b);

    let mut newer = saved.clone();
    newer["versions"]["components"]["health"] = json!(5);
    assert_eq!(
        restore_world(&mut b, &from_value(newer), &(Scene,)),
        Err(RestoreError::NewerVersion {
            kind: "component",
            name: "health".into(),
            saved: 5,
            current: 1
        })
    );
    assert_eq!(snapshot(&b), before);

    let mut failing = saved.clone();
    failing["resources"]["weather"] = json!({ "raining": "yes" });
    let err = restore_world(&mut b, &from_value(failing), &(Scene,)).unwrap_err();
    assert!(
        matches!(&err, RestoreError::Value { what, .. } if what == "resource `weather`"),
        "{err}"
    );
    assert_eq!(snapshot(&b), before);

    let mut bad_patch = saved.clone();
    bad_patch["areas"]["cave"]["named"]["bat_1"]["components"]["health"] =
        json!({ "current": "low" });
    assert!(matches!(
        restore_world(&mut b, &from_value(bad_patch), &(Scene,)),
        Err(RestoreError::Value { .. })
    ));
    assert_eq!(snapshot(&b), before);

    // Formats 1 and 2 read; a later one is refused, and so is none.
    for found in [3, 0] {
        let mut format = saved.clone();
        format["format"] = json!(found);
        assert_eq!(
            restore_world(&mut b, &from_value(format), &(Scene,)),
            Err(RestoreError::Format { found })
        );
    }
    assert_eq!(snapshot(&b), before);
    assert!(b.query::<&Scene>().next().is_none());
}

#[test]
fn an_unknown_component_prefab_or_object_is_dropped_and_reported() {
    let mut a = world();
    let la = level(&mut a);
    fire(&mut a, la.turret, 210.0);
    fire(&mut a, la.turret, 220.0);
    flush(&mut a);
    a.get_mut::<Health>(la.bat_1).unwrap().current = 6.0;
    let mut saved = value(&capture_world(&a).unwrap());
    let cave = &mut saved["areas"]["cave"];
    cave["named"]["bat_9"] = json!({ "components": { "health": { "current": 1.0 } } });
    cave["named"]["bat_1"]["components"]["mana"] = json!(3);
    cave["named"]["bat_1"]["removed"] = json!(["ghost"]);
    cave["spawns"][0]["components"]["ghost"] = json!(1);
    cave["spawns"][1]["prefab"] = json!("ghost");

    let mut b = world();
    let lb = level(&mut b);
    let report = restore_world(&mut b, &from_value(saved), &(Scene,)).unwrap();
    assert_eq!(
        report.dropped,
        [
            "`bat_1` in area `cave`'s `mana`: not a saved component",
            "`bat_1` in area `cave`'s removed `ghost`: not a saved component",
            "`bat_9` in area `cave`: the level no longer has it",
            "spawn 0 (`bolt`)'s `ghost`: not a saved component",
            "spawn 1 (`ghost`): the manifest has no such prefab",
        ]
    );
    assert_eq!(b.get::<Health>(lb.bat_1).unwrap().current, 6.0);
    assert_eq!(spawns(&b).len(), 1, "the known one");
}

#[test]
fn a_spawn_whose_prefab_no_longer_prepares_fails_the_restore_and_changes_nothing() {
    let mut a = world();
    let la = level(&mut a);
    fire(&mut a, la.turret, 210.0);
    flush(&mut a);
    let mut saved = value(&capture_world(&a).unwrap());
    // A position past the prefab's bound: no unknown prefab to drop, so
    // the restore fails.
    saved["areas"]["cave"]["spawns"][0]["components"]["position"] = json!([1.0e8, 0.0]);

    let mut b = world();
    level(&mut b);
    let before = snapshot(&b);
    let err = restore_world(&mut b, &from_value(saved), &(Scene,)).unwrap_err();
    assert!(
        matches!(&err, RestoreError::Prefab { what, message }
            if what == "spawn 0 (`bolt`)" && message.contains("must be finite and within")),
        "{err:?}"
    );
    assert_eq!(snapshot(&b), before);
    assert!(b.query::<&Scene>().next().is_none());
}

#[test]
fn resources_restore_whole() {
    let mut a = world();
    level(&mut a);
    *a.resource_mut::<Weather>() = Weather {
        raining: true,
        wind: 2.5,
    };
    let saved = capture_world(&a).unwrap();
    assert_eq!(
        value(&saved)["resources"],
        json!({ "weather": { "raining": true, "wind": 2.5 } })
    );

    let mut b = world();
    level(&mut b);
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert_eq!(b.resource::<Weather>(), a.resource::<Weather>());
}

#[test]
fn two_captures_of_one_world_are_byte_equal() {
    let mut world = world();
    let level = level(&mut world);
    for x in [210.0, 220.0] {
        fire(&mut world, level.turret, x);
    }
    flush(&mut world);
    world.get_mut::<Brain>(level.bat_2).unwrap().seen = vec![3, 1];
    world.get_mut::<Health>(level.hero).unwrap().current = 0.1;
    let first = capture_world(&world).unwrap();
    let second = capture_world(&world).unwrap();
    assert_eq!(text(&first), text(&second));
    assert_eq!(
        serde_json::to_vec_pretty(&first).unwrap(),
        serde_json::to_vec_pretty(&second).unwrap()
    );
}

/// A 20 × 10 map of 16 px cells, with a wall at column 6 (x 96 to 112)
/// when `wall`, and its instance at the origin.
fn with_room(mut world: World, wall: bool) -> World {
    use crate::assets::{LayerKind, TilemapData, TilemapInstance, TilemapRegistry};

    let mut map = TilemapData::new(16, 16, 20, 10);
    let rock = map.add_tile("rock");
    let walls = map.add_layer("walls", LayerKind::Collision);
    if wall {
        for row in 0..10 {
            map.set_tile(walls, 6, row, rock);
        }
    }
    let mut maps = TilemapRegistry::new();
    maps.insert("room".into(), map);
    world.insert_resource(maps);
    world.spawn_with((TilemapInstance::new("room", Vec2::ZERO),));
    world
}

/// A named 8 × 8 crate at (40, 80).
fn crate_body(world: &mut World) -> Entity {
    use crate::physics::{Collider, RigidBodyBundle};

    let body = world.spawn_with(RigidBodyBundle::dynamic(
        Position(Vec2::new(40.0, 80.0)),
        Collider::aabb(Vec2::splat(4.0)),
    ));
    name(world, body, None, "crate").unwrap();
    body
}

#[test]
fn a_restored_body_inside_a_new_wall_comes_back_beside_it() {
    use crate::physics::PrevPosition;

    let mut a = with_room(world(), false);
    let body = crate_body(&mut a);
    a.get_mut::<Position>(body).unwrap().0 = Vec2::new(100.0, 80.0);
    let saved = capture_world(&a).unwrap();

    let mut b = with_room(world(), true);
    let body = crate_body(&mut b);
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    let beside = Vec2::new(92.0, 80.0);
    assert_eq!(
        b.get::<Position>(body),
        Some(&Position(beside)),
        "half a cell left, the nearer face"
    );
    assert_eq!(b.get::<PrevPosition>(body), Some(&PrevPosition(beside)));

    let mut walled = with_room(world(), true);
    let fenced = crate_body(&mut walled);
    restore_world(&mut walled, &capture_world(&b).unwrap(), &(Scene,)).unwrap();
    assert_eq!(
        walled.get::<Position>(fenced),
        Some(&Position(beside)),
        "a body already free stays"
    );
}

#[test]
fn a_replaced_entity_keeps_its_save_identity() {
    /// A marker the spawn inheritance copies.
    #[derive(Debug, Clone, PartialEq)]
    struct Team(u8);

    let charred = || {
        prefab(json!({ "tag": "charred", "components": {
            "health": { "current": 2.0, "max": 2.0 } } }))
    };
    let setup = |world: &mut World| {
        world
            .resource_mut::<PrefabRegistry>()
            .insert("charred".into(), charred());
        world.inherit_from_source::<Team>();
    };
    let charred_of = |world: &World| -> Vec<Entity> {
        world
            .query::<(Entity, &Tag)>()
            .filter(|(_, tag)| tag.name == "charred")
            .map(|(entity, _)| entity)
            .collect()
    };
    let mut a = world();
    setup(&mut a);
    let la = level(&mut a);
    a.insert(la.bat_1, Team(3));
    let id = a.get::<Saved>(la.bat_1).unwrap().id();
    let prepared = a.prepare_prefab("charred", [100.0, 40.0]).unwrap();
    a.resource_mut::<CommandBuffer>()
        .replace_prefab(la.bat_1, prepared);
    flush(&mut a);
    // One entity with the new prefab's components, under the named bat's
    // ID with the new prefab, and what a spawn from it inherits.
    assert!(!a.is_alive(la.bat_1));
    let [charred_a] = charred_of(&a)[..] else {
        panic!("one charred bat");
    };
    let saved = a.get::<Saved>(charred_a).unwrap();
    assert_eq!(
        (saved.id(), saved.area(), saved.prefab()),
        (id, Some("cave"), Some("charred"))
    );
    assert_eq!(a.get::<Team>(charred_a), Some(&Team(3)));
    assert_eq!(a.get::<Health>(charred_a).unwrap().max, 2.0);
    a.get_mut::<Health>(charred_a).unwrap().current = 1.5;
    // The other bat chases the charred one: a reference by save ID.
    a.get_mut::<Chase>(la.bat_2).unwrap().target = charred_a;
    let save = capture_world(&a).unwrap();
    let cave = &value(&save)["areas"]["cave"];
    assert!(
        cave["gone"].as_array().unwrap().contains(&json!("bat_1")),
        "{cave}"
    );
    let spawned: Vec<(u64, &str)> = cave["spawns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|spawn| {
            (
                spawn["id"].as_u64().unwrap(),
                spawn["prefab"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(spawned, [(id.0, "charred")]);

    // A restore into a World whose allocations differ: the named bat goes,
    // the charred one comes once under its ID with its saved components,
    // and the chase resolves to it.
    let mut b = world();
    setup(&mut b);
    churn(&mut b, 7);
    let lb = level(&mut b);
    restore_world(&mut b, &save, &(Scene,)).unwrap();
    assert!(!b.is_alive(lb.bat_1));
    let [charred_b] = charred_of(&b)[..] else {
        panic!("one charred bat after the restore");
    };
    assert_eq!(b.get::<Saved>(charred_b).unwrap().id(), id);
    assert_eq!(b.get::<Health>(charred_b).unwrap().current, 1.5);
    let bat_2 = b.get::<Saved>(lb.bat_2).unwrap().id();
    assert_eq!(chases(&b).get(&bat_2), Some(&Some(id)));
    assert_eq!(b.get::<Chase>(lb.bat_2).unwrap().target, charred_b);

    // A run's spawn replaced keeps its ID; a dead entity's replace spawns
    // nothing.
    let mut c = world();
    setup(&mut c);
    let lc = level(&mut c);
    fire(&mut c, lc.turret, 210.0);
    flush(&mut c);
    let (bolt_id, bolt) = spawns(&c)[0];
    let prepared = c.prepare_prefab("charred", [210.0, 40.0]).unwrap();
    c.resource_mut::<CommandBuffer>()
        .replace_prefab(bolt, prepared);
    flush(&mut c);
    let (kept, _) = spawns(&c)[0];
    assert_eq!(kept, bolt_id);
    assert_eq!(charred_of(&c).len(), 1);
    let count = c.entity_count();
    let gone = c.spawn();
    c.despawn(gone);
    let prepared = c.prepare_prefab("charred", [0.0, 0.0]).unwrap();
    c.resource_mut::<CommandBuffer>()
        .replace_prefab(gone, prepared);
    flush(&mut c);
    assert_eq!((c.entity_count(), charred_of(&c).len()), (count, 1));
}

#[test]
fn a_body_resting_on_a_floor_comes_back_where_it_was_saved() {
    // The crate rests 0.256 px into the floor, past `linear_slop`; the
    // restore's free spot leaves it there.
    use crate::assets::{LayerKind, TilemapData, TilemapInstance, TilemapRegistry};
    use crate::physics::PhysicsConfig;

    // A 20 × 10 map of 16 px cells, its floor row 9 (top at y 144), and the crate.
    let floored = || {
        let mut map = TilemapData::new(16, 16, 20, 10);
        let rock = map.add_tile("rock");
        let walls = map.add_layer("walls", LayerKind::Collision);
        for col in 0..20 {
            map.set_tile(walls, col, 9, rock);
        }
        let mut maps = TilemapRegistry::new();
        maps.insert("room".into(), map);
        let mut world = world();
        world.insert_resource(maps);
        world.spawn_with((TilemapInstance::new("room", Vec2::ZERO),));
        world.insert_resource(PhysicsConfig {
            gravity: Vec2::new(0.0, 900.0),
            ..PhysicsConfig::default()
        });
        world.insert_resource(crate::time::Time::new());
        let body = crate_body(&mut world);
        (world, body)
    };
    let (mut a, body) = floored();
    for _ in 0..120 {
        a.resource_mut::<crate::time::Time>()
            .advance_frame(1.0 / 60.0);
        crate::physics::physics_step(&mut a);
    }
    let rest = a.get::<Position>(body).unwrap().0;
    let (mut b, body) = floored();
    let report = restore_world(&mut b, &capture_world(&a).unwrap(), &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(
        b.get::<Position>(body).unwrap().0,
        rest,
        "rested {} px deep",
        rest.y + 4.0 - 144.0
    );
}

// Evidence for oracle-ecs new-4 (core hardening 2026-11-07, save oracle).

#[test]
fn oracle_new_4_a_restore_leaves_a_world_that_names_nothing_unsaving() {
    // A game that saves its resources alone and names no entity: the live
    // run holds no save state, so the kit's passes keep their query order.
    let start = || {
        let mut world = World::new();
        world.register_saved_resource::<Weather>("weather");
        world.insert_resource(Weather {
            raining: false,
            wind: 0.0,
        });
        world
    };
    let mut a = start();
    a.resource_mut::<Weather>().raining = true;
    let saved = capture_world(&a).unwrap();
    let mut b = start();
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert_eq!(b.resource::<Weather>(), a.resource::<Weather>());
    assert_eq!(
        crate::save::saving(&b),
        crate::save::saving(&a),
        "the loaded run saves as the live one does"
    );
}

// Evidence for oracle-ecs new-2 (core hardening 2026-11-07, save oracle).
// new-3's test shares `Marks` and `hero_dog`.

/// A list of references under one key whose codec's `load` extends it, as
/// the area tests' `Hits` and the kit's hit lists do.
#[derive(Debug, Clone, PartialEq)]
struct Marks {
    key: u32,
    targets: Vec<Entity>,
}

#[derive(Serialize, Deserialize)]
struct MarksForm {
    key: u32,
}

impl From<&Marks> for MarksForm {
    fn from(marks: &Marks) -> Self {
        Self { key: marks.key }
    }
}

impl From<MarksForm> for Marks {
    fn from(form: MarksForm) -> Self {
        Self {
            key: form.key,
            targets: Vec::new(),
        }
    }
}

fn save_marks(marks: &Marks, out: &mut RefWriter<'_>) {
    for &target in &marks.targets {
        out.put("marks", target);
    }
}

fn load_marks(marks: &mut Marks, refs: &RefReader<'_>) {
    marks.targets.extend_from_slice(refs.get("marks"));
}

fn marks_keys(_: &Marks) -> Vec<String> {
    vec!["marks".into()]
}

/// `world()` with `Marks` saved under `marks`.
fn world_with_marks() -> World {
    let mut world = world();
    world.register_saved_with(
        "marks",
        SaveOptions::<Marks>::form::<MarksForm>().refs(RefCodec {
            save: save_marks,
            load: load_marks,
            keys: marks_keys,
        }),
    );
    world
}

/// A run's start for new-2 and new-3: `hero` and `dog`, global, and, when
/// `cave`, `bat_1` in area `cave`. The hero chases the dog.
fn hero_dog(world: &mut World, cave: bool) -> (Entity, Entity, Option<Entity>) {
    let hero = world.spawn_prefab("hero", [16.0, 16.0]).unwrap();
    let dog = world.spawn_prefab("hero", [32.0, 16.0]).unwrap();
    world.insert(
        hero,
        Chase {
            target: dog,
            speed: 2.0,
        },
    );
    name(world, hero, None, "hero").unwrap();
    name(world, dog, None, "dog").unwrap();
    let bat = cave.then(|| {
        let bat = world.spawn_prefab("bat", [100.0, 40.0]).unwrap();
        name(world, bat, Some("cave"), "bat_1").unwrap();
        bat
    });
    (hero, dog, bat)
}

#[test]
fn oracle_new_2_a_named_entitys_list_loads_onto_its_saved_form() {
    // The bat's level gives it one mark, the hero; the run changes
    // nothing. The restore leaves the fresh value (no patch) and its codec
    // extends it with the saved mark.
    let start = |world: &mut World| {
        let hero = world.spawn_prefab("hero", [16.0, 16.0]).unwrap();
        let bat = world.spawn_prefab("bat", [100.0, 40.0]).unwrap();
        world.insert(
            bat,
            Marks {
                key: 1,
                targets: vec![hero],
            },
        );
        name(world, hero, None, "hero").unwrap();
        name(world, bat, Some("cave"), "bat_1").unwrap();
        (hero, bat)
    };
    let mut a = world_with_marks();
    start(&mut a);
    let saved = capture_world(&a).unwrap();
    let mut b = world_with_marks();
    let (hero, bat) = start(&mut b);
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert_eq!(
        b.get::<Marks>(bat).unwrap().targets,
        [hero],
        "the live World's one mark"
    );
}

#[test]
fn oracle_new_2_a_reference_waiting_on_a_kept_area_leaves_no_fresh_one() {
    // The hero's chase, the dog at its naming, turns to the bat in area
    // `cave`; the restoring World has not loaded the cave.
    let mut a = world();
    let (hero, _, bat) = hero_dog(&mut a, true);
    a.get_mut::<Chase>(hero).unwrap().target = bat.unwrap();
    let first = text(&capture_world(&a).unwrap());
    let mut b = world();
    let (hero_b, dog_b, _) = hero_dog(&mut b, false);
    restore_world(&mut b, &serde_json::from_str(&first).unwrap(), &(Scene,)).unwrap();
    assert!(b.resource::<SaveState>().is_kept("cave"));
    let chasing_dog = b.get::<Chase>(hero_b).unwrap().target == dog_b;
    let second = text(&capture_world(&b).unwrap());
    assert!(
        !chasing_dog && second == first,
        "the hero chases the fresh dog: {chasing_dog}; the capture writes {}",
        &second[second.find("\"hero\"").unwrap_or(0)..][..120]
    );
}

// Evidence for oracle-ecs new-1 (core hardening 2026-11-07, save oracle).

#[test]
#[ignore = "7.21 step 15: fails until its fix"]
fn oracle_new_1_a_saved_f64_restores_from_its_text_bit_for_bit() {
    // A saved resource's f64, written to text and read back as a game's
    // slot does: serde_json without `float_roundtrip` parses a 17-digit
    // number best-effort.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Gauge {
        pressure: f64,
    }
    let mut drifted = Vec::new();
    let mut rewritten = 0;
    for n in 0..2_000 {
        let pressure = f64::from(n) * 0.1 - 100.0;
        let mut a = World::new();
        a.register_saved_resource::<Gauge>("gauge");
        a.insert_resource(Gauge { pressure });
        let first = text(&capture_world(&a).unwrap());
        let read: WorldSave = serde_json::from_str(&first).unwrap();
        rewritten += usize::from(text(&read) != first);
        let mut b = World::new();
        b.register_saved_resource::<Gauge>("gauge");
        restore_world(&mut b, &read, &(Scene,)).unwrap();
        let back = b.resource::<Gauge>().pressure;
        if back.to_bits() != pressure.to_bits() {
            drifted.push((pressure, back));
        }
    }
    assert!(
        drifted.is_empty() && rewritten == 0,
        "{} of 2,000 pressures restored off (first {:?}); {rewritten} saves wrote other bytes once read",
        drifted.len(),
        &drifted[..drifted.len().min(3)]
    );
}

#[test]
#[ignore = "7.21 step 15: fails until its fix"]
fn oracle_new_1_a_kept_areas_f32_writes_back_as_it_was_read() {
    // `bat_1`'s f32 health in area `cave`, which the restoring World has
    // not loaded: the restore keeps the record as parsed, and the next
    // capture writes it back.
    let mut a = world();
    let la = level(&mut a);
    let mut differ = Vec::new();
    for n in 1..200_u16 {
        a.get_mut::<Health>(la.bat_1).unwrap().current = f32::from(n) * 0.1;
        let first = text(&capture_world(&a).unwrap());
        let read: WorldSave = serde_json::from_str(&first).unwrap();
        let mut b = world();
        let hero = b.spawn_prefab("hero", [16.0, 16.0]).unwrap();
        name(&mut b, hero, None, "hero").unwrap();
        restore_world(&mut b, &read, &(Scene,)).unwrap();
        assert!(b.resource::<SaveState>().is_kept("cave"));
        if text(&capture_world(&b).unwrap()) != first {
            differ.push(f32::from(n) * 0.1);
        }
    }
    assert!(
        differ.is_empty(),
        "{} of 199 healths wrote other bytes from a kept area (first {:?})",
        differ.len(),
        &differ[..differ.len().min(5)]
    );
}

// Evidence for oracle-ecs new-3 (core hardening 2026-11-07, save oracle).
// It uses new-2's `Marks`, `world_with_marks` and `hero_dog`.

#[test]
fn oracle_new_3_a_key_split_across_a_kept_area_keeps_its_order() {
    // The hero marks the bat (area `cave`), then the dog (global). The
    // restoring World loads the global record alone, then enters the cave.
    let mut a = world_with_marks();
    let (hero, dog, bat) = hero_dog(&mut a, true);
    a.insert(
        hero,
        Marks {
            key: 1,
            targets: vec![bat.unwrap(), dog],
        },
    );
    let first = text(&capture_world(&a).unwrap());
    let mut b = world_with_marks();
    let (hero_b, _, _) = hero_dog(&mut b, false);
    restore_world(&mut b, &serde_json::from_str(&first).unwrap(), &(Scene,)).unwrap();
    let kept = text(&capture_world(&b).unwrap());
    let bat_b = b.spawn_prefab("bat", [100.0, 40.0]).unwrap();
    name(&mut b, bat_b, Some("cave"), "bat_1").unwrap();
    crate::save::enter_area(&mut b, "cave", &(Scene,)).unwrap();
    let ids = |world: &World, entity: Entity| -> Vec<u64> {
        world
            .get::<Marks>(entity)
            .unwrap()
            .targets
            .iter()
            .map(|&target| world.get::<Saved>(target).unwrap().id().0)
            .collect()
    };
    // `D-257`: a split key loads in two calls, each in saved order, the dog
    // (global) at the restore and the bat at the cave's entry; a capture
    // between them writes the live reference, then the waiting one.
    let marks = |text: &str| -> Vec<u64> {
        let save: serde_json::Value = serde_json::from_str(text).unwrap();
        save["global"]["named"]["hero"]["refs"]["marks"]["marks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_u64().unwrap())
            .collect()
    };
    let live = ids(&a, hero);
    let loaded_then_waiting = vec![live[1], live[0]];
    assert_eq!(marks(&first), live, "the save keeps the live order");
    assert_eq!(
        (ids(&b, hero_b), marks(&kept)),
        (loaded_then_waiting.clone(), loaded_then_waiting),
        "the marks after the entry, and the window's capture"
    );
}

#[test]
fn a_restored_spawn_shows_its_variant_in_a_churned_world() {
    use crate::assets::AssetRegistry;
    use crate::components::Sprite;
    /// `world()` with the prefab `orb` showing `orb`, `orb_b` or `orb_c`,
    /// their names interned in `order`.
    fn orbs(order: [&str; 3]) -> World {
        let mut world = world();
        let mut assets = AssetRegistry::new();
        for name in order {
            assets.intern_sprite(name);
        }
        world.insert_resource(assets);
        world.resource_mut::<PrefabRegistry>().insert(
            "orb".into(),
            prefab(json!({ "tag": "orb",
                "sprite": { "asset_id": "orb", "variants": ["orb_b", "orb_c"] },
                "components": { "brain": { "mood": "drift" } } })),
        );
        world
    }
    /// Each saved orb's save ID with the sprite it shows, by name.
    fn shown(world: &World) -> Vec<(SaveId, String)> {
        let assets = world.resource::<AssetRegistry>();
        let mut shown: Vec<(SaveId, String)> = world
            .query::<(&Saved, &Sprite)>()
            .filter(|(saved, _)| saved.prefab() == Some("orb"))
            .map(|(saved, sprite)| {
                let name = assets.sprite_name(sprite.asset_id).unwrap();
                (saved.id(), name.to_owned())
            })
            .collect();
        shown.sort();
        shown
    }
    let mut a = orbs(["orb", "orb_b", "orb_c"]);
    let la = level(&mut a);
    for x in 0..9 {
        let orb = a.prepare_prefab("orb", [x as f32 * 24.0, 40.0]).unwrap();
        a.resource_mut::<CommandBuffer>()
            .spawn_prefab_from(la.turret, orb);
    }
    flush(&mut a);
    let before = shown(&a);
    assert_eq!(before.len(), 9);
    let names: BTreeSet<&str> = before.iter().map(|(_, name)| name.as_str()).collect();
    assert!(names.len() > 1, "{before:?}");
    let saved = capture_world(&a).unwrap();
    assert!(!text(&saved).contains("orb_b"), "a variant saves nothing");
    // Other entity indices, the names interned in another order, and the
    // spawns placed where the save names no position: the save ID picks.
    let mut b = orbs(["orb_c", "orb", "orb_b"]);
    churn(&mut b, 40);
    level(&mut b);
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert_eq!(shown(&b), before);
}

/// Generated areas (redo 7.12): a save of a generated run as its seed and
/// its changes, regenerated and matched against the World's generation.
mod generated {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::assets::{
        Generate, GenerateError, GeneratedRooms, Generation, Room, RoomCommand, RoomGenerator,
        RoomRegistry, Tileset,
    };
    use crate::save::{enter_area, regenerate};

    const TILESET: &str = r#"{ "version": 1, "tiles": [{ "sprite": "rock" }] }"#;

    /// An 8 × 4 template with a floor, a crate and a door on each side.
    const HALL: &str = r#"{ "version": 1, "cell": [16, 16], "size": [8, 4], "tileset": "cave",
      "layers": [{ "name": "solid", "kind": "collision" }],
      "shapes": [{ "id": "floor", "rect": [0, 3, 8, 1], "tile": "rock", "layers": ["solid"] }],
      "objects": [{ "id": "crate_1", "prefab": "crate", "at": [1, 2] }],
      "places": [{ "id": "door_west", "point": [0, 2] }, { "id": "door_east", "point": [7, 2] }] }"#;

    type Migrate = fn(&Generation, Option<&Generation>, &mut Value) -> bool;

    /// What each `migrate` call saw: the saved generation, the World's and
    /// the area's record.
    type Seen = Rc<RefCell<Vec<(Generation, Option<Generation>, Value)>>>;

    /// Copies of `hall` in a row, 128 px apart, each with a second crate at
    /// a column from the seed: three rooms at version 1, two from 2.
    struct Chain {
        version: u32,
        reproduces: &'static [u32],
        migrate: Option<Migrate>,
        seen: Seen,
    }

    fn chain(version: u32) -> Chain {
        Chain {
            version,
            reproduces: &[],
            migrate: None,
            seen: Seen::default(),
        }
    }

    impl RoomGenerator for Chain {
        fn version(&self) -> u32 {
            self.version
        }

        fn reproduces(&self, version: u32) -> bool {
            version == self.version || self.reproduces.contains(&version)
        }

        fn generate(&self, generation: &mut Generate<'_>) -> Result<(), String> {
            let hall = generation.template("hall")?;
            let rooms = if generation.version() == 1 { 3 } else { 2 };
            for at in 0..rooms {
                let local = format!("r{at}");
                let mut room = hall.clone();
                let col = generation.rng().next_u32() % 6 + 1;
                let place = RoomCommand::batch_from_value(&json!([{ "place": { "objects":
                    { "id": "crate_2", "prefab": "crate", "at": [col, 2] } } }]))
                .unwrap();
                generation.apply(&local, &mut room, &place)?;
                generation.add(&local, room, [at as f32 * 128.0, 0.0])?;
            }
            Ok(())
        }

        fn migrate(&self, from: &Generation, now: Option<&Generation>, area: &mut Value) -> bool {
            self.seen
                .borrow_mut()
                .push((from.clone(), now.cloned(), area.clone()));
            self.migrate.is_some_and(|migrate| migrate(from, now, area))
        }
    }

    /// A migration that keeps the record as it is.
    fn keep(_: &Generation, _: Option<&Generation>, _: &mut Value) -> bool {
        true
    }

    /// A migration doubling each named record's saved `health.current`.
    fn double_saved_health(_: &Generation, _: Option<&Generation>, area: &mut Value) -> bool {
        if let Some(named) = area["named"].as_object_mut() {
            for record in named.values_mut() {
                let health = &mut record["components"]["health"];
                if let Some(current) = health["current"].as_f64() {
                    health["current"] = json!(current * 2.0);
                }
            }
        }
        true
    }

    /// `chain` without a `migrate` of its own: the trait's, which refuses.
    struct Bare(Chain);

    impl RoomGenerator for Bare {
        fn version(&self) -> u32 {
            self.0.version()
        }

        fn reproduces(&self, version: u32) -> bool {
            self.0.reproduces(version)
        }

        fn generate(&self, generation: &mut Generate<'_>) -> Result<(), String> {
            self.0.generate(generation)
        }
    }

    /// A migration that leaves the record unreadable: `named` a number.
    fn unreadable(_: &Generation, _: Option<&Generation>, area: &mut Value) -> bool {
        area["named"] = json!(5);
        true
    }

    /// A World saving `health` with `options`, the prefab `crate`, the
    /// cave tileset, the template `hall`, and `chain` registered.
    fn world_of_with(chain: impl RoomGenerator, options: SaveOptions<Health>) -> World {
        let mut world = World::new();
        world.register_component::<Health>("health");
        world.register_saved_with("health", options);
        let mut prefabs = PrefabRegistry::new();
        prefabs.insert(
            "crate".into(),
            prefab(json!({ "tag": "crate", "components": {
                "health": { "current": 3.0, "max": 3.0 } } })),
        );
        world.insert_resource(prefabs);
        let mut rooms = RoomRegistry::new();
        rooms.insert_tileset("cave", Tileset::from_json(TILESET).unwrap());
        world.insert_resource(rooms);
        world
            .insert_room("hall", Room::from_json(HALL).unwrap())
            .unwrap();
        world.register_generator("chain", chain);
        world
    }

    fn world_of(chain: impl RoomGenerator) -> World {
        world_of_with(chain, SaveOptions::new())
    }

    fn spawn_all(world: &mut World, rooms: &GeneratedRooms) {
        for (id, origin) in rooms.rooms() {
            world.spawn_room(id, origin, &(Scene,)).unwrap();
        }
    }

    /// `chain` at seed 7, each room spawned at its origin.
    fn run(world: &mut World) -> GeneratedRooms {
        let rooms = world.generate_rooms("chain", 7, "chain").unwrap();
        spawn_all(world, &rooms);
        rooms
    }

    /// The live entity named `name`.
    fn named(world: &World, name: &str) -> Option<Entity> {
        world
            .get_resource::<SaveState>()?
            .named_entity(SaveId::authored(name))
            .filter(|&entity| world.is_alive(entity))
    }

    fn health(world: &World, name: &str) -> Option<f32> {
        named(world, name)
            .and_then(|entity| world.get::<Health>(entity))
            .map(|health| health.current)
    }

    /// The run's play: `chain_r0.crate_1` down to 1, `chain_r1.crate_2`
    /// broken, `chain_r2.crate_1` down to 2.
    fn play(world: &mut World) {
        let first = named(world, "chain_r0.crate_1").unwrap();
        world.get_mut::<Health>(first).unwrap().current = 1.0;
        let broken = named(world, "chain_r1.crate_2").unwrap();
        world.despawn(broken);
        let last = named(world, "chain_r2.crate_1").unwrap();
        world.get_mut::<Health>(last).unwrap().current = 2.0;
    }

    /// The run at version 1 played and captured, as `generated_v2.json`
    /// holds it.
    fn v1_save() -> WorldSave {
        let mut world = world_of(chain(1));
        run(&mut world);
        play(&mut world);
        capture_world(&world).unwrap()
    }

    /// The save captured once from `v1_save`, committed, at format 2.
    fn fixture() -> WorldSave {
        serde_json::from_str(include_str!("generated_v2.json")).unwrap()
    }

    /// The same save as format 1 wrote it, before tables: the file an older
    /// build left.
    fn format_1_fixture() -> WorldSave {
        serde_json::from_str(include_str!("generated_v1.json")).unwrap()
    }

    #[test]
    fn a_format_1_save_restores_and_captures_as_format_2() {
        let old = format_1_fixture();
        assert_eq!(value(&old)["format"], json!(1));
        let mut as_two = value(&old);
        as_two["format"] = json!(FORMAT);
        assert_eq!(as_two, value(&fixture()), "the fixtures differ by format");
        let (world, restored) = restore(chain(1), &old);
        assert!(restored.unwrap().is_clean());
        assert_eq!(health(&world, "chain_r0.crate_1"), Some(1.0));
        assert_eq!(named(&world, "chain_r1.crate_2"), None, "broken in the run");
        assert_eq!(
            text(&capture_world(&world).unwrap()),
            text(&fixture()),
            "the restored run captures at this build's format"
        );
    }

    /// `save` restored into a new World of `chain`: regenerated, each room
    /// spawned at its origin, then restored.
    fn restore(chain: Chain, save: &WorldSave) -> (World, Result<RestoreReport, RestoreError>) {
        let mut world = world_of(chain);
        for rooms in regenerate(&mut world, save).unwrap() {
            spawn_all(&mut world, &rooms);
        }
        let restored = restore_world(&mut world, save, &(Scene,));
        (world, restored)
    }

    /// Whether `area`'s record names `name`, as a named record or a gone one.
    fn mentions(area: &Value, name: &str) -> bool {
        area["named"].get(name).is_some()
            || area["gone"]
                .as_array()
                .is_some_and(|gone| gone.iter().any(|gone| gone == name))
    }

    /// The `migrate` call that saw `name`'s record.
    fn call_for(seen: &Seen, name: &str) -> Option<(Generation, Option<Generation>, Value)> {
        seen.borrow()
            .iter()
            .find(|(_, _, area)| mentions(area, name))
            .cloned()
    }

    fn room_ids(world: &World) -> Vec<String> {
        world
            .resource::<RoomRegistry>()
            .ids()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn a_capture_writes_each_generated_areas_generation() {
        let mut world = world_of(chain(1));
        let rooms = run(&mut world);
        world
            .insert_room("lobby", Room::from_json(HALL).unwrap())
            .unwrap();
        world.spawn_room("lobby", [0.0, 200.0], &(Scene,)).unwrap();
        let save = capture_world(&world).unwrap();
        let generations: Vec<(&str, &Generation)> = save.generations().collect();
        let areas: Vec<&str> = generations.iter().map(|&(area, _)| area).collect();
        assert_eq!(areas, ["chain_r0", "chain_r1", "chain_r2"]);
        for (area, generation) in &generations {
            assert_eq!(*generation, rooms.generation(), "{area}");
        }
        let json = value(&save);
        assert_eq!(json["generated"]["chain_r1"]["seed"], json!(7));
        assert!(
            json["generated"].get("lobby").is_none(),
            "an authored room's area names none"
        );
        assert!(
            json["areas"]["chain_r1"].get("generated").is_none(),
            "apart from the areas"
        );
        // The fixture, captured once from version 1 over `hall` at a known
        // hash, names that hash.
        let fixture = fixture();
        let (_, saved) = fixture.generations().next().unwrap();
        assert_eq!((saved.version, saved.seed), (1, 7));
        assert_eq!(saved.templates["hall"], "81171bda5a44e2c5");
        assert_eq!(rooms.generation().templates["hall"], "81171bda5a44e2c5");
    }

    #[test]
    fn a_generated_run_restores_from_its_seed_and_its_changes() {
        let save = v1_save();
        let (world, restored) = restore(chain(1), &save);
        let report = restored.unwrap();
        assert!(report.is_clean(), "{report:?}");
        assert_eq!(health(&world, "chain_r0.crate_1"), Some(1.0));
        assert_eq!(named(&world, "chain_r1.crate_2"), None, "broken in the run");
        assert_eq!(health(&world, "chain_r1.crate_1"), Some(3.0), "untouched");
        assert_eq!(health(&world, "chain_r2.crate_1"), Some(2.0));
        assert_eq!(
            text(&capture_world(&world).unwrap()),
            text(&save),
            "the restored run captures the same save"
        );
        assert_eq!(text(&fixture()), text(&save), "the fixture is such a save");
    }

    #[test]
    fn regenerate_registers_the_saves_generations_and_nothing_on_an_error() {
        let mut a = world_of(chain(1));
        run(&mut a);
        let deep = a.generate_rooms("chain", 9, "deep").unwrap();
        spawn_all(&mut a, &deep);
        let save = capture_world(&a).unwrap();
        let mut b = world_of(chain(1));
        let rooms = regenerate(&mut b, &save).unwrap();
        let ids: Vec<Vec<&str>> = rooms
            .iter()
            .map(|rooms| rooms.rooms().map(|(id, _)| id).collect())
            .collect();
        assert_eq!(
            ids,
            [
                ["chain_r0", "chain_r1", "chain_r2"],
                ["deep_r0", "deep_r1", "deep_r2"]
            ]
        );
        assert_eq!(rooms[1].generation(), deep.generation());
        assert!(
            regenerate(&mut b, &save).unwrap().is_empty(),
            "held under equal records, each is skipped"
        );
        // An unknown generator registers nothing, not even the run before it.
        let mut edited = value(&save);
        for area in ["deep_r0", "deep_r1", "deep_r2"] {
            edited["generated"][area]["generator"] = json!("chian");
        }
        let edited = from_value(edited);
        let mut c = world_of(chain(1));
        let err = regenerate(&mut c, &edited).unwrap_err();
        assert!(
            matches!(err, GenerateError::UnknownGenerator { ref generator, .. } if generator == "chian"),
            "{err:?}"
        );
        assert_eq!(room_ids(&c), ["hall"]);
        // Nor does a run whose rooms the World holds under another record.
        let mut d = world_of(chain(1));
        d.generate_rooms("chain", 8, "deep").unwrap();
        let err = regenerate(&mut d, &save).unwrap_err();
        assert_eq!(
            err,
            GenerateError::Taken {
                room: "deep_r0".into()
            }
        );
        assert_eq!(room_ids(&d), ["deep_r0", "deep_r1", "deep_r2", "hall"]);
        // Nor do two of the save's generations that make one room ID.
        let mut split = value(&save);
        split["generated"]["chain_r1"]["seed"] = json!(8);
        let mut fresh = world_of(chain(1));
        let err = regenerate(&mut fresh, &from_value(split)).unwrap_err();
        assert_eq!(
            err,
            GenerateError::Taken {
                room: "chain_r0".into()
            }
        );
        assert_eq!(room_ids(&fresh), ["hall"]);
    }

    #[test]
    fn a_save_from_before_a_generator_change_is_refused() {
        let save = fixture();
        let mut world = world_of(chain(2));
        let rooms = regenerate(&mut world, &save).unwrap();
        assert_eq!(rooms[0].generation().version, 2, "it reproduces only 2");
        spawn_all(&mut world, &rooms[0]);
        let before = snapshot(&world);
        let err = restore_world(&mut world, &save, &(Scene,)).unwrap_err();
        let RestoreError::Generation { area, saved, now } = &err else {
            panic!("{err:?}");
        };
        assert_eq!(area, "chain_r0");
        assert_eq!(saved.version, 1);
        assert_eq!(now.as_deref(), Some(rooms[0].generation()));
        assert_eq!(
            err.to_string(),
            format!(
                "area `chain_r0` was generated by `chain` version 1 at seed 7 under prefix `chain`, template `hall` 81171bda5a44e2c5, and the World's by {}; its generator did not migrate it",
                rooms[0].generation()
            )
        );
        assert_eq!(snapshot(&world), before, "nothing changed");
        assert_eq!(health(&world, "chain_r0.crate_1"), Some(3.0));
    }

    #[test]
    fn a_generated_area_its_generator_does_not_migrate_fails_and_changes_nothing() {
        // A generator the World lacks, whose namesake would keep the record
        // and is never asked; the trait's default `migrate`; a migration
        // whose record no longer reads.
        let seen = Seen::default();
        let mut renamed = value(&fixture());
        for generation in renamed["generated"].as_object_mut().unwrap().values_mut() {
            generation["generator"] = json!("chian");
        }
        let keeping = Chain {
            migrate: Some(keep),
            seen: seen.clone(),
            ..chain(1)
        };
        fn regenerated(generator: impl RoomGenerator) -> World {
            let mut world = world_of(generator);
            for rooms in regenerate(&mut world, &fixture()).unwrap() {
                spawn_all(&mut world, &rooms);
            }
            world
        }
        let unreadable = Chain {
            migrate: Some(unreadable),
            ..chain(2)
        };
        let cases = [
            (
                "a generator the World lacks",
                world_of(keeping),
                from_value(renamed),
                "area `chain_r0` refused, now None",
            ),
            (
                "the default migrate",
                regenerated(Bare(chain(2))),
                fixture(),
                "area `chain_r0` refused, now Some(2)",
            ),
            (
                "an unreadable record",
                regenerated(unreadable),
                fixture(),
                "area `chain_r0`'s migrated record does not load",
            ),
        ];
        for (case, mut world, save, want) in cases {
            let before = snapshot(&world);
            let err = restore_world(&mut world, &save, &(Scene,)).unwrap_err();
            let got = match &err {
                RestoreError::Generation { area, now, .. } => format!(
                    "area `{area}` refused, now {:?}",
                    now.as_ref().map(|now| now.version)
                ),
                RestoreError::Value { what, .. } => format!("{what} does not load"),
                other => format!("{other:?}"),
            };
            assert_eq!(got, want, "{case}");
            assert_eq!(snapshot(&world), before, "{case}: nothing changed");
        }
        assert!(seen.borrow().is_empty(), "the lacking generator's namesake");
    }

    #[test]
    fn a_generator_that_reproduces_its_old_version_keeps_the_changes() {
        let reproducing = Chain {
            reproduces: &[1],
            ..chain(2)
        };
        let (world, restored) = restore(reproducing, &fixture());
        assert!(restored.unwrap().is_clean());
        let generation = world.resource::<RoomRegistry>().generation("chain_r2");
        assert_eq!(generation.map(|generation| generation.version), Some(1));
        assert_eq!(health(&world, "chain_r0.crate_1"), Some(1.0));
        assert_eq!(named(&world, "chain_r1.crate_2"), None);
        assert_eq!(health(&world, "chain_r2.crate_1"), Some(2.0));
    }

    #[test]
    fn a_generators_migration_rewrites_the_saved_changes() {
        let migrating = Chain {
            migrate: Some(double_saved_health),
            ..chain(2)
        };
        let (world, restored) = restore(migrating, &fixture());
        assert!(restored.unwrap().is_clean());
        assert_eq!(health(&world, "chain_r0.crate_1"), Some(2.0), "1, doubled");
        assert_eq!(named(&world, "chain_r1.crate_2"), None, "still broken");
        // Each record now stands under version 2's generation, the one
        // version 2 no longer makes kept.
        let again = capture_world(&world).unwrap();
        let versions: Vec<(&str, u32)> = again
            .generations()
            .map(|(area, generation)| (area, generation.version))
            .collect();
        assert_eq!(
            versions,
            [("chain_r0", 2), ("chain_r1", 2), ("chain_r2", 2)]
        );
        assert_eq!(
            value(&again)["areas"]["chain_r2"]["named"]["chain_r2.crate_1"]["components"]["health"]
                ["current"],
            json!(4.0)
        );
    }

    #[test]
    fn a_generators_migration_reads_a_tables_spawns_as_records() {
        // Twenty crates spawned in the run from the first room's crate: a
        // table in that area's record. Version 2's migration is handed each
        // as a record, and the area restores from what it returns.
        let mut world = world_of(chain(1));
        run(&mut world);
        let source = named(&world, "chain_r0.crate_1").unwrap();
        let mut commands = CommandBuffer::new();
        for at in 0..20_u8 {
            let spawn = world.prepare_prefab("crate", [f32::from(at), 8.0]).unwrap();
            commands.spawn_prefab_from(source, spawn);
        }
        world.flush(commands);
        let save = capture_world(&world).unwrap();
        let held = &value(&save)["areas"]["chain_r0"]["spawns"];
        assert_eq!(held.as_array().unwrap().len(), 1, "one table");
        assert_eq!(held[0]["rows"].as_array().unwrap().len(), 20);

        let seen = Seen::default();
        let migrating = Chain {
            migrate: Some(keep),
            seen: seen.clone(),
            ..chain(2)
        };
        let (world, restored) = restore(migrating, &save);
        restored.unwrap();
        let calls = seen.borrow();
        let spawned = |area: &Value| {
            area["spawns"]
                .as_array()
                .is_some_and(|held| !held.is_empty())
        };
        let (from, _, area) = calls
            .iter()
            .find(|(_, _, area)| spawned(area))
            .expect("the record that holds the spawns");
        assert_eq!(from.version, 1);
        let handed = area["spawns"].as_array().unwrap();
        assert_eq!(handed.len(), 20);
        for (at, spawn) in handed.iter().enumerate() {
            assert!(spawn.get("rows").is_none() && spawn.get("columns").is_none());
            assert_eq!(spawn["id"], json!(at), "by ID");
            assert_eq!(spawn["prefab"], json!("crate"));
            assert_eq!(spawn["components"]["health"]["current"], json!(3.0));
        }
        assert_eq!(spawns(&world).len(), 20, "the migrated record's spawns");
    }

    #[test]
    fn a_template_change_is_refused_without_a_migration() {
        let save = fixture();
        let mut world = world_of(chain(1));
        let mut hall = Room::from_json(HALL).unwrap();
        hall.size = [8, 5];
        world.insert_room("hall", hall).unwrap();
        let rooms = regenerate(&mut world, &save).unwrap();
        let now = rooms[0].generation();
        assert_eq!(now.version, 1);
        assert_ne!(now.templates["hall"], "81171bda5a44e2c5");
        spawn_all(&mut world, &rooms[0]);
        let before = snapshot(&world);
        let err = restore_world(&mut world, &save, &(Scene,)).unwrap_err();
        assert!(
            matches!(&err, RestoreError::Generation { saved, now: Some(now), .. }
                if saved.version == 1 && now.templates != saved.templates),
            "{err:?}"
        );
        assert_eq!(snapshot(&world), before);
    }

    #[test]
    fn a_room_the_new_version_no_longer_makes_reaches_the_migration() {
        let seen = Seen::default();
        let migrating = Chain {
            migrate: Some(keep),
            seen: seen.clone(),
            ..chain(2)
        };
        let (world, restored) = restore(migrating, &fixture());
        assert!(restored.unwrap().is_clean());
        assert!(world.resource::<RoomRegistry>().room("chain_r2").is_none());
        let (from, now, area) = call_for(&seen, "chain_r2.crate_1").expect("chain_r2's record");
        assert_eq!(from.version, 1);
        assert_eq!(
            now.as_ref().map(|now| (now.version, now.seed)),
            Some((2, 7)),
            "the generation of its generator, seed and prefix"
        );
        assert!(area.get("generated").is_none(), "the record without it");
        assert!(world.resource::<SaveState>().is_kept("chain_r2"));
    }

    #[test]
    fn a_kept_and_a_loaded_copy_of_one_area_migrate_alike() {
        let options = || {
            SaveOptions::<Health>::new()
                .version(2)
                .step(1, double_current)
        };
        let save = fixture();
        let (loaded_seen, kept_seen) = (Seen::default(), Seen::default());
        let migrating = |seen: &Seen| Chain {
            migrate: Some(keep),
            seen: seen.clone(),
            ..chain(2)
        };
        let mut loaded = world_of_with(migrating(&loaded_seen), options());
        for rooms in regenerate(&mut loaded, &save).unwrap() {
            spawn_all(&mut loaded, &rooms);
        }
        restore_world(&mut loaded, &save, &(Scene,)).unwrap();
        let mut kept = world_of_with(migrating(&kept_seen), options());
        regenerate(&mut kept, &save).unwrap();
        kept.spawn_room("chain_r1", [128.0, 0.0], &(Scene,))
            .unwrap();
        restore_world(&mut kept, &save, &(Scene,)).unwrap();
        assert!(kept.resource::<SaveState>().is_kept("chain_r0"));
        let (_, _, as_loaded) = call_for(&loaded_seen, "chain_r0.crate_1").unwrap();
        let (_, _, as_kept) = call_for(&kept_seen, "chain_r0.crate_1").unwrap();
        assert_eq!(as_loaded, as_kept);
        assert_eq!(
            as_loaded["named"]["chain_r0.crate_1"]["components"]["health"]["current"],
            json!(2.0),
            "at the registry's version: 1 through its step"
        );
        kept.spawn_room("chain_r0", [0.0, 0.0], &(Scene,)).unwrap();
        enter_area(&mut kept, "chain_r0", &(Scene,)).unwrap();
        assert_eq!(health(&kept, "chain_r0.crate_1"), Some(2.0));
        assert_eq!(
            health(&kept, "chain_r0.crate_1"),
            health(&loaded, "chain_r0.crate_1")
        );
    }

    #[test]
    fn a_migration_takes_the_matched_rooms_generation() {
        let seen = Seen::default();
        let migrating = Chain {
            migrate: Some(keep),
            seen: seen.clone(),
            ..chain(1)
        };
        let mut world = world_of(migrating);
        // `chain_r0` and `chain_r2` from another seed, `chain_r1` authored.
        let other = world.generate_rooms("chain", 8, "chain").unwrap();
        world.remove_room("chain_r1").unwrap();
        world
            .insert_room("chain_r1", Room::from_json(HALL).unwrap())
            .unwrap();
        for (id, origin) in other.rooms() {
            world.spawn_room(id, origin, &(Scene,)).unwrap();
        }
        restore_world(&mut world, &fixture(), &(Scene,)).unwrap();
        let (_, now, _) = call_for(&seen, "chain_r0.crate_1").unwrap();
        assert_eq!(now.as_ref(), Some(other.generation()), "its room's, seed 8");
        let (_, now, _) = call_for(&seen, "chain_r1.crate_2").unwrap();
        assert_eq!(now, None, "an authored room's");
        assert_eq!(
            health(&world, "chain_r0.crate_1"),
            Some(1.0),
            "kept as saved"
        );
        let again = capture_world(&world).unwrap();
        let areas: Vec<&str> = again.generations().map(|(area, _)| area).collect();
        assert_eq!(
            areas,
            ["chain_r0", "chain_r2"],
            "the authored one's is gone"
        );
    }

    #[test]
    fn a_save_naming_a_generation_the_world_did_not_regenerate_is_refused() {
        let mut world = world_of(chain(1));
        let before = snapshot(&world);
        let err = restore_world(&mut world, &fixture(), &(Scene,)).unwrap_err();
        assert!(
            matches!(&err, RestoreError::Generation { area, now: None, .. } if area == "chain_r0"),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .ends_with("and the World's by none; its generator did not migrate it"),
            "{err}"
        );
        assert_eq!(snapshot(&world), before);
        assert!(!world.has_resource::<SaveState>(), "no save state gained");
    }

    #[test]
    fn a_kept_generated_area_enters_with_its_matched_record() {
        let save = fixture();
        let mut world = world_of(chain(1));
        regenerate(&mut world, &save).unwrap();
        world.spawn_room("chain_r0", [0.0, 0.0], &(Scene,)).unwrap();
        assert!(
            restore_world(&mut world, &save, &(Scene,))
                .unwrap()
                .is_clean()
        );
        let state = world.resource::<SaveState>();
        assert!(state.is_kept("chain_r1") && state.is_kept("chain_r2"));
        // A capture before the entry writes the kept records as saved.
        assert_eq!(text(&capture_world(&world).unwrap()), text(&save));
        world
            .spawn_room("chain_r2", [256.0, 0.0], &(Scene,))
            .unwrap();
        assert!(
            enter_area(&mut world, "chain_r2", &(Scene,))
                .unwrap()
                .is_clean()
        );
        assert_eq!(health(&world, "chain_r2.crate_1"), Some(2.0));
        assert!(!world.resource::<SaveState>().is_kept("chain_r2"));
        assert_eq!(text(&capture_world(&world).unwrap()), text(&save));
    }

    #[test]
    fn an_area_with_no_generation_restores_as_before() {
        // The fixture without its records' generations: names decide, a
        // generated room or not, so even a changed generator restores it.
        let mut plain = value(&fixture());
        plain.as_object_mut().unwrap().remove("generated");
        let plain = from_value(plain);
        assert_eq!(plain.generations().count(), 0);
        let mut world = world_of(chain(2));
        let rooms = world.generate_rooms("chain", 7, "chain").unwrap();
        spawn_all(&mut world, &rooms);
        assert!(
            restore_world(&mut world, &plain, &(Scene,))
                .unwrap()
                .is_clean()
        );
        assert_eq!(health(&world, "chain_r0.crate_1"), Some(1.0));
        assert_eq!(named(&world, "chain_r1.crate_2"), None);
        assert!(world.resource::<SaveState>().is_kept("chain_r2"));
        // A World with no generated room writes no generation.
        let mut old = super::world();
        level(&mut old);
        assert!(!text(&capture_world(&old).unwrap()).contains("generated"));
    }
}

#[test]
fn a_kept_identitys_holder_has_no_record_and_resolves_after_a_restore_that_spawned_it_back() {
    use crate::save::take_id;

    let mut a = world();
    let la = level(&mut a);
    let pet_a = a.spawn_with((Health {
        current: 4.0,
        max: 4.0,
        shield: None,
    },));
    let id = take_id(&mut a).unwrap();
    a.insert(pet_a, Saved::kept(id));
    a.get_mut::<Chase>(la.bat_2).unwrap().target = pet_a;
    let saved = capture_world(&a).unwrap();
    assert_eq!(saved.dropped_refs(), 0, "referenced by its ID");
    let json = value(&saved);
    assert_eq!(
        json["areas"]["cave"]["named"]["bat_2"]["refs"]["chase"]["target"],
        json!([id.0])
    );
    assert_eq!(json["global"]["spawns"], json!([]), "no record of it");
    assert_eq!(json["areas"]["cave"]["spawns"], json!([]));
    assert!(
        !text(&saved).contains(r#""max":4.0"#),
        "its holder's components are the game's to save"
    );

    let mut b = world();
    churn(&mut b, 40);
    let lb = level(&mut b);
    let pet_b = b.spawn_with((Saved::kept(id),));
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(b.get::<Chase>(lb.bat_2).unwrap().target, pet_b);
    assert_eq!(b.resource::<SaveState>().next_id(), 1);

    let mut c = world();
    let lc = level(&mut c);
    let report = restore_world(&mut c, &saved, &(Scene,)).unwrap();
    assert_eq!(
        report.dropped,
        [format!(
            "`chase`'s reference `target` to save ID {}: no such entity",
            id.0
        )],
        "not spawned back first, it cannot resolve"
    );
    assert_ne!(c.get::<Chase>(lc.bat_2).unwrap().target, lc.bat_1);
}

#[test]
fn a_prepared_saved_spawn_is_captured_whole_and_restored_from_its_prefab() {
    let mut a = world();
    level(&mut a);
    let bolt = a
        .prepare_prefab("bolt", [60.0, 40.0])
        .unwrap()
        .saved(&mut a, Some("cave"));
    let id = bolt.save_id().unwrap();
    let bolt = bolt.spawn(&mut a);
    a.get_mut::<Health>(bolt).unwrap().current = 0.5;
    let saved = capture_world(&a).unwrap();
    let json = value(&saved);
    let records = json["areas"]["cave"]["spawns"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["id"], json!(id.0));
    assert_eq!(records[0]["prefab"], json!("bolt"));
    assert_eq!(
        records[0]["components"],
        json!({ "brain": { "mood": "wild", "timer": 0.0, "seen": [] },
                "health": { "current": 0.5, "max": 1.0 } }),
        "whole"
    );
    assert_eq!(json["next_id"], json!(1));

    let mut b = world();
    churn(&mut b, 40);
    level(&mut b);
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    let restored = spawns(&b);
    assert_eq!(restored.len(), 1);
    let (restored_id, bolt) = restored[0];
    assert_eq!(restored_id, id);
    let tagged = b.get::<Saved>(bolt).unwrap();
    assert_eq!(
        (tagged.area(), tagged.prefab()),
        (Some("cave"), Some("bolt"))
    );
    assert_eq!(b.get::<Health>(bolt).unwrap().current, 0.5);
    assert_eq!(b.get::<Tag>(bolt).unwrap().name, "bolt", "from its prefab");
    assert!(b.has::<Scene>(bolt));
    assert_eq!(b.resource::<SaveState>().next_id(), 1);
}

/// A gauge whose shown level a save holds as its holder's ammo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Meter {
    shown: f32,
    tint: u8,
}

/// `Meter` as a save holds it: its level at its holder's ammo, when it has
/// any.
fn meter_saved(world: &World, holder: Option<Entity>, live: &Meter) -> Option<Meter> {
    let &Ammo(ammo) = world.get::<Ammo>(holder?)?;
    Some(Meter {
        shown: ammo as f32,
        ..live.clone()
    })
}

/// A forecast a save holds as the weather's wind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Forecast {
    wind: f32,
}

fn forecast_saved(world: &World, holder: Option<Entity>, _: &Forecast) -> Option<Forecast> {
    assert!(holder.is_none(), "a resource has no holder");
    world
        .get_resource::<Weather>()
        .map(|weather| Forecast { wind: weather.wind })
}

/// The entity it aims at, which a save holds as what its holder chases.
#[derive(Debug, Clone, PartialEq)]
struct Aim {
    at: Entity,
}

#[derive(Serialize, Deserialize)]
struct AimForm {}

impl From<&Aim> for AimForm {
    fn from(_: &Aim) -> Self {
        Self {}
    }
}

impl From<AimForm> for Aim {
    fn from(_: AimForm) -> Self {
        Self {
            at: Entity::PLACEHOLDER,
        }
    }
}

fn aim_saved(world: &World, holder: Option<Entity>, _: &Aim) -> Option<Aim> {
    Some(Aim {
        at: world.get::<Chase>(holder?)?.target,
    })
}

/// `world()` with `Meter`, `Aim` and `Forecast` saved through `capture_as`.
fn projected_world() -> World {
    let mut world = world();
    world.register_saved_with("meter", SaveOptions::<Meter>::new().capture_as(meter_saved));
    world.register_saved_with(
        "aim",
        SaveOptions::<Aim>::form::<AimForm>()
            .refs(RefCodec {
                save: |aim, out| out.put("at", aim.at),
                load: |aim, refs| {
                    if let Some(at) = refs.first("at") {
                        aim.at = at;
                    }
                },
                keys: |_| vec!["at".into()],
            })
            .capture_as(aim_saved),
    );
    world.register_saved_resource_with(
        "forecast",
        SaveOptions::<Forecast>::new().capture_as(forecast_saved),
    );
    world.insert_resource(Forecast { wind: -1.0 });
    world
}

/// A hero and a turret, each with a meter; the turret's shows its ammo.
fn metered(world: &mut World) -> [Entity; 2] {
    let hero = world.spawn_prefab("hero", [16.0, 16.0]).unwrap();
    let turret = world.spawn_prefab("turret", [200.0, 40.0]).unwrap();
    for entity in [hero, turret] {
        world.insert(
            entity,
            Meter {
                shown: 9.0,
                tint: 1,
            },
        );
    }
    name(world, hero, None, "hero").unwrap();
    name(world, turret, Some("cave"), "turret").unwrap();
    [hero, turret]
}

#[test]
fn a_capture_as_value_is_what_a_save_writes_keeps_and_patches_over() {
    let mut a = projected_world();
    let [hero_a, turret_a] = metered(&mut a);
    let fresh = value(&capture_world(&a).unwrap());
    assert_eq!(
        fresh["areas"]["cave"]["named"].get("turret"),
        None,
        "the baseline holds the saved level, 3, not the live 9"
    );
    assert_eq!(fresh["resources"]["forecast"], json!({ "wind": 0.0 }));

    // The live level moves alone: nothing to save. The tint and the ammo
    // move: both are patched, the level at the ammo.
    a.get_mut::<Meter>(turret_a).unwrap().shown = 7.0;
    a.get_mut::<Meter>(hero_a).unwrap().shown = 4.0;
    assert_eq!(
        value(&capture_world(&a).unwrap())["areas"]["cave"]["named"].get("turret"),
        None
    );
    a.get_mut::<Meter>(turret_a).unwrap().tint = 5;
    a.resource_mut::<Weather>().wind = 3.0;
    let saved = value(&capture_world(&a).unwrap());
    assert_eq!(
        saved["areas"]["cave"]["named"]["turret"]["components"],
        json!({ "meter": { "tint": 5 } })
    );
    assert_eq!(
        saved["global"]["named"]["hero"]["components"]["meter"],
        json!({ "shown": 4.0 }),
        "no ammo: the live value"
    );
    assert_eq!(saved["resources"]["forecast"], json!({ "wind": 3.0 }));

    // The patch goes over the fresh spawn's saved value, its level 3, not
    // its live 9.
    let mut b = projected_world();
    let [hero_b, turret_b] = metered(&mut b);
    restore_world(&mut b, &from_value(saved), &(Scene,)).unwrap();
    assert_eq!(
        b.get::<Meter>(turret_b),
        Some(&Meter {
            shown: 3.0,
            tint: 5
        })
    );
    assert_eq!(b.get::<Meter>(hero_b).unwrap().shown, 4.0);
    assert_eq!(b.resource::<Forecast>(), &Forecast { wind: 3.0 });
}

#[test]
fn a_capture_as_value_writes_its_own_references() {
    let mut world = projected_world();
    let level = level(&mut world);
    let stray = world.spawn();
    world.insert(level.bat_1, Aim { at: stray });
    let saved = capture_world(&world).unwrap();
    assert_eq!(saved.dropped_refs(), 0, "the live aim's stray is not saved");
    assert_eq!(
        value(&saved)["areas"]["cave"]["named"]["bat_1"]["refs"]["aim"],
        json!({ "at": [SaveId::authored("hero").0] }),
        "the saved aim's, at what it chases"
    );
}

#[test]
fn check_world_refuses_what_restore_world_refuses_and_changes_nothing() {
    let mut a = world();
    let la = level(&mut a);
    fire(&mut a, la.turret, 210.0);
    flush(&mut a);
    a.despawn(la.bat_2);
    a.get_mut::<Health>(la.bat_1).unwrap().current = 2.0;
    a.resource_mut::<Weather>().wind = 4.0;
    let saved = value(&capture_world(&a).unwrap());

    let mut b = world();
    level(&mut b);
    let before = |world: &World| {
        (
            snapshot(world),
            format!("{:?}", world.resource::<SaveState>()),
        )
    };
    let fresh = before(&b);

    let mut newer = saved.clone();
    newer["versions"]["components"]["health"] = json!(5);
    let newer = from_value(newer);
    assert_eq!(
        check_world(&mut b, &newer),
        restore_world(&mut b, &newer, &(Scene,)),
    );
    assert!(matches!(
        check_world(&mut b, &newer),
        Err(RestoreError::NewerVersion { .. })
    ));
    assert_eq!(before(&b), fresh);

    let mut failing = saved.clone();
    failing["resources"]["weather"] = json!({ "raining": "yes" });
    let failing = from_value(failing);
    let err = check_world(&mut b, &failing).unwrap_err();
    assert!(
        matches!(&err, RestoreError::Value { what, .. } if what == "resource `weather`"),
        "{err}"
    );
    assert_eq!(Err(err), restore_world(&mut b, &failing, &(Scene,)));
    assert_eq!(before(&b), fresh);

    let saved = from_value(saved);
    let report = check_world(&mut b, &saved).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(before(&b), fresh, "a passing check changes nothing either");
    assert_eq!(restore_world(&mut b, &saved, &(Scene,)), Ok(report));
    assert_eq!(b.resource::<Weather>().wind, 4.0);
}

// The capture's spawns read an archetype at a time (phase 7's C5), against
// the walk that asks every registered component of every spawn.

/// `world`'s capture as text, its spawns read both ways: the saves are
/// equal, their bytes and the references they counted as dropped.
fn captured_both_ways(world: &World) -> (String, usize) {
    let by_archetype = capture_world(world).unwrap();
    let asked = capture_world_asking_every_component(world).unwrap();
    assert_eq!(text(&by_archetype), text(&asked));
    assert_eq!(by_archetype.dropped_refs(), asked.dropped_refs());
    assert_eq!(by_archetype, asked);
    (text(&by_archetype), by_archetype.dropped_refs())
}

/// A run whose spawns differ: the level, a second turret in area `tower`,
/// and 60 bolts shot from the hero (global), the cave's turret and the
/// tower's by seeded turns, each then losing its brain or its health,
/// gaining ammo, or chasing the hero, the cave's bat, another bolt or an
/// entity no save holds, by seeded picks. Returns the bolts.
fn varied_run(world: &mut World, seed: u64) -> Vec<Entity> {
    let level = level(world);
    let tower = world.spawn_prefab("turret", [300.0, 40.0]).unwrap();
    name(world, tower, Some("tower"), "tower_turret").unwrap();
    let mut stream = seed;
    let mut draw = move || {
        stream = stream.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = stream;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    for shot in 0..60_u8 {
        let source = [level.hero, level.turret, tower][(draw() % 3) as usize];
        shoot(world, source, 10.0 * f32::from(shot));
    }
    flush(world);
    let stray = world.spawn();
    let bolts: Vec<Entity> = spawns(world).into_iter().map(|(_, bolt)| bolt).collect();
    assert_eq!(bolts.len(), 60);
    for (at, &bolt) in bolts.iter().enumerate() {
        let pick = draw();
        if pick & 1 != 0 {
            world.remove_component::<Brain>(bolt);
        }
        if pick & 2 != 0 {
            world.insert(bolt, Ammo((pick >> 8) as u32 % 9));
        }
        if pick & 4 != 0 {
            world.remove_component::<Health>(bolt);
        }
        if pick & 0b1_1000 != 0 {
            let target = match (pick >> 5) % 4 {
                0 => level.hero,
                1 => level.bat_1,
                2 => bolts[(at + 7) % bolts.len()],
                _ => stray,
            };
            world.insert(
                bolt,
                Chase {
                    target,
                    speed: at as f32,
                },
            );
        }
    }
    bolts
}

#[test]
fn a_capture_by_archetype_writes_the_bytes_asking_every_component_does() {
    for seed in [1_u64, 2, 3] {
        let mut world = world();
        let bolts = varied_run(&mut world, seed);
        // The spawns are spread over component sets, each an archetype.
        let sets: BTreeSet<Vec<&str>> = bolts
            .iter()
            .map(|&bolt| {
                let types = world.component_types(bolt);
                types.into_iter().map(|(_, name)| name).collect()
            })
            .collect();
        assert!(
            sets.len() >= 8,
            "seed {seed}: {} component sets",
            sets.len()
        );
        let (saved, dropped) = captured_both_ways(&world);
        let saved: Value = serde_json::from_str(&saved).unwrap();
        // Three areas' records hold them, and the references to the entity
        // no save holds were dropped and counted.
        let held = |area: &Value| area["spawns"].as_array().map_or(0, Vec::len);
        let (global, cave, tower) = (
            held(&saved["global"]),
            held(&saved["areas"]["cave"]),
            held(&saved["areas"]["tower"]),
        );
        assert_eq!(global + cave + tower, 60, "seed {seed}");
        assert!(global > 5 && cave > 5 && tower > 5, "seed {seed}");
        assert!(dropped > 0, "seed {seed}");
        // The run goes on: bolts despawn, a bolt changes its set, others
        // move to a set no spawn had, and the walks still agree.
        for &bolt in bolts.iter().step_by(4) {
            world.despawn(bolt);
        }
        world.remove_component::<Chase>(bolts[1]);
        struct Marked;
        world.insert(bolts[2], Marked);
        world.insert(bolts[3], Marked);
        let later: Value = serde_json::from_str(&captured_both_ways(&world).0).unwrap();
        let spawned = held(&later["global"]) + held(&later["areas"]["cave"]);
        assert_eq!(spawned + held(&later["areas"]["tower"]), 45, "seed {seed}");
    }
}

#[test]
fn a_capture_by_archetype_writes_its_spawns_waiting_references() {
    // Two bolts shot from the hero, global spawns: one chases the cave's
    // bat, one the hero. The restoring World has not loaded the cave, so
    // the cave is kept and the first bolt's reference waits; a capture adds
    // it to that bolt's record, by either walk.
    let mut a = world();
    let (hero, _, bat) = hero_dog(&mut a, true);
    shoot(&mut a, hero, 10.0);
    shoot(&mut a, hero, 20.0);
    flush(&mut a);
    let bolts = spawns(&a);
    let (waits, speed) = (bolts[0].0, 3.0);
    a.insert(
        bolts[0].1,
        Chase {
            target: bat.unwrap(),
            speed,
        },
    );
    a.insert(
        bolts[1].1,
        Chase {
            target: hero,
            speed: 4.0,
        },
    );
    let (first, _) = captured_both_ways(&a);

    let mut b = world();
    hero_dog(&mut b, false);
    restore_world(&mut b, &serde_json::from_str(&first).unwrap(), &(Scene,)).unwrap();
    let state = b.resource::<SaveState>();
    assert!(state.is_kept("cave"));
    let waiting: Vec<_> = state
        .pending
        .iter()
        .filter(|pending| pending.holder == Some(waits))
        .collect();
    assert_eq!(waiting.len(), 1, "the bolt's reference waits");
    let second: Value = serde_json::from_str(&captured_both_ways(&b).0).unwrap();
    let records = second["global"]["spawns"].as_array().unwrap();
    assert_eq!(records.len(), 2);
    let record = records
        .iter()
        .find(|record| record["id"] == json!(waits.0))
        .unwrap();
    assert_eq!(record["components"]["chase"]["speed"], json!(speed));
    assert_eq!(
        record["refs"]["chase"]["target"],
        json!(waiting[0].ids),
        "the waiting reference is written with the record"
    );
}

/// A saved value whose `Serialize` fails on a NaN.
#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Brittle {
    load: f32,
}

impl Serialize for Brittle {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        if self.load.is_nan() {
            return Err(serde::ser::Error::custom("the load is not a number"));
        }
        let mut fields = serializer.serialize_struct("Brittle", 1)?;
        fields.serialize_field("load", &self.load)?;
        fields.end()
    }
}

#[test]
fn a_spawn_that_fails_to_serialize_is_named_by_either_walk() {
    // Three bolts carry a `Brittle`; the second's fails. Both walks stop at
    // it with one error, which names the spawn by its ID and the component.
    let mut world = world();
    world.register_saved::<Brittle>("brittle");
    let level = level(&mut world);
    for shot in 0..3_u8 {
        shoot(&mut world, level.turret, 10.0 * f32::from(shot));
    }
    flush(&mut world);
    let bolts = spawns(&world);
    for (at, &(_, bolt)) in bolts.iter().enumerate() {
        let load = if at == 1 { f32::NAN } else { 2.0 };
        world.insert(bolt, Brittle { load });
    }
    let by_archetype = capture_world(&world).unwrap_err();
    let asked = capture_world_asking_every_component(&world).unwrap_err();
    assert_eq!(by_archetype, asked);
    assert_eq!(
        by_archetype,
        CaptureError::Serialize {
            what: format!("`spawn {}`'s `brittle`", bolts[1].0.0),
            message: "the load is not a number".into(),
        }
    );
}

// 7.14's C5 (`D-295`): sixteen or more spawns of one area, one prefab and
// one set of saved components are a table. The reference is the walk that
// asks every component of every spawn, which writes records alone.

/// `world`'s capture and the capture of the walk that writes records alone:
/// the first with its tables expanded is the second, as values and byte for
/// byte, both dropped the same references, and a second capture is the
/// first's bytes.
#[track_caller]
fn captured_and_expanded(world: &World) -> (WorldSave, WorldSave) {
    let tabled = capture_world(world).unwrap();
    let asked = capture_world_asking_every_component(world).unwrap();
    let mut expanded = value(&tabled);
    crate::save::expand_tables(&mut expanded).unwrap();
    assert_eq!(expanded, value(&asked), "the expansion is the records");
    assert_eq!(text(&from_value(expanded)), text(&asked), "byte for byte");
    assert_eq!(tabled.dropped_refs(), asked.dropped_refs());
    assert_eq!(text(&capture_world(world).unwrap()), text(&tabled));
    (tabled, asked)
}

/// How `saved`'s record of `area` (the global one for `None`) holds its
/// spawns of `prefab`: the records, and each table's rows.
fn held_as(saved: &Value, area: Option<&str>, prefab: &str) -> (usize, Vec<usize>) {
    let record = match area {
        Some(area) => &saved["areas"][area],
        None => &saved["global"],
    };
    let (mut records, mut tables) = (0, Vec::new());
    for spawn in record["spawns"].as_array().into_iter().flatten() {
        if spawn["prefab"] != json!(prefab) {
            continue;
        }
        match spawn["rows"].as_array() {
            Some(rows) => tables.push(rows.len()),
            None => records += 1,
        }
    }
    (records, tables)
}

/// A run whose spawns fill tables: the level, a turret in area `tower`,
/// 300 bolts shot from the hero (global), the cave's turret and the tower's
/// by seeded turns, then fifteen `spark`s and sixteen `ember`s from the
/// cave's turret. Each bolt then loses its brain or its health, gains ammo
/// or a shield (a field its saved form skips without one), or chases the
/// hero (most), the cave's bat, another bolt or an entity no save holds, by
/// seeded picks drawn in ID order and applied in that order or, `backwards`,
/// the other way, so the archetypes are made in another order. Returns the
/// bolts by ID.
fn tabled_run(world: &mut World, seed: u64, backwards: bool) -> Vec<Entity> {
    let level = level(world);
    let tower = world.spawn_prefab("turret", [300.0, 40.0]).unwrap();
    name(world, tower, Some("tower"), "tower_turret").unwrap();
    for id in ["spark", "ember"] {
        let ember = prefab(json!({ "tag": id, "components": {
            "health": { "current": 1.0, "max": 1.0 } } }));
        world
            .resource_mut::<PrefabRegistry>()
            .insert(id.into(), ember);
    }
    let mut stream = seed;
    let mut draw = move || {
        stream = stream.wrapping_add(0x9E37_79B9_7F4A_7C15);
        crate::rng::splitmix64(stream)
    };
    for shot in 0..300_u16 {
        let source = [level.hero, level.turret, tower][(draw() % 3) as usize];
        shoot(world, source, f32::from(shot));
    }
    for shot in 0..31_u8 {
        let id = if shot < 15 { "spark" } else { "ember" };
        let prepared = world.prepare_prefab(id, [f32::from(shot), 80.0]).unwrap();
        world
            .resource_mut::<CommandBuffer>()
            .spawn_prefab_from(level.turret, prepared);
    }
    flush(world);
    let stray = world.spawn();
    let bolts: Vec<Entity> = spawns(world)
        .into_iter()
        .take(300)
        .map(|(_, bolt)| bolt)
        .collect();
    let picks: Vec<u64> = bolts.iter().map(|_| draw()).collect();
    let mut order: Vec<usize> = (0..bolts.len()).collect();
    if backwards {
        order.reverse();
    }
    for at in order {
        let (bolt, pick) = (bolts[at], picks[at]);
        if pick % 8 == 0 {
            world.remove_component::<Brain>(bolt);
        }
        if (pick >> 3) % 4 == 0 {
            world.insert(bolt, Ammo((pick >> 8) as u32 % 9));
        }
        if (pick >> 5) % 8 == 0 {
            world.remove_component::<Health>(bolt);
        } else if (pick >> 12) % 6 == 0 {
            world.get_mut::<Health>(bolt).unwrap().shield = Some(0.5);
        }
        if (pick >> 16) % 2 == 0 {
            let target = match (pick >> 20) % 16 {
                0..=11 => level.hero,
                12 => level.bat_1,
                13 | 14 => bolts[(at + 7) % bolts.len()],
                _ => stray,
            };
            world.insert(
                bolt,
                Chase {
                    target,
                    speed: (at % 4) as f32,
                },
            );
        }
    }
    bolts
}

#[test]
fn a_capture_with_tables_expands_to_the_bytes_of_records_alone() {
    // Over the seeds: tables of bolts, and tables that share a reference.
    let (mut all_tables, mut all_shared) = (0, 0);
    for seed in [1_u64, 2, 3, 4, 5, 6] {
        let mut world = world();
        let bolts = tabled_run(&mut world, seed, false);
        let sets: BTreeSet<Vec<&str>> = bolts
            .iter()
            .map(|&bolt| {
                let types = world.component_types(bolt);
                types.into_iter().map(|(_, name)| name).collect()
            })
            .collect();
        assert!(sets.len() >= 8, "seed {seed}: {} sets", sets.len());
        let (tabled, asked) = captured_and_expanded(&world);
        assert!(asked.dropped_refs() > 0, "seed {seed}");
        let saved = value(&tabled);
        assert_eq!(saved["format"], json!(2));

        // The bolts, over three areas: tables of sixteen rows or more, and
        // records for the small groups and for the bolts whose references
        // or whose layout are not their table's.
        let (mut records, mut rows, mut tables) = (0, 0, 0);
        for area in [None, Some("cave"), Some("tower")] {
            let (alone, tabled) = held_as(&saved, area, "bolt");
            assert!(tabled.iter().all(|&rows| rows >= 16), "seed {seed}");
            records += alone;
            rows += tabled.iter().sum::<usize>();
            tables += tabled.len();
        }
        assert_eq!(records + rows, 300, "seed {seed}");
        assert!(records > 100, "seed {seed}: {records} records");
        all_tables += tables;
        // A table of bolts that chase the hero writes the reference once.
        let shared = saved["areas"]
            .as_object()
            .unwrap()
            .values()
            .chain([&saved["global"]])
            .flat_map(|area| area["spawns"].as_array().unwrap())
            .filter(|spawn| spawn["rows"].is_array() && spawn["refs"]["chase"].is_object())
            .count();
        all_shared += shared;
        // Fifteen of a prefab stay records; sixteen are a table.
        assert_eq!(held_as(&saved, Some("cave"), "spark"), (15, vec![]));
        assert_eq!(held_as(&saved, Some("cave"), "ember"), (0, vec![16]));

        // The run goes on: bolts and an ember despawn, others move to sets
        // no spawn had, and the capture still expands to the records.
        for &bolt in bolts.iter().step_by(4) {
            world.despawn(bolt);
        }
        for &bolt in bolts.iter().skip(1).step_by(8) {
            world.insert(bolt, Ammo(7));
        }
        let ember = spawns(&world).last().unwrap().1;
        world.despawn(ember);
        let later = value(&captured_and_expanded(&world).0);
        assert_eq!(held_as(&later, Some("cave"), "ember"), (15, vec![]));
    }
    assert!(
        all_tables >= 9 && all_shared >= 2,
        "{all_tables} tables of bolts, {all_shared} sharing a reference"
    );
}

#[test]
fn a_table_holds_the_references_waiting_on_a_kept_area() {
    // Twenty bolts shot from the hero, global spawns, each chasing the
    // cave's bat. The restoring World has not loaded the cave, so each
    // bolt's reference waits; a capture writes it once, with their table.
    let mut a = world();
    let (hero, _, bat) = hero_dog(&mut a, true);
    for shot in 0..20_u8 {
        shoot(&mut a, hero, f32::from(shot));
    }
    flush(&mut a);
    for (_, bolt) in spawns(&a) {
        a.insert(
            bolt,
            Chase {
                target: bat.unwrap(),
                speed: 3.0,
            },
        );
    }
    let (first, _) = captured_and_expanded(&a);
    assert_eq!(held_as(&value(&first), None, "bolt"), (0, vec![20]));

    let mut b = world();
    hero_dog(&mut b, false);
    restore_world(&mut b, &first, &(Scene,)).unwrap();
    let state = b.resource::<SaveState>();
    assert!(state.is_kept("cave"));
    assert_eq!(state.pending.len(), 20, "each bolt's reference waits");
    let (second, _) = captured_and_expanded(&b);
    assert_eq!(text(&second), text(&first));
    let table = &value(&second)["global"]["spawns"][0];
    assert_eq!(
        table["refs"]["chase"]["target"],
        json!([SaveId::authored("bat_1")])
    );
}

#[test]
fn a_tabled_capture_does_not_depend_on_the_order_the_world_was_made_in() {
    for seed in [4_u64, 5] {
        let mut a = world();
        tabled_run(&mut a, seed, false);
        let saved = capture_world(&a).unwrap();
        assert!(text(&saved).contains("\"rows\""));
        // The same run, its archetypes made the other way round and its
        // entities at other indices.
        let mut b = world();
        churn(&mut b, 70);
        tabled_run(&mut b, seed, true);
        assert_eq!(
            text(&capture_world(&b).unwrap()),
            text(&saved),
            "seed {seed}"
        );
        // And restored into other allocations.
        let mut c = world();
        churn(&mut c, 40);
        level(&mut c);
        let tower = c.spawn_prefab("turret", [300.0, 40.0]).unwrap();
        name(&mut c, tower, Some("tower"), "tower_turret").unwrap();
        for id in ["spark", "ember"] {
            let ember = prefab(json!({ "tag": id, "components": {
                "health": { "current": 1.0, "max": 1.0 } } }));
            c.resource_mut::<PrefabRegistry>().insert(id.into(), ember);
        }
        let report = restore_world(&mut c, &saved, &(Scene,)).unwrap();
        assert!(report.is_clean(), "{report:?}");
        assert_eq!(
            text(&capture_world(&c).unwrap()),
            text(&saved),
            "seed {seed}"
        );
    }
}

#[test]
fn spawns_with_references_of_their_own_open_one_table_and_serialize_once() {
    // 2,000 bolts of one group, each chasing the one before it: no two hold
    // the same references, so none joins the table their lowest ID opens.
    let mut world = world();
    let level = level(&mut world);
    for shot in 0..2_000_u16 {
        shoot(&mut world, level.turret, f32::from(shot));
    }
    flush(&mut world);
    let bolts = spawns(&world);
    for pair in bolts.windows(2) {
        world.insert(
            pair[1].1,
            Chase {
                target: pair[0].1,
                speed: 1.0,
            },
        );
    }
    world.insert(
        bolts[0].1,
        Chase {
            target: level.hero,
            speed: 1.0,
        },
    );
    CAPTURE_COUNTS.with(|counts| counts.set((0, 0)));
    let tabled = capture_world(&world).unwrap();
    let (opened, serialized) = CAPTURE_COUNTS.with(std::cell::Cell::get);
    assert_eq!(opened, 1, "one group, one table opened");
    assert_eq!(serialized, 2_000, "each spawn's components serialized once");
    assert_eq!(
        held_as(&value(&tabled), Some("cave"), "bolt"),
        (2_000, vec![])
    );
    assert_eq!(
        text(&tabled),
        text(&capture_world_asking_every_component(&world).unwrap())
    );
}

/// A World of the level, the tower and the two extra prefabs, ready for a
/// restore of a [`tabled_run`]'s save.
fn tabled_level(world: &mut World) {
    level(world);
    let tower = world.spawn_prefab("turret", [300.0, 40.0]).unwrap();
    name(world, tower, Some("tower"), "tower_turret").unwrap();
    for id in ["spark", "ember"] {
        let ember = prefab(json!({ "tag": id, "components": {
            "health": { "current": 1.0, "max": 1.0 } } }));
        world
            .resource_mut::<PrefabRegistry>()
            .insert(id.into(), ember);
    }
}

#[test]
fn a_tabled_save_and_its_expansion_restore_alike() {
    for seed in [6_u64, 7] {
        let mut a = world();
        tabled_run(&mut a, seed, false);
        let (tabled, asked) = captured_and_expanded(&a);
        // Twin Worlds: one restores the tables, one their records.
        let restored: Vec<World> = [&tabled, &asked]
            .into_iter()
            .map(|save| {
                let mut world = world();
                tabled_level(&mut world);
                let report = check_world(&mut world, save).unwrap();
                assert!(report.is_clean(), "seed {seed}: {report:?}");
                let report = restore_world(&mut world, save, &(Scene,)).unwrap();
                assert!(report.is_clean(), "seed {seed}: {report:?}");
                world
            })
            .collect();
        for world in &restored {
            assert_eq!(text(&capture_world(world).unwrap()), text(&tabled));
            assert_eq!(chases(world), chases(&a), "seed {seed}");
        }
        // Each area's spawns were made in ID order, by either save, so the
        // twins hold each ID on the same entity.
        assert_eq!(spawns(&restored[0]), spawns(&restored[1]), "seed {seed}");
        let by_area = |world: &World| {
            let mut areas: BTreeMap<Option<String>, Vec<(SaveId, u32)>> = BTreeMap::new();
            for (id, entity) in spawns(world) {
                let area = world
                    .get::<Saved>(entity)
                    .unwrap()
                    .area()
                    .map(str::to_owned);
                areas.entry(area).or_default().push((id, entity.id()));
            }
            areas
        };
        for (area, made) in by_area(&restored[0]) {
            assert!(
                made.windows(2).all(|pair| pair[0].1 < pair[1].1),
                "seed {seed}: area {area:?} out of ID order"
            );
        }
    }

    // A format-1 save, records alone, restores and captures as format 2.
    let mut a = world();
    let la = level(&mut a);
    for x in [210.0, 220.0, 230.0] {
        fire(&mut a, la.turret, x);
    }
    flush(&mut a);
    let now = capture_world(&a).unwrap();
    let mut old = value(&now);
    old["format"] = json!(1);
    let mut b = world();
    level(&mut b);
    let report = restore_world(&mut b, &from_value(old), &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(text(&capture_world(&b).unwrap()), text(&now));
}

/// Twenty bolts in the cave from its turret, five of them shielded, and the
/// capture: a table of fifteen... of the unshielded and records beside it.
fn cave_of_bolts() -> (World, WorldSave) {
    let mut world = world();
    let level = level(&mut world);
    for shot in 0..40_u8 {
        shoot(&mut world, level.turret, f32::from(shot));
    }
    flush(&mut world);
    for (at, (_, bolt)) in spawns(&world).into_iter().enumerate() {
        world.get_mut::<Health>(bolt).unwrap().current = at as f32;
        if at % 8 == 3 {
            world.get_mut::<Health>(bolt).unwrap().shield = Some(2.0);
        }
    }
    let saved = capture_world(&world).unwrap();
    (world, saved)
}

/// A World that named the hero alone: the cave is not loaded, so a restore
/// keeps it.
fn without_the_cave(options: SaveOptions<Health>) -> World {
    let mut world = world_with(options);
    let hero = world.spawn_prefab("hero", [16.0, 16.0]).unwrap();
    name(&mut world, hero, None, "hero").unwrap();
    world
}

#[test]
fn a_kept_table_is_checked_as_its_records_are() {
    let (_, saved) = cave_of_bolts();
    assert_eq!(held_as(&value(&saved), Some("cave"), "bolt"), (5, vec![35]));
    let mut expanded = value(&saved);
    crate::save::expand_tables(&mut expanded).unwrap();
    let expanded = from_value(expanded);

    // Kept as it came: a capture before the area loads writes the table.
    let mut kept = without_the_cave(SaveOptions::new());
    assert!(
        restore_world(&mut kept, &saved, &(Scene,))
            .unwrap()
            .is_clean()
    );
    assert!(kept.resource::<SaveState>().is_kept("cave"));
    assert_eq!(text(&capture_world(&kept).unwrap()), text(&saved));

    // A column saved at an older version: the table is expanded, migrated
    // and checked as its records are, and kept as those records.
    let newer = || {
        without_the_cave(SaveOptions::new().version(2).step(
            1,
            |mut value| -> Result<Value, String> {
                let current = value["current"].as_f64().ok_or("no current")?;
                value["current"] = json!(current * 2.0);
                Ok(value)
            },
        ))
    };
    let captured: Vec<String> = [&saved, &expanded]
        .into_iter()
        .map(|save| {
            let mut world = newer();
            assert!(
                restore_world(&mut world, save, &(Scene,))
                    .unwrap()
                    .is_clean()
            );
            text(&capture_world(&world).unwrap())
        })
        .collect();
    assert_eq!(captured[0], captured[1]);
    let migrated: Value = serde_json::from_str(&captured[0]).unwrap();
    assert_eq!(held_as(&migrated, Some("cave"), "bolt"), (40, vec![]));
    assert_eq!(
        migrated["areas"]["cave"]["spawns"][7]["components"]["health"]["current"],
        json!(14.0),
        "7, doubled by the step"
    );

    // A bad value at its registered version fails the check and the
    // restore as its record's does, and changes nothing.
    let mut bad = value(&saved);
    let table = bad["areas"]["cave"]["spawns"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|spawn| spawn["rows"].is_array())
        .unwrap();
    let columns: Vec<String> = table["columns"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(columns[0], "brain", "the first leaf is the brain's mood");
    table["rows"][9][1] = json!(false);
    let id = table["rows"][9][0].as_u64().unwrap();
    let mut bad_expanded = bad.clone();
    crate::save::expand_tables(&mut bad_expanded).unwrap();
    let errors: Vec<RestoreError> = [bad, bad_expanded]
        .into_iter()
        .map(|save| {
            let save = from_value(save);
            let mut world = without_the_cave(SaveOptions::new());
            let before = snapshot(&world);
            let checked = check_world(&mut world, &save).unwrap_err();
            let restored = restore_world(&mut world, &save, &(Scene,)).unwrap_err();
            assert_eq!(checked, restored);
            assert_eq!(snapshot(&world), before, "nothing changed");
            assert!(world.get_resource::<SaveState>().unwrap().kept.is_empty());
            restored
        })
        .collect();
    assert_eq!(errors[0], errors[1]);
    assert!(
        matches!(&errors[0], RestoreError::Value { what, .. }
            if *what == format!("spawn {id} (`bolt`)'s `brain`")),
        "{:?}",
        errors[0]
    );

    // A component the registry lacks is dropped with its record's line,
    // and the rest is kept as records.
    let lacking = || {
        let mut world = World::new();
        PhysicsPlugin.build(&mut Schedule::new(), &mut world);
        world.register_component::<Health>("health");
        world.register_saved::<Health>("health");
        world.insert_resource(PrefabRegistry::new());
        world.insert_resource(CommandBuffer::new());
        world
    };
    let reports: Vec<(RestoreReport, String)> = [&saved, &expanded]
        .into_iter()
        .map(|save| {
            let mut world = lacking();
            let report = restore_world(&mut world, save, &(Scene,)).unwrap();
            (report, text(&capture_world(&world).unwrap()))
        })
        .collect();
    assert_eq!(reports[0], reports[1]);
    let brains = reports[0]
        .0
        .dropped
        .iter()
        .filter(|line| line.ends_with("'s `brain`: not a saved component"))
        .count();
    assert_eq!(brains, 40, "{:?}", reports[0].0.dropped);
    let kept: Value = serde_json::from_str(&reports[0].1).unwrap();
    assert_eq!(held_as(&kept, Some("cave"), "bolt"), (40, vec![]));
}

#[test]
fn a_kept_table_enters_its_area_as_its_records_do() {
    let (a, saved) = cave_of_bolts();
    let mut b = without_the_cave(SaveOptions::new());
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(spawns(&b).is_empty(), "the cave's spawns wait with it");
    // The game spawns the cave, then enters it.
    let bat_1 = b.spawn_prefab("bat", [100.0, 40.0]).unwrap();
    let bat_2 = b.spawn_prefab("bat", [140.0, 40.0]).unwrap();
    let turret = b.spawn_prefab("turret", [200.0, 40.0]).unwrap();
    let hero = b
        .resource::<SaveState>()
        .named_entity(SaveId::authored("hero"))
        .unwrap();
    b.insert(
        bat_1,
        Chase {
            target: hero,
            speed: 2.0,
        },
    );
    b.insert(
        bat_2,
        Chase {
            target: bat_1,
            speed: 1.5,
        },
    );
    name(&mut b, bat_1, Some("cave"), "bat_1").unwrap();
    name(&mut b, bat_2, Some("cave"), "bat_2").unwrap();
    name(&mut b, turret, Some("cave"), "turret").unwrap();
    let report = crate::save::enter_area(&mut b, "cave", &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(spawns(&b).len(), 40);
    let healths = |world: &World| -> Vec<(SaveId, Health)> {
        spawns(world)
            .into_iter()
            .map(|(id, bolt)| (id, world.get::<Health>(bolt).unwrap().clone()))
            .collect()
    };
    assert_eq!(healths(&b), healths(&a));
    assert_eq!(text(&capture_world(&b).unwrap()), text(&saved));
}
