//! What two features share: the save IDs every saved entity carries, the
//! actors' bodies as their data holds them, a body's centre and the
//! distance to its surface, the short-lived particle bursts, cosmetic and
//! key, and the actions read off the input the UI left to the game.

use std::collections::BTreeMap;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::{
    ActionMap, Collider, DataRegistry, Entity, InputState, Particle, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, Position, RoutedInput, Shape, Transform, World,
};

use tungsten::kit::{Aim, Children, Hurtbox};

use crate::game::GAMEPLAY;

/// A saved entity's stable identity: a save names entities by it, never by
/// `Entity`, which the ECS reuses. The map's objects and the player take
/// theirs from their names ([`SaveId::authored`]), at or above
/// [`AUTHORED`]; anything spawned later takes the run's next
/// ([`SaveIds`]), below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SaveId(pub u64);

/// The bit every authored save ID sets and no runtime one reaches.
pub const AUTHORED: u64 = 1 << 31;

impl SaveId {
    /// The save ID of the object named `name`: the name's 32-bit FNV-1a
    /// hash with [`AUTHORED`] set, so the map's order moves no save.
    #[must_use]
    pub fn authored(name: &str) -> Self {
        let mut hash: u32 = 0x811c_9dc5;
        for byte in name.bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        Self(u64::from(hash) | AUTHORED)
    }
}

/// The run's next save ID, a `World` resource: what spawns during a run
/// takes the next, counting from 0 below [`AUTHORED`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveIds {
    next: u64,
}

impl SaveIds {
    /// A counter whose next ID is `next`, as a save holds it.
    #[must_use]
    pub fn starting_at(next: u64) -> Self {
        Self { next }
    }

    /// The ID the next spawn takes.
    #[must_use]
    pub fn peek(&self) -> u64 {
        self.next
    }

    /// Takes the next ID.
    ///
    /// # Panics
    ///
    /// When the count reaches [`AUTHORED`], where the map's IDs start.
    pub fn take(&mut self) -> SaveId {
        assert!(self.next < AUTHORED, "the run's save IDs ran out");
        let id = SaveId(self.next);
        self.next += 1;
        id
    }
}

/// `entity`'s body centre: its `Position` plus its collider's offset, the
/// point itself for an entity with no collider. None without a position.
#[must_use]
pub fn body_centre(world: &World, entity: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(entity)?.0;
    Some(
        at + world
            .get::<Collider>(entity)
            .map_or(Vec2::ZERO, |c| c.offset),
    )
}

/// The data kind of the actors' bodies ([`Body`]), each by its actor's
/// name: `player`, `dummy` and each enemy kind's plain prefab's.
pub const BODIES: &str = "bodies";

/// An actor's body in its frames, from a frame's top-left facing right: the
/// `bodies` data kind, one file an actor, which the generator writes from
/// the table its art is drawn to (`tools/actors.py`'s `BODIES`). Its
/// frame's size, its rigid body's centre and half size, its hurtbox, and
/// its named points facing either way, each left-facing one mirrored about
/// the body's centre, where a left-facing frame's body is drawn. A spawn of
/// the actor takes its collider and hurtbox from here ([`fit_body`]), over
/// its prefab's copies, which the generator holds to it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "BodyFile")]
pub struct Body {
    pub frame: Vec2,
    pub centre: Vec2,
    pub half: Vec2,
    pub hurtbox: Hurtbox,
    /// Each point by its name, facing left and right.
    points: BTreeMap<String, [Vec2; 2]>,
}

/// A body's file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BodyFile {
    frame: [f32; 2],
    centre: [f32; 2],
    half: [f32; 2],
    hurtbox: Hurtbox,
    #[serde(default)]
    points: BTreeMap<String, Facings>,
}

/// A point facing left and right.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Facings {
    left: [f32; 2],
    right: [f32; 2],
}

