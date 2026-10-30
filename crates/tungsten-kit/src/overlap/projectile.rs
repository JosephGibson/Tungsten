//! Projectiles: an entity whose hitbox ends it at its first hit, its first
//! wall or its range, unless it pierces, bounces or passes the wall.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{
    BodyKind, Collider, Position, RigidBody, Shape, SpatialFilter, SpatialHit, Velocity,
    cast_shape, overlap,
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
/// - `bounce_layers`: the physics layers whose walls it turns at while it
///   has bounces left; a wall outside them ends it. Every layer by default.
/// - `pass_layers`: the physics layers whose walls it passes as if they
///   were open. None by default.
///
/// Walls are tiles of collision layers, in their layer's physics layers
/// (layer 0 by default, [`LayerPhysics`](tungsten_core::LayerPhysics)), and
/// static colliders, in their [`RigidBody::layers`] (layer 0 without a body);
/// kinematic and dynamic bodies are never walls, their hurtboxes are what
/// it hits. A wall is in the masks when one of its layers is, and passing
/// comes first. In a step the sweep meets walls in order: each one passed
/// sends [`ProjectilePassed`] and the sweep goes on, and the first one not
/// passed turns it, sending [`ProjectileBounced`], or ends it; hits before
/// that wall land first, and a pierce count and a pass are independent. A
/// wall it starts the step inside is behind it: passing a tile, it meets
/// the terrain where it enters from open space, once a step. An end sends
/// [`ProjectileExpired`], the cue event `expired`, with the wall it ended
/// at, and the kit's end-of-step despawn takes it after its cues. A
/// dynamic body or a child is skipped, with one warning.
///
/// Scene name `projectile`: `{ "pierce": 1, "range": 600, "bounces": 0,
/// "bounce_layers": [0, 2], "pass_layers": [3] }`, each optional: `pierce`
/// and `bounces` 0, `range` none, `bounce_layers` every layer and
/// `pass_layers` none, as lists of layer numbers 0 to 31. It is data: a
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
/// // A bolt that turns off armour in layer 2 and passes a hedge in layer 3.
/// let bolt = Projectile::new().with_bounces(2).with_bounce_layers(1 << 2).with_pass_layers(1 << 3);
/// assert_eq!((bolt.bounce_layers(), bolt.pass_layers()), (0b100, 0b1000));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ProjectileFields", into = "ProjectileFields")]
pub struct Projectile {
    pierce: u32,
    range: Option<f32>,
    bounces: u32,
    /// The layers whose walls it turns at.
    bounce_layers: u32,
    /// The layers whose walls it passes.
    pass_layers: u32,
    /// The targets it has hit.
    hits: u32,
    /// The world pixels it has covered.
    travelled: f32,
    /// The walls it has turned at.
    bounced: u32,
    /// It has ended.
    spent: bool,
}

impl Default for Projectile {
    // Spelled out, as the derive wrote it.
    fn default() -> Projectile {
        Projectile {
            pierce: 0,
            range: None,
            bounces: 0,
            bounce_layers: RigidBody::ALL_LAYERS,
            pass_layers: 0,
            hits: 0,
            travelled: 0.0,
            bounced: 0,
            spent: false,
        }
    }
}

/// A projectile as a save writes it, before its values are checked. The
/// masks are left out at their defaults, so a save without them loads as
/// one that bounces off every layer and passes none.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectileFields {
    #[serde(default)]
    pierce: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    range: Option<f32>,
    #[serde(default)]
    bounces: u32,
    #[serde(default = "every_layer", skip_serializing_if = "is_every_layer")]
    bounce_layers: u32,
    #[serde(default, skip_serializing_if = "is_no_layer")]
    pass_layers: u32,
    #[serde(default)]
    hits: u32,
    #[serde(default)]
    travelled: f32,
    #[serde(default)]
    bounced: u32,
    #[serde(default)]
    spent: bool,
}

/// Every layer: the saved form's default `bounce_layers`.
fn every_layer() -> u32 {
    RigidBody::ALL_LAYERS
}

