use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::asset_loader;
use crate::audio::AudioSystem;
use crate::capture::{Capture, parse_capture_config};
use crate::crash::{self, CrashContext};
use crate::debug_hud::{DebugHud, HudActiveState};
use crate::debug_views::{DEBUG_WINDOWS_FILE, LogConsole, UiInspector};
use crate::display::{
    DisplayDelta, PendingDisplay, frame_budget_for, read_monitor, runtime_display_mode,
    set_monitor, sync_display_state_and_telemetry, sync_window_resolution, take_pending_display,
};
use crate::extract::Extracts;
use crate::hot_reload::HotReloadWatcher;
use crate::input_bridge;
use crate::inspector::{InspectorState, UiRouteInstalled, default_inspect_registry};
use crate::logging;
use crate::physics_debug::{PhysicsDebugOverlay, physics_debug_emit_system};
use crate::plugins::DefaultPlugins;
use crate::plugins::UI_ROUTE;
use crate::post_aa::{PendingPostAa, PostAaState, sync_post_aa_state, take_pending_post_aa};
use crate::settings::{ShippedConfig, WindowedTarget};
use crate::sprite_extract::ExtractScratch;
use crate::state::StateStack;
use crate::systems_overlay::{SystemStages, SystemTimingOverlay};
use crate::telemetry::{DisplayTelemetry, FrameTimings, RenderCounts};
use crate::user_dir::{self, Platform};
use log::LevelFilter;
use serde::de::DeserializeOwned;
use tungsten_core::assets::{
    FontRegistry, ParticleConfigRegistry, ParticleMeshRegistry, ShaderRegistry, SoundRegistry,
    TilemapRegistry,
};
use tungsten_core::physics::{PhysicsBuffers, PhysicsConfig};
use tungsten_core::post::{PostPass, PostStack};
use tungsten_core::ui::{
    CursorIcon, PaintList, UiDebug, UiRouter, UiThemeRegistry, UiTree, WindowPlacements,
};
// `App` writes the deprecated `DeltaTime` each frame until W4b removes it.
#[allow(deprecated)]
use tungsten_core::DeltaTime;
use tungsten_core::{
    ActionMap, AssetRegistry, AudioCommand, AudioCommands, Binding, CameraController, CameraState,
    CommandBuffer, ComponentRegistry, Config, ConfigError, DebugDraw, DebugShape, DisplayMode,
    DisplayState, InputState, InspectRegistry, Inspectable, ParticleActive, ParticleBudget, Plugin,
    PluginSet, RawInput, RoutedInput, SaveSlots, Schedule, Settings, Stage, SystemDesc, Time,
    TimeConfig, World, WorldLabels, WorldRngSeed, system,
};
use tungsten_render::renderer::RenderError;
use tungsten_render::{
    DebugLineInstance, GpuFrameTimings, MeshParticleBatch, QuadInstance, RenderOutcome, Renderer,
    SpriteBatch, TextSection, WorldLabelRun,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowId};

/// Fixed per-frame dt (seconds) used under `TUNGSTEN_SMOKE_FRAMES`. Pinned to
/// 60 Hz so smoke-mode captures and visual regressions are frame-deterministic
/// across runs regardless of build profile or host load.
const SMOKE_MODE_FIXED_DT_SECS: f32 = 1.0 / 60.0;
/// Names the report file `tungsten check` reads. Set, `App::run` checks the
/// game's files in place of opening a window (M70).
const CHECK_ENV: &str = "TUNGSTEN_CHECK";

/// Longest per-frame dt (seconds) a frame hands to the simulation (`D-088`).
/// A stall (debugger pause, window drag, a blocked acquire) would otherwise
/// reach `physics_step` whole: one 0.2 s step throws a settled awake pile
/// about, and a 2 s step drops bodies through the floor. One 0.1 s step
/// leaves such a pile at about 40 px/s.
const MAX_DT_SECS: f32 = 0.1;

pub use tungsten_core::schedule::SystemFn;

/// World-to-quad extract, as [`Extracts`]' quad channel holds it.
pub type ExtractQuadsFn = Box<dyn Fn(&World) -> Vec<QuadInstance>>;

/// World-to-sprite extract, as [`Extracts`]' sprite channel holds it.
pub type ExtractSpritesFn = Box<dyn Fn(&World) -> Vec<SpriteBatch>>;

/// World-to-text extract, as [`Extracts`]' text channel holds it.
pub type ExtractTextFn = Box<dyn Fn(&World) -> Vec<TextSection>>;

/// The window's size in physical pixels, a core type since M38 (`D-128`);
/// `tungsten::WindowSize` and this path still name it.
pub use tungsten_core::WindowSize;

/// Post-renderer startup hook.
pub type StartupFn = Box<dyn FnOnce(&mut World, &mut Renderer)>;

/// Winit loop, ECS, and renderer owner.
pub struct App {
    config: Config,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    world: World,
    /// Every system, by stage; `run_frame` drives it (`D-018`'s frame order).
    schedule: Schedule,
    /// Whether the `Startup` stage has run.
    startup_done: bool,
    startup: Option<StartupFn>,
    last_frame: Option<Instant>,
    exit_on_escape: bool,
    audio: Option<AudioSystem>,
    hot_reload: Option<HotReloadWatcher>,
    /// Directories given to `enable_hot_reload`; `run` starts the watcher.
    hot_reload_dirs: Option<Vec<PathBuf>>,
    manifest_path: Option<PathBuf>,
    // D-052: ordered manifest roots merged before user startup.
    manifest_roots: Vec<PathBuf>,
    input_map_path: PathBuf,
    smoke_frames_remaining: Option<u32>,
    system_name_counter: usize,
    // GPU timing path adds `device.poll` stall.
    gpu_timing_enabled: bool,
    frame_budget: Option<Duration>,
    // Capped frames: when `about_to_wait` requests the next redraw.
    redraw_deadline: Option<Instant>,
    // Start of the previous redraw; the `interval` telemetry measures from it.
    prev_frame_start: Option<Instant>,
    /// The one-frame PNG capture `TUNGSTEN_CAPTURE_FRAME` asks for (`D-142`).
    capture: Option<Capture>,
    /// With no audio device, the audio stage appends a frame's commands here
    /// when set, instead of dropping them: the commands that frame would have
    /// played. Only a headless caller sets it.
    pub(crate) audio_capture: Option<Vec<AudioCommand>>,
    /// Frames that reached render, presented or skipped; the capture's frame
    /// numbers count them (`D-142`).
    frames_rendered: u64,
    /// The setup or render failure that ended the run; [`App::run`] returns it.
    fatal_error: Option<anyhow::Error>,
    // M31: the user's post stack plus the transition pass, rebuilt on each
    // frame a screen transition draws; kept to reuse its allocation.
    transition_post_stack: PostStack,
    /// The UI paint list (M46), retained across frames and repainted when
    /// the tree is paint-dirty or `painted_at` no longer matches.
    ui_paint: PaintList,
    /// What the retained list was painted with; `None` before the first.
    painted_at: Option<PaintedAt>,
    /// An exclusive fullscreen request's downgrade to borderless, logged
    /// once a run (M59), not at every apply.
    exclusive_warning: std::sync::Once,
    /// The system cursor last set on the window (`D-149`).
    cursor: CursorIcon,
    /// The frame's raw events when `PreUpdate`, and so the UI's routing,
    /// ended; the exit check reads later ones raw.
    routed_events: usize,
}

impl App {
    /// Builds the app with [`DefaultPlugins`]. First it installs the engine
    /// logger, unless the game set one, with a log file in the game's user
    /// folder (`D-119`), then logs [`Config::take_load_warnings`]. An error
    /// it returns is logged first, so it reaches the log file too.
    ///
    /// The player's [`Settings`] and bindings load from the user folder's
    /// `settings` folder (`D-159`): the settings between `tungsten.json` and
    /// the environment, the bindings over the shipped `input.json`. A file
    /// there that is corrupt falls back to the defaults, kept beside them as
    /// `.bad`, and never stops the game. [`SaveSlots`] keep the game's saves
    /// in its `saves` folder (`D-160`). Without a folder nothing persists.
    ///
    /// # Errors
    ///
    /// An invalid `game.id`, `time` section, `audio` volume (`D-184`) or
    /// `logging.level`, or a shipped `input.json` that cannot be read or
    /// parsed.
    pub fn new(config: Config) -> anyhow::Result<Self> {
        Self::with_plugins(config, DefaultPlugins::set())
    }

