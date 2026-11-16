//! The toys' systems and the work behind the functions the spells call.

use glam::{Vec2, Vec3};
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, AudioCommands, BodyKind, Bundle, Collider, Easing, Entity, EventCursor,
    EventQueue, Light, LightKind, Particle, ParticleConfigRegistry, ParticleEmitter,
    ParticleEmitterState, Position, PrevPosition, RigidBody, Shape, SoundRegistry, Sprite, Time,
    Transform, Tween, TweenChannel, Velocity, Visibility, With, World,
};
use tungsten::kit::{
    Blast, Children, CooldownElapsed, EffectInstance, Effects, Health, LinearMotion, Modifier,
    PlatformerController, Projectile, ProjectileExpired, Stats, Steering, Tags, TopDownMover,
    blast, carrier_of, knock_back, steered_direction, stimulate,
};
use tungsten::physics::{SpatialFilter, SpatialHit, cast_shape_where, wake};

use super::components::{
    Ball, BallHue, BlackHole, Element, FireMissile, Flare, HoleHold, IronBlock, IronScrap,
    OpenRing, OpeningOf, Oriented, PreviousVelocity, Pull, Ring, RingLook, ShockRing, SmallBall,
    Spray, SprayEmitter,
};
use super::draw;
use super::{
    BLAST_RADIUS, BROOM_MOST, BROOM_REACH, COLLAPSE_RING, FIRE_SPRAY_RANGE, FROZEN_FRICTION,
    HOLD_EASE, HOLD_RING, HOLD_SWIRL, HOLE_FORCE, HOLE_RADIUS, IRON_FALL_FROM, IRON_FRICTION,
    IRON_WEIGHT, LOOP_FADE, MISSILE_RADIUS, MISSILE_SIZE, OPEN_INWARD, OPEN_REACH, PUSH_RADIUS,
    PUSH_SPEED, RING_SECS, SCRAP_SCALE, SPRAY_HALF_ANGLE, SPRAY_RANGE, rings_full,
};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{
    self, BLAST_FIRE, COLD_SPRAY_DPS, FIRE_SPRAY_DPS, FROZEN, Kind, SPELL, SPRAY_PULSE, has_status,
};
use crate::features::dummy::components::Dummy;
use crate::features::elements::cascade;
use crate::features::elements::components::Flame;
use crate::features::elements::{self, FIRE_CENTRE, FIRE_HALF, STEAM_REACH, freeze_amount};
use crate::features::elements::{BALL_BURN, COAT, is_burning, is_frozen};
use crate::features::elements::{Impact, Raised};
use crate::features::enemies;
use crate::features::enemies::components::{Enemy, EnemyKind, Throws};
use crate::features::level::components::Hazard;
use crate::features::level::geyser::{self, Geyser};
use crate::features::level::systems::{CORE, CORE_MATERIAL, drawn_time};
use crate::features::player::components::Player;
use crate::features::player::systems::escaped;
use crate::features::props::{self, components::Crate};
use crate::features::run::Run;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, Saved, body_centre, despawn_tree, spawn_burst, surface_distance};

/// The light a missile leaves where it ends, and its fades.
const FLARE_COLOR: Vec3 = Vec3::new(1.0, 0.56, 0.22);
const FLARE_RADIUS: f32 = 192.0;
const FLARE_INTENSITY: f32 = 2.4;
const FLARE_SECS: f32 = 0.3;
const BLAST_FLARE_SECS: f32 = 0.5;
/// A spray's beam half width, its start from the caster's centre, and how
/// fast iron frosts and thaws under it.
const BEAM_HALF_WIDTH: f32 = 4.0;
pub(super) const SPRAY_MUZZLE: f32 = 24.0;
const FREEZE_TIME: f32 = elements::FREEZE_TIME;
/// The particle configs of a cold spray and of a fire spray: the stream,
/// the mist and the contact.
const COLD_EMITTERS: [&str; 3] = ["ex10_ice_beam", "ex10_ice_mist", "ex10_ice_contact"];
const FIRE_EMITTERS: [&str; 3] = ["ex10_flame_spray", "ex10_flame_mist", "ex10_ball_burn"];
/// The bursts on a target a spray damages, every second pulse (01 (h)),
/// and on every other surface it meets (e01sfx).
const FROST_HIT: &str = "ex10_frost_hit";
const FLAME_HIT: &str = "ex10_flame_hit";
pub const FROST_WALL_HIT: &str = "ex10_frost_wall_hit";
pub const FLAME_WALL_HIT: &str = "ex10_flame_wall_hit";
/// A spray's loop, by its element.
const COLD_LOOP: &str = "ex10_ice_spray_sfx";
const FIRE_LOOP: &str = "ex10_flame_spray_sfx";

/// `toys_tick`, in the tick phase after the kit's cooldowns: the hues' turn
/// and the shock rings' growth, each by the step, then each hole whose
/// life's cooldown elapsed this step ([`CooldownElapsed`], read once
/// through `cursor`) collapsing, in save-ID order.
pub fn tick(world: &mut World, cursor: &mut EventCursor<CooldownElapsed>) {
    let mut collapsed: Vec<(Option<SaveId>, Entity)> = world
        .get_resource::<EventQueue<CooldownElapsed>>()
        .map(|events| {
            cursor
                .read(events)
                .filter(|elapsed| world.has::<BlackHole>(elapsed.entity))
                .map(|elapsed| {
                    let id = world.get::<Saved>(elapsed.entity).map(Saved::id);
                    (id, elapsed.entity)
                })
                .collect()
        })
        .unwrap_or_default();
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let mut closed = Vec::new();
    for hue in world.query_mut::<&mut BallHue>() {
        hue.hue = (hue.hue + hue.speed * dt).rem_euclid(1.0);
    }
    for (entity, opening) in world.query_mut::<(Entity, &mut OpenRing)>() {
        opening.age += dt;
        if opening.age >= RING_SECS {
            closed.push(entity);
        }
    }
    for entity in closed {
        despawn_tree(world, entity);
    }
    collapsed.sort_by_key(|(id, _)| *id);
    for (_, hole) in collapsed {
        collapse(world, hole);
    }
}

/// Closes `hole`, by its time or the cap: its collapse sounds where it
/// stood and leaves a small pale ring there, which pushes nothing, and it
/// goes with its emitters.
pub fn collapse(world: &mut World, hole: Entity) {
    if let Some(at) = world.get::<Transform>(hole).map(|at| at.position) {
        ring(world, at, COLLAPSE_RING, Ring::Collapse);
        audio::play(world, Sound::HoleCollapsed, at);
    }
    despawn_tree(world, hole);
}

/// `hole_broom`, in the overlap phase: each open hole, in save-ID order,
/// takes the small balls whose centre lies within [`BROOM_REACH`] of its
/// own, nearest first and then by save ID, at most [`BROOM_MOST`] a step;
/// every eighth it takes puffs.
pub fn broom(world: &mut World) {
    let mut holes: Vec<(SaveId, Vec2)> = world
        .query::<(&BlackHole, &Transform, &Saved)>()
        .map(|(_, at, id)| (id.id(), at.position))
        .collect();
    if holes.is_empty() {
        return;
    }
    holes.sort_by_key(|(id, _)| *id);
    let reach = BROOM_REACH * BROOM_REACH;
    let near: Vec<(SaveId, Entity, Vec2)> = world
        .query::<(Entity, &SmallBall, &Position, &Collider, &Saved)>()
        .filter_map(|(entity, _, p, collider, id)| {
            let centre = p.0 + collider.offset;
            holes
                .iter()
                .any(|(_, hole)| hole.distance_squared(centre) <= reach)
                .then_some((id.id(), entity, centre))
        })
        .collect();
    let mut taken: Vec<(Entity, Vec2)> = Vec::new();
    let mut puffs = Vec::new();
    for (_, hole) in &holes {
        let mut here: Vec<(f32, SaveId, Entity, Vec2)> = near
            .iter()
            .filter(|(_, entity, _)| !taken.iter().any(|(ball, _)| ball == entity))
            .filter_map(|&(id, entity, centre)| {
                let distance = hole.distance_squared(centre);
                (distance <= reach).then_some((distance, id, entity, centre))
            })
            .collect();
        here.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (i, &(_, _, ball, centre)) in here.iter().take(BROOM_MOST).enumerate() {
            taken.push((ball, centre));
            if i % 8 == 0 {
                puffs.push(centre);
            }
        }
    }
    for (ball, _) in taken {
        despawn_tree(world, ball);
    }
    for at in puffs {
        spawn_burst(world, "ex10_poof", at);
    }
}

