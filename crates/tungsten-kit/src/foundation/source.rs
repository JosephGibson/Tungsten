//! The spawn chain's origin, derived at the flush from each spawn's source.

use tungsten_core::{Entity, Inspectable, Plugin, Schedule, World};

use crate::foundation::inspect::register_rows;
use crate::foundation::phases::ensure_phases;

/// The origin of the entity's spawn chain: the entity a spawn from a source
/// (`CommandBuffer::spawn_prefab_from`, as a [`Spawner`](crate::Spawner)
/// spawns) is derived from at the flush, once [`SourcePlugin`] has built
/// (`D-170`). A spawn takes its source's own `Source` when the source has
/// one, else the source itself, so a bolt fired by a turret that a nest
/// spawned names the nest, and credit or an "on source" reaction follows
/// the chain. A game sets one by hand with [`new`](Self::new); no scene
/// form, since a spawn derives it.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use tungsten_core::{CommandBuffer, Plugin, PrefabData, PrefabRegistry, Schedule, Tag, World};
/// use tungsten_kit::{Source, SourcePlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// SourcePlugin.build(&mut schedule, &mut world);
/// let slime: PrefabData = serde_json::from_value(json!({ "tag": "slime" })).unwrap();
/// let mut prefabs = PrefabRegistry::new();
/// prefabs.insert("slime".to_string(), slime);
/// world.insert_resource(prefabs);
///
/// // A nest spawns a slime, and the slime a smaller one: both name the nest.
/// let nest = world.spawn();
/// let mut commands = CommandBuffer::new();
/// let prepared = world.prepare_prefab("slime", [0.0, 0.0]).unwrap();
/// commands.spawn_prefab_from(nest, prepared);
/// world.flush_reusing(&mut commands);
/// let slime = world.query::<(tungsten_core::Entity, &Tag)>().next().unwrap().0;
/// assert_eq!(world.get::<Source>(slime), Some(&Source::new(nest)));
///
/// let prepared = world.prepare_prefab("slime", [8.0, 0.0]).unwrap();
/// commands.spawn_prefab_from(slime, prepared);
/// world.flush_reusing(&mut commands);
/// assert!(world.query::<&Source>().all(|source| source.entity() == nest));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Source(Entity);

impl Source {
    /// A source of `entity`, as a game sets one by hand.
    #[must_use]
    pub fn new(entity: Entity) -> Self {
        Self(entity)
    }

    /// The chain's origin.
    #[must_use]
    pub fn entity(self) -> Entity {
        self.0
    }
}

/// Registers the derivation of [`Source`] from a spawn's source
/// (`World::derive_from_source`); it adds no system. Build it before the
/// first spawn from a source.
#[derive(Debug, Default, Clone, Copy)]
pub struct SourcePlugin;

impl Plugin for SourcePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Source>(world, "Source");
        world.derive_from_source::<Source>(derive_source);
    }
}

/// The source's own `Source`, else the source; the `Option` is the
/// derivation's signature, and a spawn from a source always takes one.
#[allow(clippy::unnecessary_wraps)]
fn derive_source(world: &World, source: Entity) -> Option<Source> {
    Some(
        world
            .get::<Source>(source)
            .copied()
            .unwrap_or(Source(source)),
    )
}

impl Inspectable for Source {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("source", format!("{:?}", self.0))]
    }
}

#[cfg(test)]
#[path = "../tests/source.rs"]
mod tests;