// The references are serde's `skip_serializing_if` signature.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_every_layer(mask: &u32) -> bool {
    *mask == RigidBody::ALL_LAYERS
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_no_layer(mask: &u32) -> bool {
    *mask == 0
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
            bounce_layers: fields.bounce_layers,
            pass_layers: fields.pass_layers,
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
            bounce_layers: projectile.bounce_layers,
            pass_layers: projectile.pass_layers,
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

    /// The projectile, turning only at walls in `layers` (one bit per
    /// physics layer; a tile is in its collision layer's) while it has
    /// bounces left, and ending at others. Every layer by default.
    #[must_use]
    pub fn with_bounce_layers(mut self, layers: u32) -> Self {
        self.bounce_layers = layers;
        self
    }

    /// The projectile, passing walls in `layers` (one bit per physics
    /// layer; a tile is in its collision layer's) as if they were open. None
    /// by default.
    #[must_use]
    pub fn with_pass_layers(mut self, layers: u32) -> Self {
        self.pass_layers = layers;
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

    /// The physics layers whose walls it turns at, one bit per layer.
    #[must_use]
    pub fn bounce_layers(&self) -> u32 {
        self.bounce_layers
    }

    /// The physics layers whose walls it passes, one bit per layer.
    #[must_use]
    pub fn pass_layers(&self) -> u32 {
        self.pass_layers
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

/// The wall a projectile met: its entity (`None` for a tile), its outward
/// normal where the projectile met it and its physics layers, one bit per
/// layer (a tile's are its collision layer's). Build one with
/// [`WallContact::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct WallContact {
    /// The wall's entity; `None` for a tile.
    pub entity: Option<Entity>,
    /// The wall's outward unit normal where the projectile met it.
    pub normal: Vec2,
    /// The wall's physics layers, one bit per layer.
    pub layers: u32,
}

impl WallContact {
    /// A contact with these fields, as the projectile system builds one.
    #[must_use]
    pub fn new(entity: Option<Entity>, normal: Vec2, layers: u32) -> Self {
        Self {
            entity,
            normal,
            layers,
        }
    }
}

/// A projectile ended, at its last hit, a wall or its range: the cue event
/// `expired`, whose subject is the projectile. Build one with
/// [`ProjectileExpired::new`] or [`ProjectileExpired::at_wall`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ProjectileExpired {
    /// The projectile.
    pub entity: Entity,
    /// Where it ended.
    pub position: Vec2,
    /// The wall it ended at; `None` at a hit or its range's end.
    pub wall: Option<WallContact>,
}

impl ProjectileExpired {
    /// An event with these fields and no wall, as the projectile system
    /// sends one at a hit or its range's end.
    #[must_use]
    pub fn new(entity: Entity, position: Vec2) -> Self {
        Self {
            entity,
            position,
            wall: None,
        }
    }

    /// An event for a projectile that ended at `wall`.
    #[must_use]
    pub fn at_wall(entity: Entity, position: Vec2, wall: WallContact) -> Self {
        Self {
            entity,
            position,
            wall: Some(wall),
        }
    }
}

/// A projectile turned at a wall in its bounce layers, its movers'
/// velocities reflected about the wall. Build one with
/// [`ProjectileBounced::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ProjectileBounced {
    /// The projectile.
    pub entity: Entity,
    /// Where it turned.
    pub position: Vec2,
    /// The wall it turned at.
    pub wall: WallContact,
}

impl ProjectileBounced {
    /// An event with these fields, as the projectile system sends one.
    #[must_use]
    pub fn new(entity: Entity, position: Vec2, wall: WallContact) -> Self {
        Self {
            entity,
            position,
            wall,
        }
    }
}

/// A projectile passed a wall in its pass layers and went on. Build one
/// with [`ProjectilePassed::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ProjectilePassed {
    /// The projectile.
    pub entity: Entity,
    /// Where it met the wall.
    pub position: Vec2,
    /// The wall it passed.
    pub wall: WallContact,
}

impl ProjectilePassed {
    /// An event with these fields, as the projectile system sends one.
    #[must_use]
    pub fn new(entity: Entity, position: Vec2, wall: WallContact) -> Self {
        Self {
            entity,
            position,
            wall,
        }
    }
}

/// One layer number in `projectile.bounce_layers` or
/// `projectile.pass_layers`: 0 to 31.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
struct SceneLayer(u32);

impl TryFrom<u32> for SceneLayer {
    type Error = String;

    fn try_from(layer: u32) -> Result<Self, String> {
        if layer < u32::BITS {
            Ok(Self(layer))
        } else {
            Err(format!("a layer must be 0 to 31 (got {layer})"))
        }
    }
}

impl From<SceneLayer> for u32 {
    fn from(layer: SceneLayer) -> Self {
        layer.0
    }
}

