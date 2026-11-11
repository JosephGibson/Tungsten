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
fn a_clip_action_plays_on_the_subject() {
    use tungsten_core::assets::AnimationFrame;
    use tungsten_core::{AnimationData, AnimationRegistry, SpriteAssetId, Transform};

    use crate::foundation::trace::KitTrace;
    use crate::{CharacterAnimator, Health, damage};

    let (mut world, mut schedule) = running::rig();
    let mut clips = AnimationRegistry::new();
    for (id, first) in [("hero_idle", 0), ("hero_hurt", 5)] {
        let frames = vec![AnimationFrame {
            sprite: SpriteAssetId::new(first),
            duration_ms: 100,
            sound: None,
        }];
        clips.insert(
            id.to_string(),
            AnimationData {
                looping: true,
                frames,
            },
        );
    }
    world.insert_resource(clips);
    let hurt = Cues::new().on("damaged", [CueAction::clip("hero_hurt")]);
    let hero = world.spawn_with((
        Health::new(5.0),
        CharacterAnimator::new("hero_idle"),
        hurt.clone(),
        Transform::default(),
    ));
    let post = world.spawn_with((Health::new(5.0), hurt, Transform::default()));
    damage(&mut world, hero, 1.0);
    damage(&mut world, post, 1.0);
    running::step(&mut world, &mut schedule);
    assert_eq!(
        world.get::<CharacterAnimator>(hero).unwrap().playing(),
        Some(&Clips::one("hero_hurt"))
    );
    assert!(
        world.resource::<KitTrace>().iter().any(|entry| {
            entry.entity == post
                && matches!(
                    entry.kind,
                    TraceKind::Skipped {
                        action: "clip",
                        why: NO_ANIMATOR,
                        ..
                    }
                )
        }),
        "a subject without an animator skips it, traced"
    );
    let action = CueAction::clip(Clips::aim(["hero_idle", "hero_hurt"]));
    let data = json!({ "clip": { "aim": ["hero_idle", "hero_hurt"] } });
    assert_eq!(serde_json::to_value(&action).unwrap(), data);
    assert_eq!(serde_json::from_value::<CueAction>(data).unwrap(), action);
    assert_eq!(action.name(), "clip");
    check(&world, &json!({ "damaged": [{ "clip": "hero_hurt" }] })).unwrap();
    let error = check(
        &world,
        &json!({ "damaged": [{ "clip": { "aim": ["hero_idle", "hero_hrt"] } }] }),
    )
    .unwrap_err();
    assert!(error.contains("clip.aim.1"), "{error}");
}

