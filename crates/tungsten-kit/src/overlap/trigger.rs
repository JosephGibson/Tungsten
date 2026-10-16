//! Trigger zones: a sensor carrying one game type, entered by an entity
//! carrying another.

use std::fmt;
use std::marker::PhantomData;

use tungsten_core::physics::SensorEntered;
use tungsten_core::{Entity, EventCursor, EventQueue, Plugin, Schedule, World, system};

use crate::foundation::phases::{Phase, ensure_phases};

/// Sends a [`TriggerEntered<Z, T>`] each time an entity carrying a `T`
/// enters a sensor carrying a `Z`: a gem picked up by the player, a door's
/// zone, a checkpoint. `Z` and `T` are the game's own component types,
/// which its prefab files already name, so a zone needs no kit component;
/// a zone over tags is a [`Zone`](crate::Zone).
///
/// The plugin registers `EventQueue<TriggerEntered<Z, T>>` and its system
/// in the overlap phase under the name the game passes, after
/// `physics_step`. Each step the system reads the step's
/// `SensorEntered` events in order, each once through an [`EventCursor`],
/// and sends one `TriggerEntered` for each
/// whose sensor carries a `Z` and whose other body carries a `T`; a tile
/// never triggers. It reports entering only, as the physics does. Two
/// zones over different types are two plugins with two names.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Collider, PhysicsPlugin, Position, RigidBodyBundle, Velocity};
/// use tungsten_core::{Bundle, EventQueue, Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{TriggerEntered, TriggerZone};
///
/// /// The game's types: a gem's zone and the player.
/// struct Gem;
/// struct Player;
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// PhysicsPlugin.build(&mut schedule, &mut world);
/// TriggerZone::<Gem, Player>::new("gem_pickup").build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let gem = world.spawn_with(
///     RigidBodyBundle::sensor(Position::new(100.0, 0.0), Collider::circle(8.0)).with((Gem,)),
/// );
/// let player = world.spawn_with(
///     RigidBodyBundle::kinematic(Position::new(0.0, 0.0), Collider::aabb(Vec2::splat(8.0)))
///         .with_velocity(Velocity::new(600.0, 0.0))
///         .with((Player,)),
/// );
/// let mut entered = Vec::new();
/// for _ in 0..20 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
///     let queue = world.resource::<EventQueue<TriggerEntered<Gem, Player>>>();
///     entered.extend(queue.iter_current().copied());
///     world.flush_events();
/// }
/// assert_eq!(entered, [TriggerEntered::new(gem, player)]);
/// ```
pub struct TriggerZone<Z, T> {
    name: &'static str,
    marker: PhantomData<fn() -> (Z, T)>,
}

impl<Z: 'static, T: 'static> TriggerZone<Z, T> {
    /// The zone's plugin, its system named `name`.
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }
}

impl<Z, T> Clone for TriggerZone<Z, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Z, T> Copy for TriggerZone<Z, T> {}

impl<Z, T> fmt::Debug for TriggerZone<Z, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TriggerZone")
            .field("name", &self.name)
            .finish()
    }
}

impl<Z: 'static, T: 'static> Plugin for TriggerZone<Z, T> {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<TriggerEntered<Z, T>>();
        ensure_phases(schedule);
        let mut sensors = EventCursor::<SensorEntered>::new();
        let mut entered: Vec<TriggerEntered<Z, T>> = Vec::new();
        schedule.add(
            Phase::Overlap.stage(),
            Phase::Overlap.join(system(self.name, move |world: &mut World| {
                read_zone::<Z, T>(world, &mut sensors, &mut entered);
            })),
        );
    }
}

/// Sends the entries of a `T` into a `Z` sent since the system last ran,
/// after `physics_step` the step's own; `cursor` is the system's place in
/// the sensor queue and `entered` its scratch list.
fn read_zone<Z: 'static, T: 'static>(
    world: &mut World,
    cursor: &mut EventCursor<SensorEntered>,
    entered: &mut Vec<TriggerEntered<Z, T>>,
) {
    let Some(sensors) = world.get_resource::<EventQueue<SensorEntered>>() else {
        return;
    };
    entered.clear();
    entered.extend(cursor.read(sensors).filter_map(|event| {
        let other = event.other?;
        (world.has::<Z>(event.sensor) && world.has::<T>(other))
            .then(|| TriggerEntered::new(event.sensor, other))
    }));
    if entered.is_empty() {
        return;
    }
    let Some(queue) = world.get_resource_mut::<EventQueue<TriggerEntered<Z, T>>>() else {
        return;
    };
    for event in entered.drain(..) {
        queue.send(event);
    }
}

/// An entity carrying a `T` entered a sensor carrying a `Z`, sent by
/// [`TriggerZone<Z, T>`]'s system in the step the overlap began. Build one
/// with [`TriggerEntered::new`].
pub struct TriggerEntered<Z, T> {
    /// The sensor entered.
    pub zone: Entity,
    /// The entity that entered it.
    pub other: Entity,
    marker: PhantomData<fn() -> (Z, T)>,
}

impl<Z, T> TriggerEntered<Z, T> {
    /// An event with these fields, as the zone's system sends one.
    #[must_use]
    pub fn new(zone: Entity, other: Entity) -> Self {
        Self {
            zone,
            other,
            marker: PhantomData,
        }
    }
}

impl<Z, T> Clone for TriggerEntered<Z, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Z, T> Copy for TriggerEntered<Z, T> {}

impl<Z, T> PartialEq for TriggerEntered<Z, T> {
    fn eq(&self, other: &Self) -> bool {
        self.zone == other.zone && self.other == other.other
    }
}

impl<Z, T> Eq for TriggerEntered<Z, T> {}

impl<Z, T> fmt::Debug for TriggerEntered<Z, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TriggerEntered")
            .field("zone", &self.zone)
            .field("other", &self.other)
            .finish()
    }
}

#[cfg(test)]
#[path = "../tests/trigger.rs"]
mod tests;
