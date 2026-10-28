//! The spells and their toys: slot 1 fires a missile toward the cursor,
//! mana regenerates to its max and a cast pauses it; a missile's contact
//! blasts and lights small balls, the black hole leaves what burns in its
//! reach spent, a blast pushes bodies by distance and mass, and a blast's
//! layered bursts drain within the shared budget; three black holes open
//! at once, a fourth collapsing the oldest, each sweeping small balls from
//! its core, all humming on one voice; the sprays reach 30 % further and
//! burst on what they hit, and draw their sprites while they run; the toxic
//! flask lobs through the cursor, passes through the wizard, bursts on the
//! step it meets a floor, a ledge or a goblin and splashes poison on the
//! living but him, three at most in flight.

use example_01_platformer::features::audio::HOLE_HUM;
use example_01_platformer::features::audio::components::Music;
use example_01_platformer::features::combat::Kind;
use example_01_platformer::features::elements;
use example_01_platformer::features::elements::components::BallBurn;
use example_01_platformer::features::elements::systems::douse;
use example_01_platformer::features::enemies;
use example_01_platformer::features::enemies::components::{EnemyKind, EnemyState};
use example_01_platformer::features::props::{self, components::PropKind};
use example_01_platformer::features::spells::systems::adopt;
use example_01_platformer::features::spells::{
    SUMMON_NEXT, SUMMON_PREV, SUMMONS, Summoned, summon_type,
};
use example_01_platformer::features::toys::components::{
    BlackHole, FireMissile, Ring, ShockRing, SmallBall, Spray, SprayLook, SprayPuff, Wave,
};
use example_01_platformer::features::toys::flask;
use example_01_platformer::features::toys::{
    self, BALL_RADIUS, BROOM_MOST, HOLE_RADIUS, IRON_HALF, IRON_MASS, MISSILE_SPEED, PUSH_RADIUS,
    PUSH_SPEED, RING_CAP,
};
use example_01_platformer::shared::{BURST_CAP, Burst, SaveIds};
use example_01_platformer::shell::Difficulty;
use glam::Vec2;
use tungsten::core::{
    AudioCommand, Collider, Entity, Particle, ParticleBudget, Position, RigidBody, RigidBodyBundle,
    SoundRegistry, Velocity,
};
use tungsten::kit::{DamagedPart, Health, LinearMotion, StateMachine, Stats};
use tungsten::testing::UiHarness;

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::level::components::Glow;
use example_01_platformer::features::player::components::Lantern;
use example_01_platformer::features::player::{GUTTER_PUFF, STAFF_LIT};
use example_01_platformer::features::spells::components::Spell;
use tungsten::kit::Tags;

use super::kind_body;
use super::{
    aim_at, ball, body, calm, centre, choose, floor, ground, mana, pin_camera, run, stand,
};

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
    // The geysers' bursts aside, which never drain (e01hollow).
    calm(&mut harness);
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
    choose(&mut harness, Spell::FireMissile);
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
    choose(&mut harness, Spell::ForcePush);
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

/// The open holes.
fn holes(harness: &UiHarness) -> usize {
    harness.world().query::<&BlackHole>().count()
}

#[test]
fn three_black_holes_stay_open_and_a_fourth_collapses_the_oldest() {
    let (mut harness, _) = run();
    let world = harness.world_mut();
    let opened: Vec<Entity> = (0..3)
        .map(|i| toys::place_black_hole(world, Vec2::new(900.0 + 400.0 * i as f32, 500.0)))
        .collect();
    harness.step(1);
    assert_eq!(holes(&harness), 3, "three stay open");
    let fourth = toys::place_black_hole(harness.world_mut(), Vec2::new(2100.0, 500.0));
    harness.step(1);
    let collapsed = harness
        .harness()
        .events::<Sounded>()
        .filter(|sounded| sounded.sound == Sound::HoleCollapsed)
        .count();
    assert_eq!(collapsed, 1, "the fourth's opening collapses one");
    let world = harness.world();
    assert!(!world.is_alive(opened[0]), "the oldest gone");
    assert!(opened[1..].iter().all(|&hole| world.is_alive(hole)));
    assert!(world.is_alive(fourth));
    assert_eq!(holes(&harness), 3);
    assert_eq!(rings(&harness, Ring::Collapse), 1, "its ring");
}

