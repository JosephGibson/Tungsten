//! Lightning: the bolt stops at the first solid on its way down, and meets
//! an enemy across its art; it falls on the nearest rod; its chain jumps one
//! conductor at a time, every enemy one, weaker each jump, at most six and
//! never past ice or what takes no shock; a strike lights nothing; and it
//! shows its bolt, its veil for one frame, its light and its shake, and
//! says what it met: a body snaps, the ground and a deck crackle, the
//! ground keeps a scorch; a jump's arc crackles for its life.

use std::collections::BTreeSet;

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::combat::{BURNING, has_status};
use example_01_platformer::features::elements::cascade::{CascadeKind, Cascades, Entry};
use example_01_platformer::features::elements::components::{BallBurn, Frozen};
use example_01_platformer::features::elements::{self};
use example_01_platformer::features::enemies::components::{EnemyKind, EnemyState};
use example_01_platformer::features::enemies::{self};
use example_01_platformer::features::level::TILE;
use example_01_platformer::features::level::draw::view;
use example_01_platformer::features::lightning::components::{Arc, Bolt, Scorch, StrikeLight};
use example_01_platformer::features::lightning::{
    self, ARC_SECS, JUMP_KEEP, MAX_JUMPS, Met, SCORCH_ANCHOR, SCORCH_CAP, STRIKE_LIGHT,
    STRIKE_SHOCK, draw,
};
use example_01_platformer::features::player::PLAYER_HALF;
use example_01_platformer::features::toys::{self, IRON_HALF};
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shared::{SaveId, SaveIds};
use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, Bundle, CameraController, Collider, Entity, Light, Position, RigidBodyBundle,
    Sprite, TextureHandle, Transform,
};
use tungsten::kit::{Health, Resist, StateMachine};
use tungsten::testing::UiHarness;

use super::combat::{TOP, dummy_at, health};
use super::{
    aim_at, ball, body, bursts, centre, choose, floor, ground, mana, mock_sprite, pin_camera, run,
};
use super::{stand, steps_this_frame};

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

/// `count` iron blocks on the sky's ground, 200 px apart from x 600: each
/// within a jump of the next and out of reach of the one after.
fn irons(harness: &mut UiHarness, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|i| {
            let at = Vec2::new(600.0 + 200.0 * i as f32, IRON_Y);
            toys::place_iron(harness.world_mut(), at)
        })
        .collect()
}

fn id(harness: &UiHarness, entity: Entity) -> u64 {
    harness.world().get::<SaveId>(entity).expect("a save ID").0
}

fn ids(harness: &UiHarness, entities: &[Entity]) -> Vec<Option<u64>> {
    entities
        .iter()
        .map(|&entity| Some(id(harness, entity)))
        .collect()
}

/// The jumps waiting now.
fn jumps(harness: &UiHarness) -> Vec<Entry> {
    harness
        .world()
        .resource::<Cascades>()
        .entries
        .iter()
        .filter(|entry| entry.kind == CascadeKind::Jump)
        .cloned()
        .collect()
}

/// Every jump waiting now or queued over the next `frames` frames, each
/// once, in the order they were queued, and the most that waited at once.
fn conduction(harness: &mut UiHarness, frames: usize) -> (Vec<Entry>, usize) {
    let mut seen = jumps(harness);
    let mut most = seen.len();
    for _ in 0..frames {
        harness.step(1);
        let waiting = jumps(harness);
        most = most.max(waiting.len());
        for jump in waiting {
            if !seen
                .iter()
                .any(|known| known.strike == jump.strike && known.target == jump.target)
            {
                seen.push(jump);
            }
        }
    }
    (seen, most)
}

fn receivers(chain: &[Entry]) -> Vec<Option<u64>> {
    chain.iter().map(|jump| jump.target).collect()
}

/// Strikes from `player` on `target`'s body centre, between two frames.
fn strike_on(harness: &mut UiHarness, player: Entity, target: Entity) -> Vec2 {
    let aim = body(harness, target);
    lightning::strike(harness.world_mut(), player, aim)
}

/// The strike's bolt.
fn bolt(harness: &UiHarness) -> Bolt {
    harness
        .world()
        .query::<&Bolt>()
        .next()
        .cloned()
        .expect("a bolt")
}

