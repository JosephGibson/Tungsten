//! The toys' systems and the work behind the functions the spells call.

use glam::{Vec2, Vec3};
use tungsten::SceneEntity;
use tungsten::WindowSize;
use tungsten::core::assets::LayerKind;
use tungsten::core::{
    AnimationRegistry, AnimationState, AssetRegistry, AudioCommands, BodyKind, CameraState,
    Collider, Entity, EventCursor, EventQueue, Light, LightKind, Particle, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, Position, PrevPosition, RigidBody, ShakeEvent, Shape,
    SoundRegistry, Sprite, TilemapInstance, TilemapRegistry, Time, Transform, Velocity, Visibility,
    With, World,
};
use tungsten::kit::{
    Children, Health, LinearMotion, Modifier, PlatformerController, Projectile, ProjectileExpired,
    Stats, knock_back,
};
use tungsten::physics::wake;

use super::components::{
    Ball, BallHue, BlackHole, Element, FireMissile, Flare, IronBlock, IronScrap, Oriented,
    PreviousVelocity, Ring, ScrapTumble, ShockRing, SmallBall, Spray, SprayEmitter,
};
use super::{
    BLAST_RADIUS, BLAST_TRAUMA, FIRE_SPRAY_RANGE, FROZEN_FRICTION, HOLE_FORCE, HOLE_RADIUS,
    IRON_FRICTION, IRON_PULL, MISSILE_RADIUS, MISSILE_SIZE, PUSH_RADIUS, PUSH_SPEED, RING_SECS,
    SCRAP_PULL, SCRAP_SCALE, SPRAY_HALF_ANGLE, SPRAY_RANGE, rings_full,
};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{self, BLAST_FIRE, FROZEN, Kind, SPRAY_DPS, SPRAY_PULSE, has_status};
use crate::features::dummy::components::Dummy;
use crate::features::elements::cascade::{self, CascadeKind};
use crate::features::elements::components::{BallBurn, Frozen};
use crate::features::elements::{self, FIRE_CENTRE, FIRE_HALF, SMALL_STEAM_REACH, freeze_amount};
use crate::features::enemies::components::{Enemy, EnemyKind};
use crate::features::level::components::{Doused, Hazard};
use crate::features::level::systems::drawn_time;
use crate::features::player::components::Player;
use crate::features::player::systems::escaped;
use crate::features::run::Run;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, despawn_tree, spawn_burst};

/// The light a missile leaves where it ends, and its fades.
const FLARE_COLOR: Vec3 = Vec3::new(1.0, 0.56, 0.22);
const FLARE_RADIUS: f32 = 192.0;
const FLARE_INTENSITY: f32 = 2.4;
const FLARE_SECS: f32 = 0.3;
const BLAST_FLARE_SECS: f32 = 0.5;
/// Longest gap between a missile's contact samples along its step.
const SAMPLE_STEP: f32 = 6.0;
/// A spray's beam half width, its start from the caster's centre, and how
/// fast iron frosts and thaws under it.
const BEAM_HALF_WIDTH: f32 = 4.0;
const SPRAY_MUZZLE: f32 = 24.0;
const FREEZE_TIME: f32 = elements::FREEZE_TIME;
/// The particle configs of a cold spray and of a fire spray: the stream,
/// the mist and the contact.
const COLD_EMITTERS: [&str; 3] = ["ex10_ice_beam", "ex10_ice_mist", "ex10_ice_contact"];
const FIRE_EMITTERS: [&str; 3] = ["ex10_flame_spray", "ex10_flame_mist", "ex10_ball_burn"];
/// A spray's loop, by its element.
const COLD_LOOP: &str = "ex10_ice_spray_sfx";
const FIRE_LOOP: &str = "ex10_flame_spray_sfx";

/// A body's centre: its `Position` plus its collider's offset.
fn body_centre(world: &World, entity: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(entity)?.0;
    Some(
        at + world
            .get::<Collider>(entity)
            .map_or(Vec2::ZERO, |c| c.offset),
    )
}

/// `toys_tick`, in the tick phase: the holes' lives, the small balls'
/// impact cooldowns, the hues' turn and the shock rings' growth, each by
/// the step.
pub fn tick(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let mut closed = Vec::new();
    for (entity, hole) in world.query_mut::<(Entity, &mut BlackHole)>() {
        hole.left -= dt;
        if hole.left <= 0.0 {
            closed.push(entity);
        }
    }
    for ball in world.query_mut::<&mut SmallBall>() {
        ball.impact_cooldown = (ball.impact_cooldown - dt).max(0.0);
    }
    for hue in world.query_mut::<&mut BallHue>() {
        hue.hue = (hue.hue + hue.speed * dt).rem_euclid(1.0);
    }
    for (entity, ring) in world.query_mut::<(Entity, &mut ShockRing)>() {
        ring.age += dt;
        if ring.age >= RING_SECS {
            closed.push(entity);
        }
    }
    for entity in closed {
        despawn_tree(world, entity);
    }
}

/// `toys_velocities`, in the move phase before the physics step: each
/// body's velocity as the step starts, which impacts read after it.
pub fn remember_velocities(world: &mut World) {
    for (velocity, previous) in world.query_mut::<(&Velocity, &mut PreviousVelocity)>() {
        previous.0 = velocity.0;
    }
}

/// The open holes' centres.
#[must_use]
pub fn holes(world: &World) -> Vec<Vec2> {
    world
        .query::<(&BlackHole, &Transform)>()
        .map(|(_, at)| at.position)
        .collect()
}

/// The pull of every hole on `at`: toward each, falling linearly to 0 at
/// its reach.
#[must_use]
pub fn hole_pull(holes: &[Vec2], at: Vec2) -> Vec2 {
    let mut pull = Vec2::ZERO;
    for &hole in holes {
        let delta = hole - at;
        let distance_sq = delta.length_squared();
        if !(1.0e-4..HOLE_RADIUS * HOLE_RADIUS).contains(&distance_sq) {
            continue;
        }
        let distance = distance_sq.sqrt();
        pull += delta / distance * HOLE_FORCE * (1.0 - distance / HOLE_RADIUS);
    }
    pull
}

/// How much of a hole's pull `entity` takes: iron a fifth of a ball's,
/// scraps as balls, a frozen dummy as iron and a thawed one none, the rest
/// by their mass.
fn pull_response(world: &World, entity: Entity, inv_mass: f32) -> f32 {
    if world.has::<IronBlock>(entity) {
        IRON_PULL
    } else if world.has::<IronScrap>(entity) {
        SCRAP_PULL
    } else if world.has::<Dummy>(entity) {
        if has_status(world, entity, FROZEN) {
            IRON_PULL
        } else {
            0.0
        }
    } else {
        inv_mass
    }
}

