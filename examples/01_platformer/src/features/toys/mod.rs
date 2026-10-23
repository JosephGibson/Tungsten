//! The physics toys: small balls set down at a point, bowling balls tossed,
//! iron blocks placed, black holes opened, pushes from a point, sprays of
//! fire or cold along an aim, and the fire missile's flight, blast and
//! bending, each a function the spells call. Their caps bound the pile:
//! [`BALL_CAP`] balls, [`IRON_CAP`] blocks, [`MISSILE_CAP`] missiles and
//! [`HOLE_CAP`] black holes.

pub mod components;
pub mod draw;
pub mod systems;

use glam::Vec2;
use tungsten::core::{
    BodyKind, CommandBuffer, Entity, EventCursor, RigidBody, Schedule, Shape, Stage, Time,
    Transform, Velocity, World, system,
};
use tungsten::kit::{LINEAR_MOTION, LinearMotion, PROJECTILE_TICK, Phase, ensure_phases};
use tungsten::physics::{SpatialFilter, overlap};
use tungsten::plugins::{PARTICLE_COUNT_REFRESH, PHYSICS_SYNC};
use tungsten::{Extracts, Plugin, SceneEntity};

use crate::features::audio::{self, components::Sound};
use crate::features::level::TILE;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, SaveIds, body_centre, despawn_tree, spawn_burst};
use components::{
    Ball, BallHue, BallSpawner, BlackHole, BowlingBall, Element, FireMissile, IronBlock, IronScrap,
    PreviousVelocity, Ring, ShockRing, SmallBall, Spray,
};