/// `toys_velocities`, in the move phase before the physics step: each
/// body's velocity as the step starts, which impacts read after it.
pub fn remember_velocities(world: &mut World) {
    for (velocity, previous) in world.query_mut::<(&Velocity, &mut PreviousVelocity)>() {
        previous.0 = velocity.0;
    }
}

/// `iron_weight`, in the move phase after `toys_velocities` and before the
/// physics step: a block falling faster than [`IRON_FALL_FROM`] falls
/// harder by [`IRON_WEIGHT`], since a `RigidBody` has no gravity scale;
/// never while a troll holds or throws it, whose throw aims by the world's
/// gravity alone.
pub fn weight(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let throws = world.get_resource::<Throws>();
    let falling: Vec<Entity> = world
        .query::<(Entity, &IronBlock, &Velocity)>()
        .filter(|(block, _, velocity)| {
            velocity.0.y > IRON_FALL_FROM
                && carrier_of(world, *block).is_none()
                && throws.is_none_or(|throws| throws.of(*block).is_none())
        })
        .map(|(block, ..)| block)
        .collect();
    for block in falling {
        if let Some(velocity) = world.get_mut::<Velocity>(block) {
            velocity.0.y += IRON_WEIGHT * dt;
        }
    }
}

/// `block` landing hard at `at`, its contact: its `iron_dusted` impact,
/// whose rule raises dust there, and its thud, whose rule shakes the
/// camera by its distance from the listener.
pub fn iron_landed(world: &mut World, block: Entity, at: Vec2) {
    let impact = Impact {
        raised: Raised::IronDust,
        body: block,
        at,
    };
    if let Some(queue) = world.get_resource_mut::<EventQueue<Impact>>() {
        queue.send(impact);
    }
    audio::play(world, Sound::IronLanded, at);
}

/// The open holes' centres.
#[must_use]
pub fn holes(world: &World) -> Vec<Vec2> {
    world
        .query::<(&BlackHole, &Transform)>()
        .map(|(_, at)| at.position)
        .collect()
}

/// The pull of every hole on `at`: toward each, falling linearly to 0 at
/// its reach.
#[must_use]
pub fn hole_pull(holes: &[Vec2], at: Vec2) -> Vec2 {
    let mut pull = Vec2::ZERO;
    for &hole in holes {
        let delta = hole - at;
        let distance_sq = delta.length_squared();
        if !(1.0e-4..HOLE_RADIUS * HOLE_RADIUS).contains(&distance_sq) {
            continue;
        }
        let distance = distance_sq.sqrt();
        pull += delta / distance * HOLE_FORCE * (1.0 - distance / HOLE_RADIUS);
    }
    pull
}

/// How much of a hole's pull `entity` takes: its prefab's [`Pull`] (iron a
/// fifth of a ball's, a bowling ball enough to lift it near the core,
/// scraps as balls, a crate a half and a keg two fifths, a frozen dummy as
/// iron and a thawed one none), the rest by their mass.
fn pull_response(world: &World, entity: Entity, inv_mass: f32) -> f32 {
    world.get::<Pull>(entity).map_or(inv_mass, |pull| {
        pull.taken(has_status(world, entity, FROZEN))
    })
}

/// The seconds of a hole's pull an enemy's mover takes each step.
pub const ENEMY_PULL_SECS: f32 = 0.25;
/// A troll within a hole's reach is slowed by this share of its speed, from
/// the source [`HOLE_SLOW_SOURCE`], since the pull barely moves it.
pub const HOLE_SLOW: f32 = 0.5;
pub const HOLE_SLOW_SOURCE: &str = "black_hole";

/// Slows each live troll within a hole's reach, and lets the others go.
fn slow_trolls(world: &mut World, holes: &[Vec2]) {
    let trolls: Vec<(Entity, bool)> = world
        .query::<(Entity, &Enemy, &Position, &Health)>()
        .filter(|(_, enemy, ..)| enemy.kind == EnemyKind::Troll)
        .map(|(entity, enemy, at, health)| {
            let centre = at.0 + enemies::body(world, enemy.kind).centre;
            let inside = holes
                .iter()
                .any(|&hole| hole.distance(centre) < HOLE_RADIUS);
            (entity, inside && !health.is_dead())
        })
        .collect();
    for (troll, inside) in trolls {
        if let Some(stats) = world.get_mut::<Stats>(troll) {
            stats.remove_modifiers(HOLE_SLOW_SOURCE);
            if inside {
                let slow = Modifier::Pct(-HOLE_SLOW);
                stats.add_modifier(Stats::MOVE_SPEED, HOLE_SLOW_SOURCE, slow);
            }
        }
    }
}

/// `toys_pull`, in the move phase: the holes pull every dynamic body in
/// reach by its response, and every live walker through its controller,
/// the flyers being held on their own movers (`toys_hold`); a troll, which
/// the pull barely moves, is slowed ([`HOLE_SLOW`]) while in reach.
pub fn pull(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let holes = holes(world);
    slow_trolls(world, &holes);
    if dt <= 0.0 || holes.is_empty() {
        return;
    }
    let pulled: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &RigidBody, &Position, &Collider)>()
        .filter(|(_, body, ..)| body.kind == BodyKind::Dynamic)
        .filter_map(|(entity, body, at, collider)| {
            // The reach first, then the body's response (P6).
            let pull = hole_pull(&holes, at.0 + collider.offset);
            if pull == Vec2::ZERO {
                return None;
            }
            let pull = pull * pull_response(world, entity, body.inv_mass);
            (pull != Vec2::ZERO).then_some((entity, pull * dt))
        })
        .collect();
    for (entity, kick) in pulled {
        if let Some(velocity) = world.get_mut::<Velocity>(entity) {
            velocity.0 += kick;
        }
        wake(world, entity);
    }
    // The enemies' kinematic bodies ride their movers, so the pull reaches
    // them as the velocity it would build over [`ENEMY_PULL_SECS`], by each
    // kind's hold: a walker's controller, a frozen flyer's borrowed one
    // too, takes it as a push; a flyer on its own mover is steered by
    // `toys_hold` instead, since a knockback renewed each step flung it
    // through the core.
    let enemies: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &Enemy, &Position, &Health)>()
        .filter(|(.., health)| !health.is_dead())
        .filter_map(|(entity, enemy, at, _)| {
            let centre = at.0 + enemies::body(world, enemy.kind).centre;
            let pull =
                hole_pull(&holes, centre) * enemies::info(world, enemy.kind).pull * ENEMY_PULL_SECS;
            (pull != Vec2::ZERO).then_some((entity, pull))
        })
        .collect();
    for (enemy, push) in enemies {
        if let Some(controller) = world.get_mut::<PlatformerController>(enemy) {
            controller.push(push);
        } else if !world.has::<TopDownMover>(enemy) {
            knock_back(world, enemy, push);
        }
    }
}