/// The mask of `layers`, one bit per layer.
fn mask_of(layers: &[SceneLayer]) -> u32 {
    layers.iter().fold(0, |mask, layer| mask | 1 << layer.0)
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bounce_layers: Option<Vec<SceneLayer>>,
    #[serde(default)]
    pass_layers: Vec<SceneLayer>,
}

impl From<ProjectileDef> for Projectile {
    fn from(def: ProjectileDef) -> Self {
        let projectile = Projectile::new()
            .with_pierce(def.pierce)
            .with_bounces(def.bounces)
            .with_bounce_layers(
                def.bounce_layers
                    .as_deref()
                    .map_or(RigidBody::ALL_LAYERS, mask_of),
            )
            .with_pass_layers(mask_of(&def.pass_layers));
        match def.range {
            Some(range) => projectile.with_range(range),
            None => projectile,
        }
    }
}

/// Registers `EventQueue<ProjectileExpired>`,
/// `EventQueue<ProjectileBounced>`, `EventQueue<ProjectilePassed>` and
/// `EventQueue<Struck>`, the `projectile` scene component, and in the overlap phase the hit grid
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
                "Ends at its hitbox's first hit, its first wall or its range, unless it pierces, bounces or passes the wall.",
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
            .field(
                FieldDescription::new(
                    "bounce_layers",
                    FieldType::List(Box::new(FieldType::U32 {
                        min: None,
                        max: Some(31),
                    })),
                    "physics layers whose walls it turns at while it has bounces left, a tile in its collision layer's; a wall outside them ends it; every layer when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "pass_layers",
                    FieldType::List(Box::new(FieldType::U32 {
                        min: None,
                        max: Some(31),
                    })),
                    "physics layers whose walls it passes as if open, before the bounce layers",
                )
                .default("[]"),
            )
            .event("ProjectileExpired")
            .event("ProjectileBounced")
            .event("ProjectilePassed")
            .event("Struck")
            .genre(TOP_DOWN)
            .genre(PLATFORMER)
            .genre(SHMUP)
            .sample("{}")
            .roundtrip(round_trip::<ProjectileDef>),
        );
        world.register_event::<ProjectileExpired>();
        world.register_event::<ProjectileBounced>();
        world.register_event::<ProjectilePassed>();
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
    /// A wall it does not pass.
    Wall(WallContact),
    /// Its range.
    Range,
}

/// [`PROJECTILE_TICK`]'s state between steps: the step's projectiles, a
/// query's candidates, a projectile's hits, the walls an overlap lists and
/// the walls it passes, kept so a step allocates nothing once warm, and the
/// skips warned about.
#[derive(Default)]
struct ProjectileRunner {
    projectiles: Vec<Entity>,
    candidates: Vec<u32>,
    /// (t, grid entry), in sweep order once sorted.
    hits: Vec<(f32, u32)>,
    walls: Vec<Option<Entity>>,
    /// (t, wall) of the walls in its pass layers before its stop, in sweep
    /// order once sorted.
    passed: Vec<(f32, WallContact)>,
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

