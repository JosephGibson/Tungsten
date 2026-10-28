//! The enemies: line of sight and its walls, the alert passed to the near
//! ones, dormancy, the difficulty's factor; the wisp's chase, search and
//! return, its fire immunity, the fire that draws and heals it, its freeze,
//! its burning touch and its light; the crawler's ledge, the lantern's light
//! it keeps out of, its resistances and bleeding, its chilling touch and its
//! lunge after a tell, a freeze that holds the tell; the beetle's shell,
//! whose front turns a bullet and whose rear takes it whole, its paw and
//! its charge, a stun that ends the paw, its resistances and conduction,
//! the brittle shatter and the iron crush; the bat's roost, its flash, dive
//! and climb, its fall and shatter when frozen, its burning and bleeding;
//! the troll's healing that burning holds, its slam, its throws credited
//! and scaled at a finite mass and kept through a load, its hold through a
//! load and dropped at its death or eviction, the knockback, pull and
//! impacts that barely move it, its high cold and shock rungs and its bar;
//! the drops, an orb's heal or mana, the enemies it passes and its fading;
//! the combat layer; a kill between a machine's `go` and its next visit; a
//! map enemy killed for good; the summon slot's enemies; the frozen cube
//! fitted to each body; the pushes that throw them by their pull, the
//! troll standing, and the level's edge that takes them for good; and the
//! goblin's rocks and bombs on their arcs, its distance, and a rock that
//! outlives its thrower.

use example_01_platformer::features::audio::components::Music;
use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::audio::{COMBAT_IN, COMBAT_OUT, MUSIC_COMBAT};
use example_01_platformer::features::combat::draw::overlays;
use example_01_platformer::features::combat::{FREEZE_NEEDS, Hit, Kind};
use example_01_platformer::features::dummy::{self, DUMMY_CENTRE};
use example_01_platformer::features::elements;
use example_01_platformer::features::elements::systems::crush;
use example_01_platformer::features::elements::{BODY_HIT_MOST, CRUSH_SPEED, body_hit};
use example_01_platformer::features::enemies::bat::{DIVE_SECS, DIVE_SPEED, FLASH_SECS};
use example_01_platformer::features::enemies::beetle::{CHARGE_SECS, CHARGE_SPEED, PAW_SECS};
use example_01_platformer::features::enemies::components::{
    Brain, Enemy, EnemyKind, EnemyState, Held, Shown, Sight, Strike, Throws,
};
use example_01_platformer::features::enemies::crawler::{
    self, FROST_BREATH, LUNGE_SPEED, SNOWBALL_COOLDOWN,
};
use example_01_platformer::features::enemies::drops::{
    self, ORB_CENTRE, ORB_HALF, ORB_SECS, Orb, OrbKind, PlacedOrbs,
};
use example_01_platformer::features::enemies::goblin::{
    BOMB_CHANCE, COOLDOWN, DASH_CHANCE, DASH_SPEED, KEEP_OFF, LEAP_BACK, ROCK_DAMAGE, STAB_DAMAGE,
};
use example_01_platformer::features::enemies::snowball::{self, SNOWBALL_COLD};
use example_01_platformer::features::enemies::{self, ARRIVED, DORMANT_PAST, SUMMON_CAP, troll};
use example_01_platformer::features::gun::components::Bullet;
use example_01_platformer::features::gun::{self, SHOT_DAMAGE};
use example_01_platformer::features::level::TILE;
use example_01_platformer::features::lightning;
use example_01_platformer::features::player::components::Lantern;
use example_01_platformer::features::props::components::{Bomb, Keg, PropKind, Rock};
use example_01_platformer::features::props::{self, CRATE_HALF};
use example_01_platformer::features::spells::components::Spell;
use example_01_platformer::features::spells::{SUMMON_NEXT, SUMMONS, Summoned};
use example_01_platformer::features::toys::IRON_HALF;
use example_01_platformer::features::toys::components::{PreviousVelocity, ScrapTumble};
use example_01_platformer::features::{player as courier, toys};
use example_01_platformer::game;
use example_01_platformer::save;
use example_01_platformer::shared::{SaveId, SaveIds, body_centre};
use example_01_platformer::shell::{self as game_shell, Difficulty, RunStart};
use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AnimationState, AssetRegistry, AudioCommand, Bundle, CameraState, Collider, Entity, EventQueue,
    Position, PrevPosition, RigidBodyBundle, Time, Velocity,
};
use tungsten::kit::{
    AilmentBuckets, AilmentThreshold, ContactDamage, Damaged, Health, Hitbox, KitRng, Knockback,
    PlatformerController, StateMachine, TopDownMover,
};
use tungsten::testing::UiHarness;

use super::combat::{PLAIN_SEED, TOP, apply, dummy_at};
use super::{
    aim_at, body, bursts, calm, choose, ground, harness, kick as kick_at, mock_sprite, pin_camera,
    player_entity, run, settle, stand, start_at, teleport, top,
};

/// The sky's ground with the map's enemies taken off, the player standing
/// at x 300 and the camera held over it.
fn arena() -> (UiHarness, Entity) {
    let (mut harness, player) = run();
    calm(&mut harness);
    ground(&mut harness, TOP);
    stand(&mut harness, player, 300.0, TOP);
    harness.step(1);
    pin_camera(&mut harness, Vec2::new(0.0, 400.0));
    (harness, player)
}

/// An enemy of `kind` with its body's centre at `centre`, at Normal, its
/// save ID the run's next.
fn enemy(harness: &mut UiHarness, kind: EnemyKind, centre: Vec2) -> Entity {
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let entity = enemies::spawn(world, kind, centre - kind.info().centre, id).expect("its prefab");
    enemies::scale(world, entity, Difficulty::Normal);
    entity
}

/// A walker of `kind` standing on a floor whose top is at `top`, its centre
/// at `x`.
fn walker(harness: &mut UiHarness, kind: EnemyKind, x: f32, top: f32) -> Entity {
    let half = kind.info().half;
    enemy(harness, kind, Vec2::new(x, top - half.y - 0.5))
}

fn state(harness: &UiHarness, enemy: Entity) -> EnemyState {
    harness
        .world()
        .get::<StateMachine<EnemyState>>(enemy)
        .expect("a machine")
        .state()
}

fn held(harness: &UiHarness, enemy: Entity) -> bool {
    harness
        .world()
        .get::<StateMachine<EnemyState>>(enemy)
        .is_some_and(StateMachine::is_held)
}

fn sees(harness: &UiHarness, enemy: Entity) -> bool {
    harness
        .world()
        .get::<Sight>(enemy)
        .is_some_and(|sight| sight.sees)
}

fn at(harness: &UiHarness, enemy: Entity) -> Vec2 {
    enemies::centre(harness.world(), enemy).expect("a body")
}

fn health(harness: &UiHarness, entity: Entity) -> f32 {
    harness.world().get::<Health>(entity).unwrap().current()
}

fn tagged(harness: &UiHarness, entity: Entity, tag: &str) -> bool {
    harness
        .world()
        .get::<tungsten::kit::Tags>(entity)
        .is_some_and(|tags| tags.has(tag))
}

/// Lands a hit of one `kind` part of `amount` on `target` at once.
fn land(harness: &mut UiHarness, target: Entity, kind: Kind, amount: f32) {
    let world = harness.world_mut();
    Hit::new(target, [(kind, amount)])
        .request(world)
        .apply(world);
}

/// Presses `action` until a fixed step has read it, then lets it go.
fn turn(harness: &mut UiHarness, action: &str) {
    harness.harness_mut().press_action(action);
    for _ in 0..4 {
        harness.step(1);
        if harness.world().resource::<Time>().fixed_steps_this_frame() > 0 {
            break;
        }
    }
    harness.harness_mut().release_action(action);
    harness.step(1);
}

/// A static box in the slabs' physics layer, which sight casts against.
fn slab(harness: &mut UiHarness, centre: Vec2, half: Vec2) -> Entity {
    let mut bundle = RigidBodyBundle::r#static(Position(centre), Collider::aabb(half));
    bundle.body.layers = enemies::SLAB_LAYER;
    harness.world_mut().spawn_with(bundle.with((SceneEntity {
        state_id: game::GAMEPLAY,
    },)))
}

/// Steps one frame at a time, at most `frames`, until `done`; whether it
/// came.
fn until(harness: &mut UiHarness, frames: u32, mut done: impl FnMut(&UiHarness) -> bool) -> bool {
    for _ in 0..frames {
        harness.step(1);
        if done(harness) {
            return true;
        }
    }
    false
}

/// The hits `target` took in the last frame, from `source` when it names
/// one: their amounts.
fn took(harness: &UiHarness, target: Entity, source: Option<Entity>) -> Vec<f32> {
    harness
        .harness()
        .events::<Damaged>()
        .filter(|hit| hit.entity == target && source.is_none_or(|from| hit.source == Some(from)))
        .map(|hit| hit.amount)
        .collect()
}

fn speed(harness: &UiHarness, entity: Entity) -> Vec2 {
    harness.world().get::<Velocity>(entity).expect("a body").0
}

/// Fires a bullet from the player with its centre at `centre` at the gun's
/// speed along `along`, landed at the frame's flush, and the dice set so
/// the shot does not crit.
fn shoot(harness: &mut UiHarness, centre: Vec2, along: Vec2) -> SaveId {
    let player = player_entity(harness);
    let id =
        gun::fire(harness.world_mut(), centre, along * 2400.0, Some(player)).expect("a bullet");
    harness.step(1);
    harness
        .world_mut()
        .insert_resource(KitRng::seeded(PLAIN_SEED));
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

/// A slab whose underside is at `underside`, about `x`: a ceiling a bat
/// roosts under once a step has indexed it.
fn ceiling(harness: &mut UiHarness, x: f32, underside: f32) -> Entity {
    slab(
        harness,
        Vec2::new(x, underside - 12.0),
        Vec2::new(160.0, 12.0),
    )
}

/// A bat under a ceiling at y 600 about x 1300, far from the arena's
/// player, hanging at its roost.
fn roosting() -> (UiHarness, Entity, Entity) {
    let (mut harness, player) = arena();
    ceiling(&mut harness, 1300.0, 600.0);
    harness.step(2);
    let bat = enemy(&mut harness, EnemyKind::Bat, Vec2::new(1300.0, 700.0));
    harness.step(120);
    (harness, player, bat)
}

fn roost(harness: &UiHarness, bat: Entity) -> Vec2 {
    harness.world().get::<Enemy>(bat).expect("an enemy").home()
}
/// A troll standing on the sky's ground with its centre at `x`, at
/// `difficulty`, its save ID the run's next.
fn troll_at(harness: &mut UiHarness, x: f32, difficulty: Difficulty) -> Entity {
    let kind = EnemyKind::Troll;
    let centre = Vec2::new(x, TOP - kind.info().half.y - 0.5);
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let troll = enemies::spawn(world, kind, centre - kind.info().centre, id).expect("its prefab");
    enemies::scale(world, troll, difficulty);
    troll
}

/// Takes `enemy`'s mind away, so it stands: no machine, no save.
fn still(harness: &mut UiHarness, enemy: Entity) {
    harness
        .world_mut()
        .remove_component::<StateMachine<EnemyState>>(enemy);
}

/// An iron block resting on the sky's ground with its centre at `x`.
fn iron_at(harness: &mut UiHarness, x: f32) -> Entity {
    toys::place_iron(harness.world_mut(), Vec2::new(x, TOP - IRON_HALF.y))
}

/// Every hit `target` takes over `frames` frames: its amount and source,
/// and the frame it came in.
fn hits_over(
    harness: &mut UiHarness,
    target: Entity,
    frames: u32,
) -> Vec<(f32, Option<Entity>, u32)> {
    let mut hits = Vec::new();
    for frame in 0..frames {
        harness.step(1);
        hits.extend(
            harness
                .harness()
                .events::<Damaged>()
                .filter(|hit| hit.entity == target)
                .map(|hit| (hit.amount, hit.source, frame)),
        );
    }
    hits
}

/// `troll` lifts `object` and throws it at the player at once.
fn lift_and_throw(harness: &mut UiHarness, troll: Entity, object: Entity) {
    let world = harness.world_mut();
    assert!(troll::pick_up(world, troll, object), "it lifts it");
    assert_eq!(troll::throw(world, troll), Some(object));
}

/// Steps until `target` is hit, at most `frames` frames, keeping the
/// `body`'s rule for the hit: the impact by the game's rule from the body's
/// speed before the hit's step, the player's speed before it, and the
/// centres after it, for a body of `mass`. The hit and that impact.
fn first_impact(
    harness: &mut UiHarness,
    body: Entity,
    target: Entity,
    mass: f32,
    frames: u32,
) -> Option<((f32, Option<Entity>), f32)> {
    for _ in 0..frames {
        let target_before = speed(harness, target);
        harness.step(1);
        let hit = harness
            .harness()
            .events::<Damaged>()
            .find(|hit| hit.entity == target)
            .map(|hit| (hit.amount, hit.source));
        if let Some(hit) = hit {
            let world = harness.world();
            let before = world
                .get::<PreviousVelocity>(body)
                .map_or_else(|| speed(harness, body), |velocity| velocity.0);
            let towards = (body_of(harness, target) - body_of(harness, body)).normalize_or_zero();
            let closing = (before - target_before).dot(towards);
            let rule = body_hit(mass, closing);
            return Some((hit, rule));
        }
    }
    None
}

fn body_of(harness: &UiHarness, entity: Entity) -> Vec2 {
    body(harness, entity)
}

/// Loads `saved` into a fresh game as Continue does, its clock held until
/// the test's ground and camera are back.
fn reload(saved: save::SaveGame) -> UiHarness {
    let mut fresh = harness();
    fresh.world_mut().resource_mut::<Time>().pause();
    game_shell::start_run(fresh.world_mut(), RunStart::from(saved), None);
    settle(&mut fresh);
    fresh.step(1);
    assert_eq!(top(&fresh), Some(game::GAMEPLAY));
    ground(&mut fresh, TOP);
    pin_camera(&mut fresh, Vec2::new(0.0, 400.0));
    fresh.world_mut().resource_mut::<Time>().resume();
    fresh
}

/// The entity of save ID `id`.
fn by_id(harness: &UiHarness, id: u64) -> Option<Entity> {
    harness
        .world()
        .query::<(Entity, &SaveId)>()
        .find(|&(_, own)| own.0 == id)
        .map(|(entity, _)| entity)
}

fn id_of(harness: &UiHarness, entity: Entity) -> u64 {
    harness.world().get::<SaveId>(entity).expect("a save ID").0
}

fn move_speed(harness: &UiHarness, entity: Entity) -> f32 {
    harness
        .world()
        .get::<tungsten::kit::Stats>(entity)
        .and_then(|stats| stats.get("move_speed"))
        .expect("a move speed")
}

#[test]
fn line_of_sight_is_blocked_by_a_wall() {
    // A tile wall: the clearing's stepped block, its lower step x 1024 to
    // 1280 at y 2176, its upper x 1280 to 1536 from y 2048; the player on
    // the lower step, one wisp past the upper, one over the clearing.
    let (mut harness, player) = run();
    calm(&mut harness);
    stand(&mut harness, player, 1150.0, 34.0 * 64.0);
    harness.step(1);
    let behind = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(1600.0, 2150.0));
    let open = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(700.0, 2150.0));
    for wisp in [behind, open] {
        let eye = enemies::systems::eye(harness.world(), wisp).unwrap();
        let target = super::centre(&harness, player);
        assert!(
            eye.distance(target) < EnemyKind::Wisp.info().sight,
            "in reach"
        );
    }
    harness.step(3);
    assert!(sees(&harness, open), "the clearing's air hides nothing");
    assert!(!sees(&harness, behind), "the pillar hides the player");
    // A body in the slabs' layer blocks it; one in any other layer does not.
    // Each wall a step ahead, so the physics step has indexed it.
    let (mut harness, _) = arena();
    slab(
        &mut harness,
        Vec2::new(500.0, 850.0),
        Vec2::new(10.0, 200.0),
    );
    harness.step(1);
    let walled = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(700.0, 900.0));
    let (mut other, _) = arena();
    super::floor(&mut other, Vec2::new(500.0, 850.0), Vec2::new(10.0, 200.0));
    other.step(1);
    let clear = enemy(&mut other, EnemyKind::Wisp, Vec2::new(700.0, 900.0));
    harness.step(3);
    other.step(3);
    assert!(!sees(&harness, walled), "a slab between");
    assert!(
        sees(&other, clear),
        "a still body outside the walls hides nothing"
    );
}

#[test]
fn crates_kegs_iron_scrap_dummies_and_the_frozen_hide_and_the_rest_do_not() {
    // Q47: each body alone on its own row of the sky, a level line along
    // the row through its centre, the seer elsewhere.
    let (mut harness, _) = arena();
    let seer = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(1800.0, 300.0));
    let row = |k: usize| Vec2::new(1000.0, 180.0 + 130.0 * k as f32);
    let world = harness.world_mut();
    let mut bodies: Vec<(&str, Entity, bool)> = vec![
        (
            "a crate",
            props::spawn(world, PropKind::Crate, row(0)),
            true,
        ),
        ("a keg", props::spawn(world, PropKind::Keg, row(1)), true),
        ("iron", toys::place_iron(world, row(2)), true),
        (
            "a scrap",
            toys::spawn_scrap(
                world,
                row(3),
                Vec2::ZERO,
                0,
                ScrapTumble {
                    born_at: 0.0,
                    initial_angle: 0.0,
                    total_turn: 0.0,
                },
            ),
            true,
        ),
        ("a dummy", dummy::spawn(world, row(4) - DUMMY_CENTRE), true),
        ("a small ball", toys::spawn_ball(world, row(5), true), false),
        (
            "a bowling ball",
            toys::spawn_ball(world, row(6), false),
            false,
        ),
        ("a rock", props::spawn(world, PropKind::Rock, row(7)), false),
    ];
    let frozen = enemy(
        &mut harness,
        EnemyKind::Goblin,
        Vec2::new(1000.0, 180.0 + 130.0 * 8.0),
    );
    let live = enemy(
        &mut harness,
        EnemyKind::Goblin,
        Vec2::new(1000.0, 180.0 + 130.0 * 9.0),
    );
    apply(&mut harness, frozen, "frozen", None);
    bodies.push(("a frozen goblin", frozen, true));
    bodies.push(("a live goblin", live, false));
    // Small balls before a crate: the ray passes them and meets it.
    let world = harness.world_mut();
    let behind = Vec2::new(1400.0, 180.0 + 130.0 * 10.0);
    for k in 0..3 {
        toys::spawn_ball(world, behind - Vec2::new(60.0 + 20.0 * k as f32, 0.0), true);
    }
    let crate_ = props::spawn(world, PropKind::Crate, behind);
    bodies.push(("small balls, then a crate", crate_, true));
    // Indexed, as a step leaves them.
    harness.step(1);
    let world = harness.world();
    for (name, body, hides) in bodies {
        let y = body_centre(world, body).expect("a body").y;
        let clear = enemies::systems::clear(world, seer, Vec2::new(700.0, y), Vec2::new(1500.0, y));
        assert_eq!(clear, !hides, "{name}");
    }
}

#[test]
fn a_spotting_enemy_alerts_those_within_reach() {
    let (mut harness, _) = arena();
    let spotter = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(650.0, 900.0));
    let near = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(850.0, 900.0));
    let far = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(1250.0, 900.0));
    let mut seen = [false; 3];
    for _ in 0..30 {
        harness.step(1);
        for (i, wisp) in [spotter, near, far].into_iter().enumerate() {
            seen[i] |= state(&harness, wisp) == EnemyState::Alert;
        }
    }
    assert!(sees(&harness, spotter) && !sees(&harness, near) && !sees(&harness, far));
    assert_eq!(seen, [true, true, false], "the spotter and its neighbour");
    assert_eq!(state(&harness, far), EnemyState::Patrol);
    let point = harness.world().get::<Brain>(near).unwrap();
    assert!(
        point.alert.or(point.last_seen).is_some(),
        "the alert gave it the point"
    );
}

