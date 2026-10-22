//! The gun: the magazine empties and comes back whole; a shot hits the
//! dummy for low physical damage that can crit, kicks a ball and turns off
//! iron; one decision a step takes what the shot met first, so what lies
//! behind a nearer dummy or wall is untouched, iron in front of a dummy
//! ends the shot, and a ball crossing before a wall stop is kicked and one
//! after it is not; the HUD shows the magazine; on the map, a shot turns
//! off the mine's iron and passes the rope bridge's wood to end at stone.

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::gun::components::Bullet;
use example_01_platformer::features::gun::{self, BALL_KICK, MAX_RICOCHETS, SHOOT, SHOT_DAMAGE};
use example_01_platformer::features::spells::hud::Hud;
use example_01_platformer::features::toys::{self, IRON_HALF};
use example_01_platformer::shared::SaveId;
use example_01_platformer::shell::widgets::text_of;
use glam::Vec2;
use tungsten::core::{Entity, EventQueue, RigidBody, Velocity};
use tungsten::kit::{
    DamagedPart, KitRng, LinearMotion, ProjectileBounced, ProjectileExpired, ProjectilePassed,
};
use tungsten::testing::UiHarness;

use super::combat::{CRIT_SEED, PLAIN_SEED, TOP, dummy_at, health};
use example_01_platformer::features::level::TILE;

use super::{
    aim_at, ball, body, bursts, calm, floor, ground, kick, pin_camera, player_entity, run,
};
use super::{stand, steps_this_frame};

/// A bullet's speed, as the gun's payload fires it.
const SPEED: f32 = 2400.0;
/// An iron block's centre on the sky's ground.
const IRON_Y: f32 = TOP - IRON_HALF.y;
/// The camera's corner over the sky's ground.
const CORNER: Vec2 = Vec2::new(0.0, 400.0);

/// A run on the sky's ground, the player far to the left and the camera
/// held over it.
fn sky() -> (UiHarness, Entity) {
    let (mut harness, player) = run();
    ground(&mut harness, TOP);
    stand(&mut harness, player, 100.0, TOP);
    harness.step(1);
    pin_camera(&mut harness, CORNER);
    (harness, player)
}

/// Fires a bullet from the player with its centre at `centre` at
/// `velocity`, landed at the frame's flush: it flies from the next step.
fn shot(harness: &mut UiHarness, centre: Vec2, velocity: Vec2) -> SaveId {
    let player = player_entity(harness);
    let id = gun::fire(harness.world_mut(), centre, velocity, Some(player)).expect("a bullet");
    harness.step(1);
    assert!(bullet(harness, id).is_some(), "landed");
    id
}

/// The bullet of save ID `id`, while it flies.
fn bullet(harness: &UiHarness, id: SaveId) -> Option<Entity> {
    harness
        .world()
        .query::<(Entity, &Bullet, &SaveId)>()
        .find(|&(_, _, own)| *own == id)
        .map(|(entity, ..)| entity)
}

fn velocity(harness: &UiHarness, entity: Entity) -> Vec2 {
    harness.world().get::<Velocity>(entity).expect("a body").0
}

fn sounds(harness: &UiHarness, sound: Sound) -> usize {
    harness
        .harness()
        .events::<Sounded>()
        .filter(|sounded| sounded.sound == sound)
        .count()
}

/// The magazine's rounds left and its most.
fn rounds(harness: &UiHarness) -> (u32, u32) {
    gun::rounds(harness.world(), player_entity(harness)).expect("the gun")
}

/// The physical parts `target` took this frame: their amounts and crits.
fn parts(harness: &UiHarness, target: Entity) -> Vec<(f32, bool)> {
    let world = harness.world();
    world
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .filter(|part| part.entity == target && !part.periodic)
        .filter(|part| part.kinds.has(world, "damage.physical"))
        .map(|part| (part.amount, part.crit))
        .collect()
}

#[test]
fn the_magazine_empties_and_comes_back_whole_after_the_reload() {
    let (mut harness, _) = sky();
    aim_at(&mut harness, Vec2::new(700.0, 500.0));
    assert_eq!(rounds(&harness), (12, 12));
    harness.harness_mut().press_action(SHOOT);
    let mut seen = vec![12];
    let mut shots = 0;
    for _ in 0..200 {
        harness.step(1);
        shots += sounds(&harness, Sound::Shot);
        let (left, _) = rounds(&harness);
        if seen.last() != Some(&left) {
            seen.push(left);
        }
        if left == 0 {
            break;
        }
    }
    harness.harness_mut().release_action(SHOOT);
    assert_eq!(seen, (0..=12).rev().collect::<Vec<u32>>(), "a round a shot");
    assert_eq!(shots, 12);
    // Held through the empty magazine, it fires nothing; then every round
    // comes back at once, 1.2 s after the last.
    let mut waited = 0;
    let mut reloads = 0;
    while rounds(&harness).0 == 0 && waited < 200 {
        harness.step(1);
        waited += steps_this_frame(&harness);
        reloads += sounds(&harness, Sound::Reloaded);
        assert!(matches!(rounds(&harness).0, 0 | 12), "all at once");
    }
    assert_eq!(rounds(&harness), (12, 12));
    assert!((70..=74).contains(&waited), "1.2 s: {waited} steps");
    assert_eq!(reloads, 1, "the reload's sound");
}

