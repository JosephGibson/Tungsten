//! The spells and their toys: slot 1 fires a missile toward the cursor,
//! mana regenerates to its max and a cast pauses it; a missile's contact
//! blasts and lights small balls, the black hole leaves what burns in its
//! reach spent, a blast pushes bodies by distance and mass, and a blast's
//! layered bursts drain within the shared budget.

use example_01_platformer::features::elements::components::BallBurn;
use example_01_platformer::features::elements::systems::douse;
use example_01_platformer::features::elements::{self};
use example_01_platformer::features::spells::systems::adopt;
use example_01_platformer::features::toys::components::{FireMissile, Ring, ShockRing};
use example_01_platformer::features::toys::{
    self, BALL_RADIUS, HOLE_RADIUS, IRON_MASS, MISSILE_SPEED, PUSH_RADIUS, PUSH_SPEED, RING_CAP,
};
use example_01_platformer::shared::{BURST_CAP, Burst};
use glam::Vec2;
use tungsten::core::{
    Collider, Entity, Particle, ParticleBudget, Position, RigidBody, RigidBodyBundle, Velocity,
};
use tungsten::kit::{LinearMotion, Stats};
use tungsten::testing::UiHarness;

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::level::components::Glow;
use example_01_platformer::features::player::components::Lantern;
use example_01_platformer::features::player::{GUTTER_PUFF, STAFF_LIT};
use tungsten::kit::Tags;

use super::{aim_at, ball, body, centre, choose, floor, ground, mana, pin_camera, run, stand};

/// Steps up to `frames` frames until `done`; whether it came.
fn until(harness: &mut UiHarness, frames: u32, mut done: impl FnMut(&UiHarness) -> bool) -> bool {
    for _ in 0..frames {
        harness.step(1);
        if done(harness) {
            return true;
        }
    }
    false
}

fn rings(harness: &UiHarness, kind: Ring) -> usize {
    harness
        .world()
        .query::<&ShockRing>()
        .filter(|ring| ring.kind == kind)
        .count()
}

#[test]
fn fireball_contact_explodes_and_ignites_small_balls() {
    let (mut harness, _) = run();
    let o = Vec2::new(700.0, 500.0);
    // The ball rests on a ledge, so the missile meets it where it waits.
    floor(
        &mut harness,
        o + Vec2::new(200.0, 13.0),
        Vec2::new(12.0, 5.0),
    );
    let target = ball(&mut harness, o + Vec2::new(200.0, 0.0), true);
    let missile = toys::fire_missile(harness.world_mut(), o, Vec2::X * MISSILE_SPEED).unwrap();
    for _ in 0..30 {
        harness.step(1);
    }
    assert!(!harness.world().is_alive(missile));
    assert_eq!(rings(&harness, Ring::Fireball), 1, "one blast");
    assert!(
        harness
            .world()
            .get::<BallBurn>(target)
            .is_some_and(|b| b.remaining > 0.0)
    );
}

#[test]
fn black_hole_leaves_burning_balls_inside_its_radius_spent() {
    let (mut harness, _) = run();
    let centre = Vec2::new(1200.0, 600.0);
    toys::place_black_hole(harness.world_mut(), centre);
    let inside = ball(
        &mut harness,
        centre + Vec2::new(HOLE_RADIUS * 0.5, 0.0),
        true,
    );
    let outside = ball(
        &mut harness,
        centre + Vec2::new(HOLE_RADIUS * 1.5, 0.0),
        true,
    );
    elements::ignite(harness.world_mut(), inside);
    elements::ignite(harness.world_mut(), outside);
    douse(harness.world_mut());
    elements::ignite(harness.world_mut(), inside);
    let burn = |e| harness.world().get::<BallBurn>(e).unwrap().remaining;
    assert_eq!(burn(inside), 0.0);
    assert!(burn(outside) > 0.0);
}

