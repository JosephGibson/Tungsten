//! Damage numbers, while the Damage numbers option is on: each part a hit
//! landed floats in its type's style through the kit's `float_label`, at
//! the request's point or the target's centre. A type's first plain part on
//! a target floats at once; the parts that follow within [`FOLLOW_SECS`]
//! merge into one number when that window closes, and the next part after
//! it floats at once again. A burn's or a bleed's ticks (periodic fire and
//! bleed parts) are sums of their own, at [`TICK_SIZE`]. A step's crit
//! parts on one target float at once as one sum, in the first part's
//! type's style at [`CRIT_SIZE`] in the crit yellow. The open sums are
//! presentation, never saved.

use glam::Vec2;
use tungsten::core::{Collider, Entity, EventCursor, EventQueue, Position, Time, World};
use tungsten::kit::{DamageKinds, DamagedPart, float_label};

use crate::shell::options::GameOptions;

/// How long after a sum's first part its follow-ups keep merging.
pub const FOLLOW_SECS: f32 = 0.25;
/// A tick's size, UI units: a burn's and a bleed's numbers read smaller.
pub const TICK_SIZE: f32 = 11.0;
/// A crit's size, UI units, and its colour: the palette's crit yellow,
/// `[255, 214, 10]`, in linear RGBA.
pub const CRIT_SIZE: f32 = 24.0;
pub const CRIT_YELLOW: [f32; 4] = [1.0, 0.672, 0.003, 1.0];

/// One type's plain parts on one target since its first floated: the
/// follow-ups' sum, where the last landed, the seconds since the first,
/// and whether these are a burn's or a bleed's ticks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Open {
    pub target: Entity,
    pub kinds: DamageKinds,
    pub tick: bool,
    pub sum: f32,
    pub at: Vec2,
    pub since_first: f32,
}

/// The open sums: a World resource the numbers keep.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Numbers {
    pub open: Vec<Open>,
}

/// Floats `sum` of `kinds` at `at`, at the tick size for a tick.
fn float(world: &mut World, kinds: DamageKinds, tick: bool, at: Vec2, sum: f32) {
    if let Some(style) = kinds.label_style(world) {
        let style = if tick {
            style.with_size(TICK_SIZE)
        } else {
            style
        };
        float_label(world, &style, at, sum);
    }
}

/// Whether `part` is a burn's or a bleed's tick: a periodic fire or bleed
/// part. An instant effect's damage, such as a shatter's, is periodic too
/// and reads at full size.
fn tick(world: &World, part: &DamagedPart) -> bool {
    part.periodic
        && (part.kinds.has(world, "damage.fire") || part.kinds.has(world, "damage.physical.bleed"))
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

/// `damage_numbers`, in the react phase: the step's parts floated or merged,
/// its crits floated at once, and the follow-up sums whose window closed
/// floated. With the option off it floats nothing and keeps nothing open.
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
    }
    let mut crits: Vec<(Entity, DamageKinds, f32, Vec2)> = Vec::new();
    let mut now: Vec<(DamageKinds, bool, Vec2, f32)> = Vec::new();
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
        let tick = tick(world, &part);
        let open = numbers.open.iter_mut().find(|open| {
            open.target == part.entity
                && open.kinds == part.kinds
                && open.tick == tick
                && open.since_first < FOLLOW_SECS
        });
        match open {
            Some(open) => {
                open.sum += part.amount;
                open.at = at;
            }
            None => {
                now.push((part.kinds, tick, at, part.amount));
                numbers.open.push(Open {
                    target: part.entity,
                    kinds: part.kinds,
                    tick,
                    sum: 0.0,
                    at,
                    since_first: 0.0,
                });
            }
        }
    }
    for (_, kinds, sum, at) in crits {
        if let Some(style) = kinds.label_style(world) {
            let style = style.with_size(CRIT_SIZE).with_color(CRIT_YELLOW);
            float_label(world, &style, at, sum);
        }
    }
    for (kinds, tick, at, amount) in now {
        float(world, kinds, tick, at, amount);
    }
    let (due, open): (Vec<Open>, Vec<Open>) = numbers
        .open
        .into_iter()
        .partition(|open| open.since_first >= FOLLOW_SECS);
    for open in due {
        if open.sum > 0.0 {
            float(world, open.kinds, open.tick, open.at, open.sum);
        }
    }
    world.insert_resource(Numbers { open });
}