/// A bowling ball's radius, and a small ball's, each half its drawn size.
pub const BALL_RADIUS: f32 = 15.0;
pub const SMALL_BALL_RADIUS: f32 = 7.5;
/// The balls' art is 64 px, drawn at these scales: 32 and 16 px, the body
/// centred in it, this far from the entity's point.
pub const BALL_SCALE: f32 = 0.5;
pub const SMALL_BALL_SCALE: f32 = 0.25;
pub const BALL_CENTRE: Vec2 = Vec2::splat(16.0);
pub const SMALL_BALL_CENTRE: Vec2 = Vec2::splat(8.0);
/// Most balls alive at once; placing stops at the count (`D-094`).
pub const BALL_CAP: usize = 12_000;
/// Seconds between two small balls while their spell holds: five times
/// today's orb rate.
pub const SMALL_BALL_INTERVAL: f32 = 0.032 / 5.0;
/// The ring small balls are set down on about the point, so no two meet
/// in one place.
pub const BALL_JITTER: f32 = TILE / 16.0;
/// A tossed bowling ball's speed, and the steepest it leaves the hand at,
/// so it lands low and rolls.
pub const TOSS_SPEED: f32 = 700.0;
pub const TOSS_STEEPEST: f32 = 25.0 * std::f32::consts::PI / 180.0;
/// Where a tossed ball leaves the caster's centre, along the aim.
pub const TOSS_OFFSET: f32 = 28.0;
/// An iron block's half size, its mass, and its art's offset: 128 px of
/// art about the 120 px block. Most blocks at once: a new one replaces the
/// oldest, which poofs.
pub const IRON_HALF: Vec2 = Vec2::splat(4.0 * BALL_RADIUS);
pub const IRON_MASS: f32 = 400.0;
pub const IRON_CAP: usize = 24;
/// Iron's weight: a block falling faster than [`IRON_FALL_FROM`] falls
/// harder by [`IRON_WEIGHT`] px/s², half the world's gravity again, unless
/// a troll holds or throws it.
pub const IRON_WEIGHT: f32 = 1800.0;
pub const IRON_FALL_FROM: f32 = 30.0;
/// Iron's blow: a body hit from iron is pushed along the closing line by
/// this share of the closing speed, at most [`IRON_PUSH_MOST`] px/s.
pub const IRON_PUSH_SHARE: f32 = 0.6;
pub const IRON_PUSH_MOST: f32 = 900.0;
/// Iron's hard landing: a block meeting a still body or a tile at this
/// closing speed or more raises dust and shakes the camera by this share of
/// a blast's.
pub const IRON_LAND_SPEED: f32 = 900.0;
pub const IRON_LAND_SHAKE: f32 = 0.2;
/// A scrap: a quarter of a block's side, a sixteenth of its mass, its art
/// drawn at half size.
pub const SCRAP_HALF: Vec2 = Vec2::splat(IRON_HALF.x / 4.0);
pub const SCRAP_COUNT: usize = 16;
pub const SCRAP_MASS: f32 = IRON_MASS / 16.0;
pub const SCRAP_SCALE: f32 = 0.5;
/// Sliding friction while iron rests on something, px/s², and on ice; a
/// scrap's is a sixteenth.
pub const IRON_FRICTION: f32 = 1500.0;
pub const FROZEN_FRICTION: f32 = 120.0;
/// How much of a hole's pull iron takes, and a scrap.
pub const IRON_PULL: f32 = 0.2;
pub const SCRAP_PULL: f32 = 1.0;
/// A black hole's reach, pull and life, which its prefab's `left` restates;
/// at most [`HOLE_CAP`] are open, a further one closing the oldest.
pub const HOLE_RADIUS: f32 = 6.0 * TILE;
pub const HOLE_FORCE: f32 = 12_000.0;
pub const HOLE_SECS: f32 = 6.0;
pub const HOLE_DIAMETER: f32 = 2.8125 * TILE;
pub const HOLE_CAP: usize = 3;
/// A hole shrinks to nothing over its last seconds, and its collapse
/// leaves a small ring this many pixels across.
pub const HOLE_SHRINK_SECS: f32 = 0.5;
pub const COLLAPSE_RING: f32 = 2.0 * HOLE_DIAMETER;
/// The broom: the small balls whose centre comes within this reach of a
/// hole's centre go, at most this many a hole a step.
pub const BROOM_REACH: f32 = 24.0;
pub const BROOM_MOST: usize = 12;
/// The fire missile: its speed, its fall, its reach, its body's radius,
/// its drawn size, and at most so many at once.
pub const MISSILE_SPEED: f32 = 900.0;
pub const MISSILE_GRAVITY: f32 = 360.0;
pub const MISSILE_RANGE: f32 = MISSILE_SPEED * 2.1;
pub const MISSILE_RADIUS: f32 = 10.0;
pub const MISSILE_SIZE: f32 = 40.0;
pub const MISSILE_CAP: usize = 6;
/// A blast: its heat's reach, which lights small balls and shatters frozen
/// iron, its push's reach and speed at its centre, and its camera kick.
pub const BLAST_RADIUS: f32 = 72.0;
pub const PUSH_RADIUS: f32 = 4.5 * TILE;
pub const PUSH_SPEED: f32 = 2500.0;
pub const BLAST_TRAUMA: f32 = 0.68;
/// The force push's reach, wider than a blast's.
pub const FORCE_PUSH_RADIUS: f32 = 6.0 * TILE;
/// A spray's reach and half angle, and the rays it traces.
pub const SPRAY_RANGE: f32 = 7.0 * TILE;
pub const SPRAY_HALF_ANGLE: f32 = 14.0 * std::f32::consts::PI / 180.0;
pub const FIRE_SPRAY_RANGE: f32 = 3.5 * TILE;
/// Most shock rings at once.
pub const RING_CAP: usize = 16;
/// The small balls' loop, which plays while they pour, and the seconds a
/// spray's loop and theirs fade over when they stop.
pub const SMALL_BALLS_LOOP: &str = "ex10_small_balls_sfx";
pub const LOOP_FADE: f32 = 0.08;
/// The toys' prefabs, by their manifest IDs.
pub const SMALL_BALL: &str = "small_ball";
pub const BOWLING_BALL: &str = "bowling_ball";
pub const IRON_BLOCK_PREFAB: &str = "iron_block";
pub const IRON_SCRAP_PREFAB: &str = "iron_scrap";
pub const BLACK_HOLE_PREFAB: &str = "black_hole";
pub const FIRE_MISSILE_PREFAB: &str = "fire_missile";

