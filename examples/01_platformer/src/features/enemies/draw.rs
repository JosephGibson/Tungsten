//! What the enemies draw over the sprites: each live troll's health bar
//! over its head, its fill the share of health left; a troll's slam's dust
//! ring growing on the ground over its tell; each flat colour, one white
//! texel of a particle sprite, tinted. A charging beetle's streaks behind
//! it, their own clip's frames. And each enemy's mark over its frame: "!"
//! after a fresh alert, "?" through a search. Under F1, the colliders'
//! overlay, the kit's debug shapes draw too, with 01's own blows beside its
//! armed hitboxes while they land ([`debug_boxes`]).

use std::f32::consts::TAU;

use glam::Vec2;
use tungsten::PhysicsDebugOverlay;
use tungsten::core::{
    AssetRegistry, DebugDraw, Entity, Position, PrevPosition, SpriteAsset, Time, Transform, World,
};
use tungsten::kit::{Health, KitDebug, StateMachine};
use tungsten::render::{SpriteBatch, SpriteInstance};

use super::beetle::{self, PLOUGH_HALF};
use super::components::{Brain, Enemy, EnemyKind, EnemyState, Strike};
use super::goblin::{self, STAB_HALF};
use super::troll::{self, FIST, SLAM_REACH, SLAM_TELL, SWIPE_HALF};
use super::{KindInfo, SEARCH_MARK_AFTER};
use crate::features::level::draw::{instance, push_quad, view};
use crate::features::level::systems::drawn_time;

/// The bar's size, its gap over the troll's frame, and its back's and its
/// fill's colours.
pub const BAR_SIZE: Vec2 = Vec2::new(64.0, 6.0);
pub const BAR_RISE: f32 = 6.0;
pub const BAR_BACK: [u8; 4] = [24, 18, 22, 220];
pub const BAR_FILL: [u8; 4] = [196, 48, 40, 255];
/// The sprite whose white texel the bar samples, and that texel's centre.
pub const FLAT: &str = "ex10_mote";
const FLAT_TEXEL: Vec2 = Vec2::new(31.5, 31.5);

/// The x of `enemy`'s body in its frame as it is drawn: a left frame is its
/// right one mirrored, drawn shifted ([`super::systems::clips`]).
fn body_x(info: KindInfo, heading: f32) -> f32 {
    if heading < 0.0 {
        info.frame - info.centre.x
    } else {
        info.centre.x
    }
}

/// A quad of `asset`'s white texel over the box at `origin` of `size`:
/// flat `color`.
fn flat(asset: &SpriteAsset, origin: Vec2, size: Vec2, color: [u8; 4]) -> SpriteInstance {
    let mut quad = instance(asset, origin, size, color);
    let texel = FLAT_TEXEL / Vec2::new(asset.width as f32, asset.height as f32);
    quad.uv_min = (Vec2::from(quad.uv_min) + Vec2::from(quad.uv_size) * texel).to_array();
    quad.uv_size = [0.0, 0.0];
    quad
}

/// Every live troll's bar in view: its back centred over its body as it is
/// drawn, [`BAR_RISE`] over its frame, and its fill from the left.
#[must_use]
pub fn bars(world: &World) -> Vec<SpriteBatch> {
    let Some(asset) = world
        .get_resource::<AssetRegistry>()
        .and_then(|assets| assets.get_sprite(FLAT))
    else {
        return Vec::new();
    };
    let (min, max) = view(world);
    let mut batches = Vec::new();
    for (enemy, health, at) in world.query::<(&Enemy, &Health, &Transform)>() {
        if enemy.kind != EnemyKind::Troll || health.is_dead() {
            continue;
        }
        let body = body_x(enemy.kind.info(), enemy.heading);
        let origin = at.position + Vec2::new(body - BAR_SIZE.x / 2.0, -BAR_RISE - BAR_SIZE.y);
        if (origin + BAR_SIZE).cmplt(min).any() || origin.cmpgt(max).any() {
            continue;
        }
        push_quad(
            &mut batches,
            asset,
            flat(asset, origin, BAR_SIZE, BAR_BACK),
            None,
        );
        let share = (health.current() / health.max()).clamp(0.0, 1.0);
        if share > 0.0 {
            let inner = BAR_SIZE - Vec2::splat(2.0);
            let fill = Vec2::new(inner.x * share, inner.y);
            push_quad(
                &mut batches,
                asset,
                flat(asset, origin + Vec2::ONE, fill, BAR_FILL),
                None,
            );
        }
    }
    batches
}

/// A charging beetle's streaks: [`STREAK_FRAMES`] frames of `<STREAK>_<f>`
/// (`_<f>_left` facing left), each [`STREAK_FRAME_SECS`] by the drawn time,
/// their edge nearest it centred on its rear's point, [`STREAK_BEHIND`]
/// past its body.
pub const STREAK: &str = "ex10_charge_streak";
pub const STREAK_FRAMES: usize = 4;
pub const STREAK_FRAME_SECS: f32 = 0.05;
pub const STREAK_BEHIND: f32 = 4.0;

