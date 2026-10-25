//! The force push as a wave (01 (j), N4 and N20): a front of bent air that
//! leaves the wizard's free hand toward the cursor at [`WAVE_SPEED`] for
//! [`WAVE_SECS`], its sector widening from [`WAVE_HALF_FROM`] to
//! [`WAVE_HALF_TO`]. It crosses each body and enemy once, where the target
//! meets its front, and throws it along the aim, less with distance; it
//! raises dust where it scrapes a floor, and draws as an animated crescent
//! of bent air (`ex10_push_wave`). Not saved: a wave lives 0.3 s, and an
//! enemy's saved knockback and controller carry a throw already running.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AnimationState, AssetRegistry, BodyKind, Collider, Entity, Position, RigidBody, Shape, Sprite,
    Time, Transform, Velocity, Visibility, World,
};
use tungsten::kit::{Health, Lifetime, PlatformerController, knock_back};
use tungsten::physics::{SpatialFilter, cast_ray, wake};

use super::components::Wave;
use crate::features::enemies::{self, components::Enemy};
use crate::game::GAMEPLAY;
use crate::shared::{body_centre, spawn_burst};

/// The front's speed and life, so its reach; its sector's half angle at its
/// start and at its end; and where it starts ahead of the wizard's centre,
/// at the push gesture's hand.
pub const WAVE_SPEED: f32 = 1200.0;
pub const WAVE_SECS: f32 = 0.3;
pub const WAVE_RANGE: f32 = WAVE_SPEED * WAVE_SECS;
pub const WAVE_HALF_FROM: f32 = 15.0 * std::f32::consts::PI / 180.0;
pub const WAVE_HALF_TO: f32 = 30.0 * std::f32::consts::PI / 180.0;
pub const WAVE_START: f32 = 24.0;
/// Its throw where it starts: an enemy's along the aim and a walker's hop,
/// each by its kind's weight ([`KindInfo::wave`](enemies::KindInfo::wave)),
/// a walker's held through a lock of [`WAVE_LOCK`]; a body's along the aim
/// and a lift, over its mass; and the share of it left at its reach.
pub const WAVE_THROW: f32 = 500.0;
pub const WAVE_HOP: f32 = 300.0;
pub const WAVE_LOCK: f32 = 0.35;
pub const WAVE_SHOVE: f32 = 1800.0;
pub const WAVE_BODY_HOP: f32 = 500.0;
pub const WAVE_FAR: f32 = 0.5;
/// The samples across the front that raise its dust.
pub const DUST_SAMPLES: usize = 9;
/// The crescent's art, the seam's x in it, the share longer it grows over
/// its life, how much wider than the sector it is drawn, its tint and its
/// draw order; it fades in over the first share of its life and out over
/// the last.
const ART: Vec2 = Vec2::new(72.0, 128.0);
const ART_SEAM: f32 = 66.0;
const GROWS: f32 = 0.35;
const MARGIN: f32 = 22.0;
const TINT: [u8; 3] = [236, 214, 255];
const WAVE_Z: i32 = 11;
const FADE_IN: f32 = 0.12;
const FADE_OUT: f32 = 0.3;
/// Its dust: a floor this far under a sample of its front raises a puff of
/// [`DUST_SIZE`] and a burst of grit each [`DUST_EVERY`] px the sample
/// moves, at most [`DUST_MOST`] a wave, each puff living its clip.
const DUST_BELOW: f32 = 40.0;
const DUST_EVERY: f32 = 44.0;
const DUST_MOST: usize = 12;
const DUST_SIZE: Vec2 = Vec2::new(48.0, 32.0);
const DUST_SECS: f32 = 7.0 * 0.055;

/// The sector's half angle at `age`, widening linearly over the wave's life.
fn half_angle(age: f32) -> f32 {
    let t = (age / WAVE_SECS).clamp(0.0, 1.0);
    WAVE_HALF_FROM + (WAVE_HALF_TO - WAVE_HALF_FROM) * t
}

/// The share of the throw left `along` px from the front's start.
fn falloff(along: f32) -> f32 {
    1.0 - (1.0 - WAVE_FAR) * (along / WAVE_RANGE).clamp(0.0, 1.0)
}

/// Casts a wave from `centre`, the wizard's, along `aim`; a zero aim casts
/// it to the right.
pub fn cast(world: &mut World, centre: Vec2, aim: Vec2) {
    let aim = aim.try_normalize().unwrap_or(Vec2::X);
    let look = part(world, "ex10_push_wave", WAVE_Z);
    world.spawn_with((
        Wave {
            origin: centre + aim * WAVE_START,
            aim,
            age: 0.0,
            crossed: Vec::new(),
            look,
            dusted: [0.0; DUST_SAMPLES],
            puffs: 0,
        },
        SceneEntity { state_id: GAMEPLAY },
    ));
}