/// `toys_hold`, in the move phase before the top-down mover, after the
/// enemies' states have set their directions: a live flyer in a hole's
/// reach is steered on its own mover toward a ring [`HOLD_RING`] about the
/// nearest core at the hole's pull on it, capped at its own speed, the pull
/// easing within [`HOLD_EASE`] of the ring and circling there at
/// [`HOLD_SWIRL`] of its speed. That is blended with its own direction by
/// the pull over its speed, so near the core the hole holds it and at its
/// edge its own steering wins. Its own direction is the one its kit
/// steering would give its mover this step (`steered_direction`), whose
/// steering it switches off for the step, else its mover's, a bat's dive
/// or a rest; it is kept for `toys_release`, so the hole's correction never
/// stays.
pub fn hold(world: &mut World) {
    let holes = holes(world);
    if holes.is_empty() {
        return;
    }
    let dt = world.resource::<Time>().delta();
    let held: Vec<(Entity, Vec2, bool, Vec2)> = world
        .query::<(Entity, &Enemy, &Position, &Health, &TopDownMover)>()
        .filter(|(.., health, _)| !health.is_dead())
        .filter_map(|(entity, enemy, at, _, mover)| {
            let info = enemies::info(world, enemy.kind);
            let centre = at.0 + enemies::body(world, enemy.kind).centre;
            let core = holes
                .iter()
                .copied()
                .filter(|hole| hole.distance(centre) < HOLE_RADIUS)
                .min_by(|a, b| a.distance(centre).total_cmp(&b.distance(centre)))?;
            let speed = world
                .get::<Stats>(entity)
                .and_then(|stats| stats.get(Stats::MOVE_SPEED))
                .unwrap_or(100.0)
                .max(1.0);
            let to_core = core - centre;
            let distance = to_core.length().max(1.0);
            let pull = HOLE_FORCE * (1.0 - distance / HOLE_RADIUS) * info.pull * ENEMY_PULL_SECS;
            let off = distance - HOLD_RING;
            let inward = pull.min(speed) * (off / HOLD_EASE).clamp(-1.0, 1.0);
            let tangent = Vec2::new(-to_core.y, to_core.x) / distance;
            let near = 1.0 - (off / (2.0 * HOLD_EASE)).clamp(0.0, 1.0);
            let wanted = to_core / distance * inward + tangent * HOLD_SWIRL * speed * near;
            let hold = (pull / speed).min(1.0);
            let steered = steered_direction(world, entity, dt);
            let own = steered.unwrap_or_else(|| mover.direction());
            Some((
                entity,
                own,
                steered.is_some(),
                own * (1.0 - hold) + wanted / speed * hold,
            ))
        })
        .collect();
    let mut kept = Vec::with_capacity(held.len());
    for (flyer, own, steered, blend) in held {
        if steered && let Some(steering) = world.get_mut::<Steering>(flyer) {
            steering.set_on(false);
        }
        if let Some(mover) = world.get_mut::<TopDownMover>(flyer) {
            mover.set_direction(blend);
            kept.push((flyer, own, steered));
        }
    }
    world.resource_mut::<HoleHold>().0 = kept;
}

/// `toys_release`, in the move phase after the top-down mover: each flyer
/// `toys_hold` steered takes its own direction back once its mover has
/// moved, so a direction its state set once, a bat's dive, is whole again
/// when the hole closes or it leaves the reach, and a flyer its kit
/// steering flew has that steering on again.
pub fn release(world: &mut World) {
    let kept = std::mem::take(&mut world.resource_mut::<HoleHold>().0);
    for (flyer, own, steered) in kept {
        if let Some(mover) = world.get_mut::<TopDownMover>(flyer) {
            mover.set_direction(own);
        }
        if steered && let Some(steering) = world.get_mut::<Steering>(flyer) {
            steering.set_on(true);
        }
    }
}

/// `missile_bend`, in the move phase before the linear motion: the holes
/// bend each missile's flight.
pub fn bend(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let holes = holes(world);
    if dt <= 0.0 || holes.is_empty() {
        return;
    }
    let centre = Vec2::splat(MISSILE_SIZE * 0.5);
    for (_, at, motion) in world.query_mut::<(&FireMissile, &Position, &mut LinearMotion)>() {
        let velocity = motion.velocity() + hole_pull(&holes, at.0 + centre) * dt;
        motion.set_velocity(velocity);
    }
}

/// Where a missile's circle, its centre moving from `start` to `end`, first
/// meets a solid tile or a body other than the player's or a sensor (balls,
/// iron, decks, lifts): its centre then, by a core shape cast of the
/// step's own bodies, so its blast is where it met. A start overlapping
/// something passes it unless the missile moves into it.
fn sweep(world: &World, start: Vec2, end: Vec2) -> Option<Vec2> {
    let travel = end - start;
    let length = travel.length();
    if length <= 0.0 {
        return None;
    }
    let direction = travel / length;
    let circle = Shape::Circle {
        radius: MISSILE_RADIUS,
    };
    let spare_player = |hit: &SpatialHit| hit.entity.is_none_or(|body| !world.has::<Player>(body));
    cast_shape_where(
        world,
        circle,
        start,
        direction,
        length,
        &SpatialFilter::new(),
        spare_player,
    )
    .map(|hit| start + direction * hit.distance)
}

/// Shows `entity`, a prefab spawned hidden.
pub fn show(world: &mut World, entity: Entity) {
    if let Some(visibility) = world.get_mut::<Visibility>(entity) {
        visibility.visible = true;
    }
}

/// `missile_contacts`, in the overlap phase before the kit's projectile
/// step: a missile whose circle met a tile or a body in its step
/// ([`sweep`]) blasts where it met it and goes, its drips with it.
pub fn missile_contacts(world: &mut World) {
    let centre = Vec2::splat(MISSILE_SIZE * 0.5);
    let missiles: Vec<(Entity, Vec2, Vec2)> = world
        .query::<(Entity, &FireMissile, &Position, &PrevPosition)>()
        .map(|(entity, _, at, prev)| (entity, prev.0 + centre, at.0 + centre))
        .collect();
    for (missile, start, end) in missiles {
        if let Some(hit) = sweep(world, start, end) {
            flare(world, hit, true);
            despawn_tree(world, missile);
            explode(world, hit);
        }
    }
}

/// `missile_ends`, in the overlap phase after the kit's projectile step,
/// so its blast's hits take the step's resolve: a missile that step ended
/// at a wall blasts there; one that flew its range fizzles. `cursor` reads
/// each end once.
pub fn missile_ends(world: &mut World, cursor: &mut EventCursor<ProjectileExpired>) {
    let Some(events) = world.get_resource::<EventQueue<ProjectileExpired>>() else {
        return;
    };
    let centre = Vec2::splat(MISSILE_SIZE * 0.5);
    let ended: Vec<(Vec2, bool)> = cursor
        .read(events)
        .filter(|end| world.has::<FireMissile>(end.entity))
        .map(|end| {
            let spent = world
                .get::<Projectile>(end.entity)
                .is_some_and(|p| p.range().is_some_and(|range| p.travelled() >= range - 1.0));
            (end.position + centre, !spent)
        })
        .collect();
    for (at, blast) in ended {
        flare(world, at, blast);
        if blast {
            explode(world, at);
        }
    }
}

/// A missile's light left at `at`, fading; a blast's flares, as a keg's
/// detonation does.
pub(crate) fn flare(world: &mut World, at: Vec2, blast: bool) {
    let mut light = Light::point(FLARE_COLOR, FLARE_RADIUS * if blast { 2.0 } else { 1.0 });
    light.intensity = FLARE_INTENSITY;
    let secs = if blast { BLAST_FLARE_SECS } else { FLARE_SECS };
    let phase = crate::shared::next_id(world) as f32;
    world.spawn_with((
        Transform::from_position(at),
        light,
        Flare {
            age: 0.0,
            secs,
            blast,
            phase,
        },
        SceneEntity { state_id: GAMEPLAY },
    ));
}

/// `toys_flares`, in the present phase: each flare fades by the frame and
/// goes; a blast's starts at four times the light and drops.
pub fn flares(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let mut done = Vec::new();
    for (entity, flare, light) in world.query_mut::<(Entity, &mut Flare, &mut Light)>() {
        flare.age += dt;
        if flare.age >= flare.secs {
            done.push(entity);
            continue;
        }
        let left = 1.0 - flare.age / flare.secs;
        let boost = if flare.blast { 4.0 * left } else { 1.0 };
        light.intensity = FLARE_INTENSITY * left * boost;
        if let LightKind::Point { radius } = &mut light.kind {
            *radius = FLARE_RADIUS * if flare.blast { 2.0 } else { 1.0 };
        }
    }
    for entity in done {
        world.despawn(entity);
    }
}

