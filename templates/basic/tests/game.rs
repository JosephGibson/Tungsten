//! The game on the engine's headless harness: no window, renderer or
//! manifest, the frames stepped one call at a time. A test per system.

use std::path::Path;

use tungsten::App;
use tungsten::core::assets::manifest::ResolvedManifest;
use tungsten::core::{ActionMap, Config, Time, Transform, With};
use tungsten::testing::Harness;
use tungsten_template_basic::components::Player;
use tungsten_template_basic::game::{self, active_state};
use tungsten_template_basic::states::{GAMEPLAY, PAUSE, PLAYER_START, TITLE};

/// The game as `main.rs` builds it, with one frame stepped: its `Startup`
/// stage runs `setup`, which requests the title state, and the engine's
/// dispatcher applies it in the same frame's `pre_update`.
fn harness() -> Harness {
    let config = Config::load("tungsten.json").expect("tungsten.json loads");
    let mut app = App::new(config).expect("App::new failed");
    game::register(&mut app);
    let mut harness = Harness::new(app);
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

/// The resolved schedule as `register` builds it: the game's two systems in
/// their stages between the engine's. A reorder shows up here as a diff.
const SCHEDULE: &str = "\
startup: setup
pre_update: physics_debug_toggle, systems_overlay_toggle, inspector_toggle, inspector_pick, hud_toggle, display_input, state_dispatcher
fixed_update: physics_prev_snapshot, physics_step
update: player_movement
post_update: physics_sync, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, camera_update
";

#[test]
fn the_schedule_matches_the_snapshot() {
    let config = Config::load("tungsten.json").expect("tungsten.json loads");
    let mut app = App::new(config).expect("App::new failed");
    game::register(&mut app);
    app.resolve_schedule().expect("the schedule resolves");
    assert_eq!(app.schedule().resolved_text(), SCHEDULE);
}

#[test]
fn data_files_load() {
    let config = Config::load("tungsten.json").expect("tungsten.json loads");
    assert_eq!(config.game.id.as_deref(), Some("tungsten-template-basic"));
    let actions = ActionMap::load(Path::new("input.json")).expect("input.json loads");
    for action in ["move_left", "move_right", "move_up", "move_down"] {
        assert!(
            !actions.bindings(action).is_empty(),
            "{action} has no binding"
        );
    }
    let manifest = ResolvedManifest::load(game::MANIFEST).expect("the manifest loads");
    assert!(manifest.sprites["player"].path.is_file());
    assert!(manifest.fonts[game::FONT].path.is_file());
}

#[test]
fn state_start_starts_gameplay_with_the_player() {
    let mut harness = harness();
    assert_eq!(active_state(harness.world()), Some(TITLE));
    assert_eq!(player(&harness), None);
    tap(&mut harness, "state_start");
    assert_eq!(active_state(harness.world()), Some(GAMEPLAY));
    assert_eq!(player(&harness), Some(PLAYER_START));
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
