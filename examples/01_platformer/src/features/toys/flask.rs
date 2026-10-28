//! The toxic flask (e01flask, V39): a glass flask of poison the wizard lobs
//! at the cursor on the world's gravity ([`throw`]). Its `toxic_flask`
//! prefab is the fire missile's kit projectile on its linear motion, which
//! is exact under a constant fall, so it passes through the cursor; it has
//! no body, so nothing pushes it and it passes through the wizard. It
//! bursts where its step first meets a tile or a body but the wizard's
//! ([`contacts`]), where the kit's projectile step ends it at a wall, or at
//! its range's end in the air ([`ends`]): a splash of poison on the living
//! in reach ([`SPLASH_RADIUS`]). At most [`FLASK_CAP`] fly at once.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AnimationState, AssetRegistry, Collider, Entity, EventCursor, EventQueue, Position,
    PrevPosition, Sprite, Transform, Visibility, World,
};
use tungsten::kit::{Health, Lifetime, LinearMotion, ProjectileExpired, Source, Tags};

use super::components::ToxicFlask;
use super::systems::first_contact;
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{Hit, Kind, LIVING};
use crate::features::enemies::troll;
use crate::features::level::GRAVITY;
use crate::features::run::Run;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, despawn_tree, spawn_burst, surface_distance};

/// Its prefab, its radius, its centre from its sprite's top-left and the
/// most that fly at once, past which a toss is refused.
pub const FLASK_PREFAB: &str = "toxic_flask";
pub const FLASK_RADIUS: f32 = 5.0;
pub const FLASK_CENTRE: Vec2 = Vec2::splat(8.0);
pub const FLASK_CAP: usize = 3;
/// Its flight to the cursor: `sqrt(2 d / g)` seconds over the distance `d`,
/// within these (Q10).
pub const FLIGHT_LEAST: f32 = 0.25;
pub const FLIGHT_MOST: f32 = 0.9;
/// Its splash: the poison at its centre, half of it at its reach, measured
/// to each target's surface (Q9).
pub const SPLASH_RADIUS: f32 = 96.0;
pub const SPLASH_POISON: f32 = 16.0;
/// Its burst: the clip played once over its centre, its frames' seconds,
/// the clip's frame's centre, its particles and its draw order, the
/// flask's.
pub const BURST_CLIP: &str = "ex10_toxic_flask_burst";
pub const BURST_CENTRE: Vec2 = Vec2::splat(48.0);
pub const BURST_SECS: f32 = 0.3;
pub const BURST_PARTICLES: &str = "ex10_flask_splash";
const BURST_Z: i32 = 14;

/// The flasks in flight with their save IDs, in save-ID order.
#[must_use]
pub fn flying(world: &World) -> Vec<(SaveId, Entity)> {
    let mut flasks: Vec<(SaveId, Entity)> = world
        .query::<(Entity, &ToxicFlask, &SaveId)>()
        .map(|(entity, _, id)| (*id, entity))
        .collect();
    flasks.sort_unstable_by_key(|&(id, _)| id);
    flasks
}

/// How many flasks fly.
#[must_use]
pub fn count(world: &World) -> usize {
    world.query::<&ToxicFlask>().count()
}

/// The seconds a toss takes over `distance`.
#[must_use]
pub fn flight(distance: f32) -> f32 {
    (2.0 * distance / GRAVITY)
        .sqrt()
        .clamp(FLIGHT_LEAST, FLIGHT_MOST)
}

/// `thrower` tosses a flask from `from`, its centre, through `to` in
/// [`flight`]'s seconds. None at [`FLASK_CAP`].
pub fn throw(world: &mut World, thrower: Entity, from: Vec2, to: Vec2) -> Option<Entity> {
    if count(world) >= FLASK_CAP {
        return None;
    }
    let time = flight(from.distance(to));
    let velocity = troll::ballistic(from, to, time, Vec2::new(0.0, GRAVITY));
    Some(spawn(world, from - FLASK_CENTRE, velocity, Some(thrower)))
}

