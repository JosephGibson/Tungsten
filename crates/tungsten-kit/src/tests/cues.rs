use super::*;
use serde_json::{Value, json};
use tungsten_core::{ComponentRegistry, PrefabData, PrefabRegistry, SoundData};

use crate::TagsPlugin;

/// A World with the cue and tags plugins built, the tag `team.enemy`, the
/// prefab `puff` and the sound `blip`.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    let mut prefabs = PrefabRegistry::new();
    let puff: PrefabData = serde_json::from_value(json!({ "tag": "puff" })).unwrap();
    prefabs.insert("puff".to_string(), puff);
    world.insert_resource(prefabs);
    let mut sounds = SoundRegistry::new();
    let blip = SoundData {
        samples: vec![0.0; 4],
        sample_rate: 44_100,
        channels: 1,
    };
    sounds.register("blip".to_string(), blip, 1.0, false);
    world.insert_resource(sounds);
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.enemy"])
        .build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn check(world: &World, value: &Value) -> Result<(), String> {
    world
        .resource::<ComponentRegistry>()
        .check_in(CUES, value, world)
        .map_err(|e| e.to_string())
}

fn rules(value: Value) -> Result<CueRules, serde_json::Error> {
    serde_json::from_value(value)
}

/// The `cue_rules` kind's check on `value`.
fn check_rules(world: &mut World, value: Value) -> Result<(), String> {
    let rules = rules(value).map_err(|e| e.to_string())?;
    check_rule_list(&rules, world)
        .map_err(|e| format!("unknown {} `{}` at {}", e.kind, e.id, e.field))
}

/// The rig with health and the damage types `elemental.fire` and
/// `elemental.cold` declared.
fn typed_rig() -> World {
    let (mut world, mut schedule) = rig();
    crate::TagsPlugin::new()
        .with_tags(["elemental.fire", "elemental.cold"])
        .build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(json!({ "elemental.fire": {}, "elemental.cold": {} })).unwrap();
    world.resource_mut::<DataRegistry>().insert("game", types);
    world
}

#[test]
fn a_kind_filter_on_an_event_without_kinds_or_an_unknown_type_fails_the_load() {
    let mut world = typed_rig();
    for good in [
        json!([{ "on": "damaged_part", "kind": { "any": ["elemental"] }, "do": [] }]),
        json!([{ "on": "killed", "kind": { "all": ["elemental.fire"], "none": ["elemental.cold"] }, "do": [] }]),
        json!([{ "on": "damaged", "kind": {}, "do": [] }]),
        json!([{ "on": "hit", "do": [] }]),
    ] {
        assert!(check_rules(&mut world, good.clone()).is_ok(), "{good}");
    }
    let cases = [
        (
            json!([{ "on": "hit", "kind": { "any": ["elemental"] }, "do": [] }]),
            "unknown cue event with damage kinds `hit` at 0.kind",
        ),
        (
            json!([{ "on": "damaged", "do": [] }, { "on": "zone_entered", "kind": { "none": ["elemental"] }, "do": [] }]),
            "unknown cue event with damage kinds `zone_entered` at 1.kind",
        ),
        (
            json!([{ "on": "damaged", "kind": { "any": ["elemental.acid"] }, "do": [] }]),
            "unknown damage type `elemental.acid` at 0.kind.any.0",
        ),
        (
            json!([{ "on": "damaged_part", "kind": { "all": ["elemental", "team"] }, "do": [] }]),
            "unknown damage type `team` at 0.kind.all.1",
        ),
    ];
    for (value, error) in cases {
        assert_eq!(check_rules(&mut world, value).unwrap_err(), error);
    }
    for bad in [
        json!([{ "on": "damaged", "kind": { "any": ["Fire"] }, "do": [] }]),
        json!([{ "on": "damaged", "kind": { "some": ["fire"] }, "do": [] }]),
    ] {
        assert!(rules(bad.clone()).is_err(), "{bad}");
    }
    let rule: CueRuleDef = serde_json::from_value(
        json!({ "on": "killed", "kind": { "any": ["elemental"] }, "do": [] }),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(rule).unwrap()["kind"],
        json!({ "any": ["elemental"] })
    );
}

#[test]
fn a_style_less_label_is_refused_outside_damaged_part() {
    let (mut world, mut schedule) = rig();
    assert!(
        check(
            &world,
            &json!({ "damaged_part": [{ "label": {} }, { "label": { "font": "sans" } }] })
        )
        .is_ok()
    );
    assert_eq!(
        check(&world, &json!({ "damaged": [{ "label": {} }] })).unwrap_err(),
        "component `cues`, field `damaged.0.label`: unknown cue event with a damage type's label style `damaged`"
    );
    assert_eq!(
        check(&world, &json!({ "zone_entered": [{ "label": {} }] })).unwrap_err(),
        "component `cues`, field `zone_entered.0.label`: unknown cue event with an amount `zone_entered`"
    );
    assert_eq!(
        check_rules(
            &mut world,
            json!([{ "on": "killed", "do": [{ "label": {} }] }])
        )
        .unwrap_err(),
        "unknown cue event with a damage type's label style `killed` at 0.do.0.label"
    );
    CuesOnEvent::<Looted>::new("looted", looted).build(&mut schedule, &mut world);
    assert!(check(&world, &json!({ "looted": [{ "label": {} }] })).is_err());
    assert_eq!(
        serde_json::to_value(CueAction::type_label()).unwrap(),
        json!({ "label": {} })
    );
    assert_eq!(
        serde_json::from_value::<CueAction>(json!({ "label": {} })).unwrap(),
        CueAction::type_label()
    );
    assert_eq!(CueAction::type_label().name(), "label");
    assert!(serde_json::from_value::<CueAction>(json!({ "label": { "size": 20 } })).is_err());
}

