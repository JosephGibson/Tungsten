//! The toxic flask (e01flask, V39): a glass flask of poison the wizard lobs
//! at the cursor on the world's gravity ([`throw`]). Its `toxic_flask`
//! prefab is the fire missile's kit projectile on its linear motion, which
//! is exact under a constant fall, so it passes through the cursor; it has
//! no body, so nothing pushes it and it passes through the wizard. The
//! kit's projectile step ends it where its own shape's sweep first meets a
//! tile or a body in its body walls but the wizard's, or at its range's end
//! in the air, and sets off its prefab's end blast, its splash: poison on
//! the living in reach, the wizard spared, a spell's hit from him while he
//! lives, at the step's resolve. [`ends`] shows its burst. At most
//! [`FLASK_CAP`] fly at once.

use glam::Vec2;
use serde::Deserialize;
use tungsten::SceneEntity;
use tungsten::core::save::Value;
use tungsten::core::{
    AnimationState, AssetRegistry, Entity, EventCursor, EventQueue, PhysicsLayers, PrefabRegistry,
    PrevPosition, Sprite, Transform, Visibility, World,
};
use tungsten::kit::{
    Blast, HitShape, Lifetime, LinearMotion, Projectile, ProjectileExpired, Source, lob_velocity,
};

use super::components::ToxicFlask;
use crate::features::audio::{self, components::Sound};
use crate::features::level::GRAVITY;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, Saved, spawn_burst};

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
        .query::<(Entity, &ToxicFlask, &Saved)>()
        .map(|(entity, _, id)| (id.id(), entity))
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
    let velocity = lob_velocity(from, to, time, Vec2::new(0.0, GRAVITY));
    Some(spawn(world, from - FLASK_CENTRE, velocity, Some(thrower)))
}

/// A flask with its sprite's top-left at `corner` flying at `velocity`,
/// from `source`, at once, one of the run's records: a toss's.
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

/// `flask_ends`, in the overlap phase after the kit's projectile step: a
/// flask the step ended, at a wall or a body it swept its shape to or at
/// its range's end, shows its burst where its shape's centre ended, its
/// end's blast the kit's splash. `cursor` reads each end once.
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

/// A flask's burst's look at `at`, its splash the kit's: its clip once over
/// its centre, its particles and its sound.
pub fn burst(world: &mut World, at: Vec2) {
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

/// What a restored flask's spawn gave it beyond its saved state: an older
/// save's, whose projectile has no end, shape or body walls, takes its
/// prefab's, read from the prefab's data, so it splashes and meets bodies
/// as a fresh toss does.
pub fn restored(world: &mut World, flask: Entity) {
    let Some(projectile) = world.get::<Projectile>(flask) else {
        return;
    };
    let (end, shape, walls) = (
        projectile.end_blast().is_none(),
        projectile.shape().is_none(),
        projectile.body_walls() == 0,
    );
    if projectile.is_spent() || !(end || shape || walls) {
        return;
    }
    let Some(data) = world
        .get_resource::<PrefabRegistry>()
        .and_then(|prefabs| prefabs.get(FLASK_PREFAB))
        .and_then(|data| data.components.get("projectile"))
    else {
        return;
    };
    let mut whole = projectile.clone();
    if end
        && let Some(blast) = data
            .get("end")
            .and_then(|end| end.get("blast"))
            .and_then(|blast| Blast::deserialize(blast.clone()).ok())
    {
        whole = whole.with_end(blast);
    }
    if shape
        && let Some(own) = data
            .get("shape")
            .and_then(|own| HitShape::deserialize(own.clone()).ok())
    {
        let offset = data
            .get("offset")
            .and_then(Value::as_array)
            .and_then(|xy| {
                Some(Vec2::new(
                    xy.first()?.as_f64()? as f32,
                    xy.get(1)?.as_f64()? as f32,
                ))
            })
            .unwrap_or(Vec2::ZERO);
        whole = whole.with_shape(own, offset);
    }
    if walls && let Some(list) = data.get("body_walls").and_then(Value::as_array) {
        whole = whole.with_body_walls(layer_mask(world, list));
    }
    if let Some(projectile) = world.get_mut::<Projectile>(flask) {
        *projectile = whole;
    }
}

/// A prefab's layer list as a mask, one bit per layer: each entry a layer
/// number or the manifest's `physics_layers` name for one.
fn layer_mask(world: &World, list: &[Value]) -> u32 {
    let names = world.get_resource::<PhysicsLayers>();
    list.iter()
        .filter_map(|entry| match entry {
            Value::String(name) => names?.bit(name),
            number => 1_u32.checked_shl(u32::try_from(number.as_u64()?).ok()?),
        })
        .fold(0, |mask, bit| mask | bit)
}