#[test]
fn an_open_hole_sweeps_small_balls_from_its_core_a_few_a_step() {
    let (mut harness, _) = run();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let centre = Vec2::new(1200.0, 500.0);
    toys::place_black_hole(harness.world_mut(), centre);
    // A disc of small balls about the hole, within 190 px, where its pull
    // beats their fall.
    for i in 0..400 {
        let angle = i as f32 * 2.399_963_2;
        let at = centre + Vec2::from_angle(angle) * (30.0 + 8.0 * (i as f32).sqrt());
        ball(&mut harness, at, true);
    }
    let count = |h: &UiHarness| h.world().query::<&SmallBall>().count();
    // Those still in its reach: a ball the crowd flings past it falls away.
    let held = |h: &UiHarness| {
        h.world()
            .query::<(&SmallBall, &Position, &Collider)>()
            .filter(|(_, at, collider)| (at.0 + collider.offset).distance(centre) < HOLE_RADIUS)
            .count()
    };
    let mut seconds = Vec::new();
    for _ in 0..5 {
        let start = held(&harness);
        if start == 0 {
            break;
        }
        for _ in 0..60 {
            let before = count(&harness);
            harness.step(1);
            let gone = before - count(&harness);
            assert!(gone <= BROOM_MOST, "{gone} gone in one step");
        }
        seconds.push((start, held(&harness)));
    }
    assert!(seconds[0].1 < seconds[0].0, "it sweeps: {seconds:?}");
    assert!(
        seconds.iter().all(|&(start, end)| end < start),
        "fewer left each second: {seconds:?}"
    );
}

#[test]
fn the_holes_hum_on_one_voice_that_falls_silent_when_the_last_closes() {
    let (mut harness, player) = run();
    let voice = harness
        .world()
        .resource::<Music>()
        .holes
        .expect("the holes' hum starts with the run");
    let hum = harness
        .world()
        .resource::<SoundRegistry>()
        .get_by_id(HOLE_HUM);
    let mut heard: Vec<f32> = Vec::new();
    let mut plays = 0;
    let mut listen = |h: &mut UiHarness, frames: u32, heard: &mut Vec<f32>| {
        for _ in 0..frames {
            h.step(1);
            for command in h.harness().audio() {
                match command {
                    AudioCommand::SetVoiceVolume {
                        voice: own, volume, ..
                    } if *own == voice => heard.push(*volume),
                    AudioCommand::Play { handle, .. } if Some(*handle) == hum => plays += 1,
                    _ => {}
                }
            }
        }
    };
    let at = centre(&harness, player);
    let world = harness.world_mut();
    let near = [
        toys::place_black_hole(world, at + Vec2::new(200.0, -100.0)),
        toys::place_black_hole(world, at + Vec2::new(-250.0, -60.0)),
    ];
    listen(&mut harness, 10, &mut heard);
    assert!(
        heard.last().is_some_and(|&volume| volume > 0.0),
        "{heard:?}"
    );
    for hole in near {
        harness.world_mut().get_mut::<BlackHole>(hole).unwrap().left = 0.01;
    }
    heard.clear();
    listen(&mut harness, 10, &mut heard);
    assert_eq!(holes(&harness), 0, "both closed");
    assert_eq!(heard.last(), Some(&0.0), "silent: {heard:?}");
    assert_eq!(plays, 0, "the run's one voice, never another");
}

/// Presses `cast` until a fixed step has read it, then lets it go: the
/// sounds those frames made.
fn cast_once(harness: &mut UiHarness) -> Vec<Sound> {
    let mut heard = Vec::new();
    harness.harness_mut().press_action("cast");
    for _ in 0..4 {
        harness.step(1);
        heard.extend(harness.harness().events::<Sounded>().map(|s| s.sound));
        if super::steps_this_frame(harness) > 0 {
            break;
        }
    }
    harness.harness_mut().release_action("cast");
    harness.step(1);
    heard.extend(harness.harness().events::<Sounded>().map(|s| s.sound));
    heard
}

#[test]
fn a_refused_cast_says_why_and_spends_nothing() {
    use tungsten::kit::Abilities;
    let (mut harness, player) = run();
    ground(&mut harness, 1000.0);
    stand(&mut harness, player, 600.0, 1000.0);
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
    let slab = Vec2::new(900.0, 700.0);
    floor(&mut harness, slab, Vec2::new(100.0, 20.0));
    harness.step(2);
    pin_camera(&mut harness, Vec2::new(200.0, 400.0));
    let set_mana = |h: &mut UiHarness, amount: f32| {
        let stats = h.world_mut().get_mut::<Stats>(player).unwrap();
        // At its most, so the regeneration adds none.
        stats.set_base("max_mana", amount);
        stats.set_base("mana", amount);
    };
    let spent = |h: &UiHarness, spell: Spell| {
        let abilities = h.world().get::<Abilities>(player).unwrap();
        (mana(h), abilities.charges(spell.ability()))
    };
    let count = |heard: &[Sound], sound: Sound| heard.iter().filter(|&&s| s == sound).count();
    // No mana: a missile at none says so, once.
    aim_at(&mut harness, Vec2::new(900.0, 500.0));
    choose(&mut harness, Spell::FireMissile);
    set_mana(&mut harness, 0.0);
    let before = spent(&harness, Spell::FireMissile);
    let heard = cast_once(&mut harness);
    assert_eq!(count(&heard, Sound::NoMana), 1, "{heard:?}");
    assert_eq!(spent(&harness, Spell::FireMissile), before, "nothing spent");
    // A cooldown: a second missile at once is not ready, once.
    set_mana(&mut harness, 100.0);
    cast_once(&mut harness);
    let before = spent(&harness, Spell::FireMissile);
    let heard = cast_once(&mut harness);
    assert_eq!(count(&heard, Sound::NotReady), 1, "{heard:?}");
    assert_eq!(count(&heard, Sound::Cast), 0, "{heard:?}");
    assert_eq!(spent(&harness, Spell::FireMissile), before, "nothing spent");
    // Iron in a wall fizzles at the cursor, once.
    aim_at(&mut harness, slab);
    choose(&mut harness, Spell::IronBlock);
    let before = spent(&harness, Spell::IronBlock);
    let fizzles = bursts(&harness);
    let heard = cast_once(&mut harness);
    assert_eq!(count(&heard, Sound::Fizzled), 1, "{heard:?}");
    assert_eq!(spent(&harness, Spell::IronBlock), before, "nothing spent");
    assert_eq!(bursts(&harness), fizzles + 1, "its fizzle");
}

