//! Projectiles: an entity whose hitbox ends it at its first hit, its first
//! wall or its range, unless it pierces or bounces.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{
    BodyKind, Collider, Position, RigidBody, Shape, SpatialFilter, Velocity, cast_shape, overlap,
};
use tungsten_core::{
    Entity, EventQueue, Inspectable, Plugin, Schedule, SpawnContext, Transform, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, SHMUP, TOP_DOWN,
    describe, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::optional_positive;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::tags::Tags;
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::ended::{Despawning, end, ensure_kit_despawn};
use crate::motion::drive::{Body, Skip, Warned, body_of};
use crate::motion::linear::LinearMotion;
use crate::motion::steering::Steering;
use crate::overlap::grid::{
    Blow, HitGrid, Sweep, ensure_emitter, first_contact, reflect, spares, strike, sweep_of,
    wall_contact,
};
use crate::overlap::hit::{Hitbox, Struck, after_grid, ensure_grid};

/// [`ProjectilePlugin`]'s system, in the overlap phase.
pub const PROJECTILE_TICK: &str = "projectile_tick";

/// The scene component the plugin registers.
const PROJECTILE: &str = "projectile";

/// A bolt, an arrow, a fireball: an entity whose
/// [`Hitbox`](crate::Hitbox) hits the hurtboxes its sweep meets, in the
/// order it meets them, and which ends at its first hit, its first wall or
/// its range. [`ProjectilePlugin`]'s system moves nothing: the entity's
/// movers ([`LinearMotion`](crate::LinearMotion),
/// [`Steering`](crate::Steering), a kinematic body's `Velocity`) carry it,
/// and the system ends it where the step's sweep met its end, placing it
/// there and stopping its movers.
///
/// - `pierce`: it passes this many targets and ends at the next.
/// - `bounces`: at a wall it turns this many times, its movers' velocities
///   reflected about the wall, and ends at the next.
/// - `range`: it ends once it has travelled this far, in world pixels.
///
/// Walls are tile layers and static colliders; kinematic and dynamic
/// bodies are never walls, their hurtboxes are what it hits. An end sends
/// [`ProjectileExpired`], the cue event `expired`, and the kit's
/// end-of-step despawn takes it after its cues. A dynamic body or a child
/// is skipped, with one warning.
///
/// Scene name `projectile`: `{ "pierce": 1, "range": 600, "bounces": 0 }`,
/// each optional: `pierce` and `bounces` 0, `range` none. It is data: a
/// game's save can hold it, its hits, distance and bounces included.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Projectile;
///
/// let arrow = Projectile::new().with_pierce(1).with_range(600.0);
/// assert_eq!((arrow.pierce(), arrow.range(), arrow.bounces()), (1, Some(600.0), 0));
/// assert!(!arrow.is_spent() && arrow.travelled() == 0.0);
/// let read: Projectile = serde_json::from_str(r#"{ "bounces": 2 }"#).unwrap();
/// assert_eq!(read, Projectile::new().with_bounces(2));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ProjectileFields", into = "ProjectileFields")]
pub struct Projectile {
    pierce: u32,
    range: Option<f32>,
    bounces: u32,
    /// The targets it has hit.
    hits: u32,
    /// The world pixels it has covered.
    travelled: f32,
    /// The walls it has turned at.
    bounced: u32,
    /// It has ended.
    spent: bool,
}

/// A projectile as a save writes it, before its values are checked.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectileFields {
    #[serde(default)]
    pierce: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    range: Option<f32>,
    #[serde(default)]
    bounces: u32,
    #[serde(default)]
    hits: u32,
    #[serde(default)]
    travelled: f32,
    #[serde(default)]
    bounced: u32,
    #[serde(default)]
    spent: bool,
}

impl TryFrom<ProjectileFields> for Projectile {
    type Error = String;

