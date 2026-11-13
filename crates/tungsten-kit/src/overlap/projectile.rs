//! Projectiles: an entity whose hitbox ends it at its first hit, its first
//! wall or its range, unless it pierces, bounces or passes the wall.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{
    BodyKind, Collider, Position, PrevPosition, RigidBody, Shape, SpatialFilter, SpatialHit,
    Velocity, cast_shape, overlap,
};
use tungsten_core::{
    Entity, EventQueue, Inspectable, MissingAsset, Plugin, Schedule, SpawnContext, Transform,
    World, system,
};

use crate::chemistry::blast::{Blast, Blasts, blast, credits};
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, SHOOTER, TOP_DOWN,
    describe, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::layers::{LayerRef, check_layers, layer_bits, layer_list};
use crate::foundation::optional_positive;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::saved::order_of;
use crate::foundation::source::Source;
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
use crate::overlap::hit::{HitShape, Hitbox, Struck, after_grid, ensure_grid, optional_offset};

/// [`ProjectilePlugin`]'s system, in the overlap phase.
pub const PROJECTILE_TICK: &str = "projectile_tick";

/// The scene component the plugin registers.
const PROJECTILE: &str = "projectile";

/// The shape of `projectile.end`.
const PROJECTILE_END: &str = "projectile_end";

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
/// - `end`: what it sets off at each of its own ends, a hit past its
///   pierce, a wall it neither passes nor turns at, or its range: a
///   [`Blast`](crate::Blast), in the chemistry output's form, at its
///   hitbox's centre there (its point without a hitbox), which neither
///   hurts nor pushes it. Once the step's sweeps are done the system sets
///   the step's ends off through [`blast`](crate::blast), in the order the
///   projectiles ended, each credited to its [`Source`](crate::Source)
///   while that lives and, holding [`Health`](crate::Health), is not dead,
///   else to nobody, and to itself only without a source: a `queue` hit
///   lands at the step's resolve, a `now` hit at once, and its stimuli
///   reach [`ChemistryPlugin`](crate::ChemistryPlugin)'s run that step, so
///   an end's parts and statuses come in its own step, as a hit's do. A
///   World without that plugin traces the end's blast as skipped. A
///   [`Lifetime`](crate::Lifetime)'s despawn is no end of its own and sets
///   off nothing. None by default; Rust gives one with
///   [`with_end`](Self::with_end), and [`end_blast`](Self::end_blast) reads
///   it.
///
/// Walls are tiles of collision layers, in their layer's physics layers
/// (layer 0 by default, [`LayerPhysics`](tungsten_core::LayerPhysics)), and
/// static colliders, in their [`RigidBody::layers`] (layer 0 without a body);
/// kinematic and dynamic bodies are walls only in its `body_walls`, met over
/// their step's move from their `PrevPosition` and turning it in their own
/// frame, and never its [`Source`](crate::Source)'s; otherwise their
/// hurtboxes are what it hits. A wall is in the masks when one of its
/// layers is, and passing comes first. Without a hitbox it sweeps its own
/// `shape` at its `offset`, its walls and its end measured from it, else
/// its point. In a step the sweep meets walls in order: each one passed
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
/// "bounce_layers": [0, "armour"], "pass_layers": [3], "body_walls":
/// ["crates"], "shape": { "circle": 5 }, "offset": [8, 8], "end": { "blast":
/// { "radius": 64, "damage": 20 } } }`, each optional: `pierce` and
/// `bounces` 0, `range` none, `bounce_layers` every layer, `pass_layers`
/// and `body_walls` none, as lists of layer numbers 0 to 31 or
/// `physics_layers` names, no shape, and `end` none; the load checks the
/// layer names and the blast's damage types and context tags. It is data:
/// a game's save can hold it, its end, shape, body walls, hits, distance
/// and bounces included.
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
    /// The layers whose moving bodies are walls to it.
    body_walls: u32,
    /// Its sweep's own shape and offset, which a hitbox's replace.
    shape: Option<(HitShape, Vec2)>,
    /// What it sets off at its own end; taken when it goes off.
    end: Option<ProjectileEnd>,
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
            body_walls: 0,
            shape: None,
            end: None,
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
    #[serde(default, skip_serializing_if = "is_no_layer")]
    body_walls: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape: Option<HitShape>,
    #[serde(
        default,
        with = "optional_offset",
        skip_serializing_if = "Option::is_none"
    )]
    offset: Option<Vec2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end: Option<ProjectileEnd>,
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
            body_walls: fields.body_walls,
            shape: own_shape(fields.shape, fields.offset)?,
            end: fields.end,
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
            body_walls: projectile.body_walls,
            shape: projectile.shape.map(|(shape, _)| shape),
            offset: projectile
                .shape
                .map(|(_, offset)| offset)
                .filter(|&offset| offset != Vec2::ZERO),
            end: projectile.end,
            hits: projectile.hits,
            travelled: projectile.travelled,
            bounced: projectile.bounced,
            spent: projectile.spent,
        }
    }
}