#[test]
fn an_enemy_far_from_the_player_stops_thinking() {
    let (mut harness, player) = arena();
    let wisp = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(1650.0, 900.0));
    let start = at(&harness, wisp);
    harness.step(60);
    assert!(held(&harness, wisp), "1,350 px off: held");
    assert_eq!(at(&harness, wisp), start, "its mover at rest");
    let sight = *harness.world().get::<Sight>(wisp).unwrap();
    assert_eq!((sight.next, sight.sees), (0.0, false), "no sight check ran");
    // Near again, it wakes and sees.
    teleport(&mut harness, player, Vec2::new(1250.0, TOP - 28.5));
    assert!(until(&mut harness, 30, |h| sees(h, wisp)));
    assert!(!held(&harness, wisp));
}

#[test]
fn each_difficulty_scales_enemy_health_and_damage_alone() {
    for (difficulty, factor) in [
        (Difficulty::Easy, 0.75),
        (Difficulty::Normal, 1.0),
        (Difficulty::Hard, 1.5),
    ] {
        let mut harness = harness();
        start_at(&mut harness, difficulty);
        harness.step(2);
        let world = harness.world();
        let wisp = world
            .query::<(Entity, &Enemy)>()
            .find(|(_, enemy)| enemy.kind == EnemyKind::Wisp)
            .map(|(entity, _)| entity)
            .expect("the map's wisp");
        assert_eq!(world.get::<Health>(wisp).unwrap().max(), 40.0 * factor);
        assert_eq!(
            world.get::<Health>(wisp).unwrap().current(),
            40.0 * factor,
            "full"
        );
        assert_eq!(
            world.get::<ContactDamage>(wisp).unwrap().damage(),
            10.0 * factor
        );
        let threshold = world.get::<AilmentThreshold>(wisp).unwrap();
        assert_eq!(
            (threshold.flat(), threshold.of_max_health()),
            (16.0, 0.0),
            "its threshold unscaled"
        );
        let player = player_entity(&harness);
        let health = world.get::<Health>(player).unwrap();
        assert_eq!(
            (health.current(), health.max()),
            (courier::MAX_HEALTH, courier::MAX_HEALTH),
            "the player's own, the same at each"
        );
        let dummy = world
            .query::<(
                Entity,
                &example_01_platformer::features::dummy::components::Dummy,
            )>()
            .map(|(entity, _)| entity)
            .next()
            .unwrap();
        assert_eq!(
            world.get::<Health>(dummy).unwrap().max(),
            400.0,
            "the dummy's own"
        );
        // A summon takes it too.
        let summoned = enemies::summon(
            harness.world_mut(),
            EnemyKind::Crawler,
            Vec2::new(900.0, 400.0),
        )
        .unwrap();
        harness.step(1);
        assert_eq!(
            harness.world().get::<Health>(summoned).unwrap().max(),
            60.0 * factor
        );
        assert_eq!(
            game::difficulty_note(difficulty),
            format!("enemy health ×{factor} · enemy damage ×{factor}")
        );
    }
}

#[test]
fn the_wisp_chases_on_sight_searches_the_last_seen_spot_then_returns() {
    let (mut harness, player) = arena();
    let home = Vec2::new(650.0, 850.0);
    let wisp = enemy(&mut harness, EnemyKind::Wisp, home);
    assert!(until(&mut harness, 60, |h| state(h, wisp) == EnemyState::Chase));
    let before = at(&harness, wisp);
    harness.step(40);
    assert!(
        at(&harness, wisp).distance(super::centre(&harness, player))
            < before.distance(super::centre(&harness, player)),
        "it closed in"
    );
    // The player gone from its sight: it searches where it last saw it.
    let last = super::centre(&harness, player);
    teleport(&mut harness, player, Vec2::new(1250.0, TOP - 28.5));
    assert!(until(&mut harness, 30, |h| state(h, wisp) == EnemyState::Search));
    let search = harness
        .world()
        .get::<Brain>(wisp)
        .unwrap()
        .search
        .map(Vec2::from)
        .unwrap();
    assert!(
        search.distance(last) < 40.0,
        "the last seen spot: {search} and {last}"
    );
    let mut nearest = f32::MAX;
    let returned = until(&mut harness, 400, |h| {
        nearest = nearest.min(at(h, wisp).distance(search));
        state(h, wisp) == EnemyState::Return
    });
    assert!(returned, "three seconds, then home");
    assert!(nearest < 30.0, "it went there: {nearest}");
    assert!(until(&mut harness, 400, |h| state(h, wisp) == EnemyState::Patrol));
    assert!(
        at(&harness, wisp).distance(home) < 20.0,
        "home: {}",
        at(&harness, wisp)
    );
}

#[test]
fn the_wisp_is_immune_to_fire_and_burning_balls_draw_and_heal_it() {
    let (mut harness, _) = arena();
    let wisp = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(1000.0, 850.0));
    harness.step(2);
    land(&mut harness, wisp, Kind::Fire, 30.0);
    assert_eq!(health(&harness, wisp), 40.0, "fire is its own");
    land(&mut harness, wisp, Kind::Physical, 20.0);
    assert_eq!(health(&harness, wisp), 20.0);
    let ball = toys::spawn_ball(harness.world_mut(), Vec2::new(1200.0, TOP - 8.0), true);
    elements::ignite(harness.world_mut(), ball);
    let fire = super::body(&harness, ball);
    let start = at(&harness, wisp).distance(fire);
    harness.step(360);
    assert!(
        at(&harness, wisp).distance(fire) < start - 100.0,
        "drawn to it"
    );
    assert!(
        health(&harness, wisp) > 24.0,
        "healed: {}",
        health(&harness, wisp)
    );
    assert!(!tagged(&harness, wisp, "status.burning"), "never burning");
}

/// Waits, at most `frames` frames, for a thermal shock's frost blast to
/// burst more often than `before`.
fn blown(harness: &mut UiHarness, before: usize, frames: u32) -> bool {
    until(harness, frames, |h| bursts(h, "ex10_frost_blast") > before)
}

#[test]
fn a_frozen_wisp_blows_apart_under_a_fire_missile() {
    let (mut harness, _) = arena();
    // Within the missile's arc from the player at x 300: its blast reaches
    // 72 px from where it lands.
    let wisp = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(520.0, 850.0));
    harness.step(2);
    apply(&mut harness, wisp, "frozen", None);
    // It falls to the ground and lies there, frozen.
    harness.step(60);
    assert!(tagged(&harness, wisp, "status.frozen"));
    let before = bursts(&harness, "ex10_frost_blast");
    let aim = at(&harness, wisp);
    aim_at(&mut harness, aim);
    choose(&mut harness, Spell::FireMissile);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    assert!(
        blown(&mut harness, before, 120),
        "thermal shock, from fire it takes none of"
    );
    harness.step(2);
    assert!(!tagged(&harness, wisp, "status.frozen"), "its ice ends");
    assert_eq!(
        health(&harness, wisp),
        24.0,
        "the blast's 16 physical alone"
    );
}

#[test]
fn a_frozen_wisp_blows_apart_in_the_lava() {
    let (mut harness, player) = arena();
    // Basin 0's lake, columns 232 to 239 under row 64; the player on the
    // hollow's floor west of it, the wisp over its east pool.
    stand(&mut harness, player, 226.0 * 64.0, 63.0 * 64.0);
    harness.step(1);
    let wisp = enemy(
        &mut harness,
        EnemyKind::Wisp,
        Vec2::new(238.5 * 64.0, 64.0 * 64.0 - 160.0),
    );
    harness.step(2);
    apply(&mut harness, wisp, "frozen", None);
    let before = bursts(&harness, "ex10_frost_blast");
    assert!(
        blown(&mut harness, before, 120),
        "the lava's fire blows it apart"
    );
    harness.step(2);
    assert!(!tagged(&harness, wisp, "status.frozen"));
    assert_eq!(
        health(&harness, wisp),
        24.0,
        "the blast's 16 physical alone"
    );
}

#[test]
fn cold_freezes_the_wisp() {
    let (mut harness, _) = arena();
    let wisp = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(1000.0, 800.0));
    harness.step(2);
    for amount in [10.0, 20.0] {
        assert!(Hit::new(wisp, [(Kind::Cold, amount)]).queue(harness.world_mut()));
        harness.step(1);
    }
    assert!(
        tagged(&harness, wisp, "status.frozen"),
        "chilled, then frozen"
    );
    harness.step(1);
    assert!(held(&harness, wisp));
    let world = harness.world();
    assert!(
        world.get::<TopDownMover>(wisp).is_none(),
        "its mover set aside"
    );
    assert!(
        world.get::<PlatformerController>(wisp).is_some(),
        "it falls"
    );
    let high = at(&harness, wisp).y;
    harness.step(60);
    assert!(
        at(&harness, wisp).y > high + 100.0,
        "fallen: {}",
        at(&harness, wisp)
    );
    // Seven and a half seconds, then it thaws and flies again.
    assert!(until(&mut harness, 480, |h| !tagged(
        h,
        wisp,
        "status.frozen"
    )));
    harness.step(2);
    assert!(harness.world().get::<TopDownMover>(wisp).is_some());
    assert!(harness.world().get::<PlatformerController>(wisp).is_none());
    assert!(!held(&harness, wisp));
}

#[test]
fn the_wisp_burns_on_touch_and_lights_the_scene() {
    let (mut harness, player) = arena();
    let on_player = super::centre(&harness, player);
    let wisp = enemy(&mut harness, EnemyKind::Wisp, on_player);
    let mut burned = Vec::new();
    for _ in 0..3 {
        harness.step(1);
        burned.extend(
            harness
                .world()
                .resource::<EventQueue<Damaged>>()
                .iter()
                .filter(|hit| hit.entity == player && hit.source == Some(wisp))
                .map(|hit| hit.amount),
        );
    }
    assert_eq!(burned, [10.0], "its touch, 10 fire");
    assert_eq!(health(&harness, player), 90.0);
    let world = harness.world();
    let parts = world
        .get::<example_01_platformer::features::level::components::GlowParts>(wisp)
        .expect("its glow");
    assert!(
        world
            .get::<tungsten::core::Light>(parts.light.expect("its light"))
            .is_some(),
        "a light"
    );
}

#[test]
fn the_crawler_walks_its_ledge_and_turns_at_the_ends() {
    let (mut harness, _) = arena();
    let top = 700.0;
    super::floor(
        &mut harness,
        Vec2::new(900.0, top + 10.0),
        Vec2::new(200.0, 10.0),
    );
    let crawler = walker(&mut harness, EnemyKind::Crawler, 900.0, top);
    let (mut low, mut high, mut turns) = (f32::MAX, f32::MIN, 0);
    let mut heading = 1.0;
    let rest = at(&harness, crawler).y;
    for _ in 0..900 {
        harness.step(1);
        let now = at(&harness, crawler);
        low = low.min(now.x);
        high = high.max(now.x);
        assert!((now.y - rest).abs() < 2.0, "on the ledge: {now}");
        let facing = harness.world().get::<Enemy>(crawler).unwrap().heading;
        if facing != heading {
            turns += 1;
            heading = facing;
        }
    }
    assert!(turns >= 2, "{turns} turns");
    assert!(low < 760.0 && high > 1040.0, "end to end: {low} to {high}");
    assert!(
        low > 690.0 && high < 1110.0,
        "never past the ends: {low} to {high}"
    );
}

#[test]
fn a_crawler_is_scared_at_the_drawn_light() {
    // From each side, the player facing it: facing left, the drawn glow
    // hangs on the body's left, where the crawler reads it (I8).
    for side in [1.0_f32, -1.0] {
        let (mut harness, player) = arena();
        harness.world_mut().get_mut::<Lantern>(player).unwrap().on = true;
        let action = if side > 0.0 {
            "move_right"
        } else {
            "move_left"
        };
        harness.harness_mut().press_action(action);
        harness.step(1);
        harness.harness_mut().release_action(action);
        stand(&mut harness, player, 900.0, TOP);
        harness.step(2);
        let crawler = walker(&mut harness, EnemyKind::Crawler, 900.0 + side * 340.0, TOP);
        // Its snowball cooling, so it holds at the edge.
        cool(&mut harness, crawler);
        let light = |h: &UiHarness| crawler::light(h.world()).expect("the light burns");
        // It keeps out of its fear's edge, 1.3 times the drawn glow's.
        let fear = |h: &UiHarness| crawler::fear(h.world()).expect("the light burns");
        let (mut nearest, mut hisses) = (f32::MAX, 0);
        let mut puffs = 0;
        for _ in 0..240 {
            harness.step(1);
            let (centre, reach) = fear(&harness);
            nearest = nearest.min(at(&harness, crawler).distance(centre) - reach);
            hisses += harness
                .harness()
                .events::<Sounded>()
                .filter(|sounded| sounded.sound == Sound::CrawlerScared)
                .count();
            puffs = puffs.max(super::bursts(&harness, FROST_BREATH));
        }
        assert!(nearest >= -1.0, "outside its fear's edge: {nearest}");
        assert!(nearest <= 24.0, "held at the edge: {nearest}");
        let heading = harness.world().get::<Enemy>(crawler).unwrap().heading;
        assert_eq!(heading, -side, "facing the player");
        let clip = &harness
            .world()
            .get::<AnimationState>(crawler)
            .unwrap()
            .animation_id;
        let facing = if heading < 0.0 { "_left" } else { "" };
        assert_eq!(*clip, format!("ex10_crawler_scared{facing}"), "scared");
        assert_eq!(
            (hisses, puffs),
            (1, 1),
            "one hiss and one puff as it starts"
        );
        // With the light out, it comes in, its scare over.
        let (centre, reach) = light(&harness);
        harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
        let came = until(&mut harness, 240, |h| {
            at(h, crawler).distance(centre) < reach - 30.0
        });
        assert!(came, "the dark lets it in");
        let clip = &harness
            .world()
            .get::<AnimationState>(crawler)
            .unwrap()
            .animation_id;
        assert!(!clip.contains("scared"), "{clip}");
    }
}

/// The arena's player facing right with its staff lit, at x 900, and the
/// light's centre.
fn lit_arena() -> (UiHarness, Entity, Vec2) {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = true;
    harness.harness_mut().press_action("move_right");
    harness.step(1);
    harness.harness_mut().release_action("move_right");
    stand(&mut harness, player, 900.0, TOP);
    harness.step(2);
    let (light, _) = crawler::light(harness.world()).expect("the light burns");
    (harness, player, light)
}

#[test]
fn a_crawler_in_the_light_backs_out_past_its_fear_facing_the_player() {
    let (mut harness, _, light) = lit_arena();
    // On the ground right of the light, 120 px from its centre.
    let y = TOP - EnemyKind::Crawler.info().half.y - 0.5;
    let x = light.x + (120.0_f32.powi(2) - (y - light.y).powi(2)).sqrt();
    let crawler = walker(&mut harness, EnemyKind::Crawler, x, TOP);
    // Its snowball cooling, so it holds once out.
    cool(&mut harness, crawler);
    let (mut out, mut retreats, mut clips) = (None, 0, Vec::new());
    for frame in 1..=90 {
        harness.step(1);
        retreats += heard(&harness, Sound::CrawlerRetreated);
        assert_ne!(state(&harness, crawler), EnemyState::WindUp, "no tell");
        clips.push(
            harness
                .world()
                .get::<AnimationState>(crawler)
                .unwrap()
                .animation_id
                .clone(),
        );
        let (centre, reach) = crawler::fear(harness.world()).unwrap();
        if out.is_none() && at(&harness, crawler).distance(centre) >= reach {
            out = Some(frame);
        }
    }
    assert!(out.is_some(), "out past its fear's edge within 1.5 s");
    let heading = harness.world().get::<Enemy>(crawler).unwrap().heading;
    assert_eq!(heading, -1.0, "facing the player");
    assert!(
        clips.iter().any(|clip| clip == "ex10_crawler_retreat_left"),
        "{clips:?}"
    );
    assert_eq!(retreats, 1, "one retreat's hiss");
}

#[test]
fn a_cornered_crawler_stands_and_still_chills_on_touch() {
    let (mut harness, player, _) = lit_arena();
    // Right against the player, a wall at its back.
    let half = EnemyKind::Crawler.info().half;
    let x = 900.0 + courier::PLAYER_HALF.x + half.x + 2.0;
    super::floor(
        &mut harness,
        Vec2::new(x + half.x + 11.0, TOP - 60.0),
        Vec2::new(10.0, 60.0),
    );
    harness.step(1);
    let crawler = walker(&mut harness, EnemyKind::Crawler, x, TOP);
    let start = at(&harness, crawler);
    let hits = crawler_hits(&mut harness, player, crawler, 60);
    assert!(crawler::afraid(harness.world(), crawler), "inside its fear");
    assert!(
        (at(&harness, crawler).x - start.x).abs() < 4.0,
        "it stands: {start} to {}",
        at(&harness, crawler)
    );
    assert_eq!(hits.first(), Some(&15.0), "its touch, 15 cold");
}

#[test]
fn the_crawler_runs_at_210_from_a_fresh_spawn() {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
    let crawler = walker(&mut harness, EnemyKind::Crawler, 300.0 + 395.0, TOP);
    // Facing the player, which its eye sees from there.
    harness
        .world_mut()
        .get_mut::<Enemy>(crawler)
        .unwrap()
        .heading = -1.0;
    let mut fastest = 0.0_f32;
    for _ in 0..60 {
        harness.step(1);
        if state(&harness, crawler) == EnemyState::Chase {
            fastest = fastest.max(speed(&harness, crawler).x.abs());
        }
    }
    assert!((fastest - crawler::MOVE_SPEED).abs() < 1.0, "{fastest}");
    assert_eq!(crawler::MOVE_SPEED, 210.0);
}

/// A crawler 50 px short of a ledge's edge at x 1200 high in the sky, the
/// dark player 400 px on along `side` on ground `up` px higher across a gap
/// of `gap` px, a step up when it has none: over 3 s, whether the crawler
/// stood on the far ground, whether it kept to the near one throughout,
/// its hops' sounds and the nearest its centre came to the edge.
fn crossing(side: f32, gap: f32, up: f32) -> (bool, bool, usize, f32) {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
    let (edge, near) = (1200.0, TOP - 300.0);
    let far = near - up;
    super::floor(
        &mut harness,
        Vec2::new(edge - side * 200.0, near + 10.0),
        Vec2::new(200.0, 10.0),
    );
    // Solid down past the near ledge's top, so a step up shows its face.
    let bottom = far.max(near) + 20.0;
    super::floor(
        &mut harness,
        Vec2::new(edge + side * (gap + 300.0), (far + bottom) / 2.0),
        Vec2::new(300.0, (bottom - far) / 2.0),
    );
    harness.step(1);
    stand(&mut harness, player, edge + side * 350.0, far);
    let crawler = walker(&mut harness, EnemyKind::Crawler, edge - side * 50.0, near);
    harness
        .world_mut()
        .get_mut::<Enemy>(crawler)
        .unwrap()
        .heading = side;
    let half = EnemyKind::Crawler.info().half;
    let (mut hops, mut reached, mut kept, mut nearest) = (0, false, true, f32::MAX);
    for _ in 0..180 {
        harness.step(1);
        hops += heard(&harness, Sound::CrawlerHopped);
        let at = at(&harness, crawler);
        let on = |top: f32| (at.y - (top - half.y)).abs() < 2.0;
        reached |= (at.x - (edge + side * gap)) * side > 0.0 && on(far);
        kept &= (at.x - edge) * side < 0.0 && at.y <= near - half.y + 2.0;
        nearest = nearest.min((edge - at.x) * side);
    }
    (reached, kept, hops, nearest)
}

#[test]
fn a_chasing_crawler_hops_a_step_and_leaps_the_gaps_it_can_land_across() {
    for side in [1.0, -1.0] {
        for (gap, up, what) in [
            (0.0, 48.0, "a 48 px step"),
            (64.0, 0.0, "a level 64 px gap"),
            (64.0, 16.0, "a 64 px gap onto ground 16 px higher"),
            (140.0, -90.0, "a 140 px gap onto ground 90 px lower"),
        ] {
            let (reached, _, hops, _) = crossing(side, gap, up);
            assert!(reached, "{side}: over {what}");
            assert!(hops >= 1, "{side}: it chitters over {what}");
        }
        for (gap, up, what) in [
            (64.0, 48.0, "a gap onto ground 48 px higher"),
            (256.0, 0.0, "a 256 px gap"),
        ] {
            let (_, kept, hops, nearest) = crossing(side, gap, up);
            assert!(kept, "{side}: it stops at {what}");
            let half = EnemyKind::Crawler.info().half.x;
            assert!(
                nearest <= half + 8.0,
                "{side}: at the edge of {what}: {nearest}"
            );
            assert_eq!(hops, 0, "{side}: no hop at {what}");
        }
    }
}

