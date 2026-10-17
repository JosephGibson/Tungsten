//! The area a [`Wrap`](crate::Wrap) or a [`StayInView`](crate::StayInView)
//! holds its entity in: a world-space box, or the last frame's camera view
//! without its shake, changed by a margin.

use glam::Vec2;
use tungsten_core::{CameraController, CameraState, WindowSize, World};

use crate::foundation::inspect::point;

/// Where a [`Wrap`](crate::Wrap) or a [`StayInView`](crate::StayInView)
/// holds its entity: a world-space box, or the last frame's camera view without its
/// shake, changed by a margin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Area {
    /// The closed box `min..=max`.
    Region { min: Vec2, max: Vec2 },
    /// The camera's view, the margin outside it for a wrap and inside it
    /// for a clamp.
    View { margin: f32 },
}

impl Area {
    /// The box `min..=max`.
    ///
    /// # Panics
    ///
    /// When a coordinate is not finite, or `min` lies above `max` on an
    /// axis; `owner` names the constructor.
    #[track_caller]
    pub(crate) fn new_region(min: Vec2, max: Vec2, owner: &str) -> Self {
        assert!(
            min.is_finite() && max.is_finite(),
            "{owner}: {min:?}..={max:?} is not finite"
        );
        assert!(
            min.x <= max.x && min.y <= max.y,
            "{owner}: {min:?} lies above {max:?} on an axis"
        );
        Self::Region { min, max }
    }

    /// The view with `margin`.
    ///
    /// # Panics
    ///
    /// When `margin` is negative or not finite; `owner` names the
    /// constructor.
    #[track_caller]
    pub(crate) fn new_view(margin: f32, owner: &str) -> Self {
        assert!(
            margin.is_finite() && margin >= 0.0,
            "{owner}: the margin {margin} is negative or not finite"
        );
        Self::View { margin }
    }

    /// The area a scene or a save names: exactly one of `region` and
    /// `view`, checked as the constructors check them.
    pub(crate) fn from_fields(
        region: Option<[[f32; 2]; 2]>,
        view: Option<f32>,
    ) -> Result<Self, String> {
        match (region, view) {
            (Some([min, max]), None) => {
                let (min, max) = (Vec2::from(min), Vec2::from(max));
                if !(min.is_finite() && max.is_finite()) {
                    return Err(format!("region: {min:?}..={max:?} is not finite"));
                }
                if min.x > max.x || min.y > max.y {
                    return Err(format!("region: {min:?} lies above {max:?} on an axis"));
                }
                Ok(Self::Region { min, max })
            }
            (None, Some(margin)) => {
                if margin.is_finite() && margin >= 0.0 {
                    Ok(Self::View { margin })
                } else {
                    Err(format!(
                        "view: the margin {margin} is negative or not finite"
                    ))
                }
            }
            _ => Err("exactly one of `region` and `view`".to_string()),
        }
    }

    /// The area as a scene writes it: its `region` and its `view`.
    pub(crate) fn fields(self) -> (Option<[[f32; 2]; 2]>, Option<f32>) {
        match self {
            Self::Region { min, max } => (Some([min.to_array(), max.to_array()]), None),
            Self::View { margin } => (None, Some(margin)),
        }
    }

    /// The area in a line, for inspector rows.
    pub(crate) fn summary(self) -> String {
        match self {
            Self::Region { min, max } => format!("region {}..{}", point(min), point(max)),
            Self::View { margin } => format!("view, margin {margin}"),
        }
    }
}

/// A box's size from its entity's point: finite and at least 0 on each
/// axis.
pub(crate) fn check_size(size: Vec2) -> Result<(), String> {
    if size.is_finite() && size.x >= 0.0 && size.y >= 0.0 {
        Ok(())
    } else {
        Err(format!("size: must be finite and >= 0 (got {size})"))
    }
}

impl Area {
    /// The box the area spans: the region, or the view grown by its margin
    /// when `grow`, for a wrap, and shrunk by it otherwise, for a clamp.
    /// `None` for a view without a camera and a window size.
    pub(crate) fn bounds(self, view: Option<(Vec2, Vec2)>, grow: bool) -> Option<(Vec2, Vec2)> {
        match self {
            Self::Region { min, max } => Some((min, max)),
            Self::View { margin } => {
                let (min, max) = view?;
                let margin = Vec2::splat(if grow { margin } else { -margin });
                Some((min - margin, max + margin))
            }
        }
    }

    /// The box, when the area is a region, for the debug drawing.
    pub(crate) fn region(self) -> Option<(Vec2, Vec2)> {
        match self {
            Self::Region { min, max } => Some((min, max)),
            Self::View { .. } => None,
        }
    }
}

/// The last frame's camera view without its shake, as a world-space box:
/// the camera's visible box at the `WindowSize`, from its position less the
/// shake offset the camera update last added. A shake never moves what a
/// wrap or a clamp holds. `None` without a `CameraState` or a `WindowSize`.
pub(crate) fn unshaken_view(world: &World) -> Option<(Vec2, Vec2)> {
    let mut camera = *world.get_resource::<CameraState>()?;
    let size = world.get_resource::<WindowSize>()?;
    if let Some(controller) = world.get_resource::<CameraController>() {
        camera.position = controller.resolve_base_position(camera.position);
    }
    Some(camera.visible_world_aabb(size.width as f32, size.height as f32))
}
