//! Iron: a surface blast throws a frozen block's scraps away from it,
//! keeping its velocity; a blast on the floor scatters them; frozen iron
//! breaks only under a fast head-on iron impact, once per contact set, when
//! its queued shatter runs; a falling block shatters a frozen one; fast iron hurts the player and
//! crushes small balls, more the faster, keeping its momentum and sharing
//! its sound; holes pull iron slowly and scraps as balls; a fast drop
//! plows through a pile; iron is set only where it fits, at no cost where it
//! does not; a falling block knocks back what it does not kill and bursts
//! in gore what it does; and a thrown block falls on the troll's arc.

use example_01_platformer::features::combat::{self, Kind};
use example_01_platformer::features::elements::systems::crush;
use example_01_platformer::features::elements::{self, CRUSH_SPEED, SHATTER_SPEED, SMASH_KEEP};
use example_01_platformer::features::enemies::components::{EnemyKind, EnemyState};
use example_01_platformer::features::enemies::{self, troll};
use example_01_platformer::features::level::GRAVITY;
use example_01_platformer::features::player::PLAYER_HALF;
use example_01_platformer::features::player::components::Lantern;
use example_01_platformer::features::spells::components::Spell;
use example_01_platformer::features::toys::components::{
    IronBlock, IronScrap, PreviousVelocity, ScrapTumble, SmallBall,
};
use example_01_platformer::features::toys::systems::pull;
use example_01_platformer::features::toys::{
    self, BALL_RADIUS, IRON_HALF, IRON_MASS, IRON_PULL, SCRAP_HALF, SCRAP_MASS, SCRAP_PULL,
    SMALL_BALL_RADIUS,
};
use example_01_platformer::shared::SaveIds;
use example_01_platformer::shell::Difficulty;
use glam::Vec2;
use tungsten::core::{
    AudioCommand, Collider, Entity, Position, RigidBody, RigidBodyBundle, SoundRegistry, Velocity,
};
use tungsten::kit::{Abilities, Health, Knockback, StateMachine, Stats};
use tungsten::testing::UiHarness;

use super::{
    aim_at, ball, body, bursts, calm, cascades, choose, contact, floor, ground, kick, mana,
    pin_camera, player_entity, run, teleport,
};

/// The sky's ground the iron tests stand things on.
const TOP: f32 = 1000.0;

/// An enemy of `kind` standing still on the sky's ground, its centre at
/// `x`: no mind, so it stays.
fn standing(harness: &mut UiHarness, kind: EnemyKind, x: f32) -> Entity {
    let centre = Vec2::new(x, TOP - kind.info().half.y - 0.5);
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let enemy = enemies::spawn(world, kind, centre - kind.info().centre, id).expect("its prefab");
    enemies::scale(world, enemy, Difficulty::Normal);
    world.remove_component::<StateMachine<EnemyState>>(enemy);
    enemy
}

/// Gives `entity` `health` to the full: a target no blow kills.
fn tough(harness: &mut UiHarness, entity: Entity, health: f32) {
    let world = harness.world_mut();
    world
        .get_mut::<Stats>(entity)
        .unwrap()
        .set_base(Stats::MAX_HEALTH, health);
    harness.step(1);
    let world = harness.world_mut();
    world.get_mut::<Health>(entity).unwrap().heal(health);
}

/// The arena: the map's enemies off, the sky's ground, the camera held and
/// the lantern out, so mana only moves by a cast; one step a frame.
fn iron_arena() -> UiHarness {
    let (mut harness, player) = run();
    calm(&mut harness);
    ground(&mut harness, TOP);
    pin_camera(&mut harness, Vec2::new(0.0, 400.0));
    harness.harness_mut().set_dt(1.0 / 60.0);
    if let Some(lantern) = harness.world_mut().get_mut::<Lantern>(player) {
        lantern.on = false;
    }
    harness.step(2);
    harness
}

/// A block about `at`.
fn block(harness: &mut UiHarness, at: Vec2) -> Entity {
    toys::place_iron(harness.world_mut(), at)
}