/// Every charging beetle's streaks in view, behind its rear.
#[must_use]
pub fn charges(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let (min, max) = view(world);
    let half = EnemyKind::Beetle.info().half;
    let frame = (drawn_time(world) / STREAK_FRAME_SECS) as usize % STREAK_FRAMES;
    let mut batches = Vec::new();
    for (enemy, machine, at) in world.query::<(&Enemy, &StateMachine<EnemyState>, &Transform)>() {
        if enemy.kind != EnemyKind::Beetle || machine.state() != EnemyState::Attack {
            continue;
        }
        let side = if enemy.heading < 0.0 { "_left" } else { "" };
        let Some(asset) = assets.get_sprite(&format!("{STREAK}_{frame}{side}")) else {
            continue;
        };
        let info = enemy.kind.info();
        let centre = at.position + Vec2::new(body_x(info, enemy.heading), info.centre.y);
        let size = Vec2::new(asset.width as f32, asset.height as f32);
        let rear = centre.x - enemy.heading * (half.x + STREAK_BEHIND);
        let left = if enemy.heading > 0.0 {
            rear - size.x
        } else {
            rear
        };
        let origin = Vec2::new(left, centre.y - size.y / 2.0);
        if (origin + size).cmplt(min).any() || origin.cmpgt(max).any() {
            continue;
        }
        let quad = instance(asset, origin, size, [255; 4]);
        push_quad(&mut batches, asset, quad, None);
    }
    batches
}

/// A slam's ring: its colour, how flat it lies (its height over its
/// width), its dots' size and the pixels between them along it.
pub const RING: [u8; 4] = [190, 168, 132, 200];
pub const RING_FLAT: f32 = 0.12;
const RING_DOT: Vec2 = Vec2::new(3.0, 2.0);
const RING_GAP: f32 = 5.0;

/// Each troll winding up its slam in view: a ring of dust on the ground
/// about its fist's point, where the slam is measured, growing from nothing
/// to [`SLAM_REACH`] over its tell, flat as ground seen from the side, and
/// firming as it grows.
#[must_use]
pub fn rings(world: &World) -> Vec<SpriteBatch> {
    let Some(asset) = world
        .get_resource::<AssetRegistry>()
        .and_then(|assets| assets.get_sprite(FLAT))
    else {
        return Vec::new();
    };
    let (min, max) = view(world);
    let mut batches = Vec::new();
    for (entity, enemy, machine, at) in
        world.query::<(Entity, &Enemy, &StateMachine<EnemyState>, &Transform)>()
    {
        let slamming = enemy.kind == EnemyKind::Troll
            && machine.state() == EnemyState::WindUp
            && troll::strike(world, entity) == Strike::Slam;
        if !slamming {
            continue;
        }
        let grown = (machine.elapsed() / SLAM_TELL).clamp(0.0, 1.0);
        let reach = SLAM_REACH * grown;
        if reach < 2.0 {
            continue;
        }
        let info = enemy.kind.info();
        let centre = at.position + Vec2::new(body_x(info, enemy.heading), info.centre.y);
        let fist = centre + Vec2::new(FIST.x * enemy.heading, FIST.y);
        let span = Vec2::new(reach, reach * RING_FLAT) + RING_DOT;
        if (fist + span).cmplt(min).any() || (fist - span).cmpgt(max).any() {
            continue;
        }
        let mut color = RING;
        color[3] = (f32::from(RING[3]) * (0.4 + 0.6 * grown)) as u8;
        let dots = ((TAU * reach / RING_GAP) as usize).max(8);
        for i in 0..dots {
            let turn = TAU * i as f32 / dots as f32;
            let point = fist + Vec2::new(reach * turn.cos(), reach * RING_FLAT * turn.sin());
            let quad = flat(asset, point - RING_DOT / 2.0, RING_DOT, color);
            push_quad(&mut batches, asset, quad, None);
        }
    }
    batches
}

/// The marks' sprites, their side, their rise over the frame and the
/// alert's bob.
pub const MARK_ALERT: &str = "ex10_mark_alert";
pub const MARK_SEARCH: &str = "ex10_mark_search";
pub const MARK_SIDE: f32 = 16.0;
pub const MARK_RISE: f32 = 6.0;
const MARK_BOB: f32 = 2.0;

/// The mark `enemy` shows: "!" while a fresh alert's
/// [`super::ALERT_MARK_SECS`] run, past its alert into the chase; else "?" through a search past its
/// first [`SEARCH_MARK_AFTER`], so a chase that misses one sight check
/// blinks nothing; none for a dead one.
#[must_use]
pub fn mark(world: &World, enemy: Entity) -> Option<&'static str> {
    if world.get::<Health>(enemy).is_none_or(Health::is_dead) {
        return None;
    }
    if world
        .get::<Brain>(enemy)
        .is_some_and(|brain| brain.alerted > 0.0)
    {
        return Some(MARK_ALERT);
    }
    let machine = world.get::<StateMachine<EnemyState>>(enemy)?;
    let searching =
        machine.state() == EnemyState::Search && machine.elapsed() >= SEARCH_MARK_AFTER - 1e-4;
    searching.then_some(MARK_SEARCH)
}