/// A projectile's own sweep shape and its offset, which comes only with it.
fn own_shape(
    shape: Option<HitShape>,
    offset: Option<Vec2>,
) -> Result<Option<(HitShape, Vec2)>, String> {
    match (shape, offset) {
        (Some(shape), offset) => Ok(Some((shape, offset.unwrap_or(Vec2::ZERO)))),
        (None, None) => Ok(None),
        (None, Some(_)) => Err("an `offset` comes only with its own `shape`".to_string()),
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

    /// The projectile, setting `blast` off at each of its own ends: the
    /// Rust form of data's `end`.
    #[must_use]
    pub fn with_end(mut self, blast: Blast) -> Self {
        self.end = Some(ProjectileEnd::Blast(Box::new(blast)));
        self
    }

    /// The projectile, meeting the dynamic and kinematic bodies in `layers`
    /// (one bit per physics layer) as walls over their step's move, but its
    /// [`Source`](crate::Source)'s. None by default: every moving body
    /// passes.
    #[must_use]
    pub fn with_body_walls(mut self, layers: u32) -> Self {
        self.body_walls = layers;
        self
    }

    /// The projectile, sweeping `shape` at `offset` from its point when it
    /// has no hitbox: its walls, body walls and end's blast measured from
    /// it, as from a hitbox's. A point by default.
    #[must_use]
    pub fn with_shape(mut self, shape: HitShape, offset: Vec2) -> Self {
        self.shape = Some((shape, offset));
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

    /// The physics layers whose moving bodies are walls to it, one bit per
    /// layer.
    #[must_use]
    pub fn body_walls(&self) -> u32 {
        self.body_walls
    }

    /// Its own sweep shape and offset, when it has one.
    #[must_use]
    pub fn shape(&self) -> Option<(HitShape, Vec2)> {
        self.shape
    }

    /// The blast it sets off at its end, while it has one: none once it
    /// has gone off.
    #[must_use]
    pub fn end_blast(&self) -> Option<&Blast> {
        match &self.end {
            Some(ProjectileEnd::Blast(blast)) => Some(blast),
            None => None,
        }
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

/// What a projectile sets off at its own end: `{ "blast": { … } }`, a
/// blast in the chemistry output's form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProjectileEnd {
    /// A blast at its hitbox's centre where it ended.
    Blast(Box<Blast>),
}

impl ProjectileEnd {
    /// The load's check: the blast's damage types and context tags.
    fn check(&self, world: &World) -> Result<(), MissingAsset> {
        match self {
            Self::Blast(blast) => blast.check(world, "end.blast"),
        }
    }
}

/// `components.projectile` as a scene writes it, its own shape's offset
/// checked.
#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "ProjectileScene", into = "ProjectileScene")]
struct ProjectileDef(ProjectileScene);

/// [`ProjectileDef`]'s fields as written.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectileScene {
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
    bounce_layers: Option<Vec<LayerRef>>,
    #[serde(default)]
    pass_layers: Vec<LayerRef>,
    #[serde(default)]
    body_walls: Vec<LayerRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape: Option<HitShape>,
    #[serde(
        default,
        with = "optional_offset",
        skip_serializing_if = "Option::is_none"
    )]
    offset: Option<Vec2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end: Option<ProjectileEnd>,
}

impl TryFrom<ProjectileScene> for ProjectileDef {
    type Error = String;