/// A flask with its sprite's top-left at `corner` flying at `velocity`,
/// from `source`, at once, with the run's next save ID: a toss's, or a
/// load's.
pub fn spawn(world: &mut World, corner: Vec2, velocity: Vec2, source: Option<Entity>) -> Entity {
    let flask = super::spawn_toy(world, FLASK_PREFAB, corner, 1.0);
    if let Some(motion) = world.get_mut::<LinearMotion>(flask) {
        motion.set_velocity(velocity);
    }
    if let Some(at) = world.get_mut::<PrevPosition>(flask) {
        at.0 = corner;
    }
    if let Some(source) = source {
        world.insert(flask, Source::new(source));
    }
    flask
}

/// `flask_contacts`, in the overlap phase before the kit's projectile step:
/// each flask in save-ID order whose centre's step met a tile or a body
/// but the wizard's, within its radius, bursts at the first point it met.
pub fn contacts(world: &mut World) {
    for (_, flask) in flying(world) {
        let (Some(at), Some(prev)) = (
            world.get::<Position>(flask).map(|at| at.0),
            world.get::<PrevPosition>(flask).map(|prev| prev.0),
        ) else {
            continue;
        };
        let hit = first_contact(world, prev + FLASK_CENTRE, at + FLASK_CENTRE, FLASK_RADIUS);
        if let Some(hit) = hit {
            despawn_tree(world, flask);
            burst(world, hit);
        }
    }
}

/// `flask_ends`, in the overlap phase after the kit's projectile step,
/// so its splash takes the step's resolve: a flask the step ended, at a
/// wall it swept to or at its range's end, bursts where it ended. `cursor`
/// reads each end once.
pub fn ends(world: &mut World, cursor: &mut EventCursor<ProjectileExpired>) {
    let Some(events) = world.get_resource::<EventQueue<ProjectileExpired>>() else {
        return;
    };
    let ended: Vec<Vec2> = cursor
        .read(events)
        .filter(|end| world.has::<ToxicFlask>(end.entity))
        .map(|end| end.position + FLASK_CENTRE)
        .collect();
    for at in ended {
        burst(world, at);
    }
}

/// A flask's burst at `at`: its splash, its clip once over its centre, its
/// particles and its sound.
pub fn burst(world: &mut World, at: Vec2) {
    splash(world, at);
    if let Some(assets) = world.get_resource_mut::<AssetRegistry>() {
        let mut sprite = Sprite::new(assets.intern_sprite(&format!("{BURST_CLIP}_0")));
        sprite.z_order = BURST_Z;
        world.spawn_with((
            Transform::from_position(at - BURST_CENTRE),
            sprite,
            Visibility { visible: true },
            AnimationState::new(BURST_CLIP),
            Lifetime::new(BURST_SECS),
            SceneEntity { state_id: GAMEPLAY },
        ));
    }
    spawn_burst(world, BURST_PARTICLES, at);
    audio::play(world, Sound::FlaskBurst, at);
}

/// The splash at `at`: each live `living` target with health but the
/// wizard whose surface lies within [`SPLASH_RADIUS`] takes
/// [`SPLASH_POISON`] at the burst's centre falling to half at the reach, a
/// spell's hit from the wizard while he lives, in save-ID order.
fn splash(world: &mut World, at: Vec2) {
    let player = world.get_resource::<Run>().map(|run| run.player);
    let caster = player.filter(|&player| {
        world.is_alive(player) && world.get::<Health>(player).is_some_and(|h| !h.is_dead())
    });
    let mut targets: Vec<(Option<SaveId>, Entity, f32)> = world
        .query::<(Entity, &Health, &Tags, &Position, &Collider)>()
        .filter(|(entity, health, tags, ..)| {
            !health.is_dead() && Some(*entity) != player && tags.has(LIVING)
        })
        .filter_map(|(entity, _, _, p, collider)| {
            let distance = surface_distance(at, p.0 + collider.offset, collider.shape);
            (distance <= SPLASH_RADIUS)
                .then(|| (world.get::<SaveId>(entity).copied(), entity, distance))
        })
        .collect();
    targets.sort_by_key(|(id, ..)| *id);
    for (_, target, distance) in targets {
        let amount = SPLASH_POISON * (1.0 - 0.5 * distance / SPLASH_RADIUS);
        let mut hit = Hit::new(target, [(Kind::Poison, amount)]).at(at).spell();
        if let Some(caster) = caster {
            hit = hit.from(caster);
        }
        hit.queue(world);
    }
}
