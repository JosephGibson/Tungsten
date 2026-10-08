//! Example-local hazards and prescribed platforms using public physics components.
use crate::level_layout::{DECK_DEPTH, LANTERN_ANCHORS, SLAB_COLLIDERS};
use crate::state::{
    SMALL_BALL_BURSTS_PER_FRAME, SMALL_BALL_IMPACT_COOLDOWN, SMALL_BALL_IMPACT_SPEED,
    SMALL_BALL_SCALE, SmallBall,
};
use crate::{
    level_layout::{HAZARDS, MOVING_PLATFORMS, MotionPlacement},
    state::{
        AnimatedProp, BALL_RADIUS, Ball, BlackHole, CurrentSprite, PLAYER_HALF, Player, TILE,
        TRANSIENT_EMITTER_CAP,
    },
};
use glam::{Vec2, Vec3};
use tungsten::core::{
    AnimationState, Entity, InputState, KeyCode, Light, Particle, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, Time, Transform, With, World,
};
use tungsten::physics::{
    Collider, Position, PrevPosition, RigidBody, RigidBodyBundle, Velocity, wake,
};

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
pub(crate) struct PreviousVelocity(pub(crate) Vec2);

pub(crate) fn ball_radius(world: &World, entity: Entity) -> f32 {
    BALL_RADIUS
        * if world.get::<SmallBall>(entity).is_some() {
            SMALL_BALL_SCALE
        } else {
            1.0
        }
}

/// Contact normals reject glancing/separating contacts. Sample velocity before
/// resolution, which may have stopped or reversed the ball by the time we run.
pub(crate) fn small_ball_impacts(world: &mut World) {
    use tungsten::core::EventQueue;
    use tungsten::physics::CollisionEvent;
    let Some(events) = world.get_resource::<EventQueue<CollisionEvent>>() else {
        return;
    };
    let mut hits = Vec::with_capacity(SMALL_BALL_BURSTS_PER_FRAME);
    for event in events.iter_current() {
        let va = world
            .get::<PreviousVelocity>(event.a)
            .map_or(Vec2::ZERO, |v| v.0);
        let vb = event
            .b
            .and_then(|b| world.get::<PreviousVelocity>(b))
            .map_or(Vec2::ZERO, |v| v.0);
        if -(va - vb).dot(event.normal) <= SMALL_BALL_IMPACT_SPEED {
            continue;
        }
        for entity in [Some(event.a), event.b].into_iter().flatten() {
            if world
                .get::<SmallBall>(entity)
                .is_some_and(|b| b.impact_cooldown <= 0.0)
                && !hits.contains(&entity)
                && world.get::<Position>(entity).is_some()
            {
                hits.push(entity);
                if hits.len() == SMALL_BALL_BURSTS_PER_FRAME {
                    break;
                }
            }
        }
        if hits.len() == SMALL_BALL_BURSTS_PER_FRAME {
            break;
        }
    }
    for entity in hits {
        world.get_mut::<SmallBall>(entity).unwrap().impact_cooldown = SMALL_BALL_IMPACT_COOLDOWN;
        let position = world.get::<Position>(entity).unwrap().0;
        crate::systems::spawn_transient_effect(world, "ex10_small_ball_impact", position);
    }
}
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
/// Keeps a secondary emitter at a fixed offset from a moving parent.
pub(crate) struct EmitterAnchor {
    pub(crate) parent: Entity,
    pub(crate) offset: Vec2,
}
/// Molten drips leak from the underside of each fireball.
pub(crate) const FIREBALL_DRIP_OFFSET: Vec2 = Vec2::new(0.0, 12.0);
/// The expanding shock ring of a blast, `size` pixels across at its end.
#[derive(Clone, Copy)]
pub(crate) struct Explosion {
    pub(crate) age: f32,
    pub(crate) size: f32,
}
/// Final ring diameter of a burst ball.
pub(crate) const BALL_BURST_RING: f32 = 140.0;

#[derive(Clone, Copy)]
pub(crate) struct PlayerLantern {
    pub(crate) enabled: bool,
}

pub(crate) fn lantern_input(world: &mut World) {
    if world
        .get_resource::<InputState>()
        .is_some_and(|input| input.just_pressed(KeyCode::KeyL))
    {
        for e in world
            .query_filtered::<Entity, With<PlayerLantern>>()
            .collect::<Vec<_>>()
        {
            let lantern = world.get_mut::<PlayerLantern>(e).unwrap();
            lantern.enabled = !lantern.enabled;
        }
    }
}

