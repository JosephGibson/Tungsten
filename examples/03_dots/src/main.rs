use tungsten::core::{Config, DeltaTime, Entity, InputState, KeyCode, MouseButton, World};
use tungsten::render::QuadInstance;
use tungsten::App;

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
    let entities = world.query_entities::<Velocity>();
    for entity in entities {
        let size = world
            .get::<Size>(entity)
            .map(|s| (s.w, s.h))
            .unwrap_or((10.0, 10.0));
        let pos = world.get::<Position>(entity).unwrap().clone();

        let mut vel = world.get::<Velocity>(entity).unwrap().clone();
        let mut bounced = false;

        if pos.x < 0.0 || pos.x + size.0 > 1280.0 {
            vel.dx = -vel.dx;
            bounced = true;
        }
        if pos.y < 0.0 || pos.y + size.1 > 720.0 {
            vel.dy = -vel.dy;
            bounced = true;
        }

        if bounced {
            *world.get_mut::<Velocity>(entity).unwrap() = vel;
            let pos = world.get_mut::<Position>(entity).unwrap();
            pos.x = pos.x.clamp(0.0, 1280.0 - size.0);
            pos.y = pos.y.clamp(0.0, 720.0 - size.1);
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

fn spawn_dot(
    world: &mut World,
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    r: f32,
    g: f32,
    b: f32,
    w: f32,
    h: f32,
) -> Entity {
    let e = world.spawn();
    world.insert(e, Position { x, y });
    world.insert(e, Velocity { dx, dy });
    world.insert(e, Color { r, g, b, a: 1.0 });
    world.insert(e, Size { w, h });
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
    spawn_dot(world, 100.0, 100.0, 200.0, 150.0, 1.0, 0.3, 0.3, 20.0, 20.0);
    spawn_dot(
        world, 300.0, 200.0, -180.0, 220.0, 0.3, 1.0, 0.3, 15.0, 15.0,
    );
    spawn_dot(
        world, 500.0, 300.0, 250.0, -100.0, 0.3, 0.3, 1.0, 25.0, 25.0,
    );
    spawn_dot(
        world, 700.0, 400.0, -120.0, 180.0, 1.0, 1.0, 0.3, 12.0, 12.0,
    );
    spawn_dot(
        world, 200.0, 500.0, 160.0, -200.0, 1.0, 0.3, 1.0, 18.0, 18.0,
    );

    app.add_system(player_control_system);
    app.add_system(click_spawn_system);
    app.add_system(movement_system);
    app.add_system(bounce_system);
    app.set_extract_quads(extract_quads);

    app.run()
}