/// The scraps' centres.
fn scraps(harness: &UiHarness) -> Vec<Vec2> {
    let world = harness.world();
    world
        .query::<(&IronScrap, &Position, &Collider)>()
        .map(|(_, p, c)| p.0 + c.offset)
        .collect()
}

fn moving(harness: &mut UiHarness, e: Entity, velocity: Vec2) {
    harness
        .world_mut()
        .get_mut::<PreviousVelocity>(e)
        .unwrap()
        .0 = velocity;
}

#[test]
fn surface_blasts_throw_scraps_away_from_the_contact_and_keep_parent_velocity() {
    let centre = Vec2::new(900.0, 500.0);
    let inherited = Vec2::new(120.0, -80.0);
    for direction in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
        let (mut harness, _) = run();
        let iron = block(&mut harness, centre);
        kick(&mut harness, iron, inherited);
        elements::freeze_iron(harness.world_mut(), iron);
        elements::thermal_shatter(harness.world_mut(), centre - direction * IRON_HALF.x, 72.0);
        cascades(&mut harness);
        let world = harness.world();
        let kicks: Vec<Vec2> = world
            .query::<(&IronScrap, &Velocity)>()
            .map(|(_, v)| v.0 - inherited)
            .collect();
        assert_eq!(kicks.len(), 16);
        assert!(kicks.iter().all(|v| v.is_finite()));
        let mean = kicks.iter().copied().sum::<Vec2>() / kicks.len() as f32;
        assert!(mean.dot(direction) > 300.0, "{direction} blast: {mean}");
    }
}

#[test]
fn a_floor_level_explosion_disperses_sixteen_scraps_through_real_physics() {
    let (mut harness, _) = run();
    let centre = Vec2::new(900.0, 500.0);
    floor(
        &mut harness,
        Vec2::new(900.0, 570.0),
        Vec2::new(1200.0, 10.0),
    );
    let iron = block(&mut harness, centre);
    elements::freeze_iron(harness.world_mut(), iron);
    toys::explode(harness.world_mut(), centre);
    harness.step(15);
    let positions = scraps(&harness);
    assert_eq!(positions.len(), 16);
    assert!(positions.iter().all(|p| p.is_finite()));
    let far = positions
        .iter()
        .filter(|p| p.distance(centre) > 180.0)
        .count();
    assert!(far >= 8, "the pieces stayed bunched: {positions:?}");
    let (min, max) = positions
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.x), hi.max(p.x))
        });
    assert!(
        max - min > 400.0,
        "the fracture spread across several tiles"
    );
    assert!(positions.iter().all(|p| p.y <= 560.0 - SCRAP_HALF.y + 1.0));
}

#[test]
fn frozen_iron_breaks_only_under_a_fast_head_on_iron_impact() {
    let cases = [
        (true, Vec2::X * (SHATTER_SPEED - 1.0), Vec2::ZERO, false),
        (true, Vec2::X * SHATTER_SPEED, Vec2::ZERO, true),
        (false, Vec2::X * 1200.0, Vec2::ZERO, false),
        (true, Vec2::Y * 1200.0, Vec2::ZERO, false),
        (true, -Vec2::X * 1200.0, Vec2::ZERO, false),
        (true, Vec2::X * 1200.0, Vec2::X * 1200.0, false),
    ];
    for (frozen, velocity, target_velocity, breaks) in cases {
        let (mut harness, _) = run();
        let o = Vec2::new(900.0, 500.0);
        let striker = block(&mut harness, o);
        let target = block(&mut harness, o + Vec2::X * (IRON_HALF.x * 2.0));
        moving(&mut harness, striker, velocity);
        moving(&mut harness, target, target_velocity);
        if frozen {
            elements::freeze_iron(harness.world_mut(), target);
        }
        crush(harness.world_mut());
        cascades(&mut harness);
        let world = harness.world();
        assert_eq!(
            !world.is_alive(target),
            breaks,
            "frozen={frozen}, {velocity}, {target_velocity}"
        );
        assert!(world.is_alive(striker));
        assert_eq!(scraps(&harness).len(), if breaks { 16 } else { 0 });
        if breaks {
            assert_eq!(
                world.get::<Velocity>(striker).unwrap().0,
                velocity * SMASH_KEEP
            );
        }
    }
}

