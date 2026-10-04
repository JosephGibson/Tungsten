//! `integrated` scene building in the startup hook: the assets
//! (`integrated/assets.rs`), then every entity and the runtime.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use tungsten::core::{
    AnimationState, AssetId, AssetRegistry, Collider, Easing, Light, MaterialRegistry,
    ParallaxLayer, ParticleConfig, ParticleEmitter, ParticleEmitterState, Pcg32, Position,
    RigidBody, Sprite, SpriteAssetId, SpriteSquashStretch, SquashTrigger, Transform, Tween,
    TweenChannel, TweenRepeat, Velocity, Visibility, World, splitmix64,
};
use tungsten::render::Renderer;

use super::assets::{
    self, CRATE, GEM, POLE, PROP_LIT, PROP_UNLIT, RIDGE_LAYERS, RIDGE_PX, SPRITE_PX, VARIANTS,
    WALK_FRAME_MS, WALK_FRAMES, ridge_id, walk_clip, walk_frame,
};
use super::level::{self, Level};
use super::runtime::{
    Actor, Body, JUMP_EVERY, Kind, Runtime, Torch, WALK_SPEED, WALKER_HALF, top_left,
};
use super::{CASTER_EVERY, CameraPath, PILE, Params};
use crate::view::VIEWPORT;

const FLASH_MATERIAL: &str = "damage_flash";
pub(super) const BOLT_SCALE: f32 = 0.5;
pub(super) const BOLT_TINT: [u8; 4] = [255, 170, 90, 255];
/// A spark emitter sits this far up-left of the impact, so its particles'
/// sprites center on it.
pub(super) const SPARK_ORIGIN: Vec2 = Vec2::splat(4.0);
const SCENE_SALT: u64 = 0x1E7E_3000_0000_0003;

// Draw order: parallax, then the tilemap (spliced in by the extract), then
// these. Particles always draw at z 0.
const RIDGE_Z: i32 = -40;
const PROP_BACK_Z: i32 = -10;
const TORCH_Z: i32 = -5;
const CRATE_Z: i32 = 1;
const PICKUP_Z: i32 = 2;
const ACTOR_Z: i32 = 5;
pub(super) const BOLT_Z: i32 = 6;
const PROP_FRONT_Z: i32 = 10;

/// Parallax layers, farthest first: scroll factor and on-screen top edge.
const RIDGES: [(f32, f32); RIDGE_LAYERS] =
    [(0.1, 250.0), (0.25, 370.0), (0.45, 490.0), (0.7, 610.0)];
const RIDGE_SCALE: Vec2 = Vec2::new(4.0, 2.5);
const RIDGE_TINTS: [[u8; 4]; 4] = [
    [150, 160, 200, 255],
    [170, 180, 210, 255],
    [190, 200, 220, 255],
    [215, 220, 230, 255],
];

const ACTOR_SPACING: f32 = 18.0;
const PROP_SPACING: f32 = 7.0;
const TORCH_SPACING: f32 = 64.0;
const PICKUP_SPACING: f32 = 24.0;
const PILE_SPACING: f32 = 112.0;
/// Walkers spawn at least this far from a crate pile's center.
const PILE_CLEARANCE: f32 = 52.0;
const PILE_ROWS: u32 = 4;
/// Extra pile layers, once every site holds one, drop from this high.
const PILE_LIFT: f32 = 80.0;
const CRATE_HALF: f32 = 8.0;
const CASTER_TINT: [u8; 4] = [255, 205, 205, 255];
const LAND_SQUASH: Vec2 = Vec2::new(1.3, 0.72);
const TORCH_COLOR: Vec3 = Vec3::new(1.0, 0.62, 0.28);
const TORCH_RADIUS: f32 = 170.0;
/// Pickups float this high over their surface and bob this far.
const PICKUP_FLOAT: f32 = 26.0;
const PICKUP_BOB: f32 = 8.0;
const PICKUP_SCALE: f32 = 0.6;

