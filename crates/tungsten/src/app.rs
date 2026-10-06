use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::asset_loader;
use crate::audio::AudioSystem;
use crate::crash::{self, CrashContext};
use crate::debug_hud::{DebugHud, HudActiveState, compose_hud_text_sections};
use crate::display::{
    DisplayDelta, PendingDisplay, frame_budget_for, sync_display_state_and_telemetry,
    sync_window_resolution, take_pending_display,
};
use crate::hot_reload::HotReloadWatcher;
use crate::input_bridge;
use crate::inspector::{InspectorState, compose_inspector_text_section, default_inspect_registry};
use crate::logging;
use crate::physics_debug::{PhysicsDebugOverlay, physics_debug_emit_system};
use crate::plugins::DefaultPlugins;
use crate::post_aa::{PendingPostAa, PostAaState, sync_post_aa_state, take_pending_post_aa};
use crate::sprite_extract::ExtractScratch;
use crate::state::StateStack;
use crate::systems_overlay::{SystemTimingOverlay, compose_systems_overlay_text_section};
use crate::telemetry::{DisplayTelemetry, FrameTimings, RenderCounts};
use crate::user_dir::{self, Platform};
use log::LevelFilter;
use tungsten_core::assets::{
    AnimationRegistry, FontRegistry, ParticleConfigRegistry, ParticleMeshRegistry, ShaderRegistry,
    SoundRegistry, TilemapRegistry,
};
use tungsten_core::physics::{PhysicsBuffers, PhysicsConfig};
use tungsten_core::post::{PostPass, PostStack};
// `App` writes the deprecated `DeltaTime` each frame until W4b removes it.
#[allow(deprecated)]
use tungsten_core::DeltaTime;
use tungsten_core::{
    ActionMap, AssetRegistry, AudioCommand, AudioCommands, CameraController, CameraState,
    CommandBuffer, Config, ConfigError, DebugDraw, DebugShape, DisplayMode, DisplayState,
    InputState, InspectRegistry, Inspectable, ParticleActive, ParticleBudget, Plugin, PluginSet,
    Schedule, Stage, SystemDesc, Time, World, WorldRngSeed, system,
};
use tungsten_render::{
    DebugLineInstance, GpuFrameTimings, MeshParticleBatch, QuadInstance, Renderer, SpriteBatch,
    TextSection,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowId};

/// Fixed per-frame dt (seconds) used under `TUNGSTEN_SMOKE_FRAMES`. Pinned to
/// 60 Hz so smoke-mode captures and visual regressions are frame-deterministic
/// across runs regardless of build profile or host load.
const SMOKE_MODE_FIXED_DT_SECS: f32 = 1.0 / 60.0;

/// Longest per-frame dt (seconds) a frame hands to the simulation (`D-088`).
/// A stall (debugger pause, window drag, a blocked acquire) would otherwise
/// reach `physics_step` whole: one 0.2 s step throws a settled awake pile
/// about, and a 2 s step drops bodies through the floor. One 0.1 s step
/// leaves such a pile at about 40 px/s.
const MAX_DT_SECS: f32 = 0.1;

pub use tungsten_core::schedule::SystemFn;

/// World-to-quad extract.
pub type ExtractQuadsFn = Box<dyn Fn(&World) -> Vec<QuadInstance>>;

/// World-to-sprite extract.
pub type ExtractSpritesFn = Box<dyn Fn(&World) -> Vec<SpriteBatch>>;

/// World-to-text extract.
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
    extract_quads: Option<ExtractQuadsFn>,
    extract_sprites: Option<ExtractSpritesFn>,
    extract_text: Option<ExtractTextFn>,
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
    capture_config: Option<CaptureConfig>,
    /// With no audio device, the audio stage appends a frame's commands here
    /// when set, instead of dropping them: the commands that frame would have
    /// played. Only a headless caller sets it.
    pub(crate) audio_capture: Option<Vec<AudioCommand>>,
    frames_rendered: u64,
    fatal_error: Option<anyhow::Error>,
    // M31: the user's post stack plus the transition pass, rebuilt on each
    // frame a screen transition draws; kept to reuse its allocation.
    transition_post_stack: PostStack,
}

#[derive(Debug, Clone)]
struct CaptureConfig {
    target_frame: u64,
    path: PathBuf,
    /// `TUNGSTEN_CAPTURE_DIRECT=1`: capture what the direct present path
    /// draws, not the blit path's source (`D-087`).
    direct: bool,
    captured: bool,
}

