//! The playthrough: a New Game at Normal on the full game, headless at dt
//! 1/60. Scripted moves and jumps walk the `lower` route from the spawn to
//! the tower, then on into the collection pit, jumping the hazards in the
//! way; the run names each area in order and autosaves once in each, and
//! the player ends alive with gameplay on top.

use example_01_platformer::features::level::TILE;
use example_01_platformer::features::level::components::Hazard;
use example_01_platformer::features::player::{self, PLAYER_HALF};
use example_01_platformer::game;
use example_01_platformer::save::SaveGame;
use example_01_platformer::shell::AUTOSAVES;
use example_01_platformer::shell::toast::Toasts;
use glam::Vec2;
use serde::Deserialize;
use tungsten::core::{Entity, SaveSlots, Transform, UserFolder};
use tungsten::kit::{Health, PlatformerController};
use tungsten::testing::UiHarness;

use super::{calm, harness_with, player_entity, start, top, user_folder};

/// The areas the route crosses, in order.
const AREAS: [&str; 4] = [
    "Lantern clearing",
    "Broken aqueduct",
    "Waterfall ravine",
    "Ruined tower",
];

/// Each launch of the `lower` route: the platform it lands on, and where
/// it starts and aims, in tiles: the level test's `LAUNCHES` rows, which a
/// map without hazards found, but for the short ledge with spikes before
/// lower_c, which the courier lands on short of the spikes and leaves at
/// once, jumping them.
const LOWER: &[(&str, f32, f32)] = &[
    ("terrace_a", 15.85, 16.75),
    ("terrace_b", 18.35, 23.25),
    ("lower_a", 23.85, 24.75),
    ("lower_b", 31.85, 34.75),
    ("aqueduct_step", 37.85, 38.75),
    ("lower_b_exit", 40.85, 41.35),
    ("lower_c", f32::NAN, 46.75),
    ("climb_a", 55.85, 56.75),
    ("climb_b", 60.85, 62.75),
    ("climb_c", 66.85, 68.75),
    ("climb_d", 72.85, 74.75),
    ("climb_e", 78.85, 80.75),
    ("climb_f", 84.85, 86.75),
    ("tower_lower_a", 91.85, 92.75),
    ("tower_lower_b", 96.85, 98.75),
    ("tower_lower_c", 102.85, 104.75),
    ("gate", 107.85, 108.75),
];

/// Frames each launch waits at its start, standing, and the last before the
/// pit: found once by replaying the route at dt 1/60, a wait at a time,
/// until no hazard touched the courier. Lower_c's lets the moving deck over
/// the ledge and the bobbing fire pass; tower_lower_b's times the walk
/// under the fire that sweeps it.
const WAITS: [u32; 18] = [0, 0, 0, 0, 0, 0, 105, 0, 0, 0, 0, 0, 0, 0, 105, 0, 0, 0];

/// `tools/level.json`, as far as the playthrough reads it.
#[derive(Deserialize)]
struct Source {
    platforms: Vec<Platform>,
    routes: std::collections::BTreeMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct Platform {
    name: String,
    left: f32,
    right: f32,
    row: f32,
}

/// What the run has shown and saved so far: the toasts in order, the area
/// of each autosave written, and each autosave slot's last save time.
#[derive(Default)]
struct Watch {
    toasts: Vec<String>,
    autosaves: Vec<Option<String>>,
    saved_at: [u64; 3],
}

/// The courier on its way: the run, its player, what it has been shown and
/// saved, and the frames it has taken.
struct Walker {
    harness: UiHarness,
    player: Entity,
    watch: Watch,
    frames: u32,
}

impl Walker {
    fn centre(&self) -> Vec2 {
        player::centre(self.harness.world(), self.player).expect("the player has a body")
    }

    fn health(&self) -> f32 {
        self.harness
            .world()
            .get::<Health>(self.player)
            .map_or(0.0, Health::current)
    }

    fn grounded(&self) -> bool {
        self.harness
            .world()
            .get::<PlatformerController>(self.player)
            .is_some_and(PlatformerController::grounded)
    }

