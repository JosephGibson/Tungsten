//! The game across frame-rate regimes on the default 60 Hz step (the 0.57
//! fixed-step health pass): a walk over the slab joins of the y = 1920 row,
//! every collision event reaching a fixed reader before and after
//! `physics_step` exactly once, and the HUD's contact count.

use super::*;
use tungsten::physics::PrevPosition;
use tungsten::plugins::PHYSICS_STEP;
use tungsten::{Stage, system};

/// Frame dts: two steps a frame, one, and mostly none.
const REGIMES: [(&str, f32); 4] = [
    ("1/30 s", 1.0 / 30.0),
    ("1/60 s", 1.0 / 60.0),
    ("1/144 s", 1.0 / 144.0),
    ("1/1000 s", 1.0 / 1000.0),
];
/// The top of the slab row the walk crosses: six slabs from x = 1536 to
/// 1920, joined at 1600, 1664, 1728, 1792 and 1856.
const ROW_TOP: f32 = 1920.0;
const START_X: f32 = 1560.0;
/// Past the last join: the player's right edge, x + 20, beyond 1856 + 4.
const PAST_LAST_JOIN: f32 = 1860.0;
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
    /// `iter_current`, as the game's contact readers read.
    after: Vec<Key>,
    /// Every event of each frame, read in `post_update`.
    sent: Vec<Key>,
    /// Each step's own events, as the after-reader counted them.
    step_counts: Vec<usize>,
}

fn collision_keys(world: &World, current: bool) -> Vec<Key> {
    let queue = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
    if current {
        queue.iter_current().map(key).collect()
    } else {
        queue.iter().map(key).collect()
    }
}

fn probe_mut(world: &mut World) -> &mut Probe {
    world.get_resource_mut::<Probe>().unwrap()
}

fn read_before(world: &mut World) {
    let keys = collision_keys(world, false);
    probe_mut(world).before.extend(keys);
}

fn read_after(world: &mut World) {
    let keys = collision_keys(world, true);
    let probe = probe_mut(world);
    probe.step_counts.push(keys.len());
    probe.after.extend(keys);
}

fn read_sent(world: &mut World) {
    let keys = collision_keys(world, true);
    probe_mut(world).sent.extend(keys);
}

/// One regime's run: its report line and its failed checks.
fn run(name: &str, dt: f32) -> (String, Vec<String>) {
    let mut app = App::new(Config::default()).expect("App::new failed");
    crate::setup::configure_app(&mut app);
    let world = app.world_mut();
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("ex10_level".into(), level_map());
    world.insert_resource(Probe::default());
    app.add_system_to(
        Stage::FixedUpdate,
        system("regime_before", read_before).before(PHYSICS_STEP),
    );
    app.add_system_to(
        Stage::FixedUpdate,
        system("regime_after", read_after).after(PHYSICS_STEP),
    );
    app.add_system_to(Stage::PostUpdate, system("regime_sent", read_sent));
    let mut harness = Harness::new(app);
    harness.set_dt(dt);
    harness.step(1);

    let player = harness
        .world()
        .query::<(Entity, &Player)>()
        .next()
        .map(|(e, _)| e)
        .expect("the player is seeded");
    let rest_y = ROW_TOP - PLAYER_HALF.y;
    let start = Vec2::new(START_X, rest_y - 0.5);
    let world = harness.world_mut();
    world.get_mut::<Position>(player).unwrap().0 = start;
    world.get_mut::<PrevPosition>(player).unwrap().0 = start;
    world.get_mut::<Velocity>(player).unwrap().0 = Vec2::ZERO;
    let frames = |secs: f32| (secs / dt).round() as u32;
    harness.step(frames(SETTLE_SECS));
    harness.press_action("move_right");

    let mut far_x = f32::MIN;
    let mut refreshes = 0;
    let mut bad_contacts = Vec::new();
    let mut timer = harness
        .world()
        .get_resource::<TextDisplayState>()
        .unwrap()
        .timer;
    for _ in 0..frames(WALK_SECS) {
        let counted = harness
            .world()
            .get_resource::<Probe>()
            .unwrap()
            .step_counts
            .len();
        harness.step(1);
        let world = harness.world();
        let at = world.get::<Position>(player).unwrap().0;
        let on_row = (at.y - rest_y).abs() < 1.0;
        if on_row {
            far_x = far_x.max(at.x);
        }
        let hud = world.get_resource::<TextDisplayState>().unwrap();
        // The timer only drops when the HUD refreshes.
        if hud.timer < timer && on_row {
            refreshes += 1;
            // One step's events: this frame's steps or the one before them.
            let counts = &world.get_resource::<Probe>().unwrap().step_counts;
            let one_step = &counts[counted.saturating_sub(1)..];
            if hud.contacts == 0 || !one_step.contains(&hud.contacts) {
                bad_contacts.push(format!("{} (steps {one_step:?})", hud.contacts));
            }
        }
        timer = hud.timer;
    }

    let probe = harness.world().get_resource::<Probe>().unwrap();
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
    let contacts = refreshes > 0 && bad_contacts.is_empty();
    if !contacts {
        failures.push(format!(
            "{name}: Contacts read {bad_contacts:?} over {refreshes} refreshes on the row, not one step's events"
        ));
    }
    let verdict = |ok: bool| if ok { "ok" } else { "FAIL" };
    let line = format!(
        "regime {name}: {} events, after {}, before {}, contacts {} ({refreshes} refreshes), walk to x = {far_x} (seams {})",
        probe.sent.len(),
        verdict(after),
        verdict(before),
        verdict(contacts),
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
