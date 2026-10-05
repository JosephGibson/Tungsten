//! A [`Harness`] with a `UiTree` laid out after every frame (`D-125`), the
//! way M2's layout stage will run it, through a [`FixedAdvanceMeasure`] so
//! no font is needed: every glyph advances half the font size.

use glam::Vec2;
use tungsten_core::World;
use tungsten_core::ui::{FixedAdvanceMeasure, LayoutStats, Rect, UiTree, WidgetId};

use super::Harness;
use crate::app::{App, WindowSize};

/// Runs an [`App`]'s frames as [`Harness`] does and lays its `UiTree` out
/// at the `WindowSize` viewport after each one, so a system reads the
/// previous frame's rects, as it will under M2's layout stage.
pub struct UiHarness {
    harness: Harness,
    text: FixedAdvanceMeasure,
}

impl UiHarness {
    /// Wraps `app` as [`Harness::new`] does and inserts a `UiTree` when the
    /// world has none, so a game that builds its UI in a system or a startup
    /// path finds the resource it expects.
    #[must_use]
    pub fn new(app: App) -> Self {
        let mut harness = Harness::new(app);
        if !harness.world().has_resource::<UiTree>() {
            harness.world_mut().insert_resource(UiTree::new());
        }
        Self {
            harness,
            text: FixedAdvanceMeasure::default(),
        }
    }

    /// The frame harness underneath.
    #[must_use]
    pub fn harness(&self) -> &Harness {
        &self.harness
    }

    /// The frame harness underneath, for input and dt.
    pub fn harness_mut(&mut self) -> &mut Harness {
        &mut self.harness
    }

    /// The app's world.
    #[must_use]
    pub fn world(&self) -> &World {
        self.harness.world()
    }

    /// The app's world, to seed or change between frames.
    pub fn world_mut(&mut self) -> &mut World {
        self.harness.world_mut()
    }

    /// The UI tree.
    ///
    /// # Panics
    ///
    /// When a test removed the `UiTree` resource.
    #[must_use]
    pub fn tree(&self) -> &UiTree {
        self.world()
            .get_resource::<UiTree>()
            .expect("UiHarness: the UiTree resource was removed")
    }

    /// The UI tree, to build or change between frames.
    ///
    /// # Panics
    ///
    /// When a test removed the `UiTree` resource.
    pub fn tree_mut(&mut self) -> &mut UiTree {
        self.world_mut()
            .get_resource_mut::<UiTree>()
            .expect("UiHarness: the UiTree resource was removed")
    }

    /// Runs `frames` frames, one at a time, laying the tree out after each,
    /// so the second frame of a batch reads the first's rects. Returns the
    /// frames run; stops where [`Harness::step`] stops.
    pub fn step(&mut self, frames: u32) -> u32 {
        for completed in 0..frames {
            if self.harness.step(1) == 0 {
                return completed;
            }
            self.layout();
        }
        frames
    }

    /// Lays the tree out at the `WindowSize` viewport without running a
    /// frame.
    pub fn layout(&mut self) -> LayoutStats {
        let viewport = self
            .harness
            .world()
            .get_resource::<WindowSize>()
            .map_or(Vec2::ZERO, |size| {
                Vec2::new(size.width as f32, size.height as f32)
            });
        let text = &mut self.text;
        self.harness
            .world_mut()
            .get_resource_mut::<UiTree>()
            .expect("UiHarness: the UiTree resource was removed")
            .layout(viewport, text)
    }

    /// The widget under (`x`, `y`) in window pixels, after the last layout.
    #[must_use]
    pub fn hit(&self, x: f32, y: f32) -> Option<WidgetId> {
        self.tree().hit_test(Vec2::new(x, y))
    }

    /// The widget's committed box after the last layout.
    #[must_use]
    pub fn rect(&self, id: WidgetId) -> Option<Rect> {
        self.tree().rect(id)
    }

    /// Focuses `id`; see `UiTree::focus`.
    pub fn focus(&mut self, id: WidgetId) -> bool {
        self.tree_mut().focus(id)
    }

    /// Moves focus forward in tab order; see `UiTree::focus_next`.
    pub fn focus_next(&mut self) -> Option<WidgetId> {
        self.tree_mut().focus_next()
    }

    /// Moves focus backward in tab order; see `UiTree::focus_prev`.
    pub fn focus_prev(&mut self) -> Option<WidgetId> {
        self.tree_mut().focus_prev()
    }

    /// The tree's layout dump after the last layout, one line per node.
    #[must_use]
    pub fn dump(&self) -> String {
        self.tree().dump_layout()
    }

    /// The text double the layouts measure with, for its counts and commits.
    #[must_use]
    pub fn text(&self) -> &FixedAdvanceMeasure {
        &self.text
    }
}

#[cfg(test)]
#[path = "../tests/testing_ui.rs"]
mod tests;
