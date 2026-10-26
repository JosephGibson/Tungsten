//! Props: a crate set alight burns down, lighting its neighbours, and falls
//! in; cold and a black hole put one out with its fuel kept; a lit keg
//! blows after its fuse and its neighbours follow in waves; a bullet, a
//! hard landing and lightning each set a keg off, a blow by its weight, a
//! small ball's never and a short fall's not; a frozen keg shatters in
//! fire and never blows; a detonation's credit passes through the endings
//! it sets off; two kegs blowing in one step blow once each; and a bomb
//! rolls with its fuse turning on it, while a goblin's rock stays a body
//! that rolls, the oldest crumbling past the cap.

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::elements::cascade::{CascadeKind, Cascades};
use example_01_platformer::features::elements::components::{BallBurn, Frozen};
use example_01_platformer::features::elements::{self};
use example_01_platformer::features::enemies::components::EnemyKind;
use example_01_platformer::features::enemies::{self};
use example_01_platformer::features::gun;
use example_01_platformer::features::lightning;
use example_01_platformer::features::props::components::{Fuel, FuseEmitter, PropKind};
use example_01_platformer::features::props::systems::bomb_tip;
use example_01_platformer::features::props::{
    self, BOMB_RADIUS, BURN_SECS, CRATE_HALF, KEG_BLAST, KEG_HALF, ROCK_CAP, ROCK_RADIUS,
    SPREAD_AFTER,
};
use example_01_platformer::features::toys::{self, SMALL_BALL_RADIUS};
use example_01_platformer::shared::{SaveId, SaveIds};
use example_01_platformer::shell::Difficulty;
use glam::Vec2;
use tungsten::core::{Entity, Position, RigidBody, Transform, Velocity, World};
use tungsten::kit::{DamagedPart, ImmunePart, Stats};
use tungsten::testing::UiHarness;

use super::combat::{TOP, apply, dummy_at};
use super::{aim_at, ball, body, bursts, choose, ground, pin_camera, run, stand};

/// A run on the sky's ground at one step a frame, the courier at `x`, the
/// camera held over it, with mana to spare.
fn sky(x: f32) -> (UiHarness, Entity) {
    let (mut harness, player) = run();
    harness.harness_mut().set_dt(1.0 / 60.0);
    ground(&mut harness, TOP);
    stand(&mut harness, player, x, TOP);
    harness.step(1);
    pin_camera(&mut harness, Vec2::new(x - 400.0, 500.0));
    let stats = harness.world_mut().get_mut::<Stats>(player).unwrap();
    stats.set_base("max_mana", 10_000.0);
    stats.set_base("mana", 10_000.0);
    (harness, player)
}

/// A prop of `kind` resting on the sky's ground, its centre at `x`.
fn prop(harness: &mut UiHarness, kind: PropKind, x: f32) -> Entity {
    let half = props::half(kind);
    props::spawn(harness.world_mut(), kind, Vec2::new(x, TOP - half.y))
}

fn fuel(harness: &UiHarness, crate_: Entity) -> Fuel {
    *harness.world().get::<Fuel>(crate_).expect("a crate's fuel")
}

fn alive(harness: &UiHarness, entity: Entity) -> bool {
    harness.world().is_alive(entity)
}

fn id_of(world: &World, entity: Entity) -> u64 {
    world.get::<SaveId>(entity).expect("a save ID").0
}

/// The sounds of `sound` this frame.
fn sounds(harness: &UiHarness, sound: Sound) -> usize {
    harness
        .harness()
        .events::<Sounded>()
        .filter(|sounded| sounded.sound == sound)
        .count()
}

/// Holds `cast` on slot `slot` aimed at `at`, a frame at a time, at most
/// `frames`, until `done`; whether it came.
fn spray_until(
    harness: &mut UiHarness,
    slot: usize,
    at: Vec2,
    frames: u32,
    mut done: impl FnMut(&UiHarness) -> bool,
) -> bool {
    aim_at(harness, at);
    choose(harness, slot);
    harness.harness_mut().press_action("cast");
    let mut came = false;
    for _ in 0..frames {
        harness.step(1);
        if done(harness) {
            came = true;
            break;
        }
    }
    harness.harness_mut().release_action("cast");
    came
}

