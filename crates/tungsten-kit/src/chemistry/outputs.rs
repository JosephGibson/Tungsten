//! Chemistry's outputs: what a rule, a status's end or a band's entry does
//! to its receiver, a fixed list run in order.

use std::rc::Rc;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::Position;
use tungsten_core::{CommandBuffer, DataRef, Entity, MissingAsset, Transform, World};

use crate::damage::effects::{Effect, EffectRequest, StatModifierDef, consume, end_effect_at};
use crate::damage::pipeline::DamageRequest;
use crate::foundation::lines::Line;
use crate::foundation::trace::{TraceKind, trace};
use crate::overlap::hit::DamageDef;

/// The event name an output's trace line carries.
const INTERACTION: &str = "interaction";

/// Why an output did not run, for the trace.
pub(crate) const DEAD: &str = "the receiver is dead: a status is refused";
const NO_STATUS: &str = "the receiver carries no such status";
const NO_HANDLER: &str = "no handler of that name is registered";
const NO_SPAWN: &str = "the prefab did not prepare";

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
/// | handler | `{ "handler": "douse" }` | runs a game's Rust handler ([`ChemistryPlugin::with_handler`](crate::ChemistryPlugin::with_handler)) |
///
/// On a dead receiver `build` and `apply` are refused, as effects refuse the
/// dead, and the rest run. The load checks each status, prefab, damage type,
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
    Handler(String),
}

impl Output {
    /// Its name, as data writes it: `build`, `apply`, `remove`, `consume`,
    /// `damage`, `spawn` or `handler`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match &self.0 {
            Kind::Build(_) => "build",
            Kind::Apply(_) => "apply",
            Kind::Remove(_) => "remove",
            Kind::Consume(_) => "consume",
            Kind::Damage(_) => "damage",
            Kind::Spawn(_) => "spawn",
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

/// What an output runs for, which a handler reads: the receiver, what
/// reached it, the rule, the amounts, the credit and the depth.
#[derive(Debug, Clone, PartialEq)]
pub struct Reaction {
    pub(crate) receiver: Entity,
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
    /// The entity it runs on.
    #[must_use]
    pub fn receiver(&self) -> Entity {
        self.receiver
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
/// under `label.<index>`; on a `dead` receiver `build` and `apply` are
/// refused.
pub(crate) fn run_outputs(
    world: &mut World,
    reaction: &Reaction,
    outputs: &[Output],
    label: &str,
    dead: bool,
) {
    let receiver = reaction.receiver;
    // What the outputs set off runs one reaction deeper.
    let deeper = reaction.depth.saturating_add(1);
    for (index, output) in outputs.iter().enumerate() {
        let ran = match &output.0 {
            Kind::Build(_) | Kind::Apply(_) if dead => Err(DEAD),
            Kind::Build(def) => {
                // A fixed build at the entry's largest status effect; a
                // scaled one of the hits, each at its own.
                let (amount, status_effect) = if let Some(amount) = def.amount {
                    (amount, reaction.status_effect)
                } else {
                    let scale = f64::from(def.scale.unwrap_or(1.0));
                    let amount = (scale * reaction.weighted).min(f64::from(f32::MAX)) as f32;
                    (amount, 1.0)
                };
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
                    let rate = (f64::from(dot) * f64::from(reaction.dot_base))
                        .min(f64::from(f32::MAX)) as f32;
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
        };
        match ran {
            Ok(()) => trace(world, receiver, || TraceKind::Cue {
                event: INTERACTION,
                action: output.name(),
                rule: Some(format!("{label}.{index}")),
            }),
            Err(why) => trace(world, receiver, || TraceKind::Skipped {
                event: INTERACTION,
                action: output.name(),
                why,
            }),
        }
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
