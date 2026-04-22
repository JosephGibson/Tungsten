use std::any::TypeId;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::asset_loader;
use crate::audio::AudioSystem;
use crate::debug_hud::{compose_hud_text_sections, hud_toggle_system, DebugHud, HudActiveState};
use crate::display::{
    engine_display_input_system, frame_budget_for, sync_display_state_and_telemetry,
    sync_window_resolution, take_pending_display, DisplayDelta, PendingDisplay,
};
use crate::hot_reload::HotReloadWatcher;
use crate::input_bridge;
use crate::inspector::{
    compose_inspector_text_section, inspector_pick_system, inspector_toggle_system, InspectorState,
};
use crate::physics_debug::{
    physics_debug_emit_system, physics_debug_toggle_system, PhysicsDebugOverlay,
};
use crate::state::{state_dispatcher_system, StateStack};
use crate::systems_overlay::{
    compose_systems_overlay_text_section, systems_overlay_toggle_system, SystemTimingOverlay,
};
use crate::telemetry::{DisplayTelemetry, FrameTimings, RenderCounts};
use tungsten_core::assets::{
    AnimationRegistry, FontRegistry, ParticleConfigRegistry, SoundRegistry, TilemapRegistry,
};
use tungsten_core::physics::{CollisionEvent, PhysicsConfig};
use tungsten_core::{
    ActionMap, AssetRegistry, AudioCommands, CameraController, CameraState, CommandBuffer, Config,
    DebugDraw, DebugShape, DeltaTime, DisplayMode, DisplayState, EventQueue, InputState,
    Inspectable, ParticleActive, ParticleBudget, World, WorldRngSeed,
};
use tungsten_render::{
    DebugLineInstance, GpuFrameTimings, QuadInstance, Renderer, SpriteBatch, TextSection,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowId};

/// A system function that runs each tick.
pub type SystemFn = Box<dyn FnMut(&mut World)>;

/// A render-extraction function: reads the World and produces QuadInstances.
pub type ExtractQuadsFn = Box<dyn Fn(&World) -> Vec<QuadInstance>>;

/// A render-extraction function: reads the World and produces SpriteBatches.
pub type ExtractSpritesFn = Box<dyn Fn(&World) -> Vec<SpriteBatch>>;

/// A render-extraction function: reads the World and produces TextSections.
pub type ExtractTextFn = Box<dyn Fn(&World) -> Vec<TextSection>>;

/// Resource: current window dimensions in physical pixels.
#[derive(Debug, Clone, Copy)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

/// Called once after the renderer is initialized, for asset loading.
pub type StartupFn = Box<dyn FnOnce(&mut World, &mut Renderer)>;

type EventFlusher = Box<dyn FnMut(&mut World)>;