/// The fizzles alive.
fn bursts(harness: &UiHarness) -> usize {
    super::bursts(harness, "ex10_fizzle")
}

/// The live props of `kind`, in save-ID order.
fn props_of(harness: &UiHarness, kind: PropKind) -> Vec<Entity> {
    let world = harness.world();
    let mut live: Vec<(u64, Entity)> = world
        .query::<(
            Entity,
            &tungsten::core::Position,
            &example_01_platformer::shared::SaveId,
        )>()
        .filter(|&(entity, ..)| props::kind(world, entity) == Some(kind))
        .map(|(entity, _, id)| (id.0, entity))
        .collect();
    live.sort_unstable_by_key(|&(id, _)| id);
    live.into_iter().map(|(_, entity)| entity).collect()
}

/// Turns the summon's type by `action` once, pressed until a step read it.
fn turn(harness: &mut UiHarness, action: &str) {
    harness.harness_mut().press_action(action);
    harness.step(1);
    harness.harness_mut().release_action(action);
    harness.step(1);
}

/// One press of `cast` at `at`, then a second for the summon's cooldown.
fn summon_at(harness: &mut UiHarness, at: Vec2) {
    aim_at(harness, at);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    harness.step(61);
}

#[test]
fn the_summon_slot_places_crates_and_kegs_where_they_fit() {
    let (mut harness, player) = run();
    // The map's crates and kegs taken off, so the slot's count from none.
    for prop in props::all(harness.world()) {
        harness.world_mut().despawn(prop);
    }
    harness.harness_mut().set_dt(1.0 / 60.0);
    ground(&mut harness, 1000.0);
    stand(&mut harness, player, 300.0, 1000.0);
    harness.step(1);
    pin_camera(&mut harness, Vec2::new(0.0, 400.0));
    let stats = harness.world_mut().get_mut::<Stats>(player).unwrap();
    stats.set_base("max_mana", 10_000.0);
    stats.set_base("mana", 10_000.0);
    choose(&mut harness, Spell::Summon);
    // Past the dummy and the enemies, the crate, then the keg.
    let crate_at = SUMMONS
        .iter()
        .position(|summon| summon.summoned == Summoned::Crate)
        .expect("a crate to summon");
    for _ in 0..crate_at {
        turn(&mut harness, SUMMON_NEXT);
    }
    assert_eq!(summon_type(harness.world()).summoned, Summoned::Crate);
    let at = Vec2::new(800.0, 900.0);
    aim_at(&mut harness, at);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    let crates = props_of(&harness, PropKind::Crate);
    assert_eq!(crates.len(), 1, "a crate");
    assert!(
        body(&harness, crates[0]).distance(at) < 2.0,
        "at the cursor"
    );
    harness.step(61);
    turn(&mut harness, SUMMON_NEXT);
    assert_eq!(summon_type(harness.world()).summoned, Summoned::Keg);
    // Near the ground: a keg landing hard ([`elements::blows_keg`]) goes off.
    let at = Vec2::new(1000.0, 965.0);
    aim_at(&mut harness, at);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    let kegs = props_of(&harness, PropKind::Keg);
    assert_eq!(kegs.len(), 1, "a keg");
    assert!(body(&harness, kegs[0]).distance(at) < 2.0, "at the cursor");
    harness.step(61);
    // In a wall, and on iron, it fizzles and spends nothing.
    let iron = toys::place_iron(harness.world_mut(), Vec2::new(1400.0, 1000.0 - IRON_HALF.y));
    harness.step(2);
    for at in [Vec2::new(1150.0, 1040.0), super::body(&harness, iron)] {
        let before = mana(&harness);
        let fizzles = bursts(&harness);
        aim_at(&mut harness, at);
        harness.harness_mut().press_action("cast");
        harness.step(1);
        harness.harness_mut().release_action("cast");
        assert_eq!(mana(&harness), before, "nothing spent at {at}");
        assert_eq!(bursts(&harness), fizzles + 1, "its fizzle at {at}");
        assert_eq!(
            props_of(&harness, PropKind::Keg).len(),
            1,
            "refused at {at}"
        );
        harness.step(61);
    }
    // Past its cap, the oldest crate goes in a poof.
    turn(&mut harness, SUMMON_PREV);
    let first = props_of(&harness, PropKind::Crate)[0];
    for i in 1..props::CRATE_CAP {
        summon_at(&mut harness, Vec2::new(1440.0 + 60.0 * i as f32, 960.0));
    }
    assert_eq!(props_of(&harness, PropKind::Crate).len(), props::CRATE_CAP);
    assert!(harness.world().is_alive(first), "the cap holds it");
    aim_at(&mut harness, Vec2::new(1700.0, 600.0));
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    assert!(!harness.world().is_alive(first), "the oldest went");
    assert!(super::bursts(&harness, "ex10_poof") > 0, "in a poof");
    assert_eq!(props_of(&harness, PropKind::Crate).len(), props::CRATE_CAP);
}

