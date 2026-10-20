use super::*;
use crate::App;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use tungsten_core::config::PresentModeConfig;
use tungsten_core::{AudioBus, AudioCommand, DisplayMode, Resolution, UserFolder};

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

#[test]
fn apply_settings_sets_the_trees_text_scale_and_unset_returns_it_to_one() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    let world = app.world_mut();
    world.resource_mut::<Settings>().set_text_scale(Some(1.5));
    apply_settings(world);
    assert_eq!(world.resource::<tungsten_core::UiTree>().text_scale(), 1.5);
    world.resource_mut::<Settings>().set_text_scale(None);
    apply_settings(world);
    assert_eq!(world.resource::<tungsten_core::UiTree>().text_scale(), 1.0);
}

#[test]
fn a_window_resized_by_hand_keeps_its_size_at_the_next_apply() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    let world = app.world_mut();
    // The `Resized` event of a drag's end.
    crate::display::sync_window_resolution(world, 1600, 900, None);
    world
        .resource_mut::<Settings>()
        .set_master_volume(Some(0.5));
    apply_settings(world);
    assert_eq!(
        pending(world).map(|request| request.resolution),
        None,
        "an unrelated apply leaves the hand size (D-169)"
    );
    assert_eq!(
        world.resource::<DisplayState>().resolution,
        Resolution {
            width: 1600,
            height: 900
        }
    );
}

#[test]
fn a_resolution_set_while_fullscreen_applies_when_windowed() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    let world = app.world_mut();
    let settings = world.resource_mut::<Settings>();
    settings.set_display_mode(Some(DisplayMode::BorderlessFullscreen));
    settings.set_resolution(Some(HD));
    apply_settings(world);
    apply_request(world);
    world
        .resource_mut::<Settings>()
        .set_display_mode(Some(DisplayMode::Windowed));
    apply_settings(world);
    let request = pending(world).expect("back to windowed");
    assert_eq!(request.display_mode, DisplayMode::Windowed);
    assert_eq!(request.resolution, HD, "the size set while fullscreen");
}

#[test]
fn two_applies_before_a_frame_keep_the_queued_resolution() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    let world = app.world_mut();
    world.resource_mut::<Settings>().set_resolution(Some(HD));
    apply_settings(world);
    world
        .resource_mut::<Settings>()
        .set_master_volume(Some(0.5));
    apply_settings(world);
    assert_eq!(
        pending(world)
            .expect("the first apply's request")
            .resolution,
        HD
    );
}

/// The bus volumes the queued commands set, draining them.
fn bus_volumes(world: &mut World) -> Vec<(AudioBus, f32)> {
    world
        .resource_mut::<AudioCommands>()
        .drain()
        .into_iter()
        .filter_map(|command| match command {
            AudioCommand::SetBusVolume { bus, volume, .. } => Some((bus, volume)),
            _ => None,
        })
        .collect()
}

#[test]
fn apply_settings_sends_every_bus_volume_a_set_one_or_1() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    let world = app.world_mut();
    world.resource_mut::<AudioCommands>().drain();
    world
        .resource_mut::<Settings>()
        .set_bus_volume(AudioBus::Music, Some(0.25));
    apply_settings(world);
    assert_eq!(
        bus_volumes(world),
        [(AudioBus::Music, 0.25), (AudioBus::Effects, 1.0)]
    );
    let settings = world.resource_mut::<Settings>();
    settings.set_bus_volume(AudioBus::Music, None);
    settings.set_bus_volume(AudioBus::Effects, Some(0.0));
    apply_settings(world);
    assert_eq!(
        bus_volumes(world),
        [(AudioBus::Music, 1.0), (AudioBus::Effects, 0.0)]
    );
}

#[test]
fn startup_queues_only_the_set_bus_volumes() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    assert!(
        bus_volumes(app.world_mut()).is_empty(),
        "nothing set, nothing queued"
    );
    let settings = app.world_mut().resource_mut::<Settings>();
    settings.set_bus_volume(AudioBus::Effects, Some(0.4));
    settings.save().unwrap();
    let mut restarted = app_at(&dir, Config::default());
    assert_eq!(
        bus_volumes(restarted.world_mut()),
        [(AudioBus::Effects, 0.4)]
    );
}

fn fps(n: u32) -> FrameRateCap {
    FrameRateCap::Fps(std::num::NonZeroU32::new(n).unwrap())
}

#[test]
fn player_frame_cap_reaches_the_display_request() {
    let dir = tempdir();
    let mut app = app_at(&dir, Config::default());
    let world = app.world_mut();
    assert_eq!(world.resource::<DisplayState>().frame_rate_cap, None);
    world
        .resource_mut::<Settings>()
        .set_frame_rate_cap(Some(fps(30)));
    apply_settings(world);
    assert_eq!(
        pending(world).expect("a display request").frame_rate_cap,
        Some(30)
    );
}