/// A fireball's blast at `at`: its heat, the kit's blast of
/// [`BLAST_FIRE`] fire on each live target whose body lies within
/// [`BLAST_RADIUS`], half at its edge, a spell's hit from the player who
/// cast it, who is spared, queued for the step's resolve; the small balls
/// in its heat lit by their centres, and the crates in it lit and its kegs
/// set off by their boxes, credited to the player; its boom's rule shakes
/// the camera.
pub fn explode(world: &mut World, at: Vec2) {
    let player = world.get_resource::<Run>().map(|run| run.player);
    let heat = Blast::new(BLAST_RADIUS)
        .with_part(Kind::Fire.tag(), BLAST_FIRE)
        .with_far(0.5)
        .with_context([SPELL])
        .sparing_source()
        .at_centre()
        .with_stimulus(false)
        .with_prepare(enemies::armour::PREPARE);
    blast(world, at, &heat, player, None);
    for burst in [
        "ex10_fireball_blast",
        "ex10_blast_embers",
        "ex10_blast_dust",
        "ex10_blast_smoke",
    ] {
        spawn_burst(world, burst, at);
    }
    ring(world, at, 2.0 * PUSH_RADIUS, Ring::Fireball);
    elements::thermal_shatter(world, at, BLAST_RADIUS);
    radial_push(world, at, PUSH_RADIUS, PUSH_SPEED);
    let mut lit: Vec<(SaveId, Entity)> = world
        .query::<(Entity, &SmallBall, &Position, &Collider, &Saved)>()
        .filter(|(_, _, p, c, _)| (p.0 + c.offset).distance_squared(at) <= BLAST_RADIUS.powi(2))
        .map(|(entity, .., id)| (id.id(), entity))
        .collect();
    lit.sort_by_key(|(id, _)| *id);
    for (_, ball) in lit {
        elements::ignite(world, ball);
    }
    heat_props(world, at);
    audio::play(world, Sound::Blasted, at);
}

/// A fireball's heat on the props whose bodies lie within [`BLAST_RADIUS`]
/// of `at`, in save-ID order: a crate catches and a keg or a bomb goes off,
/// credited to the player; a frozen one's thermal shock is the blast's
/// `thermal_shatter`.
fn heat_props(world: &mut World, at: Vec2) {
    let mut near: Vec<(Option<SaveId>, Entity)> = props::all(world)
        .into_iter()
        .filter(|&prop| {
            let (Some(centre), Some(collider)) =
                (body_centre(world, prop), world.get::<Collider>(prop))
            else {
                return false;
            };
            surface_distance(at, centre, collider.shape) <= BLAST_RADIUS
        })
        .map(|prop| (crate::shared::save_id(world, prop), prop))
        .collect();
    near.sort_by_key(|(id, _)| *id);
    let player = cascade::player_credit(world);
    for (_, prop) in near {
        if is_frozen(world, prop) {
            continue;
        }
        if world.has::<Crate>(prop) {
            props::light(world, prop, player);
        } else {
            props::set_off(world, prop, player);
        }
    }
}

/// A shock ring at `at`, growing to `size` from [`RING_FROM`] across over
/// [`RING_SECS`], unless [`RING_CAP`] live: a tweened sprite fading from
/// [`RING_ALPHA`], ended by its tween, linear but a fireball's, which eases
/// out with a hot front and a flash; a hole's opening a ring drawn by hand
/// from its age (`toys::draw`).
pub fn ring(world: &mut World, at: Vec2, size: f32, kind: Ring) {
    if rings_full(world) {
        return;
    }
    let ring = ShockRing { size, kind };
    if kind == Ring::Open {
        world.spawn_with((
            ring,
            OpenRing { age: 0.0 },
            Transform::from_position(at),
            SceneEntity { state_id: GAMEPLAY },
        ));
        return;
    }
    let fireball = kind == Ring::Fireball;
    let grow = if fireball {
        Easing::CubicOut
    } else {
        Easing::Linear
    };
    let shock = RingCurve {
        art: draw::SHOCK_RING,
        across: (RING_FROM, size),
        grow,
        color: if kind == Ring::Collapse {
            [244, 166, 250, RING_ALPHA]
        } else {
            [255, 210, 140, RING_ALPHA]
        },
        fade: Easing::Linear,
        secs: RING_SECS,
        material: None,
    };
    let look = ring_look(world, at, &shock);
    let main = world.spawn_with(look.with((ring,)));
    if !fireball {
        return;
    }
    // Its front, at a share of its size, fading by the square of what is
    // left; its flash, growing and fading as one over its first seconds.
    let front = RingCurve {
        across: (RING_FROM * FRONT, size * FRONT),
        color: [255, 95, 28, 240],
        fade: Easing::QuadOut,
        ..shock
    };
    let flash = RingCurve {
        art: CORE,
        across: (100.0, 280.0),
        grow: Easing::Linear,
        color: [255, 245, 200, 255],
        fade: Easing::Linear,
        secs: FLASH_SECS,
        material: Some(CORE_MATERIAL),
    };
    let front = ring_look(world, at, &front);
    let front = world.spawn_with(front);
    let flash = ring_look(world, at, &flash);
    let flash = world.spawn_with(flash);
    if let Some(look) = world.get_mut::<RingLook>(main) {
        *look = RingLook {
            front: Some(front),
            flash: Some(flash),
        };
    }
}

/// A shock ring's size at its birth, its alpha then, its fireball's front's
/// share of its size and its flash's seconds.
pub const RING_FROM: f32 = 32.0;
pub const RING_ALPHA: u8 = 220;
pub const FRONT: f32 = 0.72;
pub const FLASH_SECS: f32 = 0.18;

/// A ring look's curve: its art, its pixels across at its start and end in
/// `grow`, its tint and alpha, its alpha falling to none in `fade`, its
/// seconds and its material.
#[derive(Clone, Copy)]
struct RingCurve {
    art: &'static str,
    across: (f32, f32),
    grow: Easing,
    color: [u8; 4],
    fade: Easing,
    secs: f32,
    material: Option<&'static str>,
}

/// A ring look of `curve` centred on `at`, to spawn: a sprite with no
/// `Visibility` ([`RingLook`]), its `Transform` its top-left, its scale and
/// top-left tweened in its curve's `grow` so its centre holds, its alpha
/// in its `fade`, and its tween ending it.
fn ring_look(
    world: &mut World,
    at: Vec2,
    curve: &RingCurve,
) -> (Transform, Sprite, Tween, RingLook, SceneEntity) {
    let art = crate::shared::sprite_size(world, curve.art);
    let material = curve
        .material
        .and_then(|name| crate::features::level::draw::material(world, name));
    let id = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(curve.art);
    let mut sprite = Sprite::new(id);
    sprite.color = curve.color;
    sprite.material_id = material;
    let (from, to) = curve.across;
    let corner = |across: f32| at - Vec2::splat(across * 0.5);
    let scale = |across: f32| Vec2::splat(across) / art;
    let tween = Tween::new(curve.secs, curve.grow)
        .with_channel(TweenChannel::PositionX {
            from: corner(from).x,
            to: corner(to).x,
        })
        .with_channel(TweenChannel::PositionY {
            from: corner(from).y,
            to: corner(to).y,
        })
        .with_channel(TweenChannel::ScaleX {
            from: scale(from).x,
            to: scale(to).x,
        })
        .with_channel(TweenChannel::ScaleY {
            from: scale(from).y,
            to: scale(to).y,
        })
        .with_channel_eased(
            TweenChannel::ColorA {
                from: curve.color[3],
                to: 0,
            },
            curve.fade,
        )
        .with_despawn_at_end();
    let transform = Transform {
        position: corner(from),
        rotation: 0.0,
        scale: scale(from),
    };
    (
        transform,
        sprite,
        tween,
        RingLook::default(),
        SceneEntity { state_id: GAMEPLAY },
    )
}

/// Pushes what stands within `radius` of `at` straight away from it, up at
/// the centre, by `speed` there falling linearly to nothing at the reach,
/// measured from each body's centre: each dynamic body over its mass, and
/// each live enemy through the kit's `knock_back` by its kind's pull, the
/// hole's own response, so both gravity spells move each kind alike, the
/// kit taking the knockback's resist, so a kind that resists whole (the
/// troll) stands firm. The holes' own pull stays its own (`toys_pull`): a
/// knockback each step would lock a walker's input.
pub fn radial_push(world: &mut World, at: Vec2, radius: f32, speed: f32) {
    let away = |centre: Vec2| -> Option<Vec2> {
        let offset = centre - at;
        let distance = offset.length();
        (distance < radius)
            .then(|| offset.try_normalize().unwrap_or(-Vec2::Y) * speed * (1.0 - distance / radius))
    };
    let pushed: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &RigidBody, &Position, &Collider)>()
        .filter(|(_, body, ..)| body.kind == BodyKind::Dynamic)
        .filter_map(|(entity, body, p, collider)| {
            Some((entity, away(p.0 + collider.offset)? * body.inv_mass))
        })
        .collect();
    for (entity, kick) in pushed {
        if let Some(velocity) = world.get_mut::<Velocity>(entity) {
            velocity.0 += kick;
        }
        wake(world, entity);
    }
    let mut thrown: Vec<(Option<SaveId>, Entity, Vec2)> = world
        .query::<(Entity, &Enemy, &Health)>()
        .filter(|(_, _, health)| !health.is_dead())
        .filter_map(|(entity, enemy, _)| {
            let push = away(body_centre(world, entity)?)? * enemies::info(world, enemy.kind).pull;
            Some((crate::shared::save_id(world, entity), entity, push))
        })
        .collect();
    thrown.sort_by_key(|(id, ..)| *id);
    for (_, enemy, push) in thrown {
        knock_back(world, enemy, push);
    }
}

