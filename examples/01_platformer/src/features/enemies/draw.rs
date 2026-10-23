//! What the enemies draw over the sprites: each live troll's health bar
//! over its head, its fill the share of health left, and a beetle's tell
//! and charge: a glint swelling at its horn while it paws, three streaks
//! behind it while it charges. Each is flat colour: one white texel of a
//! particle sprite, tinted.

use glam::Vec2;
use tungsten::core::{AssetRegistry, SpriteAsset, Transform, World};
use tungsten::kit::{Health, StateMachine};
use tungsten::render::{SpriteBatch, SpriteInstance};

use super::beetle::PAW_SECS;
use super::components::{Enemy, EnemyKind, EnemyState};
use crate::features::level::draw::{instance, push_quad, view};

/// The bar's size, its gap over the troll's frame, and its back's and its
/// fill's colours.
pub const BAR_SIZE: Vec2 = Vec2::new(64.0, 6.0);
pub const BAR_RISE: f32 = 6.0;
pub const BAR_BACK: [u8; 4] = [24, 18, 22, 220];
pub const BAR_FILL: [u8; 4] = [196, 48, 40, 255];
/// The sprite whose white texel the bar samples, and that texel's centre.
pub const FLAT: &str = "ex10_mote";
const FLAT_TEXEL: Vec2 = Vec2::new(31.5, 31.5);

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
        let info = enemy.kind.info();
        // A left frame is its right one mirrored, drawn shifted ([`super::systems::clips`]).
        let body = if enemy.heading < 0.0 {
            info.frame - info.centre.x
        } else {
            info.centre.x
        };
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

/// A pawing beetle's glint: a square at its horn swelling to
/// [`GLINT_SIZE`] over its tell; a charging one's streaks behind it.
pub const GLINT_SIZE: f32 = 9.0;
pub const GLINT: [u8; 4] = [255, 214, 150, 255];
pub const STREAK: [u8; 4] = [226, 196, 150, 150];

/// Every pawing or charging beetle's marks in view: the glint at its horn's
/// tip, a little ahead and over its centre, and three streaks trailing its
/// rear at three heights, the middle one longest.
#[must_use]
pub fn charges(world: &World) -> Vec<SpriteBatch> {
    let Some(asset) = world
        .get_resource::<AssetRegistry>()
        .and_then(|assets| assets.get_sprite(FLAT))
    else {
        return Vec::new();
    };
    let (min, max) = view(world);
    let half = EnemyKind::Beetle.info().half;
    let mut batches = Vec::new();
    for (enemy, machine, at) in world.query::<(&Enemy, &StateMachine<EnemyState>, &Transform)>() {
        if enemy.kind != EnemyKind::Beetle {
            continue;
        }
        let info = enemy.kind.info();
        let body = if enemy.heading < 0.0 {
            info.frame - info.centre.x
        } else {
            info.centre.x
        };
        let centre = at.position + Vec2::new(body, info.centre.y);
        if (centre + half * 4.0).cmplt(min).any() || (centre - half * 4.0).cmpgt(max).any() {
            continue;
        }
        match machine.state() {
            EnemyState::WindUp => {
                let grown = (machine.elapsed() / PAW_SECS).clamp(0.0, 1.0);
                let side = 2.0 + (GLINT_SIZE - 2.0) * grown;
                let tip = centre + Vec2::new(enemy.heading * (half.x + 2.0), -half.y * 0.45);
                let mut color = GLINT;
                color[3] = (90.0 + 165.0 * grown) as u8;
                let quad = flat(
                    asset,
                    tip - Vec2::splat(side / 2.0),
                    Vec2::splat(side),
                    color,
                );
                push_quad(&mut batches, asset, quad, None);
            }
            EnemyState::Attack => {
                for (rise, length) in [(-0.35, 26.0), (0.0, 40.0), (0.35, 22.0)] {
                    let rear = centre.x - enemy.heading * (half.x + 4.0);
                    let x0 = if enemy.heading > 0.0 {
                        rear - length
                    } else {
                        rear
                    };
                    let origin = Vec2::new(x0, centre.y + rise * half.y);
                    let quad = flat(asset, origin, Vec2::new(length, 2.0), STREAK);
                    push_quad(&mut batches, asset, quad, None);
                }
            }
            _ => {}
        }
    }
    batches
}
