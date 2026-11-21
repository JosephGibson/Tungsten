//! Pickups: an entity collected once by the first entity whose hurtbox it
//! meets and whose tags pass its filter, drawn toward the nearest collector
//! by a magnet, or falling onto the ground.

use std::collections::HashSet;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{
    BodyKind, Position, PrevPosition, RigidBody, SpatialFilter, cast_shape_where,
};
use tungsten_core::{
    Entity, EventQueue, Inspectable, Plugin, Schedule, SpawnContext, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, RPG, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::layers::{LayerRef, check_layers, layer_bits, layer_list};
use crate::foundation::non_negative;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::saved::order_of;
use crate::foundation::stats::Stats;
use crate::foundation::tags::{TagFilter, TagFilterDef, Tags};
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::ended::{Despawning, end, ensure_kit_despawn};
use crate::motion::drive::{Body, Write, body_of, point_of, step_position, write_velocity};
use crate::motion::oscillator::OSCILLATOR;
use crate::motion::slide::{SKIN, step_dt};
use crate::motion::speed_limit::SPEED_LIMIT;
use crate::overlap::grid::{HitGrid, first_contact, sweep_of, takes_part};
use crate::overlap::hit::{
    HitShape, Hurtbox, after_grid, check_names, checked_offset, describe_hit_shape, ensure_grid,
    offset, pickup_shape_fields,
};

/// [`PickupPlugin`]'s system that collects pickups, in the overlap phase.
pub const PICKUP_TICK: &str = "pickup_tick";

/// [`PickupPlugin`]'s system that moves pickups toward their collectors, in
/// the move phase.
pub const PICKUP_MAGNET: &str = "pickup_magnet";

/// The scene component the plugin registers.
const PICKUP: &str = "pickup";

/// A coin, a gem, a potion: an entity collected once by the first entity
/// whose [`Hurtbox`](crate::Hurtbox) its shape meets over the step's
/// motion, whose [`Tags`](crate::Tags) pass its filter. Collecting it marks
/// it collected, sends [`PickedUp`], the cue event `picked_up` with the
/// collector as the other entity, and the kit's end-of-step despawn takes
/// it after its cues: a `counter` with `target: other` counts it on the
/// collector.
///
/// With a magnet speed, [`PickupPlugin`]'s magnet moves it each step toward
/// the nearest collector whose `pickup_radius` stat reaches it, at that
/// speed and never past the collector; ties go to the lower entity index,
/// and a collector that is dead or being despawned draws nothing. The
/// magnet adds to a bodiless or static pickup's `Position` after the
/// oscillator, so a bobbing gem drifts as it bobs. A kinematic pickup it
/// pulls is the magnet's alone that step: its `Velocity` is set to the
/// pull, held to the distance over the step, replacing another driver's,
/// so it never passes the collector. A dynamic pickup's `Velocity` is set
/// so at each step it is pulled, and the physics carries it between;
/// gravity acts within the step, so it passes the collector along the pull
/// by at most `g · gravity_scale · dt²`. A child is skipped, with one
/// warning.
///
/// With a [fall](Self::with_fall), a bodiless pickup no magnet pulls falls
/// at that speed onto the first tile or static body under its shape in its
/// [fall layers](Self::with_fall_layers), a cast that passes every other
/// body, stopping the skin the controllers keep short of it, so its next
/// cast starts clear: a coin dropped in the air lands on the floor.
///
/// Scene name `pickup`:
/// `{ "shape": { "circle": 6 }, "filter": { "all": ["team.player"] }, "magnet": 240, "fall": 60, "fall_layers": [0, "ground"] }`,
/// `offset` `[0, 0]`, `filter` everything, `magnet` and `fall` 0, none, and
/// `fall_layers` every layer by default, its entries layer numbers or the
/// manifest's `physics_layers` names, checked at load;
/// it spawns a `Position` and a `PrevPosition` at the spawn point, so a
/// prefab without a `rigid_body` moves bodiless. It is data: a game's save
/// can hold it.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{HitShape, Pickup, TagFilter};
///
/// let gem = Pickup::new(HitShape::circle(6.0))
///     .with_filter(TagFilter::new().all(["team.player"]))
///     .with_magnet(240.0);
/// assert_eq!(gem.magnet(), 240.0);
/// assert!(!gem.is_collected());
/// let saved = serde_json::to_value(&gem).unwrap();
/// assert_eq!(serde_json::from_value::<Pickup>(saved).unwrap(), gem);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PickupFields", into = "PickupFields")]
pub struct Pickup {
    shape: HitShape,
    offset: Vec2,
    filter: TagFilter,
    magnet: f32,
    /// World pixels per second it falls at while no magnet pulls it; 0
    /// falls not at all.
    fall: f32,
    /// The physics layers it lands on, one bit per layer.
    fall_layers: u32,
    collected: bool,
}

/// A pickup as a save writes it, before its values are checked.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PickupFields {
    shape: HitShape,
    #[serde(default, with = "offset")]
    offset: Vec2,
    #[serde(default)]
    filter: TagFilter,
    #[serde(default)]
    magnet: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    fall: f32,
    #[serde(default = "every_layer", skip_serializing_if = "is_every_layer")]
    fall_layers: u32,
    #[serde(default)]
    collected: bool,
}

