use super::*;
use crate::App;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use tungsten_core::config::PresentModeConfig;
use tungsten_core::{AudioCommand, DisplayMode, Resolution, UserFolder};

fn tempdir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tungsten_settings_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An app whose user folder is `dir`.
fn app_at(dir: &Path, mut config: Config) -> App {
    config.game.user_folder = UserFolder::At(dir.to_path_buf());
    App::new(config).unwrap()
}

fn pending(world: &World) -> Option<DisplayState> {
    world.resource::<PendingDisplay>().0
}

fn volumes(world: &mut World) -> Vec<f32> {
    world
        .resource_mut::<AudioCommands>()
        .drain()
        .into_iter()
        .filter_map(|command| match command {
            AudioCommand::SetMasterVolume(volume) => Some(volume),
            _ => None,
        })
        .collect()
}

/// What a frame with a window would do with the request.
fn apply_request(world: &mut World) {
    if let Some(requested) = world.resource_mut::<PendingDisplay>().0.take() {
        *world.resource_mut::<DisplayState>() = requested;
    }
}

const HD: Resolution = Resolution {
    width: 1920,
    height: 1080,
};

#[test]
fn apply_settings_requests_the_display_and_sets_scale_and_volume() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    let world = app.world_mut();
    volumes(world);
    let settings = world.resource_mut::<Settings>();
    settings.set_display_mode(Some(DisplayMode::BorderlessFullscreen));
    settings.set_resolution(Some(HD));
    settings.set_ui_scale(Some(1.5));
    settings.set_master_volume(Some(0.25));
    apply_settings(world);

    let request = pending(world).expect("a display request");
    assert_eq!(request.display_mode, DisplayMode::BorderlessFullscreen);
    assert_eq!(request.resolution, HD);
    assert_eq!(world.resource::<ScaleFactor>().ui_scale(), 1.5);
    assert_eq!(world.resource::<ScaleFactor>().get(), 1.5);
    assert_eq!(volumes(world), [0.25]);
}

#[test]
fn apply_settings_requests_nothing_when_the_display_matches() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    let world = app.world_mut();
    let settings = world.resource_mut::<Settings>();
    settings.set_display_mode(Some(DisplayMode::Windowed));
    settings.set_resolution(Some(Resolution::default()));
    settings.set_ui_scale(Some(2.0));
    apply_settings(world);
    assert_eq!(pending(world), None);
    assert_eq!(world.resource::<ScaleFactor>().get(), 2.0);
}

#[test]
fn unset_values_return_to_the_shipped_ones() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    let world = app.world_mut();
    let settings = world.resource_mut::<Settings>();
    settings.set_resolution(Some(HD));
    settings.set_master_volume(Some(0.0));
    apply_settings(world);
    assert_eq!(volumes(world), [0.0]);
    apply_request(world);
    assert_eq!(world.resource::<DisplayState>().resolution, HD);

    let settings = world.resource_mut::<Settings>();
    settings.set_resolution(None);
    settings.set_master_volume(None);
    apply_settings(world);
    assert_eq!(
        pending(world).expect("back to the shipped size").resolution,
        Resolution {
            width: 1280,
            height: 720
        }
    );
    assert_eq!(volumes(world), [1.0]);
    assert_eq!(world.resource::<ScaleFactor>().ui_scale(), 1.0);
}

#[test]
fn a_runtime_apply_matches_a_restart() {
    let dir = tempdir();
    let mut shipped = Config::default();
    shipped.render.present_mode = Some(PresentModeConfig::Fifo);
    shipped.window.vsync = false;

    let mut app = app_at(&dir, shipped.clone());
    let world = app.world_mut();
    assert_eq!(
        world.resource::<DisplayState>().present_mode,
        Some(PresentModeConfig::Fifo)
    );
    let settings = world.resource_mut::<Settings>();
    settings.set_vsync(Some(false));
    settings.save().unwrap();
    apply_settings(world);
    let request = pending(world).expect("the present mode changes");
    assert!(!request.vsync);
    assert_eq!(request.present_mode, None, "a set vsync clears it");

    let mut restarted = app_at(&dir, shipped);
    assert_eq!(*restarted.world_mut().resource::<DisplayState>(), request);
}

#[test]
fn the_last_apply_before_a_frame_wins() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    let world = app.world_mut();
    let current = *world.resource::<DisplayState>();

    world
        .resource_mut::<Settings>()
        .set_display_mode(Some(DisplayMode::BorderlessFullscreen));
    apply_settings(world);
    assert!(pending(world).is_some());
    world.resource_mut::<Settings>().set_display_mode(None);
    apply_settings(world);
    assert_eq!(
        pending(world),
        None,
        "the later apply clears the earlier one"
    );

    // F11 queues its request in `PreUpdate`, before a settings screen's
    // `Update` system applies.
    let mut fullscreen = current;
    fullscreen.display_mode = DisplayMode::BorderlessFullscreen;
    request_display_settings(world, fullscreen).unwrap();
    apply_settings(world);
    assert_eq!(pending(world), None, "the hotkey's change is replaced");
}
