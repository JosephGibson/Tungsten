//! The camera's target: a marker the camera follows.

use serde::{Deserialize, Serialize};
use tungsten_core::{
    CameraController, CameraMode, Entity, Inspectable, Plugin, Schedule, SpawnContext, With, World,
    system,
};

use crate::foundation::describe::{
    ComponentDescription, GRID, PLATFORMER, SHMUP, TOP_DOWN, describe, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};

/// [`CameraTargetPlugin`]'s system, in the present phase.
pub const CAMERA_TARGET: &str = "camera_target";

/// A marker: while an entity carries it, the camera follows the
/// lowest-indexed live one. [`CameraTargetPlugin`]'s system sets the
/// `CameraController`'s mode to `Follow` of that entity when the mode is
/// `Free` or its own earlier `Follow`, and a mode the game set itself
/// (`Scripted`, or a `Follow` of the game's choosing) is left alone.
/// Remove the marker, or despawn the entity, and a mode still equal to the
/// plugin's `Follow` goes back to `Free`.
///
/// Scene name `camera_target`: `"camera_target": null`.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{
///     CameraController, CameraMode, Plugin, Schedule, Stage, Transform, World,
/// };
/// use tungsten_kit::{CameraTarget, CameraTargetPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(CameraController::default());
/// let mut schedule = Schedule::new();
/// CameraTargetPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let hero = world.spawn_with((Transform::from_position(Vec2::ZERO), CameraTarget));
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// assert_eq!(world.resource::<CameraController>().mode, CameraMode::Follow(hero));
///
/// world.remove_component::<CameraTarget>(hero);
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// assert_eq!(world.resource::<CameraController>().mode, CameraMode::Free);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraTarget;

/// `components.camera_target` as a scene writes it: `null`.
#[derive(Serialize, Deserialize)]
struct CameraTargetDef;

/// Registers the `camera_target` scene component and [`CAMERA_TARGET`] in
/// the present phase, before `camera_update`, so the camera follows the
/// target the same frame. Without a `CameraController` resource it does
/// nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct CameraTargetPlugin;

impl Plugin for CameraTargetPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<CameraTarget>(world, "CameraTarget");
        describe(
            world,
            ComponentDescription::new(
                CAMERA_TARGET,
                "The marker the camera follows: the lowest-indexed live one.",
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .genre(GRID)
            .sample("null")
            .roundtrip(round_trip::<CameraTargetDef>),
        );
        world.register_component_with(CAMERA_TARGET, |_: CameraTargetDef, _: &SpawnContext<'_>| {
            (CameraTarget,)
        });
        let mut following: Option<Entity> = None;
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(CAMERA_TARGET, move |world: &mut World| {
                camera_target(world, &mut following);
            })),
        );
    }
}

/// One frame: `following` is the target the plugin set last.
fn camera_target(world: &mut World, following: &mut Option<Entity>) {
    let target = world
        .query_filtered::<Entity, With<CameraTarget>>()
        .min_by_key(|entity| entity.id());
    let Some(controller) = world.get_resource_mut::<CameraController>() else {
        return;
    };
    let own = following.map(CameraMode::Follow);
    if let Some(target) = target {
        let takes_over = controller.mode == CameraMode::Free || Some(controller.mode) == own;
        if takes_over {
            controller.mode = CameraMode::Follow(target);
            *following = Some(target);
        }
    } else {
        if own.is_some() && Some(controller.mode) == own {
            controller.mode = CameraMode::Free;
        }
        *following = None;
    }
}

impl Inspectable for CameraTarget {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("camera", "follows".to_string())]
    }
}

#[cfg(test)]
#[path = "../tests/camera.rs"]
mod tests;