    /// One projectile's step: its hits up to the first wall it does not
    /// pass or its range, the walls it passed before its end, then its end,
    /// its bounce or its full travel.
    fn fly(&mut self, world: &mut World, grid: &HitGrid, entity: Entity) {
        self.passed.clear();
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
            let pass = world.get::<Projectile>(entity).unwrap().pass_layers;
            if let Some((t, wall)) = self.wall(world, entity, &sweep, length, pass) {
                until = t;
                stop = Some(Stop::Wall(wall));
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
        let ended = self.hit(world, grid, entity, &sweep, until);
        self.send_passed(world, entity, &sweep, offset, ended.unwrap_or(until));
        if let Some(t) = ended {
            return finish(world, entity, &sweep, offset, length, t, None);
        }
        match stop {
            Some(Stop::Wall(wall)) if turns(world, entity, wall.layers) => {
                bounce(world, entity, &sweep, offset, length, until, wall);
            }
            Some(Stop::Wall(wall)) => {
                finish(world, entity, &sweep, offset, length, until, Some(wall));
            }
            Some(Stop::Range) => finish(world, entity, &sweep, offset, length, until, None),
            None => world.get_mut::<Projectile>(entity).unwrap().travelled += length,
        }
    }

    /// The first wall the sweep meets outside `pass`, the projectile's pass
    /// layers: a tile in none of them by a cast, or a static collider by an
    /// overlap of the sweep's bounds and the swept test; the earlier of the
    /// two, with its contact. The walls in `pass` it meets at or before that
    /// one, or over the whole sweep, go to `passed` in sweep order; one it
    /// starts inside is behind it, and of tiles only where it enters them
    /// from open space, a cast over the tiles in a pass layer.
    fn wall(
        &mut self,
        world: &World,
        entity: Entity,
        sweep: &Sweep,
        length: f32,
        pass: u32,
    ) -> Option<(f32, WallContact)> {
        let motion = sweep.end - sweep.start;
        let tiles = SpatialFilter::new().with_bodies(false);
        let tile_contact = |hit: SpatialHit| {
            (
                (hit.distance / length).clamp(0.0, 1.0),
                WallContact::new(None, hit.normal, hit.layers),
            )
        };
        let stops = tiles.without_layers(pass);
        let mut first =
            cast_shape(world, sweep.shape, sweep.start, motion, length, &stops).map(tile_contact);
        if pass != 0 {
            let passable = tiles.with_mask(pass);
            overlap(world, sweep.shape, sweep.start, &passable, &mut self.walls);
            if self.walls.is_empty() {
                self.passed.extend(
                    cast_shape(world, sweep.shape, sweep.start, motion, length, &passable)
                        .map(tile_contact),
                );
            }
        }
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
            let layers = world
                .get::<RigidBody>(wall)
                .map_or(RigidBody::DEFAULT_LAYERS, |body| body.layers);
            if layers & pass != 0 {
                let still = Sweep {
                    end: sweep.start,
                    ..*sweep
                };
                if first_contact(&still, &face).is_none()
                    && let Some((t, normal)) = wall_contact(sweep, &face)
                {
                    self.passed
                        .push((t, WallContact::new(Some(wall), normal, layers)));
                }
            } else if let Some((t, normal)) = wall_contact(sweep, &face)
                && first.is_none_or(|(earliest, _)| t < earliest)
            {
                first = Some((t, WallContact::new(Some(wall), normal, layers)));
            }
        }
        if !self.passed.is_empty() {
            let until = first.map_or(1.0, |(t, _)| t);
            self.passed.retain(|&(t, _)| t <= until);
            self.passed.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        first
    }

    /// Sends [`ProjectilePassed`] for each wall passed at or before `last`
    /// of the sweep, where the step ends, in sweep order.
    fn send_passed(
        &self,
        world: &mut World,
        entity: Entity,
        sweep: &Sweep,
        offset: Vec2,
        last: f32,
    ) {
        if self.passed.is_empty() {
            return;
        }
        let Some(queue) = world.get_resource_mut::<EventQueue<ProjectilePassed>>() else {
            return;
        };
        for &(t, wall) in self.passed.iter().take_while(|(t, _)| *t <= last) {
            queue.send(ProjectilePassed::new(entity, sweep.at(t) - offset, wall));
        }
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

/// Whether the projectile turns at a wall in `layers`: one in its bounce
/// layers, with bounces left.
fn turns(world: &World, entity: Entity, layers: u32) -> bool {
    world.get::<Projectile>(entity).is_some_and(|projectile| {
        projectile.bounced < projectile.bounces && layers & projectile.bounce_layers != 0
    })
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

/// Turns `entity` at `wall`: placed at the contact, each velocity its
/// movers keep reflected about the wall's normal, the rest of the step's
/// travel dropped, and [`ProjectileBounced`] sent.
fn bounce(
    world: &mut World,
    entity: Entity,
    sweep: &Sweep,
    offset: Vec2,
    length: f32,
    t: f32,
    wall: WallContact,
) {
    let point = sweep.at(t) - offset;
    let normal = wall.normal;
    place(world, entity, point);
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
    if let Some(queue) = world.get_resource_mut::<EventQueue<ProjectileBounced>>() {
        queue.send(ProjectileBounced::new(entity, point, wall));
    }
}

/// Ends `entity` at `t` of its sweep, at `wall` when it ended at one:
/// placed there, its movers stopped, spent, [`ProjectileExpired`] sent and
/// traced, and listed for the kit's end-of-step despawn.
fn finish(
    world: &mut World,
    entity: Entity,
    sweep: &Sweep,
    offset: Vec2,
    length: f32,
    t: f32,
    wall: Option<WallContact>,
) {
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
        queue.send(ProjectileExpired {
            entity,
            position: point,
            wall,
        });
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
            ("bounce layers", format!("{:#010x}", self.bounce_layers)),
            ("pass layers", format!("{:#010x}", self.pass_layers)),
            ("spent", self.spent.to_string()),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/projectile.rs"]
mod tests;
