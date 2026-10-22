//! The gun's systems: the trigger, the bullets' adoption and paths, the
//! one decision a bullet a step, and the muzzle flashes.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, Collider, Entity, EventCursor, EventQueue, Position, PrevPosition, RigidBody,
    Shape, Sprite, Time, Transform, Velocity, Visibility, With, Without, World,
};
use tungsten::kit::{
    Abilities, Aim, Health, Hurtbox, LinearMotion, Projectile, ProjectileBounced,
    ProjectileExpired, ProjectilePassed, Source, Struck, Used, use_ability,
};
use tungsten::physics::wake;

use super::components::{Bullet, Muzzle, Reload};
use super::{
    BALL_KICK, BULLET_CENTRE, BULLET_RADIUS, GUN, MAX_RICOCHETS, MUZZLE_FRAME_SECS, MUZZLE_FRAMES,
    MUZZLE_OFFSET, RELOAD_SECS, SHOOT, SHOT_DAMAGE, SHOT_PUSH, heading, rounds,
};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{FROZEN, Hit, Kind, has_status};
use crate::features::elements::cascade::{self, CascadeKind};
use crate::features::elements::components::Frozen;
use crate::features::enemies::beetle;
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

/// What a bullet's step can meet beside the kit's hurtboxes: a ball, an
/// iron block, or a live beetle's hurtbox, which the bullets' filter skips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum What {
    Ball,
    Iron,
    Beetle,
}

/// A body a bullet's step met: when, as a fraction of the step, the body,
/// its save ID and what it is, the bullet's centre there, the face's
/// outward normal, and how far the body moves on from there to the step's
/// end.
#[derive(Debug, Clone, Copy)]
struct Contact {
    t: f32,
    body: Entity,
    id: u64,
    what: What,
    point: Vec2,
    normal: Vec2,
    moved: Vec2,
}

/// One body over the step: its centre at the step's start and end, its
/// shape, its save ID and what it is.
#[derive(Debug, Clone, Copy)]
struct Swept {
    body: Entity,
    id: u64,
    what: What,
    start: Vec2,
    end: Vec2,
    shape: Shape,
}

/// The step's balls and iron blocks, each swept from its `PrevPosition` to
/// its `Position`, and its live beetles' hurtboxes.
fn bodies(world: &World) -> Vec<Swept> {
    let mut swept = Vec::new();
    for (body, at, prev, collider, id) in
        world.query::<(Entity, &Position, &PrevPosition, &Collider, &SaveId)>()
    {
        let what = if world.has::<IronBlock>(body) {
            What::Iron
        } else if world.has::<Ball>(body) {
            What::Ball
        } else {
            continue;
        };
        swept.push(Swept {
            body,
            id: id.0,
            what,
            start: prev.0 + collider.offset,
            end: at.0 + collider.offset,
            shape: collider.shape,
        });
    }
    for (body, at, prev, hurtbox, health, id) in world.query::<(
        Entity,
        &Position,
        Option<&PrevPosition>,
        &Hurtbox,
        &Health,
        &SaveId,
    )>() {
        if health.is_dead() || !beetle::is_beetle(world, body) {
            continue;
        }
        let start = prev.map_or(at.0, |prev| prev.0);
        swept.push(Swept {
            body,
            id: id.0,
            what: What::Beetle,
            start: start + hurtbox.offset(),
            end: at.0 + hurtbox.offset(),
            shape: hurtbox.shape().shape(),
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

/// The first body the bullet's path `from` to `to` meets, each swept with
/// its own step's motion: the earliest, then the lower save ID.
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
                what: body.what,
                point: from + path * t,
                normal,
                moved: (body.end - body.start) * (1.0 - t),
            });
        }
    }
    first
}

/// [`hits`]' readers of the kit's projectile events.
#[derive(Default)]
pub struct ShotCursors {
    struck: EventCursor<Struck>,
    expired: EventCursor<ProjectileExpired>,
    bounced: EventCursor<ProjectileBounced>,
    passed: EventCursor<ProjectilePassed>,
}

/// Where a bullet's step met a wall, its centre there and the wall's
/// normal: a bounce, a pass or an end.
type WallMet = (Entity, Vec2, Vec2);

/// The bullets' events in `queue` that `cursor` has not read, each as a
/// [`WallMet`].
fn bullet_walls<E: 'static>(
    world: &World,
    cursor: &mut EventCursor<E>,
    meet: impl Fn(&E) -> Option<WallMet>,
) -> Vec<WallMet> {
    world
        .get_resource::<EventQueue<E>>()
        .map(|queue| {
            cursor
                .read(queue)
                .filter_map(&meet)
                .filter(|(entity, ..)| world.has::<Bullet>(*entity))
                .collect()
        })
        .unwrap_or_default()
}

