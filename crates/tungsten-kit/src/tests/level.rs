use super::*;
use serde_json::json;
use tungsten_core::{CommandBuffer, ComponentRegistry, Schedule, Stage, Time};

use crate::damage::effects::{Effects, EffectsPlugin};
use crate::foundation::tags::{Tags, TagsPlugin};
use crate::foundation::trace::KitTrace;
use crate::intent::ability::{AbilitiesPlugin, Payload, use_ability};
use crate::react::counters::CountersPlugin;
use crate::react::cues::{CueAction, Cues, CuesPlugin};
use crate::{KIT_REACT, Used};

const STEP: f32 = 1.0 / 60.0;

/// A World with time, a command buffer, a trace, tags, counters, effects,
/// abilities, cues and levels.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["status.swift"])
        .build(&mut schedule, &mut world);
    CountersPlugin.build(&mut schedule, &mut world);
    EffectsPlugin.build(&mut schedule, &mut world);
    AbilitiesPlugin.build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    LevelPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(STEP);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

fn level_of(world: &World, entity: Entity) -> u32 {
    world.get::<Level>(entity).unwrap().level()
}

fn ups(world: &World) -> Vec<u32> {
    world
        .resource::<EventQueue<LevelUp>>()
        .iter()
        .map(|event| event.level)
        .collect()
}

fn xp_curve() -> DataRef<Curve> {
    DataRef::inline(Curve::new([
        (1.0, 0.0),
        (2.0, 10.0),
        (3.0, 25.0),
        (4.0, 50.0),
    ]))
}

#[test]
fn the_plugin_registers_its_kinds_queue_and_system_before_cues() {
    let (world, schedule) = rig();
    assert!(world.has_event::<LevelUp>());
    let data = world.resource::<DataRegistry>();
    assert_eq!(data.kind_of::<Offers>(), Some(OFFERS));
    assert_eq!(data.kind_of::<Curve>(), Some(CURVES));
    assert!(
        world
            .resource::<ComponentRegistry>()
            .names()
            .any(|n| n == LEVEL)
    );
    let names: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| names.iter().position(|n| *n == name).unwrap();
    assert!(at(KIT_REACT) < at(LEVEL_TICK) && at(LEVEL_TICK) < at(CUES));
}

#[test]
fn two_levels_in_one_step_from_one_count_and_the_top_holds() {
    let (mut world, mut schedule) = rig();
    let hero = world.spawn_with((Level::new("xp", xp_curve()), Counters::new().with("xp", 30)));
    assert_eq!(world.get::<Level>(hero).unwrap().top(), 4);
    step(&mut world, &mut schedule);
    assert_eq!(level_of(&world, hero), 3);
    assert_eq!(ups(&world), [2, 3], "one event per level");
    world.get_mut::<Counters>(hero).unwrap().set("xp", 1_000);
    step(&mut world, &mut schedule);
    assert_eq!(level_of(&world, hero), 4, "the top");
    step(&mut world, &mut schedule);
    assert_eq!(level_of(&world, hero), 4);
    // A count that falls does not lower the level.
    world.get_mut::<Counters>(hero).unwrap().set("xp", 0);
    step(&mut world, &mut schedule);
    assert_eq!(level_of(&world, hero), 4);
    let trace = world.resource::<KitTrace>();
    let levels: Vec<f32> = trace
        .about(hero)
        .filter_map(|entry| match entry.kind {
            TraceKind::Event {
                name: "level_up",
                amount,
                ..
            } => amount,
            _ => None,
        })
        .collect();
    assert_eq!(levels, [2.0, 3.0, 4.0]);
}

#[test]
fn twenty_levels_climb_sixteen_in_the_first_step_and_four_in_the_next() {
    let (mut world, mut schedule) = rig();
    // Level n needs n − 1 counts, up to level 50.
    let curve = DataRef::inline(Curve::new([(1.0, 0.0), (50.0, 49.0)]));
    let hero = world.spawn_with((Level::new("xp", curve), Counters::new().with("xp", 20)));
    step(&mut world, &mut schedule);
    assert_eq!(level_of(&world, hero), 17, "sixteen a step at most");
    assert_eq!(ups(&world).len(), 16);
    step(&mut world, &mut schedule);
    assert_eq!(level_of(&world, hero), 21);
    step(&mut world, &mut schedule);
    assert_eq!(level_of(&world, hero), 21);
}

