//! Mouse 4 fireball spell: a missile launched from the player toward the cursor.
//! It falls under low gravity, bends around black holes, carries a small
//! light and explodes on the first solid it touches: the blast pushes every
//! dynamic body in reach away and ignites nearby small balls.
use glam::{Vec2, Vec3};
use tungsten::WindowSize;
use tungsten::core::assets::LayerKind;
use tungsten::core::{
    ActionMap, AnimationState, CameraState, Entity, EventQueue, InputState, Light, LightKind,
    ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, ShakeEvent, TilemapInstance,
    TilemapRegistry, Time, Transform, World,
};
use tungsten::physics::{BodyKind, Collider, Position, PrevPosition, RigidBody, Shape, Velocity};

use crate::gameplay::{EmitterAnchor, Explosion, Health};
use crate::state::{
    CurrentSprite, GRAVITY_Y, Player, PlayerPresentation, SmallBall, TILE, TRANSIENT_EMITTER_CAP,
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
pub(crate) const FIREBALL_LIFETIME: f32 = 2.1;
pub(crate) const FIREBALL_COOLDOWN: f32 = 0.2;
pub(crate) const FIREBALL_MAX_ALIVE: usize = 6;
pub(crate) const FIREBALL_BLAST_RADIUS: f32 = 72.0;
/// Reach of the blast's push: a pile crater a little wider than the player is tall.
pub(crate) const FIREBALL_PUSH_RADIUS: f32 = 2.5 * TILE;
/// Speed the push gives a unit-mass body (a ball, the player) at the blast
/// centre, pixels/second: falling linearly to zero at the reach, divided by
/// mass, so the iron brick barely moves. Close to the player's jump.
pub(crate) const FIREBALL_PUSH_SPEED: f32 = 1100.0;
/// Seconds of lost control at full push, so the shove is not walked off at once.
const PUSH_CONTROL_LOCK: f32 = 0.15;
/// Camera trauma of a blast within a quarter view width of the view's
/// centre, falling to none a view width and a half away. The shake grows
/// with its square: about 4 pixels at most, for a quarter second.
pub(crate) const BLAST_TRAUMA: f32 = 0.55;
const TRAIL_LIGHT_COLOR: Vec3 = Vec3::new(1.0, 0.56, 0.22);
// Bright enough to warm the dark, low-albedo masonry it passes.
const TRAIL_LIGHT_RADIUS: f32 = 3.0 * TILE;
const TRAIL_LIGHT_INTENSITY: f32 = 2.4;
/// Fade after the missile burns out, and after it explodes (with a flare).
const TRAIL_LIGHT_FADE: f32 = 0.3;
const TRAIL_LIGHT_BLAST_FADE: f32 = 0.5;
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
    /// The missile's `TrailLight`, which outlives it to fade.
    pub(crate) light: Option<Entity>,
}

/// The one light of a fireball: it follows the missile while it flies, then
/// stays where the missile ended and fades out.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TrailLight {
    pub(crate) missile: Entity,
    /// Seconds since the missile ended; `None` while it flies.
    pub(crate) fading: Option<f32>,
    /// The missile exploded: the light flares and fades more slowly.
    pub(crate) blast: bool,
}

/// One missile per fresh press, capped in count and rate.
pub(crate) fn cast_fireball_system(world: &mut World) {
    if crate::death::player_dead(world) {
        return;
    }
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
    world.insert(entity, PrevPosition(position));
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
    let light = world.spawn();
    let mut glow = Light::point(TRAIL_LIGHT_COLOR, TRAIL_LIGHT_RADIUS);
    glow.intensity = TRAIL_LIGHT_INTENSITY;
    world.insert(light, glow);
    world.insert(light, Transform::from_position(position));
    world.insert(
        light,
        TrailLight {
            missile: entity,
            fading: None,
            blast: false,
        },
    );
    world.insert(
        entity,
        FireballMissile {
            velocity,
            age: 0.0,
            drips,
            light: Some(light),
        },
    );
    entity
}

