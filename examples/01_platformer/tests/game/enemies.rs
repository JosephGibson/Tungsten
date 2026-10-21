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
//! map enemy killed for good; the summon slot's enemies; and the frozen
//! cube fitted to each body.

use example_01_platformer::features::audio::components::Music;
use example_01_platformer::features::audio::{COMBAT_IN, COMBAT_OUT, MUSIC_COMBAT};
use example_01_platformer::features::combat::draw::overlays;
use example_01_platformer::features::combat::{Hit, Kind};
use example_01_platformer::features::elements;
use example_01_platformer::features::elements::systems::crush;
use example_01_platformer::features::elements::{
    BODY_HIT_K, BODY_HIT_MOST, BODY_HIT_SPEED, CRUSH_SPEED,
};
use example_01_platformer::features::enemies::bat::{DIVE_SECS, DIVE_SPEED, FLASH_SECS};
use example_01_platformer::features::enemies::beetle::{CHARGE_SPEED, PAW_SECS};
use example_01_platformer::features::enemies::components::{
    Brain, Enemy, EnemyKind, EnemyState, Shown, Sight,
};
use example_01_platformer::features::enemies::crawler::{
    LANTERN_OFFSET, LANTERN_REACH, LUNGE_SPEED,
};
use example_01_platformer::features::enemies::drops::{self, ORB_SECS, Orb, OrbKind};
use example_01_platformer::features::enemies::{self, ARRIVED, SUMMON_CAP, troll};
use example_01_platformer::features::gun::components::Bullet;
use example_01_platformer::features::gun::{self, SHOT_DAMAGE};
use example_01_platformer::features::lightning;
use example_01_platformer::features::player::components::Lantern;
use example_01_platformer::features::spells::{SUMMON_NEXT, SUMMONS};
use example_01_platformer::features::toys::IRON_HALF;
use example_01_platformer::features::toys::components::PreviousVelocity;
use example_01_platformer::features::{player as courier, toys};
use example_01_platformer::game;
use example_01_platformer::save;
use example_01_platformer::shared::{SaveId, SaveIds};
use example_01_platformer::shell::{self as game_shell, Difficulty, RunStart};
use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, AudioCommand, Bundle, CameraState, Collider, Entity, EventQueue, Position,
    RigidBodyBundle, Time, Velocity,
};
use tungsten::kit::{
    AilmentThreshold, ContactDamage, Damaged, Health, Hitbox, KitRng, PlatformerController,
    StateMachine, TopDownMover,
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
            let rule = (BODY_HIT_K * mass * (closing - BODY_HIT_SPEED)).min(BODY_HIT_MOST);
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
    assert!(sees(&other, clear), "a crate's layer is not looked through");
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
        assert_eq!(
            world.get::<Health>(player).unwrap().max(),
            courier::max_health(difficulty),
            "the player's own"
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
            format!(
                "{} health · enemies ×{factor}",
                courier::max_health(difficulty)
            )
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
    // Five seconds, then it thaws and flies again.
    assert!(until(&mut harness, 360, |h| !tagged(
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
        world.get::<tungsten::core::Light>(parts.light).is_some(),
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
fn the_crawler_keeps_out_of_the_lanterns_light() {
    let (mut harness, player) = arena();
    let crawler = walker(&mut harness, EnemyKind::Crawler, 640.0, TOP);
    let light =
        |h: &UiHarness| h.world().get::<Position>(player_entity(h)).unwrap().0 + LANTERN_OFFSET;
    let mut nearest = f32::MAX;
    for _ in 0..240 {
        harness.step(1);
        nearest = nearest.min(at(&harness, crawler).distance(light(&harness)));
    }
    assert!(
        nearest >= LANTERN_REACH - 1.0,
        "outside the light: {nearest}"
    );
    assert!(
        nearest <= LANTERN_REACH + 24.0,
        "held at its edge: {nearest}"
    );
    assert!(
        harness.world().get::<Enemy>(crawler).unwrap().heading < 0.0,
        "facing the player"
    );
    // With the lantern out, it comes in.
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
    let came = until(&mut harness, 240, |h| {
        at(h, crawler).distance(light(h)) < LANTERN_REACH - 30.0
    });
    assert!(came, "the dark lets it in");
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
/// player's lantern out, so it lunges.
fn stalker() -> (UiHarness, Entity, Entity) {
    let (mut harness, player) = arena();
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = false;
    let crawler = walker(&mut harness, EnemyKind::Crawler, 560.0, TOP);
    (harness, player, crawler)
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
    assert_eq!(chilled, [12.0], "its touch, 12 cold");
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
    assert!(until(&mut harness, 300, |h| !tagged(
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
    // A strike beside it takes it as its rod.
    let aim = at(&harness, beetle) + Vec2::new(60.0, -40.0);
    let landing = lightning::strike(harness.world_mut(), player, aim);
    assert!(
        landing.distance(body(&harness, beetle)) < 1.0,
        "on the beetle: {landing}"
    );
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
fn the_troll_slams_after_a_long_wind_up_hurting_scattering_and_shattering() {
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
        "a 1.2 s wind-up: {tell}"
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
    let kicked = speed(&harness, ball);
    assert!(
        kicked.x > 100.0 && kicked.y < 0.0,
        "the ball scattered: {kicked}"
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

#[test]
fn a_save_while_the_troll_holds_iron_loads_with_the_hold() {
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 1300.0, Difficulty::Normal);
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
    assert_eq!(move_speed(&harness, troll), 35.0, "the hole slows it");
    harness.step(58);
    // A crawler's hold would take it past 200 px in this second.
    assert!(
        at(&harness, troll).distance(start) < 20.0,
        "the pull barely moves it: {}",
        at(&harness, troll)
    );
    harness.world_mut().despawn(hole);
    harness.step(2);
    assert_eq!(move_speed(&harness, troll), 70.0, "its speed back");
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
fn the_troll_slows_in_stages_and_freezes_only_past_a_high_threshold() {
    let (mut harness, _) = arena();
    let troll = troll_at(&mut harness, 1300.0, Difficulty::Normal);
    still(&mut harness, troll);
    harness.step(2);
    let cold = |harness: &mut UiHarness, amount: f32| {
        assert!(Hit::new(troll, [(Kind::Cold, amount)]).queue(harness.world_mut()));
        harness.step(2);
    };
    assert_eq!(move_speed(&harness, troll), 70.0);
    cold(&mut harness, 15.0);
    assert!(tagged(&harness, troll, "status.chilled"), "chilled");
    let first = move_speed(&harness, troll);
    assert!(
        (first - 70.0 * 0.65).abs() < 0.01,
        "the first stage: {first}"
    );
    // Past the others' freezing needs, it is not frozen.
    cold(&mut harness, 60.0);
    assert!(!tagged(&harness, troll, "status.frozen"));
    assert!((move_speed(&harness, troll) - first).abs() < 0.01);
    cold(&mut harness, 30.0);
    let second = move_speed(&harness, troll);
    assert!(
        (second - 70.0 * (1.0 - 2.0 * troll::DEEP_SLOW)).abs() < 0.01,
        "the second stage: {second}"
    );
    assert!(!tagged(&harness, troll, "status.frozen"));
    // Past four times the needs, a short freeze.
    cold(&mut harness, 80.0);
    assert!(tagged(&harness, troll, "status.frozen"), "frozen at last");
    let mut frames = 0;
    assert!(until(&mut harness, 200, |h| {
        frames += 1;
        !tagged(h, troll, "status.frozen")
    }));
    let held = frames as f32 / 60.0;
    assert!(held <= 1.6, "a short freeze: {held} s");
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
    // 120 more, past four times.
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

/// The live orbs of `kind`.
fn orbs(harness: &UiHarness, kind: OrbKind) -> Vec<Entity> {
    harness
        .world()
        .query::<(Entity, &Orb)>()
        .filter(|(_, orb)| orb.kind == kind)
        .map(|(entity, _)| entity)
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
    choose(&mut harness, 9);
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
    let kinds: Vec<EnemyKind> = SUMMONS.iter().filter_map(|summon| summon.enemy).collect();
    assert_eq!(kinds[..2], [EnemyKind::Wisp, EnemyKind::Crawler]);
    assert!(SUMMONS[0].enemy.is_none(), "the dummy first");
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