#[test]
fn a_shot_hits_the_dummy_for_low_physical_and_can_crit() {
    let (mut harness, _) = sky();
    let dummy = dummy_at(&mut harness, 900.0);
    harness.step(2);
    let aim = body(&harness, dummy);
    for (seed, amount, crit) in [
        (PLAIN_SEED, SHOT_DAMAGE, false),
        (CRIT_SEED, 2.0 * SHOT_DAMAGE, true),
    ] {
        let id = shot(&mut harness, Vec2::new(700.0, aim.y), Vec2::X * SPEED);
        harness.world_mut().insert_resource(KitRng::seeded(seed));
        let mut landed = Vec::new();
        for _ in 0..12 {
            harness.step(1);
            landed.extend(parts(&harness, dummy));
            if bullet(&harness, id).is_none() {
                break;
            }
        }
        assert_eq!(landed, [(amount, crit)], "seed {seed}");
        assert!(bullet(&harness, id).is_none(), "the hit ends the shot");
    }
    assert!(velocity(&harness, dummy).x > 0.0, "pushed along the shot");
    assert!(health(&harness, dummy) < 400.0);
}

#[test]
fn a_shot_pushes_a_ball() {
    let (mut harness, _) = sky();
    let small = ball(&mut harness, Vec2::new(900.0, TOP - 7.5), true);
    let bowling = ball(&mut harness, Vec2::new(1300.0, TOP - 15.0), false);
    let frozen = ball(&mut harness, Vec2::new(1700.0, TOP - 7.5), true);
    example_01_platformer::features::elements::freeze_iron(harness.world_mut(), frozen);
    harness.step(10);
    for target in [small, bowling] {
        let at = body(&harness, target);
        let id = shot(&mut harness, at - Vec2::new(150.0, 0.0), Vec2::X * SPEED);
        let before = velocity(&harness, target);
        let inverse = harness.world().get::<RigidBody>(target).unwrap().inv_mass;
        for _ in 0..10 {
            harness.step(1);
            if bullet(&harness, id).is_none() {
                break;
            }
        }
        assert!(bullet(&harness, id).is_none(), "the ball ends the shot");
        let kicked = velocity(&harness, target) - before;
        let expected = BALL_KICK * inverse;
        assert!(
            (kicked.x - expected).abs() <= 0.02 * expected,
            "{kicked} for {expected} along the shot"
        );
    }
    // A frozen ball shatters.
    let at = body(&harness, frozen);
    shot(&mut harness, at - Vec2::new(150.0, 0.0), Vec2::X * SPEED);
    harness.step(10);
    assert!(!harness.world().is_alive(frozen), "shattered");
}

#[test]
fn a_shot_bounces_off_iron() {
    let (mut harness, _) = sky();
    let iron = toys::place_iron(harness.world_mut(), Vec2::new(1000.0, IRON_Y));
    harness.step(10);
    let id = shot(&mut harness, Vec2::new(700.0, IRON_Y), Vec2::X * SPEED);
    let mut turned = None;
    for _ in 0..12 {
        harness.step(1);
        let Some(entity) = bullet(&harness, id) else {
            break;
        };
        if harness.world().get::<Bullet>(entity).unwrap().ricochets == 1 {
            turned = Some(entity);
            break;
        }
    }
    let entity = turned.expect("it turned");
    let world = harness.world();
    let flight = world
        .get::<tungsten::kit::LinearMotion>(entity)
        .unwrap()
        .velocity();
    assert!(
        (flight - Vec2::new(-SPEED, 0.0)).length() < 1e-2,
        "{flight}"
    );
    assert!(bursts(&harness, "ex10_ricochet") >= 1, "sparks");
    assert_eq!(sounds(&harness, Sound::Ricocheted), 1);
    assert!(velocity(&harness, iron).length() < 1.0, "the iron stays");
    // Between two blocks it turns twice, then the third face ends it.
    let (mut harness, _) = sky();
    for x in [600.0, 1200.0] {
        toys::place_iron(harness.world_mut(), Vec2::new(x, IRON_Y));
    }
    harness.step(10);
    let id = shot(&mut harness, Vec2::new(900.0, IRON_Y), Vec2::X * SPEED);
    let mut most = 0;
    for _ in 0..60 {
        harness.step(1);
        let Some(entity) = bullet(&harness, id) else {
            break;
        };
        most = most.max(harness.world().get::<Bullet>(entity).unwrap().ricochets);
    }
    assert_eq!(most, MAX_RICOCHETS);
    assert!(bullet(&harness, id).is_none(), "the third face ends it");
}