#[test]
fn the_crawler_takes_double_fire_no_cold_and_bleeds() {
    let (mut harness, _) = arena();
    let crawler = walker(&mut harness, EnemyKind::Crawler, 1100.0, TOP);
    harness.step(2);
    land(&mut harness, crawler, Kind::Fire, 10.0);
    assert_eq!(health(&harness, crawler), 40.0, "double fire");
    land(&mut harness, crawler, Kind::Cold, 10.0);
    assert_eq!(health(&harness, crawler), 40.0, "no cold");
    assert!(Hit::new(crawler, [(Kind::Physical, 10.0)]).queue(harness.world_mut()));
    harness.step(2);
    assert!(health(&harness, crawler) <= 30.0);
    assert!(tagged(&harness, crawler, "status.bleeding"), "it bleeds");
}

/// A crawler 260 px to the player's right on the sky's ground, the
/// player's lantern out and its snowball cooling a minute, so it lunges.
fn stalker() -> (UiHarness, Entity, Entity) {
    let (mut harness, player) = arena();
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
    let crawler = walker(&mut harness, EnemyKind::Crawler, 560.0, TOP);
    cool(&mut harness, crawler);
    (harness, player, crawler)
}

/// Sets `enemy`'s snowball, throw or dash-in cooling for a minute.
fn cool(harness: &mut UiHarness, enemy: Entity) {
    harness
        .world_mut()
        .get_mut::<Brain>(enemy)
        .unwrap()
        .cooldown = 60.0;
}

#[test]
fn the_crawler_chills_on_touch_and_lunges_after_its_tell() {
    let (mut harness, player) = arena();
    let crawler = walker(&mut harness, EnemyKind::Crawler, 300.0, TOP);
    let mut chilled = Vec::new();
    for _ in 0..3 {
        harness.step(1);
        chilled.extend(
            harness
                .world()
                .resource::<EventQueue<Damaged>>()
                .iter()
                .filter(|hit| hit.entity == player && hit.source == Some(crawler))
                .map(|hit| hit.amount),
        );
    }
    assert_eq!(chilled, [15.0], "its touch, 15 cold");
    let (mut harness, _, crawler) = stalker();
    let mut step = 0;
    let (mut windup, mut attack) = (None, None);
    while step < 240 && attack.is_none() {
        harness.step(1);
        step += 1;
        match state(&harness, crawler) {
            EnemyState::WindUp if windup.is_none() => windup = Some(step),
            EnemyState::Attack => attack = Some(step),
            _ => {}
        }
    }
    let (windup, attack) = (windup.expect("its tell"), attack.expect("its lunge"));
    let tell = (attack - windup) as f32 / 60.0;
    assert!((tell - 0.4).abs() <= 1.5 / 60.0, "a 0.4 s tell: {tell}");
    harness.step(2);
    let speed = harness
        .world()
        .get::<tungsten::core::Velocity>(crawler)
        .unwrap()
        .0
        .x;
    assert!(
        (speed + LUNGE_SPEED).abs() < 1.0,
        "a lunge at the player: {speed}"
    );
}

/// The crawler's hits on `player` over `frames` frames.
fn crawler_hits(harness: &mut UiHarness, player: Entity, crawler: Entity, frames: u32) -> Vec<f32> {
    hits_over(harness, player, frames)
        .into_iter()
        .filter(|&(_, source, _)| source == Some(crawler))
        .map(|(amount, ..)| amount)
        .collect()
}

#[test]
fn a_crawler_lunge_at_a_still_player_lands_its_touch() {
    let (mut harness, player, crawler) = stalker();
    assert!(
        until(&mut harness, 240, |h| state(h, crawler)
            == EnemyState::Attack),
        "its lunge"
    );
    // Both bodies block each other, so the leap stops flush against the
    // player; the touch reaches past its body and lands.
    let hits = crawler_hits(&mut harness, player, crawler, 45);
    assert_eq!(hits, [15.0], "one touch, 15 cold");
}

#[test]
fn a_crawler_lunge_needs_its_target_within_reach_above() {
    let (mut harness, player, crawler) = stalker();
    // A ledge two tiles up, within its range across: out of the leap's reach.
    let ledge = TOP - 128.0;
    super::floor(
        &mut harness,
        Vec2::new(420.0, ledge + 10.0),
        Vec2::new(80.0, 10.0),
    );
    harness.step(1);
    stand(&mut harness, player, 420.0, ledge);
    let wound = until(&mut harness, 180, |h| {
        state(h, crawler) == EnemyState::WindUp
    });
    assert!(!wound, "no tell at a player 128 px up");
}

#[test]
fn a_crawler_lunge_landing_on_the_players_head_lands_its_touch() {
    let (mut harness, player) = arena();
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
    let head = super::centre(&harness, player).y - courier::PLAYER_HALF.y;
    let half = EnemyKind::Crawler.info().half.y;
    // Dropped from 40 px over the player's head, it lands flush on it.
    let crawler = enemy(
        &mut harness,
        EnemyKind::Crawler,
        Vec2::new(300.0, head - half - 40.0),
    );
    let hits = crawler_hits(&mut harness, player, crawler, 60);
    assert_eq!(
        hits.first(),
        Some(&15.0),
        "its touch reaches the head it stands on"
    );
}

#[test]
fn the_crawlers_lunge_leaps_about_220_px_and_53_up() {
    let (mut harness, _) = arena();
    let crawler = walker(&mut harness, EnemyKind::Crawler, 900.0, TOP);
    still(&mut harness, crawler);
    harness.step(2);
    let start = at(&harness, crawler);
    harness
        .world_mut()
        .get_mut::<Enemy>(crawler)
        .unwrap()
        .heading = 1.0;
    crawler::enter_attack(harness.world_mut(), crawler);
    let grounded = |h: &UiHarness| {
        h.world()
            .get::<PlatformerController>(crawler)
            .is_some_and(PlatformerController::grounded)
    };
    let (mut top, mut landed, mut aloft) = (start.y, None, false);
    for _ in 0..90 {
        harness.step(1);
        top = top.min(at(&harness, crawler).y);
        aloft |= !grounded(&harness);
        if aloft && grounded(&harness) && landed.is_none() {
            landed = Some(at(&harness, crawler).x - start.x);
        }
    }
    let rise = start.y - top;
    assert!((rise - 53.0).abs() <= 3.0, "its apex: {rise}");
    let landed = landed.expect("it lands");
    assert!((landed - 220.0).abs() <= 8.0, "its landing: {landed}");
    assert!(speed(&harness, crawler).x.abs() < 1.0, "it stops");
    let skid = at(&harness, crawler).x - start.x - landed;
    assert!((0.0..=90.0).contains(&skid), "its skid: {skid}");
}

#[test]
fn a_freeze_mid_tell_holds_the_lunge_until_the_thaw() {
    let (mut harness, _, crawler) = stalker();
    assert!(until(&mut harness, 240, |h| state(h, crawler)
        == EnemyState::WindUp));
    harness.step(12);
    let elapsed = harness
        .world()
        .get::<StateMachine<EnemyState>>(crawler)
        .unwrap()
        .elapsed();
    apply(&mut harness, crawler, "frozen", None);
    harness.step(120);
    let machine = harness
        .world()
        .get::<StateMachine<EnemyState>>(crawler)
        .unwrap();
    assert_eq!(machine.state(), EnemyState::WindUp, "its lunge holds");
    assert_eq!(machine.elapsed(), elapsed, "the tell's time kept");
    let speed = harness
        .world()
        .get::<tungsten::core::Velocity>(crawler)
        .unwrap()
        .0
        .x;
    assert!(speed.abs() < 1.0, "no lunge while frozen: {speed}");
    // At the thaw the tell resumes with the time it had left.
    assert!(until(&mut harness, 420, |h| !tagged(
        h,
        crawler,
        "status.frozen"
    )));
    let mut left = 0;
    assert!(until(&mut harness, 60, |h| {
        left += 1;
        state(h, crawler) == EnemyState::Attack
    }));
    let resumed = left as f32 / 60.0;
    assert!(
        (resumed - (0.4 - elapsed)).abs() <= 2.5 / 60.0,
        "the tell's rest: {resumed} after {elapsed}"
    );
}

/// The snowballs in flight, in save-ID order.
fn snowballs(harness: &UiHarness) -> Vec<Entity> {
    snowball::flying(harness.world())
        .into_iter()
        .map(|(_, ball)| ball)
        .collect()
}

#[test]
fn a_dark_crawler_snowballs_the_player_on_its_arc_then_cools_four_seconds() {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
    let x = 1540.0;
    stand(&mut harness, player, x, TOP);
    let crawler = walker(&mut harness, EnemyKind::Crawler, x + 260.0, TOP);
    harness
        .world_mut()
        .get_mut::<Enemy>(crawler)
        .unwrap()
        .heading = -1.0;
    let gravity = harness
        .world()
        .resource::<tungsten::core::PhysicsConfig>()
        .gravity;
    let mut arc: Option<Arc> = None;
    let (mut throws, mut hits, mut chilled) = (Vec::new(), Vec::new(), false);
    for frame in 0..900 {
        // Still until the first snowball lands; then 330 px ahead of the
        // crawler, in its band and past its lunge's reach.
        let lead = if hits.is_empty() {
            x
        } else {
            at(&harness, crawler).x - 330.0
        };
        stand(&mut harness, player, lead, TOP);
        tungsten::kit::heal(harness.world_mut(), player, 100.0);
        harness.step(1);
        if let Some(arc) = arc.as_mut() {
            arc.check(&harness);
        }
        if heard(&harness, Sound::SnowballThrown) > 0 {
            throws.push(frame);
        }
        if arc.is_none()
            && let Some(&ball) = snowballs(&harness).first()
        {
            arc = Some(Arc {
                what: "a snowball",
                entity: ball,
                from: body(&harness, ball),
                launch: speed(&harness, ball),
                gravity,
                steps: 0,
            });
        }
        hits.extend(took(&harness, player, Some(crawler)));
        chilled |= tagged(&harness, player, "status.chilled");
        if throws.len() >= 2 {
            break;
        }
    }
    assert_eq!(arc.map(|arc| arc.steps), Some(12), "on its arc");
    assert_eq!(hits.first(), Some(&SNOWBALL_COLD), "12 cold from it");
    assert!(chilled, "it chills the player");
    assert!(throws.len() >= 2, "it throws again: {throws:?}");
    let after = (throws[1] - throws[0]) as f32 / 60.0;
    assert!(
        after >= SNOWBALL_COOLDOWN - 1e-3,
        "again {after} s after, not sooner than 4"
    );
}

#[test]
fn a_cooling_crawler_closes_in_and_lunges() {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
    let crawler = walker(&mut harness, EnemyKind::Crawler, 300.0 + 350.0, TOP);
    harness
        .world_mut()
        .get_mut::<Enemy>(crawler)
        .unwrap()
        .heading = -1.0;
    cool(&mut harness, crawler);
    let start = at(&harness, crawler).x;
    let (mut lunged, mut thrown) = (false, 0);
    for _ in 0..240 {
        harness.step(1);
        lunged |= heard(&harness, Sound::CrawlerLunge) > 0;
        thrown += heard(&harness, Sound::SnowballThrown);
        if lunged {
            break;
        }
    }
    assert!(lunged, "it lunges");
    assert_eq!(thrown, 0, "no snowball while it cools");
    assert!(at(&harness, crawler).x < start - 40.0, "it closed in");
}

#[test]
fn a_crawler_held_at_the_fear_edge_snowballs_a_lit_player_and_never_lunges() {
    let (mut harness, player, _) = lit_arena();
    let crawler = walker(&mut harness, EnemyKind::Crawler, 900.0 + 340.0, TOP);
    harness
        .world_mut()
        .get_mut::<Enemy>(crawler)
        .unwrap()
        .heading = -1.0;
    let (mut thrown, mut lunges, mut hits, mut deepest) = (0, 0, Vec::new(), f32::MAX);
    for _ in 0..360 {
        stand(&mut harness, player, 900.0, TOP);
        tungsten::kit::heal(harness.world_mut(), player, 100.0);
        harness.step(1);
        thrown += heard(&harness, Sound::SnowballThrown);
        lunges += heard(&harness, Sound::CrawlerLunge);
        hits.extend(took(&harness, player, Some(crawler)));
        let (centre, reach) = crawler::fear(harness.world()).unwrap();
        deepest = deepest.min(at(&harness, crawler).distance(centre) - reach);
    }
    // The staff's orb sways with the wizard's idle frames, its light a few
    // px with it, which the crawler backs out of at once.
    assert!(deepest >= -12.0, "at its fear's edge: {deepest}");
    assert!(thrown >= 1, "it snowballs from the edge");
    assert_eq!(lunges, 0, "never a lunge while the light burns");
    assert!(hits.contains(&SNOWBALL_COLD), "{hits:?}");
}

#[test]
fn an_afraid_crawler_throws_nothing() {
    let (mut harness, player, _) = lit_arena();
    // Cornered inside its fear, a wall at its back, the player in its band.
    let half = EnemyKind::Crawler.info().half;
    let x = 900.0 + 150.0;
    super::floor(
        &mut harness,
        Vec2::new(x + half.x + 11.0, TOP - 60.0),
        Vec2::new(10.0, 60.0),
    );
    harness.step(1);
    let crawler = walker(&mut harness, EnemyKind::Crawler, x, TOP);
    let mut thrown = 0;
    for _ in 0..240 {
        stand(&mut harness, player, 900.0, TOP);
        harness.step(1);
        assert!(crawler::afraid(harness.world(), crawler), "afraid");
        assert_ne!(state(&harness, crawler), EnemyState::WindUp, "no tell");
        thrown += heard(&harness, Sound::SnowballThrown);
    }
    assert_eq!(thrown, 0, "nothing thrown");
    assert!(snowballs(&harness).is_empty());
}

/// A snowball thrown from `from` at `velocity` in the arena: where its
/// centre was as it burst, within 120 steps, and every hit meanwhile.
fn burst_of(harness: &mut UiHarness, from: Vec2, velocity: Vec2) -> (Option<Vec2>, Vec<f32>) {
    let ball = snowball::throw(harness.world_mut(), from, velocity, None, 1.0);
    let (mut burst, mut hits) = (None, Vec::new());
    for _ in 0..120 {
        harness.step(1);
        hits.extend(harness.harness().events::<Damaged>().map(|hit| hit.amount));
        let at = harness
            .harness()
            .events::<Sounded>()
            .find(|sounded| sounded.sound == Sound::SnowballBurst)
            .map(|sounded| sounded.at);
        burst = burst.or(at);
        if !harness.world().is_alive(ball) {
            break;
        }
    }
    assert!(!harness.world().is_alive(ball), "it is gone");
    (burst, hits)
}

#[test]
fn a_snowball_bursts_at_its_first_touch_of_the_ground_a_wall_and_a_crate_hurting_nothing() {
    let (mut harness, _) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let reach = snowball::SNOWBALL_RADIUS + snowball::SNOWBALL_MARGIN;
    // Straight down onto the ground.
    let (burst, hits) = burst_of(&mut harness, Vec2::new(1200.0, TOP - 200.0), Vec2::ZERO);
    let at = burst.expect("it bursts on the ground");
    assert!(TOP - at.y <= reach + 1.0, "at its first touch: {at}");
    assert!(hits.is_empty(), "{hits:?}");
    // Across into a wall.
    let face = 1600.0;
    super::floor(
        &mut harness,
        Vec2::new(face + 20.0, TOP - 200.0),
        Vec2::new(20.0, 200.0),
    );
    harness.step(1);
    let from = Vec2::new(face - 200.0, TOP - 300.0);
    let (burst, hits) = burst_of(&mut harness, from, Vec2::new(700.0, 0.0));
    let at = burst.expect("it bursts on the wall");
    assert!(face - at.x <= reach + 1.0, "at its first touch: {at}");
    assert!(
        at.y < TOP - 100.0,
        "on the wall's face, not off the ground: {at}"
    );
    assert!(hits.is_empty(), "{hits:?}");
    // Down onto a crate.
    let crate_ = props::spawn(
        harness.world_mut(),
        PropKind::Crate,
        Vec2::new(2000.0, TOP - CRATE_HALF.y - 0.5),
    );
    harness.step(2);
    let lid = body(&harness, crate_).y - CRATE_HALF.y;
    let (burst, hits) = burst_of(&mut harness, Vec2::new(2000.0, lid - 150.0), Vec2::ZERO);
    let at = burst.expect("it bursts on the crate");
    assert!(lid - at.y <= reach + 1.0, "at its first touch: {at}");
    assert!(hits.is_empty(), "{hits:?}");
    assert!(harness.world().is_alive(crate_), "the crate stands");
}

#[test]
fn a_snowball_on_a_dummy_chilled_near_its_freeze_rung_freezes_it() {
    let (mut harness, _) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let dummy = dummy_at(&mut harness, 1400.0);
    harness.step(2);
    // Chilled, which empties its cold, then 10 of the 20 cold its freeze
    // rung needs.
    for _ in 0..2 {
        assert!(Hit::new(dummy, [(Kind::Cold, 10.0)]).queue(harness.world_mut()));
        harness.step(1);
    }
    assert!(tagged(&harness, dummy, "status.chilled"), "chilled");
    assert!(!tagged(&harness, dummy, "status.frozen"));
    let centre = body(&harness, dummy);
    let from = centre - Vec2::new(120.0, 20.0);
    let gravity = harness
        .world()
        .resource::<tungsten::core::PhysicsConfig>()
        .gravity;
    let velocity = troll::ballistic(from, centre, 0.25, gravity);
    snowball::throw(harness.world_mut(), from, velocity, None, 1.0);
    let frozen = until(&mut harness, 60, |h| tagged(h, dummy, "status.frozen"));
    assert!(frozen, "its 12 cold freezes it");
}

#[test]
fn the_beetles_front_turns_a_bullet_that_flies_on_and_hits_another_target() {
    let (mut harness, player) = arena();
    let dummy = dummy_at(&mut harness, 700.0);
    let beetle = walker(&mut harness, EnemyKind::Beetle, 1300.0, TOP);
    harness
        .world_mut()
        .get_mut::<Enemy>(beetle)
        .unwrap()
        .heading = -1.0;
    harness.step(2);
    let y = at(&harness, beetle).y;
    let id = shoot(&mut harness, Vec2::new(1000.0, y), Vec2::X);
    let (mut turned, mut shell, mut flown_on) = (0, Vec::new(), Vec::new());
    for _ in 0..30 {
        harness.step(1);
        shell.extend(took(&harness, beetle, None));
        flown_on.extend(took(&harness, dummy, Some(player)));
        let Some(shot) = bullet(&harness, id) else {
            break;
        };
        turned = turned.max(harness.world().get::<Bullet>(shot).unwrap().ricochets);
    }
    assert_eq!(turned, 1, "its front turned the shot");
    assert!(shell.is_empty(), "the front takes nothing: {shell:?}");
    assert_eq!(flown_on, [SHOT_DAMAGE], "the shot flew on into the dummy");
    assert!(bullet(&harness, id).is_none(), "and ended there");
}

#[test]
fn the_beetles_rear_takes_a_bullet_whole_and_ends_it() {
    let (mut harness, player) = arena();
    let beetle = walker(&mut harness, EnemyKind::Beetle, 1300.0, TOP);
    harness.step(2);
    assert_eq!(
        harness.world().get::<Enemy>(beetle).unwrap().heading,
        1.0,
        "its rear toward the shot"
    );
    let y = at(&harness, beetle).y;
    let id = shoot(&mut harness, Vec2::new(1000.0, y), Vec2::X);
    let mut landed = Vec::new();
    for _ in 0..30 {
        harness.step(1);
        landed.extend(took(&harness, beetle, Some(player)));
        if bullet(&harness, id).is_none() {
            break;
        }
    }
    assert_eq!(landed, [SHOT_DAMAGE], "whole, past its shell");
    assert!(bullet(&harness, id).is_none(), "the hit ends the shot");
    // Any hit 01 builds lands whole on its rear and a quarter on its front.
    for (side, part) in [(1.0, 0.25), (-1.0, 1.0)] {
        let before = health(&harness, beetle);
        let point = at(&harness, beetle) + Vec2::new(side * 20.0, 0.0);
        let world = harness.world_mut();
        Hit::new(beetle, [(Kind::Physical, 8.0)])
            .at(point)
            .request(world)
            .apply(world);
        assert_eq!(before - health(&harness, beetle), 8.0 * part, "side {side}");
    }
}