#[test]
fn a_blast_pushes_dynamic_bodies_away_by_distance_and_mass() {
    let (mut harness, _) = run();
    let o = Vec2::new(1200.0, 600.0);
    let spawn = |harness: &mut UiHarness, at: Vec2, mass: f32| -> Entity {
        harness.world_mut().spawn_with(
            RigidBodyBundle::dynamic(Position(o + at), Collider::circle(BALL_RADIUS))
                .with_body(RigidBody::dynamic().with_mass(mass)),
        )
    };
    let near = spawn(&mut harness, Vec2::new(40.0, 0.0), 1.0);
    let far = spawn(&mut harness, Vec2::new(120.0, 0.0), 1.0);
    let heavy = spawn(&mut harness, Vec2::new(-40.0, 0.0), IRON_MASS);
    let beyond = spawn(&mut harness, Vec2::new(0.0, PUSH_RADIUS + 1.0), 1.0);
    toys::explode(harness.world_mut(), o);
    let v = |harness: &UiHarness, e| harness.world().get::<Velocity>(e).unwrap().0;
    let falloff = 1.0 - 40.0 / PUSH_RADIUS;
    assert!((v(&harness, near) - Vec2::new(PUSH_SPEED * falloff, 0.0)).length() < 1e-3);
    assert!(v(&harness, far).x > 0.0 && v(&harness, far).x < v(&harness, near).x);
    assert!((v(&harness, heavy).x + v(&harness, near).x / IRON_MASS).abs() < 1e-3);
    assert_eq!(v(&harness, beyond), Vec2::ZERO);
    // The step carries the push into the body's place.
    harness.step(1);
    assert!(body(&harness, near).x > o.x + 40.0);
}

#[test]
fn fireball_blast_layers_emit_particles_and_drain_within_the_shared_budget() {
    let (mut harness, _) = run();
    let o = Vec2::new(1200.0, 600.0);
    toys::explode(harness.world_mut(), o);
    let layers: Vec<Entity> = harness
        .world()
        .query::<(Entity, &Burst)>()
        .map(|(e, _)| e)
        .collect();
    assert_eq!(layers.len(), 4);
    assert_eq!(rings(&harness, Ring::Fireball), 1);
    harness.step(1);
    let emitted = harness
        .world()
        .query::<&Particle>()
        .filter(|p| p.emitter.is_some_and(|e| layers.contains(&e)))
        .count();
    assert_eq!(emitted, 156);
    for _ in 0..10 {
        toys::explode(harness.world_mut(), o);
    }
    assert_eq!(harness.world().query::<&Burst>().count(), BURST_CAP);
    assert!(harness.world().query::<&ShockRing>().count() <= RING_CAP);
    for _ in 0..120 {
        harness.step(1);
        let world = harness.world();
        let cap = world.resource::<ParticleBudget>().global_cap as usize;
        assert!(world.query::<&Particle>().count() <= cap);
    }
    assert_eq!(harness.world().query::<&Burst>().count(), 0);
    assert_eq!(harness.world().query::<&ShockRing>().count(), 0);
    assert_eq!(harness.world().query::<&FireMissile>().count(), 0);
}

#[test]
fn mouse4_casts_a_fireball_toward_the_cursor() {
    // Slot 1's spell, cast with `cast`: the kit's spawn payload fires it.
    let (mut harness, player) = run();
    ground(&mut harness, 600.0);
    stand(&mut harness, player, 700.0, 600.0);
    harness.step(1);
    pin_camera(&mut harness, Vec2::new(200.0, 200.0));
    let from = centre(&harness, player);
    let target = from + Vec2::new(300.0, -400.0);
    aim_at(&mut harness, target);
    choose(&mut harness, 0);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    assert_eq!(mana(&harness), 96.0, "a missile costs 4");
    // The frame's flush landed it; the next frame takes it in first.
    adopt(harness.world_mut());
    let missiles: Vec<(Entity, Vec2, Vec2)> = harness
        .world()
        .query::<(Entity, &FireMissile, &Position, &LinearMotion)>()
        .map(|(e, _, at, motion)| (e, at.0, motion.velocity()))
        .collect();
    assert_eq!(missiles.len(), 1);
    let (missile, at, velocity) = missiles[0];
    let aim = (target - from).normalize();
    assert!(velocity.normalize().dot(aim) > 0.999);
    assert!((velocity.length() - MISSILE_SPEED).abs() < 1.0);
    let start = at + Vec2::splat(toys::MISSILE_SIZE * 0.5);
    assert!(
        (start - (from + aim * 24.0)).length() < 1.0,
        "it leaves the courier's centre at the payload's offset: {start}"
    );
    let distance = |harness: &UiHarness| {
        harness
            .world()
            .get::<Position>(missile)
            .unwrap()
            .0
            .distance(target)
    };
    let before = distance(&harness);
    harness.step(1);
    assert!(distance(&harness) < before);
}

