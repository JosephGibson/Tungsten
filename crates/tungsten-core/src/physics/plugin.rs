//! The physics plugin: the snapshot and the step at the fixed stage, the
//! one-way, interpolating sync to `Transform` after update, and the
//! collision event queue. A plugin whose every need is met by core, so it
//! lives here (the kit's shape, `D-007`).

use crate::components::Transform;
use crate::ecs::World;
use crate::physics::{CollisionEvent, Position, PrevPosition, physics_step};
use crate::schedule::{Plugin, Schedule, Stage, system};
use crate::time::Time;

/// `physics_prev_snapshot` in `FixedUpdate`, before `physics_step`.
pub const PHYSICS_PREV_SNAPSHOT: &str = "physics_prev_snapshot";
/// `physics_step` in `FixedUpdate`.
pub const PHYSICS_STEP: &str = "physics_step";
/// [`physics_sync`] in `PostUpdate`, before every other engine system there.
pub const PHYSICS_SYNC: &str = "physics_sync";

/// Registers [`PHYSICS_PREV_SNAPSHOT`] and [`PHYSICS_STEP`] after it in
/// `FixedUpdate`, [`PHYSICS_SYNC`] in `PostUpdate` and
/// `EventQueue<CollisionEvent>`. The snapshot is registered first, so it
/// heads the stage ahead of a game's fixed systems, which `DefaultPlugins`
/// builds before. `PhysicsConfig` stays the app's resource, so a game edits
/// it before or after adding the plugin.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<CollisionEvent>();
        schedule.add(
            Stage::FixedUpdate,
            system(PHYSICS_PREV_SNAPSHOT, physics_prev_snapshot),
        );
        schedule.add(
            Stage::FixedUpdate,
            system(PHYSICS_STEP, physics_step).after(PHYSICS_PREV_SNAPSHOT),
        );
        schedule.add(Stage::PostUpdate, system(PHYSICS_SYNC, physics_sync));
    }
}

/// Copies each body's `Position` into its [`PrevPosition`]: the history a
/// fixed step starts from. `PhysicsPlugin` runs it first in `fixed_update`
/// under [`PHYSICS_PREV_SNAPSHOT`], so a step's history spans whatever moves
/// the body in that step, gameplay's writes and the solve.
pub fn physics_prev_snapshot(world: &mut World) {
    world
        .query_mut_slices::<(&mut PrevPosition, &Position)>()
        .for_each(|(_, (prevs, positions))| {
            prevs
                .iter_mut()
                .zip(positions)
                .for_each(|(prev, position)| prev.0 = position.0);
        });
}

/// Draws each body: writes `Transform.position` as
/// `prev + (cur - prev) * alpha` ([`Time::alpha`]) for an entity with a
/// [`PrevPosition`] while [`Time::interpolate`] is on, and as its `Position`
/// otherwise and in a world without `Time`. Never writes `Position`
/// (`D-033`).
pub fn physics_sync(world: &mut World) {
    let alpha = world
        .get_resource::<Time>()
        .filter(|time| time.interpolate())
        .map(Time::alpha);
    // The slice form with `for_each` at both levels keeps the row loop in
    // registers (`D-135`); whether the history column exists is per
    // archetype.
    world
        .query_mut_slices::<(&mut Transform, &Position, Option<&PrevPosition>)>()
        .for_each(|(_, (transforms, positions, prevs))| match (alpha, prevs) {
            (Some(alpha), Some(prevs)) => transforms.iter_mut().zip(positions).zip(prevs).for_each(
                |((transform, position), prev)| {
                    transform.position = prev.0 + (position.0 - prev.0) * alpha;
                },
            ),
            _ => transforms
                .iter_mut()
                .zip(positions)
                .for_each(|(transform, position)| transform.position = position.0),
        });
}

#[cfg(test)]
#[path = "../tests/physics/plugin.rs"]
mod tests;
