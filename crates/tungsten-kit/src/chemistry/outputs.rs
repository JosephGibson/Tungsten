//! Chemistry's outputs: what a rule, a status's end or a band's entry does
//! to its receiver, a fixed list run in order.

use std::rc::Rc;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::Position;
use tungsten_core::{CommandBuffer, DataRef, Entity, MissingAsset, Transform, World};

use crate::chemistry::becoming::BecomeDef;
use crate::chemistry::blast::BlastDef;
use crate::chemistry::grid::locate;
use crate::chemistry::queue::Receiver;
use crate::chemistry::rules::check_stimulus;
use crate::chemistry::stimuli::{Named, emit_from};
use crate::chemistry::tiles::{self, Cell, NOT_A_TILE};
use crate::damage::effects::{Effect, EffectRequest, StatModifierDef, consume, end_effect_at};
use crate::damage::pipeline::DamageRequest;
use crate::foundation::lines::Line;
use crate::foundation::tags::valid_tag;
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::cap::admit;
use crate::overlap::hit::DamageDef;

/// The event name an output's trace line carries.
const INTERACTION: &str = "interaction";

/// Why an output did not run, for the trace.
pub(crate) const DEAD: &str = "the receiver is dead: a status is refused";
const NO_STATUS: &str = "the receiver carries no such status";
const NO_HANDLER: &str = "no handler of that name is registered";
const NO_SPAWN: &str = "the prefab did not prepare";
const NOT_AN_ENTITY: &str = "the receiver is a tile cell, not an entity";

/// One output of a rule, a status's `on_end` or a band's `enter`, run on
/// the receiver. Data form, an object of one field:
///
/// | Output | Data | Does |
/// | --- | --- | --- |
/// | build | `{ "build": { "status": "burning", "amount": 1 } }` or `{ "scale": 0.5 }` | builds a status by a fixed amount at the entry's largest status effect, or by `scale` times the entry's hits, each at its own status effect (1 by default) |
/// | apply | `{ "apply": { "status": "burning", "dot": 0.25, "lines": [..] } }` | applies a status at the entry's largest status effect; `dot` fixes its damage-over-time rate at `dot` times the hit's base a second, `lines` go on its instance |
/// | remove | `{ "remove": "burning" }` | ends the status's instance |
/// | consume | `{ "consume": { "status": "burning", "amount": 1 } }` | takes intensity off; at 0 it has run out |
/// | damage | `{ "damage": { "elemental.fire": 2 } }` | queues typed damage on the receiver from the credit, for the next step's resolve |
/// | spawn | `{ "spawn": "puff" }` | spawns a prefab at the receiver's point, from it |
/// | emit | `{ "emit": { "stimulus": "heat", "amount": 1, "radius": 40 } }` | sends a stimulus once from the receiver's point to every receiver within the radius but itself, at the next run |
/// | set_tile | `{ "set_tile": { "tile": "ash", "revert": 5 } }` | on a tile cell, writes the first tileset entry with that sprite through the registry, every placement of the map showing it; `revert` writes the old tile back after that many seconds |
/// | become | `{ "become": { "prefab": "lamp_doused", "revert": { "after": 3, "to": "lamp" } } }` | replaces the receiver by a prefab at its point under its save identity, keeping its source, its statuses not carried; `revert` turns it into `to` after `after` seconds, under the same identity |
/// | blast | `{ "blast": { "radius": 80, "damage": { "elemental.fire": 8 }, "context": ["hit.blast"], "push": 600, "impulse": 300 } }` | once the run's reactions are done, each entity with health within the radius but the receiver takes the damage and a knockback away from the centre, dynamic bodies the impulse, and each receiver and cell within it `stimulus.blast`, each by `1 − d / r`, one reaction deeper |
/// | handler | `{ "handler": "douse" }` | runs a game's Rust handler ([`ChemistryPlugin::with_handler`](crate::ChemistryPlugin::with_handler)) |
///
/// On a dead receiver `build` and `apply` are refused, as effects refuse the
/// dead, and the rest run. On a tile cell the status outputs keep its
/// statuses, `damage` damages it by the parts' sum through
/// `TilemapRegistry::damage_tile`, `emit` and `spawn` go from its centre
/// (the spawn with no source), and `set_tile` writes it; `set_tile` on an
/// entity and `become` on a cell do nothing. The load checks each status, prefab, damage type,
/// tag and handler.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Output;
///
/// let output: Output = serde_json::from_str(r#"{ "remove": "burning" }"#).unwrap();
/// assert_eq!(output.name(), "remove");
/// assert!(serde_json::from_str::<Output>(r#"{ "remove": "a", "spawn": "b" }"#).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Output(Kind);

