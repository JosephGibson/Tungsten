//! The sprays drawn as animated sprites (01 (h)): each held spray's muzzle
//! flare at the staff with a light at the flame, its impact where the cone
//! meets something, the ice beam's core tiled down the cone, and the
//! stream's puffs, each flying until its own line meets something and
//! playing out there. Self-lit art, at [`SPRAY_Z`] with the particles,
//! which stay as accents (`systems::spray_look`).

use glam::{Vec2, Vec3};
use tungsten::SceneEntity;
use tungsten::core::{
    AnimationRegistry, AnimationState, AssetRegistry, Entity, Light, Sprite, Time, Transform,
    Visibility, World,
};

use super::SPRAY_HALF_ANGLE;
use super::components::{Element, Spray, SprayLook, SprayPuff};
use super::systems::{SPRAY_MUZZLE, SPRAY_Z, SolidTiles, reach, trace_among, traced_near};
use crate::game::GAMEPLAY;

/// A spray's look by its element: its muzzle clip, its art's size and how
/// far ahead of the spray's start its centre sits, so the flare's root is
/// at the start; its splash, its point this far behind the art's centre;
/// its stream's puff clip, one every `every` game seconds at `speed`
/// px/s, growing from `grow.0` to `grow.1` times its art over its clip's
/// `life`; and its light, `ahead` of the start.
struct Style {
    muzzle: &'static str,
    muzzle_size: Vec2,
    muzzle_ahead: f32,
    splash: &'static str,
    puff: &'static str,
    every: f32,
    speed: f32,
    life: f32,
    grow: (f32, f32),
    color: Vec3,
    reach: f32,
    intensity: f32,
    light_ahead: f32,
}

const FLAME: Style = Style {
    muzzle: "ex10_flame_muzzle",
    muzzle_size: Vec2::new(40.0, 32.0),
    muzzle_ahead: 17.0,
    splash: "ex10_flame_splash",
    puff: "ex10_flame_puff",
    every: 0.012,
    speed: 1170.0,
    life: 0.32,
    grow: (0.9, 1.6),
    color: Vec3::new(1.0, 0.6, 0.26),
    reach: 190.0,
    intensity: 1.8,
    light_ahead: 40.0,
};

const FROST: Style = Style {
    muzzle: "ex10_frost_muzzle",
    muzzle_size: Vec2::new(32.0, 32.0),
    muzzle_ahead: 8.0,
    splash: "ex10_frost_splash",
    puff: "ex10_frost_puff",
    every: 0.018,
    speed: 1600.0,
    life: 0.36,
    grow: (1.0, 2.2),
    color: Vec3::new(0.55, 0.85, 1.0),
    reach: 160.0,
    intensity: 1.2,
    light_ahead: 24.0,
};

fn style(element: Element) -> &'static Style {
    match element {
        Element::Fire => &FLAME,
        Element::Cold => &FROST,
    }
}

/// The splashes' and the puffs' art, 48 px; the splash's point sits this
/// far ahead of its centre.
const SPLASH_SIZE: f32 = 48.0;
const SPLASH_AHEAD: f32 = 12.0;
const PUFF_SIZE: f32 = 48.0;
/// The ice's core: its clip, its tiles' art, laid end to end from the
/// start, drawn this many times as tall, on one frame of this many
/// milliseconds stepped by the spray's age.
const CORE: &str = "ex10_frost_core";
const CORE_SIZE: Vec2 = Vec2::new(32.0, 14.0);
const CORE_TALL: f32 = 1.5;
const CORE_FRAME_MS: f32 = 40.0;
/// A puff's spread, a share of the cone's half angle, its speed's spread,
/// and how far short of the surface its line meets it stops.
const PUFF_SPREAD: f32 = 0.75;
const PUFF_SPEED_SPREAD: f32 = 0.16;
const PUFF_SHORT: f32 = 6.0;
/// A young puff's streak along its line: its length's extra at its birth.
const STREAK: f32 = 0.9;

