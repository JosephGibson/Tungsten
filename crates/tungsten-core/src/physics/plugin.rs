//! The physics plugin: the step at the fixed stage, the one-way sync to
//! `Transform` after update, and the collision event queue. A plugin whose
//! every need is met by core, so it lives here (the kit's shape, `D-007`).

use crate::components::sync_position_to_transform;
use crate::ecs::World;
use crate::physics::{CollisionEvent, physics_step};
use crate::schedule::{Plugin, Schedule, Stage, system};

/// `physics_step` in `FixedUpdate`.
pub const PHYSICS_STEP: &str = "physics_step";
/// `sync_position_to_transform` in `PostUpdate`, before every other engine
/// system there.
pub const PHYSICS_SYNC: &str = "physics_sync";

/// Registers [`PHYSICS_STEP`], [`PHYSICS_SYNC`] and `EventQueue<CollisionEvent>`.
/// `PhysicsConfig` stays the app's resource, so a game edits it before or
/// after adding the plugin.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<CollisionEvent>();
        schedule.add(Stage::FixedUpdate, system(PHYSICS_STEP, physics_step));
        schedule.add(
            Stage::PostUpdate,
            system(PHYSICS_SYNC, sync_position_to_transform),
        );
    }
}
