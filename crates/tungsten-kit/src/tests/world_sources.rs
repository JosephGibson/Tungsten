use serde_json::json;
use tungsten_core::{Plugin, Schedule, Stage, Time};

use super::*;
use crate::damage::health::{Health, HealthPlugin};
use crate::foundation::stats::StatsPlugin;
use crate::{Modifier, Read, TagSet};

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    StatsPlugin::new().build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

fn enemies() -> TagFilter {
    TagFilter::new().all(["team.enemy"])
}

/// Hard: `mul 1.5` on the enemies' health and damage.
fn hard() -> WorldSource {
    WorldSource::new(enemies())
        .with_line(Stats::MAX_HEALTH, Line::new(Modifier::Mul(1.5)))
        .with_line(Stats::DAMAGE, Line::new(Modifier::Mul(1.5)))
}

fn grunt(world: &mut World) -> Entity {
    world.spawn_with((
        Tags::new(["team.enemy"]),
        Health::new(40.0),
        Stats::new()
            .with(Stats::MAX_HEALTH, 40.0)
            .with(Stats::DAMAGE, 10.0),
    ))
}

fn stat(world: &World, entity: Entity, name: &str) -> Option<f32> {
    world.get::<Stats>(entity).and_then(|stats| stats.get(name))
}

#[test]
fn a_world_source_reaches_the_living_at_the_next_step_and_a_removed_one_leaves() {
    let (mut world, mut schedule) = rig();
    let grunt = grunt(&mut world);
    let hero = world.spawn_with((
        Tags::new(["team.player"]),
        Stats::new().with(Stats::DAMAGE, 5.0),
    ));
    step(&mut world, &mut schedule);
    WorldSources::set(&mut world, "difficulty", hard());
    assert_eq!(
        stat(&world, grunt, Stats::DAMAGE),
        Some(10.0),
        "not before the step"
    );
    step(&mut world, &mut schedule);
    assert_eq!(stat(&world, grunt, Stats::DAMAGE), Some(15.0));
    assert_eq!(stat(&world, grunt, Stats::MAX_HEALTH), Some(60.0));
    let health = world.get::<Health>(grunt).unwrap();
    assert_eq!(
        (health.current(), health.max()),
        (60.0, 60.0),
        "its item too"
    );
    assert_eq!(stat(&world, hero, Stats::DAMAGE), Some(5.0), "the filter");

    assert!(WorldSources::remove(&mut world, "difficulty"));
    assert!(!WorldSources::remove(&mut world, "difficulty"), "gone");
    assert!(!WorldSources::remove(&mut world, "weather"), "never held");
    step(&mut world, &mut schedule);
    assert_eq!(stat(&world, grunt, Stats::DAMAGE), Some(10.0));
    assert_eq!(world.get::<Health>(grunt).unwrap().max(), 40.0);
    let stats = world.get::<Stats>(grunt).unwrap();
    assert_eq!(
        serde_json::to_value(stats).unwrap(),
        json!({
            "damage": { "base": 10.0, "min": 0.0, "max": null },
            "max_health": { "base": 40.0, "min": 1.0, "max": null }
        }),
        "no line left behind"
    );
}

#[test]
fn apply_to_brings_one_entity_up_to_date_at_once() {
    let (mut world, _) = rig();
    let first = grunt(&mut world);
    let second = grunt(&mut world);
    WorldSources::apply_to(&mut world, first);
    assert_eq!(
        stat(&world, first, Stats::DAMAGE),
        Some(10.0),
        "no sources yet"
    );
    WorldSources::set(&mut world, "difficulty", hard());
    WorldSources::apply_to(&mut world, first);
    assert_eq!(stat(&world, first, Stats::DAMAGE), Some(15.0), "at once");
    assert_eq!(
        stat(&world, second, Stats::DAMAGE),
        Some(10.0),
        "one entity"
    );
    WorldSources::apply_to(&mut world, first);
    assert_eq!(stat(&world, first, Stats::DAMAGE), Some(15.0), "once");
    let bare = world.spawn();
    WorldSources::apply_to(&mut world, bare);

    // A tag gained after the source reached it is read by `apply_to`.
    let hero = world.spawn_with((
        Tags::new(["team.player"]),
        Stats::new().with(Stats::DAMAGE, 4.0),
    ));
    WorldSources::apply_to(&mut world, hero);
    assert_eq!(stat(&world, hero, Stats::DAMAGE), Some(4.0));
    world.get_mut::<Tags>(hero).unwrap().insert("team.enemy");
    WorldSources::apply_to(&mut world, hero);
    assert_eq!(stat(&world, hero, Stats::DAMAGE), Some(6.0));
}

#[test]
fn a_spawn_takes_the_sources_in_the_step_its_stats_first_apply() {
    let (mut world, mut schedule) = rig();
    WorldSources::set(&mut world, "difficulty", hard());
    step(&mut world, &mut schedule);
    let late = grunt(&mut world);
    step(&mut world, &mut schedule);
    let health = world.get::<Health>(late).unwrap();
    assert_eq!((health.current(), health.max()), (60.0, 60.0));
    assert_eq!(
        world.get::<Stats>(late).unwrap().revision(),
        world.resource::<WorldSources>().revision()
    );
}