/// The seconds of a hole's pull an enemy's mover takes each step.
pub const ENEMY_PULL_SECS: f32 = 0.25;
/// A troll within a hole's reach is slowed by this share of its speed, from
/// the source [`HOLE_SLOW_SOURCE`], since the pull barely moves it.
pub const HOLE_SLOW: f32 = 0.5;
pub const HOLE_SLOW_SOURCE: &str = "black_hole";

/// Slows each live troll within a hole's reach, and lets the others go.
fn slow_trolls(world: &mut World, holes: &[Vec2]) {
    let trolls: Vec<(Entity, bool)> = world
        .query::<(Entity, &Enemy, &Position, &Health)>()
        .filter(|(_, enemy, ..)| enemy.kind == EnemyKind::Troll)
        .map(|(entity, enemy, at, health)| {
            let centre = at.0 + enemy.kind.info().centre;
            let inside = holes
                .iter()
                .any(|&hole| hole.distance(centre) < HOLE_RADIUS);
            (entity, inside && !health.is_dead())
        })
        .collect();
    for (troll, inside) in trolls {
        if let Some(stats) = world.get_mut::<Stats>(troll) {
            stats.remove_modifiers(HOLE_SLOW_SOURCE);
            if inside {
                let slow = Modifier::Pct(-HOLE_SLOW);
                stats.add_modifier(Stats::MOVE_SPEED, HOLE_SLOW_SOURCE, slow);
            }
        }
    }
}

/// `toys_pull`, in the move phase: the holes pull every dynamic body in
/// reach by its response, and every live enemy through its mover; a troll,
/// which the pull barely moves, is slowed ([`HOLE_SLOW`]) while in reach.
pub fn pull(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let holes = holes(world);
    slow_trolls(world, &holes);
    if dt <= 0.0 || holes.is_empty() {
        return;
    }
    let pulled: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &RigidBody, &Position, &Collider)>()
        .filter(|(_, body, ..)| body.kind == BodyKind::Dynamic)
        .filter_map(|(entity, body, at, collider)| {
            // The reach first, then the body's response (P6).
            let pull = hole_pull(&holes, at.0 + collider.offset);
            if pull == Vec2::ZERO {
                return None;
            }
            let pull = pull * pull_response(world, entity, body.inv_mass);
            (pull != Vec2::ZERO).then_some((entity, pull * dt))
        })
        .collect();
    for (entity, kick) in pulled {
        if let Some(velocity) = world.get_mut::<Velocity>(entity) {
            velocity.0 += kick;
        }
        wake(world, entity);
    }
    // The enemies' kinematic bodies ride their movers, so the pull reaches
    // them as the velocity it would build over [`ENEMY_PULL_SECS`], by each
    // kind's hold: a walker's controller takes it as a push, a flyer's
    // knockback as a push it renews each step.
    let enemies: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &Enemy, &Position, &Health)>()
        .filter(|(.., health)| !health.is_dead())
        .filter_map(|(entity, enemy, at, _)| {
            let info = enemy.kind.info();
            let pull = hole_pull(&holes, at.0 + info.centre) * info.pull * ENEMY_PULL_SECS;
            (pull != Vec2::ZERO).then_some((entity, pull))
        })
        .collect();
    for (enemy, push) in enemies {
        if let Some(controller) = world.get_mut::<PlatformerController>(enemy) {
            controller.push(push);
        } else {
            knock_back(world, enemy, push);
        }
    }
}

/// `missile_bend`, in the move phase before the linear motion: the holes
/// bend each missile's flight.
pub fn bend(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let holes = holes(world);
    if dt <= 0.0 || holes.is_empty() {
        return;
    }
    let centre = Vec2::splat(MISSILE_SIZE * 0.5);
    for (_, at, motion) in world.query_mut::<(&FireMissile, &Position, &mut LinearMotion)>() {
        let velocity = motion.velocity() + hole_pull(&holes, at.0 + centre) * dt;
        motion.set_velocity(velocity);
    }
}

/// The world's maps, looked up once for many point tests: a query and a
/// registry lookup per sampled point cost more than the test itself.
pub struct SolidTiles<'w> {
    maps: Vec<(Vec2, &'w tungsten::core::TilemapData)>,
}

impl<'w> SolidTiles<'w> {
    #[must_use]
    pub fn of(world: &'w World) -> Self {
        let Some(registry) = world.get_resource::<TilemapRegistry>() else {
            return Self { maps: Vec::new() };
        };
        let maps = world
            .query::<&TilemapInstance>()
            .filter_map(|instance| Some((instance.origin, registry.get(&instance.id)?)))
            .collect();
        Self { maps }
    }