#[test]
fn a_game_counter_climbs_the_level_too() {
    let (mut world, mut schedule) = rig();
    let party = world.spawn_with((Level::new("score", xp_curve()).with_game_counter(),));
    world.resource_mut::<Counters>().set("score", 12);
    step(&mut world, &mut schedule);
    assert_eq!(level_of(&world, party), 2);
    assert!(world.get::<Level>(party).unwrap().reads_game());
    // A missing counter reads 0; a missing curve climbs nothing.
    let lost = world.spawn_with((Level::new("xp", DataRef::by_id("nowhere")),));
    step(&mut world, &mut schedule);
    assert_eq!(level_of(&world, lost), 1);
    assert_eq!(world.get::<Level>(lost).unwrap().top(), 1);
}

#[test]
fn offers_are_drawn_distinct_by_weight_under_a_seed_and_fewer_from_a_short_list() {
    let offers = Offers::new(2)
        .with_effect(10.0, DataRef::by_id("common"))
        .with_effect(1.0, DataRef::by_id("rare"))
        .with_ability(1.0, DataRef::by_id("wand"));
    let mut commons = 0;
    for seed in 0..200 {
        let mut rng = KitRng::seeded(seed);
        let drawn = offers.draw(&mut rng);
        assert_eq!(drawn.len(), 2);
        assert_ne!(drawn[0], drawn[1], "distinct");
        if drawn[0].effect().and_then(|e| e.id()) == Some("common") {
            commons += 1;
        }
        assert_eq!(rng.draws(), 2, "one number per draw");
    }
    assert!(
        commons > 140,
        "the heavy entry comes first most of the time: {commons}"
    );
    let mut a = KitRng::seeded(9);
    let mut b = KitRng::seeded(9);
    assert_eq!(
        offers.draw(&mut a),
        offers.draw(&mut b),
        "one seed, one draw"
    );
    let short = Offers::new(5).with_effect(1.0, DataRef::by_id("only"));
    let drawn = short.draw(&mut KitRng::seeded(1));
    assert_eq!(drawn.len(), 1, "fewer than count from a short list");
    assert!(Offers::new(1).draw(&mut KitRng::seeded(1)).is_empty());
    assert!(std::panic::catch_unwind(|| Offers::new(0)).is_err());
    assert!(
        std::panic::catch_unwind(|| Offers::new(1).with_effect(0.0, DataRef::by_id("x"))).is_err()
    );
}

