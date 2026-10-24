//! The title's night (01 (h)): while no run is live, the title draws 01's
//! sky in OLED black (`draw::backdrop`) over a view low under the start's,
//! looking up, with stars twinkling over its upper half and a shooting
//! star every few seconds. The stars' emitters belong to the title's state,
//! so they leave when a run replaces it and stay under a screen pushed over
//! it; their particles, which no state owns, go the first frame a run is
//! live, under the curtain.

use glam::Vec2;
use tungsten::core::{
    CameraController, CameraState, Entity, Particle, ParticleConfigRegistry, ParticleEmitter,
    ParticleEmitterState, Time, Transform, World,
};
use tungsten::{SceneEntity, WindowSize};

use super::components::SceneTime;
use super::draw::START_VIEW;
use super::{CAMERA_ROWS, TILE};
use crate::shell::{self, title::TITLE};

/// The title's sky, while no run is live: its own clock, in real seconds,
/// the shooting stars it has sent, and the one in flight, with its start
/// and the clock at its birth.
#[derive(Debug, Clone, Copy, Default)]
pub struct TitleSky {
    pub clock: f32,
    pub meteors: u32,
    pub meteor: Option<(Entity, Vec2, f32)>,
}

/// The title's stars: the twinkles' and the shooting star's configs, the
/// star both draw, and how many twinkles it sends.
pub const TWINKLE: &str = "ex10_title_twinkle";
pub const METEOR: &str = "ex10_title_meteor";
pub const TITLE_STAR: &str = "ex10_title_star";
pub const TWINKLES: u32 = 40;
/// How many tile rows above the start's view the title's view stands, so
/// the backdrop's lift sets the treeline on its foot and the sky fills the
/// rest: the view low, looking up.
pub const SKY_LIFT: f32 = 80.0;
/// A shooting star: one every so many seconds, crossing down and right at
/// this speed for this long.
const METEOR_EVERY: f32 = 4.0;
const METEOR_SPEED: f32 = 1500.0;
const METEOR_SECS: f32 = 0.7;
const METEOR_HEADING: Vec2 = Vec2::new(1.0, 0.42);

/// The world box the title holds: its corner and its size, the base
/// zoom's [`CAMERA_ROWS`] rows tall, [`SKY_LIFT`] rows above the start's
/// view.
#[must_use]
pub fn sky_view(world: &World) -> (Vec2, Vec2) {
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1280,
            height: 720,
        });
    let height = CAMERA_ROWS * TILE;
    let width = height * window.width as f32 / window.height.max(1) as f32;
    let corner = Vec2::new(0.0, (START_VIEW - SKY_LIFT) * TILE - height * 0.5);
    (corner, Vec2::new(width, height))
}

/// A value in [0, 1) hashed from `a` and `b`: a star's place.
fn unit(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9e37_79b9) ^ b.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h % 10_000) as f32 / 10_000.0
}

/// An emitter of the config `name` at `at`, seeded, owned by the title.
fn emitter(world: &mut World, name: &str, at: Vec2, seed: u64) -> Option<Entity> {
    let config = world
        .get_resource::<ParticleConfigRegistry>()?
        .id_for_name(name)?;
    Some(world.spawn_with((
        Transform::from_position(at),
        ParticleEmitter::with_seed(config, seed),
        ParticleEmitterState::default(),
        SceneEntity { state_id: TITLE },
    )))
}

/// The emitters of the config `name`.
fn emitters(world: &World, name: &str) -> Vec<Entity> {
    let config = world
        .get_resource::<ParticleConfigRegistry>()
        .and_then(|configs| configs.id_for_name(name));
    world
        .query::<(Entity, &ParticleEmitter)>()
        .filter(|(_, emitter)| Some(emitter.config) == config)
        .map(|(entity, _)| entity)
        .collect()
}

/// `title_sky`, in `Update`: while no run is live the title's sky runs its
/// clock; the first frame the title is on top with no twinkles it sends
/// [`TWINKLES`] at hashed points over its view's upper half, and while it
/// is on top a shooting star every [`METEOR_EVERY`] seconds, which crosses
/// down and right and goes. The first frame a run is live the sky goes,
/// and with it the stars' emitters and every particle they left.
pub fn sky(world: &mut World) {
    if world.has_resource::<SceneTime>() {
        if world.remove_resource::<TitleSky>().is_some() {
            clear(world);
        }
        return;
    }
    let dt = world.resource::<Time>().real_delta();
    let mut sky = world.remove_resource::<TitleSky>().unwrap_or_default();
    sky.clock += dt;
    let on_top = shell::top(world) == Some(TITLE);
    let (corner, size) = sky_view(world);
    if on_top && emitters(world, TWINKLE).is_empty() {
        for i in 0..TWINKLES {
            let at = corner
                + Vec2::new(
                    (i as f32 + unit(i, 1)) / TWINKLES as f32 * size.x,
                    (0.04 + 0.5 * unit(i, 2)) * size.y,
                );
            emitter(world, TWINKLE, at, 0x7100 + u64::from(i));
        }
    }
    if let Some((meteor, start, born)) = sky.meteor {
        let t = sky.clock - born;
        if t > METEOR_SECS || !world.is_alive(meteor) {
            if world.is_alive(meteor) {
                world.despawn(meteor);
            }
            sky.meteor = None;
        } else if let Some(at) = world.get_mut::<Transform>(meteor) {
            at.position = start + METEOR_HEADING.normalize() * METEOR_SPEED * t;
        }
    }
    if on_top && sky.meteor.is_none() && sky.clock >= (sky.meteors + 1) as f32 * METEOR_EVERY {
        let k = sky.meteors;
        let start = corner + Vec2::new((0.15 + 0.5 * unit(k, 7)) * size.x, 0.06 * size.y);
        sky.meteors += 1;
        sky.meteor = emitter(world, METEOR, start, 0x7200 + u64::from(k))
            .map(|meteor| (meteor, start, sky.clock));
    }
    world.insert_resource(sky);
}

/// The title's stars gone: their emitters, should any be left, and every
/// particle that draws [`TITLE_STAR`].
fn clear(world: &mut World) {
    let mut gone = emitters(world, TWINKLE);
    gone.extend(emitters(world, METEOR));
    gone.extend(
        world
            .query::<(Entity, &Particle)>()
            .filter(|(_, particle)| particle.config.sprite == TITLE_STAR)
            .map(|(entity, _)| entity),
    );
    for entity in gone {
        world.despawn(entity);
    }
}

/// `title_view`, after the level's zoom and before the camera's update:
/// while the title is on top, the camera holds the title's view
/// ([`sky_view`]) at the base zoom, unbounded.
pub fn view(world: &mut World) {
    if !world.has_resource::<TitleSky>() || shell::top(world) != Some(TITLE) {
        return;
    }
    let (corner, size) = sky_view(world);
    let height = world
        .get_resource::<WindowSize>()
        .map_or(720.0, |window| window.height as f32);
    if let Some(controller) = world.get_resource_mut::<CameraController>() {
        controller.bounds = None;
        controller.zoom_multiplier = height / size.y;
    }
    if let Some(camera) = world.get_resource_mut::<CameraState>() {
        camera.position = corner;
    }
}
