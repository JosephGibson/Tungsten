//! The carry (redo 7.10g, S-37): a body held at its carrier's point plus
//! an offset, its own `RigidBody` a sensor until a release lets it go.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{BodyKind, Position, PrevPosition, RigidBody, Velocity, wake};
use tungsten_core::save::{RefCodec, RefReader, RefWriter, SaveOptions};
use tungsten_core::{Entity, World};

use crate::foundation::saved::saving;
use crate::foundation::tags::{TagFilter, Tags};
use crate::foundation::vec2;
use crate::motion::drive::point_of;
use crate::motion::platformer::PlatformerController;
use crate::overlap::grid::takes_part;
use crate::present::facing::Facing;

/// The name a save holds [`Carried`] under, and its carrier's key.
pub(crate) const CARRIED: &str = "carried";

/// How a carrier holds a body ([`carry`]): an offset from the carrier's
/// point, written for a carrier facing right, and the carrier's tags that
/// let go.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_kit::{Hold, TagFilter};
///
/// // Over a troll's head, dropped when it is frozen or stunned.
/// let hold = Hold::new(Vec2::new(4.0, -30.0))
///     .with_drop(TagFilter::new().any(["status.frozen", "status.stunned"]));
/// assert_eq!(hold.offset(), Vec2::new(4.0, -30.0));
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Hold {
    offset: Vec2,
    drop: Option<TagFilter>,
}

impl Hold {
    /// A hold `offset` world pixels from the carrier's point, its x
    /// mirrored while the carrier's [`Facing`] points left.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite.
    #[must_use]
    #[track_caller]
    pub fn new(offset: Vec2) -> Self {
        assert!(
            offset.is_finite(),
            "Hold::new: offset must be finite (got {offset})"
        );
        Self { offset, drop: None }
    }

    /// The hold, let go at rest once the carrier's [`Tags`] pass `filter`:
    /// a frozen or stunned carrier drops its load. An empty filter passes
    /// every carrier, so it lets go at the next step.
    #[must_use]
    pub fn with_drop(mut self, filter: TagFilter) -> Self {
        self.drop = Some(filter);
        self
    }

    /// Its offset from the carrier's point, for a carrier facing right.
    #[must_use]
    pub fn offset(&self) -> Vec2 {
        self.offset
    }

    /// The offset for a carrier facing `facing`: its x negated while the
    /// facing points left.
    fn faced(&self, facing: Option<&Facing>) -> Vec2 {
        match facing {
            Some(facing) if facing.direction().x < 0.0 => Vec2::new(-self.offset.x, self.offset.y),
            _ => self.offset,
        }
    }

    /// Whether `carrier` lets go by its tags.
    fn drops(&self, world: &World, carrier: Entity) -> bool {
        self.drop
            .as_ref()
            .is_some_and(|filter| filter.matches(world.get::<Tags>(carrier)))
    }
}

/// A held body's hold: its carrier, its own `RigidBody` while it is a
/// sensor, and how it is held. Saved as `carried`, its carrier by save ID;
/// no scene component names it.
#[derive(Debug, Clone)]
pub(crate) struct Carried {
    /// Its carrier; `None` after a load whose carrier did not resolve.
    by: Option<Entity>,
    /// Its own body, back at its release; `None` after a load until its
    /// first step takes the fresh spawn's, since a save holds no body.
    body: Option<RigidBody>,
    hold: Hold,
}

/// A [`Carried`] in a save: its hold, its carrier written by the codec.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CarriedSave {
    #[serde(default, skip_serializing_if = "is_zero", with = "vec2")]
    offset: Vec2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    drop: Option<TagFilter>,
}

// The reference is serde's `skip_serializing_if` signature.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(offset: &Vec2) -> bool {
    *offset == Vec2::ZERO
}

impl From<&Carried> for CarriedSave {
    fn from(carried: &Carried) -> Self {
        Self {
            offset: carried.hold.offset,
            drop: carried.hold.drop.clone(),
        }
    }
}

impl From<CarriedSave> for Carried {
    fn from(save: CarriedSave) -> Self {
        Self {
            by: None,
            body: None,
            hold: Hold {
                offset: save.offset,
                drop: save.drop,
            },
        }
    }
}