    fn try_from(fields: ProjectileFields) -> Result<Self, Self::Error> {
        if let Some(range) = fields.range
            && !(range.is_finite() && range > 0.0)
        {
            return Err(format!("range: must be finite and > 0 (got {range})"));
        }
        if !(fields.travelled.is_finite() && fields.travelled >= 0.0) {
            return Err(format!(
                "travelled: must be finite and >= 0 (got {})",
                fields.travelled
            ));
        }
        Ok(Self {
            pierce: fields.pierce,
            range: fields.range,
            bounces: fields.bounces,
            hits: fields.hits,
            travelled: fields.travelled,
            bounced: fields.bounced,
            spent: fields.spent,
        })
    }
}

impl From<Projectile> for ProjectileFields {
    fn from(projectile: Projectile) -> Self {
        Self {
            pierce: projectile.pierce,
            range: projectile.range,
            bounces: projectile.bounces,
            hits: projectile.hits,
            travelled: projectile.travelled,
            bounced: projectile.bounced,
            spent: projectile.spent,
        }
    }
}

impl Projectile {
    /// A projectile that ends at its first hit or wall, with no range.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The projectile, passing `targets` targets and ending at the next.
    #[must_use]
    pub fn with_pierce(mut self, targets: u32) -> Self {
        self.pierce = targets;
        self
    }

    /// The projectile, ending once it has travelled `distance` world
    /// pixels.
    ///
    /// # Panics
    ///
    /// When `distance` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_range(mut self, distance: f32) -> Self {
        assert!(
            distance.is_finite() && distance > 0.0,
            "Projectile::with_range: distance must be finite and > 0 (got {distance})"
        );
        self.range = Some(distance);
        self
    }

    /// The projectile, turning at `walls` walls and ending at the next.
    #[must_use]
    pub fn with_bounces(mut self, walls: u32) -> Self {
        self.bounces = walls;
        self
    }

    /// The targets it passes before the next ends it.
    #[must_use]
    pub fn pierce(&self) -> u32 {
        self.pierce
    }

    /// The distance it ends at, when it has one.
    #[must_use]
    pub fn range(&self) -> Option<f32> {
        self.range
    }

    /// The walls it turns at before the next ends it.
    #[must_use]
    pub fn bounces(&self) -> u32 {
        self.bounces
    }

    /// The targets it has hit.
    #[must_use]
    pub fn hits(&self) -> u32 {
        self.hits
    }

    /// The world pixels it has travelled.
    #[must_use]
    pub fn travelled(&self) -> f32 {
        self.travelled
    }

    /// Whether it has ended: it hits nothing more, and the kit despawns it.
    #[must_use]
    pub fn is_spent(&self) -> bool {
        self.spent
    }
}

/// A projectile ended, at its last hit, a wall or its range: the cue event
/// `expired`, whose subject is the projectile. Build one with
/// [`ProjectileExpired::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ProjectileExpired {
    /// The projectile.
    pub entity: Entity,
    /// Where it ended.
    pub position: Vec2,
}

impl ProjectileExpired {
    /// An event with these fields, as the projectile system sends one.
    #[must_use]
    pub fn new(entity: Entity, position: Vec2) -> Self {
        Self { entity, position }
    }
}

/// `components.projectile` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectileDef {
    #[serde(default)]
    pierce: u32,
    #[serde(
        default,
        deserialize_with = "optional_positive",
        skip_serializing_if = "Option::is_none"
    )]
    range: Option<f32>,
    #[serde(default)]
    bounces: u32,
}

impl From<ProjectileDef> for Projectile {
    fn from(def: ProjectileDef) -> Self {
        let projectile = Projectile::new()
            .with_pierce(def.pierce)
            .with_bounces(def.bounces);
        match def.range {
            Some(range) => projectile.with_range(range),
            None => projectile,
        }
    }
}

/// Registers `EventQueue<ProjectileExpired>` and `EventQueue<Struck>`, the
/// `projectile` scene component, and in the overlap phase the hit grid
/// when the schedule lacks it and then [`PROJECTILE_TICK`], before
/// `despawn_outside`, `wrap` and `stay_in_view` when each is registered,
/// so an outside region judges a projectile where its wall left it; and
/// the kit's end-of-step despawn when the schedule lacks it.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, World};
/// use tungsten_kit::{KIT_DESPAWN, PROJECTILE_TICK, ProjectilePlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// ProjectilePlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// assert!(schedule.contains(PROJECTILE_TICK) && schedule.contains(KIT_DESPAWN));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectilePlugin;