/// `gun_hits`, in the overlap phase after the kit's projectile pass: one
/// decision a bullet, by what its step met first, each a fraction of the
/// path `bullet_paths` recorded: the kit's hurtbox hit, its turn off iron,
/// its wall or range end, or a ball, an iron block or a beetle, swept with
/// its own motion. A hit deals [`SHOT_DAMAGE`] physical with its push,
/// from the shooter, so it can crit; a ball takes [`BALL_KICK`] over its
/// mass, a frozen one shatters, and the shot ends; an iron block, and a
/// live beetle's front, turn the shot about the face it met, at most
/// [`MAX_RICOCHETS`] times, when the pass left it alive, else end it there;
/// a beetle's rear, or a frozen beetle anywhere, takes the shot as a hit,
/// which its rear takes whole. A `Struck` the decision passes over deals
/// nothing. The walls' answers: the kit's turn off an iron wall throws
/// sparks with the ricochet's sound, a wooden deck passed throws splinters
/// with theirs, and a shot ending at a wall puffs dust.
pub fn hits(world: &mut World, cursors: &mut ShotCursors) {
    let mut met: Vec<(Entity, Entity, Vec2)> = Vec::new();
    if let Some(events) = world.get_resource::<EventQueue<Struck>>() {
        met.extend(
            cursors
                .struck
                .read(events)
                .filter(|hit| world.has::<Bullet>(hit.entity))
                .map(|hit| (hit.entity, hit.target, hit.position)),
        );
    }
    let ended = bullet_walls(world, &mut cursors.expired, |end: &ProjectileExpired| {
        let normal = end.wall.map_or(Vec2::ZERO, |wall| wall.normal);
        Some((end.entity, end.position + BULLET_CENTRE, normal))
    });
    let bounced = bullet_walls(world, &mut cursors.bounced, |turn: &ProjectileBounced| {
        Some((turn.entity, turn.position + BULLET_CENTRE, turn.wall.normal))
    });
    let passed = bullet_walls(world, &mut cursors.passed, |pass: &ProjectilePassed| {
        Some((pass.entity, pass.position + BULLET_CENTRE, pass.wall.normal))
    });
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
        let find = |list: &[WallMet]| {
            list.iter()
                .find(|&&(entity, ..)| entity == bullet)
                .map(|&(_, at, normal)| (along(at), at, normal))
        };
        let (end, turn) = (find(&ended), find(&bounced));
        // The step's travel stops at the first of a hit, the kit's turn and
        // its end.
        let stop = [
            hit.map(|(t, ..)| t),
            turn.map(|(t, ..)| t),
            end.map(|(t, ..)| t),
        ]
        .into_iter()
        .flatten()
        .min_by(f32::total_cmp);
        let body = first_body(&swept, from, to).filter(|body| stop.is_none_or(|t| body.t < t));
        let reach = body.map_or(1.0, |body| body.t);
        for &(_, at, _) in passed.iter().filter(|&&(entity, ..)| entity == bullet) {
            if along(at) <= reach {
                spawn_burst(world, "ex10_poof", at);
                audio::play(world, Sound::Splintered, at);
            }
        }
        let spent = world
            .get::<Projectile>(bullet)
            .is_some_and(Projectile::is_spent);
        let along_shot = path / length;
        match body {
            Some(contact) if contact.what == What::Iron => {
                ricochet(world, bullet, contact, along_shot, spent);
            }
            Some(contact) if contact.what == What::Beetle => {
                shell(world, bullet, contact, along_shot, spent);
            }
            Some(contact) => kick(world, bullet, contact, along_shot, spent),
            None => {
                if let Some((_, target, at)) = hit {
                    strike(world, bullet, target, at, along_shot);
                } else if let Some((t, at, _)) = turn
                    && end.is_none_or(|(end, ..)| t <= end)
                {
                    sparks(world, bullet, at);
                } else if let Some((_, at, normal)) =
                    end.filter(|&(.., normal)| normal != Vec2::ZERO)
                {
                    spawn_burst(world, "ex10_landing_dust", at + normal * BULLET_RADIUS);
                }
            }
        }
    }
}

/// The kit's turn of `bullet` off an iron wall at `at`: its tracer turned
/// along its new flight, sparks and the ricochet's sound.
fn sparks(world: &mut World, bullet: Entity, at: Vec2) {
    let velocity = world
        .get::<LinearMotion>(bullet)
        .map_or(Vec2::X, LinearMotion::velocity);
    if let Some(transform) = world.get_mut::<Transform>(bullet) {
        transform.rotation = heading(velocity);
    }
    spawn_burst(world, "ex10_ricochet", at);
    audio::play(world, Sound::Ricocheted, at);
}

/// The shot's hit on `target` at `at`: [`SHOT_DAMAGE`] physical with its
/// push along the shot, from the shooter.
fn strike(world: &mut World, bullet: Entity, target: Entity, at: Vec2, along: Vec2) {
    let mut shot = Hit::new(target, [(Kind::Physical, SHOT_DAMAGE)])
        .at(at)
        .push(along * SHOT_PUSH);
    if let Some(source) = world.get::<Source>(bullet).map(|source| source.entity()) {
        shot = shot.from(source);
    }
    shot.queue(world);
}

/// A shot's beetle, its contact moved on with the beetle to the step's
/// end: a live beetle's front turns the shot as iron does; its rear, or a
/// frozen beetle anywhere, takes the shot as a hit and ends it.
fn shell(world: &mut World, bullet: Entity, contact: Contact, along: Vec2, spent: bool) {
    let contact = Contact {
        point: contact.point + contact.moved,
        ..contact
    };
    let frozen = has_status(world, contact.body, FROZEN);
    if !frozen && beetle::front(world, contact.body, contact.point) {
        return ricochet(world, bullet, contact, along, spent);
    }
    strike(world, bullet, contact.body, contact.point, along);
    end_at(world, bullet, contact.point, spent);
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

/// A shot's iron or beetle's front: while the kit's pass left it alive and
/// it has turned fewer than [`MAX_RICOCHETS`] times, it turns about the
/// face it met from where it met it, flying in `along`, at its speed (the
/// kit's turn off a wall later in the step undone), with sparks and their
/// sound; else it ends there.
fn ricochet(world: &mut World, bullet: Entity, contact: Contact, along: Vec2, spent: bool) {
    let turns = world
        .get::<Bullet>(bullet)
        .map_or(0, |state| state.ricochets);
    if spent || turns >= MAX_RICOCHETS {
        return end_at(world, bullet, contact.point, spent);
    }
    let Some(velocity) = world
        .get::<LinearMotion>(bullet)
        .map(|motion| along * motion.velocity().length())
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
