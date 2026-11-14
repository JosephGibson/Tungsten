//! The carry (redo 7.10g, S-37): a body held at its carrier's point plus
//! an offset, or centre over centre (7.13c), its own `RigidBody` a sensor
//! until a release lets it go.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{BodyKind, Position, PrevPosition, RigidBody, Velocity, wake};
use tungsten_core::save::{RefCodec, RefReader, RefWriter, SaveOptions};
use tungsten_core::{Entity, World};

use crate::foundation::saved::saving;
use crate::foundation::tags::{TagFilter, Tags};
use crate::foundation::vec2;
use crate::motion::drive::{centre_of, point_of};
use crate::motion::platformer::PlatformerController;
use crate::overlap::grid::takes_part;
use crate::present::facing::Facing;

/// The name a save holds [`Carried`] under, and its carrier's key.
pub(crate) const CARRIED: &str = "carried";

/// How a carrier holds a body ([`carry`]): an offset from the carrier's
/// point, or from its centre to the body's ([`centred`](Self::centred)),
/// written for a carrier facing right, and the carrier's and the held
/// body's tags that let go.
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
    centred: bool,
    drop: Option<TagFilter>,
    held_drop: Option<TagFilter>,
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
        Self {
            offset,
            ..Self::default()
        }
    }

    /// The hold, its offset run from the carrier's centre to the held
    /// body's, each [`centre_of`](crate::centre_of)'s (a collider's centre,
    /// else the drawn centre, else the point), its x still mirrored while the
    /// carrier faces left: a load over a carrier's middle stays over it
    /// facing either way, where an offset from a frame's corner mirrors about
    /// that corner.
    ///
    /// # Examples
    ///
    /// ```
    /// use glam::Vec2;
    /// use tungsten_core::physics::{Collider, Position, RigidBodyBundle};
    /// use tungsten_core::World;
    /// use tungsten_kit::{Hold, carry, centre_of};
    ///
    /// let mut world = World::new();
    /// // A troll whose body's centre is 40 px right of its corner.
    /// let body = Collider::aabb(Vec2::new(30.0, 40.0)).with_offset(Vec2::new(40.0, 40.0));
    /// let troll = world.spawn_with((Position(Vec2::new(100.0, 0.0)), body));
    /// let iron = world.spawn_with(RigidBodyBundle::dynamic(
    ///     Position(Vec2::ZERO),
    ///     Collider::aabb(Vec2::splat(8.0)).with_offset(Vec2::splat(8.0)),
    /// ));
    /// assert!(carry(&mut world, troll, iron, Hold::new(Vec2::new(0.0, -48.0)).centred()));
    /// assert_eq!(centre_of(&world, iron), Some(Vec2::new(140.0, -8.0)));
    /// ```
    #[must_use]
    pub fn centred(mut self) -> Self {
        self.centred = true;
        self
    }

    /// The hold, let go at rest once the carrier's [`Tags`] pass `filter`:
    /// a frozen or stunned carrier drops its load. An empty filter passes
    /// every carrier, so it lets go at the next step.
    #[must_use]
    pub fn with_drop(mut self, filter: TagFilter) -> Self {
        self.drop = Some(filter);
        self
    }

    /// The hold, let go at rest once the held body's [`Tags`] pass
    /// `filter`, checked beside the carrier's [`with_drop`](Self::with_drop)
    /// before each placement: a held foe that thaws slips out.
    #[must_use]
    pub fn with_held_drop(mut self, filter: TagFilter) -> Self {
        self.held_drop = Some(filter);
        self
    }

    /// Its offset from the carrier's point, or from its centre to the held
    /// body's when [`centred`](Self::centred), for a carrier facing right.
    #[must_use]
    pub fn offset(&self) -> Vec2 {
        self.offset
    }

    /// Whether its offset runs centre to centre.
    #[must_use]
    pub fn is_centred(&self) -> bool {
        self.centred
    }

    /// The offset for a carrier facing `facing`: its x negated while the
    /// facing points left.
    fn faced(&self, facing: Option<&Facing>) -> Vec2 {
        match facing {
            Some(facing) if facing.direction().x < 0.0 => Vec2::new(-self.offset.x, self.offset.y),
            _ => self.offset,
        }
    }

    /// Whether `carrier` lets go of `body` by the carrier's tags or the
    /// held body's.
    fn drops(&self, world: &World, carrier: Entity, body: Entity) -> bool {
        let passes = |filter: &Option<TagFilter>, entity: Entity| {
            filter
                .as_ref()
                .is_some_and(|filter| filter.matches(world.get::<Tags>(entity)))
        };
        passes(&self.drop, carrier) || passes(&self.held_drop, body)
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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    centred: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    drop: Option<TagFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    held_drop: Option<TagFilter>,
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
            centred: carried.hold.centred,
            drop: carried.hold.drop.clone(),
            held_drop: carried.hold.held_drop.clone(),
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
                centred: save.centred,
                drop: save.drop,
                held_drop: save.held_drop,
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
/// else its `Transform.position`) plus the hold's offset, or with its
/// centre at the carrier's centre plus the offset for a
/// [`centred`](Hold::centred) hold, mirrored while the carrier's [`Facing`]
/// points left, at the carrier's velocity. Each step
/// [`FollowPlugin`](crate::FollowPlugin)'s system, after its follows,
/// places it again, or lets it go at rest when its carrier is gone,
/// despawning, dead by its [`Health`](crate::Health) or past either of the
/// hold's drop filters, and after a load whose carrier did not come back.
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
    let (at, velocity) = placement(world, carrier, body, &hold);
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

/// The carrier holding `body`, if any: [`carried_by`]'s reverse.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Collider, Position, RigidBodyBundle};
/// use tungsten_core::World;
/// use tungsten_kit::{Hold, carrier_of, carry};
///
/// let mut world = World::new();
/// let troll = world.spawn_with((Position(Vec2::ZERO),));
/// let keg = world.spawn_with(RigidBodyBundle::dynamic(Position(Vec2::ZERO), Collider::circle(6.0)));
/// assert_eq!(carrier_of(&world, keg), None);
/// assert!(carry(&mut world, troll, keg, Hold::new(Vec2::new(0.0, -30.0))));
/// assert_eq!(carrier_of(&world, keg), Some(troll));
/// ```
#[must_use]
pub fn carrier_of(world: &World, body: Entity) -> Option<Entity> {
    world.get::<Carried>(body).and_then(|carried| carried.by)
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

/// Where `hold` puts `body`'s point by `carrier`, and the carrier's
/// velocity, zero without one: the carrier's point plus the faced offset,
/// or, centred, the carrier's centre plus it less the body's own centre's
/// offset from its point.
fn placement(world: &World, carrier: Entity, body: Entity, hold: &Hold) -> (Vec2, Vec2) {
    let faced = hold.faced(world.get::<Facing>(carrier));
    let at = if hold.centred {
        let point = world
            .get::<Position>(body)
            .map_or(Vec2::ZERO, |position| position.0);
        let own = centre_of(world, body).map_or(Vec2::ZERO, |centre| centre - point);
        centre_of(world, carrier).unwrap_or_default() + faced - own
    } else {
        point_of(world, carrier).unwrap_or_default() + faced
    };
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
                        && !carried.hold.drops(world, carrier, body)
                })
                .map(|carrier| {
                    let (at, velocity) = placement(world, carrier, body, &carried.hold);
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
