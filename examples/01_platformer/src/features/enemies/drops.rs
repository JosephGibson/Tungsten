//! The enemies' drops: health and mana orbs, which each enemy's `killed`
//! cue rolls from `enemy_drops` (the troll's from `troll_drops`, three
//! rolls). An orb is a bodiless pickup filtered to `team.player`, drawn by
//! the player's `pickup_radius` magnet, and fades after [`ORB_SECS`];
//! [`adopt`] gives each its save ID before the shell's save, and [`collect`]
//! queues its effect on the player in the overlap phase, so the step's own
//! resolve takes it.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::{DataRef, Entity, EventCursor, EventQueue, With, Without, World};
use tungsten::kit::{EffectRequest, Lifetime, PickedUp};

use crate::game::GAMEPLAY;
use crate::shared::{SaveId, SaveIds};

/// An orb's seconds before it fades.
pub const ORB_SECS: f32 = 20.0;

/// One of the two orbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrbKind {
    Health,
    Mana,
}

impl OrbKind {
    /// Its prefab.
    #[must_use]
    pub const fn prefab(self) -> &'static str {
        match self {
            OrbKind::Health => "health_orb",
            OrbKind::Mana => "mana_orb",
        }
    }

    /// The instant effect its pickup queues on the player: 20 health, or 25
    /// mana up to its most.
    #[must_use]
    pub const fn effect(self) -> &'static str {
        match self {
            OrbKind::Health => "orb_heal",
            OrbKind::Mana => "orb_mana",
        }
    }
}

/// An orb, which its prefab names `orb` with its kind. Saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Orb {
    pub kind: OrbKind,
}

/// `orb_adopt`, in `PreUpdate` before the shell's save: each orb a roll
/// landed at the last flush joins the run, its save ID its own.
pub fn adopt(world: &mut World) {
    let fresh: Vec<Entity> = world
        .query_filtered::<Entity, (With<Orb>, Without<SaveId>)>()
        .collect();
    for orb in fresh {
        let id = world.resource_mut::<SaveIds>().take();
        world.insert_bundle(orb, (id, SceneEntity { state_id: GAMEPLAY }));
    }
}

/// `orb_collect`, in the overlap phase after the kit's pickup: each orb
/// collected queues its kind's effect on its collector, from the orb.
pub fn collect(world: &mut World, cursor: &mut EventCursor<PickedUp>) {
    let Some(events) = world.get_resource::<EventQueue<PickedUp>>() else {
        return;
    };
    let picked: Vec<(Entity, OrbKind)> = cursor
        .read(events)
        .filter_map(|picked| Some((picked.by, world.get::<Orb>(picked.entity)?.kind)))
        .collect();
    for (by, kind) in picked {
        EffectRequest::new(by, DataRef::by_id(kind.effect())).queue(world);
    }
}

/// Spawns an orb of `kind` with its sprite's top-left at `corner`, with the
/// save ID `id` and `left` seconds before it fades, at once: a load's or a
/// test's.
pub fn spawn(
    world: &mut World,
    kind: OrbKind,
    corner: Vec2,
    id: SaveId,
    left: f32,
) -> Option<Entity> {
    let orb = world
        .prepare_prefab(kind.prefab(), corner.to_array())
        .ok()?
        .with((id, SceneEntity { state_id: GAMEPLAY }))
        .spawn(world);
    world.insert(orb, Lifetime::new(left.max(0.0)));
    Some(orb)
}
