use std::sync::Arc;
use std::time::Instant;

use crate::input_bridge;
use tungsten_core::{AssetRegistry, Config, DeltaTime, InputState, World};
use tungsten_render::{QuadInstance, Renderer, SpriteBatch};
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
    systems: Vec<SystemFn>,
    extract_quads: Option<ExtractQuadsFn>,
    extract_sprites: Option<ExtractSpritesFn>,
    startup: Option<StartupFn>,
    last_frame: Option<Instant>,
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

        Self {
            config,
            window: None,
            renderer: None,
            world,
            systems: Vec::new(),
            extract_quads: None,
            extract_sprites: None,
            startup: None,
            last_frame: None,
        }
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

    /// Set a one-shot startup function that runs after the renderer is ready.
    /// Use this for asset loading that requires GPU access.
    pub fn on_startup(&mut self, f: impl FnOnce(&mut World, &mut Renderer) + 'static) {
        self.startup = Some(Box::new(f));
    }

    /// Run the application. Blocks until the window is closed.
    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut self)?;
        Ok(())
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
                if event.state == ElementState::Pressed {
                    if let winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape) =
                        event.physical_key
                    {
                        event_loop.exit();
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

                if let Some(input) = self.world.get_resource_mut::<InputState>() {
                    input.begin_frame();
                }

                for system in &mut self.systems {
                    system(&mut self.world);
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

                if let Some(renderer) = &self.renderer {
                    if let Err(e) = renderer.render_frame_full(&quads, &sprites) {
                        log::error!("Render error: {e}");
                    }
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