/// Keeps each trail light on its missile's drawn point with a quick
/// flicker, then fades it out once the missile is gone. Runs after the
/// physics sync has drawn the missiles.
pub(crate) fn fireball_light_system(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let time = crate::extract::drawn_scene_time(world);
    let lights: Vec<_> = world
        .query::<(Entity, &TrailLight)>()
        .map(|(e, l)| (e, *l))
        .collect();
    for (entity, mut light) in lights {
        let missile = world
            .get::<FireballMissile>(light.missile)
            .and_then(|_| world.get::<Transform>(light.missile))
            .map(|t| t.position);
        if missile.is_none() && light.fading.is_none() {
            light.fading = Some(0.0);
        }
        let intensity = match light.fading {
            None => TRAIL_LIGHT_INTENSITY * (1.0 + 0.12 * (time * 31.0 + entity.id() as f32).sin()),
            Some(elapsed) => {
                let elapsed = elapsed + dt;
                light.fading = Some(elapsed);
                let secs = if light.blast {
                    TRAIL_LIGHT_BLAST_FADE
                } else {
                    TRAIL_LIGHT_FADE
                };
                if elapsed >= secs {
                    world.despawn(entity);
                    continue;
                }
                let left = 1.0 - elapsed / secs;
                // A blast flares to over twice the flight brightness, then drops.
                let flare = if light.blast { 2.4 * left } else { 1.0 };
                TRAIL_LIGHT_INTENSITY * left * flare
            }
        };
        if let Some(position) = missile
            && let Some(transform) = world.get_mut::<Transform>(entity)
        {
            transform.position = position;
        }
        if let Some(value) = world.get_mut::<Light>(entity) {
            value.intensity = intensity;
            if let LightKind::Point { radius } = &mut value.kind {
                *radius = TRAIL_LIGHT_RADIUS * if light.blast { 1.5 } else { 1.0 };
            }
        }
        *world.get_mut::<TrailLight>(entity).unwrap() = light;
    }
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
            despawn_fireball(world, entity, missile, true);
            continue;
        }
        if missile.age >= FIREBALL_LIFETIME
            || end.cmplt(WORLD_BOUNDS_MIN).any()
            || end.cmpgt(WORLD_BOUNDS_MAX).any()
        {
            despawn_fireball(world, entity, missile, false);
            continue;
        }
        // `physics_sync` draws the missile and `anchor_emitters` its drips.
        if let Some(position) = world.get_mut::<Position>(entity) {
            position.0 = end;
        }
        if let Some(state) = world.get_mut::<FireballMissile>(entity) {
            *state = missile;
        }
    }
}

/// Removes the missile and its drips; its light stays to fade, flaring if
/// the missile `exploded`.
fn despawn_fireball(world: &mut World, entity: Entity, missile: FireballMissile, exploded: bool) {
    if let Some(drips) = missile.drips {
        world.despawn(drips);
    }
    if let Some(light) = missile.light.and_then(|l| world.get_mut::<TrailLight>(l)) {
        light.fading = Some(0.0);
        light.blast = exploded;
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

/// Flame bloom, sparks, a dust ring and a shock ring as wide as the push;
/// every dynamic body in reach is pushed away, small balls in the blast
/// catch fire and the camera shakes with the blast's nearness. The blast
/// does not hurt the player.
pub(crate) fn explode_fireball(world: &mut World, at: Vec2) {
    spawn_transient_effect(world, "ex10_fireball_blast", at);
    spawn_transient_effect(world, "ex10_ball_explosion", at);
    spawn_transient_effect(world, "ex10_blast_dust", at);
    if world.query::<(Entity, &Explosion)>().count() < TRANSIENT_EMITTER_CAP {
        let e = world.spawn();
        world.insert(
            e,
            Explosion {
                age: 0.0,
                size: 2.0 * FIREBALL_PUSH_RADIUS,
            },
        );
        world.insert(e, Transform::from_position(at));
    }
    push_bodies(world, at);
    let trauma = blast_trauma(world, at);
    if trauma > 0.0
        && let Some(queue) = world.get_resource_mut::<EventQueue<ShakeEvent>>()
    {
        queue.send(ShakeEvent { trauma_add: trauma });
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

/// Pushes every dynamic body within `FIREBALL_PUSH_RADIUS` of `at` straight
/// away from it (up at the centre): `FIREBALL_PUSH_SPEED` at the centre,
/// linearly less with distance, divided by the body's mass. A pushed player
/// briefly loses control, so input does not cancel the shove.
pub(crate) fn push_bodies(world: &mut World, at: Vec2) {
    let pushed: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &RigidBody, &Position)>()
        .filter(|(_, body, _)| body.kind == BodyKind::Dynamic)
        .filter_map(|(e, body, p)| {
            let offset = p.0 - at;
            let distance = offset.length();
            if distance >= FIREBALL_PUSH_RADIUS {
                return None;
            }
            let falloff = 1.0 - distance / FIREBALL_PUSH_RADIUS;
            let direction = offset.try_normalize().unwrap_or(-Vec2::Y);
            Some((e, direction * FIREBALL_PUSH_SPEED * falloff * body.inv_mass))
        })
        .collect();
    for (entity, kick) in pushed {
        let Some(velocity) = world.get_mut::<Velocity>(entity) else {
            continue;
        };
        velocity.0 += kick;
        tungsten::physics::wake(world, entity);
        if let Some(health) = world.get_mut::<Health>(entity) {
            let lock = PUSH_CONTROL_LOCK * kick.length() / FIREBALL_PUSH_SPEED;
            health.control_lock = health.control_lock.max(lock);
        }
    }
}

/// `BLAST_TRAUMA` near the view's centre, falling linearly to none a view
/// width and a half away, so a blast far off screen leaves the camera still.
fn blast_trauma(world: &World, at: Vec2) -> f32 {
    let Some(camera) = world.get_resource::<CameraState>() else {
        return 0.0;
    };
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1920,
            height: 1080,
        });
    let (min, max) = camera.visible_world_aabb(window.width as f32, window.height as f32);
    let width = (max.x - min.x).max(1.0);
    let beyond = at.distance((min + max) * 0.5) - 0.25 * width;
    BLAST_TRAUMA * (1.0 - beyond / (1.25 * width)).clamp(0.0, 1.0)
}