// The references are serde's `skip_serializing_if` signature.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

fn every_layer() -> u32 {
    RigidBody::ALL_LAYERS
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_every_layer(layers: &u32) -> bool {
    *layers == RigidBody::ALL_LAYERS
}

impl TryFrom<PickupFields> for Pickup {
    type Error = String;

    fn try_from(fields: PickupFields) -> Result<Self, Self::Error> {
        for (field, value) in [("magnet", fields.magnet), ("fall", fields.fall)] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            shape: fields.shape,
            offset: fields.offset,
            filter: fields.filter,
            magnet: fields.magnet,
            fall: fields.fall,
            fall_layers: fields.fall_layers,
            collected: fields.collected,
        })
    }
}

impl From<Pickup> for PickupFields {
    fn from(pickup: Pickup) -> Self {
        Self {
            shape: pickup.shape,
            offset: pickup.offset,
            filter: pickup.filter,
            magnet: pickup.magnet,
            fall: pickup.fall,
            fall_layers: pickup.fall_layers,
            collected: pickup.collected,
        }
    }
}

impl Pickup {
    /// A pickup of `shape` at its entity's point, collected by anything,
    /// with no magnet.
    #[must_use]
    pub fn new(shape: HitShape) -> Self {
        Self {
            shape,
            offset: Vec2::ZERO,
            filter: TagFilter::new(),
            magnet: 0.0,
            fall: 0.0,
            fall_layers: RigidBody::ALL_LAYERS,
            collected: false,
        }
    }

    /// The pickup, its shape at `offset` world pixels from its entity's
    /// point.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite and within ±16384 on each axis.
    #[must_use]
    #[track_caller]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = checked_offset("Pickup::with_offset", offset);
        self
    }

    /// The pickup, collected only by entities whose [`Tags`](crate::Tags)
    /// pass `filter`.
    #[must_use]
    pub fn with_filter(mut self, filter: TagFilter) -> Self {
        self.filter = filter;
        self
    }

    /// The pickup, drawn toward the nearest collector whose
    /// `pickup_radius` stat reaches it at `speed` world pixels per second;
    /// 0 draws it nowhere.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_magnet(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "Pickup::with_magnet: speed must be finite and >= 0 (got {speed})"
        );
        self.magnet = speed;
        self
    }

    /// The pickup, falling at `speed` world pixels per second, while no
    /// magnet pulls it, onto the first tile or static body under it in its
    /// [fall layers](Self::with_fall_layers): bodiless, it lands on the
    /// ground. 0 falls not at all.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_fall(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "Pickup::with_fall: speed must be finite and >= 0 (got {speed})"
        );
        self.fall = speed;
        self
    }

    /// The pickup, landing on the tiles and static bodies in the physics
    /// layers `layers` holds, one bit per layer; every layer by default.
    #[must_use]
    pub fn with_fall_layers(mut self, layers: u32) -> Self {
        self.fall_layers = layers;
        self
    }

    /// Its shape.
    #[must_use]
    pub fn shape(&self) -> HitShape {
        self.shape
    }

    /// Its shape's offset from its entity's point.
    #[must_use]
    pub fn offset(&self) -> Vec2 {
        self.offset
    }

    /// What collects it.
    #[must_use]
    pub fn filter(&self) -> &TagFilter {
        &self.filter
    }

    /// Its magnet's speed, in world pixels per second; 0 for none.
    #[must_use]
    pub fn magnet(&self) -> f32 {
        self.magnet
    }

    /// The speed it falls at, in world pixels per second; 0 for none.
    #[must_use]
    pub fn fall(&self) -> f32 {
        self.fall
    }

    /// The physics layers it lands on, one bit per layer.
    #[must_use]
    pub fn fall_layers(&self) -> u32 {
        self.fall_layers
    }

    /// Whether a collector has taken it: it is collected no more, and the
    /// kit despawns it.
    #[must_use]
    pub fn is_collected(&self) -> bool {
        self.collected
    }
}

