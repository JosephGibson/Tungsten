//! The lightning's systems: its presentation's clocks.

use tungsten::core::{Entity, Light, Sprite, Time, World};

use super::components::{Arc, Bolt, Scorch, StrikeLight};
use super::{ARC_SECS, BOLT_SECS, LIGHT_SECS, SCORCH_SECS, STRIKE_LIGHT};

/// `lightning_age`, in the present phase: a bolt or an arc counts the
/// frames that show it and ages from its second, so its first always
/// shows, a bolt going after [`BOLT_SECS`] and an arc after [`ARC_SECS`];
/// a strike's light fades to nothing over [`LIGHT_SECS`], and a scorch
/// over [`SCORCH_SECS`].
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
        if arc.age >= ARC_SECS {
            done.push(entity);
        }
    }
    for (entity, scorch, sprite) in world.query_mut::<(Entity, &mut Scorch, &mut Sprite)>() {
        scorch.age += dt;
        if scorch.age >= SCORCH_SECS {
            done.push(entity);
            continue;
        }
        sprite.color[3] = (255.0 * (1.0 - scorch.age / SCORCH_SECS)).round() as u8;
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