/// The sky's ground for the spray tests, and the flamethrower's and the ice
/// spray's slots.
const SKY: f32 = 1000.0;
const FLAMETHROWER: Spell = Spell::Flamethrower;
const ICE_SPRAY: Spell = Spell::IceSpray;

/// A run with the player on the sky's ground at x 300, the map's enemies
/// gone, the camera held over it, and a dummy whose near face stands
/// `tiles` from the player's centre, if any.
fn spray_range(tiles: Option<f32>) -> (UiHarness, Entity, Option<Entity>) {
    let (mut harness, player) = run();
    calm(&mut harness);
    ground(&mut harness, SKY);
    stand(&mut harness, player, 300.0, SKY);
    harness.step(1);
    pin_camera(&mut harness, Vec2::new(0.0, SKY - 500.0));
    let me = centre(&harness, player);
    let dummy = tiles.map(|tiles| {
        let face = me.x + tiles * example_01_platformer::features::level::TILE;
        super::combat::dummy_at(&mut harness, face + 14.0)
    });
    harness.step(2);
    (harness, player, dummy)
}

/// Whether a burst of `name` lies within 24 px of the face at x `face`,
/// within `half` px of `middle`'s height: a surface's hit on it.
fn bursts_on_face(harness: &UiHarness, name: &str, face: f32, middle: f32, half: f32) -> bool {
    let world = harness.world();
    let config = world
        .resource::<tungsten::core::ParticleConfigRegistry>()
        .id_for_name(name);
    world
        .query::<(
            &Burst,
            &tungsten::core::ParticleEmitter,
            &tungsten::core::Transform,
        )>()
        .any(|(_, emitter, at)| {
            Some(emitter.config) == config
                && (at.position.x - face).abs() <= 24.0
                && (at.position.y - middle).abs() <= half
        })
}

/// The bursts of `name` within `reach` of `at`.
fn bursts_near(harness: &UiHarness, name: &str, at: Vec2, reach: f32) -> usize {
    let world = harness.world();
    let config = world
        .resource::<tungsten::core::ParticleConfigRegistry>()
        .id_for_name(name);
    world
        .query::<(
            &Burst,
            &tungsten::core::ParticleEmitter,
            &tungsten::core::Transform,
        )>()
        .filter(|(_, emitter, at_)| {
            Some(emitter.config) == config && at_.position.distance(at) <= reach
        })
        .count()
}

#[test]
fn the_sprays_reach_further_and_burst_on_what_they_hit() {
    // Past 01 (f)'s reach (3.5 and 7 tiles), inside 01 (h)'s.
    for (slot, tiles, hit) in [
        (FLAMETHROWER, 4.3, "ex10_flame_hit"),
        (ICE_SPRAY, 8.8, "ex10_frost_hit"),
    ] {
        let (mut harness, player, dummy) = spray_range(Some(tiles));
        let dummy = dummy.unwrap();
        let face = body(&harness, dummy) - Vec2::new(14.0, 0.0);
        let full = harness
            .world()
            .get::<tungsten::kit::Health>(dummy)
            .unwrap()
            .current();
        choose(&mut harness, slot);
        let at = body(&harness, dummy);
        aim_at(&mut harness, at);
        harness.harness_mut().press_action("cast");
        let burst = until(&mut harness, 18, |h| bursts_near(h, hit, face, 24.0) > 0);
        harness.harness_mut().release_action("cast");
        assert!(burst, "{hit} at the dummy within 0.3 s");
        let left = harness
            .world()
            .get::<tungsten::kit::Health>(dummy)
            .unwrap()
            .current();
        assert!(left < full, "{hit}: the dummy took the spray's damage");
        let _ = player;
    }
    // On a bare wall, its surface hit at the face and no target's (e01sfx).
    for (slot, surface) in [
        (FLAMETHROWER, "ex10_flame_wall_hit"),
        (ICE_SPRAY, "ex10_frost_wall_hit"),
    ] {
        let (mut harness, player, _) = spray_range(None);
        let me = centre(&harness, player);
        let wall = me + Vec2::new(150.0, -100.0);
        floor(&mut harness, wall, Vec2::new(10.0, 100.0));
        harness.step(1);
        choose(&mut harness, slot);
        aim_at(&mut harness, me + Vec2::new(400.0, 0.0));
        harness.harness_mut().press_action("cast");
        let face = wall.x - 10.0;
        let burst = until(&mut harness, 18, |h| {
            bursts_on_face(h, surface, face, wall.y, 100.0)
        });
        assert!(
            harness.world().query::<&Spray>().count() == 1,
            "{slot:?}: it sprays"
        );
        harness.harness_mut().release_action("cast");
        assert!(burst, "{slot:?}: {surface} at the wall's face within 0.3 s");
        let none =
            super::bursts(&harness, "ex10_flame_hit") + super::bursts(&harness, "ex10_frost_hit");
        assert_eq!(none, 0, "{slot:?}: no target's hit on a wall");
    }
}

