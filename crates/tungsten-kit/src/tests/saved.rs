//! The kit's saved components, references and resources through a capture
//! and a restore into another World.

use std::collections::BTreeSet;

use glam::Vec2;
use serde_json::{Value, json};
use tungsten_core::physics::Position;
use tungsten_core::save::{
    self, SaveId, SaveRegistry, SaveState, WorldSave, capture_world, enter_area, restore_world,
};
use tungsten_core::{
    CommandBuffer, DataRef, DataRegistry, Entity, PrefabRegistry, Schedule, Stage, Tag, Time,
    Transform, World,
};

use super::*;
use crate::{
    Ability, ContactDamage, ContactTouch, Counters, Curve, DamageRequest, Effect, EffectRequest,
    Effects, Flags, HealRequest, Health, HitShape, Hitbox, KitDescriptions, KitPlugins, KitRng,
    Lifetime, Modifier, Source, SpawnChildren, Spawner, Stats, TagsPlugin,
};

const DT: f32 = 1.0 / 60.0;

/// What a restored spawn takes, as a state's marker.
#[derive(Debug, Clone, Copy)]
struct Probe;

/// A World with every kit plugin built, the tags the samples name
/// declared, a clock, a command buffer, and the prefabs and data the
/// samples name.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(PrefabRegistry::new());
    let mut schedule = Schedule::new();
    KitPlugins::set()
        .without::<TagsPlugin>()
        .with(TagsPlugin::new().with_tags(["team.player", "damage.fire", "door.locked"]))
        .build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    for (id, value) in [
        ("slime", json!({ "tag": "slime" })),
        ("glow", json!({ "tag": "glow" })),
        ("bolt", json!({ "tag": "bolt" })),
    ] {
        prefab(&mut world, id, value);
    }
    if !world.has_resource::<DataRegistry>() {
        world.insert_resource(DataRegistry::new());
    }
    let data = world.resource_mut::<DataRegistry>();
    data.insert("wand", Ability::new());
    data.insert("regen", Effect::new().with_duration(2.0));
    data.insert("hero_xp", Curve::new([(1.0, 0.0), (2.0, 10.0)]));
    (world, schedule)
}

fn prefab(world: &mut World, id: &str, value: Value) {
    world
        .resource_mut::<PrefabRegistry>()
        .insert(id.to_owned(), serde_json::from_value(value).unwrap());
}

/// One fixed step, its commands flushed and its events rotated.
fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(DT);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    flush(world);
    world.flush_events();
}

fn flush(world: &mut World) {
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
}

/// Spawns `count` entities and despawns two in three, so later spawns take
/// other indices.
fn churn(world: &mut World, count: usize) {
    let spawned: Vec<Entity> = (0..count).map(|_| world.spawn()).collect();
    for (index, entity) in spawned.into_iter().enumerate() {
        if index % 3 != 1 {
            world.despawn(entity);
        }
    }
}

fn named(world: &mut World, entity: Entity, area: Option<&str>, name: &str) -> Entity {
    save::name(world, entity, area, name).unwrap();
    entity
}

fn entity_named(world: &World, name: &str) -> Entity {
    world
        .resource::<SaveState>()
        .named_entity(SaveId::authored(name))
        .unwrap()
}

fn id_of(world: &World, entity: Entity) -> Option<SaveId> {
    world.get::<save::Saved>(entity).map(save::Saved::id)
}

fn text(save: &WorldSave) -> String {
    serde_json::to_string(save).unwrap()
}

fn restore(world: &mut World, save: &WorldSave) {
    let report = restore_world(world, save, &(Probe,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
}

fn tagged(world: &World, tag: &str) -> usize {
    world.query::<&Tag>().filter(|t| t.name == tag).count()
}

/// The described components the registry saves, by name.
fn saved_described(world: &World) -> Vec<&'static str> {
    let registry = world.resource::<SaveRegistry>();
    let saved: BTreeSet<&str> = registry.component_names().collect();
    world
        .resource::<KitDescriptions>()
        .iter()
        .map(crate::ComponentDescription::name)
        .filter(|name| saved.contains(name))
        .collect()
}

/// A prefab per saved described component, holding its sample.
fn probes(world: &mut World) -> Vec<&'static str> {
    let names = saved_described(world);
    for name in &names {
        let sample = world
            .resource::<KitDescriptions>()
            .get(name)
            .and_then(crate::ComponentDescription::sample_json)
            .unwrap();
        let sample: Value = serde_json::from_str(sample).unwrap();
        prefab(
            world,
            &format!("probe_{name}"),
            json!({ "components": { *name: sample } }),
        );
    }
    names
}