    fn set(&mut self, action: &str, down: bool) {
        let input = self.harness.harness_mut();
        if down {
            input.press_action(action);
        } else {
            input.release_action(action);
        }
    }

    /// Steps a frame and records what it toasted and autosaved.
    fn step(&mut self) {
        self.harness.step(1);
        self.frames += 1;
        assert!(
            self.frames < 60 * 120,
            "the playthrough took over two minutes: at {} ({} HP)",
            self.centre(),
            self.health()
        );
        let world = self.harness.world();
        let tree = self.harness.tree();
        for text in world.resource::<Toasts>().texts(tree) {
            if self.watch.toasts.last() != Some(&text) {
                self.watch.toasts.push(text);
            }
        }
        // The autosave slot that changed, each area's taking the oldest.
        let slots = world.resource::<SaveSlots>();
        let list = slots.list().unwrap_or_default();
        for (at, slot) in AUTOSAVES.into_iter().enumerate() {
            let info = list.slots.iter().find(|info| info.name == slot);
            let written = info.map_or(0, |info| info.saved_at_ms);
            if written != self.watch.saved_at[at] {
                self.watch.saved_at[at] = written;
                let save = slots.load::<SaveGame>(slot).expect("the autosave reads");
                self.watch.autosaves.push(save.area);
            }
        }
    }

    /// Whether a hazard's touch box lies ahead of the player along
    /// `direction`, low and close enough that a jump now clears it.
    fn hazard_ahead(&self, direction: f32) -> bool {
        let at = self.centre();
        let world = self.harness.world();
        // The spikes have no body, so their drawn point places them.
        world
            .query::<(&Hazard, &Transform)>()
            .any(|(hazard, point)| {
                let (offset, half) = if hazard.fire {
                    (Vec2::new(32.0, 32.0), Vec2::new(18.0, 24.0))
                } else {
                    (Vec2::new(32.0, 48.0), Vec2::new(27.0, 14.0))
                };
                let centre = point.position + offset;
                let ahead = (centre.x - at.x) * direction;
                let near = ahead > -(half.x + PLAYER_HALF.x) && ahead < half.x + 1.4 * TILE;
                // In the way: from a little above the body's centre down past
                // its feet, where spikes sit; a fire hovering higher passes over
                // a walk, and a jump would meet it.
                let dy = centre.y - at.y;
                let level = dy > -(half.y + 16.0) && dy < PLAYER_HALF.y + TILE;
                near && level
            })
    }

    /// Walks on the platform underfoot to `x` tiles, jumping the hazards in
    /// the way, and stops there, or against the wall before it: a launch
    /// row's start may lie in the step up it jumps.
    fn walk_to(&mut self, x: f32) {
        if x.is_nan() {
            return;
        }
        let mut airborne = 0;
        let mut blocked = 0;
        let mut last = self.centre().x;
        loop {
            let dx = x * TILE - self.centre().x;
            let close = dx.abs() <= 560.0 / 60.0 * 0.6;
            if (close || blocked >= 3) && self.grounded() || self.health() <= 0.0 {
                break;
            }
            let direction = dx.signum();
            self.set("move_right", !close && direction > 0.0);
            self.set("move_left", !close && direction < 0.0);
            let jump = self.grounded() && self.hazard_ahead(direction);
            self.set("jump", jump || (airborne > 0 && airborne < 20));
            airborne = if jump {
                1
            } else if airborne > 0 {
                airborne + 1
            } else {
                0
            };
            if self.grounded() && airborne > 20 {
                airborne = 0;
            }
            self.step();
            let now = self.centre().x;
            blocked = if (now - last).abs() < 0.5 && airborne == 0 {
                blocked + 1
            } else {
                0
            };
            last = now;
        }
        for action in ["move_left", "move_right", "jump"] {
            self.set(action, false);
        }
        // Still, as a launch row starts.
        for _ in 0..4 {
            self.step();
        }
    }