/// A pickup was collected: the cue event `picked_up`, whose subject is the
/// pickup and whose other is the collector. Build one with
/// [`PickedUp::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PickedUp {
    /// The pickup.
    pub entity: Entity,
    /// The entity that collected it.
    pub by: Entity,
    /// The pickup's centre where it was collected.
    pub position: Vec2,
}

impl PickedUp {
    /// An event with these fields, as the pickup system sends one.
    #[must_use]
    pub fn new(entity: Entity, by: Entity, position: Vec2) -> Self {
        Self {
            entity,
            by,
            position,
        }
    }
}

/// `components.pickup` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PickupDef {
    shape: HitShape,
    #[serde(default, with = "offset")]
    offset: Vec2,
    #[serde(default)]
    filter: TagFilterDef,
    #[serde(default, deserialize_with = "non_negative")]
    magnet: f32,
    #[serde(default, deserialize_with = "non_negative")]
    fall: f32,
    #[serde(default)]
    fall_layers: Option<Vec<LayerRef>>,
}

/// Registers `EventQueue<PickedUp>`, the `pickup` scene component,
/// [`PICKUP_MAGNET`] in the move phase, after the oscillator and before the
/// speed limit when each is registered, and in the overlap phase the hit
/// grid when the schedule lacks it and then [`PICKUP_TICK`], before
/// `despawn_outside`, `wrap` and `stay_in_view` when each is registered;
/// and the kit's end-of-step despawn when the schedule lacks it.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, World};
/// use tungsten_kit::{PICKUP_MAGNET, PICKUP_TICK, PickupPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// PickupPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
/// let at = |name: &str| order.iter().position(|n| *n == name).unwrap();
/// assert!(at(PICKUP_MAGNET) < at("kit_overlap") && at("kit_overlap") < at(PICKUP_TICK));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct PickupPlugin;

impl Plugin for PickupPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Pickup>(world, "Pickup");
        world.register_saved::<Pickup>(PICKUP);
        describe_hit_shape(world);
        describe(
            world,
            pickup_shape_fields(ComponentDescription::new(
                PICKUP,
                "Collected once by the first entity whose hurtbox it meets and whose tags pass its filter, drawn in by a magnet.",
            ))
            .field(
                FieldDescription::new(
                    "filter",
                    FieldType::Filter,
                    "what collects it, by the collector's tags; the load checks its tags",
                )
                .default("{}"),
            )
            .field(
                FieldDescription::new(
                    "magnet",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels per second toward the nearest collector whose `pickup_radius` stat reaches it; 0 none",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "fall",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels per second a bodiless pickup falls at, while no magnet pulls it, onto the first tile or static body under it; 0 none",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "fall_layers",
                    layer_list(),
                    "the physics layers whose tiles and static bodies it lands on; every layer when left out",
                )
                .default("null"),
            )
            .event("PickedUp")
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(RPG)
            .sample(r#"{ "shape": { "circle": 6 } }"#)
            .roundtrip(round_trip::<PickupDef>),
        );
        world.register_event::<PickedUp>();
        world.register_component_with_assets(
            PICKUP,
            |def: &PickupDef, world: &World| {
                check_names(world, None, &def.filter)?;
                check_layers(
                    "fall_layers",
                    def.fall_layers.as_deref().unwrap_or_default(),
                    world,
                )
            },
            |def: PickupDef, at: &SpawnContext<'_>| {
                let spawn = at.transform().position;
                let mut pickup = Pickup::new(def.shape)
                    .with_offset(def.offset)
                    .with_filter(def.filter.into())
                    .with_magnet(def.magnet)
                    .with_fall(def.fall);
                if let Some(layers) = &def.fall_layers {
                    pickup = pickup.with_fall_layers(layer_bits(layers, at.world()));
                }
                (pickup, Position(spawn), PrevPosition(spawn))
            },
        );
        let mut magnet = Magnet::default();
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(PICKUP_MAGNET, move |world: &mut World| {
                    magnet.run(world);
                }))
                .after_if_present(OSCILLATOR)
                .before_if_present(SPEED_LIMIT),
        );
        ensure_grid(schedule);
        let mut collector = Collector::default();
        schedule.add(
            Phase::Overlap.stage(),
            after_grid(system(PICKUP_TICK, move |world: &mut World| {
                collector.run(world);
            })),
        );
        ensure_kit_despawn(schedule, world);
    }
}