#[test]
fn a_crate_set_alight_burns_down_and_falls_in() {
    let (mut harness, _) = sky(730.0);
    let lit = prop(&mut harness, PropKind::Crate, 600.0);
    let near = prop(
        &mut harness,
        PropKind::Crate,
        600.0 - 2.0 * CRATE_HALF.x - 3.0,
    );
    // A small ball resting 1 px off its right face, under the flames' aim.
    let edge = 600.0 + CRATE_HALF.x + 1.0 + SMALL_BALL_RADIUS;
    let small = ball(&mut harness, Vec2::new(edge, TOP - SMALL_BALL_RADIUS), true);
    harness.step(2);
    let face = Vec2::new(600.0 + CRATE_HALF.x, TOP - 50.0);
    assert!(
        spray_until(&mut harness, 1, face, 60, |h| fuel(h, lit).lit),
        "the flamethrower lights it"
    );
    assert!(
        harness
            .world()
            .get::<BallBurn>(small)
            .is_some_and(|burn| burn.remaining > 0.0),
        "the ball by its box at once"
    );
    assert!(!fuel(&harness, near).lit, "its neighbour not yet");
    let (mut caught, mut fell) = (None, None);
    for frame in 1..=(BURN_SECS * 60.0) as u32 + 30 {
        harness.step(1);
        if caught.is_none() && alive(&harness, near) && fuel(&harness, near).lit {
            caught = Some(frame);
        }
        if fell.is_none() && !alive(&harness, lit) {
            fell = Some(frame);
            assert!(bursts(&harness, "ex10_crate_collapse") > 0, "it falls in");
        }
    }
    let spread = (SPREAD_AFTER * 60.0) as u32;
    let caught = caught.expect("its neighbour caught");
    assert!(caught.abs_diff(spread) <= 1, "3 px away at 2 s: {caught}");
    let fell = fell.expect("it burned down");
    let burn = (BURN_SECS * 60.0) as u32;
    assert!(fell.abs_diff(burn) <= 1, "8 s: {fell}");
}

#[test]
fn cold_and_a_hole_put_a_crate_out_with_its_fuel_kept() {
    let (mut harness, _) = sky(300.0);
    let wet = prop(&mut harness, PropKind::Crate, 520.0);
    harness.step(2);
    assert!(props::light(harness.world_mut(), wet, None));
    harness.step(60);
    let at = body(&harness, wet);
    let steam = bursts(&harness, "ex10_steam_burst");
    assert!(
        spray_until(&mut harness, 2, at, 30, |h| !fuel(h, wet).lit),
        "the ice spray puts it out"
    );
    assert!(bursts(&harness, "ex10_steam_burst") > steam, "with steam");
    let kept = fuel(&harness, wet).left;
    assert!(
        (kept - (BURN_SECS - 1.0)).abs() < 0.1,
        "its fuel kept: {kept}"
    );
    // It cannot light for the douse's 4 s, then it burns its fuel out.
    harness.step(238);
    assert!(
        !props::light(harness.world_mut(), wet, None),
        "still doused"
    );
    harness.step(3);
    let left = fuel(&harness, wet).left;
    assert!(props::light(harness.world_mut(), wet, None), "dry again");
    let mut fell = None;
    for frame in 1..=(left * 60.0) as u32 + 30 {
        harness.step(1);
        if !alive(&harness, wet) {
            fell = Some(frame);
            break;
        }
    }
    let fell = fell.expect("it burned down");
    let due = (left * 60.0).round() as u32;
    assert!(fell.abs_diff(due) <= 1, "its fuel left: {fell} of {due}");
    // In a hole's reach a crate goes out and never lights again.
    let held = prop(&mut harness, PropKind::Crate, 900.0);
    harness.step(2);
    assert!(props::light(harness.world_mut(), held, None));
    toys::place_black_hole(harness.world_mut(), Vec2::new(900.0, TOP - 300.0));
    harness.step(1);
    assert!(!fuel(&harness, held).lit, "the hole puts it out");
    for _ in 0..300 {
        harness.step(1);
        let world = harness.world_mut();
        assert!(!props::light(world, held, None), "never relit");
    }
}

