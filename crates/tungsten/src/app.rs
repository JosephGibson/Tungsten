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
    AnimationRegistry, FontRegistry, ParticleConfigRegistry, ShaderRegistry, SoundRegistry,
    TilemapRegistry,
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

/// Fixed per-frame dt (seconds) used under `TUNGSTEN_SMOKE_FRAMES`. Pinned to
/// 60 Hz so smoke-mode captures and visual regressions are frame-deterministic
/// across runs regardless of build profile or host load.
const SMOKE_MODE_FIXED_DT_SECS: f32 = 1.0 / 60.0;

/// Tick system.
pub type SystemFn = Box<dyn FnMut(&mut World)>;

/// World-to-quad extract.
pub type ExtractQuadsFn = Box<dyn Fn(&World) -> Vec<QuadInstance>>;

/// World-to-sprite extract.
pub type ExtractSpritesFn = Box<dyn Fn(&World) -> Vec<SpriteBatch>>;

/// World-to-text extract.
pub type ExtractTextFn = Box<dyn Fn(&World) -> Vec<TextSection>>;

/// Window dimensions in physical pixels.
#[derive(Debug, Clone, Copy)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

/// Post-renderer startup hook.
pub type StartupFn = Box<dyn FnOnce(&mut World, &mut Renderer)>;

type EventFlusher = Box<dyn FnMut(&mut World)>;