/// `toys_bounds`, in the overlap phase: a ball, block, scrap, crate, keg,
/// dummy or enemy that has left the level's box goes, as the player's
/// escape ends a run: a placed enemy is gone from the next save, and what a
/// troll held drops at the next `troll_holds`.
pub fn bounds(world: &mut World) {
    let escaped: Vec<Entity> = world
        .query::<(Entity, &Position, Option<&Collider>)>()
        .filter(|(_, at, collider)| escaped(at.0, *collider))
        .filter(|(entity, ..)| {
            world.has::<Ball>(*entity)
                || world.has::<IronBlock>(*entity)
                || world.has::<IronScrap>(*entity)
                || props::is_prop(world, *entity)
                || world.has::<Dummy>(*entity)
                || world.has::<Enemy>(*entity)
        })
        .map(|(entity, ..)| entity)
        .collect();
    for entity in escaped {
        despawn_tree(world, entity);
    }
}

/// `iron_friction`, in the overlap phase after the physics step: the solver
/// has no friction, so iron, a prop (a crate, a keg or a stone as wood, a
/// bomb or a rock rolling) or a dummy resting on something loses horizontal
/// speed, a frozen one less, a scrap a sixteenth.
pub fn friction(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let Some(events) = world.get_resource::<EventQueue<tungsten::core::CollisionEvent>>() else {
        return;
    };
    // The few bodies it acts on, marked by their entities' slots, so a
    // contact between balls costs a bit test, not three component checks
    // (P6).
    let mut slots: Vec<u64> = Vec::new();
    let marked = world
        .query::<(Entity, &IronBlock)>()
        .map(|(entity, _)| entity)
        .chain(
            world
                .query::<(Entity, &IronScrap)>()
                .map(|(entity, _)| entity),
        )
        .chain(world.query::<(Entity, &Dummy)>().map(|(entity, _)| entity))
        .chain(props::all(world));
    for entity in marked {
        let slot = entity.id() as usize;
        if slots.len() <= slot / 64 {
            slots.resize(slot / 64 + 1, 0);
        }
        slots[slot / 64] |= 1 << (slot % 64);
    }
    let marked = |entity: Entity| {
        let slot = entity.id() as usize;
        slots
            .get(slot / 64)
            .is_some_and(|word| word >> (slot % 64) & 1 == 1)
    };
    // `normal` is the way `a` was pushed: up for a body resting on `b`.
    let supported: std::collections::HashSet<Entity> = events
        .iter_current()
        .filter_map(|event| {
            if event.normal.y < -0.5 {
                Some(event.a)
            } else if event.normal.y > 0.5 {
                event.b
            } else {
                None
            }
        })
        .filter(|&e| {
            marked(e)
                && (world.has::<IronBlock>(e)
                    || world.has::<IronScrap>(e)
                    || world.has::<Dummy>(e)
                    || props::is_prop(world, e))
        })
        .collect();
    for iron in supported {
        let base = if has_status(world, iron, FROZEN) {
            FROZEN_FRICTION
        } else if let Some(kind) = props::kind(world, iron) {
            if props::rolls(kind) {
                props::ROLL_FRICTION
            } else {
                props::WOOD_FRICTION
            }
        } else {
            IRON_FRICTION
        };
        let friction = if world.has::<IronScrap>(iron) {
            base / 16.0
        } else {
            base
        };
        if let Some(velocity) = world.get_mut::<Velocity>(iron) {
            let speed = (velocity.0.x.abs() - friction * dt).max(0.0);
            velocity.0.x = speed.copysign(velocity.0.x);
        }
    }
}

/// The distance along `aim` from `start` at which a ray of half width
/// [`BEAM_HALF_WIDTH`] meets a body of `shape` about `centre`.
fn ray_distance(start: Vec2, aim: Vec2, centre: Vec2, shape: Shape, range: f32) -> Option<f32> {
    let offset = start - centre;
    match shape {
        Shape::Circle { radius } => {
            let radius = radius + BEAM_HALF_WIDTH;
            let c = offset.length_squared() - radius * radius;
            if c <= 0.0 {
                return Some(0.0);
            }
            let b = offset.dot(aim);
            let discriminant = b * b - c;
            if discriminant < 0.0 {
                return None;
            }
            let t = -b - discriminant.sqrt();
            (t >= 0.0).then_some(t)
        }
        other => {
            let half = other.half_extents() + Vec2::splat(BEAM_HALF_WIDTH);
            let (mut near, mut far) = (0.0_f32, range);
            for axis in 0..2 {
                if aim[axis].abs() < 1e-6 {
                    if offset[axis].abs() > half[axis] {
                        return None;
                    }
                } else {
                    let a = (-half[axis] - offset[axis]) / aim[axis];
                    let b = (half[axis] - offset[axis]) / aim[axis];
                    near = near.max(a.min(b));
                    far = far.min(a.max(b));
                }
            }
            (near <= far).then_some(near)
        }
    }
}

/// Casts a spray's beam, a circle of [`BEAM_HALF_WIDTH`], from `start`
/// along `aim` for `range`: where it stops on its centreline, and the body
/// it stops on. A core shape cast of the last step's bodies (all but the
/// caster's, the player's and sensors) and solid tiles: a body met stops it
/// at the circle's centre where they meet; a tile met at its contact's
/// place along the centreline, when that lies within the reach, else it
/// runs its reach; nothing met, its reach. Ties go by the index's order.
#[must_use]
pub fn beam(
    world: &World,
    caster: Entity,
    start: Vec2,
    aim: Vec2,
    range: f32,
) -> (Vec2, Option<Entity>) {
    let circle = Shape::Circle {
        radius: BEAM_HALF_WIDTH,
    };
    let keep = |hit: &SpatialHit| {
        hit.entity
            .is_none_or(|body| body != caster && !world.has::<Player>(body))
    };
    let reached = (start + aim * range, None);
    let Some(hit) = cast_shape_where(
        world,
        circle,
        start,
        aim,
        range,
        &SpatialFilter::new(),
        keep,
    ) else {
        return reached;
    };
    match hit.entity {
        Some(body) => (start + aim * hit.distance, Some(body)),
        None => {
            let along = (hit.point - start).dot(aim).max(0.0);
            if along <= range {
                (start + aim * along, None)
            } else {
                reached
            }
        }
    }
}

/// A spray's reach by its element.
pub(super) fn reach(element: Element) -> f32 {
    match element {
        Element::Cold => SPRAY_RANGE,
        Element::Fire => FIRE_SPRAY_RANGE,
    }
}

/// The seven rays of a spray's cone along `aim`.
fn rays(aim: Vec2) -> impl Iterator<Item = Vec2> {
    (-3..=3).map(move |ray| Vec2::from_angle(ray as f32 / 3.0 * SPRAY_HALF_ANGLE).rotate(aim))
}

/// Holds `caster`'s spray of `element` along `aim`, started when none
/// runs: kept for this step, at this aim. Its caster's centre and the aim,
/// unit length, when the caster has a body.
pub fn hold_spray(
    world: &mut World,
    caster: Entity,
    aim: Vec2,
    element: Element,
) -> Option<(Vec2, Vec2)> {
    let centre = body_centre(world, caster)?;
    let aim = aim.try_normalize().unwrap_or(Vec2::X);
    let held = world
        .query::<(Entity, &Spray)>()
        .find(|(_, spray)| spray.caster == caster && spray.element == element)
        .map(|(entity, _)| entity);
    let entity = held.unwrap_or_else(|| start_spray(world, caster, centre, aim, element));
    if let Some(spray) = world.get_mut::<Spray>(entity) {
        spray.kept = true;
        spray.aim = aim;
    }
    Some((centre, aim))
}