#[test]
fn a_spray_bursts_its_surface_hit_on_a_crate() {
    use example_01_platformer::features::props::components::PropKind;
    use example_01_platformer::features::props::{self, CRATE_HALF};
    for (slot, surface) in [
        (FLAMETHROWER, "ex10_flame_wall_hit"),
        (ICE_SPRAY, "ex10_frost_wall_hit"),
    ] {
        let (mut harness, player, _) = spray_range(None);
        let me = centre(&harness, player);
        let at = Vec2::new(me.x + 180.0, SKY - CRATE_HALF.y - 0.5);
        let crate_ = props::spawn(harness.world_mut(), PropKind::Crate, at);
        harness.step(1);
        choose(&mut harness, slot);
        let middle = body(&harness, crate_);
        aim_at(&mut harness, middle);
        harness.harness_mut().press_action("cast");
        let face = middle.x - CRATE_HALF.x;
        let burst = until(&mut harness, 18, |h| {
            bursts_on_face(h, surface, face, middle.y, CRATE_HALF.y)
        });
        harness.harness_mut().release_action("cast");
        assert!(burst, "{slot:?}: {surface} at the crate within 0.3 s");
        let none =
            super::bursts(&harness, "ex10_flame_hit") + super::bursts(&harness, "ex10_frost_hit");
        assert_eq!(none, 0, "{slot:?}: a crate takes no target's hit");
    }
}

#[test]
fn a_spray_draws_its_sprites_and_clears_them() {
    let (mut harness, _, dummy) = spray_range(Some(4.0));
    let dummy = dummy.unwrap();
    choose(&mut harness, FLAMETHROWER);
    let at = body(&harness, dummy);
    aim_at(&mut harness, at);
    harness.harness_mut().press_action("cast");
    harness.step(6);
    let look = |h: &UiHarness| h.world().query::<&SprayLook>().next().cloned();
    let shown = |h: &UiHarness, e: Option<Entity>| {
        e.is_some_and(|e| {
            h.world()
                .get::<tungsten::core::Visibility>(e)
                .is_some_and(|v| v.visible)
        })
    };
    let fire = look(&harness).expect("the flamethrower's look");
    assert!(shown(&harness, fire.muzzle), "its muzzle");
    assert!(
        fire.light
            .is_some_and(|light| harness.world().has::<tungsten::core::Light>(light)),
        "its light"
    );
    assert!(shown(&harness, fire.impact), "its impact on the dummy");
    assert!(
        harness.world().query::<&SprayPuff>().count() > 0,
        "its puffs"
    );
    // The ice spray lays its core from its start to its end.
    harness.harness_mut().release_action("cast");
    harness.step(2);
    choose(&mut harness, ICE_SPRAY);
    let at = body(&harness, dummy);
    aim_at(&mut harness, at);
    harness.harness_mut().press_action("cast");
    harness.step(6);
    let ice = look(&harness).expect("the ice spray's look");
    let spray = *harness.world().query::<&Spray>().next().unwrap();
    let length = spray.start.distance(spray.end);
    assert_eq!(
        ice.core.len(),
        ((length - 4.0) / 32.0).ceil() as usize,
        "{length}"
    );
    assert!(ice.core.iter().all(|&tile| shown(&harness, Some(tile))));
    // A release takes the look at once; its puffs play out within 0.4 s.
    harness.harness_mut().release_action("cast");
    harness.step(1);
    let parts: Vec<Entity> = ice
        .muzzle
        .into_iter()
        .chain(ice.light)
        .chain(ice.impact)
        .chain(ice.core.iter().copied())
        .collect();
    assert!(parts.iter().all(|&part| !harness.world().is_alive(part)));
    assert!(until(&mut harness, 24, |h| {
        h.world().query::<&SprayPuff>().count() == 0
    }));
}