    /// Builds the app with `plugins` in place of [`DefaultPlugins`]: the
    /// default set without the plugin a game replaces, or an empty set for
    /// a program that wires every engine system itself, as the benchmarks
    /// do. The engine's resources are inserted whichever plugins run.
    ///
    /// # Errors
    ///
    /// As [`App::new`].
    pub fn with_plugins(mut config: Config, plugins: PluginSet) -> anyhow::Result<Self> {
        let smoke_frames = smoke_frames_from_env();
        let checking = check_report_from_env().is_some();
        let invalid = config
            .game
            .validate()
            .err()
            .or_else(|| config.time.validate().err())
            .or_else(|| config.audio.validate().err())
            .or_else(|| invalid_logging_level(&config.logging.level));
        let user_dirs = if invalid.is_none() {
            user_dir::resolve(
                &config.game,
                smoke_frames.is_some() || checking,
                Platform::current(),
                |name| std::env::var_os(name),
            )
        } else {
            None
        };
        let level = config.logging.level.parse().unwrap_or(LevelFilter::Info);
        // A check run's output is its errors: the loaders' info lines stay out.
        let level = if checking {
            level.min(LevelFilter::Warn)
        } else {
            level
        };
        let (log_file, console_feed) =
            logging::install(level, user_dirs.as_ref().map(|dirs| dirs.logs.as_path()));
        if let Some(err) = invalid {
            log::error!("{err}");
            return Err(err.into());
        }
        for warning in config.take_load_warnings() {
            log::warn!(target: "tungsten_core::config", "{warning}");
        }
        if user_dirs.is_none()
            && smoke_frames.is_none()
            && !checking
            && let Some(id) = &config.game.id
        {
            log::warn!("Game '{id}' has no user folder, so no log or crash file is written");
        }
        if let Some(dirs) = &user_dirs {
            crash::install(CrashContext::new(&config.game, dirs.logs.clone(), log_file));
        }

        let mut settings = match &user_dirs {
            Some(dirs) => Settings::load(dirs.settings.join(Settings::FILE)),
            None => Settings::default(),
        };
        for warning in settings.take_load_warnings() {
            log::warn!(target: "tungsten_core::settings", "{warning}");
        }
        // M57: the debug views' window places, beside the settings.
        let mut placements = match &user_dirs {
            Some(dirs) => WindowPlacements::load(dirs.settings.join(DEBUG_WINDOWS_FILE)),
            None => WindowPlacements::default(),
        };
        for warning in placements.take_load_warnings() {
            log::warn!(target: "tungsten::debug_views", "{warning}");
        }
        // The baseline `apply_settings` starts from at runtime too, so a
        // runtime apply and a restart agree.
        let shipped = ShippedConfig(config.clone());
        if let Err(err) = config.apply_settings(&settings) {
            log::error!("{err}");
            return Err(err.into());
        }

        let resolved_display = resolve_startup_display(&config);
        let input_map_path = PathBuf::from("input.json");
        let user_bindings = user_dirs
            .as_ref()
            .map(|dirs| dirs.settings.join("input.json"));
        let action_map = load_action_map_at_startup(&input_map_path, user_bindings.as_deref())
            .inspect_err(|err| log::error!("{err:#}"))?;
        let mut scale_factor = tungsten_core::ScaleFactor::default();
        if let Some(ui_scale) = settings.ui_scale() {
            scale_factor.set_ui_scale(ui_scale);
        }
        scale_factor.set_window_size(
            resolved_display.resolution.width,
            resolved_display.resolution.height,
        );
        // Each volume the player set, else one the game ships below 1
        // (`D-184`); 1 is the mixer's own.
        let below_one = |volume: f32| (volume < 1.0).then_some(volume);
        let mut audio_commands = AudioCommands::new();
        if let Some(volume) = settings
            .master_volume()
            .or_else(|| below_one(config.audio.master_volume))
        {
            audio_commands.set_master_volume(volume);
        }
        for &bus in tungsten_core::AudioBus::ALL {
            if let Some(volume) = settings
                .bus_volume(bus)
                .or_else(|| below_one(config.audio.bus_volume(bus)))
            {
                audio_commands.set_bus_volume(bus, volume);
            }
        }
        let mut ui_tree = UiTree::new();
        if let Some(text_scale) = settings.text_scale() {
            ui_tree.set_text_scale(text_scale);
        }
        let mut world = World::new();
        world.insert_resource(time_from_config(&config.time));
        #[allow(deprecated)]
        world.insert_resource(DeltaTime::new());
        world.insert_resource(InputState::new());
        world.insert_resource(action_map);
        world.insert_resource(WindowSize {
            width: resolved_display.resolution.width,
            height: resolved_display.resolution.height,
        });
        // The window's factor replaces the OS scale at `resumed` (D-144);
        // the player's UI scale stays.
        world.insert_resource(scale_factor);
        world.insert_resource(settings);
        world.insert_resource(shipped);
        // D-169: the windowed size startup resolved, which an unrelated
        // apply does not send again.
        world.insert_resource(WindowedTarget(resolved_display.resolution));
        world.insert_resource(SaveSlots::new(
            user_dirs.as_ref().map(|dirs| dirs.saves.clone()),
        ));
        world.insert_resource(AssetRegistry::new());
        world.insert_resource(SoundRegistry::new());
        world.insert_resource(audio_commands);
        world.insert_resource(TilemapRegistry::new());
        world.insert_resource(tungsten_core::PrefabRegistry::new());
        world.insert_resource(tungsten_core::DataRegistry::new());
        world.insert_resource(ParticleConfigRegistry::new());
        world.insert_resource(ParticleMeshRegistry::new());
        world.insert_resource(ShaderRegistry::new());
        world.insert_resource(ParticleBudget::default());
        world.insert_resource(ParticleActive::default());
        world.insert_resource(WorldRngSeed::default());
        world.insert_resource(CameraState::new());
        world.insert_resource(CameraController::default());
        world.insert_resource(PhysicsConfig::default());
        world.insert_resource(FrameTimings::new());
        world.insert_resource(GpuFrameTimings::default());
        world.insert_resource(resolved_display);
        world.insert_resource(PendingDisplay::default());
        world.insert_resource(DisplayTelemetry::from_state(&resolved_display, None));
        world.insert_resource(CommandBuffer::new());
        world.insert_resource(DebugHud::new());
        world.insert_resource(StateStack::new());
        world.insert_resource(HudActiveState::default());
        world.insert_resource(RenderCounts::default());
        world.insert_resource(ExtractScratch::default());
        world.insert_resource(Extracts::default());
        world.insert_resource(DebugDraw::new());
        world.insert_resource(WorldLabels::default());
        // M46: the UI tree, its debug flags and the loaded theme assets; the
        // player's text scale on the tree from the start (M56).
        world.insert_resource(ui_tree);
        world.insert_resource(UiDebug::default());
        world.insert_resource(UiRouter::default());
        world.insert_resource(RoutedInput::default());
        world.insert_resource(UiThemeRegistry::new());
        world.insert_resource(PhysicsDebugOverlay::default());
        world.insert_resource(SystemTimingOverlay::default());
        world.insert_resource(InspectorState::new());
        // M57: the log console on the logger's feed, and the UI inspector.
        world.insert_resource(LogConsole::with_feed(console_feed));
        world.insert_resource(UiInspector::default());
        world.insert_resource(placements);
        world.insert_resource(default_inspect_registry());
        // M48: the components scene entries name, with those the default
        // extracts draw; plugins register theirs as they build.
        world.insert_resource(ComponentRegistry::new());
        asset_loader::register_extract_components(&mut world);
        // M26: empty post stack by default — byte-identical to M25 baseline.
        world.insert_resource(tungsten_core::post::PostStack::new());
        // M27: track post-AA state from startup config; pending request seam.
        world.insert_resource(PostAaState {
            mode: config.render.post_aa,
        });
        world.insert_resource(PendingPostAa::default());

        let mut app = Self {
            config,
            window: None,
            renderer: None,
            world,
            schedule: Schedule::new(),
            startup_done: false,
            startup: None,
            last_frame: None,
            exit_on_escape: true,
            audio: None,
            hot_reload: None,
            hot_reload_dirs: None,
            manifest_path: None,
            manifest_roots: Vec::new(),
            input_map_path,
            smoke_frames_remaining: smoke_frames,
            system_name_counter: 0,
            gpu_timing_enabled: std::env::var("TUNGSTEN_GPU_TIMING").is_ok(),
            frame_budget: frame_budget_for(resolved_display.frame_rate_cap),
            redraw_deadline: None,
            prev_frame_start: None,
            capture: parse_capture_config().map(Capture::new),
            audio_capture: None,
            frames_rendered: 0,
            fatal_error: None,
            transition_post_stack: PostStack::new(),
            ui_paint: PaintList::default(),
            painted_at: None,
            exclusive_warning: std::sync::Once::new(),
            cursor: CursorIcon::Default,
            routed_events: 0,
        };

        // Engine plugins register first, so their `PreUpdate` systems precede
        // a game's and a game's `PostUpdate` systems follow theirs.
        app.add_plugins(plugins);
        if crash::test_panic_requested(std::env::var_os(crash::TEST_PANIC_ENV).as_deref()) {
            app.schedule.add(
                Stage::PreUpdate,
                system("__test_panic", crash::test_panic_system),
            );
        }

        Ok(app)
    }

