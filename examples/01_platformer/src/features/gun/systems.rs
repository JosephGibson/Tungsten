//! The gun's systems: the trigger, the bullets' adoption and paths, the
//! one decision a bullet a step, and the muzzle flashes.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, Collider, Entity, EventCursor, EventQueue, Position, PrevPosition, RigidBody,
    Shape, Sprite, Time, Transform, Velocity, Visibility, With, Without, World,
};
use tungsten::kit::{
    Abilities, Aim, Health, LinearMotion, Projectile, ProjectileExpired, Source, Struck, Used,
    use_ability,
};
use tungsten::physics::wake;

use super::components::{Bullet, Muzzle, Reload};
use super::{
    BALL_KICK, BULLET_CENTRE, BULLET_RADIUS, GUN, MAX_RICOCHETS, MUZZLE_FRAME_SECS, MUZZLE_FRAMES,
    MUZZLE_OFFSET, RELOAD_SECS, SHOOT, SHOT_DAMAGE, SHOT_PUSH, heading, rounds,
};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{Hit, Kind};
use crate::features::elements::cascade::{self, CascadeKind};
use crate::features::elements::components::Frozen;
use crate::features::player::{self, BODY_OFFSET};
use crate::features::run::Run;
use crate::features::toys::components::{Ball, IronBlock};
use crate::features::toys::systems::shift;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, SaveIds, despawn_tree, pressed, spawn_burst};

/// `gun_shoot`, in the intent phase after the kit's ability tick: a
/// reload running counts down, and at its end every round comes back with
/// its sound; then `shoot` held fires the gun when its cooldown and a round
/// allow, with its sound and its muzzle flash, and the magazine's last
/// round starts the reload. The run's player alone fires, while it lives.
pub fn shoot(world: &mut World) {
    let Some(player) = world.get_resource::<Run>().map(|run| run.player) else {
        return;
    };
    let dead = world.get::<Health>(player).is_some_and(Health::is_dead);
    if !world.is_alive(player) || dead {
        return;
    }
    let centre = player::centre(world, player).unwrap_or_default();
    let dt = world.resource::<Time>().delta();
    if let Some(reload) = world.get_mut::<Reload>(player) {
        reload.left -= dt;
        if reload.left <= 0.0 {
            world.remove_component::<Reload>(player);
            if let Some(abilities) = world.get_mut::<Abilities>(player) {
                abilities.refill(GUN);
            }
            audio::play(world, Sound::Reloaded, centre);
        }
    }
    if pressed(world, SHOOT) && use_ability(world, player, GUN) == Used::Used {
        let aim = world
            .get::<Aim>(player)
            .and_then(Aim::direction)
            .unwrap_or(Vec2::X);
        let muzzle = centre + aim * MUZZLE_OFFSET;
        flash(world, muzzle, aim);
        spawn_burst(world, "ex10_muzzle_flash", muzzle);
        audio::play(world, Sound::Shot, muzzle);
        if rounds(world, player).is_some_and(|(left, _)| left == 0) {
            world.insert(player, Reload { left: RELOAD_SECS });
        }
    }
}

/// A muzzle flash at `at`, turned along `aim`.
fn flash(world: &mut World, at: Vec2, aim: Vec2) {
    let Some(asset) = world
        .get_resource_mut::<AssetRegistry>()
        .map(|assets| assets.intern_sprite(MUZZLE_FRAMES[0]))
    else {
        return;
    };
    let mut sprite = Sprite::new(asset);
    sprite.z_order = 15;
    world.spawn_with((
        Transform {
            position: at - Vec2::splat(8.0),
            rotation: heading(aim),
            scale: Vec2::ONE,
        },
        sprite,
        Visibility { visible: true },
        Muzzle { age: 0.0 },
        SceneEntity { state_id: GAMEPLAY },
    ));
}

/// `muzzle_flashes`, in the present phase: each flash shows its frames in
/// turn, then goes.
pub fn muzzles(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let Some(frames) = world.get_resource_mut::<AssetRegistry>().map(|assets| {
        MUZZLE_FRAMES
            .iter()
            .map(|name| assets.intern_sprite(name))
            .collect::<Vec<_>>()
    }) else {
        return;
    };
    let mut done = Vec::new();
    for (entity, muzzle, sprite) in world.query_mut::<(Entity, &mut Muzzle, &mut Sprite)>() {
        muzzle.age += dt;
        match frames.get((muzzle.age / MUZZLE_FRAME_SECS) as usize) {
            Some(&frame) => sprite.asset_id = frame,
            None => done.push(entity),
        }
    }
    for entity in done {
        world.despawn(entity);
    }
}