#[test]
fn each_saved_kit_component_round_trips_through_a_capture() {
    let (mut a, mut schedule) = rig();
    let names = probes(&mut a);
    assert_eq!(
        names.len() + UNSAVED_COMPONENTS.len(),
        a.resource::<KitDescriptions>().iter().count()
    );
    let source = a.spawn_with((Transform::from_position(Vec2::ZERO),));
    named(&mut a, source, None, "source");
    for name in &names {
        let prepared = a
            .prepare_prefab(&format!("probe_{name}"), [8.0, 8.0])
            .unwrap_or_else(|err| panic!("{name}: {err}"));
        a.resource_mut::<CommandBuffer>()
            .spawn_prefab_from(source, prepared);
    }
    flush(&mut a);
    // Six steps move each clock, timer and phase off its spawn's.
    for _ in 0..6 {
        step(&mut a, &mut schedule);
    }
    let saved = capture_world(&a).unwrap();
    let spawns = serde_json::to_value(&saved).unwrap()["global"]["spawns"].clone();
    for name in &names {
        assert!(
            spawns
                .as_array()
                .unwrap()
                .iter()
                .any(|spawn| spawn["components"].get(*name).is_some()),
            "{name} is in the save"
        );
    }

    let (mut b, _) = rig();
    probes(&mut b);
    churn(&mut b, 40);
    let source = b.spawn_with((Transform::from_position(Vec2::ZERO),));
    named(&mut b, source, None, "source");
    restore(&mut b, &saved);
    assert_eq!(
        text(&capture_world(&b).unwrap()),
        text(&saved),
        "every saved component as it was"
    );
}

#[test]
fn a_lifetime_and_a_spawner_keep_their_time() {
    let level = |world: &mut World| -> (Entity, Entity) {
        let candle = world.spawn_with((Lifetime::new(1.0),));
        let nest = world.spawn_with((
            Position(Vec2::ZERO),
            Spawner::new("slime", 0.25).with_count(3),
        ));
        (
            named(world, candle, None, "candle"),
            named(world, nest, None, "nest"),
        )
    };
    let (mut a, mut schedule_a) = rig();
    let (candle_a, nest_a) = level(&mut a);
    for _ in 0..18 {
        step(&mut a, &mut schedule_a);
    }
    assert_eq!(tagged(&a, "slime"), 1);
    let saved = capture_world(&a).unwrap();

    let (mut b, mut schedule_b) = rig();
    churn(&mut b, 30);
    let (candle_b, nest_b) = level(&mut b);
    restore(&mut b, &saved);
    let lifetime = |world: &World, candle| world.get::<Lifetime>(candle).unwrap().timer().elapsed();
    assert_eq!(lifetime(&b, candle_b), lifetime(&a, candle_a));
    let spawner = |world: &World, nest| {
        let spawner = world.get::<Spawner>(nest).unwrap();
        (spawner.timer().elapsed(), spawner.remaining())
    };
    assert_eq!(spawner(&b, nest_b), spawner(&a, nest_a));
    assert_eq!(spawner(&b, nest_b).1, Some(2));
    assert_eq!(tagged(&b, "slime"), 1, "the spawned slime came back");

    for frame in 0..60 {
        step(&mut a, &mut schedule_a);
        step(&mut b, &mut schedule_b);
        assert_eq!(
            (a.is_alive(candle_a), tagged(&a, "slime")),
            (b.is_alive(candle_b), tagged(&b, "slime")),
            "frame {frame}"
        );
    }
    assert!(!b.is_alive(candle_b));
    assert_eq!(tagged(&b, "slime"), 3);
}

/// A mage, a golem, a sword, spikes and a hero, all named.
fn cast(world: &mut World) -> [Entity; 5] {
    let mage = world.spawn_with((Transform::from_position(Vec2::ZERO),));
    let golem = world.spawn_with((Health::new(10.0),));
    let sword = world.spawn_with((Hitbox::new(HitShape::circle(4.0), 1.0),));
    let spikes = world.spawn_with((ContactDamage::new(1.0).with_interval(0.5),));
    let hero = world.spawn_with((Health::new(10.0),));
    [
        named(world, mage, None, "mage"),
        named(world, golem, None, "golem"),
        named(world, sword, None, "sword"),
        named(world, spikes, None, "spikes"),
        named(world, hero, None, "hero"),
    ]
}

