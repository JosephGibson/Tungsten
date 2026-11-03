//! `become` (redo 7.10b): a receiver replaced by another prefab under the
//! same save identity (`CommandBuffer::replace_prefab`), and turned back
//! after a while by [`Became`], which `chemistry_tick` counts.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::{CommandBuffer, Entity, MissingAsset, World};

use crate::chemistry::outputs::point_of;
use crate::count_down;
use crate::foundation::saved::order_of;
use crate::foundation::source::Source;
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::ended::Despawning;

/// The name a save holds [`Became`] under.
pub(crate) const BECAME: &str = "became";

/// Why a `become` did not run, for the trace.
pub(crate) const NO_PREFAB: &str = "the prefab did not prepare";

/// A replacement that turns back: the prefab it becomes when `left` game
/// seconds run out, under the same identity. A component a `become` with a
/// `revert` gives its replacement, saved as `became`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Became {
    pub(crate) to: String,
    pub(crate) left: f32,
}

/// A `become` as data writes it: the prefab the receiver becomes, and the
/// revert that turns it back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BecomeDef {
    prefab: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revert: Option<RevertDef>,
}

/// A `become`'s revert: after `after` seconds, the prefab `to`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RevertFields")]
pub(crate) struct RevertDef {
    after: f32,
    to: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevertFields {
    after: f32,
    to: String,
}

impl TryFrom<RevertFields> for RevertDef {
    type Error = String;

    fn try_from(fields: RevertFields) -> Result<Self, Self::Error> {
        if !(fields.after.is_finite() && fields.after > 0.0) {
            return Err(format!(
                "after: must be finite and > 0 (got {})",
                fields.after
            ));
        }
        Ok(Self {
            after: fields.after,
            to: fields.to,
        })
    }
}

impl BecomeDef {
    /// The load's check at `field`: both prefabs.
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        world.check_prefab_id(&format!("{field}.prefab"), &self.prefab)?;
        if let Some(revert) = &self.revert {
            world.check_prefab_id(&format!("{field}.revert.to"), &revert.to)?;
        }
        Ok(())
    }

    /// Runs the `become` on `receiver`: [`replace`] with its prefab, its
    /// replacement holding [`Became`] when it reverts.
    pub(crate) fn run(&self, world: &mut World, receiver: Entity) -> Result<(), &'static str> {
        let became = self.revert.as_ref().map(|revert| Became {
            to: revert.to.clone(),
            left: revert.after,
        });
        replace(world, receiver, &self.prefab, became)
    }
}

/// Queues `prefab` at `receiver`'s point to take its place under its save
/// identity: the replacement keeps the receiver's `Source` when it has one
/// (a `with` bundle, which the spawn inheritance leaves) and otherwise
/// takes the inheritance's, naming the receiver; with `became`, it holds
/// it. Statuses are not carried.
pub(crate) fn replace(
    world: &mut World,
    receiver: Entity,
    prefab: &str,
    became: Option<Became>,
) -> Result<(), &'static str> {
    if !world.has_resource::<CommandBuffer>() {
        return Err(NO_PREFAB);
    }
    let at = point_of(world, receiver).unwrap_or(Vec2::ZERO);
    let mut prepared = match world.prepare_prefab(prefab, at) {
        Ok(prepared) => prepared,
        Err(error) => {
            trace(world, receiver, || TraceKind::SpawnFailed {
                prefab: prefab.to_owned(),
                by: "interaction",
                error: error.to_string(),
            });
            return Err(NO_PREFAB);
        }
    };
    if let Some(source) = world.get::<Source>(receiver).copied() {
        prepared = prepared.with((source,));
    }
    if let Some(became) = became {
        prepared = prepared.with((became,));
    }
    world
        .resource_mut::<CommandBuffer>()
        .replace_prefab(receiver, prepared);
    trace(world, receiver, || TraceKind::Spawn {
        prefab: prefab.to_owned(),
        by: "interaction",
    });
    Ok(())
}

/// [`chemistry_tick`](crate::CHEMISTRY_TICK)'s work on [`Became`]: each
/// counts down by `dt`, and one run out becomes its prefab under the same
/// identity, in save order, its `Became` taken off so a second step before
/// the flush does not queue it again. `due` is a kept list.
pub(crate) fn tick(world: &mut World, dt: f32, due: &mut Vec<(Entity, String)>) {
    due.clear();
    if dt <= 0.0 {
        return;
    }
    for (entity, became) in world.query_mut::<(Entity, &mut Became)>() {
        became.left = count_down(became.left, dt);
        if became.left <= 0.0 {
            due.push((entity, became.to.clone()));
        }
    }
    if due.is_empty() {
        return;
    }
    let order = order_of(world);
    due.sort_by_key(|(entity, _)| order.key(*entity));
    for (entity, to) in due.drain(..) {
        world.remove_component::<Became>(entity);
        if world.has::<Despawning>(entity) {
            continue;
        }
        let _ = replace(world, entity, &to, None);
    }
}