#[test]
fn a_ball_or_iron_behind_a_nearer_dummy_or_wall_is_untouched() {
    let (mut harness, _) = sky();
    // A small ball against the dummy's back, both within the shot's step.
    let dummy = dummy_at(&mut harness, 900.0);
    let behind = ball(&mut harness, Vec2::new(923.0, TOP - 7.5), true);
    // A thin wall, then iron against it.
    floor(
        &mut harness,
        Vec2::new(1500.0, IRON_Y),
        Vec2::new(4.0, 60.0),
    );
    let iron = toys::place_iron(harness.world_mut(), Vec2::new(1565.0, IRON_Y));
    harness.step(10);
    let id = shot(&mut harness, Vec2::new(880.0, TOP - 7.5), Vec2::X * SPEED);
    harness.step(1);
    assert!(bullet(&harness, id).is_none());
    assert!(health(&harness, dummy) < 400.0, "the dummy took the shot");
    assert!(
        velocity(&harness, behind).length() < 1.0,
        "the ball behind it did not"
    );
    let id = shot(&mut harness, Vec2::new(1480.0, IRON_Y), Vec2::X * SPEED);
    harness.step(1);
    assert!(bullet(&harness, id).is_none(), "the wall ends it");
    assert_eq!(bursts(&harness, "ex10_ricochet"), 0, "no turn off the iron");
    assert!(velocity(&harness, iron).length() < 1.0);
}

#[test]
fn iron_in_front_of_a_dummy_ends_the_shot_at_the_iron() {
    let (mut harness, _) = sky();
    toys::place_iron(harness.world_mut(), Vec2::new(1000.0, IRON_Y));
    let dummy = dummy_at(&mut harness, 1090.0);
    harness.step(10);
    let y = body(&harness, dummy).y;
    // A shot fast enough to cross the block in one step, so the kit's pass,
    // which sees no iron, reaches the dummy behind it: the iron comes first.
    let id = shot(&mut harness, Vec2::new(900.0, y), Vec2::X * 12_000.0);
    harness.step(2);
    assert!(bullet(&harness, id).is_none());
    assert_eq!(health(&harness, dummy), 400.0, "the iron took it");
    assert_eq!(bursts(&harness, "ex10_ricochet"), 0, "it ended, not turned");
    // At the gun's speed the shot turns off the iron's face instead.
    let id = shot(&mut harness, Vec2::new(900.0, y), Vec2::X * SPEED);
    harness.step(3);
    let entity = bullet(&harness, id).expect("turned back");
    assert_eq!(harness.world().get::<Bullet>(entity).unwrap().ricochets, 1);
    harness.step(30);
    assert_eq!(health(&harness, dummy), 400.0);
}

#[test]
fn a_moving_ball_crossing_before_a_wall_stop_is_pushed_and_after_it_is_not() {
    let (mut harness, _) = sky();
    // A thin wall whose face the shot's step meets at 0.6 of the step.
    floor(
        &mut harness,
        Vec2::new(1500.0, IRON_Y),
        Vec2::new(4.0, 60.0),
    );
    harness.step(1);
    let line = 950.0;
    // Before the stop: a ball falling across the shot's line in front of
    // the wall meets the shot early in the step.
    let id = shot(&mut harness, Vec2::new(1470.0, line), Vec2::X * SPEED);
    let early = ball(&mut harness, Vec2::new(1480.0, line - 5.0), true);
    kick(&mut harness, early, Vec2::new(0.0, 1200.0));
    harness.step(1);
    assert!(bullet(&harness, id).is_none());
    assert!(velocity(&harness, early).x > 0.5 * BALL_KICK, "kicked");
    // After it: one falling behind the wall would meet the shot's whole
    // step only past the wall, where the shot no longer is.
    let id = shot(&mut harness, Vec2::new(1470.0, line), Vec2::X * SPEED);
    let late = ball(&mut harness, Vec2::new(1515.0, line - 20.0), true);
    kick(&mut harness, late, Vec2::new(0.0, 1200.0));
    harness.step(1);
    assert!(bullet(&harness, id).is_none(), "the wall ended it");
    assert!(velocity(&harness, late).x.abs() < 1.0, "untouched");
}