/// The toys as a plugin: the components their prefabs name; in the fixed
/// step, the holes' life, the balls' hues and cooldowns, the velocities
/// before physics, the holes' pull, the missiles' bending before their
/// motion, their contacts before the kit's projectile step and their ends
/// after it, so a blast's hits take the step's resolve, the escaped
/// bodies' cull, the holes' broom and the sprays' end; in the present
/// phase the toys' looks, and before the frame's particles the sprays' and
/// the holes' particles; and their effects drawn after the sprites, the
/// holes under them (`level::draw::base`).
pub struct ToysPlugin;

impl Plugin for ToysPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_component::<Ball>("ball");
        world.register_component::<SmallBall>("small_ball");
        world.register_component::<BowlingBall>("bowling_ball");
        world.register_component::<IronBlock>("iron_block");
        world.register_component::<IronScrap>("iron_scrap");
        world.register_component::<BlackHole>("black_hole");
        world.register_component::<FireMissile>("fire_missile");
        world.insert_resource(BallSpawner::default());
        world.resource_mut::<Extracts>().add_sprites(draw::effects);
        ensure_phases(schedule);
        let fixed = [
            Phase::Tick.join(system("toys_tick", systems::tick)),
            Phase::Move.join(system("toys_velocities", systems::remember_velocities)),
            Phase::Move.join(system("iron_weight", systems::weight).after("toys_velocities")),
            Phase::Move.join(system("toys_pull", systems::pull)),
            Phase::Move.join(system("missile_bend", systems::bend).before(LINEAR_MOTION)),
            Phase::Overlap.join(
                system("missile_contacts", systems::missile_contacts).before(PROJECTILE_TICK),
            ),
            Phase::Overlap.join(system("toys_bounds", systems::bounds)),
            Phase::Overlap.join(system("hole_broom", systems::broom)),
            Phase::Overlap.join(system("iron_friction", systems::friction)),
            Phase::Overlap.join(
                system("missile_ends", {
                    let mut cursor = EventCursor::new();
                    move |world: &mut World| systems::missile_ends(world, &mut cursor)
                })
                .after(PROJECTILE_TICK),
            ),
            Phase::React.join(system("sprays_end", systems::sprays_end)),
        ];
        for desc in fixed {
            schedule.add(Stage::FixedUpdate, desc);
        }
        let present = [
            system("ball_look", systems::ball_look),
            system("iron_look", systems::iron_look),
            system("missile_look", systems::missile_look),
            system("toys_flares", systems::flares),
        ];
        for desc in present {
            schedule.add(Stage::PostUpdate, Phase::Present.join(desc));
        }
        // Before the frame's emission, which they aim and place, and the
        // particles' tick, which would age their newborns first.
        for desc in [
            system("spray_look", systems::spray_look),
            system("hole_particles", systems::hole_particles),
        ] {
            let desc = desc.after(PHYSICS_SYNC).before(PARTICLE_COUNT_REFRESH);
            schedule.add(Stage::PostUpdate, desc);
        }
    }
}

/// The run's next save ID.
pub(crate) fn next_id(world: &mut World) -> SaveId {
    world.resource_mut::<SaveIds>().take()
}

