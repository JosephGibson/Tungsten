//! The enemies' drops: health and mana orbs, which each enemy's `killed`
//! cue rolls from `enemy_drops` (the troll's from `troll_drops`, three
//! rolls). An orb is a bodiless pickup filtered to `team.player`, drawn by
//! the player's `pickup_radius` magnet, and fades after [`ORB_SECS`]; its
//! foe's `killed` roll centres it on the foe's body (`at: centre`,
//! `centred`), and [`adopt`] gives each its save ID before the shell's
//! save, moved clear of the rock there. Its `picked_up` cue lands its
//! effect on the player at the next resolve and bursts in its colour. A dropped orb out of the magnet's reach
//! falls by its pickup's `fall` onto the tiles, walls, lava or crust under
//! it, and blinks over its last seconds by its lifetime's `blink`, both its
//! prefab's; an older save's orb takes its fall back ([`restored`]). The
//! map's `pickups` layer places health orbs that never fade or fall
//! ([`start`]); one taken stays gone through a save.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::save::Value;
use tungsten::core::{
    Entity, PhysicsLayers, Position, PrefabRegistry, Shape, With, Without, World,
};
use tungsten::kit::{Lifetime, Pickup};
use tungsten::physics::{SpatialFilter, overlap};

use super::WALL_LAYERS;
use crate::features::audio::{self, components::Sound};
use crate::features::level::{LAVA_LAYER, TILE};
use crate::game::GAMEPLAY;
use crate::shared::{Joined, Saved, spawn_burst};

/// The orbs the map placed this run, by save ID, sorted: a World resource a
/// run's start inserts, which a save's `gone` reads.
#[derive(Debug, Clone, Default)]
pub struct PlacedOrbs(pub Vec<u64>);