#[test]
fn the_beetle_paws_then_charges_and_its_charge_hurts_enemies_too() {
    let (mut harness, _) = arena();
    // A crawler with no mind of its own, in the charge's way.
    let crawler = walker(&mut harness, EnemyKind::Crawler, 480.0, TOP);
    harness
        .world_mut()
        .remove_component::<StateMachine<EnemyState>>(crawler);
    let beetle = walker(&mut harness, EnemyKind::Beetle, 620.0, TOP);
    let (mut step, mut windup, mut attack) = (0, None, None);
    let mut struck = Vec::new();
    while step < 300 && attack.is_none() {
        harness.step(1);
        step += 1;
        match state(&harness, beetle) {
            EnemyState::WindUp if windup.is_none() => windup = Some(step),
            EnemyState::Attack => attack = Some(step),
            _ => {}
        }
    }
    let (windup, attack) = (windup.expect("its paw"), attack.expect("its charge"));
    let paw = (attack - windup) as f32 / 60.0;
    assert!((paw - PAW_SECS).abs() <= 1.5 / 60.0, "a 0.8 s paw: {paw}");
    for i in 0..60 {
        if i == 2 {
            let pace = speed(&harness, beetle).x;
            assert!(
                (pace + CHARGE_SPEED).abs() < 1.0,
                "a charge at the player: {pace}"
            );
        }
        struck.extend(took(&harness, crawler, Some(beetle)));
        harness.step(1);
    }
    // One charge hit; what follows is the crawler's bleeding, from it too.
    assert_eq!(struck.first(), Some(&30.0), "its charge hurts the crawler");
    assert!(struck[1..].iter().all(|&tick| tick < 30.0), "{struck:?}");
}

/// A beetle standing on the sky's ground at `x` with no mind, and `prop`
/// spawned before it along `heading`, 4 px off its face; both indexed.
fn beetle_before(
    harness: &mut UiHarness,
    x: f32,
    heading: f32,
    prop: impl FnOnce(&mut UiHarness, f32) -> Entity,
) -> (Entity, Entity) {
    harness.harness_mut().set_dt(1.0 / 60.0);
    let beetle = walker(harness, EnemyKind::Beetle, x, TOP);
    harness
        .world_mut()
        .remove_component::<StateMachine<EnemyState>>(beetle);
    let face = x + heading * EnemyKind::Beetle.info().half.x;
    let prop = prop(harness, face + heading * 4.0);
    harness.step(2);
    (beetle, prop)
}

/// Sets `beetle` charging along `heading`: its machine starts in its
/// attack, entered at the next step.
fn charge(harness: &mut UiHarness, beetle: Entity, heading: f32) {
    let world = harness.world_mut();
    world.get_mut::<Enemy>(beetle).unwrap().heading = heading;
    world.insert(beetle, StateMachine::new(EnemyState::Attack));
}

/// How often `sound` was heard in the last frame.
fn heard(harness: &UiHarness, sound: Sound) -> usize {
    harness
        .harness()
        .events::<Sounded>()
        .filter(|sounded| sounded.sound == sound)
        .count()
}

#[test]
fn a_charge_ploughs_crates_and_kegs_out_of_its_way_from_either_side() {
    for heading in [1.0, -1.0] {
        for (kind, half) in [
            (PropKind::Crate, CRATE_HALF),
            (PropKind::Keg, props::KEG_HALF),
        ] {
            let (mut harness, _) = arena();
            let (beetle, prop) = beetle_before(&mut harness, 1200.0, heading, |h, near| {
                let centre = Vec2::new(near + heading * half.x, TOP - half.y - 0.5);
                props::spawn(h.world_mut(), kind, centre)
            });
            let start = body(&harness, prop);
            charge(&mut harness, beetle, heading);
            let (mut ploughed, mut blown) = (0, 0);
            for _ in 0..60 {
                harness.step(1);
                let now = heard(&harness, Sound::BeetlePloughed);
                assert!(now <= 1, "{kind:?}, {heading}: {now} plough sounds a step");
                ploughed += now;
                blown += heard(&harness, Sound::KegBlasted);
            }
            assert!(ploughed >= 1, "{kind:?}, {heading}: it sounds");
            assert_eq!(blown, 0, "{kind:?}, {heading}: nothing blew");
            assert!(harness.world().is_alive(prop), "{kind:?}, {heading}");
            let end = body(&harness, prop);
            assert!(
                (end.x - start.x) * heading >= 120.0,
                "{kind:?}, {heading}: from {start} to {end}"
            );
            assert!(
                (at(&harness, beetle).x - start.x) * heading > 0.0,
                "{kind:?}, {heading}: the beetle passed where it stood: {}",
                at(&harness, beetle)
            );
        }
    }
}

#[test]
fn a_charge_throws_a_standing_player_back_and_up() {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let beetle = walker(&mut harness, EnemyKind::Beetle, 560.0, TOP);
    harness
        .world_mut()
        .remove_component::<StateMachine<EnemyState>>(beetle);
    harness.step(2);
    charge(&mut harness, beetle, -1.0);
    let (mut hit, mut thrown) = (false, None);
    for _ in 0..60 {
        harness.step(1);
        if hit {
            let velocity = speed(&harness, player);
            if velocity.length() > 100.0 {
                thrown = Some(velocity);
                break;
            }
        }
        hit |= !took(&harness, player, Some(beetle)).is_empty();
    }
    let thrown = thrown.expect("the charge hit the player and threw it");
    assert!(
        (700.0..=850.0).contains(&thrown.length()),
        "thrown at {thrown}"
    );
    assert!(thrown.x < 0.0 && -thrown.y > 150.0, "back and up: {thrown}");
}

#[test]
fn a_charge_stops_at_iron_and_presses_it_until_it_ends() {
    let (mut harness, _) = arena();
    let (beetle, iron) = beetle_before(&mut harness, 1200.0, 1.0, |h, near| {
        let centre = Vec2::new(near + IRON_HALF.x, TOP - IRON_HALF.y - 0.5);
        toys::place_iron(h.world_mut(), centre)
    });
    harness.step(30);
    let start = body(&harness, iron);
    charge(&mut harness, beetle, 1.0);
    let mut recovered = None;
    for step in 1..=70 {
        harness.step(1);
        if recovered.is_none() && state(&harness, beetle) == EnemyState::Recover {
            recovered = Some(step);
        }
    }
    let end = body(&harness, iron);
    assert!(start.distance(end) < 4.0, "iron from {start} to {end}");
    let face = at(&harness, beetle).x + EnemyKind::Beetle.info().half.x;
    assert!(face <= end.x - IRON_HALF.x + 0.5, "behind it: {face}");
    // Its first visit enters the attack at step 1.
    let charged = (recovered.expect("its recovery") - 1) as f32 / 60.0;
    assert!(
        (charged - CHARGE_SECS).abs() <= 1.5 / 60.0,
        "a charge of {charged} s"
    );
}

#[test]
fn the_beetles_paw_puffs_its_dust_and_its_charge_draws_its_streaks() {
    let (mut harness, _) = arena();
    let names: Vec<String> = (0..4)
        .flat_map(|f| {
            [
                format!("ex10_charge_streak_{f}"),
                format!("ex10_charge_streak_{f}_left"),
            ]
        })
        .collect();
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    for (i, name) in names.iter().enumerate() {
        mock_sprite(assets, name, 800 + i as u32, false);
    }
    let beetle = walker(&mut harness, EnemyKind::Beetle, 620.0, TOP);
    assert!(until(&mut harness, 240, |h| state(h, beetle)
        == EnemyState::WindUp));
    harness.step(12);
    assert!(bursts(&harness, "ex10_paw_dust") > 0, "its paw's dust");
    assert_eq!(bursts(&harness, "ex10_charge_dust"), 0, "no trail yet");
    assert!(until(&mut harness, 60, |h| state(h, beetle) == EnemyState::Attack));
    let mut drawn = std::collections::BTreeSet::new();
    for _ in 0..12 {
        harness.step(1);
        let batches = enemies::draw::charges(harness.world());
        drawn.extend(batches.iter().map(|batch| batch.texture.0));
    }
    // It charges left, at the player: the left-facing frames, 801 to 807.
    assert!(
        drawn
            .iter()
            .all(|handle| handle % 2 == 1 && (801..808).contains(handle)),
        "{drawn:?}"
    );
    assert!(drawn.len() >= 2, "its frames turn over: {drawn:?}");
}

#[test]
fn a_stun_interrupts_the_beetles_wind_up() {
    let (mut harness, _) = arena();
    let beetle = walker(&mut harness, EnemyKind::Beetle, 620.0, TOP);
    assert!(until(&mut harness, 240, |h| state(h, beetle)
        == EnemyState::WindUp));
    harness.step(12);
    apply(&mut harness, beetle, "stunned", None);
    let mut seen = Vec::new();
    for _ in 0..60 {
        harness.step(1);
        seen.push(state(&harness, beetle));
        let armed = harness.world().get::<Hitbox>(beetle).unwrap().is_armed();
        assert!(!armed, "no charge");
        assert!(speed(&harness, beetle).x.abs() < 1.0);
    }
    assert!(!seen.contains(&EnemyState::Attack), "{seen:?}");
    assert!(
        seen.contains(&EnemyState::Recover),
        "the stun ended the paw: {seen:?}"
    );
}

#[test]
fn the_beetle_takes_a_quarter_physical_half_fire_double_shock_and_conducts() {
    let (mut harness, player) = arena();
    let beetle = walker(&mut harness, EnemyKind::Beetle, 1300.0, TOP);
    harness.step(2);
    land(&mut harness, beetle, Kind::Physical, 20.0);
    assert_eq!(health(&harness, beetle), 115.0, "a quarter physical");
    land(&mut harness, beetle, Kind::Fire, 10.0);
    assert_eq!(health(&harness, beetle), 110.0, "half fire");
    land(&mut harness, beetle, Kind::Shock, 10.0);
    assert_eq!(health(&harness, beetle), 90.0, "double shock");
    assert!(lightning::conductive(harness.world(), beetle));
    // A strike above it falls onto it and doubles on it.
    let aim = at(&harness, beetle) + Vec2::new(0.0, -200.0);
    let landing = lightning::strike(harness.world_mut(), player, aim);
    let centre = body(&harness, beetle);
    assert!(
        (landing.x - aim.x).abs() < 0.01 && landing.y < centre.y && landing.y > centre.y - 40.0,
        "on the beetle: {landing} against {centre}"
    );
    harness.step(2);
    assert_eq!(health(&harness, beetle), 10.0, "double the strike's shock");
}

#[test]
fn a_frozen_beetle_shatters_at_a_light_blow() {
    let (mut harness, _) = arena();
    let dummy = dummy_at(&mut harness, 900.0);
    let beetle = walker(&mut harness, EnemyKind::Beetle, 1300.0, TOP);
    harness.step(2);
    for target in [beetle, dummy] {
        apply(&mut harness, target, "frozen", None);
    }
    harness.step(1);
    assert!(tagged(&harness, beetle, "status.frozen"));
    let before = bursts(&harness, "ex10_ice_shatter");
    for target in [beetle, dummy] {
        assert!(Hit::new(target, [(Kind::Physical, 5.0)]).queue(harness.world_mut()));
    }
    harness.step(3);
    assert!(
        !tagged(&harness, beetle, "status.frozen"),
        "its brittle rung shattered it"
    );
    assert!(bursts(&harness, "ex10_ice_shatter") > before);
    assert!(
        tagged(&harness, dummy, "status.frozen"),
        "the same blow leaves the dummy frozen"
    );
}

#[test]
fn a_fast_iron_block_crushes_the_beetle() {
    let (mut harness, _) = arena();
    let beetle = walker(&mut harness, EnemyKind::Beetle, 1300.0, TOP);
    harness.step(2);
    let centre = body(&harness, beetle);
    let top = centre.y - EnemyKind::Beetle.info().half.y;
    let block = toys::place_iron(harness.world_mut(), Vec2::new(centre.x, top - IRON_HALF.y));
    let drive = |harness: &mut UiHarness, speed: f32| {
        harness
            .world_mut()
            .get_mut::<PreviousVelocity>(block)
            .unwrap()
            .0 = Vec2::new(0.0, speed);
        crush(harness.world_mut());
    };
    drive(&mut harness, CRUSH_SPEED - 1.0);
    assert_eq!(health(&harness, beetle), 120.0, "too slow to crush");
    drive(&mut harness, CRUSH_SPEED + 40.0);
    assert_eq!(health(&harness, beetle), 0.0, "crushed, its shell or not");
    harness.step(3);
    assert!(!harness.world().is_alive(beetle));
}

#[test]
fn the_maps_bats_hang_at_their_roosts_from_the_start() {
    // Placed flush under their roofs, each hangs at its roost on its first
    // frame, a dormant one far off too.
    let (mut harness, player) = run();
    harness.step(1);
    let eye = super::centre(&harness, player);
    let world = harness.world();
    let bats: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &Enemy)>()
        .filter(|(_, enemy)| enemy.kind == EnemyKind::Bat)
        .map(|(entity, enemy)| (entity, enemy.home()))
        .collect();
    assert_eq!(bats.len(), 8, "the map's bats");
    let mut farthest: f32 = 0.0;
    for (bat, home) in bats {
        let hangs = at(&harness, bat);
        assert!(hangs.distance(home) < 1.0, "{hangs} off its roost {home}");
        let shown = world.get::<Shown>(bat).unwrap().clip.clone();
        assert!(
            ["ex10_bat_roost", "ex10_bat_roost_left"].contains(&shown.as_str()),
            "{shown} at {hangs}"
        );
        farthest = farthest.max(hangs.distance(eye));
    }
    assert!(farthest > DORMANT_PAST, "a dormant one: {farthest}");
}

#[test]
fn the_bat_roosts_until_it_sees_the_player() {
    let (mut harness, player, bat) = roosting();
    let roost = roost(&harness, bat);
    let grip = EnemyKind::Bat.info().centre.y;
    assert!(
        roost.distance(Vec2::new(1300.0, 600.0 + grip)) < 0.5,
        "its frame's top on the ceiling: {roost}"
    );
    assert_eq!(state(&harness, bat), EnemyState::Patrol);
    assert!(
        at(&harness, bat).distance(roost) <= 2.0,
        "it hangs at its roost: {}",
        at(&harness, bat)
    );
    let shown = harness.world().get::<Shown>(bat).unwrap().clip.clone();
    assert!(shown.starts_with("ex10_bat_roost"), "{shown}");
    harness.step(60);
    assert!(at(&harness, bat).distance(roost) <= 2.0, "still there");
    // The player comes into view below it.
    stand(&mut harness, player, 1200.0, TOP);
    assert!(until(&mut harness, 30, |h| sees(h, bat)));
    assert!(until(&mut harness, 30, |h| state(h, bat) != EnemyState::Patrol));
    harness.step(60);
    assert!(
        at(&harness, bat).distance(roost) > 20.0,
        "it left its roost: {}",
        at(&harness, bat)
    );
}

#[test]
fn the_bat_flashes_dives_in_its_window_and_climbs_back() {
    let (mut harness, player, bat) = roosting();
    let roost = roost(&harness, bat);
    stand(&mut harness, player, 1200.0, TOP);
    let (mut step, mut windup, mut attack) = (0, None, None);
    let mut flashed = false;
    while step < 300 && attack.is_none() {
        harness.step(1);
        step += 1;
        match state(&harness, bat) {
            EnemyState::WindUp if windup.is_none() => windup = Some(step),
            EnemyState::WindUp => {
                let shown = &harness.world().get::<Shown>(bat).unwrap().clip;
                flashed |= shown.starts_with("ex10_bat_windup");
            }
            EnemyState::Attack => attack = Some(step),
            _ => {}
        }
    }
    let (windup, attack) = (windup.expect("its flash"), attack.expect("its dive"));
    let flash = (attack - windup) as f32 / 60.0;
    assert!(
        (flash - FLASH_SECS).abs() <= 1.5 / 60.0,
        "a 0.5 s flash: {flash}"
    );
    assert!(flashed, "it shows its white flash");
    let (mut armed, mut fastest, mut hurt) = (0, 0.0_f32, Vec::new());
    for _ in 0..60 {
        if harness.world().get::<Hitbox>(bat).unwrap().is_armed() {
            armed += 1;
        }
        fastest = fastest.max(speed(&harness, bat).length());
        hurt.extend(took(&harness, player, Some(bat)));
        harness.step(1);
    }
    let window = armed as f32 / 60.0;
    assert!(
        (window - DIVE_SECS).abs() <= 1.5 / 60.0,
        "its window: {window}"
    );
    assert!(
        (fastest - DIVE_SPEED).abs() < 1.0,
        "a dive at 700: {fastest}"
    );
    assert_eq!(hurt, [14.0], "the dive hits the player once");
    assert!(
        until(&mut harness, 240, |h| at(h, bat).distance(roost) <= ARRIVED),
        "it climbs back to its roost"
    );
}

#[test]
fn a_frozen_bat_falls_and_shatters_on_landing() {
    let (mut harness, _, bat) = roosting();
    let high = at(&harness, bat).y;
    let before = bursts(&harness, "ex10_ice_shatter");
    apply(&mut harness, bat, "frozen", None);
    harness.step(10);
    assert!(at(&harness, bat).y > high + 5.0, "it falls");
    assert!(health(&harness, bat) > 0.0, "whole while it falls");
    let mut low = high;
    let shattered = until(&mut harness, 120, |h| {
        if let Some(centre) = enemies::centre(h.world(), bat) {
            low = low.max(centre.y);
        }
        h.world().get::<Health>(bat).is_none_or(Health::is_dead)
    });
    assert!(shattered, "it shatters");
    let half = EnemyKind::Bat.info().half.y;
    assert!(low >= TOP - half - 2.0, "on landing: {low}");
    assert!(bursts(&harness, "ex10_ice_shatter") > before);
}

#[test]
fn the_bat_burns_and_bleeds() {
    let (mut harness, _) = arena();
    let bat = enemy(&mut harness, EnemyKind::Bat, Vec2::new(1300.0, 800.0));
    harness.step(2);
    assert!(Hit::new(bat, [(Kind::Fire, 6.0)]).queue(harness.world_mut()));
    harness.step(2);
    assert!(health(&harness, bat) <= 24.0, "all of the fire");
    assert!(tagged(&harness, bat, "status.burning"), "it burns");
    assert!(Hit::new(bat, [(Kind::Physical, 6.0)]).queue(harness.world_mut()));
    harness.step(2);
    assert!(tagged(&harness, bat, "status.bleeding"), "it bleeds");
    assert!(health(&harness, bat) > 0.0);
}

#[test]
fn the_trolls_healing_stops_while_it_burns() {
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 1300.0, Difficulty::Normal);
    still(&mut harness, troll);
    harness.step(2);
    // An untyped hit, which starts no bleeding.
    tungsten::kit::damage(harness.world_mut(), troll, 200.0);
    let hurt = health(&harness, troll);
    harness.step(60);
    let healed = health(&harness, troll) - hurt;
    assert!((healed - 6.0).abs() < 0.01, "6 a second: {healed}");
    assert!(Hit::new(troll, [(Kind::Fire, 20.0)]).queue(harness.world_mut()));
    harness.step(2);
    assert!(tagged(&harness, troll, "status.burning"));
    let mut last = health(&harness, troll);
    for _ in 0..120 {
        harness.step(1);
        let now = health(&harness, troll);
        if !tagged(&harness, troll, "status.burning") {
            break;
        }
        assert!(now <= last, "no healing while it burns: {last} → {now}");
        last = now;
    }
    // Once the fire is out, it heals again.
    assert!(until(&mut harness, 360, |h| !tagged(
        h,
        troll,
        "status.burning"
    )));
    let out = health(&harness, troll);
    harness.step(60);
    assert!(health(&harness, troll) > out + 4.0, "healing again");
}

