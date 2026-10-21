use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, Stage};

use crate::foundation::tags::TagsPlugin;
use crate::{DespawnOutsidePlugin, StayInViewPlugin, WrapPlugin};

/// A World with hit shapes and the tags `team.enemy` and `damage.fire`.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.enemy", "damage.fire"])
        .build(&mut schedule, &mut world);
    HitPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn check(world: &World, name: &str, value: &serde_json::Value) -> Result<(), String> {
    world
        .resource::<ComponentRegistry>()
        .check_in(name, value, world)
        .map_err(|e| e.to_string())
}

/// A World with hit shapes, health and the damage types `physical.slash`
/// and `elemental.fire` declared.
fn typed_rig() -> World {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.enemy", "physical.slash", "elemental.fire"])
        .build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    HitPlugin.build(&mut schedule, &mut world);
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(json!({ "physical.slash": {}, "elemental.fire": {} })).unwrap();
    world
        .resource_mut::<tungsten_core::DataRegistry>()
        .insert("game", types);
    world
}

#[test]
fn damage_takes_a_number_or_a_map_and_names_the_field_in_errors() {
    let world = typed_rig();
    let shape = json!({ "circle": 4 });
    let hitbox = |extra: serde_json::Value| {
        let mut value = json!({ "shape": shape });
        value
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        check(&world, HITBOX, &value)
    };
    assert!(hitbox(json!({ "damage": 2 })).is_ok());
    let typed = json!({
        "damage": { "physical.slash": 8, "elemental.fire": 4 },
        "pen": { "elemental": 0.25 },
        "convert": [{ "from": "physical", "to": "elemental.fire", "pct": 0.5 }]
    });
    assert!(hitbox(typed.clone()).is_ok());
    for (extra, error) in [
        (
            json!({ "damage": -1 }),
            "component `hitbox`, field `damage`: must be finite and >= 0 (got -1)",
        ),
        (
            json!({ "damage": { "physical.slash": -1 } }),
            "component `hitbox`, field `damage.physical.slash`: must be finite and >= 0 (got -1)",
        ),
        (
            json!({ "damage": { "physical.blunt": 2 } }),
            "component `hitbox`, field `damage.physical.blunt`: unknown damage type `physical.blunt`",
        ),
        (
            json!({ "damage": 1, "pen": { "chaos": 0.5 } }),
            "component `hitbox`, field `pen.chaos`: unknown damage type `chaos`",
        ),
        (
            json!({ "damage": 1, "convert": [{ "from": "physical.slash", "to": "physical", "pct": 0.5 }] }),
            "component `hitbox`, field `convert.0.to`: unknown damage type `physical`",
        ),
        (
            json!({ "damage": 1, "pen": { "elemental": 2 } }),
            "component `hitbox`, field `pen`: elemental: points must be in [0, 1] (got 2)",
        ),
    ] {
        assert_eq!(hitbox(extra).unwrap_err(), error);
    }
    assert!(hitbox(json!({ "damage": "lots" })).is_err());
    assert!(hitbox(json!({ "damage": { "Physical": 1 } })).is_err());

    // A map's parts come in type-name order, its sum the damage.
    let mut value = typed;
    value["shape"] = shape.clone();
    let def: HitboxDef = serde_json::from_value(value).unwrap();
    let spellblade = Hitbox::from(def);
    assert_eq!(spellblade.damage(), 12.0);
    let parts: Vec<(f32, Vec<&str>)> = spellblade
        .parts()
        .iter()
        .map(|part| (part.amount(), part.tags().iter().collect()))
        .collect();
    assert_eq!(
        parts,
        [(4.0, vec!["elemental.fire"]), (8.0, vec!["physical.slash"])]
    );
    assert_eq!(
        spellblade.penetration(),
        Some(&Penetration::new().with_points("elemental", 0.25))
    );
    assert_eq!(
        spellblade.conversion(),
        Some(&Conversion::new().with_rule("physical", "elemental.fire", 0.5))
    );
    assert_eq!(
        spellblade.inspect_rows()[1].1,
        "12 of elemental.fire 4, physical.slash 8"
    );

    // A save keeps the damage and the parts, and reads back whole.
    let saved = serde_json::to_value(&spellblade).unwrap();
    assert_eq!(saved["damage"], json!(12.0));
    assert_eq!(
        saved["parts"],
        json!([
            { "tags": ["elemental.fire"], "amount": 4.0 },
            { "tags": ["physical.slash"], "amount": 8.0 }
        ])
    );
    assert_eq!(serde_json::from_value::<Hitbox>(saved).unwrap(), spellblade);
    assert!(
        serde_json::from_value::<Hitbox>(
            json!({ "shape": shape, "damage": 1, "parts": [{ "amount": 0 }] })
        )
        .is_err(),
        "a save's parts hold one above 0"
    );
}