/// Winit loop, ECS, and renderer owner.
pub struct App {
    config: Config,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    world: World,
    // Order: registration order; no priorities.
    systems: Vec<SystemFn>,
    extract_quads: Option<ExtractQuadsFn>,
    extract_sprites: Option<ExtractSpritesFn>,
    extract_text: Option<ExtractTextFn>,
    startup: Option<StartupFn>,
    last_frame: Option<Instant>,
    exit_on_escape: bool,
    audio: Option<AudioSystem>,
    hot_reload: Option<HotReloadWatcher>,
    manifest_path: Option<PathBuf>,
    // D-052: ordered manifest roots merged before user startup.
    manifest_roots: Vec<PathBuf>,
    input_map_path: PathBuf,
    smoke_frames_remaining: Option<u32>,
    system_names: Vec<String>,
    system_name_counter: usize,
    // GPU timing path adds `device.poll` stall.
    gpu_timing_enabled: bool,
    // Event queue flushers run after command flush.
    event_flushers: Vec<EventFlusher>,
    registered_event_types: HashSet<TypeId>,
    frame_budget: Option<Duration>,
    capture_config: Option<CaptureConfig>,
    frames_rendered: u64,
}

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
        world.insert_resource(DebugDraw::new());
        world.insert_resource(PhysicsDebugOverlay::default());
        world.insert_resource(SystemTimingOverlay::default());
        world.insert_resource(InspectorState::new_with_defaults());
        // M26: empty post stack by default — byte-identical to M25 baseline.
        world.insert_resource(tungsten_core::post::PostStack::new());
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
        Self::register_event_inner::<tungsten_core::TweenComplete>(
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

        // Engine input consumers precede user systems; overlay toggles precede HUD.
        app.add_engine_system("__physics_debug_toggle", physics_debug_toggle_system);
        app.add_engine_system("__systems_overlay_toggle", systems_overlay_toggle_system);
        app.add_engine_system("__inspector_toggle", inspector_toggle_system);
        app.add_engine_system("__inspector_pick", inspector_pick_system);
        app.add_engine_system("__hud_toggle", hud_toggle_system);
        app.add_engine_system("__display_input", engine_display_input_system);
        app.add_engine_system("__state_dispatcher", state_dispatcher_system);

        Ok(app)
    }

    /// Register inspectable component rows under `label`.
    pub fn register_inspectable<T: 'static + Inspectable>(&mut self, label: &'static str) {
        if let Some(state) = self.world.get_resource_mut::<InspectorState>() {
            state.register::<T>(label);
        }
    }

    /// Register engine system without touching user-system naming.
    fn add_engine_system(&mut self, name: &str, system: impl FnMut(&mut World) + 'static) {
        self.system_names.push(name.to_string());
        self.systems.push(Box::new(system));
    }

    /// Enable or disable engine exit action handling.
    pub fn set_exit_on_escape(&mut self, exit: bool) {
        self.exit_on_escape = exit;
    }

    /// Mutable world access for setup.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Mutable renderer access after window creation.
    pub fn renderer_mut(&mut self) -> Option<&mut Renderer> {
        self.renderer.as_mut()
    }

    /// Register unnamed tick system.
    pub fn add_system(&mut self, system: impl FnMut(&mut World) + 'static) {
        let name = format!("system_{}", self.system_name_counter);
        self.system_name_counter += 1;
        self.system_names.push(name);
        self.systems.push(Box::new(system));
    }

    /// Register named tick system for profiling output.
    pub fn add_system_named(
        &mut self,
        name: impl Into<String>,
        system: impl FnMut(&mut World) + 'static,
    ) {
        self.system_names.push(name.into());
        self.systems.push(Box::new(system));
    }

    /// Register `EventQueue<T>`; flushes once per frame after command flush.
    pub fn register_event<T: 'static>(&mut self) {
        Self::register_event_inner::<T>(
            &mut self.world,
            &mut self.event_flushers,
            &mut self.registered_event_types,
        );
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
    pub fn enable_hot_reload(&mut self, assets_dirs: &[PathBuf], manifest_path: PathBuf) {
        let extra_files = [self.input_map_path.clone()];
        self.hot_reload = HotReloadWatcher::new(assets_dirs, &extra_files);
        self.manifest_path = Some(manifest_path);
    }

    /// Run until window close or explicit exit.
    pub fn run(mut self) -> anyhow::Result<()> {
        self.install_default_extracts();
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut self)?;
        Ok(())
    }

    /// Install default extracts; idempotent.
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

        for path in &ready {
            let canon = path.canonicalize().unwrap_or_else(|_| path.clone());

            if canon.file_name().is_some_and(|name| name == "input.json") {
                if let Err(e) = asset_loader::reload_action_map(&canon, &mut self.world) {
                    log::error!("Action map reload: {e}");
                }
                continue;
            }

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
                    // Drop registry borrow before `reload_sprite(&mut World)`.
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

// Extract output kept in umbrella crate; renderer stays World-free.
struct FrameExtract {
    quads: Vec<QuadInstance>,
    sprites: Vec<SpriteBatch>,
    text: Vec<TextSection>,
    debug_quads: Vec<QuadInstance>,
    debug_lines: Vec<DebugLineInstance>,
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
    system_timings: Vec<(String, f32)>,
}

impl App {
    // Debug perf: flatten single-call frame stages to avoid stack/memcpy overhead.

    #[inline(always)]
    fn stage_delta_time(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_frame {
            // Smoke mode (TUNGSTEN_SMOKE_FRAMES set) pins dt to 60 Hz so
            // physics / particles / tweens / scene animation produce
            // frame-accurate deterministic output. Wall-clock dt varies
            // with CPU load and makes visual-regression captures drift
            // between runs on the same binary. The capture path used by
            // `visual_regression.rs` (smoke frames + capture frame env
            // vars) is the canonical consumer.
            let dt = if self.smoke_frames_remaining.is_some() {
                SMOKE_MODE_FIXED_DT_SECS
            } else {
                now.duration_since(last).as_secs_f32()
            };
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
            // Input edges: events received before RedrawRequested in same loop turn.
            let t0 = Instant::now();
            system(&mut self.world);
            system_timings.push((name.clone(), t0.elapsed().as_secs_f64() as f32 * 1000.0));
        }
        let update_ms = update_start.elapsed().as_secs_f64() as f32 * 1000.0;
        (update_ms, system_timings)
    }

    #[inline(always)]
    fn stage_particles(&mut self) {
        // Order: count refresh -> emit -> tick, after systems and before command flush.
        crate::particles::particle_count_refresh_system(&mut self.world);
        crate::particles::particle_emit_system(&mut self.world);
        crate::particles::particle_tick_system(&mut self.world);
    }

    #[inline(always)]
    fn stage_tweens(&mut self) {
        // Tween writes override particle writes; `TweenComplete` lands before event flush rotates.
        crate::tweens::tween_tick_system(&mut self.world);
    }

    #[inline(always)]
    fn stage_flush_commands(&mut self) -> f32 {
        // Frame order: systems -> command flush -> event flush -> hot reload -> extract -> render.
        // Frame-N command mutations: invisible to systems, visible to extract/render.
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
        for flusher in &mut self.event_flushers {
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

            // Arm before target render; mark captured after successful submit.
            if let Some(cfg) = self.capture_config.as_mut() {
                if !cfg.captured && self.frames_rendered + 1 == cfg.target_frame {
                    if let Err(e) = renderer.capture_frame(&cfg.path) {
                        log::warn!("capture_frame({}) failed to arm: {e}", cfg.path.display());
                    }
                }
            }

            // M26: PostStack is a world resource; default is empty.
            let post_stack = self
                .world
                .get_resource::<tungsten_core::post::PostStack>()
                .cloned()
                .unwrap_or_default();

            let result = if self.gpu_timing_enabled {
                renderer.render_frame_full_timed(
                    &view_proj,
                    &extract.quads,
                    &extract.sprites,
                    &extract.debug_quads,
                    &extract.debug_lines,
                    &extract.text,
                    &post_stack,
                )
            } else {
                renderer.render_frame_full(
                    &view_proj,
                    &extract.quads,
                    &extract.sprites,
                    &extract.debug_quads,
                    &extract.debug_lines,
                    &extract.text,
                    &post_stack,
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
        .map_or_else(|| "n/a".to_string(), |ms| format!("{ms:.2}ms"));
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
    Some(CaptureConfig {
        target_frame,
        path,
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

        // D-052: merge manifests before user startup; duplicate IDs halt boot.
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

        if let Some(startup) = self.startup.take() {
            if let Some(renderer) = &mut self.renderer {
                startup(&mut self.world, renderer);
            }
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
                self.apply_pending_display_request();

                // HUD smoothing uses previous frame; compose still occurs before render.
                let prev_total_ms = self
                    .world
                    .get_resource::<FrameTimings>()
                    .map_or(0.0, |ft| ft.total_ms);

                self.stage_delta_time();
                let (update_ms, system_timings) = self.stage_update();

                if self.engine_exit_requested() {
                    event_loop.exit();
                    return;
                }

                self.stage_particles();
                self.stage_tweens();
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
