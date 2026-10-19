//! Tabbed settings and the gameplay options: each tab's rows, the display
//! confirm and its revert, the focus mute, Reset and Back.

use std::num::NonZeroU32;

use tungsten::core::ui::UiTree;
use tungsten::core::{
    AudioBus, AudioCommand, CameraController, DisplayMode, DisplayState, FrameRateCap, InputState,
    SaveSlots, ScaleFactor, Settings, Time, UserFolder,
};
use tungsten::kit::{Hit, damage};
use tungsten::testing::UiHarness;
use tungsten_template_basic::game;
use tungsten_template_basic::sample::CAMERA_SHAKE;
use tungsten_template_basic::save::SaveGame;
use tungsten_template_basic::shell::confirm::CONFIRM;
use tungsten_template_basic::shell::options::GameOptions;
use tungsten_template_basic::shell::settings::{self, SETTINGS};
use tungsten_template_basic::shell::{AUTOSAVE, Difficulty, widgets};

use super::{
    find, harness, harness_with, health, named, player_entity, press, settle, start, tap, top,
    user_folder,
};

fn settings(harness: &UiHarness) -> &Settings {
    harness.world().resource::<Settings>()
}

fn options(harness: &UiHarness) -> GameOptions {
    *harness.world().resource::<GameOptions>()
}

/// The master volumes the last frame sent.
fn master_volumes(harness: &UiHarness) -> Vec<f32> {
    harness
        .harness()
        .audio()
        .iter()
        .filter_map(|command| match command {
            AudioCommand::SetMasterVolume(volume) => Some(*volume),
            _ => None,
        })
        .collect()
}

fn set_focus(harness: &mut UiHarness, focused: bool) {
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .set_focused(focused);
}

/// Focuses the control named `name` and taps `action` on it.
fn nudge(harness: &mut UiHarness, name: &str, action: &str) {
    let id = find(harness, name).unwrap_or_else(|| panic!("no control '{name}'"));
    assert!(harness.focus(id));
    tap(harness, action);
}

fn row_text(harness: &UiHarness, name: &str) -> String {
    widgets::text_of(harness.tree(), named(harness, name).expect("a row"))
}

/// Opens Settings from the title, on its Display tab.
fn from_title(harness: &mut UiHarness) {
    press(harness, "Settings");
    assert_eq!(top(harness), Some(SETTINGS));
}

#[test]
fn a_display_revert_keeps_the_focus_mute() {
    let mut harness = harness();
    harness
        .world_mut()
        .resource_mut::<GameOptions>()
        .mute_unfocused = true;
    from_title(&mut harness);
    press(&mut harness, "Window mode");
    assert_eq!(top(&harness), Some(CONFIRM));
    set_focus(&mut harness, false);
    // From the focus loss through the countdown's end and its revert.
    for frame in 0..700 {
        harness.step(1);
        let volumes = master_volumes(&harness);
        assert!(
            volumes.iter().all(|volume| *volume == 0.0),
            "frame {frame}: {volumes:?}"
        );
    }
    assert_eq!(top(&harness), Some(SETTINGS), "the countdown reverted");
    assert_eq!(settings(&harness).display_mode(), None);
    set_focus(&mut harness, true);
    harness.step(1);
    assert_eq!(master_volumes(&harness), [1.0], "focus back unmutes");
}

