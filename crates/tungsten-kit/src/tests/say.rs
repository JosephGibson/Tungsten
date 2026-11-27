use tungsten_core::save::{capture_world, restore_world};
use tungsten_core::{Stage, Transform};

use super::*;

/// What a restore's spawns take.
#[derive(Debug, Clone, Copy)]
struct Restored;

fn world() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(WorldLabels::default());
    let mut schedule = Schedule::new();
    SayPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn at(x: f32) -> Transform {
    Transform::from_position(Vec2::new(x, 0.0))
}

/// One frame of `dt` game seconds: its labels, (text, anchor, alpha).
fn frame(world: &mut World, schedule: &mut Schedule, dt: f32) -> Vec<(String, Vec2, u8)> {
    world.resource_mut::<Time>().advance_frame(dt);
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
    world
        .resource_mut::<WorldLabels>()
        .drain()
        .map(|label| (label.text, label.anchor, label.color[3]))
        .collect()
}

fn said(text: &str, seconds: f32) -> SayStyle {
    SayStyle {
        seconds,
        ..SayStyle::new(text.to_owned())
    }
}

#[test]
fn a_bark_follows_its_entity_and_fades_at_its_end() {
    let (mut world, mut schedule) = world();
    assert!(
        frame(&mut world, &mut schedule, 0.1).is_empty(),
        "no barks: nothing"
    );
    let gull = world.spawn_with((at(0.0),));
    bark(&mut world, gull, &said("Fog's coming in.", 1.0));
    let first = frame(&mut world, &mut schedule, 0.1);
    assert_eq!(
        first,
        [("Fog's coming in.".to_owned(), Vec2::ZERO, 255)],
        "fresh at age 0"
    );
    world.get_mut::<Transform>(gull).unwrap().position = Vec2::new(50.0, 0.0);
    let moved = frame(&mut world, &mut schedule, 0.5);
    assert_eq!(moved[0].1, Vec2::new(50.0, 0.0), "over its entity");
    assert_eq!(moved[0].2, 255);
    let fading = frame(&mut world, &mut schedule, 0.4);
    assert!(
        fading[0].2 > 100 && fading[0].2 < 160,
        "half faded: {}",
        fading[0].2
    );
    assert!(
        frame(&mut world, &mut schedule, 0.2).is_empty(),
        "past its seconds"
    );
    assert!(world.resource::<Barks>().is_empty());

    bark(&mut world, gull, &said("Again.", 5.0));
    world.despawn(gull);
    assert!(
        frame(&mut world, &mut schedule, 0.1).is_empty(),
        "its entity gone"
    );
    assert!(world.resource::<Barks>().is_empty());
}

#[test]
fn a_new_bark_replaces_the_last() {
    let (mut world, mut schedule) = world();
    let keeper = world.spawn_with((at(0.0),));
    let warden = world.spawn_with((at(40.0),));
    bark(&mut world, keeper, &said("Evening.", 3.0));
    bark(&mut world, warden, &said("Evening.", 3.0));
    bark(&mut world, keeper, &said("Cold one.", 3.0));
    let barks = world.resource::<Barks>();
    assert_eq!(barks.len(), 2);
    assert_eq!(barks.text_of(keeper), Some("Cold one."));
    assert_eq!(barks.text_of(warden), Some("Evening."));
    assert_eq!(frame(&mut world, &mut schedule, 0.1).len(), 2);
}

#[test]
fn a_bark_holds_while_the_clock_does() {
    let (mut world, mut schedule) = world();
    let keeper = world.spawn_with((at(0.0),));
    bark(&mut world, keeper, &said("Wait.", 0.5));
    frame(&mut world, &mut schedule, 0.1);
    world.resource_mut::<Time>().set_paused(true);
    for _ in 0..120 {
        assert_eq!(frame(&mut world, &mut schedule, 0.1).len(), 1, "held");
    }
    world.resource_mut::<Time>().set_paused(false);
    assert!(
        frame(&mut world, &mut schedule, 0.6).is_empty(),
        "gone once play resumes"
    );
}

#[test]
fn the_bark_cap_drops_the_oldest() {
    let (mut world, mut schedule) = world();
    let entities: Vec<Entity> = (0..=MAX_BARKS)
        .map(|i| world.spawn_with((at(i as f32),)))
        .collect();
    for (i, &entity) in entities.iter().enumerate() {
        bark(&mut world, entity, &said(&format!("{i}"), 3.0));
    }
    let barks = world.resource::<Barks>();
    assert_eq!(barks.len(), MAX_BARKS);
    assert_eq!(barks.text_of(entities[0]), None, "the oldest dropped");
    assert_eq!(barks.text_of(entities[MAX_BARKS]), Some("64"));
    assert_eq!(frame(&mut world, &mut schedule, 0.1).len(), MAX_BARKS);
}

#[test]
fn a_restore_clears_the_barks() {
    let (mut world, mut schedule) = world();
    let keeper = world.spawn_with((at(0.0),));
    let saved = capture_world(&world).unwrap();
    bark(&mut world, keeper, &said("Evening.", 3.0));
    restore_world(&mut world, &saved, &(Restored,)).unwrap();
    assert!(world.resource::<Barks>().is_empty());
    assert!(frame(&mut world, &mut schedule, 0.1).is_empty());
}