#[test]
fn the_staff_light_burns_mana_and_gutters_out() {
    let (mut harness, player) = run();
    harness.step(2);
    let lantern = |h: &UiHarness| *h.world().get::<Lantern>(player).unwrap();
    let tagged = |h: &UiHarness| {
        h.world()
            .get::<Tags>(player)
            .is_some_and(|tags| tags.has(STAFF_LIT))
    };
    assert!(
        !lantern(&harness).on && !tagged(&harness),
        "dark at the start"
    );
    harness.harness_mut().press_action("lantern");
    harness.step(2);
    harness.harness_mut().release_action("lantern");
    assert!(
        lantern(&harness).lit() && tagged(&harness),
        "lit by its action"
    );
    assert_eq!(
        mana(&harness),
        100.0,
        "at full mana the regeneration feeds it"
    );
    // Under a cast's pause it burns 3 a second outright.
    let stats = harness.world_mut().get_mut::<Stats>(player).unwrap();
    stats.set_base("mana", 2.0);
    super::combat::apply(&mut harness, player, "casting", None);
    harness.step(20);
    let burnt = 2.0 - mana(&harness);
    assert!((burnt - 1.0).abs() < 0.06, "20 steps of 0.05: {burnt}");
    // Out of mana it gutters out once, with a puff and a sound.
    let (mut gutters, mut puffs) = (0, 0);
    let out = until(&mut harness, 60, |h| {
        gutters += h
            .harness()
            .events::<Sounded>()
            .filter(|sounded| sounded.sound == Sound::StaffGuttered)
            .count();
        puffs = puffs.max(super::bursts(h, GUTTER_PUFF));
        lantern(h).guttered
    });
    assert!(out, "it gutters out");
    assert_eq!((gutters, puffs), (1, 1), "one puff and one sound");
    harness.step(1);
    assert!(harness.world().get::<Glow>(player).unwrap().hidden, "dark");
    // It waits for 10 mana, then relights.
    let mut waited = 0.0_f32;
    let relit = until(&mut harness, 180, |h| {
        if lantern(h).guttered {
            waited = waited.max(mana(h));
        }
        !lantern(h).guttered
    });
    assert!(relit, "it relights");
    assert!(waited >= 9.0, "dark through the lower mana: {waited}");
    assert!(mana(&harness) >= 9.9, "relit at 10: {}", mana(&harness));
    harness.step(1);
    assert!(!harness.world().get::<Glow>(player).unwrap().hidden);
}

#[test]
fn mana_regenerates_to_its_max_and_a_cast_pauses_it_for_about_a_second() {
    let (mut harness, player) = run();
    // The staff light off, which would burn through the pause.
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
    let set = |harness: &mut UiHarness, value: f32| {
        let stats = harness.world_mut().get_mut::<Stats>(player).unwrap();
        stats.set_base("mana", value);
    };
    set(&mut harness, 40.0);
    harness.step(60);
    assert!(
        (mana(&harness) - 70.0).abs() <= 3.0,
        "30 a second: {}",
        mana(&harness)
    );
    // Force push, 12.5; the step's regeneration may come before it.
    choose(&mut harness, 4);
    let before = mana(&harness);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    let cast = mana(&harness);
    let spent = before - cast;
    assert!(
        spent > 9.0 && spent <= 12.5,
        "the push spent its 12.5: {spent}"
    );
    harness.step(54);
    assert_eq!(mana(&harness), cast, "held while casting");
    harness.step(30);
    assert!(mana(&harness) > cast + 3.0, "it resumed");
    harness.step(360);
    assert_eq!(mana(&harness), 100.0, "capped at max_mana");
}
