//! The training dummy: a straw target on a post that every element and
//! weapon shows on, from the `dummy` prefab, which the map's `dummies`
//! layer places (one in the Lantern clearing, `flesh`, `freezable` and
//! `conductive` by its Tiled properties). Its prefab holds its kit items:
//! a dynamic body, health with no window, so every pulse lands, its death
//! policy, a hurtbox, knockback, the hit flash it shows unlit, resistances,
//! an ailment threshold and its regeneration, which burning holds. 01 adds
//! its friction and the black hole's pull (`toys`), its looks (`combat`)
//! and its end: one burst by its state. A run keeps its placed dummies'
//! save IDs, so a save names those no longer alive as gone. The summon
//! slot places more, [`SUMMON_CAP`] at most, the oldest going.

pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::core::{Entity, EventCursor, Schedule, Shape, Stage, World, system};
use tungsten::kit::{Health, Phase, ensure_phases};
use tungsten::physics::{SpatialFilter, overlap};
use tungsten::{Plugin, SceneEntity};

use crate::features::toys::components::PreviousVelocity;
use crate::game::GAMEPLAY;
use crate::shared::{self, BODIES, Body, SaveId, SaveIds, despawn_tree, spawn_key_burst};
use components::{Dummy, PlacedDummies};

/// The dummy's prefab, by its manifest ID.
pub const DUMMY_PREFAB: &str = "dummy";
/// The most summoned dummies live at once (Q14, Q45).
pub const SUMMON_CAP: usize = 8;

/// The dummy as a plugin: the `dummy` component its prefab names, and
/// `dummy_deaths` in the react phase after combat's reactions.
pub struct DummyPlugin;

impl Plugin for DummyPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Dummy>("dummy");
        world.register_data_kind::<Body>(BODIES);
        ensure_phases(schedule);
        let mut cursor = EventCursor::new();
        let deaths = system("dummy_deaths", move |world: &mut World| {
            systems::deaths(world, &mut cursor);
        })
        .after("combat_reactions");
        schedule.add(Stage::FixedUpdate, Phase::React.join(deaths));
    }
}

/// Spawns a dummy with its sprite's top-left at `at`, owned by gameplay,
/// with the run's next save ID and its body from its data
/// ([`shared::fit_body`]), at once: a summon's or a load's.
pub fn spawn(world: &mut World, at: Vec2) -> Entity {
    let id = world.resource_mut::<SaveIds>().take();
    let dummy = world
        .prepare_prefab(DUMMY_PREFAB, at.to_array())
        .expect("the manifest's prefabs load with it")
        .with((
            id,
            PreviousVelocity::default(),
            SceneEntity { state_id: GAMEPLAY },
        ))
        .spawn(world);
    shared::fit_body(world, dummy, DUMMY_PREFAB);
    dummy
}

/// The dummy's body, as its data holds it now: every dummy prefab's.
#[must_use]
pub fn body(world: &World) -> &Body {
    shared::body(world, DUMMY_PREFAB)
}

/// Whether a dummy's body fits with its centre at `at`: no solid tile in
/// its box.
#[must_use]
pub fn fits(world: &World, at: Vec2) -> bool {
    let mut met = Vec::new();
    let shape = Shape::Aabb {
        half_extents: body(world).half,
    };
    overlap(
        world,
        shape,
        at,
        &SpatialFilter::new().with_bodies(false),
        &mut met,
    );
    met.is_empty()
}

/// Summons a dummy of `prefab` with its body's centre at `at`, with a
/// poof; past [`SUMMON_CAP`] summoned live, the oldest goes, with its own.
pub fn summon(world: &mut World, prefab: &str, at: Vec2) -> Option<Entity> {
    let id = world.resource_mut::<SaveIds>().take();
    let corner = at - body(world).centre;
    let dummy = world
        .prepare_prefab(prefab, corner.to_array())
        .ok()?
        .with((
            id,
            PreviousVelocity::default(),
            SceneEntity { state_id: GAMEPLAY },
        ))
        .spawn(world);
    shared::fit_body(world, dummy, DUMMY_PREFAB);
    spawn_key_burst(world, "ex10_poof", at);
    let summoned: Vec<(u64, Entity)> = live(world)
        .into_iter()
        .filter(|&(id, _)| !placed(world, id))
        .collect();
    for &(_, oldest) in summoned
        .iter()
        .take(summoned.len().saturating_sub(SUMMON_CAP))
    {
        if let Some(centre) = world
            .get::<tungsten::core::Position>(oldest)
            .map(|at| at.0 + body(world).centre)
        {
            spawn_key_burst(world, "ex10_poof", centre);
        }
        despawn_tree(world, oldest);
    }
    Some(dummy)
}

/// Keeps the save IDs of the dummies the map has just placed for a run,
/// and gives each its body from its data ([`shared::fit_body`]) and its
/// velocity before a step, which an impact reads.
pub fn start(world: &mut World) {
    let placed: Vec<(Entity, u64)> = world
        .query::<(Entity, &Dummy, &SaveId)>()
        .map(|(entity, _, id)| (entity, id.0))
        .collect();
    for &(entity, _) in &placed {
        shared::fit_body(world, entity, DUMMY_PREFAB);
        world.insert(entity, PreviousVelocity::default());
    }
    let mut placed: Vec<u64> = placed.into_iter().map(|(_, id)| id).collect();
    placed.sort_unstable();
    world.insert_resource(PlacedDummies(placed));
}

/// The live dummies, alive and not dead, with their save IDs, in save-ID
/// order.
#[must_use]
pub fn live(world: &World) -> Vec<(u64, Entity)> {
    let mut live: Vec<(u64, Entity)> = world
        .query::<(Entity, &Dummy, &SaveId, &Health)>()
        .filter(|(.., health)| !health.is_dead())
        .map(|(entity, _, id, _)| (id.0, entity))
        .collect();
    live.sort_unstable_by_key(|(id, _)| *id);
    live
}

/// Whether the dummy of save ID `id` is one the map placed this run.
#[must_use]
pub fn placed(world: &World, id: u64) -> bool {
    world
        .get_resource::<PlacedDummies>()
        .is_some_and(|placed| placed.0.binary_search(&id).is_ok())
}

/// The save IDs of the map's dummies no longer alive: what a save names as
/// gone, so a load leaves them out.
#[must_use]
pub fn gone(world: &World) -> Vec<u64> {
    let live = live(world);
    world
        .get_resource::<PlacedDummies>()
        .map(|placed| {
            placed
                .0
                .iter()
                .copied()
                .filter(|id| !live.iter().any(|(live, _)| live == id))
                .collect()
        })
        .unwrap_or_default()
}