/// Selects slot 4 and casts it at `at` with one press.
fn cast_lightning(harness: &mut UiHarness, at: Vec2) {
    aim_at(harness, at);
    choose(harness, 3);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    assert!(steps_this_frame(harness) > 0, "the press met a fixed step");
}

#[test]
fn a_strike_under_a_ceiling_hits_the_ceiling() {
    let (mut harness, player) = sky();
    // A ceiling over a dummy: the bolt falls at the dummy's x and stops on
    // the ceiling's top, so it hurts nothing and starts no chain.
    floor(
        &mut harness,
        Vec2::new(1000.0, 700.0),
        Vec2::new(120.0, 16.0),
    );
    let dummy = dummy_at(&mut harness, 1000.0);
    harness.step(2);
    let aim = body(&harness, dummy);
    let landing = lightning::strike(harness.world_mut(), player, aim);
    assert_eq!(landing, Vec2::new(aim.x, 684.0), "the ceiling's top");
    harness.step(4);
    assert_eq!(health(&harness, dummy), 400.0, "unhurt under it");
    assert!(jumps(&harness).is_empty(), "no chain from an unstruck rod");
    // The map's tiles stop it too: aimed under the ground the courier
    // stands on, it lands on that ground's top or above it.
    let (mut harness, player) = run();
    harness.step(30);
    let feet = centre(&harness, player) + Vec2::Y * PLAYER_HALF.y;
    let aim = feet + Vec2::Y * 3.0 * TILE;
    let landing = lightning::bolt_end(harness.world(), aim, Some(player)).at;
    assert!(
        landing.y <= feet.y + 1.0 && (landing.x - aim.x).abs() < 1e-3,
        "{landing} over the feet at {feet}"
    );
}

#[test]
fn a_bolt_falls_to_the_first_body_under_the_cursor() {
    let (mut harness, player) = sky();
    let near = toys::place_iron(harness.world_mut(), Vec2::new(900.0, IRON_Y));
    let far = toys::place_iron(harness.world_mut(), Vec2::new(1150.0, IRON_Y));
    harness.step(2);
    // Aimed over the near block, the bolt keeps the cursor's x and falls
    // past the cursor onto the block, the first body under it, and the
    // chain starts there.
    let aim = Vec2::new(body(&harness, near).x, IRON_Y - 300.0);
    cast_lightning(&mut harness, aim);
    assert_eq!(mana(&harness), 85.0, "a strike costs 15");
    let landing = bolt(&harness).landing;
    assert!(
        (landing.x - aim.x).abs() < 1e-3,
        "the cursor's x: {landing}"
    );
    assert!(
        landing.y > aim.y && landing.y < body(&harness, near).y,
        "past the cursor, on the block: {landing}"
    );
    let waiting = jumps(&harness);
    assert_eq!(receivers(&waiting), ids(&harness, &[far]));
    assert_eq!(waiting[0].chain, [id(&harness, near)]);
    let step = harness.world().resource::<Cascades>().step;
    assert_eq!(waiting[0].due, step + 1);
    // Aimed over open ground, it falls to the ground under the cursor.
    let open = Vec2::new(400.0, TOP - 200.0);
    let landed = lightning::strike(harness.world_mut(), player, open);
    assert_eq!(landed, Vec2::new(open.x, TOP), "the ground's top");
}

#[test]
fn conduction_loses_strength_each_jump_and_stops_at_six() {
    let (mut harness, player) = sky();
    let line = irons(&mut harness, 8);
    harness.step(2);
    strike_on(&mut harness, player, line[0]);
    let (chain, most) = conduction(&mut harness, 20);
    assert_eq!(
        receivers(&chain),
        ids(&harness, &line[1..7]),
        "six jumps down the line"
    );
    assert_eq!(chain.len() as u32, MAX_JUMPS);
    assert_eq!(most, 1, "a jump at a time");
    for (k, jump) in chain.iter().enumerate() {
        assert_eq!(jump.jumps, k as u32 + 1);
        let strength = STRIKE_SHOCK * JUMP_KEEP.powi(k as i32 + 1);
        assert!(
            (jump.strength - strength).abs() < 1e-3,
            "jump {}: {} for {strength}",
            k + 1,
            jump.strength
        );
    }
    assert!(jumps(&harness).is_empty());
}