    /// Register inspectable component rows under `label`.
    pub fn register_inspectable<T: 'static + Inspectable>(&mut self, label: &'static str) {
        if let Some(registry) = self.world.get_resource_mut::<InspectRegistry>() {
            registry.register::<T>(label);
        }
    }

    /// [`World::register_component`] on the app's world: `components.<name>`
    /// in a scene entry spawns a `T`.
    ///
    /// # Panics
    ///
    /// When `name` is not `[a-z][a-z0-9_]*`, already names another type, or
    /// `T` already has another name ([`ComponentRegistry::register`]).
    #[track_caller]
    pub fn register_component<T: DeserializeOwned + 'static>(&mut self, name: &str) {
        self.world.register_component::<T>(name);
    }

    /// Builds `plugin` into the schedule and the world now.
    pub fn add_plugin(&mut self, plugin: impl Plugin) {
        self.add_plugins(PluginSet::new().with(plugin));
    }

    /// Builds every plugin of `plugins`, in order.
    pub fn add_plugins(&mut self, plugins: PluginSet) {
        plugins.build(&mut self.schedule, &mut self.world);
    }

    /// The schedule, for a snapshot of its resolved order.
    #[must_use]
    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    /// Adds `desc` to `stage`: `app.add_system_to(Stage::PostUpdate,
    /// system("hierarchy", propagate).before(tungsten::plugins::CAMERA_UPDATE))`.
    pub fn add_system_to(&mut self, stage: Stage, desc: SystemDesc) {
        self.schedule.add(stage, desc);
    }

    /// Resolves the schedule, as `run` and the harness do before the first
    /// frame; an unknown name, a duplicate or a cycle is the error.
    ///
    /// # Errors
    ///
    /// [`tungsten_core::ScheduleError`], as an `anyhow` error.
    pub fn resolve_schedule(&mut self) -> anyhow::Result<()> {
        self.schedule
            .resolve()
            .map_err(|err| anyhow::anyhow!("Schedule: {err}"))?;
        // M57: the inspector's pick reads the routed input when routing runs.
        let routed = self
            .schedule
            .names(Stage::PreUpdate)
            .any(|name| name == UI_ROUTE);
        self.world.insert_resource(UiRouteInstalled(routed));
        // M58: the systems overlay groups its rows by stage.
        let stages = Stage::ALL
            .into_iter()
            .flat_map(|stage| {
                self.schedule
                    .names(stage)
                    .map(move |name| (name.to_string(), stage))
            })
            .collect();
        self.world.insert_resource(SystemStages(stages));
        Ok(())
    }

    /// Enable or disable engine exit action handling: the `engine_exit`
    /// action, which ends its frame after `Update` with no last frame. A
    /// game's own quit calls [`request_exit`] instead (`D-180`).
    pub fn set_exit_on_escape(&mut self, exit: bool) {
        self.exit_on_escape = exit;
    }

    /// Mutable world access for setup.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// World access for the headless harness.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// Mutable renderer access after window creation.
    pub fn renderer_mut(&mut self) -> Option<&mut Renderer> {
        self.renderer.as_mut()
    }

    /// Register an unnamed `Update` system, named `system_N`.
    pub fn add_system(&mut self, run: impl FnMut(&mut World) + 'static) {
        let name = format!("system_{}", self.system_name_counter);
        self.system_name_counter += 1;
        self.schedule.add(Stage::Update, system(name, run));
    }

    /// Register a named `Update` system; the name is its profiling row.
    pub fn add_system_named(
        &mut self,
        name: impl Into<String>,
        run: impl FnMut(&mut World) + 'static,
    ) {
        self.schedule.add(Stage::Update, system(name, run));
    }

    /// Register `EventQueue<T>`; flushes once per frame after command flush.
    pub fn register_event<T: 'static>(&mut self) {
        self.world.register_event::<T>();
    }

    /// Replaces the quad channel's base, by default nothing, with `f`; the
    /// quads plugins add still follow. [`Extracts::replace_quads`] on the
    /// app's world.
    ///
    /// # Panics
    ///
    /// When the [`Extracts`] resource has been removed.
    pub fn set_extract_quads(&mut self, f: impl Fn(&World) -> Vec<QuadInstance> + 'static) {
        self.world.resource_mut::<Extracts>().replace_quads(f);
    }

    /// Replaces the sprite channel's base, by default the tilemaps then the
    /// `Sprite` entities, with `f`; the batches plugins add still follow.
    /// [`Extracts::replace_sprites`] on the app's world.
    ///
    /// # Panics
    ///
    /// When the [`Extracts`] resource has been removed.
    pub fn set_extract_sprites(&mut self, f: impl Fn(&World) -> Vec<SpriteBatch> + 'static) {
        self.world.resource_mut::<Extracts>().replace_sprites(f);
    }

    /// Replaces the text channel's base, by default nothing, with `f`; the
    /// sections plugins add still follow. [`Extracts::replace_text`] on the
    /// app's world.
    ///
    /// # Panics
    ///
    /// When the [`Extracts`] resource has been removed.
    pub fn set_extract_text(&mut self, f: impl Fn(&World) -> Vec<TextSection> + 'static) {
        self.world.resource_mut::<Extracts>().replace_text(f);
    }

    /// Set post-renderer startup hook.
    pub fn on_startup(&mut self, f: impl FnOnce(&mut World, &mut Renderer) + 'static) {
        self.startup = Some(Box::new(f));
    }

    /// D-052 manifest composition roots; D-017 duplicate IDs are fatal.
    pub fn set_manifest_roots(&mut self, roots: Vec<PathBuf>) {
        self.manifest_roots = roots;
    }

    /// Watch asset roots; reload at frame boundary.
    ///
    /// The watcher starts in [`App::run`], so this and
    /// [`App::set_manifest_roots`] may come in either order: an edit to any
    /// root manifest reloads the merged graph (`D-089`). `manifest_path` is
    /// the manifest reloaded by an app that declares no roots.
    pub fn enable_hot_reload(&mut self, assets_dirs: &[PathBuf], manifest_path: PathBuf) {
        self.hot_reload_dirs = Some(assets_dirs.to_vec());
        self.manifest_path = Some(manifest_path);
    }

    /// Run until window close or explicit exit.
    ///
    /// # Errors
    ///
    /// The event loop's, logged first. Otherwise the fatal error that ended
    /// the run, logged where it arose: a setup failure (window, renderer,
    /// manifests) or a failed frame, whose render error names the frame
    /// (`D-142`). Otherwise the capture error, logged once, when the frame
    /// `TUNGSTEN_CAPTURE_FRAME` names wrote no file: its readback, encode or
    /// write failed, it was skipped, or the run ended before it.
    ///
    /// With `TUNGSTEN_CHECK` set to a file path, as `tungsten check` and
    /// `tungsten package` set it, the run opens no window, renderer or audio
    /// device and runs no startup hook or frame: it loads every file the
    /// manifest roots name, and every unlisted JSON file under a root's
    /// folder as a scene, into this app's World, past every error, writes
    /// one `<file>\t<message>` line per error to that file and returns an
    /// error when there is one. [`App::new`] resolves no user folder then.
    /// The variable is the command line's, not a promised interface.
    pub fn run(mut self) -> anyhow::Result<()> {
        if let Some(report) = check_report_from_env() {
            let game_dir = std::env::current_dir()
                .inspect_err(|err| log::error!("No current folder to check: {err}"))?;
            return self.run_check(&report, &game_dir);
        }
        self.resolve_schedule()
            .inspect_err(|err| log::error!("{err}"))?;
        self.start_hot_reload();
        let event_loop = EventLoop::new()
            .inspect_err(|err| log::error!("Failed to create the event loop: {err}"))?;
        event_loop
            .run_app(&mut self)
            .inspect_err(|err| log::error!("Event loop failed: {err}"))?;
        match self.take_run_error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// `tungsten check`'s content stage in the game's own build (M70): the
    /// reload roots' files checked in `game_dir` with this app's World, and
    /// the report written to `report`, one line per error. Loaders log
    /// warnings and errors only.
    pub(crate) fn run_check(mut self, report: &Path, game_dir: &Path) -> anyhow::Result<()> {
        use std::fmt::Write as _;

        log::set_max_level(LevelFilter::Warn);
        let roots = manifest_reload_roots(&self.manifest_roots, self.manifest_path.as_deref());
        let errors = asset_loader::check::check_files(&mut self.world, &roots, game_dir);
        let mut lines = String::new();
        for error in &errors {
            let _ = writeln!(lines, "{}\t{}", error.file.display(), error.message);
        }
        std::fs::write(report, lines).map_err(|err| {
            anyhow::anyhow!(
                "Failed to write the check report '{}': {err}",
                report.display()
            )
        })?;
        match errors.len() {
            0 => Ok(()),
            n => Err(anyhow::anyhow!(
                "the check found {n} error{}, written to '{}'",
                if n == 1 { "" } else { "s" },
                report.display()
            )),
        }
    }

    /// The error a finished run returns (`D-142`): the fatal error, else the
    /// capture's. A capture whose frame never came records its error here.
    fn take_run_error(&mut self) -> Option<anyhow::Error> {
        let capture_error = self
            .capture
            .as_mut()
            .and_then(|capture| capture.finish(self.frames_rendered));
        self.fatal_error.take().or(capture_error)
    }

    /// Start the watcher `enable_hot_reload` asked for: its directories, plus
    /// the action map and every reload root as explicit files.
    fn start_hot_reload(&mut self) {
        let Some(dirs) = self.hot_reload_dirs.take() else {
            return;
        };
        let mut extra_files = vec![self.input_map_path.clone()];
        extra_files.extend(manifest_reload_roots(
            &self.manifest_roots,
            self.manifest_path.as_deref(),
        ));
        self.hot_reload = HotReloadWatcher::new(&dirs, &extra_files);
    }

    /// Whether `engine_exit` was just pressed (`D-149`). When the schedule
    /// routes UI input, in the routed view, so an Escape the UI took does not
    /// exit, or as a key or button press that reached the raw input after
    /// `PreUpdate`, which the UI never saw; else in the raw input.
    fn engine_exit_requested(&self) -> bool {
        if !self.exit_on_escape {
            return false;
        }
        let Some(actions) = self.world.get_resource::<ActionMap>() else {
            return false;
        };
        let raw = self.world.get_resource::<InputState>();
        if !self
            .schedule
            .names(Stage::PreUpdate)
            .any(|name| name == UI_ROUTE)
        {
            return raw.is_some_and(|input| actions.just_pressed(input, "engine_exit"));
        }
        let routed = self
            .world
            .get_resource::<RoutedInput>()
            .is_some_and(|routed| actions.just_pressed(routed, "engine_exit"));
        let bindings = actions.bindings("engine_exit");
        let late = raw.is_some_and(|input| {
            input
                .raw_events()
                .get(self.routed_events..)
                .unwrap_or_default()
                .iter()
                .any(|event| presses(bindings, event.input))
        });
        routed || late
    }

    /// Poll hot reload; apply before extract/render.
    fn process_hot_reload(&mut self) {
        let ready = match self.hot_reload.as_mut() {
            Some(w) => w.drain_ready(),
            None => return,
        };
        // A stopped watcher sends nothing more; the drain above flushed its
        // last paths, which still apply.
        if self
            .hot_reload
            .as_ref()
            .is_some_and(HotReloadWatcher::disconnected)
        {
            self.hot_reload = None;
        }
        if ready.is_empty() {
            return;
        }

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        let reload_roots =
            manifest_reload_roots(&self.manifest_roots, self.manifest_path.as_deref());
        let mut manifest_reloaded = false;
        // M61: the batch's JSON files reload after the loop, its prefab and
        // data files in one commit.
        let mut json_paths = Vec::new();

        for path in &ready {
            let canon = path.canonicalize().unwrap_or_else(|_| path.clone());

            if is_action_map(&canon, &self.input_map_path) {
                if let Err(e) = asset_loader::reload_action_map(&canon, &mut self.world) {
                    log::error!("Action map reload: {e}");
                }
                continue;
            }

            if is_reload_root(&canon, &reload_roots) {
                // One reload rebuilds the merged graph, however many roots
                // changed in this batch.
                if !manifest_reloaded {
                    manifest_reloaded = true;
                    if let Err(e) =
                        asset_loader::reload_manifest(&reload_roots, &mut self.world, renderer)
                    {
                        log::error!("Manifest reload error: {e}");
                    }
                }
                continue;
            }

            let ext = canon.extension().and_then(|e| e.to_str()).unwrap_or("");
            match ext {
                "png" | "jpg" | "jpeg" => {
                    // Drop registry borrow before `reload_sprite(&mut World)`.
                    let id_filter = {
                        let reg = self
                            .world
                            .get_resource::<AssetRegistry>()
                            .expect("AssetRegistry resource missing");
                        reg.sprite_name_for_path(&canon)
                            .and_then(|id| reg.get_sprite(id).map(|a| (id.to_string(), a.filter)))
                    };
                    if let Some((id, filter)) = id_filter {
                        if let Err(e) = asset_loader::reload_sprite(
                            &id,
                            &canon,
                            filter,
                            &mut self.world,
                            renderer,
                        ) {
                            log::error!("Sprite reload '{id}': {e}");
                        }
                    } else {
                        log::debug!("Hot reload: no sprite registered for '{}'", canon.display());
                    }
                }
                // Themes, animations, particle configs, prefabs and data
                // entries, by the paths their loaders recorded (M46, M61).
                "json" => json_paths.push(canon),
                "ttf" | "otf" => {
                    let id = self
                        .world
                        .get_resource::<FontRegistry>()
                        .and_then(|fr| fr.id_for_path(&canon).map(ToString::to_string));
                    if let Some(id) = id {
                        if let Err(e) = asset_loader::reload_font(&id, &canon, renderer) {
                            log::error!("Font reload '{id}': {e}");
                        }
                    } else {
                        log::debug!("Hot reload: no font registered for '{}'", canon.display());
                    }
                }
                "tmj" => {
                    let id = self
                        .world
                        .get_resource::<TilemapRegistry>()
                        .and_then(|tr| tr.id_for_path(&canon).map(ToString::to_string));
                    if let Some(id) = id {
                        if let Err(e) = asset_loader::reload_tilemap(&id, &canon, &mut self.world) {
                            log::error!("Tilemap reload '{id}': {e}");
                        }
                    } else {
                        log::debug!(
                            "Hot reload: no tilemap registered for '{}'",
                            canon.display()
                        );
                    }
                }
                "wgsl" => {
                    let id = self.world.get_resource::<ShaderRegistry>().and_then(|reg| {
                        reg.id_for_path(&canon)
                            .and_then(|sid| reg.name_for_id(sid).map(ToString::to_string))
                    });
                    if let Some(id) = id {
                        if let Err(e) =
                            asset_loader::reload_shader(&id, &canon, &mut self.world, renderer)
                        {
                            log::error!("Shader reload '{id}': {e}");
                        }
                    } else {
                        log::debug!("Hot reload: no shader registered for '{}'", canon.display());
                    }
                }
                _ => {}
            }
        }
        if !json_paths.is_empty() {
            asset_loader::reload_json_files(&json_paths, &mut self.world, manifest_reloaded);
        }
    }

    fn apply_pending_display_request(&mut self) {
        let Some(requested) = take_pending_display(&mut self.world) else {
            return;
        };

        let current = self
            .world
            .get_resource::<DisplayState>()
            .copied()
            .unwrap_or_default();
        let delta = DisplayDelta::between(&current, &requested);
        let mut effective = current;
        let runtime_mode = runtime_display_mode(requested.display_mode);
        if runtime_mode != requested.display_mode {
            self.exclusive_warning.call_once(|| {
                log::warn!(
                    "Display mode '{}' is not supported at runtime yet; downgrading to '{}'",
                    requested.display_mode.as_str(),
                    runtime_mode.as_str()
                );
            });
        }

        let Some(window) = self.window.as_ref() else {
            sync_display_state_and_telemetry(&mut self.world, current, None);
            return;
        };

        let mut actual_present_mode = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.gpu_timings.present_mode.clone());

        if delta.display_mode_changed {
            apply_window_fullscreen(window, runtime_mode);
            effective.display_mode = runtime_mode;

            let size = window.inner_size();
            if size.width > 0 && size.height > 0 {
                effective.resolution.width = size.width;
                effective.resolution.height = size.height;
            }
        }

        if delta.resize && matches!(effective.display_mode, DisplayMode::Windowed) {
            let fallback_size = window.inner_size();
            let actual_size = window
                .request_inner_size(winit::dpi::PhysicalSize::new(
                    requested.resolution.width,
                    requested.resolution.height,
                ))
                .unwrap_or(fallback_size);
            if actual_size.width > 0 && actual_size.height > 0 {
                effective.resolution.width = actual_size.width;
                effective.resolution.height = actual_size.height;
            }
        }

        if delta.surface_pacing_changed
            && let Some(renderer) = self.renderer.as_mut()
        {
            match renderer.reconfigure_surface_pacing(
                requested.present_mode,
                requested.vsync,
                requested.max_frame_latency,
            ) {
                Ok(()) => {
                    effective.vsync = requested.vsync;
                    effective.present_mode = requested.present_mode;
                    effective.max_frame_latency = requested.max_frame_latency;
                    actual_present_mode.clone_from(&renderer.gpu_timings.present_mode);
                    if let Some(gpu) = self.world.get_resource_mut::<GpuFrameTimings>() {
                        *gpu = renderer.gpu_timings.clone();
                    }
                }
                Err(err) => {
                    log::error!("Failed to apply display pacing change: {err}");
                }
            }
        }

        if delta.scale_mode_changed {
            effective.scale_mode = requested.scale_mode;
        }

        if delta.frame_rate_cap_changed {
            effective.frame_rate_cap = requested.frame_rate_cap;
            self.frame_budget = frame_budget_for(requested.frame_rate_cap);
        }

        if delta.resize && !matches!(effective.display_mode, DisplayMode::Windowed) {
            let size = window.inner_size();
            if size.width > 0 && size.height > 0 {
                effective.resolution.width = size.width;
                effective.resolution.height = size.height;
            }
        }

        sync_display_state_and_telemetry(&mut self.world, effective, actual_present_mode);
    }

    /// Reads the window's monitor again into [`MonitorInfo`](tungsten_core::MonitorInfo)
    /// (`D-184`); without a window it does nothing.
    fn refresh_monitor(&mut self) {
        if let Some(window) = &self.window {
            let monitor = read_monitor(window);
            set_monitor(&mut self.world, monitor);
        }
    }

    /// M27: apply any post-AA mode change requested via `request_post_aa`.
    /// Called between hot-reload and extract so SMAA reallocation never
    /// happens mid-frame.
    fn apply_pending_post_aa_request(&mut self) {
        let Some(requested) = take_pending_post_aa(&mut self.world) else {
            return;
        };
        // The renderer keeps SMAA off when its pipelines fail validation.
        let applied = self.renderer.as_mut().map_or(requested, |renderer| {
            renderer.set_post_aa(requested);
            renderer.post_aa()
        });
        sync_post_aa_state(&mut self.world, applied);
    }
}