/// A toy's prefab at `at`, owned by gameplay, with the run's next save ID
/// and its art at `scale`, spawned at once: a toy's or a prop's.
pub(crate) fn spawn_toy(world: &mut World, prefab: &str, at: Vec2, scale: f32) -> Entity {
    let id = next_id(world);
    let transform = Transform {
        position: at,
        rotation: 0.0,
        scale: Vec2::splat(scale),
    };
    world
        .prepare_prefab(prefab, at.to_array())
        .expect("the manifest's prefabs load with it")
        .with((
            transform,
            id,
            PreviousVelocity::default(),
            SceneEntity { state_id: GAMEPLAY },
        ))
        .spawn(world)
}

/// How many balls are alive or queued in this frame's earlier steps.
#[must_use]
pub fn ball_count(world: &World) -> usize {
    let frame = world.resource::<Time>().frame();
    let spawner = world.resource::<BallSpawner>();
    let queued = if spawner.queued_frame == frame {
        spawner.queued as usize
    } else {
        0
    };
    world.query::<&Ball>().count() + queued
}

/// Sets down the small balls `dt` of a held spell owes at `at`: one every
/// [`SMALL_BALL_INTERVAL`], each on the golden-angle ring about the point,
/// while fewer than [`BALL_CAP`] balls live, their loop playing while they
/// pour. They spawn at the frame's command flush; a cap that stops them
/// drops what was owed. Returns how many it queued.
pub fn place_small_balls(world: &mut World, at: Vec2, dt: f32) -> u32 {
    let budget = BALL_CAP.saturating_sub(ball_count(world)) as u32;
    let frame = world.resource::<Time>().frame();
    let spawner = world.resource_mut::<BallSpawner>();
    if spawner.queued_frame != frame {
        spawner.queued = 0;
        spawner.queued_frame = frame;
    }
    if budget == 0 {
        spawner.owed = 0.0;
        return 0;
    }
    spawner.owed += dt;
    let mut count = 0;
    while spawner.owed >= SMALL_BALL_INTERVAL {
        spawner.owed -= SMALL_BALL_INTERVAL;
        count += 1;
    }
    let count = count.min(budget);
    let first = spawner.phase;
    spawner.phase = spawner.phase.wrapping_add(count);
    spawner.queued += count;
    const GOLDEN_ANGLE: f32 = 2.399_963_2;
    for i in 0..count {
        let phase = first.wrapping_add(i);
        let angle = phase as f32 * GOLDEN_ANGLE;
        let centre = at + Vec2::new(angle.cos(), angle.sin()) * BALL_JITTER;
        let point = centre - SMALL_BALL_CENTRE;
        let id = next_id(world);
        let transform = Transform {
            position: point,
            rotation: 0.0,
            scale: Vec2::splat(SMALL_BALL_SCALE),
        };
        let prepared = world
            .prepare_prefab(SMALL_BALL, point.to_array())
            .expect("the manifest's prefabs load with it")
            .with((
                transform,
                id,
                BallHue::from_seed(phase),
                PreviousVelocity::default(),
                SceneEntity { state_id: GAMEPLAY },
            ));
        world.resource_mut::<CommandBuffer>().spawn_prefab(prepared);
    }
    if count > 0 && world.resource::<BallSpawner>().voice.is_none() {
        let voice = systems::loop_voice(world, SMALL_BALLS_LOOP);
        world.resource_mut::<BallSpawner>().voice = voice;
    }
    count
}

/// Forgets what a held small-ball spell owed and stops their loop, as a
/// release does.
pub fn stop_small_balls(world: &mut World) {
    world.resource_mut::<BallSpawner>().owed = 0.0;
    stop_small_balls_loop(world);
}

/// Stops the small balls' loop over [`LOOP_FADE`], when it plays: a
/// release, or gameplay leaving the top, between two steps too.
pub fn stop_small_balls_loop(world: &mut World) {
    if let Some(voice) = world.resource_mut::<BallSpawner>().voice.take() {
        world
            .resource_mut::<tungsten::core::AudioCommands>()
            .stop_voice(voice, LOOP_FADE);
    }
}