#[test]
fn a_path_sends_arrived_and_departed_only_when_named() {
    use tungsten_core::physics::{Position, Velocity};
    use tungsten_core::{CommandBuffer, Stage, system};

    use crate::foundation::phases::Phase;
    use crate::{Counters, PathFollower, PathFollowerPlugin};

    /// The kit cue list as the cue system finds it each step.
    #[derive(Default)]
    struct Seen(Vec<(Entity, &'static str)>);

    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(Seen::default());
    let mut schedule = Schedule::new();
    PathFollowerPlugin.build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    schedule.add(
        Phase::React.stage(),
        Phase::React
            .join(system("seen", |world: &mut World| {
                let cues: Vec<(Entity, &'static str)> = world
                    .resource::<KitCues>()
                    .0
                    .iter()
                    .map(|cue| (cue.entity, cue.event))
                    .collect();
                world.resource_mut::<Seen>().0.extend(cues);
            }))
            .before(CUES),
    );
    schedule.resolve().unwrap();
    // Two ping-pongs of 10 px at 60 px/s: a turn every 10 steps.
    let path = || PathFollower::new([Vec2::ZERO, Vec2::new(10.0, 0.0)], 60.0).ping_pong();
    let counted = Cues::new()
        .on(
            "arrived",
            [CueAction::counter_add("arrivals", 1).for_game()],
        )
        .on(
            "departed",
            [CueAction::counter_add("departures", 1).for_game()],
        );
    let named = world.spawn_with((Position(Vec2::ZERO), Velocity(Vec2::ZERO), path(), counted));
    let plain = world.spawn_with((Position(Vec2::ZERO), Velocity(Vec2::ZERO), path()));
    for _ in 0..40 {
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        world.resource_mut::<Time>().enter_fixed_step();
        schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
        world.resource_mut::<Time>().leave_fixed_steps();
    }
    let seen = &world.resource::<Seen>().0;
    assert!(seen.iter().all(|&(entity, _)| entity == named), "{seen:?}");
    assert!(!seen.iter().any(|&(entity, _)| entity == plain));
    let count = |event: &str| seen.iter().filter(|(_, name)| *name == event).count();
    // Out of its first point at once, then a turn at each end.
    assert!(count(DEPARTED) >= 4 && count(ARRIVED) >= 3, "{seen:?}");
    let counters = world.resource::<Counters>();
    assert_eq!(counters.get("arrivals"), count(ARRIVED) as i64);
    assert_eq!(counters.get("departures"), count(DEPARTED) as i64);
    assert!(KIT_EVENTS.contains(&ARRIVED) && KIT_EVENTS.contains(&DEPARTED));
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
fn a_listener_hears_in_full_within_full_then_falls_to_silence_at_silent() {
    let listener = CueListener::new(100.0, 300.0).at(Vec2::new(10.0, 0.0));
    assert_eq!((listener.full(), listener.silent()), (100.0, 300.0));
    assert_eq!(listener.position(), Vec2::new(10.0, 0.0));
    let gain = |x: f32| listener.gain(Vec2::new(10.0 + x, 0.0));
    assert_eq!(gain(0.0), 1.0);
    assert_eq!(gain(100.0), 1.0);
    let falling: Vec<f32> = (0..=100).map(|i| gain(100.0 + 2.0 * i as f32)).collect();
    assert!(
        falling.windows(2).all(|pair| pair[1] < pair[0]),
        "{falling:?}"
    );
    assert_eq!(gain(300.0), 0.0);
    assert_eq!(gain(-900.0), 0.0);
    assert_eq!(listener.gain(Vec2::NAN), 0.0);
}

#[test]
#[should_panic(expected = "0 <= full < silent")]
fn a_listener_whose_full_reaches_its_silent_panics() {
    let _ = CueListener::new(300.0, 300.0);
}

#[test]
#[should_panic(expected = "0 <= full < silent")]
fn a_listener_with_a_negative_distance_panics() {
    let _ = CueListener::new(-1.0, 300.0);
}

#[test]
#[should_panic(expected = "0 <= full < silent")]
fn a_listener_with_a_nan_distance_panics() {
    let _ = CueListener::new(100.0, f32::NAN);
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
    pub(super) fn rig() -> (World, Schedule) {
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
    pub(super) fn step(world: &mut World, schedule: &mut Schedule) {
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

    /// The volumes of the one-shots played since the last drain.
    fn volumes(world: &mut World) -> Vec<f32> {
        world
            .resource_mut::<AudioCommands>()
            .drain()
            .into_iter()
            .filter_map(|command| match command {
                AudioCommand::Play {
                    volume,
                    looping: false,
                    ..
                } => Some(volume),
                _ => None,
            })
            .collect()
    }

    /// A target at `x` on the axis whose hits play `blip` at most every
    /// `interval` seconds.
    fn sounding(world: &mut World, x: f32, interval: f32) -> Entity {
        world.spawn_with((
            Health::new(1000.0),
            Position(Vec2::new(x, 0.0)),
            Cues::new().on(
                "damaged",
                [CueAction::sound("blip").with_min_interval(interval)],
            ),
        ))
    }

    #[test]
    fn a_listener_quiets_cue_sounds_by_distance_and_skips_those_past_its_hearing() {
        let (mut world, mut schedule) = rig();
        world.insert_resource(CueListener::new(100.0, 300.0));
        let mut heard = Vec::new();
        for x in [50.0, 200.0, 300.0, 500.0] {
            let target = sounding(&mut world, x, 0.0);
            damage(&mut world, target, 1.0);
            step(&mut world, &mut schedule);
            let sounds: Vec<String> = traced(&world, target)
                .into_iter()
                .filter(|line| line.contains("sound"))
                .collect();
            heard.push((volumes(&mut world), sounds));
            flush(&mut world);
        }
        let unheard = "damaged: sound skipped, the cue's position is past the listener's hearing";
        assert_eq!(
            heard,
            [
                (vec![1.0], vec!["damaged: sound (own cue)".to_owned()]),
                (vec![0.25], vec!["damaged: sound (own cue)".to_owned()]),
                (vec![], vec![unheard.to_owned()]),
                (vec![], vec![unheard.to_owned()]),
            ]
        );
    }

    #[test]
    fn an_unheard_cue_leaves_its_interval_to_a_nearer_one_and_a_moved_listener_hears_anew() {
        let (mut world, mut schedule) = rig();
        world.insert_resource(CueListener::new(100.0, 300.0));
        let (far, near) = (
            sounding(&mut world, 500.0, 0.1),
            sounding(&mut world, 50.0, 0.1),
        );
        damage(&mut world, far, 1.0);
        step(&mut world, &mut schedule);
        assert_eq!(volumes(&mut world), [] as [f32; 0]);
        flush(&mut world);
        damage(&mut world, near, 1.0);
        step(&mut world, &mut schedule);
        assert_eq!(volumes(&mut world), [1.0], "within the far one's 0.1 s");
        flush(&mut world);
        for _ in 0..6 {
            step(&mut world, &mut schedule);
            flush(&mut world);
        }
        world
            .resource_mut::<CueListener>()
            .set_position(Vec2::new(450.0, 0.0));
        damage(&mut world, far, 1.0);
        step(&mut world, &mut schedule);
        assert_eq!(volumes(&mut world), [1.0], "50 px from the moved listener");
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

// 7.10a, Step 8: the type label's merge, ticks and crit style.

/// Damage types whose label styles are fire red, cold blue and elemental
/// yellow.
fn styled() -> Value {
    let style = |color: [f32; 4]| json!({ "label_style": { "font": "sans", "color": color } });
    json!({
        "elemental": style([1.0, 1.0, 0.0, 1.0]),
        "elemental.fire": style([1.0, 0.0, 0.0, 1.0]),
        "elemental.cold": style([0.0, 0.0, 1.0, 1.0])
    })
}

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const GOLD: [u8; 4] = [255, 218, 0, 255];

/// A target whose `damaged_part` cue is the type label `label`.
fn numbered(world: &mut World, label: &Value) -> Entity {
    let action: CueAction = serde_json::from_value(json!({ "label": label })).unwrap();
    world.spawn_with((
        crate::Health::new(1000.0),
        Cues::new().on("damaged_part", [action]),
    ))
}

/// Every label floated so far, in float order, as text, size and colour:
/// a frame of no time drawn, so none ages.
fn floated(world: &mut World, schedule: &mut Schedule) -> Vec<(String, f32, [u8; 4])> {
    world.resource_mut::<Time>().advance_frame(0.0);
    schedule.run_stage(tungsten_core::Stage::PostUpdate, world, |_, _| {});
    world
        .resource_mut::<tungsten_core::WorldLabels>()
        .drain()
        .map(|label| (label.text, label.font_size, label.color))
        .collect()
}

fn label(text: &str, size: f32, color: [u8; 4]) -> (String, f32, [u8; 4]) {
    (text.to_string(), size, color)
}

/// A periodic request of one part: an effect's tick.
fn tick(world: &mut World, target: Entity, amount: f32, kind: &str) {
    crate::DamageRequest::from_parts(
        target,
        [crate::DamagePart::new(amount).with_tags(Tags::new([kind]))],
    )
    .with_periodic(true)
    .apply(world);
}

#[test]
fn a_types_first_plain_part_floats_at_once_and_its_followers_within_the_window_as_one_sum_when_it_closes()
 {
    let (mut world, mut schedule) = typed_running(styled());
    let golem = numbered(&mut world, &json!({ "merge": 0.25 }));
    hit(&mut world, golem, &[(4.0, &["elemental.fire"])]);
    typed_step(&mut world, &mut schedule);
    let first = vec![label("4", 16.0, RED)];
    assert_eq!(
        floated(&mut world, &mut schedule),
        first,
        "the first at once"
    );
    // Fire's followers sum; cold's first floats at once in its window.
    hit(
        &mut world,
        golem,
        &[(2.0, &["elemental.fire"]), (1.0, &["elemental.cold"])],
    );
    typed_step(&mut world, &mut schedule);
    hit(&mut world, golem, &[(1.5, &["elemental.fire"])]);
    let mut so_far = first;
    so_far.push(label("1", 16.0, BLUE));
    for _ in 2..15 {
        typed_step(&mut world, &mut schedule);
        assert_eq!(
            floated(&mut world, &mut schedule),
            so_far,
            "the window open"
        );
    }
    // 0.25 s after the first: fire's window closes with its followers.
    typed_step(&mut world, &mut schedule);
    so_far.push(label("3.5", 16.0, RED));
    assert_eq!(floated(&mut world, &mut schedule), so_far);
    // Cold's closes with nothing in it; the next fire part opens anew.
    typed_step(&mut world, &mut schedule);
    hit(&mut world, golem, &[(5.0, &["elemental.fire"])]);
    typed_step(&mut world, &mut schedule);
    so_far.push(label("5", 16.0, RED));
    assert_eq!(floated(&mut world, &mut schedule), so_far);
}

#[test]
fn periodic_parts_passing_ticks_kind_sum_on_their_own_at_its_size() {
    let (mut world, mut schedule) = typed_running(styled());
    let golem = numbered(
        &mut world,
        &json!({ "merge": 0.25, "ticks": { "size": 10, "kind": { "any": ["elemental.fire"] } } }),
    );
    // A fire tick waits; a cold tick fails `kind` and floats as a plain
    // part.
    tick(&mut world, golem, 1.0, "elemental.fire");
    tick(&mut world, golem, 2.0, "elemental.cold");
    typed_step(&mut world, &mut schedule);
    let mut so_far = vec![label("2", 16.0, BLUE)];
    assert_eq!(floated(&mut world, &mut schedule), so_far);
    // A plain fire part keeps its own window beside the ticks'.
    tick(&mut world, golem, 1.0, "elemental.fire");
    hit(&mut world, golem, &[(4.0, &["elemental.fire"])]);
    typed_step(&mut world, &mut schedule);
    so_far.push(label("4", 16.0, RED));
    assert_eq!(floated(&mut world, &mut schedule), so_far);
    tick(&mut world, golem, 1.5, "elemental.fire");
    for _ in 2..15 {
        typed_step(&mut world, &mut schedule);
    }
    assert_eq!(floated(&mut world, &mut schedule), so_far, "the ticks wait");
    typed_step(&mut world, &mut schedule);
    so_far.push(label("3.5", 10.0, RED));
    assert_eq!(
        floated(&mut world, &mut schedule),
        so_far,
        "three fire ticks as one, at the tick size"
    );
}

#[test]
fn a_steps_crit_parts_on_a_target_float_at_once_as_one_sum_in_the_crit_style() {
    let (mut world, mut schedule) = typed_running(styled());
    let crit = json!({ "merge": 0.25, "crit": { "size": 24, "color": [1, 0.7, 0, 1] } });
    let golem = numbered(&mut world, &crit);
    let imp = numbered(&mut world, &crit);
    let crit_hit = |world: &mut World, target: Entity, parts: &[(f32, &str)]| {
        let parts = parts
            .iter()
            .map(|&(amount, kind)| crate::DamagePart::new(amount).with_tags(Tags::new([kind])));
        crate::DamageRequest::from_parts(target, parts)
            .with_crit(true)
            .apply(world);
    };
    crit_hit(
        &mut world,
        golem,
        &[(4.0, "elemental.fire"), (2.0, "elemental.cold")],
    );
    hit(&mut world, golem, &[(1.0, &["elemental.cold"])]);
    crit_hit(&mut world, golem, &[(3.0, "elemental.cold")]);
    crit_hit(&mut world, imp, &[(5.0, "elemental.cold")]);
    typed_step(&mut world, &mut schedule);
    assert_eq!(
        floated(&mut world, &mut schedule),
        [
            label("1", 16.0, BLUE),
            label("9", 24.0, GOLD),
            label("5", 24.0, GOLD)
        ],
        "the plain part at once, each target's crits as one at the step's end"
    );
    crit_hit(&mut world, golem, &[(2.0, "elemental.fire")]);
    typed_step(&mut world, &mut schedule);
    assert_eq!(
        floated(&mut world, &mut schedule).last(),
        Some(&label("2", 24.0, GOLD)),
        "the next step's crits on their own"
    );
}

#[test]
fn a_label_without_the_fields_floats_each_part_alone() {
    let (mut world, mut schedule) = typed_running(styled());
    let golem = numbered(&mut world, &json!({}));
    // Two requests: a request's parts of one type land as one.
    hit(&mut world, golem, &[(4.0, &["elemental.fire"])]);
    hit(&mut world, golem, &[(2.0, &["elemental.fire"])]);
    crate::DamageRequest::from_parts(
        golem,
        [crate::DamagePart::new(3.0).with_tags(Tags::new(["elemental.cold"]))],
    )
    .with_crit(true)
    .apply(&mut world);
    tick(&mut world, golem, 1.0, "elemental.fire");
    typed_step(&mut world, &mut schedule);
    hit(&mut world, golem, &[(1.5, &["elemental.fire"])]);
    typed_step(&mut world, &mut schedule);
    assert_eq!(
        floated(&mut world, &mut schedule),
        [
            label("4", 16.0, RED),
            label("2", 16.0, RED),
            label("3", 16.0, BLUE),
            label("1", 16.0, RED),
            label("1.5", 16.0, RED),
        ]
    );
    // A window without ticks or crit floats ticks and crits as plain parts.
    let grouped: CueAction = serde_json::from_value(json!({ "label": { "merge": 0.25 } })).unwrap();
    assert_ne!(grouped, CueAction::type_label());
}

#[test]
fn a_grouped_label_is_checked_as_a_type_label_with_its_ticks_kind_declared() {
    let world = typed_rig();
    let grouped =
        json!({ "merge": 0.25, "ticks": { "size": 10, "kind": { "any": ["elemental"] } } });
    check(&world, &json!({ "damaged_part": [{ "label": grouped }] })).unwrap();
    assert!(check(&world, &json!({ "damaged": [{ "label": grouped }] })).is_err());
    let unknown = check(
        &world,
        &json!({ "damaged_part": [{ "label": { "ticks": { "size": 10, "kind": { "all": ["arcane"] } } } }] }),
    )
    .unwrap_err();
    assert!(unknown.contains("ticks.kind.all.0"), "{unknown}");
}

/// 7.10p's rule filters and limits, and `CueEvent::of`.
mod filters {
    use super::*;
    use tungsten_core::Stage;

    use crate::damage::effects::{EffectRequest, EffectsPlugin, end_effect};
    use crate::foundation::trace::KitTrace;
    use crate::{CountersPlugin, Damaged, Health, HealthPlugin, StatsPlugin, damage};

    /// The cue rig with time, a command buffer, a trace, stats, health,
    /// counters and effects, and the game event `rang`.
    fn rig() -> (World, Schedule) {
        let (mut world, _) = super::rig();
        world.insert_resource(Time::new());
        world.insert_resource(CommandBuffer::new());
        world.insert_resource(KitTrace::default());
        world.register_event::<ZoneEntered>();
        let mut schedule = Schedule::new();
        crate::TagsPlugin::new()
            .with_tags(["team.enemy"])
            .build(&mut schedule, &mut world);
        StatsPlugin::new().build(&mut schedule, &mut world);
        HealthPlugin.build(&mut schedule, &mut world);
        CountersPlugin.build(&mut schedule, &mut world);
        CuesPlugin.build(&mut schedule, &mut world);
        EffectsPlugin.build(&mut schedule, &mut world);
        CuesOnEvent::<Rang>::new("rang", rang).build(&mut schedule, &mut world);
        schedule.resolve().unwrap();
        (world, schedule)
    }

    /// A game event about a bell, with an amount when it has one.
    struct Rang(Entity, Option<f32>);

    #[allow(clippy::unnecessary_wraps)]
    fn rang(event: &Rang) -> Option<CueEvent> {
        let at = CueEvent::of(event.0);
        Some(match event.1 {
            Some(amount) => at.with_amount(amount),
            None => at,
        })
    }

    fn ring(world: &mut World, bell: Entity, amount: Option<f32>) {
        world
            .resource_mut::<EventQueue<Rang>>()
            .send(Rang(bell, amount));
    }

    /// One step of `dt` as the frame's game time, then the command and
    /// event flushes that end the frame.
    fn step(world: &mut World, schedule: &mut Schedule, dt: f32) {
        world.resource_mut::<Time>().advance_frame(dt);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
        let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
        world.flush_reusing(&mut commands);
        world.insert_resource(commands);
        world.flush_events();
    }

    fn insert_rules(world: &mut World, value: Value) {
        let rules: CueRules = serde_json::from_value(value).unwrap();
        world.resource_mut::<DataRegistry>().insert("r", rules);
    }

    /// A rule counting `name` for the game.
    fn count(name: &str) -> Value {
        json!([{ "counter": { "name": name, "add": 1, "target": "game" } }])
    }

    fn game(world: &World, name: &str) -> i64 {
        world
            .get_resource::<Counters>()
            .map_or(0, |counters| counters.get(name))
    }

    fn load_error(world: &mut World, value: Value) -> String {
        check_rules(world, value).unwrap_err()
    }

    #[test]
    fn a_rule_tells_a_blow_from_a_tick() {
        let (mut world, mut schedule) = rig();
        world.resource_mut::<DataRegistry>().insert(
            "burning",
            Effect::new()
                .with_duration(3.0)
                .with_period(0.5)
                .with_damage(1.0),
        );
        insert_rules(
            &mut world,
            json!([
                { "on": "damaged", "periodic": false, "do": count("blows") },
                { "on": "damaged", "periodic": true, "do": count("ticks") },
                { "on": "killed", "periodic": true, "do": count("tick_kills") },
                { "on": "killed", "periodic": false, "do": count("blow_kills") }
            ]),
        );
        let target = world.spawn_with((Health::new(100.0),));
        damage(&mut world, target, 1.0);
        EffectRequest::new(target, DataRef::by_id("burning")).apply(&mut world);
        for _ in 0..240 {
            step(&mut world, &mut schedule, 1.0 / 60.0);
        }
        assert_eq!(
            (game(&world, "blows"), game(&world, "ticks")),
            (1, 6),
            "the blow alone, then a tick each 0.5 s over 3 s"
        );
        let weak = world.spawn_with((Health::new(2.5),));
        EffectRequest::new(weak, DataRef::by_id("burning")).apply(&mut world);
        for _ in 0..120 {
            step(&mut world, &mut schedule, 1.0 / 60.0);
        }
        assert!(world.get::<Health>(weak).unwrap().is_dead());
        assert_eq!(
            (game(&world, "tick_kills"), game(&world, "blow_kills")),
            (1, 0),
            "a kill by a tick passes `killed`'s `true` alone"
        );
        let error = load_error(
            &mut world,
            json!([{ "on": "zone_entered", "periodic": true, "do": [] }]),
        );
        assert!(error.ends_with("at 0.periodic"), "{error}");
    }

    #[test]
    fn a_rule_reads_the_amount() {
        let (mut world, mut schedule) = rig();
        insert_rules(
            &mut world,
            json!([{ "on": "rang", "amount": { "min": 300, "max": 600 }, "do": count("passed") }]),
        );
        let bell = world.spawn();
        for (amount, passes) in [
            (Some(300.0), 1),
            (Some(600.0), 2),
            (Some(299.9), 2),
            (Some(600.1), 2),
            (None, 2),
            (Some(450.0), 3),
        ] {
            ring(&mut world, bell, amount);
            step(&mut world, &mut schedule, 1.0 / 60.0);
            assert_eq!(game(&world, "passed"), passes, "{amount:?}");
        }
        let error = load_error(
            &mut world,
            json!([{ "on": "cooldown_elapsed", "amount": { "min": 1 }, "do": [] }]),
        );
        assert!(error.ends_with("at 0.amount"), "{error}");
        assert!(
            rules(json!([{ "on": "rang", "amount": { "min": 2, "max": 1 }, "do": [] }])).is_err()
        );
    }

    #[test]
    fn a_rule_reads_the_games_flags() {
        let (mut world, mut schedule) = rig();
        insert_rules(
            &mut world,
            json!([
                { "on": "rang", "flags": { "all": ["armed"] }, "do": count("all") },
                { "on": "rang", "flags": { "any": ["red", "blue"] }, "do": count("any") },
                { "on": "rang", "flags": { "none": ["muted"] }, "do": count("none") },
                { "on": "rang", "amount": { "min": 1 }, "do": [{ "flag": { "name": "armed", "target": "game" } }] }
            ]),
        );
        let bell = world.spawn();
        let counts = |world: &World| [game(world, "all"), game(world, "any"), game(world, "none")];
        world.remove_resource::<Flags>();
        ring(&mut world, bell, None);
        step(&mut world, &mut schedule, 1.0 / 60.0);
        assert!(!world.has_resource::<Flags>());
        assert_eq!(counts(&world), [0, 0, 1], "none set without the resource");
        // The fourth rule sets `armed` for the game; the next step's ring
        // reads it.
        ring(&mut world, bell, Some(1.0));
        step(&mut world, &mut schedule, 1.0 / 60.0);
        assert_eq!(counts(&world), [0, 0, 2], "set after this step's match");
        ring(&mut world, bell, None);
        step(&mut world, &mut schedule, 1.0 / 60.0);
        assert_eq!(counts(&world), [1, 0, 3]);
        world.resource_mut::<Flags>().set("blue", true);
        world.resource_mut::<Flags>().set("muted", true);
        ring(&mut world, bell, None);
        step(&mut world, &mut schedule, 1.0 / 60.0);
        assert_eq!(counts(&world), [2, 1, 3]);
        assert!(rules(json!([{ "on": "rang", "flags": { "all": ["Bad"] }, "do": [] }])).is_err());
    }

    #[test]
    fn a_rule_picks_an_effect_fresh_or_ended() {
        let (mut world, mut schedule) = rig();
        let scared = Effect::new().with_duration(0.5);
        world
            .resource_mut::<DataRegistry>()
            .insert("scared", scared.clone());
        world.resource_mut::<DataRegistry>().insert("other", scared);
        insert_rules(
            &mut world,
            json!([
                { "on": "effect_applied", "effect": "scared", "do": count("applied") },
                { "on": "effect_applied", "effect": "scared", "fresh": true, "do": count("fresh") },
                { "on": "effect_applied", "effect": "scared", "fresh": false, "do": count("again") },
                { "on": "effect_expired", "effect": "scared", "ended": false, "do": count("ran_out") },
                { "on": "effect_expired", "effect": "scared", "ended": true, "do": count("ended") }
            ]),
        );
        let fox = world.spawn();
        let apply = |world: &mut World, id: &str| {
            EffectRequest::new(fox, DataRef::by_id(id)).apply(world);
        };
        apply(&mut world, "scared");
        apply(&mut world, "other");
        step(&mut world, &mut schedule, 1.0 / 60.0);
        step(&mut world, &mut schedule, 1.0 / 60.0);
        apply(&mut world, "scared");
        step(&mut world, &mut schedule, 1.0 / 60.0);
        let counts = |world: &World| {
            ["applied", "fresh", "again", "ran_out", "ended"].map(|name| game(world, name))
        };
        assert_eq!(
            counts(&world),
            [2, 1, 1, 0, 0],
            "its entry alone; then a refresh"
        );
        for _ in 0..40 {
            step(&mut world, &mut schedule, 1.0 / 60.0);
        }
        assert_eq!(counts(&world), [2, 1, 1, 1, 0], "its natural end");
        apply(&mut world, "scared");
        step(&mut world, &mut schedule, 1.0 / 60.0);
        assert!(end_effect(&mut world, fox, &DataRef::by_id("scared")));
        step(&mut world, &mut schedule, 1.0 / 60.0);
        assert_eq!(counts(&world), [3, 2, 1, 1, 1], "an `end_effect`");
        let error = load_error(
            &mut world,
            json!([{ "on": "effect_applied", "effect": "nope", "do": [] }]),
        );
        assert!(error.ends_with("at 0.effect"), "{error}");
        let error = load_error(
            &mut world,
            json!([{ "on": "effect_expired", "fresh": true, "do": [] }]),
        );
        assert!(error.ends_with("at 0.fresh"), "{error}");
        let error = load_error(
            &mut world,
            json!([{ "on": "rang", "effect": "scared", "do": [] }]),
        );
        assert!(error.ends_with("at 0.effect"), "{error}");
        let error = load_error(
            &mut world,
            json!([{ "on": "rang", "ended": true, "do": [] }]),
        );
        assert!(error.ends_with("at 0.ended"), "{error}");
    }

    /// Runs `reactor` alone for one step of `dt`, then flushes the events.
    fn react(world: &mut World, reactor: &mut Reactor, dt: f32) {
        world.resource_mut::<Time>().advance_frame(dt);
        reactor.run(world);
        world.flush_events();
    }

    #[test]
    fn a_rule_waits_its_interval_and_caps_a_step() {
        for dt in [1.0_f32 / 60.0, 1.0 / 30.0] {
            let (mut world, _) = rig();
            insert_rules(
                &mut world,
                json!([
                    { "on": "rang", "amount": { "min": 10 }, "min_interval": 0.25, "do": [{ "counter": { "name": "runs", "add": 1 } }] },
                    { "on": "rang", "amount": { "max": 0 }, "cap": 2, "do": count("capped") }
                ]),
            );
            let mut reactor = Reactor::default();
            let bells = [world.spawn(), world.spawn()];
            let steps = (1.0 / dt).round() as usize;
            for _ in 0..steps {
                for bell in bells {
                    ring(&mut world, bell, Some(20.0));
                }
                react(&mut world, &mut reactor, dt);
            }
            for bell in bells {
                assert_eq!(
                    world.get::<Counters>(bell).unwrap().get("runs"),
                    4,
                    "dt {dt}: each subject at 0, 0.25, 0.5 and 0.75 s"
                );
            }
            let held = world
                .resource::<KitTrace>()
                .about(bells[0])
                .filter(|entry| matches!(entry.kind, TraceKind::Skipped { why: WAITING, .. }))
                .count();
            assert_eq!(held, steps - 4, "dt {dt}: each held step traced");
            // Five matches in a step run twice, each step.
            for _ in 0..2 {
                for _ in 0..5 {
                    ring(&mut world, bells[0], Some(0.0));
                }
                react(&mut world, &mut reactor, dt);
            }
            assert_eq!(game(&world, "capped"), 4, "dt {dt}");
            // A ring the filter turns away spends no interval: the next one
            // runs.
            let quiet = world.spawn();
            ring(&mut world, quiet, Some(5.0));
            react(&mut world, &mut reactor, dt);
            ring(&mut world, quiet, Some(20.0));
            react(&mut world, &mut reactor, dt);
            assert_eq!(world.get::<Counters>(quiet).unwrap().get("runs"), 1);
            // A gone subject's wait is swept within a game second; a live
            // one's once its interval passes.
            world.despawn(bells[1]);
            let waiting =
                |reactor: &Reactor, bell: Entity| reactor.waits["r.0"].contains_key(&bell);
            assert!(waiting(&reactor, bells[1]) && waiting(&reactor, quiet));
            for _ in 0..steps {
                react(&mut world, &mut reactor, dt);
            }
            assert!(reactor.waits.is_empty(), "dt {dt}: {:?}", reactor.waits);
        }
    }

    #[test]
    fn cue_event_of_takes_its_subjects_point() {
        let (mut world, mut schedule) = rig();
        let bell = world.spawn_with((
            Position(Vec2::new(12.0, -7.0)),
            Cues::new().on("rang", [CueAction::prefab("puff")]),
        ));
        let plain = world.spawn_with((
            Transform::from_position(Vec2::new(3.0, 4.0)),
            Cues::new().on("rang", [CueAction::prefab("puff")]),
        ));
        ring(&mut world, bell, None);
        ring(&mut world, plain, None);
        step(&mut world, &mut schedule, 1.0 / 60.0);
        let mut puffs: Vec<Vec2> = world
            .query::<(&tungsten_core::Tag, &Transform)>()
            .filter(|(tag, _)| tag.name == "puff")
            .map(|(_, transform)| transform.position)
            .collect();
        puffs.sort_by(|a, b| a.x.total_cmp(&b.x));
        assert_eq!(puffs, [Vec2::new(3.0, 4.0), Vec2::new(12.0, -7.0)]);
    }

    #[test]
    fn damaged_says_what_the_shield_took() {
        use crate::Shield;

        let (mut world, mut schedule) = rig();
        insert_rules(
            &mut world,
            json!([
                { "on": "damaged", "absorbed": true, "do": count("blocked") },
                { "on": "damaged", "absorbed": false, "do": count("hurt") }
            ]),
        );
        let knight = world.spawn_with((Health::new(100.0), Shield::new(50.0)));
        let squire = world.spawn_with((Health::new(100.0),));
        let absorbed = |world: &World| -> Vec<f32> {
            world
                .resource::<EventQueue<Damaged>>()
                .iter_current()
                .map(|event| event.absorbed)
                .collect()
        };
        damage(&mut world, knight, 30.0);
        assert_eq!(absorbed(&world), [30.0], "the whole hit");
        step(&mut world, &mut schedule, 1.0 / 60.0);
        assert_eq!((game(&world, "blocked"), game(&world, "hurt")), (1, 0));
        damage(&mut world, knight, 80.0);
        damage(&mut world, squire, 5.0);
        assert_eq!(
            absorbed(&world),
            [20.0, 0.0],
            "20 of the 80; none without a shield"
        );
        step(&mut world, &mut schedule, 1.0 / 60.0);
        assert_eq!((game(&world, "blocked"), game(&world, "hurt")), (1, 2));
        let error = load_error(
            &mut world,
            json!([{ "on": "killed", "absorbed": true, "do": [] }]),
        );
        assert!(error.ends_with("at 0.absorbed"), "{error}");
    }

    #[test]
    fn the_occurrence_keeps_its_size() {
        assert_eq!(
            std::mem::size_of::<Occurrence>(),
            64,
            "its size at cda6987a, measured before the filters"
        );
    }
}

/// 7.18's `camera` action: a game event that raises or releases rigs.
struct Looked(Entity);

#[allow(clippy::unnecessary_wraps)]
fn looked(event: &Looked) -> Option<CueEvent> {
    Some(CueEvent::of(event.0))
}

/// The cue rig with time, commands, a trace, a camera controller and the
/// game event `looked`.
fn camera_rig() -> (World, Schedule) {
    let (mut world, _) = rig();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(crate::KitTrace::default());
    world.insert_resource(tungsten_core::CameraController::default());
    let mut schedule = Schedule::new();
    CuesPlugin.build(&mut schedule, &mut world);
    CuesOnEvent::<Looked>::new("looked", looked).build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// `subject`'s `looked` cue, run in one step.
fn look(world: &mut World, schedule: &mut Schedule, subject: Entity) {
    world
        .resource_mut::<EventQueue<Looked>>()
        .send(Looked(subject));
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(tungsten_core::Stage::FixedUpdate, world, |_, _| {});
    world.flush_events();
}

/// A lowered fixed rig named `id`.
fn named_rig(id: &str) -> (tungsten_core::CameraRig, CameraRigId) {
    let rig = tungsten_core::CameraRig::fixed(tungsten_core::FixedRig::At(Vec2::ZERO));
    (rig.lowered(), CameraRigId::new(id))
}

fn rig_live(world: &World, entity: Entity) -> bool {
    world
        .get::<tungsten_core::CameraRig>(entity)
        .unwrap()
        .is_live()
}

#[test]
fn a_camera_action_raises_and_releases_rigs_by_id() {
    let (mut world, mut schedule) = camera_rig();
    let cliff = world.spawn_with(named_rig("vista.cliff"));
    let ledge = world.spawn_with(named_rig("vista.cliff"));
    let sea = world.spawn_with(named_rig("vista.sea"));
    let raiser =
        world.spawn_with((Cues::new().on("looked", [CueAction::raise_camera("vista.cliff")]),));
    let releaser =
        world.spawn_with((Cues::new().on("looked", [CueAction::release_camera("vista.cliff")]),));
    look(&mut world, &mut schedule, raiser);
    assert!(
        rig_live(&world, cliff) && rig_live(&world, ledge),
        "every rig with the ID"
    );
    assert!(!rig_live(&world, sea), "not another ID's");
    look(&mut world, &mut schedule, releaser);
    assert!(!rig_live(&world, cliff) && !rig_live(&world, ledge));
    for (action, data) in [
        (
            CueAction::raise_camera("vista.cliff"),
            json!({ "camera": { "raise": "vista.cliff" } }),
        ),
        (
            CueAction::release_camera("vista.cliff"),
            json!({ "camera": { "release": "vista.cliff" } }),
        ),
    ] {
        assert_eq!(serde_json::to_value(&action).unwrap(), data);
        assert_eq!(serde_json::from_value::<CueAction>(data).unwrap(), action);
        assert_eq!(action.name(), "camera");
    }
    for bad in [
        json!({ "camera": {} }),
        json!({ "camera": { "raise": "a", "release": "b" } }),
        json!({ "camera": { "raise": "Vista Cliff" } }),
        json!({ "camera": { "raise": "a", "pan": 1 } }),
    ] {
        assert!(
            serde_json::from_value::<CueAction>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn a_camera_action_skips_an_id_no_rig_holds() {
    let (mut world, mut schedule) = camera_rig();
    let sea = world.spawn_with(named_rig("vista.sea"));
    let raiser =
        world.spawn_with((Cues::new().on("looked", [CueAction::raise_camera("vista.cliff")]),));
    look(&mut world, &mut schedule, raiser);
    assert!(!rig_live(&world, sea));
    assert!(
        world.resource::<crate::KitTrace>().iter().any(|entry| {
            entry.entity == raiser
                && matches!(
                    entry.kind,
                    TraceKind::Skipped {
                        action: "camera",
                        why: NO_RIG,
                        ..
                    }
                )
        }),
        "a traced skip"
    );
}

/// 7.10p's listener: following, panning, a shake's falloff and loops.
mod listening {
    use super::*;
    use tungsten_core::{AudioCommand, Stage};

    use crate::foundation::trace::KitTrace;

    /// A game event about an entity: `rang` plays a cue, `hushed` stops a
    /// loop.
    struct Rang(Entity);
    struct Hushed(Entity);

    #[allow(clippy::unnecessary_wraps)]
    fn rang(event: &Rang) -> Option<CueEvent> {
        Some(CueEvent::of(event.0))
    }

    #[allow(clippy::unnecessary_wraps)]
    fn hushed(event: &Hushed) -> Option<CueEvent> {
        Some(CueEvent::of(event.0))
    }

    /// The cue rig with time, a trace, audio, the shake queue and the game
    /// events `rang` and `hushed`.
    fn rig() -> (World, Schedule) {
        let (mut world, _) = super::rig();
        world.insert_resource(Time::new());
        world.insert_resource(CommandBuffer::new());
        world.insert_resource(KitTrace::default());
        world.insert_resource(AudioCommands::new());
        world.register_event::<ShakeEvent>();
        let mut schedule = Schedule::new();
        CuesPlugin.build(&mut schedule, &mut world);
        CuesOnEvent::<Rang>::new("rang", rang).build(&mut schedule, &mut world);
        CuesOnEvent::<Hushed>::new("hushed", hushed).build(&mut schedule, &mut world);
        schedule.resolve().unwrap();
        (world, schedule)
    }

    fn step(world: &mut World, schedule: &mut Schedule) {
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
        let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
        world.flush_reusing(&mut commands);
        world.insert_resource(commands);
        world.flush_events();
    }

    fn ring(world: &mut World, entity: Entity) {
        world.resource_mut::<EventQueue<Rang>>().send(Rang(entity));
    }

    fn hush(world: &mut World, entity: Entity) {
        world
            .resource_mut::<EventQueue<Hushed>>()
            .send(Hushed(entity));
    }

    /// A command as the tests read it.
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Heard {
        Play {
            volume: f32,
            looping: bool,
            fade: f32,
            pan: f32,
        },
        Volume(f32),
        Pan(f32),
        Stop(f32),
        Other,
    }

    fn heard(world: &mut World) -> Vec<Heard> {
        world
            .resource_mut::<AudioCommands>()
            .drain()
            .into_iter()
            .map(|command| match command {
                AudioCommand::Play {
                    volume,
                    looping,
                    fade,
                    pan,
                    ..
                } => Heard::Play {
                    volume,
                    looping,
                    fade,
                    pan: pan.get(),
                },
                AudioCommand::SetVoiceVolume { volume, .. } => Heard::Volume(volume),
                AudioCommand::SetVoicePan { pan, .. } => Heard::Pan(pan.get()),
                AudioCommand::StopVoice { fade, .. } => Heard::Stop(fade),
                _ => Heard::Other,
            })
            .collect()
    }

    /// `pan` as a command carries it, to the nearest 1/16,384.
    fn quantized(pan: f32) -> f32 {
        tungsten_core::audio::Pan::new(pan).get()
    }

    fn at(world: &mut World, x: f32, cues: Cues) -> Entity {
        world.spawn_with((Position(Vec2::new(x, 0.0)), cues))
    }

    #[test]
    fn the_listener_follows_its_entity() {
        let (mut world, mut schedule) = rig();
        let hero = world.spawn_with((Position(Vec2::new(400.0, 0.0)),));
        world.insert_resource(CueListener::new(10.0, 100.0).following(hero));
        let bell = at(
            &mut world,
            900.0,
            Cues::new().on("rang", [CueAction::sound("blip")]),
        );
        step(&mut world, &mut schedule);
        assert_eq!(
            world.resource::<CueListener>().position(),
            Vec2::new(400.0, 0.0)
        );
        // Moved before the step's occurrences: a cue at the hero's new
        // place is heard whole.
        world.get_mut::<Position>(hero).unwrap().0 = Vec2::new(900.0, 0.0);
        ring(&mut world, bell);
        step(&mut world, &mut schedule);
        assert_eq!(
            heard(&mut world),
            [Heard::Play {
                volume: 1.0,
                looping: false,
                fade: 0.0,
                pan: 0.0
            }]
        );
        world.despawn(hero);
        step(&mut world, &mut schedule);
        assert_eq!(
            world.resource::<CueListener>().position(),
            Vec2::new(900.0, 0.0),
            "a gone entity leaves it"
        );
    }

    #[test]
    fn a_panned_listener_pans_cue_sounds() {
        let (mut world, mut schedule) = rig();
        world.insert_resource(CueListener::new(1000.0, 2000.0).with_pan(200.0));
        let blip = || Cues::new().on("rang", [CueAction::sound("blip")]);
        let left = at(&mut world, -100.0, blip());
        let right = at(&mut world, 300.0, blip());
        ring(&mut world, left);
        ring(&mut world, right);
        step(&mut world, &mut schedule);
        let pans: Vec<f32> = heard(&mut world)
            .into_iter()
            .map(|heard| match heard {
                Heard::Play { pan, .. } => pan,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(pans, [-0.5, 1.0]);
        // Without a pan width, the commands are today's byte for byte.
        let (mut world, mut schedule) = rig();
        world.insert_resource(CueListener::new(1000.0, 2000.0));
        let left = at(&mut world, -100.0, blip());
        ring(&mut world, left);
        step(&mut world, &mut schedule);
        let handle = world.resource::<SoundRegistry>().get_by_id("blip").unwrap();
        let mut today = AudioCommands::new();
        today.play_with(handle, 1.0, false);
        assert_eq!(world.resource_mut::<AudioCommands>().drain(), today.drain());
    }

    fn shakes(world: &World) -> Vec<f32> {
        world
            .resource::<EventQueue<ShakeEvent>>()
            .iter()
            .map(|shake| shake.trauma_add)
            .collect()
    }

    #[test]
    fn a_shake_falls_off_from_the_listener() {
        let (mut world, mut schedule) = rig();
        world.insert_resource(CueListener::new(10.0, 100.0));
        let blast = || Cues::new().on("rang", [CueAction::shake(0.8).with_falloff(100.0, 500.0)]);
        let near = at(&mut world, 50.0, blast());
        let middle = at(&mut world, 300.0, blast());
        let far = at(&mut world, 600.0, blast());
        for entity in [near, middle, far] {
            ring(&mut world, entity);
        }
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
        assert_eq!(shakes(&world), [0.8, 0.4], "whole, half, none");
        world.flush_events();
        world.flush_events();
        world.remove_resource::<CueListener>();
        ring(&mut world, far);
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
        assert_eq!(shakes(&world), [0.8], "whole with no listener");
        let number: CueAction = serde_json::from_value(json!({ "shake": 0.5 })).unwrap();
        assert_eq!(number, CueAction::shake(0.5));
        assert_eq!(
            serde_json::to_value(&number).unwrap(),
            json!({ "shake": 0.5 })
        );
        let falloff: CueAction =
            serde_json::from_value(json!({ "shake": { "trauma": 0.8, "near": 100, "far": 500 } }))
                .unwrap();
        assert_eq!(falloff, CueAction::shake(0.8).with_falloff(100.0, 500.0));
        for bad in [
            json!({ "shake": { "trauma": 0.8, "near": 500, "far": 100 } }),
            json!({ "shake": { "trauma": -1 } }),
            json!({ "shake": -1 }),
        ] {
            assert!(
                serde_json::from_value::<CueAction>(bad.clone()).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_loop_follows_its_subject_and_stops() {
        let (mut world, mut schedule) = rig();
        world.insert_resource(CueListener::new(100.0, 500.0));
        let quiet = world.spawn();
        for _ in 0..60 {
            ring(&mut world, quiet);
            step(&mut world, &mut schedule);
        }
        assert!(heard(&mut world).is_empty(), "no loop, no audio command");
        let brazier = at(
            &mut world,
            0.0,
            Cues::new()
                .on(
                    "rang",
                    [CueAction::looping("blip"), CueAction::looping("blip")],
                )
                .on("hushed", [CueAction::stop_loop("blip").with_fade(0.5)]),
        );
        ring(&mut world, brazier);
        step(&mut world, &mut schedule);
        assert_eq!(
            heard(&mut world),
            [Heard::Play {
                volume: 1.0,
                looping: true,
                fade: 0.1,
                pan: 0.0
            }],
            "one voice from two loops"
        );
        // Away from the listener, the voice follows the gain; under 1/64
        // no command goes.
        world.get_mut::<Position>(brazier).unwrap().0.x = 300.0;
        step(&mut world, &mut schedule);
        assert_eq!(heard(&mut world), [Heard::Volume(0.25)]);
        world.get_mut::<Position>(brazier).unwrap().0.x = 301.0;
        step(&mut world, &mut schedule);
        assert!(heard(&mut world).is_empty(), "under 1/64");
        world.get_mut::<Position>(brazier).unwrap().0.x = 460.0;
        step(&mut world, &mut schedule);
        assert_eq!(heard(&mut world), [Heard::Volume(0.010_000_004)]);
        world.get_mut::<Position>(brazier).unwrap().0.x = 600.0;
        step(&mut world, &mut schedule);
        assert_eq!(
            heard(&mut world),
            [Heard::Volume(0.0)],
            "to 0, though under 1/64"
        );
        hush(&mut world, brazier);
        step(&mut world, &mut schedule);
        assert_eq!(heard(&mut world), [Heard::Stop(0.5)], "its stop's fade");
        // Despawned, its want lets go over 0.1 s.
        ring(&mut world, brazier);
        step(&mut world, &mut schedule);
        assert_eq!(heard(&mut world).len(), 1);
        world.despawn(brazier);
        step(&mut world, &mut schedule);
        assert_eq!(heard(&mut world), [Heard::Stop(0.1)]);
        assert!(world.resource::<CueLoops>().is_idle());
    }

    #[test]
    fn subjects_share_a_sounds_voice() {
        let (mut world, mut schedule) = rig();
        let handle = world.resource::<SoundRegistry>().get_by_id("blip").unwrap();
        world
            .resource_mut::<SoundRegistry>()
            .set_max_voices(handle, 1);
        world.insert_resource(CueListener::new(100.0, 500.0).with_pan(200.0));
        let looper = |volume: f32| {
            Cues::new()
                .on("rang", [CueAction::looping("blip").with_volume(volume)])
                .on("hushed", [CueAction::stop_loop("blip")])
        };
        let near = at(&mut world, -40.0, looper(0.5));
        let far = at(&mut world, 600.0, looper(1.0));
        ring(&mut world, near);
        ring(&mut world, far);
        step(&mut world, &mut schedule);
        assert_eq!(
            heard(&mut world),
            [
                Heard::Play {
                    volume: 0.5,
                    looping: true,
                    fade: 0.1,
                    pan: 0.0
                },
                Heard::Pan(quantized(-0.2))
            ],
            "one voice: the want of 1 past hearing, the 0.5 at the listener heard"
        );
        // The near one comes loud: the voice follows it, panned to it.
        world.get_mut::<Position>(far).unwrap().0.x = 200.0;
        step(&mut world, &mut schedule);
        assert_eq!(heard(&mut world), [Heard::Volume(0.5625), Heard::Pan(1.0)]);
        // The louder's stop hands the voice to the other, no new play.
        hush(&mut world, far);
        step(&mut world, &mut schedule);
        assert_eq!(
            heard(&mut world),
            [Heard::Volume(0.5), Heard::Pan(quantized(-0.2))]
        );
        hush(&mut world, near);
        step(&mut world, &mut schedule);
        assert_eq!(
            heard(&mut world),
            [Heard::Stop(0.1)],
            "the last stop fades it"
        );
    }

    /// What a restored spawn takes, as a state's marker.
    #[derive(Debug, Clone, Copy)]
    struct Marker;

    #[test]
    fn a_new_run_ends_the_old_runs_loops() {
        use tungsten_core::save::{self, capture_world, restore_world};

        let (mut world, mut schedule) = rig();
        let fire = || Cues::new().on("rang", [CueAction::looping("blip")]);
        let brazier = at(&mut world, 0.0, fire());
        save::name(&mut world, brazier, None, "brazier").unwrap();
        ring(&mut world, brazier);
        step(&mut world, &mut schedule);
        assert_eq!(heard(&mut world).len(), 1, "playing at the capture");
        let saved = capture_world(&world).unwrap();
        // The run ends and a new one spawns; the capture goes back on it.
        world.despawn(brazier);
        let again = at(&mut world, 0.0, fire());
        save::name(&mut world, again, None, "brazier").unwrap();
        restore_world(&mut world, &saved, &(Marker,)).unwrap();
        step(&mut world, &mut schedule);
        assert_eq!(
            heard(&mut world),
            [Heard::Stop(0.1)],
            "the old voice fades out"
        );
    }
}

#[test]
fn an_unknown_world_cue_warns_once() {
    use crate::foundation::trace::KitTrace;
    use crate::react::world_cues::WorldCue;

    let (mut world, _) = rig();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut reactor = Reactor::default();
    for _ in 0..3 {
        WorldCue::new("nope", Vec2::ZERO).play(&mut world);
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        reactor.run(&mut world);
        world.flush_events();
    }
    assert_eq!(reactor.unknown_cues.len(), 1, "warned once");
    let skipped = world
        .resource::<KitTrace>()
        .about(Entity::PLACEHOLDER)
        .filter(|entry| {
            matches!(
                entry.kind,
                TraceKind::Skipped {
                    why: NO_WORLD_CUE,
                    ..
                }
            )
        })
        .count();
    assert_eq!(skipped, 3, "each play traced, nothing played");
}

/// 7.10p's named game events.
mod named {
    use super::*;

    /// The game's chest event: opened, emptied or named another way.
    struct Chest(Entity, Option<&'static str>);

    #[allow(clippy::unnecessary_wraps)]
    fn chest(event: &Chest) -> Option<CueEvent> {
        let at = CueEvent::of(event.0);
        Some(match event.1 {
            Some(name) => at.named(name),
            None => at,
        })
    }

    /// A rig with `opened` and `emptied` from one registration, and
    /// `knocked` from `new`, whose map names what the event says.
    fn rig() -> (World, Schedule) {
        let (mut world, _) = super::rig();
        world.insert_resource(Time::new());
        world.insert_resource(CommandBuffer::new());
        let mut schedule = Schedule::new();
        CuesPlugin.build(&mut schedule, &mut world);
        CuesOnEvent::<Chest>::with_names(&["opened", "emptied"], chest)
            .build(&mut schedule, &mut world);
        CuesOnEvent::<Knock>::new("knocked", knock).build(&mut schedule, &mut world);
        schedule.resolve().unwrap();
        (world, schedule)
    }

    /// A knock that may name another event.
    struct Knock(Entity, Option<&'static str>);

    #[allow(clippy::unnecessary_wraps)]
    fn knock(event: &Knock) -> Option<CueEvent> {
        let at = CueEvent::of(event.0);
        Some(match event.1 {
            Some(name) => at.named(name),
            None => at,
        })
    }

    fn step(world: &mut World, schedule: &mut Schedule) {
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        schedule.run_stage(tungsten_core::Stage::FixedUpdate, world, |_, _| {});
        world.flush_events();
    }

    fn count(world: &World, entity: Entity, name: &str) -> i64 {
        world
            .get::<Counters>(entity)
            .map_or(0, |counters| counters.get(name))
    }

    #[test]
    fn one_registration_brings_many_names() {
        let (mut world, mut schedule) = rig();
        let names = world.resource::<CueEventNames>();
        assert!(names.get("opened").is_some() && names.get("emptied").is_some());
        let coffer = world.spawn_with((Cues::new()
            .on("opened", [CueAction::counter_add("opened", 1)])
            .on("emptied", [CueAction::counter_add("emptied", 1)])
            .on("knocked", [CueAction::counter_add("knocked", 1)]),));
        let send = |world: &mut World, name: Option<&'static str>| {
            world
                .resource_mut::<EventQueue<Chest>>()
                .send(Chest(coffer, name));
        };
        send(&mut world, Some("opened"));
        send(&mut world, Some("opened"));
        send(&mut world, Some("emptied"));
        step(&mut world, &mut schedule);
        assert_eq!(
            (
                count(&world, coffer, "opened"),
                count(&world, coffer, "emptied")
            ),
            (2, 1),
            "each name's cues for its own occurrences"
        );
        // A name not its own, twice, and none: each skipped, logged once.
        send(&mut world, Some("closed"));
        send(&mut world, Some("closed"));
        send(&mut world, None);
        step(&mut world, &mut schedule);
        assert_eq!(
            (
                count(&world, coffer, "opened"),
                count(&world, coffer, "emptied")
            ),
            (2, 1),
            "runs nothing"
        );
        // Under `new`, no name is its one; its own name runs; another is
        // skipped.
        for name in [None, Some("knocked"), Some("opened")] {
            world
                .resource_mut::<EventQueue<Knock>>()
                .send(Knock(coffer, name));
        }
        step(&mut world, &mut schedule);
        assert_eq!(count(&world, coffer, "knocked"), 2);
        assert_eq!(count(&world, coffer, "opened"), 2, "not another's");
    }

    #[test]
    #[should_panic(expected = "the cue event `opened` is already registered")]
    fn a_name_given_twice_panics() {
        let (mut world, mut schedule) = super::rig();
        CuesOnEvent::<Chest>::with_names(&["opened", "opened"], chest)
            .build(&mut schedule, &mut world);
    }

    #[test]
    #[should_panic(expected = "is one of the kit's cue events")]
    fn a_controller_event_name_is_the_kits() {
        let (mut world, mut schedule) = super::rig();
        CuesOnEvent::<Chest>::with_names(&["opened", "landed"], chest)
            .build(&mut schedule, &mut world);
    }

    #[test]
    fn a_controller_event_is_named_by_its_cues_or_a_rule() {
        let (mut world, _) = super::rig();
        let hero =
            world.spawn_with((Cues::new().on(LANDED, [CueAction::counter_add("landings", 1)]),));
        let other = world.spawn();
        assert!(!cue_named(&world, hero, JUMPED), "nothing names it yet");
        let rules: CueRules = serde_json::from_value(json!([
            { "on": "jumped", "do": [{ "counter": { "name": "jumps", "add": 1 } }] }
        ]))
        .unwrap();
        world.resource_mut::<DataRegistry>().insert("hops", rules);
        for entity in [hero, other] {
            assert!(cue_named(&world, entity, JUMPED), "a rule names it");
            assert!(!cue_named(&world, entity, JUMP_REFUSED), "nothing names it");
        }
        assert!(cue_named(&world, hero, LANDED), "its own cues");
        assert!(!cue_named(&world, other, LANDED), "another's cues");
        for (event, carries) in [
            (AIR_JUMPED, true),
            (LANDED, true),
            (JUMPED, false),
            (JUMP_REFUSED, false),
        ] {
            let checked = check_rules(
                &mut world,
                json!([{ "on": event, "amount": { "min": 100 }, "do": [] }]),
            );
            if carries {
                assert!(checked.is_ok(), "{event}: {checked:?}");
            } else {
                let error = checked.unwrap_err();
                assert!(error.ends_with("at 0.amount"), "{event}: {error}");
            }
        }
    }
}

/// 7.10c's ability events: pushed only when named, and filtered by an
/// ability's name and a refusal's reason.
mod ability_events {
    use super::*;
    use tungsten_core::Stage;

    use crate::foundation::trace::KitTrace;
    use crate::intent::action_input::Intent;
    use crate::{AbilitiesPlugin, Ability, CountersPlugin, Payload, StatsPlugin, Trigger};

    /// One step at 1/60 s, then the flushes that end the frame.
    fn step(world: &mut World, schedule: &mut Schedule) {
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
        world.resource_mut::<Time>().leave_fixed_steps();
        let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
        world.flush_reusing(&mut commands);
        world.insert_resource(commands);
        world.flush_events();
    }

    /// The cue rig with time, a command buffer, a trace, stats, counters
    /// and abilities: the `bell` (a 3-step cooldown), the `flask` (one
    /// charge, refilled after 3 steps) and the `horn` (held on `horn`). With
    /// `reacting`, the cue system runs each step; without, its list keeps
    /// what the abilities push.
    fn rig(reacting: bool) -> (World, Schedule) {
        let (mut world, _) = super::rig();
        world.insert_resource(Time::new());
        world.insert_resource(CommandBuffer::new());
        world.insert_resource(KitTrace::default());
        let mut schedule = Schedule::new();
        StatsPlugin::new().build(&mut schedule, &mut world);
        CountersPlugin.build(&mut schedule, &mut world);
        AbilitiesPlugin.build(&mut schedule, &mut world);
        if reacting {
            CuesPlugin.build(&mut schedule, &mut world);
        }
        schedule.resolve().unwrap();
        let step = 1.0 / 60.0;
        let data = world.resource_mut::<DataRegistry>();
        data.insert(
            "bell",
            Ability::new()
                .with_cooldown(3.0 * step)
                .with_payload(Payload::Hitbox(0.1)),
        );
        data.insert(
            "flask",
            Ability::new()
                .with_charges(1, 3.0 * step, false)
                .with_payload(Payload::Hitbox(0.1)),
        );
        data.insert(
            "horn",
            Ability::new()
                .with_trigger(Trigger::Held("horn".to_string()))
                .with_payload(Payload::Hitbox(0.1)),
        );
        (world, schedule)
    }

    /// A user of the three, with `cues`; empty ones name nothing.
    fn user(world: &mut World, cues: Cues) -> Entity {
        let abilities = ["bell", "flask", "horn"]
            .into_iter()
            .fold(Abilities::new(), |abilities, id| {
                abilities.with(DataRef::by_id(id))
            });
        world.spawn_with((abilities, Intent::default(), Counters::new(), cues))
    }

    /// The pushed ability cues: each event, entity and reason.
    fn pushed(world: &mut World) -> Vec<(&'static str, Entity, u8)> {
        world
            .resource_mut::<KitCues>()
            .0
            .drain(..)
            .map(|cue| (cue.event, cue.entity, cue.reason))
            .collect()
    }

    /// Refuses the bell (not ready) and the flask (no charge), holds and
    /// releases the horn, and steps until both turn ready: what `user`'s
    /// abilities push, by step.
    fn run_through(
        world: &mut World,
        schedule: &mut Schedule,
        user: Entity,
    ) -> Vec<(&'static str, Entity, u8)> {
        let mut all = Vec::new();
        for name in ["bell", "flask"] {
            assert_eq!(use_ability(world, user, name), Used::Used);
            assert_ne!(use_ability(world, user, name), Used::Used);
        }
        all.extend(pushed(world));
        for i in 1..=5 {
            world
                .get_mut::<Intent>(user)
                .unwrap()
                .set_slot("horn", i <= 2, i == 1);
            step(world, schedule);
            all.extend(pushed(world));
        }
        all
    }

    #[test]
    fn ability_events_push_only_what_is_named() {
        let (mut world, mut schedule) = rig(false);
        let capacity = world.resource::<KitCues>().0.capacity();
        assert!(capacity >= 32, "reserved: {capacity}");
        // Nothing names them: nothing pushed.
        let quiet = user(&mut world, Cues::new());
        assert_eq!(run_through(&mut world, &mut schedule, quiet), []);
        // Its own cues name `ability_held` alone.
        let held = user(
            &mut world,
            Cues::new().on(ABILITY_HELD, [CueAction::counter_add("held", 1)]),
        );
        assert_eq!(
            run_through(&mut world, &mut schedule, held),
            [(ABILITY_HELD, held, 0)]
        );
        // A rule on each names it for every entity, whatever its filters.
        let rules: CueRules = serde_json::from_value(json!([
            { "on": "ability_refused", "filter": { "all": ["team.enemy"] }, "do": [] },
            { "on": "ability_ready", "do": [] },
            { "on": "ability_released", "do": [] }
        ]))
        .unwrap();
        world.resource_mut::<DataRegistry>().insert("all", rules);
        let named = user(&mut world, Cues::new());
        assert_eq!(
            run_through(&mut world, &mut schedule, named),
            [
                (ABILITY_REFUSED, named, 1),
                (ABILITY_REFUSED, named, 2),
                (ABILITY_READY, named, 0),
                (ABILITY_READY, named, 0),
                (ABILITY_RELEASED, named, 0),
            ],
            "the bell not ready, the flask out of charges; at step 3 both ready, in list order, \
             then the horn let go"
        );
        assert_eq!(
            world.resource::<KitCues>().0.capacity(),
            capacity,
            "the pushes stayed in the reserve"
        );
    }

    #[test]
    fn a_rule_filters_ability_events_by_name_and_reason() {
        let (mut world, mut schedule) = rig(true);
        let count =
            |name: &str| json!([{ "counter": { "name": name, "add": 1, "target": "game" } }]);
        let filters: CueRules = serde_json::from_value(json!([
            { "on": "ability_used", "ability": "bell", "do": count("bells") },
            { "on": "ability_refused", "reason": "no_charge", "do": count("dry") },
            { "on": "ability_refused", "ability": "bell", "reason": "not_ready", "do": count("bell_wait") },
            { "on": "ability_ready", "ability": "flask", "do": count("flask_ready") },
            { "on": "ability_held", "ability": "horn", "do": count("horn_down") }
        ]))
        .unwrap();
        world
            .resource_mut::<DataRegistry>()
            .insert("filters", filters);
        let hero = user(&mut world, Cues::new());
        let game = |world: &World, name: &str| world.resource::<Counters>().get(name);
        let mut seen = Vec::new();
        for (i, name) in ["bell", "flask", "bell", "flask"].into_iter().enumerate() {
            use_ability(&mut world, hero, name);
            if i == 3 {
                world
                    .get_mut::<Intent>(hero)
                    .unwrap()
                    .set_slot("horn", true, true);
            }
            step(&mut world, &mut schedule);
            seen.push(
                ["bells", "dry", "bell_wait", "flask_ready", "horn_down"].map(|n| game(&world, n)),
            );
        }
        assert_eq!(
            seen,
            [
                [1, 0, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [1, 0, 1, 0, 0],
                [1, 1, 1, 1, 1],
            ],
            "the bell's use alone; its wait as not ready, the flask's as no charge; the flask \
             ready after its three-step refill; the horn's hold by its name"
        );
        // The load refuses a filter its event cannot carry.
        for (rule, field) in [
            (
                json!({ "on": "damaged", "ability": "bell", "do": [] }),
                "ability",
            ),
            (
                json!({ "on": "ability_used", "reason": "no_charge", "do": [] }),
                "reason",
            ),
        ] {
            let error = check_rules(&mut world, json!([rule])).unwrap_err();
            assert!(error.ends_with(&format!("at 0.{field}")), "{error}");
        }
        for rule in [
            json!({ "on": "ability_refused", "reason": "no_mana", "do": [] }),
            json!({ "on": "ability_ready", "ability": "", "do": [] }),
        ] {
            assert!(rules(json!([rule])).is_err(), "{rule}");
        }
    }
}

/// 7.10c's centred spawns: a `prefab` or `roll` action's prefab centred on
/// the event's position.
mod centred_spawns {
    use std::path::PathBuf;

    use super::*;
    use tungsten_core::assets::{FilterMode, TextureHandle, UvRect};
    use tungsten_core::{AssetRegistry, Stage, Tag, Transform};

    use crate::react::loot::WeightedTable;

    #[test]
    fn a_centred_prefab_or_roll_lands_its_centre_on_the_event() {
        let (mut world, _) = super::rig();
        world.insert_resource(Time::new());
        world.insert_resource(CommandBuffer::new());
        world.register_event::<Struck>();
        let mut schedule = Schedule::new();
        CuesPlugin.build(&mut schedule, &mut world);
        schedule.resolve().unwrap();
        // A 10 × 6 mote, drawn from its top-left: its centre (5, 3) on.
        let mut assets = AssetRegistry::new();
        assets.register_sprite(
            "mote".to_string(),
            FilterMode::Nearest,
            10,
            6,
            PathBuf::from("mote.png"),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        );
        world.insert_resource(assets);
        let mote: PrefabData =
            serde_json::from_value(json!({ "tag": "mote", "sprite": { "asset_id": "mote" } }))
                .unwrap();
        world
            .resource_mut::<PrefabRegistry>()
            .insert("mote".to_string(), mote);
        let table = || DataRef::inline(WeightedTable::new().with_entry("mote", 1.0));
        let actions: Vec<CueAction> = serde_json::from_value(json!([
            { "prefab": "mote" },
            { "prefab": { "id": "mote", "centred": true } },
            { "roll": { "table": { "entries": [{ "prefab": "mote", "weight": 1 }] }, "centred": true } }
        ]))
        .unwrap();
        assert_eq!(
            actions,
            [
                CueAction::prefab("mote"),
                CueAction::prefab("mote").centred(),
                CueAction::roll(table()).centred(),
            ]
        );
        assert_eq!(
            serde_json::to_value(&actions).unwrap(),
            json!([
                { "prefab": "mote" },
                { "prefab": { "id": "mote", "centred": true } },
                { "roll": { "table": { "rolls": 1, "entries": [{ "prefab": "mote", "weight": 1.0 }] }, "centred": true } }
            ]),
            "an uncentred action keeps its ID form"
        );
        let bell = world.spawn_with((
            Transform::from_position(Vec2::new(50.0, 50.0)),
            Cues::new().on(HIT, actions),
        ));
        let target = world.spawn();
        world.resource_mut::<EventQueue<Struck>>().send(Struck::new(
            bell,
            target,
            Vec2::new(20.0, 30.0),
            1.0,
        ));
        world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
        schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
        let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
        world.flush_reusing(&mut commands);
        world.insert_resource(commands);
        let mut placed: Vec<(u32, Vec2)> = world
            .query::<(Entity, &Tag, &Transform)>()
            .filter(|(_, tag, _)| tag.name == "mote")
            .map(|(entity, _, transform)| (entity.id(), transform.position))
            .collect();
        placed.sort_by_key(|&(id, _)| id);
        let points: Vec<Vec2> = placed.into_iter().map(|(_, at)| at).collect();
        assert_eq!(
            points,
            [
                Vec2::new(20.0, 30.0),
                Vec2::new(15.0, 27.0),
                Vec2::new(15.0, 27.0)
            ],
            "the plain prefab's point on the hit's position; the centred prefab's and the \
             roll's centre (5, 3) on"
        );
        let refused = std::panic::catch_unwind(|| CueAction::flash().centred());
        assert!(refused.is_err(), "a flash spawns nothing");
    }
}