#[test]
fn the_troll_slams_after_its_wind_up_hurting_kicking_and_shattering() {
    let (mut harness, player) = arena();
    stand(&mut harness, player, 250.0, TOP);
    let dummy = dummy_at(&mut harness, 330.0);
    let troll = troll_at(&mut harness, 380.0, Difficulty::Normal);
    let ball = toys::spawn_ball(
        harness.world_mut(),
        Vec2::new(440.0, TOP - toys::BALL_RADIUS),
        false,
    );
    harness.step(2);
    apply(&mut harness, dummy, "frozen", None);
    let (mut step, mut windup, mut attack) = (0, None, None);
    let (mut hurt, mut dummy_hurt) = (Vec::new(), Vec::new());
    let rest = body(&harness, ball);
    while step < 400 && attack.is_none() {
        harness.step(1);
        step += 1;
        match state(&harness, troll) {
            EnemyState::WindUp if windup.is_none() => windup = Some(step),
            EnemyState::Attack => attack = Some(step),
            _ => {}
        }
        hurt.extend(took(&harness, player, Some(troll)));
        dummy_hurt.extend(took(&harness, dummy, Some(troll)));
    }
    let (windup, attack) = (windup.expect("its wind-up"), attack.expect("its slam"));
    let tell = (attack - windup) as f32 / 60.0;
    assert!(
        (tell - troll::SLAM_TELL).abs() <= 1.5 / 60.0,
        "its wind-up: {tell}"
    );
    // The bowling ball kicked by its mass at the slam, away from the fist
    // and up, then the step's gravity.
    let fist = troll::fist(harness.world(), troll).unwrap();
    let kick = (troll::KICK / 8.0).min(troll::KICK_MOST);
    let gravity = harness
        .world()
        .resource::<tungsten::core::PhysicsConfig>()
        .gravity;
    let away = (rest - fist).normalize();
    let expected = away * kick + Vec2::new(0.0, -troll::KICK_RISE * kick) + gravity / 60.0;
    let kicked = speed(&harness, ball);
    assert!(
        kicked.distance(expected) < 2.0,
        "the ball kicked by its mass: {kicked} against {expected}"
    );
    for _ in 0..4 {
        harness.step(1);
        hurt.extend(took(&harness, player, Some(troll)));
        dummy_hurt.extend(took(&harness, dummy, Some(troll)));
    }
    assert_eq!(hurt, [40.0], "the slam hurts the player");
    assert_eq!(dummy_hurt, [40.0], "and the dummy");
    assert!(
        !tagged(&harness, dummy, "status.frozen"),
        "the frozen dummy shattered"
    );
}

#[test]
fn a_thrown_iron_block_hits_the_player_once_credited_and_scaled() {
    let mut landed = Vec::new();
    for difficulty in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
        let (mut harness, player) = arena();
        let troll = troll_at(&mut harness, 900.0, difficulty);
        still(&mut harness, troll);
        let block = iron_at(&mut harness, 1060.0);
        harness.step(2);
        lift_and_throw(&mut harness, troll, block);
        let hits = hits_over(&mut harness, player, 150);
        assert_eq!(hits.len(), 1, "{difficulty:?}: once: {hits:?}");
        assert_eq!(hits[0].1, Some(troll), "credited to the troll");
        landed.push(hits[0].0);
    }
    let normal = landed[1];
    assert!(normal > 0.0);
    assert!((landed[0] - 0.75 * normal).abs() < 1e-3, "{landed:?}");
    assert!((landed[2] - 1.5 * normal).abs() < 1e-3, "{landed:?}");
}

#[test]
fn a_thrown_bowling_ball_hurts_by_its_speed_and_mass() {
    let (mut harness, player) = arena();
    let troll = troll_at(&mut harness, 900.0, Difficulty::Normal);
    still(&mut harness, troll);
    let ball = toys::spawn_ball(
        harness.world_mut(),
        Vec2::new(1000.0, TOP - toys::BALL_RADIUS),
        false,
    );
    harness.step(2);
    lift_and_throw(&mut harness, troll, ball);
    let mass = 1.0
        / harness
            .world()
            .get::<tungsten::core::RigidBody>(ball)
            .unwrap()
            .inv_mass;
    assert_eq!(mass, 8.0, "a bowling ball's mass");
    let ((amount, source), rule) =
        first_impact(&mut harness, ball, player, mass, 150).expect("it hits the player");
    assert_eq!(source, Some(troll));
    assert!(amount > 0.0 && amount < BODY_HIT_MOST, "{amount}");
    assert!(
        (amount - rule).abs() < 0.05,
        "by its speed and mass: {amount} against {rule}"
    );
}

#[test]
fn a_light_body_needs_more_speed_to_hurt() {
    // A still crawler (physical resist 1) and a ball on the ground just off
    // its side, flung flat at it, its bounce taken off so it ends its step
    // at the crawler's side, where a hit is read; the dice set so the
    // player's hit does not crit.
    let fling = |small: bool, pace: f32| {
        let (mut harness, _) = arena();
        let crawler = walker(&mut harness, EnemyKind::Crawler, 1200.0, TOP);
        still(&mut harness, crawler);
        harness.step(2);
        let radius = if small {
            toys::SMALL_BALL_RADIUS
        } else {
            toys::BALL_RADIUS
        };
        let x = at(&harness, crawler).x - EnemyKind::Crawler.info().half.x - radius - 8.0;
        let ball = toys::spawn_ball(harness.world_mut(), Vec2::new(x, TOP - radius - 0.5), small);
        kick_at(&mut harness, ball, Vec2::new(pace, 0.0));
        let world = harness.world_mut();
        world.insert_resource(KitRng::seeded(PLAIN_SEED));
        let body = world.get_mut::<tungsten::core::RigidBody>(ball).unwrap();
        body.restitution = 0.0;
        let mass = 1.0 / body.inv_mass;
        first_impact(&mut harness, ball, crawler, mass, 30)
    };
    let hit = fling(true, 1500.0);
    assert!(hit.is_none(), "a small ball at 1,500 px/s: {hit:?}");
    for (small, pace) in [(true, 3000.0), (false, 600.0)] {
        let ((amount, _), rule) = fling(small, pace).expect("it hurts");
        assert!(rule > 0.0, "{pace}: {rule}");
        assert!(
            (amount - rule).abs() < 0.05,
            "at {pace}: {amount} against {rule}"
        );
    }
}

#[test]
fn a_thrown_frozen_enemy_flies_the_throws_arc_and_hurts_at_its_kinds_mass() {
    let (mut harness, player) = arena();
    let troll = troll_at(&mut harness, 900.0, Difficulty::Normal);
    still(&mut harness, troll);
    let crawler = walker(&mut harness, EnemyKind::Crawler, 1050.0, TOP);
    harness.step(2);
    apply(&mut harness, crawler, "frozen", None);
    harness.step(1);
    lift_and_throw(&mut harness, troll, crawler);
    let world = harness.world();
    let from = body(&harness, crawler);
    let launch = speed(&harness, crawler);
    let gravity = world.resource::<tungsten::core::PhysicsConfig>().gravity;
    assert!(launch.x < -100.0, "at the player: {launch}");
    // Its path over its first steps is the throw's arc.
    let mut steps = 0;
    for _ in 0..12 {
        harness.step(1);
        steps += super::steps_this_frame(&harness);
        let t = steps as f32 / 60.0;
        let arc = from + launch * t + 0.5 * gravity * t * t;
        let at = body(&harness, crawler);
        assert!(
            at.distance(arc) < 2.0,
            "step {steps}: {at} off the arc {arc}"
        );
    }
    let mass = EnemyKind::Crawler.info().mass;
    let ((amount, source), rule) =
        first_impact(&mut harness, crawler, player, mass, 150).expect("it hits the player");
    assert_eq!(source, Some(troll));
    assert!(
        amount > 0.0 && amount < BODY_HIT_MOST,
        "a finite mass: {amount}"
    );
    assert!(
        (amount - rule).abs() < 0.05,
        "at a crawler's mass: {amount} against {rule}"
    );
}

#[test]
fn a_throw_whose_troll_dies_in_flight_keeps_its_damage_through_a_load() {
    let throw = |kill: bool| -> (UiHarness, Entity, Entity) {
        let (mut harness, player) = arena();
        let troll = troll_at(&mut harness, 900.0, Difficulty::Hard);
        still(&mut harness, troll);
        let block = iron_at(&mut harness, 1060.0);
        harness.step(2);
        lift_and_throw(&mut harness, troll, block);
        harness.step(2);
        if kill {
            land(&mut harness, troll, Kind::Physical, 10_000.0);
        }
        harness.step(8);
        (harness, player, troll)
    };
    // The run the throw finishes in.
    let (mut whole, player, troll) = throw(false);
    let hits = hits_over(&mut whole, player, 150);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].1, Some(troll));
    // The troll dead in flight, a save, a load: the same hit, from nobody.
    let (killed, player, _) = throw(true);
    let player_id = id_of(&killed, player);
    let saved = save::capture(killed.world()).unwrap();
    assert_eq!(saved.throws.len(), 1, "the throw in flight");
    assert_eq!(saved.throws[0].factor, 1.5, "Hard's damage factor, kept");
    let mut fresh = reload(saved);
    let player = by_id(&fresh, player_id).expect("the player");
    let after = hits_over(&mut fresh, player, 150);
    assert_eq!(after.len(), 1, "{after:?}");
    assert_eq!(after[0].1, None, "its troll is gone");
    assert!(
        (after[0].0 - hits[0].0).abs() < 0.5,
        "the same damage: {} against {}",
        after[0].0,
        hits[0].0
    );
    assert!((after[0].2 as i64 - hits[0].2 as i64).abs() <= 1);
}

/// Sets `troll` searching nowhere, standing, so it carries what it lifts:
/// on patrol it would set it down.
fn carrying(harness: &mut UiHarness, troll: Entity) {
    harness
        .world_mut()
        .insert(troll, StateMachine::new(EnemyState::Search));
}

#[test]
fn a_save_while_the_troll_holds_iron_loads_with_the_hold() {
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 1300.0, Difficulty::Normal);
    carrying(&mut harness, troll);
    let block = iron_at(&mut harness, 1460.0);
    harness.step(2);
    assert!(troll::pick_up(harness.world_mut(), troll, block));
    harness.step(3);
    let (troll_id, block_id) = (id_of(&harness, troll), id_of(&harness, block));
    let saved = save::capture(harness.world()).unwrap();
    let held = saved.enemies.iter().find(|enemy| enemy.id == troll_id);
    assert_eq!(held.and_then(|enemy| enemy.held), Some(block_id));
    let mut fresh = reload(saved);
    let (troll, block) = (
        by_id(&fresh, troll_id).unwrap(),
        by_id(&fresh, block_id).unwrap(),
    );
    let hands = |h: &UiHarness| {
        let at = h.world().get::<Position>(troll).unwrap().0;
        at + troll::HANDS - Vec2::new(0.0, IRON_HALF.y)
    };
    let kind = |h: &UiHarness| {
        h.world()
            .get::<tungsten::core::RigidBody>(block)
            .unwrap()
            .kind
    };
    assert_eq!(troll::held(fresh.world(), troll), Some(block));
    assert_eq!(kind(&fresh), tungsten::core::BodyKind::Sensor);
    assert!(
        body(&fresh, block).distance(hands(&fresh)) < 1.0,
        "at its hands"
    );
    fresh.step(20);
    assert_eq!(troll::held(fresh.world(), troll), Some(block));
    assert!(
        body(&fresh, block).distance(hands(&fresh)) < 12.0,
        "still at its hands: {} against {}",
        body(&fresh, block),
        hands(&fresh)
    );
}

#[test]
fn killing_or_evicting_the_troll_drops_what_it_holds() {
    let dropped = |harness: &UiHarness, block: Entity| {
        let world = harness.world();
        !world.has::<example_01_platformer::features::enemies::components::Held>(block)
            && world.get::<tungsten::core::RigidBody>(block).unwrap().kind
                == tungsten::core::BodyKind::Dynamic
    };
    // Killed.
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 1300.0, Difficulty::Normal);
    carrying(&mut harness, troll);
    let block = iron_at(&mut harness, 1460.0);
    harness.step(2);
    assert!(troll::pick_up(harness.world_mut(), troll, block));
    harness.step(2);
    let high = body(&harness, block).y;
    land(&mut harness, troll, Kind::Physical, 10_000.0);
    harness.step(1);
    assert!(dropped(&harness, block), "dropped at the kill");
    harness.step(30);
    assert!(body(&harness, block).y > high + 20.0, "it falls");
    // Evicted: the oldest summoned enemy past the cap.
    let (mut harness, player) = arena();
    let stats = harness
        .world_mut()
        .get_mut::<tungsten::kit::Stats>(player)
        .unwrap();
    stats.set_base("max_mana", 10_000.0);
    let summoned = enemies::summon(
        harness.world_mut(),
        EnemyKind::Troll,
        Vec2::new(1300.0, TOP - 56.0),
    );
    let troll = summoned.expect("a summoned troll");
    carrying(&mut harness, troll);
    let block = iron_at(&mut harness, 1460.0);
    harness.step(2);
    assert!(troll::pick_up(harness.world_mut(), troll, block));
    for i in 0..SUMMON_CAP {
        let at = Vec2::new(900.0 + 60.0 * i as f32, 600.0);
        enemies::summon(harness.world_mut(), EnemyKind::Wisp, at);
    }
    harness.step(2);
    assert!(!harness.world().is_alive(troll), "evicted");
    assert!(dropped(&harness, block), "dropped at the eviction");
}

#[test]
fn knockback_the_black_hole_balls_and_iron_barely_move_the_troll() {
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 1300.0, Difficulty::Normal);
    still(&mut harness, troll);
    harness.step(2);
    let start = at(&harness, troll);
    let world = harness.world_mut();
    Hit::new(troll, [(Kind::Physical, 10.0)])
        .push(Vec2::new(900.0, -500.0))
        .request(world)
        .apply(world);
    harness.step(30);
    assert!(at(&harness, troll).distance(start) < 1.0, "no knockback");
    let hole = toys::place_black_hole(harness.world_mut(), start + Vec2::new(160.0, -40.0));
    harness.step(2);
    assert_eq!(
        move_speed(&harness, troll),
        troll::MOVE_SPEED / 2.0,
        "the hole slows it"
    );
    harness.step(58);
    // A crawler's hold would take it past 200 px in this second.
    assert!(
        at(&harness, troll).distance(start) < 20.0,
        "the pull barely moves it: {}",
        at(&harness, troll)
    );
    harness.world_mut().despawn(hole);
    harness.step(2);
    assert_eq!(
        move_speed(&harness, troll),
        troll::MOVE_SPEED,
        "its speed back"
    );
    let start = at(&harness, troll);
    let ball = toys::spawn_ball(harness.world_mut(), start + Vec2::new(-120.0, 20.0), false);
    kick_at(&mut harness, ball, Vec2::new(900.0, 0.0));
    let block = toys::place_iron(harness.world_mut(), start + Vec2::new(-260.0, -20.0));
    kick_at(&mut harness, block, Vec2::new(700.0, 0.0));
    harness.step(40);
    assert!(
        at(&harness, troll).distance(start) < 4.0,
        "balls and iron barely move it: {}",
        at(&harness, troll)
    );
}

#[test]
fn the_troll_slows_in_stages_and_freezes_past_its_threshold() {
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 1300.0, Difficulty::Normal);
    still(&mut harness, troll);
    harness.step(2);
    let cold = |harness: &mut UiHarness, amount: f32| {
        assert!(Hit::new(troll, [(Kind::Cold, amount)]).queue(harness.world_mut()));
        harness.step(2);
    };
    assert_eq!(move_speed(&harness, troll), troll::MOVE_SPEED);
    // The chill's 35 %, and the grade's up to 35 % more as the bucket, which
    // the chill empties, fills on to the freeze's rung.
    let graded = |harness: &UiHarness| -> f32 {
        let world = harness.world();
        let fill = world
            .get::<AilmentBuckets>(troll)
            .map_or(0.0, |buckets| buckets.fill("damage.cold"));
        let threshold = world.get::<AilmentThreshold>(troll).unwrap();
        let max = world.get::<Health>(troll).unwrap().max();
        let mult = world
            .get::<tungsten::kit::Stats>(troll)
            .and_then(|stats| stats.get("ailment_threshold_mult"))
            .unwrap_or(1.0);
        let at = (threshold.flat() + threshold.of_max_health() * max) * mult;
        let grade = (fill / (FREEZE_NEEDS * at)).clamp(0.0, 1.0);
        troll::MOVE_SPEED * (1.0 - 0.35 - 0.35 * grade)
    };
    cold(&mut harness, 7.5);
    assert!(tagged(&harness, troll, "status.chilled"), "chilled");
    let first = move_speed(&harness, troll);
    assert!(
        (first - graded(&harness)).abs() < 0.01,
        "the first fill: {first}"
    );
    // Past half the freeze's rung, slower again, not frozen yet.
    cold(&mut harness, 15.0);
    let second = move_speed(&harness, troll);
    assert!(
        (second - graded(&harness)).abs() < 0.01 && second < first - 5.0,
        "deeper in: {second} after {first}"
    );
    assert!(!tagged(&harness, troll, "status.frozen"));
    // Past the freeze's rung within the chill, a three-second freeze.
    cold(&mut harness, 7.5);
    assert!(tagged(&harness, troll, "status.frozen"), "frozen");
    let mut frames = 0;
    assert!(until(&mut harness, 300, |h| {
        frames += 1;
        !tagged(h, troll, "status.frozen")
    }));
    let held = frames as f32 / 60.0;
    assert!(
        (2.9..=3.1).contains(&held),
        "a three-second freeze: {held} s"
    );
}

#[test]
fn the_trolls_slam_waits_out_its_cooldown() {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let troll = troll_at(&mut harness, 1300.0, Difficulty::Normal);
    harness.step(2);
    let home = harness.world().get::<Position>(troll).unwrap().0;
    // The courier in its slam's range and out of its swipe's, both held.
    let spot = at(&harness, troll).x - 120.0;
    let mut landed = Vec::new();
    let mut was = None;
    for frame in 0..480 {
        stand(&mut harness, player, spot, TOP);
        tungsten::kit::heal(harness.world_mut(), player, 1000.0);
        let world = harness.world_mut();
        world.get_mut::<Position>(troll).unwrap().0 = home;
        world.get_mut::<PrevPosition>(troll).unwrap().0 = home;
        harness.step(1);
        let strike = harness
            .world()
            .get::<Brain>(troll)
            .and_then(|brain| brain.strike);
        let now = (state(&harness, troll), strike);
        if now == (EnemyState::Attack, Some(Strike::Slam)) && was != Some(now) {
            landed.push(frame);
        }
        was = Some(now);
    }
    assert!(landed.len() >= 2, "it slams again: {landed:?}");
    for pair in landed.windows(2) {
        let gap = (pair[1] - pair[0]) as f32 / 60.0;
        assert!(
            gap >= troll::SLAM_COOLDOWN - 1.0 / 60.0,
            "{gap} s between slams: {landed:?}"
        );
    }
}

#[test]
fn a_steady_ice_spray_freezes_the_troll() {
    // The feedback's probe: a still troll 260 px away under the ice spray
    // held on it, mana kept full, freezes within five seconds.
    let (mut harness, player) = arena();
    let troll = troll_at(&mut harness, 560.0, Difficulty::Normal);
    still(&mut harness, troll);
    harness.step(2);
    choose(&mut harness, Spell::IceSpray);
    let eye = super::centre(&harness, player);
    let target = Vec2::new(at(&harness, troll).x, eye.y);
    aim_at(&mut harness, target);
    harness.harness_mut().press_action("cast");
    let mut frozen = None;
    for frame in 0..300 {
        if let Some(stats) = harness.world_mut().get_mut::<tungsten::kit::Stats>(player) {
            stats.set_base("mana", 100.0);
        }
        harness.step(1);
        if tagged(&harness, troll, "status.frozen") {
            frozen = Some(frame);
            break;
        }
    }
    harness.harness_mut().release_action("cast");
    assert!(frozen.is_some(), "never frozen in five seconds");
}

#[test]
fn the_troll_stuns_only_past_a_high_threshold() {
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 1300.0, Difficulty::Normal);
    still(&mut harness, troll);
    harness.step(2);
    let shock = |harness: &mut UiHarness, amount: f32| {
        assert!(Hit::new(troll, [(Kind::Shock, amount)]).queue(harness.world_mut()));
        harness.step(2);
    };
    shock(&mut harness, 15.0);
    assert!(tagged(&harness, troll, "status.shocked"), "shocked");
    // 45 landed, past the others' stunning needs.
    shock(&mut harness, 30.0);
    assert!(!tagged(&harness, troll, "status.stunned"), "not yet");
    // 120 more, past three times.
    shock(&mut harness, 80.0);
    assert!(tagged(&harness, troll, "status.stunned"), "stunned");
}

