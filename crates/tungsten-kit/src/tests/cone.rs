use serde_json::json;
use tungsten_core::physics::{Collider, PhysicsPlugin, RigidBodyBundle, Velocity};
use tungsten_core::save::{capture_world, name, restore_world};
use tungsten_core::{
    CommandBuffer, DataRef, DataRegistry, EventQueue, Plugin, Schedule, Stage, Time, Transform,
};

use super::*;
use crate::foundation::trace::KitTrace;
use crate::intent::action_input::Intent;
use crate::intent::sheet::Skill;
use crate::{
    Abilities, AbilitiesPlugin, Ability, Damaged, Health, HealthPlugin, HitPlugin, HitShape,
    Payload, Struck, TagsPlugin, Used, use_ability,
};

const STEP: f32 = 1.0 / 60.0;

/// A World with a clock, a command buffer, a trace, the tags `team.enemy`,
/// `team.player`, `damage.fire` and `damage.cold` (both damage types), and
/// the physics, health, hit and ability plugins.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.enemy", "team.player", "damage.fire", "damage.cold"])
        .build(&mut schedule, &mut world);
    PhysicsPlugin.build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    HitPlugin.build(&mut schedule, &mut world);
    AbilitiesPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(json!({ "damage.fire": {}, "damage.cold": {} })).unwrap();
    world.resource_mut::<DataRegistry>().insert("types", types);
    (world, schedule)
}

/// One fixed step, then the flushes that end the frame.
fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(STEP);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    world.resource_mut::<Time>().leave_fixed_steps();
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

/// A user at the origin aiming along +x, owning `cone` as `spray`.
fn user(world: &mut World, cone: Cone) -> Entity {
    let spray = Ability::new().with_payload(Payload::Cone(Box::new(cone)));
    world.resource_mut::<DataRegistry>().insert("spray", spray);
    let mut intent = Intent::default();
    intent.set_aim(Some(Vec2::X));
    world.spawn_with((
        Position(Vec2::ZERO),
        Transform::default(),
        Abilities::new().with(DataRef::by_id("spray")),
        intent,
    ))
}

/// An enemy with a hurtbox of `radius` at `at`, and ten health.
fn enemy(world: &mut World, at: Vec2, radius: f32) -> Entity {
    world.spawn_with((
        Position(at),
        Transform::from_position(at),
        Tags::new(["team.enemy"]),
        Health::new(10.0),
        Hurtbox::new(HitShape::circle(radius)),
    ))
}

/// The step's hits: each target struck.
fn struck(world: &World) -> Vec<Entity> {
    world
        .resource::<EventQueue<Struck>>()
        .iter()
        .map(|struck| struck.target)
        .collect()
}

fn health(world: &World, entity: Entity) -> f32 {
    world.get::<Health>(entity).unwrap().current()
}

fn enemies() -> TagFilter {
    TagFilter::new().all(["team.enemy"])
}

#[test]
fn the_rays_hit_what_they_reach_once_and_stop_at_a_wall() {
    let (mut world, mut schedule) = rig();
    let spray = user(
        &mut world,
        Cone::new(300.0, 14.0, 2.0).with_filter(enemies()),
    );
    // Seven rays at 0, ±4.67, ±9.33 and ±14 degrees.
    let near = enemy(&mut world, Vec2::new(100.0, 0.0), 12.0);
    let far = enemy(&mut world, Vec2::new(250.0, 20.0), 6.0);
    let walled = enemy(&mut world, Vec2::new(250.0, -45.0), 6.0);
    let aside = enemy(&mut world, Vec2::new(100.0, 100.0), 6.0);
    // A wall across the lower rays at x 150, and a dynamic body in the
    // middle ray's way, which stops nothing.
    world.spawn_with(RigidBodyBundle::r#static(
        Position(Vec2::new(150.0, -40.0)),
        Collider::aabb(Vec2::new(5.0, 20.0)),
    ));
    world.spawn_with(RigidBodyBundle::dynamic(
        Position(Vec2::new(60.0, 0.0)),
        Collider::circle(5.0),
    ));
    step(&mut world, &mut schedule);
    for i in 1..=2 {
        assert_eq!(use_ability(&mut world, spray, "spray"), Used::Used);
        step(&mut world, &mut schedule);
        let mut hit = struck(&world);
        hit.sort_by_key(|entity| entity.id());
        assert_eq!(
            hit,
            [near, far],
            "use {i}: three rays meet the near one, once; the walled and the aside go untouched"
        );
    }
    assert_eq!(
        [near, far, walled, aside].map(|entity| health(&world, entity)),
        [6.0, 6.0, 10.0, 10.0]
    );
}

