//! A [`Harness`] with a `UiTree` laid out after every frame (`D-125`),
//! standing in for the app's layout stage, which needs a renderer, through
//! a [`FixedAdvanceMeasure`] so no font is needed: every glyph advances half
//! the font size. It lays out at the logical viewport, as the app does
//! (`D-127`), and scripts the pointer, the wheel (M57) and keys the UI
//! routes (`D-149`). The frame's extract paints the tree as the app does,
//! one layout behind; [`UiHarness::paint`] paints it now (M46).

use glam::Vec2;
use tungsten_core::ui::{
    FixedAdvanceMeasure, LayoutStats, PaintList, Rect, UiDebug, UiTree, WidgetId,
};
use tungsten_core::{AssetRegistry, KeyCode, MouseButton, ScaleFactor, World};

use super::Harness;
use crate::app::{App, WindowSize};

/// Runs an [`App`]'s frames as [`Harness`] does and lays its `UiTree` out
/// at the logical viewport, `WindowSize` divided by `ScaleFactor::get()`,
/// after each one, so a system reads the previous frame's rects, as under
/// the app's layout stage. Before each frame and each layout it copies
/// `WindowSize` into the `ScaleFactor`'s window size, as the app's window
/// syncs do, so a test that inserts a `WindowSize` gets a window-relative
/// factor (`D-184`).
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
            self.sync_window_size();
            if self.harness.step(1) == 0 {
                return completed;
            }
            self.layout();
        }
        frames
    }

    /// Lays the tree out at the logical viewport, `WindowSize` divided by
    /// `ScaleFactor::get()` as the app's layout stage does, without running
    /// a frame.
    pub fn layout(&mut self) -> LayoutStats {
        self.sync_window_size();
        let scale = self.scale();
        let viewport = self
            .harness
            .world()
            .get_resource::<WindowSize>()
            .map_or(Vec2::ZERO, |size| {
                Vec2::new(size.width as f32, size.height as f32) / scale
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
        self.tree().hit_test(Vec2::new(x, y) / self.scale())
    }

    /// Moves the pointer to (`x`, `y`) in window pixels, as a cursor event
    /// would between redraws; the next frame routes it.
    pub fn pointer_to(&mut self, x: f32, y: f32) {
        self.harness.input_mut().update_cursor_position(x, y);
    }

    /// Presses the primary button where the pointer is.
    pub fn pointer_down(&mut self) {
        self.harness.input_mut().mouse_down(MouseButton::Left);
    }

    /// Releases the primary button.
    pub fn pointer_up(&mut self) {
        self.harness.input_mut().mouse_up(MouseButton::Left);
    }

    /// Turns the wheel `lines` lines where the pointer is, as one vertical
    /// wheel event would between redraws; a positive count scrolls up, so a
    /// scroll panel under the pointer moves its content down (M57).
    pub fn wheel(&mut self, lines: f32) {
        self.harness.input_mut().add_scroll_line_delta(0.0, lines);
    }

    /// Moves the pointer to the centre of `widget`'s last box, then presses
    /// and releases the primary button there: one click for the next frame
    /// to route.
    ///
    /// # Panics
    ///
    /// When `widget` has no box: not laid out, collapsed or removed.
    pub fn click(&mut self, widget: WidgetId) {
        let rect = self
            .rect(widget)
            .unwrap_or_else(|| panic!("UiHarness::click: {widget:?} has no box"));
        let centre = (rect.min + rect.max) * 0.5 * self.scale();
        self.pointer_to(centre.x, centre.y);
        self.pointer_down();
        self.pointer_up();
    }

    /// Presses `key`, as a key event would between redraws.
    pub fn key_down(&mut self, key: KeyCode) {
        self.harness.input_mut().key_down(key);
    }

    /// Releases `key`.
    pub fn key_up(&mut self, key: KeyCode) {
        self.harness.input_mut().key_up(key);
    }

    /// The first widget whose accessible name is `name`, in tree order:
    /// roots in order, each depth first, so a button comes before its
    /// label.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<WidgetId> {
        let tree = self.tree();
        let mut stack: Vec<WidgetId> = tree.roots().iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            if tree.accessible_name(id) == Some(name) {
                return Some(id);
            }
            stack.extend(tree.children(id).iter().rev().copied());
        }
        None
    }

    /// Copies `WindowSize` into the `ScaleFactor`'s window size.
    fn sync_window_size(&mut self) {
        let world = self.harness.world_mut();
        let Some(size) = world.get_resource::<WindowSize>().copied() else {
            return;
        };
        if let Some(scale) = world.get_resource_mut::<ScaleFactor>() {
            scale.set_window_size(size.width, size.height);
        }
    }

    /// Physical pixels per UI unit, as the layout divides by.
    fn scale(&self) -> f32 {
        self.world()
            .get_resource::<ScaleFactor>()
            .map_or(1.0, ScaleFactor::get)
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

    /// Paints the tree now, at scale 1 with the world's `UiDebug` flags and
    /// sprite registry, as the app's extract paints it (M46). Clears the
    /// tree's paint-dirty flag.
    ///
    /// # Panics
    ///
    /// When a test removed the `UiTree` resource.
    pub fn paint(&mut self) -> PaintList {
        let world = self.harness.world_mut();
        let debug = world.get_resource::<UiDebug>().copied().unwrap_or_default();
        let mut tree = world
            .remove_resource::<UiTree>()
            .expect("UiHarness: the UiTree resource was removed");
        let empty = AssetRegistry::new();
        let sprites = world.get_resource::<AssetRegistry>().unwrap_or(&empty);
        let mut list = PaintList::default();
        tree.paint(1.0, &debug, sprites, &mut list);
        world.insert_resource(tree);
        list
    }

    /// [`Self::paint`]'s snapshot: one line per clip and per command, with
    /// the text batch boundaries.
    pub fn dump_paint(&mut self) -> String {
        self.paint().dump()
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

#[cfg(test)]
#[path = "../tests/ui_route.rs"]
mod ui_route;