/// The post stack a frame draws with: the user's stack, or `scratch` holding
/// its passes with the screen transition's pass last (M31, `D-093`).
fn compose_post_stack<'a>(
    user: &'a PostStack,
    transition_pass: Option<PostPass>,
    scratch: &'a mut PostStack,
) -> &'a PostStack {
    let Some(pass) = transition_pass else {
        return user;
    };
    scratch.0.clear();
    scratch.0.extend_from_slice(&user.0);
    scratch.0.push(pass);
    scratch
}

/// Where a frame's real dt comes from: the dt [`Time`]'s real clock advances
/// by, and its game clock by that times the scale, zero while paused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FrameClock {
    /// Elapsed wall time since the previous frame, capped at [`MAX_DT_SECS`]
    /// (or pinned to 60 Hz under `TUNGSTEN_SMOKE_FRAMES`): the window loop.
    Wall,
    /// This dt, as given: a headless caller.
    #[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
    Pinned(f32),
}

/// Why the run is ending (`D-180`); [`Exiting`] carries it through the last
/// frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExitReason {
    /// The game called [`request_exit`].
    Requested,
    /// The window's close button, or the OS, asked the window to close.
    CloseRequested,
}

/// A resource present for the run's last frame only (`D-180`): a system that
/// saves on quit reads it. Every stage runs in that frame, render included,
/// and the run ends after it. The `engine_exit` action and
/// `TUNGSTEN_SMOKE_FRAMES` end the run with no last frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Exiting {
    /// What ended the run.
    pub reason: ExitReason,
}

/// A [`request_exit`] the frame end has not yet seen.
pub(crate) struct PendingExit;

/// Ends the run after one more frame, which runs with [`Exiting`] (`D-180`):
/// called from a system, the next frame is the last; called between frames,
/// the frame after the next. A second call, or one during the last frame,
/// changes nothing. A game's Quit button calls it, then saves in the last
/// frame; the window's close button gives the same last frame.
pub fn request_exit(world: &mut World) {
    if !world.has_resource::<Exiting>() {
        world.insert_resource(PendingExit);
    }
}

/// How a frame ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameEnd {
    /// Every stage ran.
    Completed,
    /// `engine_exit` was just pressed; the stages after update did not run.
    ExitRequested,
    /// Every stage ran, but the render failed: the run's fatal error holds
    /// why, and the window loop exits (`D-142`).
    RenderFailed,
    /// Every stage ran with [`Exiting`]: the run's last frame (`D-180`). The
    /// window loop exits after presenting it.
    Last,
}

// Extract output kept in umbrella crate; renderer stays World-free.
pub(crate) struct FrameExtract {
    pub(crate) quads: Vec<QuadInstance>,
    pub(crate) sprites: Vec<SpriteBatch>,
    pub(crate) text: Vec<TextSection>,
    pub(crate) debug_quads: Vec<QuadInstance>,
    pub(crate) debug_lines: Vec<DebugLineInstance>,
    /// M29 per-frame lighting payload; uploaded to the GPU once per render
    /// stage. With no lights this carries `count = 0` + ambient default.
    pub(crate) light_ubo: tungsten_render::LightUbo,
    /// M31 mesh particles, one batch per mesh. Engine-owned extract: there is
    /// no setter.
    pub(crate) mesh_particles: Vec<MeshParticleBatch>,
    /// M46 UI paint list: a copy of the app's retained one.
    pub(crate) ui: PaintList,
    /// M56: the text runs of the roots a prewarm asked for, painted on the
    /// frame they were first laid out; render rasterizes them before its
    /// prepare.
    pub(crate) ui_prewarm: Option<PaintList>,
    /// M54 world labels, projected, culled and capped.
    pub(crate) world_labels: Vec<WorldLabelRun>,
    extract_ms: f32,
}

/// What the retained UI paint list was painted with (M46).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PaintedAt {
    scale_bits: u32,
    debug: UiDebug,
    sprite_epoch: u64,
}

#[allow(clippy::struct_field_names)]
#[derive(Default)]
struct FrameRenderOut {
    render_ms: f32,
    render_acquire_ms: f32,
    render_encode_ms: f32,
    render_encode_pre_acquire_ms: f32,
    render_submit_present_ms: f32,
    gpu_frame_ms: Option<f32>,
}

// One-pass telemetry write; no long `&mut FrameTimings` borrow across stages.
struct FrameStageTimings {
    update_ms: f32,
    flush_ms: f32,
    hot_reload_ms: f32,
    extract_ms: f32,
    render_ms: f32,
    render_acquire_ms: f32,
    render_encode_ms: f32,
    render_encode_pre_acquire_ms: f32,
    render_submit_present_ms: f32,
    audio_ms: f32,
    total_ms: f32,
    interval_ms: Option<f32>,
    fixed_steps: u32,
    system_timings: Vec<(String, f32)>,
}

impl App {
    // Debug perf: flatten single-call frame stages to avoid stack/memcpy overhead.

    /// Advances [`Time`] by the frame's real dt: the pinned dt as given, or a
    /// wall frame's capped elapsed time, zero with no previous frame time.
    /// `DeltaTime` gets the game dt, or the real dt if a game removed `Time`.
    #[inline(always)]
    fn stage_time(&mut self, clock: FrameClock) {
        let real_dt = match clock {
            FrameClock::Pinned(dt) => dt,
            FrameClock::Wall => {
                let now = Instant::now();
                let dt = self.last_frame.map_or(0.0, |last| {
                    frame_dt_secs(
                        now.duration_since(last),
                        self.smoke_frames_remaining.is_some(),
                    )
                });
                self.last_frame = Some(now);
                dt
            }
        };
        let game_dt = match self.world.get_resource_mut::<Time>() {
            Some(time) => {
                time.advance_frame(real_dt);
                time.game_delta()
            }
            None => real_dt,
        };
        #[allow(deprecated)]
        if let Some(delta) = self.world.get_resource_mut::<DeltaTime>() {
            delta.dt = game_dt;
        }
    }

    /// Runs `stage`'s systems, each timed into `system_timings`.
    #[inline(always)]
    fn run_stage(&mut self, stage: Stage, system_timings: &mut Vec<(String, f32)>) {
        // Input edges: events received before RedrawRequested in same loop turn.
        self.schedule
            .run_stage(stage, &mut self.world, |name, elapsed| {
                system_timings.push((name.to_string(), elapsed.as_secs_f64() as f32 * 1000.0));
            });
    }

    /// `FixedUpdate` once per fixed step the frame's [`Time`] owes, zero or
    /// more times (`D-129`), and returns the steps run. Inside the steps
    /// `Time::delta()` and `DeltaTime` read the step, and input edges and
    /// event queues their step views; after the last, the game dt and the
    /// frame views again. Each system's runs are summed under its name, and
    /// a frame with no step lists each at 0 ms and, while the game clock
    /// advances, holds the event queues for the next step (`D-139`); while
    /// it stands still and real time runs, it drops the fixed view's input
    /// edges (`D-146`). A world without `Time` runs the stage once, as
    /// before the accumulator.
    #[inline(always)]
    fn stage_fixed_update(&mut self, system_timings: &mut Vec<(String, f32)>) -> u32 {
        let Some((steps, step)) = self
            .world
            .get_resource::<Time>()
            .map(|time| (time.fixed_steps_this_frame(), time.fixed_step()))
        else {
            self.run_stage(Stage::FixedUpdate, system_timings);
            return 1;
        };
        if steps == 0 {
            system_timings.extend(
                self.schedule
                    .names(Stage::FixedUpdate)
                    .map(|name| (name.to_string(), 0.0)),
            );
            // The frame's events wait for the next step's readers, unless
            // the clock stands still or no fixed system could read them.
            let (advancing, stopped) =
                self.world
                    .get_resource::<Time>()
                    .map_or((false, false), |time| {
                        let game_dt = time.game_delta();
                        (game_dt > 0.0, game_dt == 0.0 && time.real_delta() > 0.0)
                    });
            if advancing && self.schedule.names(Stage::FixedUpdate).next().is_some() {
                self.world.hold_events_for_fixed_step();
            }
            // Edges made while the clock stands still reach no step, as its
            // events do not; a frame with no time at all keeps them.
            if stopped {
                self.end_input_steps();
            }
            return 0;
        }
        let first = system_timings.len();
        for k in 0..steps {
            if let Some(time) = self.world.get_resource_mut::<Time>() {
                time.enter_fixed_step();
            }
            self.set_step_views(true, step);
            if k == 0 {
                self.run_stage(Stage::FixedUpdate, system_timings);
            } else {
                // The k-th system adds to the entry the first step pushed.
                let mut entry = first;
                self.schedule
                    .run_stage(Stage::FixedUpdate, &mut self.world, |_, elapsed| {
                        system_timings[entry].1 += elapsed.as_secs_f64() as f32 * 1000.0;
                        entry += 1;
                    });
            }
            self.end_input_steps();
            self.world.end_fixed_step_events();
        }
        let game_dt = self.world.get_resource_mut::<Time>().map(|time| {
            time.leave_fixed_steps();
            time.game_delta()
        });
        self.set_step_views(false, game_dt.unwrap_or(step));
        steps
    }

