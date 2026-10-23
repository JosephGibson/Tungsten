//! The enemies' drops: health and mana orbs, which each enemy's `killed`
//! cue rolls from `enemy_drops` (the troll's from `troll_drops`, three
//! rolls). An orb is a bodiless pickup filtered to `team.player`, drawn by
//! the player's `pickup_radius` magnet, and fades after [`ORB_SECS`];
//! [`adopt`] gives each its save ID before the shell's save, and [`collect`]
//! queues its effect on the player in the overlap phase, so the step's own
//! resolve takes it. The map's `pickups` layer places health orbs that
//! never fade ([`start`]); one taken stays gone through a save.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::{
    DataRef, Entity, EventCursor, EventQueue, Position, Visibility, With, Without, World,
};
use tungsten::kit::{EffectRequest, Lifetime, PickedUp, Pickup};

use crate::features::audio::{self, components::Sound};
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, SaveIds, spawn_burst};

/// The orbs the map placed this run, by save ID, sorted: a World resource a
/// run's start inserts, which a save's `gone` reads.
#[derive(Debug, Clone, Default)]
pub struct PlacedOrbs(pub Vec<u64>);

/// An orb's seconds before it fades, and the last of them it blinks over,
/// this many times a second.
pub const ORB_SECS: f32 = 20.0;
pub const BLINK_SECS: f32 = 3.0;
pub const BLINK_HZ: f32 = 5.0;
/// An orb's centre from its point, its sprite's top-left: the 24 px orb's
/// middle.
pub const ORB_CENTRE: Vec2 = Vec2::new(12.0, 12.0);

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

    /// The burst it gives where it is taken, in its colour.
    #[must_use]
    pub const fn burst(self) -> &'static str {
        match self {
            OrbKind::Health => "ex10_orb_health",
            OrbKind::Mana => "ex10_orb_mana",
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

/// At a run's start, after the map spawned: the orbs it placed, which never
/// fade, recorded by save ID.
pub fn start(world: &mut World) {
    let mut placed: Vec<(u64, Entity)> = world
        .query_filtered::<(Entity, &SaveId), With<Orb>>()
        .map(|(entity, id)| (id.0, entity))
        .collect();
    placed.sort_unstable_by_key(|(id, _)| *id);
    for &(_, orb) in &placed {
        world.remove_component::<Lifetime>(orb);
    }
    world.insert_resource(PlacedOrbs(placed.into_iter().map(|(id, _)| id).collect()));
}

/// The placed orbs taken, by save ID: gone, or collected and waiting for
/// the step's end to go.
#[must_use]
pub fn gone(world: &World) -> Vec<u64> {
    let live: Vec<u64> = world
        .query_filtered::<(&SaveId, &Pickup), With<Orb>>()
        .filter(|(_, pickup)| !pickup.is_collected())
        .map(|(id, _)| id.0)
        .collect();
    world
        .get_resource::<PlacedOrbs>()
        .map(|placed| {
            placed
                .0
                .iter()
                .copied()
                .filter(|id| !live.contains(id))
                .collect()
        })
        .unwrap_or_default()
}

/// `orb_adopt`, in `PreUpdate` before the shell's save: each orb a roll
/// landed at the last flush joins the run, its save ID its own, and pops
/// out with its sound and its kind's sparks. Placed and loaded orbs never
/// pass here.
pub fn adopt(world: &mut World) {
    let fresh: Vec<Entity> = world
        .query_filtered::<Entity, (With<Orb>, Without<SaveId>)>()
        .collect();
    for orb in fresh {
        let id = world.resource_mut::<SaveIds>().take();
        world.insert_bundle(orb, (id, SceneEntity { state_id: GAMEPLAY }));
        let (Some(kind), Some(at)) = (
            world.get::<Orb>(orb).map(|orb| orb.kind),
            world.get::<Position>(orb).map(|at| at.0 + ORB_CENTRE),
        ) else {
            continue;
        };
        spawn_burst(world, kind.burst(), at);
        audio::play(world, Sound::OrbDropped, at);
    }
}

/// `orb_blink`, in the present phase: a dropped orb blinks over its last
/// [`BLINK_SECS`], faster as it runs out; a placed one never fades.
pub fn blink(world: &mut World) {
    for (_, lifetime, visibility) in world.query_mut::<(&Orb, &Lifetime, &mut Visibility)>() {
        let left = lifetime.timer().remaining();
        visibility.visible = left > BLINK_SECS || (left * BLINK_HZ).fract() < 0.6;
    }
}

/// `orb_collect`, in the overlap phase after the kit's pickup: each orb
/// collected queues its kind's effect on its collector, from the orb, and
/// bursts in its colour where it was; its cue plays the potion's sound.
pub fn collect(world: &mut World, cursor: &mut EventCursor<PickedUp>) {
    let Some(events) = world.get_resource::<EventQueue<PickedUp>>() else {
        return;
    };
    let picked: Vec<(Entity, OrbKind, Option<Vec2>)> = cursor
        .read(events)
        .filter_map(|picked| {
            let kind = world.get::<Orb>(picked.entity)?.kind;
            let at = world
                .get::<Position>(picked.entity)
                .map(|at| at.0 + ORB_CENTRE);
            Some((picked.by, kind, at))
        })
        .collect();
    for (by, kind, at) in picked {
        EffectRequest::new(by, DataRef::by_id(kind.effect())).queue(world);
        if let Some(at) = at {
            spawn_burst(world, kind.burst(), at);
        }
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