#[test]
fn a_choice_applies_an_effect_or_grants_an_ability_and_an_owned_one_refills() {
    let (mut world, mut schedule) = rig();
    world.resource_mut::<DataRegistry>().insert(
        "swift",
        Effect::new()
            .with_duration(2.0)
            .with_grant(["status.swift"]),
    );
    world.resource_mut::<DataRegistry>().insert(
        "dash",
        Ability::new()
            .with_cooldown(5.0)
            .with_charges(2, 5.0, false)
            .with_payload(Payload::Hitbox(0.1)),
    );
    let offers = DataRef::inline(
        Offers::new(2)
            .with_effect(1.0, DataRef::by_id("swift"))
            .with_ability(1.0, DataRef::by_id("dash")),
    );
    let hero = world.spawn_with((
        Level::new("xp", xp_curve()).with_offers(offers),
        Counters::new().with("xp", 10),
    ));
    step(&mut world, &mut schedule);
    let level = world.get::<Level>(hero).unwrap();
    assert_eq!(level.level(), 2);
    assert_eq!(level.offers().len(), 2, "both entries, the list is short");
    let effect_at = level
        .offers()
        .iter()
        .position(|offer| offer.effect().is_some())
        .unwrap();
    let ability_at = 1 - effect_at;
    assert!(
        !world.get_mut::<Level>(hero).unwrap().choose(2),
        "past the end"
    );
    assert!(world.get_mut::<Level>(hero).unwrap().choose(effect_at));
    assert_eq!(world.get::<Level>(hero).unwrap().chosen(), Some(effect_at));
    step(&mut world, &mut schedule);
    assert!(
        world.get::<Level>(hero).unwrap().offers().is_empty(),
        "cleared"
    );
    assert!(world.get::<Level>(hero).unwrap().chosen().is_none());
    assert!(
        world.get::<Effects>(hero).is_none(),
        "queued at the react phase's tick, after this step's resolve"
    );
    step(&mut world, &mut schedule);
    assert!(
        world.get::<Effects>(hero).unwrap().has("swift"),
        "applied at the next step's resolve"
    );
    assert!(world.get::<Tags>(hero).unwrap().has("status.swift"));
    // The next level up offers again; choosing the ability grants it.
    world.get_mut::<Counters>(hero).unwrap().set("xp", 25);
    step(&mut world, &mut schedule);
    assert_eq!(world.get::<Level>(hero).unwrap().offers().len(), 2);
    let ability_at = world
        .get::<Level>(hero)
        .unwrap()
        .offers()
        .iter()
        .position(|offer| offer.ability().is_some())
        .unwrap_or(ability_at);
    assert!(world.get_mut::<Level>(hero).unwrap().choose(ability_at));
    step(&mut world, &mut schedule);
    let abilities = world.get::<Abilities>(hero).unwrap();
    assert_eq!(
        abilities.names().collect::<Vec<_>>(),
        ["dash"],
        "inserted if absent"
    );
    assert_eq!(use_ability(&mut world, hero, "dash"), Used::Used);
    assert_eq!(use_ability(&mut world, hero, "dash"), Used::NotReady);
    // An owned ability's offer refills it with its cooldown cleared.
    world.get_mut::<Counters>(hero).unwrap().set("xp", 50);
    step(&mut world, &mut schedule);
    let ability_at = world
        .get::<Level>(hero)
        .unwrap()
        .offers()
        .iter()
        .position(|offer| offer.ability().is_some())
        .unwrap();
    assert!(world.get_mut::<Level>(hero).unwrap().choose(ability_at));
    step(&mut world, &mut schedule);
    let abilities = world.get::<Abilities>(hero).unwrap();
    assert_eq!(abilities.names().count(), 1, "still one entry");
    assert!(abilities.ready("dash"), "refilled, cooldown cleared");
    assert_eq!(use_ability(&mut world, hero, "dash"), Used::Used);
}

#[test]
fn a_second_level_up_replaces_pending_offers_and_cues_see_level_up() {
    let (mut world, mut schedule) = rig();
    let offers = DataRef::inline(
        Offers::new(1)
            .with_effect(1.0, DataRef::inline(Effect::new()))
            .with_effect(1.0, DataRef::inline(Effect::new().with_duration(1.0))),
    );
    let hero = world.spawn_with((
        Level::new("xp", xp_curve()).with_offers(offers),
        Counters::new().with("xp", 10),
        Cues::new().on("level_up", [CueAction::counter_add("ups", 1)]),
    ));
    step(&mut world, &mut schedule);
    let first = world.get::<Level>(hero).unwrap().offers().to_vec();
    assert_eq!(first.len(), 1);
    assert!(world.get_mut::<Level>(hero).unwrap().choose(0));
    world.get_mut::<Counters>(hero).unwrap().set("xp", 25);
    step(&mut world, &mut schedule);
    let level = world.get::<Level>(hero).unwrap();
    assert_eq!(level.level(), 3);
    assert_eq!(level.offers().len(), 1, "replaced by the new level's draw");
    assert!(
        level.chosen().is_none(),
        "the choice applied, then the draw replaced the rest"
    );
    assert_eq!(
        world.get::<Counters>(hero).unwrap().get("ups"),
        2,
        "the cue ran for each level up, in the level's step"
    );
    assert_eq!(
        serde_json::to_value(CueAction::counter_add("ups", 1)).unwrap()["counter"]["name"],
        "ups"
    );
}