impl From<BodyFile> for Body {
    fn from(file: BodyFile) -> Self {
        Self {
            frame: Vec2::from(file.frame),
            centre: Vec2::from(file.centre),
            half: Vec2::from(file.half),
            hurtbox: file.hurtbox,
            points: file
                .points
                .into_iter()
                .map(|(name, at)| (name, [Vec2::from(at.left), Vec2::from(at.right)]))
                .collect(),
        }
    }
}

impl Body {
    /// Its point `name` from its entity's point as it faces, left when
    /// `heading` is below 0; None when it has no point of that name.
    #[must_use]
    pub fn point(&self, name: &str, heading: f32) -> Option<Vec2> {
        let [left, right] = *self.points.get(name)?;
        Some(if heading < 0.0 { left } else { right })
    }

    /// Its rigid body's collider: its half size about its centre.
    #[must_use]
    pub fn collider(&self) -> Collider {
        Collider {
            shape: Shape::Aabb {
                half_extents: self.half,
            },
            offset: self.centre,
        }
    }
}

/// `actor`'s body, as its data holds it now.
///
/// # Panics
///
/// When the data has no body of that name: every actor's loads with the
/// manifest.
#[must_use]
pub fn body<'w>(world: &'w World, actor: &str) -> &'w Body {
    world
        .get_resource::<DataRegistry>()
        .and_then(|data| data.get::<Body>(actor))
        .unwrap_or_else(|| {
            panic!("no `{BODIES}` entry `{actor}`: each actor's loads with the manifest")
        })
}

/// Gives `entity` its actor's collider and hurtbox ([`body`]) over its
/// prefab's, and an aim's origin at its centre: at its spawn, and at a
/// run's start for the map's, so a body moved in its file moves each spawn.
pub fn fit_body(world: &mut World, entity: Entity, actor: &str) {
    let body = body(world, actor);
    let (collider, hurtbox, centre) = (body.collider(), body.hurtbox, body.centre);
    if let Some(own) = world.get_mut::<Collider>(entity) {
        *own = collider;
    }
    if let Some(own) = world.get_mut::<Hurtbox>(entity) {
        *own = hurtbox;
    }
    if let Some(aim) = world.get_mut::<Aim>(entity) {
        *aim = aim.clone().with_offset(centre);
    }
}

/// How far `at` lies from a body of `shape` about `centre`: 0 inside it.
#[must_use]
pub fn surface_distance(at: Vec2, centre: Vec2, shape: Shape) -> f32 {
    let offset = at - centre;
    match shape {
        Shape::Aabb { half_extents } => (offset.abs() - half_extents).max(Vec2::ZERO).length(),
        Shape::Circle { radius } => (offset.length() - radius).max(0.0),
    }
}

/// At most this many bursts count at once; a cosmetic burst past it is
/// dropped, and a key burst takes the oldest cosmetic one's slot.
pub const BURST_CAP: usize = 16;

/// A one-off particle burst at a point, a jump's puff or an impact's
/// sparks, despawned once its particles are gone. Presentation: never
/// saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Burst {
    /// Its place in the run's bursts, which orders the oldest first.
    pub order: u64,
    /// A key burst: a death, gore, a summon, a shatter or a refusal, which
    /// the cap never drops.
    pub key: bool,
    /// It counts against [`BURST_CAP`]; a cosmetic burst that gave its
    /// slot to a key one fades out uncounted.
    pub counted: bool,
}

/// The bursts a run has spawned, which seed the next, so a run's bursts
/// look the same whatever entity IDs the ECS hands out.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bursts(pub u64);

/// Spawns a cosmetic burst of the particle config `name` at `at`, owned by
/// gameplay, unless [`BURST_CAP`] bursts count or the config is unknown. It
/// is spawned at once, so the engine's emit pass of this frame sees it.
pub fn spawn_burst(world: &mut World, name: &str, at: Vec2) -> Option<Entity> {
    if counted_bursts(world) >= BURST_CAP {
        return None;
    }
    burst(world, name, at, false)
}

