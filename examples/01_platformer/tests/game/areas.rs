//! The areas: walking across each one's start toasts its name and
//! autosaves once, and a Retry after a death starts in the last.

use std::path::Path;

use example_01_platformer::features::level::TILE;
use example_01_platformer::features::player::PLAYER_HALF;
use example_01_platformer::features::run::Run;
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shell::AUTOSAVES;
use example_01_platformer::shell::game_over::GAME_OVER;
use example_01_platformer::shell::toast::Toasts;
use glam::Vec2;
use tungsten::core::UserFolder;
use tungsten::kit::damage;
use tungsten::testing::UiHarness;

use super::{
    centre, harness_with, player_entity, press, settle, start, teleport, top, user_folder,
};

/// Each area after the spawn's, its first column, and where the walk into
/// it starts: a column on the platform before its start, that platform's
/// row, and whether the walk jumps.
const CROSSINGS: [(&str, f32, f32, f32, bool); 3] = [
    ("Broken aqueduct", 24.0, 23.0, 32.0, false),
    ("Waterfall ravine", 56.0, 54.5, 36.0, true),
    ("Ruined tower", 92.0, 90.0, 18.0, false),
];
/// The spawn's area, which a run's first step enters.
const FIRST: &str = "Lantern clearing";

/// Each autosave's bytes, `None` without a file.
fn autosaves(dir: &Path) -> [Option<Vec<u8>>; 3] {
    AUTOSAVES.map(|slot| std::fs::read(dir.join("saves").join(format!("{slot}.json"))).ok())
}

fn area(harness: &UiHarness) -> Option<String> {
    harness.world().resource::<Run>().area.clone()
}

/// Steps one frame and adds the toasts it showed to `seen`.
fn step(harness: &mut UiHarness, seen: &mut Vec<String>) {
    let before = harness.world().resource::<Toasts>().texts(harness.tree());
    harness.step(1);
    let after = harness.world().resource::<Toasts>().texts(harness.tree());
    seen.extend(after.into_iter().filter(|text| !before.contains(text)));
}

#[test]
fn walking_across_each_area_start_autosaves_once_toasts_it_and_retry_starts_there() {
    let dir = user_folder("areas");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    let player = player_entity(&harness);
    // The run's first steps, as the curtain lifts, enter the spawn's area.
    harness.step(10);
    let shown = harness.world().resource::<Toasts>().texts(harness.tree());
    assert_eq!(shown, [FIRST], "the spawn's area names itself");
    assert_eq!(area(&harness).as_deref(), Some(FIRST));
    let mut seen = Vec::new();
    for (name, column, x, row, jump) in CROSSINGS {
        teleport(
            &mut harness,
            player,
            Vec2::new(x * TILE, row * TILE - PLAYER_HALF.y - 0.5),
        );
        for _ in 0..10 {
            harness.step(1);
        }
        seen.clear();
        let (mut bytes, mut writes) = (autosaves(&dir), 0);
        harness.harness_mut().press_action("move_right");
        if jump {
            harness.harness_mut().press_action("jump");
        }
        // In, then half a second for the autosave to land.
        let mut after = None;
        for frame in 0..120 {
            step(&mut harness, &mut seen);
            let now = autosaves(&dir);
            writes += now.iter().zip(&bytes).filter(|(a, b)| a != b).count();
            bytes = now;
            if after.is_none() && area(&harness).as_deref() == Some(name) {
                after = Some(frame);
                for action in ["move_right", "jump"] {
                    harness.harness_mut().release_action(action);
                }
            }
            if after.is_some_and(|entered| frame >= entered + 30) {
                break;
            }
        }
        assert_eq!(area(&harness).as_deref(), Some(name));
        assert!(centre(&harness, player).x + PLAYER_HALF.x > column * TILE);
        assert_eq!(seen, [name], "one toast crossing into {name}");
        assert_eq!(writes, 1, "one autosave crossing into {name}");
    }
    // A death in the tower: Retry loads the autosave taken on its start.
    let saved = centre(&harness, player);
    damage(harness.world_mut(), player, 1000.0);
    harness.step(2);
    settle(&mut harness);
    assert_eq!(top(&harness), Some(GAME_OVER));
    press(&mut harness, "Retry");
    settle(&mut harness);
    assert_eq!(top(&harness), Some(GAMEPLAY));
    let player = player_entity(&harness);
    harness.step(5);
    let tower = CROSSINGS[2];
    let at = centre(&harness, player);
    // Its body overlaps the tower's start, where the autosave stood; which
    // area a body astride two names is the save's to say (step 7).
    assert!(
        at.x + PLAYER_HALF.x > tower.1 * TILE,
        "{at} is in {}",
        tower.0
    );
    assert!(
        at.x <= saved.x,
        "it starts where the tower's autosave stood"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