#[test]
fn a_lit_keg_blows_after_its_fuse_and_its_neighbours_follow() {
    let (mut harness, _) = sky(100.0);
    let kegs: Vec<Entity> = [1000.0, 1150.0, 1300.0]
        .into_iter()
        .map(|x| prop(&mut harness, PropKind::Keg, x))
        .collect();
    let near = dummy_at(&mut harness, 900.0);
    let far = dummy_at(&mut harness, 1300.0 + KEG_BLAST + 40.0);
    harness.step(2);
    let lighter = id_of(harness.world(), far);
    assert!(props::light(harness.world_mut(), kegs[0], Some(lighter)));
    let mut gone: [Option<u32>; 3] = [None; 3];
    let mut parts: Vec<(bool, f32, Option<Entity>)> = Vec::new();
    let mut far_hit = false;
    for frame in 1..=120 {
        harness.step(1);
        for (i, &keg) in kegs.iter().enumerate() {
            if gone[i].is_none() && !alive(&harness, keg) {
                gone[i] = Some(frame);
            }
        }
        let world = harness.world();
        for part in harness.harness().events::<DamagedPart>() {
            if part.periodic {
                continue;
            }
            if part.entity == near {
                let fire = part.kinds.has(world, "damage.fire");
                parts.push((fire, part.amount, part.source));
            }
            far_hit |= part.entity == far;
        }
    }
    let first = gone[0].expect("the lit keg blew");
    assert!(first.abs_diff(90) <= 1, "after its 1.5 s fuse: {first}");
    assert_eq!(gone[1], Some(first + 1), "the next in the second wave");
    assert_eq!(gone[2], Some(first + 2), "the last in the third");
    assert_eq!(parts.len(), 2, "one fire and one physical part: {parts:?}");
    for (fire, amount, source) in &parts {
        assert!((amount - KEG_BLAST / 8.0).abs() < 1e-3, "{fire}: {amount}");
        assert_eq!(*source, Some(far), "from its lighter");
    }
    assert!(parts.iter().any(|part| part.0) && parts.iter().any(|part| !part.0));
    assert!(!far_hit, "past 192 px, untouched");
}

#[test]
fn a_bullet_a_hard_landing_and_lightning_each_set_a_keg_off() {
    let (mut harness, player) = sky(300.0);
    let shot = prop(&mut harness, PropKind::Keg, 700.0);
    let struck = prop(&mut harness, PropKind::Keg, 1800.0);
    let dropped = props::spawn(
        harness.world_mut(),
        PropKind::Keg,
        Vec2::new(1250.0, TOP - 400.0),
    );
    harness.step(2);
    // A shot from the courier at the first.
    let target = body(&harness, shot);
    aim_at(&mut harness, target);
    harness.harness_mut().press_action(gun::SHOOT);
    harness.step(1);
    harness.harness_mut().release_action(gun::SHOOT);
    let mut booms = 0;
    for _ in 0..60 {
        harness.step(1);
        booms += sounds(&harness, Sound::KegBlasted);
    }
    assert!(!alive(&harness, shot), "the shot set it off");
    // The dropped one landed hard on the ground meanwhile.
    assert!(!alive(&harness, dropped), "the landing set it off");
    assert_eq!(booms, 2);
    // A bolt on the third.
    let at = body(&harness, struck);
    lightning::strike(harness.world_mut(), player, at);
    for _ in 0..3 {
        harness.step(1);
        booms += sounds(&harness, Sound::KegBlasted);
    }
    assert!(!alive(&harness, struck), "the bolt set it off");
    assert_eq!(booms, 3);
}