    fn try_from(scene: ProjectileScene) -> Result<Self, String> {
        own_shape(scene.shape, scene.offset)?;
        Ok(Self(scene))
    }
}

impl From<ProjectileDef> for ProjectileScene {
    fn from(def: ProjectileDef) -> Self {
        def.0
    }
}

impl ProjectileDef {
    /// The load's check: its layer names and its end's blast.
    fn check(&self, world: &World) -> Result<(), MissingAsset> {
        let scene = &self.0;
        check_layers(
            "bounce_layers",
            scene.bounce_layers.as_deref().unwrap_or_default(),
            world,
        )?;
        check_layers("pass_layers", &scene.pass_layers, world)?;
        check_layers("body_walls", &scene.body_walls, world)?;
        scene.end.as_ref().map_or(Ok(()), |end| end.check(world))
    }

    /// The projectile it writes, its layer names read in `world`.
    fn into_projectile(self, world: &World) -> Projectile {
        let scene = self.0;
        let mut projectile = Projectile::new()
            .with_pierce(scene.pierce)
            .with_bounces(scene.bounces)
            .with_bounce_layers(
                scene
                    .bounce_layers
                    .as_deref()
                    .map_or(RigidBody::ALL_LAYERS, |layers| layer_bits(layers, world)),
            )
            .with_pass_layers(layer_bits(&scene.pass_layers, world))
            .with_body_walls(layer_bits(&scene.body_walls, world));
        projectile.shape = scene
            .shape
            .map(|shape| (shape, scene.offset.unwrap_or(Vec2::ZERO)));
        projectile.end = scene.end;
        match scene.range {
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
        world.register_saved::<Projectile>(PROJECTILE);
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
                    layer_list(),
                    "physics layers whose walls it turns at while it has bounces left, a tile in its collision layer's; a wall outside them ends it; every layer when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "pass_layers",
                    layer_list(),
                    "physics layers whose walls it passes as if open, before the bounce layers",
                )
                .default("[]"),
            )
            .field(
                FieldDescription::new(
                    "body_walls",
                    layer_list(),
                    "physics layers whose dynamic and kinematic bodies are walls to it, met over their step's move, turning it in their frame; never its source's",
                )
                .default("[]"),
            )
            .field(
                FieldDescription::new(
                    "shape",
                    FieldType::Shape("hit_shape"),
                    "its sweep's own circle or box without a hitbox, its walls and end measured from it; a point when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::Vec2,
                    "its own shape's offset from its point, with `shape` only; [0, 0] when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "end",
                    FieldType::Shape(PROJECTILE_END),
                    "what it sets off at each of its own ends, a hit past its pierce, a wall or its range, not at a lifetime's; none when left out",
                )
                .default("null"),
            )
            .event("ProjectileExpired")
            .event("ProjectileBounced")
            .event("ProjectilePassed")
            .event("Struck")
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHOOTER)
            .sample("{}")
            .roundtrip(round_trip::<ProjectileDef>),
        );
        describe_shape(
            world,
            ComponentDescription::new(
                PROJECTILE_END,
                "What a projectile sets off at its own end: a hit past its pierce, a wall it neither passes nor turns at, or its range.",
            )
            .one_of()
            .field(FieldDescription::new(
                "blast",
                FieldType::Shape("blast_output"),
                "a blast at its hitbox's centre there, from its source, else itself, which it neither hurts nor pushes, in chemistry's run that step",
            ))
            .sample(r#"{ "blast": { "radius": 64 } }"#)
            .roundtrip(round_trip::<ProjectileEnd>),
        );
        world.register_event::<ProjectileExpired>();
        world.register_event::<ProjectileBounced>();
        world.register_event::<ProjectilePassed>();
        world.register_event::<Struck>();
        world.register_component_with_assets(
            PROJECTILE,
            |def: &ProjectileDef, world: &World| def.check(world),
            |def: ProjectileDef, at: &SpawnContext<'_>| (def.into_projectile(at.world()),),
        );
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
    /// A wall it does not pass, and the wall's velocity: a body wall's
    /// frame, zero for a tile or a static collider.
    Wall(WallContact, Vec2),
    /// Its range.
    Range,
}

