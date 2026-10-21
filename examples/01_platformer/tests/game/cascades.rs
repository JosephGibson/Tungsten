//! Cascades: explosions chain through a frozen pile and steam bursts
//! through a burning one; a chain past the cap finishes over steps in its
//! order; a full pit's chain stays under the bound; a hard impact or a fall
//! shatters a frozen body; frozen things thaw; cold douses a fire hazard;
//! moving bodies hurt the dummy; thermal shock hurts the player at half;
//! a dummy's lethal reaction is its one burst; a steam burst's own fire
//! relights nothing; and fire and cold in one step leave one state.

use example_01_platformer::features::combat::{Hit, Kind};
use example_01_platformer::features::elements::cascade::{self, CascadeKind, Cascades};
use example_01_platformer::features::elements::components::{BallBurn, Frozen};
use example_01_platformer::features::elements::{self, CASCADE_BOUND, CASCADE_CAP, FROZEN_SECS};
use example_01_platformer::features::level::components::Doused;
use example_01_platformer::features::toys::components::IronScrap;
use example_01_platformer::features::toys::{self, BALL_CAP, IRON_HALF};
use example_01_platformer::shared::{BURST_CAP, SaveId};
use glam::Vec2;
use tungsten::core::{Entity, EventQueue, Position};
use tungsten::kit::{ContactDamage, DamageRequest, DamagedPart, KitRng, Oscillator};
use tungsten::testing::UiHarness;

use super::combat::{CRIT_SEED, PLAIN_SEED, TOP, apply, dummy_at, has, health, with_dummy};
use super::{aim_at, ball, body, bursts, cascades, ground, kick, pin_camera, run, stand};

/// A pile of `columns` × `rows` small balls on the sky's ground, 16 px
/// apart, from `x`, frozen when `frozen`, else lit.
fn pile(harness: &mut UiHarness, x: f32, columns: usize, rows: usize, frozen: bool) -> Vec<Entity> {
    let mut balls = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let at = Vec2::new(x + column as f32 * 16.0, TOP - 8.0 - row as f32 * 16.0);
            let small = ball(harness, at, true);
            let world = harness.world_mut();
            if frozen {
                elements::freeze_iron(world, small);
            } else {
                elements::ignite(world, small);
            }
            balls.push(small);
        }
    }
    balls
}

fn live(harness: &UiHarness, balls: &[Entity]) -> usize {
    balls
        .iter()
        .filter(|&&ball| harness.world().is_alive(ball))
        .count()
}

/// A run on the sky's ground, the player far to the left.
fn sky() -> (UiHarness, Entity) {
    let (mut harness, player) = run();
    ground(&mut harness, TOP);
    stand(&mut harness, player, 100.0, TOP);
    harness.step(1);
    (harness, player)
}

fn entries(harness: &UiHarness) -> Vec<cascade::Entry> {
    harness.world().resource::<Cascades>().entries.clone()
}

#[test]
fn explosions_chain_through_a_frozen_pile() {
    let (mut harness, _) = sky();
    let balls = pile(&mut harness, 1000.0, 8, 4, true);
    let first = body(&harness, balls[0]);
    // Fire on the first ice ball: thermal shock, ring after ring.
    assert!(cascade::thermal_shock(
        harness.world_mut(),
        balls[0],
        first,
        first
    ));
    let mut left = Vec::new();
    for _ in 0..60 {
        harness.step(1);
        left.push(live(&harness, &balls));
    }
    assert_eq!(left.last(), Some(&0), "the whole pile went: {left:?}");
    assert!(
        left[0] < balls.len(),
        "the first explosion's ring: {left:?}"
    );
    assert!(entries(&harness).is_empty());
}

#[test]
fn a_steam_chain_pops_a_burning_pile() {
    let (mut harness, _) = sky();
    let balls = pile(&mut harness, 1000.0, 8, 3, false);
    assert!(
        balls
            .iter()
            .all(|&b| harness.world().get::<BallBurn>(b).is_some())
    );
    let first = body(&harness, balls[0]);
    assert!(cascade::queue(
        harness.world_mut(),
        CascadeKind::Steam,
        Some(balls[0]),
        first,
        first,
        elements::SMALL_STEAM_REACH,
    ));
    harness.step(60);
    assert_eq!(live(&harness, &balls), 0, "each burning ball burst in turn");
    assert!(bursts(&harness, "ex10_steam_burst") <= BURST_CAP);
}

/// Each live ball of `balls` by save ID.
fn ids(harness: &UiHarness, balls: &[Entity]) -> Vec<u64> {
    balls
        .iter()
        .filter(|&&b| harness.world().is_alive(b))
        .map(|&b| harness.world().get::<SaveId>(b).unwrap().0)
        .collect()
}

