use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, RigidBodyBundle, Velocity};
use tungsten_core::{Bundle, Plugin, Schedule};

use crate::TagsPlugin;

fn hero_filter() -> TagFilter {
    TagFilter::new().all(["team.player"])
}

fn tagged(world: &mut World, at: Vec2) -> Entity {
    world.spawn_with((Tags::new(["team.player"]), Position(at)))
}

#[test]
fn the_three_forms_round_trip_and_anything_else_is_refused() {
    for target in [
        Target::source(),
        Target::nearest(hero_filter().none(["boss"])).with_range(64.0),
        Target::point(Vec2::new(640.0, 360.0)),
    ] {
        let saved = serde_json::to_value(&target).unwrap();
        assert_eq!(serde_json::from_value::<Target>(saved).unwrap(), target);
    }
    assert_eq!(
        serde_json::to_value(Target::source()).unwrap(),
        json!({ "source": {} })
    );
    for refused in [
        json!({}),
        json!({ "source": {}, "point": [0, 0] }),
        json!({ "source": null }),
        json!({ "source": { "depth": 1 } }),
        json!({ "nearest": {} }),
        json!({ "nearest": { "none": ["boss"] } }),
        json!({ "nearest": { "all": ["Team"] } }),
        json!({ "nearest": { "all": ["team"], "range": 0 } }),
        json!({ "nearest": { "all": ["team"], "radius": 5 } }),
        json!({ "point": [1e39, 0] }),
        json!({ "entity": 3 }),
    ] {
        assert!(
            serde_json::from_value::<Target>(refused.clone()).is_err(),
            "{refused}"
        );
        assert!(
            serde_json::from_value::<TargetDef>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
#[should_panic(expected = "names no `all` or `any` tag")]
fn a_nearest_filter_of_none_alone_panics() {
    let _ = Target::nearest(TagFilter::new().none(["boss"]));
}

#[test]
#[should_panic(expected = "only a `nearest` target has a range")]
fn a_range_on_a_point_panics() {
    let _ = Target::point(Vec2::ZERO).with_range(10.0);
}

#[test]
#[should_panic(expected = "range must be finite and > 0")]
fn a_range_of_zero_panics() {
    let _ = Target::nearest(hero_filter()).with_range(0.0);
}

#[test]
fn a_source_resolves_while_it_lives() {
    let mut world = World::new();
    let nest = world.spawn_with((Position(Vec2::new(5.0, 6.0)),));
    let slime = world.spawn_with((Source::new(nest), Position(Vec2::ZERO)));
    let stray = world.spawn_with((Position(Vec2::ZERO),));
    let mut resolver = Resolver::default();
    let source = Target::source();
    assert_eq!(
        resolver.resolve(&world, &source, slime, Vec2::ZERO, Measure::Now),
        Some((Some(nest), Vec2::new(5.0, 6.0)))
    );
    assert_eq!(
        resolver.resolve(&world, &source, stray, Vec2::ZERO, Measure::Now),
        None
    );
    world.despawn(nest);
    assert_eq!(
        resolver.resolve(&world, &source, slime, Vec2::ZERO, Measure::Now),
        None
    );
}

#[test]
fn the_nearest_other_match_within_range_wins_ties_to_the_lower_index() {
    let mut world = World::new();
    let seeker = tagged(&mut world, Vec2::ZERO);
    let far = tagged(&mut world, Vec2::new(100.0, 0.0));
    let left = tagged(&mut world, Vec2::new(-30.0, 0.0));
    let right = tagged(&mut world, Vec2::new(30.0, 0.0));
    // An untagged entity nearer than any never matches.
    world.spawn_with((Position(Vec2::new(1.0, 0.0)),));
    let mut resolver = Resolver::default();
    resolver.begin();
    let nearest = Target::nearest(hero_filter());
    assert_eq!(
        resolver.resolve(&world, &nearest, seeker, Vec2::ZERO, Measure::Now),
        Some((Some(left), Vec2::new(-30.0, 0.0))),
        "itself excluded, the tie to the lower index"
    );
    assert!(left.id() < right.id());
    let within = Target::nearest(hero_filter()).with_range(20.0);
    assert_eq!(
        resolver.resolve(&world, &within, seeker, Vec2::ZERO, Measure::Now),
        None
    );
    let from_far = resolver.resolve(&world, &nearest, far, Vec2::new(100.0, 0.0), Measure::Now);
    assert_eq!(from_far.map(|(entity, _)| entity), Some(Some(right)));
}

#[test]
fn matches_are_gathered_once_a_run_per_distinct_filter() {
    let mut world = World::new();
    let a = tagged(&mut world, Vec2::ZERO);
    let b = tagged(&mut world, Vec2::new(10.0, 0.0));
    let mut resolver = Resolver::default();
    resolver.begin();
    let nearest = Target::nearest(hero_filter());
    resolver.resolve(&world, &nearest, a, Vec2::ZERO, Measure::Now);
    resolver.resolve(&world, &nearest, b, Vec2::new(10.0, 0.0), Measure::Now);
    assert_eq!(resolver.used, 1, "one filter, gathered once");
    let other = Target::nearest(TagFilter::new().any(["team"]));
    resolver.resolve(&world, &other, a, Vec2::ZERO, Measure::Now);
    assert_eq!(resolver.used, 2);
    // A match spawned since is found once the next run begins.
    let c = tagged(&mut world, Vec2::new(1.0, 0.0));
    assert_eq!(
        resolver
            .resolve(&world, &nearest, a, Vec2::ZERO, Measure::Now)
            .unwrap()
            .0,
        Some(b),
        "the run's matches stay"
    );
    resolver.begin();
    assert_eq!(
        resolver
            .resolve(&world, &nearest, a, Vec2::ZERO, Measure::Now)
            .unwrap()
            .0,
        Some(c)
    );
    assert_eq!(resolver.used, 1);
}

#[test]
fn a_placer_takes_an_integrated_body_where_the_step_leaves_it() {
    let mut world = World::new();
    let kinematic = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::splat(4.0)))
            .with((Tags::new(["team.player"]),)),
    );
    world.get_mut::<Velocity>(kinematic).unwrap().0 = Vec2::new(60.0, 0.0);
    let seeker = world.spawn_with((Source::new(kinematic), Position(Vec2::ZERO)));
    let mut resolver = Resolver::default();
    let placed = resolver.resolve(
        &world,
        &Target::source(),
        seeker,
        Vec2::ZERO,
        Measure::Placed(0.5),
    );
    assert_eq!(placed, Some((Some(kinematic), Vec2::new(30.0, 0.0))));
    // A bodiless entity's velocity is not integrated: its point stays.
    let bodiless = world.spawn_with((Position(Vec2::new(8.0, 0.0)), Velocity(Vec2::X)));
    *world.get_mut::<Source>(seeker).unwrap() = Source::new(bodiless);
    let placed = resolver.resolve(
        &world,
        &Target::source(),
        seeker,
        Vec2::ZERO,
        Measure::Placed(0.5),
    );
    assert_eq!(placed, Some((Some(bodiless), Vec2::new(8.0, 0.0))));
    let point = resolver.resolve(
        &world,
        &Target::point(Vec2::ONE),
        seeker,
        Vec2::ZERO,
        Measure::Now,
    );
    assert_eq!(point, Some((None, Vec2::ONE)));
}

#[test]
fn the_load_checks_each_tag_naming_its_field() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.player"])
        .build(&mut schedule, &mut world);
    let def: TargetDef = serde_json::from_value(
        json!({ "nearest": { "all": ["team.player"], "none": ["team.plyer"] } }),
    )
    .unwrap();
    let error = def.check(&world, "target").unwrap_err();
    assert_eq!(error.field, "target.nearest.none.0");
    assert_eq!(error.id, "team.plyer");
    let def: TargetDef = serde_json::from_value(json!({ "point": [1, 2] })).unwrap();
    assert!(def.check(&world, "target").is_ok());
}
