//! The playthrough: a New Game at Normal on the full game, headless at dt
//! 1/60. Scripted moves and jumps walk the `main` route from the spawn
//! through all five transitions to the troll bridge's east end, jumping the
//! hazards in the way; the run names each area in order and autosaves once
//! in each, and the player ends alive with gameplay on top.

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
const AREAS: [&str; 6] = [
    "Lantern clearing",
    "Broken aqueduct",
    "Waterfall ravine",
    "Ember hollow",
    "Iron mine",
    "Troll bridge",
];

/// Each launch of the `main` route: the platform it lands on, and where it
/// starts and aims, in tiles: the level test's `LAUNCHES` rows, which a map
/// without hazards found.
const MAIN: &[(&str, f32, f32)] = &[
    ("terrace_a", 15.85, 16.75),
    ("terrace_b", 18.35, 23.25),
    ("meadow", 23.85, 24.75),
    ("bridge_a", 35.85, 36.75),
    ("bridge_b", 51.85, 52.75),
    ("bridge_c", 65.85, 66.75),
    ("east_rim", 80.85, 84.75),
    ("aqueduct_step", 91.85, 92.75),
    ("aqueduct_floor", 94.85, 95.75),
    ("ravine_step_1", 157.85, 158.75),
    ("ravine_step_2", 165.85, 166.75),
    ("ravine_step_3", 169.85, 170.75),
    ("ravine_step_4", 173.85, 174.75),
    ("pool_bed", 176.85, 177.75),
    ("cleft", 205.85, 206.75),
    ("hollow_floor_1", 215.85, 216.75),
    ("stepping_stone", 231.85, 235.75),
    ("hollow_floor_2", 236.85, 240.75),
    ("hollow_step", 249.85, 250.75),
    ("hollow_shelf_a", 253.85, 254.75),
    ("hollow_shelf_b", 259.85, 262.75),
    ("gallery", 267.85, 268.75),
    ("cut_floor_1", 282.85, 283.75),
    ("cut_floor_2", 305.35, 306.75),
    ("cut_floor_3", 323.35, 324.75),
    ("cut_step_1", 341.35, 342.75),
    ("cut_step_2", 344.35, 345.75),
    ("cut_step_3", 347.35, 348.75),
    ("ridge", 350.85, 351.75),
    ("bridge_head", 359.85, 360.75),
    ("troll_bridge", 365.85, 366.75),
    ("east_bank", 423.85, 424.75),
];

/// Frames each launch waits at its start, standing, and the last before the
/// walk to the bridge's end: found once by replaying the route at dt 1/60,
/// a wait at a time, until no hazard touched the courier.
const WAITS: [u32; 33] = [0; 33];

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
        // A launch row jumps each lava basin.
        world
            .query::<(&Hazard, &Transform)>()
            .filter(|(hazard, _)| !hazard.lava)
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
fn a_new_game_walks_the_main_route_to_the_troll_bridges_end() {
    let text = std::fs::read_to_string("tools/level.json").expect("tools/level.json reads");
    let level: Source = serde_json::from_str(&text).expect("tools/level.json parses");
    let route = &level.routes["main"];
    assert_eq!(
        route[1..].iter().map(String::as_str).collect::<Vec<_>>(),
        MAIN.iter().map(|row| row.0).collect::<Vec<_>>(),
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
    for (&(to, start_x, goal), &wait) in MAIN.iter().zip(&WAITS) {
        walker.walk_to(start_x);
        walker.wait(wait);
        assert!(
            walker.launch(goal, platform(to)),
            "landed on {to} at {} ({} HP)",
            walker.centre() / TILE,
            walker.health()
        );
    }
    // On along the east bank to its far end, under the map's east wall.
    let end = platform(route.last().expect("the route ends"));
    walker.wait(WAITS[MAIN.len()]);
    walker.walk_to(end.right - 1.0);
    let at = walker.centre();
    assert!(
        walker.grounded()
            && (at.y + PLAYER_HALF.y - end.row * TILE).abs() < 1.0
            && at.x > end.left * TILE,
        "at the bridge's end: {}",
        at / TILE
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
