//! The cursor in world space.

use glam::Vec2;
use tungsten_core::{CameraState, InputState, World};

/// The world point under the cursor: `InputState::cursor_position`, in
/// physical pixels from the window's top-left, through
/// [`CameraState::screen_to_world`], so a moved, zoomed or rotated camera
/// maps it. `None` without a cursor position, an `InputState` or a
/// `CameraState`. It does not check `pointer_in_window`: after the pointer
/// leaves, it maps the last position.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{CameraState, InputState, World};
/// use tungsten_kit::cursor_to_world;
///
/// let mut world = World::new();
/// let mut camera = CameraState::new();
/// camera.position = Vec2::new(100.0, 50.0);
/// camera.zoom = 2.0;
/// world.insert_resource(camera);
/// world.insert_resource(InputState::new());
/// assert_eq!(cursor_to_world(&world), None);
///
/// world.resource_mut::<InputState>().update_cursor_position(60.0, 40.0);
/// assert_eq!(cursor_to_world(&world), Some(Vec2::new(130.0, 70.0)));
/// ```
#[must_use]
pub fn cursor_to_world(world: &World) -> Option<Vec2> {
    let (x, y) = world.get_resource::<InputState>()?.cursor_position()?;
    let camera = world.get_resource::<CameraState>()?;
    Some(camera.screen_to_world(Vec2::new(x, y)))
}

#[cfg(test)]
#[path = "../tests/cursor.rs"]
mod tests;