#[test]
fn frozen_iron_shatters_once_from_solver_contacts_in_either_pair_order() {
    for reversed in [false, true] {
        let (mut harness, _) = run();
        let o = Vec2::new(900.0, 500.0);
        let striker = block(&mut harness, o);
        let target = block(&mut harness, o + Vec2::X * 400.0);
        elements::freeze_iron(harness.world_mut(), target);
        moving(&mut harness, striker, Vec2::X * SHATTER_SPEED);
        moving(&mut harness, target, Vec2::ZERO);
        // The solver stopped the striker and pushed the target clear;
        // repeated substep contacts make no second set of scraps.
        for _ in 0..3 {
            if reversed {
                contact(&mut harness, target, Some(striker), Vec2::X);
            } else {
                contact(&mut harness, striker, Some(target), -Vec2::X);
            }
        }
        crush(harness.world_mut());
        crush(harness.world_mut());
        cascades(&mut harness);
        assert!(!harness.world().is_alive(target));
        assert!(harness.world().is_alive(striker));
        assert_eq!(scraps(&harness).len(), 16);
        assert_eq!(bursts(&harness, "ex10_ice_shatter"), 1);
    }
}

#[test]
fn a_real_falling_iron_block_shatters_a_frozen_block_on_the_floor() {
    let (mut harness, _) = run();
    floor(
        &mut harness,
        Vec2::new(900.0, 570.0),
        Vec2::new(200.0, 10.0),
    );
    let target = block(&mut harness, Vec2::new(900.0, 500.0));
    elements::freeze_iron(harness.world_mut(), target);
    let striker = block(&mut harness, Vec2::new(900.0, 300.0));
    kick(&mut harness, striker, Vec2::Y * 1200.0);
    harness.step(10);
    assert!(!harness.world().is_alive(target));
    assert!(harness.world().is_alive(striker));
    assert_eq!(scraps(&harness).len(), 16);
}

#[test]
fn a_fast_brick_hurts_the_player_and_smashes_a_ball_and_a_resting_one_does_neither() {
    let (mut harness, player) = run();
    let at = Vec2::new(700.0, 600.0);
    teleport(&mut harness, player, at);
    let small = ball(&mut harness, at + Vec2::new(400.0, 0.0), true);
    // One block resting on the player's head, one on the ball.
    let blocks = [
        block(
            &mut harness,
            at - Vec2::new(0.0, PLAYER_HALF.y + IRON_HALF.y),
        ),
        block(
            &mut harness,
            at + Vec2::new(400.0, -(SMALL_BALL_RADIUS + IRON_HALF.y)),
        ),
    ];
    let drive = |harness: &mut UiHarness, speed: f32| {
        for block in blocks {
            moving(harness, block, Vec2::new(0.0, speed));
        }
        crush(harness.world_mut());
    };
    let health = |harness: &UiHarness| harness.world().get::<Health>(player).unwrap().current();
    drive(&mut harness, 0.0);
    drive(&mut harness, CRUSH_SPEED - 1.0);
    assert_eq!(health(&harness), 100.0);
    assert!(harness.world().is_alive(small));
    drive(&mut harness, CRUSH_SPEED + 40.0);
    assert_eq!(health(&harness), 66.0, "one hazard's hit");
    assert!(!harness.world().is_alive(small));
}

/// A plain dynamic body of `mass` about `at`.
fn mass(harness: &mut UiHarness, at: Vec2, mass: f32) -> Entity {
    harness.world_mut().spawn_with(
        RigidBodyBundle::dynamic(Position(at), Collider::circle(BALL_RADIUS))
            .with_body(RigidBody::dynamic().with_mass(mass)),
    )
}