#[test]
fn each_action_reads_and_writes_its_data_form() {
    for (action, data) in [
        (CueAction::prefab("puff"), json!({ "prefab": "puff" })),
        (
            CueAction::counter_add("kills", 1).for_game(),
            json!({ "counter": { "name": "kills", "add": 1, "target": "game" } }),
        ),
        (
            CueAction::counter_set("combo", 0),
            json!({ "counter": { "name": "combo", "set": 0, "target": "self" } }),
        ),
        (
            CueAction::flag("has_key", true).for_other(),
            json!({ "flag": { "name": "has_key", "set": true, "target": "other" } }),
        ),
        (
            CueAction::despawn(),
            json!({ "despawn": { "target": "self" } }),
        ),
        (
            CueAction::despawn().for_other(),
            json!({ "despawn": { "target": "other" } }),
        ),
        (
            CueAction::sound("blip").with_min_interval(0.25),
            json!({ "sound": { "id": "blip", "min_interval": 0.25 } }),
        ),
        (CueAction::shake(0.5), json!({ "shake": 0.5 })),
        (
            CueAction::squash(SquashTrigger::OnLand),
            json!({ "squash": "on_land" }),
        ),
        (CueAction::flash(), json!({ "flash": {} })),
        (
            CueAction::roll(DataRef::by_id("drops")),
            json!({ "roll": "drops" }),
        ),
        (
            CueAction::roll(DataRef::inline(
                WeightedTable::new()
                    .with_entry("puff", 2.0)
                    .with_nothing(1.0),
            )),
            json!({ "roll": { "rolls": 1, "entries": [{ "prefab": "puff", "weight": 2.0 }, { "weight": 1.0 }] } }),
        ),
    ] {
        assert_eq!(serde_json::to_value(&action).unwrap(), data, "{action:?}");
        assert_eq!(serde_json::from_value::<CueAction>(data).unwrap(), action);
    }
    let label: CueAction = serde_json::from_value(json!({ "label": { "font": "sans" } })).unwrap();
    assert_eq!(label, CueAction::label(LabelStyle::new("sans")));
    let short: CueAction = serde_json::from_value(json!({ "despawn": {} })).unwrap();
    assert_eq!(short, CueAction::despawn());
    let names: Vec<&str> = [
        CueAction::prefab("p"),
        CueAction::counter_add("c", 1),
        CueAction::flag("f", true),
        CueAction::despawn(),
        CueAction::sound("s"),
        CueAction::shake(0.0),
        CueAction::squash(SquashTrigger::Manual),
        CueAction::flash(),
        CueAction::label(LabelStyle::new("sans")),
        CueAction::roll(DataRef::by_id("drops")),
    ]
    .iter()
    .map(CueAction::name)
    .collect();
    assert_eq!(
        names,
        [
            "prefab", "counter", "flag", "despawn", "sound", "shake", "squash", "flash", "label",
            "roll"
        ]
    );
}

