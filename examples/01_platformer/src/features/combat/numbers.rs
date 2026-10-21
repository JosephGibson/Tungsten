//! Damage numbers, while the Damage numbers option is on: each part a hit
//! landed floats in its type's style through the kit's `float_label`, at
//! the request's point or the target's centre. A type's plain parts on one
//! target merge while they keep arriving within [`MERGE_SECS`], floated as
//! their sum [`MERGE_SECS`] after the last or [`MERGE_MOST`] after the
//! first: burn and bleed ticks, the sprays' pulses, a burst of shots. A
//! step's crit parts on one target float at once as one sum, in the first
//! part's type's style at [`CRIT_SIZE`] in the crit yellow. The open sums
//! are presentation, never saved.

use glam::Vec2;
use tungsten::core::{Collider, Entity, EventCursor, EventQueue, Position, Time, World};
use tungsten::kit::{DamageKinds, DamagedPart, float_label};

use crate::shell::options::GameOptions;

/// How long a sum stays open without a part, and at most.
pub const MERGE_SECS: f32 = 0.6;
pub const MERGE_MOST: f32 = 1.0;
/// A crit's size, UI units, and its colour: the palette's crit yellow,
/// `[255, 214, 10]`, in linear RGBA.
pub const CRIT_SIZE: f32 = 24.0;
pub const CRIT_YELLOW: [f32; 4] = [1.0, 0.672, 0.003, 1.0];

/// One type's plain parts on one target, still open: their sum, where the
/// last landed, and the seconds since the first and the last.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Open {
    pub target: Entity,
    pub kinds: DamageKinds,
    pub sum: f32,
    pub at: Vec2,
    pub since_first: f32,
    pub since_last: f32,
}

/// The open sums: a World resource the numbers keep.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Numbers {
    pub open: Vec<Open>,
}

/// A target's body centre.
fn centre(world: &World, entity: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(entity)?.0;
    Some(
        at + world
            .get::<Collider>(entity)
            .map_or(Vec2::ZERO, |c| c.offset),
    )
}

/// `damage_numbers`, in the react phase: the step's parts into their sums,
/// its crits floated at once, and the sums whose time came floated. With
/// the option off it floats nothing and keeps nothing open.
pub fn numbers(world: &mut World, cursor: &mut EventCursor<DamagedPart>) {
    let parts: Vec<DamagedPart> = world
        .get_resource::<EventQueue<DamagedPart>>()
        .map(|events| cursor.read(events).cloned().collect())
        .unwrap_or_default();
    let shown = world
        .get_resource::<GameOptions>()
        .is_none_or(|options| options.damage_numbers);
    if !shown {
        world.remove_resource::<Numbers>();
        return;
    }
    let dt = world.resource::<Time>().delta();
    let mut numbers = world.remove_resource::<Numbers>().unwrap_or_default();
    for open in &mut numbers.open {
        open.since_first += dt;
        open.since_last += dt;
    }
    let mut crits: Vec<(Entity, DamageKinds, f32, Vec2)> = Vec::new();
    for part in parts {
        if !part.amount.is_finite() || part.amount <= 0.0 {
            continue;
        }
        let Some(at) = part.position.or_else(|| centre(world, part.entity)) else {
            continue;
        };
        if part.crit {
            match crits.iter_mut().find(|crit| crit.0 == part.entity) {
                Some(crit) => crit.2 += part.amount,
                None => crits.push((part.entity, part.kinds, part.amount, at)),
            }
            continue;
        }
        let open = numbers
            .open
            .iter_mut()
            .find(|open| open.target == part.entity && open.kinds == part.kinds);
        match open {
            Some(open) => {
                open.sum += part.amount;
                open.at = at;
                open.since_last = 0.0;
            }
            None => numbers.open.push(Open {
                target: part.entity,
                kinds: part.kinds,
                sum: part.amount,
                at,
                since_first: 0.0,
                since_last: 0.0,
            }),
        }
    }
    for (_, kinds, sum, at) in crits {
        if let Some(style) = kinds.label_style(world) {
            let style = style.with_size(CRIT_SIZE).with_color(CRIT_YELLOW);
            float_label(world, &style, at, sum);
        }
    }
    let (due, open): (Vec<Open>, Vec<Open>) = numbers
        .open
        .into_iter()
        .partition(|open| open.since_last >= MERGE_SECS || open.since_first >= MERGE_MOST);
    for open in due {
        if let Some(style) = open.kinds.label_style(world) {
            float_label(world, &style, open.at, open.sum);
        }
    }
    world.insert_resource(Numbers { open });
}
