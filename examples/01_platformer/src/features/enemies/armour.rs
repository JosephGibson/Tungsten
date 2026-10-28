//! Armour (e01flask, Q3, Q6): an `armoured` creature's front, the half its
//! heading faces, turns bullets as iron does while it lives, unfrozen and
//! uncorroded (`gun::systems`); its rear half takes every hit 01 builds
//! whole (`combat::Hit`). The beetle and the armoured troll wear it, each
//! with its own resistances; Corrosive takes them for good
//! (`combat::systems::corrode`).

use glam::Vec2;
use tungsten::core::{Entity, World};

use super::centre;
use super::components::Enemy;
use crate::features::combat::{ARMOURED, has_status};

/// Whether `entity` wears armour.
#[must_use]
pub fn armoured(world: &World, entity: Entity) -> bool {
    has_status(world, entity, ARMOURED)
}

/// Whether `point` lies on `target`'s front half: past its centre on the
/// side its heading faces.
#[must_use]
pub fn front(world: &World, target: Entity, point: Vec2) -> bool {
    let (Some(mine), Some(at)) = (world.get::<Enemy>(target), centre(world, target)) else {
        return false;
    };
    (point.x - at.x) * mine.heading > 0.0
}

/// Whether `target` wears armour and `point` lies on its rear half, which
/// takes a hit whole.
#[must_use]
pub fn rear(world: &World, target: Entity, point: Vec2) -> bool {
    armoured(world, target) && !front(world, target, point)
}