/// [`PICKUP_TICK`]'s state between steps: the step's pickups and a query's
/// candidates, kept so a step allocates nothing once warm.
#[derive(Default)]
struct Collector {
    pickups: Vec<Entity>,
    candidates: Vec<u32>,
}

impl Collector {
    /// Each uncollected pickup, in entity order, goes to the first hurtbox
    /// its sweep meets whose entity's tags pass its filter.
    fn run(&mut self, world: &mut World) {
        let Some(grid) = world.remove_resource::<HitGrid>() else {
            return;
        };
        self.pickups.clear();
        self.pickups.extend(
            world
                .query::<(Entity, &Pickup)>()
                .filter(|(_, pickup)| !pickup.collected)
                .map(|(entity, _)| entity),
        );
        let order = order_of(world);
        self.pickups.sort_by_key(|&entity| order.key(entity));
        for i in 0..self.pickups.len() {
            let entity = self.pickups[i];
            if world.has::<Despawning>(entity) {
                continue;
            }
            let Some((by, at)) = self.first(world, &grid, entity) else {
                continue;
            };
            world.get_mut::<Pickup>(entity).unwrap().collected = true;
            if let Some(queue) = world.get_resource_mut::<EventQueue<PickedUp>>() {
                queue.send(PickedUp::new(entity, by, at));
            }
            trace(world, entity, || TraceKind::Event {
                name: "picked_up",
                other: Some(by),
                amount: None,
            });
            end(world, entity, PICKUP);
        }
        world.insert_resource(grid);
    }

    /// The first collector `entity`'s sweep meets, by (t, index), with the
    /// pickup's centre there.
    fn first(&mut self, world: &World, grid: &HitGrid, entity: Entity) -> Option<(Entity, Vec2)> {
        let pickup = world.get::<Pickup>(entity)?;
        let sweep = sweep_of(world, entity, pickup.shape.shape(), pickup.offset)?;
        let (min, max) = sweep.bounds();
        grid.candidates(min, max, &mut self.candidates);
        let mut first: Option<(f32, u32)> = None;
        for &index in &self.candidates {
            let entry = &grid.entries()[index as usize];
            if entry.entity == entity || !pickup.filter.matches(world.get::<Tags>(entry.entity)) {
                continue;
            }
            if let Some(t) = first_contact(&sweep, &entry.sweep)
                && first.is_none_or(|(best, at)| (t, index) < (best, at))
            {
                first = Some((t, index));
            }
        }
        first.map(|(t, index)| (grid.entries()[index as usize].entity, sweep.at(t)))
    }
}

/// One collector a magnet may pull toward: its hurtbox's centre and its
/// `pickup_radius`.
#[derive(Debug, Clone, Copy)]
struct Pull {
    entity: Entity,
    at: Vec2,
    radius: f32,
}

