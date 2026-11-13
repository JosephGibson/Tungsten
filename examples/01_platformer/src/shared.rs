//! What two features share: the save IDs every saved entity carries, the
//! actors' bodies as their data holds them, a body's centre and the
//! distance to its surface, the short-lived particle bursts, cosmetic and
//! key, and the actions read off the input the UI left to the game.

use std::collections::BTreeMap;

use glam::Vec2;
use serde::Deserialize;
use tungsten::SceneEntity;
use tungsten::core::{
    ActionMap, Collider, DataRegistry, Entity, InputState, Position, RoutedInput, Shape, World,
};

use tungsten::kit::{Aim, Children, Hurtbox};

use crate::game::GAMEPLAY;

/// A saved entity's stable identity is core's [`SaveId`] in a [`Saved`]
/// component: a save names entities by it, never by `Entity`, which the ECS
/// reuses. The map's objects and the player take theirs from their names
/// ([`SaveId::authored`]), at or above [`AUTHORED`], when the run's start
/// names them; anything spawned later takes the run's next ([`take_id`]),
/// below it, from the counter the engine's flush counts its spawns on: a
/// kept identity (`Saved::kept`) that 01's own records save, or a spawn the
/// engine saves whole.
pub use tungsten::core::save::{AUTHORED, SaveId, Saved};

/// Takes the run's next save ID, for a spawn of the run, from the counter
/// the engine's flush takes its spawns' IDs from.
///
/// # Panics
///
/// Without a run, whose start names the player and gives the World its save
/// state, or once the IDs run out.
pub fn take_id(world: &mut World) -> SaveId {
    tungsten::core::save::take_id(world).expect("a run's save state: its start names the player")
}

/// The save ID the run's next spawn takes; 0 without a run.
#[must_use]
pub fn next_id(world: &World) -> u64 {
    world
        .get_resource::<tungsten::core::save::SaveState>()
        .map_or(0, tungsten::core::save::SaveState::next_id)
}

/// `entity`'s save ID, its [`Saved`] identity's; none for an entity no
/// save names.
#[must_use]
pub fn save_id(world: &World, entity: Entity) -> Option<SaveId> {
    world.get::<Saved>(entity).map(Saved::id)
}

/// A spawn of the kit's that 01 took in, a bullet, a fire missile or a
/// rolled orb, or one 01 made in its place: its adopt system's work done,
/// at its spawn or at a load. Rebuilt, not saved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Joined;

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
        Shape::Circle { radius } => (offset.length() - radius).max(0.0),
        other => (offset.abs() - other.half_extents())
            .max(Vec2::ZERO)
            .length(),
    }
}

/// At most this many bursts count at once, the kit's [`Bursts`] budget's
/// cap at boot (`game::register`); a cosmetic burst past it is dropped,
/// and a key burst takes the oldest cosmetic one's slot.
pub const BURST_CAP: usize = 16;

/// A one-off particle burst at a point, a jump's puff or an impact's
/// sparks, despawned once its particles are gone: the kit's, whose `key`
/// marks a death, gore, a summon, a shatter or a refusal, which the cap
/// never drops. Presentation: never saved.
pub use tungsten::kit::ParticleBurst as Burst;

/// The kit's burst budget, whose count of the bursts spawned seeds the
/// next, so a run's bursts look the same whatever entity IDs the ECS hands
/// out: a run starts it again.
pub use tungsten::kit::BurstBudget as Bursts;

/// Spawns a cosmetic burst of the particle config `name` at `at`, owned by
/// gameplay, unless [`BURST_CAP`] bursts count or the config is unknown. It
/// is spawned at once, so the engine's emit pass of this frame sees it
/// ([`tungsten::kit::burst`]).
pub fn spawn_burst(world: &mut World, name: &str, at: Vec2) -> Option<Entity> {
    tungsten::kit::burst_with(world, name, at, false, OWNED)
}

/// Spawns a key burst of the particle config `name` at `at`: a death,
/// gore, a summon, a shatter or a refusal. At the cap, the oldest cosmetic
/// burst gives up its slot, drained at once and no longer counted, its
/// particles left to fade, and the kit's sweep takes it as it does a burst;
/// with every slot a key burst's, it is dropped.
pub fn spawn_key_burst(world: &mut World, name: &str, at: Vec2) -> Option<Entity> {
    tungsten::kit::burst_with(world, name, at, true, OWNED)
}

/// A burst's gameplay ownership, spawned with it in one archetype move.
const OWNED: (SceneEntity,) = (SceneEntity { state_id: GAMEPLAY },);

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