/// Each enemy's mark in view, [`MARK_RISE`] over its frame and centred on
/// its body as it is drawn, over a troll's bar; an alert's bobbing
/// [`MARK_BOB`].
#[must_use]
pub fn marks(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let (min, max) = view(world);
    let time = drawn_time(world);
    let mut batches = Vec::new();
    for (entity, enemy, at) in world.query::<(Entity, &Enemy, &Transform)>() {
        let Some(name) = mark(world, entity) else {
            continue;
        };
        let Some(asset) = assets.get_sprite(name) else {
            continue;
        };
        let bar = if enemy.kind == EnemyKind::Troll {
            BAR_RISE + BAR_SIZE.y
        } else {
            0.0
        };
        let bob = if name == MARK_ALERT {
            MARK_BOB * (time * TAU).sin().abs()
        } else {
            0.0
        };
        let body = body_x(enemy.kind.info(), enemy.heading);
        let rise = MARK_RISE + bar + MARK_SIDE + bob;
        let origin = at.position + Vec2::new(body - MARK_SIDE / 2.0, -rise);
        if (origin + Vec2::splat(MARK_SIDE)).cmplt(min).any() || origin.cmpgt(max).any() {
            continue;
        }
        let quad = instance(asset, origin, Vec2::splat(MARK_SIDE), [255; 4]);
        push_quad(&mut batches, asset, quad, None);
    }
    batches
}

/// The kit's armed hitbox under its debug drawing, its deep red and its
/// line's width, which 01's own blows draw in.
pub const HIT_COLOR: [f32; 4] = [0.8, 0.02, 0.06, 1.0];
pub const HIT_THICKNESS: f32 = 1.5;

/// The blow `enemy` is landing: its strike while it is in its attack, the
/// window its blow's test opens; none at any other time.
fn blow(world: &World, enemy: Entity) -> Option<Strike> {
    let attacking = world
        .get::<StateMachine<EnemyState>>(enemy)
        .is_some_and(|machine| machine.state() == EnemyState::Attack);
    attacking
        .then(|| world.get::<Brain>(enemy).and_then(|brain| brain.strike))
        .flatten()
}

/// `enemy_debug_boxes`, in the present phase before the kit's debug draw:
/// the kit's debug shapes follow the colliders' overlay (F1), and while it
/// is on each live goblin's stab box, each live troll's swipe box or its
/// slam's reach about its fist and each live beetle's plough box draw in
/// [`HIT_COLOR`], as the kit draws an armed hitbox, while that blow's
/// attack runs: 01's blows are box tests of its own, not kit hitboxes.
/// Each box is drawn where its enemy's sprite is ([`lag`]), not a step
/// ahead at the point its test reads.
pub fn debug_boxes(world: &mut World) {
    let on = world
        .get_resource::<PhysicsDebugOverlay>()
        .is_some_and(|overlay| overlay.enabled);
    if let Some(debug) = world.get_resource_mut::<KitDebug>() {
        debug.set_enabled(on);
    }
    if !on {
        return;
    }
    let Some(mut draw) = world.remove_resource::<DebugDraw>() else {
        return;
    };
    for (enemy, mine, health) in world.query::<(Entity, &Enemy, &Health)>() {
        if health.is_dead() {
            continue;
        }
        let lag = lag(world, enemy);
        let mut outline = |at: Vec2, half: Vec2| {
            let at = at + lag;
            draw.draw_aabb(at - half, at + half, HIT_COLOR, HIT_THICKNESS);
        };
        let charging = world
            .get::<StateMachine<EnemyState>>(enemy)
            .is_some_and(|machine| machine.state() == EnemyState::Attack);
        if mine.kind == EnemyKind::Beetle
            && charging
            && let Some(at) = beetle::plough_box(world, enemy)
        {
            outline(at, PLOUGH_HALF);
        }
        match blow(world, enemy) {
            Some(Strike::Stab) => {
                if let Some(at) = goblin::stab_box(world, enemy) {
                    outline(at, STAB_HALF);
                }
            }
            Some(Strike::Swipe) => {
                if let Some(at) = troll::swipe_box(world, enemy) {
                    outline(at, SWIPE_HALF);
                }
            }
            Some(Strike::Slam) => {
                if let Some(fist) = troll::fist(world, enemy) {
                    draw.draw_circle(fist + lag, SLAM_REACH, HIT_COLOR, HIT_THICKNESS);
                }
            }
            _ => {}
        }
    }
    world.insert_resource(draw);
}

/// How far `enemy` is drawn from its `Position`: back toward its
/// [`PrevPosition`] by `1 - alpha` of its last step while
/// [`Time::interpolate`] is on, where `physics_sync` puts its sprite.
fn lag(world: &World, enemy: Entity) -> Vec2 {
    let alpha = world
        .get_resource::<Time>()
        .filter(|time| time.interpolate())
        .map(Time::alpha);
    match (
        alpha,
        world.get::<Position>(enemy),
        world.get::<PrevPosition>(enemy),
    ) {
        (Some(alpha), Some(at), Some(prev)) => (prev.0 - at.0) * (1.0 - alpha),
        _ => Vec2::ZERO,
    }
}