#[test]
fn the_wave_meets_a_body_running_into_it() {
    let (mut harness, _) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    // In the level's empty sky.
    let caster = Vec2::new(1000.0, 700.0);
    let origin = caster + Vec2::X * toys::wave::WAVE_START;
    let velocity = |h: &UiHarness, e: Entity| h.world().get::<Velocity>(e).unwrap().0;
    // Head on: a small ball 29 px past the front's start, just past its
    // first step's 27 px, flying back at 900 px/s. A test of where it is at
    // each step alone would never cross it.
    let head_on = ball(&mut harness, origin + Vec2::new(29.0, 0.0), true);
    harness.world_mut().get_mut::<Velocity>(head_on).unwrap().0 = Vec2::new(-900.0, 0.0);
    toys::wave::cast(harness.world_mut(), caster, Vec2::X);
    harness.step(1);
    assert!(
        velocity(&harness, head_on).x > 0.0,
        "thrown back: {}",
        velocity(&harness, head_on)
    );
    // On a slant, the front at 135 px: a bowling ball 175 px along and 85
    // across, flying in at 900 px/s on each axis, meets it inside the sector
    // within the step, though at the step's start it lies outside.
    harness.step(4);
    let slant = ball(&mut harness, origin + Vec2::new(175.0, 85.0), false);
    harness.world_mut().get_mut::<Velocity>(slant).unwrap().0 = Vec2::new(-900.0, -900.0);
    harness.step(1);
    let crossed = harness
        .world()
        .query::<&Wave>()
        .any(|wave| wave.crossed.contains(&slant));
    assert!(crossed, "the front met it");
    assert!(
        velocity(&harness, slant).x > -800.0,
        "pushed along: {}",
        velocity(&harness, slant)
    );
}

#[test]
fn a_wave_lives_three_tenths_of_a_second() {
    let (mut harness, _) = run();
    harness.harness_mut().set_dt(1.0 / 60.0);
    choose(&mut harness, Spell::ForcePush);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    let looks = |h: &UiHarness| {
        h.world()
            .query::<&Wave>()
            .map(|wave| wave.look)
            .collect::<Vec<_>>()
    };
    let cast = looks(&harness);
    assert_eq!(cast.len(), 1, "the force push casts one wave");
    let look = cast[0].expect("its crescent");
    harness.step(18);
    assert!(looks(&harness).is_empty(), "gone after 19 steps");
    assert!(!harness.world().is_alive(look), "its crescent too");
}

#[test]
fn a_hole_lifts_bowling_balls_within_200_px_and_leaves_iron() {
    const FLOOR: f32 = 1000.0;
    let (mut harness, _) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    ground(&mut harness, FLOOR);
    harness.step(1);
    // Three holes 800 px apart, out of each other's reach: one 180 px over a
    // bowling ball, one 230 px over another and one 120 px over iron.
    let near = ball(&mut harness, Vec2::new(600.0, FLOOR - BALL_RADIUS), false);
    let far = ball(&mut harness, Vec2::new(1400.0, FLOOR - BALL_RADIUS), false);
    let iron = toys::place_iron(harness.world_mut(), Vec2::new(2200.0, FLOOR - IRON_HALF.y));
    harness.step(2);
    let starts = [near, far, iron].map(|entity| body(&harness, entity));
    for (thing, over) in [(starts[0], 180.0), (starts[1], 230.0), (starts[2], 120.0)] {
        toys::place_black_hole(harness.world_mut(), thing - Vec2::new(0.0, over));
    }
    let mut rose = 0.0_f32;
    let mut stirred = [0.0_f32; 2];
    for _ in 0..60 {
        harness.step(1);
        rose = rose.max(starts[0].y - body(&harness, near).y);
        stirred[0] = stirred[0].max(body(&harness, far).distance(starts[1]));
        stirred[1] = stirred[1].max(body(&harness, iron).distance(starts[2]));
    }
    assert!(rose > 60.0, "the near ball rose {rose} px");
    assert!(stirred[0] < 1.0, "the far ball stayed down: {}", stirred[0]);
    assert!(stirred[1] < 1.0, "the iron stayed down: {}", stirred[1]);
}

/// A run with the player at x 300 on the sky's ground, the map's enemies
/// gone, a 60th of a second a frame and the camera held over it: the
/// flask's range.
fn flask_range() -> (UiHarness, Entity) {
    let (mut harness, player) = run();
    calm(&mut harness);
    ground(&mut harness, SKY);
    stand(&mut harness, player, 300.0, SKY);
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.step(1);
    pin_camera(&mut harness, Vec2::new(0.0, SKY - 600.0));
    harness.step(1);
    (harness, player)
}

/// An enemy of `kind` with its body's centre at `centre`, at normal
/// difficulty, still: its state machine taken off.
fn still_foe(harness: &mut UiHarness, kind: EnemyKind, centre: Vec2) -> Entity {
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let foe = enemies::spawn(world, kind, centre - kind_body(kind).centre, id).unwrap();
    enemies::scale(world, foe, Difficulty::Normal);
    world.remove_component::<StateMachine<EnemyState>>(foe);
    foe
}