#[test]
fn retry_keeps_a_difficulty_changed_mid_run() {
    let dir = user_folder("retry-difficulty");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    harness.step(1);
    press(&mut harness, "Gameplay Options");
    press(&mut harness, "Difficulty");
    assert_eq!(row_text(&harness, "Difficulty"), "Hard");
    press(&mut harness, "Back");
    press(&mut harness, "Resume");
    let player = player_entity(&harness);
    assert_eq!(damage(harness.world_mut(), player, 99.0), Hit::Killed);
    harness.step(1);
    settle(&mut harness);
    press(&mut harness, "Retry");
    // Retry's first frame began under game over's held clock; the next
    // runs the first fixed step.
    harness.step(1);
    assert_eq!(health(&harness, player_entity(&harness)), (3.0, 3.0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tabs_switch_with_the_tab_row_and_ui_next() {
    let mut harness = harness();
    from_title(&mut harness);
    assert!(find(&harness, "Window mode").is_some());
    press(&mut harness, "Audio");
    assert!(find(&harness, "Master").is_some());
    assert_eq!(find(&harness, "Window mode"), None, "one tab at a time");
    tap(&mut harness, "ui_next");
    assert!(find(&harness, "UI scale").is_some(), "Interface");
    tap(&mut harness, "ui_prev");
    assert!(find(&harness, "Master").is_some(), "back to Audio");
    tap(&mut harness, "ui_prev");
    tap(&mut harness, "ui_prev");
    assert!(
        find(&harness, "Difficulty").is_none() && named(&harness, "Difficulty").is_some(),
        "wraps to Gameplay"
    );
}

#[test]
fn gameplay_options_opens_on_the_gameplay_tab() {
    let mut harness = harness();
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Gameplay Options");
    assert_eq!(top(&harness), Some(SETTINGS));
    assert!(find(&harness, "Difficulty").is_some(), "on in a run");
    assert_eq!(find(&harness, "Window mode"), None);
}

#[test]
fn a_window_mode_change_reverts_after_ten_seconds() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Window mode");
    assert_eq!(
        settings(&harness).display_mode(),
        Some(DisplayMode::BorderlessFullscreen)
    );
    assert_eq!(top(&harness), Some(CONFIRM));
    harness.step(9 * 60);
    assert_eq!(top(&harness), Some(CONFIRM), "still asking at 9 s");
    harness.step(70);
    settle(&mut harness);
    assert_eq!(top(&harness), Some(SETTINGS));
    assert_eq!(settings(&harness).display_mode(), None, "reverted");
    assert_eq!(row_text(&harness, "Window mode"), "Windowed");
}

#[test]
fn keep_confirms_the_display_change() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Window mode");
    press(&mut harness, "Keep");
    assert_eq!(top(&harness), Some(SETTINGS));
    assert_eq!(
        settings(&harness).display_mode(),
        Some(DisplayMode::BorderlessFullscreen)
    );
    assert_eq!(row_text(&harness, "Window mode"), "Fullscreen");
}

#[test]
fn window_size_is_disabled_in_fullscreen() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Window size");
    press(&mut harness, "Keep");
    assert_eq!(row_text(&harness, "Window size"), "1600×900");
    press(&mut harness, "Window mode");
    press(&mut harness, "Keep");
    assert_eq!(find(&harness, "Window size"), None, "off in fullscreen");
    assert_eq!(
        settings(&harness).resolution().map(|size| size.width),
        Some(1600)
    );
}

#[test]
fn the_frame_rate_limit_writes_settings_and_the_display_request() {
    let dir = user_folder("frame-limit");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    from_title(&mut harness);
    assert_eq!(row_text(&harness, "Frame rate limit"), "Off");
    press(&mut harness, "Frame rate limit");
    let fps = |n| Some(FrameRateCap::Fps(NonZeroU32::new(n).unwrap()));
    assert_eq!(settings(&harness).frame_rate_cap(), fps(30));
    press(&mut harness, "Frame rate limit");
    assert_eq!(settings(&harness).frame_rate_cap(), fps(60));
    assert_eq!(row_text(&harness, "Frame rate limit"), "60");
    press(&mut harness, "Back");
    // Headless, a display request has no window to reach, so a restart
    // shows where the cap lands.
    let restarted = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(
        restarted.world().resource::<Settings>().frame_rate_cap(),
        fps(60)
    );
    assert_eq!(
        restarted.world().resource::<DisplayState>().frame_rate_cap,
        Some(60)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn volumes_write_settings_and_audio_commands() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Audio");
    let master = find(&harness, "Master").unwrap();
    assert!(harness.focus(master));
    harness.harness_mut().press_action("ui_left");
    harness.step(1);
    assert_eq!(master_volumes(&harness), [0.95]);
    harness.harness_mut().release_action("ui_left");
    harness.step(1);
    assert_eq!(settings(&harness).master_volume(), Some(0.95));
    let music = find(&harness, "Music").unwrap();
    assert!(harness.focus(music));
    harness.harness_mut().press_action("ui_left");
    harness.step(1);
    let sent = harness.harness().audio().iter().any(|command| {
        matches!(command, AudioCommand::SetBusVolume { bus: AudioBus::Music, volume, .. } if *volume == 0.95)
    });
    assert!(sent, "{:?}", harness.harness().audio());
    assert_eq!(settings(&harness).bus_volume(AudioBus::Music), Some(0.95));
}

#[test]
fn ui_and_text_scale_apply() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Interface");
    nudge(&mut harness, "UI scale", "ui_right");
    assert_eq!(harness.world().resource::<ScaleFactor>().ui_scale(), 1.25);
    nudge(&mut harness, "Text scale", "ui_right");
    assert_eq!(harness.world().resource::<UiTree>().text_scale(), 1.25);
    assert_eq!(settings(&harness).text_scale(), Some(1.25));
}

#[test]
fn screen_shake_scales_the_camera() {
    let mut harness = harness();
    let shipped = harness
        .world()
        .resource::<CameraController>()
        .shake_max_offset;
    assert_eq!(
        shipped.to_array(),
        CAMERA_SHAKE,
        "the sample's shake, at 100 %"
    );
    from_title(&mut harness);
    press(&mut harness, "Gameplay");
    nudge(&mut harness, "Screen shake", "ui_left");
    assert_eq!(options(&harness).screen_shake(), 0.95);
    let offset = harness
        .world()
        .resource::<CameraController>()
        .shake_max_offset;
    assert!((offset - shipped * 0.95).length() < 1e-4, "{offset:?}");
}