/// The order a chain past the cap runs in, a run at a time: the save IDs
/// each run took.
fn chain_order() -> Vec<Vec<u64>> {
    let (mut harness, _) = sky();
    let balls = pile(&mut harness, 1000.0, 12, 6, true);
    let centre = Vec2::new(1088.0, TOP - 40.0);
    elements::thermal_shatter(harness.world_mut(), centre, 72.0);
    let queued = entries(&harness);
    assert!(
        queued.len() > CASCADE_CAP,
        "{} in the first ring",
        queued.len()
    );
    // The first run takes the cap's worth nearest the origin, then by save ID.
    let mut order: Vec<(f32, u64)> = queued
        .iter()
        .map(|e| (Vec2::from(e.at).distance(centre), e.target.unwrap()))
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let expected: Vec<u64> = order.iter().take(CASCADE_CAP).map(|&(_, id)| id).collect();
    let mut runs = Vec::new();
    let mut before = ids(&harness, &balls);
    for run in 0..40 {
        cascades(&mut harness);
        let after = ids(&harness, &balls);
        let took: Vec<u64> = before
            .iter()
            .copied()
            .filter(|id| !after.contains(id))
            .collect();
        assert!(took.len() <= CASCADE_CAP, "run {run} took {}", took.len());
        if run == 0 {
            let mut took_sorted = took.clone();
            took_sorted.sort_unstable();
            let mut wanted = expected.clone();
            wanted.sort_unstable();
            assert_eq!(took_sorted, wanted, "the nearest first");
        }
        runs.push(took);
        before = after;
        if entries(&harness).is_empty() {
            break;
        }
    }
    assert!(
        before.is_empty(),
        "the chain finished: {} left",
        before.len()
    );
    assert!(runs.len() > 1, "over several runs");
    runs
}

#[test]
fn a_chain_past_the_cap_finishes_over_steps_in_its_stable_order() {
    assert_eq!(chain_order(), chain_order(), "the same order every time");
}

#[test]
fn a_full_pits_chain_stays_under_the_bound() {
    let (mut harness, _) = run();
    let world = harness.world_mut();
    let live = toys::ball_count(world);
    let mut balls = Vec::with_capacity(BALL_CAP);
    for i in 0..BALL_CAP - live {
        let at = Vec2::new(
            100.0 + (i % 150) as f32 * 15.0,
            100.0 + (i / 150) as f32 * 15.0,
        );
        let small = toys::spawn_ball(world, at, true);
        elements::freeze_iron(world, small);
        balls.push(small);
    }
    let first = Vec2::new(100.0, 100.0);
    assert!(cascade::thermal_shock(world, balls[0], first, first));
    let (mut peak, mut runs) = (0, 0);
    while !world.resource::<Cascades>().entries.is_empty() && runs < 2000 {
        cascade::run(world);
        peak = peak.max(world.resource::<Cascades>().entries.len());
        runs += 1;
    }
    let dropped = world.resource::<Cascades>().dropped;
    let left = balls.iter().filter(|&&b| world.is_alive(b)).count();
    println!(
        "full pit chain: {} balls, {runs} runs, queue peak {peak}, dropped {dropped}, {left} left",
        balls.len()
    );
    assert!(peak <= CASCADE_BOUND && dropped == 0);
    assert_eq!(left, 0, "the chain took the whole pit");
}

#[test]
fn a_hard_impact_or_a_fall_shatters_a_frozen_body() {
    let (mut harness, _) = sky();
    // A bowling ball's mass into frozen iron at 900 px/s.
    let iron = toys::place_iron(harness.world_mut(), Vec2::new(1200.0, TOP - IRON_HALF.y));
    elements::freeze_iron(harness.world_mut(), iron);
    let bowling = ball(&mut harness, Vec2::new(1080.0, TOP - IRON_HALF.y), false);
    kick(&mut harness, bowling, Vec2::new(900.0, 0.0));
    // Small balls as fast into other frozen iron break no ice.
    let other = toys::place_iron(harness.world_mut(), Vec2::new(1700.0, TOP - IRON_HALF.y));
    elements::freeze_iron(harness.world_mut(), other);
    for row in 0..3 {
        let small = ball(
            &mut harness,
            Vec2::new(1600.0, TOP - 30.0 - row as f32 * 18.0),
            true,
        );
        kick(&mut harness, small, Vec2::new(900.0, 0.0));
    }
    // An ice ball falling onto the ground at 800 px/s.
    let ice = ball(&mut harness, Vec2::new(2000.0, TOP - 60.0), true);
    elements::freeze_iron(harness.world_mut(), ice);
    kick(&mut harness, ice, Vec2::new(0.0, 800.0));
    harness.step(12);
    let world = harness.world();
    assert!(!world.is_alive(iron), "the bowling ball shattered it");
    assert_eq!(world.query::<&IronScrap>().count(), 16);
    assert!(
        world.is_alive(other) && world.has::<Frozen>(other),
        "small balls break no ice"
    );
    assert!(!world.is_alive(ice), "the fall shattered it");
}