/// Tosses the flask at `at` with one press: the sounds those frames made.
fn toss_flask(harness: &mut UiHarness, at: Vec2) -> Vec<Sound> {
    aim_at(harness, at);
    choose(harness, Spell::ToxicFlask);
    cast_once(harness)
}

/// A flask's flight: where it first burst, each flying flask's centre a
/// frame before it, and every hit part landed meanwhile: its target, its
/// amount, its source and whether it was poison.
struct Flight {
    burst: Option<Vec2>,
    path: Vec<Vec2>,
    parts: Vec<(Entity, f32, Option<Entity>, bool)>,
}

impl Flight {
    /// What `target` took in the flight.
    fn took(&self, target: Entity) -> Vec<(f32, Option<Entity>, bool)> {
        self.parts
            .iter()
            .filter(|part| part.0 == target)
            .map(|&(_, amount, source, poison)| (amount, source, poison))
            .collect()
    }
}

/// Steps until the first flask burst, at most `frames` frames, then one
/// more, so its splash has landed: the flight.
fn flask_flight(harness: &mut UiHarness, frames: u32) -> Flight {
    let mut flight = Flight {
        burst: None,
        path: Vec::new(),
        parts: Vec::new(),
    };
    for _ in 0..=frames {
        if let Some(&(_, flask)) = flask::flying(harness.world()).first() {
            flight.path.push(body_point(harness, flask));
        }
        harness.step(1);
        let world = harness.world();
        flight.parts.extend(
            harness
                .harness()
                .events::<DamagedPart>()
                .filter(|part| !part.periodic)
                .map(|part| {
                    let poison = part.kinds.has(world, Kind::Poison.tag());
                    (part.entity, part.amount, part.source, poison)
                }),
        );
        if flight.burst.is_some() {
            return flight;
        }
        let config = world
            .resource::<tungsten::core::ParticleConfigRegistry>()
            .id_for_name(flask::BURST_PARTICLES);
        flight.burst = world
            .query::<(
                &Burst,
                &tungsten::core::ParticleEmitter,
                &tungsten::core::Transform,
            )>()
            .find(|(_, emitter, _)| Some(emitter.config) == config)
            .map(|(.., at)| at.position);
    }
    flight
}

/// A flask's centre.
fn body_point(harness: &UiHarness, flask: Entity) -> Vec2 {
    harness.world().get::<Position>(flask).unwrap().0 + flask::FLASK_CENTRE
}

#[test]
fn a_flask_at_a_cursor_on_the_floor_400_px_off_bursts_within_24_px_of_it() {
    let (mut harness, player) = flask_range();
    let me = centre(&harness, player);
    let cursor = Vec2::new(me.x + 400.0, SKY);
    let heard = toss_flask(&mut harness, cursor);
    assert!(heard.contains(&Sound::FlaskThrown), "{heard:?}");
    let flight = flask_flight(&mut harness, 90);
    let at = flight.burst.expect("it bursts");
    assert!(at.distance(cursor) <= 24.0, "{at} for {cursor}");
    let top = flight.path.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    assert!(top < me.y - 60.0, "a lob: its apex {top} over {}", me.y);
}

#[test]
fn a_flask_lobbed_onto_a_ledge_at_a_shallow_angle_bursts_on_the_step_it_reaches_it() {
    let (mut harness, player) = flask_range();
    let me = centre(&harness, player);
    // A ledge 250 px over the wizard's centre from 330 px across: the lob
    // clears its corner and comes down on its top some 20° from flat.
    let top = me.y - 250.0;
    let edge = me.x + 330.0;
    floor(
        &mut harness,
        Vec2::new(edge + 185.0, top + 20.0),
        Vec2::new(185.0, 20.0),
    );
    harness.step(1);
    let cursor = Vec2::new(me.x + 400.0, top);
    toss_flask(&mut harness, cursor);
    let flight = flask_flight(&mut harness, 90);
    let (at, path) = (flight.burst.expect("it bursts"), &flight.path);
    let surface = top - flask::FLASK_RADIUS;
    assert!(
        path.iter()
            .filter(|p| p.x >= edge - flask::FLASK_RADIUS)
            .all(|p| p.y < surface),
        "never inside: {path:?}"
    );
    let (last, before) = (path[path.len() - 1], path[path.len() - 2]);
    let step = last - before;
    assert!(
        step.y > 0.0 && step.y < step.x * 0.5,
        "a shallow fall: {step}"
    );
    assert!((at.y - surface).abs() <= 6.0, "on the ledge's top: {at}");
    assert!(
        at.x >= last.x && at.x <= last.x + 1.5 * step.x,
        "on the step after {last}: {at}"
    );
}

