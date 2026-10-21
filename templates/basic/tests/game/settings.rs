//! Tabbed settings and the gameplay options: each tab's rows, the display
//! confirm and its revert, the focus mute, Reset and Back.

use std::num::NonZeroU32;

use tungsten::core::post::{BloomParams, PostPass, PostStack, VignetteParams};
use tungsten::core::ui::{UiTree, WidgetKind};
use tungsten::core::{
    AudioBus, AudioCommand, CameraController, DisplayMode, DisplayState, FrameRateCap, InputState,
    KeyCode, MonitorInfo, Resolution, SaveSlots, ScaleFactor, Settings, Time, UserFolder,
    VideoMode,
};
use tungsten::kit::{Hit, damage};
use tungsten::testing::UiHarness;
use tungsten::{WindowSize, shipped_volume};
use tungsten_template_basic::game;
use tungsten_template_basic::sample::CAMERA_SHAKE;
use tungsten_template_basic::save::SaveGame;
use tungsten_template_basic::shell::confirm::CONFIRM;
use tungsten_template_basic::shell::options::GameOptions;
use tungsten_template_basic::shell::settings::{self, SETTINGS};
use tungsten_template_basic::shell::title::TitleState;
use tungsten_template_basic::shell::{AUTOSAVES, Difficulty, theme, widgets};

use super::{
    click, find, harness, harness_with, health, named, player_entity, press, settle, start, tap,
    top, user_folder,
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
    press(&mut harness, "Apply");
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
    assert_eq!(
        master_volumes(&harness),
        [0.8],
        "focus back unmutes, to the shipped volume"
    );
}

