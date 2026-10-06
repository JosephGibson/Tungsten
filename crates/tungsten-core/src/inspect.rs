//! Inspector row trait plus canonical engine component impls, and the
//! registry of rows the umbrella's inspector overlay prints.

use crate::components::{Sprite, Tag, Transform, Visibility};
use crate::ecs::{Entity, World};
use crate::physics::{Position, Velocity};

/// Per-component inspector rows.
pub trait Inspectable {
    /// The component's rows, `(label, value)` pairs in display order.
    fn inspect_rows(&self) -> Vec<(&'static str, String)>;
}

/// Rows for one component type of one entity; empty when the entity lacks it.
pub type InspectFn = Box<dyn Fn(&World, Entity) -> Vec<(&'static str, String)>>;

/// The inspector rows registered so far, a `World` resource: plugins and
/// games register here (through core alone), the umbrella's overlay reads
/// it when an entity is picked.
#[derive(Default)]
pub struct InspectRegistry {
    rows: Vec<(&'static str, InspectFn)>,
}

impl InspectRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `T`'s rows under `label`.
    pub fn register<T: 'static + Inspectable>(&mut self, label: &'static str) {
        self.register_with(label, |world: &World, entity: Entity| {
            world
                .get::<T>(entity)
                .map(Inspectable::inspect_rows)
                .unwrap_or_default()
        });
    }

    /// Registers rows computed by `rows`, for a type whose rows need the
    /// world (an asset name, say).
    pub fn register_with(
        &mut self,
        label: &'static str,
        rows: impl Fn(&World, Entity) -> Vec<(&'static str, String)> + 'static,
    ) {
        self.rows.push((label, Box::new(rows)));
    }

    /// The registered labels and row functions, in registration order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &InspectFn)> {
        self.rows.iter().map(|(label, rows)| (*label, rows))
    }

    /// How many types are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
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
            // Core has no registry here; the umbrella inspector prints the name.
            ("asset", format!("#{}", self.asset_id.index())),
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