/// A sprite playing `clip` from its first frame at `z`, hidden, owned by
/// gameplay.
fn part(world: &mut World, clip: &str, z: i32) -> Option<Entity> {
    let asset = world
        .get_resource_mut::<AssetRegistry>()?
        .intern_sprite(&format!("{clip}_0"));
    let mut sprite = Sprite::new(asset);
    sprite.z_order = z;
    Some(world.spawn_with((
        Transform::from_position(Vec2::ZERO),
        sprite,
        Visibility { visible: false },
        AnimationState::new(clip),
        SceneEntity { state_id: GAMEPLAY },
    )))
}

/// Where the front, `r0` px from its start as the step begins and `r1` as
/// it ends, meets a target at `centre` moving at `velocity` and reaching
/// `extent` about its centre: the front's distance then, when the target is
/// ahead of it and meets it within the step, inside the sector widened by
/// its extent. Each target is taken at its meeting, so one running into the
/// front or crossing it on a slant is met between two steps too.
fn meets(wave: &Wave, r0: f32, r1: f32, centre: Vec2, velocity: Vec2, extent: f32) -> Option<f32> {
    let offset = centre - wave.origin;
    let along = offset.dot(wave.aim);
    let closing = WAVE_SPEED - velocity.dot(wave.aim);
    if along <= r0 || closing <= 0.0 {
        return None;
    }
    let t = (along - r0) / closing;
    if t > (r1 - r0) / WAVE_SPEED {
        return None;
    }
    let front = r0 + WAVE_SPEED * t;
    let across = wave.aim.perp_dot(offset) + wave.aim.perp_dot(velocity) * t;
    (across.abs() <= front * half_angle(wave.age + t).tan() + extent).then_some(front)
}

/// Half a collider's smallest extent: its radius, or its box's smaller half.
fn extent(collider: &Collider) -> f32 {
    match collider.shape {
        Shape::Circle { radius } => radius,
        Shape::Aabb { half_extents } => half_extents.min_element(),
    }
}

/// `toys_waves`, in the move phase: each wave's front moves on by the step,
/// and what it meets it pushes, once: a dynamic body over its mass, woken;
/// a live enemy through the kit's `knock_back` by its kind's weight, a
/// walker's input locked for [`WAVE_LOCK`]; a kind that stands firm (the
/// troll), which `knock_back` would leave standing, launched on its
/// controller past its resist. A wave ends at [`WAVE_SECS`] with its look.
pub fn waves(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let waves: Vec<Entity> = world
        .query::<(Entity, &Wave)>()
        .map(|(entity, _)| entity)
        .collect();
    for entity in waves {
        let Some(mut wave) = world.get::<Wave>(entity).cloned() else {
            continue;
        };
        let r0 = (wave.age * WAVE_SPEED).min(WAVE_RANGE);
        let r1 = ((wave.age + dt) * WAVE_SPEED).min(WAVE_RANGE);
        let bodies: Vec<(Entity, f32, f32)> = world
            .query::<(Entity, &RigidBody, &Position, &Collider, &Velocity)>()
            .filter(|(entity, body, ..)| {
                body.kind == BodyKind::Dynamic && !wave.crossed.contains(entity)
            })
            .filter_map(|(entity, body, position, collider, velocity)| {
                let centre = position.0 + collider.offset;
                let along = meets(&wave, r0, r1, centre, velocity.0, extent(collider))?;
                Some((entity, along, body.inv_mass))
            })
            .collect();
        for (body, along, inv_mass) in bodies {
            wave.crossed.push(body);
            let kick = (wave.aim * WAVE_SHOVE - Vec2::new(0.0, WAVE_BODY_HOP)) * falloff(along);
            if let Some(velocity) = world.get_mut::<Velocity>(body) {
                velocity.0 += kick * inv_mass;
            }
            wake(world, body);
        }
        let thrown: Vec<(Entity, Vec2)> = world
            .query::<(Entity, &Enemy, &Health)>()
            .filter(|(entity, _, health)| !health.is_dead() && !wave.crossed.contains(entity))
            .filter_map(|(entity, enemy, _)| {
                let info = enemy.kind.info();
                let velocity = world.get::<Velocity>(entity).map_or(Vec2::ZERO, |v| v.0);
                let centre = body_centre(world, entity)?;
                let along = meets(&wave, r0, r1, centre, velocity, info.half.min_element())?;
                // A walker leaves the ground on a low hop; a flyer takes
                // the aim alone.
                let hop = if info.flyer { 0.0 } else { WAVE_HOP };
                let push =
                    (wave.aim * WAVE_THROW - Vec2::new(0.0, hop)) * falloff(along) * info.wave;
                Some((entity, push))
            })
            .collect();
        for (enemy, push) in thrown {
            wave.crossed.push(enemy);
            if enemies::stands_firm(world, enemy) {
                if let Some(controller) = world.get_mut::<PlatformerController>(enemy) {
                    controller.launch(push);
                    controller.lock_input(WAVE_LOCK);
                }
            } else if knock_back(world, enemy, push)
                && let Some(controller) = world.get_mut::<PlatformerController>(enemy)
            {
                controller.lock_input(WAVE_LOCK);
            }
        }
        wave.age += dt;
        dust(world, &mut wave, r1);
        if wave.age >= WAVE_SECS {
            if let Some(look) = wave.look {
                world.despawn(look);
            }
            world.despawn(entity);
        } else if let Some(slot) = world.get_mut::<Wave>(entity) {
            *slot = wave;
        }
    }
}