/// Spawns a small ball or a bowling ball with its centre at `at`, at once:
/// what a test or a load sets down where it wants it.
pub fn spawn_ball(world: &mut World, at: Vec2, small: bool) -> Entity {
    let ball = if small {
        spawn_toy(world, SMALL_BALL, at - SMALL_BALL_CENTRE, SMALL_BALL_SCALE)
    } else {
        spawn_toy(world, BOWLING_BALL, at - BALL_CENTRE, BALL_SCALE)
    };
    if small {
        let phase = world.get::<SaveId>(ball).map_or(0, |id| id.0 as u32);
        world.insert(ball, BallHue::from_seed(phase));
    }
    ball
}

/// Tosses a bowling ball from `from`, the caster's centre, along `aim`: it
/// leaves the hand at [`TOSS_SPEED`], no steeper than [`TOSS_STEEPEST`].
/// None past the ball cap.
pub fn toss_bowling_ball(world: &mut World, from: Vec2, aim: Vec2) -> Option<Entity> {
    if ball_count(world) >= BALL_CAP {
        return None;
    }
    let aim = aim.try_normalize().unwrap_or(Vec2::X);
    let side = if aim.x < 0.0 { -1.0 } else { 1.0 };
    let angle = (-aim.y).atan2(aim.x.abs()).min(TOSS_STEEPEST);
    let launch = Vec2::new(side * angle.cos(), -angle.sin());
    let centre = from + aim * TOSS_OFFSET;
    let ball = spawn_toy(world, BOWLING_BALL, centre - BALL_CENTRE, BALL_SCALE);
    if let Some(velocity) = world.get_mut::<Velocity>(ball) {
        velocity.0 = launch * TOSS_SPEED;
    }
    audio::play(world, Sound::Tossed, centre);
    Some(ball)
}

/// Whether an iron block fits with its centre at `at`: no solid tile, no
/// body that is not dynamic (a deck, a slab, lava's surface, the elevator,
/// the courier, an enemy) and no other block in its box, at the last step's
/// places, sensors aside; balls and the other dynamic bodies are shoved
/// aside as it lands (feedback Q9).
#[must_use]
pub fn iron_fits(world: &World, at: Vec2) -> bool {
    let mut met = Vec::new();
    let shape = Shape::Aabb {
        half_extents: IRON_HALF,
    };
    overlap(world, shape, at, &SpatialFilter::new(), &mut met);
    met.iter().all(|hit| {
        hit.is_some_and(|entity| {
            !world.has::<IronBlock>(entity)
                && world
                    .get::<RigidBody>(entity)
                    .is_some_and(|body| body.kind == BodyKind::Dynamic)
        })
    })
}

/// Places an iron block with its centre at `at`; past [`IRON_CAP`] the
/// oldest goes in a poof, as a summon's oldest does.
pub fn place_iron(world: &mut World, at: Vec2) -> Entity {
    let mut blocks: Vec<(SaveId, Entity)> = world
        .query::<(Entity, &IronBlock, &SaveId)>()
        .map(|(entity, _, id)| (*id, entity))
        .collect();
    blocks.sort_by_key(|(id, _)| *id);
    let surplus = (blocks.len() + 1).saturating_sub(IRON_CAP);
    for &(_, oldest) in blocks.iter().take(surplus) {
        if let Some(centre) = body_centre(world, oldest) {
            spawn_burst(world, "ex10_poof", centre);
        }
        despawn_tree(world, oldest);
    }
    let block = spawn_toy(
        world,
        IRON_BLOCK_PREFAB,
        at - IRON_HALF - Vec2::splat(4.0),
        1.0,
    );
    audio::play(world, Sound::Placed, at);
    block
}

