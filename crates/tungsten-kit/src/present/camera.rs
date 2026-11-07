//! The camera's target: a marker the camera's base rig follows.

use serde::{Deserialize, Serialize};
use tungsten_core::{
    CameraController, Entity, Inspectable, Plugin, RigKind, Schedule, SpawnContext, With, World,
    system,
};

use crate::foundation::describe::{
    ComponentDescription, PLATFORMER, RPG, SHOOTER, TOP_DOWN, describe, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};

/// [`CameraTargetPlugin`]'s system, in the present phase.
pub const CAMERA_TARGET: &str = "camera_target";

/// A marker: while an entity carries it, the camera's base rig follows the
/// lowest-indexed live one, the follow rig's shorthand. [`CameraTargetPlugin`]'s
/// system writes that entity into the base rig's follow `target` while the
/// base rig (`CameraController::rig`) is a follow whose target is none or
/// the one it set; a base rig the game made fixed or scripted, or a follow
/// of the game's own choosing, is left alone. Remove the marker, or despawn
/// the entity, and a target still the plugin's is cleared, so the base rig
/// holds where it stands.
///
/// Scene name `camera_target`: `"camera_target": null`.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{CameraController, Plugin, Schedule, Stage, Transform, World};
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
/// assert_eq!(world.resource::<CameraController>().rig.target(), Some(hero));
///
/// world.remove_component::<CameraTarget>(hero);
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// assert_eq!(world.resource::<CameraController>().rig.target(), None);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraTarget;

/// `components.camera_target` as a scene writes it: `null`.
#[derive(Serialize, Deserialize)]
struct CameraTargetDef;

/// Registers the `camera_target` scene component and [`CAMERA_TARGET`] in
/// the present phase, before `camera_update`, so the base rig follows the
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
            .genre(RPG)
            .genre(SHOOTER)
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

/// One frame: `following` is the target the plugin set last. With no
/// target in the World it clears its own and runs no query.
fn camera_target(world: &mut World, following: &mut Option<Entity>) {
    let target = if world.has_any::<CameraTarget>() {
        world
            .query_filtered::<Entity, With<CameraTarget>>()
            .min_by_key(|entity| entity.id())
    } else {
        None
    };
    let Some(controller) = world.get_resource_mut::<CameraController>() else {
        return;
    };
    let RigKind::Follow(follow) = &mut controller.rig.kind else {
        return;
    };
    if let Some(target) = target {
        if follow.target.is_none() || follow.target == *following {
            follow.target = Some(target);
            *following = Some(target);
        }
    } else {
        if following.is_some() && follow.target == *following {
            follow.target = None;
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