#[test]
fn the_data_form_refuses_what_the_constructors_refuse() {
    for bad in [
        json!({ "flash": {}, "shake": 1 }),
        json!({}),
        json!({ "explode": {} }),
        json!({ "counter": { "name": "kills" } }),
        json!({ "counter": { "name": "kills", "add": 1, "set": 2 } }),
        json!({ "counter": { "name": "Kills", "add": 1 } }),
        json!({ "counter": { "name": "kills", "add": 0.5 } }),
        json!({ "flag": { "name": "a b" } }),
        json!({ "despawn": { "target": "game" } }),
        json!({ "sound": { "id": "blip", "min_interval": -1 } }),
        json!({ "shake": -0.1 }),
        json!({ "squash": "on_jump" }),
        json!({ "flash": { "color": [1, 1, 1, 1] } }),
        json!({ "label": { "font": "" } }),
        json!({ "label": { "font": "sans", "size": 0 } }),
        json!({ "roll": 3 }),
        json!({ "roll": { "rolls": 0, "entries": [] } }),
    ] {
        assert!(
            serde_json::from_value::<CueAction>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
#[should_panic(expected = "a `prefab` action takes no target")]
fn a_prefab_for_the_other_panics() {
    let _ = CueAction::prefab("puff").for_other();
}

#[test]
#[should_panic(expected = "a `despawn` action takes no target")]
fn a_despawn_for_the_game_panics() {
    let _ = CueAction::despawn().for_game();
}

#[test]
#[should_panic(expected = "a `flash` action plays no sound")]
fn an_interval_on_a_flash_panics() {
    let _ = CueAction::flash().with_min_interval(1.0);
}

#[test]
#[should_panic(expected = "trauma must be finite and >= 0")]
fn a_negative_shake_panics() {
    let _ = CueAction::shake(-1.0);
}

#[test]
fn cues_merge_their_lists_and_read_none_for_an_absent_event() {
    let cues = Cues::new()
        .on("damaged", [CueAction::flash()])
        .on("damaged", [CueAction::shake(0.1)]);
    assert_eq!(cues.actions("damaged").len(), 2);
    assert!(cues.actions("killed").is_empty());
    assert!(serde_json::from_value::<Cues>(json!({ "Damaged": [] })).is_err());
}

#[test]
fn the_load_checks_events_ids_amounts_and_others() {
    let (world, _) = rig();
    assert!(
        check(
            &world,
            &json!({ "damaged": [{ "flash": {} }, { "label": { "font": "sans" } }] })
        )
        .is_ok()
    );
    assert!(
        check(
            &world,
            &json!({ "killed": [{ "prefab": "puff" }, { "sound": { "id": "blip" } }] })
        )
        .is_ok()
    );
    assert!(
        check(
            &world,
            &json!({ "zone_entered": [{ "flag": { "name": "has_key", "target": "other" } }, { "despawn": {} }] })
        )
        .is_ok()
    );
    assert!(
        check(
            &world,
            &json!({
                "damaged": [{ "counter": { "name": "hits", "add": 1, "target": "other" } }],
                "hit": [{ "label": { "font": "sans" } }, { "despawn": { "target": "other" } }],
                "healed": [{ "label": { "font": "sans" } }],
                "picked_up": [{ "counter": { "name": "gems", "add": 1, "target": "other" } }],
                "expired": [{ "prefab": "puff" }, { "roll": { "entries": [{ "prefab": "puff", "weight": 1 }] } }]
            })
        )
        .is_ok(),
        "C's events and the source as the hit's other"
    );
    let cases = [
        (
            json!({ "dmaged": [] }),
            "component `cues`, field `dmaged`: unknown cue event `dmaged`",
        ),
        (
            json!({ "killed": [{ "prefab": "puf" }] }),
            "component `cues`, field `killed.0.prefab`: unknown prefab `puf`",
        ),
        (
            json!({ "damaged": [{ "flash": {} }, { "sound": { "id": "blop" } }] }),
            "component `cues`, field `damaged.1.sound.id`: unknown sound `blop`",
        ),
        (
            json!({ "zone_entered": [{ "label": { "font": "sans" } }] }),
            "component `cues`, field `zone_entered.0.label`: unknown cue event with an amount `zone_entered`",
        ),
        (
            json!({ "cooldown_elapsed": [{ "despawn": { "target": "other" } }] }),
            "component `cues`, field `cooldown_elapsed.0.despawn.target`: unknown cue event with an other entity `cooldown_elapsed`",
        ),
        (
            json!({ "killed": [{ "roll": "chest" }] }),
            "component `cues`, field `killed.0.roll`: unknown loot_tables `chest`",
        ),
        (
            json!({ "killed": [{ "flash": {} }, { "roll": { "entries": [{ "weight": 1 }, { "prefab": "ruby", "weight": 1 }] } }] }),
            "component `cues`, field `killed.1.roll.entries.1.prefab`: unknown prefab `ruby`",
        ),
        (
            json!({ "expired": [{ "label": { "font": "sans" } }] }),
            "component `cues`, field `expired.0.label`: unknown cue event with an amount `expired`",
        ),
    ];
    for (value, error) in cases {
        assert_eq!(check(&world, &value).unwrap_err(), error);
    }
}

#[test]
fn a_rule_checks_its_event_its_filters_tags_and_its_actions() {
    let (mut world, _) = rig();
    assert!(
        check_rules(
            &mut world,
            json!([{ "on": "killed", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "kills", "add": 1, "target": "game" } }] }])
        )
        .is_ok()
    );
    assert_eq!(
        check_rules(&mut world, json!([{ "on": "spawned", "do": [] }])).unwrap_err(),
        "unknown cue event `spawned` at 0.on"
    );
    assert_eq!(
        check_rules(
            &mut world,
            json!([{ "on": "killed", "do": [] }, { "on": "killed", "filter": { "none": ["team.foe"] }, "do": [] }])
        )
        .unwrap_err(),
        "unknown tag `team.foe` at 1.filter.none.0"
    );
    assert_eq!(
        check_rules(
            &mut world,
            json!([{ "on": "damaged", "do": [{ "flash": {} }, { "prefab": "smoke" }] }])
        )
        .unwrap_err(),
        "unknown prefab `smoke` at 0.do.1.prefab"
    );
    for bad in [
        json!([{ "on": "killed" }]),
        json!([{ "on": "Killed", "do": [] }]),
        json!([{ "on": "killed", "do": [], "when": "always" }]),
    ] {
        assert!(rules(bad.clone()).is_err(), "{bad}");
    }
}

/// The game's loot: a chest opened by someone.
struct Looted {
    chest: Entity,
    by: Entity,
}

/// A chest that loots itself is no loot.
fn looted(event: &Looted) -> Option<CueEvent> {
    (event.chest != event.by).then(|| CueEvent::new(event.chest, Vec2::ZERO).with_other(event.by))
}

#[test]
fn a_game_event_registers_its_queue_and_its_name_for_cues() {
    let (mut world, mut schedule) = rig();
    CuesOnEvent::<Looted>::new("looted", looted).build(&mut schedule, &mut world);
    assert!(world.has_event::<Looted>());
    assert!(
        check(
            &world,
            &json!({ "looted": [{ "despawn": {} }, { "counter": { "name": "coins", "add": 1, "target": "other" } }, { "label": { "font": "sans" } }] })
        )
        .is_ok(),
        "a game event may carry an amount and an other"
    );
    let plugin = CuesOnEvent::<Looted>::new("looted", looted);
    assert_eq!(
        format!("{plugin:?}"),
        r#"CuesOnEvent { name: "looted", .. }"#
    );
    let _ = plugin.clone();
}

#[test]
#[should_panic(expected = "is one of the kit's cue events")]
fn a_game_event_named_as_the_kits_panics() {
    let (mut world, mut schedule) = rig();
    CuesOnEvent::<Looted>::new("killed", looted).build(&mut schedule, &mut world);
}

#[test]
#[should_panic(expected = "is one of the kit's cue events")]
fn a_game_event_named_as_one_of_cs_panics() {
    let (mut world, mut schedule) = rig();
    CuesOnEvent::<Looted>::new("picked_up", looted).build(&mut schedule, &mut world);
}

#[test]
#[should_panic(expected = "is already registered")]
fn a_game_event_registered_twice_panics() {
    let (mut world, mut schedule) = rig();
    CuesOnEvent::<Looted>::new("looted", looted).build(&mut schedule, &mut world);
    CuesOnEvent::<Looted>::new("looted", looted).build(&mut schedule, &mut world);
}