#[test]
fn frozen_things_thaw_after_five_seconds() {
    let (mut harness, _) = sky();
    let iron = toys::place_iron(harness.world_mut(), Vec2::new(1200.0, TOP - IRON_HALF.y));
    let small = ball(&mut harness, Vec2::new(1500.0, TOP - 8.0), true);
    let (mut dummy_harness, _, dummy) = with_dummy();
    for body in [iron, small] {
        elements::freeze_iron(harness.world_mut(), body);
    }
    apply(&mut dummy_harness, dummy, "frozen", None);
    let steps = (FROZEN_SECS * 60.0) as u32;
    harness.step(steps - 6);
    dummy_harness.step(steps - 6);
    assert!(harness.world().has::<Frozen>(iron) && harness.world().has::<Frozen>(small));
    assert!(has(&dummy_harness, dummy, "frozen"));
    harness.step(12);
    dummy_harness.step(12);
    assert!(!harness.world().has::<Frozen>(iron) && !harness.world().has::<Frozen>(small));
    assert!(!has(&dummy_harness, dummy, "frozen"));
}

#[test]
fn cold_douses_a_fire_hazard_for_a_while() {
    let (mut harness, player) = sky();
    // A fireball of the level's, set still in the sky beside the player.
    let fire = harness
        .world_mut()
        .prepare_prefab("ex10_fire_6", [420.0, TOP - 200.0])
        .expect("the level's fireball prefab")
        .spawn(harness.world_mut());
    harness.world_mut().remove_component::<Oscillator>(fire);
    harness.step(1);
    let at = harness.world().get::<Position>(fire).unwrap().0 + elements::FIRE_CENTRE;
    let eye = super::centre(&harness, player);
    pin_camera(&mut harness, eye - Vec2::new(640.0, 360.0));
    super::choose(&mut harness, 2);
    aim_at(&mut harness, at);
    harness.harness_mut().press_action("cast");
    harness.step(4);
    harness.harness_mut().release_action("cast");
    assert!(harness.world().has::<Doused>(fire), "the cold put it out");
    assert!(
        !harness.world().has::<ContactDamage>(fire),
        "its touch held aside"
    );
    assert!(bursts(&harness, "ex10_steam_burst") >= 1, "with steam");
    harness.step(60 * 4 + 10);
    assert!(!harness.world().has::<Doused>(fire), "it relit");
    assert!(harness.world().has::<ContactDamage>(fire));
}

/// The physical parts the last frame landed on `target`.
fn physical(harness: &UiHarness, target: Entity) -> Vec<f32> {
    let world = harness.world();
    world
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .filter(|part| part.entity == target && part.kinds.has(world, Kind::Physical.tag()))
        .map(|part| part.amount)
        .collect()
}

#[test]
fn moving_bodies_hurt_the_dummy_by_speed_and_mass() {
    let (mut harness, _, dummy) = with_dummy();
    harness
        .world_mut()
        .insert_resource(KitRng::seeded(PLAIN_SEED));
    let at = body(&harness, dummy);
    let bowling = ball(&mut harness, at - Vec2::new(60.0, 0.0), false);
    kick(&mut harness, bowling, Vec2::new(1000.0, 0.0));
    let mut landed = Vec::new();
    for _ in 0..6 {
        harness.step(1);
        landed.extend(physical(&harness, dummy));
    }
    // 0.002 × 8 × (about 1,000 − 400).
    assert_eq!(landed.len(), 1, "{landed:?}");
    assert!((8.0..=10.0).contains(&landed[0]), "{landed:?}");
    let (mut harness, _, dummy) = with_dummy();
    harness
        .world_mut()
        .insert_resource(KitRng::seeded(PLAIN_SEED));
    let at = body(&harness, dummy);
    let iron = toys::place_iron(
        harness.world_mut(),
        at - Vec2::new(IRON_HALF.x + 30.0, 30.0),
    );
    kick(&mut harness, iron, Vec2::new(800.0, 0.0));
    let mut landed = Vec::new();
    for _ in 0..6 {
        harness.step(1);
        landed.extend(physical(&harness, dummy));
    }
    assert_eq!(landed.first(), Some(&120.0), "iron's capped: {landed:?}");
}

