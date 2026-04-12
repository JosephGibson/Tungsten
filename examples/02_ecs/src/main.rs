use tungsten_core::{DeltaTime, World};

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

fn main() {
    let mut world = World::new();

    world.insert_resource(DeltaTime { dt: 1.0 / 60.0 });

    for i in 0..5 {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32 * 10.0,
                y: 0.0,
            },
        );
        world.insert(
            e,
            Velocity {
                dx: (i as f32 + 1.0) * 5.0,
                dy: (i as f32 + 1.0) * 2.0,
            },
        );
    }

    println!("=== ECS Demo: 5 entities with Position + Velocity ===\n");

    for tick in 0..5 {
        movement_system(&mut world);

        println!("--- Tick {} ---", tick + 1);
        for (entity, pos) in world.query::<Position>() {
            let vel = world.get::<Velocity>(entity);
            println!(
                "  {}: pos=({:.2}, {:.2}){}",
                entity,
                pos.x,
                pos.y,
                vel.map_or(String::new(), |v| format!(
                    " vel=({:.2}, {:.2})",
                    v.dx, v.dy
                )),
            );
        }
        println!();
    }
}