/// [`PICKUP_MAGNET`]'s state between steps: the step's magnets and
/// collectors, the distinct filters and one filter's binned collectors,
/// kept so a step allocates little once warm, and the skips warned about.
#[derive(Default)]
struct Magnet {
    /// (pickup, its filter's index in `filters`).
    magnets: Vec<(Entity, usize)>,
    /// The pickups that fall, and those the step's magnets pulled, sorted.
    fallers: Vec<Entity>,
    pulled: Vec<Entity>,
    collectors: Vec<Pull>,
    filters: Vec<TagFilter>,
    /// One filter's collectors, as (cell key, collector index), sorted.
    bins: Vec<(u64, u32)>,
    /// The pickups skipped with a warning.
    warned: HashSet<Entity>,
}

/// A bin's sort key.
fn bin_key(cell: (i32, i32)) -> u64 {
    (u64::from(cell.0 as u32) << 32) | u64::from(cell.1 as u32)
}

/// The bin of width `width` that holds `point`.
fn bin_of(point: Vec2, width: f32) -> (i32, i32) {
    let bin = |v: f32| {
        (v / width)
            .floor()
            .clamp(i32::MIN as f32 + 1.0, i32::MAX as f32 - 1.0) as i32
    };
    (bin(point.x), bin(point.y))
}

impl Magnet {
    /// Moves each uncollected pickup with a magnet toward its nearest
    /// collector in reach, filter by filter, then lets each one that falls
    /// and that no magnet pulled fall.
    fn run(&mut self, world: &mut World) {
        let dt = step_dt(world);
        if dt <= 0.0 {
            return;
        }
        self.gather(world);
        self.pulled.clear();
        if !(self.magnets.is_empty() || self.collectors.is_empty()) {
            for filter in 0..self.filters.len() {
                let width = self.bin(world, filter);
                if width <= 0.0 {
                    continue;
                }
                for i in 0..self.magnets.len() {
                    let (entity, of) = self.magnets[i];
                    if of == filter && self.pull(world, entity, width, dt) {
                        self.pulled.push(entity);
                    }
                }
            }
        }
        self.pulled.sort_unstable_by_key(|entity| entity.id());
        for i in 0..self.fallers.len() {
            let entity = self.fallers[i];
            if self
                .pulled
                .binary_search_by_key(&entity.id(), |pulled| pulled.id())
                .is_err()
            {
                fall(world, entity, dt);
            }
        }
    }

    /// The step's magnets with their filters, and its collectors: hurtbox
    /// entities that take part, with a `pickup_radius` above 0.
    fn gather(&mut self, world: &World) {
        self.magnets.clear();
        self.filters.clear();
        self.fallers.clear();
        // One pass finds the magnets and the pickups that fall.
        let mut magnets: Vec<(Entity, &Pickup)> = Vec::new();
        for (entity, pickup) in world.query::<(Entity, &Pickup)>() {
            if pickup.collected || world.has::<Despawning>(entity) {
                continue;
            }
            if pickup.magnet > 0.0 {
                magnets.push((entity, pickup));
            }
            if pickup.fall > 0.0 {
                self.fallers.push(entity);
            }
        }
        let order = order_of(world);
        magnets.sort_by_key(|&(entity, _)| order.key(entity));
        self.fallers.sort_by_key(|&entity| order.key(entity));
        for (entity, pickup) in magnets {
            let known = self
                .filters
                .iter()
                .position(|filter| *filter == pickup.filter);
            let of = known.unwrap_or_else(|| {
                self.filters.push(pickup.filter.clone());
                self.filters.len() - 1
            });
            self.magnets.push((entity, of));
        }
        self.collectors.clear();
        for (entity, hurtbox, stats) in world.query::<(Entity, &Hurtbox, &Stats)>() {
            let Some(radius) = stats
                .get(Stats::PICKUP_RADIUS)
                .filter(|radius| *radius > 0.0)
            else {
                continue;
            };
            if !takes_part(world, entity) {
                continue;
            }
            if let Some(point) = point_of(world, entity) {
                self.collectors.push(Pull {
                    entity,
                    at: point + hurtbox.offset(),
                    radius,
                });
            }
        }
        let order = order_of(world);
        self.collectors.sort_by_key(|pull| order.key(pull.entity));
    }

