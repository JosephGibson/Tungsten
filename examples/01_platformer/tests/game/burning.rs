//! Burning: fire lights small balls, swept crossings included, and bursts
//! bowling balls; contacts spread it symmetrically and only in their step;
//! a resting pile spreads it a hop a step; a burn lasts ten seconds and a
//! spent ball never lights again; the burning surface's particle pool
//! stays bounded and drains; and the flames draw on the surface only,
//! embers below and char after.

use example_01_platformer::features::elements::components::{BallBurn, FireEmitter};
use example_01_platformer::features::elements::draw::{FLAME_FRAMES, effects};
use example_01_platformer::features::elements::systems::{
    fire_contacts, fire_particles, flame_exposure, spread_fire, tick_fire,
};
use example_01_platformer::features::elements::{
    self, BURN_SECS, EMITTER_DWELL, FIRE_CENTRE, FIRE_EMITTER_CAP, FLAME_SIZE,
};
use example_01_platformer::features::level::components::{Hazard, SceneTime};
use example_01_platformer::features::toys::components::Ball;
use example_01_platformer::features::toys::systems::ball_look;
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shared::SaveId;
use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, CameraState, Collider, Entity, Particle, ParticleBudget, Position, PrevPosition,
    Sprite, Time, Transform,
};
use tungsten::testing::UiHarness;

use super::{ball, contact, floor, mock_sprite, run};

/// Sets a ball's centre now and before the step.
fn place(harness: &mut UiHarness, e: Entity, centre: Vec2, before: Vec2) {
    let world = harness.world_mut();
    let offset = world.get::<Collider>(e).unwrap().offset;
    world.get_mut::<Position>(e).unwrap().0 = centre - offset;
    world.get_mut::<PrevPosition>(e).unwrap().0 = before - offset;
}

/// A bare fire hazard whose touch box sits about `centre`.
fn fire(harness: &mut UiHarness, centre: Vec2) -> Entity {
    let at = centre - FIRE_CENTRE;
    harness.world_mut().spawn_with((
        Hazard { fire: true },
        Position(at),
        PrevPosition(at),
        Transform::from_position(at),
        SceneEntity { state_id: GAMEPLAY },
    ))
}

fn burn(harness: &UiHarness, e: Entity) -> Option<BallBurn> {
    harness.world().get::<BallBurn>(e).copied()
}

#[test]
fn fire_ignites_small_balls_including_swept_crossings_and_still_destroys_normal_balls() {
    let (mut harness, _) = run();
    let o = Vec2::new(1000.0, 500.0);
    let flame = fire(&mut harness, o);
    let small = ball(&mut harness, o, true);
    let crossing = ball(&mut harness, o, true);
    place(
        &mut harness,
        crossing,
        o + Vec2::new(100.0, 0.0),
        o - Vec2::new(100.0, 0.0),
    );
    let bowling = ball(&mut harness, o, false);
    let distant = ball(&mut harness, o + Vec2::new(0.0, 100.0), true);
    place(
        &mut harness,
        distant,
        o + Vec2::new(0.0, 100.0),
        o + Vec2::new(0.0, 100.0),
    );
    fire_contacts(harness.world_mut());
    for e in [small, crossing] {
        assert!(harness.world().is_alive(e));
        assert_eq!(burn(&harness, e).unwrap().remaining, BURN_SECS);
    }
    assert!(
        !harness.world().is_alive(bowling),
        "a fireball bursts a bowling ball"
    );
    assert!(burn(&harness, distant).is_none());
    harness.world_mut().get_mut::<Hazard>(flame).unwrap().fire = false;
    place(&mut harness, distant, o, o);
    fire_contacts(harness.world_mut());
    assert!(burn(&harness, distant).is_none(), "spikes light nothing");
}

#[test]
fn contact_spread_is_symmetric_and_only_uses_current_events() {
    for reverse in [false, true] {
        let (mut harness, _) = run();
        let o = Vec2::new(600.0, 400.0);
        let source = ball(&mut harness, o, true);
        let target = ball(&mut harness, o + Vec2::new(100.0, 0.0), true);
        let stale = ball(&mut harness, o + Vec2::new(200.0, 0.0), true);
        let bowling = ball(&mut harness, o + Vec2::new(300.0, 0.0), false);
        contact(&mut harness, source, Some(stale), Vec2::X);
        // Nothing burns, so this frame's spread does nothing, and its end
        // leaves the contact a frame old.
        harness.step(1);
        elements::ignite(harness.world_mut(), source);
        contact(&mut harness, bowling, Some(source), Vec2::X);
        if reverse {
            contact(&mut harness, target, Some(source), Vec2::X);
        } else {
            contact(&mut harness, source, Some(target), Vec2::X);
        }
        harness.step(1);
        // Lit after this step's tick, at its full burn.
        assert_eq!(burn(&harness, target).unwrap().remaining, BURN_SECS);
        assert!(burn(&harness, stale).is_none());
        assert!(
            burn(&harness, bowling).is_none(),
            "a bowling ball never burns"
        );
    }
}

