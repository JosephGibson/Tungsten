//! The engine's camera update (redo 7.18): the winning rig's view each
//! frame, blended, clamped and shaken (`tungsten_core::camera::update`),
//! then the UI's world-anchored roots placed for it.

use glam::Vec2;
use tungsten_core::ui::UiTree;
use tungsten_core::{CameraState, ScaleFactor, Time, World};

use crate::WindowSize;

/// Draws the frame's view into `CameraState` from the camera's rigs: the
/// highest live `CameraRig` wins (the controller's base rig under the
/// entities' rigs), a switch blending over the incoming rig's time and
/// ease, at the window's size and the game clock's dt; the shake is added
/// after. A write to `CameraState` is never read back, so it lasts only
/// until this runs. Then the UI's world-anchored roots follow the view
/// (redo 7.17). Without a `CameraState` or a `CameraController` it does
/// nothing.
pub fn camera_update_system(world: &mut World) {
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1280,
            height: 720,
        });
    let window = Vec2::new(window.width as f32, window.height as f32);
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let Some(camera) = tungsten_core::camera::update(world, window, dt) else {
        return;
    };
    place_world_anchors(world, camera);
}

/// Places the UI's world-anchored roots for the view `camera` just took
/// (redo 7.17): each root's top-left at its point's screen position, in UI
/// units, plus its offset, inside the viewport. Part of the camera update
/// rather than a system of its own, so it runs after every camera move and
/// adds no system to the frame.
fn place_world_anchors(world: &mut World, camera: CameraState) {
    let scale = world
        .get_resource::<ScaleFactor>()
        .map_or(1.0, ScaleFactor::get);
    let Some(tree) = world.get_resource_mut::<UiTree>() else {
        return;
    };
    tree.place_world_anchors(|point| {
        let at = camera.world_to_screen(point) / scale;
        at.is_finite().then_some(at)
    });
}

#[cfg(test)]
#[path = "tests/camera.rs"]
mod tests;