#[test]
#[should_panic(expected = "is not an event name")]
fn a_game_event_with_a_bad_name_panics() {
    let (mut world, mut schedule) = rig();
    CuesOnEvent::<Looted>::new("Picked Up", looted).build(&mut schedule, &mut world);
}

mod running {
    use super::*;
    use std::path::PathBuf;

    use tungsten_core::{
        AudioCommand, MaterialRegistry, MaterialUniformDefaults, Sprite, SpriteAssetId, Stage,
    };

    use crate::foundation::trace::KitTrace;
    use crate::{CountersPlugin, HIT_FLASH_MATERIAL, Health, HealthPlugin, HitFlash, Tags, damage};

    /// A World running the cues with health, counters, a command buffer, a
    /// trace, the shake and squash queues, audio, the prefab `puff`, the
    /// sound `blip` and the flash material.
    fn rig() -> (World, Schedule) {
        let (mut world, _) = super::rig();
        world.insert_resource(Time::new());
        world.insert_resource(CommandBuffer::new());
        world.insert_resource(KitTrace::default());
        world.insert_resource(AudioCommands::new());
        world.register_event::<ShakeEvent>();
        world.register_event::<SquashEvent>();
        world.register_event::<ZoneEntered>();
        let mut materials = MaterialRegistry::new();
        materials.allocate(
            HIT_FLASH_MATERIAL,
            PathBuf::from("assets/manifest.json"),
            "damage_flash".to_string(),
            MaterialUniformDefaults::default(),
        );
        world.insert_resource(materials);
        let mut schedule = Schedule::new();
        crate::TagsPlugin::new()
            .with_tags(["team.enemy"])
            .build(&mut schedule, &mut world);
        HealthPlugin.build(&mut schedule, &mut world);
        CountersPlugin.build(&mut schedule, &mut world);
        CuesPlugin.build(&mut schedule, &mut world);
        schedule.resolve().unwrap();
        (world, schedule)
    }

    /// One fixed step of 1/60 s, without the command flush.
    fn step(world: &mut World, schedule: &mut Schedule) {
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        world.resource_mut::<Time>().enter_fixed_step();
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
        world.resource_mut::<Time>().leave_fixed_steps();
    }

    /// The command flush and the event flush that end a frame.
    fn flush(world: &mut World) {
        let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
        world.flush_reusing(&mut commands);
        world.insert_resource(commands);
        world.flush_events();
    }

    fn insert_rules(world: &mut World, id: &str, value: Value) {
        let rules: CueRules = serde_json::from_value(value).unwrap();
        world.resource_mut::<DataRegistry>().insert(id, rules);
    }

    fn game(world: &World, name: &str) -> i64 {
        world.resource::<Counters>().get(name)
    }

    /// The trace's kinds about `entity`, as lines without their head.
    fn traced(world: &World, entity: Entity) -> Vec<String> {
        world
            .resource::<KitTrace>()
            .about(entity)
            .map(|entry| {
                let line = entry.to_string();
                let head = format!("{} ", entry.entity);
                line.split_once(&head)
                    .map(|(_, rest)| rest.to_owned())
                    .unwrap()
            })
            .collect()
    }