#[test]
fn black_holes_move_iron_more_slowly_than_balls_and_skip_static_bodies() {
    let (mut harness, _) = run();
    let hole = Vec2::new(900.0, 500.0);
    let at = hole + Vec2::new(120.0, 0.0);
    let iron = block(&mut harness, at);
    let plain = mass(&mut harness, at, 1.0);
    let fixed = floor(&mut harness, at, Vec2::splat(10.0));
    toys::place_black_hole(harness.world_mut(), hole);
    pull(harness.world_mut());
    let world = harness.world();
    let ball_pull = world.get::<Velocity>(plain).unwrap().0;
    assert!(ball_pull.x < 0.0);
    // Both centres sit at the same point: the same pull, scaled.
    let iron_pull = world.get::<Velocity>(iron).unwrap().0;
    assert!(
        (iron_pull - ball_pull * IRON_PULL).length() < 0.5,
        "{iron_pull} vs {ball_pull}"
    );
    assert_eq!(
        world.get::<RigidBody>(iron).unwrap().inv_mass,
        1.0 / IRON_MASS
    );
    assert!(
        world
            .get::<Velocity>(fixed)
            .is_none_or(|v| v.0 == Vec2::ZERO)
    );
}

#[test]
fn black_holes_gather_warm_and_frozen_scraps_as_readily_as_balls() {
    let (mut harness, _) = run();
    let hole = Vec2::new(900.0, 500.0);
    let at = hole + Vec2::new(120.0, 0.0);
    let iron = block(&mut harness, at);
    let plain = mass(&mut harness, at, 1.0);
    let tumble = ScrapTumble {
        born_at: 0.0,
        initial_angle: 0.0,
        total_turn: 0.0,
    };
    let pieces = [false, true].map(|frozen| {
        let e = toys::spawn_scrap(harness.world_mut(), at, Vec2::ZERO, 0, tumble);
        if frozen {
            elements::freeze_iron(harness.world_mut(), e);
        }
        e
    });
    toys::place_black_hole(harness.world_mut(), hole);
    pull(harness.world_mut());
    let world = harness.world();
    let ball_pull = world.get::<Velocity>(plain).unwrap().0;
    let iron_pull = world.get::<Velocity>(iron).unwrap().0;
    assert!(ball_pull.x < 0.0);
    for scrap in pieces {
        let v = world.get::<Velocity>(scrap).unwrap().0;
        assert!(
            (v - ball_pull * SCRAP_PULL).length() < 0.5,
            "{v} vs {ball_pull}"
        );
        assert!(v.length() > iron_pull.length() * 4.9);
        assert_eq!(
            world.get::<RigidBody>(scrap).unwrap().inv_mass,
            1.0 / SCRAP_MASS
        );
    }
    assert!(world.has::<IronBlock>(iron));
}

#[test]
fn faster_slams_crush_more_marble_contacts_keep_momentum_and_throttle_audio() {
    fn slam(speed: f32) -> (usize, Vec2, usize, usize) {
        let (mut harness, _) = run();
        let o = Vec2::new(900.0, 400.0);
        let iron = block(&mut harness, o);
        moving(&mut harness, iron, Vec2::Y * speed);
        // Resolved positions no longer touch the block; repeated substep
        // contacts and both pair orders still crush each ball once.
        let small: Vec<Entity> = (0..32)
            .map(|i| {
                let e = ball(
                    &mut harness,
                    o + Vec2::new(i as f32 * 15.0 - 240.0, 150.0),
                    true,
                );
                let (a, b, normal) = if i % 2 == 0 {
                    (iron, e, -Vec2::Y)
                } else {
                    (e, iron, Vec2::Y)
                };
                for _ in 0..2 {
                    contact(&mut harness, a, Some(b), normal);
                }
                e
            })
            .collect();
        let heavy = ball(
            &mut harness,
            o + Vec2::new(0.0, IRON_HALF.y + BALL_RADIUS),
            false,
        );
        crush(harness.world_mut());
        let left = small
            .iter()
            .filter(|e| harness.world().is_alive(**e))
            .count();
        let velocity = harness.world().get::<Velocity>(iron).unwrap().0;
        let smashes = bursts(&harness, "ex10_ball_smash");
        assert!(
            harness.world().is_alive(heavy),
            "a bowling ball is no glass"
        );
        harness.step(1);
        let sound = harness
            .world()
            .resource::<SoundRegistry>()
            .get_by_id("ex10_iron_crush_sfx");
        let plays = |harness: &UiHarness| {
            harness
                .harness()
                .audio()
                .iter()
                .filter(
                    |c| matches!(c, AudioCommand::Play { handle, .. } if Some(*handle) == sound),
                )
                .count()
        };
        let first = plays(&harness);
        // A crush in the same short crunch shares that sound.
        moving(&mut harness, iron, Vec2::Y * speed);
        for e in &small {
            if harness.world().is_alive(*e) {
                contact(&mut harness, iron, Some(*e), -Vec2::Y);
            }
        }
        crush(harness.world_mut());
        harness.step(1);
        assert_eq!(plays(&harness), 0);
        (32 - left, velocity, smashes, first)
    }
    let slow = slam(CRUSH_SPEED);
    let fast = slam(1440.0);
    assert_eq!(slow.0, 4);
    assert_eq!(fast.0, 22);
    assert_eq!(fast.1, Vec2::Y * 1440.0 * SMASH_KEEP);
    assert_eq!(fast.2, 4);
    assert_eq!(fast.3, 1);
}

