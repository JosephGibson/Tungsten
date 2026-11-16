//! What two features share: the save IDs every saved entity carries, the
//! actors' bodies as their data holds them, the parts of a loaded prefab
//! that 01 reads where it places one, a body's centre and the
//! distance to its surface, the short-lived particle bursts, cosmetic and
//! key, and the actions read off the input the UI left to the game.

use std::collections::BTreeMap;

use glam::Vec2;
use serde::Deserialize;
use tungsten::SceneEntity;
use tungsten::core::{
    ActionMap, AnimationRegistry, AnimationState, AssetRegistry, Collider, DataRegistry, Entity,
    InputState, Position, PrefabRegistry, RoutedInput, Shape, SpriteAssetId, World,
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

/// The part of `prefab`'s loaded `components` at `path`, key by key
/// (`&["rigid_body", "shape", "radius"]`), read as a `T` straight off the
/// borrowed value: a number's or a pair's read allocates nothing. None
/// when the prefab, the part or a `T` there is missing.
#[must_use]
pub fn prefab_part<'w, T: Deserialize<'w>>(
    world: &'w World,
    prefab: &str,
    path: &[&str],
) -> Option<T> {
    let components = &world
        .get_resource::<PrefabRegistry>()?
        .get(prefab)?
        .components;
    let (first, rest) = path.split_first()?;
    let part = rest
        .iter()
        .try_fold(components.get(*first)?, |part, key| part.get(*key))?;
    T::deserialize(part).ok()
}

/// The number at `path` in `prefab`'s components ([`prefab_part`]).
///
/// # Panics
///
/// When it is missing: each prefab 01 places loads with the manifest.
#[must_use]
pub fn prefab_number(world: &World, prefab: &str, path: &[&str]) -> f32 {
    prefab_part(world, prefab, path).unwrap_or_else(|| {
        panic!(
            "no number `{}` in the prefab `{prefab}`: each prefab 01 places loads with the manifest",
            path.join(".")
        )
    })
}

/// `prefab`'s rigid body as its file sets it ([`prefab_part`]): its half
/// size, a circle's radius as a square's, and its offset, the body's
/// centre from the sprite's top-left, none when the file names none.
///
/// # Panics
///
/// When the prefab has no rigid body with a box or a circle: each prefab
/// 01 places loads with the manifest.
#[must_use]
pub fn prefab_box(world: &World, prefab: &str) -> (Vec2, Vec2) {
    let shape = ["rigid_body", "shape"];
    let radius = prefab_part::<f32>(world, prefab, &[shape[0], shape[1], "radius"]);
    let half = radius.map(Vec2::splat).or_else(|| {
        prefab_part::<[f32; 2]>(world, prefab, &[shape[0], shape[1], "half_extents"])
            .map(Vec2::from)
    });
    let offset = prefab_part::<[f32; 2]>(world, prefab, &["rigid_body", "offset"]);
    let half = half.unwrap_or_else(|| {
        panic!(
            "no box or circle in the prefab `{prefab}`'s rigid body: each prefab 01 places loads with the manifest"
        )
    });
    (half, offset.map_or(Vec2::ZERO, Vec2::from))
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

/// The first frame's sprite of the animation `clip`, as an
/// [`AnimationState`] starting it shows it, from the World's
/// [`AnimationRegistry`]: a one-shot look's sprite before its clip's first
/// tick, so no look names a frame by hand. None when the registry lacks the
/// clip.
#[must_use]
pub fn clip_sprite(world: &World, clip: &str) -> Option<SpriteAssetId> {
    AnimationState::new(clip).current_sprite(world.get_resource::<AnimationRegistry>()?)
}

/// The size of the sprite `name`, else of the first frame of the clip
/// `name`, in its art's pixels, as the World's registry knows it: a loaded
/// sprite's, or the size a headless run noted for it
/// (`Harness::note_sprite_sizes`), so what is laid out, placed or tiled by
/// its art reads the art.
///
/// # Panics
///
/// When the registry knows no size for it: each sprite and clip the game
/// names loads with the manifest.
#[must_use]
pub fn sprite_size(world: &World, name: &str) -> Vec2 {
    world
        .get_resource::<AssetRegistry>()
        .and_then(|assets| {
            let named = assets.sprite_id(name).and_then(|id| assets.sprite_size(id));
            named.or_else(|| {
                let clip = world.get_resource::<AnimationRegistry>()?.get(name)?;
                assets.sprite_size(clip.frames.first()?.sprite)
            })
        })
        .map(|(width, height)| Vec2::new(width as f32, height as f32))
        .unwrap_or_else(|| {
            panic!("no size for the sprite or clip `{name}`: each loads with the manifest")
        })
}

/// A value in `[0, 1)` drawn from `seed`: the top 24 bits of core's
/// `splitmix64` of it, over 2^24. Every hashed look and spread draws from
/// it: a site with two numbers folds them as `(a << 32) | b`, one with
/// several draws from one number takes the draw's index in the top byte
/// (`seed ^ (k << 56)`).
#[must_use]
pub fn unit(seed: u64) -> f32 {
    (tungsten::core::splitmix64(seed) >> 40) as f32 / (1u64 << 24) as f32
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