/// `bullet_adopt`, in `PreUpdate` before the shell's save: each bullet the
/// gun's spawn payload landed at the last flush joins the run, its save ID
/// its own, its centre moved from the payload's point off the courier's
/// to the same place off its centre, its tracer turned along its flight
/// and shown.
pub fn adopt(world: &mut World) {
    let fresh: Vec<Entity> = world
        .query_filtered::<Entity, (With<Bullet>, Without<SaveId>)>()
        .collect();
    for bullet in fresh {
        let id = world.resource_mut::<SaveIds>().take();
        world.insert(bullet, id);
        shift(world, bullet, BODY_OFFSET - BULLET_CENTRE);
        let velocity = world
            .get::<LinearMotion>(bullet)
            .map_or(Vec2::X, LinearMotion::velocity);
        if let Some(transform) = world.get_mut::<Transform>(bullet) {
            transform.rotation = heading(velocity);
        }
        if let Some(visibility) = world.get_mut::<Visibility>(bullet) {
            visibility.visible = true;
        }
    }
}

/// `bullet_paths`, in the move phase after the movers: each bullet's
/// centre's path this step, from the step's start to where its motion
/// took it, before the kit's pass moves it back.
pub fn paths(world: &mut World) {
    for (bullet, at, prev) in world.query_mut::<(&mut Bullet, &Position, &PrevPosition)>() {
        bullet.segment = Some((prev.0 + BULLET_CENTRE, at.0 + BULLET_CENTRE));
    }
}

/// A ball or an iron block a bullet's step met: when, as a fraction of the
/// step, the body and its save ID, whether it is iron, the bullet's centre
/// there and the face's outward normal.
#[derive(Debug, Clone, Copy)]
struct Contact {
    t: f32,
    body: Entity,
    id: u64,
    iron: bool,
    point: Vec2,
    normal: Vec2,
}

/// One ball or iron block over the step: its centre at the step's start and
/// end, its shape and its save ID.
#[derive(Debug, Clone, Copy)]
struct Swept {
    body: Entity,
    id: u64,
    iron: bool,
    start: Vec2,
    end: Vec2,
    shape: Shape,
}

/// The step's balls and iron blocks, each swept from its `PrevPosition` to
/// its `Position`.
fn bodies(world: &World) -> Vec<Swept> {
    let mut swept = Vec::new();
    for (body, at, prev, collider, id) in
        world.query::<(Entity, &Position, &PrevPosition, &Collider, &SaveId)>()
    {
        let iron = world.has::<IronBlock>(body);
        if !iron && !world.has::<Ball>(body) {
            continue;
        }
        swept.push(Swept {
            body,
            id: id.0,
            iron,
            start: prev.0 + collider.offset,
            end: at.0 + collider.offset,
            shape: collider.shape,
        });
    }
    swept
}

/// When a point from `start` moving by `motion` over the step meets a
/// circle of `radius` about the origin, and the outward normal there.
fn circle_contact(start: Vec2, motion: Vec2, radius: f32) -> Option<(f32, Vec2)> {
    let c = start.length_squared() - radius * radius;
    if c <= 0.0 {
        return Some((
            0.0,
            start.try_normalize().unwrap_or(-motion.normalize_or_zero()),
        ));
    }
    let a = motion.length_squared();
    let b = start.dot(motion);
    if a <= f32::EPSILON || b >= 0.0 {
        return None;
    }
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / a;
    (t <= 1.0).then(|| (t, (start + motion * t).normalize_or_zero()))
}