#[test]
fn player_frame_cap_unset_returns_to_the_shipped_one() {
    let dir = tempdir();
    let mut shipped = Config::default();
    shipped.display.frame_rate_cap = Some(60);
    let mut app = app_at(&dir, shipped.clone());
    let world = app.world_mut();
    assert_eq!(world.resource::<DisplayState>().frame_rate_cap, Some(60));

    world
        .resource_mut::<Settings>()
        .set_frame_rate_cap(Some(FrameRateCap::Uncapped));
    world.resource::<Settings>().save().unwrap();
    apply_settings(world);
    let request = pending(world).expect("uncapped over the shipped 60");
    assert_eq!(request.frame_rate_cap, None);
    apply_request(world);

    let mut restarted = app_at(&dir, shipped);
    assert_eq!(
        restarted
            .world_mut()
            .resource::<DisplayState>()
            .frame_rate_cap,
        None,
        "a restart agrees"
    );

    world.resource_mut::<Settings>().set_frame_rate_cap(None);
    apply_settings(world);
    assert_eq!(
        pending(world)
            .expect("back to the shipped cap")
            .frame_rate_cap,
        Some(60)
    );
}

#[test]
fn player_frame_cap_without_a_shipped_config_sets_only_a_set_cap() {
    let mut world = World::new();
    world.insert_resource(DisplayState {
        frame_rate_cap: Some(144),
        ..DisplayState::default()
    });
    world.insert_resource(PendingDisplay::default());
    world.insert_resource(Settings::default());
    apply_settings(&mut world);
    assert_eq!(pending(&world), None, "unset keeps the current cap");

    world
        .resource_mut::<Settings>()
        .set_frame_rate_cap(Some(fps(30)));
    apply_settings(&mut world);
    assert_eq!(pending(&world).unwrap().frame_rate_cap, Some(30));
}

/// A config shipping the master and music at 0.8 and effects at 0.6
/// (`D-184`).
fn shipped_audio() -> Config {
    let mut config = Config::default();
    config.audio.master_volume = 0.8;
    config.audio.bus_volumes.insert(AudioBus::Music, 0.8);
    config.audio.bus_volumes.insert(AudioBus::Effects, 0.6);
    config
}

/// The master and bus volumes the queued commands set, draining them.
fn levels(world: &mut World) -> (Vec<f32>, Vec<(AudioBus, f32)>) {
    let mut master = Vec::new();
    let mut buses = Vec::new();
    for command in world.resource_mut::<AudioCommands>().drain() {
        match command {
            AudioCommand::SetMasterVolume(volume) => master.push(volume),
            AudioCommand::SetBusVolume { bus, volume, .. } => buses.push((bus, volume)),
            _ => {}
        }
    }
    (master, buses)
}

#[test]
fn an_unset_volume_returns_to_the_shipped_one() {
    let dir = tempdir();
    let mut app = app_at(&dir, shipped_audio());
    let world = app.world_mut();
    levels(world);
    let settings = world.resource_mut::<Settings>();
    settings.set_master_volume(Some(0.25));
    settings.set_bus_volume(AudioBus::Music, Some(0.5));
    apply_settings(world);
    assert_eq!(
        levels(world),
        (
            vec![0.25],
            vec![(AudioBus::Music, 0.5), (AudioBus::Effects, 0.6)]
        ),
        "a set volume wins; an unset bus plays at its shipped volume"
    );
    let settings = world.resource_mut::<Settings>();
    settings.set_master_volume(None);
    settings.set_bus_volume(AudioBus::Music, None);
    apply_settings(world);
    assert_eq!(
        levels(world),
        (
            vec![0.8],
            vec![(AudioBus::Music, 0.8), (AudioBus::Effects, 0.6)]
        ),
        "unset, each returns to the shipped volume, not to 1"
    );
}

#[test]
fn shipped_volume_reads_the_config_and_defaults_to_one() {
    let bare = World::new();
    assert_eq!(shipped_volume(&bare, None), 1.0, "no shipped config");
    assert_eq!(shipped_volume(&bare, Some(AudioBus::Music)), 1.0);
    let dir = tempdir();
    let app = app_at(&dir, shipped_audio());
    assert_eq!(shipped_volume(app.world(), None), 0.8);
    assert_eq!(shipped_volume(app.world(), Some(AudioBus::Music)), 0.8);
    assert_eq!(shipped_volume(app.world(), Some(AudioBus::Effects)), 0.6);
    let plain = app_at(&tempdir(), Config::default());
    for bus in [None, Some(AudioBus::Music), Some(AudioBus::Effects)] {
        assert_eq!(shipped_volume(plain.world(), bus), 1.0, "{bus:?}");
    }
    // The settings never change what ships.
    let mut app = app;
    app.world_mut()
        .resource_mut::<Settings>()
        .set_master_volume(Some(0.1));
    apply_settings(app.world_mut());
    assert_eq!(shipped_volume(app.world(), None), 0.8);
}
