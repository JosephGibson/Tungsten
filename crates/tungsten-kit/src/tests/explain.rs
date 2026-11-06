use tungsten_core::{Plugin, Schedule, Time, World};

use super::*;
use crate::{DamageRequest, Health, HealthPlugin, KitRng, Penetration, Resist, Stats};

/// A world whose hits resolve, keeping why-records.
fn rig() -> World {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    HealthPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    world.insert_resource(KitExplain::new());
    world
}

#[test]
fn the_ring_keeps_its_latest_records_to_its_capacity() {
    let mut world = World::new();
    let [a, b, c] = [world.spawn(), world.spawn(), world.spawn()];
    let explain = KitExplain::with_capacity(2);
    for target in [a, b, c] {
        explain.push(ExplainRecord::Hit(HitRecord::new(target, None, false)));
    }
    let targets: Vec<Entity> = explain
        .records()
        .into_iter()
        .map(|record| match record {
            ExplainRecord::Hit(hit) => hit.target,
            ExplainRecord::Sheet(sheet) => sheet.holder,
        })
        .collect();
    assert_eq!(targets, [b, c], "the oldest dropped");
    assert_eq!(explain.last_hit().map(|hit| hit.target), Some(c));
    assert!(explain.last_sheet().is_none());
    assert_eq!(KitExplain::with_capacity(0).capacity, 1);
    let mut explain = explain;
    explain.clear();
    assert!(explain.is_empty());
}

#[test]
fn a_plain_hit_records_its_floor_penetration_and_damage_taken_a_line_each() {
    let mut world = rig();
    let wall = world.spawn_with((
        Health::new(100.0),
        Resist::new()
            .with_factor("damage.fire", 0.25)
            .with_floor("damage.fire", 0.5),
        Stats::new().with(Stats::DAMAGE_TAKEN, 0.5),
    ));
    DamageRequest::new(wall, 8.0)
        .with_tags(Tags::new(["damage.fire"]))
        .with_penetration(Penetration::new().with_points("damage.fire", 0.25))
        .apply(&mut world);
    let hit = world.resource::<KitExplain>().last_hit().unwrap();
    assert_eq!(
        (hit.amount, hit.crit, hit.draw),
        (3.0, CritRecord::None, None)
    );
    let part = &hit.parts[0];
    let resist = part.resist.unwrap();
    assert_eq!(
        (resist.factor, resist.floor, resist.pen, resist.product),
        (0.5, Some(0.5), 0.25, 0.75),
        "0.25 raised to its floor, then 0.25 points"
    );
    assert_eq!(
        (part.base, part.before, part.taken, part.after),
        (8.0, 8.0, 0.5, 3.0)
    );
    assert_eq!(
        hit.to_string(),
        format!(
            "hit on {wall}: 3.00\n\
             fire part  8.00\n\
             \x20 = 8.00 × 1.00 × 1.00 = 8.00; no crit\n\
             \x20 resist  damage.fire 0.50 + 0.25 pen = 0.75 (floor 0.50) → 6.00\n\
             \x20 damage_taken 0.50 → 3.00\n"
        )
    );
}

#[test]
fn a_drawn_crit_records_its_chance_roll_and_multiplier_and_a_decided_one_no_draw() {
    let mut world = rig();
    world.insert_resource(KitRng::seeded(5));
    let source = world.spawn_with((Stats::new()
        .with(Stats::CRIT_CHANCE, 1.0)
        .with(Stats::CRIT_MULTIPLIER, 2.0),));
    let target = world.spawn_with((Health::new(100.0),));
    DamageRequest::new(target, 4.0)
        .with_source(source)
        .apply(&mut world);
    let hit = world.resource::<KitExplain>().last_hit().unwrap();
    let roll = KitRng::seeded(5).unit();
    assert_eq!(
        hit.crit,
        CritRecord::Drawn {
            chance: 1.0,
            roll,
            multiplier: 2.0
        }
    );
    let part = &hit.parts[0];
    assert_eq!(
        (part.base, part.crit, part.before, part.after),
        (4.0, 2.0, 8.0, 8.0)
    );
    assert!(hit.to_string().contains(&format!(
        "untyped part  4.00\n  = 4.00 × 1.00 × 1.00 = 4.00; crit ×2.00 (draw {roll:.2} < 1.00) → 8.00\n  resist  none → 8.00\n"
    )));
    DamageRequest::new(target, 1.0)
        .with_crit(true)
        .apply(&mut world);
    let hit = world.resource::<KitExplain>().last_hit().unwrap();
    assert_eq!(hit.crit, CritRecord::Decided(true));
}