    /// Whether `point` lies in a solid collision tile of one of the maps.
    #[must_use]
    pub fn contains(&self, point: Vec2) -> bool {
        self.maps.iter().any(|&(origin, map)| {
            let tile = Vec2::new(map.tile_width as f32, map.tile_height as f32);
            let cell = ((point - origin) / tile).floor();
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
}

/// Whether `point` lies in a solid collision tile of a map in the world.
#[must_use]
pub fn solid_tile(world: &World, point: Vec2) -> bool {
    SolidTiles::of(world).contains(point)
}

/// Whether a point meets a body of `shape` about `centre`, the missile's
/// radius included.
fn touches(centre: Vec2, shape: Shape, point: Vec2, margin: f32) -> bool {
    match shape {
        Shape::Circle { radius } => point.distance_squared(centre) <= (radius + margin).powi(2),
        Shape::Aabb { half_extents } => ((point - centre).abs() - half_extents)
            .cmple(Vec2::splat(margin))
            .all(),
    }
}

/// The first sampled point on `start..end` that meets a solid tile or a
/// body other than the player's or a sensor: balls, iron, decks, lifts.
fn first_contact(world: &World, start: Vec2, end: Vec2) -> Option<Vec2> {
    let travel = end - start;
    let lead = travel.normalize_or_zero() * MISSILE_RADIUS;
    let steps = (travel.length() / SAMPLE_STEP).ceil().max(1.0) as usize;
    let solids: Vec<(Vec2, Shape)> = world
        .query::<(Entity, &Collider, &Position, &RigidBody)>()
        .filter(|(entity, _, _, body)| {
            body.kind != BodyKind::Sensor && !world.has::<Player>(*entity)
        })
        .map(|(_, collider, at, _)| (at.0 + collider.offset, collider.shape))
        .collect();
    let tiles = SolidTiles::of(world);
    (0..=steps)
        .map(|i| start + travel * (i as f32 / steps as f32))
        .find(|&point| {
            tiles.contains(point)
                || tiles.contains(point + lead)
                || solids
                    .iter()
                    .any(|&(centre, shape)| touches(centre, shape, point, MISSILE_RADIUS))
        })
}

/// Shows `entity`, a prefab spawned hidden.
pub fn show(world: &mut World, entity: Entity) {
    if let Some(visibility) = world.get_mut::<Visibility>(entity) {
        visibility.visible = true;
    }
}

/// Moves `entity` by `by`: its point, its last step's and its drawn one,
/// so nothing draws it sliding.
pub fn shift(world: &mut World, entity: Entity, by: Vec2) {
    if let Some(at) = world.get_mut::<Position>(entity) {
        at.0 += by;
    }
    if let Some(prev) = world.get_mut::<PrevPosition>(entity) {
        prev.0 += by;
    }
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.position += by;
    }
}

/// `missile_contacts`, in the overlap phase before the kit's projectile
/// step: a missile whose step met a tile or a body blasts where it met it
/// and goes, its drips with it.
pub fn missile_contacts(world: &mut World) {
    let centre = Vec2::splat(MISSILE_SIZE * 0.5);
    let missiles: Vec<(Entity, Vec2, Vec2)> = world
        .query::<(Entity, &FireMissile, &Position, &PrevPosition)>()
        .map(|(entity, _, at, prev)| (entity, prev.0 + centre, at.0 + centre))
        .collect();
    for (missile, start, end) in missiles {
        if let Some(hit) = first_contact(world, start, end) {
            flare(world, hit, true);
            despawn_tree(world, missile);
            explode(world, hit);
        }
    }
}

/// `missile_ends`, in the overlap phase after the kit's projectile step,
/// so its blast's hits take the step's resolve: a missile that step ended
/// at a wall blasts there; one that flew its range fizzles. `cursor` reads
/// each end once.
pub fn missile_ends(world: &mut World, cursor: &mut EventCursor<ProjectileExpired>) {
    let Some(events) = world.get_resource::<EventQueue<ProjectileExpired>>() else {
        return;
    };
    let centre = Vec2::splat(MISSILE_SIZE * 0.5);
    let ended: Vec<(Vec2, bool)> = cursor
        .read(events)
        .filter(|end| world.has::<FireMissile>(end.entity))
        .map(|end| {
            let spent = world
                .get::<Projectile>(end.entity)
                .is_some_and(|p| p.range().is_some_and(|range| p.travelled() >= range - 1.0));
            (end.position + centre, !spent)
        })
        .collect();
    for (at, blast) in ended {
        flare(world, at, blast);
        if blast {
            explode(world, at);
        }
    }
}

/// A missile's light left at `at`, fading; a blast's flares.
fn flare(world: &mut World, at: Vec2, blast: bool) {
    let mut light = Light::point(FLARE_COLOR, FLARE_RADIUS * if blast { 2.0 } else { 1.0 });
    light.intensity = FLARE_INTENSITY;
    let secs = if blast { BLAST_FLARE_SECS } else { FLARE_SECS };
    let phase = world.resource::<crate::shared::SaveIds>().peek() as f32;
    world.spawn_with((
        Transform::from_position(at),
        light,
        Flare {
            age: 0.0,
            secs,
            blast,
            phase,
        },
        SceneEntity { state_id: GAMEPLAY },
    ));
}

/// `toys_flares`, in the present phase: each flare fades by the frame and
/// goes; a blast's starts at four times the light and drops.
pub fn flares(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let mut done = Vec::new();
    for (entity, flare, light) in world.query_mut::<(Entity, &mut Flare, &mut Light)>() {
        flare.age += dt;
        if flare.age >= flare.secs {
            done.push(entity);
            continue;
        }
        let left = 1.0 - flare.age / flare.secs;
        let boost = if flare.blast { 4.0 * left } else { 1.0 };
        light.intensity = FLARE_INTENSITY * left * boost;
        if let LightKind::Point { radius } = &mut light.kind {
            *radius = FLARE_RADIUS * if flare.blast { 2.0 } else { 1.0 };
        }
    }
    for entity in done {
        world.despawn(entity);
    }
}

/// A fireball's blast at `at`: its fire on each live target in its heat but
/// the player who cast it, [`BLAST_FIRE`] at its centre falling to half at
/// the heat's edge, from the player.
pub fn explode(world: &mut World, at: Vec2) {
    burn_targets(world, at);
    for burst in [
        "ex10_fireball_blast",
        "ex10_blast_embers",
        "ex10_blast_dust",
        "ex10_blast_smoke",
    ] {
        spawn_burst(world, burst, at);
    }
    ring(world, at, 2.0 * PUSH_RADIUS, Ring::Fireball);
    elements::thermal_shatter(world, at, BLAST_RADIUS);
    push_bodies(world, at, PUSH_RADIUS);
    let trauma = blast_trauma(world, at);
    if trauma > 0.0
        && let Some(queue) = world.get_resource_mut::<EventQueue<ShakeEvent>>()
    {
        queue.send(ShakeEvent { trauma_add: trauma });
    }
    let mut lit: Vec<(SaveId, Entity)> = world
        .query::<(Entity, &SmallBall, &Position, &Collider, &SaveId)>()
        .filter(|(_, _, p, c, _)| (p.0 + c.offset).distance_squared(at) <= BLAST_RADIUS.powi(2))
        .map(|(entity, .., id)| (*id, entity))
        .collect();
    lit.sort_by_key(|(id, _)| *id);
    for (_, ball) in lit {
        elements::ignite(world, ball);
    }
    audio::play(world, Sound::Blasted, at);
}

/// Queues the blast's fire on each live target with a body within
/// [`BLAST_RADIUS`] of `at`, its surface's distance, in save-ID order.
fn burn_targets(world: &mut World, at: Vec2) {
    let player = world.get_resource::<Run>().map(|run| run.player);
    let mut targets: Vec<(Option<SaveId>, Entity, f32)> = world
        .query::<(Entity, &tungsten::kit::Health, &Position, &Collider)>()
        .filter(|(entity, health, ..)| !health.is_dead() && Some(*entity) != player)
        .filter_map(|(entity, _, p, collider)| {
            let distance = surface_distance(at, p.0 + collider.offset, collider.shape);
            (distance <= BLAST_RADIUS)
                .then(|| (world.get::<SaveId>(entity).copied(), entity, distance))
        })
        .collect();
    targets.sort_by_key(|(id, ..)| *id);
    for (_, target, distance) in targets {
        let amount = BLAST_FIRE * (1.0 - 0.5 * distance / BLAST_RADIUS);
        let mut hit = combat::Hit::new(target, [(Kind::Fire, amount)]).at(at);
        if let Some(player) = player {
            hit = hit.from(player);
        }
        hit.queue(world);
    }
}

/// How far `at` lies from a body of `shape` about `centre`: 0 inside it.
fn surface_distance(at: Vec2, centre: Vec2, shape: Shape) -> f32 {
    let offset = at - centre;
    match shape {
        Shape::Aabb { half_extents } => (offset.abs() - half_extents).max(Vec2::ZERO).length(),
        Shape::Circle { radius } => (offset.length() - radius).max(0.0),
    }
}

/// A shock ring at `at`, growing to `size`, unless [`RING_CAP`] live.
pub fn ring(world: &mut World, at: Vec2, size: f32, kind: Ring) {
    if rings_full(world) {
        return;
    }
    world.spawn_with((
        ShockRing {
            age: 0.0,
            size,
            kind,
        },
        Transform::from_position(at),
        SceneEntity { state_id: GAMEPLAY },
    ));
}

/// Pushes every dynamic body within `radius` of `at` away from it.
pub fn push_bodies(world: &mut World, at: Vec2, radius: f32) {
    let pushed: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &RigidBody, &Position, &Collider)>()
        .filter(|(_, body, ..)| body.kind == BodyKind::Dynamic)
        .filter_map(|(entity, body, p, collider)| {
            let offset = p.0 + collider.offset - at;
            let distance = offset.length();
            if distance >= radius {
                return None;
            }
            let falloff = 1.0 - distance / radius;
            let direction = offset.try_normalize().unwrap_or(-Vec2::Y);
            Some((entity, direction * PUSH_SPEED * falloff * body.inv_mass))
        })
        .collect();
    for (entity, kick) in pushed {
        if let Some(velocity) = world.get_mut::<Velocity>(entity) {
            velocity.0 += kick;
        }
        wake(world, entity);
    }
}

/// [`BLAST_TRAUMA`] near the view's centre, falling linearly to none a view
/// width and a half away, so a blast far off screen leaves the camera
/// still.
fn blast_trauma(world: &World, at: Vec2) -> f32 {
    let Some(camera) = world.get_resource::<CameraState>() else {
        return 0.0;
    };
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1280,
            height: 720,
        });
    let (min, max) = camera.visible_world_aabb(window.width as f32, window.height as f32);
    let width = (max.x - min.x).max(1.0);
    let beyond = at.distance((min + max) * 0.5) - 0.25 * width;
    BLAST_TRAUMA * (1.0 - beyond / (1.25 * width)).clamp(0.0, 1.0)
}