#[test]
fn a_map_beside_tags_or_an_all_zero_map_fails() {
    let world = typed_rig();
    let refused = [
        json!({ "shape": { "circle": 4 }, "damage": { "physical.slash": 2 }, "tags": ["elemental.fire"] }),
        json!({ "shape": { "circle": 4 }, "damage": { "physical.slash": 0, "elemental.fire": 0 } }),
        json!({ "shape": { "circle": 4 }, "damage": {} }),
    ];
    for value in &refused {
        assert!(check(&world, HITBOX, value).is_err(), "{value}");
    }
    let beside = check(&world, HITBOX, &refused[0]).unwrap_err();
    assert!(beside.contains("may not sit beside it"), "{beside}");
    assert!(
        check(
            &world,
            HITBOX,
            &json!({ "shape": { "circle": 4 }, "damage": { "physical.slash": 2, "elemental.fire": 0 }, "tags": [] })
        )
        .is_ok(),
        "an empty `tags` and a part of 0 beside one above 0"
    );
    let panics = |f: fn()| std::panic::catch_unwind(f).is_err();
    assert!(panics(|| {
        let _ = Hitbox::new(HitShape::circle(1.0), 1.0).with_parts([]);
    }));
    assert!(panics(|| {
        let _ = Hitbox::new(HitShape::circle(1.0), 1.0).with_parts([DamagePart::new(0.0)]);
    }));
}

