//! The shell's screens and run: the title and its difficulty picker, pause,
//! game over and Retry, quitting, the exit's and the close's last frame,
//! focus loss, the HUD, the theme and the toasts.

use std::path::Path;

use tungsten::StateStack;
use tungsten::core::ui::ThemeFile;
use tungsten::core::{InputState, KeyCode, SaveSlots, Time, UserFolder};
use tungsten::kit::{Hit, damage};
use tungsten::testing::UiHarness;
use tungsten_template_basic::game;
use tungsten_template_basic::sample::hud::Hud;
use tungsten_template_basic::sample::{GEM_STARTS, PLAYER_START, SampleRun};
use tungsten_template_basic::save::{self, SaveGame};
use tungsten_template_basic::shell::confirm::CONFIRM;
use tungsten_template_basic::shell::game_over::GAME_OVER;
use tungsten_template_basic::shell::pause::PAUSE;
use tungsten_template_basic::shell::title::TITLE;
use tungsten_template_basic::shell::toast::{self, TOAST_SECS, Toasts};
use tungsten_template_basic::shell::{AUTOSAVE, Difficulty, RunState, widgets};

use super::{
    find, harness, harness_with, health, player_entity, player_position, press, settle, slot_file,
    start, start_at, tap, top, user_folder,
};

fn depth(harness: &UiHarness) -> usize {
    harness.world().resource::<StateStack>().depth()
}

fn paused(harness: &UiHarness) -> bool {
    harness.world().resource::<Time>().is_paused()
}

fn read(dir: &Path, slot: &str) -> SaveGame {
    SaveSlots::new(Some(dir.join("saves")))
        .load::<SaveGame>(slot)
        .unwrap_or_else(|error| panic!("{slot}: {error}"))
}

/// Moves the player right for `frames` frames.
fn walk(harness: &mut UiHarness, frames: u32) {
    harness.harness_mut().press_action("move_right");
    harness.step(frames);
    harness.harness_mut().release_action("move_right");
    harness.step(1);
}

/// Kills the player and steps into game over.
fn kill(harness: &mut UiHarness) {
    let player = player_entity(harness);
    assert_eq!(damage(harness.world_mut(), player, 99.0), Hit::Killed);
    harness.step(1);
    settle(harness);
    assert_eq!(top(harness), Some(GAME_OVER));
}

fn focused(harness: &UiHarness) -> Option<tungsten::core::ui::WidgetId> {
    harness.tree().focused()
}

/// A run in `dir` that leaves an autosave: started, walked, paused.
fn leave_autosave(dir: &Path) -> SaveGame {
    let mut harness = harness_with(UserFolder::At(dir.to_path_buf()));
    start(&mut harness);
    walk(&mut harness, 20);
    tap(&mut harness, "state_pause");
    harness.step(1);
    read(dir, AUTOSAVE)
}