#[test]
fn references_come_back_by_save_id() {
    let (mut a, _) = rig();
    let [mage, golem, sword, spikes, hero] = cast(&mut a);
    let burn = DataRef::inline(
        Effect::new()
            .with_duration(4.0)
            .with_period(0.5)
            .with_damage(1.0),
    );
    EffectRequest::new(golem, burn)
        .with_source(mage)
        .apply(&mut a);
    a.get_mut::<Hitbox>(sword).unwrap().set_hit_targets([golem]);
    a.get_mut::<ContactDamage>(spikes)
        .unwrap()
        .set_touches([ContactTouch::new(hero, 0.25, 0.0)]);
    let bolt = a.prepare_prefab("bolt", [0.0, 0.0]).unwrap();
    a.resource_mut::<CommandBuffer>()
        .spawn_prefab_from(mage, bolt);
    flush(&mut a);
    let saved = capture_world(&a).unwrap();
    assert_eq!(saved.dropped_refs(), 0);

    let (mut b, _) = rig();
    churn(&mut b, 50);
    let [mage, golem, sword, spikes, hero] = cast(&mut b);
    restore(&mut b, &saved);
    let instance = b
        .get::<Effects>(golem)
        .unwrap()
        .instances()
        .next()
        .unwrap()
        .clone();
    assert_eq!(instance.source(), Some(mage), "an effect's source");
    assert_eq!(
        b.get::<Hitbox>(sword).unwrap().hit_targets(),
        [golem],
        "a hitbox's targets"
    );
    let touch = b
        .get::<ContactDamage>(spikes)
        .unwrap()
        .touches()
        .next()
        .unwrap();
    assert_eq!(
        (touch.target(), touch.since_hit(), touch.since_met()),
        (hero, 0.25, 0.0),
        "a contact's touch with its times"
    );
    let bolt = b
        .query::<(Entity, &Tag)>()
        .find(|(_, tag)| tag.name == "bolt")
        .unwrap()
        .0;
    assert_eq!(b.get::<Source>(bolt).unwrap().entity(), mage, "a `Source`");
    assert_eq!(id_of(&b, mage), Some(SaveId::authored("mage")));
}

#[test]
fn kit_resources_restore_whole() {
    let (mut a, _) = rig();
    for _ in 0..5 {
        a.resource_mut::<KitRng>().next_u32();
    }
    a.resource_mut::<Counters>().add("coins", 7);
    a.resource_mut::<Flags>().set("door.locked", true);
    let saved = capture_world(&a).unwrap();

    let (mut b, _) = rig();
    restore(&mut b, &saved);
    assert_eq!(b.resource::<KitRng>(), a.resource::<KitRng>());
    assert_eq!(b.resource::<Counters>(), a.resource::<Counters>());
    assert_eq!(b.resource::<Flags>(), a.resource::<Flags>());
    assert_eq!(
        b.resource_mut::<KitRng>().next_u32(),
        a.resource_mut::<KitRng>().next_u32(),
        "the stream continues"
    );
}

#[test]
fn queued_damage_heal_and_effect_requests_resolve_once_after_a_load() {
    let level = |world: &mut World| -> (Entity, Entity) {
        let golem = world.spawn_with((Health::new(10.0),));
        let hero = world.spawn();
        (
            named(world, golem, None, "golem"),
            named(world, hero, None, "hero"),
        )
    };
    let (mut a, mut schedule_a) = rig();
    let (golem, hero) = level(&mut a);
    step(&mut a, &mut schedule_a);
    // Queued after the step's resolve, as a cue queues them in `React`.
    assert!(
        DamageRequest::new(golem, 3.0)
            .with_source(hero)
            .queue(&mut a)
    );
    assert!(HealRequest::new(golem, 1.0).queue(&mut a));
    let chill = DataRef::inline(Effect::new().with_duration(2.0));
    assert!(
        EffectRequest::new(golem, chill)
            .with_source(hero)
            .queue(&mut a)
    );
    let saved = capture_world(&a).unwrap();

    let (mut b, mut schedule_b) = rig();
    churn(&mut b, 25);
    let (golem_b, _) = level(&mut b);
    restore(&mut b, &saved);
    let state = |world: &World, golem| {
        let effects = world
            .get::<Effects>(golem)
            .map_or(0, |e| e.instances().count());
        (world.get::<Health>(golem).unwrap().current(), effects)
    };
    for (world, schedule, golem) in [
        (&mut a, &mut schedule_a, golem),
        (&mut b, &mut schedule_b, golem_b),
    ] {
        step(world, schedule);
        assert_eq!(state(world, golem), (8.0, 1), "each landed once");
        step(world, schedule);
        assert_eq!(state(world, golem), (8.0, 1), "and once only");
    }
}