#[test]
fn a_hit_shape_reads_one_form_within_its_bounds() {
    assert_eq!(
        serde_json::from_value::<HitShape>(json!({ "aabb": [6, 3] })).unwrap(),
        HitShape::aabb(Vec2::new(6.0, 3.0))
    );
    assert_eq!(
        serde_json::to_value(HitShape::circle(MAX_EXTENT)).unwrap(),
        json!({ "circle": 16384.0 })
    );
    for bad in [
        json!({}),
        json!({ "circle": 0 }),
        json!({ "circle": 16385 }),
        json!({ "aabb": [6, -1] }),
        json!({ "circle": 2, "aabb": [1, 1] }),
        json!({ "kind": "circle", "radius": 2 }),
    ] {
        assert!(
            serde_json::from_value::<HitShape>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    assert_eq!(HitShape::circle(8.0).summary(), "circle 8");
    assert_eq!(HitShape::aabb(Vec2::new(6.0, 3.0)).summary(), "box 6 x 3");
}

#[test]
fn a_hitbox_saves_its_fields_but_not_its_targets() {
    let mut world = World::new();
    let target = world.spawn();
    let mut sword = Hitbox::new(HitShape::circle(4.0), 2.0)
        .with_tags(Tags::new(["damage.fire"]))
        .with_knockback(100.0);
    sword.hit.push(target);
    assert!(sword.has_hit(target));
    let saved = serde_json::to_value(&sword).unwrap();
    assert_eq!(
        saved,
        json!({
            "shape": { "circle": 4.0 }, "offset": [0.0, 0.0], "damage": 2.0,
            "tags": ["damage.fire"], "filter": {}, "knockback": 100.0
        })
    );
    let loaded: Hitbox = serde_json::from_value(saved).unwrap();
    assert!(!loaded.has_hit(target), "a fresh activation");
    sword.rearm();
    assert_eq!(loaded, sword);
    for bad in [
        json!({ "shape": { "circle": 4 }, "damage": -1 }),
        json!({ "shape": { "circle": 4 }, "damage": 1, "knockback": -5 }),
        json!({ "shape": { "circle": 4 }, "damage": 1, "offset": [20000, 0] }),
        json!({ "shape": { "circle": 4 }, "damage": 1, "spin": true }),
    ] {
        assert!(
            serde_json::from_value::<Hitbox>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_damage_stat_writes_only_a_valid_damage() {
    let mut hitbox = Hitbox::new(HitShape::circle(4.0), 2.0);
    hitbox.set_damage(5.0);
    assert_eq!(hitbox.damage(), 5.0);
    hitbox.set_damage(-1.0);
    hitbox.set_damage(f32::NAN);
    assert_eq!(hitbox.damage(), 5.0);
}

#[test]
fn the_scene_forms_check_their_fields_and_tags() {
    let (world, _) = rig();
    assert!(
        check(
            &world,
            HITBOX,
            &json!({ "shape": { "aabb": [6, 3] }, "offset": [8, 0], "damage": 2, "tags": ["damage.fire"], "filter": { "all": ["team.enemy"] }, "knockback": 300 })
        )
        .is_ok()
    );
    assert_eq!(
        check(
            &world,
            HITBOX,
            &json!({ "shape": { "circle": 4 }, "damage": 1, "tags": ["damage.acid"] })
        )
        .unwrap_err(),
        "component `hitbox`, field `tags.0`: unknown tag `damage.acid`"
    );
    assert_eq!(
        check(
            &world,
            HITBOX,
            &json!({ "shape": { "circle": 4 }, "damage": 1, "filter": { "all": ["team.foe"] } })
        )
        .unwrap_err(),
        "component `hitbox`, field `filter.all.0`: unknown tag `team.foe`"
    );
    for bad in [
        json!({ "damage": 1 }),
        json!({ "shape": { "circle": 4 } }),
        json!({ "shape": { "circle": 4 }, "damage": 1, "tags": ["Fire"] }),
        json!({ "shape": { "circle": -4 }, "damage": 1 }),
    ] {
        assert!(check(&world, HITBOX, &bad).is_err(), "{bad}");
    }
    assert!(
        check(
            &world,
            HURTBOX,
            &json!({ "shape": { "circle": 8 }, "offset": [0, -4] })
        )
        .is_ok()
    );
    assert!(
        check(
            &world,
            HURTBOX,
            &json!({ "shape": { "circle": 8 }, "radius": 3 })
        )
        .is_err()
    );
}

#[test]
fn the_plugin_registers_the_queue_the_grid_and_the_tick_before_the_corrections() {
    let (world, schedule) = rig();
    assert!(world.has_event::<Struck>());
    assert_eq!(schedule.stage_of(HIT_GRID), Some(Stage::FixedUpdate));
    assert_eq!(
        schedule.len(),
        9,
        "the seven anchors, the grid and the tick"
    );
    let mut world = World::new();
    let mut schedule = Schedule::new();
    DespawnOutsidePlugin.build(&mut schedule, &mut world);
    WrapPlugin.build(&mut schedule, &mut world);
    StayInViewPlugin.build(&mut schedule, &mut world);
    HitPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| order.iter().position(|n| *n == name).unwrap();
    for later in ["despawn_outside", "wrap", "stay_in_view"] {
        assert!(
            at(HIT_GRID) < at(HITBOX_TICK) && at(HITBOX_TICK) < at(later),
            "{order:?}"
        );
    }
}

#[test]
fn a_hurtbox_and_a_struck_read_back_what_they_were_built_with() {
    let mut world = World::new();
    let (hitter, target) = (world.spawn(), world.spawn());
    let body = Hurtbox::new(HitShape::circle(8.0)).with_offset(Vec2::new(0.0, -4.0));
    assert_eq!(
        (body.shape(), body.offset()),
        (HitShape::circle(8.0), Vec2::new(0.0, -4.0))
    );
    let struck = Struck::new(hitter, target, Vec2::ONE, 2.0);
    assert_eq!(
        (struck.entity, struck.target, struck.position, struck.amount),
        (hitter, target, Vec2::ONE, 2.0)
    );
}

#[test]
#[should_panic(expected = "radius must be finite and in (0, 16384]")]
fn a_zero_circle_panics() {
    let _ = HitShape::circle(0.0);
}

#[test]
#[should_panic(expected = "each half extent must be finite and in (0, 16384]")]
fn a_huge_box_panics() {
    let _ = HitShape::aabb(Vec2::new(1.0, 20_000.0));
}

#[test]
#[should_panic(expected = "offset must be finite and within ±16384")]
fn a_far_offset_panics() {
    let _ = Hurtbox::new(HitShape::circle(1.0)).with_offset(Vec2::new(16_385.0, 0.0));
}

#[test]
#[should_panic(expected = "damage must be finite and >= 0")]
fn a_negative_hitbox_damage_panics() {
    let _ = Hitbox::new(HitShape::circle(1.0), -1.0);
}

#[test]
#[should_panic(expected = "speed must be finite and >= 0")]
fn a_nan_hitbox_knockback_panics() {
    let _ = Hitbox::new(HitShape::circle(1.0), 1.0).with_knockback(f32::NAN);
}

/// A World running hitboxes, stats and the damage pipeline at 1/60 s steps,
/// with `types` declared as tags and as damage types.
fn hit_rig(types: &[&str]) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(tungsten_core::Time::new());
    world.insert_resource(tungsten_core::CommandBuffer::new());
    let mut schedule = Schedule::new();
    crate::TagsPlugin::new()
        .with_tags(types.iter().copied())
        .build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    crate::StatsPlugin::new().build(&mut schedule, &mut world);
    HitPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let entry: serde_json::Map<String, serde_json::Value> = types
        .iter()
        .map(|name| ((*name).to_owned(), serde_json::json!({})))
        .collect();
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(serde_json::Value::Object(entry)).unwrap();
    world
        .resource_mut::<tungsten_core::DataRegistry>()
        .insert("game", types);
    (world, schedule)
}

/// One fixed step of 1/60 s.
fn fixed_step(world: &mut World, schedule: &mut Schedule) {
    world
        .resource_mut::<tungsten_core::Time>()
        .advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

/// Each part sent so far: its amount and its kinds by name.
fn parts_sent(world: &World) -> Vec<(f32, Vec<&str>)> {
    world
        .resource::<tungsten_core::EventQueue<crate::DamagedPart>>()
        .iter()
        .map(|part| (part.amount, part.kinds.names(world)))
        .collect()
}

fn typed(amount: f32, name: &str) -> DamagePart {
    DamagePart::new(amount).with_tags(Tags::new([name]))
}

/// A target at the origin: a hurtbox of radius 8 and plenty of health.
fn target(world: &mut World) -> Entity {
    world.spawn_with((
        tungsten_core::Transform::from_position(Vec2::ZERO),
        Hurtbox::new(HitShape::circle(8.0)),
        crate::Health::new(1000.0),
    ))
}

#[test]
fn a_typed_hitbox_queues_its_parts_split_to_its_damage() {
    let (mut world, mut schedule) = hit_rig(&["fire", "slash"]);
    target(&mut world);
    let blade = world.spawn_with((
        tungsten_core::Transform::from_position(Vec2::ZERO),
        Hitbox::new(HitShape::circle(4.0), 0.0)
            .with_parts([typed(4.0, "fire"), typed(8.0, "slash")]),
    ));
    fixed_step(&mut world, &mut schedule);
    assert_eq!(
        parts_sent(&world),
        [(4.0, vec!["fire"]), (8.0, vec!["slash"])],
        "the parts as they are at their sum"
    );
    world.insert(blade, crate::Stats::new().with(crate::Stats::DAMAGE, 6.0));
    world.get_mut::<Hitbox>(blade).unwrap().rearm();
    // Both of the queue's windows, so only the next hit's parts are read.
    world.flush_events();
    world.flush_events();
    fixed_step(&mut world, &mut schedule);
    assert_eq!(world.get::<Hitbox>(blade).unwrap().damage(), 6.0);
    assert_eq!(
        parts_sent(&world),
        [(2.0, vec!["fire"]), (4.0, vec!["slash"])],
        "split to the stat's 6 by their proportions"
    );
    assert_eq!(
        world.get::<Hitbox>(blade).unwrap().parts()[1].amount(),
        8.0,
        "the base parts stay"
    );
}

#[test]
fn a_zero_damage_stat_makes_a_typed_hitbox_a_trigger_and_its_return_restores_the_parts() {
    let (mut world, mut schedule) = hit_rig(&["fire", "slash"]);
    target(&mut world);
    let mut stats = crate::Stats::new().with(crate::Stats::DAMAGE, 12.0);
    stats.add_modifier(crate::Stats::DAMAGE, "dull", crate::Modifier::Mul(0.0));
    let blade = world.spawn_with((
        tungsten_core::Transform::from_position(Vec2::ZERO),
        Hitbox::new(HitShape::circle(4.0), 0.0)
            .with_parts([typed(4.0, "fire"), typed(8.0, "slash")]),
        stats,
    ));
    fixed_step(&mut world, &mut schedule);
    let struck: Vec<f32> = world
        .resource::<tungsten_core::EventQueue<Struck>>()
        .iter()
        .map(|hit| hit.amount)
        .collect();
    assert_eq!(struck, [0.0], "a trigger: the hit lands");
    assert!(parts_sent(&world).is_empty(), "and nothing is hurt");
    world
        .get_mut::<crate::Stats>(blade)
        .unwrap()
        .remove_modifiers("dull");
    world.get_mut::<Hitbox>(blade).unwrap().rearm();
    // Both of the queue's windows, so only the next hit's parts are read.
    world.flush_events();
    world.flush_events();
    fixed_step(&mut world, &mut schedule);
    assert_eq!(
        parts_sent(&world),
        [(4.0, vec!["fire"]), (8.0, vec!["slash"])],
        "the stat's return restores the parts"
    );
}

/// A World with hit shapes, health, time and the tag `team.enemy`.
fn windowed_rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.enemy"])
        .build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    HitPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

#[test]
fn a_windowed_hitbox_hits_only_while_armed() {
    use tungsten_core::physics::{Position, PrevPosition};

    for (dt, steps) in [(1.0 / 60.0, 6), (1.0 / 30.0, 3)] {
        let (mut world, mut schedule) = windowed_rig();
        // Three steps' window at 1/60, which is 0.05 s; the same at 1/30
        // is a step and a half, so two steps land.
        let window = 3.0 / 60.0;
        let placed = |world: &mut World, rest: Box<dyn FnOnce(&mut World) -> Entity>| rest(world);
        let turret = placed(
            &mut world,
            Box::new(move |world| {
                world.spawn_with((
                    Position(Vec2::ZERO),
                    PrevPosition(Vec2::ZERO),
                    Hitbox::new(HitShape::circle(40.0), 1.0).with_window(window),
                ))
            }),
        );
        let target = world.spawn_with((
            Position(Vec2::new(10.0, 0.0)),
            PrevPosition(Vec2::new(10.0, 0.0)),
            Hurtbox::new(HitShape::circle(4.0)),
            crate::Health::new(100.0),
        ));
        let run = |world: &mut World, schedule: &mut Schedule| {
            world.resource_mut::<Time>().advance_frame(dt);
            schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
            world.flush_events();
        };
        for _ in 0..3 {
            run(&mut world, &mut schedule);
        }
        assert_eq!(
            world.get::<crate::Health>(target).unwrap().current(),
            100.0,
            "disarmed, nothing lands"
        );
        assert!(!world.get::<Hitbox>(turret).unwrap().is_armed());
        world.get_mut::<Hitbox>(turret).unwrap().arm();
        let mut hits = Vec::new();
        for _ in 0..steps {
            run(&mut world, &mut schedule);
            hits.push(world.get::<crate::Health>(target).unwrap().current());
            // Each step a fresh target may be hit: the activation is one
            // hit per target, so rearm without opening the window.
            world.get_mut::<Hitbox>(turret).unwrap().rearm();
        }
        let expected: Vec<f32> = match dt {
            dt if dt < 1.0 / 50.0 => vec![99.0, 98.0, 97.0, 97.0, 97.0, 97.0],
            _ => vec![99.0, 98.0, 98.0],
        };
        assert_eq!(hits, expected, "at dt {dt}");
        assert!(
            !world.get::<Hitbox>(turret).unwrap().is_armed(),
            "closed again"
        );
        // `rearm` alone opens no window; `arm` does, and the save carries both.
        world.get_mut::<Hitbox>(turret).unwrap().rearm();
        assert!(!world.get::<Hitbox>(turret).unwrap().is_armed());
        world.get_mut::<Hitbox>(turret).unwrap().arm();
        let saved = serde_json::to_value(world.get::<Hitbox>(turret).unwrap()).unwrap();
        assert_eq!(saved["window"].as_f64().unwrap() as f32, window);
        assert_eq!(saved["armed_left"].as_f64().unwrap() as f32, window);
        let read: Hitbox = serde_json::from_value(saved).unwrap();
        assert!(read.is_armed());
        assert_eq!(read.window(), Some(window));
    }
    // An unwindowed hitbox saves neither field and is always armed.
    let plain = Hitbox::new(HitShape::circle(4.0), 1.0);
    let saved = serde_json::to_value(&plain).unwrap();
    assert!(saved.get("window").is_none() && saved.get("armed_left").is_none());
    assert!(plain.is_armed());
    assert!(
        serde_json::from_value::<Hitbox>(
            json!({ "shape": { "circle": 4 }, "damage": 1, "window": 0 })
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<Hitbox>(
            json!({ "shape": { "circle": 4 }, "damage": 1, "armed_left": -1 })
        )
        .is_err()
    );
    let (world, _) = windowed_rig();
    assert!(
        check(
            &world,
            HITBOX,
            &json!({ "shape": { "circle": 4 }, "damage": 1, "window": 0.2 })
        )
        .is_ok()
    );
    assert!(
        check(
            &world,
            HITBOX,
            &json!({ "shape": { "circle": 4 }, "damage": 1, "window": -1 })
        )
        .is_err()
    );
    let rows = Hitbox::new(HitShape::circle(4.0), 1.0)
        .with_window(0.2)
        .inspect_rows();
    assert_eq!(rows[2].1, "0 targets, disarmed (0.20 s window)");
}

#[test]
fn a_loaded_hitbox_given_its_targets_hits_none_again() {
    use tungsten_core::physics::{Position, PrevPosition};

    // A sword's swing over two targets in its reach, then a third.
    let run = |named_again: bool| -> Vec<f32> {
        let (mut world, mut schedule) = windowed_rig();
        let sword = world.spawn_with((
            Position(Vec2::ZERO),
            PrevPosition(Vec2::ZERO),
            Hitbox::new(HitShape::circle(30.0), 1.0).with_window(0.5),
        ));
        let mut targets: Vec<Entity> = [10.0, -10.0]
            .into_iter()
            .map(|x| {
                world.spawn_with((
                    Position(Vec2::new(x, 0.0)),
                    PrevPosition(Vec2::new(x, 0.0)),
                    Hurtbox::new(HitShape::circle(4.0)),
                    crate::Health::new(100.0),
                ))
            })
            .collect();
        world.get_mut::<Hitbox>(sword).unwrap().arm();
        fixed_step(&mut world, &mut schedule);
        let hitbox = world.get::<Hitbox>(sword).unwrap();
        assert_eq!(hitbox.hit_targets(), targets.as_slice(), "in the order hit");
        // The game's save: the hitbox, and its targets by the game's IDs.
        let ids: Vec<usize> = hitbox
            .hit_targets()
            .iter()
            .map(|hit| targets.iter().position(|target| target == hit).unwrap())
            .collect();
        let mut loaded: Hitbox =
            serde_json::from_value(serde_json::to_value(hitbox).unwrap()).unwrap();
        assert!(loaded.hit_targets().is_empty() && loaded.is_armed());
        if named_again {
            // Named twice, kept once.
            loaded.set_hit_targets(ids.iter().chain(&ids).map(|&id| targets[id]));
            assert_eq!(loaded.hit_targets(), targets.as_slice());
        }
        world.insert(sword, loaded);
        // A third steps into the reach while the window runs.
        targets.push(world.spawn_with((
            Position(Vec2::new(0.0, 20.0)),
            PrevPosition(Vec2::new(0.0, 20.0)),
            Hurtbox::new(HitShape::circle(4.0)),
            crate::Health::new(100.0),
        )));
        for _ in 0..10 {
            fixed_step(&mut world, &mut schedule);
        }
        targets
            .iter()
            .map(|&target| world.get::<crate::Health>(target).unwrap().current())
            .collect()
    };
    assert_eq!(run(true), [99.0, 99.0, 99.0], "each once in the activation");
    assert_eq!(
        run(false),
        [98.0, 98.0, 99.0],
        "unnamed: a fresh activation"
    );
}