#[test]
fn an_equal_set_bumps_nothing_and_a_new_one_replaces_the_lines() {
    let (mut world, mut schedule) = rig();
    let grunt = grunt(&mut world);
    WorldSources::set(&mut world, "difficulty", hard());
    let revision = world.resource::<WorldSources>().revision();
    WorldSources::set(&mut world, "difficulty", hard());
    assert_eq!(world.resource::<WorldSources>().revision(), revision);
    step(&mut world, &mut schedule);
    let easy = WorldSource::new(enemies()).with_line(Stats::DAMAGE, Line::new(Modifier::Mul(0.75)));
    WorldSources::set(&mut world, "difficulty", easy.clone());
    assert_eq!(world.resource::<WorldSources>().revision(), revision + 1);
    assert_eq!(
        world.resource::<WorldSources>().get("difficulty"),
        Some(&easy)
    );
    step(&mut world, &mut schedule);
    assert_eq!(stat(&world, grunt, Stats::DAMAGE), Some(7.5));
    assert_eq!(
        stat(&world, grunt, Stats::MAX_HEALTH),
        Some(40.0),
        "the old line left"
    );
    let names: Vec<&str> = world
        .resource::<WorldSources>()
        .iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names, ["difficulty"]);
}

#[test]
fn a_scoped_source_line_is_kept_for_reads_in_context() {
    let (mut world, mut schedule) = rig();
    let grunt = grunt(&mut world);
    let stormy = WorldSource::new(enemies()).with_line(
        Stats::DAMAGE,
        Line::new(Modifier::Pct(0.5)).with(TagFilter::new().all(["damage.lightning"])),
    );
    WorldSources::set(&mut world, "weather", stormy);
    step(&mut world, &mut schedule);
    let stats = world.get::<Stats>(grunt).unwrap();
    assert_eq!(stats.get(Stats::DAMAGE), Some(10.0));
    let bolt = [TagSet::Name("damage.lightning")];
    assert_eq!(stats.read(Stats::DAMAGE, &Read::new(&bolt)), Some(15.0));
    step(&mut world, &mut schedule);
    assert_eq!(
        world.get::<Stats>(grunt).unwrap().lines().count(),
        1,
        "placed once"
    );
}

#[test]
fn a_loaded_world_set_again_keeps_every_value_and_every_modifiers_order() {
    let (mut world, mut schedule) = rig();
    let grunt = grunt(&mut world);
    // A game's own modifier before the source's, one after.
    world
        .get_mut::<Stats>(grunt)
        .unwrap()
        .add_modifier(Stats::DAMAGE, "rage", Modifier::Pct(0.5));
    WorldSources::set(&mut world, "difficulty", hard());
    WorldSources::apply_to(&mut world, grunt);
    world.get_mut::<Stats>(grunt).unwrap().add_modifier(
        Stats::DAMAGE,
        "curse",
        Modifier::Add(-1.0),
    );
    let saved = serde_json::to_string(world.get::<Stats>(grunt).unwrap()).unwrap();
    assert!(
        saved.contains(
            r#"[["rage",{"pct":0.5}],["difficulty",{"mul":1.5}],["curse",{"add":-1.0}]]"#
        ),
        "{saved}"
    );

    // A new run of the same world: the stats loaded, the source set again
    // at the run's start, then a step and a direct apply.
    let (mut loaded, mut schedule2) = rig();
    let stats: Stats = serde_json::from_str(&saved).unwrap();
    let back = loaded.spawn_with((Tags::new(["team.enemy"]), Health::new(40.0), stats));
    WorldSources::set(&mut loaded, "difficulty", hard());
    step(&mut loaded, &mut schedule2);
    WorldSources::apply_to(&mut loaded, back);
    let again = serde_json::to_string(loaded.get::<Stats>(back).unwrap()).unwrap();
    assert_eq!(again, saved, "every value and every modifier's order");
    assert_eq!(
        stat(&loaded, back, Stats::DAMAGE),
        stat(&world, grunt, Stats::DAMAGE)
    );
    step(&mut world, &mut schedule);
    assert_eq!(
        serde_json::to_string(world.get::<Stats>(grunt).unwrap()).unwrap(),
        saved
    );
}

#[test]
fn a_world_without_sources_meets_no_stale_stats() {
    let (mut world, mut schedule) = rig();
    let grunt = grunt(&mut world);
    step(&mut world, &mut schedule);
    assert!(!world.has_resource::<WorldSources>());
    assert_eq!(world.get::<Stats>(grunt).unwrap().revision(), 0);
    assert!(!WorldSources::remove(&mut world, "difficulty"));
}

#[test]
#[should_panic(expected = "is not a source name")]
fn a_bad_source_name_panics() {
    let (mut world, _) = rig();
    WorldSources::set(&mut world, "effect#1", hard());
}
