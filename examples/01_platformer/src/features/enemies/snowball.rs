//! The crawler's snowball (e01foes, V22; the kit's projectile since 7.13c's
//! carry, flight and data): a clump of packed snow it flicks at the player
//! on the world's gravity. Its `snowball` prefab is a bodiless kit
//! projectile on `linear_motion`, as the flask flies: its hitbox hurts the
//! first live target off the enemies' team it meets with [`SNOWBALL_COLD`]
//! cold at its thrower's damage factor, from its thrower while that lives
//! (`credit: live`), as any cold builds toward a chill and a freezable
//! target's freeze; its own [`SNOWBALL_RADIUS`] circle meets the tiles, the
//! static colliders and the moving bodies but its thrower's, and it ends
//! there, at its first hit or after its seconds. [`ends`] shows its burst
//! where it ended. At most [`SNOWBALL_CAP`] fly at once.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AnimationState, AssetRegistry, Entity, EventCursor, EventQueue, Position, PrevPosition, Sprite,
    Transform, Visibility, World,
};
use tungsten::kit::{DamagePart, Hitbox, Lifetime, LinearMotion, ProjectileExpired, Source};

use super::components::Snowball;
use crate::features::audio::{self, components::Sound};
use crate::features::toys;
use crate::game::GAMEPLAY;
use crate::shared::{Saved, despawn_tree, spawn_burst};

/// Its prefab, its own shape's radius, which meets its walls (its drawn
/// ball's 5 px and 2 px past it), and its centre from its sprite's
/// top-left.
pub const SNOWBALL_PREFAB: &str = "snowball";
pub const SNOWBALL_RADIUS: f32 = 7.0;
pub const SNOWBALL_CENTRE: Vec2 = Vec2::new(8.0, 8.0);
/// The cold it lands at a damage factor of 1, its prefab's hitbox's, and
/// the most that fly at once, the oldest bursting for a new one.
pub const SNOWBALL_COLD: f32 = 12.0;
pub const SNOWBALL_CAP: usize = 8;
/// Its burst: the clip played once at its centre, over its six frames'
/// seconds, the clip's frame's centre, its particles and its draw order,
/// the snowball's.
pub const BURST_CLIP: &str = "ex10_snowball_burst";
pub const BURST_CENTRE: Vec2 = Vec2::new(24.0, 24.0);
pub const BURST_SECS: f32 = 0.3;
pub const BURST_PARTICLES: &str = "ex10_snowball_burst";
const BURST_Z: i32 = 14;

/// A snowball with its sprite's top-left at `corner`, at once, one of the
/// run's records: a throw's.
pub fn spawn(world: &mut World, corner: Vec2) -> Entity {
    toys::spawn_toy(world, SNOWBALL_PREFAB, corner, 1.0)
}

/// The snowballs in flight with their save IDs, in save-ID order.
#[must_use]
pub fn flying(world: &World) -> Vec<(u64, Entity)> {
    let mut balls: Vec<(u64, Entity)> = world
        .query::<(Entity, &Snowball, &Saved)>()
        .map(|(entity, _, id)| (id.id().0, entity))
        .collect();
    balls.sort_unstable_by_key(|&(id, _)| id);
    balls
}

/// Throws a snowball with its centre at `from` at `velocity`, from
/// `thrower`, its hitbox's cold at the thrower's damage `factor`; past
/// [`SNOWBALL_CAP`] in flight the oldest bursts first. Its entity.
pub fn throw(
    world: &mut World,
    from: Vec2,
    velocity: Vec2,
    thrower: Option<Entity>,
    factor: f32,
) -> Entity {
    let balls = flying(world);
    for &(_, oldest) in balls
        .iter()
        .take((balls.len() + 1).saturating_sub(SNOWBALL_CAP))
    {
        burst(world, oldest);
    }
    let corner = from - SNOWBALL_CENTRE;
    let ball = spawn(world, corner);
    if let Some(motion) = world.get_mut::<LinearMotion>(ball) {
        motion.set_velocity(velocity);
    }
    if let Some(at) = world.get_mut::<PrevPosition>(ball) {
        at.0 = corner;
    }
    if let Some(thrower) = thrower {
        world.insert(ball, Source::new(thrower));
    }
    if let Some(hitbox) = world.get_mut::<Hitbox>(ball) {
        let parts: Vec<DamagePart> = hitbox
            .parts()
            .iter()
            .map(|part| DamagePart::new(part.amount() * factor).with_tags(part.tags().clone()))
            .collect();
        *hitbox = hitbox.clone().with_parts(parts);
    }
    ball
}

/// `snowball_ends`, in the overlap phase after the kit's projectile step:
/// a snowball the step ended, at a target, a tile, a wall or a body, or at
/// its seconds' end, shows its burst where its centre ended. `cursor` reads
/// each end once.
pub fn ends(world: &mut World, cursor: &mut EventCursor<ProjectileExpired>) {
    let Some(events) = world.get_resource::<EventQueue<ProjectileExpired>>() else {
        return;
    };
    let ended: Vec<Vec2> = cursor
        .read(events)
        .filter(|end| world.has::<Snowball>(end.entity))
        .map(|end| end.position + SNOWBALL_CENTRE)
        .collect();
    for at in ended {
        look(world, at);
    }
}

/// `ball` bursts where it is, the cap's: its burst's look at its centre,
/// and it goes.
pub fn burst(world: &mut World, ball: Entity) {
    if let Some(at) = world.get::<Position>(ball).map(|at| at.0 + SNOWBALL_CENTRE) {
        look(world, at);
    }
    despawn_tree(world, ball);
}

/// A snowball's burst's look at `at`: its clip once over its centre, its
/// particles and its sound.
fn look(world: &mut World, at: Vec2) {
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
