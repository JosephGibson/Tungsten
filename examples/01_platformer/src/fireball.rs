//! Mouse 4 fireball spell: a missile launched from the player toward the cursor.
//! It falls under low gravity, bends around black holes and explodes on the
//! first solid it touches, igniting nearby small balls.
use glam::Vec2;
use tungsten::core::assets::LayerKind;
use tungsten::core::{
    ActionMap, AnimationState, CameraState, Entity, InputState, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, TilemapInstance, TilemapRegistry, Time, Transform,
    World,
};
use tungsten::physics::{Collider, Position, Shape};

use crate::gameplay::{EmitterAnchor, Explosion};
use crate::state::{
    CurrentSprite, GRAVITY_Y, Player, PlayerPresentation, SmallBall, TRANSIENT_EMITTER_CAP,
    WORLD_BOUNDS_MAX, WORLD_BOUNDS_MIN,
};
use crate::systems::{
    black_hole_acceleration, black_hole_positions, cursor_to_world, play_effect_sound,
    spawn_transient_effect,
};

pub(crate) const FIREBALL_SPEED: f32 = 900.0;
/// A tenth of world gravity: the missile flies nearly straight but still arcs.
pub(crate) const FIREBALL_GRAVITY: f32 = GRAVITY_Y * 0.1;
pub(crate) const FIREBALL_RADIUS: f32 = 10.0;
pub(crate) const FIREBALL_LIFETIME: f32 = 1.4;
pub(crate) const FIREBALL_COOLDOWN: f32 = 0.2;
pub(crate) const FIREBALL_MAX_ALIVE: usize = 6;
pub(crate) const FIREBALL_BLAST_RADIUS: f32 = 72.0;
pub(crate) const FIREBALL_VISUAL_SIZE: f32 = 40.0;
/// Launch distance from the player's centre along the aim, near the body edge.
const MUZZLE_OFFSET: f32 = 24.0;
/// Longest gap between contact samples along one frame's travel.
const SAMPLE_STEP: f32 = 6.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FireballMissile {
    pub(crate) velocity: Vec2,
    pub(crate) age: f32,
    /// Anchored molten-drip emitter that despawns with the missile.
    pub(crate) drips: Option<Entity>,
}

/// One missile per fresh press, capped in count and rate.
pub(crate) fn cast_fireball_system(world: &mut World) {
    let pressed = world
        .get_resource::<InputState>()
        .zip(world.get_resource::<ActionMap>())
        .is_some_and(|(input, actions)| actions.just_pressed(input, "cast_fireball"));
    if !pressed {
        return;
    }
    let ages: Vec<f32> = world
        .query::<(Entity, &FireballMissile)>()
        .map(|(_, m)| m.age)
        .collect();
    if ages.len() >= FIREBALL_MAX_ALIVE || ages.iter().any(|&age| age < FIREBALL_COOLDOWN) {
        return;
    }
    let Some(player) = world.query::<(Entity, &Player)>().next().map(|(e, _)| e) else {
        return;
    };
    let Some(center) = world.get::<Position>(player).map(|p| p.0) else {
        return;
    };
    let facing = if world
        .get::<PlayerPresentation>(player)
        .is_some_and(|p| p.facing_left)
    {
        -Vec2::X
    } else {
        Vec2::X
    };
    let target = world
        .get_resource::<InputState>()
        .and_then(InputState::cursor_position)
        .zip(world.get_resource::<CameraState>())
        .and_then(|((x, y), camera)| cursor_to_world(Vec2::new(x, y), camera));
    let aim = target.map_or(facing, |t| (t - center).try_normalize().unwrap_or(facing));
    spawn_fireball(world, center + aim * MUZZLE_OFFSET, aim * FIREBALL_SPEED);
    play_effect_sound(world, |s| s.cast);
}

pub(crate) fn spawn_fireball(world: &mut World, position: Vec2, velocity: Vec2) -> Entity {
    let registry = world.get_resource::<ParticleConfigRegistry>();
    let trail = registry.and_then(|r| r.id_for_name("ex10_spell_trail"));
    let drip_config = registry.and_then(|r| r.id_for_name("ex10_fireball_drips"));
    let entity = world.spawn();
    world.insert(entity, Position(position));
    world.insert(entity, Transform::from_position(position));
    world.insert(entity, CurrentSprite("ex10_fireball_0".into()));
    world.insert(entity, AnimationState::new("ex10_fireball"));
    if let Some(config) = trail {
        world.insert(
            entity,
            ParticleEmitter::with_seed(config, 0x5f_0000 + entity.id() as u64),
        );
        world.insert(entity, ParticleEmitterState::default());
    }
    let drips = drip_config.map(|config| {
        let drips = world.spawn();
        world.insert(
            drips,
            EmitterAnchor {
                parent: entity,
                offset: Vec2::ZERO,
            },
        );
        world.insert(drips, Transform::from_position(position));
        world.insert(
            drips,
            ParticleEmitter::with_seed(config, 0x5f_8000 + drips.id() as u64),
        );
        world.insert(drips, ParticleEmitterState::default());
        drips
    });
    world.insert(
        entity,
        FireballMissile {
            velocity,
            age: 0.0,
            drips,
        },
    );
    entity
}