/// When a point from `start` moving by `motion` over the step meets a box
/// of `half` extents about the origin, and the face's outward normal.
fn box_contact(start: Vec2, motion: Vec2, half: Vec2) -> Option<(f32, Vec2)> {
    if start.abs().cmple(half).all() {
        // Inside at the step's start: out through the nearest face.
        let gap = half - start.abs();
        let normal = if gap.x < gap.y {
            Vec2::new(start.x.signum(), 0.0)
        } else {
            Vec2::new(0.0, start.y.signum())
        };
        return Some((0.0, normal));
    }
    let (mut enter, mut exit, mut normal) = (f32::NEG_INFINITY, f32::INFINITY, Vec2::ZERO);
    for axis in 0..2 {
        let (from, by, reach) = (start[axis], motion[axis], half[axis]);
        if by.abs() <= f32::EPSILON {
            if from.abs() > reach {
                return None;
            }
            continue;
        }
        let (a, b) = ((-reach - from) / by, (reach - from) / by);
        let (near, far) = if a < b { (a, b) } else { (b, a) };
        if near > enter {
            enter = near;
            normal = Vec2::ZERO;
            normal[axis] = -by.signum();
        }
        exit = exit.min(far);
    }
    (enter <= exit && (0.0..=1.0).contains(&enter)).then_some((enter, normal))
}

/// The first ball or iron block the bullet's path `from` to `to` meets,
/// each swept with its own step's motion: the earliest, then the lower
/// save ID.
fn first_body(swept: &[Swept], from: Vec2, to: Vec2) -> Option<Contact> {
    let path = to - from;
    let (low, high) = (
        from.min(to) - Vec2::splat(BULLET_RADIUS),
        from.max(to) + Vec2::splat(BULLET_RADIUS),
    );
    let mut first: Option<Contact> = None;
    for body in swept {
        let half = match body.shape {
            Shape::Circle { radius } => Vec2::splat(radius),
            Shape::Aabb { half_extents } => half_extents,
        };
        let (min, max) = (
            body.start.min(body.end) - half,
            body.start.max(body.end) + half,
        );
        if max.cmplt(low).any() || min.cmpgt(high).any() {
            continue;
        }
        let start = from - body.start;
        let motion = path - (body.end - body.start);
        let met = match body.shape {
            Shape::Circle { radius } => circle_contact(start, motion, radius + BULLET_RADIUS),
            Shape::Aabb { half_extents } => {
                box_contact(start, motion, half_extents + Vec2::splat(BULLET_RADIUS))
            }
        };
        let Some((t, normal)) = met else {
            continue;
        };
        let earlier = first.is_none_or(|best| t < best.t || (t == best.t && body.id < best.id));
        if earlier {
            first = Some(Contact {
                t,
                body: body.body,
                id: body.id,
                iron: body.iron,
                point: from + path * t,
                normal,
            });
        }
    }
    first
}

/// `gun_hits`, in the overlap phase after the kit's projectile pass: one
/// decision a bullet, by what its step met first, each a fraction of the
/// path `bullet_paths` recorded: the kit's hurtbox hit, its wall or range
/// end, or a ball or iron block, swept with its own motion. A hit deals
/// [`SHOT_DAMAGE`] physical with its push, from the shooter, so it can
/// crit; a ball takes [`BALL_KICK`] over its mass, a frozen one shatters,
/// and the shot ends; iron turns the shot about the face it met, at most
/// [`MAX_RICOCHETS`] times, when the pass left it alive, else ends it
/// there. A `Struck` the decision passes over deals nothing.
pub fn hits(
    world: &mut World,
    struck: &mut EventCursor<Struck>,
    expired: &mut EventCursor<ProjectileExpired>,
) {
    let mut met: Vec<(Entity, Entity, Vec2)> = Vec::new();
    if let Some(events) = world.get_resource::<EventQueue<Struck>>() {
        met.extend(
            struck
                .read(events)
                .filter(|hit| world.has::<Bullet>(hit.entity))
                .map(|hit| (hit.entity, hit.target, hit.position)),
        );
    }
    let mut ended: Vec<(Entity, Vec2)> = Vec::new();
    if let Some(events) = world.get_resource::<EventQueue<ProjectileExpired>>() {
        ended.extend(
            expired
                .read(events)
                .filter(|end| world.has::<Bullet>(end.entity))
                .map(|end| (end.entity, end.position + BULLET_CENTRE)),
        );
    }
    let mut bullets: Vec<(u64, Entity, Vec2, Vec2)> = world
        .query::<(Entity, &Bullet, &SaveId)>()
        .filter_map(|(entity, bullet, id)| {
            let (from, to) = bullet.segment?;
            Some((id.0, entity, from, to))
        })
        .collect();
    if bullets.is_empty() {
        return;
    }
    bullets.sort_by_key(|&(id, ..)| id);
    let swept = bodies(world);
    for (_, bullet, from, to) in bullets {
        if let Some(state) = world.get_mut::<Bullet>(bullet) {
            state.segment = None;
        }
        let path = to - from;
        let length = path.length();
        if length <= f32::EPSILON {
            continue;
        }
        let along = |point: Vec2| ((point - from).dot(path) / (length * length)).clamp(0.0, 1.0);
        let hit = met
            .iter()
            .find(|&&(entity, ..)| entity == bullet)
            .map(|&(_, target, at)| (along(at), target, at));
        let stop = hit.map(|(t, ..)| t).or_else(|| {
            ended
                .iter()
                .find(|&&(entity, _)| entity == bullet)
                .map(|&(_, at)| along(at))
        });
        let body = first_body(&swept, from, to).filter(|body| stop.is_none_or(|t| body.t < t));
        let spent = world
            .get::<Projectile>(bullet)
            .is_some_and(Projectile::is_spent);
        let along_shot = path / length;
        match body {
            Some(contact) if contact.iron => ricochet(world, bullet, contact, spent),
            Some(contact) => kick(world, bullet, contact, along_shot, spent),
            None => {
                if let Some((_, target, at)) = hit {
                    let mut shot = Hit::new(target, [(Kind::Physical, SHOT_DAMAGE)])
                        .at(at)
                        .push(along_shot * SHOT_PUSH);
                    if let Some(source) = world.get::<Source>(bullet).map(|source| source.entity())
                    {
                        shot = shot.from(source);
                    }
                    shot.queue(world);
                }
            }
        }
    }
}