#[test]
fn thermal_shock_hurts_the_player_at_half() {
    let (mut harness, player) = sky();
    harness
        .world_mut()
        .insert_resource(KitRng::seeded(PLAIN_SEED));
    let eye = super::centre(&harness, player);
    let iron = toys::place_iron(
        harness.world_mut(),
        Vec2::new(eye.x + 120.0, TOP - IRON_HALF.y),
    );
    harness.step(1);
    elements::freeze_iron(harness.world_mut(), iron);
    let at = body(&harness, iron);
    assert!(cascade::thermal_shock(harness.world_mut(), iron, at, at));
    harness.step(1);
    let world = harness.world();
    let mut parts: Vec<f32> = world
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .filter(|part| part.entity == player)
        .map(|part| part.amount)
        .collect();
    parts.sort_by(f32::total_cmp);
    assert_eq!(
        parts,
        [10.0, 10.0],
        "half of frozen iron's 20 fire and 20 physical"
    );
    assert!(!world.is_alive(iron));
}

#[test]
fn lethal_fire_on_a_frozen_dummy_explodes_as_its_one_burst() {
    let (mut harness, _, dummy) = with_dummy();
    apply(&mut harness, dummy, "frozen", None);
    DamageRequest::new(dummy, 395.0).apply(harness.world_mut());
    let id = harness.world().get::<SaveId>(dummy).unwrap().0;
    Hit::new(dummy, [(Kind::Fire, 10.0)]).queue(harness.world_mut());
    harness.step(1);
    assert_eq!(health(&harness, dummy), 0.0, "killed");
    let world = harness.world();
    assert!(
        world
            .resource::<Cascades>()
            .waits(CascadeKind::Explosion, id)
    );
    assert_eq!(
        bursts(&harness, "ex10_ice_shatter") + bursts(&harness, "ex10_poof"),
        0
    );
    harness.step(1);
    assert_eq!(bursts(&harness, "ex10_frost_blast"), 1, "its one burst");
    assert_eq!(
        bursts(&harness, "ex10_ice_shatter") + bursts(&harness, "ex10_poof"),
        0
    );
}

#[test]
fn lethal_cold_on_a_burning_dummy_steams_as_its_one_burst() {
    let (mut harness, _, dummy) = with_dummy();
    apply(&mut harness, dummy, "burning", None);
    DamageRequest::new(dummy, 395.0).apply(harness.world_mut());
    let id = harness.world().get::<SaveId>(dummy).unwrap().0;
    Hit::new(dummy, [(Kind::Cold, 10.0)]).queue(harness.world_mut());
    harness.step(1);
    assert_eq!(health(&harness, dummy), 0.0, "killed");
    assert!(
        harness
            .world()
            .resource::<Cascades>()
            .waits(CascadeKind::Steam, id)
    );
    assert_eq!(
        bursts(&harness, "ex10_burn_out") + bursts(&harness, "ex10_poof"),
        0
    );
    harness.step(1);
    assert_eq!(bursts(&harness, "ex10_steam_burst"), 1, "its one burst");
    assert_eq!(
        bursts(&harness, "ex10_burn_out") + bursts(&harness, "ex10_poof"),
        0
    );
}

#[test]
fn a_steam_bursts_own_fire_relights_nothing() {
    let (mut harness, _, near) = with_dummy();
    let far = dummy_at(&mut harness, 960.0);
    harness.step(1);
    for dummy in [near, far] {
        apply(&mut harness, dummy, "burning", None);
    }
    // A crit's steam: 10 fire would reach burning's quarter of 40.
    harness
        .world_mut()
        .insert_resource(KitRng::seeded(CRIT_SEED));
    for dummy in [near, far] {
        Hit::new(dummy, [(Kind::Cold, 5.0)]).queue(harness.world_mut());
    }
    harness.step(1);
    let waiting: Vec<CascadeKind> = entries(&harness).iter().map(|e| e.kind).collect();
    assert_eq!(
        waiting,
        [CascadeKind::Steam, CascadeKind::Steam],
        "overlapping bursts"
    );
    harness.step(3);
    for dummy in [near, far] {
        assert!(
            !has(&harness, dummy, "burning"),
            "put out, and not lit again"
        );
        assert!(has(&harness, dummy, "doused"));
    }
}

#[test]
fn fire_and_cold_in_one_step_leave_one_state_and_no_ending() {
    let (mut harness, _, dummy) = with_dummy();
    let world = harness.world_mut();
    Hit::new(dummy, [(Kind::Fire, 15.0)]).queue(world);
    Hit::new(dummy, [(Kind::Cold, 15.0)]).queue(world);
    harness.step(1);
    assert!(has(&harness, dummy, "burning") && !has(&harness, dummy, "chilled"));
    assert!(
        entries(&harness).is_empty(),
        "neither state held when the step began"
    );
    Hit::new(dummy, [(Kind::Cold, 15.0)]).queue(harness.world_mut());
    harness.step(1);
    let waiting: Vec<CascadeKind> = entries(&harness).iter().map(|e| e.kind).collect();
    assert_eq!(
        waiting,
        [CascadeKind::Steam],
        "the next step's cold meets the burning"
    );
}