/// The fire hazards a ray from `start` along `aim` crosses before `length`,
/// burning ones only, by their touch boxes.
fn fires_crossed(world: &World, start: Vec2, aim: Vec2, length: f32) -> Vec<Entity> {
    world
        .query::<(Entity, &Hazard, &Position)>()
        .filter(|(entity, hazard, _)| hazard.fire && !elements::is_doused(world, *entity))
        .filter(|(_, _, at)| {
            let shape = Shape::Aabb {
                half_extents: FIRE_HALF,
            };
            ray_distance(start, aim, at.0 + FIRE_CENTRE, shape, length).is_some_and(|t| t <= length)
        })
        .map(|(entity, ..)| entity)
        .collect()
}

/// The geysers a ray from `start` along `aim` crosses before `length`, by
/// their vent boxes, which stand over the floor.
fn geysers_crossed(world: &World, start: Vec2, aim: Vec2, length: f32) -> Vec<Entity> {
    world
        .query::<(Entity, &Geyser, &Transform)>()
        .filter(|(_, _, at)| {
            let shape = Shape::Aabb {
                half_extents: geyser::VENT_HALF,
            };
            let centre = at.position + geyser::VENT_CENTRE;
            ray_distance(start, aim, centre, shape, length).is_some_and(|t| t <= length)
        })
        .map(|(entity, ..)| entity)
        .collect()
}

/// Holds `caster`'s spray of `element` for this step. In a cold cone, iron,
/// balls, crates and kegs chill, a burning small ball bursts into steam, a
/// burning crate or a lit keg steams and goes out, a fire hazard goes out
/// for a while and a geyser's vent chills toward its choke; in a fire cone small balls, crates and kegs light,
/// credited to the caster, and a frozen body explodes. Every pulse of
/// [`SPRAY_PULSE`] the live targets its rays meet take the element's damage
/// ([`COLD_SPRAY_DPS`] or [`FIRE_SPRAY_DPS`] a second) from the caster,
/// and every second pulse, from the first, each bursts at
/// its contact in the element's hit ([`FROST_HIT`], [`FLAME_HIT`]), every
/// other body met (props, balls, iron, frozen bodies, a lava basin) and the
/// nearest ray stop on a tile short of the reach in its surface hit
/// ([`FROST_WALL_HIT`], [`FLAME_WALL_HIT`]): cosmetic bursts the cap may
/// drop.
pub fn spray(world: &mut World, caster: Entity, aim: Vec2, element: Element) {
    let Some((centre, aim)) = hold_spray(world, caster, aim, element) else {
        return;
    };
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    // Each surface counts once, however many rays meet it.
    let mut met: Vec<(Entity, Vec2)> = Vec::new();
    let mut fires: Vec<Entity> = Vec::new();
    let mut basins: Vec<Entity> = Vec::new();
    let mut geysers: Vec<Entity> = Vec::new();
    // The nearest ray stop on a tile short of the reach.
    let mut wall: Option<Vec2> = None;
    for direction in rays(aim) {
        let (at, hit) = beam(world, caster, centre, direction, reach(element));
        let short = centre.distance(at) < reach(element) - 0.01;
        if hit.is_none()
            && short
            && wall.is_none_or(|w| centre.distance_squared(at) < centre.distance_squared(w))
        {
            wall = Some(at);
        }
        if element == Element::Cold {
            for fire in fires_crossed(world, centre, direction, centre.distance(at)) {
                if !fires.contains(&fire) {
                    fires.push(fire);
                }
            }
            for geyser in geysers_crossed(world, centre, direction, centre.distance(at)) {
                if !geysers.contains(&geyser) {
                    geysers.push(geyser);
                }
            }
            // A cold ray that stops on lava's surface chills its basin.
            if let Some(hit) = hit
                && world.get::<Hazard>(hit).is_some_and(|hazard| hazard.lava)
                && !basins.contains(&hit)
            {
                basins.push(hit);
            }
        }
        if let Some(hit) = hit
            && !met.iter().any(|&(e, _)| e == hit)
        {
            met.push((hit, at));
        }
    }
    for fire in fires {
        elements::douse_hazard(world, fire);
    }
    for basin in basins {
        crate::features::level::lava::chill(world, basin, dt);
    }
    for geyser in geysers {
        geyser::chill(world, geyser);
    }
    let (pulses, bursts) = world
        .query_mut::<&mut Spray>()
        .find(|spray| spray.caster == caster && spray.element == element)
        .map_or((0, 0), |spray| {
            spray.pulse += dt;
            let (mut pulses, mut bursts) = (0, 0);
            while spray.pulse >= SPRAY_PULSE - 1e-4 {
                spray.pulse -= SPRAY_PULSE;
                bursts += u32::from(spray.pulses % 2 == 0);
                spray.pulses = spray.pulses.wrapping_add(1);
                pulses += 1;
            }
            (pulses, bursts)
        });
    let (kind, hit, surface, dps) = match element {
        Element::Cold => (Kind::Cold, FROST_HIT, FROST_WALL_HIT, COLD_SPRAY_DPS),
        Element::Fire => (Kind::Fire, FLAME_HIT, FLAME_WALL_HIT, FIRE_SPRAY_DPS),
    };
    for _ in 0..bursts {
        if let Some(at) = wall {
            spawn_burst(world, surface, at);
        }
    }
    for &(target, contact) in &met {
        let live = world
            .get::<tungsten::kit::Health>(target)
            .is_some_and(|health| !health.is_dead());
        if target == caster {
            continue;
        }
        if !live {
            for _ in 0..bursts {
                spawn_burst(world, surface, contact);
            }
            continue;
        }
        for _ in 0..pulses {
            combat::Hit::new(target, [(kind, dps * SPRAY_PULSE)])
                .from(caster)
                .at(contact)
                .spell()
                .queue(world);
        }
        for _ in 0..bursts {
            spawn_burst(world, hit, contact);
        }
    }
    match element {
        Element::Cold => {
            for (hit, contact) in met {
                if is_burning(world, hit) && world.has::<SmallBall>(hit) {
                    // Cold on a burning ball: its steam rule's burst.
                    stimulate(world, hit, elements::COLD, dt / FREEZE_TIME, Some(caster));
                } else if props::burning(world, hit) {
                    // A prop's fire is its fuel's, no status: its steam at once.
                    props::put_out(world, hit);
                    cascade::steam_at(world, hit, STEAM_REACH, Some(caster));
                } else if world.has::<IronBlock>(hit)
                    || world.has::<IronScrap>(hit)
                    || world.has::<Ball>(hit)
                    || props::is_prop(world, hit)
                {
                    elements::chill(world, hit, dt / FREEZE_TIME, contact);
                }
            }
        }
        Element::Fire => {
            let lighter = crate::shared::save_id(world, caster).map(|id| id.0);
            for (hit, contact) in met {
                if is_frozen(world, hit) {
                    cascade::heat(world, hit, contact, Some(caster));
                } else if world.has::<SmallBall>(hit) {
                    elements::ignite(world, hit);
                } else if props::is_prop(world, hit) {
                    props::light(world, hit, lighter);
                }
            }
        }
    }
}

/// Starts a spray: its particles, its look (`spray_look`), its start sound
/// and its loop.
fn start_spray(
    world: &mut World,
    caster: Entity,
    centre: Vec2,
    aim: Vec2,
    element: Element,
) -> Entity {
    let (end, _) = beam(world, caster, centre, aim, reach(element));
    let start = centre + aim * SPRAY_MUZZLE.min(centre.distance(end));
    let names = match element {
        Element::Cold => COLD_EMITTERS,
        Element::Fire => FIRE_EMITTERS,
    };
    let seed = crate::shared::next_id(world);
    let emitters = names.map(|name| {
        let config = world
            .get_resource::<ParticleConfigRegistry>()?
            .id_for_name(name)?;
        Some(world.spawn_with((
            ParticleEmitter::with_seed(config, 0x1ce0_0000 + seed),
            ParticleEmitterState::default(),
            Transform::from_position(start),
            SceneEntity { state_id: GAMEPLAY },
        )))
    });
    for emitter in emitters[..2].iter().flatten() {
        world.insert(*emitter, SprayEmitter { direction: aim });
    }
    let started = match element {
        Element::Cold => Sound::ColdStarted,
        Element::Fire => Sound::FireStarted,
    };
    audio::play(world, started, start);
    let voice = loop_voice(
        world,
        match element {
            Element::Cold => COLD_LOOP,
            Element::Fire => FIRE_LOOP,
        },
    );
    let spray = world.spawn_with((
        Spray {
            caster,
            element,
            start,
            end,
            contact: None,
            aim,
            age: 0.0,
            emitters,
            voice,
            kept: true,
            pulse: SPRAY_PULSE,
            pulses: 0,
        },
        SceneEntity { state_id: GAMEPLAY },
    ));
    super::spray_look::start(world, spray, element);
    spray
}

