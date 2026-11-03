//! The blast (redo 7.10b, K10): an output that hurts, pushes and sets off
//! what lies around its receiver, each by a linear falloff from its point.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{SpatialFilter, apply_radial_impulse};
use tungsten_core::{Entity, MissingAsset, World};

use crate::chemistry::queue::{Chemistry, Receiver, Settings, Stimulus};
use crate::chemistry::stimuli::Reach;
use crate::damage::health::Health;
use crate::damage::pipeline::{DamagePart, DamageRequest};
use crate::foundation::tags::{STIMULUS_BLAST, Tags, check_declared, valid_tag};
use crate::overlap::hit::DamageDef;

/// A `blast` as data writes it: its radius, the damage it deals at its
/// centre, the context its hits carry, the knockback and the physics
/// impulse at its centre, each falling off linearly to 0 at the radius.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "BlastFields")]
pub(crate) struct BlastDef {
    radius: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    damage: Option<DamageDef>,
    #[serde(default)]
    context: Vec<String>,
    #[serde(default)]
    push: f32,
    #[serde(default)]
    impulse: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlastFields {
    radius: f32,
    #[serde(default)]
    damage: Option<DamageDef>,
    #[serde(default)]
    context: Vec<String>,
    #[serde(default)]
    push: f32,
    #[serde(default)]
    impulse: f32,
}

impl TryFrom<BlastFields> for BlastDef {
    type Error = String;

    fn try_from(fields: BlastFields) -> Result<Self, Self::Error> {
        if !(fields.radius.is_finite() && fields.radius > 0.0) {
            return Err(format!(
                "radius: must be finite and > 0 (got {})",
                fields.radius
            ));
        }
        for (field, value) in [("push", fields.push), ("impulse", fields.impulse)] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        if let Some(tag) = fields.context.iter().find(|tag| !valid_tag(tag)) {
            return Err(format!(
                "context: `{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
            ));
        }
        Ok(Self {
            radius: fields.radius,
            damage: fields.damage,
            context: fields.context,
            push: fields.push,
            impulse: fields.impulse,
        })
    }
}

impl BlastDef {
    /// The load's check at `field`: the damage's types and the context's
    /// tags.
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        if let Some(damage) = &self.damage {
            damage.check(world)?;
        }
        for (i, tag) in self.context.iter().enumerate() {
            check_declared(world, &format!("{field}.context.{i}"), tag)?;
        }
        Ok(())
    }

    /// Queues the blast at `centre` for chemistry's run to set off once its
    /// reactions are done: from `from`, credited to `credit`, `depth`
    /// reactions deep.
    pub(crate) fn queue(
        &self,
        world: &mut World,
        centre: Vec2,
        from: Receiver,
        credit: Option<Entity>,
        depth: u8,
    ) {
        if let Some(blasts) = world.get_resource_mut::<Blasts>() {
            blasts.0.push(Blast {
                def: self.clone(),
                centre,
                from,
                credit,
                depth,
            });
        }
    }
}

/// The blasts a run's reactions set off, which the run detonates once they
/// are done: a World resource [`ChemistryPlugin`](crate::ChemistryPlugin)
/// inserts and no save holds, since a run makes and takes them.
#[derive(Debug, Default)]
pub(crate) struct Blasts(pub(crate) Vec<Blast>);

/// A blast waiting for the end of chemistry's run.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Blast {
    def: BlastDef,
    centre: Vec2,
    /// Its receiver, which it neither hurts, pushes nor sets off.
    from: Receiver,
    credit: Option<Entity>,
    depth: u8,
}

/// `1 − d / r`, in `[0, 1]`.
fn falloff(distance: f32, radius: f32) -> f32 {
    (1.0 - distance / radius).clamp(0.0, 1.0)
}

/// The damage request of `def`'s damage at `falloff` on `target`, from
/// `centre`: its parts scaled, in its context, pushing `push × falloff`
/// away from the centre (straight up where they coincide); `None` when it
/// deals nothing.
fn hurt(
    def: &BlastDef,
    target: Entity,
    at: Vec2,
    centre: Vec2,
    scale: f32,
) -> Option<DamageRequest> {
    let damage = def.damage.as_ref()?;
    let request = if let Some(parts) = damage.parts() {
        let parts: Vec<DamagePart> = parts
            .iter()
            .map(|part| DamagePart::new(part.amount() * scale).with_tags(part.tags().clone()))
            .collect();
        if parts.iter().all(|part| part.amount() <= 0.0) {
            return None;
        }
        DamageRequest::from_parts(target, parts)
    } else {
        let amount = damage.total() * scale;
        if amount <= 0.0 {
            return None;
        }
        DamageRequest::new(target, amount)
    };
    let mut request = request.with_context(Tags::new(def.context.iter().cloned()));
    if def.push > 0.0 {
        let away = (at - centre).normalize_or(Vec2::NEG_Y);
        request = request.with_knockback(away * def.push * scale);
    }
    Some(request)
}

/// Sets off `blasts`, in order: each entity with `Health` within the radius
/// but the blast's receiver, measured as receivers are, takes the damage by
/// the falloff, credited to the blast's credit, at its depth, for the next
/// step's resolve; dynamic bodies and kinematic ones with a mass take the
/// physics impulse ([`apply_radial_impulse`]); each tagged entity and each
/// material cell within the radius but the receiver takes `stimulus.blast`
/// of its falloff at the blast's depth, for run `run`.
pub(crate) fn detonate(
    world: &mut World,
    settings: &Settings,
    blasts: &mut Vec<Blast>,
    reach: &mut Reach,
    run: u64,
) {
    for blast in blasts.drain(..) {
        let Blast {
            def,
            centre,
            from,
            credit,
            depth,
        } = blast;
        let radius = def.radius;
        reach.gather(world, centre, radius, Some(&from));
        let mut requests: Vec<DamageRequest> = Vec::new();
        let mut stimuli: Vec<(Receiver, f32)> = Vec::new();
        for candidate in reach.entities() {
            let scale = falloff(candidate.distance(centre), radius);
            if world.has::<Health>(candidate.entity)
                && scale > 0.0
                && let Some(request) = hurt(&def, candidate.entity, candidate.centre, centre, scale)
            {
                let request = match credit {
                    Some(credit) => request.with_source(credit),
                    None => request,
                };
                requests.push(request.with_depth(depth));
            }
            if candidate.tagged {
                stimuli.push((Receiver::Entity(candidate.entity), scale));
            }
        }
        for cell in reach.cells() {
            let scale = cell
                .centre_in(world)
                .map_or(0.0, |at| falloff((at - centre).length(), radius));
            stimuli.push((Receiver::Cell(cell.clone()), scale));
        }
        for request in requests {
            request.queue(world);
        }
        if def.impulse > 0.0 {
            let filter = match from {
                Receiver::Entity(receiver) => SpatialFilter::new().excluding(receiver),
                Receiver::Cell(_) => SpatialFilter::new(),
            };
            apply_radial_impulse(world, centre, radius, def.impulse, &filter);
        }
        let chemistry = world.resource_mut::<Chemistry>();
        let id = chemistry.intern(STIMULUS_BLAST);
        for (receiver, scale) in stimuli {
            let stimulus = Stimulus {
                amount: scale,
                status_effect: 1.0,
                dot_base: scale,
                depth,
                credit,
            };
            chemistry.stimulate_id(settings, receiver, id, stimulus, run);
        }
    }
}
