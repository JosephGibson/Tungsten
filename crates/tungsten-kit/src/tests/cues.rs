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
    let world = &*world;
    rules
        .0
        .iter()
        .enumerate()
        .try_for_each(|(i, rule)| {
            let event = event_info(world, &format!("{i}.on"), &rule.on)?;
            for (field, name) in &rule.names {
                check_declared(world, &format!("{i}.filter.{field}"), name)?;
            }
            check_actions(world, &format!("{i}.do"), event, &rule.actions)
        })
        .map_err(|e| format!("unknown {} `{}` at {}", e.kind, e.id, e.field))
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
    ]
    .iter()
    .map(CueAction::name)
    .collect();
    assert_eq!(
        names,
        [
            "prefab", "counter", "flag", "despawn", "sound", "shake", "squash", "flash", "label"
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

/// The game's pickup.
struct PickedUp {
    item: Entity,
    by: Entity,
}

/// A pickup by the item itself is no pickup.
fn picked_up(event: &PickedUp) -> Option<CueEvent> {
    (event.item != event.by).then(|| CueEvent::new(event.item, Vec2::ZERO).with_other(event.by))
}

#[test]
fn a_game_event_registers_its_queue_and_its_name_for_cues() {
    let (mut world, mut schedule) = rig();
    CuesOnEvent::<PickedUp>::new("picked_up", picked_up).build(&mut schedule, &mut world);
    assert!(world.has_event::<PickedUp>());
    assert!(
        check(
            &world,
            &json!({ "picked_up": [{ "despawn": {} }, { "counter": { "name": "coins", "add": 1, "target": "other" } }, { "label": { "font": "sans" } }] })
        )
        .is_ok(),
        "a game event may carry an amount and an other"
    );
    let plugin = CuesOnEvent::<PickedUp>::new("picked_up", picked_up);
    assert_eq!(
        format!("{plugin:?}"),
        r#"CuesOnEvent { name: "picked_up", .. }"#
    );
    let _ = plugin.clone();
}

#[test]
#[should_panic(expected = "is one of the kit's cue events")]
fn a_game_event_named_as_the_kits_panics() {
    let (mut world, mut schedule) = rig();
    CuesOnEvent::<PickedUp>::new("killed", picked_up).build(&mut schedule, &mut world);
}

#[test]
#[should_panic(expected = "is already registered")]
fn a_game_event_registered_twice_panics() {
    let (mut world, mut schedule) = rig();
    CuesOnEvent::<PickedUp>::new("picked_up", picked_up).build(&mut schedule, &mut world);
    CuesOnEvent::<PickedUp>::new("picked_up", picked_up).build(&mut schedule, &mut world);
}

#[test]
#[should_panic(expected = "is not an event name")]
fn a_game_event_with_a_bad_name_panics() {
    let (mut world, mut schedule) = rig();
    CuesOnEvent::<PickedUp>::new("Picked Up", picked_up).build(&mut schedule, &mut world);
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
        damage(&mut world, slime, 1.0);
        step(&mut world, &mut schedule);
        assert_eq!(game(&world, "hits"), 1);
        assert!(
            traced(&world, slime)
                .contains(&"damaged: all skipped, the subject is despawning".to_string())
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