impl App {
    /// Builds the app with [`DefaultPlugins`]. First it installs the engine
    /// logger, unless the game set one, with a log file in the game's user
    /// folder (`D-119`), then logs [`Config::take_load_warnings`]. An error
    /// it returns is logged first, so it reaches the log file too.
    ///
    /// # Errors
    ///
    /// An invalid `game.id` or `logging.level`, or an `input.json` that
    /// cannot be read or parsed.
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
        let invalid = config
            .game
            .validate()
            .err()
            .or_else(|| invalid_logging_level(&config.logging.level));
        let user_dirs = if invalid.is_none() {
            user_dir::resolve(
                &config.game,
                smoke_frames.is_some(),
                Platform::current(),
                |name| std::env::var_os(name),
            )
        } else {
            None
        };
        let level = config.logging.level.parse().unwrap_or(LevelFilter::Info);
        let log_file = logging::install(level, user_dirs.as_ref().map(|dirs| dirs.logs.as_path()));
        if let Some(err) = invalid {
            log::error!("{err}");
            return Err(err.into());
        }
        for warning in config.take_load_warnings() {
            log::warn!(target: "tungsten_core::config", "{warning}");
        }
        if user_dirs.is_none()
            && smoke_frames.is_none()
            && let Some(id) = &config.game.id
        {
            log::warn!("Game '{id}' has no user folder, so no log or crash file is written");
        }
        if let Some(dirs) = &user_dirs {
            crash::install(CrashContext::new(&config.game, dirs.logs.clone(), log_file));
        }