/// Spawns a scrap of `variant` with its centre at `at`, flying at
/// `velocity`, tumbling from level time `born_at`.
pub fn spawn_scrap(
    world: &mut World,
    at: Vec2,
    velocity: Vec2,
    variant: usize,
    tumble: components::ScrapTumble,
) -> Entity {
    let scrap = spawn_toy(
        world,
        IRON_SCRAP_PREFAB,
        at - SCRAP_HALF - Vec2::splat(1.0),
        SCRAP_SCALE,
    );
    let variant = variant % 4;
    world.insert_bundle(scrap, (IronScrap { variant }, tumble));
    if let Some(v) = world.get_mut::<Velocity>(scrap) {
        v.0 = velocity;
    }
    systems::show_scrap_variant(world, scrap, variant);
    scrap
}

/// A saved ball set back with its point at `at`, the top-left of its art,
/// at once and quietly: a load's.
pub fn restore_ball(world: &mut World, small: bool, at: Vec2) -> Entity {
    if small {
        spawn_toy(world, SMALL_BALL, at, SMALL_BALL_SCALE)
    } else {
        spawn_toy(world, BOWLING_BALL, at, BALL_SCALE)
    }
}

/// A saved iron block set back with its point at `at`, quietly.
pub fn restore_iron(world: &mut World, at: Vec2) -> Entity {
    spawn_toy(world, IRON_BLOCK_PREFAB, at, 1.0)
}

/// A saved scrap of `variant` set back with its point at `at`, at rest.
pub fn restore_scrap(world: &mut World, at: Vec2, variant: usize) -> Entity {
    let scrap = spawn_toy(world, IRON_SCRAP_PREFAB, at, SCRAP_SCALE);
    let variant = variant % 4;
    let rest = components::ScrapTumble {
        born_at: 0.0,
        initial_angle: 0.0,
        total_turn: 0.0,
    };
    world.insert_bundle(scrap, (IronScrap { variant }, rest));
    systems::show_scrap_variant(world, scrap, variant);
    scrap
}

/// A saved black hole set back open at `at`, quietly.
pub fn restore_hole(world: &mut World, at: Vec2) -> Entity {
    spawn_toy(world, BLACK_HOLE_PREFAB, at, 1.0)
}

/// Opens a black hole at `at` for [`HOLE_SECS`]; past [`HOLE_CAP`] the
/// oldest, the lowest save ID, collapses first.
pub fn place_black_hole(world: &mut World, at: Vec2) -> Entity {
    let mut open: Vec<(SaveId, Entity)> = world
        .query::<(Entity, &BlackHole, &SaveId)>()
        .map(|(entity, _, id)| (*id, entity))
        .collect();
    open.sort_by_key(|(id, _)| *id);
    let surplus = (open.len() + 1).saturating_sub(HOLE_CAP);
    for &(_, oldest) in open.iter().take(surplus) {
        systems::collapse(world, oldest);
    }
    let hole = spawn_toy(world, BLACK_HOLE_PREFAB, at, 1.0);
    audio::play(world, Sound::HoleOpened, at);
    hole
}

/// How many missiles fly.
#[must_use]
pub fn missile_count(world: &World) -> usize {
    world.query::<&FireMissile>().count()
}

/// Fires a missile from `centre` at `velocity`, at once: the prefab the
/// fire missile's spell spawns, set at its centre. None past
/// [`MISSILE_CAP`].
pub fn fire_missile(world: &mut World, centre: Vec2, velocity: Vec2) -> Option<Entity> {
    if missile_count(world) >= MISSILE_CAP {
        return None;
    }
    let at = centre - Vec2::splat(MISSILE_SIZE * 0.5);
    let missile = spawn_toy(world, FIRE_MISSILE_PREFAB, at, MISSILE_SIZE / 64.0);
    if let Some(motion) = world.get_mut::<LinearMotion>(missile) {
        motion.set_velocity(velocity);
    }
    systems::show(world, missile);
    Some(missile)
}