/// Startup hook: textures and registries, then the entities and the runtime.
pub(super) fn startup(
    world: &mut World,
    renderer: &mut Renderer,
    params: Params,
    level: Level,
    camera: CameraPath,
) {
    assets::register_textures(world, renderer);
    assets::register_tilemap(world, &level, params.tile_collision);
    assets::register_clips(world);
    let (fire, spark) = assets::register_particles(world);
    let flash = world
        .get_resource::<MaterialRegistry>()
        .and_then(|registry| registry.get(FLASH_MATERIAL));
    if flash.is_none() {
        log::warn!("integrated: no '{FLASH_MATERIAL}' material in the manifest; hits won't flash");
    }
    let parallax_page = world
        .get_resource::<AssetRegistry>()
        .and_then(|registry| registry.get_sprite(&ridge_id(0)))
        .map(|asset| asset.atlas);

    let mut rng = Pcg32::seeded(splitmix64(params.seed ^ SCENE_SALT));
    spawn_parallax(world, &camera);
    if !params.tile_collision {
        spawn_boxes(world, &level);
    }
    spawn_props(world, &level, params.props, &mut rng);
    spawn_torches(world, &level, params.torches, fire, &mut rng);
    spawn_pickups(world, &level, params.pickups, &mut rng);
    let mut runtime = Runtime::new(params, level, camera, Some(spark), flash, parallax_page);
    let piles = spawn_crates(world, &mut runtime, params.crates);
    spawn_actors(world, &mut runtime, &params, &piles, &mut rng);
    world.insert_resource(runtime);
}

/// The world registry's ID for sprite `name`.
fn sprite_id(world: &mut World, name: &str) -> SpriteAssetId {
    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .intern_sprite(name)
}

fn spawn_sprite(
    world: &mut World,
    position: Vec2,
    scale: Vec2,
    sprite: Sprite,
) -> tungsten::core::Entity {
    let entity = world.spawn();
    world.insert(
        entity,
        Transform {
            position,
            rotation: 0.0,
            scale,
        },
    );
    world.insert(entity, sprite);
    world.insert(entity, Visibility::default());
    entity
}

/// Ridge strips per layer, enough to cover the view anywhere on the
/// camera's path, shake included.
fn spawn_parallax(world: &mut World, camera: &CameraPath) {
    let strip = RIDGE_PX as f32 * RIDGE_SCALE;
    for (layer, &(factor, top)) in RIDGES.iter().enumerate() {
        let count =
            ((VIEWPORT.x + strip.x + factor * (camera.x1 - camera.x0)) / strip.x).ceil() as u32 + 1;
        let y = top + factor * camera.y;
        let x0 = factor * camera.x0 - strip.x * 0.5;
        for index in 0..count {
            let mut sprite = Sprite::new(sprite_id(world, &ridge_id(layer)));
            sprite.z_order = RIDGE_Z + layer as i32;
            sprite.color = RIDGE_TINTS[layer];
            let entity = spawn_sprite(
                world,
                Vec2::new(x0 + index as f32 * strip.x, y),
                RIDGE_SCALE,
                sprite,
            );
            world.insert(entity, ParallaxLayer::uniform(factor));
        }
    }
}