/// Holds `shoot` until the magazine holds `left` rounds, a few seconds at
/// most.
fn hold_until(harness: &mut UiHarness, left: u32) {
    harness.harness_mut().press_action(SHOOT);
    for _ in 0..240 {
        if rounds(harness).0 <= left {
            break;
        }
        harness.step(1);
    }
    harness.harness_mut().release_action(SHOOT);
    assert_eq!(rounds(harness).0, left);
}

#[test]
fn the_magazine_shows_its_rounds_and_reloading() {
    let (mut harness, _) = sky();
    aim_at(&mut harness, Vec2::new(700.0, 500.0));
    harness.step(1);
    let hud = *harness.world().resource::<Hud>();
    let colors = harness.tree().theme().tokens.colors;
    let lit = |harness: &UiHarness| {
        hud.magazine
            .pips
            .iter()
            .filter(|&&pip| {
                harness
                    .tree()
                    .overrides(pip)
                    .and_then(|style| style.background)
                    == Some(colors.text)
            })
            .count()
    };
    assert_eq!(text_of(harness.tree(), hud.magazine.text), "12 / 12");
    assert_eq!(lit(&harness), 12);
    hold_until(&mut harness, 9);
    harness.step(1);
    assert_eq!(text_of(harness.tree(), hud.magazine.text), "9 / 12");
    assert_eq!(lit(&harness), 9);
    hold_until(&mut harness, 0);
    harness.step(1);
    assert_eq!(text_of(harness.tree(), hud.magazine.text), "reloading");
    assert_eq!(lit(&harness), 0);
    harness.step(80);
    assert_eq!(text_of(harness.tree(), hud.magazine.text), "12 / 12");
    assert_eq!(lit(&harness), 12);
}

/// What a shot met on the map over up to a second: its wall meetings in
/// frame order, each "bounced", "passed" or "expired", with the wall's
/// layers (none for an end without a wall) and normal.
fn wall_meetings(harness: &mut UiHarness, shot: Entity) -> Vec<(&'static str, u32, Vec2)> {
    let mut met = Vec::new();
    for _ in 0..60 {
        harness.step(1);
        let events = harness.harness();
        met.extend(
            events
                .events::<ProjectileBounced>()
                .filter(|event| event.entity == shot)
                .map(|event| ("bounced", event.wall.layers, event.wall.normal)),
        );
        met.extend(
            events
                .events::<ProjectilePassed>()
                .filter(|event| event.entity == shot)
                .map(|event| ("passed", event.wall.layers, event.wall.normal)),
        );
        let ended: Vec<_> = events
            .events::<ProjectileExpired>()
            .filter(|event| event.entity == shot)
            .map(|event| {
                let wall = event.wall.map_or((0, Vec2::ZERO), |w| (w.layers, w.normal));
                ("expired", wall.0, wall.1)
            })
            .collect();
        let done = !ended.is_empty();
        met.extend(ended);
        if done {
            break;
        }
    }
    met
}

#[test]
fn a_shot_turns_off_the_mines_iron_and_passes_the_bridges_wood_to_end_at_stone() {
    let (mut harness, _) = run();
    calm(&mut harness);
    // At the cut's second floor's riser, its west face clad in iron.
    let face = Vec2::new(306.0 * TILE, 54.5 * TILE);
    let id = shot(&mut harness, face - Vec2::new(300.0, 0.0), Vec2::X * SPEED);
    let iron = bullet(&harness, id).expect("it flies");
    let met = wall_meetings(&mut harness, iron);
    assert_eq!(
        met.first().copied(),
        Some(("bounced", 1 << 2, Vec2::NEG_X)),
        "it turned off the iron: {met:?}"
    );
    let turned = harness
        .world()
        .get::<LinearMotion>(iron)
        .map(LinearMotion::velocity);
    assert!(
        turned.is_none() || turned.is_some_and(|v| v.x < 0.0),
        "flying back west: {turned:?}"
    );
    // Down through the rope bridge's west deck over the pit to the stone
    // shelf under it.
    let id = shot(
        &mut harness,
        Vec2::new(56.0 * TILE, 34.0 * TILE),
        Vec2::Y * SPEED,
    );
    let down = bullet(&harness, id).expect("it flies");
    let met = wall_meetings(&mut harness, down);
    assert_eq!(
        met,
        [
            ("passed", 1 << 3, Vec2::NEG_Y),
            ("expired", 1 << 1, Vec2::NEG_Y)
        ],
        "it passed the wood and ended at the stone"
    );
}