#[test]
fn resting_pile_spreads_one_contact_hop_per_pass_without_collision_events() {
    let (mut harness, _) = run();
    // Across a negative-coordinate cell boundary, with resting slack.
    let o = Vec2::new(0.0, 400.0);
    let at = |x: f32, y: f32| o + Vec2::new(x, y);
    let source = ball(&mut harness, at(-1.0, 0.0), true);
    let next = ball(&mut harness, at(14.25, 0.0), true);
    let third = ball(&mut harness, at(29.5, 0.0), true);
    let gap = ball(&mut harness, at(0.0, 17.0), true);
    elements::ignite(harness.world_mut(), source);
    spread_fire(harness.world_mut());
    assert!(burn(&harness, next).is_some());
    assert!(burn(&harness, third).is_none());
    assert!(burn(&harness, gap).is_none());
    spread_fire(harness.world_mut());
    assert!(burn(&harness, third).is_some());
    assert!(burn(&harness, gap).is_none());
}

#[test]
fn burns_last_ten_seconds_without_refresh_and_spent_balls_never_reignite() {
    let (mut harness, _) = run();
    let o = Vec2::new(600.0, 400.0);
    let spent = ball(&mut harness, o, true);
    elements::ignite(harness.world_mut(), spent);
    harness
        .world_mut()
        .resource_mut::<Time>()
        .advance_frame(0.25);
    for _ in 0..39 {
        tick_fire(harness.world_mut());
        elements::ignite(harness.world_mut(), spent);
    }
    assert!((burn(&harness, spent).unwrap().remaining - 0.25).abs() < 1e-4);
    tick_fire(harness.world_mut());
    assert_eq!(burn(&harness, spent).unwrap().remaining, 0.0);
    assert!(harness.world().is_alive(spent));
    let cold = ball(&mut harness, o + Vec2::new(15.0, 0.0), true);
    spread_fire(harness.world_mut());
    assert!(
        burn(&harness, cold).is_none(),
        "a spent ball spreads nothing"
    );
    elements::ignite(harness.world_mut(), cold);
    fire(&mut harness, o);
    fire_contacts(harness.world_mut());
    spread_fire(harness.world_mut());
    assert_eq!(burn(&harness, spent).unwrap().remaining, 0.0);
    assert_eq!(burn(&harness, cold).unwrap().remaining, BURN_SECS);
}

/// The particles the burning pool's emitters own.
fn pool_particles(harness: &UiHarness, emitters: &[Entity]) -> usize {
    harness
        .world()
        .query::<&Particle>()
        .filter(|p| p.emitter.is_some_and(|e| emitters.contains(&e)))
        .count()
}

fn pool(harness: &UiHarness) -> Vec<Entity> {
    let world = harness.world();
    world
        .query::<(Entity, &FireEmitter)>()
        .map(|(e, _)| e)
        .collect()
}