/// Gives the spray `spray` of `element` its look: its muzzle, its light
/// and its impact, hidden until its first frame places them.
pub(super) fn start(world: &mut World, spray: Entity, element: Element) {
    let style = style(element);
    let mut light = Light::point(style.color, style.reach);
    light.intensity = style.intensity;
    let light = world.spawn_with((
        light,
        Transform::from_position(Vec2::ZERO),
        SceneEntity { state_id: GAMEPLAY },
    ));
    let look = SprayLook {
        muzzle: part(world, style.muzzle, SPRAY_Z + 1),
        light: Some(light),
        impact: part(world, style.splash, SPRAY_Z + 1),
        ..SprayLook::default()
    };
    world.insert(spray, look);
}

/// Takes the spray `spray`'s look with it: its muzzle, its light, its
/// impact and its core; its puffs in flight play out.
pub(super) fn end(world: &mut World, spray: Entity) {
    let Some(look) = world.get::<SprayLook>(spray).cloned() else {
        return;
    };
    for part in look
        .muzzle
        .into_iter()
        .chain(look.light)
        .chain(look.impact)
        .chain(look.core)
    {
        world.despawn(part);
    }
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

/// Puts the sprite `entity`, art `size` across, centred on `centre`,
/// turned to `angle` and drawn `scale` times its art, mirrored top to
/// bottom by a negative `scale.y`; shown.
fn place(world: &mut World, entity: Entity, centre: Vec2, size: Vec2, angle: f32, scale: Vec2) {
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.rotation = angle;
        transform.scale = scale;
        transform.position = centre - size * scale * 0.5;
    }
    show(world, entity, true);
}

fn show(world: &mut World, entity: Entity, visible: bool) {
    if let Some(visibility) = world.get_mut::<Visibility>(entity) {
        visibility.visible = visible;
    }
}

