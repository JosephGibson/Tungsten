//! The enemies' drops: health and mana orbs, which each enemy's `killed`
//! cue rolls from `enemy_drops` (the troll's from `troll_drops`, three
//! rolls). An orb is a bodiless pickup filtered to `team.player`, drawn by
//! the player's `pickup_radius` magnet, and fades after [`ORB_SECS`];
//! [`adopt`] gives each its save ID before the shell's save, centred on the
//! body of the enemy that dropped it, as [`fallen`] recorded it at the
//! kill, and moved clear of the rock there, and
//! [`collect`] queues its effect on the player in the overlap phase, so the
//! step's own resolve takes it. A dropped orb out of the magnet's reach
//! falls at [`ORB_FALL`] onto the tiles, walls or lava under it ([`fall`]).
//! The map's `pickups` layer places health orbs that never fade or fall
//! ([`start`]); one taken stays gone through a save.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::{
    DataRef, Entity, EventCursor, EventQueue, Position, Shape, Time, Visibility, With, Without,
    World,
};
use tungsten::kit::{Damaged, EffectRequest, Lifetime, PickedUp, Pickup, Source, Stats};
use tungsten::physics::{SpatialFilter, cast_shape, overlap};

use super::WALL_LAYERS;
use crate::features::audio::{self, components::Sound};
use crate::features::level::{LAVA_LAYER, TILE};
use crate::features::player;
use crate::features::run::Run;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, SaveIds, spawn_burst};

/// The orbs the map placed this run, by save ID, sorted: a World resource a
/// run's start inserts, which a save's `gone` reads.
#[derive(Debug, Clone, Default)]
pub struct PlacedOrbs(pub Vec<u64>);

/// The enemies killed since the last [`adopt`], each with its body's centre
/// at the kill, where its drops land: a dead enemy goes the step after its
/// kill, before its drops are adopted. Not saved.
#[derive(Debug, Clone, Default)]
pub struct Fallen(pub Vec<(Entity, Vec2)>);

/// An orb's seconds before it fades, and the last of them it blinks over,
/// this many times a second.
pub const ORB_SECS: f32 = 20.0;
pub const BLINK_SECS: f32 = 3.0;
pub const BLINK_HZ: f32 = 5.0;
/// An orb's centre from its point, its sprite's top-left: the 24 px orb's
/// middle, and its box's half size.
pub const ORB_CENTRE: Vec2 = Vec2::new(12.0, 12.0);
pub const ORB_HALF: Vec2 = Vec2::new(12.0, 12.0);
/// How fast a dropped orb out of the magnet's reach falls, px/s.
pub const ORB_FALL: f32 = 120.0;
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

/// `orb_fallen`, in the react phase after the enemies' deaths: each enemy
/// killed this step leaves its body's centre for [`adopt`].
pub fn fallen(world: &mut World, cursor: &mut EventCursor<Damaged>) {
    let Some(events) = world.get_resource::<EventQueue<Damaged>>() else {
        return;
    };
    let killed: Vec<(Entity, Vec2)> = cursor
        .read(events)
        .filter(|hit| hit.killed)
        .filter_map(|hit| Some((hit.entity, super::centre(world, hit.entity)?)))
        .collect();
    if killed.is_empty() {
        return;
    }
    match world.get_resource_mut::<Fallen>() {
        Some(fallen) => fallen.0.extend(killed),
        None => world.insert_resource(Fallen(killed)),
    }
}

/// `orb_adopt`, in `PreUpdate` before the shell's save: each orb a roll
/// landed at the last flush joins the run, its save ID its own, centred on
/// the body of the enemy that dropped it (its `Source`, at its kill as
/// [`fallen`] recorded it), else at its cue's point, and moved clear of the
/// rock there ([`clear`]); it pops out with its sound and its kind's
/// sparks. Placed and loaded orbs never pass here.
pub fn adopt(world: &mut World) {
    let fallen = world.remove_resource::<Fallen>().unwrap_or_default();
    let fresh: Vec<Entity> = world
        .query_filtered::<Entity, (With<Orb>, Without<SaveId>)>()
        .collect();
    for orb in fresh {
        let id = world.resource_mut::<SaveIds>().take();
        world.insert_bundle(orb, (id, SceneEntity { state_id: GAMEPLAY }));
        let (Some(kind), Some(point)) = (
            world.get::<Orb>(orb).map(|orb| orb.kind),
            world.get::<Position>(orb).map(|at| at.0 + ORB_CENTRE),
        ) else {
            continue;
        };
        let centre = world
            .get::<Source>(orb)
            .map(|source| source.entity())
            .and_then(|enemy| {
                let killed = fallen.0.iter().find(|(dead, _)| *dead == enemy);
                killed
                    .map(|&(_, at)| at)
                    .or_else(|| super::centre(world, enemy))
            })
            .unwrap_or(point);
        let at = clear(world, centre);
        if let Some(position) = world.get_mut::<Position>(orb) {
            position.0 = at - ORB_CENTRE;
        }
        spawn_burst(world, kind.burst(), at);
        audio::play(world, Sound::OrbDropped, at);
    }
}

/// `orb_fall`, in the move phase after the kit's magnet: each dropped orb
/// no magnet reaches, the player's body centre farther than its
/// `pickup_radius` from the orb's, falls at [`ORB_FALL`], at most onto the
/// first tile, wall or lava surface under its box; nothing else holds it
/// up, so it passes enemies, the player and loose bodies. Nothing new is
/// saved: a loaded orb falls from where it was.
pub fn fall(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    if dt <= 0.0 {
        return;
    }
    let placed = world
        .get_resource::<PlacedOrbs>()
        .map(|placed| placed.0.clone())
        .unwrap_or_default();
    let pull = world.get_resource::<Run>().and_then(|run| {
        let radius = world.get::<Stats>(run.player)?.get(Stats::PICKUP_RADIUS)?;
        Some((player::centre(world, run.player)?, radius))
    });
    let falling: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &Orb, &Pickup, &Position, Option<&SaveId>)>()
        .filter(|(_, _, pickup, _, id)| {
            !pickup.is_collected() && !id.is_some_and(|id| placed.contains(&id.0))
        })
        .map(|(entity, _, _, at, _)| (entity, at.0 + ORB_CENTRE))
        .filter(|&(_, centre)| !pull.is_some_and(|(at, radius)| centre.distance(at) <= radius))
        .collect();
    let shape = Shape::Aabb {
        half_extents: ORB_HALF,
    };
    let most = ORB_FALL * dt;
    let filters = rock();
    for (orb, centre) in falling {
        let drop = filters
            .iter()
            .filter_map(|filter| cast_shape(world, shape, centre, Vec2::Y, most, filter))
            .map(|hit| hit.distance)
            .fold(most, f32::min);
        if let (true, Some(position)) = (drop > 0.0, world.get_mut::<Position>(orb)) {
            position.0.y += drop;
        }
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