#[test]
fn damage_numbers_off_floats_no_label() {
    let mut harness = harness();
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Gameplay Options");
    press(&mut harness, "Damage numbers");
    assert!(!options(&harness).damage_numbers);
    press(&mut harness, "Back");
    press(&mut harness, "Resume");
    harness.step(1);
    let player = player_entity(&harness);
    assert_eq!(damage(harness.world_mut(), player, 1.0), Hit::Hurt);
    harness.step(1);
    let draw = harness.harness().draw().unwrap();
    assert!(draw.world_labels.is_empty(), "no label with the option off");
}

#[test]
fn difficulty_is_off_outside_a_run_and_saved_with_it() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Gameplay");
    assert_eq!(find(&harness, "Difficulty"), None);
    assert_eq!(row_text(&harness, "Difficulty"), "Chosen at New Game");

    let dir = user_folder("difficulty-saved");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Gameplay Options");
    press(&mut harness, "Difficulty");
    press(&mut harness, "Difficulty");
    assert_eq!(row_text(&harness, "Difficulty"), "Easy");
    press(&mut harness, "Back");
    press(&mut harness, "Quit to Title");
    let save = SaveSlots::new(Some(dir.join("saves")))
        .load::<SaveGame>(AUTOSAVE)
        .unwrap();
    assert_eq!(save.meta.difficulty, Difficulty::Easy, "saved with the run");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mute_when_unfocused_mutes_and_restores() {
    let mut harness = harness();
    harness
        .world_mut()
        .resource_mut::<GameOptions>()
        .mute_unfocused = true;
    start(&mut harness);
    set_focus(&mut harness, false);
    harness.step(1);
    assert_eq!(master_volumes(&harness), [0.0]);
    assert_eq!(settings(&harness).master_volume(), None, "never written");
    set_focus(&mut harness, true);
    harness.step(1);
    assert_eq!(master_volumes(&harness), [1.0]);
}

#[test]
fn pause_when_unfocused_off_keeps_playing() {
    let mut harness = harness();
    harness
        .world_mut()
        .resource_mut::<GameOptions>()
        .pause_unfocused = false;
    start(&mut harness);
    set_focus(&mut harness, false);
    harness.step(2);
    settle(&mut harness);
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    assert!(!harness.world().resource::<Time>().is_paused());
}

#[test]
fn reset_tab_restores_only_that_tab() {
    let mut harness = harness();
    settings::set_volume(harness.world_mut(), None, 40.0);
    settings::set_scale(harness.world_mut(), false, 1.5);
    from_title(&mut harness);
    press(&mut harness, "Audio");
    press(&mut harness, "Reset tab");
    assert_eq!(settings(&harness).master_volume(), None);
    assert_eq!(
        settings(&harness).ui_scale(),
        Some(1.5),
        "Interface's stays"
    );
}

#[test]
fn back_saves_settings_and_options_across_a_restart() {
    let dir = user_folder("settings-back");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    from_title(&mut harness);
    press(&mut harness, "Audio");
    nudge(&mut harness, "Master", "ui_left");
    press(&mut harness, "Gameplay");
    press(&mut harness, "Damage numbers");
    press(&mut harness, "Back");
    assert_eq!(
        top(&harness),
        Some(tungsten_template_basic::shell::title::TITLE)
    );
    let restarted = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(
        restarted.world().resource::<Settings>().master_volume(),
        Some(0.95)
    );
    assert!(!restarted.world().resource::<GameOptions>().damage_numbers);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shell_values_clamp_and_ignore_non_finite_input() {
    let mut harness = harness();
    let world = harness.world_mut();
    settings::set_volume(world, None, 150.0);
    assert_eq!(world.resource::<Settings>().master_volume(), Some(1.0));
    settings::set_volume(world, None, f32::NAN);
    assert_eq!(
        world.resource::<Settings>().master_volume(),
        Some(1.0),
        "ignored"
    );
    settings::set_volume(world, Some(AudioBus::Effects), -5.0);
    assert_eq!(
        world.resource::<Settings>().bus_volume(AudioBus::Effects),
        Some(0.0)
    );
    settings::set_scale(world, false, 10.0);
    assert_eq!(world.resource::<Settings>().ui_scale(), Some(3.0));
    settings::set_scale(world, true, f32::INFINITY);
    assert_eq!(world.resource::<Settings>().text_scale(), None, "ignored");
    settings::set_scale(world, true, 0.1);
    assert_eq!(world.resource::<Settings>().text_scale(), Some(0.75));
    let options = world.resource_mut::<GameOptions>();
    options.set_screen_shake(2.0);
    assert_eq!(options.screen_shake(), 1.0);
    options.set_screen_shake(-1.0);
    assert_eq!(options.screen_shake(), 0.0);
    options.set_screen_shake(f32::NAN);
    assert_eq!(options.screen_shake(), 0.0, "ignored");
}
