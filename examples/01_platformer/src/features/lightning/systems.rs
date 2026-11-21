//! The lightning's systems: each jump's look and keg, and its
//! presentation's clocks.

use tungsten::SceneEntity;
use tungsten::core::{Entity, EventCursor, EventQueue, Light, Sprite, Time, World};
use tungsten::kit::Conducted;

use super::components::{Arc, Bolt, Scorch, StrikeLight};
use super::{ARC_SECS, BOLT_SECS, LIGHT_SECS, SCORCH_SECS, STRIKE_LIGHT};
use crate::features::audio::{self, components::Sound};
use crate::features::elements::cascade::player_credit;
use crate::features::props::{self, components::Keg};
use crate::game::GAMEPLAY;

/// `lightning_jumps`, in the intent phase after the kit's ability tick,
/// which jumps the chains: each jump the step sent ([`Conducted`], read
/// once through `cursor`) draws its arc from the point it left to the
/// receiver's centre, its zigzag seeded by the receiver's save ID and the
/// jump's number (Q6), crackles there, and sets a keg receiver off,
/// credited to the run's player, struck or not.
pub fn jumps(world: &mut World, cursor: &mut EventCursor<Conducted>) {
    let Some(events) = world.get_resource::<EventQueue<Conducted>>() else {
        return;
    };
    let jumps: Vec<Conducted> = cursor.read(events).copied().collect();
    if jumps.is_empty() {
        return;
    }
    let credit = player_credit(world);
    for jump in jumps {
        let id = crate::shared::save_id(world, jump.receiver).map_or(0, |id| id.0);
        world.spawn_with((
            Arc {
                from: jump.from,
                to: jump.to,
                seed: id.wrapping_mul(8).wrapping_add(u64::from(jump.jump)),
                age: 0.0,
                frames: 0,
            },
            SceneEntity { state_id: GAMEPLAY },
        ));
        audio::play(world, Sound::Crackled, jump.to);
        if world.has::<Keg>(jump.receiver) {
            props::set_off(world, jump.receiver, credit);
        }
    }
}

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