/// Puts the bullet's centre at `point`, its history too.
fn place(world: &mut World, bullet: Entity, point: Vec2) {
    let at = point - BULLET_CENTRE;
    if let Some(position) = world.get_mut::<Position>(bullet) {
        position.0 = at;
    }
    if let Some(prev) = world.get_mut::<PrevPosition>(bullet) {
        prev.0 = at;
    }
    if let Some(transform) = world.get_mut::<Transform>(bullet) {
        transform.position = at;
    }
}

/// Ends the shot at `point`: one the kit's pass left alive goes now; one it
/// ended beyond is put there for the kit's despawn.
fn end_at(world: &mut World, bullet: Entity, point: Vec2, spent: bool) {
    if spent {
        place(world, bullet, point);
    } else {
        despawn_tree(world, bullet);
    }
}

/// A shot's ball: a frozen one shatters, another takes the kick along the
/// shot over its mass; the shot ends.
fn kick(world: &mut World, bullet: Entity, contact: Contact, along: Vec2, spent: bool) {
    if world.has::<Frozen>(contact.body) {
        let at = contact.point - contact.normal * BULLET_RADIUS;
        cascade::queue(world, CascadeKind::Shatter, Some(contact.body), at, at, 0.0);
    } else {
        let inverse = world
            .get::<RigidBody>(contact.body)
            .map_or(0.0, |body| body.inv_mass);
        if let Some(velocity) = world.get_mut::<Velocity>(contact.body) {
            velocity.0 += along * BALL_KICK * inverse;
        }
        wake(world, contact.body);
    }
    end_at(world, bullet, contact.point, spent);
}

/// A shot's iron: while the kit's pass left it alive and it has turned
/// fewer than [`MAX_RICOCHETS`] times, it turns about the face it met from
/// where it met it, with sparks and their sound; else it ends there.
fn ricochet(world: &mut World, bullet: Entity, contact: Contact, spent: bool) {
    let turns = world
        .get::<Bullet>(bullet)
        .map_or(0, |state| state.ricochets);
    if spent || turns >= MAX_RICOCHETS {
        return end_at(world, bullet, contact.point, spent);
    }
    let Some(velocity) = world
        .get::<LinearMotion>(bullet)
        .map(LinearMotion::velocity)
    else {
        return;
    };
    let turned = velocity - 2.0 * velocity.dot(contact.normal) * contact.normal;
    place(world, bullet, contact.point + contact.normal * 0.5);
    if let Some(motion) = world.get_mut::<LinearMotion>(bullet) {
        motion.set_velocity(turned);
    }
    if let Some(transform) = world.get_mut::<Transform>(bullet) {
        transform.rotation = heading(turned);
    }
    if let Some(state) = world.get_mut::<Bullet>(bullet) {
        state.ricochets += 1;
    }
    spawn_burst(world, "ex10_ricochet", contact.point);
    audio::play(world, Sound::Ricocheted, contact.point);
}