#[test]
fn a_flask_tossed_100_px_straight_down_passes_through_the_wizard_and_bursts_at_the_cursor() {
    let (mut harness, player) = flask_range();
    // A slab 100 px under the wizard's centre in the air, his feet 72 px
    // over it.
    let me = Vec2::new(900.0, SKY - 400.0);
    floor(
        &mut harness,
        me + Vec2::new(0.0, 120.0),
        Vec2::new(150.0, 20.0),
    );
    harness.step(1);
    super::teleport(&mut harness, player, me);
    let full = harness.world().get::<Health>(player).unwrap().current();
    let cursor = me + Vec2::new(0.0, 100.0);
    toss_flask(&mut harness, cursor);
    let flight = flask_flight(&mut harness, 60);
    let at = flight.burst.expect("it bursts");
    assert!(
        at.distance(cursor) <= 8.0,
        "at the cursor: {at} for {cursor}"
    );
    assert!(flight.took(player).is_empty(), "nothing on the wizard");
    let health = harness.world().get::<Health>(player).unwrap().current();
    assert_eq!(health, full, "the wizard takes none");
}

#[test]
fn a_flask_lobbed_at_a_goblin_bursts_at_it_poisons_it_and_credits_the_wizard() {
    let (mut harness, player) = flask_range();
    let me = centre(&harness, player);
    let kind = EnemyKind::Goblin;
    let at = Vec2::new(me.x + 300.0, SKY - kind_body(kind).half.y - 0.5);
    let goblin = still_foe(&mut harness, kind, at);
    harness.step(1);
    toss_flask(&mut harness, at);
    let flight = flask_flight(&mut harness, 60);
    let burst = flight.burst.expect("it bursts");
    let half = kind_body(kind).half + Vec2::splat(flask::FLASK_RADIUS + 1.0);
    assert!(
        ((burst - at).abs() - half).max_element() <= 0.0,
        "at the goblin: {burst} for {at}"
    );
    let took = flight.took(goblin);
    assert_eq!(took.len(), 1, "one splash: {took:?}");
    assert!(took[0].0 > 0.0 && took[0].2, "poison: {took:?}");
    assert_eq!(took[0].1, Some(player), "the wizard's credit");
    let tags = harness.world().get::<Tags>(goblin).unwrap();
    assert!(tags.has("status.poisoned"), "poisoned");
}

#[test]
fn the_flask_splash_spares_the_wizard_and_a_wisp_beside_the_goblin() {
    let (mut harness, player) = flask_range();
    let me = centre(&harness, player);
    let kind = EnemyKind::Goblin;
    let at = Vec2::new(me.x + 80.0, SKY - kind_body(kind).half.y - 0.5);
    let goblin = still_foe(&mut harness, kind, at);
    let wisp = still_foe(&mut harness, EnemyKind::Wisp, at + Vec2::new(50.0, -20.0));
    harness.step(1);
    let health = |h: &UiHarness, e: Entity| h.world().get::<Health>(e).unwrap().current();
    let (wizard, spirit) = (health(&harness, player), health(&harness, wisp));
    toss_flask(&mut harness, at);
    let flight = flask_flight(&mut harness, 30);
    let burst = flight.burst.expect("it bursts");
    assert!(
        burst.distance(me) <= flask::SPLASH_RADIUS,
        "the wizard in reach"
    );
    assert!(
        burst.distance(body(&harness, wisp)) <= flask::SPLASH_RADIUS,
        "the wisp in reach"
    );
    assert_eq!(flight.took(goblin).len(), 1, "the goblin's splash");
    assert!(flight.took(player).is_empty() && flight.took(wisp).is_empty());
    harness.step(30);
    assert_eq!(health(&harness, player), wizard, "the wizard takes none");
    assert_eq!(health(&harness, wisp), spirit, "a wisp takes none");
    let poisoned =
        |h: &UiHarness, e: Entity| h.world().get::<Tags>(e).unwrap().has("status.poisoned");
    assert!(!poisoned(&harness, player) && !poisoned(&harness, wisp));
}

#[test]
fn a_fourth_flask_with_three_in_flight_is_refused_and_spends_nothing() {
    use tungsten::kit::Abilities;
    let (mut harness, player) = flask_range();
    let me = centre(&harness, player);
    let far = Vec2::new(me.x + 1400.0, SKY);
    for _ in 0..flask::FLASK_CAP {
        assert!(flask::throw(harness.world_mut(), player, me, far).is_some());
    }
    assert_eq!(flask::count(harness.world()), flask::FLASK_CAP);
    aim_at(&mut harness, far);
    choose(&mut harness, Spell::ToxicFlask);
    let spent = |h: &UiHarness| {
        let abilities = h.world().get::<Abilities>(player).unwrap();
        (
            mana(h),
            abilities.cooldown_left(Spell::ToxicFlask.ability()),
        )
    };
    let before = spent(&harness);
    let heard = cast_once(&mut harness);
    assert!(heard.contains(&Sound::Fizzled), "{heard:?}");
    assert!(!heard.contains(&Sound::FlaskThrown), "{heard:?}");
    let after = spent(&harness);
    assert!(after.0 >= before.0, "no mana spent: {before:?} → {after:?}");
    assert_eq!(after.1, before.1, "no cooldown");
    assert!(flask::count(harness.world()) <= flask::FLASK_CAP);
}