/// Starts the manifest loop `id` at its volume: a spray's voice, which its
/// end stops, or the small balls'.
pub(crate) fn loop_voice(world: &mut World, id: &str) -> Option<tungsten::core::Voice> {
    let registry = world.get_resource::<SoundRegistry>()?;
    let handle = registry.get_by_id(id)?;
    let volume = registry.get_volume(handle);
    Some(
        world
            .resource_mut::<AudioCommands>()
            .play_with(handle, volume, true),
    )
}

/// Ends the spray `entity`: its particles and its look go, its puffs left
/// to play out, its loop fades out over [`LOOP_FADE`], and a caster still
/// alive hears its end, the cold spray's pressure release or the
/// flamethrower's dying roar.
pub fn end_spray(world: &mut World, entity: Entity) {
    let Some(spray) = world.get::<Spray>(entity).copied() else {
        return;
    };
    for emitter in spray.emitters.into_iter().flatten() {
        world.despawn(emitter);
    }
    super::spray_look::end(world, entity);
    if let Some(voice) = spray.voice {
        world
            .resource_mut::<AudioCommands>()
            .stop_voice(voice, LOOP_FADE);
    }
    let alive = world
        .get::<tungsten::kit::Health>(spray.caster)
        .is_some_and(|health| !health.is_dead());
    if alive {
        let ended = match spray.element {
            Element::Cold => Sound::ColdEnded,
            Element::Fire => Sound::FireEnded,
        };
        audio::play(world, ended, spray.start);
    }
    world.despawn(entity);
}

/// `sprays_end`, in the react phase: a spray no spell kept this step ends,
/// and the rest wait for the next step's spells.
pub fn sprays_end(world: &mut World) {
    let mut ended = Vec::new();
    for (entity, spray) in world.query_mut::<(Entity, &mut Spray)>() {
        if !spray.kept {
            ended.push(entity);
        }
        spray.kept = false;
    }
    let lost: Vec<Entity> = world
        .query::<(Entity, &Spray)>()
        .filter(|(_, spray)| {
            !world.is_alive(spray.caster)
                || world
                    .get::<tungsten::kit::Health>(spray.caster)
                    .is_some_and(tungsten::kit::Health::is_dead)
        })
        .map(|(entity, _)| entity)
        .collect();
    let mut seen = std::collections::HashSet::new();
    ended.extend(lost);
    ended.retain(|entity| seen.insert(*entity));
    for entity in ended {
        end_spray(world, entity);
    }
}

/// `spray_look`, in the present phase: each spray follows its caster's
/// drawn point and its aim, its stream and mist from the muzzle and its
/// contact where the cone meets a surface, iron first, which its sprites
/// read (`spray_look::sprites`); newborn particles turn to the aim they
/// were born at and die before a surface.
pub fn spray_look(world: &mut World) {
    orient_newborns(world);
    let dt = world.resource::<Time>().delta();
    let sprays: Vec<(Entity, Spray)> = world
        .query::<(Entity, &Spray)>()
        .map(|(entity, spray)| (entity, *spray))
        .collect();
    for (entity, mut spray) in sprays {
        let Some(at) = world.get::<Transform>(spray.caster).map(|t| t.position) else {
            continue;
        };
        let offset = world
            .get::<Collider>(spray.caster)
            .map_or(Vec2::ZERO, |c| c.offset);
        let centre = at + offset;
        let range = reach(spray.element);
        let (end, _) = beam(world, spray.caster, centre, spray.aim, range);
        let contact = contact_point(world, spray.caster, centre, spray.aim, range);
        spray.start = centre + spray.aim * SPRAY_MUZZLE.min(centre.distance(end));
        spray.end = end;
        spray.contact = contact;
        spray.age += dt;
        for (index, emitter) in spray.emitters.into_iter().enumerate() {
            let Some(emitter) = emitter else { continue };
            let at = if index == 2 {
                contact.unwrap_or(end)
            } else {
                spray.start
            };
            if let Some(transform) = world.get_mut::<Transform>(emitter) {
                transform.position = at;
            }
            if let Some(aimed) = world.get_mut::<SprayEmitter>(emitter) {
                aimed.direction = spray.aim;
            }
            if index == 2
                && let Some(state) = world.get_mut::<ParticleEmitterState>(emitter)
            {
                state.drained = contact.is_none();
            }
        }
        world.insert(entity, spray);
    }
}

/// Where a spray's cone meets a surface, iron before anything else.
fn contact_point(
    world: &World,
    caster: Entity,
    centre: Vec2,
    aim: Vec2,
    range: f32,
) -> Option<Vec2> {
    let (mut iron, mut surface): (Option<Vec2>, Option<Vec2>) = (None, None);
    let nearer = |best: Option<Vec2>, at: Vec2| {
        best.is_none_or(|p| centre.distance_squared(at) < centre.distance_squared(p))
    };
    for direction in rays(aim) {
        let (at, hit) = beam(world, caster, centre, direction, range);
        if centre.distance(at) >= range - 0.01 {
            continue;
        }
        if nearer(surface, at) {
            surface = Some(at);
        }
        let metal = hit.is_some_and(|e| world.has::<IronBlock>(e) || world.has::<IronScrap>(e));
        if metal && nearer(iron, at) {
            iron = Some(at);
        }
    }
    iron.or(surface)
}

/// The draw depth a spray's particles and sprites take once `spray_look`
/// meets them: over the enemies (8), the dummy (9) and the wizard (10),
/// under the balls (12) and iron. A frame's newborns draw once at the particles' 0
/// first, as they draw once unturned, since a particle config has no depth
/// (K2).
pub const SPRAY_Z: i32 = 11;

/// Lifts each newborn of a spray's three emitters to [`SPRAY_Z`], and turns
/// each of its two aimed emitters' to the aim and shortens its life so it
/// dies before the surface it would pass through within its own spray's
/// reach; the contact's splash is not aimed.
fn orient_newborns(world: &mut World) {
    let emitters: Vec<Entity> = world
        .query::<&Spray>()
        .flat_map(|spray| spray.emitters.into_iter().flatten())
        .collect();
    if emitters.is_empty() {
        return;
    }
    let newborns: Vec<(Entity, Option<(Vec2, f32)>)> = world
        .query::<(Entity, &Particle, &Transform)>()
        .filter_map(|(entity, particle, at)| {
            if world.has::<Oriented>(entity) || particle.age > 0.0 {
                return None;
            }
            let emitter = particle
                .emitter
                .filter(|emitter| emitters.contains(emitter))?;
            let Some(aimed) = world.get::<SprayEmitter>(emitter) else {
                return Some((entity, None));
            };
            let velocity = aimed.direction.rotate(particle.velocity);
            let speed = velocity.length();
            let direction = velocity.try_normalize()?;
            let (caster, element) = world
                .query::<&Spray>()
                .find(|spray| spray.emitters.contains(&particle.emitter))
                .map_or((entity, Element::Cold), |spray| {
                    (spray.caster, spray.element)
                });
            let (end, _) = beam(
                world,
                caster,
                at.position,
                direction,
                reach(element) - SPRAY_MUZZLE,
            );
            let lifetime = (at.position.distance(end) - BEAM_HALF_WIDTH).max(0.0) / speed;
            Some((entity, Some((velocity, lifetime))))
        })
        .collect();
    for (entity, turned) in newborns {
        if let Some((velocity, lifetime)) = turned
            && let Some(particle) = world.get_mut::<Particle>(entity)
        {
            particle.velocity = velocity;
            particle.lifetime = particle.lifetime.min(lifetime);
        }
        if let Some(sprite) = world.get_mut::<Sprite>(entity) {
            sprite.z_order = SPRAY_Z;
        }
        world.insert(entity, Oriented);
    }
}