impl Plugin for ProjectilePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Projectile>(world, "Projectile");
        describe(
            world,
            ComponentDescription::new(
                PROJECTILE,
                "Ends at its hitbox's first hit, its first wall or its range, unless it pierces or bounces.",
            )
            .field(
                FieldDescription::new(
                    "pierce",
                    FieldType::U32 {
                        min: None,
                        max: None,
                    },
                    "targets it passes before the next ends it",
                )
                .default("0"),
            )
            .field(
                FieldDescription::new(
                    "range",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels it travels before it ends; none when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "bounces",
                    FieldType::U32 {
                        min: None,
                        max: None,
                    },
                    "walls it turns at before the next ends it",
                )
                .default("0"),
            )
            .event("ProjectileExpired")
            .event("Struck")
            .genre(TOP_DOWN)
            .genre(PLATFORMER)
            .genre(SHMUP)
            .sample("{}")
            .roundtrip(round_trip::<ProjectileDef>),
        );
        world.register_event::<ProjectileExpired>();
        world.register_event::<Struck>();
        world.register_component_with(PROJECTILE, |def: ProjectileDef, _: &SpawnContext<'_>| {
            (Projectile::from(def),)
        });
        ensure_emitter(world);
        ensure_grid(schedule);
        let mut runner = ProjectileRunner::default();
        schedule.add(
            Phase::Overlap.stage(),
            after_grid(system(PROJECTILE_TICK, move |world: &mut World| {
                runner.run(world);
            })),
        );
        ensure_kit_despawn(schedule, world);
    }
}

/// How a projectile's step ends before its full sweep.
#[derive(Debug, Clone, Copy)]
enum Stop {
    /// A wall, with its outward normal at the contact.
    Wall(Vec2),
    /// Its range.
    Range,
}

/// [`PROJECTILE_TICK`]'s state between steps: the step's projectiles, a
/// query's candidates, a projectile's hits and the walls an overlap lists,
/// kept so a step allocates nothing once warm, and the skips warned about.
#[derive(Default)]
struct ProjectileRunner {
    projectiles: Vec<Entity>,
    candidates: Vec<u32>,
    /// (t, grid entry), in sweep order once sorted.
    hits: Vec<(f32, u32)>,
    walls: Vec<Option<Entity>>,
    warned: Warned,
}

impl ProjectileRunner {
    /// Runs every live projectile, in entity order.
    fn run(&mut self, world: &mut World) {
        let Some(grid) = world.remove_resource::<HitGrid>() else {
            return;
        };
        self.projectiles.clear();
        self.projectiles.extend(
            world
                .query::<(Entity, &Projectile)>()
                .filter(|(_, projectile)| !projectile.spent)
                .map(|(entity, _)| entity),
        );
        self.projectiles.sort_by_key(|entity| entity.id());
        for i in 0..self.projectiles.len() {
            let entity = self.projectiles[i];
            if world.has::<Despawning>(entity) {
                continue;
            }
            match body_of(world, entity) {
                Body::Dynamic => self.warned.skip("Projectile", entity, Skip::Dynamic),
                Body::Child => self.warned.skip("Projectile", entity, Skip::Child),
                _ => self.fly(world, &grid, entity),
            }
        }
        world.insert_resource(grid);
    }

