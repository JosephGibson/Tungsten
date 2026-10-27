//! The crawler's snowball (e01foes, V22): a clump of packed snow it flicks
//! at the player on the world's gravity. Its `snowball` prefab is a light
//! dynamic body; [`Snowball`] keeps its thrower, its damage factor and its
//! seconds left. In the overlap phase after the physics step it bursts at
//! the first thing it touches, a live target off the enemies' team taking
//! [`SNOWBALL_COLD`] cold at its factor, as any cold builds toward a chill
//! and a freezable target's freeze; else at a tile or a body, or once its
//! time runs out. At most [`SNOWBALL_CAP`] fly at once.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AnimationState, AssetRegistry, Collider, Entity, Position, Shape, Sprite, Time, Transform,
    Velocity, Visibility, World,
};
use tungsten::kit::{Health, Lifetime, Tags};
use tungsten::physics::{SpatialFilter, overlap};

use super::components::Snowball;
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{Hit, Kind};
use crate::features::toys;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, body_centre, despawn_tree, spawn_burst};

/// Its prefab, its body's radius and its centre from its sprite's top-left.
pub const SNOWBALL_PREFAB: &str = "snowball";
pub const SNOWBALL_RADIUS: f32 = 5.0;
pub const SNOWBALL_CENTRE: Vec2 = Vec2::new(8.0, 8.0);
/// The cold it lands at a damage factor of 1, the most that fly at once,
/// the oldest bursting for a new one, and its seconds before it bursts in
/// the air.
pub const SNOWBALL_COLD: f32 = 12.0;
pub const SNOWBALL_CAP: usize = 8;
pub const SNOWBALL_LIFE: f32 = 3.0;
/// How far past a target's surface it lands, the impacts' contact margin,
/// and how far past its body a tile or a body bursts it, since a contact
/// event misses a body stopped at touching.
pub const SNOWBALL_REACH: f32 = 3.0;
pub const SNOWBALL_MARGIN: f32 = 2.0;
/// Its burst: the clip played once at its centre, over its six frames'
/// seconds, the clip's frame's centre, its particles and its draw order,
/// the snowball's.
pub const BURST_CLIP: &str = "ex10_snowball_burst";
pub const BURST_CENTRE: Vec2 = Vec2::new(24.0, 24.0);
pub const BURST_SECS: f32 = 0.3;
pub const BURST_PARTICLES: &str = "ex10_snowball_burst";
const BURST_Z: i32 = 14;

/// A snowball's body with its sprite's top-left at `corner`, at once, with
/// the run's next save ID: a throw's, or a load's.
pub fn spawn(world: &mut World, corner: Vec2) -> Entity {
    toys::spawn_toy(world, SNOWBALL_PREFAB, corner, 1.0)
}

/// The snowballs in flight with their save IDs, in save-ID order.
#[must_use]
pub fn flying(world: &World) -> Vec<(u64, Entity)> {
    let mut balls: Vec<(u64, Entity)> = world
        .query::<(Entity, &Snowball, &SaveId)>()
        .map(|(entity, _, id)| (id.0, entity))
        .collect();
    balls.sort_unstable_by_key(|&(id, _)| id);
    balls
}

/// Throws a snowball with its centre at `from` at `velocity`, from the
/// thrower of save ID `thrower` at its damage `factor`; past
/// [`SNOWBALL_CAP`] in flight the oldest bursts first. Its body.
pub fn throw(
    world: &mut World,
    from: Vec2,
    velocity: Vec2,
    thrower: Option<u64>,
    factor: f32,
) -> Entity {
    let balls = flying(world);
    for &(_, oldest) in balls
        .iter()
        .take((balls.len() + 1).saturating_sub(SNOWBALL_CAP))
    {
        burst(world, oldest);
    }
    let ball = spawn(world, from - SNOWBALL_CENTRE);
    let snowball = Snowball {
        thrower,
        factor,
        left: SNOWBALL_LIFE,
    };
    world.insert_bundle(ball, (snowball, Velocity(velocity)));
    ball
}

/// The gap between a circle of `radius` about `centre` and a body of
/// `shape` about `at`: below 0 where they overlap.
fn gap(centre: Vec2, radius: f32, at: Vec2, shape: Shape) -> f32 {
    match shape {
        Shape::Circle { radius: other } => centre.distance(at) - radius - other,
        Shape::Aabb { half_extents } => {
            ((centre - at).abs() - half_extents)
                .max(Vec2::ZERO)
                .length()
                - radius
        }
    }
}

/// The live entity of save ID `id`.
fn live(world: &World, id: u64) -> Option<Entity> {
    world
        .query::<(Entity, &SaveId, &Health)>()
        .find(|(_, own, health)| own.0 == id && !health.is_dead())
        .map(|(entity, ..)| entity)
}

/// `snowballs`, in the overlap phase after the physics step: each snowball
/// in save-ID order bursts at the first live target off the enemies' team,
/// in save-ID order, its body reaches within [`SNOWBALL_REACH`], which takes
/// [`SNOWBALL_COLD`] cold at its factor, from its thrower while it lives;
/// else at a solid tile or a body within [`SNOWBALL_MARGIN`] of its body,
/// its thrower passed over; else once its seconds run out.
pub fn snowballs(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    if dt <= 0.0 {
        return;
    }
    let balls = flying(world);
    if balls.is_empty() {
        return;
    }
    let mut targets: Vec<(u64, Entity, Vec2, Shape)> = world
        .query::<(Entity, &SaveId, &Health, &Position, &Collider)>()
        .filter(|&(entity, _, health, ..)| {
            !health.is_dead()
                && !world
                    .get::<Tags>(entity)
                    .is_some_and(|tags| tags.has("team.enemy"))
        })
        .map(|(entity, id, _, at, collider)| (id.0, entity, at.0 + collider.offset, collider.shape))
        .collect();
    targets.sort_unstable_by_key(|&(id, ..)| id);
    for (_, ball) in balls {
        let (Some(centre), Some(&snowball)) =
            (body_centre(world, ball), world.get::<Snowball>(ball))
        else {
            continue;
        };
        let thrower = snowball.thrower.and_then(|id| live(world, id));
        let touched = targets
            .iter()
            .find(|&&(_, target, at, shape)| {
                target != ball && gap(centre, SNOWBALL_RADIUS, at, shape) <= SNOWBALL_REACH
            })
            .map(|&(_, target, at, _)| (target, at));
        if let Some((target, at)) = touched {
            let mut hit = Hit::new(target, [(Kind::Cold, SNOWBALL_COLD * snowball.factor)]).at(at);
            if let Some(thrower) = thrower {
                hit = hit.from(thrower);
            }
            hit.queue(world);
            burst(world, ball);
            continue;
        }
        let shape = Shape::Circle {
            radius: SNOWBALL_RADIUS + SNOWBALL_MARGIN,
        };
        let mut met = Vec::new();
        overlap(
            world,
            shape,
            centre,
            &SpatialFilter::new().excluding(ball),
            &mut met,
        );
        let struck = met
            .iter()
            .any(|&hit| hit.is_none_or(|entity| Some(entity) != thrower));
        let left = snowball.left - dt;
        if struck || left <= 0.0 {
            burst(world, ball);
        } else if let Some(own) = world.get_mut::<Snowball>(ball) {
            own.left = left;
        }
    }
}

/// `ball` bursts where it is: its clip once over its centre, its
/// particles, its sound, and it goes.
pub fn burst(world: &mut World, ball: Entity) {
    if let Some(at) = body_centre(world, ball) {
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
        audio::play(world, Sound::SnowballBurst, at);
    }
    despawn_tree(world, ball);
}
