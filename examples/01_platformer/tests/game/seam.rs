//! The seam the shell calls: a run's start, `capture` and the bindable
//! actions.

use std::collections::BTreeSet;

use example_01_platformer::features::level::MAP;
use example_01_platformer::features::player::MAX_HEALTH;
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::save;
use example_01_platformer::shell::Difficulty;
use tungsten::core::{CameraController, CameraRig, TilemapInstance};
use tungsten::kit::{Health, PlatformerController};

use super::{harness, player_entity, press, start, top};

#[test]
fn a_new_game_at_normal_spawns_the_player_on_the_map_with_a_camera_rig() {
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
    let rig = world.get::<CameraRig>(player).expect("the player's rig");
    assert!(rig.is_live());
    assert_eq!(rig.target(), None, "a follow of its own entity");
    assert_eq!(
        world.resource::<CameraController>().winner(),
        Some(player),
        "the camera follows the player"
    );
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
    let map = tungsten::core::ActionMap::load(std::path::Path::new("input.json"))
        .expect("input.json loads");
    let labelled: Vec<(&str, &str)> = map
        .labelled()
        .iter()
        .map(|(action, label)| (action.as_str(), label.as_str()))
        .collect();
    for (action, label) in &labelled {
        assert!(declared.contains(action), "{action} is in input.json");
        assert!(
            !action.starts_with("ui_") && !action.starts_with("engine_"),
            "{action}"
        );
        assert!(!label.is_empty());
    }
    let listed: BTreeSet<&str> = labelled.iter().map(|(action, _)| *action).collect();
    assert_eq!(listed, declared, "every input.json action is bindable");
}