/// An output's kinds, as data names them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Build(BuildDef),
    Apply(ApplyDef),
    Remove(DataRef<Effect>),
    Consume(ConsumeDef),
    Damage(DamageDef),
    Spawn(String),
    Emit(EmitDef),
    SetTile(SetTileDef),
    Become(BecomeDef),
    Blast(BlastDef),
    Handler(String),
}

impl Output {
    /// Its name, as data writes it: `build`, `apply`, `remove`, `consume`,
    /// `damage`, `spawn`, `emit`, `set_tile`, `become`, `blast` or
    /// `handler`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match &self.0 {
            Kind::Build(_) => "build",
            Kind::Apply(_) => "apply",
            Kind::Remove(_) => "remove",
            Kind::Consume(_) => "consume",
            Kind::Damage(_) => "damage",
            Kind::Spawn(_) => "spawn",
            Kind::Emit(_) => "emit",
            Kind::SetTile(_) => "set_tile",
            Kind::Become(_) => "become",
            Kind::Blast(_) => "blast",
            Kind::Handler(_) => "handler",
        }
    }
}

/// A `build` as data writes it: a fixed amount, or a scale on the entry's
/// hits (1 when neither is given).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "BuildFields")]
pub(crate) struct BuildDef {
    status: DataRef<Effect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scale: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildFields {
    status: DataRef<Effect>,
    #[serde(default)]
    amount: Option<f32>,
    #[serde(default)]
    scale: Option<f32>,
}

impl TryFrom<BuildFields> for BuildDef {
    type Error = String;

