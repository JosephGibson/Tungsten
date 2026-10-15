//! The game on the engine's headless harness: no window or renderer, the
//! frames stepped one call at a time. A test per system.

use std::path::{Path, PathBuf};

use tungsten::core::assets::manifest::ResolvedManifest;
use tungsten::core::{
    ActionMap, AssetRegistry, Binding, Config, Settings, Sprite, Time, Transform, UserFolder, With,
};
use tungsten::testing::Harness;
use tungsten::{App, SceneEntity};
use tungsten_template_basic::components::Player;
use tungsten_template_basic::game::{self, active_state};
use tungsten_template_basic::states::{
    AUTOSAVE, GAMEPLAY, PAUSE, PLAYER_PREFAB, PLAYER_START, TITLE,
};

/// The game's config with its user folder set: `Off` in tests, so they
/// never read or write the developer's own settings and saves.
fn config(user_folder: UserFolder) -> Config {
    let mut config = Config::load("tungsten.json").expect("tungsten.json loads");
    config.game.user_folder = user_folder;
    config
}

/// The game as `main.rs` builds it, with one frame stepped: its `Startup`
/// stage runs `setup`, which requests the title state, and the engine's
/// dispatcher applies it in the same frame's `pre_update`. The harness loads
/// no manifest, so the prefabs load from it here, as a window's startup
/// loads them.
fn harness() -> Harness {
    harness_with(UserFolder::Off)
}

fn harness_with(user_folder: UserFolder) -> Harness {
    let mut app = App::new(config(user_folder)).expect("App::new failed");
    game::register(&mut app);
    let mut harness = Harness::new(app);
    let manifest = ResolvedManifest::load(game::MANIFEST).expect("the manifest loads");
    tungsten::asset_loader::load_prefabs(&manifest, harness.world_mut()).expect("the prefabs load");
    harness.step(1);
    harness
}

/// Presses `action` for a frame, then steps the frame that applies the
/// state change it asked for.
fn tap(harness: &mut Harness, action: &str) {
    harness.press_action(action);
    harness.step(1);
    harness.release_action(action);
    harness.step(1);
}

fn player(harness: &Harness) -> Option<[f32; 2]> {
    harness
        .world()
        .query_filtered::<&Transform, With<Player>>()
        .next()
        .map(|transform| [transform.position.x, transform.position.y])
}

/// A fresh user folder for one persistence test.
fn user_folder(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tungsten-template-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The resolved schedule as `register` builds it: the game's systems in
/// their stages between the engine's. A reorder shows up here as a diff.
const SCHEDULE: &str = "\
startup: setup
pre_update: physics_debug_toggle, systems_overlay_toggle, inspector_toggle, hud_toggle, ui_debug_toggle, ui_inspector_toggle, log_console_toggle, display_input, ui_state_roots, ui_route, inspector_pick, ui_animate, state_dispatcher
fixed_update: physics_prev_snapshot, physics_step
update: player_movement, toggle_mute
post_update: physics_sync, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, camera_update
";

#[test]
fn the_schedule_matches_the_snapshot() {
    let mut app = App::new(config(UserFolder::Off)).expect("App::new failed");
    game::register(&mut app);
    app.resolve_schedule().expect("the schedule resolves");
    assert_eq!(app.schedule().resolved_text(), SCHEDULE);
}

#[test]
fn data_files_load() {
    let config = Config::load("tungsten.json").expect("tungsten.json loads");
    // Config::load validates the ID; copied games choose their own name.
    assert!(
        config.game.id.is_some(),
        "the game config needs an explicit ID"
    );
    let actions = ActionMap::load(Path::new("input.json")).expect("input.json loads");
    for action in ["move_left", "move_right", "move_up", "move_down"] {
        assert!(
            !actions.bindings(action).is_empty(),
            "{action} has no binding"
        );
        assert!(
            actions.bindings(action).iter().any(|binding| matches!(
                binding,
                Binding::GamepadAxis { .. } | Binding::GamepadButton { .. }
            )),
            "{action} has no gamepad binding"
        );
    }
    let manifest = ResolvedManifest::load(game::MANIFEST).expect("the manifest loads");
    assert!(manifest.sprites["player"].path.is_file());
    assert!(manifest.fonts[game::FONT].path.is_file());
    assert!(manifest.prefabs[PLAYER_PREFAB].path.is_file());
}

#[test]
fn state_start_starts_gameplay_with_the_player() {
    let mut harness = harness();
    assert_eq!(active_state(harness.world()), Some(TITLE));
    assert_eq!(player(&harness), None);
    tap(&mut harness, "state_start");
    assert_eq!(active_state(harness.world()), Some(GAMEPLAY));
    assert_eq!(player(&harness), Some(PLAYER_START));
    // The prefab's sprite, and gameplay owns the player.
    let world = harness.world();
    let (sprite, owner) = world
        .query_filtered::<(&Sprite, &SceneEntity), With<Player>>()
        .next()
        .expect("the player spawned");
    let assets = world.resource::<AssetRegistry>();
    assert_eq!(assets.sprite_name(sprite.asset_id), Some("player"));
    assert_eq!(owner.state_id, GAMEPLAY);
}

#[test]
fn player_moves_under_move_right_only_in_gameplay() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    harness.press_action("move_right");
    harness.step(30);
    let [x, y] = player(&harness).expect("the player spawned");
    let expected = PLAYER_START[0] + game::PLAYER_SPEED * 30.0 / 60.0;
    assert!((x - expected).abs() < 1e-3, "x = {x}, expected {expected}");
    assert!((y - PLAYER_START[1]).abs() < f32::EPSILON);
    harness.release_action("move_right");

    tap(&mut harness, "state_pause");
    harness.press_action("move_right");
    harness.step(10);
    assert_eq!(
        player(&harness).map(|[x, _]| x),
        Some(x),
        "paused, the player stays"
    );
}