    #[test]
    fn a_hit_runs_the_subjects_cues_then_the_rules_by_entry_id() {
        let (mut world, mut schedule) = rig();
        insert_rules(
            &mut world,
            "b",
            json!([{ "on": "damaged", "do": [{ "counter": { "name": "order", "add": 100, "target": "game" } }] }]),
        );
        insert_rules(
            &mut world,
            "a",
            json!([
                { "on": "damaged", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "order", "add": 10, "target": "game" } }] },
                { "on": "damaged", "filter": { "none": ["team.enemy"] }, "do": [{ "counter": { "name": "order", "add": 1000, "target": "game" } }] }
            ]),
        );
        let slime = world.spawn_with((
            Health::new(5.0),
            Tags::new(["team.enemy"]),
            Cues::new().on("damaged", [CueAction::counter_set("order", 1).for_game()]),
        ));
        damage(&mut world, slime, 1.0);
        step(&mut world, &mut schedule);
        assert_eq!(game(&world, "order"), 111, "set 1, then rule a.0, then b.0");
        let rules: Vec<String> = traced(&world, slime)
            .into_iter()
            .filter(|line| line.contains("counter"))
            .collect();
        assert_eq!(
            rules,
            [
                "damaged: counter (own cue)",
                "damaged: counter (rule a.0)",
                "damaged: counter (rule b.0)"
            ]
        );
        step(&mut world, &mut schedule);
        assert_eq!(game(&world, "order"), 111, "each event once");
    }

    #[test]
    fn a_kill_runs_after_its_hit_and_despawns_queue_after_spawns() {
        let (mut world, mut schedule) = rig();
        let slime = world.spawn_with((
            Health::new(1.0),
            Transform::from_position(Vec2::new(30.0, 40.0)),
            Cues::new()
                .on("damaged", [CueAction::counter_add("hits", 1).for_game()])
                .on(
                    "killed",
                    [
                        CueAction::despawn(),
                        CueAction::counter_set("hits", 7).for_game(),
                        CueAction::prefab("puff"),
                    ],
                ),
        ));
        damage(&mut world, slime, 5.0);
        step(&mut world, &mut schedule);
        assert_eq!(game(&world, "hits"), 7, "the kill's cues after the hit's");
        flush(&mut world);
        assert!(!world.is_alive(slime));
        let puffs: Vec<Vec2> = world
            .query::<(&tungsten_core::Tag, &Transform)>()
            .map(|(_, transform)| transform.position)
            .collect();
        assert_eq!(
            puffs,
            [Vec2::new(30.0, 40.0)],
            "spawned before its source went"
        );
    }

    #[test]
    fn a_despawning_subject_reacts_no_more_in_the_frames_next_step() {
        let (mut world, mut schedule) = rig();
        let slime = world.spawn_with((
            Health::new(5.0),
            Cues::new().on(
                "damaged",
                [
                    CueAction::counter_add("hits", 1).for_game(),
                    CueAction::despawn(),
                ],
            ),
        ));
        damage(&mut world, slime, 1.0);
        step(&mut world, &mut schedule);
        assert_eq!(
            damage(&mut world, slime, 1.0),
            crate::Hit::Ignored,
            "the pipeline ignores a despawning target"
        );
        let hero = world.spawn();
        world
            .resource_mut::<EventQueue<ZoneEntered>>()
            .send(ZoneEntered::new(slime, hero, Vec2::ZERO));
        step(&mut world, &mut schedule);
        assert_eq!(game(&world, "hits"), 1);
        assert!(
            traced(&world, slime)
                .contains(&"zone_entered: all skipped, the subject is despawning".to_string())
        );
        let despawns = traced(&world, slime)
            .iter()
            .filter(|line| line.starts_with("despawned"))
            .count();
        assert_eq!(despawns, 1);
        flush(&mut world);
        assert!(!world.is_alive(slime));
    }

    #[test]
    fn a_zone_entry_reaches_the_zone_the_entrant_and_the_game() {
        let (mut world, mut schedule) = rig();
        let hero = world.spawn();
        let key = world.spawn_with((Cues::new().on(
            "zone_entered",
            [
                CueAction::flag("has_key", true).for_other(),
                CueAction::counter_add("keys", 1).for_game(),
                CueAction::flag("taken", true),
                CueAction::despawn(),
            ],
        ),));
        world
            .resource_mut::<EventQueue<ZoneEntered>>()
            .send(ZoneEntered::new(key, hero, Vec2::ZERO));
        step(&mut world, &mut schedule);
        assert!(
            world.get::<Flags>(hero).unwrap().has("has_key"),
            "inserted on the entrant"
        );
        assert!(world.get::<Flags>(key).unwrap().has("taken"));
        assert_eq!(game(&world, "keys"), 1);
        flush(&mut world);
        assert!(!world.is_alive(key) && world.is_alive(hero));
    }

    /// A game event with no amount and no other entity.
    struct Rang(Entity);

    #[allow(clippy::unnecessary_wraps)]
    fn rang(event: &Rang) -> Option<CueEvent> {
        Some(CueEvent::new(event.0, Vec2::ZERO))
    }

    #[test]
    fn an_action_missing_its_amount_or_other_is_skipped_and_traced() {
        let (mut world, mut schedule) = rig();
        CuesOnEvent::<Rang>::new("rang", rang).build(&mut schedule, &mut world);
        schedule.resolve().unwrap();
        let bell = world.spawn_with((Cues::new().on(
            "rang",
            [
                CueAction::label(LabelStyle::new("sans")),
                CueAction::counter_add("rings", 1).for_other(),
                CueAction::counter_add("rings", 1),
            ],
        ),));
        world.resource_mut::<EventQueue<Rang>>().send(Rang(bell));
        step(&mut world, &mut schedule);
        assert_eq!(world.get::<Counters>(bell).unwrap().get("rings"), 1);
        assert_eq!(
            traced(&world, bell),
            [
                "rang: label skipped, the event has no amount",
                "rang: counter skipped, the event has no other entity, or it is gone",
                "rang: counter (own cue)"
            ]
        );
    }

    #[test]
    fn a_cue_sound_waits_its_min_interval_on_the_game_clock() {
        let (mut world, mut schedule) = rig();
        let brute = world.spawn_with((
            Health::new(1000.0),
            Cues::new().on("damaged", [CueAction::sound("blip").with_min_interval(0.1)]),
        ));
        let mut played = Vec::new();
        for i in 0..13 {
            damage(&mut world, brute, 1.0);
            step(&mut world, &mut schedule);
            let commands = world.resource_mut::<AudioCommands>().drain();
            if commands
                .iter()
                .any(|command| matches!(command, AudioCommand::Play { volume, looping: false, .. } if *volume == 1.0))
            {
                played.push(i);
            }
            flush(&mut world);
        }
        assert_eq!(played, [0, 6, 12], "a hit every step, a sound every 0.1 s");
    }

    #[test]
    fn presentation_actions_shake_squash_flash_and_float_the_amount() {
        let (mut world, mut schedule) = rig();
        let slime = world.spawn_with((
            Health::new(5.0),
            HitFlash::new([1.0; 4], 0.2),
            Sprite::new(SpriteAssetId::new(0)),
            Transform::from_position(Vec2::new(5.0, 6.0)),
            Cues::new().on(
                "damaged",
                [
                    CueAction::shake(0.3),
                    CueAction::squash(SquashTrigger::OnHit),
                    CueAction::flash(),
                    CueAction::label(LabelStyle::new("sans")),
                ],
            ),
        ));
        damage(&mut world, slime, 2.0);
        step(&mut world, &mut schedule);
        let shakes: Vec<f32> = world
            .resource::<EventQueue<ShakeEvent>>()
            .iter()
            .map(|event| event.trauma_add)
            .collect();
        assert_eq!(shakes, [0.3]);
        let squashes: Vec<SquashEvent> = world
            .resource::<EventQueue<SquashEvent>>()
            .iter()
            .copied()
            .collect();
        assert_eq!(
            squashes,
            [SquashEvent {
                entity: slime,
                trigger: SquashTrigger::OnHit
            }]
        );
        assert!(world.get::<HitFlash>(slime).unwrap().is_active());
        assert_eq!(world.resource::<LiveLabels>().len(), 1);
    }

    /// The positions of the World's tagged entities, with their sources.
    fn spawned(world: &World) -> Vec<(String, Vec2, Option<Entity>)> {
        let mut out: Vec<(String, Vec2, Option<Entity>)> = world
            .query::<(Entity, &tungsten_core::Tag, &Transform)>()
            .map(|(entity, tag, transform)| {
                (
                    tag.name.clone(),
                    transform.position,
                    world
                        .get::<crate::Source>(entity)
                        .map(|source| source.entity()),
                )
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    #[test]
    fn a_roll_draws_its_rolls_and_spawns_each_from_the_subject_at_its_point() {
        let (mut world, mut schedule) = rig();
        crate::SourcePlugin.build(&mut schedule, &mut world);
        let drops = WeightedTable::new().with_entry("puff", 1.0).with_rolls(3);
        world.resource_mut::<DataRegistry>().insert("drops", drops);
        let chest = world.spawn_with((
            Health::new(1.0),
            Transform::from_position(Vec2::new(12.0, -3.0)),
            Cues::new().on("killed", [CueAction::roll(DataRef::by_id("drops"))]),
        ));
        let draws = world.resource::<KitRng>().draws();
        damage(&mut world, chest, 1.0);
        step(&mut world, &mut schedule);
        flush(&mut world);
        assert_eq!(
            world.resource::<KitRng>().draws(),
            draws + 3,
            "one number a roll"
        );
        assert_eq!(
            spawned(&world),
            vec![("puff".to_string(), Vec2::new(12.0, -3.0), Some(chest)); 3]
        );
        let rolls: Vec<String> = traced(&world, chest)
            .into_iter()
            .filter(|line| line.contains("roll") || line.contains("spawned"))
            .collect();
        assert_eq!(
            rolls,
            [
                "spawned `puff` by cues",
                "spawned `puff` by cues",
                "spawned `puff` by cues",
                "killed: roll (own cue)"
            ]
        );
    }

    #[test]
    fn a_roll_of_a_missing_table_is_skipped_and_traced() {
        let (mut world, mut schedule) = rig();
        let chest = world.spawn_with((
            Health::new(1.0),
            Cues::new().on("killed", [CueAction::roll(DataRef::by_id("nowhere"))]),
        ));
        damage(&mut world, chest, 1.0);
        step(&mut world, &mut schedule);
        assert!(
            traced(&world, chest)
                .contains(&"killed: roll skipped, the World has no such loot table".to_string())
        );
    }

    #[test]
    fn cs_events_reach_their_subjects_cues_with_their_others() {
        let (mut world, mut schedule) = rig();
        for register in [
            World::register_event::<crate::Struck>,
            World::register_event::<crate::PickedUp>,
            World::register_event::<crate::ProjectileExpired>,
        ] {
            let _ = register(&mut world);
        }
        let hero = world.spawn();
        let gem = world.spawn();
        let bolt = world.spawn();
        let blade = world.spawn();
        let knight = world.spawn_with((Health::new(10.0),));
        let count = |name: &str| CueAction::counter_add(name, 1).for_other();
        world.insert(blade, Cues::new().on("hit", [count("struck")]));
        world.insert(gem, Cues::new().on("picked_up", [count("gems")]));
        world.insert(
            bolt,
            Cues::new().on("expired", [CueAction::counter_add("expired", 1).for_game()]),
        );
        world.insert(
            knight,
            Cues::new()
                .on("damaged", [count("hits")])
                .on("healed", [count("heals")]),
        );
        world
            .resource_mut::<EventQueue<crate::Struck>>()
            .send(crate::Struck::new(blade, hero, Vec2::ZERO, 2.0));
        world
            .resource_mut::<EventQueue<crate::PickedUp>>()
            .send(crate::PickedUp::new(gem, hero, Vec2::ZERO));
        world
            .resource_mut::<EventQueue<crate::ProjectileExpired>>()
            .send(crate::ProjectileExpired::new(bolt, Vec2::ZERO));
        crate::DamageRequest::new(knight, 2.0)
            .with_source(hero)
            .apply(&mut world);
        crate::HealRequest::new(knight, 1.0)
            .with_source(hero)
            .apply(&mut world);
        step(&mut world, &mut schedule);
        let counters = world.get::<Counters>(hero).unwrap();
        for name in ["struck", "gems", "hits", "heals"] {
            assert_eq!(counters.get(name), 1, "{name}");
        }
        assert_eq!(game(&world, "expired"), 1);
    }

    #[test]
    fn a_prefab_that_does_not_prepare_spawns_nothing_and_traces_why() {
        let (mut world, mut schedule) = rig();
        let slime = world.spawn_with((
            Health::new(5.0),
            Cues::new().on("damaged", [CueAction::prefab("ruby")]),
        ));
        damage(&mut world, slime, 1.0);
        step(&mut world, &mut schedule);
        flush(&mut world);
        assert_eq!(world.query::<&tungsten_core::Tag>().count(), 0);
        assert!(
            traced(&world, slime)
                .iter()
                .any(|line| line.starts_with("`ruby` by cues spawned nothing"))
        );
    }
}

/// A World running the cues with health, counters, a command buffer, a
/// trace, the frame's labels, the prefabs `puff` and `smoke`, and the
/// damage types `entry` defines, each declared as a tag.
fn typed_running(entry: Value) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(crate::foundation::trace::KitTrace::default());
    world.insert_resource(tungsten_core::WorldLabels::default());
    let mut prefabs = PrefabRegistry::new();
    for name in ["puff", "smoke"] {
        let prefab: PrefabData = serde_json::from_value(json!({ "tag": name })).unwrap();
        prefabs.insert(name.to_string(), prefab);
    }
    world.insert_resource(prefabs);
    let mut schedule = Schedule::new();
    let declared: Vec<String> = entry.as_object().unwrap().keys().cloned().collect();
    crate::TagsPlugin::new()
        .with_tags(declared)
        .build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    crate::CountersPlugin.build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    define(&mut world, entry);
    (world, schedule)
}

/// Puts `entry` in place as the `damage_types` entry `game`, as a reload
/// replaces it.
fn define(world: &mut World, entry: Value) {
    let types: crate::damage::kinds::DamageTypes = serde_json::from_value(entry).unwrap();
    world.resource_mut::<DataRegistry>().insert("game", types);
}

/// One fixed step of 1/60 s, its spawns flushed.
fn typed_step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    world.resource_mut::<Time>().enter_fixed_step();
    schedule.run_stage(tungsten_core::Stage::FixedUpdate, world, |_, _| {});
    world.resource_mut::<Time>().leave_fixed_steps();
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
}