/// `toys_bounds`, in the overlap phase: a ball, block, scrap or dummy that
/// has left the level's box goes, as the player's escape ends a run.
pub fn bounds(world: &mut World) {
    let escaped: Vec<Entity> = world
        .query::<(Entity, &Position, Option<&Collider>)>()
        .filter(|(_, at, collider)| escaped(at.0, *collider))
        .filter(|(entity, ..)| {
            world.has::<Ball>(*entity)
                || world.has::<IronBlock>(*entity)
                || world.has::<IronScrap>(*entity)
                || world.has::<Dummy>(*entity)
        })
        .map(|(entity, ..)| entity)
        .collect();
    for entity in escaped {
        despawn_tree(world, entity);
    }
}

/// `iron_friction`, in the overlap phase after the physics step: the solver
/// has no friction, so iron or a dummy resting on something loses
/// horizontal speed, frozen iron or a frozen dummy less, a scrap a
/// sixteenth.
pub fn friction(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let Some(events) = world.get_resource::<EventQueue<tungsten::core::CollisionEvent>>() else {
        return;
    };
    // The few bodies it acts on, marked by their entities' slots, so a
    // contact between balls costs a bit test, not three component checks
    // (P6).
    let mut slots: Vec<u64> = Vec::new();
    let marked = world
        .query::<(Entity, &IronBlock)>()
        .map(|(entity, _)| entity)
        .chain(
            world
                .query::<(Entity, &IronScrap)>()
                .map(|(entity, _)| entity),
        )
        .chain(world.query::<(Entity, &Dummy)>().map(|(entity, _)| entity));
    for entity in marked {
        let slot = entity.id() as usize;
        if slots.len() <= slot / 64 {
            slots.resize(slot / 64 + 1, 0);
        }
        slots[slot / 64] |= 1 << (slot % 64);
    }
    let marked = |entity: Entity| {
        let slot = entity.id() as usize;
        slots
            .get(slot / 64)
            .is_some_and(|word| word >> (slot % 64) & 1 == 1)
    };
    // `normal` is the way `a` was pushed: up for a body resting on `b`.
    let supported: std::collections::HashSet<Entity> = events
        .iter_current()
        .filter_map(|event| {
            if event.normal.y < -0.5 {
                Some(event.a)
            } else if event.normal.y > 0.5 {
                event.b
            } else {
                None
            }
        })
        .filter(|&e| {
            marked(e)
                && (world.has::<IronBlock>(e) || world.has::<IronScrap>(e) || world.has::<Dummy>(e))
        })
        .collect();
    for iron in supported {
        let base = if world.has::<Frozen>(iron) || has_status(world, iron, FROZEN) {
            FROZEN_FRICTION
        } else {
            IRON_FRICTION
        };
        let friction = if world.has::<IronScrap>(iron) {
            base / 16.0
        } else {
            base
        };
        if let Some(velocity) = world.get_mut::<Velocity>(iron) {
            let speed = (velocity.0.x.abs() - friction * dt).max(0.0);
            velocity.0.x = speed.copysign(velocity.0.x);
        }
    }
}

/// The distance along `aim` from `start` at which a ray of half width
/// [`BEAM_HALF_WIDTH`] meets a body of `shape` about `centre`.
fn ray_distance(start: Vec2, aim: Vec2, centre: Vec2, shape: Shape, range: f32) -> Option<f32> {
    let offset = start - centre;
    match shape {
        Shape::Circle { radius } => {
            let radius = radius + BEAM_HALF_WIDTH;
            let c = offset.length_squared() - radius * radius;
            if c <= 0.0 {
                return Some(0.0);
            }
            let b = offset.dot(aim);
            let discriminant = b * b - c;
            if discriminant < 0.0 {
                return None;
            }
            let t = -b - discriminant.sqrt();
            (t >= 0.0).then_some(t)
        }
        Shape::Aabb { half_extents } => {
            let half = half_extents + Vec2::splat(BEAM_HALF_WIDTH);
            let (mut near, mut far) = (0.0_f32, range);
            for axis in 0..2 {
                if aim[axis].abs() < 1e-6 {
                    if offset[axis].abs() > half[axis] {
                        return None;
                    }
                } else {
                    let a = (-half[axis] - offset[axis]) / aim[axis];
                    let b = (half[axis] - offset[axis]) / aim[axis];
                    near = near.max(a.min(b));
                    far = far.min(a.max(b));
                }
            }
            (near <= far).then_some(near)
        }
    }
}

/// A body a trace can stop on: its entity, centre, shape and save ID.
#[derive(Debug, Clone, Copy)]
pub struct Traced {
    entity: Entity,
    centre: Vec2,
    shape: Shape,
    id: Option<SaveId>,
}