    /// One projectile's step: its hits up to its first wall or its range,
    /// then its end, its bounce or its full travel.
    fn fly(&mut self, world: &mut World, grid: &HitGrid, entity: Entity) {
        let (shape, offset) = world
            .get::<Hitbox>(entity)
            .map_or((Shape::Circle { radius: 0.0 }, Vec2::ZERO), |hitbox| {
                (hitbox.shape().shape(), hitbox.offset())
            });
        let Some(sweep) = sweep_of(world, entity, shape, offset) else {
            return;
        };
        let length = (sweep.end - sweep.start).length();
        let mut until = 1.0_f32;
        let mut stop = None;
        if length > 0.0 {
            if let Some((t, normal)) = self.wall(world, entity, &sweep, length) {
                until = t;
                stop = Some(Stop::Wall(normal));
            }
            let projectile = world.get::<Projectile>(entity).unwrap();
            if let Some(range) = projectile.range {
                let t = ((range - projectile.travelled) / length).max(0.0);
                if t <= until {
                    until = t;
                    stop = Some(Stop::Range);
                }
            }
        }
        if let Some(t) = self.hit(world, grid, entity, &sweep, until) {
            return finish(world, entity, &sweep, offset, length, t);
        }
        match stop {
            Some(Stop::Wall(normal)) if bounces_left(world, entity) => {
                bounce(world, entity, &sweep, offset, length, until, normal);
            }
            Some(_) => finish(world, entity, &sweep, offset, length, until),
            None => world.get_mut::<Projectile>(entity).unwrap().travelled += length,
        }
    }

    /// The first wall the sweep meets: a tile by a cast, or a static
    /// collider by an overlap of the sweep's bounds and the swept test; the
    /// earlier of the two, with its normal.
    fn wall(
        &mut self,
        world: &World,
        entity: Entity,
        sweep: &Sweep,
        length: f32,
    ) -> Option<(f32, Vec2)> {
        let motion = sweep.end - sweep.start;
        let tiles = SpatialFilter::new().with_bodies(false);
        let mut first = cast_shape(world, sweep.shape, sweep.start, motion, length, &tiles)
            .map(|hit| ((hit.distance / length).clamp(0.0, 1.0), hit.normal));
        let (min, max) = sweep.bounds();
        let around = Shape::Aabb {
            half_extents: (max - min) * 0.5,
        };
        let bodies = SpatialFilter::new().with_tiles(false).excluding(entity);
        overlap(world, around, (min + max) * 0.5, &bodies, &mut self.walls);
        for &wall in &self.walls {
            let Some(wall) = wall else {
                continue;
            };
            let still = world
                .get::<RigidBody>(wall)
                .is_none_or(|body| body.kind == BodyKind::Static);
            let (Some(collider), Some(position), true) = (
                world.get::<Collider>(wall),
                world.get::<Position>(wall),
                still,
            ) else {
                continue;
            };
            let at = position.0 + collider.offset;
            let face = Sweep {
                shape: collider.shape,
                start: at,
                end: at,
            };
            if let Some((t, normal)) = wall_contact(sweep, &face)
                && first.is_none_or(|(earliest, _)| t < earliest)
            {
                first = Some((t, normal));
            }
        }
        first
    }

    /// Lands the hits up to `until` in sweep order, counting each; the `t`
    /// of the one past the projectile's pierce, which ends it.
    fn hit(
        &mut self,
        world: &mut World,
        grid: &HitGrid,
        entity: Entity,
        sweep: &Sweep,
        until: f32,
    ) -> Option<f32> {
        self.hits.clear();
        let hitbox = world.get::<Hitbox>(entity)?;
        let (min, max) = sweep.bounds();
        grid.candidates(min, max, &mut self.candidates);
        for &index in &self.candidates {
            let entry = &grid.entries()[index as usize];
            if hitbox.has_hit(entry.entity)
                || spares(world, entity, entry.entity)
                || !hitbox.filter().matches(world.get::<Tags>(entry.entity))
            {
                continue;
            }
            if let Some(t) = first_contact(sweep, &entry.sweep)
                && t <= until
            {
                self.hits.push((t, index));
            }
        }
        if self.hits.is_empty() {
            return None;
        }
        self.hits
            .sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let (damage, tags, knockback, typed) = (
            hitbox.damage(),
            hitbox.tags().clone(),
            hitbox.knockback(),
            hitbox.payload().cloned(),
        );
        for &(t, index) in &self.hits {
            let entry = grid.entries()[index as usize];
            world
                .get_mut::<Hitbox>(entity)
                .unwrap()
                .mark_hit(entry.entity);
            strike(
                world,
                &Blow {
                    hitter: entity,
                    target: entry.entity,
                    hitter_at: sweep.at(t),
                    target_at: entry.sweep.at(t),
                    relative: (sweep.end - sweep.start) - (entry.sweep.end - entry.sweep.start),
                    damage,
                    tags: &tags,
                    typed: typed.as_ref(),
                    knockback,
                },
            );
            let projectile = world.get_mut::<Projectile>(entity).unwrap();
            projectile.hits = projectile.hits.saturating_add(1);
            if projectile.hits > projectile.pierce {
                return Some(t);
            }
        }
        None
    }
}

