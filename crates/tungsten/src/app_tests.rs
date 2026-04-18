use super::{resolve_startup_display, runtime_display_mode, App};
use tungsten_core::{CollisionEvent, Config, DisplayMode, DisplayState, EventQueue};

#[derive(Debug, Clone, Copy)]
struct ExampleEvent;

#[test]
fn register_event_is_idempotent_per_type() {
    let mut app = App::new(Config::default());
    let initial_flushers = app.event_flushers.len();

    app.register_event::<ExampleEvent>();
    let after_first = app.event_flushers.len();
    app.register_event::<ExampleEvent>();

    assert_eq!(after_first, initial_flushers + 1);
    assert_eq!(app.event_flushers.len(), after_first);
    assert!(app
        .world
        .get_resource::<EventQueue<ExampleEvent>>()
        .is_some());
}

#[test]
fn collision_event_is_pre_registered() {
    let mut app = App::new(Config::default());
    let initial_flushers = app.event_flushers.len();

    app.register_event::<CollisionEvent>();

    assert_eq!(app.event_flushers.len(), initial_flushers);
}

#[test]
fn default_sprite_extract_installed_when_not_set() {
    let mut app = App::new(Config::default());
    assert!(app.extract_sprites.is_none());
    app.install_default_extracts();
    assert!(app.extract_sprites.is_some());
}

#[test]
fn user_extract_sprites_overrides_default() {
    use tungsten_core::assets::{FilterMode, TextureHandle};
    use tungsten_render::{SpriteBatch, SpriteInstance};

    let mut app = App::new(Config::default());
    // Sentinel closure: returns a batch with one specific SpriteInstance.
    app.set_extract_sprites(|_| {
        vec![SpriteBatch {
            texture: TextureHandle(42),
            filter: FilterMode::Linear,
            instances: vec![SpriteInstance {
                position: [1.5, 2.5],
                size: [3.0, 4.0],
                rotation: 0.25,
                color: [1, 2, 3, 4],
            }],
        }]
    });

    app.install_default_extracts();

    let batches = app.extract_sprites.as_ref().expect("extract_sprites set")(&app.world);
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].texture, TextureHandle(42));
    assert_eq!(batches[0].instances[0].color, [1, 2, 3, 4]);
}

#[test]
fn startup_display_downgrades_invalid_resolution_to_engine_defaults() {
    let mut config = Config::default();
    config.window.width = 0;

    let resolved = resolve_startup_display(&config);
    assert_eq!(resolved, DisplayState::default());
}

#[test]
fn runtime_display_mode_downgrades_exclusive_fullscreen() {
    assert_eq!(
        runtime_display_mode(DisplayMode::ExclusiveFullscreen),
        DisplayMode::BorderlessFullscreen
    );
    assert_eq!(
        runtime_display_mode(DisplayMode::Windowed),
        DisplayMode::Windowed
    );
}