#[test]
fn conduction_stops_at_ice() {
    let (mut harness, player) = sky();
    let line = irons(&mut harness, 6);
    harness.step(2);
    elements::freeze_iron(harness.world_mut(), line[3]);
    strike_on(&mut harness, player, line[0]);
    let (chain, _) = conduction(&mut harness, 20);
    assert_eq!(
        receivers(&chain),
        ids(&harness, &line[1..4]),
        "the ice takes its jump and passes none on"
    );
    let world = harness.world();
    assert!(
        world.is_alive(line[3]) && world.has::<Frozen>(line[3]),
        "shock starts no ending"
    );
    // A frozen start passes nothing on either.
    elements::freeze_iron(harness.world_mut(), line[5]);
    strike_on(&mut harness, player, line[5]);
    let (chain, _) = conduction(&mut harness, 5);
    assert!(chain.is_empty(), "{chain:?}");
}

#[test]
fn a_receiver_frozen_before_its_jump_ends_the_chain() {
    let (mut harness, player) = sky();
    let line = irons(&mut harness, 4);
    harness.step(2);
    strike_on(&mut harness, player, line[0]);
    // The jump was decided when it was queued, to a receiver not yet ice.
    assert_eq!(receivers(&jumps(&harness)), ids(&harness, &line[1..2]));
    elements::freeze_iron(harness.world_mut(), line[1]);
    let (chain, _) = conduction(&mut harness, 10);
    assert_eq!(receivers(&chain), ids(&harness, &line[1..2]));
}

#[test]
fn two_struck_conductors_start_one_chain_of_at_most_six_jumps() {
    let (mut harness, player) = sky();
    // Two dummies within the strike's reach, then a line of iron.
    let first = dummy_at(&mut harness, 1000.0);
    let second = dummy_at(&mut harness, 1040.0);
    let line: Vec<Entity> = (0..7)
        .map(|i| {
            let at = Vec2::new(1200.0 + 180.0 * i as f32, IRON_Y);
            toys::place_iron(harness.world_mut(), at)
        })
        .collect();
    harness.step(2);
    strike_on(&mut harness, player, first);
    let members = [id(&harness, first), id(&harness, second)];
    let waiting = jumps(&harness);
    assert_eq!(waiting.len(), 1, "one chain");
    assert_eq!(waiting[0].chain, members, "both struck are its members");
    let (chain, most) = conduction(&mut harness, 20);
    assert_eq!(most, 1, "one path");
    assert_eq!(receivers(&chain), ids(&harness, &line[..6]));
    let distinct: BTreeSet<Option<u64>> = receivers(&chain).into_iter().collect();
    assert_eq!(distinct.len(), chain.len(), "none returns");
    assert!(
        chain
            .iter()
            .all(|jump| jump.strike == waiting[0].strike && jump.jumps <= MAX_JUMPS)
    );
    assert!(health(&harness, first) < 400.0 && health(&harness, second) < 400.0);
}

#[test]
fn a_strike_lights_nothing() {
    let (mut harness, player) = sky();
    let dummy = dummy_at(&mut harness, 1000.0);
    let balls: Vec<Entity> = [950.0, 966.0, 1034.0, 1050.0]
        .into_iter()
        .map(|x| ball(&mut harness, Vec2::new(x, TOP - 8.0), true))
        .collect();
    harness.step(2);
    strike_on(&mut harness, player, dummy);
    harness.step(30);
    let world = harness.world();
    assert!(health(&harness, dummy) < 400.0, "the shock landed");
    assert!(!has_status(world, dummy, BURNING), "shock never burns");
    assert!(
        balls
            .iter()
            .all(|&ball| world.get::<BallBurn>(ball).is_none()),
        "no ball lit"
    );
}

/// The instances drawn from the textures `handles`.
fn drawn(batches: &[tungsten::render::SpriteBatch], handles: &[u32]) -> usize {
    batches
        .iter()
        .filter(|batch| handles.iter().any(|&h| batch.texture == TextureHandle(h)))
        .map(|batch| batch.instances.len())
        .sum()
}