#[test]
fn a_fast_drop_plows_through_a_real_physics_pile() {
    let (mut harness, _) = run();
    floor(
        &mut harness,
        Vec2::new(900.0, 630.0),
        Vec2::new(200.0, 10.0),
    );
    for row in 0..9 {
        for col in 0..7 {
            let at = Vec2::new(900.0 + (col as f32 - 3.0) * 15.2, 612.5 - row as f32 * 15.2);
            ball(&mut harness, at, true);
        }
    }
    // The pile settles before the weight falls.
    harness.step(60);
    let iron = block(&mut harness, Vec2::new(900.0, 300.0));
    kick(&mut harness, iron, Vec2::Y * 1800.0);
    let before = harness.world().query::<&SmallBall>().count();
    harness.step(30);
    let smashed = before - harness.world().query::<&SmallBall>().count();
    assert!(smashed >= 21, "the drop crushed {smashed} of {before}");
    assert!(body(&harness, iron).y > 400.0);
}

#[test]
fn iron_is_not_set_inside_a_wall_or_a_body_and_costs_nothing() {
    let mut harness = iron_arena();
    let slab = Vec2::new(900.0, 700.0);
    floor(&mut harness, slab, Vec2::new(100.0, 20.0));
    let crawler = standing(&mut harness, EnemyKind::Crawler, 1300.0);
    harness.step(2);
    let player = player_entity(&harness);
    let iron = Spell::IronBlock.ability();
    let state = |h: &UiHarness| {
        let abilities = h.world().get::<Abilities>(player).unwrap();
        (
            mana(h),
            abilities.cooldown_left(iron),
            abilities.charges(iron),
        )
    };
    let blocks = |h: &UiHarness| h.world().query::<&IronBlock>().count();
    let cast = |h: &mut UiHarness, at: Vec2| {
        aim_at(h, at);
        choose(h, Spell::IronBlock);
        h.harness_mut().press_action("cast");
        h.step(1);
        h.harness_mut().release_action("cast");
        h.step(1);
    };
    let before = state(&harness);
    // The map's own blocks, in the mine.
    let placed = blocks(&harness);
    let into_crawler = body(&harness, crawler);
    for at in [slab, into_crawler] {
        cast(&mut harness, at);
        assert_eq!(blocks(&harness), placed, "no block at {at}");
        assert_eq!(state(&harness), before, "nothing spent at {at}");
    }
    // In the open, the same cast sets one and spends.
    cast(&mut harness, Vec2::new(1700.0, 600.0));
    assert_eq!(blocks(&harness), placed + 1);
    assert_ne!(state(&harness), before);
}

