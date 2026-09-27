//! Example-local hazards and prescribed platforms using public physics components.
use crate::level_layout::{DECK_DEPTH, LANTERN_ANCHORS, SLAB_COLLIDERS};
use crate::{
    level_layout::{HAZARDS, MOVING_PLATFORMS, MotionPlacement},
    state::{
        AnimatedProp, BALL_RADIUS, Ball, BlackHole, CurrentSprite, PLAYER_HALF, Player, TILE,
        TRANSIENT_EMITTER_CAP,
    },
};
use glam::{Vec2, Vec3};
use tungsten::core::{
    AnimationState, DeltaTime, Entity, InputState, KeyCode, Light, Particle,
    ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Transform, World,
};
use tungsten::physics::{Collider, Position, RigidBody, Velocity, wake};

#[derive(Clone, Copy)]
pub(crate) struct Health {
    pub(crate) hearts: u8,
    pub(crate) immunity: f32,
    pub(crate) control_lock: f32,
}
impl Default for Health {
    fn default() -> Self {
        Self {
            hearts: 3,
            immunity: 0.0,
            control_lock: 0.0,
        }
    }
}
#[derive(Default)]
pub(crate) struct SceneTime(pub(crate) f32);
#[derive(Clone, Copy)]
pub(crate) struct PreviousPosition(pub(crate) Vec2);
#[derive(Clone, Copy)]
pub(crate) struct Hazard {
    pub(crate) fire: bool,
}
impl Hazard {
    pub(crate) fn half(self) -> Vec2 {
        if self.fire {
            Vec2::new(18.0, 24.0)
        } else {
            Vec2::new(27.0, 14.0)
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct MovingPlatform {
    pub(crate) half_width: f32,
}
#[derive(Clone, Copy)]
struct Motion(MotionPlacement);
#[derive(Clone, Copy)]
pub(crate) struct Glow {
    pub(crate) offset: Vec2,
    pub(crate) radius: f32,
    pub(crate) color: [u8; 4],
}
struct LightAnchor(Entity, Vec2);
#[derive(Clone, Copy)]
pub(crate) struct Explosion {
    pub(crate) age: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct PlayerLantern {
    pub(crate) enabled: bool,
}

pub(crate) fn lantern_input(world: &mut World) {
    if world
        .get_resource::<InputState>()
        .is_some_and(|input| input.just_pressed(KeyCode::KeyL))
    {
        for e in world.query_entities::<PlayerLantern>() {
            let lantern = world.get_mut::<PlayerLantern>(e).unwrap();
            lantern.enabled = !lantern.enabled;
        }
    }
}

pub(crate) fn glow_center(world: &World, entity: Entity, offset: Vec2) -> Option<Vec2> {
    let transform = world.get::<Transform>(entity)?;
    if let Some(lantern) = world.get::<PlayerLantern>(entity) {
        if !lantern.enabled {
            return None;
        }
        let name = &world.get::<CurrentSprite>(entity)?.0;
        let anchor = LANTERN_ANCHORS
            .iter()
            .find(|(id, _)| id == name)
            .map_or([50.0, 46.0], |(_, a)| *a);
        let facing = if world
            .get::<crate::state::PlayerPresentation>(entity)
            .is_some_and(|p| p.facing_left)
        {
            -1.0
        } else {
            1.0
        };
        return Some(
            transform.position
                + Vec2::new(
                    (anchor[0] - 32.0) * facing * transform.scale.x,
                    PLAYER_HALF.y + (anchor[1] - 61.0) * transform.scale.y,
                ),
        );
    }
    Some(transform.position + offset)
}

/// Thin visible decks must not retain invisible full-tile collision below them.
pub(crate) fn spawn_platform_colliders(world: &mut World) {
    for &[x, y, width, height] in SLAB_COLLIDERS {
        let e = world.spawn();
        let half = Vec2::new(width, height) * 0.5;
        world.insert(e, Position(Vec2::new(x, y) + half));
        world.insert(e, Collider::aabb(half));
        world.insert(e, RigidBody::r#static());
    }
}

fn motion_position(m: MotionPlacement, time: f32) -> Vec2 {
    Vec2::from_array(m.position) * TILE
        + Vec2::from_array(m.travel)
            * TILE
            * (time * std::f32::consts::TAU / m.period + m.phase).sin()
}

pub(crate) fn spawn_obstacles(world: &mut World) {
    world.insert_resource(SceneTime::default());
    for e in world.query_entities::<Player>() {
        world.insert(e, PlayerLantern { enabled: true });
        add_glow(world, e, Vec2::ZERO, 125.0, [255, 190, 100, 255]);
    }
    for placement in HAZARDS {
        let e = world.spawn();
        let position = motion_position(placement.motion, 0.0);
        world.insert(
            e,
            Hazard {
                fire: placement.fire,
            },
        );
        world.insert(e, Motion(placement.motion));
        world.insert(e, Position(position));
        world.insert(e, Transform::from_position(position));
        world.insert(
            e,
            CurrentSprite(
                if placement.fire {
                    "ex10_fire_0"
                } else {
                    "ex10_spikes"
                }
                .into(),
            ),
        );
        if placement.fire {
            world.insert(e, AnimationState::new("ex10_fire_dance"));
            add_glow(world, e, Vec2::ZERO, 175.0, [255, 125, 45, 255]);
            if let Some(config) = world
                .get_resource::<ParticleConfigRegistry>()
                .and_then(|r| r.id_for_name("ex10_fire_trail"))
            {
                world.insert(e, ParticleEmitter::with_seed(config, e.id() as u64 + 700));
                world.insert(e, ParticleEmitterState::default());
            }
        }
    }
    for placement in MOVING_PLATFORMS {
        let e = world.spawn();
        let position = motion_position(placement.motion, 0.0);
        world.insert(
            e,
            MovingPlatform {
                half_width: placement.half_width * TILE,
            },
        );
        world.insert(e, Motion(placement.motion));
        world.insert(e, Position(position));
        world.insert(e, Transform::from_position(position));
        world.insert(
            e,
            Collider::aabb(Vec2::new(placement.half_width * TILE, DECK_DEPTH * 0.5)),
        );
        world.insert(e, RigidBody::r#static());
    }
    let props: Vec<_> = world
        .query::<AnimatedProp>()
        .filter_map(|(e, _)| {
            let name = &world.get::<CurrentSprite>(e)?.0;
            if name.starts_with("ex10_lantern") {
                Some((e, [255, 181, 98, 200]))
            } else if name == "ex10_crystal" {
                Some((e, [94, 210, 255, 180]))
            } else {
                None
            }
        })
        .collect();
    for (e, color) in props {
        add_glow(world, e, Vec2::new(32.0, 36.0), 150.0, color);
    }
}
fn add_glow(world: &mut World, parent: Entity, offset: Vec2, radius: f32, color: [u8; 4]) {
    world.insert(
        parent,
        Glow {
            offset,
            radius,
            color,
        },
    );
    let light = world.spawn();
    world.insert(light, LightAnchor(parent, offset));
    world.insert(light, Transform::default());
    let mut value = Light::point(
        Vec3::new(color[0] as f32, color[1] as f32, color[2] as f32) / 255.0,
        radius * 1.4,
    );
    value.intensity = 0.8;
    world.insert(light, value);
}

/// Capture motion before physics so fire catches fast crossings in either direction.
/// Platforms carry only supported bodies; accepted jumps leave them immediately.
pub(crate) fn move_obstacles(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);
    let time = if let Some(t) = world.get_resource_mut::<SceneTime>() {
        t.0 += dt;
        t.0
    } else {
        0.0
    };
    for e in world.query_entities::<Health>() {
        let h = world.get_mut::<Health>(e).unwrap();
        h.immunity = (h.immunity - dt).max(0.0);
        h.control_lock = (h.control_lock - dt).max(0.0);
    }
    let bodies: Vec<_> = world
        .query::<Position>()
        .filter(|(e, _)| {
            world.get::<Player>(*e).is_some()
                || world.get::<Ball>(*e).is_some()
                || world.get::<Hazard>(*e).is_some()
        })
        .map(|(e, p)| (e, p.0))
        .collect();
    for (e, p) in bodies {
        world.insert(e, PreviousPosition(p));
    }
    let moves: Vec<_> = world
        .query::<Motion>()
        .map(|(e, m)| (e, motion_position(m.0, time)))
        .collect();
    for (e, next) in moves {
        let old = world.get::<Position>(e).unwrap().0;
        if let Some(platform) = world.get::<MovingPlatform>(e).copied() {
            let riders: Vec<_> = world
                .query::<Position>()
                .filter_map(|(r, p)| {
                    let half = if world.get::<Player>(r).is_some() {
                        PLAYER_HALF
                    } else if world.get::<Ball>(r).is_some() {
                        Vec2::splat(BALL_RADIUS)
                    } else {
                        return None;
                    };
                    let velocity = world.get::<Velocity>(r)?.0;
                    ((p.0.y + half.y - (old.y - DECK_DEPTH * 0.5)).abs() < 2.0
                        && (p.0.x - old.x).abs() < platform.half_width + half.x - 2.0
                        && velocity.y >= -1.0)
                        .then_some(r)
                })
                .collect();
            for rider in riders {
                world.get_mut::<Position>(rider).unwrap().0 += next - old;
                wake(world, rider);
            }
        }
        world.get_mut::<Position>(e).unwrap().0 = next;
    }
    for e in world.query_entities::<Explosion>() {
        let explosion = world.get_mut::<Explosion>(e).unwrap();
        explosion.age += dt;
        if explosion.age >= 0.45 {
            world.despawn(e);
        }
    }
}

/// Slab test in relative motion space. Returns the first contact fraction.
fn swept_box(start: Vec2, end: Vec2, half: Vec2) -> Option<f32> {
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for axis in 0..2 {
        let d = end[axis] - start[axis];
        if d.abs() < 0.00001 {
            if start[axis].abs() > half[axis] {
                return None;
            }
        } else {
            let a = (-half[axis] - start[axis]) / d;
            let b = (half[axis] - start[axis]) / d;
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
            if enter > exit {
                return None;
            }
        }
    }
    Some(enter)
}
fn contact(world: &World, body: Entity, hazard: Entity, half: Vec2) -> Option<f32> {
    let end = world.get::<Position>(body)?.0;
    let hazard_end = world.get::<Position>(hazard)?.0;
    let start = world.get::<PreviousPosition>(body).map_or(end, |p| p.0);
    let hazard_start = world
        .get::<PreviousPosition>(hazard)
        .map_or(hazard_end, |p| p.0);
    swept_box(start - hazard_start, end - hazard_end, half)
}

pub(crate) fn hazard_contacts(world: &mut World) {
    let hazards: Vec<_> = world.query::<Hazard>().map(|(e, h)| (e, *h)).collect();
    // One removal/burst per ball, even when two flames overlap.
    let destroyed: Vec<_> = world
        .query::<Ball>()
        .filter_map(|(ball, _)| {
            let hit = hazards
                .iter()
                .filter(|(_, h)| h.fire)
                .filter_map(|(e, h)| contact(world, ball, *e, h.half() + Vec2::splat(BALL_RADIUS)))
                .min_by(f32::total_cmp)?;
            let end = world.get::<Position>(ball)?.0;
            let start = world.get::<PreviousPosition>(ball).map_or(end, |p| p.0);
            Some((ball, start.lerp(end, hit)))
        })
        .collect();
    for (ball, position) in destroyed {
        world.despawn(ball);
        crate::systems::spawn_transient_effect(world, "ex10_ball_explosion", position);
        if world.query::<Explosion>().count() < TRANSIENT_EMITTER_CAP {
            let e = world.spawn();
            world.insert(e, Explosion { age: 0.0 });
            world.insert(e, Transform::from_position(position));
        }
    }
    for player in world.query_entities::<Player>() {
        if world
            .get::<Health>(player)
            .is_some_and(|h| h.immunity > 0.0)
        {
            continue;
        }
        if let Some((hazard, _)) = hazards
            .iter()
            .find(|(e, h)| contact(world, player, *e, h.half() + PLAYER_HALF).is_some())
        {
            let origin = world.get::<Position>(*hazard).unwrap().0;
            damage_player(world, player, origin);
        }
    }
}
pub(crate) fn damage_player(world: &mut World, player: Entity, origin: Vec2) {
    if world.get::<Health>(player).is_none() {
        world.insert(player, Health::default());
    }
    let health = world.get_mut::<Health>(player).unwrap();
    if health.immunity > 0.0 {
        return;
    }
    health.hearts = health.hearts.saturating_sub(1);
    health.immunity = 1.1;
    health.control_lock = 0.18;
    if health.hearts == 0 {
        crate::systems::respawn_player(world, player);
    } else {
        let direction = if world.get::<Position>(player).unwrap().0.x < origin.x {
            -1.0
        } else {
            1.0
        };
        if let Some(v) = world.get_mut::<Velocity>(player) {
            v.0 = Vec2::new(direction * 380.0, -420.0);
        }
        if let Some(p) = world.get_mut::<Player>(player) {
            p.grounded = false;
            p.was_grounded = false;
        }
        wake(world, player);
    }
    crate::systems::damage_feedback(world, player);
}

pub(crate) fn scene_effects(world: &mut World) {
    let time = world.get_resource::<SceneTime>().map_or(0.0, |t| t.0);
    let lights: Vec<_> = world
        .query::<LightAnchor>()
        .map(|(e, a)| {
            (
                e,
                glow_center(world, a.0, a.1),
                world.get::<Hazard>(a.0).is_some(),
                world.get::<PlayerLantern>(a.0).is_some(),
            )
        })
        .collect();
    for (e, position, fire, lantern) in lights {
        if let Some(p) = position {
            world.get_mut::<Transform>(e).unwrap().position = p;
        }
        let strength = if fire {
            1.6
        } else if lantern {
            1.2
        } else {
            0.8
        };
        world.get_mut::<Light>(e).unwrap().intensity = if position.is_none() {
            0.0
        } else {
            strength + 0.13 * (time * 5.0 + e.id() as f32).sin()
        };
    }
    // Only steer owned vortex particles; the engine still integrates them once.
    let particles: Vec<_> = world
        .query::<Particle>()
        .filter_map(|(e, p)| {
            let owner = p.emitter?;
            world.get::<BlackHole>(owner)?;
            let center = world.get::<Position>(owner)?.0;
            let delta = world.get::<Transform>(e)?.position - center;
            let radial = delta.normalize_or_zero();
            Some((e, Vec2::new(-radial.y, radial.x) * 220.0 - radial * 70.0))
        })
        .collect();
    for (e, velocity) in particles {
        world.get_mut::<Particle>(e).unwrap().velocity = velocity;
    }
}