/// `tile_collision=off`: merged static boxes instead of per-tile proxies.
fn spawn_boxes(world: &mut World, level: &Level) {
    for (center, half) in level.boxes() {
        let entity = world.spawn();
        world.insert(entity, Position(center));
        world.insert(entity, RigidBody::r#static());
        world.insert(entity, Collider::aabb(half));
    }
}

/// Static decoration standing on the surfaces: three lit and three unlit
/// kinds, a quarter of them in front of the walkers.
fn spawn_props(world: &mut World, level: &Level, count: u32, rng: &mut Pcg32) {
    let slots = level.slots(PROP_SPACING, 4.0);
    for (index, (at, layer)) in level::spread(&slots, count as usize)
        .into_iter()
        .enumerate()
    {
        let kind = index % 6;
        let id = if kind < 3 {
            PROP_LIT[kind]
        } else {
            PROP_UNLIT[kind - 3]
        };
        let scale = rng.next_range(0.45, 0.9);
        let size = SPRITE_PX as f32 * scale;
        let mut sprite = Sprite::new(sprite_id(world, id));
        sprite.z_order = if index % 4 == 0 {
            PROP_FRONT_Z
        } else {
            PROP_BACK_Z
        };
        let position = Vec2::new(at.x - size * 0.5, at.y - size * (1.0 + layer as f32));
        spawn_sprite(world, position, Vec2::splat(scale), sprite);
    }
}

/// Torches: a pole, plus a flame entity carrying the point light and a fire
/// emitter whose accumulator starts staggered.
fn spawn_torches(
    world: &mut World,
    level: &Level,
    count: u32,
    fire: AssetId<ParticleConfig>,
    rng: &mut Pcg32,
) {
    let slots = level.slots(TORCH_SPACING, 16.0);
    for (index, (at, _)) in level::spread(&slots, count as usize)
        .into_iter()
        .enumerate()
    {
        let mut pole = Sprite::new(sprite_id(world, POLE));
        pole.z_order = TORCH_Z;
        pole.color = [150, 110, 80, 255];
        let pole_scale = Vec2::new(10.0, 30.0) / SPRITE_PX as f32;
        spawn_sprite(world, Vec2::new(at.x - 5.0, at.y - 30.0), pole_scale, pole);
        let flame = world.spawn();
        world.insert(
            flame,
            Transform::from_position(Vec2::new(at.x - 6.0, at.y - 38.0)),
        );
        let mut light = Light::point(TORCH_COLOR, TORCH_RADIUS);
        light.intensity = 1.1;
        world.insert(flame, light);
        world.insert(
            flame,
            ParticleEmitter::with_seed(fire, splitmix64(SCENE_SALT ^ (index as u64 + 1))),
        );
        world.insert(
            flame,
            ParticleEmitterState {
                continuous_accum: rng.next_f32_unit(),
                ..ParticleEmitterState::default()
            },
        );
        world.insert(
            flame,
            Torch {
                base: 1.1,
                rate: rng.next_range(7.0, 13.0),
                phase: rng.next_range(0.0, TAU),
            },
        );
    }
}

/// Gems floating over the surfaces, bobbing through ping-pong position tweens
/// that start at staggered phases.
fn spawn_pickups(world: &mut World, level: &Level, count: u32, rng: &mut Pcg32) {
    let slots = level.slots(PICKUP_SPACING, 12.0);
    let size = SPRITE_PX as f32 * PICKUP_SCALE;
    for (at, layer) in level::spread(&slots, count as usize) {
        let y = at.y - PICKUP_FLOAT - size - layer as f32 * 20.0;
        let mut sprite = Sprite::new(sprite_id(world, GEM));
        sprite.z_order = PICKUP_Z;
        sprite.color = [255, 230, 120, 255];
        let entity = spawn_sprite(
            world,
            Vec2::new(at.x - size * 0.5, y),
            Vec2::splat(PICKUP_SCALE),
            sprite,
        );
        let mut tween = Tween::new(rng.next_range(0.5, 0.9), Easing::SineInOut)
            .with_channel(TweenChannel::PositionY {
                from: y,
                to: y - PICKUP_BOB,
            })
            .with_repeat(TweenRepeat::PingPong);
        tween.elapsed = rng.next_f32_unit() * tween.duration;
        world.insert(entity, tween);
    }
}

/// Crate piles of up to `PILE` (rows of 4, 3, 2, 1) spread over the
/// surfaces; returns the pile sites, which walkers keep clear of.
fn spawn_crates(world: &mut World, runtime: &mut Runtime, count: u32) -> Vec<Vec2> {
    let sites = runtime.level.slots(PILE_SPACING, 48.0);
    let placed = level::spread(&sites, count.div_ceil(PILE) as usize);
    let side = 2.0 * CRATE_HALF + 0.5;
    let body = Body {
        half: Vec2::splat(CRATE_HALF),
        size: Vec2::splat(SPRITE_PX as f32),
    };
    let scale = Vec2::splat(2.0 * CRATE_HALF / SPRITE_PX as f32);
    let mut remaining = count;
    for &(site, layer) in &placed {
        let lift = layer as f32 * PILE_LIFT;
        'pile: for row in 0..PILE_ROWS {
            let across = PILE_ROWS - row;
            for col in 0..across {
                if remaining == 0 {
                    break 'pile;
                }
                remaining -= 1;
                let center = Vec2::new(
                    site.x + (col as f32 - (across - 1) as f32 * 0.5) * side,
                    site.y - CRATE_HALF - 0.25 - row as f32 * side - lift,
                );
                let mut sprite = Sprite::new(sprite_id(world, CRATE));
                sprite.z_order = CRATE_Z;
                let entity = spawn_sprite(world, top_left(center, body, scale), scale, sprite);
                world.insert(entity, Position(center));
                world.insert(entity, Velocity::default());
                world.insert(entity, RigidBody::dynamic().with_mass(2.0));
                world.insert(entity, Collider::aabb(Vec2::splat(CRATE_HALF)));
                world.insert(entity, body);
                runtime.set_kind(entity, Kind::Crate);
            }
        }
    }
    placed.into_iter().map(|(site, _)| site).collect()
}