/// A value in [0, 1) hashed from `n`: a puff's spread and speed.
fn hash01(n: u32) -> f32 {
    let x = n
        .wrapping_mul(0x9e37_79b1)
        .rotate_left(13)
        .wrapping_mul(0x85eb_ca6b);
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// `spray_sprites`, after `spray_look`: each spray's muzzle and light at
/// its start, its impact at its contact while it has one, the ice's core
/// tiles laid to its end and stepped by its age, and the stream's puffs
/// spawned by the game clock, each along a line within the cone that stops
/// where it meets something.
pub fn sprites(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let sprays: Vec<(Entity, Spray)> = world
        .query::<(Entity, &Spray)>()
        .map(|(entity, spray)| (entity, *spray))
        .collect();
    for (entity, spray) in sprays {
        let Some(mut look) = world.get::<SprayLook>(entity).cloned() else {
            continue;
        };
        let style = style(spray.element);
        let angle = spray.aim.y.atan2(spray.aim.x);
        let flip = if spray.aim.x < 0.0 { -1.0 } else { 1.0 };
        let upright = Vec2::new(1.0, flip);
        if let Some(muzzle) = look.muzzle {
            let centre = spray.start + spray.aim * style.muzzle_ahead;
            place(world, muzzle, centre, style.muzzle_size, angle, upright);
        }
        if let Some(light) = look.light
            && let Some(transform) = world.get_mut::<Transform>(light)
        {
            transform.position = spray.start + spray.aim * style.light_ahead;
        }
        if let Some(impact) = look.impact {
            match spray.contact {
                Some(at) => {
                    let centre = at - spray.aim * SPLASH_AHEAD;
                    place(
                        world,
                        impact,
                        centre,
                        Vec2::splat(SPLASH_SIZE),
                        angle,
                        upright,
                    );
                }
                None => show(world, impact, false),
            }
        }
        if spray.element == Element::Cold {
            lay_core(world, &mut look, &spray, angle, flip);
        }
        look.due += dt;
        if look.due >= style.every {
            let range = reach(spray.element) - SPRAY_MUZZLE;
            let lines: Vec<(Vec2, f32, f32)> = {
                let bodies = traced_near(world, spray.caster, spray.start, range);
                let tiles = SolidTiles::of(world);
                let mut lines = Vec::new();
                while look.due >= style.every {
                    look.due -= style.every;
                    look.puffs = look.puffs.wrapping_add(1);
                    let turn = (hash01(look.puffs) * 2.0 - 1.0) * SPRAY_HALF_ANGLE * PUFF_SPREAD;
                    let direction = Vec2::from_angle(turn).rotate(spray.aim);
                    let (end, _) = trace_among(&bodies, &tiles, spray.start, direction, range);
                    let stop = (spray.start.distance(end) - PUFF_SHORT).max(0.0);
                    let jitter = 1.0 - PUFF_SPEED_SPREAD / 2.0
                        + PUFF_SPEED_SPREAD * hash01(look.puffs ^ 0x5bd1);
                    lines.push((direction, stop, style.speed * jitter));
                }
                lines
            };
            for (direction, stop, speed) in lines {
                if let Some(puff) = part(world, style.puff, SPRAY_Z) {
                    world.insert(
                        puff,
                        SprayPuff {
                            origin: spray.start,
                            direction,
                            speed,
                            stop,
                            age: 0.0,
                            life: style.life,
                            grow: style.grow,
                        },
                    );
                }
            }
        }
        world.insert(entity, look);
    }
}

/// The ice's core: as many tiles as reach from the spray's start to its
/// end, each centred along the aim, drawn [`CORE_TALL`] times as tall, all
/// on the frame the spray's age has reached.
fn lay_core(world: &mut World, look: &mut SprayLook, spray: &Spray, angle: f32, flip: f32) {
    let length = spray.start.distance(spray.end);
    let wanted = ((length - 4.0) / CORE_SIZE.x).ceil().max(0.0) as usize;
    while look.core.len() < wanted {
        match part(world, CORE, SPRAY_Z) {
            Some(tile) => {
                if let Some(state) = world.get_mut::<AnimationState>(tile) {
                    state.playing = false;
                }
                look.core.push(tile);
            }
            None => break,
        }
    }
    while look.core.len() > wanted {
        if let Some(tile) = look.core.pop() {
            world.despawn(tile);
        }
    }
    let frames = world
        .get_resource::<AnimationRegistry>()
        .and_then(|registry| registry.get(CORE))
        .map_or(1, |clip| clip.frames.len().max(1));
    let frame = (spray.age * 1000.0 / CORE_FRAME_MS) as usize % frames;
    let scale = Vec2::new(1.0, flip * CORE_TALL);
    for (k, &tile) in look.core.iter().enumerate() {
        let centre = spray.start + spray.aim * (CORE_SIZE.x * (k as f32 + 0.5));
        place(world, tile, centre, CORE_SIZE, angle, scale);
        if let Some(state) = world.get_mut::<AnimationState>(tile) {
            state.frame_index = frame;
        }
    }
}

/// `spray_puffs`, after `spray_look`: each puff flies on to its stop,
/// grows and streaks along its line while young, and goes when its clip
/// has played out.
pub fn puffs(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let mut gone = Vec::new();
    for (entity, puff, transform, visibility) in
        world.query_mut::<(Entity, &mut SprayPuff, &mut Transform, &mut Visibility)>()
    {
        puff.age += dt;
        if puff.age >= puff.life {
            gone.push(entity);
            continue;
        }
        let t = puff.age / puff.life;
        let centre = puff.origin + puff.direction * (puff.speed * puff.age).min(puff.stop);
        let grown = puff.grow.0 + (puff.grow.1 - puff.grow.0) * t;
        let flip = if puff.direction.x < 0.0 { -1.0 } else { 1.0 };
        let streak = 1.0 + STREAK * (1.0 - t) * (1.0 - t);
        let scale = Vec2::new(grown * streak, grown * flip);
        transform.rotation = puff.direction.y.atan2(puff.direction.x);
        transform.scale = scale;
        transform.position = centre - Vec2::splat(PUFF_SIZE) * scale * 0.5;
        visibility.visible = true;
    }
    for entity in gone {
        world.despawn(entity);
    }
}
