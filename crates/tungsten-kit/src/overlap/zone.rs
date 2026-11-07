//! Zones: a sensor entered by entities whose tags pass its filter.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::camera;
use tungsten_core::physics::{Position, SensorEntered, SensorExited};
use tungsten_core::{
    CameraRig, Entity, EventCursor, EventQueue, Inspectable, Plugin, RigKind, Schedule,
    SpawnContext, Transform, World, system,
};

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, PLATFORMER, RPG, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::tags::{TagFilter, TagFilterDef, Tags, check_declared};
use crate::foundation::trace::{TraceKind, trace};
use crate::present::camera::CameraZone;

/// [`ZonePlugin`]'s system, in the overlap phase.
pub const ZONE_TICK: &str = "zone_tick";

/// The scene component the plugin registers.
const ZONE: &str = "zone";

/// A zone on a sensor body: a door, a trap, a checkpoint. [`ZonePlugin`]'s
/// system sends a [`ZoneEntered`] in the step an entity whose [`Tags`]
/// pass the zone's [`TagFilter`] begins to overlap the sensor; a tile
/// never enters, nor does a bodiless entity, which the physics never sees
/// (a recorded limit). It reports entering only, as the physics does.
///
/// Scene name `zone`, on a prefab with a `rigid_body` of kind `sensor`:
/// `"zone": { "filter": { "all": ["team.player"] } }`; `filter` defaults to
/// everything, and the load checks its tags against the declared ones
/// ([`TagsPlugin`](crate::TagsPlugin)).
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Collider, PhysicsPlugin, Position, RigidBodyBundle, Velocity};
/// use tungsten_core::{Bundle, EventQueue, Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{TagFilter, Tags, Zone, ZoneEntered, ZonePlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// PhysicsPlugin.build(&mut schedule, &mut world);
/// ZonePlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let door = world.spawn_with(
///     RigidBodyBundle::sensor(Position::new(100.0, 0.0), Collider::aabb(Vec2::splat(8.0)))
///         .with((Zone::new(TagFilter::new().all(["team.player"])),)),
/// );
/// let runner = |world: &mut World, y: f32, tags: Tags| {
///     world.spawn_with(
///         RigidBodyBundle::kinematic(Position::new(0.0, y), Collider::aabb(Vec2::splat(4.0)))
///             .with_velocity(Velocity::new(600.0, 0.0))
///             .with((tags,)),
///     )
/// };
/// let hero = runner(&mut world, 0.0, Tags::new(["team.player"]));
/// let slime = runner(&mut world, 2.0, Tags::new(["team.enemy"]));
/// let mut entered = Vec::new();
/// for _ in 0..20 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
///     entered.extend(world.resource::<EventQueue<ZoneEntered>>().iter_current().copied());
///     world.flush_events();
/// }
/// assert_eq!(entered.len(), 1, "the hero, not the slime");
/// assert_eq!((entered[0].zone, entered[0].entity), (door, hero));
/// assert!(entered[0].position.x > 80.0);
/// let _ = slime;
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    filter: TagFilter,
}

impl Zone {
    /// A zone that the entities passing `filter` enter.
    #[must_use]
    pub fn new(filter: TagFilter) -> Self {
        Self { filter }
    }

    /// What enters it.
    #[must_use]
    pub fn filter(&self) -> &TagFilter {
        &self.filter
    }
}

/// An entity whose [`Tags`] pass a [`Zone`]'s filter began to overlap it
/// this step, sent by [`ZonePlugin`]'s system. Build one with
/// [`ZoneEntered::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ZoneEntered {
    /// The zone's entity.
    pub zone: Entity,
    /// The entity that entered it.
    pub entity: Entity,
    /// Where the entity was at the step's end: its `Position`.
    pub position: Vec2,
}

impl ZoneEntered {
    /// An event with these fields, as the zone's system sends one.
    #[must_use]
    pub fn new(zone: Entity, entity: Entity, position: Vec2) -> Self {
        Self {
            zone,
            entity,
            position,
        }
    }
}

/// `components.zone` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoneDef {
    #[serde(default)]
    filter: TagFilterDef,
}

/// Registers `EventQueue<ZoneEntered>`, the `zone` scene component and
/// [`ZONE_TICK`] in the overlap phase: after `physics_step`, it reads the
/// step's `SensorEntered` events, each once through an [`EventCursor`], and
/// sends a [`ZoneEntered`] for each whose sensor carries a [`Zone`] and
/// whose other body's [`Tags`] pass the zone's filter. While the World
/// holds a [`CameraZone`], it also reads the `SensorExited` events through
/// a cursor of its own, keeps each camera zone's list of what is inside and
/// raises or lowers its rig.
#[derive(Debug, Default, Clone, Copy)]
pub struct ZonePlugin;

