//! The training dummy: the one the Lantern clearing holds, a hit's flash
//! and push, a frozen dummy's slide, pull and thaw, and the summon slot's
//! dummies at the cursor, four at most.

use example_01_platformer::features::combat::{Hit, Kind};
use example_01_platformer::features::dummy::components::Dummy;
use example_01_platformer::features::dummy::{self, DUMMY_CENTRE, SUMMON_CAP};
use example_01_platformer::features::player::PLAYER_HALF;
use example_01_platformer::features::toys;
use example_01_platformer::save::{MAP_ENEMIES, MAP_PICKUPS};
use example_01_platformer::shared::SaveId;
use glam::Vec2;
use tungsten::core::{Entity, Velocity};
use tungsten::kit::{HitFlash, Stats, Tags};
use tungsten::testing::UiHarness;

use super::combat::{TOP, apply, dummy_at, has, with_dummy};
use super::{
    aim_at, body, centre, choose, ground, kick, mana, pin_camera, player_entity, run, stand,
};

#[test]
fn the_clearing_holds_a_dummy_with_its_three_tags() {
    let (mut harness, player) = run();
    harness.step(60);
    let world = harness.world();
    let dummies: Vec<Entity> = world
        .query::<(Entity, &Dummy)>()
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(dummies.len(), 1, "one placed");
    let dummy = dummies[0];
    let tags = world.get::<Tags>(dummy).unwrap();
    for tag in ["flesh", "freezable", "conductive"] {
        assert!(tags.has(tag), "{tag}");
    }
    let at = body(&harness, dummy);
    assert!(at.x < 23.0 * 64.0, "in the Lantern clearing: {at}");
    assert!(
        (at.y - (36.0 * 64.0 - 28.0)).abs() < 2.0,
        "standing on the clearing's turf: {at}"
    );
    let id = |entity: Entity| harness.world().get::<SaveId>(entity).unwrap().0;
    assert_eq!(
        id(dummy) + 1 + MAP_ENEMIES + MAP_PICKUPS,
        id(player),
        "the map's dummies, then its enemies and its pickups"
    );
}

#[test]
fn a_hit_flashes_and_knocks_the_dummy_back() {
    let (mut harness, player, dummy) = with_dummy();
    let world = harness.world_mut();
    Hit::new(dummy, [(Kind::Physical, 5.0)])
        .from(player)
        .push(Vec2::new(300.0, 0.0))
        .queue(world);
    harness.step(1);
    let world = harness.world();
    assert!(world.get::<HitFlash>(dummy).unwrap().is_active());
    assert!(world.get::<Velocity>(dummy).unwrap().0.x > 100.0, "pushed");
    // No window: the next step's hit lands too.
    let before = super::combat::health(&harness, dummy);
    Hit::new(dummy, [(Kind::Physical, 5.0)]).queue(harness.world_mut());
    harness.step(1);
    assert!(super::combat::health(&harness, dummy) < before);
    let _ = player_entity(&harness);
}

#[test]
fn a_frozen_dummy_slides_is_pulled_by_the_hole_and_thaws() {
    let (mut harness, _, thawed) = with_dummy();
    let frozen = dummy_at(&mut harness, 1500.0);
    harness.step(2);
    apply(&mut harness, frozen, "frozen", None);
    assert!(has(&harness, frozen, "frozen"));
    let (a, b) = (body(&harness, thawed), body(&harness, frozen));
    kick(&mut harness, thawed, Vec2::new(500.0, 0.0));
    kick(&mut harness, frozen, Vec2::new(500.0, 0.0));
    harness.step(30);
    let slid = |at: Vec2, from: Vec2| at.x - from.x;
    let (thawed_slid, frozen_slid) = (
        slid(body(&harness, thawed), a),
        slid(body(&harness, frozen), b),
    );
    assert!(
        frozen_slid > thawed_slid + 40.0,
        "ice slides on: {frozen_slid} against {thawed_slid}"
    );
    // A hole between them pulls the frozen one only: each 150 px from it.
    let b = body(&harness, frozen);
    let hole = Vec2::new(b.x - 150.0, TOP - 40.0);
    super::teleport(&mut harness, thawed, Vec2::new(hole.x - 150.0, b.y));
    kick(&mut harness, thawed, Vec2::ZERO);
    kick(&mut harness, frozen, Vec2::ZERO);
    harness.step(1);
    let (a, b) = (body(&harness, thawed), body(&harness, frozen));
    toys::place_black_hole(harness.world_mut(), hole);
    harness.step(20);
    assert!(
        body(&harness, frozen).x < b.x - 2.0,
        "pulled toward the hole: {} from {}",
        body(&harness, frozen).x,
        b.x
    );
    assert!(
        (body(&harness, thawed).x - a.x).abs() < 1.0,
        "a thawed dummy is left be"
    );
    // Seven and a half seconds in all, then it thaws.
    harness.step(410);
    assert!(!has(&harness, frozen, "frozen"));
    assert!(
        !harness
            .world()
            .get::<Tags>(frozen)
            .unwrap()
            .has("status.frozen")
    );
}

/// The summoned dummies alive: the live dummies the map did not place, by
/// save ID.
fn summoned(harness: &UiHarness) -> Vec<(u64, Entity)> {
    dummy::live(harness.world())
        .into_iter()
        .filter(|&(id, _)| !dummy::placed(harness.world(), id))
        .collect()
}

/// Casts the selected slot at `at`: one press of `cast`.
fn cast_at(harness: &mut UiHarness, at: Vec2) {
    aim_at(harness, at);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
}

#[test]
fn summon_places_a_dummy_at_the_cursor_and_caps_the_summoned() {
    let (mut harness, player) = run();
    ground(&mut harness, TOP);
    stand(&mut harness, player, 300.0, TOP);
    harness.step(1);
    pin_camera(&mut harness, Vec2::new(0.0, 400.0));
    let stats = harness.world_mut().get_mut::<Stats>(player).unwrap();
    stats.set_base("max_mana", 1000.0);
    stats.set_base("mana", 1000.0);
    choose(&mut harness, 9);
    let placed: Vec<(u64, Entity)> = dummy::live(harness.world());
    let before = mana(&harness);
    let at = Vec2::new(800.0, 700.0);
    cast_at(&mut harness, at);
    let mine = summoned(&harness);
    assert_eq!(mine.len(), 1);
    let (_, first) = mine[0];
    assert!(body(&harness, first).distance(at) < 2.0, "at the cursor");
    assert_eq!(mana(&harness), before - 20.0, "a summon costs 20");
    // Five summons a second apart leave the four latest.
    for i in 1..=SUMMON_CAP {
        harness.step(61);
        cast_at(&mut harness, at + Vec2::new(150.0 * i as f32, 0.0));
    }
    let mine = summoned(&harness);
    assert_eq!(mine.len(), SUMMON_CAP);
    assert!(!harness.world().is_alive(first), "the oldest went");
    assert!(
        placed
            .iter()
            .all(|&(_, entity)| harness.world().is_alive(entity)),
        "the map's dummy stays"
    );
    // Over a solid tile it is refused before it spends anything.
    let (mut harness, player) = run();
    harness.step(30);
    choose(&mut harness, 9);
    harness.step(61);
    let feet = centre(&harness, player) + Vec2::Y * PLAYER_HALF.y;
    let before = mana(&harness);
    cast_at(&mut harness, feet + Vec2::Y * DUMMY_CENTRE.y);
    assert!(summoned(&harness).is_empty(), "refused in the ground");
    assert_eq!(mana(&harness), before, "and spent nothing");
}