#[test]
fn a_keg_blows_from_a_blow_as_hard_as_its_weight_asks() {
    // Q46: by the other body's mass.
    assert!(!elements::blows_keg(1.0, 10_000.0), "a small ball never");
    assert!(!elements::blows_keg(3.0, 10_000.0), "a goblin's bomb never");
    assert!(!elements::blows_keg(
        f32::INFINITY,
        elements::KEG_SPEED - 1.0
    ));
    assert!(elements::blows_keg(f32::INFINITY, elements::KEG_SPEED));
    assert!(
        elements::blows_keg(8.0, elements::KEG_SPEED),
        "a bowling ball"
    );
    assert!(elements::blows_keg(400.0, elements::KEG_SPEED), "iron");
    assert!(
        !elements::blows_keg(6.0, elements::KEG_SPEED),
        "a rock, faster"
    );
    assert!(elements::blows_keg(6.0, elements::KEG_SPEED * 8.0 / 6.0));
    // In play: a 120 px drop, a small ball and a bowling ball.
    let (mut harness, _) = sky(300.0);
    let dropped = props::spawn(
        harness.world_mut(),
        PropKind::Keg,
        Vec2::new(700.0, TOP - KEG_HALF.y - 120.0),
    );
    let pelted = prop(&mut harness, PropKind::Keg, 1100.0);
    let bowled = prop(&mut harness, PropKind::Keg, 1500.0);
    harness.step(2);
    let world = harness.world_mut();
    let small = toys::spawn_ball(world, Vec2::new(1000.0, TOP - KEG_HALF.y), true);
    world.get_mut::<Velocity>(small).unwrap().0 = Vec2::X * 2700.0;
    let bowling = toys::spawn_ball(world, Vec2::new(1400.0, TOP - KEG_HALF.y), false);
    world.get_mut::<Velocity>(bowling).unwrap().0 = Vec2::X * 1200.0;
    for _ in 0..60 {
        harness.step(1);
    }
    assert!(alive(&harness, dropped), "a short fall leaves it");
    assert!(alive(&harness, pelted), "a small ball never sets one off");
    assert!(!alive(&harness, bowled), "a fast bowling ball does");
}

#[test]
fn a_frozen_keg_shatters_in_fire_and_never_blows() {
    let (mut harness, _) = sky(450.0);
    let keg = prop(&mut harness, PropKind::Keg, 600.0);
    harness.step(2);
    elements::freeze_iron(harness.world_mut(), keg);
    assert!(harness.world().has::<Frozen>(keg), "frozen through");
    let world = harness.world_mut();
    assert!(!props::light(world, keg, None), "its fuse never lights");
    assert!(
        !props::set_off(world, keg, None, None),
        "nor does it go off"
    );
    let at = body(&harness, keg);
    let mut blasts = (0, 0);
    let gone = spray_until(&mut harness, 1, at, 30, |h| {
        blasts.0 += sounds(h, Sound::FrostBlasted);
        blasts.1 += sounds(h, Sound::KegBlasted);
        !alive(h, keg)
    });
    assert!(gone, "fire on it is thermal shock");
    assert_eq!(blasts, (1, 0), "one frost blast, no detonation");
    assert_eq!(bursts(&harness, "ex10_keg_staves"), 0);
}

/// An enemy of `kind` with its body's centre at `centre`.
fn enemy(harness: &mut UiHarness, kind: EnemyKind, centre: Vec2) -> Entity {
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let entity = enemies::spawn(world, kind, centre - kind.info().centre, id).expect("its prefab");
    enemies::scale(world, entity, Difficulty::Normal);
    entity
}

