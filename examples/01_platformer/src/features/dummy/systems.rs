//! The dummy's systems.

use glam::Vec2;
use tungsten::core::{Collider, Entity, EventCursor, EventQueue, Position, Visibility, World};
use tungsten::kit::Damaged;

use super::components::Dummy;
use crate::features::combat::{BURNING, FROZEN, has_status};
use crate::features::elements;
use crate::features::elements::cascade::{CascadeKind, Cascades};
use crate::shared::{SaveId, spawn_burst};

/// A dummy's body centre.
fn centre(world: &World, dummy: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(dummy)?.0;
    Some(
        at + world
            .get::<Collider>(dummy)
            .map_or(Vec2::ZERO, |c| c.offset),
    )
}

/// `dummy_deaths`, in the react phase after the step's reactions: each
/// dummy a hit killed ends in one burst, and its sprite goes with it while
/// the kit's death policy runs out. `cursor` reads each hit once.
pub fn deaths(world: &mut World, cursor: &mut EventCursor<Damaged>) {
    let Some(events) = world.get_resource::<EventQueue<Damaged>>() else {
        return;
    };
    let killed: Vec<Entity> = cursor
        .read(events)
        .filter(|hit| hit.killed && world.has::<Dummy>(hit.entity))
        .map(|hit| hit.entity)
        .collect();
    for dummy in killed {
        end(world, dummy);
    }
}

/// A dummy's one burst: the reaction its killing step's parts queued on it
/// when they queued one, thermal shock's explosion or a steam burst, which
/// runs from the next step; else, by its state, a shatter if frozen, a
/// steam pop if a steam burst killed it, a burn-out if burning, a poof
/// otherwise.
pub fn end(world: &mut World, dummy: Entity) {
    let Some(at) = centre(world, dummy) else {
        return;
    };
    if let Some(visibility) = world.get_mut::<Visibility>(dummy) {
        visibility.visible = false;
    }
    let id = world.get::<SaveId>(dummy).map(|id| id.0);
    let cascades = world.get_resource::<Cascades>();
    let reacted = id.is_some_and(|id| {
        cascades.is_some_and(|cascades| {
            cascades.waits(CascadeKind::Explosion, id) || cascades.waits(CascadeKind::Steam, id)
        })
    });
    if reacted {
        return;
    }
    let steamed = cascades.is_some_and(|cascades| cascades.steamed.contains(&dummy));
    if has_status(world, dummy, FROZEN) {
        spawn_burst(world, "ex10_ice_shatter", at);
        elements::ice_pulse(world, at, 64.0, true);
    } else if steamed {
        spawn_burst(world, "ex10_steam_burst", at);
    } else if has_status(world, dummy, BURNING) {
        spawn_burst(world, "ex10_burn_out", at);
    } else {
        spawn_burst(world, "ex10_poof", at);
    }
}