/// [`PROJECTILE_TICK`]'s state between steps: the step's projectiles, a
/// query's candidates, a projectile's hits, the walls an overlap lists, the
/// walls it passes and the step's end blasts, kept so a step allocates
/// nothing once warm, and the skips warned about.
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
    /// The step's end blasts, in the order their projectiles ended: the
    /// projectile, its blast and the blast's centre.
    ends: Vec<(Entity, Box<Blast>, Vec2)>,
    warned: Warned,
}

impl ProjectileRunner {
    /// Runs every live projectile, in entity order, then sets off the
    /// step's end blasts in the order their projectiles ended.
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
        let order = order_of(world);
        self.projectiles.sort_by_key(|&entity| order.key(entity));
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
        for (entity, end, centre) in self.ends.drain(..) {
            set_off(world, entity, &end, centre);
        }
    }

    /// One projectile's step: its hits up to the first wall it does not
    /// pass or its range, the walls it passed before its end, then its end,
    /// its bounce or its full travel; an end's blast kept for the step's
    /// ends.
    fn fly(&mut self, world: &mut World, grid: &HitGrid, entity: Entity) {
        self.passed.clear();
        let (shape, offset) = match world.get::<Hitbox>(entity) {
            Some(hitbox) => (hitbox.shape().shape(), hitbox.offset()),
            None => world
                .get::<Projectile>(entity)
                .and_then(|projectile| projectile.shape)
                .map_or(
                    (Shape::Circle { radius: 0.0 }, Vec2::ZERO),
                    |(shape, offset)| (shape.shape(), offset),
                ),
        };
        let Some(sweep) = sweep_of(world, entity, shape, offset) else {
            return;
        };
        let length = (sweep.end - sweep.start).length();
        let mut until = 1.0_f32;
        let mut stop = None;
        if length > 0.0 {
            let pass = world.get::<Projectile>(entity).unwrap().pass_layers;
            if let Some((t, wall, frame)) = self.wall(world, entity, &sweep, length, pass) {
                until = t;
                stop = Some(Stop::Wall(wall, frame));
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
        let end = if let Some(t) = ended {
            finish(world, entity, &sweep, offset, length, t, None)
        } else {
            match stop {
                Some(Stop::Wall(wall, frame)) if turns(world, entity, wall.layers) => {
                    bounce(world, entity, &sweep, offset, length, until, wall, frame);
                    None
                }
                Some(Stop::Wall(wall, _)) => {
                    finish(world, entity, &sweep, offset, length, until, Some(wall))
                }
                Some(Stop::Range) => finish(world, entity, &sweep, offset, length, until, None),
                None => {
                    world.get_mut::<Projectile>(entity).unwrap().travelled += length;
                    None
                }
            }
        };
        if let Some((blast, centre)) = end {
            self.ends.push((entity, blast, centre));
        }
    }

    /// The first wall the sweep meets outside `pass`, the projectile's pass
    /// layers: a tile in none of them by a cast, or a static collider, or a
    /// moving body in its body walls but its source's over that body's own
    /// move, by an overlap of the sweep's bounds and the swept test; the
    /// earliest, with its contact and the wall's velocity. The walls in
    /// `pass` it meets at or before that one, or over the whole sweep, go to
    /// `passed` in sweep order; one it starts inside is behind it, and of
    /// tiles only where it enters them from open space, a cast over the
    /// tiles in a pass layer.
    fn wall(
        &mut self,
        world: &World,
        entity: Entity,
        sweep: &Sweep,
        length: f32,
        pass: u32,
    ) -> Option<(f32, WallContact, Vec2)> {
        let motion = sweep.end - sweep.start;
        let tiles = SpatialFilter::new().with_bodies(false);
        let tile_contact = |hit: SpatialHit| {
            (
                (hit.distance / length).clamp(0.0, 1.0),
                WallContact::new(None, hit.normal, hit.layers),
            )
        };
        let stops = tiles.without_layers(pass);
        let mut first = cast_shape(world, sweep.shape, sweep.start, motion, length, &stops)
            .map(tile_contact)
            .map(|(t, wall)| (t, wall, Vec2::ZERO));
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
        let body_walls = world
            .get::<Projectile>(entity)
            .map_or(0, |projectile| projectile.body_walls);
        let source = world.get::<Source>(entity).map(|source| source.entity());
        for &wall in &self.walls {
            let Some(wall) = wall else {
                continue;
            };
            let body = world.get::<RigidBody>(wall);
            let moving = body.is_some_and(|body| body.kind != BodyKind::Static);
            let layers = body.map_or(RigidBody::DEFAULT_LAYERS, |body| body.layers);
            if moving && (layers & body_walls == 0 || Some(wall) == source) {
                continue;
            }
            let (Some(collider), Some(position)) =
                (world.get::<Collider>(wall), world.get::<Position>(wall))
            else {
                continue;
            };
            let at = position.0 + collider.offset;
            // A body wall over its step's move, in its frame; a static one
            // where it stands.
            let (from, frame) = if moving {
                (
                    world
                        .get::<PrevPosition>(wall)
                        .map_or(at, |prev| prev.0 + collider.offset),
                    world.get::<Velocity>(wall).map_or(Vec2::ZERO, |v| v.0),
                )
            } else {
                (at, Vec2::ZERO)
            };
            let face = Sweep {
                shape: collider.shape,
                start: from,
                end: at,
            };
            if layers & pass != 0 {
                let still = Sweep {
                    end: sweep.start,
                    ..*sweep
                };
                let standing = Sweep {
                    end: face.start,
                    ..face
                };
                if first_contact(&still, &standing).is_none()
                    && let Some((t, normal)) = wall_contact(sweep, &face)
                {
                    self.passed
                        .push((t, WallContact::new(Some(wall), normal, layers)));
                }
            } else if let Some((t, normal)) = wall_contact(sweep, &face)
                && first.is_none_or(|(earliest, ..)| t < earliest)
            {
                first = Some((t, WallContact::new(Some(wall), normal, layers), frame));
            }
        }
        if !self.passed.is_empty() {
            let until = first.map_or(1.0, |(t, ..)| t);
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
#[allow(clippy::too_many_arguments)]
fn bounce(
    world: &mut World,
    entity: Entity,
    sweep: &Sweep,
    offset: Vec2,
    length: f32,
    t: f32,
    wall: WallContact,
    frame: Vec2,
) {
    let point = sweep.at(t) - offset;
    let normal = wall.normal;
    // Reflected in the wall's frame: a static wall's is at rest.
    let turn = |velocity: Vec2| {
        if frame == Vec2::ZERO {
            reflect(velocity, normal)
        } else {
            reflect(velocity - frame, normal) + frame
        }
    };
    place(world, entity, point);
    if let Some(motion) = world.get_mut::<LinearMotion>(entity) {
        let turned = turn(motion.velocity());
        motion.set_velocity(turned);
    }
    if let Some(steering) = world.get_mut::<Steering>(entity) {
        let velocity = steering.velocity_mut();
        *velocity = turn(*velocity);
    }
    if let Some(velocity) = world.get_mut::<Velocity>(entity) {
        velocity.0 = turn(velocity.0);
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
/// traced, and listed for the kit's end-of-step despawn; its end's blast,
/// taken, and the sweep's centre there.
fn finish(
    world: &mut World,
    entity: Entity,
    sweep: &Sweep,
    offset: Vec2,
    length: f32,
    t: f32,
    wall: Option<WallContact>,
) -> Option<(Box<Blast>, Vec2)> {
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
    let goes_off = projectile.end.take();
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
    goes_off.map(|ProjectileEnd::Blast(blast)| (blast, sweep.at(t)))
}

/// Sets `def` off at `centre` through [`blast`], from the projectile, which
/// it neither hurts nor pushes, credited to its [`Source`] while that lives
/// and, holding [`Health`](crate::Health), is not dead, else to nobody, and
/// to itself only without a source; traced as skipped in a World without
/// chemistry's blasts.
fn set_off(world: &mut World, entity: Entity, def: &Blast, centre: Vec2) {
    if !world.has_resource::<Blasts>() {
        trace(world, entity, || TraceKind::Skipped {
            event: "expired",
            action: "blast",
            why: "the World has no ChemistryPlugin",
        });
        return;
    }
    let credit = match world.get::<Source>(entity) {
        Some(source) => Some(source.entity()).filter(|&source| credits(world, source)),
        None => Some(entity),
    };
    blast(world, centre, def, credit, Some(entity));
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