impl Plugin for ZonePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Zone>(world, "Zone");
        describe(
            world,
            ComponentDescription::new(
                ZONE,
                "A sensor entered by the entities whose tags pass its filter.",
            )
            .field(
                FieldDescription::new(
                    "filter",
                    FieldType::Filter,
                    "what enters; the load checks its tags",
                )
                .default("{}"),
            )
            .event("ZoneEntered")
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(RPG)
            .sample("{}")
            .roundtrip(round_trip::<ZoneDef>),
        );
        world.register_event::<ZoneEntered>();
        world.register_component_with_assets(
            ZONE,
            |def: &ZoneDef, world: &World| {
                for (field, name) in def.filter.names() {
                    check_declared(world, &format!("filter.{field}"), name)?;
                }
                Ok(())
            },
            |def: ZoneDef, _: &SpawnContext<'_>| (Zone::new(def.filter.into()),),
        );
        let mut sensors = EventCursor::<SensorEntered>::new();
        let mut exits = EventCursor::<SensorExited>::new();
        let mut entered: Vec<ZoneEntered> = Vec::new();
        let mut left: Vec<(Entity, Entity)> = Vec::new();
        schedule.add(
            Phase::Overlap.stage(),
            Phase::Overlap.join(system(ZONE_TICK, move |world: &mut World| {
                zone_tick(world, &mut sensors, &mut entered);
                if world.has_any::<CameraZone>() {
                    camera_zones(world, &mut exits, &entered, &mut left);
                }
                send_entries(world, &mut entered);
            })),
        );
    }
}

/// Lists the entries since the system last ran in `entered`; `cursor` is
/// its place in the sensor queue.
fn zone_tick(
    world: &mut World,
    cursor: &mut EventCursor<SensorEntered>,
    entered: &mut Vec<ZoneEntered>,
) {
    entered.clear();
    let Some(sensors) = world.get_resource::<EventQueue<SensorEntered>>() else {
        return;
    };
    entered.extend(cursor.read(sensors).filter_map(|event| {
        let other = event.other?;
        let zone = world.get::<Zone>(event.sensor)?;
        zone.filter
            .matches(world.get::<Tags>(other))
            .then(|| ZoneEntered::new(event.sensor, other, point_of(world, other)))
    }));
}

/// The camera zones' lists: each entry joins its zone's, raising the rig
/// when the zone was empty; then each exit since the last step, read
/// through `exits` into the scratch list `left`, leaves it, lowering the
/// rig when it empties. A follow rig follows the first inside.
fn camera_zones(
    world: &mut World,
    exits: &mut EventCursor<SensorExited>,
    entered: &[ZoneEntered],
    left: &mut Vec<(Entity, Entity)>,
) {
    left.clear();
    if let Some(queue) = world.get_resource::<EventQueue<SensorExited>>() {
        left.extend(
            exits
                .read(queue)
                .filter_map(|exit| Some((exit.sensor, exit.other?)))
                .filter(|&(sensor, _)| world.has::<CameraZone>(sensor)),
        );
    }
    for event in entered {
        let Some(was_empty) = world
            .get_mut::<CameraZone>(event.zone)
            .and_then(|zone| zone.enter(event.entity))
        else {
            continue;
        };
        follow_first(world, event.zone);
        if was_empty {
            camera::raise(world, event.zone);
        }
    }
    for &(sensor, other) in left.iter() {
        let Some(empty) = world
            .get_mut::<CameraZone>(sensor)
            .and_then(|zone| zone.leave(other))
        else {
            continue;
        };
        follow_first(world, sensor);
        if empty {
            camera::lower(world, sensor);
        }
    }
}

/// Points a camera zone's follow rig at the first entity inside it.
fn follow_first(world: &mut World, zone: Entity) {
    let first = world
        .get::<CameraZone>(zone)
        .and_then(|camera| camera.inside().first().copied());
    if let Some(rig) = world.get_mut::<CameraRig>(zone)
        && let RigKind::Follow(follow) = &mut rig.kind
    {
        follow.target = first;
    }
}

/// Traces and sends the step's entries, draining `entered`.
fn send_entries(world: &mut World, entered: &mut Vec<ZoneEntered>) {
    if entered.is_empty() {
        return;
    }
    for event in entered.iter() {
        trace(world, event.zone, || TraceKind::Event {
            name: "zone_entered",
            other: Some(event.entity),
            amount: None,
        });
    }
    let Some(queue) = world.get_resource_mut::<EventQueue<ZoneEntered>>() else {
        return;
    };
    for event in entered.drain(..) {
        queue.send(event);
    }
}

/// Where `entity` is: its `Position`, else its `Transform.position`.
fn point_of(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).map_or_else(
        || {
            world
                .get::<Transform>(entity)
                .map_or(Vec2::ZERO, |transform| transform.position)
        },
        |position| position.0,
    )
}

impl Inspectable for Zone {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("filter", self.filter.summary())]
    }
}

#[cfg(test)]
#[path = "../tests/zone.rs"]
mod tests;