    /// Bins the collectors `filter` passes in cells as wide as the largest
    /// radius among them; that width, 0 when none passes.
    fn bin(&mut self, world: &World, filter: usize) -> f32 {
        self.bins.clear();
        let filter = &self.filters[filter];
        let width = self
            .collectors
            .iter()
            .filter(|pull| filter.matches(world.get::<Tags>(pull.entity)))
            .map(|pull| pull.radius)
            .fold(0.0_f32, f32::max);
        if width <= 0.0 {
            return 0.0;
        }
        for (index, pull) in self.collectors.iter().enumerate() {
            if filter.matches(world.get::<Tags>(pull.entity)) {
                self.bins
                    .push((bin_key(bin_of(pull.at, width)), index as u32));
            }
        }
        self.bins.sort_unstable();
        width
    }

    /// Moves `entity` toward the nearest binned collector whose radius
    /// reaches its centre, by its speed and never past it; whether it
    /// pulled it.
    fn pull(&mut self, world: &mut World, entity: Entity, width: f32, dt: f32) -> bool {
        let Some((point, offset, speed)) = world.get::<Pickup>(entity).and_then(|pickup| {
            point_of(world, entity).map(|point| (point, pickup.offset, pickup.magnet))
        }) else {
            return false;
        };
        let centre = point + offset;
        let (bx, by) = bin_of(centre, width);
        let mut nearest: Option<(f32, u32)> = None;
        for x in bx - 1..=bx + 1 {
            for y in by - 1..=by + 1 {
                let key = bin_key((x, y));
                let start = self.bins.partition_point(|&(k, _)| k < key);
                for &(_, index) in self.bins[start..].iter().take_while(|&&(k, _)| k == key) {
                    let pull = self.collectors[index as usize];
                    if pull.entity == entity {
                        continue;
                    }
                    let distance = centre.distance(pull.at);
                    if distance <= pull.radius
                        && nearest.is_none_or(|(best, at)| (distance, index) < (best, at))
                    {
                        nearest = Some((distance, index));
                    }
                }
            }
        }
        let Some((distance, index)) = nearest else {
            return false;
        };
        if distance <= 0.0 {
            return true;
        }
        let toward = (self.collectors[index as usize].at - centre) / distance;
        let delta = toward * (speed * dt).min(distance);
        match body_of(world, entity) {
            Body::Bodiless | Body::Static => step_position(world, entity, delta, dt, Write::Add),
            // A moving body's pull is its velocity for the step, replacing
            // its driver's: the physics moves it by it.
            Body::Kinematic | Body::Dynamic => {
                write_velocity(world, entity, delta / dt, Write::Set);
            }
            skipped => {
                if self.warned.insert(entity) {
                    log::warn!(
                        "Pickup on {entity:?}: its magnet moves a root with a position only, not {skipped:?}; skipped"
                    );
                }
                return false;
            }
        }
        true
    }
}

/// Lets `entity`, a bodiless pickup, fall by its fall speed onto the first
/// tile or static body under its shape in its fall layers, the skin short of
/// it.
fn fall(world: &mut World, entity: Entity, dt: f32) {
    if body_of(world, entity) != Body::Bodiless {
        return;
    }
    let (Some(pickup), Some(point)) = (world.get::<Pickup>(entity), point_of(world, entity)) else {
        return;
    };
    let reach = pickup.fall * dt;
    let filter = SpatialFilter::new()
        .with_mask(pickup.fall_layers)
        .excluding(entity);
    let hit = cast_shape_where(
        world,
        pickup.shape.shape(),
        point + pickup.offset,
        Vec2::Y,
        reach,
        &filter,
        |hit| {
            hit.entity.is_none_or(|body| {
                world
                    .get::<RigidBody>(body)
                    .is_none_or(|rigid| rigid.kind == BodyKind::Static)
            })
        },
    );
    let drop = hit.map_or(reach, |hit| (hit.distance - SKIN).max(0.0));
    if drop > 0.0 {
        step_position(world, entity, Vec2::new(0.0, drop), dt, Write::Add);
    }
}

impl Inspectable for Pickup {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                "pickup",
                format!("{} at {}", self.shape.summary(), point(self.offset)),
            ),
            ("magnet", format!("{} px/s", self.magnet)),
            ("fall", format!("{} px/s", self.fall)),
            ("collected", self.collected.to_string()),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/pickup.rs"]
mod tests;