/// `Carried` saved with its carrier's reference; a carrier that does not
/// resolve leaves it naming none, and its first step drops it.
pub(crate) fn carried_saved() -> SaveOptions<Carried> {
    SaveOptions::<Carried>::form::<CarriedSave>().refs(RefCodec {
        save: |carried, out: &mut RefWriter<'_>| {
            if let Some(by) = carried.by {
                out.put(CARRIED, by);
            }
        },
        load: |carried, refs: &RefReader<'_>| {
            if let Some(by) = refs.first(CARRIED) {
                carried.by = Some(by);
            }
        },
        keys: |_| vec![CARRIED.to_owned()],
    })
}

/// Present while a [`carry`] may hold a body in a World that does not save,
/// so the step's pass runs only then; a saving World's pass runs each step,
/// since a load can bring a hold back.
pub(crate) struct Carrying;

/// Holds `body` at `carrier`: its own `RigidBody` a sensor until a
/// [`release`], placed at once at the carrier's point (its `Position`,
/// else its `Transform.position`) plus the hold's offset, mirrored while
/// the carrier's [`Facing`] points left, at the carrier's velocity. Each
/// step [`FollowPlugin`](crate::FollowPlugin)'s system, after its follows,
/// places it again, or lets it go at rest when its carrier is gone,
/// despawning, dead by its [`Health`](crate::Health) or past the hold's
/// drop filter, and after a load whose carrier did not come back.
///
/// Whether it held: it refuses the carrier itself, a carrier without a
/// point, a body without a `RigidBody` and a `Position`, a body already
/// held, a carrier already carrying, a carrier that is itself held and a
/// body that carries, so no hold is a chain or a cycle. A game moves a held
/// body by its carrier and lets it go with [`release`], a throw onto a
/// point at [`lob_velocity`](crate::lob_velocity)'s launch; a held
/// [`PlatformerController`]'s system skips its sensor body, with its one
/// warning, until the release launches it.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{BodyKind, Collider, Position, RigidBody, RigidBodyBundle};
/// use tungsten_core::World;
/// use tungsten_kit::{Hold, carried_by, carry, release};
///
/// let mut world = World::new();
/// let troll = world.spawn_with((Position(Vec2::new(100.0, 50.0)),));
/// let barrel = world.spawn_with(RigidBodyBundle::dynamic(
///     Position(Vec2::ZERO),
///     Collider::circle(6.0),
/// ));
/// assert!(carry(&mut world, troll, barrel, Hold::new(Vec2::new(0.0, -30.0))));
/// assert_eq!(world.get::<Position>(barrel).unwrap().0, Vec2::new(100.0, 20.0));
/// assert_eq!(world.get::<RigidBody>(barrel).unwrap().kind, BodyKind::Sensor);
/// assert_eq!(carried_by(&world, troll), Some(barrel));
///
/// assert_eq!(release(&mut world, troll, Vec2::new(200.0, -100.0)), Some(barrel));
/// assert_eq!(world.get::<RigidBody>(barrel).unwrap().kind, BodyKind::Dynamic);
/// assert_eq!(carried_by(&world, troll), None);
/// ```
pub fn carry(world: &mut World, carrier: Entity, body: Entity, hold: Hold) -> bool {
    let refused = carrier == body
        || point_of(world, carrier).is_none()
        || !world.has::<Position>(body)
        || world.has::<Carried>(body)
        || world.has::<Carried>(carrier)
        || carried_by(world, carrier).is_some()
        || carried_by(world, body).is_some();
    let Some(own) = world.get::<RigidBody>(body).copied().filter(|_| !refused) else {
        return false;
    };
    if let Some(rigid) = world.get_mut::<RigidBody>(body) {
        *rigid = RigidBody::sensor();
    }
    let (at, velocity) = placement(world, carrier, &hold);
    world.insert(
        body,
        Carried {
            by: Some(carrier),
            body: Some(own),
            hold,
        },
    );
    place(world, body, at, velocity);
    if let Some(prev) = world.get_mut::<PrevPosition>(body) {
        prev.0 = at;
    }
    if !world.has_resource::<Carrying>() {
        world.insert_resource(Carrying);
    }
    true
}

