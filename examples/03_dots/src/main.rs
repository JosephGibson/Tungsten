use tungsten::core::{Config, DeltaTime, Entity, InputState, KeyCode, MouseButton, World};
use tungsten::render::QuadInstance;
use tungsten::App;
use tungsten::WindowSize;

#[derive(Debug, Clone)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone)]
struct Velocity {
    dx: f32,
    dy: f32,
}

#[derive(Debug, Clone)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[derive(Debug, Clone)]
struct Size {
    w: f32,
    h: f32,
}

/// Tag component marking the player-controlled dot.
#[derive(Debug, Clone)]
struct Player;

fn player_control_system(world: &mut World) {
    let dt = world.get_resource::<DeltaTime>().unwrap().seconds();
    let input = world.get_resource::<InputState>().unwrap().clone();
    let speed = 300.0;

    let entities = world.query_entities::<Player>();
    for entity in entities {
        if let Some(pos) = world.get_mut::<Position>(entity) {
            if input.is_pressed(KeyCode::ArrowLeft) || input.is_pressed(KeyCode::KeyA) {
                pos.x -= speed * dt;
            }
            if input.is_pressed(KeyCode::ArrowRight) || input.is_pressed(KeyCode::KeyD) {
                pos.x += speed * dt;
            }
            if input.is_pressed(KeyCode::ArrowUp) || input.is_pressed(KeyCode::KeyW) {
                pos.y -= speed * dt;
            }
            if input.is_pressed(KeyCode::ArrowDown) || input.is_pressed(KeyCode::KeyS) {
                pos.y += speed * dt;
            }
        }
    }
}

fn click_spawn_system(world: &mut World) {
    let input = world.get_resource::<InputState>().unwrap().clone();

    if input.mouse_just_pressed(MouseButton::Left) {
        if let Some((mx, my)) = input.cursor_position {
            let e = world.spawn();
            world.insert(
                e,
                Position {
                    x: mx - 8.0,
                    y: my - 8.0,
                },
            );
            world.insert(
                e,
                Velocity {
                    dx: ((mx * 7.0) % 400.0) - 200.0,
                    dy: ((my * 11.0) % 400.0) - 200.0,
                },
            );
            world.insert(
                e,
                Color {
                    r: (mx * 0.003) % 1.0,
                    g: (my * 0.005) % 1.0,
                    b: ((mx + my) * 0.004) % 1.0,
                    a: 1.0,
                },
            );
            world.insert(e, Size { w: 16.0, h: 16.0 });
        }
    }
}

fn movement_system(world: &mut World) {
    let dt = world.get_resource::<DeltaTime>().unwrap().seconds();
    let entities = world.query_entities::<Velocity>();
    for entity in entities {
        let vel = world.get::<Velocity>(entity).unwrap().clone();
        let pos = world.get_mut::<Position>(entity).unwrap();
        pos.x += vel.dx * dt;
        pos.y += vel.dy * dt;
    }
}

fn bounce_system(world: &mut World) {
    let win = world
        .get_resource::<WindowSize>()
        .map(|ws| (ws.width as f32, ws.height as f32))
        .unwrap_or((1280.0, 720.0));

    let entities = world.query_entities::<Velocity>();
    for entity in entities {
        let size = world
            .get::<Size>(entity)
            .map(|s| (s.w, s.h))
            .unwrap_or((10.0, 10.0));
        let pos = world.get::<Position>(entity).unwrap().clone();

        let mut vel = world.get::<Velocity>(entity).unwrap().clone();
        let mut bounced = false;

        if pos.x < 0.0 || pos.x + size.0 > win.0 {
            vel.dx = -vel.dx;
            bounced = true;
        }
        if pos.y < 0.0 || pos.y + size.1 > win.1 {
            vel.dy = -vel.dy;
            bounced = true;
        }

        if bounced {
            *world.get_mut::<Velocity>(entity).unwrap() = vel;
            let pos = world.get_mut::<Position>(entity).unwrap();
            pos.x = pos.x.clamp(0.0, win.0 - size.0);
            pos.y = pos.y.clamp(0.0, win.1 - size.1);
        }
    }
}

fn extract_quads(world: &World) -> Vec<QuadInstance> {
    world
        .query::<Position>()
        .map(|(entity, pos)| {
            let color = world
                .get::<Color>(entity)
                .map(|c| [c.r, c.g, c.b, c.a])
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);
            let size = world
                .get::<Size>(entity)
                .map(|s| [s.w, s.h])
                .unwrap_or([10.0, 10.0]);
            QuadInstance {
                position: [pos.x, pos.y],
                size,
                color,
            }
        })
        .collect()
}

struct DotDesc {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    r: f32,
    g: f32,
    b: f32,
    w: f32,
    h: f32,
}

fn spawn_dot(world: &mut World, d: DotDesc) -> Entity {
    let e = world.spawn();
    world.insert(e, Position { x: d.x, y: d.y });
    world.insert(e, Velocity { dx: d.dx, dy: d.dy });
    world.insert(
        e,
        Color {
            r: d.r,
            g: d.g,
            b: d.b,
            a: 1.0,
        },
    );
    world.insert(e, Size { w: d.w, h: d.h });
    e
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::load("tungsten.json")?;
    let mut app = App::new(config);

    let world = app.world_mut();

    // Player-controlled dot (white, larger)
    let player = world.spawn();
    world.insert(player, Position { x: 640.0, y: 360.0 });
    world.insert(
        player,
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    );
    world.insert(player, Size { w: 30.0, h: 30.0 });
    world.insert(player, Player);

    // Bouncing dots
    spawn_dot(
        world,
        DotDesc {
            x: 100.0,
            y: 100.0,
            dx: 200.0,
            dy: 150.0,
            r: 1.0,
            g: 0.3,
            b: 0.3,
            w: 20.0,
            h: 20.0,
        },
    );
    spawn_dot(
        world,
        DotDesc {
            x: 300.0,
            y: 200.0,
            dx: -180.0,
            dy: 220.0,
            r: 0.3,
            g: 1.0,
            b: 0.3,
            w: 15.0,
            h: 15.0,
        },
    );
    spawn_dot(
        world,
        DotDesc {
            x: 500.0,
            y: 300.0,
            dx: 250.0,
            dy: -100.0,
            r: 0.3,
            g: 0.3,
            b: 1.0,
            w: 25.0,
            h: 25.0,
        },
    );
    spawn_dot(
        world,
        DotDesc {
            x: 700.0,
            y: 400.0,
            dx: -120.0,
            dy: 180.0,
            r: 1.0,
            g: 1.0,
            b: 0.3,
            w: 12.0,
            h: 12.0,
        },
    );
    spawn_dot(
        world,
        DotDesc {
            x: 200.0,
            y: 500.0,
            dx: 160.0,
            dy: -200.0,
            r: 1.0,
            g: 0.3,
            b: 1.0,
            w: 18.0,
            h: 18.0,
        },
    );

    app.add_system(player_control_system);
    app.add_system(click_spawn_system);
    app.add_system(movement_system);
    app.add_system(bounce_system);
    app.set_extract_quads(extract_quads);

    app.run()
}