/// Makes `missile`, which an ability's spawn payload landed from its
/// caster's point, the top-left of the caster's art, one of the toys: the
/// run's next save ID and the fall the payload's motion leaves out, drawn
/// at its size, shown, and moved so its centre sits `centre` from where
/// it landed, as the caster's centre sits from the caster's point.
pub fn adopt_missile(world: &mut World, missile: Entity, centre: Vec2) {
    let id = next_id(world);
    world.insert(missile, id);
    world.insert(missile, PreviousVelocity::default());
    let shift = centre - Vec2::splat(MISSILE_SIZE * 0.5);
    systems::shift(world, missile, shift);
    if let Some(motion) = world.get_mut::<LinearMotion>(missile) {
        *motion =
            LinearMotion::new(motion.velocity()).with_acceleration(Vec2::new(0.0, MISSILE_GRAVITY));
    }
    if let Some(transform) = world.get_mut::<Transform>(missile) {
        transform.scale = Vec2::splat(MISSILE_SIZE / 64.0);
    }
    systems::show(world, missile);
}

/// Pushes what stands within `radius` of `at` straight away from it, up
/// at the centre: [`PUSH_SPEED`] there, falling linearly to 0 at the
/// reach, a dynamic body over its mass and a live enemy by its kind's pull
/// (`systems::radial_push`).
pub fn push(world: &mut World, at: Vec2, radius: f32) {
    systems::radial_push(world, at, radius, PUSH_SPEED);
}

/// A force push from `at`: the push over [`FORCE_PUSH_RADIUS`], its ring
/// and its warp in the gravity purple.
pub fn force_push(world: &mut World, at: Vec2) {
    push(world, at, FORCE_PUSH_RADIUS);
    systems::ring(world, at, 2.0 * FORCE_PUSH_RADIUS, Ring::Push);
    spawn_burst(world, "ex10_warp_burst", at);
}

/// A fireball's blast at `at`: its fire on the targets in its heat but the
/// player's, its bursts and hot shock ring, the frozen iron in its heat
/// shattered, the push, the camera's kick, the small balls in its heat
/// lit, its sound and its flare.
pub fn explode(world: &mut World, at: Vec2) {
    systems::explode(world, at);
}

/// Holds `caster`'s spray of `element` along `aim` for this step: started
/// on its first, with its particles and its loop, its cone traced, iron in
/// a cold cone chilled by the step and small balls in a fire cone lit. A
/// step whose spells hold no spray ends it.
pub fn spray(world: &mut World, caster: Entity, aim: Vec2, element: Element) {
    systems::spray(world, caster, aim, element);
}

/// Starts `caster`'s spray of `element` along `aim` when none runs, with
/// its particles and sounds but none of a step's exposure: a press between
/// two steps shows at once, and the next step's spell sprays.
pub fn start_spray(world: &mut World, caster: Entity, aim: Vec2, element: Element) {
    systems::hold_spray(world, caster, aim, element);
}

/// The element of `caster`'s spray, while one runs.
#[must_use]
pub fn spraying(world: &World, caster: Entity) -> Option<Element> {
    world
        .query::<&Spray>()
        .find(|spray| spray.caster == caster)
        .map(|spray| spray.element)
}

/// Ends every spray at once, as a release or a death does.
pub fn end_sprays(world: &mut World) {
    let sprays: Vec<Entity> = world
        .query::<(Entity, &Spray)>()
        .map(|(entity, _)| entity)
        .collect();
    for spray in sprays {
        systems::end_spray(world, spray);
    }
}

/// The ring a bowling ball bursts into: presentation.
pub(crate) fn ball_ring(world: &mut World, at: Vec2) {
    systems::ring(world, at, 140.0, Ring::Ball);
}

/// A shock ring's whole life, seconds.
pub const RING_SECS: f32 = 0.45;

/// Whether a shock ring may still spawn.
#[must_use]
pub fn rings_full(world: &World) -> bool {
    world.query::<&ShockRing>().count() >= RING_CAP
}