#[test]
fn a_falling_block_knocks_back_the_crawler_it_does_not_kill() {
    let mut harness = iron_arena();
    let crawler = standing(&mut harness, EnemyKind::Crawler, 1300.0);
    tough(&mut harness, crawler, 1000.0);
    let start = body(&harness, crawler);
    // Over its left half, so the closing line leans right.
    block(
        &mut harness,
        Vec2::new(1230.0, TOP - 20.0 - IRON_HALF.y - 100.0),
    );
    let mut pushed = Vec2::ZERO;
    for _ in 0..60 {
        harness.step(1);
        let knockback = harness.world().get::<Knockback>(crawler).unwrap();
        if knockback.is_active() {
            pushed = knockback.push();
            break;
        }
    }
    assert!(pushed.x > 0.0, "knocked away from the block: {pushed}");
    assert!(!harness.world().get::<Health>(crawler).unwrap().is_dead());
    harness.step(10);
    assert!(
        body(&harness, crawler).x > start.x + 2.0,
        "{}",
        body(&harness, crawler)
    );
}

#[test]
fn gore_follows_only_the_blow_that_kills() {
    // A block that kills a crawler bursts it in gore, no poof.
    let mut harness = iron_arena();
    let crawler = standing(&mut harness, EnemyKind::Crawler, 1300.0);
    harness.step(1);
    tungsten::kit::damage(harness.world_mut(), crawler, 55.0);
    block(
        &mut harness,
        Vec2::new(1300.0, TOP - 20.0 - IRON_HALF.y - 100.0),
    );
    let mut died = false;
    for _ in 0..60 {
        harness.step(1);
        if harness.world().get::<Health>(crawler).unwrap().is_dead() {
            died = true;
            break;
        }
    }
    assert!(died, "the block killed it");
    assert_eq!(bursts(&harness, "ex10_gore_burst"), 1, "gore");
    assert_eq!(bursts(&harness, "ex10_poof"), 0, "no poof");
    // A block that only hurts one, a shot then killing it in the same step,
    // leaves a poof.
    let mut harness = iron_arena();
    let crawler = standing(&mut harness, EnemyKind::Crawler, 1300.0);
    harness.step(1);
    let top = body(&harness, crawler).y - 10.0;
    let iron = block(&mut harness, Vec2::new(1300.0, top - IRON_HALF.y - 0.5));
    kick(&mut harness, iron, Vec2::new(0.0, 430.0));
    combat::Hit::new(crawler, [(Kind::Physical, 100.0)]).queue(harness.world_mut());
    harness.step(1);
    let health = harness.world().get::<Health>(crawler).unwrap();
    assert!(health.is_dead(), "the shot killed it");
    assert_eq!(bursts(&harness, "ex10_gore_burst"), 0, "no gore");
    assert_eq!(bursts(&harness, "ex10_poof"), 1, "its poof");
}

#[test]
fn a_thrown_block_falls_on_the_trolls_arc() {
    let mut harness = iron_arena();
    let step_fall = GRAVITY / 60.0;
    // Falling free, a block falls harder than the world's gravity.
    let free = block(&mut harness, Vec2::new(1700.0, 300.0));
    kick(&mut harness, free, Vec2::new(0.0, 100.0));
    let fall = |h: &mut UiHarness, iron: Entity| {
        let before = h.world().get::<Velocity>(iron).unwrap().0.y;
        h.step(1);
        h.world().get::<Velocity>(iron).unwrap().0.y - before
    };
    assert!(fall(&mut harness, free) > step_fall + 1.0, "its own weight");
    // Thrown, it falls by the world's gravity alone, as the troll aimed it.
    let thrower = standing(&mut harness, EnemyKind::Troll, 900.0);
    let thrown = block(&mut harness, Vec2::new(1000.0, TOP - IRON_HALF.y - 1.0));
    harness.step(1);
    let world = harness.world_mut();
    assert!(troll::pick_up(world, thrower, thrown), "it lifts it");
    assert_eq!(troll::throw(world, thrower), Some(thrown));
    let mut checked = 0;
    for _ in 0..90 {
        let falling = harness.world().get::<Velocity>(thrown).unwrap().0.y > 60.0;
        let gained = fall(&mut harness, thrown);
        if falling && gained > 0.0 {
            assert!((gained - step_fall).abs() < 1.0, "{gained} a step");
            checked += 1;
        }
        if checked >= 5 {
            break;
        }
    }
    assert!(checked >= 5, "it fell on its arc");
}