#[test]
fn the_troll_shows_its_health_bar() {
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 900.0, Difficulty::Normal);
    still(&mut harness, troll);
    harness.step(2);
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    mock_sprite(assets, enemies::draw::FLAT, 41, false);
    let quads = |harness: &UiHarness| -> Vec<(Vec2, Vec2, [u8; 4])> {
        enemies::draw::bars(harness.world())
            .into_iter()
            .filter(|batch| batch.texture.0 == 41)
            .flat_map(|batch| batch.instances)
            .map(|quad| (Vec2::from(quad.position), Vec2::from(quad.size), quad.color))
            .collect()
    };
    let full = quads(&harness);
    assert_eq!(full.len(), 2, "its back and its fill: {full:?}");
    let (back, fill) = (full[0], full[1]);
    let size = enemies::draw::BAR_SIZE;
    assert_eq!(back.1, size);
    assert_eq!(fill.1, size - Vec2::splat(2.0), "full");
    let frame_top = harness
        .world()
        .get::<tungsten::core::Transform>(troll)
        .unwrap()
        .position
        .y;
    assert!(back.0.y + size.y <= frame_top, "over its head");
    assert!(
        (back.0.x + size.x / 2.0 - at(&harness, troll).x).abs() < 1.0,
        "centred over its body"
    );
    assert_eq!(
        (back.2, fill.2),
        (enemies::draw::BAR_BACK, enemies::draw::BAR_FILL)
    );
    land(&mut harness, troll, Kind::Physical, 300.0);
    let half = quads(&harness)[1].1.x;
    assert!(
        (half - (size.x - 2.0) / 2.0).abs() < 0.01,
        "half its health: {half}"
    );
}

/// A seed whose stream's first draw from the loot table `table` is
/// `prefab`.
fn seed_for(harness: &UiHarness, table: &str, prefab: Option<&str>) -> u64 {
    let data = harness.world().resource::<tungsten::core::DataRegistry>();
    let reference = tungsten::core::DataRef::<tungsten::kit::WeightedTable>::by_id(table);
    let table = reference.resolve(data).expect("the loot table");
    (0..1000)
        .find(|&seed| table.draw(&mut KitRng::seeded(seed)) == prefab)
        .expect("a seed")
}

/// The live orbs of `kind` that dropped: the map's placed ones left out.
fn orbs(harness: &UiHarness, kind: OrbKind) -> Vec<Entity> {
    let world = harness.world();
    let placed = &world.resource::<PlacedOrbs>().0;
    world
        .query::<(Entity, &Orb, Option<&SaveId>)>()
        .filter(|(_, orb, id)| orb.kind == kind && !id.is_some_and(|id| placed.contains(&id.0)))
        .map(|(entity, ..)| entity)
        .collect()
}

#[test]
fn a_killed_enemy_rolls_its_drops_and_an_orb_heals_or_restores_mana() {
    let (mut harness, player) = arena();
    // The player hurt by an untyped hit, which starts no bleeding.
    tungsten::kit::damage(harness.world_mut(), player, 50.0);
    for (kind, x) in [(OrbKind::Health, 700.0), (OrbKind::Mana, 1100.0)] {
        let crawler = walker(&mut harness, EnemyKind::Crawler, x, TOP);
        still(&mut harness, crawler);
        harness.step(2);
        if kind == OrbKind::Mana {
            let stats = harness
                .world_mut()
                .get_mut::<tungsten::kit::Stats>(player)
                .unwrap();
            stats.set_base("mana", 40.0);
        }
        let seed = seed_for(&harness, "enemy_drops", Some(kind.prefab()));
        harness.world_mut().insert_resource(KitRng::seeded(seed));
        land(&mut harness, crawler, Kind::Physical, 1000.0);
        harness.step(3);
        let dropped = orbs(&harness, kind);
        assert_eq!(dropped.len(), 1, "{kind:?}: its roll dropped one");
        let orb = dropped[0];
        assert!(harness.world().get::<SaveId>(orb).is_some(), "adopted");
        // Within the player's reach, the magnet draws it in.
        let near = body(&harness, orb).x - 70.0;
        stand(&mut harness, player, near, TOP);
        let mut before = (health(&harness, player), super::mana(&harness));
        let taken = until(&mut harness, 120, |h| {
            let alive = h.world().is_alive(orb);
            if alive {
                before = (health(h, player), super::mana(h));
            }
            !alive
        });
        assert!(taken, "{kind:?}: the player takes it");
        harness.step(1);
        let after = (health(&harness, player), super::mana(&harness));
        match kind {
            OrbKind::Health => assert_eq!(after.0 - before.0, 20.0, "20 health"),
            OrbKind::Mana => {
                let restored = after.1 - before.1;
                // Mana regenerates 1.5 a tenth of a second besides.
                assert!((restored - 25.0).abs() <= 1.5, "25 mana: {restored}");
            }
        }
    }
}

/// The orb a roll dropped of `kind` once adopted, its save ID its own, at
/// most `frames` frames on.
fn adopted(harness: &mut UiHarness, kind: OrbKind, frames: u32) -> Option<Entity> {
    let mut found = None;
    until(harness, frames, |h| {
        found = orbs(h, kind)
            .into_iter()
            .find(|&orb| h.world().get::<SaveId>(orb).is_some());
        found.is_some()
    });
    found
}

fn orb_centre(harness: &UiHarness, orb: Entity) -> Vec2 {
    harness.world().get::<Position>(orb).unwrap().0 + ORB_CENTRE
}

#[test]
fn a_drop_lands_clear_of_rock_falls_and_is_drawn_in() {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    // Stone over the ground, which an orb lands on as on the map's walls.
    slab(
        &mut harness,
        Vec2::new(1000.0, TOP + 12.0),
        Vec2::new(500.0, 12.0),
    );
    harness.step(1);
    // A bat killed in mid-air: its orb centred on its body, then falling.
    let bat = enemy(&mut harness, EnemyKind::Bat, Vec2::new(900.0, TOP - 200.0));
    still(&mut harness, bat);
    harness.step(2);
    let seed = seed_for(&harness, "enemy_drops", Some(OrbKind::Health.prefab()));
    harness.world_mut().insert_resource(KitRng::seeded(seed));
    let died = at(&harness, bat);
    land(&mut harness, bat, Kind::Physical, 1000.0);
    let orb = adopted(&mut harness, OrbKind::Health, 5).expect("the bat's roll dropped one");
    let first = orb_centre(&harness, orb);
    let below = first.y - died.y;
    assert!(
        first.x == died.x && (0.0..=2.001).contains(&below),
        "centred on its body, a step's fall below: {first} at {died}"
    );
    harness.step(30);
    let fell = orb_centre(&harness, orb).y - first.y;
    assert!((fell - 60.0).abs() <= 2.0, "it falls 120 px/s: {fell}");
    harness.step(120);
    let bottom = orb_centre(&harness, orb).y + ORB_HALF.y;
    assert!(
        (bottom - TOP).abs() <= 1.0,
        "it rests on the stone: {bottom}"
    );
    // A crawler killed on the stone: its orb lands clear of it.
    let crawler = walker(&mut harness, EnemyKind::Crawler, 1300.0, TOP);
    still(&mut harness, crawler);
    harness.step(2);
    let seed = seed_for(&harness, "enemy_drops", Some(OrbKind::Mana.prefab()));
    harness.world_mut().insert_resource(KitRng::seeded(seed));
    land(&mut harness, crawler, Kind::Physical, 1000.0);
    let orb = adopted(&mut harness, OrbKind::Mana, 5).expect("the crawler's roll dropped one");
    let bottom = orb_centre(&harness, orb).y + ORB_HALF.y;
    assert!(bottom <= TOP + 1e-3, "clear of the stone: {bottom}");
    // The player stood 180 px off draws it in.
    let x = orb_centre(&harness, orb).x;
    stand(&mut harness, player, x - 180.0, TOP);
    let drawn = until(&mut harness, 60, |h| !h.world().is_alive(orb));
    assert!(drawn, "drawn in from 180 px");
}

#[test]
fn an_orb_passes_enemies_and_fades_after_its_lifetime() {
    let (mut harness, _) = arena();
    let crawler = walker(&mut harness, EnemyKind::Crawler, 1300.0, TOP);
    still(&mut harness, crawler);
    harness.step(2);
    let corner = at(&harness, crawler) - Vec2::splat(12.0);
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let orb = drops::spawn(world, OrbKind::Health, corner, id, ORB_SECS).expect("an orb");
    harness.step(60);
    assert!(harness.world().is_alive(orb), "the crawler passes it by");
    let mut frames = 60;
    let faded = until(&mut harness, 1300, |h| {
        frames += 1;
        !h.world().is_alive(orb)
    });
    assert!(faded, "it fades");
    let lived = frames as f32 / 60.0;
    assert!(
        (lived - ORB_SECS).abs() <= 2.0 / 60.0,
        "after its lifetime: {lived} s"
    );
}

#[test]
fn the_combat_layer_fades_in_while_an_enemy_sees_the_player() {
    let (mut harness, _) = arena();
    let world = harness.world();
    let combat = world
        .resource::<Music>()
        .combat
        .expect("the combat layer plays");
    let registry = world.resource::<tungsten::core::SoundRegistry>();
    let level = registry
        .get_by_id(MUSIC_COMBAT)
        .map(|handle| registry.get_volume(handle))
        .expect("its sound");
    let moves = |h: &UiHarness| -> Vec<(f32, f32)> {
        h.harness()
            .audio()
            .iter()
            .filter_map(|command| match command {
                AudioCommand::SetVoiceVolume {
                    voice,
                    volume,
                    fade,
                    ..
                } if *voice == combat => Some((*volume, *fade)),
                _ => None,
            })
            .collect()
    };
    let mut heard = Vec::new();
    for _ in 0..30 {
        harness.step(1);
        heard.extend(moves(&harness));
    }
    assert!(heard.is_empty(), "silent while no enemy sees: {heard:?}");
    let wisp = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(520.0, 900.0));
    for _ in 0..30 {
        harness.step(1);
        heard.extend(moves(&harness));
    }
    assert!(sees(&harness, wisp));
    assert_eq!(heard, [(level, COMBAT_IN)], "it fades in");
    calm(&mut harness);
    heard.clear();
    for _ in 0..10 {
        harness.step(1);
        heard.extend(moves(&harness));
    }
    assert_eq!(heard, [(0.0, COMBAT_OUT)], "and out after the last");
}

#[test]
fn a_killed_enemy_starts_no_pending_attack() {
    let (mut harness, _, crawler) = stalker();
    let asked = until(&mut harness, 240, |h| {
        h.world()
            .get::<StateMachine<EnemyState>>(crawler)
            .is_some_and(|machine| machine.pending() == Some(EnemyState::Attack))
    });
    assert!(asked, "its tell asked for the lunge");
    land(&mut harness, crawler, Kind::Physical, 1000.0);
    assert!(health(&harness, crawler) <= 0.0);
    for _ in 0..4 {
        harness.step(1);
        if !harness.world().is_alive(crawler) {
            break;
        }
        assert_ne!(
            state(&harness, crawler),
            EnemyState::Attack,
            "no lunge from the dead"
        );
        let speed = harness
            .world()
            .get::<tungsten::core::Velocity>(crawler)
            .unwrap()
            .0;
        assert!(speed.length() < 1.0, "{speed}");
    }
    assert!(
        !harness.world().is_alive(crawler),
        "gone the step after the kill"
    );
}

#[test]
fn a_killed_map_enemy_stays_dead_through_a_load() {
    let (mut harness, _) = run();
    let world = harness.world();
    let mut placed: Vec<(u64, Entity)> = world
        .query::<(Entity, &Enemy, &SaveId)>()
        .map(|(entity, _, id)| (id.0, entity))
        .collect();
    placed.sort_unstable_by_key(|&(id, _)| id);
    assert_eq!(placed.len() as u64, save::MAP_ENEMIES, "the map's enemies");
    let (dead_id, dead) = placed[0];
    land(&mut harness, dead, Kind::Physical, 1000.0);
    harness.step(3);
    assert!(!harness.world().is_alive(dead));
    let saved = save::capture(harness.world()).unwrap();
    assert!(saved.gone.contains(&dead_id), "gone: {:?}", saved.gone);
    assert!(saved.enemies.iter().all(|enemy| enemy.id != dead_id));
    assert_eq!(saved.enemies.len() as u64, save::MAP_ENEMIES - 1);
    let mut fresh = harness_loaded(saved);
    fresh.step(2);
    let live: Vec<u64> = fresh
        .world()
        .query::<(&Enemy, &SaveId)>()
        .map(|(_, id)| id.0)
        .collect();
    assert!(!live.contains(&dead_id), "it stays dead");
    assert_eq!(live.len() as u64, save::MAP_ENEMIES - 1);
}

/// A fresh game that loads `save`.
fn harness_loaded(saved: save::SaveGame) -> UiHarness {
    let mut harness = harness();
    game_shell::start_run(harness.world_mut(), RunStart::from(saved), None);
    settle(&mut harness);
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    harness
}

#[test]
fn slot_zero_summons_each_enemy_after_the_dummy_capped() {
    let (mut harness, player) = arena();
    let stats = harness
        .world_mut()
        .get_mut::<tungsten::kit::Stats>(player)
        .unwrap();
    stats.set_base("max_mana", 10_000.0);
    stats.set_base("mana", 10_000.0);
    choose(&mut harness, Spell::Summon);
    let summoned = |h: &UiHarness| -> Vec<(u64, Entity)> {
        enemies::live(h.world())
            .into_iter()
            .filter(|&(id, _)| !enemies::placed(h.world(), id))
            .collect()
    };
    let cast_at = |h: &mut UiHarness, point: Vec2| {
        aim_at(h, point);
        h.harness_mut().press_action("cast");
        h.step(1);
        h.harness_mut().release_action("cast");
        h.step(61);
    };
    // Past the dummy, each enemy as it landed.
    let kinds: Vec<EnemyKind> = SUMMONS
        .iter()
        .filter_map(|summon| match summon.summoned {
            Summoned::Enemy(kind) => Some(kind),
            _ => None,
        })
        .collect();
    assert_eq!(kinds[..2], [EnemyKind::Wisp, EnemyKind::Crawler]);
    assert_eq!(SUMMONS[0].summoned, Summoned::Dummy, "the dummy first");
    for (i, kind) in kinds.iter().enumerate() {
        turn(&mut harness, SUMMON_NEXT);
        let point = Vec2::new(950.0 + 120.0 * i as f32, 700.0);
        cast_at(&mut harness, point);
        let mine = summoned(&harness);
        let (_, last) = *mine.last().expect("one summoned");
        assert_eq!(harness.world().get::<Enemy>(last).unwrap().kind, *kind);
    }
    let first = summoned(&harness)[0].1;
    // Past six summoned, the oldest goes.
    for i in 0..SUMMON_CAP {
        cast_at(&mut harness, Vec2::new(1000.0 + 100.0 * i as f32, 600.0));
    }
    assert_eq!(summoned(&harness).len(), SUMMON_CAP);
    assert!(!harness.world().is_alive(first), "the oldest went");
}

#[test]
fn a_frozen_body_draws_the_cube_fitted_to_its_hurtbox() {
    let (mut harness, _) = arena();
    let dummy = dummy_at(&mut harness, 900.0);
    let wisp = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(1100.0, 850.0));
    let crawler = walker(&mut harness, EnemyKind::Crawler, 1300.0, TOP);
    harness.step(2);
    let cubes = [
        ("ex10_status_ice_dummy", dummy),
        ("ex10_status_ice_wisp", wisp),
        ("ex10_status_ice_crawler", crawler),
    ];
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    for (i, (name, _)) in cubes.iter().enumerate() {
        mock_sprite(assets, name, 31 + i as u32, false);
    }
    for (_, body) in cubes {
        apply(&mut harness, body, "frozen", None);
    }
    harness.step(1);
    harness.world_mut().resource_mut::<CameraState>().position = Vec2::new(500.0, 500.0);
    let drawn: Vec<(u32, Vec2)> = overlays(harness.world())
        .into_iter()
        .filter(|batch| (31..34).contains(&batch.texture.0))
        .flat_map(|batch| {
            let handle = batch.texture.0;
            batch
                .instances
                .into_iter()
                .map(move |instance| (handle, Vec2::from(instance.position)))
        })
        .collect();
    for (i, (name, body)) in cubes.iter().enumerate() {
        let corner = harness
            .world()
            .get::<tungsten::core::Transform>(*body)
            .unwrap()
            .position;
        // The quad over the body's whole frame, by its corner or its centre.
        let over =
            |at: Vec2| at.distance(corner) < 1.0 || at.distance(corner + Vec2::splat(32.0)) < 1.0;
        assert!(
            drawn
                .iter()
                .any(|&(handle, at)| handle == 31 + i as u32 && over(at)),
            "{name} over its body at {corner}: {drawn:?}"
        );
    }
}

/// `entity`'s knockback.
fn knockback(harness: &UiHarness, entity: Entity) -> Knockback {
    harness
        .world()
        .get::<Knockback>(entity)
        .expect("a knockback")
        .clone()
}

/// Whether `walker`'s controller could jump: grounded, its input unlocked.
fn free(harness: &UiHarness, walker: Entity) -> bool {
    harness
        .world()
        .get::<PlatformerController>(walker)
        .is_some_and(PlatformerController::can_jump)
}

#[test]
fn the_wave_throws_each_kind_along_the_aim_the_troll_a_little() {
    // Each kind alone, its mind taken away, 1.5 tiles along a level aim
    // from the wave's caster: how far the wave throws it.
    let thrown = |kind: EnemyKind| {
        let (mut harness, _) = arena();
        harness.harness_mut().set_dt(1.0 / 60.0);
        let caster = Vec2::new(1000.0, TOP - 30.0);
        let x = caster.x + 1.5 * TILE;
        let enemy = if kind.info().flyer {
            enemy(&mut harness, kind, Vec2::new(x, caster.y))
        } else {
            walker(&mut harness, kind, x, TOP)
        };
        still(&mut harness, enemy);
        harness.step(10);
        let start = at(&harness, enemy).x;
        toys::wave::cast(harness.world_mut(), caster, Vec2::X);
        harness.step(60);
        at(&harness, enemy).x - start
    };
    for (kind, low, high) in [
        (EnemyKind::Crawler, 220.0, 265.0),
        (EnemyKind::Goblin, 185.0, 230.0),
        (EnemyKind::Beetle, 55.0, 75.0),
        (EnemyKind::Bat, 150.0, f32::INFINITY),
        (EnemyKind::Wisp, 150.0, f32::INFINITY),
        // Pushed back a little, past its resist.
        (EnemyKind::Troll, 24.0, 45.0),
    ] {
        let moved = thrown(kind);
        assert!(
            (low..=high).contains(&moved),
            "{kind:?} thrown {moved} px, not {low} to {high}"
        );
    }
}

#[test]
fn the_wave_pushes_once_and_never_behind() {
    let (mut harness, _) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let caster = Vec2::new(1000.0, TOP - 30.0);
    let ball = toys::spawn_ball(
        harness.world_mut(),
        Vec2::new(caster.x + 150.0, TOP - 15.0),
        false,
    );
    let behind = walker(&mut harness, EnemyKind::Crawler, caster.x - 1.5 * TILE, TOP);
    still(&mut harness, behind);
    harness.step(10);
    let start = at(&harness, behind);
    toys::wave::cast(harness.world_mut(), caster, Vec2::X);
    // The ball rolls with no friction: only a push changes its speed along.
    let mut last = speed(&harness, ball).x;
    let mut pushes = 0;
    for _ in 0..30 {
        harness.step(1);
        let now = speed(&harness, ball).x;
        pushes += usize::from((now - last).abs() > 1.0);
        last = now;
    }
    assert_eq!(pushes, 1, "one push, now {last} px/s");
    assert!(last > 100.0, "along the aim: {last}");
    assert!(
        !knockback(&harness, behind).is_active(),
        "nothing behind the caster"
    );
    assert!(at(&harness, behind).distance(start) < 0.5, "it stands");
}

#[test]
fn a_missile_blast_knocks_a_crawler_away() {
    let (mut harness, _) = arena();
    let crawler = walker(&mut harness, EnemyKind::Crawler, 1100.0, TOP);
    still(&mut harness, crawler);
    harness.step(10);
    let start = at(&harness, crawler);
    let whole = health(&harness, crawler);
    // Past the blast's heat, inside its push.
    toys::explode(harness.world_mut(), start - Vec2::new(150.0, 0.0));
    assert!(knockback(&harness, crawler).push().x > 0.0, "pushed away");
    harness.step(10);
    assert!(
        at(&harness, crawler).x > start.x + 5.0,
        "{}",
        at(&harness, crawler)
    );
    assert_eq!(health(&harness, crawler), whole, "out of the blast's heat");
}