#[test]
fn a_sweep_hits_each_target_once_as_it_passes_and_pushes_bodies() {
    let (mut world, mut schedule) = rig();
    let wave = Cone::new(300.0, 20.0, 4.0)
        .with_filter(enemies())
        .with_sweep(600.0)
        .with_push(300.0)
        .with_far(0.5);
    let bell = user(&mut world, wave);
    let near = enemy(&mut world, Vec2::new(60.0, 0.0), 6.0);
    let far = enemy(&mut world, Vec2::new(240.0, 0.0), 6.0);
    let crate_ = world.spawn_with(RigidBodyBundle::dynamic(
        Position(Vec2::new(150.0, 0.0)),
        Collider::circle(5.0),
    ));
    step(&mut world, &mut schedule);
    assert_eq!(use_ability(&mut world, bell, "spray"), Used::Used);
    let mut hits = Vec::new();
    let mut pushed_at = None;
    for i in 1..=40 {
        step(&mut world, &mut schedule);
        hits.extend(struck(&world).into_iter().map(|target| (i, target)));
        let velocity = world.get::<Velocity>(crate_).unwrap().0;
        if pushed_at.is_none() && velocity.x > 0.0 {
            pushed_at = Some((i, velocity));
        }
    }
    // 10 px a step: the near one's circle met in the band to 60, the far
    // one's in the band to 240, the crate's in the band to 150.
    assert_eq!(
        hits,
        [(6, near), (24, far)],
        "each once, as the front passes"
    );
    let (at, velocity) = pushed_at.unwrap();
    assert_eq!(at, 15, "the crate's band");
    assert!(
        (velocity.x - 225.0).abs() < 1e-3 && velocity.y.abs() < 1e-3,
        "300 at its share 0.75: {velocity}"
    );
    assert!((health(&world, near) - 6.4).abs() < 1e-4, "4 at 0.9");
    assert!((health(&world, far) - 7.6).abs() < 1e-4, "4 at 0.6");
    assert!(
        world.resource::<ConeFronts>().0.is_empty(),
        "the front ended at its reach"
    );
}

/// What a restored entity takes, as a state's marker.
#[derive(Debug, Clone, Copy)]
struct Loaded;

#[test]
fn a_front_ends_when_its_user_goes() {
    let (mut world, mut schedule) = rig();
    let wave = || {
        Cone::new(300.0, 20.0, 4.0)
            .with_filter(enemies())
            .with_sweep(600.0)
    };
    // Its user despawned mid-sweep: no hit after.
    let bell = user(&mut world, wave());
    let target = enemy(&mut world, Vec2::new(200.0, 0.0), 6.0);
    step(&mut world, &mut schedule);
    assert_eq!(use_ability(&mut world, bell, "spray"), Used::Used);
    for _ in 0..5 {
        step(&mut world, &mut schedule);
    }
    world.despawn(bell);
    for _ in 0..30 {
        step(&mut world, &mut schedule);
        assert!(struck(&world).is_empty(), "the front ended with its user");
    }
    assert_eq!(health(&world, target), 10.0);
    // A capture restored into the same World, as the template's load runs:
    // the run's entities despawned, the new run's spawned and named, the
    // save written over them.
    let bell = user(&mut world, wave());
    name(&mut world, bell, None, "bell").unwrap();
    assert_eq!(use_ability(&mut world, bell, "spray"), Used::Used);
    for _ in 0..5 {
        step(&mut world, &mut schedule);
    }
    let saved = capture_world(&world).unwrap();
    world.despawn(bell);
    let bell = user(&mut world, wave());
    name(&mut world, bell, None, "bell").unwrap();
    restore_world(&mut world, &saved, &(Loaded,)).unwrap();
    for _ in 0..30 {
        step(&mut world, &mut schedule);
        assert!(struck(&world).is_empty(), "the front ended with its run");
    }
    assert_eq!(health(&world, target), 10.0);
    assert!(world.resource::<ConeFronts>().0.is_empty());
}

#[test]
fn a_cone_carries_its_sheets_delivery() {
    let (mut world, mut schedule) = rig();
    let skilled = Ability::new()
        .with_tags(["damage.fire"])
        .with_payload(Payload::Cone(Box::new(
            Cone::new(200.0, 10.0, 1.0).with_filter(enemies()),
        )))
        .with_skill(Skill::new().with_damage("damage.fire", 7.0, 7.0));
    world
        .resource_mut::<DataRegistry>()
        .insert("blaze", skilled);
    let mut intent = Intent::default();
    intent.set_aim(Some(Vec2::X));
    let mage = world.spawn_with((
        Position(Vec2::ZERO),
        Transform::default(),
        Abilities::new().with(DataRef::by_id("blaze")),
        intent,
    ));
    let target = enemy(&mut world, Vec2::new(100.0, 0.0), 8.0);
    step(&mut world, &mut schedule);
    assert_eq!(use_ability(&mut world, mage, "blaze"), Used::Used);
    step(&mut world, &mut schedule);
    let hits: Vec<f32> = world
        .resource::<EventQueue<Damaged>>()
        .iter()
        .filter(|hit| hit.entity == target)
        .map(|hit| hit.amount)
        .collect();
    assert_eq!(hits, [7.0], "the sheet's fire 7, not the cone's 1");
    // The data form: needed fields, the defaults, a refused angle.
    let cone: Ability = serde_json::from_value(json!({
        "payload": [{ "cone": { "reach": 380, "half_angle": 14, "damage": 3, "tags": ["damage.cold"] } }]
    }))
    .unwrap();
    let Payload::Cone(read) = &cone.payload()[0] else {
        panic!("a cone");
    };
    assert_eq!(
        (read.rays(), read.sweep(), read.push(), read.far()),
        (7, None, 0.0, 1.0)
    );
    assert!(cone.check(&world).is_ok());
    for bad in [
        json!({ "reach": 380, "half_angle": 190 }),
        json!({ "reach": 0, "half_angle": 14 }),
        json!({ "reach": 380, "half_angle": 14, "far": 1.5 }),
        json!({ "reach": 380, "half_angle": 14, "rays": 0 }),
    ] {
        assert!(
            serde_json::from_value::<Ability>(json!({ "payload": [{ "cone": bad }] })).is_err(),
            "{bad}"
        );
    }
}
