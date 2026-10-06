use super::*;
use crate::burning::{
    BALL_BURN_SECONDS, BALL_FIRE_EMITTER_CAP, BallBurn, BallFireEmitter, ball_fire_particles,
    ignite, spread_ball_fire, tick_ball_fire,
};
use crate::gameplay::{Hazard, PreviousPosition, hazard_contacts};
use crate::state::SmallBall;
use tungsten::core::{AssetRegistry, Particle, ParticleBudget};

fn ball(world: &mut World, position: Vec2, small: bool) -> tungsten::core::Entity {
    let entity = world.spawn();
    world.insert(entity, Ball);
    world.insert(entity, Position(position));
    world.insert(entity, Velocity(Vec2::ZERO));
    if small {
        world.insert(entity, SmallBall::default());
    }
    entity
}

fn contact(world: &mut World, a: tungsten::core::Entity, b: tungsten::core::Entity) {
    world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap()
        .send(CollisionEvent {
            a,
            b: Some(b),
            normal: Vec2::X,
            penetration: 0.1,
        });
}

#[test]
fn fire_ignites_small_balls_including_swept_crossings_and_still_destroys_normal_balls() {
    let mut world = seed_world();
    let fire = world.spawn();
    world.insert(fire, Hazard { fire: true });
    world.insert(fire, Position(Vec2::ZERO));
    let small = ball(&mut world, Vec2::ZERO, true);
    let crossing = ball(&mut world, Vec2::new(100.0, 0.0), true);
    world.insert(crossing, PreviousPosition(Vec2::new(-100.0, 0.0)));
    let normal = ball(&mut world, Vec2::ZERO, false);
    let distant = ball(&mut world, Vec2::new(0.0, 100.0), true);
    hazard_contacts(&mut world);
    for e in [small, crossing] {
        assert!(world.get::<Ball>(e).is_some());
        assert_eq!(
            world.get::<BallBurn>(e).unwrap().remaining,
            BALL_BURN_SECONDS
        );
    }
    assert!(world.get::<Ball>(normal).is_none());
    assert!(world.get::<BallBurn>(distant).is_none());
    world.get_mut::<Hazard>(fire).unwrap().fire = false;
    world.get_mut::<Position>(distant).unwrap().0 = Vec2::ZERO;
    hazard_contacts(&mut world);
    assert!(
        world.get::<BallBurn>(distant).is_none(),
        "spikes cannot ignite balls"
    );
}

#[test]
fn contact_spread_is_symmetric_and_only_uses_current_events() {
    for reverse in [false, true] {
        let mut harness = platformer_harness(&[("spread_ball_fire", spread_ball_fire)]);
        let world = harness.world_mut();
        let source = ball(world, Vec2::ZERO, true);
        let target = ball(world, Vec2::new(100.0, 0.0), true);
        let stale = ball(world, Vec2::new(200.0, 0.0), true);
        let normal = ball(world, Vec2::ZERO, false);
        contact(world, source, stale);
        // Nothing burns yet, so this frame's spread returns early; its event
        // flush leaves the contact a frame old.
        harness.step(1);
        let world = harness.world_mut();
        ignite(world, source);
        contact(world, normal, source);
        if reverse {
            contact(world, target, source);
        } else {
            contact(world, source, target);
        }
        harness.step(1);
        let world = harness.world();
        assert_eq!(
            world.get::<BallBurn>(target).unwrap().remaining,
            BALL_BURN_SECONDS
        );
        assert!(world.get::<BallBurn>(stale).is_none());
        assert!(world.get::<BallBurn>(normal).is_none());
    }
}

#[test]
fn resting_pile_spreads_one_contact_hop_per_pass_without_collision_events() {
    let mut world = seed_world();
    // Cross a negative-coordinate grid boundary and allow resting solver slop.
    let source = ball(&mut world, Vec2::new(-1.0, 0.0), true);
    let next = ball(&mut world, Vec2::new(14.25, 0.0), true);
    let third = ball(&mut world, Vec2::new(29.5, 0.0), true);
    let gap = ball(&mut world, Vec2::new(0.0, 17.0), true);
    ignite(&mut world, source);
    spread_ball_fire(&mut world);
    assert!(world.get::<BallBurn>(next).is_some());
    assert!(world.get::<BallBurn>(third).is_none());
    assert!(world.get::<BallBurn>(gap).is_none());
    spread_ball_fire(&mut world);
    assert!(world.get::<BallBurn>(third).is_some());
    assert!(world.get::<BallBurn>(gap).is_none());
}

#[test]
fn burns_last_ten_seconds_without_refresh_and_spent_balls_never_reignite() {
    let mut world = seed_world();
    let spent = ball(&mut world, Vec2::ZERO, true);
    ignite(&mut world, spent);
    world
        .get_resource_mut::<Time>()
        .unwrap()
        .advance_frame(0.25);
    for _ in 0..39 {
        tick_ball_fire(&mut world);
        ignite(&mut world, spent);
    }
    assert_eq!(world.get::<BallBurn>(spent).unwrap().remaining, 0.25);
    tick_ball_fire(&mut world);
    assert_eq!(world.get::<BallBurn>(spent).unwrap().remaining, 0.0);
    assert!(world.get::<Ball>(spent).is_some());
    let cold = ball(&mut world, Vec2::new(15.0, 0.0), true);
    spread_ball_fire(&mut world);
    assert!(
        world.get::<BallBurn>(cold).is_none(),
        "spent balls cannot spread fire"
    );
    ignite(&mut world, cold);
    let fire = world.spawn();
    world.insert(fire, Hazard { fire: true });
    world.insert(fire, Position(Vec2::ZERO));
    hazard_contacts(&mut world);
    spread_ball_fire(&mut world);
    assert_eq!(world.get::<BallBurn>(spent).unwrap().remaining, 0.0);
    assert_eq!(
        world.get::<BallBurn>(cold).unwrap().remaining,
        BALL_BURN_SECONDS
    );
}