#[test]
fn a_pushed_hit_leaves_the_troll_standing() {
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 1300.0, Difficulty::Normal);
    let crawler = walker(&mut harness, EnemyKind::Crawler, 900.0, TOP);
    still(&mut harness, troll);
    still(&mut harness, crawler);
    harness.step(10);
    let before = speed(&harness, troll);
    for target in [troll, crawler] {
        let world = harness.world_mut();
        Hit::new(target, [(Kind::Physical, 5.0)])
            .push(Vec2::new(600.0, -200.0))
            .request(world)
            .apply(world);
    }
    assert!(
        !knockback(&harness, troll).is_active(),
        "the troll's is idle"
    );
    assert!(
        knockback(&harness, crawler).is_active(),
        "the crawler's runs"
    );
    harness.step(1);
    assert_eq!(speed(&harness, troll), before, "its velocity unchanged");
    assert!(free(&harness, troll), "its input is not locked");
}

#[test]
fn an_enemy_thrown_out_of_the_level_is_gone_and_stays_gone_through_a_load() {
    let (mut harness, _) = run();
    let world = harness.world();
    let (enemy, id) = world
        .query::<(Entity, &Enemy, &SaveId)>()
        .map(|(entity, _, id)| (entity, id.0))
        .min_by_key(|&(_, id)| id)
        .expect("a placed enemy");
    let out = example_01_platformer::features::level::WORLD_MAX + Vec2::new(400.0, 400.0);
    harness.world_mut().get_mut::<Position>(enemy).unwrap().0 = out;
    harness.step(2);
    assert!(!harness.world().is_alive(enemy), "the level's edge took it");
    let saved = save::capture(harness.world()).expect("a live run captures");
    assert!(saved.gone.contains(&id), "gone in the save");
    let text = serde_json::to_value(&saved).expect("a save serializes");
    let loaded: save::SaveGame = serde_json::from_value(text).expect("it loads");
    let fresh = reload(loaded);
    assert!(by_id(&fresh, id).is_none(), "and stays gone");
}

#[test]
fn a_kill_plays_the_death_sound_not_the_hit() {
    let (mut harness, _) = arena();
    let crawler = walker(&mut harness, EnemyKind::Crawler, 1100.0, TOP);
    still(&mut harness, crawler);
    harness.step(2);
    let registry = harness.world().resource::<tungsten::core::SoundRegistry>();
    let (hit, death) = (
        registry.get_by_id("ex10_enemy_hit_sfx"),
        registry.get_by_id("ex10_enemy_death_sfx"),
    );
    let plays = |h: &mut UiHarness, frames: u32| -> (usize, usize) {
        let mut counts = (0, 0);
        for _ in 0..frames {
            h.step(1);
            for command in h.harness().audio() {
                if let AudioCommand::Play { handle, .. } = command {
                    counts.0 += usize::from(Some(*handle) == hit);
                    counts.1 += usize::from(Some(*handle) == death);
                }
            }
        }
        counts
    };
    // A blow that hurts: the hit alone.
    land(&mut harness, crawler, Kind::Physical, 10.0);
    assert_eq!(plays(&mut harness, 2), (1, 0), "the hit");
    harness.step(12);
    // The killing blow: its own hit, then the death, not the hit again.
    land(&mut harness, crawler, Kind::Physical, 500.0);
    assert!(harness.world().get::<Health>(crawler).unwrap().is_dead());
    assert_eq!(
        plays(&mut harness, 2),
        (1, 1),
        "the blow's hit and the death"
    );
}

/// The goblins' rocks and bombs alive: their save IDs, entities and
/// whether each is a bomb.
fn thrown(harness: &UiHarness) -> Vec<(u64, Entity, bool)> {
    let world = harness.world();
    let mut thrown: Vec<(u64, Entity, bool)> = world
        .query::<(Entity, &Rock, &SaveId)>()
        .map(|(entity, _, id)| (id.0, entity, false))
        .chain(
            world
                .query::<(Entity, &Bomb, &SaveId)>()
                .map(|(entity, _, id)| (id.0, entity, true)),
        )
        .collect();
    thrown.sort_unstable_by_key(|&(id, ..)| id);
    thrown
}

/// What a goblin's approach drew.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Approach {
    Dash,
    Rock,
    Bomb,
}

/// The first `count` approaches the kit's stream of `seed` draws for a
/// goblin, in turn: one draw each, a dash-in under 1/3, else a throw whose
/// tell's draw makes it a bomb under 0.2.
fn approaches(seed: u64, count: usize) -> Vec<Approach> {
    let mut rng = KitRng::seeded(seed);
    (0..count)
        .map(|_| {
            if rng.unit() < DASH_CHANCE {
                Approach::Dash
            } else if rng.unit() < BOMB_CHANCE {
                Approach::Bomb
            } else {
                Approach::Rock
            }
        })
        .collect()
}

/// A seed whose stream's first approach throws a rock and whose next
/// `bombs` throws hold a bomb, when `bombs` is more than 0.
fn goblin_seed(bombs: usize) -> u64 {
    (0..10_000)
        .find(|&seed| {
            let drawn = approaches(seed, 64);
            let throws: Vec<Approach> = drawn
                .iter()
                .copied()
                .filter(|&approach| approach != Approach::Dash)
                .take(bombs + 1)
                .collect();
            drawn[0] == Approach::Rock && (bombs == 0 || throws[1..].contains(&Approach::Bomb))
        })
        .expect("a seed")
}

/// A thrown thing's arc check: what it is, its entity, where it was first
/// seen, its velocity then, the gravity it falls by, and the steps since.
struct Arc {
    what: &'static str,
    entity: Entity,
    from: Vec2,
    launch: Vec2,
    gravity: Vec2,
    steps: u32,
}

impl Arc {
    /// Its place now against the arc: within 2 px, for its first twelve
    /// steps.
    fn check(&mut self, harness: &UiHarness) {
        let what = self.what;
        if self.steps >= 12 || !harness.world().is_alive(self.entity) {
            return;
        }
        self.steps += 1;
        let t = self.steps as f32 / 60.0;
        let arc = self.from + self.launch * t + 0.5 * self.gravity * t * t;
        let at = body(harness, self.entity);
        assert!(
            at.distance(arc) < 2.0,
            "{what} step {}: {at} off the arc {arc}",
            self.steps
        );
    }
}

#[test]
fn a_goblin_lobs_rocks_on_their_arc_and_one_throw_in_five_is_a_bomb() {
    let seed = goblin_seed(5);
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.world_mut().insert_resource(KitRng::seeded(seed));
    let goblin = walker(&mut harness, EnemyKind::Goblin, 620.0, TOP);
    harness.step(1);
    let gravity = harness
        .world()
        .resource::<tungsten::core::PhysicsConfig>()
        .gravity;
    let mut throws: Vec<(u64, bool)> = Vec::new();
    let (mut rock_arc, mut bomb_arc): (Option<Arc>, Option<Arc>) = (None, None);
    let (mut rock_hit, mut blast) = (false, false);
    for _ in 0..8000 {
        stand(&mut harness, player, 300.0, TOP);
        tungsten::kit::heal(harness.world_mut(), player, 100.0);
        harness.step(1);
        for arc in [&mut rock_arc, &mut bomb_arc].into_iter().flatten() {
            arc.check(&harness);
        }
        for (id, entity, bomb) in thrown(&harness) {
            if throws.iter().any(|&(seen, _)| seen == id) {
                continue;
            }
            throws.push((id, bomb));
            let world = harness.world();
            let slot = if bomb { &mut bomb_arc } else { &mut rock_arc };
            if slot.is_none() {
                *slot = Some(Arc {
                    what: if bomb { "a bomb" } else { "a rock" },
                    entity,
                    from: body(&harness, entity),
                    launch: world.get::<Velocity>(entity).unwrap().0,
                    gravity,
                    steps: 0,
                });
            }
        }
        let world = harness.world();
        for part in harness.harness().events::<tungsten::kit::DamagedPart>() {
            if part.periodic || part.source != Some(goblin) {
                continue;
            }
            let physical = part.kinds.has(world, "damage.physical");
            rock_hit |=
                part.entity == player && physical && (part.amount - ROCK_DAMAGE).abs() < 1e-3;
            blast |= part.entity == player && part.kinds.has(world, "damage.fire");
        }
        if throws.len() >= 12 && blast {
            break;
        }
    }
    assert!(throws.len() >= 12, "12 throws: {}", throws.len());
    let checked = |arc: &Option<Arc>| arc.as_ref().map_or(0, |arc| arc.steps);
    assert_eq!(checked(&rock_arc), 12, "a rock on its arc");
    assert_eq!(checked(&bomb_arc), 12, "a bomb on its arc");
    assert!(rock_hit, "8 physical on the courier, from the goblin");
    assert!(blast, "a bomb's detonation credited to the goblin");
    // Each approach draws its dash-in, each throw's tell its bomb.
    let drawn: Vec<bool> = approaches(seed, 64)
        .into_iter()
        .filter(|&approach| approach != Approach::Dash)
        .take(12)
        .map(|approach| approach == Approach::Bomb)
        .collect();
    let bombs: Vec<bool> = throws.iter().take(12).map(|&(_, bomb)| bomb).collect();
    assert_eq!(bombs, drawn, "the bombs are the stream's draws under 0.2");
}

#[test]
fn a_goblins_stab_hurts_a_courier_too_close_and_it_leaps_back() {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let goblin = walker(&mut harness, EnemyKind::Goblin, 620.0, TOP);
    harness.step(1);
    // The courier held 40 px off, within its reach: its stab's tell, its
    // stab.
    let mut stabbed = None;
    let mut told = false;
    for frame in 0..240 {
        let x = at(&harness, goblin).x - 40.0;
        stand(&mut harness, player, x, TOP);
        harness.step(1);
        told |= state(&harness, goblin) == EnemyState::WindUp
            && harness.world().get::<Brain>(goblin).unwrap().strike == Some(Strike::Stab);
        let hit = took(&harness, player, Some(goblin))
            .into_iter()
            .any(|amount| (amount - STAB_DAMAGE).abs() < 1e-3);
        if hit {
            stabbed = Some(frame);
            break;
        }
    }
    assert!(told, "its stab's tell");
    assert!(stabbed.is_some(), "a stab of {STAB_DAMAGE} from the goblin");
    // Then it leaps back, away from the courier, off the ground: about
    // 75 px at its leap's speed.
    let from = at(&harness, goblin).x;
    let mut airborne = false;
    for _ in 0..40 {
        harness.step(1);
        airborne |= !harness
            .world()
            .get::<tungsten::kit::PlatformerController>(goblin)
            .unwrap()
            .grounded();
    }
    let back = at(&harness, goblin).x - from;
    assert!(airborne, "it leapt");
    assert!(
        back > 0.1 * LEAP_BACK.x,
        "back {back} px, away from the courier"
    );
}

#[test]
fn a_goblin_hops_a_crate_in_its_way() {
    let (mut harness, _) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let goblin = walker(&mut harness, EnemyKind::Goblin, 1000.0, TOP);
    // A crate just ahead of it, the way it patrols.
    let crate_at = Vec2::new(1000.0 + 9.0 + 3.0 + CRATE_HALF.x, TOP - CRATE_HALF.y - 0.5);
    props::spawn(harness.world_mut(), PropKind::Crate, crate_at);
    harness.step(2);
    let past = crate_at.x + CRATE_HALF.x + 9.0;
    let mut airborne = false;
    let over = until(&mut harness, 240, |h| {
        airborne |= !h
            .world()
            .get::<tungsten::kit::PlatformerController>(goblin)
            .unwrap()
            .grounded();
        at(h, goblin).x > past
    });
    assert!(
        over && airborne,
        "over the crate by a hop: {}",
        at(&harness, goblin).x
    );
}

#[test]
fn a_goblin_keeps_its_distance() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    ground(&mut harness, TOP);
    stand(&mut harness, player, 900.0, TOP);
    harness.step(1);
    pin_camera(&mut harness, Vec2::new(500.0, 400.0));
    let goblin = walker(&mut harness, EnemyKind::Goblin, 1000.0, TOP);
    harness.step(1);
    let across =
        |h: &UiHarness| (at(h, goblin).x - courier::centre(h.world(), player).unwrap().x).abs();
    assert!(across(&harness) < 110.0);
    // The courier 100 px off: it backs away to its throw's distance.
    let backed = until(&mut harness, 120, |h| across(h) >= KEEP_OFF);
    assert!(backed, "backed off to {} within 2 s", across(&harness));
    // The courier 600 px off: it closes in.
    let x = at(&harness, goblin).x - 600.0;
    stand(&mut harness, player, x, TOP);
    let far = across(&harness);
    let closed = until(&mut harness, 180, |h| across(h) < far - 50.0);
    assert!(closed, "closed from {far} to {}", across(&harness));
}

#[test]
fn a_rock_from_a_goblin_killed_in_flight_still_lands_at_its_factor() {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness
        .world_mut()
        .insert_resource(KitRng::seeded(goblin_seed(0)));
    let kind = EnemyKind::Goblin;
    let centre = Vec2::new(620.0, TOP - kind.info().half.y - 0.5);
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let goblin = enemies::spawn(world, kind, centre - kind.info().centre, id).expect("its prefab");
    enemies::scale(world, goblin, Difficulty::Hard);
    harness.step(1);
    let threw = until(&mut harness, 300, |h| !thrown(h).is_empty());
    assert!(threw, "it throws");
    assert!(!thrown(&harness)[0].2, "a rock");
    land(&mut harness, goblin, Kind::Physical, 10_000.0);
    assert!(health(&harness, goblin) <= 0.0, "dead in flight");
    let hits = hits_over(&mut harness, player, 90);
    let rock: Vec<&(f32, Option<Entity>, u32)> = hits.iter().filter(|hit| hit.0 > 2.5).collect();
    assert_eq!(rock.len(), 1, "{hits:?}");
    assert!(
        (rock[0].0 - ROCK_DAMAGE * enemies::factor(Difficulty::Hard)).abs() < 1e-3,
        "at its factor: {}",
        rock[0].0
    );
    assert_eq!(rock[0].1, None, "from nobody");
}

/// A seed whose stream's first approaches go as `first` does.
fn seed_opening(first: &[Approach]) -> u64 {
    (0..10_000)
        .find(|&seed| approaches(seed, first.len()) == first)
        .expect("a seed")
}

/// A goblin at x 620 on the sky's ground, the kit's stream at `seed`, the
/// courier 320 px off at x 300, in its band.
fn goblin_in_band(seed: u64) -> (UiHarness, Entity, Entity) {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.world_mut().insert_resource(KitRng::seeded(seed));
    let goblin = walker(&mut harness, EnemyKind::Goblin, 620.0, TOP);
    harness.step(1);
    (harness, player, goblin)
}

/// Holds the courier at x 300, whole, and steps a frame.
fn held_step(harness: &mut UiHarness, player: Entity) {
    stand(harness, player, 300.0, TOP);
    tungsten::kit::heal(harness.world_mut(), player, 100.0);
    harness.step(1);
}

fn strike_of(harness: &UiHarness, enemy: Entity) -> Option<Strike> {
    harness
        .world()
        .get::<Brain>(enemy)
        .and_then(|brain| brain.strike)
}

#[test]
fn a_goblins_throws_come_at_least_its_cooldown_apart_with_no_tell_between() {
    let seed = seed_opening(&[Approach::Rock, Approach::Rock, Approach::Rock]);
    let (mut harness, player, goblin) = goblin_in_band(seed);
    let (mut throws, mut tells, mut was) = (Vec::new(), Vec::new(), state(&harness, goblin));
    for frame in 0..900 {
        held_step(&mut harness, player);
        if heard(&harness, Sound::RockThrown) > 0 {
            throws.push(frame);
        }
        let now = state(&harness, goblin);
        if now == EnemyState::WindUp && was != EnemyState::WindUp {
            tells.push(frame);
        }
        was = now;
        if throws.len() >= 3 {
            break;
        }
    }
    assert_eq!(throws.len(), 3, "three throws: {throws:?}");
    for pair in throws.windows(2) {
        let apart = (pair[1] - pair[0]) as f32 / 60.0;
        assert!(apart >= COOLDOWN, "{apart} s apart");
        let between = tells
            .iter()
            .filter(|&&tell| tell > pair[0] && tell < pair[1]);
        let early: Vec<&u32> = between
            .filter(|&&tell| ((tell - pair[0]) as f32 / 60.0) < COOLDOWN)
            .collect();
        assert!(early.is_empty(), "a tell while it cools: {early:?}");
    }
}

#[test]
fn a_goblins_dash_in_crouches_sprints_and_stabs_then_leaps_back() {
    let (mut harness, player, goblin) = goblin_in_band(seed_opening(&[Approach::Dash]));
    let (mut crouches, mut dashing, mut fastest) = (0, false, 0.0_f32);
    let mut stabbed = None;
    for frame in 0..240 {
        held_step(&mut harness, player);
        crouches += heard(&harness, Sound::GoblinCrouched);
        if state(&harness, goblin) == EnemyState::WindUp {
            assert_eq!(
                strike_of(&harness, goblin),
                Some(Strike::Dash),
                "a dash-in's tell"
            );
        }
        if state(&harness, goblin) == EnemyState::Attack
            && strike_of(&harness, goblin) == Some(Strike::Dash)
        {
            dashing = true;
            fastest = fastest.max(-speed(&harness, goblin).x);
        }
        let hit = took(&harness, player, Some(goblin))
            .into_iter()
            .any(|amount| (amount - STAB_DAMAGE).abs() < 1e-3);
        if hit {
            stabbed = Some(frame);
            break;
        }
    }
    assert_eq!(crouches, 1, "one crouch");
    assert!(dashing, "it dashes");
    assert!((fastest - DASH_SPEED).abs() < 10.0, "at {fastest} px/s");
    assert!(stabbed.is_some(), "a stab of {STAB_DAMAGE} that reaches");
    // Then the leap back, away from the courier.
    let from = at(&harness, goblin).x;
    let mut airborne = false;
    for _ in 0..40 {
        held_step(&mut harness, player);
        airborne |= !harness
            .world()
            .get::<PlatformerController>(goblin)
            .unwrap()
            .grounded();
    }
    assert!(airborne, "it leapt");
    assert!(at(&harness, goblin).x - from > 0.1 * LEAP_BACK.x, "back");
}

#[test]
fn a_goblins_dash_ins_are_the_streams_draws_under_a_third() {
    let seed = goblin_seed(3);
    let (mut harness, player, goblin) = goblin_in_band(seed);
    let (mut seen, mut was) = (Vec::new(), state(&harness, goblin));
    for _ in 0..12_000 {
        held_step(&mut harness, player);
        let now = state(&harness, goblin);
        if now == EnemyState::WindUp && was != EnemyState::WindUp {
            match strike_of(&harness, goblin) {
                Some(Strike::Dash) => seen.push(true),
                Some(Strike::Rock | Strike::Bomb) => seen.push(false),
                _ => {}
            }
        }
        was = now;
        if seen.len() >= 12 {
            break;
        }
    }
    assert_eq!(seen.len(), 12, "twelve approaches");
    let drawn: Vec<bool> = approaches(seed, 12)
        .into_iter()
        .map(|approach| approach == Approach::Dash)
        .collect();
    assert_eq!(seen, drawn, "the dash-ins are the stream's draws under 1/3");
    assert!(seen.contains(&true) && seen.contains(&false), "{seen:?}");
}

