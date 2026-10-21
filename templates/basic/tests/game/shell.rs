//! The shell's screens and run: the title and its difficulty picker, pause,
//! game over and Retry, quitting, the exit's and the close's last frame,
//! focus loss, the HUD, the theme and the toasts.

use std::path::Path;

use tungsten::core::text::{TextAlign, TextNodeStore};
use tungsten::core::ui::{ThemeFile, UiTree, Visibility, WidgetId};
use tungsten::core::{
    InputState, KeyCode, SaveSlots, Schedule, Stage, Time, UserFolder, World, system,
};
use tungsten::kit::{Health, Hit, damage};
use tungsten::testing::UiHarness;
use tungsten::{Plugin, StateStack};
use tungsten_template_basic::game;
use tungsten_template_basic::sample::hud::Hud;
use tungsten_template_basic::sample::{GEM_STARTS, PLAYER_START, SampleRun};
use tungsten_template_basic::save::{self, SaveGame};
use tungsten_template_basic::shell::confirm::CONFIRM;
use tungsten_template_basic::shell::game_over::GAME_OVER;
use tungsten_template_basic::shell::pause::PAUSE;
use tungsten_template_basic::shell::title::TITLE;
use tungsten_template_basic::shell::toast::{self, TOAST_SECS, Toasts};
use tungsten_template_basic::shell::{
    AUTOSAVE, Difficulty, RunStart, RunState, request_autosave, theme, widgets,
};