        let resolved_display = resolve_startup_display(&config);
        let input_map_path = PathBuf::from("input.json");
        let action_map = load_action_map_at_startup(&input_map_path)
            .inspect_err(|err| log::error!("{err:#}"))?;
        let mut world = World::new();
        world.insert_resource(Time::new());
        #[allow(deprecated)]
        world.insert_resource(DeltaTime::new());
        world.insert_resource(InputState::new());
        world.insert_resource(action_map);
        world.insert_resource(WindowSize {
            width: resolved_display.resolution.width,
            height: resolved_display.resolution.height,
        });
        world.insert_resource(AssetRegistry::new());
        world.insert_resource(SoundRegistry::new());
        world.insert_resource(AudioCommands::new());
        world.insert_resource(TilemapRegistry::new());
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
        world.insert_resource(DebugDraw::new());
        world.insert_resource(PhysicsDebugOverlay::default());
        world.insert_resource(SystemTimingOverlay::default());
        world.insert_resource(InspectorState::new());
        world.insert_resource(default_inspect_registry());
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
            extract_quads: None,
            extract_sprites: None,
            extract_text: None,
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
            capture_config: parse_capture_config(),
            audio_capture: None,
            frames_rendered: 0,
            fatal_error: None,
            transition_post_stack: PostStack::new(),
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
            .map_err(|err| anyhow::anyhow!("Schedule: {err}"))
    }

    /// Enable or disable engine exit action handling.
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

    /// Set quad extract function.
    pub fn set_extract_quads(&mut self, f: impl Fn(&World) -> Vec<QuadInstance> + 'static) {
        self.extract_quads = Some(Box::new(f));
    }

    /// Set sprite extract function.
    pub fn set_extract_sprites(&mut self, f: impl Fn(&World) -> Vec<SpriteBatch> + 'static) {
        self.extract_sprites = Some(Box::new(f));
    }

    /// Set text extract function.
    pub fn set_extract_text(&mut self, f: impl Fn(&World) -> Vec<TextSection> + 'static) {
        self.extract_text = Some(Box::new(f));
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
    /// The event loop's, logged first, or the fatal error that ended the
    /// run, logged where it arose.
    pub fn run(mut self) -> anyhow::Result<()> {
        self.resolve_schedule()
            .inspect_err(|err| log::error!("{err}"))?;
        self.install_default_extracts();
        self.start_hot_reload();
        let event_loop = EventLoop::new()
            .inspect_err(|err| log::error!("Failed to create the event loop: {err}"))?;
        event_loop
            .run_app(&mut self)
            .inspect_err(|err| log::error!("Event loop failed: {err}"))?;
        if let Some(error) = self.fatal_error {
            return Err(error);
        }
        Ok(())
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

    /// Install default extracts; idempotent.
    pub(crate) fn install_default_extracts(&mut self) {
        if self.extract_sprites.is_none() {
            self.extract_sprites = Some(Box::new(crate::sprite_extract::extract_sprites_default));
        }
    }

    fn engine_exit_requested(&self) -> bool {
        if !self.exit_on_escape {
            return false;
        }

        let Some(input) = self.world.get_resource::<InputState>() else {
            return false;
        };
        let Some(actions) = self.world.get_resource::<ActionMap>() else {
            return false;
        };
        actions.just_pressed(input, "engine_exit")
    }

    /// Poll hot reload; apply before extract/render.
    fn process_hot_reload(&mut self) {
        let ready = match self.hot_reload.as_mut() {
            Some(w) => w.drain_ready(),
            None => return,
        };
        if ready.is_empty() {
            return;
        }

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        let reload_roots =
            manifest_reload_roots(&self.manifest_roots, self.manifest_path.as_deref());
        let mut manifest_reloaded = false;

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
                "json" => {
                    let anim_id = self
                        .world
                        .get_resource::<AnimationRegistry>()
                        .and_then(|ar| ar.id_for_path(&canon).map(ToString::to_string));
                    let particle_id = self
                        .world
                        .get_resource::<ParticleConfigRegistry>()
                        .and_then(|pr| {
                            pr.id_for_path(&canon)
                                .and_then(|aid| pr.name_for_id(aid).map(ToString::to_string))
                        });
                    if let Some(id) = anim_id {
                        if let Err(e) = asset_loader::reload_animation(&id, &canon, &mut self.world)
                        {
                            log::error!("Animation reload '{id}': {e}");
                        }
                    } else if let Some(id) = particle_id {
                        if let Err(e) = asset_loader::reload_particle(&id, &canon, &mut self.world)
                        {
                            log::error!("Particle reload '{id}': {e}");
                        }
                    } else {
                        log::debug!(
                            "Hot reload: no animation or particle registered for '{}'",
                            canon.display()
                        );
                    }
                }
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

        let Some(window) = self.window.as_ref() else {
            sync_display_state_and_telemetry(&mut self.world, current, None);
            return;
        };

        let mut actual_present_mode = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.gpu_timings.present_mode.clone());

        if delta.display_mode_changed {
            let runtime_mode = runtime_display_mode(requested.display_mode);
            if runtime_mode != requested.display_mode {
                log::warn!(
                    "Display mode '{}' is not supported at runtime yet; downgrading to '{}'",
                    requested.display_mode.as_str(),
                    runtime_mode.as_str()
                );
            }
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

    /// M27: apply any post-AA mode change requested via `request_post_aa`.
    /// Called between hot-reload and extract so SMAA reallocation never
    /// happens mid-frame.
    fn apply_pending_post_aa_request(&mut self) {
        let Some(requested) = take_pending_post_aa(&mut self.world) else {
            return;
        };
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_post_aa(requested);
        }
        sync_post_aa_state(&mut self.world, requested);
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

/// How a frame ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameEnd {
    /// Every stage ran.
    Completed,
    /// `engine_exit` was just pressed; the stages after update did not run.
    ExitRequested,
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
    extract_ms: f32,
}

#[allow(clippy::struct_field_names)]
#[derive(Default)]
struct FrameRenderOut {
    render_ms: f32,
    render_acquire_ms: f32,
    render_encode_ms: f32,
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
    render_submit_present_ms: f32,
    audio_ms: f32,
    total_ms: f32,
    interval_ms: Option<f32>,
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

    /// `Startup` once, then `PreUpdate`, `FixedUpdate` and `Update`; `None`
    /// when the frame's update asked the engine to exit, else the time so
    /// far and the per-system timings `stage_post_update` extends.
    #[inline(always)]
    fn stage_update(&mut self) -> (Instant, Vec<(String, f32)>, bool) {
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
        self.run_stage(Stage::FixedUpdate, &mut system_timings);
        self.run_stage(Stage::Update, &mut system_timings);
        let exit = self.engine_exit_requested();
        (update_start, system_timings, exit)
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

    #[inline(always)]
    fn stage_extract(&mut self, prev_total_ms: f32) -> FrameExtract {
        let extract_start = Instant::now();
        // The sprite extract culls at the size render projects with
        // (`D-114`); a pending resize leaves `WindowSize` ahead of the surface.
        if let (Some(renderer), Some(scratch)) =
            (&self.renderer, self.world.get_resource::<ExtractScratch>())
        {
            let cfg = &renderer.surface_config;
            scratch.set_viewport(cfg.width, cfg.height);
        }
        let quads = self
            .extract_quads
            .as_ref()
            .map(|f| f(&self.world))
            .unwrap_or_default();

        let sprites = self
            .extract_sprites
            .as_ref()
            .map(|f| f(&self.world))
            .unwrap_or_default();

        let mut text = self
            .extract_text
            .as_ref()
            .map(|f| f(&self.world))
            .unwrap_or_default();

        // Before HUD compose: counts row uses this frame's extract.
        let entity_count = self.world.entity_count();
        let sprite_instance_count: u32 = sprites.iter().map(|b| b.instances.len() as u32).sum();
        if let Some(rc) = self.world.get_resource_mut::<RenderCounts>() {
            rc.entities = entity_count;
            rc.sprite_instances = sprite_instance_count;
        }

        // Extract-start emit: debug draw visible same frame.
        physics_debug_emit_system(&mut self.world);

        // AABBs -> quads; angled lines/circles -> debug-line pipeline.
        let (debug_quads, debug_lines) = drain_debug_draw(&mut self.world);

        // Borrow split: remove UI state, compose with `&World`, reinsert.
        let viewport = self
            .world
            .get_resource::<WindowSize>()
            .map_or((0, 0), |w| (w.width, w.height));
        if let Some(mut hud) = self.world.remove_resource::<DebugHud>() {
            let hud_sections =
                compose_hud_text_sections(&mut hud, &self.world, viewport, prev_total_ms);
            self.world.insert_resource(hud);
            text.extend(hud_sections);
        }
        if let Some(mut overlay) = self.world.remove_resource::<SystemTimingOverlay>() {
            let sections = compose_systems_overlay_text_section(
                &mut overlay,
                &self.world,
                viewport,
                prev_total_ms,
            );
            self.world.insert_resource(overlay);
            text.extend(sections);
        }
        if let Some(mut state) = self.world.remove_resource::<InspectorState>() {
            let sections =
                compose_inspector_text_section(&mut state, &self.world, viewport, prev_total_ms);
            self.world.insert_resource(state);
            text.extend(sections);
        }

        // M29: pull lights into a UBO every frame regardless of presence so
        // the lit pipeline (when active) reads coherent ambient + count = 0.
        let camera = self
            .world
            .get_resource::<CameraState>()
            .copied()
            .unwrap_or_default();
        let (vw, vh) = (viewport.0 as f32, viewport.1 as f32);
        let light_ubo = crate::light_extract::extract_lights(&self.world, &camera, vw, vh);

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
            extract_ms,
        }
    }

    #[inline(always)]
    fn stage_render(&mut self, extract: &FrameExtract) -> FrameRenderOut {
        let render_start = Instant::now();
        let mut out = FrameRenderOut::default();
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

            // Arm before target render; mark captured after successful submit.
            if let Some(cfg) = self.capture_config.as_mut()
                && !cfg.captured
                && self.frames_rendered + 1 == cfg.target_frame
                && let Err(e) = if cfg.direct {
                    renderer.capture_frame_direct(&cfg.path)
                } else {
                    renderer.capture_frame(&cfg.path)
                }
            {
                log::warn!("capture_frame({}) failed to arm: {e}", cfg.path.display());
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

            let result = if self.gpu_timing_enabled {
                renderer.render_frame_full_timed(
                    &view_proj,
                    &extract.quads,
                    &extract.sprites,
                    &extract.debug_quads,
                    &extract.debug_lines,
                    &extract.text,
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
                    post_stack,
                )
            };
            if let Err(e) = result {
                log::error!("Render error: {e}");
            } else {
                self.frames_rendered = self.frames_rendered.saturating_add(1);
                if let Some(cfg) = self.capture_config.as_mut()
                    && !cfg.captured
                    && self.frames_rendered == cfg.target_frame
                {
                    cfg.captured = true;
                    log::info!(
                        "captured frame {} -> {}",
                        cfg.target_frame,
                        cfg.path.display()
                    );
                }
            }
            out.render_acquire_ms = renderer.cpu_timings.acquire_ms;
            out.render_encode_ms = renderer.cpu_timings.encode_ms;
            out.render_submit_present_ms = renderer.cpu_timings.submit_present_ms;
            out.gpu_frame_ms = renderer.gpu_timings.frame_gpu_ms;
            if let Some(gpu) = self.world.get_resource_mut::<GpuFrameTimings>() {
                *gpu = renderer.gpu_timings.clone();
            }
        }
        out.render_ms = render_start.elapsed().as_secs_f64() as f32 * 1000.0;
        out
    }

    /// Hands the drawn frame's sprite batches back to the extract scratch:
    /// the next frame's batches reuse their instance vectors.
    #[inline(always)]
    fn stage_recycle(&mut self, extract: &mut FrameExtract) {
        if let Some(scratch) = self.world.get_resource::<ExtractScratch>() {
            scratch.recycle(std::mem::take(&mut extract.sprites));
        }
    }

    #[inline(always)]
    fn stage_audio(&mut self) -> f32 {
        let audio_start = Instant::now();
        if let Some(cmds) = self.world.get_resource_mut::<AudioCommands>() {
            for cmd in cmds.drain() {
                if let Some(audio) = &mut self.audio {
                    audio.send(cmd);
                } else if let Some(capture) = &mut self.audio_capture {
                    capture.push(cmd);
                }
            }
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
            ft.render_submit_present_ms = t.render_submit_present_ms;
            ft.audio_ms = t.audio_ms;
            ft.hot_reload_ms = t.hot_reload_ms;
            ft.flush_ms = t.flush_ms;
            ft.total_ms = t.total_ms;
            ft.interval_ms = t.interval_ms;
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

        // HUD smoothing uses previous frame; compose still occurs before render.
        let prev_total_ms = self
            .world
            .get_resource::<FrameTimings>()
            .map_or(0.0, |ft| ft.total_ms);

        self.stage_time(clock);
        let (update_start, mut system_timings, exit) = self.stage_update();
        if exit {
            return FrameEnd::ExitRequested;
        }
        self.stage_post_update(&mut system_timings);
        let update_ms = update_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let flush_ms = self.stage_flush_commands();
        self.stage_flush_events();
        let hot_reload_ms = self.stage_hot_reload();
        self.apply_pending_post_aa_request();

        let mut extract_out = self.stage_extract(prev_total_ms);
        let extract_ms = extract_out.extract_ms;

        let render_out = self.stage_render(&extract_out);
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
            render_submit_present_ms: render_out.render_submit_present_ms,
            audio_ms,
            total_ms,
            interval_ms,
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

        FrameEnd::Completed
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
        "frame: total={:.2}ms interval={} update={:.2}ms flush={:.2}ms extract={:.2}ms render={:.2}ms render_acquire={:.2}ms render_encode={:.2}ms render_submit_present={:.2}ms gpu={} audio={:.2}ms hot_reload={:.2}ms",
        total_ms,
        interval_for_log,
        update_ms,
        flush_ms,
        extract_ms,
        render_out.render_ms,
        render_out.render_acquire_ms,
        render_out.render_encode_ms,
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

/// `TUNGSTEN_SMOKE_FRAMES` as a frame count above zero: smoke mode.
fn smoke_frames_from_env() -> Option<u32> {
    std::env::var("TUNGSTEN_SMOKE_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n > 0)
}

/// A `logging.level` set in code that names no level; `Config::load`
/// rejects the same value read from the file (`D-119`).
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
fn load_action_map_at_startup(path: &Path) -> anyhow::Result<ActionMap> {
    match ActionMap::load(path) {
        Ok(loaded) => {
            log::info!("Loaded action map from '{}'", path.display());
            Ok(ActionMap::merged_with_defaults(loaded))
        }
        Err(err) if err.is_not_found() => {
            log::info!(
                "Action map '{}' not found; using engine defaults",
                path.display()
            );
            let mut map = ActionMap::default_map();
            map.set_source_path(path);
            Ok(map)
        }
        Err(err) => Err(err.into()),
    }
}

fn resolve_startup_display(config: &Config) -> DisplayState {
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

/// Parse one-shot capture env vars.
fn parse_capture_config() -> Option<CaptureConfig> {
    let target_frame: u64 = std::env::var("TUNGSTEN_CAPTURE_FRAME").ok()?.parse().ok()?;
    if target_frame == 0 {
        return None;
    }
    let path = std::env::var("TUNGSTEN_CAPTURE_PATH")
        .map_or_else(|_| PathBuf::from("actual.png"), PathBuf::from);
    let direct = std::env::var("TUNGSTEN_CAPTURE_DIRECT").is_ok_and(|value| value == "1");
    Some(CaptureConfig {
        target_frame,
        path,
        direct,
        captured: false,
    })
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

fn runtime_display_mode(requested: DisplayMode) -> DisplayMode {
    match requested {
        DisplayMode::ExclusiveFullscreen => DisplayMode::BorderlessFullscreen,
        other => other,
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
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
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
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let key = input_bridge::translate_key(event.physical_key);
                if let Some(input) = self.world.get_resource_mut::<InputState>() {
                    match event.state {
                        ElementState::Pressed => input.key_down(key),
                        ElementState::Released => input.key_up(key),
                    }
                }
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
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                if self.run_frame(frame_start, FrameClock::Wall, |_| {}) == FrameEnd::ExitRequested
                {
                    event_loop.exit();
                    return;
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