/// The bodies, in query order, that a trace from within `radius` of
/// `centre` could stop on, when its rays reach no farther than `radius`
/// from `centre`: all but the caster's, the player's and sensors.
#[must_use]
pub fn traced_near(world: &World, caster: Entity, centre: Vec2, radius: f32) -> Vec<Traced> {
    let reach = Vec2::splat(radius + BEAM_HALF_WIDTH);
    world
        .query::<(Entity, &Collider, &Position, &RigidBody)>()
        .filter(|(entity, _, _, body)| {
            *entity != caster && body.kind != BodyKind::Sensor && !world.has::<Player>(*entity)
        })
        .filter_map(|(entity, collider, at, _)| {
            let body = at.0 + collider.offset;
            let half = match collider.shape {
                Shape::Circle { radius } => Vec2::splat(radius),
                Shape::Aabb { half_extents } => half_extents,
            };
            ((body - centre).abs().cmple(reach + half).all()).then(|| Traced {
                entity,
                centre: body,
                shape: collider.shape,
                id: world.get::<SaveId>(entity).copied(),
            })
        })
        .collect()
}

/// Traces a ray from `start` along `aim` for `range`: where it stops, on a
/// body or a solid tile, and the body, by exact intersections and tile
/// samples four pixels apart. Ties go to the body with the lower save ID.
#[must_use]
pub fn trace(
    world: &World,
    caster: Entity,
    start: Vec2,
    aim: Vec2,
    range: f32,
) -> (Vec2, Option<Entity>) {
    let bodies = traced_near(world, caster, start, range);
    trace_among(&bodies, &SolidTiles::of(world), start, aim, range)
}

/// [`trace`] over `bodies`, which hold every body the ray can meet.
#[must_use]
pub fn trace_among(
    bodies: &[Traced],
    tiles: &SolidTiles,
    start: Vec2,
    aim: Vec2,
    range: f32,
) -> (Vec2, Option<Entity>) {
    let mut distance = range;
    let mut hit: Option<(Entity, Option<SaveId>)> = None;
    for body in bodies {
        let Some(t) = ray_distance(start, aim, body.centre, body.shape, range) else {
            continue;
        };
        let earlier = hit.is_none_or(|(_, old)| body.id < old);
        if t < distance || (t == distance && earlier) {
            distance = t;
            hit = Some((body.entity, body.id));
        }
    }
    let normal = Vec2::new(-aim.y, aim.x) * BEAM_HALF_WIDTH;
    let steps = (distance / 4.0).ceil().max(1.0) as usize;
    for i in 0..=steps {
        let t = distance * i as f32 / steps as f32;
        let at = start + aim * t;
        if [at, at + normal, at - normal]
            .into_iter()
            .any(|p| tiles.contains(p))
        {
            return (at, None);
        }
    }
    (start + aim * distance, hit.map(|(entity, _)| entity))
}

/// A spray's reach by its element.
fn reach(element: Element) -> f32 {
    match element {
        Element::Cold => SPRAY_RANGE,
        Element::Fire => FIRE_SPRAY_RANGE,
    }
}

/// The seven rays of a spray's cone along `aim`.
fn rays(aim: Vec2) -> impl Iterator<Item = Vec2> {
    (-3..=3).map(move |ray| Vec2::from_angle(ray as f32 / 3.0 * SPRAY_HALF_ANGLE).rotate(aim))
}

/// The body centre of `caster`, the spray's root.
fn caster_centre(world: &World, caster: Entity) -> Option<Vec2> {
    body_centre(world, caster)
}

/// Holds `caster`'s spray of `element` along `aim`, started when none
/// runs: kept for this step, at this aim. Its caster's centre and the aim,
/// unit length, when the caster has a body.
pub fn hold_spray(
    world: &mut World,
    caster: Entity,
    aim: Vec2,
    element: Element,
) -> Option<(Vec2, Vec2)> {
    let centre = caster_centre(world, caster)?;
    let aim = aim.try_normalize().unwrap_or(Vec2::X);
    let held = world
        .query::<(Entity, &Spray)>()
        .find(|(_, spray)| spray.caster == caster && spray.element == element)
        .map(|(entity, _)| entity);
    let entity = held.unwrap_or_else(|| start_spray(world, caster, centre, aim, element));
    if let Some(spray) = world.get_mut::<Spray>(entity) {
        spray.kept = true;
        spray.aim = aim;
    }
    Some((centre, aim))
}

/// The fire hazards a ray from `start` along `aim` crosses before `length`,
/// burning ones only, by their touch boxes.
fn fires_crossed(world: &World, start: Vec2, aim: Vec2, length: f32) -> Vec<Entity> {
    world
        .query::<(Entity, &Hazard, &Position)>()
        .filter(|(entity, hazard, _)| hazard.fire && !world.has::<Doused>(*entity))
        .filter(|(_, _, at)| {
            let shape = Shape::Aabb {
                half_extents: FIRE_HALF,
            };
            ray_distance(start, aim, at.0 + FIRE_CENTRE, shape, length).is_some_and(|t| t <= length)
        })
        .map(|(entity, ..)| entity)
        .collect()
}