#[test]
fn a_detonations_credit_passes_through_the_endings_it_sets_off() {
    let (mut harness, _) = sky(100.0);
    let lighter = dummy_at(&mut harness, 2000.0);
    let keg = prop(&mut harness, PropKind::Keg, 1000.0);
    // Its blast reaches a frozen ball (the bin's thermal shock), a frozen
    // dummy (the reactions' landed fire) and a frozen wisp (their immune
    // fire); the dummy's explosion reaches the second keg, past the first's.
    let small = ball(
        &mut harness,
        Vec2::new(1060.0, TOP - SMALL_BALL_RADIUS),
        true,
    );
    let frozen = dummy_at(&mut harness, 880.0);
    let wisp = enemy(
        &mut harness,
        EnemyKind::Wisp,
        Vec2::new(1000.0, TOP - 150.0),
    );
    let second = prop(&mut harness, PropKind::Keg, 760.0);
    harness.step(2);
    let credit = id_of(harness.world(), lighter);
    assert!(props::light(harness.world_mut(), keg, Some(credit)));
    harness.step(85);
    elements::freeze_iron(harness.world_mut(), small);
    apply(&mut harness, frozen, "frozen", None);
    apply(&mut harness, wisp, "frozen", None);
    let mut hits: Vec<(Entity, Option<Entity>, bool)> = Vec::new();
    let mut entries: Vec<(CascadeKind, Option<u64>)> = Vec::new();
    for _ in 0..12 {
        harness.step(1);
        let landed = harness.harness().events::<DamagedPart>();
        hits.extend(
            landed
                .filter(|part| !part.periodic)
                .map(|part| (part.entity, part.source, part.crit)),
        );
        let immune = harness.harness().events::<ImmunePart>();
        hits.extend(
            immune
                .filter(|part| !part.periodic)
                .map(|part| (part.entity, part.source, part.crit)),
        );
        let queue = harness.world().resource::<Cascades>();
        entries.extend(queue.entries.iter().map(|entry| (entry.kind, entry.source)));
    }
    assert!(
        !alive(&harness, keg) && !alive(&harness, second),
        "both blew"
    );
    for kind in [CascadeKind::Explosion, CascadeKind::Detonation] {
        assert!(entries.iter().any(|&(k, _)| k == kind), "{kind:?} queued");
    }
    // A hard landing's shatter deals no hit; every blast credits the lighter.
    assert!(
        entries
            .iter()
            .filter(|&&(kind, _)| kind != CascadeKind::Shatter)
            .all(|&(_, source)| source == Some(credit)),
        "every blast credits the lighter: {entries:?}"
    );
    for target in [frozen, wisp] {
        assert!(
            hits.iter().any(|&(entity, ..)| entity == target),
            "{target:?} hit"
        );
    }
    assert!(
        hits.iter()
            .all(|&(_, source, crit)| source == Some(lighter) && !crit),
        "every hit from the lighter, none a crit: {hits:?}"
    );
}

#[test]
fn two_kegs_blowing_in_one_step_blow_once_each() {
    let (mut harness, _) = sky(100.0);
    let a = prop(&mut harness, PropKind::Keg, 1000.0);
    let b = prop(&mut harness, PropKind::Keg, 1000.0 + 2.0 * KEG_HALF.x);
    harness.step(2);
    let world = harness.world_mut();
    assert!(props::light(world, a, None) && props::light(world, b, None));
    let mut booms = Vec::new();
    for _ in 0..100 {
        harness.step(1);
        booms.push(sounds(&harness, Sound::KegBlasted));
    }
    let step = booms.iter().position(|&n| n > 0).expect("they blew");
    assert_eq!(booms[step], 2, "both in one step");
    assert_eq!(booms[step + 1], 0, "none in the next");
    assert_eq!(booms.iter().sum::<usize>(), 2, "once each");
    assert!(!alive(&harness, a) && !alive(&harness, b));
}