/// `ball_look`, in the present phase: a frozen or chilled ball takes the
/// frost's tint, as deep as the frost; a burning or spent small ball its
/// ember tint, another small ball its rainbow hue, and a bowling ball its
/// own colours.
pub fn ball_look(world: &mut World) {
    let time = drawn_time(world);
    for (_, id, sprite, flame, hue, effects, tags) in world.query_mut::<(
        &Ball,
        Option<&Saved>,
        &mut Sprite,
        Option<&Flame>,
        Option<&BallHue>,
        Option<&Effects>,
        Option<&Tags>,
    )>() {
        // `freeze_amount` from the row; the frost wins, as it wrote last.
        let frost = if tags.is_some_and(|tags| tags.has(FROZEN)) {
            1.0
        } else {
            effects
                .and_then(|effects| effects.intensity(COAT))
                .unwrap_or(0.0)
        };
        if frost > 0.0 {
            sprite.color = frost_tint(frost);
            continue;
        }
        let Some(id) = id else { continue };
        // A lit ball's burn's seconds left, 0 once spent.
        let left = || {
            effects
                .and_then(|effects| {
                    effects
                        .instances()
                        .find(|instance| instance.effect().id() == Some(BALL_BURN))
                })
                .and_then(EffectInstance::left)
                .unwrap_or(0.0)
        };
        sprite.color = match (flame, hue) {
            (Some(_), _) => elements::ember_tint(id.id(), left(), time),
            (None, Some(hue)) => rainbow_rgba(hue.hue),
            (None, None) => [255; 4],
        };
    }
}

/// A full-saturation rainbow colour at `hue`, in turns: one channel at
/// 255, one at 0 and the third ramping between them, its ramp lifted so
/// the secondary hues read as bright.
#[allow(clippy::many_single_char_names)] // h/x/r/g/b in the hue-to-RGB math
#[must_use]
pub fn rainbow_rgba(hue: f32) -> [u8; 4] {
    let h = hue.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let x = (x.powf(0.55) * 255.0).round().clamp(0.0, 255.0) as u8;
    let (r, g, b) = match h as u32 {
        0 => (255, x, 0),
        1 => (x, 255, 0),
        2 => (0, 255, x),
        3 => (0, x, 255),
        4 => (x, 0, 255),
        _ => (255, 0, x),
    };
    [r, g, b, 255]
}

/// The tint frost gives iron: blue as it takes hold.
#[must_use]
pub fn frost_tint(amount: f32) -> [u8; 4] {
    [
        (255.0 - 155.0 * amount) as u8,
        (255.0 - 60.0 * amount) as u8,
        255,
        255,
    ]
}

/// The four broken-metal sprites and their frost.
const SCRAP_SPRITES: [&str; 4] = [
    "ex10_iron_scrap_0",
    "ex10_iron_scrap_1",
    "ex10_iron_scrap_2",
    "ex10_iron_scrap_3",
];
const SCRAP_FROST_SPRITES: [&str; 4] = [
    "ex10_iron_scrap_frost_0",
    "ex10_iron_scrap_frost_1",
    "ex10_iron_scrap_frost_2",
    "ex10_iron_scrap_frost_3",
];

/// Shows a scrap's broken-metal sprite of `variant`.
pub fn show_scrap_variant(world: &mut World, scrap: Entity, variant: usize) {
    let id = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(SCRAP_SPRITES[variant % 4]);
    if let Some(sprite) = world.get_mut::<Sprite>(scrap) {
        sprite.asset_id = id;
    }
}

/// `iron_look`, in the present phase after the tween tick: iron takes the
/// frost's tint, and its frost art shows as deep as the frost, over the
/// metal; a scrap's frost turns with it as it tumbles ([`toys::tumble`]).
pub fn iron_look(world: &mut World) {
    let blocks: Vec<(Entity, f32, Vec<Entity>)> = world
        .query::<(Entity, &IronBlock, &Children)>()
        .map(|(entity, _, children)| {
            (
                entity,
                freeze_amount(world, entity),
                children.iter().collect(),
            )
        })
        .collect();
    for (_, amount, children) in blocks {
        for (index, child) in children.into_iter().enumerate() {
            let frost = index >= 4;
            tint(world, child, amount, frost);
        }
    }
    let scraps: Vec<(Entity, f32, usize, f32, Vec<Entity>)> = world
        .query::<(Entity, &IronScrap, &Transform, Option<&Children>)>()
        .map(|(entity, scrap, transform, children)| {
            let turn = transform.rotation;
            let children = children.map(|c| c.iter().collect()).unwrap_or_default();
            (
                entity,
                freeze_amount(world, entity),
                scrap.variant,
                turn,
                children,
            )
        })
        .collect();
    for (scrap, amount, variant, turn, children) in scraps {
        if let Some(sprite) = world.get_mut::<Sprite>(scrap) {
            sprite.color = frost_tint(amount);
        }
        for child in children {
            let id = world
                .resource_mut::<AssetRegistry>()
                .intern_sprite(SCRAP_FROST_SPRITES[variant % 4]);
            if let Some(sprite) = world.get_mut::<Sprite>(child) {
                sprite.asset_id = id;
            }
            if let Some(transform) = world.get_mut::<Transform>(child) {
                transform.rotation = turn;
                transform.scale = Vec2::splat(SCRAP_SCALE);
            }
            tint(world, child, amount, true);
        }
    }
}

/// Tints one of iron's art pieces: the metal by the frost, the frost art
/// shown only while there is frost, as opaque as it is deep.
fn tint(world: &mut World, piece: Entity, amount: f32, frost: bool) {
    if let Some(sprite) = world.get_mut::<Sprite>(piece) {
        sprite.color = if frost {
            [255, 255, 255, (255.0 * amount) as u8]
        } else {
            frost_tint(amount)
        };
    }
    if frost && let Some(visibility) = world.get_mut::<Visibility>(piece) {
        visibility.visible = amount > 0.0;
    }
}

/// `hole_particles`, in the present phase: a hole's newborn particles are
/// set on a ring about it, and every one of its particles is aimed at the
/// next point of a spiral inward, so the dust reads as accretion; one at
/// the core is gone. Its opening's rush (an emitter with [`OpeningOf`]) is
/// set [`OPEN_REACH`] out and drawn in at [`OPEN_INWARD`] px/s.
pub fn hole_particles(world: &mut World) {
    // No hole open, no particle of its to steer (P6).
    if world.query::<&BlackHole>().next().is_none() {
        return;
    }
    let dt = world.resource::<Time>().delta();
    let steered: Vec<(Entity, Vec2, bool, Vec2)> = world
        .query::<(Entity, &Particle, &Transform)>()
        .filter_map(|(entity, particle, at)| {
            let emitter = particle.emitter?;
            let opening = world.get::<OpeningOf>(emitter).map(|of| of.0);
            let owner = opening.unwrap_or_else(|| {
                world
                    .get::<tungsten::kit::Parent>(emitter)
                    .map_or(emitter, |parent| parent.entity())
            });
            world.get::<BlackHole>(owner)?;
            let centre = world.get::<Transform>(owner)?.position;
            let (reach, share, inward) = if opening.is_some() {
                (OPEN_REACH, 0.08, OPEN_INWARD)
            } else {
                (110.0, 0.25, 105.0)
            };
            let position = if particle.age == 0.0 {
                centre
                    + particle.velocity.normalize_or_zero()
                        * (reach + particle.velocity.length() * share)
            } else {
                at.position
            };
            let delta = position - centre;
            let radial = delta.normalize_or_zero();
            let tangent_speed = (32_000.0 / delta.length().max(50.0)).clamp(180.0, 640.0);
            // Aim at the next point on the spiral: tangential speed alone
            // drifts outward near the core at low frame rates.
            let velocity = if dt > 0.0 {
                let (sin, cos) = (tangent_speed / delta.length().max(14.0) * dt).sin_cos();
                let next = Vec2::new(
                    radial.x * cos - radial.y * sin,
                    radial.x * sin + radial.y * cos,
                );
                (next * (delta.length() - inward * dt).max(0.0) - delta) / dt
            } else {
                particle.velocity
            };
            Some((entity, position, delta.length() < 14.0, velocity))
        })
        .collect();
    for (entity, position, absorbed, velocity) in steered {
        if let Some(at) = world.get_mut::<Transform>(entity) {
            at.position = position;
        }
        if let Some(particle) = world.get_mut::<Particle>(entity) {
            particle.velocity = velocity;
            if absorbed {
                particle.age = particle.lifetime;
            }
        }
    }
}

/// The bodies a query of `With<Ball>` holds, for tests.
#[must_use]
pub fn balls(world: &World) -> Vec<Entity> {
    world.query_filtered::<Entity, With<Ball>>().collect()
}
