//! `Inspectable` trait (M21): user-supplied label -> value rows rendered by
//! the text-only entity inspector overlay. Blanket impls cover the canonical
//! engine components (`Tag`, `Transform`, `Visibility`, `Sprite`, `Position`,
//! `Velocity`) so consumers get useful output without extra wiring.

use crate::components::{Sprite, Tag, Transform, Visibility};
use crate::physics::{Position, Velocity};

/// Opt-in per-component inspector. Implementors return a list of
/// `(label, rendered value)` pairs; the overlay joins them with newlines.
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
mod tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn tag_emits_single_name_row() {
        let t = Tag::new("hero");
        let rows = t.inspect_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "name");
        assert_eq!(rows[0].1, "hero");
    }

    #[test]
    fn transform_emits_three_rows() {
        let t = Transform {
            position: Vec2::new(1.0, 2.0),
            rotation: 0.5,
            scale: Vec2::new(2.0, 3.0),
        };
        let rows = t.inspect_rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].0, "pos");
        assert_eq!(rows[1].0, "rot");
        assert_eq!(rows[2].0, "scale");
    }

    #[test]
    fn visibility_emits_single_row() {
        let v = Visibility { visible: true };
        let rows = v.inspect_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "visible");
        assert_eq!(rows[0].1, "true");
    }

    #[test]
    fn sprite_emits_three_rows() {
        let s = Sprite::new("player");
        let rows = s.inspect_rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].0, "asset");
        assert_eq!(rows[0].1, "player");
        assert_eq!(rows[1].0, "tint");
        assert_eq!(rows[2].0, "z");
    }

    #[test]
    fn position_emits_single_row() {
        let p = Position(Vec2::new(3.0, 4.0));
        let rows = p.inspect_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "pos");
    }

    #[test]
    fn velocity_emits_single_row() {
        let v = Velocity(Vec2::new(3.0, 4.0));
        let rows = v.inspect_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "vel");
    }
}