pub(crate) fn glow_center(world: &World, entity: Entity, offset: Vec2) -> Option<Vec2> {
    let transform = world.get::<Transform>(entity)?;
    if let Some(lantern) = world.get::<PlayerLantern>(entity) {
        if !lantern.enabled || crate::death::player_dead(world) {
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
/// Each platform is one collider: at a join between two static boxes a body
/// that has sunk into the deck meets the next box's side as a wall (0.57).
pub(crate) fn spawn_platform_colliders(world: &mut World) {
    for [x, y, width, height] in slab_runs(SLAB_COLLIDERS) {
        let half = Vec2::new(width, height) * 0.5;
        world.spawn_with(RigidBodyBundle::r#static(
            Position(Vec2::new(x, y) + half),
            Collider::aabb(half),
        ));
    }
}

/// `slabs` (`[x, y, width, height]`) with every run that shares a row and a
/// height and touches end to end merged into one box, by row, then x.
pub(crate) fn slab_runs(slabs: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let mut sorted = slabs.to_vec();
    sorted.sort_by(|a, b| {
        a[1].total_cmp(&b[1])
            .then(a[3].total_cmp(&b[3]))
            .then(a[0].total_cmp(&b[0]))
    });
    let mut runs: Vec<[f32; 4]> = Vec::with_capacity(sorted.len());
    for [x, y, width, height] in sorted {
        match runs.last_mut() {
            Some(run) if run[1] == y && run[3] == height && run[0] + run[2] == x => {
                run[2] += width;
            }
            _ => runs.push([x, y, width, height]),
        }
    }
    runs
}

fn motion_position(m: MotionPlacement, time: f32) -> Vec2 {
    Vec2::from_array(m.position) * TILE
        + Vec2::from_array(m.travel)
            * TILE
            * (time * std::f32::consts::TAU / m.period + m.phase).sin()
}

/// Current velocity of a prescribed mover, in pixels per second.
pub(crate) fn motion_velocity(world: &World, entity: Entity) -> Option<Vec2> {
    let Motion(m) = *world.get::<Motion>(entity)?;
    let time = world.get_resource::<SceneTime>().map_or(0.0, |t| t.0);
    let rate = std::f32::consts::TAU / m.period;
    Some(Vec2::from_array(m.travel) * TILE * rate * (time * rate + m.phase).cos())
}

/// Fireballs face their horizontal travel; vertical-only movers watch the player.
pub(crate) fn fireball_faces_left(world: &World, hazard: Entity) -> bool {
    if let Some(Motion(m)) = world.get::<Motion>(hazard).copied()
        && m.travel[0] != 0.0
    {
        return motion_velocity(world, hazard).is_some_and(|v| v.x < 0.0);
    }
    let own = world.get::<Position>(hazard).map(|p| p.0.x);
    let player = world
        .query::<(Entity, &Player)>()
        .next()
        .and_then(|(e, _)| world.get::<Position>(e))
        .map(|p| p.0.x);
    matches!((own, player), (Some(own), Some(player)) if player < own)
}

pub(crate) fn spawn_obstacles(world: &mut World) {
    world.insert_resource(SceneTime::default());
    for e in world
        .query_filtered::<Entity, With<Player>>()
        .collect::<Vec<_>>()
    {
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
        world.insert(e, PrevPosition(position));
        world.insert(e, Transform::from_position(position));
        world.insert(
            e,
            CurrentSprite(
                if placement.fire {
                    "ex10_fireball_0"
                } else {
                    "ex10_spikes"
                }
                .into(),
            ),
        );
        if placement.fire {
            world.insert(e, AnimationState::new("ex10_fireball"));
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
        world.insert(e, PrevPosition(position));
        world.insert(e, Transform::from_position(position));
        world.insert(
            e,
            Collider::aabb(Vec2::new(placement.half_width * TILE, DECK_DEPTH * 0.5)),
        );
        world.insert(e, RigidBody::r#static());
    }
    let props: Vec<_> = world
        .query::<(Entity, &AnimatedProp)>()
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
    // Spawned last so every earlier entity keeps its id; ids seed emitters and
    // phase the glows.
    let Some(drips) = world
        .get_resource::<ParticleConfigRegistry>()
        .and_then(|r| r.id_for_name("ex10_fireball_drips"))
    else {
        return;
    };
    let fires: Vec<_> = world
        .query::<(Entity, &Hazard)>()
        .filter(|(_, h)| h.fire)
        .filter_map(|(e, _)| Some((e, world.get::<Position>(e)?.0)))
        .collect();
    for (parent, position) in fires {
        let e = world.spawn();
        world.insert(
            e,
            EmitterAnchor {
                parent,
                offset: FIREBALL_DRIP_OFFSET,
            },
        );
        world.insert(e, Transform::from_position(position + FIREBALL_DRIP_OFFSET));
        world.insert(
            e,
            ParticleEmitter::with_seed(drips, parent.id() as u64 + 900),
        );
        world.insert(e, ParticleEmitterState::default());
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
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let time = if let Some(t) = world.get_resource_mut::<SceneTime>() {
        t.0 += dt;
        t.0
    } else {
        0.0
    };
    for e in world
        .query_filtered::<Entity, With<SmallBall>>()
        .collect::<Vec<_>>()
    {
        let ball = world.get_mut::<SmallBall>(e).unwrap();
        ball.impact_cooldown = (ball.impact_cooldown - dt).max(0.0);
    }
    for e in world
        .query_filtered::<Entity, With<Velocity>>()
        .collect::<Vec<_>>()
    {
        let velocity = world.get::<Velocity>(e).unwrap().0;
        world.insert(e, PreviousVelocity(velocity));
    }
    for e in world
        .query_filtered::<Entity, With<Health>>()
        .collect::<Vec<_>>()
    {
        let h = world.get_mut::<Health>(e).unwrap();
        h.immunity = (h.immunity - dt).max(0.0);
        h.control_lock = (h.control_lock - dt).max(0.0);
    }
    let bodies: Vec<_> = world
        .query::<(Entity, &Position)>()
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
        .query::<(Entity, &Motion)>()
        .map(|(e, m)| (e, motion_position(m.0, time)))
        .collect();
    for (e, next) in moves {
        let old = world.get::<Position>(e).unwrap().0;
        if let Some(platform) = world.get::<MovingPlatform>(e).copied() {
            let riders: Vec<_> = world
                .query::<(Entity, &Position)>()
                .filter_map(|(r, p)| {
                    let half = if world.get::<Player>(r).is_some() {
                        PLAYER_HALF
                    } else if world.get::<Ball>(r).is_some() {
                        Vec2::splat(ball_radius(world, r))
                    } else if world.get::<crate::brick::IronBrick>(r).is_some() {
                        crate::brick::BRICK_HALF
                    } else if world.has::<crate::brick::IronScrap>(r) {
                        crate::brick::SCRAP_HALF
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
    for e in world
        .query_filtered::<Entity, With<Explosion>>()
        .collect::<Vec<_>>()
    {
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
    let hazards: Vec<_> = world
        .query::<(Entity, &Hazard)>()
        .map(|(e, h)| (e, *h))
        .collect();
    // One ignition or destruction per ball, even when two flames overlap.
    let touched: Vec<_> = world
        .query::<(Entity, &Ball)>()
        .filter_map(|(ball, _)| {
            let hit = hazards
                .iter()
                .filter(|(_, h)| h.fire)
                .filter_map(|(e, h)| {
                    contact(
                        world,
                        ball,
                        *e,
                        h.half() + Vec2::splat(ball_radius(world, ball)),
                    )
                })
                .min_by(f32::total_cmp)?;
            let end = world.get::<Position>(ball)?.0;
            let start = world.get::<PreviousPosition>(ball).map_or(end, |p| p.0);
            Some((ball, start.lerp(end, hit)))
        })
        .collect();
    for (ball, position) in touched {
        if world.get::<SmallBall>(ball).is_some() {
            crate::burning::ignite(world, ball);
            continue;
        }
        world.despawn(ball);
        crate::systems::spawn_transient_effect(world, "ex10_ball_explosion", position);
        if world.query::<(Entity, &Explosion)>().count() < TRANSIENT_EMITTER_CAP {
            let e = world.spawn();
            world.insert(
                e,
                Explosion {
                    age: 0.0,
                    size: BALL_BURST_RING,
                },
            );
            world.insert(e, Transform::from_position(position));
        }
    }
    for player in world
        .query_filtered::<Entity, With<Player>>()
        .collect::<Vec<_>>()
    {
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
            damage_player(world, player, origin, 1);
        }
    }
}

/// Seconds after a hit during which further damage is ignored; the player
/// blinks for as long (`extract`).
pub(crate) const HIT_IMMUNITY: f32 = 1.1;

/// The one damage path: takes `hearts` unless the player is immune or dead,
/// knocks the player away from `origin`, and on the last heart starts the
/// death screen. Every hit plays the hit feedback.
pub(crate) fn damage_player(world: &mut World, player: Entity, origin: Vec2, hearts: u8) {
    if crate::death::player_dead(world) {
        return;
    }
    if world.get::<Health>(player).is_none() {
        world.insert(player, Health::default());
    }
    let health = world.get_mut::<Health>(player).unwrap();
    if health.immunity > 0.0 {
        return;
    }
    health.hearts = health.hearts.saturating_sub(hearts);
    health.immunity = HIT_IMMUNITY;
    health.control_lock = 0.18;
    if health.hearts == 0 {
        crate::death::kill_player(world, player);
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

/// Puts each anchored emitter (hazard and missile drips, black-hole dust) at
/// its parent's drawn point plus its offset: after `physics_sync` draws the
/// parent and before the engine's emit pass reads the emitter (M41).
pub(crate) fn anchor_emitters(world: &mut World) {
    let anchored: Vec<_> = world
        .query::<(Entity, &EmitterAnchor)>()
        .filter_map(|(e, a)| Some((e, world.get::<Transform>(a.parent)?.position + a.offset)))
        .collect();
    for (e, position) in anchored {
        if let Some(transform) = world.get_mut::<Transform>(e) {
            transform.position = position;
        }
    }
}

pub(crate) fn scene_effects(world: &mut World) {
    let time = crate::extract::drawn_scene_time(world);
    let lights: Vec<_> = world
        .query::<(Entity, &LightAnchor)>()
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
    // New particles are visible here on the frame after the engine's deferred
    // emission. Their age is still zero; initialize once before the first tick.
    let rainbow = world
        .get_resource::<ParticleConfigRegistry>()
        .and_then(|r| r.id_for_name("ex10_small_ball_impact"));
    let newborn: Vec<_> = world
        .query::<(Entity, &Particle)>()
        .filter_map(|(e, p)| {
            let emitter = world.get::<ParticleEmitter>(p.emitter?)?;
            (p.age == 0.0 && Some(emitter.config) == rainbow).then_some((e, p.velocity))
        })
        .collect();
    for (e, velocity) in newborn {
        let hue = velocity.y.atan2(velocity.x) / std::f32::consts::TAU;
        let color = crate::systems::hsv_to_rgb(hue.rem_euclid(1.0), 0.85, 1.0);
        world.get_mut::<Particle>(e).unwrap().base_rgba = [color.x, color.y, color.z, 1.0];
    }
    // Annular births and accelerating tangential motion read as accretion.
    // Only owned vortex particles are steered; the engine integrates them once.
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let particles: Vec<_> = world
        .query::<(Entity, &Particle)>()
        .filter_map(|(e, p)| {
            // Black-hole dust comes from an emitter anchored to the hole.
            let emitter = p.emitter?;
            let owner = world
                .get::<EmitterAnchor>(emitter)
                .map_or(emitter, |a| a.parent);
            world.get::<BlackHole>(owner)?;
            let center = world.get::<Position>(owner)?.0;
            let position = if p.age == 0.0 {
                center + p.velocity.normalize_or_zero() * (110.0 + p.velocity.length() * 0.25)
            } else {
                world.get::<Transform>(e)?.position
            };
            let delta = position - center;
            let radial = delta.normalize_or_zero();
            let tangent_speed = (32_000.0 / delta.length().max(50.0)).clamp(180.0, 640.0);
            // Aim at the next point on the spiral. Euler tangential velocity
            // alone drifts outward near the core, especially at low frame rates.
            let velocity = if dt > 0.0 {
                let (sin, cos) = (tangent_speed / delta.length().max(14.0) * dt).sin_cos();
                let next_radial = Vec2::new(
                    radial.x * cos - radial.y * sin,
                    radial.x * sin + radial.y * cos,
                );
                (next_radial * (delta.length() - 105.0 * dt).max(0.0) - delta) / dt
            } else {
                p.velocity
            };
            Some((e, position, delta.length() < 14.0, velocity))
        })
        .collect();
    for (e, position, absorbed, velocity) in particles {
        world.get_mut::<Transform>(e).unwrap().position = position;
        let particle = world.get_mut::<Particle>(e).unwrap();
        particle.velocity = velocity;
        if absorbed {
            particle.age = particle.lifetime;
        }
    }
}