/// Runs after physics so contacts see this frame's ball positions, and before
/// fire spreads so a blast's ignitions spread in the same frame.
pub(crate) fn fireball_flight_system(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    if dt <= 0.0 {
        return;
    }
    let holes = black_hole_positions(world);
    let missiles: Vec<_> = world
        .query::<(Entity, &FireballMissile)>()
        .map(|(e, m)| (e, *m))
        .collect();
    for (entity, mut missile) in missiles {
        let Some(start) = world.get::<Position>(entity).map(|p| p.0) else {
            continue;
        };
        let gravity = Vec2::new(0.0, FIREBALL_GRAVITY);
        missile.velocity += (gravity + black_hole_acceleration(&holes, start)) * dt;
        missile.age += dt;
        let end = start + missile.velocity * dt;
        if let Some(hit) = first_contact(world, start, end) {
            explode_fireball(world, hit);
            despawn_fireball(world, entity, missile);
            continue;
        }
        if missile.age >= FIREBALL_LIFETIME
            || end.cmplt(WORLD_BOUNDS_MIN).any()
            || end.cmpgt(WORLD_BOUNDS_MAX).any()
        {
            despawn_fireball(world, entity, missile);
            continue;
        }
        for e in [Some(entity), missile.drips].into_iter().flatten() {
            if let Some(transform) = world.get_mut::<Transform>(e) {
                transform.position = end;
            }
        }
        if let Some(position) = world.get_mut::<Position>(entity) {
            position.0 = end;
        }
        if let Some(state) = world.get_mut::<FireballMissile>(entity) {
            *state = missile;
        }
    }
}

fn despawn_fireball(world: &mut World, entity: Entity, missile: FireballMissile) {
    if let Some(drips) = missile.drips {
        world.despawn(drips);
    }
    world.despawn(entity);
}

/// First sampled point on `start..end` touching any collider except the
/// player's (balls, decks, lifts) or a solid collision tile.
fn first_contact(world: &World, start: Vec2, end: Vec2) -> Option<Vec2> {
    let travel = end - start;
    let lead = travel.normalize_or_zero() * FIREBALL_RADIUS;
    let steps = (travel.length() / SAMPLE_STEP).ceil().max(1.0) as usize;
    let solids: Vec<(Vec2, Shape)> = world
        .query::<(Entity, &Collider)>()
        .filter(|(e, _)| world.get::<Player>(*e).is_none())
        .filter_map(|(e, c)| Some((world.get::<Position>(e)?.0 + c.offset, c.shape)))
        .collect();
    (0..=steps)
        .map(|i| start + travel * (i as f32 / steps as f32))
        .find(|&p| {
            solid_tile(world, p)
                || solid_tile(world, p + lead)
                || solids
                    .iter()
                    .any(|&(center, shape)| touches(center, shape, p))
        })
}

fn touches(center: Vec2, shape: Shape, point: Vec2) -> bool {
    match shape {
        Shape::Circle { radius } => {
            point.distance_squared(center) <= (radius + FIREBALL_RADIUS).powi(2)
        }
        Shape::Aabb { half_extents } => ((point - center).abs() - half_extents)
            .cmple(Vec2::splat(FIREBALL_RADIUS))
            .all(),
    }
}

fn solid_tile(world: &World, point: Vec2) -> bool {
    let Some(registry) = world.get_resource::<TilemapRegistry>() else {
        return false;
    };
    world
        .query::<(Entity, &TilemapInstance)>()
        .any(|(_, instance)| {
            let Some(map) = registry.get(&instance.id) else {
                return false;
            };
            let tile = Vec2::new(map.tile_width as f32, map.tile_height as f32);
            let cell = ((point - instance.origin) / tile).floor();
            if cell.x < 0.0
                || cell.y < 0.0
                || cell.x >= map.width as f32
                || cell.y >= map.height as f32
            {
                return false;
            }
            let index = cell.y as usize * map.width as usize + cell.x as usize;
            map.layers
                .iter()
                .any(|layer| layer.kind == LayerKind::Collision && layer.tiles[index] >= 0)
        })
}

/// Flame bloom, sparks and a shock ring; small balls in the blast catch fire.
/// The blast neither pushes bodies nor hurts the player.
pub(crate) fn explode_fireball(world: &mut World, at: Vec2) {
    spawn_transient_effect(world, "ex10_fireball_blast", at);
    spawn_transient_effect(world, "ex10_ball_explosion", at);
    if world.query::<(Entity, &Explosion)>().count() < TRANSIENT_EMITTER_CAP {
        let e = world.spawn();
        world.insert(e, Explosion { age: 0.0 });
        world.insert(e, Transform::from_position(at));
    }
    let mut targets: Vec<Entity> = world
        .query::<(Entity, &SmallBall)>()
        .filter(|(e, _)| {
            world
                .get::<Position>(*e)
                .is_some_and(|p| p.0.distance_squared(at) <= FIREBALL_BLAST_RADIUS.powi(2))
        })
        .map(|(e, _)| e)
        .collect();
    targets.sort_by_key(|e| e.id());
    for ball in targets {
        crate::burning::ignite(world, ball);
    }
    play_effect_sound(world, |s| s.blast);
}
