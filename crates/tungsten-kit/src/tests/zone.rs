use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, PhysicsPlugin, RigidBodyBundle, Velocity};
use tungsten_core::{Bundle, ComponentRegistry, PrefabData, PrefabRegistry, Stage, Time};

use crate::TagsPlugin;

const DT: f32 = 1.0 / 60.0;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    TagsPlugin::new()
        .with_tags(["team.player", "team.enemy"])
        .build(&mut schedule, &mut world);
    ZonePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn door(world: &mut World, filter: TagFilter) -> Entity {
    world.spawn_with(
        RigidBodyBundle::sensor(Position::new(100.0, 0.0), Collider::aabb(Vec2::splat(8.0)))
            .with((Zone::new(filter),)),
    )
}

fn runner(world: &mut World, y: f32, tags: Option<Tags>) -> Entity {
    let body = RigidBodyBundle::kinematic(Position::new(0.0, y), Collider::aabb(Vec2::splat(4.0)))
        .with_velocity(Velocity::new(600.0, 0.0));
    match tags {
        Some(tags) => world.spawn_with(body.with((tags,))),
        None => world.spawn_with(body),
    }
}

/// Runs `count` steps, collecting the entries.
fn steps(world: &mut World, schedule: &mut Schedule, count: usize) -> Vec<ZoneEntered> {
    let mut entered = Vec::new();
    for _ in 0..count {
        world.resource_mut::<Time>().advance_frame(DT);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
        entered.extend(
            world
                .resource::<EventQueue<ZoneEntered>>()
                .iter_current()
                .copied(),
        );
        world.flush_events();
    }
    entered
}

#[test]
fn the_plugin_registers_its_queue_component_and_system() {
    let (world, schedule) = rig();
    assert!(world.has_event::<ZoneEntered>());
    assert_eq!(schedule.stage_of(ZONE_TICK), Some(Stage::FixedUpdate));
    assert!(world.resource::<ComponentRegistry>().contains("zone"));
}

#[test]
fn only_the_entities_passing_the_filter_enter() {
    let (mut world, mut schedule) = rig();
    let door = door(&mut world, TagFilter::new().all(["team.player"]));
    let hero = runner(&mut world, 0.0, Some(Tags::new(["team.player"])));
    runner(&mut world, 2.0, Some(Tags::new(["team.enemy"])));
    runner(&mut world, -2.0, None);
    let entered = steps(&mut world, &mut schedule, 20);
    assert_eq!(entered.len(), 1);
    assert_eq!((entered[0].zone, entered[0].entity), (door, hero));
    let x = entered[0].position.x;
    assert!((88.0..=100.0).contains(&x), "where it met the door: {x}");
    assert!(world.get::<Position>(hero).unwrap().0.x > x, "it went on");
}

#[test]
fn an_empty_filter_takes_every_body() {
    let (mut world, mut schedule) = rig();
    door(&mut world, TagFilter::new());
    runner(&mut world, 0.0, Some(Tags::new(["team.enemy"])));
    runner(&mut world, 2.0, None);
    assert_eq!(steps(&mut world, &mut schedule, 20).len(), 2);
}

#[test]
fn a_sensor_without_a_zone_sends_nothing() {
    let (mut world, mut schedule) = rig();
    world.spawn_with(RigidBodyBundle::sensor(
        Position::new(100.0, 0.0),
        Collider::aabb(Vec2::splat(8.0)),
    ));
    runner(&mut world, 0.0, Some(Tags::new(["team.player"])));
    assert!(steps(&mut world, &mut schedule, 20).is_empty());
}

#[test]
fn the_scene_component_checks_its_filter_and_spawns_the_zone() {
    let (mut world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    let error = components
        .check_in(
            "zone",
            &json!({ "filter": { "any": ["team.player", "team.ghost"] } }),
            &world,
        )
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("team.ghost") && error.contains("filter.any.1"),
        "{error}"
    );
    assert!(
        components
            .check_in("zone", &json!({ "filter": { "size": 2 } }), &world)
            .is_err()
    );
    assert!(components.check_in("zone", &json!({}), &world).is_ok());

    let trap: PrefabData = serde_json::from_value(json!({
        "components": {
            "rigid_body": { "kind": "sensor", "shape": { "kind": "aabb", "half_extents": [8, 8] } },
            "zone": { "filter": { "all": ["team.player"] } }
        }
    }))
    .unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("trap".to_string(), trap);
    world.insert_resource(prefabs);
    let entity = world
        .prepare_prefab("trap", [0.0, 0.0])
        .unwrap()
        .spawn(&mut world);
    assert_eq!(
        world.get::<Zone>(entity).unwrap().filter(),
        &TagFilter::new().all(["team.player"])
    );
}

#[test]
fn a_zone_is_data() {
    let zone = Zone::new(TagFilter::new().none(["boss"]));
    let value = serde_json::to_value(&zone).unwrap();
    assert_eq!(value, json!({ "filter": { "none": ["boss"] } }));
    assert_eq!(serde_json::from_value::<Zone>(value).unwrap(), zone);
}

#[test]
fn an_entry_goes_in_the_trace_with_the_entrant() {
    use crate::foundation::trace::{KitTrace, TraceKind};

    let (mut world, mut schedule) = rig();
    world.insert_resource(KitTrace::default());
    let zone = door(&mut world, TagFilter::new().all(["team.player"]));
    let hero = runner(&mut world, 0.0, Some(Tags::new(["team.player"])));
    steps(&mut world, &mut schedule, 20);
    let trace = world.resource::<KitTrace>();
    let entries: Vec<_> = trace.about(hero).collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].entity, zone);
    assert_eq!(
        entries[0].kind,
        TraceKind::Event {
            name: "zone_entered",
            other: Some(hero),
            amount: None
        }
    );
}