use super::{
    act, find, harness, harness_with, health, heard, player_entity, player_position, plays, press,
    settle, shows, slot_file, start, start_at, stops, tap, top, user_folder,
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

/// Moves the player right for `frames` frames, then lets it slide to a
/// stop.
fn walk(harness: &mut UiHarness, frames: u32) {
    harness.harness_mut().press_action("move_right");
    harness.step(frames);
    super::release_and_settle(harness, "move_right");
}

/// Kills the player and steps into game over.
fn kill(harness: &mut UiHarness) {
    let player = player_entity(harness);
    assert_eq!(damage(harness.world_mut(), player, 99.0), Hit::Killed);
    harness.step(1);
    settle(harness);
    assert_eq!(top(harness), Some(GAME_OVER));
}

fn focused(harness: &UiHarness) -> Option<WidgetId> {
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
    assert!(shows(&harness, "Quit to the title?"), "one question");
    press(&mut harness, "Quit");
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
fn quit_to_title_asks_once_and_names_lost_progress_only_when_the_autosave_failed() {
    let lost = "Quit to the title? Progress since your last save will be lost.";
    // An autosave that works: one plain question, the save already written.
    let dir = user_folder("quit-asks");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    std::fs::remove_file(slot_file(&dir, AUTOSAVE)).unwrap();
    press(&mut harness, "Quit to Title");
    assert_eq!(top(&harness), Some(CONFIRM));
    assert!(shows(&harness, "Quit to the title?") && !shows(&harness, lost));
    assert!(slot_file(&dir, AUTOSAVE).is_file(), "saved before asking");
    assert_eq!(focused(&harness), find(&harness, "Stay"), "the safe answer");
    press(&mut harness, "Stay");
    assert_eq!(top(&harness), Some(PAUSE));
    assert_eq!(focused(&harness), find(&harness, "Quit to Title"));
    let _ = std::fs::remove_dir_all(&dir);

    // No user folder: the autosave fails, so the question names the loss.
    let mut harness = super::harness();
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Quit to Title");
    assert!(shows(&harness, lost), "no user folder");
    press(&mut harness, "Stay");

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
    assert!(shows(&harness, lost), "a read-only autosave");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), newer);
    press(&mut harness, "Quit");
    assert_eq!(top(&harness), Some(TITLE));
    assert_eq!(depth(&harness), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quit_game_asks_first_from_pause_and_title() {
    // From pause: one question, after the autosave; Stay keeps playing.
    let dir = user_folder("quit-game-asks");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    std::fs::remove_file(slot_file(&dir, AUTOSAVE)).unwrap();
    press(&mut harness, "Quit Game");
    assert!(shows(&harness, "Quit the game?"));
    assert!(slot_file(&dir, AUTOSAVE).is_file(), "saved before asking");
    assert!(!harness.harness().exit_requested());
    press(&mut harness, "Stay");
    assert_eq!(top(&harness), Some(PAUSE));
    press(&mut harness, "Quit Game");
    press(&mut harness, "Quit");
    assert!(harness.harness().exit_requested());
    let _ = std::fs::remove_dir_all(&dir);

    // From the title: the same question; Stay stays.
    let mut harness = super::harness();
    press(&mut harness, "Quit");
    assert!(shows(&harness, "Quit the game?"));
    assert!(!harness.harness().exit_requested());
    press(&mut harness, "Stay");
    assert_eq!(top(&harness), Some(TITLE));
    press(&mut harness, "Quit");
    press(&mut harness, "Quit");
    assert!(harness.harness().exit_requested());
}

#[test]
fn game_over_quits_to_the_title_without_asking() {
    let mut harness = harness();
    start(&mut harness);
    kill(&mut harness);
    press(&mut harness, "Quit to Title");
    assert_eq!(top(&harness), Some(TITLE));
    assert_eq!(depth(&harness), 1);
    assert!(!harness.world().has_resource::<RunState>());
}

#[test]
fn continue_names_its_slot_and_age() {
    let dir = user_folder("continue-names");
    leave_autosave(&dir);
    let harness = harness_with(UserFolder::At(dir.clone()));
    let tree = harness.tree();
    let resume = find(&harness, "Continue").unwrap();
    // The button's own label, then the line under it.
    let texts: Vec<String> = descendants(tree, resume)[1..]
        .iter()
        .map(|id| widgets::text_of(tree, *id))
        .filter(|text| !text.is_empty())
        .collect();
    assert_eq!(texts, ["Continue", "Autosave ·", "just now"]);
    let age = descendants(tree, resume)
        .into_iter()
        .find(|id| widgets::text_of(tree, *id) == "just now")
        .unwrap();
    let style = &tree.resolved_style(age).unwrap().text;
    assert_eq!(style.family, "engine_mono", "the age in mono");
    assert_eq!(style.color, tree.theme().tokens.colors.text_muted, "muted");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `id` and everything under it, depth first.
fn descendants(tree: &UiTree, id: WidgetId) -> Vec<WidgetId> {
    let mut out = Vec::new();
    let mut stack = vec![id];
    while let Some(id) = stack.pop() {
        out.push(id);
        stack.extend(tree.children(id).iter().rev().copied());
    }
    out
}

#[test]
fn the_picker_notes_what_each_difficulty_changes() {
    let mut harness = harness();
    press(&mut harness, "New Game");
    for (difficulty, note) in [
        ("Easy", "8 health"),
        ("Normal", "5 health"),
        ("Hard", "3 health"),
    ] {
        let tree = harness.tree();
        let button = find(&harness, difficulty).unwrap();
        let row = tree.parent(button).unwrap();
        let beside: Vec<String> = tree.children(row)[1..]
            .iter()
            .map(|id| widgets::text_of(tree, *id))
            .collect();
        assert_eq!(beside, [note], "{difficulty}");
        assert_eq!(
            game::difficulty_note(
                Difficulty::ALL
                    .into_iter()
                    .find(|d| d.label() == difficulty)
                    .unwrap()
            ),
            note
        );
    }
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
    // "You died" in the danger colour, over a scrim deepened to red once
    // the entrance has run.
    let root = front(&harness).expect("game over's root");
    let tree = harness.tree();
    let heading = labelled(tree, root, "You died").expect("the heading");
    let danger = tree.theme().tokens.colors.danger;
    assert_eq!(tree.resolved_style(heading).unwrap().text.color, danger);
    harness.step(60);
    let scrim = harness.tree().overrides(root).unwrap().background;
    assert_eq!(scrim, Some(theme::DIED_TINT));
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

/// Makes `dir` read-only; returns its permissions before, to put back. Unix
/// only: Windows does not honor the read-only attribute on folders.
fn make_read_only(dir: &Path) -> std::fs::Permissions {
    let writable = std::fs::metadata(dir).unwrap().permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(dir, read_only).unwrap();
    writable
}

#[test]
#[cfg_attr(not(unix), ignore = "Windows does not honor read-only on folders")]
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
    // The frame of the replace, under the curtain: the old, dead player is
    // despawned at its end, after the new gameplay's update ran beside it.
    for _ in 0..30 {
        if top(&harness) == Some(game::GAMEPLAY) {
            break;
        }
        harness.step(1);
    }
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    harness.step(1);
    assert_eq!(top(&harness), Some(game::GAMEPLAY), "and the next");
    settle(&mut harness);
    assert_eq!(top(&harness), Some(game::GAMEPLAY), "and once settled");
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
    std::fs::remove_file(slot_file(&dir, AUTOSAVE)).unwrap();
    press(&mut harness, "Quit");
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
fn losing_focus_during_a_screen_change_pauses_after_it() {
    let set_focused = |harness: &mut UiHarness, focused: bool| {
        let input = harness.world_mut().resource_mut::<InputState>();
        input.set_focused(focused);
    };
    // Under a run's curtain: the run starts paused.
    let mut harness = harness();
    let start = RunStart::default();
    tungsten_template_basic::shell::start_run(harness.world_mut(), start, None);
    harness.step(1);
    assert!(tungsten_template_basic::shell::transitioning(
        harness.world()
    ));
    set_focused(&mut harness, false);
    for _ in 0..3 {
        harness.step(1);
        settle(&mut harness);
    }
    assert_eq!(top(&harness), Some(PAUSE), "paused once the curtain ends");
    // While pause closes: it opens again over gameplay, once.
    set_focused(&mut harness, true);
    harness.step(1);
    let resume = find(&harness, "Resume").unwrap();
    assert!(harness.focus(resume));
    harness.harness_mut().press_action("ui_accept");
    harness.step(1);
    harness.harness_mut().release_action("ui_accept");
    set_focused(&mut harness, false);
    for _ in 0..3 {
        harness.step(1);
        settle(&mut harness);
    }
    assert_eq!(top(&harness), Some(PAUSE), "paused again after the close");
    assert_eq!(depth(&harness), 2, "one pause");
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
        prompt.contains("P Pause") && !prompt.contains("Mute"),
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
        ("danger", colors.danger, surface, 6.4),
        (
            "danger on the background",
            colors.danger,
            colors.background,
            7.0,
        ),
        (
            "danger on a hovered fill",
            colors.danger,
            [0x1e, 0x21, 0x29, 255],
            5.8,
        ),
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

/// The front-most shown modal root: the screen on top, or the curtain.
fn front(harness: &UiHarness) -> Option<WidgetId> {
    let tree = harness.tree();
    tree.roots()
        .iter()
        .rev()
        .copied()
        .find(|root| tree.modal(*root) && tree.visibility(*root) == Some(Visibility::Visible))
}

/// The first widget under `root` whose text reads `text`.
fn labelled(tree: &UiTree, root: WidgetId, text: &str) -> Option<WidgetId> {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if widgets::text_of(tree, id) == text {
            return Some(id);
        }
        stack.extend(tree.children(id).iter().copied());
    }
    None
}

/// Steps until no screen change runs, counting the steps.
fn steps_to_idle(harness: &mut UiHarness) -> u32 {
    let mut steps = 0;
    while tungsten_template_basic::shell::transitioning(harness.world()) {
        harness.step(1);
        steps += 1;
        assert!(steps < 100, "the change never ended");
    }
    steps
}

#[test]
fn the_title_is_one_serif_line_above_the_menu() {
    let harness = harness();
    let tree = harness.tree();
    let root = front(&harness).expect("the title's root");
    let title = labelled(tree, root, &super::info().title).expect("the game's name");
    let text = &tree.resolved_style(title).unwrap().text;
    assert_eq!(text.family, "serif");
    assert_eq!([text.size, text.line_height], theme::TITLE);
    let node = tree.text_node(title).unwrap();
    assert_eq!(harness.text().committed(node).unwrap().line_count, 1);
    let menu = harness.rect(find(&harness, "New Game").unwrap()).unwrap();
    assert!(
        harness.rect(title).unwrap().max.y + theme::TITLE_SPACE <= menu.min.y,
        "well above the menu"
    );
    let width = menu.max.x - menu.min.x;
    assert!(
        (width - theme::TITLE_MENU_WIDTH).abs() < 0.5,
        "narrower: {width}"
    );
    // The version and "Saving is off", small in the corner.
    let viewport = harness.rect(root).unwrap();
    let version = format!("Version {}", super::info().version);
    for (line, caption) in [version.as_str(), "Saving is off"].iter().enumerate() {
        let label = labelled(tree, root, caption).expect("a corner line");
        let rect = harness.rect(label).unwrap();
        assert_eq!(rect.min.x, theme::MARGIN, "{caption}");
        assert!(rect.min.y > menu.max.y, "{caption} below the menu");
        let style = &tree.resolved_style(label).unwrap().text;
        assert_eq!(
            (style.size, style.line_height),
            (theme::SMALL[0], theme::SMALL[1])
        );
        if line == 1 {
            assert_eq!(rect.max.y, viewport.max.y - theme::MARGIN, "at the bottom");
        }
    }
}

#[test]
fn pause_and_dialogs_show_over_a_scrim_with_the_screen_below_drawn() {
    let mut harness = harness();
    start(&mut harness);
    let hud = harness.world().resource::<Hud>().root;
    tap(&mut harness, "state_pause");
    let pause = front(&harness).unwrap();
    let background = harness.tree().theme().tokens.colors.background;
    let scrim = |harness: &UiHarness, root| {
        let style = harness.tree().resolved_style(root).unwrap();
        let rect = harness.rect(root).unwrap();
        (
            style.background,
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
        )
    };
    let window = *harness.world().resource::<tungsten::WindowSize>();
    let scale = harness
        .world()
        .resource::<tungsten::core::ScaleFactor>()
        .get();
    let viewport = [
        0.0,
        0.0,
        window.width as f32 / scale,
        window.height as f32 / scale,
    ];
    let [r, g, b, _] = background;
    assert_eq!(
        scrim(&harness, pause),
        ([r, g, b, theme::SCRIM_ALPHA], viewport),
        "pause dims the whole window"
    );
    let tree = harness.tree();
    assert_eq!(
        tree.visibility(hud),
        Some(Visibility::Visible),
        "the HUD under it"
    );
    assert!(tree.frozen(hud));
    let heading = labelled(tree, pause, "Paused").unwrap();
    let text = &tree.resolved_style(heading).unwrap().text;
    assert_eq!(text.family, "serif");
    assert_eq!(
        text.layout.align,
        TextAlign::Center,
        "a menu centres its heading"
    );
    // No user folder, so Quit to Title asks over the pause card.
    press(&mut harness, "Quit to Title");
    assert_eq!(top(&harness), Some(CONFIRM));
    let confirm = front(&harness).unwrap();
    assert_ne!(confirm, pause);
    let tree = harness.tree();
    assert_eq!(
        tree.visibility(pause),
        Some(Visibility::Visible),
        "pause stays drawn"
    );
    assert!(tree.frozen(pause), "and takes no input");
    assert_eq!(scrim(&harness, confirm).0, [r, g, b, theme::SCRIM_ALPHA]);
    press(&mut harness, "Stay");
    assert_eq!(front(&harness), Some(pause));
    assert!(!harness.tree().frozen(pause));
}

#[test]
fn menu_changes_take_their_times() {
    let dir = user_folder("change-times");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    let dt = 1.0 / 60.0;
    harness.harness_mut().set_dt(dt);
    // A push acts on its first frame: pause opens with no change waiting.
    harness.harness_mut().press_action("state_pause");
    harness.step(1);
    harness.harness_mut().release_action("state_pause");
    harness.step(1);
    assert_eq!(top(&harness), Some(PAUSE));
    assert!(tungsten_template_basic::shell::idle(harness.world()));
    // A pop: the card fades out over 0.08 s, then the screen below acts.
    let resume = find(&harness, "Resume").unwrap();
    assert!(harness.focus(resume));
    harness.harness_mut().press_action("ui_accept");
    harness.step(1);
    harness.harness_mut().release_action("ui_accept");
    let pop = steps_to_idle(&mut harness);
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    assert!(
        (pop as f32 * dt - theme::POP_SECS).abs() <= dt + 1e-4,
        "{pop} steps"
    );
    // A run change: the curtain's 0.24 s, from the question's Quit.
    tap(&mut harness, "state_pause");
    press(&mut harness, "Quit to Title");
    let quit = find(&harness, "Quit").unwrap();
    assert!(harness.focus(quit));
    harness.harness_mut().press_action("ui_accept");
    harness.step(1);
    harness.harness_mut().release_action("ui_accept");
    let run = steps_to_idle(&mut harness);
    assert_eq!(top(&harness), Some(TITLE));
    let curtain = theme::CURTAIN_IN_SECS + theme::CURTAIN_OUT_SECS;
    assert!(
        (run as f32 * dt - curtain).abs() <= dt + 1e-4,
        "{run} steps"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_run_change_fades_through_the_curtain_and_autosaves_the_new_run_once() {
    let dir = user_folder("curtain-autosave");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "New Game");
    let normal = find(&harness, "Normal").unwrap();
    assert!(harness.focus(normal));
    harness.harness_mut().press_action("ui_accept");
    harness.step(1);
    harness.harness_mut().release_action("ui_accept");
    let curtain = front(&harness).expect("the curtain");
    let tree = harness.tree();
    assert!(tree.children(curtain).is_empty(), "a bare root");
    assert_eq!(
        tree.resolved_style(curtain).unwrap().background,
        tree.theme().tokens.colors.background,
        "the theme's background"
    );
    let (mut peak, mut saves, mut written) = (0.0_f32, 0, None);
    for _ in 0..60 {
        harness.step(1);
        let tree = harness.tree();
        if !tree.contains(curtain) {
            break;
        }
        let opacity = tree.opacity(curtain).unwrap();
        peak = peak.max(opacity);
        if top(&harness) == Some(TITLE) {
            assert!(
                !slot_file(&dir, AUTOSAVE).exists(),
                "nothing saved before the swap"
            );
        }
        let bytes = std::fs::read(slot_file(&dir, AUTOSAVE)).ok();
        if bytes.is_some() && bytes != written {
            saves += 1;
            written = bytes;
        }
    }
    assert!(!harness.tree().contains(curtain), "gone once faded");
    assert_eq!(peak, 1.0, "opaque at the swap");
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    harness.step(10);
    assert_eq!(saves, 1, "the first frame's autosave, once");
    assert_eq!(std::fs::read(slot_file(&dir, AUTOSAVE)).ok(), written);
    let save = read(&dir, AUTOSAVE);
    assert_eq!(save.meta.difficulty, Difficulty::Normal);
    assert_eq!(save.player, PLAYER_START, "the new run's");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A run in `dir`, walked, then paused with `slot_1` holding a Hard run at
/// (50, 60), and Slot 1 activated on the Load screen.
fn load_under_the_curtain(dir: &Path) -> (UiHarness, [f32; 2]) {
    let mut harness = harness_with(UserFolder::At(dir.to_path_buf()));
    start(&mut harness);
    walk(&mut harness, 20);
    let walked = player_position(&harness).unwrap();
    let mut hard = save::capture(harness.world()).unwrap();
    (hard.meta.difficulty, hard.player) = (Difficulty::Hard, [50.0, 60.0]);
    SaveSlots::new(Some(dir.join("saves")))
        .save("slot_1", &hard)
        .unwrap();
    tap(&mut harness, "state_pause");
    press(&mut harness, "Load");
    let slot = find(&harness, "Slot 1").unwrap();
    assert!(harness.focus(slot));
    harness.harness_mut().press_action("ui_accept");
    harness.step(1);
    harness.harness_mut().release_action("ui_accept");
    assert!(tungsten_template_basic::shell::flow_pending(
        harness.world()
    ));
    (harness, walked)
}

/// How many more frames the sample player's `Health` stays hidden, and the
/// hidden `Health`.
struct Hidden(u32, Option<Health>);

/// A system before `shell_save` that hides the player's `Health` while
/// [`Hidden`] counts, so `capture` returns `None`, then gives it back.
struct HideHealth;

impl Plugin for HideHealth {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.insert_resource(Hidden(0, None));
        let hide = system("test_hide_health", hide_health).before("shell_save");
        schedule.add(Stage::PreUpdate, hide);
    }
}

fn hide_health(world: &mut World) {
    let Some(player) = world.get_resource::<SampleRun>().map(|run| run.player) else {
        return;
    };
    let Hidden(left, held) = world.remove_resource::<Hidden>().unwrap();
    let held = match left {
        0 => {
            if let Some(health) = held {
                world.insert(player, health);
            }
            None
        }
        _ => world.remove_component::<Health>(player).or(held),
    };
    world.insert_resource(Hidden(left.saturating_sub(1), held));
}

/// The game in `dir` with [`HideHealth`], hiding for `frames` frames of a
/// run once it starts.
fn hiding(dir: &Path, frames: u32) -> UiHarness {
    let mut app = super::app(UserFolder::At(dir.to_path_buf()));
    app.add_plugin(HideHealth);
    let mut harness = UiHarness::new(app);
    let manifest = harness.harness_mut().load_manifest(game::MANIFEST);
    manifest.expect("the manifest loads");
    harness.world_mut().insert_resource(Hidden(frames, None));
    harness.step(1);
    harness
}

/// The autosave's writes over `frames` frames, as changes of its bytes.
fn autosaves(harness: &mut UiHarness, dir: &Path, frames: u32) -> usize {
    let mut bytes = std::fs::read(slot_file(dir, AUTOSAVE)).ok();
    let mut writes = 0;
    for _ in 0..frames {
        harness.step(1);
        let now = std::fs::read(slot_file(dir, AUTOSAVE)).ok();
        writes += usize::from(now != bytes);
        bytes = now;
    }
    writes
}

#[test]
fn a_first_autosave_waits_for_a_capture() {
    let dir = user_folder("first-autosave-waits");
    let mut harness = hiding(&dir, u32::MAX);
    start(&mut harness);
    assert_eq!(autosaves(&mut harness, &dir, 10), 0, "none while hidden");
    assert!(!slot_file(&dir, AUTOSAVE).exists());
    harness.world_mut().resource_mut::<Hidden>().0 = 0;
    assert_eq!(
        autosaves(&mut harness, &dir, 20),
        1,
        "one, once it has a save"
    );
    assert_eq!(read(&dir, AUTOSAVE).player, PLAYER_START);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_requested_autosave_waits_for_a_capture() {
    let dir = user_folder("requested-autosave-waits");
    let mut harness = hiding(&dir, 0);
    start(&mut harness);
    assert_eq!(
        autosaves(&mut harness, &dir, 5),
        0,
        "the first came already"
    );
    assert!(slot_file(&dir, AUTOSAVE).exists());
    harness.world_mut().resource_mut::<Hidden>().0 = u32::MAX;
    harness.step(1);
    request_autosave(harness.world_mut());
    assert_eq!(autosaves(&mut harness, &dir, 10), 0, "none while hidden");
    harness.world_mut().resource_mut::<Hidden>().0 = 0;
    assert_eq!(autosaves(&mut harness, &dir, 20), 1, "one for the request");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_close_during_a_run_change_saves_one_run_whole() {
    // Before the swap: the old run, whole.
    let dir = user_folder("close-curtain-before");
    let (mut harness, walked) = load_under_the_curtain(&dir);
    harness.step(2);
    assert_eq!(
        harness.world().resource::<RunState>().difficulty,
        Difficulty::Normal
    );
    harness.harness_mut().request_close();
    assert_eq!(harness.step(1), 1, "the last frame runs");
    let save = read(&dir, AUTOSAVE);
    assert_eq!(
        (save.meta.difficulty, save.player),
        (Difficulty::Normal, walked)
    );
    let _ = std::fs::remove_dir_all(&dir);

    // Right after the swap, before the replace: the loaded run, whole.
    let dir = user_folder("close-curtain-after");
    let (mut harness, _) = load_under_the_curtain(&dir);
    while harness.world().resource::<RunState>().difficulty != Difficulty::Hard {
        assert_eq!(harness.step(1), 1);
    }
    harness.harness_mut().request_close();
    assert_eq!(harness.step(1), 1);
    let save = read(&dir, AUTOSAVE);
    assert_eq!(
        (save.meta.difficulty, save.player),
        (Difficulty::Hard, [50.0, 60.0])
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn one_screen_change_at_a_time() {
    let mut harness = harness();
    start(&mut harness);
    // Escape and a focus loss in one frame: one pause.
    harness.key_down(KeyCode::Escape);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .set_focused(false);
    harness.step(1);
    harness.key_up(KeyCode::Escape);
    settle(&mut harness);
    assert_eq!(top(&harness), Some(PAUSE));
    assert_eq!(depth(&harness), 2, "one pause");
    // A closing pause takes no second change: Resume twice pops once.
    let resume = find(&harness, "Resume").unwrap();
    assert!(harness.focus(resume));
    harness.harness_mut().press_action("ui_accept");
    harness.step(1);
    tungsten_template_basic::shell::push(
        harness.world_mut(),
        tungsten_template_basic::shell::pause::PauseState::default(),
    );
    harness.harness_mut().release_action("ui_accept");
    settle(&mut harness);
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    assert_eq!(depth(&harness), 1);
}

#[test]
fn delete_buttons_and_low_health_use_danger() {
    let dir = user_folder("danger");
    leave_autosave(&dir);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let danger = harness.tree().theme().tokens.colors.danger;
    press(&mut harness, "Load");
    let delete = find(&harness, "Delete Autosave").expect("a save to delete");
    assert_eq!(
        harness.tree().resolved_style(delete).unwrap().text.color,
        danger
    );
    tap(&mut harness, "ui_cancel");
    let _ = std::fs::remove_dir_all(&dir);

    let mut harness = super::harness();
    start(&mut harness);
    let accent = harness.tree().theme().tokens.colors.accent;
    let fill = |harness: &UiHarness| {
        let hud = harness.world().resource::<Hud>().fill;
        harness.tree().resolved_style(hud).unwrap().background
    };
    assert_eq!(fill(&harness), accent, "full health");
    let player = player_entity(&harness);
    for (hurt, color) in [(3.0, accent), (1.0, danger)] {
        // 5 → 2, above a third; 2 → 1, a third or less.
        harness.step(31);
        assert_eq!(damage(harness.world_mut(), player, hurt), Hit::Hurt);
        harness.step(1);
        assert_eq!(fill(&harness), color, "after {hurt}");
    }
}

/// The sounds this frame started, by handle.
fn played(harness: &UiHarness) -> Vec<tungsten::core::AudioHandle> {
    harness
        .harness()
        .audio()
        .iter()
        .filter_map(|command| match command {
            tungsten::core::AudioCommand::Play { handle, .. } => Some(*handle),
            _ => None,
        })
        .collect()
}

#[test]
fn menu_sounds_play_on_move_activate_and_back() {
    let mut harness = harness();
    let sounds = harness.world().resource::<tungsten::core::SoundRegistry>();
    let [moved, selected, back] =
        theme::MENU_SOUNDS.map(|name| sounds.get_by_id(name).expect("a manifest sound"));
    let once = |harness: &mut UiHarness, action: &str| {
        harness.harness_mut().press_action(action);
        harness.step(1);
        let heard = played(harness);
        harness.harness_mut().release_action(action);
        settle(harness);
        heard
    };
    assert_eq!(
        once(&mut harness, "ui_down"),
        [moved],
        "a press that moves focus"
    );
    let settings = find(&harness, "Settings").unwrap();
    assert!(harness.focus(settings));
    assert_eq!(once(&mut harness, "ui_accept"), [selected], "an activation");
    assert_eq!(
        top(&harness),
        Some(tungsten_template_basic::shell::settings::SETTINGS)
    );
    assert_eq!(once(&mut harness, "ui_cancel"), [back], "a cancel");
    assert_eq!(top(&harness), Some(TITLE));
    press(&mut harness, "Settings");
    let leave = find(&harness, "Back").unwrap();
    assert!(harness.focus(leave));
    assert_eq!(once(&mut harness, "ui_accept"), [back], "Back is a back");
    // Focus a screen moves itself makes no sound.
    press(&mut harness, "New Game");
    harness.step(1);
    assert!(played(&harness).is_empty());
}

#[test]
fn the_title_plays_its_song_until_a_run_starts() {
    let mut harness = harness();
    // The title came on the first frame, with its song.
    let first = harness.harness().audio().to_vec();
    let song = plays(&harness, &first, theme::TITLE_MUSIC);
    assert_eq!(song.len(), 1, "the title's song");
    let (song, looping, fade) = song[0];
    assert!(looping && fade == theme::TITLE_FADE);
    let shell =
        |harness: &UiHarness| tungsten_template_basic::shell::transitioning(harness.world());
    tungsten_template_basic::shell::start_run(harness.world_mut(), RunStart::default(), None);
    let mut started = heard(&mut harness, 1);
    while shell(&harness) {
        started.extend(heard(&mut harness, 1));
    }
    assert!(
        stops(&started).contains(&(song, theme::TITLE_FADE)),
        "a run fades it out"
    );
    tap(&mut harness, "state_pause");
    press(&mut harness, "Quit to Title");
    let quit = find(&harness, "Quit").unwrap();
    assert!(harness.focus(quit));
    let back = act(&mut harness, "ui_accept");
    assert_eq!(
        plays(&harness, &back, theme::TITLE_MUSIC).len(),
        1,
        "back again"
    );
}

#[test]
fn game_over_plays_its_sting_and_music_until_the_run_ends() {
    let mut harness = harness();
    start(&mut harness);
    let player = player_entity(&harness);
    assert_eq!(damage(harness.world_mut(), player, 99.0), Hit::Killed);
    let mut over = heard(&mut harness, 1);
    while tungsten_template_basic::shell::transitioning(harness.world()) {
        over.extend(heard(&mut harness, 1));
    }
    over.extend(heard(&mut harness, 30));
    assert_eq!(top(&harness), Some(GAME_OVER));
    let [sting, music] = theme::GAME_OVER_SOUNDS;
    assert_eq!(plays(&harness, &over, sting).len(), 1, "the sting, once");
    let music = plays(&harness, &over, music);
    assert_eq!(music.len(), 1, "game over's music");
    let (music, looping, fade) = music[0];
    assert!(looping && fade == theme::GAME_OVER_FADE);
    let quit = find(&harness, "Quit to Title").unwrap();
    assert!(harness.focus(quit));
    let ended = act(&mut harness, "ui_accept");
    assert!(
        stops(&ended).contains(&(music, theme::GAME_OVER_FADE)),
        "its music stops with the run"
    );
}
