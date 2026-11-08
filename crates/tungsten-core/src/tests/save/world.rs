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
    assert_eq!(
        b.get::<Chase>(lb.bat_2).unwrap().target,
        lb.bat_1,
        "the fresh spawn's"
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

    let mut format = saved;
    format["format"] = json!(2);
    assert_eq!(
        restore_world(&mut b, &from_value(format), &(Scene,)),
        Err(RestoreError::Format { found: 2 })
    );
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
    cave["spawns"][1]["prefab"] = json!("ghost");

    let mut b = world();
    let lb = level(&mut b);
    let report = restore_world(&mut b, &from_value(saved), &(Scene,)).unwrap();
    assert_eq!(
        report.dropped,
        [
            "`bat_1` in area `cave`'s `mana`: not a saved component",
            "`bat_9` in area `cave`: the level no longer has it",
            "spawn 1 (`ghost`): the manifest has no such prefab",
        ]
    );
    assert_eq!(b.get::<Health>(lb.bat_1).unwrap().current, 6.0);
    assert_eq!(spawns(&b).len(), 1, "the known one");
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