#[test]
fn the_component_saves_its_level_offers_and_choice_and_the_load_checks_its_data() {
    let (mut world, mut schedule) = rig();
    world
        .resource_mut::<DataRegistry>()
        .insert("hero_xp", Curve::new([(1.0, 0.0), (2.0, 10.0)]));
    world.resource_mut::<DataRegistry>().insert(
        "hero_offers",
        Offers::new(1).with_effect(1.0, DataRef::inline(Effect::new())),
    );
    world
        .resource_mut::<DataRegistry>()
        .insert("swift", Effect::new());
    let hero = world.spawn_with((
        Level::new("xp", DataRef::by_id("hero_xp")).with_offers(DataRef::by_id("hero_offers")),
        Counters::new().with("xp", 10),
    ));
    step(&mut world, &mut schedule);
    assert!(world.get_mut::<Level>(hero).unwrap().choose(0));
    let level = world.get::<Level>(hero).unwrap().clone();
    assert_eq!(level.top(), 1, "the top of an ID curve needs the data");
    let saved = serde_json::to_value(&level).unwrap();
    assert_eq!(saved["level"], 2);
    assert_eq!(saved["curve"], "hero_xp");
    assert_eq!(saved["chosen"], 0);
    assert_eq!(saved["pending"].as_array().unwrap().len(), 1);
    let read: Level = serde_json::from_value(saved).unwrap();
    assert_eq!(read, level);
    assert_eq!(read.counter(), "xp");
    assert_eq!(read.curve().id(), Some("hero_xp"));
    for bad in [
        json!({ "counter": "Xp", "curve": "hero_xp" }),
        json!({ "counter": "xp", "curve": "hero_xp", "level": 0 }),
        json!({ "counter": "xp", "curve": "hero_xp", "chosen": 0 }),
        json!({ "counter": "xp", "curve": "hero_xp", "target": "other" }),
        json!({ "curve": "hero_xp" }),
    ] {
        assert!(
            serde_json::from_value::<Level>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    let rows = level.inspect_rows();
    assert_eq!(rows[0], ("level", "2 of 1, by xp".to_string()));
    assert_eq!(rows[1], ("offers", "effect inline (chosen)".to_string()));
    let game = Level::new("score", DataRef::by_id("hero_xp")).with_game_counter();
    assert_eq!(game.inspect_rows()[0].1, "1 of 1, by score (game)");
    // The scene component and the offers kind check their data.
    let components = world.resource::<ComponentRegistry>();
    let check = |value: serde_json::Value| {
        components
            .check_in(LEVEL, &value, &world)
            .map_err(|e| e.to_string())
    };
    assert!(check(json!({ "counter": "xp", "curve": "hero_xp", "offers": "hero_offers" })).is_ok());
    assert!(check(json!({ "counter": "xp", "target": "game", "curve": [[1, 0], [2, 5]] })).is_ok());
    assert_eq!(
        check(json!({ "counter": "xp", "curve": "nowhere" })).unwrap_err(),
        "component `level`, field `curve`: unknown curves `nowhere`"
    );
    assert_eq!(
        check(json!({ "counter": "xp", "curve": "hero_xp", "offers": "none" })).unwrap_err(),
        "component `level`, field `offers`: unknown offers `none`"
    );
    assert_eq!(
        check(json!({ "counter": "xp", "curve": "hero_xp", "offers": { "count": 1, "entries": [{ "weight": 1, "gives": { "effect": "gone" } }] } })).unwrap_err(),
        "component `level`, field `offers.entries.0.gives.effect`: unknown effects `gone`"
    );
    assert_eq!(
        check(json!({ "counter": "xp", "curve": "hero_xp", "offers": { "count": 1, "entries": [{ "weight": 1, "gives": { "ability": "gone" } }] } })).unwrap_err(),
        "component `level`, field `offers.entries.0.gives.ability`: unknown abilities `gone`"
    );
    assert!(check(json!({ "counter": "xp", "curve": "hero_xp", "offers": { "count": 1, "entries": [{ "weight": 1, "gives": { "effect": "swift" } }] } })).is_ok());
    for bad in [
        json!({ "count": 0 }),
        json!({ "count": 1, "entries": [{ "weight": 0, "gives": { "effect": "swift" } }] }),
        json!({ "count": 1, "entries": [{ "weight": 1, "gives": { "effect": "swift", "ability": "x" } }] }),
        json!({ "count": 1, "entries": [{ "weight": 1, "gives": {} }] }),
        json!({ "entries": [] }),
    ] {
        assert!(
            serde_json::from_value::<Offers>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    let offers: Offers = serde_json::from_value(json!({
        "count": 2,
        "entries": [{ "weight": 2, "gives": { "effect": "swift" } }, { "weight": 1, "gives": { "ability": "wand" } }]
    }))
    .unwrap();
    assert_eq!(
        offers,
        Offers::new(2)
            .with_effect(2.0, DataRef::by_id("swift"))
            .with_ability(1.0, DataRef::by_id("wand"))
    );
    assert_eq!(
        serde_json::from_value::<Offers>(serde_json::to_value(&offers).unwrap()).unwrap(),
        offers
    );
}