/// Top-level application that drives the winit event loop, ECS, and renderer.
pub struct App {
    config: Config,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    world: World,
    // Systems run in registration order. No priority or grouping mechanism.
    // If ordering becomes painful by M13, revisit with named priorities.
    systems: Vec<SystemFn>,
    extract_quads: Option<ExtractQuadsFn>,
    extract_sprites: Option<ExtractSpritesFn>,
    extract_text: Option<ExtractTextFn>,
    startup: Option<StartupFn>,
    last_frame: Option<Instant>,
    /// If true (the default), the engine's exit action exits the process.
    /// The default binding is `Escape`, but `input.json` may rebind it. Set
    /// this to false when game code needs to keep the engine-side exit action
    /// disabled (e.g. a pause menu that owns Escape itself).
    exit_on_escape: bool,
    /// Audio subsystem — initialized after the startup callback runs.
    /// Kept alive here so the cpal stream doesn't drop.
    audio: Option<AudioSystem>,
    /// Hot-reload file watcher. None when hot reload is not enabled.
    hot_reload: Option<HotReloadWatcher>,
    /// Path to the primary manifest file, used to detect manifest changes.
    manifest_path: Option<PathBuf>,
    /// Ordered list of manifest JSON paths the engine merges and loads at
    /// startup (D-052). Populated by `set_manifest_roots`; when empty,
    /// asset composition is left entirely to the user's `on_startup` hook.
    manifest_roots: Vec<PathBuf>,
    /// Workspace-root `input.json` path. Present from `App::new`; wired
    /// into the hot-reload watcher when `enable_hot_reload` is called.
    input_map_path: PathBuf,
    /// Smoke-test mode: when `Some(n)`, render `n` frames then exit cleanly.
    /// Populated from the `TUNGSTEN_SMOKE_FRAMES` env var. Used by
    /// `scripts/smoke-examples.sh` to drive examples in CI-like checks.
    smoke_frames_remaining: Option<u32>,
    /// Names for registered systems, parallel to `systems`.
    system_names: Vec<String>,
    /// Auto-incrementing counter for unnamed system registration.
    system_name_counter: usize,
    /// When true, use render_frame_full_timed each frame (adds device.poll stall).
    /// Set via TUNGSTEN_GPU_TIMING env var. Never enable in production.
    gpu_timing_enabled: bool,
    /// Type-erased flush closures for registered event queues.
    /// Populated by `register_event::<T>()` and run once per frame after the
    /// command-buffer flush stage.
    event_flushers: Vec<EventFlusher>,
    /// Event types that already have queue resources registered.
    registered_event_types: HashSet<TypeId>,
    /// Optional frame pacing budget derived from `DisplayState.frame_rate_cap`.
    frame_budget: Option<Duration>,
    /// Screenshot capture wiring driven by `TUNGSTEN_CAPTURE_FRAME` /
    /// `TUNGSTEN_CAPTURE_PATH`. `None` unless the env var is set.
    capture_config: Option<CaptureConfig>,
    /// Monotonic rendered-frame counter; only incremented after a successful
    /// `render_frame_full[_timed]` call so capture targets an actually-drawn
    /// frame rather than an early-exit frame.
    frames_rendered: u64,
}

/// Resolved `TUNGSTEN_CAPTURE_FRAME` / `TUNGSTEN_CAPTURE_PATH` config. One-shot
/// screenshot hook: once `captured` flips true the hook never fires again.
#[derive(Debug, Clone)]
struct CaptureConfig {
    target_frame: u64,
    path: PathBuf,
    captured: bool,
}