#[test]
fn a_goblin_set_burning_mid_tell_panics_from_the_courier_on_either_side() {
    for side in [1.0, -1.0] {
        let (mut harness, player) = arena();
        harness.harness_mut().set_dt(1.0 / 60.0);
        harness
            .world_mut()
            .insert_resource(KitRng::seeded(goblin_seed(0)));
        let courier = 1200.0 - side * 320.0;
        let goblin = walker(&mut harness, EnemyKind::Goblin, 1200.0, TOP);
        let hold = |harness: &mut UiHarness| {
            stand(harness, player, courier, TOP);
            tungsten::kit::heal(harness.world_mut(), player, 100.0);
            harness.step(1);
        };
        let told = (0..300).any(|_| {
            hold(&mut harness);
            state(&harness, goblin) == EnemyState::WindUp
        });
        assert!(told, "{side}: its throw's tell");
        apply(&mut harness, goblin, "burning", None);
        let start = (at(&harness, goblin).x - courier).abs();
        let (mut yelps, mut clips, mut panicked) = (0, Vec::new(), false);
        for _ in 0..60 {
            hold(&mut harness);
            yelps += heard(&harness, Sound::GoblinYelped);
            panicked |= state(&harness, goblin) == EnemyState::Panic;
            clips.push(
                harness
                    .world()
                    .get::<AnimationState>(goblin)
                    .unwrap()
                    .animation_id
                    .clone(),
            );
        }
        assert!(panicked, "{side}: it panics");
        assert_eq!(
            strike_of(&harness, goblin),
            None,
            "{side}: its strike spent"
        );
        assert_eq!(yelps, 1, "{side}: one yelp");
        assert!(
            clips
                .iter()
                .any(|clip| clip.starts_with("ex10_goblin_panic")),
            "{side}: {clips:?}"
        );
        let now = (at(&harness, goblin).x - courier).abs();
        assert!(now > start + 60.0, "{side}: it ran from {start} to {now}");
        // The fire out, it chases or searches.
        let ended = (0..300).any(|_| {
            hold(&mut harness);
            matches!(
                state(&harness, goblin),
                EnemyState::Chase | EnemyState::Search | EnemyState::WindUp
            )
        });
        assert!(ended, "{side}: past its panic once the fire is out");
    }
}

#[test]
fn a_goblin_set_burning_mid_dash_turns_away_at_once_and_stabs_no_one() {
    let (mut harness, player, goblin) = goblin_in_band(seed_opening(&[Approach::Dash]));
    let dashing = (0..240).any(|_| {
        held_step(&mut harness, player);
        state(&harness, goblin) == EnemyState::Attack
            && strike_of(&harness, goblin) == Some(Strike::Dash)
    });
    assert!(dashing, "its dash-in");
    held_step(&mut harness, player);
    assert!(speed(&harness, goblin).x < -100.0, "at the courier");
    apply(&mut harness, goblin, "burning", None);
    let mut away = false;
    for _ in 0..2 {
        held_step(&mut harness, player);
        away |= speed(&harness, goblin).x > 0.0;
    }
    assert!(away, "away from the courier within two steps");
    let stabs: Vec<f32> = (0..60)
        .flat_map(|_| {
            held_step(&mut harness, player);
            took(&harness, player, Some(goblin))
        })
        .filter(|&amount| (amount - STAB_DAMAGE).abs() < 1e-3)
        .collect();
    assert!(stabs.is_empty(), "no stab: {stabs:?}");
}

/// Steps until `troll` enters its attack, at most `frames`: the steps from
/// its tell's start to it, and the hits `target` takes from it meanwhile
/// and over the attack's first two steps. The courier is held on its spot
/// and healed, so the blows land again.
fn troll_blow(
    harness: &mut UiHarness,
    troll: Entity,
    target: Entity,
    x: f32,
    frames: u32,
) -> (Option<u32>, Vec<f32>) {
    let (mut tell, mut hits) = (None::<u32>, Vec::new());
    let mut steps = 0;
    for _ in 0..frames {
        stand(harness, target, x, TOP);
        tungsten::kit::heal(harness.world_mut(), target, 100.0);
        harness.step(1);
        hits.extend(took(harness, target, Some(troll)));
        match state(harness, troll) {
            EnemyState::WindUp if tell.is_none() => tell = Some(0),
            EnemyState::WindUp => {}
            EnemyState::Attack if tell.is_some() => {
                steps = tell.unwrap_or(0);
                for _ in 0..2 {
                    harness.step(1);
                    hits.extend(took(harness, target, Some(troll)));
                }
                return (Some(steps), hits);
            }
            _ => tell = None,
        }
        if let Some(count) = tell.as_mut()
            && state(harness, troll) == EnemyState::WindUp
        {
            *count += 1;
        }
    }
    (tell.map(|_| steps), hits)
}

#[test]
fn the_troll_walks_at_a_courier_on_either_side_and_slams_it() {
    for (from, courier) in [(1000.0, 600.0), (600.0, 1000.0)] {
        let (mut harness, player) = arena();
        harness.harness_mut().set_dt(1.0 / 60.0);
        stand(&mut harness, player, courier, TOP);
        let troll = troll_at(&mut harness, from, Difficulty::Normal);
        harness.step(1);
        let start = at(&harness, troll).x;
        let mut slammed = false;
        for _ in 0..600 {
            stand(&mut harness, player, courier, TOP);
            tungsten::kit::heal(harness.world_mut(), player, 100.0);
            harness.step(1);
            slammed |= !took(&harness, player, Some(troll)).is_empty();
            if slammed {
                break;
            }
        }
        // It closes to its slam's range, then the slam lands.
        let moved = at(&harness, troll).x - start;
        assert!(
            moved.signum() == (courier - from).signum() && moved.abs() > 200.0,
            "from {from}: walked {moved} px, not at the courier"
        );
        assert!(slammed, "from {from}: no blow");
    }
}

#[test]
fn the_troll_swipes_what_is_close_and_slams_what_is_farther() {
    // The courier 60 px ahead: the swipe, after its 0.3 s tell.
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let troll = troll_at(&mut harness, 360.0, Difficulty::Normal);
    harness.step(1);
    let (tell, hits) = troll_blow(&mut harness, troll, player, 300.0, 300);
    let tell = tell.expect("its swipe") as f32 / 60.0;
    assert!(
        (tell - troll::SWIPE_TELL).abs() <= 1.5 / 60.0,
        "a 0.3 s tell: {tell}"
    );
    assert_eq!(hits, [troll::SWIPE_DAMAGE], "the swipe's 18");
    // One 120 px ahead: the slam, after its 0.8 s tell.
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let troll = troll_at(&mut harness, 420.0, Difficulty::Normal);
    harness.step(1);
    let (tell, hits) = troll_blow(&mut harness, troll, player, 300.0, 300);
    let tell = tell.expect("its slam") as f32 / 60.0;
    assert!(
        (tell - troll::SLAM_TELL).abs() <= 1.5 / 60.0,
        "a 0.8 s tell: {tell}"
    );
    assert_eq!(hits, [40.0], "the slam's 40");
    // Its slam's cooldown running, it closes in and swipes.
    let (mut next, mut was) = (None, state(&harness, troll));
    for _ in 1..=200 {
        stand(&mut harness, player, 300.0, TOP);
        tungsten::kit::heal(harness.world_mut(), player, 100.0);
        harness.step(1);
        let now = state(&harness, troll);
        if now == EnemyState::Attack && was != EnemyState::Attack {
            next = harness
                .world()
                .get::<Brain>(troll)
                .and_then(|brain| brain.strike);
            break;
        }
        was = now;
    }
    assert_eq!(next, Some(Strike::Swipe), "a swipe, not a slam");
    // A crate in the swipe's box, kicked by its mass away from the troll.
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 900.0, Difficulty::Normal);
    still(&mut harness, troll);
    let heading = harness.world().get::<Enemy>(troll).unwrap().heading;
    let crate_at = Vec2::new(900.0 + 70.0 * heading, TOP - CRATE_HALF.y);
    let crate_ = props::spawn(harness.world_mut(), PropKind::Crate, crate_at);
    harness.step(2);
    let before = speed(&harness, crate_);
    let away = (body(&harness, crate_) - at(&harness, troll)).normalize();
    troll::swipe(harness.world_mut(), troll);
    let kick = (troll::KICK / 12.0).min(troll::KICK_MOST);
    let expected = before + away * kick + Vec2::new(0.0, -troll::KICK_RISE * kick);
    let kicked = speed(&harness, crate_);
    assert!(
        kicked.distance(expected) < 1e-3,
        "by its mass: {kicked} against {expected}"
    );
}

/// A troll on the sky's ground at x 1400 and a prop or body before its
/// left face, `gap` px off, both indexed, the player far behind, past its
/// sight and short of its dormancy.
fn troll_before(
    harness: &mut UiHarness,
    gap: f32,
    body: impl FnOnce(&mut UiHarness, f32) -> Entity,
) -> (Entity, Entity) {
    harness.harness_mut().set_dt(1.0 / 60.0);
    let troll = troll_at(harness, 1400.0, Difficulty::Normal);
    let face = 1400.0 - EnemyKind::Troll.info().half.x;
    let thing = body(harness, face - gap);
    harness.step(2);
    (troll, thing)
}

#[test]
fn a_chasing_troll_lifts_an_unlit_keg_and_throws_it_but_leaves_a_lit_one() {
    for lit in [false, true] {
        let (mut harness, player) = arena();
        harness.harness_mut().set_dt(1.0 / 60.0);
        let troll = troll_at(&mut harness, 900.0, Difficulty::Normal);
        let half = props::KEG_HALF;
        let keg = props::spawn(
            harness.world_mut(),
            PropKind::Keg,
            Vec2::new(1020.0, TOP - half.y - 0.5),
        );
        harness.step(2);
        if lit {
            assert!(props::light(harness.world_mut(), keg, None), "lit");
        }
        let start = body(&harness, keg);
        let (mut last, mut lifted, mut thrown, mut lighted) = (start, false, false, false);
        for _ in 0..120 {
            stand(&mut harness, player, 300.0, TOP);
            tungsten::kit::heal(harness.world_mut(), player, 100.0);
            harness.step(1);
            let world = harness.world();
            if !world.is_alive(keg) {
                break;
            }
            last = body(&harness, keg);
            lifted |= world.get::<Held>(keg).is_some_and(|hold| hold.by == troll);
            thrown |= world.resource::<Throws>().of(keg).is_some();
            lighted |= !lit && world.get::<Keg>(keg).is_some_and(|own| own.fuse.is_some());
        }
        if lit {
            assert!(!lifted, "it leaves a lit keg");
        } else {
            assert!(lifted && thrown, "it lifts the keg and throws it");
            assert!(!lighted, "never lit");
            assert!(last.x < start.x - 150.0, "at the player: {start} to {last}");
        }
    }
}

#[test]
fn a_searching_troll_lifts_the_crate_in_its_way_and_carries_it_past() {
    let (mut harness, player) = arena();
    stand(&mut harness, player, 300.0, TOP);
    let (troll, crate_) = troll_before(&mut harness, 60.0, |h, near| {
        let centre = Vec2::new(near - CRATE_HALF.x, TOP - CRATE_HALF.y - 0.5);
        props::spawn(h.world_mut(), PropKind::Crate, centre)
    });
    let start = body(&harness, crate_);
    // It searches toward a point past the crate.
    let world = harness.world_mut();
    world.get_mut::<Brain>(troll).unwrap().last_seen = Some([900.0, TOP - 60.0]);
    world.insert(troll, StateMachine::new(EnemyState::Search));
    let mut lifted = false;
    for _ in 0..300 {
        harness.step(1);
        lifted |= harness
            .world()
            .get::<Held>(crate_)
            .is_some_and(|hold| hold.by == troll);
        if at(&harness, troll).x < start.x - 20.0 {
            break;
        }
    }
    assert!(lifted, "it lifts the crate");
    assert!(
        at(&harness, troll).x < start.x,
        "past where it stood: {}",
        at(&harness, troll)
    );
}

#[test]
fn a_returning_troll_kicks_scrap_from_its_way_and_passes_it() {
    let (mut harness, player) = arena();
    stand(&mut harness, player, 300.0, TOP);
    let (troll, scrap) = troll_before(&mut harness, 60.0, |h, near| {
        let half = toys::SCRAP_HALF;
        let tumble = ScrapTumble {
            born_at: 0.0,
            initial_angle: 0.0,
            total_turn: 0.0,
        };
        let centre = Vec2::new(near - half.x, TOP - half.y - 0.5);
        toys::spawn_scrap(h.world_mut(), centre, Vec2::ZERO, 0, tumble)
    });
    let start = body(&harness, scrap);
    // It goes home, past the scrap.
    let world = harness.world_mut();
    world.get_mut::<Enemy>(troll).unwrap().home = [900.0, TOP - 55.5];
    world.insert(troll, StateMachine::new(EnemyState::Return));
    let mut kicks = 0;
    for _ in 0..600 {
        harness.step(1);
        kicks += heard(&harness, Sound::TrollSwiped);
        if at(&harness, troll).x < start.x - 20.0 {
            break;
        }
    }
    assert!(kicks >= 1, "it kicks the scrap");
    assert!(
        at(&harness, troll).x < start.x,
        "past where it lay: {}",
        at(&harness, troll)
    );
    assert!(harness.world().get::<Held>(scrap).is_none(), "never lifted");
}

#[test]
fn a_troll_sets_down_what_it_holds_back_on_patrol() {
    let (mut harness, player) = arena();
    stand(&mut harness, player, 300.0, TOP);
    let (troll, keg) = troll_before(&mut harness, 200.0, |h, near| {
        let half = props::KEG_HALF;
        let centre = Vec2::new(near - half.x, TOP - half.y - 0.5);
        props::spawn(h.world_mut(), PropKind::Keg, centre)
    });
    assert!(
        troll::pick_up(harness.world_mut(), troll, keg),
        "it holds the keg"
    );
    let world = harness.world_mut();
    world.get_mut::<Enemy>(troll).unwrap().home = [1300.0, TOP - 55.5];
    world.insert(troll, StateMachine::new(EnemyState::Return));
    let patrol = until(&mut harness, 300, |h| state(h, troll) == EnemyState::Patrol);
    assert!(patrol, "home, on patrol");
    let mut blasts = 0;
    for _ in 0..60 {
        harness.step(1);
        blasts += heard(&harness, Sound::KegBlasted);
    }
    assert!(harness.world().get::<Held>(keg).is_none(), "set down");
    assert_eq!(blasts, 0, "a fall too short to set it off");
    let half = props::KEG_HALF;
    assert!(
        (body(&harness, keg).y - (TOP - half.y)).abs() < 2.0,
        "on the ground: {}",
        body(&harness, keg)
    );
}

#[test]
fn a_keg_a_troll_threw_that_blows_credits_it() {
    let (mut harness, _) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let troll = troll_at(&mut harness, 900.0, Difficulty::Normal);
    still(&mut harness, troll);
    let half = props::KEG_HALF;
    let keg = props::spawn(
        harness.world_mut(),
        PropKind::Keg,
        Vec2::new(1000.0, TOP - half.y - 0.5),
    );
    // A dummy within its blast's reach.
    let dummy = dummy_at(&mut harness, 1020.0);
    harness.step(2);
    lift_and_throw(&mut harness, troll, keg);
    // Driven down hard from its hands, past the speed that sets a keg off.
    kick_at(&mut harness, keg, Vec2::new(0.0, 1500.0));
    let (mut blasts, mut sources) = (0, Vec::new());
    for _ in 0..30 {
        harness.step(1);
        blasts += heard(&harness, Sound::KegBlasted);
        sources.extend(
            harness
                .harness()
                .events::<tungsten::kit::DamagedPart>()
                .filter(|part| part.entity == dummy && !part.periodic)
                .map(|part| part.source),
        );
    }
    assert_eq!(blasts, 1, "it blew");
    assert!(!sources.is_empty(), "its blast reached the dummy");
    assert!(
        sources.iter().all(|&source| source == Some(troll)),
        "credited to the troll: {sources:?}"
    );
}

#[test]
fn a_walker_steps_over_balls_on_its_way() {
    // A beetle on patrol, the wizard 700 px off, past its sight: a crawler's
    // lunge would leap the balls.
    let (mut harness, _) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let beetle = walker(&mut harness, EnemyKind::Beetle, 1000.0, TOP);
    toys::spawn_ball(harness.world_mut(), Vec2::new(1050.0, TOP - 7.5), true);
    toys::spawn_ball(harness.world_mut(), Vec2::new(1140.0, TOP - 15.0), false);
    let half = EnemyKind::Beetle.info().half;
    let past = 1140.0 + 15.0 + half.x;
    let mut rose = 0.0_f32;
    let over = until(&mut harness, 300, |h| {
        rose = rose.max(TOP - half.y - at(h, beetle).y);
        at(h, beetle).x > past
    });
    assert!(
        over,
        "its centre passed both balls: {}",
        at(&harness, beetle).x
    );
    assert!(rose > 20.0, "it went over them: {rose} px up");
    let grounded = until(&mut harness, 30, |h| {
        h.world()
            .get::<PlatformerController>(beetle)
            .is_some_and(PlatformerController::grounded)
            && (at(h, beetle).y - (TOP - half.y)).abs() < 1.0
    });
    assert!(grounded, "on the ground again: {}", at(&harness, beetle));
}

#[test]
fn a_walker_leaves_crates_and_kegs_where_they_stand() {
    for (kind, half) in [
        (PropKind::Crate, CRATE_HALF),
        (PropKind::Keg, props::KEG_HALF),
    ] {
        let (mut harness, _) = arena();
        harness.harness_mut().set_dt(1.0 / 60.0);
        let beetle = walker(&mut harness, EnemyKind::Beetle, 1000.0, TOP);
        let front = 1000.0 + EnemyKind::Beetle.info().half.x;
        let centre = Vec2::new(front + 4.0 + half.x, TOP - half.y - 0.5);
        let prop = props::spawn(harness.world_mut(), kind, centre);
        harness.step(2);
        let start = body(&harness, prop);
        harness.step(240);
        let end = body(&harness, prop);
        assert!(
            start.distance(end) <= 1.0,
            "{kind:?} moved from {start} to {end}"
        );
        assert!(
            at(&harness, beetle).x + EnemyKind::Beetle.info().half.x <= end.x - half.x + 0.5,
            "the beetle stayed behind the {kind:?}: {}",
            at(&harness, beetle)
        );
    }
}

#[test]
fn a_hole_holds_a_bat_and_a_wisp_about_its_core() {
    for kind in [EnemyKind::Bat, EnemyKind::Wisp] {
        let (mut harness, _) = arena();
        harness.harness_mut().set_dt(1.0 / 60.0);
        let core = Vec2::new(1300.0, TOP - 300.0);
        let flyer = enemy(&mut harness, kind, core + Vec2::new(250.0, 0.0));
        harness.step(2);
        toys::place_black_hole(harness.world_mut(), core);
        let (mut caught, mut farthest) = (false, 0.0_f32);
        for _ in 0..(toys::HOLE_SECS * 60.0) as u32 - 2 {
            harness.step(1);
            let distance = at(&harness, flyer).distance(core);
            if caught {
                farthest = farthest.max(distance);
            }
            caught |= distance <= 60.0;
        }
        assert!(caught, "{kind:?} came within 60 px of the core");
        assert!(
            farthest <= 100.0,
            "{kind:?} held within 100 px, not flung: {farthest}"
        );
    }
}

#[test]
fn a_bat_diving_through_a_hole_dives_on_its_own_way_after() {
    let (mut harness, player) = arena();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let wizard = courier::centre(harness.world(), player).expect("a body");
    let bat = enemy(
        &mut harness,
        EnemyKind::Bat,
        wizard + Vec2::new(240.0, -240.0),
    );
    harness.step(2);
    // A hole by the dive's line, 21 px off it.
    let hole = toys::place_black_hole(harness.world_mut(), wizard + Vec2::new(150.0, -120.0));
    let start = at(&harness, bat);
    let dive = (wizard - start).normalize();
    harness
        .world_mut()
        .get_mut::<StateMachine<EnemyState>>(bat)
        .expect("a machine")
        .go(EnemyState::Attack);
    let direction = |h: &UiHarness| {
        h.world()
            .get::<TopDownMover>(bat)
            .expect("its mover")
            .direction()
    };
    let mut bent = 0.0_f32;
    for _ in 0..8 {
        harness.step(1);
        assert!(
            direction(&harness).distance(dive) < 1e-4,
            "its own way kept: {}",
            direction(&harness)
        );
        bent = bent.max(
            speed(&harness, bat)
                .normalize_or_zero()
                .angle_to(dive)
                .abs(),
        );
    }
    assert!(bent > 0.1, "the hole bent its dive: {bent} rad");
    toys::systems::collapse(harness.world_mut(), hole);
    harness.step(1);
    assert_eq!(state(&harness, bat), EnemyState::Attack, "still diving");
    let flight = speed(&harness, bat).normalize_or_zero();
    assert!(
        flight.distance(dive) < 1e-3,
        "on its dive's own way: {flight} against {dive}"
    );
}