#[test]
fn a_strike_shows_its_bolt_flash_light_and_shake() {
    let (mut harness, _) = sky();
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    for (i, name) in draw::BOLT_PIECES.iter().enumerate() {
        mock_sprite(assets, name, 900 + i as u32, false);
    }
    mock_sprite(assets, draw::TRACER, 920, false);
    let line = irons(&mut harness, 2);
    harness.step(2);
    let shake = |harness: &UiHarness| harness.world().resource::<CameraController>().shake_trauma;
    assert_eq!(shake(&harness), 0.0);
    let aim = body(&harness, line[0]);
    cast_lightning(&mut harness, aim);
    let pieces: Vec<u32> = (900..908).collect();
    let world = harness.world();
    let strike = bolt(&harness);
    assert!((5..=8).contains(&strike.pieces.len()));
    let (min, max) = view(world);
    let veil = |batches: &[tungsten::render::SpriteBatch]| {
        batches
            .iter()
            .flat_map(|batch| &batch.instances)
            .filter(|quad| quad.size == (max - min).to_array())
            .count()
    };
    let first = draw::bolts(world);
    assert_eq!(veil(&first), 1, "the veil over the view");
    assert_eq!(drawn(&first, &pieces), strike.pieces.len());
    let quads: Vec<_> = first
        .iter()
        .filter(|batch| pieces.iter().any(|&h| batch.texture == TextureHandle(h)))
        .flat_map(|batch| &batch.instances)
        .collect();
    let bottom = quads
        .iter()
        .map(|quad| quad.position[1] + quad.size[1])
        .fold(f32::MIN, f32::max);
    let top = quads
        .iter()
        .map(|quad| quad.position[1])
        .fold(f32::MAX, f32::min);
    assert!(
        (bottom - strike.landing.y).abs() < 1e-3,
        "down to the strike"
    );
    assert!(top <= min.y, "from above the view: {top} over {}", min.y);
    let glow = |harness: &UiHarness| {
        harness
            .world()
            .query::<(&StrikeLight, &Light, &Transform)>()
            .map(|(_, light, at)| (light.intensity, at.position))
            .next()
    };
    let (lit, at) = glow(&harness).expect("a light at the strike");
    assert!(lit > 0.0 && lit < STRIKE_LIGHT && at == strike.landing);
    assert!(shake(&harness) > 0.0, "the strike shakes the view");
    let thunder = |harness: &UiHarness, sound: Sound| {
        harness
            .harness()
            .events::<Sounded>()
            .filter(|sounded| sounded.sound == sound)
            .count()
    };
    assert_eq!(thunder(&harness, Sound::Thundered), 1);
    // The next frame: the veil is gone, the bolt stays, the light fades;
    // the chain's first jump crackles and draws its arc.
    harness.step(1);
    let second = draw::bolts(harness.world());
    assert_eq!(veil(&second), 0, "one frame of veil");
    assert_eq!(drawn(&second, &pieces), strike.pieces.len());
    assert!(glow(&harness).expect("still lit").0 < lit);
    let arcs: Vec<Arc> = harness.world().query::<&Arc>().copied().collect();
    assert_eq!(arcs.len(), 1, "the jump's arc");
    assert!(arcs[0].from.distance(body(&harness, line[0])) < 1.0);
    assert!(arcs[0].to.distance(body(&harness, line[1])) < 1.0);
    assert!(drawn(&second, &[920]) > 1, "the arc's quads");
    assert_eq!(thunder(&harness, Sound::Crackled), 1);
    harness.step(14);
    let world = harness.world();
    assert_eq!(world.query::<&Bolt>().count(), 0, "gone after 0.15 s");
    assert_eq!(world.query::<&Arc>().count(), 1, "the arc shows on");
    assert!(glow(&harness).is_none(), "the light faded out");
    harness.step(9);
    assert_eq!(
        harness.world().query::<&Arc>().count(),
        0,
        "gone after 0.35 s"
    );
}

/// The sounds sent this frame.
fn sounded(harness: &UiHarness) -> Vec<Sound> {
    harness
        .harness()
        .events::<Sounded>()
        .map(|sounded| sounded.sound)
        .collect()
}

/// Each scorch's corner and its alpha.
fn scorches(harness: &UiHarness) -> Vec<(Vec2, u8)> {
    harness
        .world()
        .query::<(&Scorch, &Transform, &Sprite)>()
        .map(|(_, at, sprite)| (at.position, sprite.color[3]))
        .collect()
}

/// Asserts the strike this frame sounded and sparked as the ground's.
fn struck_ground(harness: &UiHarness) {
    let sounds = sounded(harness);
    assert!(
        sounds.contains(&Sound::GroundStruck) && sounds.contains(&Sound::Thundered),
        "{sounds:?}"
    );
    assert!(!sounds.contains(&Sound::Snapped), "{sounds:?}");
    assert_eq!(bursts(harness, "ex10_ground_sparks"), 1);
    assert_eq!(bursts(harness, "ex10_strike_sparks"), 0);
}

