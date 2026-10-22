//! The game across frame-rate regimes on the default 60 Hz step: a walk
//! along the rope bridge's deck over the ball pit at y = 2304, one merged
//! collider over its tiles' slabs, and every collision event reaching a
//! fixed reader before and after `physics_step` exactly once.

use example_01_platformer::features::player::PLAYER_HALF;
use example_01_platformer::game;
use glam::Vec2;
use tungsten::core::{CollisionEvent, Entity, EventQueue, UserFolder, World};
use tungsten::plugins::PHYSICS_STEP;
use tungsten::testing::UiHarness;
use tungsten::{Stage, system};

use super::{app, centre, player_entity, start, teleport};

/// Frame dts: two steps a frame, one, and mostly none.
const REGIMES: [(&str, f32); 4] = [
    ("1/30 s", 1.0 / 30.0),
    ("1/60 s", 1.0 / 60.0),
    ("1/144 s", 1.0 / 144.0),
    ("1/1000 s", 1.0 / 1000.0),
];
/// The top of the deck the walk crosses, from x 2304, where its tiles'
/// slabs meet at 2368, 2432, 2496, 2560 and 2624.
const ROW_TOP: f32 = 2304.0;
const START_X: f32 = 2328.0;
/// Past the last join it crosses: the player's right edge, x + 20, beyond
/// 2624 + 4.
const PAST_LAST_JOIN: f32 = 2608.0;
const SETTLE_SECS: f32 = 0.1;
const WALK_SECS: f32 = 0.6;

/// A collision event as bits, so two reads compare exactly.
type Key = (Entity, Option<Entity>, u32, u32, u32);

fn key(event: &CollisionEvent) -> Key {
    (
        event.a,
        event.b,
        event.normal.x.to_bits(),
        event.normal.y.to_bits(),
        event.penetration.to_bits(),
    )
}

/// What the three readers saw, each in the order it read.
#[derive(Default)]
struct Probe {
    /// A `fixed_update` reader before `physics_step`, through `iter`.
    before: Vec<Key>,
    /// A `fixed_update` reader after `physics_step`, through
    /// `iter_current`, as a contact reader reads.
    after: Vec<Key>,
    /// Every event of each frame, read in `post_update`.
    sent: Vec<Key>,
    /// Each step's own events, as the after-reader counted them.
    step_counts: Vec<usize>,
}

fn collision_keys(world: &World, current: bool) -> Vec<Key> {
    let queue = world.resource::<EventQueue<CollisionEvent>>();
    if current {
        queue.iter_current().map(key).collect()
    } else {
        queue.iter().map(key).collect()
    }
}

fn read_before(world: &mut World) {
    let keys = collision_keys(world, false);
    world.resource_mut::<Probe>().before.extend(keys);
}

fn read_after(world: &mut World) {
    let keys = collision_keys(world, true);
    let probe = world.resource_mut::<Probe>();
    probe.step_counts.push(keys.len());
    probe.after.extend(keys);
}

fn read_sent(world: &mut World) {
    let keys = collision_keys(world, true);
    world.resource_mut::<Probe>().sent.extend(keys);
}

/// One regime's run: its report line and its failed checks.
fn run(name: &str, dt: f32) -> (String, Vec<String>) {
    let mut app = app(UserFolder::Off);
    app.world_mut().insert_resource(Probe::default());
    app.add_system_to(
        Stage::FixedUpdate,
        system("regime_before", read_before).before(PHYSICS_STEP),
    );
    app.add_system_to(
        Stage::FixedUpdate,
        system("regime_after", read_after).after(PHYSICS_STEP),
    );
    app.add_system_to(Stage::PostUpdate, system("regime_sent", read_sent));
    let mut harness = UiHarness::new(app);
    harness
        .harness_mut()
        .load_manifest(game::MANIFEST)
        .expect("the manifest loads");
    harness.step(1);
    start(&mut harness);
    harness.harness_mut().set_dt(dt);
    let player = player_entity(&harness);
    let rest_y = ROW_TOP - PLAYER_HALF.y;
    teleport(&mut harness, player, Vec2::new(START_X, rest_y - 0.5));
    let frames = |secs: f32| (secs / dt).round() as u32;
    harness.step(frames(SETTLE_SECS));
    harness.harness_mut().press_action("move_right");
    let mut far_x = f32::MIN;
    for _ in 0..frames(WALK_SECS) {
        harness.step(1);
        let at = centre(&harness, player);
        if (at.y - rest_y).abs() < 1.0 {
            far_x = far_x.max(at.x);
        }
    }

    let probe = harness.world().resource::<Probe>();
    let mut failures = Vec::new();
    let seams = far_x >= PAST_LAST_JOIN;
    if !seams {
        failures.push(format!(
            "{name}: seams: the player stopped at x = {far_x} on the row, short of {PAST_LAST_JOIN}"
        ));
    }
    let after = probe.after == probe.sent;
    if !after {
        failures.push(format!(
            "{name}: the reader after physics_step read {} events of {} sent, not each once",
            probe.after.len(),
            probe.sent.len()
        ));
    }
    // The last steps' events wait for the next step's earlier readers.
    let pending: usize = probe.step_counts.iter().rev().take(2).sum();
    let before =
        probe.sent.starts_with(&probe.before) && probe.sent.len() - probe.before.len() <= pending;
    if !before {
        failures.push(format!(
            "{name}: the reader before physics_step read {} events of {} sent, not each once",
            probe.before.len(),
            probe.sent.len()
        ));
    }
    let verdict = |ok: bool| if ok { "ok" } else { "FAIL" };
    let line = format!(
        "regime {name}: {} events, after {}, before {}, walk to x = {far_x} (seams {})",
        probe.sent.len(),
        verdict(after),
        verdict(before),
        verdict(seams),
    );
    (line, failures)
}

#[test]
fn every_regime_walks_the_slab_joins_and_reads_each_collision_once() {
    let mut failures = Vec::new();
    for (name, dt) in REGIMES {
        let (line, failed) = run(name, dt);
        println!("{line}");
        failures.extend(failed);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
