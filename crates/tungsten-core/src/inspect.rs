//! Inspector row trait plus canonical engine component impls.

use crate::components::{Sprite, Tag, Transform, Visibility};
use crate::physics::{Position, Velocity};

/// Per-component inspector rows.
pub trait Inspectable {
    fn inspect_rows(&self) -> Vec<(&'static str, String)>;
}

impl Inspectable for Tag {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("name", self.name.clone())]
    }
}

impl Inspectable for Transform {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                "pos",
                format!("({:.2}, {:.2})", self.position.x, self.position.y),
            ),
            ("rot", format!("{:.3}", self.rotation)),
            (
                "scale",
                format!("({:.2}, {:.2})", self.scale.x, self.scale.y),
            ),
        ]
    }
}

impl Inspectable for Visibility {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("visible", self.visible.to_string())]
    }
}

impl Inspectable for Sprite {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("asset", self.asset_id.clone()),
            (
                "tint",
                format!(
                    "[{}, {}, {}, {}]",
                    self.color[0], self.color[1], self.color[2], self.color[3]
                ),
            ),
            ("z", self.z_order.to_string()),
        ]
    }
}

impl Inspectable for Position {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("pos", format!("({:.2}, {:.2})", self.0.x, self.0.y))]
    }
}

impl Inspectable for Velocity {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("vel", format!("({:.2}, {:.2})", self.0.x, self.0.y))]
    }
}

#[cfg(test)]
#[path = "tests/inspect.rs"]
mod tests;