#[test]
fn burning_particles_use_a_rotating_bounded_pool_and_drain_after_burnout() {
    let (mut harness, _) = run();
    let first = ball(&mut harness, Vec2::new(300.0, 300.0), true);
    elements::ignite(harness.world_mut(), first);
    harness.harness_mut().set_dt(0.25);
    harness.step(1);
    let emitters = pool(&harness);
    assert_eq!(emitters.len(), 2);
    harness.step(1);
    // A quarter second of one pair: twenty wisps and eight sparks a second.
    let world = harness.world();
    for (sprite, expected) in [("ex10_flame_glow", 5), ("ex10_spark", 2)] {
        let count = world
            .query::<&Particle>()
            .filter(|p| {
                p.emitter.is_some_and(|e| emitters.contains(&e)) && p.config.sprite == sprite
            })
            .count();
        assert!(count >= expected, "{sprite}: {count}");
    }
    harness.harness_mut().set_dt(1.0 / 60.0);
    // A floor in the sky holds the pile, so no ball falls out of the level.
    floor(
        &mut harness,
        Vec2::new(1150.0, 620.0),
        Vec2::new(1100.0, 10.0),
    );
    for i in 1..2048 {
        let at = Vec2::new(
            200.0 + (i % 96) as f32 * 20.0,
            100.0 + (i / 96) as f32 * 20.0,
        );
        let e = ball(&mut harness, at, true);
        elements::ignite(harness.world_mut(), e);
    }
    fire_particles(harness.world_mut());
    assert_eq!(pool(&harness).len(), FIRE_EMITTER_CAP);
    let mover = pool(&harness)[0];
    let before = harness.world().get::<Transform>(mover).unwrap().position;
    // Each pair hops once a dwell, the first at a whole dwell.
    // Past the part of a step the drawn clock trails by.
    harness
        .world_mut()
        .insert_resource(SceneTime(EMITTER_DWELL + 0.05));
    fire_particles(harness.world_mut());
    assert_ne!(
        harness.world().get::<Transform>(mover).unwrap().position,
        before
    );
    // The real emitter lifecycle under a tight shared budget, from no
    // particle alive.
    let world = harness.world_mut();
    let alive: Vec<Entity> = world
        .query::<(Entity, &Particle)>()
        .map(|(e, _)| e)
        .collect();
    for particle in alive {
        world.despawn(particle);
    }
    world.insert_resource(ParticleBudget { global_cap: 32 });
    let emitters = pool(&harness);
    let mut fed = false;
    for _ in 0..60 {
        harness.step(1);
        assert!(harness.world().query::<&Particle>().count() <= 32);
        fed |= pool_particles(&harness, &emitters) > 0;
    }
    assert!(fed, "the pool takes its share of the budget");
    // Burnout: every pair goes, and its particles drain.
    harness
        .world_mut()
        .resource_mut::<Time>()
        .advance_frame(10.0);
    tick_fire(harness.world_mut());
    fire_particles(harness.world_mut());
    assert!(pool(&harness).is_empty());
    harness.step(90);
    assert_eq!(pool_particles(&harness, &emitters), 0);
    assert_eq!(harness.world().query::<&Ball>().count(), 2048);
    let fresh = ball(&mut harness, Vec2::new(300.0, 200.0), true);
    elements::ignite(harness.world_mut(), fresh);
    fire_particles(harness.world_mut());
    assert_eq!(pool(&harness).len(), 2);
    harness.world_mut().despawn(fresh);
    fire_particles(harness.world_mut());
    assert!(pool(&harness).is_empty());
}

#[test]
fn extraction_shows_flames_on_the_surface_embers_beneath_and_charcoal_after_burnout() {
    let (mut harness, _) = run();
    let at = Vec2::new(800.0, 400.0);
    harness.world_mut().resource_mut::<CameraState>().position = at - Vec2::splat(300.0);
    let e = ball(&mut harness, at, true);
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    mock_sprite(assets, "ex10_ball_small", 950, false);
    for name in FLAME_FRAMES {
        mock_sprite(assets, name, 951, true);
    }
    mock_sprite(assets, "ex10_flame_glow", 952, false);
    elements::ignite(harness.world_mut(), e);
    let tongues = |harness: &UiHarness| {
        let batches = effects(harness.world());
        assert!(
            batches
                .iter()
                .filter(|b| b.texture.0 == 951)
                .all(|b| !b.lit)
        );
        batches
            .iter()
            .filter(|b| b.texture.0 == 951)
            .flat_map(|b| b.instances.clone())
            .collect::<Vec<_>>()
    };
    // One unlit tongue at its exact pixel scale over a glow.
    let drawn = tongues(&harness);
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].size, [FLAME_SIZE; 2]);
    assert!(effects(harness.world()).iter().any(|b| b.texture.0 == 952));
    ball_look(harness.world_mut());
    let id = *harness.world().get::<SaveId>(e).unwrap();
    let time = example_01_platformer::features::level::systems::drawn_time(harness.world());
    let tint = harness.world().get::<Sprite>(e).unwrap().color;
    assert_eq!(
        tint,
        elements::ember_tint(id, burn(&harness, e).unwrap(), time)
    );
    // A ball resting on it buries it: it glows but shows no tongue.
    let cover = ball(&mut harness, at - Vec2::new(0.0, 14.0), true);
    flame_exposure(harness.world_mut());
    assert!(tongues(&harness).is_empty());
    harness.world_mut().despawn(cover);
    harness
        .world_mut()
        .resource_mut::<Time>()
        .advance_frame(10.0);
    tick_fire(harness.world_mut());
    let batches = effects(harness.world());
    assert!(batches.iter().all(|b| !matches!(b.texture.0, 951 | 952)));
    ball_look(harness.world_mut());
    assert_eq!(
        harness.world().get::<Sprite>(e).unwrap().color,
        [55, 48, 45, 255]
    );
}
