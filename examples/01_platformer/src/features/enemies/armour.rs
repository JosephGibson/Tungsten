//! Armour (e01flask, Q3, Q6): an `armoured` creature's front, the half it
//! faces, turns bullets as iron does while it lives, unfrozen and
//! uncorroded (`gun::systems`); its rear half takes a hit whole: every hit
//! 01 builds (`combat::Hit`) and every kit blast and impact naming the
//! `armour` hit preparation ([`prepare`]). The beetle and the armoured
//! troll wear it, each with its own resistances; Corrosive takes them for
//! good (`combat::systems::corrode`).

use glam::Vec2;
use tungsten::core::{Entity, World};
use tungsten::kit::{DamageRequest, Penetration};

use super::{centre, heading_of};
use crate::features::combat::{ARMOURED, Kind, has_status};

/// The hit preparation [`prepare`] is registered as (`game::register`),
/// which the kit's blasts and impacts name in their `prepare`.
pub const PREPARE: &str = "armour";

/// Whether `entity` wears armour.
#[must_use]
pub fn armoured(world: &World, entity: Entity) -> bool {
    has_status(world, entity, ARMOURED)
}

/// Whether `point` lies on `target`'s front half: past its centre on the
/// side it faces ([`heading_of`]).
#[must_use]
pub fn front(world: &World, target: Entity, point: Vec2) -> bool {
    let (Some(heading), Some(at)) = (heading_of(world, target), centre(world, target)) else {
        return false;
    };
    (point.x - at.x) * heading > 0.0
}

/// Whether `target` wears armour and `point` lies on its rear half, which
/// takes a hit whole.
#[must_use]
pub fn rear(world: &World, target: Entity, point: Vec2) -> bool {
    armoured(world, target) && !front(world, target, point)
}

/// The `armour` hit preparation (`tungsten::kit::add_hit_prep`): a hit whose
/// point lies on an armoured target's rear half takes physical penetration
/// 1, so the armour's physical resistance takes nothing; a hit without a
/// point, or on its front, as it is.
pub fn prepare(world: &World, hit: &mut DamageRequest) {
    let rear = hit
        .position()
        .is_some_and(|at| rear(world, hit.target(), at));
    if rear {
        let pen = Penetration::new().with_points(Kind::Physical.tag(), 1.0);
        let taken = std::mem::replace(hit, DamageRequest::new(hit.target(), 0.0));
        *hit = taken.with_penetration(pen);
    }
}