/// Whether the projectile turns at its next wall.
fn bounces_left(world: &World, entity: Entity) -> bool {
    world
        .get::<Projectile>(entity)
        .is_some_and(|projectile| projectile.bounced < projectile.bounces)
}

/// Puts `entity`'s point at `point`: a root's `Position`, else its
/// `Transform.position`.
fn place(world: &mut World, entity: Entity, point: Vec2) {
    if let Some(position) = world.get_mut::<Position>(entity) {
        position.0 = point;
    } else if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.position = point;
    }
}

/// Turns `entity` at a wall: placed at the contact, each velocity its
/// movers keep reflected about the wall's `normal`, the rest of the step's
/// travel dropped.
fn bounce(
    world: &mut World,
    entity: Entity,
    sweep: &Sweep,
    offset: Vec2,
    length: f32,
    t: f32,
    normal: Vec2,
) {
    place(world, entity, sweep.at(t) - offset);
    if let Some(motion) = world.get_mut::<LinearMotion>(entity) {
        let turned = reflect(motion.velocity(), normal);
        motion.set_velocity(turned);
    }
    if let Some(steering) = world.get_mut::<Steering>(entity) {
        let velocity = steering.velocity_mut();
        *velocity = reflect(*velocity, normal);
    }
    if let Some(velocity) = world.get_mut::<Velocity>(entity) {
        velocity.0 = reflect(velocity.0, normal);
    }
    let projectile = world.get_mut::<Projectile>(entity).unwrap();
    projectile.bounced = projectile.bounced.saturating_add(1);
    projectile.travelled += length * t;
}

/// Ends `entity` at `t` of its sweep: placed there, its movers stopped,
/// spent, [`ProjectileExpired`] sent and traced, and listed for the kit's
/// end-of-step despawn.
fn finish(world: &mut World, entity: Entity, sweep: &Sweep, offset: Vec2, length: f32, t: f32) {
    let point = sweep.at(t) - offset;
    place(world, entity, point);
    if let Some(motion) = world.get_mut::<LinearMotion>(entity) {
        *motion = LinearMotion::new(Vec2::ZERO);
    }
    if let Some(steering) = world.get_mut::<Steering>(entity) {
        *steering.velocity_mut() = Vec2::ZERO;
    }
    if let Some(velocity) = world.get_mut::<Velocity>(entity) {
        velocity.0 = Vec2::ZERO;
    }
    let projectile = world.get_mut::<Projectile>(entity).unwrap();
    projectile.travelled += length * t;
    projectile.spent = true;
    if let Some(queue) = world.get_resource_mut::<EventQueue<ProjectileExpired>>() {
        queue.send(ProjectileExpired::new(entity, point));
    }
    trace(world, entity, || TraceKind::Event {
        name: "expired",
        other: None,
        amount: None,
    });
    end(world, entity, "projectile");
}

impl Inspectable for Projectile {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let range = self.range.map_or_else(
            || "none".to_string(),
            |range| format!("{:.1} of {range}", self.travelled),
        );
        vec![
            (
                "pierce",
                format!("{} left", self.pierce.saturating_sub(self.hits)),
            ),
            ("range", range),
            (
                "bounces",
                format!("{} left", self.bounces.saturating_sub(self.bounced)),
            ),
            ("spent", self.spent.to_string()),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/projectile.rs"]
mod tests;