#[test]
fn a_strike_on_the_ground_crackles_sparks_and_leaves_a_scorch_that_fades() {
    let (mut harness, player) = sky();
    let open = Vec2::new(400.0, TOP - 200.0);
    let landing = lightning::bolt_end(harness.world(), open, Some(player));
    assert_eq!(landing.met, Met::Ground);
    lightning::strike(harness.world_mut(), player, open);
    harness.step(1);
    struck_ground(&harness);
    let marks = scorches(&harness);
    assert_eq!(marks.len(), 1);
    assert_eq!(
        marks[0].0 + SCORCH_ANCHOR,
        landing.at,
        "its bottom row's middle"
    );
    harness.step(150);
    let half = scorches(&harness)[0].1;
    assert!((100..160).contains(&half), "half faded at 2.5 s: {half}");
    harness.step(155);
    assert!(scorches(&harness).is_empty(), "faded out by 5 s");
    for i in 0..8 {
        let aim = Vec2::new(300.0 + 120.0 * i as f32, TOP - 200.0);
        lightning::strike(harness.world_mut(), player, aim);
        harness.step(1);
    }
    assert_eq!(scorches(&harness).len(), SCORCH_CAP, "the oldest go");
}

#[test]
fn a_strike_on_an_enemy_or_iron_meets_a_body_and_snaps() {
    for case in ["a goblin", "iron"] {
        let (mut harness, player) = sky();
        let target = if case == "iron" {
            toys::place_iron(harness.world_mut(), Vec2::new(900.0, IRON_Y))
        } else {
            let half = EnemyKind::Goblin.info().half.y;
            still_enemy(
                &mut harness,
                EnemyKind::Goblin,
                Vec2::new(900.0, TOP - half - 0.5),
            )
        };
        harness.step(1);
        let aim = Vec2::new(900.0, TOP - 400.0);
        let landing = lightning::bolt_end(harness.world(), aim, Some(player));
        assert_eq!(landing.met, Met::Body(target), "{case}");
        lightning::strike(harness.world_mut(), player, aim);
        harness.step(1);
        let sounds = sounded(&harness);
        assert!(
            sounds.contains(&Sound::Snapped) && !sounds.contains(&Sound::GroundStruck),
            "{case}: {sounds:?}"
        );
        assert_eq!(bursts(&harness, "ex10_strike_sparks"), 1, "{case}");
        assert!(scorches(&harness).is_empty(), "{case}");
    }
}

#[test]
fn a_strike_on_a_deck_sparks_as_the_ground_does_and_leaves_no_scorch() {
    let (mut harness, player) = sky();
    let centre = Vec2::new(900.0, TOP - 300.0);
    let deck = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(Position(centre), Collider::aabb(Vec2::new(96.0, 12.0)))
            .with((SceneEntity { state_id: GAMEPLAY },)),
    );
    harness.step(1);
    let aim = centre - Vec2::Y * 100.0;
    let landing = lightning::bolt_end(harness.world(), aim, Some(player));
    assert_eq!(landing.met, Met::Deck(deck));
    assert!(
        (landing.at.y - (centre.y - 12.0)).abs() < 1e-3,
        "{landing:?}"
    );
    lightning::strike(harness.world_mut(), player, aim);
    harness.step(1);
    struck_ground(&harness);
    assert!(
        scorches(&harness).is_empty(),
        "a mark would hang in the air"
    );
}