    /// Turns the input and event step views on or off, the routed input's
    /// with the raw (`D-149`), and writes `dt` to `DeltaTime`: the step
    /// inside the steps, the game dt after them.
    #[inline(always)]
    fn set_step_views(&mut self, on: bool, dt: f32) {
        #[allow(deprecated)]
        if let Some(delta) = self.world.get_resource_mut::<DeltaTime>() {
            delta.dt = dt;
        }
        if let Some(input) = self.world.get_resource_mut::<InputState>() {
            input.set_fixed_view(on);
        }
        if let Some(routed) = self.world.get_resource_mut::<RoutedInput>() {
            routed.set_fixed_view(on);
        }
        self.world.set_fixed_event_view(on);
    }

    /// Ends the fixed view's step in the raw and the routed input (`D-146`,
    /// `D-149`).
    #[inline(always)]
    fn end_input_steps(&mut self) {
        if let Some(input) = self.world.get_resource_mut::<InputState>() {
            input.end_fixed_step();
        }
        if let Some(routed) = self.world.get_resource_mut::<RoutedInput>() {
            routed.end_fixed_step();
        }
    }

    /// The system cursor last set on the window.
    #[cfg(test)]
    pub(crate) fn applied_cursor(&self) -> CursorIcon {
        self.cursor
    }

    /// Sets the window's cursor to the one the UI router asks for, when it
    /// changed (`D-149`).
    #[inline(always)]
    fn stage_cursor(&mut self) {
        let Some(cursor) = self.world.get_resource::<UiRouter>().map(UiRouter::cursor) else {
            return;
        };
        if cursor == self.cursor {
            return;
        }
        self.cursor = cursor;
        if let Some(window) = &self.window {
            window.set_cursor(input_bridge::cursor_icon(cursor));
        }
    }

    /// `Startup` once, then `PreUpdate`, the frame's fixed steps and
    /// `Update`; `None` when the frame's update asked the engine to exit,
    /// else the time so far, the per-system timings `stage_post_update`
    /// extends and the fixed steps run.
    #[inline(always)]
    fn stage_update(&mut self) -> (Instant, Vec<(String, f32)>, u32, bool) {
        let update_start = Instant::now();
        if !self.schedule.is_resolved() {
            self.resolve_schedule()
                .expect("the schedule resolves before the first frame");
        }
        let mut system_timings: Vec<(String, f32)> = Vec::with_capacity(self.schedule.len());
        if !self.startup_done {
            self.startup_done = true;
            self.run_stage(Stage::Startup, &mut system_timings);
            // The stage's time is excluded from the next frame's dt, as the
            // startup hook's is: a slow startup system must not inflate it.
            self.last_frame = Some(Instant::now());
        }
        self.run_stage(Stage::PreUpdate, &mut system_timings);
        self.routed_events = self
            .world
            .get_resource::<InputState>()
            .map_or(0, |input| input.raw_events().len());
        let fixed_steps = self.stage_fixed_update(&mut system_timings);
        self.run_stage(Stage::Update, &mut system_timings);
        let exit = self.engine_exit_requested();
        (update_start, system_timings, fixed_steps, exit)
    }

    /// `PostUpdate`: physics sync, particles, tweens, game feel, camera and
    /// a game's own, before the command flush. Tween writes override
    /// particle writes; `TweenComplete` lands before the event flush rotates.
    #[inline(always)]
    fn stage_post_update(&mut self, system_timings: &mut Vec<(String, f32)>) {
        self.run_stage(Stage::PostUpdate, system_timings);
    }

    #[inline(always)]
    fn stage_flush_commands(&mut self) -> f32 {
        // Frame order: systems -> command flush -> event flush -> hot reload -> extract -> render.
        // Frame-N command mutations: invisible to systems, visible to extract/render.
        let flush_start = Instant::now();
        // The buffer goes back in drained, so its storage serves the next frame.
        let mut flush_buf = self
            .world
            .remove_resource::<CommandBuffer>()
            .expect("CommandBuffer resource missing -- was it removed by a system?");
        self.world.flush_reusing(&mut flush_buf);
        self.world.insert_resource(flush_buf);
        flush_start.elapsed().as_secs_f64() as f32 * 1000.0
    }

    #[inline(always)]
    fn stage_flush_events(&mut self) {
        self.world.flush_events();
    }

    #[inline(always)]
    fn stage_hot_reload(&mut self) -> f32 {
        let hot_reload_start = Instant::now();
        self.process_hot_reload();
        hot_reload_start.elapsed().as_secs_f64() as f32 * 1000.0
    }

    /// Lays the `UiTree` out at the window size in logical units (`D-127`)
    /// through the renderer's text nodes, before the extract paints it. With
    /// no renderer (the harness) it does nothing: `UiHarness` lays out after
    /// the frame with its own text double. Returns the stage's milliseconds
    /// and records the stats on `RenderCounts`.
    #[inline(always)]
    fn stage_ui_layout(&mut self) -> f32 {
        let Some(renderer) = self.renderer.as_mut() else {
            return 0.0;
        };
        let Some(size) = self.world.get_resource::<WindowSize>().copied() else {
            return 0.0;
        };
        let scale = self
            .world
            .get_resource::<tungsten_core::ScaleFactor>()
            .map_or(1.0, tungsten_core::ScaleFactor::get);
        let viewport = glam::Vec2::new(size.width as f32 / scale, size.height as f32 / scale);
        let start = Instant::now();
        let Some(tree) = self.world.get_resource_mut::<UiTree>() else {
            return 0.0;
        };
        let stats = tree.layout(viewport, &mut renderer.text_nodes());
        let layout_ms = start.elapsed().as_secs_f64() as f32 * 1000.0;
        if let Some(rc) = self.world.get_resource_mut::<RenderCounts>() {
            rc.ui_layout_us = (layout_ms * 1000.0) as u32;
            rc.ui_roots_laid_out = stats.roots_laid_out;
            rc.ui_nodes_laid_out = stats.nodes_laid_out;
        }
        layout_ms
    }

    /// Repaints the retained UI paint list when the tree is paint-dirty or
    /// the scale, the `UiDebug` flags or the sprite epoch moved since the
    /// last paint, so a static tree follows a debug toggle, an atlas repack
    /// and a scale change (M46).
    #[inline(always)]
    fn paint_ui(&mut self) {
        let scale = self
            .world
            .get_resource::<tungsten_core::ScaleFactor>()
            .map_or(1.0, tungsten_core::ScaleFactor::get);
        let debug = self
            .world
            .get_resource::<UiDebug>()
            .copied()
            .unwrap_or_default();
        let sprite_epoch = self
            .world
            .get_resource::<AssetRegistry>()
            .map_or(0, AssetRegistry::sprite_epoch);
        let at = PaintedAt {
            scale_bits: scale.to_bits(),
            debug,
            sprite_epoch,
        };
        let dirty = self
            .world
            .get_resource::<UiTree>()
            .is_some_and(UiTree::is_paint_dirty);
        if !dirty && self.painted_at == Some(at) {
            return;
        }
        // Borrow split: the tree paints with `&World`'s registry.
        let Some(mut tree) = self.world.remove_resource::<UiTree>() else {
            return;
        };
        let empty = AssetRegistry::new();
        let sprites = self.world.get_resource::<AssetRegistry>().unwrap_or(&empty);
        tree.paint(scale, &debug, sprites, &mut self.ui_paint);
        self.world.insert_resource(tree);
        self.painted_at = Some(at);
    }

    /// Paints the text runs of the roots a prewarm asked for (M56); `None`
    /// when no request waits or none is laid out yet.
    fn paint_prewarm(&mut self) -> Option<PaintList> {
        let scale = self
            .world
            .get_resource::<tungsten_core::ScaleFactor>()
            .map_or(1.0, tungsten_core::ScaleFactor::get);
        let tree = self.world.get_resource_mut::<UiTree>()?;
        if !tree.prewarm_pending() {
            return None;
        }
        let mut list = PaintList::default();
        tree.paint_prewarm(scale, &mut list).then_some(list)
    }

    #[inline(always)]
    fn stage_extract(&mut self) -> FrameExtract {
        let extract_start = Instant::now();
        // The sprite extract culls at the size render projects with
        // (`D-114`); a pending resize leaves `WindowSize` ahead of the surface.
        if let (Some(renderer), Some(scratch)) =
            (&self.renderer, self.world.get_resource::<ExtractScratch>())
        {
            let cfg = &renderer.surface_config;
            scratch.set_viewport(cfg.width, cfg.height);
        }
        // The channels; without `Extracts`, which only a removal causes,
        // they draw nothing.
        let (quads, sprites, text) = match self.world.get_resource::<Extracts>() {
            Some(extracts) => (
                extracts.quads(&self.world),
                extracts.sprites(&self.world),
                extracts.text(&self.world),
            ),
            None => (Vec::new(), Vec::new(), Vec::new()),
        };

        // M46: the UI paint list, before the HUD so its row counts it.
        self.paint_ui();
        // M56: a pending prewarm's runs.
        let ui_prewarm = self.paint_prewarm();

        // M54: the world labels and the lights, projected onto the size
        // render draws at; a pending resize leaves `WindowSize` ahead of it.
        let viewport = match &self.renderer {
            Some(renderer) => {
                let cfg = &renderer.surface_config;
                (cfg.width as f32, cfg.height as f32)
            }
            None => self
                .world
                .get_resource::<WindowSize>()
                .map_or((0.0, 0.0), |w| (w.width as f32, w.height as f32)),
        };
        let (world_labels, label_counts) =
            crate::extract::extract_world_labels(&mut self.world, viewport);

        // Before HUD compose: counts row uses this frame's extract.
        let entity_count = self.world.entity_count();
        let sprite_instance_count: u32 = sprites.iter().map(|b| b.instances.len() as u32).sum();
        let ui_commands = self.ui_paint.commands.len() as u32;
        if let Some(rc) = self.world.get_resource_mut::<RenderCounts>() {
            rc.entities = entity_count;
            rc.sprite_instances = sprite_instance_count;
            rc.ui_commands = ui_commands;
            rc.world_labels = label_counts.kept;
            rc.world_labels_culled = label_counts.culled;
            rc.world_labels_dropped = label_counts.dropped;
        }

        // Extract-start emit: debug draw visible same frame.
        physics_debug_emit_system(&mut self.world);

        // AABBs -> quads; angled lines/circles -> debug-line pipeline.
        let (debug_quads, debug_lines) = drain_debug_draw(&mut self.world);

        // M29: pull lights into a UBO every frame regardless of presence so
        // the lit pipeline (when active) reads coherent ambient + count = 0.
        let camera = self
            .world
            .get_resource::<CameraState>()
            .copied()
            .unwrap_or_default();
        let light_ubo =
            crate::light_extract::extract_lights(&self.world, &camera, viewport.0, viewport.1);

        let mesh_particles = crate::particles::extract_mesh_particles(&self.world);

        let extract_ms = extract_start.elapsed().as_secs_f64() as f32 * 1000.0;
        FrameExtract {
            quads,
            sprites,
            text,
            debug_quads,
            debug_lines,
            light_ubo,
            mesh_particles,
            ui: self.ui_paint.clone(),
            ui_prewarm,
            world_labels,
            extract_ms,
        }
    }

