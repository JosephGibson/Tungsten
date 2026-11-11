//! A [`Harness`] with a `UiTree` laid out after every frame (`D-125`),
//! standing in for the app's layout stage, which needs a renderer, through
//! a [`FixedAdvanceMeasure`] so no font is needed: every glyph advances half
//! the font size. It lays out at the logical viewport, as the app does
//! (`D-127`), and scripts the pointer, the wheel (M57) and keys the UI
//! routes (`D-149`). The frame's extract paints the tree as the app does,
//! one layout behind; [`UiHarness::paint`] paints it now (M46).

use glam::Vec2;
use tungsten_core::text::TextNodeStore as _;
use tungsten_core::ui::{
    FixedAdvanceMeasure, LayoutStats, PaintList, Rect, UiDebug, UiKey, UiTree, Visibility, WidgetId,
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
    /// Every layout's counts, summed (redo 7.17).
    totals: LayoutStats,
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
            totals: LayoutStats::default(),
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
        let stats = self
            .harness
            .world_mut()
            .get_resource_mut::<UiTree>()
            .expect("UiHarness: the UiTree resource was removed")
            .layout(viewport, text);
        self.totals += stats;
        stats
    }

    /// The counts of every layout this harness ran, summed (redo 7.17): an
    /// idle screen's guard reads that a frame laid out no root.
    #[must_use]
    pub fn layout_totals(&self) -> LayoutStats {
        self.totals
    }

    /// The tree as JSON ([`UiTree::dump_json`]): each node's ID, kind, key,
    /// accessible name, text, rect, focus and whether it shows.
    #[must_use]
    pub fn tree_json(&self) -> String {
        self.tree().dump_json()
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

    /// Moves the pointer to the centre of where `widget` last painted and is
    /// hit (`UiTree::visual_rect`, which maps a debug root's units at its
    /// debug scale), then presses and releases the primary button there: one
    /// click for the next frame to route.
    ///
    /// # Panics
    ///
    /// When `widget` has no box: not laid out, collapsed or removed.
    pub fn click(&mut self, widget: WidgetId) {
        let rect = self
            .tree()
            .visual_rect(widget)
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

    /// Taps `key` (S-119): down for a frame, up the next, both
    /// [`step`](Self::step)ped. Returns the frames run.
    pub fn tap_key(&mut self, key: KeyCode) -> u32 {
        self.key_down(key);
        let pressed = self.step(1);
        self.key_up(key);
        pressed + self.step(1)
    }

    /// Taps `action`'s first binding, as [`Harness::tap_action`] does, with
    /// both frames [`step`](Self::step)ped, so the tree is laid out after
    /// each. Returns the frames run.
    ///
    /// # Panics
    ///
    /// When the action map has no binding for `action`.
    pub fn tap_action(&mut self, action: &str) -> u32 {
        self.harness.press_action(action);
        let pressed = self.step(1);
        self.harness.release_action(action);
        pressed + self.step(1)
    }

    /// The shown roots from the front: the last painted first.
    fn shown_roots(&self) -> impl Iterator<Item = WidgetId> + '_ {
        let tree = self.tree();
        tree.roots()
            .iter()
            .rev()
            .copied()
            .filter(|root| tree.visibility(*root) == Some(Visibility::Visible))
    }

    /// The first of `id`'s subtree, `id` included, depth first, that
    /// `take` takes, through visible widgets only when `visible`.
    fn first_under(
        &self,
        id: WidgetId,
        visible: bool,
        take: impl Fn(&UiTree, WidgetId) -> bool,
    ) -> Option<WidgetId> {
        let tree = self.tree();
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            if visible && tree.visibility(id) != Some(Visibility::Visible) {
                continue;
            }
            if take(tree, id) {
                return Some(id);
            }
            stack.extend(tree.children(id).iter().rev().copied());
        }
        None
    }

    /// The front-most shown modal root (redo 7.17): the screen on top.
    #[must_use]
    pub fn front_modal(&self) -> Option<WidgetId> {
        let tree = self.tree();
        self.shown_roots().find(|root| tree.modal(*root))
    }

    /// The first control named `name` that can take focus, in the shown
    /// roots from the front, each depth first (redo 7.17): a toast's root
    /// above a screen is looked past, as is a covered screen.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<WidgetId> {
        self.shown_roots().find_map(|root| {
            self.first_under(root, true, |tree, id| {
                tree.accessible_name(id) == Some(name) && tree.can_focus(id)
            })
        })
    }

    /// The widget named `name` on [`Self::front_modal`], enabled or not
    /// (redo 7.17).
    #[must_use]
    pub fn named(&self, name: &str) -> Option<WidgetId> {
        let root = self.front_modal()?;
        self.first_under(root, false, |tree, id| {
            tree.accessible_name(id) == Some(name)
        })
    }

    /// Whether a label or a button on [`Self::front_modal`] reads `text`
    /// (redo 7.17).
    #[must_use]
    pub fn shows(&self, text: &str) -> bool {
        self.front_modal().is_some_and(|root| {
            self.first_under(root, false, |tree, id| {
                tree.text(id).is_some() && tree.plain_text(id) == text
            })
            .is_some()
        })
    }

    /// The widget keyed `key` in the shown roots from the front (redo 7.17).
    #[must_use]
    pub fn find_key(&self, key: UiKey) -> Option<WidgetId> {
        self.shown_roots()
            .find_map(|root| self.first_under(root, true, |tree, id| tree.key(id) == Some(key)))
    }

    /// Lays the tree out and panics, naming `what`, when the shown UI does
    /// not fit (redo 7.17): a shown root outside its layer's viewport
    /// (`UiTree::layer_viewport`), a debug root's at the debug scale
    /// (debug-pass); a shown box outside its parent across, or down unless a
    /// scroll panel above it scrolls it; a focusable control outside that
    /// viewport unless a scroll panel above it scrolls it. Measured at the
    /// harness's fixed advance, so glyph widths are approximate.
    pub fn assert_fits(&mut self, what: &str) {
        self.assert_fits_one_line(what, &[]);
    }

    /// [`Self::assert_fits`], and each of `labels` committed on one line.
    pub fn assert_fits_one_line(&mut self, what: &str, labels: &[WidgetId]) {
        self.layout();
        let scale = self.scale();
        let viewport = self
            .harness
            .world()
            .get_resource::<WindowSize>()
            .map_or(Vec2::ZERO, |size| {
                Vec2::new(size.width as f32, size.height as f32) / scale
            });
        let tree = self.tree();
        let inside = |inner: f32, outer: f32| inner >= outer - 0.01;
        let mut checked = 0;
        for &root in tree.roots() {
            if tree.visibility(root) != Some(Visibility::Visible) {
                continue;
            }
            let Some(rect) = tree.rect(root) else {
                continue;
            };
            let viewport = tree
                .layer(root)
                .and_then(|layer| tree.layer_viewport(layer))
                .unwrap_or(viewport);
            let in_view = |rect: Rect| {
                inside(rect.min.x, 0.0)
                    && inside(rect.min.y, 0.0)
                    && inside(viewport.x, rect.max.x)
                    && inside(viewport.y, rect.max.y)
            };
            assert!(
                in_view(rect),
                "{what}: root {root:?} at {rect:?} outside {viewport:?}"
            );
            let mut stack = vec![(root, false)];
            while let Some((id, scrolled)) = stack.pop() {
                let Some(parent) = tree.rect(id) else {
                    continue;
                };
                let scrolls = scrolled || tree.scroll_max(id).is_some();
                for &child in tree.children(id) {
                    if tree.visibility(child) != Some(Visibility::Visible) {
                        continue;
                    }
                    let Some(rect) = tree.rect(child) else {
                        continue;
                    };
                    let across =
                        inside(rect.min.x, parent.min.x) && inside(parent.max.x, rect.max.x);
                    let down = scrolls
                        || (inside(rect.min.y, parent.min.y) && inside(parent.max.y, rect.max.y));
                    assert!(
                        across && down,
                        "{what}: {:?} ({:?}) at {rect:?} outside its parent at {parent:?}",
                        tree.accessible_name(child),
                        tree.plain_text(child)
                    );
                    assert!(
                        scrolls || !tree.focusable(child) || in_view(rect),
                        "{what}: {:?} at {rect:?} outside {viewport:?}",
                        tree.accessible_name(child)
                    );
                    checked += 1;
                    stack.push((child, scrolls));
                }
            }
        }
        assert!(checked > 0, "{what}: nothing shown");
        for &label in labels {
            let lines = tree
                .text_node(label)
                .and_then(|node| self.text.committed(node))
                .map(|metrics| metrics.line_count);
            assert_eq!(
                lines,
                Some(1),
                "{what}: {:?} on one line",
                tree.plain_text(label)
            );
        }
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
