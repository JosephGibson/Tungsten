//! The kit's saves (redo 7.9): each kit plugin registers, in its `build`,
//! the components and resources that hold state play changes, under their
//! scene names; [`UNSAVED_COMPONENTS`] names the described components it
//! does not save, each with its reason.
//!
//! A step's work over several entities goes in one order, [`order_of`]'s:
//! by save ID while the World saves, so a loaded run's step runs as the
//! uninterrupted run's whatever indices its entities took, and by entity
//! index otherwise, as it always has.

use tungsten_core::save;
use tungsten_core::{Entity, World};

/// A component a prefab gives and play does not change.
const RULE: &str = "rule data a prefab gives";
/// Presentation, which a load restarts.
const PRESENTATION: &str = "presentation, which a load restarts";
/// The hierarchy's: its children come back from their parent's fresh spawn.
const HIERARCHY: &str = "the hierarchy: children come back from their parent's fresh spawn";

/// The described components the kit does not save, by scene name, each
/// with its reason; every other described component is saved, so a game
/// that changes one of these in play registers it itself.
pub const UNSAVED_COMPONENTS: &[(&str, &str)] = &[
    ("action_input", RULE),
    ("ailment_threshold", RULE),
    ("animation", PRESENTATION),
    ("camera_target", RULE),
    ("character_animator", PRESENTATION),
    ("conversion", RULE),
    ("cues", RULE),
    ("depth_sort", PRESENTATION),
    ("despawn_outside", RULE),
    ("hit_flash", PRESENTATION),
    ("hurtbox", RULE),
    ("penetration", RULE),
    ("spawn_children", HIERARCHY),
    ("speed_limit", RULE),
    ("stay_in_view", RULE),
    ("wrap", RULE),
    ("zone", RULE),
];

/// The order a step's work sorts its entities in, read once a sort:
/// [`save::order_key`] while the World saves, the entity's index
/// otherwise, which keeps a World that never named in today's order.
#[derive(Clone, Copy)]
pub(crate) struct Order<'w> {
    /// The World, while it saves.
    saving: Option<&'w World>,
}

impl Order<'_> {
    /// `entity`'s sort key.
    pub(crate) fn key(self, entity: Entity) -> (bool, u64) {
        match self.saving {
            Some(world) => save::order_key(world, entity),
            None => (false, u64::from(entity.id())),
        }
    }
}

/// The World's [`Order`].
pub(crate) fn order_of(world: &World) -> Order<'_> {
    Order {
        saving: save::saving(world).then_some(world),
    }
}

/// Whether the World saves: a step's work that today follows its query's
/// order sorts by [`order_of`] only then.
pub(crate) fn saving(world: &World) -> bool {
    save::saving(world)
}

#[cfg(test)]
#[path = "../tests/saved.rs"]
mod tests;