    /// Renders the frame and books its result (`D-142`); the [`FrameEnd`] is
    /// how the frame will end, `RenderFailed` on a failed render.
    #[inline(always)]
    fn stage_render(&mut self, extract: &FrameExtract) -> (FrameRenderOut, FrameEnd) {
        let render_start = Instant::now();
        let mut out = FrameRenderOut::default();
        let mut result = None;
        if let Some(renderer) = &mut self.renderer {
            let (vw, vh) = {
                let cfg = &renderer.surface_config;
                (cfg.width as f32, cfg.height as f32)
            };
            let view_proj = self
                .world
                .get_resource::<CameraState>()
                .copied()
                .unwrap_or_default()
                .view_projection(vw, vh);

            // The capture is armed for its frame alone; a skipped or failed
            // frame drops it, and the outcome carries its result.
            if let Some(cfg) = self
                .capture
                .as_ref()
                .and_then(|capture| capture.armed_for(self.frames_rendered + 1))
            {
                if cfg.direct {
                    renderer.capture_frame_direct(&cfg.path);
                } else {
                    renderer.capture_frame(&cfg.path);
                }
            }

            // M26: PostStack is a world resource; default is empty.
            let empty_stack = PostStack::default();
            let post_stack = self
                .world
                .get_resource::<PostStack>()
                .unwrap_or(&empty_stack);
            // M31: a screen transition draws as the last post-stack slot, on
            // a copy of the user's stack, so a state's `on_enter` may clear
            // or rebuild `PostStack` without losing or leaking the pass.
            let transition_pass = self
                .world
                .get_resource::<StateStack>()
                .and_then(StateStack::transition_pass);
            let post_stack =
                compose_post_stack(post_stack, transition_pass, &mut self.transition_post_stack);

            // M29: upload the per-frame light UBO before any draw records it.
            renderer.update_lights(&extract.light_ubo);
            // M31: this frame's mesh particle instances; an empty list clears
            // the previous frame's draws.
            renderer.update_mesh_particles(&extract.mesh_particles);
            // M54: this frame's world labels; an empty list draws none.
            renderer.update_world_labels(&extract.world_labels);
            // M56: a prewarm's runs reach the atlas before this frame's
            // prepare.
            if let Some(list) = &extract.ui_prewarm {
                renderer.prewarm_ui_text(list);
            }

            result = Some(if self.gpu_timing_enabled {
                renderer.render_frame_full_timed(
                    &view_proj,
                    &extract.quads,
                    &extract.sprites,
                    &extract.debug_quads,
                    &extract.debug_lines,
                    &extract.text,
                    &extract.ui,
                    post_stack,
                )
            } else {
                renderer.render_frame_full(
                    &view_proj,
                    &extract.quads,
                    &extract.sprites,
                    &extract.debug_quads,
                    &extract.debug_lines,
                    &extract.text,
                    &extract.ui,
                    post_stack,
                )
            });
            // M46: the text batch counts the HUD shows next frame.
            let text_stats = renderer.text_stats();
            if let Some(rc) = self.world.get_resource_mut::<RenderCounts>() {
                rc.text_batches = text_stats.batches;
                rc.text_batches_prepared = text_stats.prepared;
                rc.text_full_frames = text_stats.full_frames_total;
                rc.text_atlas_full = text_stats.atlas_full_total;
                rc.text_label_glyphs = text_stats.label_glyphs;
                rc.text_layouts = text_stats.layouts;
            }
            out.render_acquire_ms = renderer.cpu_timings.acquire_ms;
            out.render_encode_ms = renderer.cpu_timings.encode_ms;
            out.render_encode_pre_acquire_ms = renderer.cpu_timings.encode_pre_acquire_ms;
            out.render_submit_present_ms = renderer.cpu_timings.submit_present_ms;
            out.gpu_frame_ms = renderer.gpu_timings.frame_gpu_ms;
            if let Some(gpu) = self.world.get_resource_mut::<GpuFrameTimings>() {
                *gpu = renderer.gpu_timings.clone();
            }
        }
        let end = result.map_or(FrameEnd::Completed, |result| self.settle_render(result));
        out.render_ms = render_start.elapsed().as_secs_f64() as f32 * 1000.0;
        (out, end)
    }

    /// Books the render result of the frame after `frames_rendered`
    /// (`D-142`). A presented or skipped frame counts, and settles the
    /// capture when it is the capture's frame. A failed frame is the run's
    /// fatal error, naming the frame and logged once, and ends
    /// `RenderFailed`.
    fn settle_render(&mut self, result: Result<RenderOutcome, RenderError>) -> FrameEnd {
        let frame = self.frames_rendered + 1;
        match result {
            Ok(outcome) => {
                self.frames_rendered = frame;
                if let Some(capture) = &mut self.capture {
                    capture.frame_rendered(frame, outcome);
                }
                FrameEnd::Completed
            }
            Err(error) => {
                if self.fatal_error.is_none() {
                    let error = anyhow::Error::new(error)
                        .context(format!("render failed at frame {frame}"));
                    log::error!("{error:#}");
                    self.fatal_error = Some(error);
                }
                FrameEnd::RenderFailed
            }
        }
    }

    /// Hands the drawn frame's sprite batches back to the extract scratch:
    /// the next frame's batches reuse their instance vectors.
    #[inline(always)]
    fn stage_recycle(&mut self, extract: &mut FrameExtract) {
        if let Some(scratch) = self.world.get_resource::<ExtractScratch>() {
            scratch.recycle(std::mem::take(&mut extract.sprites));
        }
    }

    /// Checks the device's stream (a lost one is rebuilt), sends the frame's
    /// audio commands, each sound's plays capped at its voice limit (the
    /// last ones kept), to the device or to the capture list, then moves
    /// waiting commands into the ring, every frame.
    #[inline(always)]
    fn stage_audio(&mut self) -> f32 {
        let audio_start = Instant::now();
        if let (Some(audio), Some(sounds)) =
            (&mut self.audio, self.world.get_resource::<SoundRegistry>())
        {
            audio.maintain(sounds);
        }
        let mut commands = self
            .world
            .get_resource_mut::<AudioCommands>()
            .map(AudioCommands::drain)
            .unwrap_or_default();
        if !commands.is_empty() {
            let sounds = self.world.get_resource::<SoundRegistry>();
            crate::audio::cap_plays(&mut commands, |handle| {
                sounds.map_or(tungsten_core::MAX_VOICES, |sounds| {
                    sounds.get_max_voices(handle)
                })
            });
            for cmd in commands {
                if let Some(audio) = &mut self.audio {
                    audio.send(cmd);
                } else if let Some(capture) = &mut self.audio_capture {
                    capture.push(cmd);
                }
            }
        }
        if let Some(audio) = &mut self.audio {
            audio.flush();
        }
        audio_start.elapsed().as_secs_f64() as f32 * 1000.0
    }

    #[inline(always)]
    fn stage_telemetry(&mut self, t: FrameStageTimings) {
        if let Some(ft) = self.world.get_resource_mut::<FrameTimings>() {
            ft.update_ms = t.update_ms;
            ft.extract_ms = t.extract_ms;
            ft.render_ms = t.render_ms;
            ft.render_acquire_ms = t.render_acquire_ms;
            ft.render_encode_ms = t.render_encode_ms;
            ft.render_encode_pre_acquire_ms = t.render_encode_pre_acquire_ms;
            ft.render_submit_present_ms = t.render_submit_present_ms;
            ft.audio_ms = t.audio_ms;
            ft.hot_reload_ms = t.hot_reload_ms;
            ft.flush_ms = t.flush_ms;
            ft.total_ms = t.total_ms;
            ft.interval_ms = t.interval_ms;
            ft.fixed_steps = t.fixed_steps;
            ft.system_timings = t.system_timings;
        }
    }

    /// One frame, from the interval measurement through the perf log, in the
    /// order `DESIGN.md`'s frame loop lists. The window loop runs it on each
    /// redraw with [`FrameClock::Wall`], then paces; a headless caller runs it
    /// with a pinned dt. `inspect` sees the frame's extract between render and
    /// recycle.
    #[inline(always)]
    pub(crate) fn run_frame(
        &mut self,
        frame_start: Instant,
        clock: FrameClock,
        inspect: impl FnOnce(&FrameExtract),
    ) -> FrameEnd {
        let interval_ms =
            frame_interval_ms(self.prev_frame_start.replace(frame_start), frame_start);
        self.apply_pending_display_request();

        self.stage_time(clock);
        let (update_start, mut system_timings, fixed_steps, exit) = self.stage_update();
        if exit {
            return FrameEnd::ExitRequested;
        }
        self.stage_cursor();
        self.stage_post_update(&mut system_timings);
        let update_ms = update_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let flush_ms = self.stage_flush_commands();
        self.stage_flush_events();
        let hot_reload_ms = self.stage_hot_reload();
        self.apply_pending_post_aa_request();

        // M57: the debug views' windows, from the previous frame's timings,
        // before the layout lays them out; counted in `extract` as it is.
        let views_start = Instant::now();
        crate::debug_views::update_with(&mut self.world, Some(&self.ui_paint));
        let views_ms = views_start.elapsed().as_secs_f64() as f32 * 1000.0;
        // M46: the UI layout, counted in `extract` (Q11).
        let ui_layout_ms = self.stage_ui_layout();
        let mut extract_out = self.stage_extract();
        let extract_ms = extract_out.extract_ms + ui_layout_ms + views_ms;

        let (render_out, end) = self.stage_render(&extract_out);
        inspect(&extract_out);
        self.stage_recycle(&mut extract_out);

        let audio_ms = self.stage_audio();

        if let Some(input) = self.world.get_resource_mut::<InputState>() {
            input.begin_frame();
        }

        let total_ms = frame_start.elapsed().as_secs_f64() as f32 * 1000.0;
        self.stage_telemetry(FrameStageTimings {
            update_ms,
            flush_ms,
            hot_reload_ms,
            extract_ms,
            render_ms: render_out.render_ms,
            render_acquire_ms: render_out.render_acquire_ms,
            render_encode_ms: render_out.render_encode_ms,
            render_encode_pre_acquire_ms: render_out.render_encode_pre_acquire_ms,
            render_submit_present_ms: render_out.render_submit_present_ms,
            audio_ms,
            total_ms,
            interval_ms,
            fixed_steps,
            system_timings,
        });

        if std::env::var("TUNGSTEN_PERF_LOG").is_ok() {
            log_perf_line(
                total_ms,
                interval_ms,
                update_ms,
                flush_ms,
                extract_ms,
                &render_out,
                audio_ms,
                hot_reload_ms,
            );
            if log::log_enabled!(log::Level::Debug)
                && let Some(ft) = self.world.get_resource::<FrameTimings>()
                && !ft.system_timings.is_empty()
            {
                log::debug!("{}", format_perf_systems_line(&ft.system_timings));
            }
            if log::log_enabled!(log::Level::Debug)
                && let Some(gpu) = self.world.get_resource::<GpuFrameTimings>()
            {
                // Emit even an empty line on skipped/unsupported frames so
                // capture warm-up counts remain aligned with frame logs.
                let mut line = format_perf_named_timings("gpu_passes:", &gpu.pass_gpu_ms);
                if let Some(span) = gpu.render_gpu_ms {
                    use std::fmt::Write as _;
                    let _ = write!(line, " render_span={span:.2}ms");
                }
                log::debug!("{line}");
            }
            if log::log_enabled!(log::Level::Debug)
                && let Some(buffers) = self.world.get_resource::<PhysicsBuffers>()
            {
                log::debug!("{}", format_perf_physics_line(buffers));
            }
        }

        self.settle_exit(end)
    }

