//! The lightning's systems: its presentation's clocks.

use tungsten::core::{Entity, Light, Time, World};

use super::components::{Arc, Bolt, StrikeLight};
use super::{BOLT_SECS, LIGHT_SECS, STRIKE_LIGHT};

/// `lightning_age`, in the present phase: a bolt or an arc counts the
/// frames that show it and ages from its second, so its first always
/// shows, and goes after [`BOLT_SECS`]; a strike's light fades to nothing
/// over [`LIGHT_SECS`].
pub fn age(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let mut done = Vec::new();
    for (entity, bolt) in world.query_mut::<(Entity, &mut Bolt)>() {
        bolt.frames += 1;
        if bolt.frames > 1 {
            bolt.age += dt;
        }
        if bolt.age >= BOLT_SECS {
            done.push(entity);
        }
    }
    for (entity, arc) in world.query_mut::<(Entity, &mut Arc)>() {
        arc.frames += 1;
        if arc.frames > 1 {
            arc.age += dt;
        }
        if arc.age >= BOLT_SECS {
            done.push(entity);
        }
    }
    for (entity, flash, light) in world.query_mut::<(Entity, &mut StrikeLight, &mut Light)>() {
        flash.age += dt;
        if flash.age >= LIGHT_SECS {
            done.push(entity);
            continue;
        }
        light.intensity = STRIKE_LIGHT * (1.0 - flash.age / LIGHT_SECS);
    }
    for entity in done {
        world.despawn(entity);
    }
}