fn hit(world: &mut World, target: Entity, parts: &[(f32, &[&str])]) -> crate::Hit {
    let parts = parts.iter().map(|&(amount, names)| {
        crate::DamagePart::new(amount).with_tags(Tags::new(names.iter().copied()))
    });
    crate::DamageRequest::from_parts(target, parts).apply(world)
}

fn game_count(world: &World, name: &str) -> i64 {
    world
        .get_resource::<Counters>()
        .map_or(0, |counters| counters.get(name))
}

fn insert_rule_list(world: &mut World, id: &str, value: Value) {
    let rules: CueRules = serde_json::from_value(value).unwrap();
    world.resource_mut::<DataRegistry>().insert(id, rules);
}

#[test]
fn damaged_part_runs_before_damaged_in_each_pass_under_kit_rng() {
    let (mut world, mut schedule) = typed_running(json!({ "fire": {}, "ice": {} }));
    let part_table = WeightedTable::new()
        .with_entry("puff", 1.0)
        .with_entry("smoke", 1.0);
    let hit_table = WeightedTable::new()
        .with_entry("smoke", 3.0)
        .with_entry("puff", 1.0);
    let part_roll = || CueAction::roll(DataRef::inline(part_table.clone()));
    let hit_roll = || CueAction::roll(DataRef::inline(hit_table.clone()));
    insert_rule_list(
        &mut world,
        "r",
        json!([
            { "on": "damaged_part", "do": [{ "roll": { "entries": [{ "prefab": "puff", "weight": 1 }, { "prefab": "smoke", "weight": 1 }] } }] },
            { "on": "damaged", "do": [{ "roll": { "entries": [{ "prefab": "smoke", "weight": 3 }, { "prefab": "puff", "weight": 1 }] } }] }
        ]),
    );
    let golem = world.spawn_with((
        crate::Health::new(100.0),
        Cues::new()
            .on("damaged_part", [part_roll()])
            .on("damaged", [hit_roll()]),
    ));
    hit(&mut world, golem, &[(1.0, &["fire"]), (1.0, &["ice"])]);
    hit(&mut world, golem, &[(1.0, &["ice"]), (1.0, &["fire"])]);
    let mut replay = world.resource::<KitRng>().clone();
    typed_step(&mut world, &mut schedule);
    let trace = world.resource::<crate::foundation::trace::KitTrace>();
    let cues: Vec<String> = trace
        .about(golem)
        .filter_map(|entry| match &entry.kind {
            TraceKind::Cue { event, rule, .. } => {
                Some(format!("{event} {}", rule.as_deref().unwrap_or("own")))
            }
            _ => None,
        })
        .collect();
    let mut expected = vec!["damaged_part own".to_string(); 4];
    expected.extend(vec!["damaged own".to_string(); 2]);
    expected.extend(vec!["damaged_part r.0".to_string(); 4]);
    expected.extend(vec!["damaged r.1".to_string(); 2]);
    assert_eq!(cues, expected, "each pass runs every part before any total");
    let spawned: Vec<String> = trace
        .about(golem)
        .filter_map(|entry| match &entry.kind {
            TraceKind::Spawn { prefab, .. } => Some(prefab.clone()),
            _ => None,
        })
        .collect();
    let order = [&part_table; 4]
        .into_iter()
        .chain([&hit_table; 2])
        .chain([&part_table; 4])
        .chain([&hit_table; 2]);
    let drawn: Vec<String> = order
        .map(|table| table.draw(&mut replay).unwrap().to_owned())
        .collect();
    assert_eq!(spawned, drawn, "the rolls drew in that order");
}