    /// `D-180`: a frame that ran with [`Exiting`] was the run's last, unless
    /// its render failed; a [`request_exit`] since the last check makes the
    /// next frame the last.
    #[inline(always)]
    fn settle_exit(&mut self, end: FrameEnd) -> FrameEnd {
        if self.world.has_resource::<Exiting>() {
            return match end {
                FrameEnd::Completed => FrameEnd::Last,
                other => other,
            };
        }
        if self.world.remove_resource::<PendingExit>().is_some() {
            self.world.insert_resource(Exiting {
                reason: ExitReason::Requested,
            });
        }
        end
    }

    /// `D-180`: the close button's last frame, run at once from the event on
    /// the wall clock, so a minimized window that gets no redraw still
    /// saves; the run then ends whatever the frame returned. With no
    /// renderer yet, the run ends at once.
    fn close_with_last_frame(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            if !self.world.has_resource::<Exiting>() {
                self.world.insert_resource(Exiting {
                    reason: ExitReason::CloseRequested,
                });
            }
            self.run_frame(Instant::now(), FrameClock::Wall, |_| {});
        }
        event_loop.exit();
    }

    #[inline(always)]
    fn stage_pacing(&mut self, event_loop: &ActiveEventLoop, frame_start: Instant) {
        match redraw_schedule(self.frame_budget, frame_start) {
            RedrawSchedule::Immediate => {
                self.redraw_deadline = None;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            // A redraw requested here would wake the loop at once and the
            // deadline would never be waited for; `about_to_wait` requests it.
            RedrawSchedule::At(deadline) => {
                self.redraw_deadline = Some(deadline);
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        }
    }

    #[inline(always)]
    fn stage_smoke_exit(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(remaining) = self.smoke_frames_remaining.as_mut() {
            *remaining = remaining.saturating_sub(1);
            if *remaining == 0 {
                log::info!("TUNGSTEN_SMOKE_FRAMES reached; exiting cleanly");
                event_loop.exit();
            }
        }
    }
}

/// When the frame loop asks for its next redraw once a frame has ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RedrawSchedule {
    /// Request it at frame end.
    Immediate,
    /// Leave it to `about_to_wait`, which requests it once this instant has passed.
    At(Instant),
}

fn redraw_schedule(frame_budget: Option<Duration>, frame_start: Instant) -> RedrawSchedule {
    match frame_budget {
        Some(budget) => RedrawSchedule::At(frame_start + budget),
        None => RedrawSchedule::Immediate,
    }
}

/// Milliseconds from the previous frame's start to this frame's start;
/// `None` on the first frame.
fn frame_interval_ms(prev_frame_start: Option<Instant>, frame_start: Instant) -> Option<f32> {
    prev_frame_start.map(|prev| frame_start.duration_since(prev).as_secs_f64() as f32 * 1000.0)
}

/// The manifests whose edit reloads the manifest graph: the composition roots
/// (`D-052`) when the app declared any, else the manifest given to
/// `enable_hot_reload`.
fn manifest_reload_roots(roots: &[PathBuf], manifest_path: Option<&Path>) -> Vec<PathBuf> {
    if roots.is_empty() {
        manifest_path.map(Path::to_path_buf).into_iter().collect()
    } else {
        roots.to_vec()
    }
}

/// True when the canonical path `canon` names one of `reload_roots`.
fn is_reload_root(canon: &Path, reload_roots: &[PathBuf]) -> bool {
    reload_roots
        .iter()
        .any(|root| root.canonicalize().unwrap_or_else(|_| root.clone()) == canon)
}

/// True when the canonical path `canon` names the action map's own file,
/// not just any file called `input.json` (`D-123`).
fn is_action_map(canon: &Path, input_map_path: &Path) -> bool {
    input_map_path
        .canonicalize()
        .unwrap_or_else(|_| input_map_path.to_path_buf())
        == canon
}

/// Simulated seconds for a frame that starts `elapsed` after the previous
/// one.
///
/// Smoke mode (`TUNGSTEN_SMOKE_FRAMES` set) pins dt to 60 Hz so physics,
/// particles, tweens and scene animation produce frame-accurate deterministic
/// output. Wall-clock dt varies with CPU load and makes visual-regression
/// captures drift between runs on the same binary. The capture path used by
/// Whether `input` is a press, not an OS repeat, of a key or button in
/// `bindings`.
pub(crate) fn presses(bindings: &[Binding], input: RawInput) -> bool {
    match input {
        RawInput::Key {
            code,
            pressed: true,
            repeat: false,
            ..
        } => bindings.contains(&Binding::Key { code }),
        RawInput::MouseButton {
            button,
            pressed: true,
            ..
        } => bindings.contains(&Binding::Mouse { button }),
        _ => false,
    }
}

/// `visual_regression.rs` (smoke frames + capture frame env vars) is the
/// canonical consumer. A normal frame gets the elapsed time, capped at
/// [`MAX_DT_SECS`].
fn frame_dt_secs(elapsed: Duration, smoke: bool) -> f32 {
    if smoke {
        SMOKE_MODE_FIXED_DT_SECS
    } else {
        elapsed.as_secs_f32().min(MAX_DT_SECS)
    }
}

#[inline(always)]
fn drain_debug_draw(world: &mut World) -> (Vec<QuadInstance>, Vec<DebugLineInstance>) {
    let mut debug_quads: Vec<QuadInstance> = Vec::new();
    let mut debug_lines: Vec<DebugLineInstance> = Vec::new();
    if let Some(dd) = world.get_resource_mut::<DebugDraw>() {
        for cmd in dd.drain() {
            match cmd.shape {
                DebugShape::Aabb { min, max } => {
                    expand_aabb(&mut debug_quads, min, max, cmd.color, cmd.thickness);
                }
                DebugShape::Circle {
                    center,
                    radius,
                    segments,
                } => {
                    expand_circle(
                        &mut debug_lines,
                        center,
                        radius,
                        segments,
                        cmd.color,
                        cmd.thickness,
                    );
                }
                DebugShape::Line { a, b } => {
                    debug_lines.push(DebugLineInstance {
                        a: a.to_array(),
                        b: b.to_array(),
                        thickness: cmd.thickness,
                        _pad: 0.0,
                        color: cmd.color,
                    });
                }
            }
        }
    }
    (debug_quads, debug_lines)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)] // One value per field of the line.
fn log_perf_line(
    total_ms: f32,
    interval_ms: Option<f32>,
    update_ms: f32,
    flush_ms: f32,
    extract_ms: f32,
    render_out: &FrameRenderOut,
    audio_ms: f32,
    hot_reload_ms: f32,
) {
    let gpu_for_log = render_out
        .gpu_frame_ms
        .map_or_else(|| "n/a".to_string(), |ms| format!("{ms:.2}ms"));
    // The first frame has no previous frame start.
    let interval_for_log = interval_ms.map_or_else(|| "n/a".to_string(), |ms| format!("{ms:.2}ms"));
    log::debug!(
        "frame: total={:.2}ms interval={} update={:.2}ms flush={:.2}ms extract={:.2}ms render={:.2}ms render_acquire={:.2}ms render_encode={:.2}ms render_encode_pre_acquire={:.2}ms render_submit_present={:.2}ms gpu={} audio={:.2}ms hot_reload={:.2}ms",
        total_ms,
        interval_for_log,
        update_ms,
        flush_ms,
        extract_ms,
        render_out.render_ms,
        render_out.render_acquire_ms,
        render_out.render_encode_ms,
        render_out.render_encode_pre_acquire_ms,
        render_out.render_submit_present_ms,
        gpu_for_log,
        audio_ms,
        hot_reload_ms
    );
}

/// Per-system companion to the `frame:` perf line, in registration order.
/// Whitespace and `=` in names become `_` so `scripts/bench_report.py` can
/// split `name=ms` tokens on whitespace.
fn format_perf_systems_line(system_timings: &[(String, f32)]) -> String {
    format_perf_named_timings("systems:", system_timings)
}

fn format_perf_named_timings(tag: &str, system_timings: &[(String, f32)]) -> String {
    use std::fmt::Write as _;

    let mut line = String::from(tag);
    for (name, ms) in system_timings {
        line.push(' ');
        line.extend(name.chars().map(|c| {
            if c.is_whitespace() || c == '=' {
                '_'
            } else {
                c
            }
        }));
        let _ = write!(line, "={ms:.2}ms");
    }
    line
}

/// Physics companion to the `frame:` perf line: last step's proxy, dynamic
/// and sleeping body counts plus the final substep's pairs and contacts.
/// `scripts/bench_report.py` checks `sleeping` against the physics
/// benchmark's guard and hashes the line into the determinism digest.
fn format_perf_physics_line(buffers: &PhysicsBuffers) -> String {
    format!(
        "physics: proxies={} dynamic={} sleeping={} pairs={} contacts={}",
        buffers.proxy_count(),
        buffers.dynamic_count(),
        buffers.sleeping_count(),
        buffers.pair_count(),
        buffers.contact_count()
    )
}

