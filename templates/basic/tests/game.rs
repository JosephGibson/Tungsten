//! The game on the engine's headless harness: no window, renderer or
//! manifest, the frames stepped one call at a time. A test per system.

use std::path::Path;

use tungsten::App;
use tungsten::core::assets::manifest::ResolvedManifest;
use tungsten::core::{ActionMap, Config, Transform};
use tungsten::testing::Harness;
use tungsten_template_basic::components::Player;
use tungsten_template_basic::game::{self, active_state};
use tungsten_template_basic::states::{GAMEPLAY, PAUSE, PLAYER_START, TITLE};

/// The game as `main.rs` builds it, with its startup work run by hand and
/// one frame stepped, which puts the title state on the stack.
fn harness() -> Harness {
    let config = Config::load("tungsten.json").expect("tungsten.json loads");
    let mut app = App::new(config).expect("App::new failed");
    game::register(&mut app);
    let mut harness = Harness::new(app);
    game::setup(harness.world_mut());
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
    let mut players = harness.world().query2::<Player, Transform>();
    players
        .next()
        .map(|(_, _, transform)| [transform.position.x, transform.position.y])
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