#[test]
fn a_rules_kind_filter_matches_by_type_and_ancestor() {
    let (mut world, mut schedule) =
        typed_running(json!({ "elemental.fire": {}, "elemental.cold": {}, "physical": {} }));
    let count = |name: &str| json!([{ "counter": { "name": name, "add": 1, "target": "game" } }]);
    insert_rule_list(
        &mut world,
        "kinds",
        json!([
            { "on": "damaged_part", "kind": { "any": ["elemental"] }, "do": count("elemental") },
            { "on": "damaged_part", "kind": { "all": ["elemental.fire"] }, "do": count("fire") },
            { "on": "damaged_part", "kind": { "none": ["elemental"] }, "do": count("not_elemental") },
            { "on": "damaged_part", "kind": { "any": ["physical"] }, "do": count("physical") },
            { "on": "damaged", "kind": { "all": ["elemental.fire", "physical"] }, "do": count("mixed") },
            { "on": "killed", "kind": { "any": ["elemental.cold"] }, "do": count("cold_kill") }
        ]),
    );
    let golem = world.spawn_with((crate::Health::new(100.0),));
    let wisp = world.spawn_with((crate::Health::new(1.0),));
    hit(
        &mut world,
        golem,
        &[(1.0, &["elemental.fire"]), (1.0, &["physical"])],
    );
    crate::damage(&mut world, golem, 1.0);
    assert_eq!(
        hit(&mut world, wisp, &[(5.0, &["elemental.cold"])]),
        crate::Hit::Killed
    );
    typed_step(&mut world, &mut schedule);
    let counts: Vec<(&str, i64)> = [
        "elemental",
        "fire",
        "not_elemental",
        "physical",
        "mixed",
        "cold_kill",
    ]
    .into_iter()
    .map(|name| (name, game_count(&world, name)))
    .collect();
    assert_eq!(
        counts,
        [
            ("elemental", 2),
            ("fire", 1),
            ("not_elemental", 2),
            ("physical", 1),
            ("mixed", 1),
            ("cold_kill", 1)
        ],
        "fire and cold under `elemental`; the untyped and the physical part pass `none`; the total and the kill read the hit's kinds"
    );
}