/// `TUNGSTEN_CHECK`, set and not empty: the report file of a check run.
fn check_report_from_env() -> Option<PathBuf> {
    std::env::var_os(CHECK_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// `TUNGSTEN_SMOKE_FRAMES` as a frame count above zero: smoke mode.
fn smoke_frames_from_env() -> Option<u32> {
    std::env::var("TUNGSTEN_SMOKE_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n > 0)
}

/// A `logging.level` set in code that names no level; `Config::load`
/// rejects the same value read from the file (`D-119`).
/// `Time` with the `time` section's step, bound and interpolation; the
/// section is validated first, so the setters do not panic.
fn time_from_config(config: &TimeConfig) -> Time {
    let mut time = Time::new();
    time.set_fixed_step(1.0 / config.fixed_step_hz as f32);
    time.set_max_steps_per_frame(config.max_steps_per_frame);
    time.set_interpolate(config.interpolate);
    time
}

fn invalid_logging_level(level: &str) -> Option<ConfigError> {
    level
        .parse::<LevelFilter>()
        .is_err()
        .then(|| ConfigError::InvalidValue {
            path: String::new(),
            field: "logging.level",
            value: level.to_string(),
            expected: "one of: off, error, warn, info, debug, trace",
        })
}

/// D-008: missing `input.json` uses defaults; parse/IO errors are fatal.
/// The player's bindings at `user_layer` go over it and never fail
/// (`D-159`); without one nothing persists.
pub(crate) fn load_action_map_at_startup(
    path: &Path,
    user_layer: Option<&Path>,
) -> anyhow::Result<ActionMap> {
    let shipped = match ActionMap::load(path) {
        Ok(loaded) => {
            log::info!("Loaded action map from '{}'", path.display());
            ActionMap::merged_with_defaults(loaded)
        }
        Err(err) if err.is_not_found() => {
            log::info!(
                "Action map '{}' not found; using engine defaults",
                path.display()
            );
            ActionMap::default_map()
        }
        Err(err) => return Err(err.into()),
    };
    let mut map = shipped.with_user_layer(user_layer);
    for warning in map.take_load_warnings() {
        log::warn!(target: "tungsten_core::input", "{warning}");
    }
    Ok(map)
}

pub(crate) fn resolve_startup_display(config: &Config) -> DisplayState {
    let mut resolved = config.display.resolve(&config.window, &config.render);
    if let Some((w, h)) = parse_capture_resolution() {
        resolved.resolution.width = w;
        resolved.resolution.height = h;
    }
    match resolved.validate() {
        Ok(()) => resolved,
        Err(err) => {
            log::warn!("Resolved display settings are invalid ({err}); using engine defaults");
            DisplayState::default()
        }
    }
}

/// Parse `TUNGSTEN_CAPTURE_RESOLUTION=WxH`.
fn parse_capture_resolution() -> Option<(u32, u32)> {
    let raw = std::env::var("TUNGSTEN_CAPTURE_RESOLUTION").ok()?;
    let (w, h) = raw.split_once('x')?;
    let w: u32 = w.trim().parse().ok()?;
    let h: u32 = h.trim().parse().ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Expand AABB outline into interior edge quads.
fn expand_aabb(
    out: &mut Vec<QuadInstance>,
    min: glam::Vec2,
    max: glam::Vec2,
    color: [f32; 4],
    thickness: f32,
) {
    let t = thickness.max(0.0);
    let w = (max.x - min.x).max(0.0);
    let h = (max.y - min.y).max(0.0);
    out.push(QuadInstance {
        position: [min.x, min.y],
        size: [w, t],
        color,
    });
    out.push(QuadInstance {
        position: [min.x, (max.y - t).max(min.y)],
        size: [w, t],
        color,
    });
    out.push(QuadInstance {
        position: [min.x, min.y],
        size: [t, h],
        color,
    });
    out.push(QuadInstance {
        position: [(max.x - t).max(min.x), min.y],
        size: [t, h],
        color,
    });
}

/// Expand circle into closed polyline; degenerate inputs emit nothing.
fn expand_circle(
    out: &mut Vec<DebugLineInstance>,
    center: glam::Vec2,
    radius: f32,
    segments: u16,
    color: [f32; 4],
    thickness: f32,
) {
    if radius <= 0.0 || segments == 0 {
        return;
    }
    let two_pi = std::f32::consts::TAU;
    let step = two_pi / f32::from(segments);
    let mut prev = center + glam::Vec2::new(radius, 0.0);
    for i in 1..=segments {
        let t = f32::from(i) * step;
        let (sin, cos) = t.sin_cos();
        let next = center + glam::Vec2::new(cos * radius, sin * radius);
        out.push(DebugLineInstance {
            a: prev.to_array(),
            b: next.to_array(),
            thickness,
            _pad: 0.0,
            color,
        });
        prev = next;
    }
}

fn apply_window_fullscreen(window: &Window, mode: DisplayMode) {
    match mode {
        DisplayMode::Windowed => window.set_fullscreen(None),
        DisplayMode::BorderlessFullscreen => {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        DisplayMode::ExclusiveFullscreen => {
            unreachable!("exclusive mode is downgraded before apply")
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let requested_display = self
            .world
            .get_resource::<DisplayState>()
            .copied()
            .unwrap_or_default();
        let attrs = Window::default_attributes()
            .with_title(&self.config.window.title)
            .with_inner_size(winit::dpi::PhysicalSize::new(
                requested_display.resolution.width,
                requested_display.resolution.height,
            ));

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("Failed to create window: {e}");
                self.fatal_error = Some(anyhow::anyhow!("Failed to create window: {e}"));
                event_loop.exit();
                return;
            }
        };

        let startup_mode = runtime_display_mode(requested_display.display_mode);
        if startup_mode != requested_display.display_mode {
            log::warn!(
                "Display mode '{}' is not supported at runtime yet; downgrading to '{}'",
                requested_display.display_mode.as_str(),
                startup_mode.as_str()
            );
        }
        apply_window_fullscreen(&window, startup_mode);
        let initial_inner_size = window.inner_size();
        input_bridge::apply_scale_factor(&mut self.world, window.scale_factor());
        set_monitor(&mut self.world, read_monitor(&window));

        let mut render_config = self.config.render.clone();
        let startup_state = DisplayState {
            display_mode: startup_mode,
            ..requested_display
        };
        render_config.present_mode = startup_state.present_mode;
        render_config.max_frame_latency = startup_state.max_frame_latency;

        match Renderer::new(window.clone(), &render_config, startup_state.vsync) {
            Ok(mut renderer) => {
                // First, before any manifest font: a manifest face with its
                // ID replaces it, and every start registers faces in one
                // order (D-116, D-123).
                crate::engine_font::register(&mut renderer);
                if std::env::var("TUNGSTEN_PERF_LOG").is_ok() {
                    log::debug!(
                        "backend: {} adapter: {} present_mode: {} max_frame_latency: {} timestamp_query: {}",
                        renderer.gpu_timings.backend.as_deref().unwrap_or("unknown"),
                        renderer
                            .gpu_timings
                            .adapter_name
                            .as_deref()
                            .unwrap_or("unknown"),
                        renderer
                            .gpu_timings
                            .present_mode
                            .as_deref()
                            .unwrap_or("unknown"),
                        renderer.gpu_timings.max_frame_latency.unwrap_or(0),
                        renderer.timestamp_support
                    );
                }
                if let Some(gpu) = self.world.get_resource_mut::<GpuFrameTimings>() {
                    *gpu = renderer.gpu_timings.clone();
                }
                sync_display_state_and_telemetry(
                    &mut self.world,
                    startup_state,
                    renderer.gpu_timings.present_mode.clone(),
                );
                sync_window_resolution(
                    &mut self.world,
                    initial_inner_size.width,
                    initial_inner_size.height,
                    renderer.gpu_timings.present_mode.clone(),
                );
                self.renderer = Some(renderer);
            }
            Err(e) => {
                log::error!("Failed to initialize renderer: {e}");
                self.fatal_error = Some(anyhow::anyhow!("Failed to initialize renderer: {e}"));
                event_loop.exit();
                return;
            }
        }

        self.window = Some(window);

        // D-052: merge manifests before user startup; duplicate IDs halt boot.
        if !self.manifest_roots.is_empty()
            && let Some(renderer) = &mut self.renderer
            && let Err(e) =
                asset_loader::load_all_merged(&self.manifest_roots, &mut self.world, renderer)
        {
            log::error!("Manifest composition failed: {e}");
            self.fatal_error = Some(e.context("Manifest composition failed"));
            event_loop.exit();
            return;
        }

        if let Some(startup) = self.startup.take()
            && let Some(renderer) = &mut self.renderer
        {
            startup(&mut self.world, renderer);
        }

        // Startup time excluded from first-frame dt; prevents first-substep tunneling.
        self.last_frame = Some(Instant::now());

        // Audio init after startup: SoundRegistry populated.
        if let Some(sound_registry) = self.world.get_resource::<SoundRegistry>() {
            match AudioSystem::init(sound_registry) {
                Ok(sys) => {
                    self.audio = Some(sys);
                }
                Err(e) => {
                    log::warn!("Audio init failed (continuing without audio): {e}");
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            // A redraw or a second close after the last frame runs nothing.
            WindowEvent::CloseRequested | WindowEvent::RedrawRequested if event_loop.exiting() => {}
            WindowEvent::CloseRequested => self.close_with_last_frame(event_loop),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
                let actual_present_mode = self
                    .renderer
                    .as_ref()
                    .and_then(|renderer| renderer.gpu_timings.present_mode.clone());
                sync_window_resolution(
                    &mut self.world,
                    size.width,
                    size.height,
                    actual_present_mode,
                );
                // A Wayland surface learns its output late (`D-184`).
                if !self.world.has_resource::<tungsten_core::MonitorInfo>() {
                    self.refresh_monitor();
                }
            }
            WindowEvent::Moved(_) => self.refresh_monitor(),
            // Smoke runs drop the window's input, so a resting pointer cannot
            // hover a pixel fixture; focus, resize and scale still apply
            // (`D-149`).
            WindowEvent::KeyboardInput { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::MouseWheel { .. }
                if self.smoke_frames_remaining.is_some() => {}
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => input_bridge::apply_key(
                &mut self.world,
                event.physical_key,
                event.state,
                event.repeat,
                is_synthetic,
            ),
            WindowEvent::Focused(focused) => input_bridge::apply_focus(&mut self.world, focused),
            WindowEvent::ModifiersChanged(modifiers) => {
                input_bridge::apply_modifiers(&mut self.world, modifiers.state());
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let btn = input_bridge::translate_mouse_button(button);
                if let Some(input) = self.world.get_resource_mut::<InputState>() {
                    match state {
                        ElementState::Pressed => input.mouse_down(btn),
                        ElementState::Released => input.mouse_up(btn),
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(input) = self.world.get_resource_mut::<InputState>() {
                    input.update_cursor_position(position.x as f32, position.y as f32);
                }
            }
            WindowEvent::CursorLeft { .. } => input_bridge::apply_pointer_left(&mut self.world),
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(input) = self.world.get_resource_mut::<InputState>() {
                    match delta {
                        MouseScrollDelta::LineDelta(x, y) => input.add_scroll_line_delta(x, y),
                        MouseScrollDelta::PixelDelta(delta) => {
                            input.add_scroll_pixel_delta(delta.x as f32, delta.y as f32);
                        }
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                input_bridge::apply_scale_factor(&mut self.world, scale_factor);
                self.refresh_monitor();
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                match self.run_frame(frame_start, FrameClock::Wall, |_| {}) {
                    FrameEnd::Completed => {}
                    // A failed render ends the run at once; `App::run`
                    // returns its error. The last frame has presented.
                    FrameEnd::ExitRequested | FrameEnd::RenderFailed | FrameEnd::Last => {
                        event_loop.exit();
                        return;
                    }
                }

                self.stage_pacing(event_loop, frame_start);
                self.stage_smoke_exit(event_loop);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(deadline) = self.redraw_deadline else {
            return;
        };
        // Woken early by another event: `WaitUntil` from `stage_pacing` still stands.
        if Instant::now() < deadline {
            return;
        }
        self.redraw_deadline = None;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        // The pending redraw wakes the loop; the next frame sets its own pacing.
        event_loop.set_control_flow(ControlFlow::Wait);
    }
}

#[cfg(test)]
#[path = "tests/app.rs"]
mod tests;