/// Spawns a key burst of the particle config `name` at `at`: a death,
/// gore, a summon, a shatter or a refusal. At the cap, the oldest cosmetic
/// burst gives up its slot, drained at once and no longer counted, its
/// particles left to fade, and `burst_cleanup` sweeps it as it does a burst;
/// with every slot a key burst's, it is dropped.
pub fn spawn_key_burst(world: &mut World, name: &str, at: Vec2) -> Option<Entity> {
    if counted_bursts(world) >= BURST_CAP {
        let oldest = world
            .query::<(Entity, &Burst)>()
            .filter(|(_, burst)| burst.counted && !burst.key)
            .min_by_key(|(_, burst)| burst.order)
            .map(|(entity, _)| entity)?;
        if let Some(burst) = world.get_mut::<Burst>(oldest) {
            burst.counted = false;
        }
        if let Some(state) = world.get_mut::<ParticleEmitterState>(oldest) {
            state.drained = true;
        }
    }
    burst(world, name, at, true)
}

/// How many bursts count against [`BURST_CAP`].
fn counted_bursts(world: &World) -> usize {
    world
        .query::<&Burst>()
        .filter(|burst| burst.counted)
        .count()
}

/// A burst of `name` at `at`, the run's next.
fn burst(world: &mut World, name: &str, at: Vec2, key: bool) -> Option<Entity> {
    let config = world
        .get_resource::<ParticleConfigRegistry>()?
        .id_for_name(name)?;
    let order = match world.get_resource_mut::<Bursts>() {
        Some(bursts) => {
            bursts.0 = bursts.0.wrapping_add(1);
            bursts.0
        }
        None => 0,
    };
    Some(world.spawn_with((
        Burst {
            order,
            key,
            counted: true,
        },
        Transform::from_position(at),
        ParticleEmitter::with_seed(config, 0x1a_0000 + order),
        ParticleEmitterState::default(),
        SceneEntity { state_id: GAMEPLAY },
    )))
}

/// `burst_cleanup`: despawns each burst whose emitter has ticked, drained
/// and reported it, and that owns no particle.
pub fn burst_cleanup(world: &mut World) {
    let drained: Vec<Entity> = world
        .query::<(Entity, &Burst, &ParticleEmitterState)>()
        .filter(|(_, _, state)| {
            state.first_tick_done
                && state.drained
                && state.drain_reported
                && state.active_count == 0
        })
        .map(|(entity, ..)| entity)
        .collect();
    if drained.is_empty() {
        return;
    }
    // One pass over the particles for every drained burst, not one each
    // (P6).
    let owning: std::collections::HashSet<Entity> = world
        .query::<&Particle>()
        .filter_map(|particle| particle.emitter)
        .filter(|emitter| drained.contains(emitter))
        .collect();
    let finished: Vec<Entity> = drained
        .into_iter()
        .filter(|entity| !owning.contains(entity))
        .collect();
    for entity in finished {
        world.despawn(entity);
    }
}

/// Despawns `entity` and every entity that hangs from it, down the tree,
/// at once: a body that leaves mid-step takes its art and emitters along.
pub fn despawn_tree(world: &mut World, entity: Entity) {
    let mut stack = vec![entity];
    while let Some(entity) = stack.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
        world.despawn(entity);
    }
}

/// Whether `action` is held, through the `ActionMap`, in the input the UI
/// left to the game.
#[must_use]
pub fn pressed(world: &World, action: &str) -> bool {
    let Some(actions) = world.get_resource::<ActionMap>() else {
        return false;
    };
    match world.get_resource::<RoutedInput>() {
        Some(routed) => actions.is_pressed(routed, action),
        None => world
            .get_resource::<InputState>()
            .is_some_and(|input| actions.is_pressed(input, action)),
    }
}

/// Whether `action` was just pressed, through the `ActionMap`, in the input
/// the UI left to the game; in the fixed step, since the last step.
#[must_use]
pub fn just_pressed(world: &World, action: &str) -> bool {
    let Some(actions) = world.get_resource::<ActionMap>() else {
        return false;
    };
    match world.get_resource::<RoutedInput>() {
        Some(routed) => actions.just_pressed(routed, action),
        None => world
            .get_resource::<InputState>()
            .is_some_and(|input| actions.just_pressed(input, action)),
    }
}
