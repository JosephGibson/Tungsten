use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::asset_loader;
use crate::audio::AudioSystem;
use crate::hot_reload::HotReloadWatcher;
use crate::input_bridge;
use tungsten_core::assets::{AnimationRegistry, FontRegistry, SoundRegistry, TilemapRegistry};
use tungsten_core::{AssetRegistry, AudioCommands, Camera2D, Config, DeltaTime, InputState, World};
use tungsten_render::{QuadInstance, Renderer, SpriteBatch, TextSection};
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
}

impl App {
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
        world.insert_resource(Camera2D::new());

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
        self.systems.push(Box::new(system));
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

    /// Enable hot reload: watch `assets_dir` for file changes and reload
    /// assets at the next frame boundary. `manifest_path` is used to detect
    /// manifest changes specifically. Has no effect if the watcher fails to
    /// start (the error is logged and the engine continues without hot reload).
    pub fn enable_hot_reload(&mut self, assets_dir: PathBuf, manifest_path: PathBuf) {
        self.hot_reload = HotReloadWatcher::new(assets_dir);
        self.manifest_path = Some(manifest_path);
    }

    /// Run the application. Blocks until the window is closed.
    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut self)?;
        Ok(())
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

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        match Renderer::new(
            window.clone(),
            &self.config.render,
            self.config.window.vsync,
        ) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
            }
            Err(e) => {
                log::error!("Failed to initialize renderer: {e}");
                event_loop.exit();
                return;
            }
        }

        self.window = Some(window);
        self.last_frame = Some(Instant::now());

        // Run startup callback (asset loading, etc.)
        if let Some(startup) = self.startup.take() {
            if let Some(renderer) = &mut self.renderer {
                startup(&mut self.world, renderer);
            }
        }

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
                let now = Instant::now();
                if let Some(last) = self.last_frame {
                    let dt = now.duration_since(last).as_secs_f32();
                    if let Some(delta) = self.world.get_resource_mut::<DeltaTime>() {
                        delta.dt = dt;
                    }
                }
                self.last_frame = Some(now);

                // Systems run with this frame's accumulated input (edge state
                // was populated by KeyboardInput/MouseInput events that arrived
                // before RedrawRequested in the same event-loop turn).
                for system in &mut self.systems {
                    system(&mut self.world);
                }

                // Poll file watcher and apply any ready asset changes.
                // Runs after tick() and before render() as required.
                self.process_hot_reload();

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

                if let Some(renderer) = &mut self.renderer {
                    let (vw, vh) = {
                        let cfg = &renderer.surface_config;
                        (cfg.width as f32, cfg.height as f32)
                    };
                    let view_proj = self
                        .world
                        .get_resource::<Camera2D>()
                        .copied()
                        .unwrap_or_default()
                        .view_projection(vw, vh);
                    if let Err(e) = renderer.render_frame_full(&view_proj, &quads, &sprites, &text)
                    {
                        log::error!("Render error: {e}");
                    }
                }

                // Forward audio commands to the audio thread.
                if let (Some(audio), Some(cmds)) =
                    (&self.audio, self.world.get_resource_mut::<AudioCommands>())
                {
                    for cmd in cmds.drain() {
                        let _ = audio.sender().send(cmd);
                    }
                }

                // Clear edge state *after* systems have consumed it, so the
                // next frame's input events start with a clean slate.
                if let Some(input) = self.world.get_resource_mut::<InputState>() {
                    input.begin_frame();
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