/// The body `carrier` holds, if any.
#[must_use]
pub fn carried_by(world: &World, carrier: Entity) -> Option<Entity> {
    world
        .query::<(Entity, &Carried)>()
        .find(|(_, carried)| carried.by == Some(carrier))
        .map(|(body, _)| body)
}

/// Lets go of the body `carrier` holds at `velocity`, a zero velocity a
/// drop: its own `RigidBody` back, its `Velocity` set, a held
/// [`PlatformerController`] launched with it and a dynamic body woken.
/// The body it let go; `None` when `carrier` holds nothing.
///
/// # Panics
///
/// When `velocity` is not finite.
#[track_caller]
pub fn release(world: &mut World, carrier: Entity, velocity: Vec2) -> Option<Entity> {
    assert!(
        velocity.is_finite(),
        "release: velocity must be finite (got {velocity})"
    );
    let body = carried_by(world, carrier)?;
    let_go(world, body, velocity);
    Some(body)
}

/// Where a body `hold` holds sits by `carrier`, and the carrier's
/// velocity, zero without one.
fn placement(world: &World, carrier: Entity, hold: &Hold) -> (Vec2, Vec2) {
    let at =
        point_of(world, carrier).unwrap_or_default() + hold.faced(world.get::<Facing>(carrier));
    let velocity = world
        .get::<Velocity>(carrier)
        .map_or(Vec2::ZERO, |velocity| velocity.0);
    (at, velocity)
}

/// Puts `body` at `at`, moving at `velocity`.
fn place(world: &mut World, body: Entity, at: Vec2, velocity: Vec2) {
    if let Some(position) = world.get_mut::<Position>(body) {
        position.0 = at;
    }
    if let Some(own) = world.get_mut::<Velocity>(body) {
        own.0 = velocity;
    }
}

/// Ends `body`'s hold at `velocity`, its own `RigidBody` back.
fn let_go(world: &mut World, body: Entity, velocity: Vec2) {
    let Some(carried) = world.remove_component::<Carried>(body) else {
        return;
    };
    let own = carried
        .body
        .or_else(|| world.get::<RigidBody>(body).copied());
    if let (Some(own), Some(rigid)) = (own, world.get_mut::<RigidBody>(body)) {
        *rigid = own;
    }
    if let Some(own) = world.get_mut::<Velocity>(body) {
        own.0 = velocity;
    }
    if let Some(controller) = world.get_mut::<PlatformerController>(body) {
        controller.launch(velocity);
    }
    if own.is_some_and(|own| own.kind == BodyKind::Dynamic) {
        wake(world, body);
    }
}

/// Places every held body at its carrier, or lets it go at rest: run by
/// the follow system after its follows, while a [`Carrying`] is present or
/// the World saves, so a World that never carries runs no query.
pub(crate) fn hold_bodies(world: &mut World, held: &mut Vec<Entity>) {
    let saves = saving(world);
    if !saves && !world.has_resource::<Carrying>() {
        return;
    }
    held.clear();
    held.extend(world.query::<(Entity, &Carried)>().map(|(body, _)| body));
    if held.is_empty() && !saves {
        world.remove_resource::<Carrying>();
        return;
    }
    for &body in held.iter() {
        let kept = {
            let carried = world.get::<Carried>(body).unwrap();
            carried
                .by
                .filter(|&carrier| {
                    takes_part(world, carrier)
                        && point_of(world, carrier).is_some()
                        && !carried.hold.drops(world, carrier)
                })
                .map(|carrier| {
                    let (at, velocity) = placement(world, carrier, &carried.hold);
                    (at, velocity, carried.body.is_none())
                })
        };
        let Some((at, velocity, loaded)) = kept else {
            let_go(world, body, Vec2::ZERO);
            continue;
        };
        if loaded {
            // A loaded hold: its fresh spawn's body is its own.
            let own = world.get::<RigidBody>(body).copied();
            if let Some(rigid) = world.get_mut::<RigidBody>(body) {
                *rigid = RigidBody::sensor();
            }
            if let Some(carried) = world.get_mut::<Carried>(body) {
                carried.body = own;
            }
        }
        place(world, body, at, velocity);
    }
}

#[cfg(test)]
#[path = "../tests/carry.rs"]
mod tests;