/// Holds `caster`'s spray of `element` for this step. In a cold cone, iron
/// and balls chill, a burning small ball bursts into steam and a fire
/// hazard goes out for a while; in a fire cone small balls light and a
/// frozen body explodes. Every pulse of [`SPRAY_PULSE`] the live targets
/// its rays meet take the element's damage from the caster.
pub fn spray(world: &mut World, caster: Entity, aim: Vec2, element: Element) {
    let Some((centre, aim)) = hold_spray(world, caster, aim, element) else {
        return;
    };
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    // Each surface counts once, however many rays meet it.
    let mut met: Vec<(Entity, Vec2)> = Vec::new();
    let mut fires: Vec<Entity> = Vec::new();
    let bodies = traced_near(world, caster, centre, reach(element));
    let tiles = SolidTiles::of(world);
    for direction in rays(aim) {
        let (at, hit) = trace_among(&bodies, &tiles, centre, direction, reach(element));
        if element == Element::Cold {
            for fire in fires_crossed(world, centre, direction, centre.distance(at)) {
                if !fires.contains(&fire) {
                    fires.push(fire);
                }
            }
        }
        if let Some(hit) = hit
            && !met.iter().any(|&(e, _)| e == hit)
        {
            met.push((hit, at));
        }
    }
    for fire in fires {
        elements::douse_hazard(world, fire);
    }
    let pulses = world
        .query_mut::<&mut Spray>()
        .find(|spray| spray.caster == caster && spray.element == element)
        .map_or(0, |spray| {
            spray.pulse += dt;
            let mut pulses = 0;
            while spray.pulse >= SPRAY_PULSE - 1e-4 {
                spray.pulse -= SPRAY_PULSE;
                pulses += 1;
            }
            pulses
        });
    let kind = match element {
        Element::Cold => Kind::Cold,
        Element::Fire => Kind::Fire,
    };
    for &(target, contact) in &met {
        let live = world
            .get::<tungsten::kit::Health>(target)
            .is_some_and(|health| !health.is_dead());
        if !live || target == caster {
            continue;
        }
        for _ in 0..pulses {
            combat::Hit::new(target, [(kind, SPRAY_DPS * SPRAY_PULSE)])
                .from(caster)
                .at(contact)
                .queue(world);
        }
    }
    match element {
        Element::Cold => {
            for (hit, contact) in met {
                let burning = world
                    .get::<BallBurn>(hit)
                    .is_some_and(|burn| burn.remaining > 0.0);
                if burning && world.has::<SmallBall>(hit) {
                    let at = body_centre(world, hit).unwrap_or(contact);
                    cascade::queue(
                        world,
                        CascadeKind::Steam,
                        Some(hit),
                        at,
                        at,
                        SMALL_STEAM_REACH,
                    );
                } else if world.has::<IronBlock>(hit)
                    || world.has::<IronScrap>(hit)
                    || world.has::<Ball>(hit)
                {
                    elements::chill(world, hit, dt / FREEZE_TIME, contact);
                }
            }
        }
        Element::Fire => {
            for (hit, contact) in met {
                if world.has::<Frozen>(hit) {
                    let at = body_centre(world, hit).unwrap_or(contact);
                    cascade::thermal_shock(world, hit, at, at);
                } else if world.has::<SmallBall>(hit) {
                    elements::ignite(world, hit);
                }
            }
        }
    }
}

/// Starts a spray: its particles, its start sound and its loop.
fn start_spray(
    world: &mut World,
    caster: Entity,
    centre: Vec2,
    aim: Vec2,
    element: Element,
) -> Entity {
    let (end, _) = trace(world, caster, centre, aim, reach(element));
    let start = centre + aim * SPRAY_MUZZLE.min(centre.distance(end));
    let names = match element {
        Element::Cold => COLD_EMITTERS,
        Element::Fire => FIRE_EMITTERS,
    };
    let seed = world.resource::<crate::shared::SaveIds>().peek();
    let emitters = names.map(|name| {
        let config = world
            .get_resource::<ParticleConfigRegistry>()?
            .id_for_name(name)?;
        Some(world.spawn_with((
            ParticleEmitter::with_seed(config, 0x1ce0_0000 + seed),
            ParticleEmitterState::default(),
            Transform::from_position(start),
            SceneEntity { state_id: GAMEPLAY },
        )))
    });
    for emitter in emitters[..2].iter().flatten() {
        world.insert(*emitter, SprayEmitter { direction: aim });
    }
    let started = match element {
        Element::Cold => Sound::ColdStarted,
        Element::Fire => Sound::FireStarted,
    };
    audio::play(world, started, start);
    let voice = loop_voice(
        world,
        match element {
            Element::Cold => COLD_LOOP,
            Element::Fire => FIRE_LOOP,
        },
    );
    world.spawn_with((
        Spray {
            caster,
            element,
            start,
            end,
            aim,
            age: 0.0,
            emitters,
            voice,
            kept: true,
            pulse: SPRAY_PULSE,
        },
        SceneEntity { state_id: GAMEPLAY },
    ))
}

/// Starts the manifest loop `id` at its volume: a spray's voice, which its
/// end stops.
fn loop_voice(world: &mut World, id: &str) -> Option<tungsten::core::Voice> {
    let registry = world.get_resource::<SoundRegistry>()?;
    let handle = registry.get_by_id(id)?;
    let volume = registry.get_volume(handle);
    Some(
        world
            .resource_mut::<AudioCommands>()
            .play_with(handle, volume, true),
    )
}

/// Ends the spray `entity`: its particles go, its loop stops, and a cold
/// spray's caster still alive hears its end.
pub fn end_spray(world: &mut World, entity: Entity) {
    let Some(spray) = world.get::<Spray>(entity).copied() else {
        return;
    };
    for emitter in spray.emitters.into_iter().flatten() {
        world.despawn(emitter);
    }
    if let Some(voice) = spray.voice {
        world.resource_mut::<AudioCommands>().stop_voice(voice, 0.0);
    }
    let alive = world
        .get::<tungsten::kit::Health>(spray.caster)
        .is_some_and(|health| !health.is_dead());
    if alive && spray.element == Element::Cold {
        audio::play(world, Sound::ColdEnded, spray.start);
    }
    world.despawn(entity);
}

/// `sprays_end`, in the react phase: a spray no spell kept this step ends,
/// and the rest wait for the next step's spells.
pub fn sprays_end(world: &mut World) {
    let mut ended = Vec::new();
    for (entity, spray) in world.query_mut::<(Entity, &mut Spray)>() {
        if !spray.kept {
            ended.push(entity);
        }
        spray.kept = false;
    }
    let lost: Vec<Entity> = world
        .query::<(Entity, &Spray)>()
        .filter(|(_, spray)| {
            !world.is_alive(spray.caster)
                || world
                    .get::<tungsten::kit::Health>(spray.caster)
                    .is_some_and(tungsten::kit::Health::is_dead)
        })
        .map(|(entity, _)| entity)
        .collect();
    let mut seen = std::collections::HashSet::new();
    ended.extend(lost);
    ended.retain(|entity| seen.insert(*entity));
    for entity in ended {
        end_spray(world, entity);
    }
}

/// `spray_look`, in the present phase: each spray follows its caster's
/// drawn point and its aim, its stream and mist from the muzzle and its
/// contact where the cone meets a surface, iron first; newborn particles
/// turn to the aim they were born at and die before a surface.
pub fn spray_look(world: &mut World) {
    orient_newborns(world);
    let dt = world.resource::<Time>().delta();
    let sprays: Vec<(Entity, Spray)> = world
        .query::<(Entity, &Spray)>()
        .map(|(entity, spray)| (entity, *spray))
        .collect();
    for (entity, mut spray) in sprays {
        let Some(at) = world.get::<Transform>(spray.caster).map(|t| t.position) else {
            continue;
        };
        let offset = world
            .get::<Collider>(spray.caster)
            .map_or(Vec2::ZERO, |c| c.offset);
        let centre = at + offset;
        let range = reach(spray.element);
        let (end, contact) = {
            let bodies = traced_near(world, spray.caster, centre, range);
            let tiles = SolidTiles::of(world);
            let (end, _) = trace_among(&bodies, &tiles, centre, spray.aim, range);
            let contact = contact_point(world, &bodies, &tiles, centre, spray.aim, range);
            (end, contact)
        };
        spray.start = centre + spray.aim * SPRAY_MUZZLE.min(centre.distance(end));
        spray.end = end;
        spray.age += dt;
        for (index, emitter) in spray.emitters.into_iter().enumerate() {
            let Some(emitter) = emitter else { continue };
            let at = if index == 2 {
                contact.unwrap_or(end)
            } else {
                spray.start
            };
            if let Some(transform) = world.get_mut::<Transform>(emitter) {
                transform.position = at;
            }
            if let Some(aimed) = world.get_mut::<SprayEmitter>(emitter) {
                aimed.direction = spray.aim;
            }
            if index == 2
                && let Some(state) = world.get_mut::<ParticleEmitterState>(emitter)
            {
                state.drained = contact.is_none();
            }
        }
        world.insert(entity, spray);
    }
}