#[test]
fn retry_keeps_a_difficulty_changed_mid_run() {
    let dir = user_folder("retry-difficulty");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    harness.step(1);
    press(&mut harness, "Settings");
    press(&mut harness, "Gameplay");
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
fn pause_reaches_the_gameplay_options_through_settings() {
    let mut harness = harness();
    start(&mut harness);
    tap(&mut harness, "state_pause");
    assert_eq!(named(&harness, "Gameplay Options"), None, "not on pause");
    press(&mut harness, "Settings");
    assert_eq!(top(&harness), Some(SETTINGS));
    press(&mut harness, "Gameplay");
    assert!(find(&harness, "Difficulty").is_some(), "on in a run");
    assert_eq!(find(&harness, "Window mode"), None);
}

#[test]
fn window_sizes_cycle_until_apply_sends_them() {
    let mut harness = harness();
    from_title(&mut harness);
    assert_eq!(find(&harness, "Apply"), None, "nothing to apply");
    press(&mut harness, "Window size");
    press(&mut harness, "Reset tab");
    assert_eq!(
        row_text(&harness, "Window size"),
        "1280×720 (16:9)",
        "Reset drops it"
    );
    for _ in 0..3 {
        press(&mut harness, "Window size");
    }
    assert_eq!(top(&harness), Some(SETTINGS), "no question while picking");
    assert_eq!(settings(&harness).resolution(), None, "nothing sent");
    assert_eq!(row_text(&harness, "Window size"), "2560×1440 (16:9)");
    press(&mut harness, "Apply");
    assert_eq!(top(&harness), Some(CONFIRM));
    assert_eq!(
        settings(&harness).resolution().map(|size| size.width),
        Some(2560)
    );
    press(&mut harness, "Revert");
    assert_eq!(settings(&harness).resolution(), None, "reverted");
    assert_eq!(row_text(&harness, "Window size"), "1280×720 (16:9)");
    assert_eq!(find(&harness, "Apply"), None, "nothing left to apply");
}

#[test]
fn a_window_mode_change_reverts_after_ten_seconds() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Window mode");
    assert_eq!(settings(&harness).display_mode(), None, "picked, not sent");
    assert_eq!(row_text(&harness, "Window mode"), "Fullscreen");
    press(&mut harness, "Apply");
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
fn f11_and_f9_are_kept_in_settings_and_survive_a_restart() {
    let dir = user_folder("display-keys");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let vsync = harness.world().resource::<DisplayState>().vsync;
    for key in [KeyCode::F11, KeyCode::F9] {
        harness.key_down(key);
        harness.step(1);
        harness.key_up(key);
        harness.step(1);
    }
    let fullscreen = Some(DisplayMode::BorderlessFullscreen);
    assert_eq!(settings(&harness).display_mode(), fullscreen, "F11 is kept");
    assert_eq!(settings(&harness).vsync(), Some(!vsync), "F9 is kept");
    // The Display rows read the set value when Settings opens.
    from_title(&mut harness);
    assert_eq!(row_text(&harness, "Window mode"), "Fullscreen");
    // Saved at the press: a restart starts fullscreen.
    let restarted = harness_with(UserFolder::At(dir.clone()));
    let display = *restarted.world().resource::<DisplayState>();
    assert_eq!(display.display_mode, DisplayMode::BorderlessFullscreen);
    assert_eq!(display.vsync, !vsync);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keep_confirms_the_display_change() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Window mode");
    press(&mut harness, "Apply");
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
    press(&mut harness, "Apply");
    press(&mut harness, "Keep");
    assert_eq!(row_text(&harness, "Window size"), "1600×900 (16:9)");
    press(&mut harness, "Window mode");
    assert_eq!(find(&harness, "Window size"), None, "off once picked");
    press(&mut harness, "Apply");
    press(&mut harness, "Keep");
    assert_eq!(find(&harness, "Window size"), None, "off in fullscreen");
    assert_eq!(
        settings(&harness).resolution().map(|size| size.width),
        Some(1600)
    );
}

/// The text of the readout after the slider named `name`.
fn readout(harness: &UiHarness, name: &str) -> String {
    let tree = harness.tree();
    let slider = named(harness, name).expect("a slider");
    let row = tree.parent(slider).unwrap();
    let readout = *tree.children(row).last().unwrap();
    widgets::text_of(tree, readout)
}

fn fps(n: u32) -> Option<FrameRateCap> {
    Some(FrameRateCap::Fps(NonZeroU32::new(n).unwrap()))
}

fn toggled(harness: &UiHarness, name: &str) -> Option<bool> {
    harness.tree().toggled(named(harness, name)?)
}

/// The toggles a player can reach on the screen on top, by name, found by
/// their kind in tree order.
fn shown_toggles(harness: &UiHarness) -> Vec<String> {
    let tree = harness.tree();
    let mut stack: Vec<_> = tree.roots().iter().rev().copied().collect();
    let mut names = Vec::new();
    while let Some(id) = stack.pop() {
        if tree.kind(id) == Some(WidgetKind::Toggle) && tree.can_focus(id) {
            names.extend(tree.accessible_name(id).map(str::to_owned));
        }
        stack.extend(tree.children(id).iter().rev().copied());
    }
    names
}

/// The value the toggle named `name` applies, as the world holds it.
fn applied(harness: &UiHarness, name: &str) -> bool {
    let (settings, options) = (settings(harness), options(harness));
    let live = *harness.world().resource::<DisplayState>();
    match name {
        "VSync" => settings.vsync().unwrap_or(live.vsync),
        "Uncapped" => settings
            .frame_rate_cap()
            .map_or(live.frame_rate_cap.is_none(), |cap| {
                cap == FrameRateCap::Uncapped
            }),
        "Mute when unfocused" => options.mute_unfocused,
        "Damage numbers" => options.damage_numbers,
        "Pause when unfocused" => options.pause_unfocused,
        "Autosave" => options.autosave,
        "Bloom" => options.bloom,
        other => panic!("a toggle this test does not know: '{other}'"),
    }
}

/// Acts on the toggle named `name` twice: its box and the value it applies
/// flip at the first and flip back at the second.
fn flips_twice(harness: &mut UiHarness, name: &str, how: &str) {
    let was = applied(harness, name);
    assert_eq!(
        toggled(harness, name),
        Some(was),
        "{name}: the box shows it"
    );
    for (time, want) in [(1, !was), (2, was)] {
        match how {
            "ui_accept" => press(harness, name),
            _ => click(harness, name),
        }
        let now = (toggled(harness, name), applied(harness, name));
        assert_eq!(now, (Some(want), want), "{name}, {how}, time {time}");
    }
}

#[test]
fn every_toggle_takes_its_first_click() {
    for from_pause in [false, true] {
        let mut harness = harness();
        if from_pause {
            start(&mut harness);
            tap(&mut harness, "state_pause");
        }
        let mut seen = 0;
        for (at, tab) in settings::TABS.into_iter().enumerate() {
            // Right after Settings opens, on its Display tab, or right after
            // a switch to the toggle's tab; then a switch away and back.
            press(&mut harness, "Settings");
            if at > 0 {
                press(&mut harness, tab);
            }
            let names = shown_toggles(&harness);
            for name in &names {
                let first = format!("a click right after {tab} shows");
                flips_twice(&mut harness, name, &first);
                press(
                    &mut harness,
                    settings::TABS[(at + 1) % settings::TABS.len()],
                );
                press(&mut harness, tab);
                flips_twice(&mut harness, name, "a click after a tab switch");
                flips_twice(&mut harness, name, "ui_accept");
            }
            seen += names.len();
            press(&mut harness, "Back");
        }
        assert_eq!(seen, 7, "the seven toggles, from_pause {from_pause}");
    }
}

#[test]
fn the_frame_rate_limit_is_a_slider_and_an_uncapped_toggle() {
    let dir = user_folder("frame-limit");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    from_title(&mut harness);
    // Unset: the template ships no cap, so Uncapped is on and the slider
    // rests at 60, off.
    assert_eq!(toggled(&harness, "Uncapped"), Some(true));
    assert_eq!(
        find(&harness, "Frame rate limit"),
        None,
        "off while Uncapped"
    );
    assert_eq!(readout(&harness, "Frame rate limit"), "60 fps");
    press(&mut harness, "Uncapped");
    assert_eq!(settings(&harness).frame_rate_cap(), fps(60));
    nudge(&mut harness, "Frame rate limit", "ui_right");
    assert_eq!(settings(&harness).frame_rate_cap(), fps(65));
    assert_eq!(readout(&harness, "Frame rate limit"), "65 fps");
    press(&mut harness, "Uncapped");
    assert_eq!(
        settings(&harness).frame_rate_cap(),
        Some(FrameRateCap::Uncapped)
    );
    assert_eq!(find(&harness, "Frame rate limit"), None);
    press(&mut harness, "Uncapped");
    assert_eq!(settings(&harness).frame_rate_cap(), fps(65), "the slider's");
    nudge(&mut harness, "Frame rate limit", "ui_right");
    assert_eq!(settings(&harness).frame_rate_cap(), fps(70));
    press(&mut harness, "Back");
    // Headless, a display request has no window to reach, so a restart
    // shows where the cap lands.
    let restarted = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(
        restarted.world().resource::<Settings>().frame_rate_cap(),
        fps(70)
    );
    assert_eq!(
        restarted.world().resource::<DisplayState>().frame_rate_cap,
        Some(70)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_frame_rate_limit_stops_at_sixty() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Uncapped");
    assert_eq!(settings(&harness).frame_rate_cap(), fps(60));
    for action in ["ui_right"; 2].into_iter().chain(["ui_left"; 10]) {
        nudge(&mut harness, "Frame rate limit", action);
    }
    assert_eq!(settings(&harness).frame_rate_cap(), fps(60), "the floor");
    assert_eq!(readout(&harness, "Frame rate limit"), "60 fps");
}

#[test]
fn a_stored_cap_under_sixty_applies_and_reads_sixty() {
    let dir = user_folder("cap-under-floor");
    std::fs::create_dir_all(dir.join("settings")).unwrap();
    let file = dir.join("settings").join("settings.json");
    std::fs::write(&file, r#"{ "display": { "frame_rate_cap": 30 } }"#).unwrap();
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(settings(&harness).frame_rate_cap(), fps(30));
    assert_eq!(
        harness.world().resource::<DisplayState>().frame_rate_cap,
        Some(30),
        "applied as stored"
    );
    from_title(&mut harness);
    assert_eq!(toggled(&harness, "Uncapped"), Some(false));
    let slider = named(&harness, "Frame rate limit").unwrap();
    assert_eq!(harness.tree().slider_value(slider), Some(60.0));
    assert_eq!(readout(&harness, "Frame rate limit"), "60 fps");
    press(&mut harness, "Back");
    assert_eq!(
        settings(&harness).frame_rate_cap(),
        fps(30),
        "Back keeps it"
    );
    from_title(&mut harness);
    nudge(&mut harness, "Frame rate limit", "ui_right");
    assert_eq!(settings(&harness).frame_rate_cap(), fps(65));
    assert_eq!(readout(&harness, "Frame rate limit"), "65 fps");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn window_sizes_show_their_aspect_ratio() {
    let sizes = [
        ((1280, 720), "16:9"),
        ((1366, 768), "16:9"),
        ((1920, 1200), "16:10"),
        ((1280, 1024), "5:4"),
        ((1024, 768), "4:3"),
        ((2560, 1080), "21:9"),
        ((3440, 1440), "21:9"),
        ((5120, 1440), "32:9"),
        ((1280, 768), "5:3"),
    ];
    for ((width, height), ratio) in sizes {
        assert_eq!(settings::aspect(width, height), ratio, "{width}×{height}");
    }
}

#[test]
fn an_off_grid_cap_keeps_its_value_until_moved() {
    let dir = user_folder("off-grid-cap");
    std::fs::create_dir_all(dir.join("settings")).unwrap();
    let file = dir.join("settings").join("settings.json");
    std::fs::write(&file, r#"{ "display": { "frame_rate_cap": 144 } }"#).unwrap();
    for _ in 0..2 {
        // Shown exactly and kept, across a Back and a restart.
        let mut harness = harness_with(UserFolder::At(dir.clone()));
        from_title(&mut harness);
        assert_eq!(settings(&harness).frame_rate_cap(), fps(144));
        assert_eq!(readout(&harness, "Frame rate limit"), "144 fps");
        assert_eq!(toggled(&harness, "Uncapped"), Some(false));
        press(&mut harness, "Back");
        assert_eq!(settings(&harness).frame_rate_cap(), fps(144));
    }
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    from_title(&mut harness);
    nudge(&mut harness, "Frame rate limit", "ui_right");
    let Some(FrameRateCap::Fps(moved)) = settings(&harness).frame_rate_cap() else {
        panic!("a cap");
    };
    assert!(
        moved.get() % 5 == 0 && moved.get() > 144,
        "a grid value: {moved}"
    );
    assert_eq!(
        readout(&harness, "Frame rate limit"),
        format!("{moved} fps")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_slider_shows_its_value_as_it_moves() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Audio");
    for name in ["Master", "Music", "Effects"] {
        assert_eq!(readout(&harness, name), "80%", "{name}");
    }
    nudge(&mut harness, "Effects", "ui_left");
    assert_eq!(readout(&harness, "Effects"), "75%");
    press(&mut harness, "Gameplay");
    assert_eq!(readout(&harness, "Screen shake"), "100%");
    nudge(&mut harness, "Screen shake", "ui_left");
    assert_eq!(readout(&harness, "Screen shake"), "95%");
    press(&mut harness, "Interface");
    assert_eq!(readout(&harness, "UI scale"), "75%", "the shell's default");
    assert_eq!(readout(&harness, "Text scale"), "100%");
    nudge(&mut harness, "Text scale", "ui_left");
    assert_eq!(readout(&harness, "Text scale"), "75%");
    // A held scale slider shows its value at once and applies it at the
    // release, so the UI does not rescale under the pointer.
    let slider = find(&harness, "UI scale").unwrap();
    let rect = harness.rect(slider).unwrap();
    let scale = harness.world().resource::<ScaleFactor>().get();
    let y = (rect.min.y + rect.max.y) * 0.5 * scale;
    harness.pointer_to(rect.min.x * scale + 1.0, y);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(rect.max.x * scale + 40.0, y);
    harness.step(1);
    assert_eq!(readout(&harness, "UI scale"), "200%", "the most");
    assert_eq!(
        harness.world().resource::<ScaleFactor>().ui_scale(),
        theme::UI_SCALE
    );
    harness.pointer_up();
    harness.step(1);
    assert_eq!(harness.world().resource::<ScaleFactor>().ui_scale(), 2.0);
    assert_eq!(readout(&harness, "UI scale"), "200%");
}

#[test]
fn volumes_ship_at_eighty_percent_and_reset_to_it() {
    let dir = user_folder("shipped-volumes");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    for bus in [None, Some(AudioBus::Music), Some(AudioBus::Effects)] {
        assert_eq!(shipped_volume(harness.world(), bus), 0.8, "{bus:?}");
    }
    assert_eq!(settings(&harness).master_volume(), None, "nothing set");
    from_title(&mut harness);
    press(&mut harness, "Audio");
    nudge(&mut harness, "Master", "ui_left");
    nudge(&mut harness, "Music", "ui_right");
    assert_eq!(settings(&harness).master_volume(), Some(0.75));
    assert_eq!(settings(&harness).bus_volume(AudioBus::Music), Some(0.85));
    let reset = find(&harness, "Reset tab").unwrap();
    assert!(harness.focus(reset));
    harness.harness_mut().press_action("ui_accept");
    harness.step(1);
    let sent = harness.harness().audio().to_vec();
    assert!(
        sent.contains(&AudioCommand::SetMasterVolume(0.8)),
        "{sent:?}"
    );
    harness.harness_mut().release_action("ui_accept");
    settle(&mut harness);
    assert_eq!(settings(&harness).master_volume(), None);
    assert_eq!(settings(&harness).bus_volume(AudioBus::Music), None);
    for name in ["Master", "Music", "Effects"] {
        assert_eq!(readout(&harness, name), "80%", "{name} after Reset tab");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn window_sizes_follow_the_monitor() {
    let mut harness = harness();
    let size = |width, height| Resolution { width, height };
    let fixed: Vec<Resolution> = settings::SIZES
        .iter()
        .map(|(width, height)| size(*width, *height))
        .collect();
    assert_eq!(settings::window_sizes(harness.world()), fixed, "no monitor");
    // X11 lists modes past the current one, and smaller ones.
    let modes = [
        (4096, 2160),
        (3840, 2160),
        (2560, 1440),
        (1920, 1080),
        (1680, 1050),
        (1280, 1024),
        (1280, 720),
        (1024, 768),
    ]
    .map(|(width, height)| VideoMode::new(size(width, height), 60_000));
    let x11 = MonitorInfo::new(size(3840, 2160), Some(60_000), modes).unwrap();
    harness.world_mut().insert_resource(x11);
    let listed = [
        (1280, 720),
        (1280, 1024),
        (1680, 1050),
        (1920, 1080),
        (2560, 1440),
        (3840, 2160),
    ]
    .map(|(width, height)| size(width, height));
    assert_eq!(settings::window_sizes(harness.world()), listed);
    from_title(&mut harness);
    press(&mut harness, "Window size");
    press(&mut harness, "Apply");
    press(&mut harness, "Keep");
    assert_eq!(
        row_text(&harness, "Window size"),
        "1280×1024 (5:4)",
        "the next mode"
    );
    // A Wayland output often lists its current mode alone.
    let one = MonitorInfo::new(
        size(1920, 1080),
        None,
        [VideoMode::new(size(1920, 1080), 60_000)],
    );
    harness.world_mut().insert_resource(one.unwrap());
    assert_eq!(
        settings::window_sizes(harness.world()),
        [size(1280, 720), size(1600, 900), size(1920, 1080)]
    );
}

#[test]
fn ui_scale_follows_the_window_height_with_a_floor() {
    let mut harness = harness();
    let scale = *harness.world().resource::<ScaleFactor>();
    assert_eq!(scale.reference_height(), Some(theme::REFERENCE_HEIGHT));
    assert_eq!(scale.min_viewport(), Some(theme::MIN_VIEWPORT.into()));
    let viewport = |harness: &mut UiHarness, window: [u32; 2]| {
        harness.world_mut().insert_resource(WindowSize {
            width: window[0],
            height: window[1],
        });
        harness.step(1);
        let get = harness.world().resource::<ScaleFactor>().get();
        [window[0] as f32 / get, window[1] as f32 / get]
    };
    // At the shell's 75 %: 4K at 3 × 0.75, and 1280 × 720 at 0.75.
    let default = [1280.0, 720.0].map(|side| side / theme::UI_SCALE);
    for window in [[3840, 2160], [1280, 720]] {
        let [width, height] = viewport(&mut harness, window);
        assert!(
            (width - default[0]).abs() < 0.01 && (height - default[1]).abs() < 0.01,
            "{window:?}: {width}×{height}"
        );
    }
    settings::set_scale(harness.world_mut(), false, 2.0);
    for window in [[1280, 720], [3840, 2160]] {
        let [width, height] = viewport(&mut harness, window);
        assert!(
            (width - 731.0).abs() < 0.01 && height >= 411.0,
            "{window:?}: {width}×{height}"
        );
    }
    settings::set_scale(harness.world_mut(), false, 1.0);
    harness
        .world_mut()
        .resource_mut::<ScaleFactor>()
        .set_os(1.75);
    let [width, height] = viewport(&mut harness, [1280, 720]);
    assert!(
        (width - 731.4).abs() < 0.1 && (height - 411.4).abs() < 0.1,
        "{width}×{height}"
    );
}

#[test]
fn volumes_write_settings_and_audio_commands() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Audio");
    // From the shipped 80 %, a step of 5.
    let master = find(&harness, "Master").unwrap();
    assert!(harness.focus(master));
    harness.harness_mut().press_action("ui_left");
    harness.step(1);
    assert_eq!(master_volumes(&harness), [0.75]);
    harness.harness_mut().release_action("ui_left");
    harness.step(1);
    assert_eq!(settings(&harness).master_volume(), Some(0.75));
    let music = find(&harness, "Music").unwrap();
    assert!(harness.focus(music));
    harness.harness_mut().press_action("ui_left");
    harness.step(1);
    let sent = harness.harness().audio().iter().any(|command| {
        matches!(command, AudioCommand::SetBusVolume { bus: AudioBus::Music, volume, .. } if *volume == 0.75)
    });
    assert!(sent, "{:?}", harness.harness().audio());
    assert_eq!(settings(&harness).bus_volume(AudioBus::Music), Some(0.75));
}

#[test]
fn sliders_have_a_smaller_outlined_thumb() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Interface");
    let slider = find(&harness, "UI scale").unwrap();
    harness.layout();
    let height = harness.rect(slider).unwrap().height();
    assert_eq!(height, theme::SLIDER_LINE, "at text 100 %");
    let border = |harness: &UiHarness| {
        let style = harness.tree().resolved_style(slider).unwrap();
        (style.border, style.border_width)
    };
    // Painted on the thumb, which it parts from the fill and the track.
    assert_eq!(border(&harness), ([0x0b, 0x0c, 0x10, 255], 1.0));
    assert!(harness.focus(slider));
    harness.step(1);
    let focus = harness.tree().theme().tokens.colors.focus;
    assert_eq!(focus, [0xff, 0xd3, 0x6b, 255]);
    assert_eq!(border(&harness), (focus, 2.0));
}

#[test]
fn the_ui_scale_defaults_to_75_percent() {
    let dir = user_folder("ui-scale-default");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let scale = |harness: &UiHarness| *harness.world().resource::<ScaleFactor>();
    assert_eq!(settings(&harness).ui_scale(), None, "unset");
    assert_eq!(theme::UI_SCALE, 0.75);
    assert_eq!(scale(&harness).ui_scale(), 0.75, "from the first frame");
    let mut set = super::harness();
    settings::set_scale(set.world_mut(), false, 0.75);
    set.step(1);
    assert_eq!(settings(&set).ui_scale(), Some(0.75));
    assert_eq!(scale(&harness).get(), scale(&set).get(), "a player's 75 %");
    from_title(&mut harness);
    press(&mut harness, "Interface");
    assert_eq!(readout(&harness, "UI scale"), "75%");
    nudge(&mut harness, "UI scale", "ui_right");
    assert_eq!(settings(&harness).ui_scale(), Some(1.0));
    assert_eq!(scale(&harness).ui_scale(), 1.0);
    press(&mut harness, "Reset tab");
    assert_eq!(settings(&harness).ui_scale(), None);
    assert_eq!(scale(&harness).ui_scale(), 0.75);
    assert_eq!(readout(&harness, "UI scale"), "75%");
    press(&mut harness, "Back");
    let restarted = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(settings(&restarted).ui_scale(), None, "kept unset");
    assert_eq!(scale(&restarted).ui_scale(), 0.75);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ui_and_text_scale_apply() {
    let mut harness = harness();
    from_title(&mut harness);
    press(&mut harness, "Interface");
    nudge(&mut harness, "UI scale", "ui_right");
    assert_eq!(harness.world().resource::<ScaleFactor>().ui_scale(), 1.0);
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
    press(&mut harness, "Settings");
    press(&mut harness, "Gameplay");
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
    press(&mut harness, "Settings");
    press(&mut harness, "Gameplay");
    press(&mut harness, "Difficulty");
    press(&mut harness, "Difficulty");
    assert_eq!(row_text(&harness, "Difficulty"), "Easy");
    press(&mut harness, "Back");
    // Quit to Title rewrites pause's autosave, the second.
    press(&mut harness, "Quit to Title");
    let save = SaveSlots::new(Some(dir.join("saves")))
        .load::<SaveGame>(AUTOSAVES[1])
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
    assert_eq!(master_volumes(&harness), [0.8], "the shipped volume");
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
        Some(0.75)
    );
    assert!(!restarted.world().resource::<GameOptions>().damage_numbers);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_close_in_settings_keeps_the_changes_but_not_an_unkept_display() {
    let dir = user_folder("settings-close");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    from_title(&mut harness);
    press(&mut harness, "Audio");
    nudge(&mut harness, "Master", "ui_left");
    press(&mut harness, "Gameplay");
    press(&mut harness, "Damage numbers");
    press(&mut harness, "Display");
    press(&mut harness, "Window mode");
    press(&mut harness, "Apply");
    assert_eq!(top(&harness), Some(CONFIRM), "asked to keep it");
    harness.harness_mut().request_close();
    assert_eq!(harness.step(1), 1, "the last frame runs");
    let restarted = harness_with(UserFolder::At(dir.clone()));
    let settings = restarted.world().resource::<Settings>();
    assert_eq!(settings.master_volume(), Some(0.75), "saved at the close");
    assert_eq!(settings.display_mode(), None, "the unkept mode reverted");
    assert!(!restarted.world().resource::<GameOptions>().damage_numbers);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_close_while_keep_fades_out_keeps_the_display() {
    let dir = user_folder("keep-close");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    from_title(&mut harness);
    press(&mut harness, "Window mode");
    press(&mut harness, "Apply");
    let keep = find(&harness, "Keep").unwrap();
    assert!(harness.focus(keep));
    harness.harness_mut().press_action("ui_accept");
    harness.step(1);
    harness.harness_mut().release_action("ui_accept");
    assert_eq!(top(&harness), Some(CONFIRM), "still fading out");
    harness.harness_mut().request_close();
    assert_eq!(harness.step(1), 1, "the last frame runs");
    let restarted = harness_with(UserFolder::At(dir.clone()));
    let mode = restarted.world().resource::<Settings>().display_mode();
    assert_eq!(mode, Some(DisplayMode::BorderlessFullscreen), "kept");
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
    assert_eq!(world.resource::<Settings>().ui_scale(), Some(2.0));
    settings::set_scale(world, true, f32::INFINITY);
    assert_eq!(world.resource::<Settings>().text_scale(), None, "ignored");
    settings::set_scale(world, true, 0.1);
    assert_eq!(world.resource::<Settings>().text_scale(), Some(0.5));
    let options = world.resource_mut::<GameOptions>();
    options.set_screen_shake(2.0);
    assert_eq!(options.screen_shake(), 1.0);
    options.set_screen_shake(-1.0);
    assert_eq!(options.screen_shake(), 0.0);
    options.set_screen_shake(f32::NAN);
    assert_eq!(options.screen_shake(), 0.0, "ignored");
}

/// A game's stack: a bloom, then a vignette.
fn game_stack() -> Vec<PostPass> {
    let bloom = BloomParams {
        threshold: 0.8,
        ..BloomParams::default()
    };
    let vignette = VignetteParams {
        strength: 0.3,
        ..VignetteParams::default()
    };
    vec![PostPass::Bloom(bloom), PostPass::Vignette(vignette)]
}

fn post_stack(harness: &UiHarness) -> Vec<PostPass> {
    harness.world().resource::<PostStack>().0.clone()
}

/// Sets the Bloom option and runs a frame.
fn set_bloom(harness: &mut UiHarness, on: bool) {
    harness.world_mut().resource_mut::<GameOptions>().bloom = on;
    harness.step(1);
}

#[test]
fn the_bloom_option_strips_and_restores_the_games_bloom() {
    let dir = user_folder("bloom-option");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let game = game_stack();
    harness.world_mut().insert_resource(PostStack(game.clone()));
    harness.step(1);
    assert_eq!(post_stack(&harness), game, "on: the game's stack");
    set_bloom(&mut harness, false);
    assert_eq!(post_stack(&harness), game[1..], "off: the vignette alone");
    set_bloom(&mut harness, true);
    assert_eq!(post_stack(&harness), game, "on: exactly the original");
    set_bloom(&mut harness, false);
    harness.world_mut().insert_resource(PostStack(game.clone()));
    harness.step(1);
    assert_eq!(post_stack(&harness), game[1..], "inserted while off");
    from_title(&mut harness);
    assert_eq!(
        toggled(&harness, "Bloom"),
        Some(false),
        "on the Display tab"
    );
    press(&mut harness, "Reset tab");
    assert!(options(&harness).bloom, "Display's Reset turns it on");
    assert_eq!(post_stack(&harness), game);
    press(&mut harness, "Bloom");
    assert!(!options(&harness).bloom);
    press(&mut harness, "Back");
    let restarted = harness_with(UserFolder::At(dir.clone()));
    assert!(!options(&restarted).bloom, "kept across a restart");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bloom_off_then_a_run_change_restores_nothing_the_game_dropped() {
    let mut harness = harness();
    set_bloom(&mut harness, false);
    start(&mut harness);
    assert_eq!(post_stack(&harness), [], "the sample's Bloom taken out");
    tap(&mut harness, "state_pause");
    press(&mut harness, "Quit to Title");
    press(&mut harness, "Quit");
    set_bloom(&mut harness, true);
    assert_eq!(post_stack(&harness), [], "the title's stays empty");
}

#[test]
fn a_stack_that_outlives_its_run_keeps_its_bloom() {
    let mut harness = harness();
    let game = game_stack();
    harness.world_mut().insert_resource(PostStack(game.clone()));
    set_bloom(&mut harness, false);
    assert_eq!(post_stack(&harness), game[1..]);
    // A change of run that leaves the stack alone, through the curtain.
    tungsten_template_basic::shell::goto(harness.world_mut(), TitleState::default());
    settle(&mut harness);
    assert_eq!(post_stack(&harness), game[1..], "taken out again");
    set_bloom(&mut harness, true);
    assert_eq!(post_stack(&harness), game, "back exactly");
}

#[test]
fn a_stack_set_to_its_stripped_value_keeps_its_withheld_bloom() {
    let mut harness = harness();
    let game = game_stack();
    harness.world_mut().insert_resource(PostStack(game.clone()));
    set_bloom(&mut harness, false);
    // The game sets what the shell left; passes carry no identity, so the
    // shell cannot tell, and its withheld Bloom comes back when on (Q23).
    harness
        .world_mut()
        .insert_resource(PostStack(game[1..].to_vec()));
    harness.step(1);
    set_bloom(&mut harness, true);
    assert_eq!(post_stack(&harness), game);
}

#[test]
fn each_tabs_reset_keeps_the_other_tabs_options() {
    let mut harness = harness();
    let both_off = |harness: &mut UiHarness| {
        let options = harness.world_mut().resource_mut::<GameOptions>();
        (options.bloom, options.autosave) = (false, false);
    };
    let both = |harness: &UiHarness| (options(harness).bloom, options(harness).autosave);
    from_title(&mut harness);
    for (tab, after) in [
        ("Gameplay", (false, true)),
        ("Display", (true, false)),
        ("Audio", (false, false)),
    ] {
        both_off(&mut harness);
        press(&mut harness, tab);
        press(&mut harness, "Reset tab");
        assert_eq!(both(&harness), after, "{tab}'s Reset");
    }
}