#[test]
fn a_damaged_part_label_takes_its_types_style_unless_it_names_one() {
    let style = |color: [f32; 4]| json!({ "label_style": { "font": "sans", "color": color } });
    let entry = |fire: [f32; 4]| {
        json!({
            "elemental": style([1.0, 1.0, 0.0, 1.0]),
            "elemental.fire": style(fire),
            "elemental.cold": {},
            "tie.b": style([0.0, 1.0, 0.0, 1.0]),
            "tie.a": style([1.0, 0.0, 1.0, 1.0]),
            "plain": {}
        })
    };
    let (mut world, mut schedule) = typed_running(entry([1.0, 0.0, 0.0, 1.0]));
    let golem = world.spawn_with((
        crate::Health::new(1000.0),
        Cues::new().on("damaged_part", [CueAction::type_label()]),
    ));
    let named = world.spawn_with((
        crate::Health::new(1000.0),
        Cues::new().on("damaged_part", [CueAction::label(LabelStyle::new("mono"))]),
    ));
    let drawn = |world: &mut World, schedule: &mut Schedule| -> Vec<(String, String, [u8; 4])> {
        world.resource_mut::<Time>().advance_frame(0.0);
        schedule.run_stage(tungsten_core::Stage::PostUpdate, world, |_, _| {});
        let mut labels: Vec<(String, String, [u8; 4])> = world
            .resource_mut::<tungsten_core::WorldLabels>()
            .drain()
            .map(|label| (label.text, label.font_id, label.color))
            .collect();
        labels.sort();
        labels
    };
    hit(
        &mut world,
        golem,
        &[
            (1.0, &["elemental.fire"]),
            (2.0, &["elemental.cold"]),
            (3.0, &["tie.b", "tie.a"]),
        ],
    );
    hit(&mut world, named, &[(4.0, &["elemental.fire"])]);
    typed_step(&mut world, &mut schedule);
    assert_eq!(
        drawn(&mut world, &mut schedule),
        [
            ("1".to_string(), "sans".to_string(), [255, 0, 0, 255]),
            ("2".to_string(), "sans".to_string(), [255, 255, 0, 255]),
            ("3".to_string(), "sans".to_string(), [255, 0, 255, 255]),
            ("4".to_string(), "mono".to_string(), [255, 255, 255, 255]),
        ],
        "its own style, its ancestor's, the first of a tie, and a named style as it is"
    );

    // A reload restyles; a part whose types set no style, or untyped,
    // skips its label and the trace says why.
    define(&mut world, entry([0.0, 0.0, 1.0, 1.0]));
    world.resource_mut::<Time>().advance_frame(1.0);
    schedule.run_stage(tungsten_core::Stage::PostUpdate, &mut world, |_, _| {});
    world
        .resource_mut::<tungsten_core::WorldLabels>()
        .drain()
        .for_each(drop);
    world.insert_resource(crate::foundation::trace::KitTrace::default());
    hit(
        &mut world,
        golem,
        &[(5.0, &["elemental.fire"]), (6.0, &["plain"])],
    );
    crate::damage(&mut world, golem, 7.0);
    typed_step(&mut world, &mut schedule);
    assert_eq!(
        drawn(&mut world, &mut schedule),
        [("5".to_string(), "sans".to_string(), [0, 0, 255, 255])]
    );
    let skips = world
        .resource::<crate::foundation::trace::KitTrace>()
        .about(golem)
        .filter(|entry| {
            matches!(
                entry.kind,
                TraceKind::Skipped {
                    event: "damaged_part",
                    action: "label",
                    why: "the part's types set no label style"
                }
            )
        })
        .count();
    assert_eq!(skips, 2);
}

#[test]
fn a_kind_mask_follows_a_type_a_reload_adds() {
    let (mut world, mut schedule) =
        typed_running(json!({ "elemental.fire": {}, "elemental.cold": {} }));
    define(&mut world, json!({ "elemental.fire": {} }));
    insert_rule_list(
        &mut world,
        "kinds",
        json!([{ "on": "damaged_part", "kind": { "any": ["elemental"] }, "do": [{ "counter": { "name": "elemental", "add": 1, "target": "game" } }] }]),
    );
    let golem = world.spawn_with((crate::Health::new(100.0),));
    hit(&mut world, golem, &[(1.0, &["elemental.fire"])]);
    typed_step(&mut world, &mut schedule);
    assert_eq!(
        game_count(&world, "elemental"),
        1,
        "the mask of `elemental` kept"
    );
    define(
        &mut world,
        json!({ "elemental.fire": {}, "elemental.cold": {} }),
    );
    hit(&mut world, golem, &[(1.0, &["elemental.cold"])]);
    typed_step(&mut world, &mut schedule);
    assert_eq!(
        game_count(&world, "elemental"),
        2,
        "the new type joins the name's mask"
    );
}