#[test]
fn a_bomb_rolls_with_its_fuse_and_a_rock_stays_a_body_that_rolls() {
    let (mut harness, _) = sky(300.0);
    let world = harness.world_mut();
    let bomb = props::spawn_bomb(world, Vec2::new(900.0, TOP - BOMB_RADIUS - 0.5), None);
    world.get_mut::<Velocity>(bomb).unwrap().0 = Vec2::new(240.0, 0.0);
    let rock = props::lob_rock(
        world,
        Vec2::new(1400.0, TOP - 200.0),
        Vec2::new(300.0, -200.0),
    );
    harness.step(1);
    let x0 = harness.world().get::<Position>(bomb).unwrap().0.x;
    let turn0 = harness.world().get::<Transform>(bomb).unwrap().rotation;
    harness.step(20);
    // The bomb turned by the way it rolled over its radius, its fuse's
    // emitters at its turned tip.
    let world = harness.world();
    let travelled = world.get::<Position>(bomb).unwrap().0.x - x0;
    assert!(travelled > 20.0, "it rolls on: {travelled} px");
    let at = world.get::<Transform>(bomb).unwrap();
    let turned = (at.rotation - turn0).rem_euclid(std::f32::consts::TAU);
    let rolled = (travelled / BOMB_RADIUS).rem_euclid(std::f32::consts::TAU);
    let off = (turned - rolled)
        .abs()
        .min(std::f32::consts::TAU - (turned - rolled).abs());
    assert!(off < 0.25, "turned {turned} for a roll of {rolled}");
    let tip = bomb_tip(at.position, at.rotation);
    let emitters: Vec<Vec2> = world
        .query::<(&FuseEmitter, &Transform)>()
        .filter(|(emitter, _)| emitter.prop == bomb)
        .map(|(_, at)| at.position)
        .collect();
    assert_eq!(emitters.len(), 2, "its sparks and its smoke");
    for emitter in emitters {
        assert!(emitter.distance(tip) < 6.0, "{emitter} off its tip {tip}");
    }
    // The rock lands and stays: a body, rolling on, turned.
    harness.step(60);
    let world = harness.world();
    assert!(world.is_alive(rock), "the rock stays");
    assert!(world.get::<RigidBody>(rock).is_some(), "a body");
    assert!(
        world.get::<Transform>(rock).unwrap().rotation.abs() > 0.1 / ROCK_RADIUS,
        "turned as it rolled"
    );
    // Past the cap, the oldest crumbles.
    let world = harness.world_mut();
    for i in 0..ROCK_CAP {
        let at = Vec2::new(1500.0 + 20.0 * i as f32, TOP - 300.0);
        props::lob_rock(world, at, Vec2::ZERO);
    }
    assert!(!harness.world().is_alive(rock), "the oldest crumbled");
    let rocks = harness
        .world()
        .query::<&example_01_platformer::features::props::components::Rock>()
        .count();
    assert_eq!(rocks, ROCK_CAP);
}

#[test]
fn the_map_s_own_crates_and_kegs_stay_outside_the_summon_s_caps() {
    // Q45: a fresh run's crates and kegs take the map's IDs, and the
    // summon's caps count only its own.
    let (mut harness, _) = sky(300.0);
    let world = harness.world();
    let map = props::map_ids(world);
    let own: Vec<Entity> = props::all(world)
        .into_iter()
        .filter(|&prop| {
            matches!(
                props::kind(world, prop),
                Some(PropKind::Crate | PropKind::Keg)
            )
        })
        .collect();
    assert_eq!(own.len() as u64, map.end - map.start, "one ID a marker");
    assert!(own.iter().all(|&prop| map.contains(&id_of(world, prop))));
    let crates = own
        .iter()
        .filter(|&&prop| props::kind(world, prop) == Some(PropKind::Crate))
        .count();
    assert!(crates > 0, "the map sets crates down");
    // A cap's worth and one more, along the sky's ground.
    let mut summoned = Vec::new();
    for i in 0..=props::CRATE_CAP {
        let at = Vec2::new(700.0 + 80.0 * i as f32, TOP - CRATE_HALF.y);
        summoned.push(props::place(harness.world_mut(), PropKind::Crate, at));
    }
    let world = harness.world();
    assert!(
        own.iter().all(|&prop| world.is_alive(prop)),
        "the map's stay"
    );
    assert!(!world.is_alive(summoned[0]), "the oldest summoned went");
    assert!(summoned[1..].iter().all(|&prop| world.is_alive(prop)));
}