#[test]
fn a_gamepad_stick_moves_the_player() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    // The harness presses an action's first binding: make it the stick's.
    let actions = harness.world_mut().resource_mut::<ActionMap>();
    let stick: Vec<Binding> = actions
        .bindings("move_right")
        .iter()
        .copied()
        .filter(|binding| matches!(binding, Binding::GamepadAxis { .. }))
        .collect();
    assert_eq!(stick.len(), 1, "move_right has one stick binding");
    actions.replace_bindings("move_right", stick);
    harness.press_action("move_right");
    harness.step(30);
    let [x, y] = player(&harness).expect("the player spawned");
    let expected = PLAYER_START[0] + game::PLAYER_SPEED * 30.0 / 60.0;
    assert!((x - expected).abs() < 1e-3, "x = {x}, expected {expected}");
    assert!((y - PLAYER_START[1]).abs() < f32::EPSILON);
}

#[test]
fn clock_half_scale_moves_the_player_half_as_far() {
    let mut moved = Vec::new();
    for scale in [1.0, 0.5] {
        let mut harness = harness();
        tap(&mut harness, "state_start");
        // A scale set between frames applies from the next one.
        harness
            .world_mut()
            .get_resource_mut::<Time>()
            .expect("the app inserts Time")
            .set_scale(scale);
        harness.press_action("move_right");
        harness.step(30);
        let [x, _] = player(&harness).expect("the player spawned");
        moved.push(x - PLAYER_START[0]);
    }
    let (full, half) = (moved[0], moved[1]);
    assert!(
        (full - game::PLAYER_SPEED * 30.0 / 60.0).abs() < 1e-3,
        "full {full}"
    );
    assert!((half - full / 2.0).abs() < 1e-3, "half {half}, full {full}");
}

#[test]
fn state_pause_pauses_and_state_back_resumes() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    tap(&mut harness, "state_pause");
    assert_eq!(active_state(harness.world()), Some(PAUSE));
    // Gameplay and its player stay under the pause.
    assert!(player(&harness).is_some());
    tap(&mut harness, "state_back");
    assert_eq!(active_state(harness.world()), Some(GAMEPLAY));
    assert_eq!(player(&harness), Some(PLAYER_START));
}

#[test]
fn text_follows_the_state_on_top() {
    let mut harness = harness();
    let texts = |harness: &Harness| -> Vec<String> {
        game::text(harness.world())
            .into_iter()
            .map(|section| section.content)
            .collect()
    };
    assert!(texts(&harness)[0].contains("Tungsten template"));
    tap(&mut harness, "state_start");
    assert!(texts(&harness)[0].contains("WASD"));
    tap(&mut harness, "state_pause");
    assert_eq!(texts(&harness)[0], "Paused");
    let draw = harness.draw().expect("a completed frame");
    assert!(
        draw.text
            .iter()
            .any(|section| section.font_id == game::FONT)
    );
}

#[test]
fn mute_survives_a_restart() {
    let dir = user_folder("mute");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(harness.world().resource::<Settings>().master_volume(), None);
    tap(&mut harness, "toggle_mute");
    assert_eq!(
        harness.world().resource::<Settings>().master_volume(),
        Some(0.0)
    );
    assert!(dir.join("settings/settings.json").is_file());

    let mut restarted = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(
        restarted.world().resource::<Settings>().master_volume(),
        Some(0.0),
        "the mute is back after a restart"
    );
    tap(&mut restarted, "toggle_mute");
    assert_eq!(
        restarted.world().resource::<Settings>().master_volume(),
        None
    );
    let restarted = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(
        restarted.world().resource::<Settings>().master_volume(),
        None
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn continue_from_the_autosave() {
    let dir = user_folder("autosave");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    tap(&mut harness, "state_start");
    harness.press_action("move_right");
    harness.step(30);
    harness.release_action("move_right");
    let moved = player(&harness).expect("the player spawned");
    assert_ne!(moved, PLAYER_START);
    tap(&mut harness, "state_pause");
    assert!(dir.join("saves").join(format!("{AUTOSAVE}.json")).is_file());

    let mut restarted = harness_with(UserFolder::At(dir.clone()));
    tap(&mut restarted, "state_start");
    assert_eq!(player(&restarted), Some(moved), "gameplay continues");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_slot_that_fails_to_load_is_never_overwritten() {
    let dir = user_folder("newer-save");
    let slot = dir.join("saves").join(format!("{AUTOSAVE}.json"));
    std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
    // A save from a newer build of the game: schema 2.
    let newer = r#"{ "envelope": 1, "slot": "autosave", "schema": 2, "engine_version": "9.0.0", "saved_at_ms": 1, "data": { "player": [1.0, 2.0], "lives": 3 } }"#;
    std::fs::write(&slot, newer).unwrap();

    let mut harness = harness_with(UserFolder::At(dir.clone()));
    tap(&mut harness, "state_start");
    assert_eq!(player(&harness), Some(PLAYER_START));
    tap(&mut harness, "state_pause");
    assert_eq!(active_state(harness.world()), Some(PAUSE));
    assert_eq!(
        std::fs::read_to_string(&slot).unwrap(),
        newer,
        "the slot is untouched"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