#[test]
fn a_saved_parents_children_come_back_once() {
    let level = |world: &mut World| -> Entity {
        let children: SpawnChildren =
            serde_json::from_value(json!([{ "prefab": "glow" }])).unwrap();
        let parent = world.spawn_with((Transform::from_position(Vec2::new(4.0, 4.0)), children));
        named(world, parent, None, "lamp")
    };
    let (mut a, mut schedule_a) = rig();
    level(&mut a);
    step(&mut a, &mut schedule_a);
    assert_eq!(tagged(&a, "glow"), 1);
    let saved = capture_world(&a).unwrap();
    assert_eq!(
        serde_json::to_value(&saved).unwrap()["global"]["spawns"],
        json!([]),
        "the child is no save's"
    );

    let (mut b, mut schedule_b) = rig();
    level(&mut b);
    restore(&mut b, &saved);
    for _ in 0..3 {
        step(&mut b, &mut schedule_b);
    }
    assert_eq!(tagged(&b, "glow"), 1, "from the parent's fresh spawn, once");
}

#[test]
fn a_stat_modifier_added_in_play_keeps_a_retuned_base() {
    let hero = |world: &mut World, base: f32| -> Entity {
        prefab(
            world,
            "hero",
            json!({ "components": { "stats": { "move_speed": base } } }),
        );
        let hero = world.spawn_prefab("hero", [0.0, 0.0]).unwrap();
        named(world, hero, None, "hero")
    };
    let (mut a, _) = rig();
    let entity = hero(&mut a, 160.0);
    assert!(a.get_mut::<Stats>(entity).unwrap().add_modifier(
        "move_speed",
        "boots",
        Modifier::Add(30.0)
    ));
    let saved = capture_world(&a).unwrap();

    let (mut b, _) = rig();
    let entity = hero(&mut b, 200.0);
    restore(&mut b, &saved);
    let stats = b.get::<Stats>(entity).unwrap();
    assert_eq!(stats.base("move_speed"), Some(200.0), "the retuned base");
    assert_eq!(
        stats.get("move_speed"),
        Some(230.0),
        "with the run's modifier"
    );
}

#[test]
fn a_pending_hit_target_drops_once_its_hitbox_rearms() {
    let north = |world: &mut World| -> Entity {
        let sword = world.spawn_with((Hitbox::new(HitShape::circle(4.0), 1.0),));
        named(world, sword, Some("north"), "sword")
    };
    let south = |world: &mut World| -> Entity {
        let bat = world.spawn_with((Health::new(5.0),));
        named(world, bat, Some("south"), "bat")
    };
    let (mut a, _) = rig();
    let sword = north(&mut a);
    let bat = south(&mut a);
    a.get_mut::<Hitbox>(sword).unwrap().set_hit_targets([bat]);
    let saved = capture_world(&a).unwrap();

    // Its area not loaded, the target waits; a capture writes it as it came.
    let (mut kept, _) = rig();
    let sword = north(&mut kept);
    restore(&mut kept, &saved);
    assert!(kept.get::<Hitbox>(sword).unwrap().hit_targets().is_empty());
    assert_eq!(text(&capture_world(&kept).unwrap()), text(&saved));

    // Rearmed before a second capture: the capture leaves it out.
    kept.get_mut::<Hitbox>(sword).unwrap().rearm();
    let again = serde_json::to_value(capture_world(&kept).unwrap()).unwrap();
    assert_eq!(again["areas"]["north"]["named"]["sword"].get("refs"), None);

    // Rearmed before the entry: the entry drops it.
    let (mut rearmed, _) = rig();
    let sword = north(&mut rearmed);
    restore(&mut rearmed, &saved);
    rearmed.get_mut::<Hitbox>(sword).unwrap().rearm();
    south(&mut rearmed);
    enter_area(&mut rearmed, "south", &(Probe,)).unwrap();
    assert!(
        rearmed
            .get::<Hitbox>(sword)
            .unwrap()
            .hit_targets()
            .is_empty()
    );

    // Not rearmed, the entry resolves it.
    let (mut entered, _) = rig();
    let sword = north(&mut entered);
    restore(&mut entered, &saved);
    let bat = south(&mut entered);
    enter_area(&mut entered, "south", &(Probe,)).unwrap();
    assert_eq!(entered.get::<Hitbox>(sword).unwrap().hit_targets(), [bat]);
    assert_eq!(entity_named(&entered, "bat"), bat);
}