    fn try_from(fields: BuildFields) -> Result<Self, Self::Error> {
        for (field, value) in [("amount", fields.amount), ("scale", fields.scale)] {
            if let Some(value) = value
                && !(value.is_finite() && value >= 0.0)
            {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        if fields.amount.is_some() && fields.scale.is_some() {
            return Err("a build takes `amount` or `scale`, not both".into());
        }
        Ok(Self {
            status: fields.status,
            amount: fields.amount,
            scale: fields.scale,
        })
    }
}

/// An `apply` as data writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "ApplyFields")]
pub(crate) struct ApplyDef {
    status: DataRef<Effect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dot: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lines: Vec<StatModifierDef>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyFields {
    status: DataRef<Effect>,
    #[serde(default)]
    dot: Option<f32>,
    #[serde(default)]
    lines: Vec<StatModifierDef>,
}

impl TryFrom<ApplyFields> for ApplyDef {
    type Error = String;

    fn try_from(fields: ApplyFields) -> Result<Self, Self::Error> {
        if let Some(dot) = fields.dot
            && !(dot.is_finite() && dot >= 0.0)
        {
            return Err(format!("dot: must be finite and >= 0 (got {dot})"));
        }
        Ok(Self {
            status: fields.status,
            dot: fields.dot,
            lines: fields.lines,
        })
    }
}

/// A `consume` as data writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "ConsumeFields")]
pub(crate) struct ConsumeDef {
    status: DataRef<Effect>,
    amount: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsumeFields {
    status: DataRef<Effect>,
    amount: f32,
}

impl TryFrom<ConsumeFields> for ConsumeDef {
    type Error = String;

    fn try_from(fields: ConsumeFields) -> Result<Self, Self::Error> {
        if !(fields.amount.is_finite() && fields.amount > 0.0) {
            return Err(format!(
                "amount: must be finite and > 0 (got {})",
                fields.amount
            ));
        }
        Ok(Self {
            status: fields.status,
            amount: fields.amount,
        })
    }
}

/// An `emit` as data writes it: a stimulus sent once to the receivers in a
/// radius.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "EmitFields")]
pub(crate) struct EmitDef {
    stimulus: String,
    amount: f32,
    radius: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmitFields {
    stimulus: String,
    amount: f32,
    radius: f32,
}

impl TryFrom<EmitFields> for EmitDef {
    type Error = String;

    fn try_from(fields: EmitFields) -> Result<Self, Self::Error> {
        if !valid_tag(&fields.stimulus) {
            return Err(format!(
                "stimulus: `{}` is not a stimulus name (`[a-z][a-z0-9_]*` segments joined by dots)",
                fields.stimulus
            ));
        }
        for (field, value) in [("amount", fields.amount), ("radius", fields.radius)] {
            if !(value.is_finite() && value > 0.0) {
                return Err(format!("{field}: must be finite and > 0 (got {value})"));
            }
        }
        Ok(Self {
            stimulus: fields.stimulus,
            amount: fields.amount,
            radius: fields.radius,
        })
    }
}

/// A `set_tile` as data writes it: the sprite the cell takes, and the
/// seconds after which its old tile comes back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "SetTileFields")]
pub(crate) struct SetTileDef {
    tile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revert: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetTileFields {
    tile: String,
    #[serde(default)]
    revert: Option<f32>,
}

impl TryFrom<SetTileFields> for SetTileDef {
    type Error = String;

    fn try_from(fields: SetTileFields) -> Result<Self, Self::Error> {
        if fields.tile.is_empty() {
            return Err("tile: must name a sprite".into());
        }
        if let Some(revert) = fields.revert
            && !(revert.is_finite() && revert > 0.0)
        {
            return Err(format!("revert: must be finite and > 0 (got {revert})"));
        }
        Ok(Self {
            tile: fields.tile,
            revert: fields.revert,
        })
    }
}

/// A tile cell: its placement's map ID and top-left, its layer's index, its
/// column and its row; what a reaction runs on ([`Reaction::cell`]) and
/// what [`TileStatuses`](crate::TileStatuses) reads by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileCell<'a> {
    /// The placed map's ID.
    pub map: &'a str,
    /// The placement's top-left.
    pub origin: Vec2,
    /// The layer's index in the map's layers.
    pub layer: usize,
    /// The cell's column, from the map's left.
    pub col: u32,
    /// The cell's row, from the map's top.
    pub row: u32,
}

/// What an output runs for, which a handler reads: the receiver (an entity
/// or a tile cell), what reached it, the rule, the amounts, the credit and
/// the depth.
#[derive(Debug, Clone, PartialEq)]
pub struct Reaction {
    pub(crate) receiver: Receiver,
    pub(crate) stimulus: String,
    pub(crate) rule: String,
    pub(crate) raw: f64,
    pub(crate) weighted: f64,
    pub(crate) status_effect: f32,
    pub(crate) dot_base: f32,
    pub(crate) depth: u8,
    pub(crate) credit: Option<Entity>,
}

impl Reaction {
    /// The entity it runs on; `None` on a tile cell.
    #[must_use]
    pub fn receiver(&self) -> Option<Entity> {
        self.receiver.entity()
    }

    /// The tile cell it runs on; `None` on an entity.
    #[must_use]
    pub fn cell(&self) -> Option<TileCell<'_>> {
        match &self.receiver {
            Receiver::Entity(_) => None,
            Receiver::Cell(cell) => Some(TileCell {
                map: &cell.map,
                origin: cell.origin(),
                layer: cell.layer as usize,
                col: cell.col,
                row: cell.row,
            }),
        }
    }

    /// The stimulus's name; for a status's own list, its label
    /// (`burning#on_end`, `burning#enter.1`).
    #[must_use]
    pub fn stimulus(&self) -> &str {
        &self.stimulus
    }

    /// The rule's entry ID, or the status's label.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// The stimulus's amount: its hits' raw amounts summed.
    #[must_use]
    pub fn amount(&self) -> f32 {
        self.raw as f32
    }

    /// The entity it is credited to: the first hit's source.
    #[must_use]
    pub fn credit(&self) -> Option<Entity> {
        self.credit
    }

    /// How many reactions led to it, from 0.
    #[must_use]
    pub fn depth(&self) -> u8 {
        self.depth
    }
}

/// A game's handler, by name.
pub(crate) type Handler = Rc<dyn Fn(&mut World, &Reaction)>;

/// The registered handlers, a World resource.
#[derive(Default)]
pub(crate) struct Handlers(pub(crate) Vec<(String, Handler)>);

impl Handlers {
    fn get(&self, name: &str) -> Option<Handler> {
        self.0
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, handler)| Rc::clone(handler))
    }
}

/// The load's check of `outputs` at `field`: each status, prefab, damage
/// type, line tag and handler.
pub(crate) fn check_outputs(
    world: &World,
    field: &str,
    outputs: &[Output],
) -> Result<(), MissingAsset> {
    for (i, output) in outputs.iter().enumerate() {
        let at = format!("{field}.{i}.{}", output.name());
        match &output.0 {
            Kind::Build(def) => def.status.check(world, &format!("{at}.status"))?,
            Kind::Apply(def) => {
                def.status.check(world, &format!("{at}.status"))?;
                for (j, line) in def.lines.iter().enumerate() {
                    line.check(world, &format!("{at}.lines.{j}"))?;
                }
            }
            Kind::Remove(status) => status.check(world, &at)?,
            Kind::Consume(def) => def.status.check(world, &format!("{at}.status"))?,
            Kind::Damage(damage) => damage.check(world)?,
            Kind::Spawn(prefab) => world.check_prefab_id(&at, prefab)?,
            Kind::Emit(def) => check_stimulus(world, &format!("{at}.stimulus"), &def.stimulus)?,
            Kind::SetTile(_) => {}
            Kind::Become(def) => def.check(world, &at)?,
            Kind::Blast(def) => def.check(world, &at)?,
            Kind::Handler(name) => {
                let known = world
                    .get_resource::<Handlers>()
                    .is_some_and(|handlers| handlers.get(name).is_some());
                if !known {
                    return Err(MissingAsset::new(&at, "chemistry handler", name));
                }
            }
        }
    }
    Ok(())
}

/// Where `entity` is: its body's `Position`, else its `Transform`.
pub(crate) fn point_of(world: &World, entity: Entity) -> Option<Vec2> {
    world
        .get::<Position>(entity)
        .map(|position| position.0)
        .or_else(|| world.get::<Transform>(entity).map(|t| t.position))
}

/// Runs `outputs` for `reaction` on its receiver, in order, each traced
/// under `label.<index>` about the receiver (a cell's about its placement);
/// on a `dead` receiver `build` and `apply` are refused.
pub(crate) fn run_outputs(
    world: &mut World,
    reaction: &Reaction,
    outputs: &[Output],
    label: &str,
    dead: bool,
) {
    // What the outputs set off runs one reaction deeper.
    let deeper = reaction.depth.saturating_add(1);
    for (index, output) in outputs.iter().enumerate() {
        let ran = match &output.0 {
            Kind::Handler(name) => {
                let handler = world
                    .get_resource::<Handlers>()
                    .and_then(|handlers| handlers.get(name));
                match handler {
                    Some(handler) => {
                        handler(world, reaction);
                        Ok(())
                    }
                    None => Err(NO_HANDLER),
                }
            }
            _ => match &reaction.receiver {
                Receiver::Entity(receiver) => {
                    on_entity(world, reaction, *receiver, output, deeper, dead)
                }
                Receiver::Cell(cell) => on_cell(world, reaction, cell, output, deeper),
            },
        };
        let about = match &reaction.receiver {
            Receiver::Entity(receiver) => Some(*receiver),
            Receiver::Cell(cell) => cell.placement(world),
        };
        let Some(about) = about else {
            continue;
        };
        match ran {
            Ok(()) => trace(world, about, || TraceKind::Cue {
                event: INTERACTION,
                action: output.name(),
                rule: Some(format!("{label}.{index}")),
            }),
            Err(why) => trace(world, about, || TraceKind::Skipped {
                event: INTERACTION,
                action: output.name(),
                why,
            }),
        }
    }
}

/// A fixed build's amount and status effect, or a scaled one's: the
/// entry's largest status effect for a fixed amount, the hits each at
/// their own for a scale.
fn build_of(def: &BuildDef, reaction: &Reaction) -> (f32, f32) {
    if let Some(amount) = def.amount {
        (amount, reaction.status_effect)
    } else {
        let scale = f64::from(def.scale.unwrap_or(1.0));
        let amount = (scale * reaction.weighted).min(f64::from(f32::MAX)) as f32;
        (amount, 1.0)
    }
}

/// One output, not a handler, on the entity `receiver`.
fn on_entity(
    world: &mut World,
    reaction: &Reaction,
    receiver: Entity,
    output: &Output,
    deeper: u8,
    dead: bool,
) -> Result<(), &'static str> {
    match &output.0 {
        Kind::Build(_) | Kind::Apply(_) if dead => Err(DEAD),
        Kind::Build(def) => {
            let (amount, status_effect) = build_of(def, reaction);
            let request = EffectRequest::new(receiver, def.status.clone())
                .with_amount(amount)
                .with_status_effect(status_effect);
            request_from(world, request, reaction.credit, deeper);
            Ok(())
        }
        Kind::Apply(def) => {
            let mut request = EffectRequest::new(receiver, def.status.clone())
                .with_status_effect(reaction.status_effect)
                .with_lines(def.lines.iter().map(StatModifierDef::line));
            if let Some(dot) = def.dot {
                let rate =
                    (f64::from(dot) * f64::from(reaction.dot_base)).min(f64::from(f32::MAX)) as f32;
                request = request.with_rate(rate);
            }
            request_from(world, request, reaction.credit, deeper);
            Ok(())
        }
        Kind::Remove(status) => {
            if end_effect_at(world, receiver, status, deeper) {
                Ok(())
            } else {
                Err(NO_STATUS)
            }
        }
        Kind::Consume(def) => {
            if consume(world, receiver, &def.status, def.amount, deeper) {
                Ok(())
            } else {
                Err(NO_STATUS)
            }
        }
        Kind::Damage(damage) => {
            let mut request = match damage.parts() {
                Some(parts) => DamageRequest::from_parts(receiver, parts),
                None => DamageRequest::new(receiver, damage.total()),
            };
            if let Some(credit) = reaction.credit {
                request = request.with_source(credit);
            }
            request.with_depth(deeper).queue(world);
            Ok(())
        }
        Kind::Spawn(prefab) => spawn_from(world, receiver, prefab),
        Kind::Emit(def) => {
            emit_from(
                world,
                receiver,
                Named::New(def.stimulus.clone()),
                def.amount,
                def.radius,
                reaction.credit,
                deeper,
            );
            Ok(())
        }
        Kind::SetTile(_) => Err(NOT_A_TILE),
        Kind::Become(def) => def.run(world, receiver),
        Kind::Blast(def) => {
            let centre = locate(world, receiver).map_or(Vec2::ZERO, |(centre, _)| centre);
            def.queue(
                world,
                centre,
                Receiver::Entity(receiver),
                reaction.credit,
                deeper,
            );
            Ok(())
        }
        Kind::Handler(_) => unreachable!("handlers run apart"),
    }
}

/// One output, not a handler, on the tile cell `cell`.
fn on_cell(
    world: &mut World,
    reaction: &Reaction,
    cell: &Cell,
    output: &Output,
    deeper: u8,
) -> Result<(), &'static str> {
    match &output.0 {
        Kind::Build(def) => {
            let (amount, status_effect) = build_of(def, reaction);
            tiles::apply(
                world,
                cell,
                &def.status,
                amount,
                status_effect,
                reaction.credit,
                deeper,
            );
            Ok(())
        }
        Kind::Apply(def) => {
            let status_effect = reaction.status_effect;
            tiles::apply(
                world,
                cell,
                &def.status,
                1.0,
                status_effect,
                reaction.credit,
                deeper,
            );
            Ok(())
        }
        Kind::Remove(status) => {
            if tiles::remove(world, cell, status, deeper) {
                Ok(())
            } else {
                Err(NO_STATUS)
            }
        }
        Kind::Consume(def) => {
            if tiles::consume(world, cell, &def.status, def.amount, deeper) {
                Ok(())
            } else {
                Err(NO_STATUS)
            }
        }
        Kind::Damage(damage) => tiles::damage(world, cell, damage.total()),
        Kind::Spawn(prefab) => {
            let at = cell.centre_in(world).ok_or(tiles::NO_MAP)?;
            if !world.has_resource::<CommandBuffer>() {
                return Err(NO_SPAWN);
            }
            let prepared = world.prepare_prefab(prefab, at).map_err(|_| NO_SPAWN)?;
            let prepared = admit(world, prefab, None, prepared)?;
            world.resource_mut::<CommandBuffer>().spawn_prefab(prepared);
            Ok(())
        }
        Kind::Emit(def) => {
            tiles::emit_from_cell(
                world,
                cell,
                Named::New(def.stimulus.clone()),
                def.amount,
                def.radius,
                reaction.credit,
                deeper,
            );
            Ok(())
        }
        Kind::SetTile(def) => tiles::set_tile(world, cell, &def.tile, def.revert),
        Kind::Become(_) => Err(NOT_AN_ENTITY),
        Kind::Blast(def) => {
            let centre = cell.centre_in(world).ok_or(tiles::NO_MAP)?;
            def.queue(
                world,
                centre,
                Receiver::Cell(cell.clone()),
                reaction.credit,
                deeper,
            );
            Ok(())
        }
        Kind::Handler(_) => unreachable!("handlers run apart"),
    }
}

/// Queues `request` from `credit`, its own outputs `depth` deep.
fn request_from(world: &mut World, request: EffectRequest, credit: Option<Entity>, depth: u8) {
    let request = match credit {
        Some(credit) => request.with_source(credit),
        None => request,
    };
    request.at_depth(depth).queue(world);
}

/// Spawns `prefab` at `receiver`'s point, from it.
fn spawn_from(world: &mut World, receiver: Entity, prefab: &str) -> Result<(), &'static str> {
    let at = point_of(world, receiver).unwrap_or(Vec2::ZERO);
    if !world.has_resource::<CommandBuffer>() {
        return Err(NO_SPAWN);
    }
    match world.prepare_prefab(prefab, at) {
        Ok(prepared) => {
            let prepared = admit(world, prefab, Some(receiver), prepared)?;
            world
                .resource_mut::<CommandBuffer>()
                .spawn_prefab_from(receiver, prepared);
            trace(world, receiver, || TraceKind::Spawn {
                prefab: prefab.to_owned(),
                by: INTERACTION,
            });
            Ok(())
        }
        Err(error) => {
            trace(world, receiver, || TraceKind::SpawnFailed {
                prefab: prefab.to_owned(),
                by: INTERACTION,
                error: error.to_string(),
            });
            Err(NO_SPAWN)
        }
    }
}

/// The lines an `apply` writes, as `(stat, line)`.
impl StatModifierDef {
    fn line(&self) -> (String, Line) {
        self.clone().into_line()
    }
}