/// Where a spray's cone meets a surface, iron before anything else.
fn contact_point(
    world: &World,
    bodies: &[Traced],
    tiles: &SolidTiles,
    centre: Vec2,
    aim: Vec2,
    range: f32,
) -> Option<Vec2> {
    let (mut iron, mut surface): (Option<Vec2>, Option<Vec2>) = (None, None);
    let nearer = |best: Option<Vec2>, at: Vec2| {
        best.is_none_or(|p| centre.distance_squared(at) < centre.distance_squared(p))
    };
    for direction in rays(aim) {
        let (at, hit) = trace_among(bodies, tiles, centre, direction, range);
        if centre.distance(at) >= range - 0.01 {
            continue;
        }
        if nearer(surface, at) {
            surface = Some(at);
        }
        let metal = hit.is_some_and(|e| world.has::<IronBlock>(e) || world.has::<IronScrap>(e));
        if metal && nearer(iron, at) {
            iron = Some(at);
        }
    }
    iron.or(surface)
}

/// Turns each newborn spray particle to its emitter's aim and shortens its
/// life so it dies before the surface it would pass through.
fn orient_newborns(world: &mut World) {
    let tiles = SolidTiles::of(world);
    let near: Vec<(Entity, Vec<Traced>)> = world
        .query::<&Spray>()
        .map(|spray| {
            let centre = body_centre(world, spray.caster).unwrap_or(spray.start);
            let radius = SPRAY_RANGE + 2.0 * SPRAY_MUZZLE;
            (
                spray.caster,
                traced_near(world, spray.caster, centre, radius),
            )
        })
        .collect();
    let newborns: Vec<(Entity, Vec2, f32)> = world
        .query::<(Entity, &Particle, &Transform)>()
        .filter_map(|(entity, particle, at)| {
            if world.has::<Oriented>(entity) || particle.age > 0.0 {
                return None;
            }
            let aim = world.get::<SprayEmitter>(particle.emitter?)?.direction;
            let velocity = aim.rotate(particle.velocity);
            let speed = velocity.length();
            let direction = velocity.try_normalize()?;
            let caster = world
                .query::<&Spray>()
                .find(|spray| spray.emitters.contains(&particle.emitter))
                .map_or(entity, |spray| spray.caster);
            let fallback;
            let bodies = match near.iter().find(|(c, _)| *c == caster) {
                Some((_, bodies)) => bodies.as_slice(),
                None => {
                    fallback = traced_near(world, caster, at.position, SPRAY_RANGE);
                    fallback.as_slice()
                }
            };
            let (end, _) = trace_among(
                bodies,
                &tiles,
                at.position,
                direction,
                SPRAY_RANGE - SPRAY_MUZZLE,
            );
            let lifetime = (at.position.distance(end) - BEAM_HALF_WIDTH).max(0.0) / speed;
            Some((entity, velocity, lifetime))
        })
        .collect();
    for (entity, velocity, lifetime) in newborns {
        if let Some(particle) = world.get_mut::<Particle>(entity) {
            particle.velocity = velocity;
            particle.lifetime = particle.lifetime.min(lifetime);
        }
        world.insert(entity, Oriented);
    }
}

/// `ball_look`, in the present phase: a frozen or chilled ball takes the
/// frost's tint, as deep as the frost; a burning or spent small ball its
/// ember tint, another small ball its rainbow hue, and a bowling ball its
/// own colours.
pub fn ball_look(world: &mut World) {
    let time = drawn_time(world);
    for (_, id, sprite, burn, hue, frozen, chill) in world.query_mut::<(
        &Ball,
        Option<&SaveId>,
        &mut Sprite,
        Option<&BallBurn>,
        Option<&BallHue>,
        Option<&Frozen>,
        Option<&crate::features::elements::components::Chill>,
    )>() {
        // `freeze_amount` from the row; the frost wins, as it wrote last.
        let frost = if frozen.is_some() {
            1.0
        } else {
            chill.map_or(0.0, |chill| chill.amount)
        };
        if frost > 0.0 {
            sprite.color = frost_tint(frost);
            continue;
        }
        let Some(id) = id else { continue };
        sprite.color = match (burn, hue) {
            (Some(burn), _) => elements::ember_tint(*id, *burn, time),
            (None, Some(hue)) => rainbow_rgba(hue.hue),
            (None, None) => [255; 4],
        };
    }
}

/// A full-saturation rainbow colour at `hue`, in turns: one channel at
/// 255, one at 0 and the third ramping between them, its ramp lifted so
/// the secondary hues read as bright.
#[allow(clippy::many_single_char_names)] // h/x/r/g/b in the hue-to-RGB math
#[must_use]
pub fn rainbow_rgba(hue: f32) -> [u8; 4] {
    let h = hue.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let x = (x.powf(0.55) * 255.0).round().clamp(0.0, 255.0) as u8;
    let (r, g, b) = match h as u32 {
        0 => (255, x, 0),
        1 => (x, 255, 0),
        2 => (0, 255, x),
        3 => (0, x, 255),
        4 => (x, 0, 255),
        _ => (255, 0, x),
    };
    [r, g, b, 255]
}

/// The tint frost gives iron: blue as it takes hold.
#[must_use]
pub fn frost_tint(amount: f32) -> [u8; 4] {
    [
        (255.0 - 155.0 * amount) as u8,
        (255.0 - 60.0 * amount) as u8,
        255,
        255,
    ]
}

/// The four broken-metal sprites and their frost.
const SCRAP_SPRITES: [&str; 4] = [
    "ex10_iron_scrap_0",
    "ex10_iron_scrap_1",
    "ex10_iron_scrap_2",
    "ex10_iron_scrap_3",
];
const SCRAP_FROST_SPRITES: [&str; 4] = [
    "ex10_iron_scrap_frost_0",
    "ex10_iron_scrap_frost_1",
    "ex10_iron_scrap_frost_2",
    "ex10_iron_scrap_frost_3",
];