/// Walkers spread over the surfaces clear of the crate piles, standing, at a
/// random clip position; every `CASTER_EVERY`-th is a caster.
fn spawn_actors(
    world: &mut World,
    runtime: &mut Runtime,
    params: &Params,
    piles: &[Vec2],
    rng: &mut Pcg32,
) {
    let mut slots = runtime.level.slots(ACTOR_SPACING, 10.0);
    slots.retain(|slot| {
        !piles
            .iter()
            .any(|pile| pile.y == slot.y && (pile.x - slot.x).abs() < PILE_CLEARANCE)
    });
    let period = if params.fire_rate > 0.0 {
        params.fire_rate.recip()
    } else {
        f32::INFINITY
    };
    let body = Body {
        half: WALKER_HALF,
        size: Vec2::splat(SPRITE_PX as f32),
    };
    let placed = level::spread(&slots, params.actors as usize);
    for (index, (at, layer)) in placed.into_iter().enumerate() {
        let variant = (index % VARIANTS as usize) as u8;
        let caster = (index as u32).is_multiple_of(CASTER_EVERY);
        let dir = if rng.next_u32() & 1 == 0 { -1.0 } else { 1.0 };
        let speed = rng.next_range(WALK_SPEED.0, WALK_SPEED.1);
        let center = Vec2::new(
            at.x,
            at.y - WALKER_HALF.y - 0.25 - layer as f32 * (2.0 * WALKER_HALF.y + 6.0),
        );
        let frame = (rng.next_u32() % WALK_FRAMES) as usize;
        let mut state = AnimationState::new(walk_clip(variant, false));
        state.frame_index = frame;
        state.accumulated_ms = rng.next_f32_unit() * WALK_FRAME_MS as f32;
        let mut sprite = Sprite::new(sprite_id(world, &walk_frame(variant, frame, false)));
        sprite.z_order = ACTOR_Z;
        if caster {
            sprite.color = CASTER_TINT;
        }
        let entity = spawn_sprite(world, top_left(center, body, Vec2::ONE), Vec2::ONE, sprite);
        world.insert(entity, Position(center));
        world.insert(entity, Velocity(Vec2::new(dir * speed, 0.0)));
        world.insert(entity, RigidBody::dynamic());
        world.insert(entity, Collider::aabb(WALKER_HALF));
        world.insert(entity, body);
        world.insert(entity, state);
        world.insert(
            entity,
            SpriteSquashStretch {
                on: SquashTrigger::OnLand,
                amount: LAND_SQUASH,
                duration: 0.18,
                easing: Easing::QuadOut,
            },
        );
        let jump = rng.next_range(0.0, JUMP_EVERY.1);
        let fire = if caster && period.is_finite() {
            rng.next_f32_unit() * period
        } else {
            f32::INFINITY
        };
        let slot = runtime
            .actors
            .push(entity, variant, dir, speed, caster, jump, fire);
        world.insert(entity, Actor { slot });
        runtime.set_kind(entity, Kind::Actor(slot));
    }
}