    /// Stands still `frames` frames.
    fn wait(&mut self, frames: u32) {
        for _ in 0..frames {
            self.step();
        }
    }

    /// Launches as a route row does: jump held through the rise, steering
    /// toward `goal` tiles, until it stands on `to`. Whether it did.
    fn launch(&mut self, goal: f32, to: &Platform) -> bool {
        for tick in 0..(4.0 * 60.0) as u32 {
            let dx = goal * TILE - self.centre().x;
            self.set("move_left", false);
            self.set("move_right", false);
            if dx.abs() > 560.0 / 60.0 * 0.6 {
                self.set(if dx > 0.0 { "move_right" } else { "move_left" }, true);
            }
            if tick == 0 {
                self.set("jump", true);
            }
            self.step();
            let at = self.centre();
            let on = (at.y + PLAYER_HALF.y - to.row * TILE).abs() < 1.0
                && at.x >= to.left * TILE
                && at.x <= to.right * TILE;
            if tick > 3 && self.grounded() && on || self.health() <= 0.0 {
                break;
            }
        }
        for action in ["move_left", "move_right", "jump"] {
            self.set(action, false);
        }
        let at = self.centre();
        (at.y + PLAYER_HALF.y - to.row * TILE).abs() < 1.0 && self.grounded()
    }
}

#[test]
fn a_new_game_walks_the_lower_route_into_the_collection_pit() {
    let text = std::fs::read_to_string("tools/level.json").expect("tools/level.json reads");
    let level: Source = serde_json::from_str(&text).expect("tools/level.json parses");
    let route = &level.routes["lower"];
    assert_eq!(
        route[1..].iter().map(String::as_str).collect::<Vec<_>>(),
        LOWER.iter().map(|row| row.0).collect::<Vec<_>>(),
        "the script follows the route"
    );
    let platform = |name: &str| {
        level
            .platforms
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no platform {name}"))
    };
    let dir = user_folder("playthrough");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    calm(&mut harness);
    let player = player_entity(&harness);
    let mut walker = Walker {
        harness,
        player,
        watch: Watch::default(),
        frames: 0,
    };
    for (&(to, start_x, goal), &wait) in LOWER.iter().zip(&WAITS) {
        walker.walk_to(start_x);
        walker.wait(wait);
        assert!(
            walker.launch(goal, platform(to)),
            "landed on {to} at {}",
            walker.centre() / TILE
        );
    }
    // On along the gate and its apron, then down into the pit.
    let apron = platform("pit_apron");
    walker.walk_to(apron.left + 1.0);
    walker.wait(WAITS[LOWER.len()]);
    let pit = platform("pit_floor");
    walker.set("move_right", true);
    for _ in 0..(3 * 60) {
        walker.step();
        let at = walker.centre();
        if walker.grounded() && (at.y + PLAYER_HALF.y - pit.row * TILE).abs() < 1.0 {
            break;
        }
    }
    walker.set("move_right", false);
    for _ in 0..30 {
        walker.step();
    }
    let at = walker.centre();
    assert!(
        (at.y + PLAYER_HALF.y - pit.row * TILE).abs() < 1.0 && at.x > pit.left * TILE,
        "in the pit: {at}"
    );
    let world = walker.harness.world();
    let health = world
        .get::<Health>(walker.player)
        .expect("the player lives");
    assert!(!health.is_dead(), "the player is alive");
    assert_eq!(top(&walker.harness), Some(game::GAMEPLAY));
    assert_eq!(walker.watch.toasts, AREAS, "each area's toast, in order");
    let areas: Vec<&str> = walker
        .watch
        .autosaves
        .iter()
        .map(|area| area.as_deref().unwrap_or("-"))
        .collect();
    assert_eq!(areas, AREAS, "one autosave per area");
    println!(
        "playthrough: {} frames, {} HP left, toasts {:?}",
        walker.frames,
        health.current(),
        walker.watch.toasts
    );
    let _ = std::fs::remove_dir_all(dir);
}