/// The wave's dust where its front, `reach` px from its start, scrapes a
/// floor: each sample of the front with a tile or a static body within
/// [`DUST_BELOW`] under it raises a puff and a burst of grit each
/// [`DUST_EVERY`] px it moves, the puff's foot on the floor and its cloud
/// rolling along the aim.
fn dust(world: &mut World, wave: &mut Wave, reach: f32) {
    let filter = SpatialFilter::new();
    let half = half_angle(wave.age);
    let left = wave.aim.x < 0.0;
    for i in 0..DUST_SAMPLES {
        if wave.puffs >= DUST_MOST || reach - wave.dusted[i] < DUST_EVERY {
            continue;
        }
        let share = i as f32 / (DUST_SAMPLES - 1) as f32 * 2.0 - 1.0;
        let direction = Vec2::from_angle(share * half).rotate(wave.aim);
        let point = wave.origin + direction * reach;
        let Some(hit) = cast_ray(world, point, Vec2::Y, DUST_BELOW, &filter) else {
            continue;
        };
        let floor = hit.entity.is_none_or(|entity| {
            world
                .get::<RigidBody>(entity)
                .is_some_and(|body| body.kind == BodyKind::Static)
        });
        if !floor {
            continue;
        }
        wave.dusted[i] = reach;
        wave.puffs += 1;
        if let Some(puff) = part(world, "ex10_push_dust", WAVE_Z - 1) {
            // The art's foot is its bottom-left; a negative scale spans the
            // quad leftward from its point, so the foot stays on the floor.
            if let Some(transform) = world.get_mut::<Transform>(puff) {
                transform.scale = Vec2::new(if left { -1.0 } else { 1.0 }, 1.0);
                transform.position = hit.point - Vec2::new(0.0, DUST_SIZE.y - 2.0);
            }
            if let Some(visibility) = world.get_mut::<Visibility>(puff) {
                visibility.visible = true;
            }
            world.insert(puff, Lifetime::new(DUST_SECS));
        }
        spawn_burst(world, "ex10_push_grit", hit.point - Vec2::new(0.0, 2.0));
    }
}

/// `wave_look`, in the present phase: each wave's crescent with its seam at
/// the front, turned to the aim and mirrored top to bottom for a left one,
/// as the sprays are, growing longer, as wide as the sector there and
/// [`MARGIN`] more, fading in and out.
pub fn looks(world: &mut World) {
    let shown: Vec<(Entity, Vec2, f32, Vec2, u8)> = world
        .query::<&Wave>()
        .filter_map(|wave| {
            let look = wave.look?;
            let t = (wave.age / WAVE_SECS).clamp(0.0, 1.0);
            let reach = (wave.age * WAVE_SPEED).clamp(18.0, WAVE_RANGE);
            let width = 2.0 * reach * half_angle(wave.age).sin() + MARGIN;
            let along = 1.0 + GROWS * t;
            let flip = if wave.aim.x < 0.0 { -1.0 } else { 1.0 };
            let scale = Vec2::new(along, width / ART.y * flip);
            let front = wave.origin + wave.aim * reach;
            let centre = front - wave.aim * (ART_SEAM - ART.x * 0.5) * along;
            let fade = (t / FADE_IN).min(1.0) * ((1.0 - t) / FADE_OUT).clamp(0.0, 1.0);
            let angle = wave.aim.y.atan2(wave.aim.x);
            Some((look, centre, angle, scale, (255.0 * fade) as u8))
        })
        .collect();
    for (look, centre, angle, scale, alpha) in shown {
        if let Some(transform) = world.get_mut::<Transform>(look) {
            transform.rotation = angle;
            transform.scale = scale;
            transform.position = centre - ART * scale * 0.5;
        }
        if let Some(sprite) = world.get_mut::<Sprite>(look) {
            sprite.color = [TINT[0], TINT[1], TINT[2], alpha];
        }
        if let Some(visibility) = world.get_mut::<Visibility>(look) {
            visibility.visible = true;
        }
    }
}