/// Shows a scrap's broken-metal sprite of `variant`.
pub fn show_scrap_variant(world: &mut World, scrap: Entity, variant: usize) {
    let id = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(SCRAP_SPRITES[variant % 4]);
    if let Some(sprite) = world.get_mut::<Sprite>(scrap) {
        sprite.asset_id = id;
    }
}

/// `iron_look`, in the present phase: iron takes the frost's tint, and its
/// frost art shows as deep as the frost, over the metal; a scrap tumbles
/// as it flies, its frost turning with it.
pub fn iron_look(world: &mut World) {
    let time = drawn_time(world);
    let blocks: Vec<(Entity, f32, Vec<Entity>)> = world
        .query::<(Entity, &IronBlock, &Children)>()
        .map(|(entity, _, children)| {
            (
                entity,
                freeze_amount(world, entity),
                children.iter().collect(),
            )
        })
        .collect();
    for (_, amount, children) in blocks {
        for (index, child) in children.into_iter().enumerate() {
            let frost = index >= 4;
            tint(world, child, amount, frost);
        }
    }
    let scraps: Vec<(Entity, f32, usize, f32, Vec<Entity>)> = world
        .query::<(Entity, &IronScrap, Option<&ScrapTumble>, Option<&Children>)>()
        .map(|(entity, scrap, tumble, children)| {
            let turn = tumble.map_or(0.0, |tumble| tumble.rotation(time));
            let children = children.map(|c| c.iter().collect()).unwrap_or_default();
            (
                entity,
                freeze_amount(world, entity),
                scrap.variant,
                turn,
                children,
            )
        })
        .collect();
    for (scrap, amount, variant, turn, children) in scraps {
        if let Some(sprite) = world.get_mut::<Sprite>(scrap) {
            sprite.color = frost_tint(amount);
        }
        if let Some(transform) = world.get_mut::<Transform>(scrap) {
            transform.rotation = turn;
        }
        for child in children {
            let id = world
                .resource_mut::<AssetRegistry>()
                .intern_sprite(SCRAP_FROST_SPRITES[variant % 4]);
            if let Some(sprite) = world.get_mut::<Sprite>(child) {
                sprite.asset_id = id;
            }
            if let Some(transform) = world.get_mut::<Transform>(child) {
                transform.rotation = turn;
                transform.scale = Vec2::splat(SCRAP_SCALE);
            }
            tint(world, child, amount, true);
        }
    }
}

/// Tints one of iron's art pieces: the metal by the frost, the frost art
/// shown only while there is frost, as opaque as it is deep.
fn tint(world: &mut World, piece: Entity, amount: f32, frost: bool) {
    if let Some(sprite) = world.get_mut::<Sprite>(piece) {
        sprite.color = if frost {
            [255, 255, 255, (255.0 * amount) as u8]
        } else {
            frost_tint(amount)
        };
    }
    if frost && let Some(visibility) = world.get_mut::<Visibility>(piece) {
        visibility.visible = amount > 0.0;
    }
}

/// `missile_look`, in the present phase: a missile's art faces along its
/// flight, mirrored when it flies left so its drips stay underneath.
pub fn missile_look(world: &mut World) {
    let Some(registry) = world.remove_resource::<AnimationRegistry>() else {
        return;
    };
    let missiles: Vec<(Entity, Vec2, String)> = world
        .query::<(Entity, &FireMissile, &LinearMotion, &AnimationState)>()
        .map(|(entity, _, motion, state)| (entity, motion.velocity(), state.animation_id.clone()))
        .collect();
    for (missile, v, clip) in missiles {
        let left = v.x < 0.0;
        let wanted = if left {
            "ex10_fireball_left"
        } else {
            "ex10_fireball"
        };
        if clip != wanted {
            let mut state = AnimationState::new(wanted);
            if let Some(old) = world.get::<AnimationState>(missile) {
                state.frame_index = old.frame_index;
                state.accumulated_ms = old.accumulated_ms;
            }
            let frame = state.current_sprite(&registry);
            world.insert(missile, state);
            if let (Some(frame), Some(sprite)) = (frame, world.get_mut::<Sprite>(missile)) {
                sprite.asset_id = frame;
            }
        }
        if let Some(transform) = world.get_mut::<Transform>(missile) {
            transform.rotation = if left {
                (-v.y).atan2(-v.x)
            } else {
                v.y.atan2(v.x)
            };
        }
    }
    world.insert_resource(registry);
}

/// `hole_particles`, in the present phase: a hole's newborn particles are
/// set on a ring about it, and every one of its particles is aimed at the
/// next point of a spiral inward, so the dust reads as accretion; one at
/// the core is gone.
pub fn hole_particles(world: &mut World) {
    // No hole open, no particle of its to steer (P6).
    if world.query::<&BlackHole>().next().is_none() {
        return;
    }
    let dt = world.resource::<Time>().delta();
    let steered: Vec<(Entity, Vec2, bool, Vec2)> = world
        .query::<(Entity, &Particle, &Transform)>()
        .filter_map(|(entity, particle, at)| {
            let emitter = particle.emitter?;
            let owner = world
                .get::<tungsten::kit::Parent>(emitter)
                .map_or(emitter, |parent| parent.entity());
            world.get::<BlackHole>(owner)?;
            let centre = world.get::<Transform>(owner)?.position;
            let position = if particle.age == 0.0 {
                centre
                    + particle.velocity.normalize_or_zero()
                        * (110.0 + particle.velocity.length() * 0.25)
            } else {
                at.position
            };
            let delta = position - centre;
            let radial = delta.normalize_or_zero();
            let tangent_speed = (32_000.0 / delta.length().max(50.0)).clamp(180.0, 640.0);
            // Aim at the next point on the spiral: tangential speed alone
            // drifts outward near the core at low frame rates.
            let velocity = if dt > 0.0 {
                let (sin, cos) = (tangent_speed / delta.length().max(14.0) * dt).sin_cos();
                let next = Vec2::new(
                    radial.x * cos - radial.y * sin,
                    radial.x * sin + radial.y * cos,
                );
                (next * (delta.length() - 105.0 * dt).max(0.0) - delta) / dt
            } else {
                particle.velocity
            };
            Some((entity, position, delta.length() < 14.0, velocity))
        })
        .collect();
    for (entity, position, absorbed, velocity) in steered {
        if let Some(at) = world.get_mut::<Transform>(entity) {
            at.position = position;
        }
        if let Some(particle) = world.get_mut::<Particle>(entity) {
            particle.velocity = velocity;
            if absorbed {
                particle.age = particle.lifetime;
            }
        }
    }
}

/// The bodies a query of `With<Ball>` holds, for tests.
#[must_use]
pub fn balls(world: &World) -> Vec<Entity> {
    world.query_filtered::<Entity, With<Ball>>().collect()
}
