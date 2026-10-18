//! The kit's end-of-step despawn: the entities a step's items ended, each
//! marked with its descendants and queued for a recursive despawn after the
//! step's spawns, so a cue spawn from an entity that ends in the same step
//! survives the flush.

use tungsten_core::{CommandBuffer, Entity, Schedule, World, system};

use crate::foundation::phases::Phase;
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::children::SPAWN_CHILDREN;
use crate::lifecycle::spawner::SPAWNER_TICK;
use crate::react::cues::CUES;
use crate::transform::hierarchy::{Children, Parent, despawn_recursive};

/// The kit's one end-of-step removal, in the react phase after the
/// spawner, the child spawns and the cues when each is registered: it
/// despawns, with their children, the entities the step's items ended (a
/// death past its delay, an ended projectile, a collected pickup, a
/// finished lifetime, an entity outside its region), after the step's
/// spawns from them. [`HealthPlugin`](crate::HealthPlugin),
/// [`LifetimePlugin`](crate::LifetimePlugin),
/// [`DespawnOutsidePlugin`](crate::DespawnOutsidePlugin),
/// [`ProjectilePlugin`](crate::ProjectilePlugin) and
/// [`PickupPlugin`](crate::PickupPlugin) each add it when the schedule lacks
/// it.
pub const KIT_DESPAWN: &str = "kit_despawn";

/// Marks an entity the kit queued for despawn this frame, with every
/// descendant under it: the frame's later steps react to it no more, a
/// second despawn of it queues nothing, and the flush that despawns it
/// takes the mark with it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Despawning;

/// The entities the step's items ended, with the item that ended each, a
/// World resource that [`kit_despawn`] drains.
#[derive(Debug, Default)]
pub(crate) struct Ended(Vec<(Entity, &'static str)>);

/// Lists `entity` for this step's [`KIT_DESPAWN`], as `by` ended it; a
/// World without the list, whose plugins added no despawn, lists nothing.
pub(crate) fn end(world: &mut World, entity: Entity, by: &'static str) {
    if let Some(ended) = world.get_resource_mut::<Ended>() {
        ended.0.push((entity, by));
    }
}

/// Inserts the ended list when the World has none and adds [`KIT_DESPAWN`]
/// when the schedule lacks it: every plugin whose item ends an entity calls
/// it in `build`.
pub(crate) fn ensure_kit_despawn(schedule: &mut Schedule, world: &mut World) {
    if !world.has_resource::<Ended>() {
        world.insert_resource(Ended::default());
    }
    if schedule.contains(KIT_DESPAWN) {
        return;
    }
    let mut runner = Runner::default();
    schedule.add(
        Phase::React.stage(),
        Phase::React
            .join(system(KIT_DESPAWN, move |world: &mut World| {
                runner.run(world);
            }))
            .after_if_present(SPAWNER_TICK)
            .after_if_present(SPAWN_CHILDREN)
            .after_if_present(CUES),
    );
}

/// The despawn's state between steps: the drained list and the walk's
/// stack, kept so a step allocates nothing once warm.
#[derive(Default)]
struct Runner {
    ended: Vec<(Entity, &'static str)>,
    stack: Vec<Entity>,
}

impl Runner {
    /// Marks and queues each listed entity that is alive and unmarked, in
    /// list order, and traces it once. Without a `CommandBuffer` it queues
    /// nothing, and the items list their ends again on the next step.
    fn run(&mut self, world: &mut World) {
        let Some(ended) = world.get_resource_mut::<Ended>() else {
            return;
        };
        if ended.0.is_empty() {
            return;
        }
        std::mem::swap(&mut self.ended, &mut ended.0);
        if world.has_resource::<CommandBuffer>() {
            for &(entity, by) in &self.ended {
                if !world.is_alive(entity) || world.has::<Despawning>(entity) {
                    continue;
                }
                mark_despawning(world, entity, &mut self.stack);
                despawn_recursive(world.resource_mut::<CommandBuffer>(), entity);
                trace(world, entity, || TraceKind::Despawn { by });
            }
        }
        self.ended.clear();
    }
}

/// Marks `root` and every live descendant that [`despawn_recursive`] takes
/// with it [`Despawning`], through `stack`.
pub(crate) fn mark_despawning(world: &mut World, root: Entity, stack: &mut Vec<Entity>) {
    stack.clear();
    stack.push(root);
    while let Some(entity) = stack.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter().filter(|&child| {
                world.is_alive(child)
                    && world
                        .get::<Parent>(child)
                        .is_some_and(|parent| parent.entity() == entity)
            }));
        }
        world.insert(entity, Despawning);
    }
}

#[cfg(test)]
#[path = "../tests/ended.rs"]
mod tests;
