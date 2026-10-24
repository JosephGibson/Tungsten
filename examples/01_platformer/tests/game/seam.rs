//! The seam the shell calls: a run's start, `capture` and the bindable
//! actions.

use std::collections::BTreeSet;

use example_01_platformer::features::level::MAP;
use example_01_platformer::features::player::MAX_HEALTH;
use example_01_platformer::game::{BINDABLE_ACTIONS, GAMEPLAY};
use example_01_platformer::save;
use example_01_platformer::shell::Difficulty;
use tungsten::core::{CameraController, CameraMode, Entity, TilemapInstance};
use tungsten::kit::{CameraTarget, Health, PlatformerController};

use super::{harness, player_entity, press, start, top};

#[test]
fn a_new_game_at_normal_spawns_the_player_on_the_map_with_a_camera_target() {
    let mut harness = harness();
    press(&mut harness, "New Game");
    press(&mut harness, "Normal");
    assert_eq!(top(&harness), Some(GAMEPLAY));
    harness.step(2);
    let world = harness.world();
    let maps: Vec<&str> = world
        .query::<&TilemapInstance>()
        .map(|map| map.id.as_str())
        .collect();
    assert_eq!(maps, [MAP]);
    let player = player_entity(&harness);
    let health = world.get::<Health>(player).expect("the player has health");
    assert_eq!((health.current(), health.max()), (MAX_HEALTH, MAX_HEALTH));
    assert!(world.get::<PlatformerController>(player).is_some());
    assert!(world.get::<CameraTarget>(player).is_some());
    let followed = match world.resource::<CameraController>().mode {
        CameraMode::Follow(entity) => Some(entity),
        _ => None::<Entity>,
    };
    assert_eq!(followed, Some(player), "the camera follows the player");
}

#[test]
fn capture_is_none_with_no_run_and_a_save_in_one() {
    let mut harness = harness();
    assert!(
        save::capture(harness.world()).is_none(),
        "the title has no run"
    );
    start(&mut harness);
    let saved = save::capture(harness.world()).expect("a live run saves");
    assert_eq!(saved.meta.difficulty, Difficulty::Normal);
    assert_eq!(saved.player.health.current(), MAX_HEALTH);
}

#[test]
fn bindable_actions_are_input_jsons_and_no_menu_or_engine_action() {
    let text = std::fs::read_to_string("input.json").expect("input.json reads");
    let file: serde_json::Value = serde_json::from_str(&text).expect("input.json parses");
    let declared: BTreeSet<&str> = file["actions"]
        .as_object()
        .expect("an actions table")
        .keys()
        .map(String::as_str)
        .collect();
    for (action, label) in BINDABLE_ACTIONS {
        assert!(declared.contains(action), "{action} is in input.json");
        assert!(
            !action.starts_with("ui_") && !action.starts_with("engine_"),
            "{action}"
        );
        assert!(!label.is_empty());
    }
    let listed: BTreeSet<&str> = BINDABLE_ACTIONS.iter().map(|(action, _)| *action).collect();
    assert_eq!(listed, declared, "every input.json action is bindable");
}