#[test]
fn burning_particles_use_a_rotating_bounded_pool_and_drain_after_burnout() {
    let mut harness = platformer_harness(&[(
        "transient_emitter_cleanup",
        crate::systems::transient_emitter_cleanup,
    )]);
    let world = harness.world_mut();
    load_presentation_assets(world);
    let first_ball = ball(world, Vec2::new(0.0, 10.0), true);
    ignite(world, first_ball);
    set_dt(&mut harness, 0.25);
    ball_fire_particles(harness.world_mut());
    harness.step(1);
    // Six times the old 12/sec output, split into plumes and faster sparks.
    for (sprite, expected) in [("ex10_flame_glow", 12), ("ex10_spark", 6)] {
        assert_eq!(
            harness
                .world()
                .query::<Particle>()
                .filter(|(_, p)| p.config.sprite == sprite)
                .count(),
            expected
        );
    }
    set_dt(&mut harness, 1.0 / 60.0);
    let world = harness.world_mut();
    for i in 1..2048 {
        let e = ball(world, Vec2::new(i as f32 * 20.0, 10.0), true);
        ignite(world, e);
    }
    ball_fire_particles(world);
    assert_eq!(
        world.query::<BallFireEmitter>().count(),
        BALL_FIRE_EMITTER_CAP
    );
    let first = world.query::<BallFireEmitter>().next().unwrap().0;
    let old_position = world.get::<Transform>(first).unwrap().position;
    world.insert_resource(crate::gameplay::SceneTime(0.125));
    ball_fire_particles(world);
    assert_ne!(
        world.get::<Transform>(first).unwrap().position,
        old_position
    );
    // Run the real emitter lifecycle with a deliberately tight shared budget.
    world.insert_resource(ParticleBudget { global_cap: 32 });
    for _ in 0..60 {
        ball_fire_particles(harness.world_mut());
        harness.step(1);
        assert!(harness.world().query::<Particle>().count() <= 32);
    }
    assert!(harness.world().query::<Particle>().count() > 0);
    set_dt(&mut harness, 10.0);
    let world = harness.world_mut();
    tick_ball_fire(world);
    ball_fire_particles(world);
    assert_eq!(world.query::<BallFireEmitter>().count(), 0);
    set_dt(&mut harness, 1.0 / 60.0);
    harness.step(90);
    let world = harness.world_mut();
    assert_eq!(world.query::<Particle>().count(), 0);
    assert_eq!(world.query::<Ball>().count(), 2048);
    let fresh = ball(world, Vec2::ZERO, true);
    ignite(world, fresh);
    ball_fire_particles(world);
    assert_eq!(world.query::<BallFireEmitter>().count(), 2);
    world.despawn(fresh);
    ball_fire_particles(world);
    assert_eq!(world.query::<BallFireEmitter>().count(), 0);
}

#[test]
fn extraction_shows_flames_while_burning_and_charcoal_after_burnout() {
    let mut world = seed_world();
    let e = ball(&mut world, Vec2::new(200.0, 200.0), true);
    let mut assets = AssetRegistry::new();
    mock_sprite(
        &mut assets,
        crate::state::SMALL_BALL_START_SPRITE_ID,
        950,
        false,
    );
    for i in 0..8 {
        mock_sprite(&mut assets, &format!("ex10_fire_{i}"), 951, true);
    }
    mock_sprite(&mut assets, "ex10_flame_glow", 952, false);
    world.insert_resource(assets);
    ignite(&mut world, e);
    let batches = crate::extract::extract_sprites(&world);
    let flames: Vec<_> = batches
        .iter()
        .filter(|b| b.texture.0 == 951)
        .flat_map(|b| &b.instances)
        .collect();
    assert_eq!(flames.len(), 2);
    assert!(
        batches
            .iter()
            .filter(|b| b.texture.0 == 951)
            .all(|b| !b.lit)
    );
    assert!(flames.iter().any(|f| f.size[1] >= 45.0));
    assert!(batches.iter().any(|b| b.texture.0 == 952));
    assert_eq!(
        batches
            .iter()
            .find(|b| b.texture.0 == 950)
            .unwrap()
            .instances[0]
            .color,
        [255, 150, 55, 255]
    );
    world
        .get_resource_mut::<Time>()
        .unwrap()
        .advance_frame(10.0);
    tick_ball_fire(&mut world);
    let batches = crate::extract::extract_sprites(&world);
    assert!(batches.iter().all(|b| !matches!(b.texture.0, 951 | 952)));
    assert_eq!(
        batches
            .iter()
            .find(|b| b.texture.0 == 950)
            .unwrap()
            .instances[0]
            .color,
        [55, 48, 45, 255]
    );
}
