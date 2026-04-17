use std::any::TypeId;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::asset_loader;
use crate::audio::AudioSystem;
use crate::hot_reload::HotReloadWatcher;
use crate::input_bridge;
use crate::telemetry::FrameTimings;
use tungsten_core::assets::{AnimationRegistry, FontRegistry, SoundRegistry, TilemapRegistry};
use tungsten_core::physics::{CollisionEvent, PhysicsConfig};
use tungsten_core::{
    AssetRegistry, AudioCommands, CameraController, CameraState, CommandBuffer, Config, DeltaTime,
    EventQueue, InputState, World,
};
use tungsten_render::{GpuFrameTimings, QuadInstance, Renderer, SpriteBatch, TextSection};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

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
    /// If true (the default), the Escape key exits the process. Set to false
    /// when game code needs Escape for its own purposes (e.g. pause menus).
    exit_on_escape: bool,
    /// Audio subsystem — initialized after the startup callback runs.
    /// Kept alive here so the cpal stream doesn't drop.
    audio: Option<AudioSystem>,
    /// Hot-reload file watcher. None when hot reload is not enabled.
    hot_reload: Option<HotReloadWatcher>,
    /// Path to the primary manifest file, used to detect manifest changes.
    manifest_path: Option<PathBuf>,
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

    pub fn new(config: Config) -> Self {
        let mut world = World::new();
        world.insert_resource(DeltaTime::new());
        world.insert_resource(InputState::new());
        world.insert_resource(WindowSize {
            width: config.window.width,
            height: config.window.height,
        });
        world.insert_resource(AssetRegistry::new());
        world.insert_resource(SoundRegistry::new());
        world.insert_resource(AudioCommands::new());
        world.insert_resource(TilemapRegistry::new());
        world.insert_resource(CameraState::new());
        world.insert_resource(CameraController::default());
        world.insert_resource(PhysicsConfig::default());
        world.insert_resource(FrameTimings::new());
        world.insert_resource(GpuFrameTimings::default());
        world.insert_resource(CommandBuffer::new());
        let mut event_flushers: Vec<EventFlusher> = Vec::new();
        let mut registered_event_types = HashSet::new();
        Self::register_event_inner::<CollisionEvent>(
            &mut world,
            &mut event_flushers,
            &mut registered_event_types,
        );

        Self {
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
            smoke_frames_remaining: std::env::var("TUNGSTEN_SMOKE_FRAMES")
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|n| *n > 0),
            system_names: Vec::new(),
            system_name_counter: 0,
            gpu_timing_enabled: std::env::var("TUNGSTEN_GPU_TIMING").is_ok(),
            event_flushers,
            registered_event_types,
        }
    }

    /// Control whether the Escape key exits the process (default: true).
    /// Disable when game code uses Escape for its own purposes (e.g. pause menus).
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

    /// Enable hot reload: watch each directory in `assets_dirs` for file
    /// changes and reload assets at the next frame boundary. Pass multiple
    /// directories when assets span more than one location (e.g. a shared
    /// root `assets/` plus an example-local `assets/`). `manifest_path` is
    /// used to detect manifest-file changes specifically. Has no effect if
    /// the watcher fails to start (the error is logged and the engine
    /// continues without hot reload).
    pub fn enable_hot_reload(&mut self, assets_dirs: &[PathBuf], manifest_path: PathBuf) {
        self.hot_reload = HotReloadWatcher::new(assets_dirs);
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
                    let id = self
                        .world
                        .get_resource::<AnimationRegistry>()
                        .and_then(|ar| ar.id_for_path(&canon).map(|s| s.to_string()));
                    if let Some(id) = id {
                        if let Err(e) = asset_loader::reload_animation(&id, &canon, &mut self.world)
                        {
                            log::error!("Animation reload '{id}': {e}");
                        }
                    } else {
                        log::debug!(
                            "Hot reload: no animation registered for '{}'",
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
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title(&self.config.window.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.window.width,
                self.config.window.height,
            ));

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        match Renderer::new(
            window.clone(),
            &self.config.render,
            self.config.window.vsync,
        ) {
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
                self.renderer = Some(renderer);
            }
            Err(e) => {
                log::error!("Failed to initialize renderer: {e}");
                event_loop.exit();
                return;
            }
        }

        self.window = Some(window);

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
                if let Some(ws) = self.world.get_resource_mut::<WindowSize>() {
                    ws.width = size.width;
                    ws.height = size.height;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let key = input_bridge::translate_key(event.physical_key);
                if let Some(input) = self.world.get_resource_mut::<InputState>() {
                    match event.state {
                        ElementState::Pressed => input.key_down(key),
                        ElementState::Released => input.key_up(key),
                    }
                }
                if self.exit_on_escape
                    && event.state == ElementState::Pressed
                    && matches!(
                        event.physical_key,
                        winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape)
                    )
                {
                    event_loop.exit();
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
                    input.cursor_position = Some((position.x as f32, position.y as f32));
                }
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();

                // --- Delta time ---
                let now = Instant::now();
                if let Some(last) = self.last_frame {
                    let dt = now.duration_since(last).as_secs_f32();
                    if let Some(delta) = self.world.get_resource_mut::<DeltaTime>() {
                        delta.dt = dt;
                    }
                }
                self.last_frame = Some(now);

                // --- Update stage: all registered systems ---
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

                // --- Flush stage: apply deferred command buffers ---
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
                let flush_ms = flush_start.elapsed().as_secs_f64() as f32 * 1000.0;

                // --- Event queue flush stage ---
                // Frame order invariant (Phase 3 guardrail):
                //   run systems -> flush commands -> flush events -> hot-reload -> extract -> render
                for flusher in self.event_flushers.iter_mut() {
                    flusher(&mut self.world);
                }

                // --- Hot reload stage ---
                let hot_reload_start = Instant::now();
                self.process_hot_reload();
                let hot_reload_ms = hot_reload_start.elapsed().as_secs_f64() as f32 * 1000.0;

                // --- Extract stage ---
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

                let text = self
                    .extract_text
                    .as_ref()
                    .map(|f| f(&self.world))
                    .unwrap_or_default();
                let extract_ms = extract_start.elapsed().as_secs_f64() as f32 * 1000.0;

                // --- Render stage ---
                let render_start = Instant::now();
                let mut render_acquire_ms = 0.0f32;
                let mut render_encode_ms = 0.0f32;
                let mut render_submit_present_ms = 0.0f32;
                let mut gpu_frame_ms = None;
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
                    let result = if self.gpu_timing_enabled {
                        renderer.render_frame_full_timed(&view_proj, &quads, &sprites, &text)
                    } else {
                        renderer.render_frame_full(&view_proj, &quads, &sprites, &text)
                    };
                    if let Err(e) = result {
                        log::error!("Render error: {e}");
                    }
                    render_acquire_ms = renderer.cpu_timings.acquire_ms;
                    render_encode_ms = renderer.cpu_timings.encode_ms;
                    render_submit_present_ms = renderer.cpu_timings.submit_present_ms;
                    gpu_frame_ms = renderer.gpu_timings.frame_gpu_ms;
                    if let Some(gpu) = self.world.get_resource_mut::<GpuFrameTimings>() {
                        *gpu = renderer.gpu_timings.clone();
                    }
                }
                let render_ms = render_start.elapsed().as_secs_f64() as f32 * 1000.0;

                // --- Audio stage ---
                let audio_start = Instant::now();
                if let (Some(audio), Some(cmds)) = (
                    &mut self.audio,
                    self.world.get_resource_mut::<AudioCommands>(),
                ) {
                    for cmd in cmds.drain() {
                        audio.send(cmd);
                    }
                }
                let audio_ms = audio_start.elapsed().as_secs_f64() as f32 * 1000.0;

                // Clear edge state *after* systems have consumed it, so the
                // next frame's input events start with a clean slate.
                if let Some(input) = self.world.get_resource_mut::<InputState>() {
                    input.begin_frame();
                }

                let total_ms = frame_start.elapsed().as_secs_f64() as f32 * 1000.0;
                if let Some(ft) = self.world.get_resource_mut::<FrameTimings>() {
                    ft.update_ms = update_ms;
                    ft.extract_ms = extract_ms;
                    ft.render_ms = render_ms;
                    ft.render_acquire_ms = render_acquire_ms;
                    ft.render_encode_ms = render_encode_ms;
                    ft.render_submit_present_ms = render_submit_present_ms;
                    ft.audio_ms = audio_ms;
                    ft.hot_reload_ms = hot_reload_ms;
                    ft.flush_ms = flush_ms;
                    ft.total_ms = total_ms;
                    ft.system_timings = system_timings;
                }

                if std::env::var("TUNGSTEN_PERF_LOG").is_ok() {
                    let gpu_for_log = gpu_frame_ms
                        .map(|ms| format!("{ms:.2}ms"))
                        .unwrap_or_else(|| "n/a".to_string());
                    log::debug!(
                        "frame: total={:.2}ms update={:.2}ms flush={:.2}ms extract={:.2}ms render={:.2}ms render_acquire={:.2}ms render_encode={:.2}ms render_submit_present={:.2}ms gpu={} audio={:.2}ms hot_reload={:.2}ms",
                        total_ms,
                        update_ms,
                        flush_ms,
                        extract_ms,
                        render_ms,
                        render_acquire_ms,
                        render_encode_ms,
                        render_submit_present_ms,
                        gpu_for_log,
                        audio_ms,
                        hot_reload_ms
                    );
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }

                if let Some(remaining) = self.smoke_frames_remaining.as_mut() {
                    *remaining = remaining.saturating_sub(1);
                    if *remaining == 0 {
                        log::info!("TUNGSTEN_SMOKE_FRAMES reached; exiting cleanly");
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use tungsten_core::{CollisionEvent, Config, EventQueue};

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
}