#[test]
fn a_jumps_arc_crackles_on_new_paths_and_goes_after_its_life() {
    let (mut harness, player) = sky();
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    mock_sprite(assets, draw::TRACER, 920, false);
    let line = irons(&mut harness, 2);
    harness.step(2);
    strike_on(&mut harness, player, line[0]);
    let (min, max) = view(harness.world());
    // The arc's quads: the tracer's, the veil's aside.
    let path = |harness: &UiHarness| -> Vec<[f32; 2]> {
        draw::bolts(harness.world())
            .iter()
            .filter(|batch| batch.texture == TextureHandle(920))
            .flat_map(|batch| &batch.instances)
            .filter(|quad| quad.size != (max - min).to_array())
            .map(|quad| quad.position)
            .collect()
    };
    let age = |harness: &UiHarness| harness.world().query::<&Arc>().map(|arc| arc.age).next();
    for _ in 0..3 {
        harness.step(1);
        if age(&harness).is_some() {
            break;
        }
    }
    assert_eq!(age(&harness), Some(0.0), "the jump's arc");
    let first = path(&harness);
    assert!(!first.is_empty());
    for _ in 0..30 {
        if age(&harness).is_some_and(|age| age >= 0.3) {
            break;
        }
        harness.step(1);
    }
    let late = age(&harness).expect("shows at 0.3 s");
    assert!((0.3..ARC_SECS).contains(&late), "{late}");
    let later = path(&harness);
    assert!(!later.is_empty() && later != first, "a new path");
    harness.step(4);
    assert!(age(&harness).is_none(), "gone after 0.35 s");
}

/// An enemy of `kind`, its mind taken away, its body's centre at `centre`.
fn still_enemy(harness: &mut UiHarness, kind: EnemyKind, centre: Vec2) -> Entity {
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let enemy = enemies::spawn(world, kind, centre - kind.info().centre, id).expect("its prefab");
    world.remove_component::<StateMachine<EnemyState>>(enemy);
    enemy
}

#[test]
fn a_bolt_through_an_enemy_s_art_meets_it() {
    // Q49: each kind, a bolt a hair inside its art's opaque edge on either
    // side, past its body's: a walker on the sky's ground, a flyer 200 px
    // over it, which the bolt passed for the ground before.
    let mut missed = Vec::new();
    for kind in EnemyKind::ALL {
        let info = kind.info();
        assert!(info.strike > info.half.x, "{kind:?}'s art is wider");
        for side in [-1.0, 1.0] {
            let (mut harness, player) = sky();
            let centre = if info.flyer {
                Vec2::new(900.0, TOP - 200.0)
            } else {
                Vec2::new(900.0, TOP - info.half.y - 0.5)
            };
            let enemy = still_enemy(&mut harness, kind, centre);
            harness.step(1);
            let before = health(&harness, enemy);
            let at = Vec2::new(centre.x + side * (info.strike - 2.0), TOP - 400.0);
            let landing = lightning::strike(harness.world_mut(), player, at);
            harness.step(2);
            // A bat dies of it.
            let hurt = harness
                .world()
                .get::<Health>(enemy)
                .is_none_or(|health| health.current() < before);
            if !hurt {
                missed.push(format!(
                    "{kind:?} on its {side} side, the bolt at {landing}"
                ));
            }
        }
    }
    assert!(missed.is_empty(), "missed: {missed:#?}");
}

#[test]
fn every_enemy_conducts_and_one_that_takes_no_shock_ends_the_chain() {
    // Q48: iron, then a goblin, a crawler, a wisp that takes no shock and a
    // goblin past it, each within a jump of the next.
    let (mut harness, player) = sky();
    let rod = toys::place_iron(harness.world_mut(), Vec2::new(600.0, IRON_Y));
    let goblin = EnemyKind::Goblin.info().half.y;
    let crawler = EnemyKind::Crawler.info().half.y;
    let first = still_enemy(
        &mut harness,
        EnemyKind::Goblin,
        Vec2::new(800.0, TOP - goblin - 0.5),
    );
    let second = still_enemy(
        &mut harness,
        EnemyKind::Crawler,
        Vec2::new(1000.0, TOP - crawler - 0.5),
    );
    let proof = still_enemy(&mut harness, EnemyKind::Wisp, Vec2::new(1200.0, TOP - 60.0));
    let past = still_enemy(
        &mut harness,
        EnemyKind::Goblin,
        Vec2::new(1400.0, TOP - goblin - 0.5),
    );
    let resist = Resist::default().with_factor("damage.shock", 0.0);
    harness.world_mut().insert(proof, resist);
    harness.step(2);
    assert!(lightning::shock_proof(harness.world(), proof));
    assert!(!lightning::shock_proof(harness.world(), first));
    strike_on(&mut harness, player, rod);
    let (chain, _) = conduction(&mut harness, 20);
    assert_eq!(
        receivers(&chain),
        ids(&harness, &[first, second, proof]),
        "the wisp takes its jump and passes none on"
    );
    assert!(
        !ids(&harness, &[past])
            .iter()
            .any(|id| receivers(&chain).contains(id))
    );
}
