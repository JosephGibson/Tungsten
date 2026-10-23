//! The spells and their toys: slot 1 fires a missile toward the cursor,
//! mana regenerates to its max and a cast pauses it; a missile's contact
//! blasts and lights small balls, the black hole leaves what burns in its
//! reach spent, a blast pushes bodies by distance and mass, and a blast's
//! layered bursts drain within the shared budget; three black holes open
//! at once, a fourth collapsing the oldest, each sweeping small balls from
//! its core, all humming on one voice.

use example_01_platformer::features::audio::HOLE_HUM;
use example_01_platformer::features::audio::components::Music;
use example_01_platformer::features::elements::components::BallBurn;
use example_01_platformer::features::elements::systems::douse;
use example_01_platformer::features::elements::{self};
use example_01_platformer::features::props::{self, components::PropKind};
use example_01_platformer::features::spells::systems::adopt;
use example_01_platformer::features::spells::{
    SUMMON_NEXT, SUMMON_PREV, SUMMONS, Summoned, summon_type,
};
use example_01_platformer::features::toys::components::{
    BlackHole, FireMissile, Ring, ShockRing, SmallBall,
};
use example_01_platformer::features::toys::{
    self, BALL_RADIUS, BROOM_MOST, HOLE_RADIUS, IRON_HALF, IRON_MASS, MISSILE_SPEED, PUSH_RADIUS,
    PUSH_SPEED, RING_CAP,
};
use example_01_platformer::shared::{BURST_CAP, Burst};
use glam::Vec2;
use tungsten::core::{
    AudioCommand, Collider, Entity, Particle, ParticleBudget, Position, RigidBody, RigidBodyBundle,
    SoundRegistry, Velocity,
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
    use example_01_platformer::features::spells::components::Spell;
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
    choose(&mut harness, 0);
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
    choose(&mut harness, 6);
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
    choose(&mut harness, 9);
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
    // Near the ground: a keg landing at 600 px/s or more goes off.
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