/// An orb's seconds before it fades: its prefab's lifetime.
pub const ORB_SECS: f32 = 20.0;
/// An orb's centre from its point, its sprite's top-left: the 24 px orb's
/// middle, and its box's half size.
pub const ORB_CENTRE: Vec2 = Vec2::new(12.0, 12.0);
pub const ORB_HALF: Vec2 = Vec2::new(12.0, 12.0);
/// A dropped orb's way out of the rock it landed in: so far at a time, up
/// to a tile up, then down.
const CLEAR_STEP: f32 = 4.0;

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

    /// The burst it gives where it drops and where it is taken, in its
    /// colour.
    #[must_use]
    pub const fn burst(self) -> &'static str {
        match self {
            OrbKind::Health => "ex10_orb_health",
            OrbKind::Mana => "ex10_orb_mana",
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
/// fade or fall, their lifetime gone and their pickup's fall 0, recorded by
/// save ID.
pub fn start(world: &mut World) {
    let mut placed: Vec<(u64, Entity)> = world
        .query_filtered::<(Entity, &Saved), With<Orb>>()
        .map(|(entity, id)| (id.id().0, entity))
        .collect();
    placed.sort_unstable_by_key(|(id, _)| *id);
    for &(_, orb) in &placed {
        world.remove_component::<Lifetime>(orb);
        if let Some(pickup) = world.get_mut::<Pickup>(orb) {
            *pickup = pickup.clone().with_fall(0.0);
        }
        world.insert(orb, Joined);
    }
    world.insert_resource(PlacedOrbs(placed.into_iter().map(|(id, _)| id).collect()));
}

/// The placed orbs taken, by save ID: gone, or collected and waiting for
/// the step's end to go.
#[must_use]
pub fn gone(world: &World) -> Vec<u64> {
    let live: Vec<u64> = world
        .query_filtered::<(&Saved, &Pickup), With<Orb>>()
        .filter(|(_, pickup)| !pickup.is_collected())
        .map(|(id, _)| id.id().0)
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

/// The two queries that find rock, since a mask filters the tiles too: the
/// tiles alone, then the walls' and lava's layers.
fn rock() -> [SpatialFilter; 2] {
    [
        SpatialFilter::new().with_bodies(false),
        SpatialFilter::new().with_mask(WALL_LAYERS | LAVA_LAYER),
    ]
}

/// Whether an orb's box about `centre` meets a tile, a wall or lava's
/// surface.
fn in_rock(world: &World, centre: Vec2) -> bool {
    let shape = Shape::Aabb {
        half_extents: ORB_HALF,
    };
    let mut met = Vec::new();
    rock().iter().any(|filter| {
        overlap(world, shape, centre, filter, &mut met);
        !met.is_empty()
    })
}

/// Where a dropped orb about `centre` lands clear of rock: there, else
/// [`CLEAR_STEP`] at a time up to a tile up, else as far down, else there.
fn clear(world: &World, centre: Vec2) -> Vec2 {
    if !in_rock(world, centre) {
        return centre;
    }
    let steps = (TILE / CLEAR_STEP) as u32;
    [-1.0, 1.0]
        .into_iter()
        .flat_map(|way| (1..=steps).map(move |k| Vec2::new(0.0, way * CLEAR_STEP * k as f32)))
        .map(|shift| centre + shift)
        .find(|&at| !in_rock(world, at))
        .unwrap_or(centre)
}

/// `orb_adopt`, in `PreUpdate` before the shell's save: each orb a roll
/// landed at the last flush, centred on the body of the enemy that dropped
/// it, joins the run ([`Joined`]), its save ID the flush's, and is moved
/// clear of the rock there ([`clear`]); it pops out with its sound and its
/// kind's sparks. Placed and loaded orbs never pass here.
pub fn adopt(world: &mut World) {
    let fresh: Vec<Entity> = world
        .query_filtered::<Entity, (With<Orb>, Without<Joined>)>()
        .collect();
    for orb in fresh {
        world.insert_bundle(orb, (Joined, SceneEntity { state_id: GAMEPLAY }));
        let (Some(kind), Some(centre)) = (
            world.get::<Orb>(orb).map(|orb| orb.kind),
            world.get::<Position>(orb).map(|at| at.0 + ORB_CENTRE),
        ) else {
            continue;
        };
        let at = clear(world, centre);
        if let Some(position) = world.get_mut::<Position>(orb) {
            position.0 = at - ORB_CENTRE;
        }
        spawn_burst(world, kind.burst(), at);
        audio::play(world, Sound::OrbDropped, at);
    }
}

/// What a restored dropped orb's spawn gave it beyond its saved state: an
/// older save's, whose pickup has no fall, takes its prefab's fall and fall
/// layers, read from the prefab's data, so it falls as a fresh drop does.
pub fn restored(world: &mut World, orb: Entity) {
    let (Some(kind), Some(pickup)) = (
        world.get::<Orb>(orb).map(|orb| orb.kind),
        world.get::<Pickup>(orb),
    ) else {
        return;
    };
    if pickup.fall() > 0.0 {
        return;
    }
    let Some(data) = world
        .get_resource::<PrefabRegistry>()
        .and_then(|prefabs| prefabs.get(kind.prefab()))
        .and_then(|data| data.components.get("pickup"))
    else {
        return;
    };
    let fall = data.get("fall").and_then(Value::as_f64).unwrap_or(0.0) as f32;
    if fall <= 0.0 {
        return;
    }
    let mut falling = pickup.clone().with_fall(fall);
    if let Some(list) = data.get("fall_layers").and_then(Value::as_array) {
        falling = falling.with_fall_layers(layer_mask(world, list));
    }
    if let Some(pickup) = world.get_mut::<Pickup>(orb) {
        *pickup = falling;
    }
}

/// A prefab's layer list as a mask, one bit per layer: each entry a layer
/// number or the manifest's `physics_layers` name for one.
fn layer_mask(world: &World, list: &[Value]) -> u32 {
    let names = world.get_resource::<PhysicsLayers>();
    list.iter()
        .filter_map(|entry| match entry {
            Value::String(name) => names?.bit(name),
            number => 1_u32.checked_shl(u32::try_from(number.as_u64()?).ok()?),
        })
        .fold(0, |mask, bit| mask | bit)
}

/// Spawns an orb of `kind` with its sprite's top-left at `corner`, at once,
/// a spawn of its prefab the engine saves whole
/// ([`PreparedPrefab::saved`]), fading and blinking as its prefab's: a
/// test's.
///
/// [`PreparedPrefab::saved`]: tungsten::core::PreparedPrefab::saved
pub fn spawn(world: &mut World, kind: OrbKind, corner: Vec2) -> Option<Entity> {
    let orb = world
        .prepare_prefab(kind.prefab(), corner.to_array())
        .ok()?
        .saved(world, Some(crate::features::level::MAP))
        .with((Joined, SceneEntity { state_id: GAMEPLAY }))
        .spawn(world);
    Some(orb)
}