impl App {
    fn register_event_inner<T: 'static>(
        world: &mut World,
        event_flushers: &mut Vec<EventFlusher>,
        registered_event_types: &mut HashSet<TypeId>,
    ) {
        if !registered_event_types.insert(TypeId::of::<T>()) {
            return;
        }

        world.insert_resource(EventQueue::<T>::new());
        event_flushers.push(Box::new(|world: &mut World| {
            if let Some(queue) = world.get_resource_mut::<EventQueue<T>>() {
                queue.flush();
            }
        }));
    }

    pub fn new(config: Config) -> anyhow::Result<Self> {
        let resolved_display = resolve_startup_display(&config);
        let input_map_path = PathBuf::from("input.json");
        let action_map = load_action_map_at_startup(&input_map_path)?;
        let mut world = World::new();
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
        world.insert_resource(DebugDraw::new());
        world.insert_resource(PhysicsDebugOverlay::default());
        world.insert_resource(SystemTimingOverlay::default());
        world.insert_resource(InspectorState::new_with_defaults());
        let mut event_flushers: Vec<EventFlusher> = Vec::new();
        let mut registered_event_types = HashSet::new();
        Self::register_event_inner::<CollisionEvent>(
            &mut world,
            &mut event_flushers,
            &mut registered_event_types,
        );
        Self::register_event_inner::<crate::particles::ParticleBurstEmitted>(
            &mut world,
            &mut event_flushers,
            &mut registered_event_types,
        );
        Self::register_event_inner::<crate::particles::ParticleSystemDrained>(
            &mut world,
            &mut event_flushers,
            &mut registered_event_types,
        );

        let mut app = Self {
            config,
            window: None,
            renderer: None,
            world,
            systems: Vec::new(),
            extract_quads: None,
            extract_sprites: None,
            extract_text: None,
            startup: None,
            last_frame: None,
            exit_on_escape: true,
            audio: None,
            hot_reload: None,
            manifest_path: None,
            manifest_roots: Vec::new(),
            input_map_path,
            smoke_frames_remaining: std::env::var("TUNGSTEN_SMOKE_FRAMES")
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|n| *n > 0),
            system_names: Vec::new(),
            system_name_counter: 0,
            gpu_timing_enabled: std::env::var("TUNGSTEN_GPU_TIMING").is_ok(),
            event_flushers,
            registered_event_types,
            frame_budget: frame_budget_for(resolved_display.frame_rate_cap),
            capture_config: parse_capture_config(),
            frames_rendered: 0,
        };

        // Register engine-owned action consumers before user systems so they
        // observe edge-triggered input deterministically every frame. Debug
        // overlays toggle first so a single input frame carrying F1/F2/F3
        // alongside F4 resolves all four edges consistently.
        app.add_engine_system("__physics_debug_toggle", physics_debug_toggle_system);
        app.add_engine_system("__systems_overlay_toggle", systems_overlay_toggle_system);
        app.add_engine_system("__inspector_toggle", inspector_toggle_system);
        app.add_engine_system("__inspector_pick", inspector_pick_system);
        app.add_engine_system("__hud_toggle", hud_toggle_system);
        app.add_engine_system("__display_input", engine_display_input_system);
        app.add_engine_system("__state_dispatcher", state_dispatcher_system);

        Ok(app)
    }

    /// Register a component type with the inspector so it contributes rows
    /// when the entity carrying it is selected. The `label` appears in the
    /// overlay as a heading above the component's rows.
    pub fn register_inspectable<T: 'static + Inspectable>(&mut self, label: &'static str) {
        if let Some(state) = self.world.get_resource_mut::<InspectorState>() {
            state.register::<T>(label);
        }
    }

    /// Register an engine-owned system at the current end of the system list
    /// without touching the auto-naming counter. Used for systems that should
    /// run regardless of user setup (e.g. the HUD toggle).
    fn add_engine_system(&mut self, name: &str, system: impl FnMut(&mut World) + 'static) {
        self.system_names.push(name.to_string());
        self.systems.push(Box::new(system));
    }

    /// Control whether the engine exit action exits the process (default: true).
    /// The default binding is `Escape`; rebinding still routes through this
    /// gate. Disable when game code wants to own the mapped input itself.
    pub fn set_exit_on_escape(&mut self, exit: bool) {
        self.exit_on_escape = exit;
    }

    /// Access the World for setup (spawning entities, inserting resources).
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Access the renderer (available after the window is created).
    pub fn renderer_mut(&mut self) -> Option<&mut Renderer> {
        self.renderer.as_mut()
    }

    /// Register a system that runs each tick.
    pub fn add_system(&mut self, system: impl FnMut(&mut World) + 'static) {
        let name = format!("system_{}", self.system_name_counter);
        self.system_name_counter += 1;
        self.system_names.push(name);
        self.systems.push(Box::new(system));
    }

    /// Register a named system. The name appears in FrameTimings::system_timings
    /// for per-system profiling. Prefer this when the system name matters for
    /// diagnostics output.
    pub fn add_system_named(
        &mut self,
        name: impl Into<String>,
        system: impl FnMut(&mut World) + 'static,
    ) {
        self.system_names.push(name.into());
        self.systems.push(Box::new(system));
    }

    /// Register an event type with the engine.
    ///
    /// Inserts an `EventQueue<T>` resource and schedules its flush once per
    /// frame after systems complete. Call during startup before `run()`.
    /// Duplicate registration of the same event type is ignored.
    pub fn register_event<T: 'static>(&mut self) {
        Self::register_event_inner::<T>(
            &mut self.world,
            &mut self.event_flushers,
            &mut self.registered_event_types,
        );
    }

    /// Set the function that extracts quad render data from the World.
    pub fn set_extract_quads(&mut self, f: impl Fn(&World) -> Vec<QuadInstance> + 'static) {
        self.extract_quads = Some(Box::new(f));
    }

    /// Set the function that extracts sprite render data from the World.
    pub fn set_extract_sprites(&mut self, f: impl Fn(&World) -> Vec<SpriteBatch> + 'static) {
        self.extract_sprites = Some(Box::new(f));
    }

    /// Set the function that extracts text render data from the World.
    pub fn set_extract_text(&mut self, f: impl Fn(&World) -> Vec<TextSection> + 'static) {
        self.extract_text = Some(Box::new(f));
    }

    /// Set a one-shot startup function that runs after the renderer is ready.
    /// Use this for asset loading that requires GPU access.
    pub fn on_startup(&mut self, f: impl FnOnce(&mut World, &mut Renderer) + 'static) {
        self.startup = Some(Box::new(f));
    }

    /// Register an ordered list of manifest JSON paths the engine merges and
    /// loads before the user's [`on_startup`] callback runs (D-052). The
    /// merged graph is stored as a [`tungsten_core::assets::LoadedManifest`]
    /// resource so hot-reload and diagnostics can diff against it.
    ///
    /// Duplicate IDs across manifests are fatal at boot (D-017). Prefer this
    /// over calling per-type loaders from `on_startup`; those replace
    /// registries wholesale and are the reason composition used to be
    /// call-site-dependent.
    pub fn set_manifest_roots(&mut self, roots: Vec<PathBuf>) {
        self.manifest_roots = roots;
    }

    /// Enable hot reload: watch each directory in `assets_dirs` for file
    /// changes and reload assets at the next frame boundary. Pass multiple
    /// directories when assets span more than one location (e.g. a shared
    /// root `assets/` plus an example-local `assets/`). `manifest_path` is
    /// used to detect manifest-file changes specifically. Has no effect if
    /// the watcher fails to start (the error is logged and the engine
    /// continues without hot reload).
    pub fn enable_hot_reload(&mut self, assets_dirs: &[PathBuf], manifest_path: PathBuf) {
        let extra_files = [self.input_map_path.clone()];
        self.hot_reload = HotReloadWatcher::new(assets_dirs, &extra_files);
        self.manifest_path = Some(manifest_path);
    }

    /// Run the application. Blocks until the window is closed.
    pub fn run(mut self) -> anyhow::Result<()> {
        self.install_default_extracts();
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut self)?;
        Ok(())
    }

    /// Install default render-extract closures when the user did not supply
    /// their own. Today this only covers the sprite-extract slot (M15 /
    /// D-042). Called at the start of [`Self::run`] and from tests; safe to
    /// call multiple times because the conditional checks for `None`.
    fn install_default_extracts(&mut self) {
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

    /// Poll the hot-reload watcher and apply any ready changes. Called after
    /// tick() and before render() each frame. No-op when hot reload is disabled.
    fn process_hot_reload(&mut self) {
        let ready = match self.hot_reload.as_mut() {
            Some(w) => w.drain_ready(),
            None => return,
        };
        if ready.is_empty() {
            return;
        }

        let renderer = match self.renderer.as_mut() {
            Some(r) => r,
            None => return,
        };

        for path in &ready {
            let canon = path.canonicalize().unwrap_or_else(|_| path.clone());

            // Check if this is the workspace-root input.json first — its
            // name-based match takes priority over the manifest/extension
            // dispatch below.
            if canon.file_name().is_some_and(|name| name == "input.json") {
                if let Err(e) = asset_loader::reload_action_map(&canon, &mut self.world) {
                    log::error!("Action map reload: {e}");
                }
                continue;
            }

            // Check if this is the manifest file.
            if let Some(mp) = &self.manifest_path {
                let canon_mp = mp.canonicalize().unwrap_or_else(|_| mp.clone());
                if canon == canon_mp {
                    let mp = mp.clone();
                    if let Err(e) = asset_loader::reload_manifest(&mp, &mut self.world, renderer) {
                        log::error!("Manifest reload error: {e}");
                    }
                    continue;
                }
            }

            let ext = canon.extension().and_then(|e| e.to_str()).unwrap_or("");
            match ext {
                "png" | "jpg" | "jpeg" => {
                    // Scope the immutable borrow so it drops before we pass
                    // &mut self.world to reload_sprite.
                    let id_filter = {
                        let reg = self
                            .world
                            .get_resource::<AssetRegistry>()
                            .expect("AssetRegistry resource missing");
                        reg.sprite_id_for_path(&canon)
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
                        .and_then(|ar| ar.id_for_path(&canon).map(|s| s.to_string()));
                    let particle_id = self
                        .world
                        .get_resource::<ParticleConfigRegistry>()
                        .and_then(|pr| {
                            pr.id_for_path(&canon)
                                .and_then(|aid| pr.name_for_id(aid).map(|s| s.to_string()))
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
                        .and_then(|fr| fr.id_for_path(&canon).map(|s| s.to_string()));
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
                        .and_then(|tr| tr.id_for_path(&canon).map(|s| s.to_string()));
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

        if delta.surface_pacing_changed {
            if let Some(renderer) = self.renderer.as_mut() {
                match renderer.reconfigure_surface_pacing(
                    requested.present_mode,
                    requested.vsync,
                    requested.max_frame_latency,
                ) {
                    Ok(()) => {
                        effective.vsync = requested.vsync;
                        effective.present_mode = requested.present_mode;
                        effective.max_frame_latency = requested.max_frame_latency;
                        actual_present_mode = renderer.gpu_timings.present_mode.clone();
                        if let Some(gpu) = self.world.get_resource_mut::<GpuFrameTimings>() {
                            *gpu = renderer.gpu_timings.clone();
                        }
                    }
                    Err(err) => {
                        log::error!("Failed to apply display pacing change: {err}");
                    }
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
}

/// Output of the extract stage: everything needed to call `render_frame_full`.
/// Kept in the umbrella crate so it doesn't leak into `tungsten-render`.
struct FrameExtract {
    quads: Vec<QuadInstance>,
    sprites: Vec<SpriteBatch>,
    text: Vec<TextSection>,
    debug_quads: Vec<QuadInstance>,
    debug_lines: Vec<DebugLineInstance>,
    extract_ms: f32,
}

/// Output of the render stage. Timings are forwarded to `FrameTimings`
/// after the audio + input stages.
#[derive(Default)]
struct FrameRenderOut {
    render_ms: f32,
    render_acquire_ms: f32,
    render_encode_ms: f32,
    render_submit_present_ms: f32,
    gpu_frame_ms: Option<f32>,
}

/// Bundle of frame timings written in one pass at the end of each frame.
/// Prevents the telemetry stage from holding a long `&mut FrameTimings`
/// borrow across the render/audio stages.
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
    system_timings: Vec<(String, f32)>,
}

impl App {
    // Stage methods are marked `#[inline(always)]` so debug builds flatten
    // them back into `RedrawRequested` (release already does via LTO). Without
    // these hints `opt-level=0` keeps every stage as its own stack frame plus
    // struct-return memcpys (`FrameExtract`, `FrameStageTimings`,
    // `FrameRenderOut`), which shows up first at ~1000-entity physics loads.
    // Each stage is called exactly once per frame from the `RedrawRequested`
    // arm, so `always` cannot cause code bloat beyond the one inlined copy.

    #[inline(always)]
    fn stage_delta_time(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_frame {
            let dt = now.duration_since(last).as_secs_f32();
            if let Some(delta) = self.world.get_resource_mut::<DeltaTime>() {
                delta.dt = dt;
            }
        }
        self.last_frame = Some(now);
    }

    #[inline(always)]
    fn stage_update(&mut self) -> (f32, Vec<(String, f32)>) {
        let update_start = Instant::now();
        let mut system_timings: Vec<(String, f32)> = Vec::with_capacity(self.systems.len());
        debug_assert_eq!(self.systems.len(), self.system_names.len());
        for (system, name) in self.systems.iter_mut().zip(self.system_names.iter()) {
            // Systems run with this frame's accumulated input (edge state
            // was populated by KeyboardInput/MouseInput events that arrived
            // before RedrawRequested in the same event-loop turn).
            let t0 = Instant::now();
            system(&mut self.world);
            system_timings.push((name.clone(), t0.elapsed().as_secs_f64() as f32 * 1000.0));
        }
        let update_ms = update_start.elapsed().as_secs_f64() as f32 * 1000.0;
        (update_ms, system_timings)
    }

    #[inline(always)]
    fn stage_particles(&mut self) {
        // M23: count refresh → emit → tick. Runs after user systems and
        // before the command-buffer flush. Emission before tick guarantees
        // newly-spawned particles get their first integration step on the
        // following frame, and despawns are still queued via CommandBuffer
        // for the flush.
        crate::particles::particle_count_refresh_system(&mut self.world);
        crate::particles::particle_emit_system(&mut self.world);
        crate::particles::particle_tick_system(&mut self.world);
    }

    #[inline(always)]
    fn stage_flush_commands(&mut self) -> f32 {
        // Frame order invariant (Phase 3 guardrail):
        //   run systems -> flush commands -> flush events -> hot-reload -> extract -> render
        // Mutations are NOT visible to frame-N systems (they ran before this point).
        // Mutations ARE visible to extract/render within this frame.
        let flush_start = Instant::now();
        let flush_buf = self
            .world
            .remove_resource::<CommandBuffer>()
            .expect("CommandBuffer resource missing -- was it removed by a system?");
        self.world.flush(flush_buf);
        self.world.insert_resource(CommandBuffer::new());
        flush_start.elapsed().as_secs_f64() as f32 * 1000.0
    }

    #[inline(always)]
    fn stage_flush_events(&mut self) {
        for flusher in self.event_flushers.iter_mut() {
            flusher(&mut self.world);
        }
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

        // Populate RenderCounts from this frame's live entity count and
        // extracted sprite instances. Must run before HUD compose so the
        // `counts` row reflects this frame.
        let entity_count = self.world.entity_count();
        let sprite_instance_count: u32 = sprites.iter().map(|b| b.instances.len() as u32).sum();
        if let Some(rc) = self.world.get_resource_mut::<RenderCounts>() {
            rc.entities = entity_count;
            rc.sprite_instances = sprite_instance_count;
        }

        // Physics debug emits into DebugDraw at the start of extract so the
        // drain below sees this frame's commands on the same frame they were
        // produced.
        physics_debug_emit_system(&mut self.world);

        // Drain DebugDraw into the two render channels. AABB edges reuse
        // QuadPipeline; arbitrary-angle lines + circle polylines use
        // DebugLinePipeline.
        let (debug_quads, debug_lines) = drain_debug_draw(&mut self.world);

        // HUD / systems-overlay / inspector compose. Each uses the same
        // borrow dance: remove the resource so providers can hold `&World`
        // while the helper holds `&mut` to the resource, then re-insert.
        let viewport = self
            .world
            .get_resource::<WindowSize>()
            .map(|w| (w.width, w.height))
            .unwrap_or((0, 0));
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

        let extract_ms = extract_start.elapsed().as_secs_f64() as f32 * 1000.0;
        FrameExtract {
            quads,
            sprites,
            text,
            debug_quads,
            debug_lines,
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

            // Screenshot capture hook: arm the renderer for the target frame
            // so the offscreen readback runs on the same frame we count below.
            if let Some(cfg) = self.capture_config.as_mut() {
                if !cfg.captured && self.frames_rendered + 1 == cfg.target_frame {
                    if let Err(e) = renderer.capture_frame(&cfg.path) {
                        log::warn!("capture_frame({}) failed to arm: {e}", cfg.path.display());
                    }
                }
            }

            let result = if self.gpu_timing_enabled {
                renderer.render_frame_full_timed(
                    &view_proj,
                    &extract.quads,
                    &extract.sprites,
                    &extract.debug_quads,
                    &extract.debug_lines,
                    &extract.text,
                )
            } else {
                renderer.render_frame_full(
                    &view_proj,
                    &extract.quads,
                    &extract.sprites,
                    &extract.debug_quads,
                    &extract.debug_lines,
                    &extract.text,
                )
            };
            if let Err(e) = result {
                log::error!("Render error: {e}");
            } else {
                self.frames_rendered = self.frames_rendered.saturating_add(1);
                if let Some(cfg) = self.capture_config.as_mut() {
                    if !cfg.captured && self.frames_rendered == cfg.target_frame {
                        cfg.captured = true;
                        log::info!(
                            "captured frame {} -> {}",
                            cfg.target_frame,
                            cfg.path.display()
                        );
                    }
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

    #[inline(always)]
    fn stage_audio(&mut self) -> f32 {
        let audio_start = Instant::now();
        if let (Some(audio), Some(cmds)) = (
            &mut self.audio,
            self.world.get_resource_mut::<AudioCommands>(),
        ) {
            for cmd in cmds.drain() {
                audio.send(cmd);
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
            ft.system_timings = t.system_timings;
        }
    }

    #[inline(always)]
    fn stage_pacing(&self, event_loop: &ActiveEventLoop, frame_start: Instant) {
        if let Some(budget) = self.frame_budget {
            event_loop.set_control_flow(ControlFlow::WaitUntil(frame_start + budget));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
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
fn log_perf_line(
    total_ms: f32,
    update_ms: f32,
    flush_ms: f32,
    extract_ms: f32,
    render_out: &FrameRenderOut,
    audio_ms: f32,
    hot_reload_ms: f32,
) {
    let gpu_for_log = render_out
        .gpu_frame_ms
        .map(|ms| format!("{ms:.2}ms"))
        .unwrap_or_else(|| "n/a".to_string());
    log::debug!(
        "frame: total={:.2}ms update={:.2}ms flush={:.2}ms extract={:.2}ms render={:.2}ms render_acquire={:.2}ms render_encode={:.2}ms render_submit_present={:.2}ms gpu={} audio={:.2}ms hot_reload={:.2}ms",
        total_ms,
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

/// Startup load for `input.json`. Missing file → engine defaults with an
/// info log. IO or parse errors are fatal so they surface from `App::new`
/// the same way `ConfigError` does from `Config::load` (per `D-008`).
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

/// Parse the `TUNGSTEN_CAPTURE_RESOLUTION=WxH` env var. Returns `None` when
/// unset or malformed so visual-regression fixtures can pin a resolution
/// without touching `tungsten.json`.
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

/// Parse the `TUNGSTEN_CAPTURE_FRAME` / `TUNGSTEN_CAPTURE_PATH` pair. The
/// frame number is required; the path defaults to `./actual.png`.
fn parse_capture_config() -> Option<CaptureConfig> {
    let target_frame: u64 = std::env::var("TUNGSTEN_CAPTURE_FRAME").ok()?.parse().ok()?;
    if target_frame == 0 {
        return None;
    }
    let path = std::env::var("TUNGSTEN_CAPTURE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("actual.png"));
    Some(CaptureConfig {
        target_frame,
        path,
        captured: false,
    })
}

/// Expand an AABB outline into four thin axis-aligned edge quads. Edges are
/// inset by `thickness / 2` so the outline tracks the AABB interior rather
/// than ballooning outward for large thicknesses.
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
    // Top
    out.push(QuadInstance {
        position: [min.x, min.y],
        size: [w, t],
        color,
    });
    // Bottom
    out.push(QuadInstance {
        position: [min.x, (max.y - t).max(min.y)],
        size: [w, t],
        color,
    });
    // Left
    out.push(QuadInstance {
        position: [min.x, min.y],
        size: [t, h],
        color,
    });
    // Right
    out.push(QuadInstance {
        position: [(max.x - t).max(min.x), min.y],
        size: [t, h],
        color,
    });
}

/// Expand a circle command into a closed polyline of `segments` line
/// instances. Degenerate cases (`radius <= 0` or `segments == 0`) emit
/// nothing so downstream pipelines short-circuit on empty slices.
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
    let step = two_pi / segments as f32;
    let mut prev = center + glam::Vec2::new(radius, 0.0);
    for i in 1..=segments {
        let t = i as f32 * step;
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
            Ok(renderer) => {
                if std::env::var("TUNGSTEN_PERF_LOG").is_ok() {
                    log::debug!(
                        "backend: {} adapter: {} present_mode: {} max_frame_latency: {} timestamp_query: {}",
                        renderer
                            .gpu_timings
                            .backend
                            .as_deref()
                            .unwrap_or("unknown"),
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
                event_loop.exit();
                return;
            }
        }

        self.window = Some(window);

        // Merge-first asset composition (D-052). Runs before the user's
        // `on_startup` closure so game code can spawn against loaded assets.
        // Duplicate IDs across manifests halt boot with `ManifestError::DuplicateId`.
        if !self.manifest_roots.is_empty() {
            if let Some(renderer) = &mut self.renderer {
                if let Err(e) =
                    asset_loader::load_all_merged(&self.manifest_roots, &mut self.world, renderer)
                {
                    log::error!("Manifest composition failed: {e}");
                    event_loop.exit();
                    return;
                }
            }
        }

        // Run startup callback (asset loading, etc.)
        if let Some(startup) = self.startup.take() {
            if let Some(renderer) = &mut self.renderer {
                startup(&mut self.world, renderer);
            }
        }

        // Stamp last_frame AFTER startup so asset-loading time is not counted
        // as the first game frame's dt. A large first-frame dt would cause
        // fast-moving bodies (including a player falling under gravity) to
        // tunnel through thin collision geometry in a single substep.
        self.last_frame = Some(Instant::now());

        // Initialize audio after the startup callback so that load_sounds()
        // has already populated the SoundRegistry resource.
        if let Some(sound_registry) = self.world.get_resource::<SoundRegistry>() {
            match AudioSystem::init(sound_registry) {
                Ok(sys) => {
                    self.audio = Some(sys);
                }
                Err(e) => {
                    log::warn!("Audio init failed (continuing without audio): {}", e);
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
                            input.add_scroll_pixel_delta(delta.x as f32, delta.y as f32)
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                self.apply_pending_display_request();

                // Snapshot the previous frame's total so the HUD can smooth
                // frame-ms without needing compose to run after render (which
                // would hide the HUD this frame).
                let prev_total_ms = self
                    .world
                    .get_resource::<FrameTimings>()
                    .map(|ft| ft.total_ms)
                    .unwrap_or(0.0);

                self.stage_delta_time();
                let (update_ms, system_timings) = self.stage_update();

                if self.engine_exit_requested() {
                    event_loop.exit();
                    return;
                }

                self.stage_particles();
                let flush_ms = self.stage_flush_commands();
                self.stage_flush_events();
                let hot_reload_ms = self.stage_hot_reload();

                let extract_out = self.stage_extract(prev_total_ms);
                let extract_ms = extract_out.extract_ms;

                let render_out = self.stage_render(&extract_out);

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
                    system_timings,
                });

                if std::env::var("TUNGSTEN_PERF_LOG").is_ok() {
                    log_perf_line(
                        total_ms,
                        update_ms,
                        flush_ms,
                        extract_ms,
                        &render_out,
                        audio_ms,
                        hot_reload_ms,
                    );
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }

                self.stage_pacing(event_loop, frame_start);
                self.stage_smoke_exit(event_loop);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "tests/app.rs"]
mod tests;