#[test]
fn title_focuses_continue_only_with_a_save() {
    let harness = harness();
    assert_eq!(top(&harness), Some(TITLE));
    assert_eq!(find(&harness, "Continue"), None, "disabled with no save");
    assert_eq!(focused(&harness), find(&harness, "New Game"));

    let dir = user_folder("title-continue");
    leave_autosave(&dir);
    let harness = harness_with(UserFolder::At(dir.clone()));
    let resume = find(&harness, "Continue").expect("enabled over a save");
    assert_eq!(focused(&harness), Some(resume));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_game_picks_a_difficulty_and_writes_a_fresh_autosave() {
    let dir = user_folder("new-game");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "New Game");
    assert_eq!(focused(&harness), find(&harness, "Normal"), "Normal first");
    press(&mut harness, "Hard");
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    assert_eq!(health(&harness, player_entity(&harness)), (3.0, 3.0));
    let save = read(&dir, AUTOSAVE);
    assert_eq!(save.meta.difficulty, Difficulty::Hard);
    assert_eq!(save.player, PLAYER_START);
    assert_eq!(save.gems_left, [0, 1, 2]);
    assert_eq!(save.gems, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_game_asks_first_over_an_autosave() {
    let dir = user_folder("new-game-asks");
    let old = leave_autosave(&dir);
    let file = slot_file(&dir, AUTOSAVE);
    let bytes = std::fs::read(&file).unwrap();
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "New Game");
    press(&mut harness, "Normal");
    assert_eq!(top(&harness), Some(CONFIRM));
    assert_eq!(
        focused(&harness),
        find(&harness, "Cancel"),
        "the safe answer"
    );
    press(&mut harness, "Cancel");
    assert_eq!(top(&harness), Some(TITLE));
    assert_eq!(std::fs::read(&file).unwrap(), bytes, "the autosave stays");
    assert_eq!(
        focused(&harness),
        find(&harness, "Normal"),
        "focus comes back"
    );

    press(&mut harness, "Normal");
    press(&mut harness, "Start");
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    let fresh = read(&dir, AUTOSAVE);
    assert_ne!(fresh.player, old.player);
    assert_eq!(fresh.player, PLAYER_START, "the autosave is replaced");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn state_pause_opens_pause_and_holds_the_game_clock() {
    let mut harness = harness();
    start(&mut harness);
    assert!(!paused(&harness));
    tap(&mut harness, "state_pause");
    assert_eq!(top(&harness), Some(PAUSE));
    assert!(paused(&harness), "gameplay holds the clock while covered");
    assert_eq!(focused(&harness), find(&harness, "Resume"));
    assert!(harness.world().has_resource::<Hud>(), "the HUD stays");
}

#[test]
fn pause_closes_once_on_escape_resume_or_state_pause() {
    let mut harness = harness();
    start(&mut harness);
    for way in ["Escape", "Resume", "state_pause"] {
        tap(&mut harness, "state_pause");
        assert_eq!(top(&harness), Some(PAUSE), "{way}");
        match way {
            "Escape" => {
                // Both `ui_cancel` and `state_pause`: one pop.
                harness.key_down(KeyCode::Escape);
                harness.step(1);
                harness.key_up(KeyCode::Escape);
                harness.step(1);
                settle(&mut harness);
            }
            "Resume" => press(&mut harness, "Resume"),
            _ => tap(&mut harness, "state_pause"),
        }
        assert_eq!(top(&harness), Some(game::GAMEPLAY), "{way}");
        assert_eq!(depth(&harness), 1, "{way}");
        assert!(!paused(&harness), "{way}");
    }
}

#[test]
fn quit_to_title_autosaves_and_returns_to_the_title() {
    let dir = user_folder("quit-to-title");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    walk(&mut harness, 20);
    let walked = player_position(&harness).unwrap();
    tap(&mut harness, "state_pause");
    std::fs::remove_file(slot_file(&dir, AUTOSAVE)).unwrap();
    press(&mut harness, "Quit to Title");
    assert_eq!(top(&harness), Some(TITLE));
    assert_eq!(depth(&harness), 1);
    assert!(!harness.world().has_resource::<RunState>(), "the run ended");
    assert_eq!(read(&dir, AUTOSAVE).player, walked);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quit_to_title_from_a_screen_update_leaves_only_the_title() {
    // No user folder: Quit to Title asks, so the change comes from the
    // confirm's update, three states deep.
    let mut harness = harness();
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Quit to Title");
    assert_eq!(depth(&harness), 3);
    press(&mut harness, "Quit");
    assert_eq!(top(&harness), Some(TITLE));
    assert_eq!(depth(&harness), 1);
}

#[test]
fn quit_to_title_asks_first_when_the_autosave_is_unavailable() {
    let mut harness = harness();
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Quit to Title");
    assert_eq!(top(&harness), Some(CONFIRM), "no user folder");
    press(&mut harness, "Stay");
    assert_eq!(top(&harness), Some(PAUSE));
    assert_eq!(focused(&harness), find(&harness, "Quit to Title"));

    // A read-only autosave: one from a newer build.
    let dir = user_folder("quit-read-only");
    let file = slot_file(&dir, AUTOSAVE);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let newer = r#"{ "envelope": 1, "slot": "autosave", "schema": 9, "engine_version": "9.0.0", "saved_at_ms": 1, "data": {} }"#;
    std::fs::write(&file, newer).unwrap();
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Quit to Title");
    assert_eq!(top(&harness), Some(CONFIRM), "a read-only autosave");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), newer);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_killed_player_opens_game_over_and_holds_the_clock() {
    let mut harness = harness();
    start(&mut harness);
    let player = player_entity(&harness);
    kill(&mut harness);
    assert!(paused(&harness));
    assert!(harness.world().is_alive(player), "the dead stay under it");
    assert_eq!(focused(&harness), find(&harness, "Retry"));
}

#[test]
fn retry_loads_the_runs_last_good_save() {
    let dir = user_folder("retry-save");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    walk(&mut harness, 20);
    let walked = player_position(&harness).unwrap();
    tap(&mut harness, "state_pause");
    press(&mut harness, "Resume");
    walk(&mut harness, 20);
    kill(&mut harness);
    press(&mut harness, "Retry");
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    assert_eq!(depth(&harness), 1);
    assert_eq!(player_position(&harness), Some(walked), "the pause's save");
    assert_eq!(health(&harness, player_entity(&harness)), (5.0, 5.0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Makes `dir` read-only; returns its permissions before, to put back.
fn make_read_only(dir: &Path) -> std::fs::Permissions {
    let writable = std::fs::metadata(dir).unwrap().permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(dir, read_only).unwrap();
    writable
}

#[test]
fn retry_starts_fresh_when_the_runs_first_autosave_failed() {
    let dir = user_folder("retry-fresh");
    // An earlier run's readable autosave, with a gem taken.
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    let gem = harness.world().resource::<SampleRun>().gems[0].1;
    harness.world_mut().despawn(gem);
    harness
        .world_mut()
        .resource_mut::<tungsten::kit::Counters>()
        .set("gems", 1);
    tap(&mut harness, "state_pause");
    harness.step(1);
    assert_eq!(read(&dir, AUTOSAVE).gems_left.len(), 2);

    let writable = make_read_only(&dir.join("saves"));
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "New Game");
    press(&mut harness, "Normal");
    press(&mut harness, "Start");
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    assert_eq!(
        harness.world().resource::<RunState>().retry_from,
        None,
        "the first autosave failed"
    );
    kill(&mut harness);
    press(&mut harness, "Retry");
    assert_eq!(harness.world().resource::<SampleRun>().gems.len(), 3);
    assert_eq!(health(&harness, player_entity(&harness)), (5.0, 5.0));
    std::fs::set_permissions(dir.join("saves"), writable).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn retry_with_the_old_dead_player_still_queued_stays_in_gameplay() {
    let mut harness = harness();
    start(&mut harness);
    kill(&mut harness);
    let retry = find(&harness, "Retry").unwrap();
    assert!(harness.focus(retry));
    harness.harness_mut().press_action("ui_accept");
    harness.step(1);
    harness.harness_mut().release_action("ui_accept");
    // The frame of the replace: the old, dead player is despawned at its
    // end, after the new gameplay's update ran beside it.
    harness.step(1);
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    assert!(!tungsten_template_basic::shell::transitioning(
        harness.world()
    ));
    harness.step(1);
    assert_eq!(top(&harness), Some(game::GAMEPLAY), "and the next");
    assert!(!tungsten_template_basic::shell::transitioning(
        harness.world()
    ));
    assert_eq!(depth(&harness), 1);
}

#[test]
fn retry_autosaves_only_the_new_runs_gems() {
    let dir = user_folder("retry-gems");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    // A save with one gem left, which the run's Retry loads.
    let mut one = save::capture(harness.world()).expect("a live run");
    one.gems_left = vec![2];
    SaveSlots::new(Some(dir.join("saves")))
        .save("slot_1", &one)
        .unwrap();
    harness.world_mut().resource_mut::<RunState>().retry_from = Some("slot_1");
    assert_eq!(harness.world().resource::<SampleRun>().gems.len(), 3);
    kill(&mut harness);
    press(&mut harness, "Retry");
    assert_eq!(
        read(&dir, AUTOSAVE).gems_left,
        [2],
        "the new run's gem alone"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quit_game_requests_exit_and_the_last_frame_autosaves() {
    let dir = user_folder("quit-game");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    std::fs::remove_file(slot_file(&dir, AUTOSAVE)).unwrap();
    press(&mut harness, "Quit Game");
    assert!(harness.harness().exit_requested());
    assert!(
        slot_file(&dir, AUTOSAVE).is_file(),
        "saved in the last frame"
    );
    assert_eq!(harness.step(1), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_close_in_gameplay_autosaves_in_the_last_frame() {
    let dir = user_folder("close");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    walk(&mut harness, 20);
    let walked = player_position(&harness).unwrap();
    assert_ne!(read(&dir, AUTOSAVE).player, walked);
    harness.harness_mut().request_close();
    assert_eq!(harness.step(1), 1, "the last frame runs");
    assert!(harness.harness().exit_requested());
    assert_eq!(read(&dir, AUTOSAVE).player, walked);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_autosave_on_the_title_or_while_dead() {
    let dir = user_folder("no-autosave");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    harness.harness_mut().request_close();
    harness.step(1);
    assert!(
        !slot_file(&dir, AUTOSAVE).exists(),
        "the title saves nothing"
    );

    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    std::fs::remove_file(slot_file(&dir, AUTOSAVE)).unwrap();
    kill(&mut harness);
    harness.harness_mut().request_close();
    harness.step(1);
    assert!(!slot_file(&dir, AUTOSAVE).exists(), "the dead save nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn losing_focus_pauses_gameplay() {
    let mut harness = harness();
    start(&mut harness);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .set_focused(false);
    harness.step(1);
    settle(&mut harness);
    assert_eq!(top(&harness), Some(PAUSE));
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .set_focused(true);
    harness.step(2);
    assert_eq!(top(&harness), Some(PAUSE), "focus back keeps the pause");
}

#[test]
fn the_hud_shows_health_gems_and_the_bound_pause_key() {
    let mut harness = harness();
    start(&mut harness);
    let player = player_entity(&harness);
    assert_eq!(damage(harness.world_mut(), player, 1.0), Hit::Hurt);
    harness.step(1);
    let hud = *harness.world().resource::<Hud>();
    let tree = harness.tree();
    assert_eq!(widgets::text_of(tree, hud.health), "4 / 5");
    assert_eq!(
        widgets::text_of(tree, hud.gems),
        format!("Gems 0 / {}", GEM_STARTS.len())
    );
    let prompt = widgets::text_of(tree, hud.prompt);
    assert!(
        prompt.contains("P Pause") && prompt.contains("M Mute"),
        "{prompt:?}"
    );
}

/// WCAG's contrast ratio of two opaque sRGB colours.
fn contrast(a: [u8; 4], b: [u8; 4]) -> f32 {
    let luminance = |c: [u8; 4]| {
        let channel = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c[0]) + 0.7152 * channel(c[1]) + 0.0722 * channel(c[2])
    };
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[test]
fn the_theme_meets_contrast_and_has_square_corners() {
    let theme = ThemeFile::load("assets/ui/themes/shell.json")
        .expect("the theme parses")
        .into_theme();
    let colors = theme.tokens.colors;
    let surface = colors.surface;
    for (name, color, against, least) in [
        ("text", colors.text, surface, 15.0),
        ("text_muted", colors.text_muted, surface, 7.6),
        ("accent", colors.accent, surface, 8.8),
        ("accent_text", colors.accent_text, colors.accent, 9.6),
        ("focus", colors.focus, surface, 12.6),
        ("border", colors.border, surface, 3.35),
        ("disabled_text", colors.disabled_text, surface, 3.7),
    ] {
        let ratio = contrast(color, against);
        assert!(ratio >= least - 0.01, "{name}: {ratio:.2} < {least}");
        assert_eq!(color[3], 255, "{name} is opaque");
    }
    let kinds = &theme.kinds;
    for (name, kind) in [
        ("panel", &kinds.panel),
        ("label", &kinds.label),
        ("button", &kinds.button),
        ("image", &kinds.image),
        ("toggle", &kinds.toggle),
        ("slider", &kinds.slider),
    ] {
        assert_eq!(kind.normal.corner_radius, 0.0, "{name}");
        for state in [&kind.focused, &kind.disabled, &kind.hovered, &kind.pressed] {
            assert!(
                state.corner_radius.is_none_or(|radius| radius == 0.0),
                "{name}"
            );
        }
    }
}

#[test]
fn a_toast_leaves_on_the_real_clock_under_a_pause() {
    let mut harness = harness();
    start(&mut harness);
    tap(&mut harness, "state_pause");
    assert!(paused(&harness));
    toast::toast(harness.world_mut(), "Saved to Slot 1");
    harness.step(1);
    let shown = |harness: &UiHarness| harness.world().resource::<Toasts>().texts(harness.tree());
    assert_eq!(shown(&harness), ["Saved to Slot 1"]);
    let frames = (TOAST_SECS * 60.0) as u32;
    harness.step(frames - 2);
    assert_eq!(shown(&harness).len(), 1, "still up under the pause");
    harness.step(4);
    assert!(shown(&harness).is_empty(), "gone on the real clock");
}

/// The run's start difficulty reaches the HUD's max.
#[test]
fn a_hard_run_shows_its_max_health() {
    let mut harness = harness();
    start_at(&mut harness, Difficulty::Hard);
    harness.step(1);
    let hud = *harness.world().resource::<Hud>();
    assert_eq!(widgets::text_of(harness.tree(), hud.health), "3 / 3");
}
