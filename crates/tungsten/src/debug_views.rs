//! The engine's debug views as windows on the core UI (M57, w01 §11 M5):
//! the HUD with its frame-time graph, the system timing overlay, the entity
//! inspector, the log console (F8) and the UI inspector (F7) each keep a
//! root on `UiLayer::Debug`, holding a title bar that drags the window and
//! a body of label columns sized by layout. Every node a view builds sets
//! every style and text override, so a game's theme changes nothing in a
//! view. The log console shows the engine logger's records, which the
//! logger's `Tee` copies into a bounded channel; the UI inspector shows the
//! widget a middle click picked, on any layer.
//!
//! A window opens at the place saved under its name in the user folder's
//! `debug_windows.json` (`WindowPlacements`), else at its view's corner,
//! inset by the view's padding and stacked below (top corners) or above
//! (bottom corners) the views opened earlier at that corner without a saved
//! place. A saved place is clamped again with the window's measured size
//! after its first layout and whenever the logical viewport changes. The
//! end of a drag saves the window's place; only places persist, not which
//! views are open. `App::run_frame` runs [`update`] once a frame, after the
//! schedule and before the UI layout.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::Ordering;

use glam::Vec2;
use tungsten_core::input::{ActionMap, InputState, RawInput};
use tungsten_core::text::{TextAlign, TextOverflow, TextWrap};
use tungsten_core::ui::{
    Anchor, Direction, Edges, LayoutStyle, PaintCommand, PaintList, Placement, Rect, Role, Sizing,
    StyleOverrides, TextOverrides, UiLayer, UiTree, UiVertex, WidgetId, WidgetKind,
    WindowPlacements,
};
use tungsten_core::{EventQueue, ScaleFactor, UiEvent, World};

use crate::app::WindowSize;
use crate::debug_hud::{DebugHud, HudCorner};
use crate::inspector::InspectorState;
use crate::logging::ConsoleFeed;
use crate::systems_overlay::SystemTimingOverlay;

/// The file the debug views' places live in, in the user folder's
/// `settings` folder (Q6).
pub(crate) const DEBUG_WINDOWS_FILE: &str = "debug_windows.json";

/// A view window's default fill.
pub(crate) const BACKGROUND: [u8; 4] = [16, 16, 20, 200];

/// No colour.
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

/// The title bar's fill: a light veil over the window's.
const TITLE_FILL: [u8; 4] = [255, 255, 255, 24];

/// The window's corner radius, in UI units.
const RADIUS: f32 = 4.0;

/// Space inside the title bar and the body, in UI units.
const INSET: f32 = 6.0;

/// Space between columns, in UI units.
const COLUMN_GAP: f32 = 12.0;

/// The share of the logical viewport's height a scrolling body takes at
/// most.
const BODY_SHARE: f32 = 0.45;

/// The frame-time graph's height, in UI units.
const GRAPH_HEIGHT: f32 = 48.0;

/// Frame intervals the graph shows.
pub(crate) const GRAPH_SAMPLES: usize = 240;

/// The graph's lowest top, in milliseconds: two frames at 60 Hz.
const GRAPH_FLOOR_MS: f32 = 33.3;

/// Bars over this many milliseconds, a frame at 60 Hz, draw amber.
const GRAPH_SLOW_MS: f32 = 16.7;

/// A slow bar's colour, and a `WARN` line's.
pub(crate) const GRAPH_SLOW: [u8; 4] = [255, 176, 32, 255];

/// An `ERROR` line's colour.
pub(crate) const ERROR_LINE: [u8; 4] = [255, 96, 96, 255];

/// A view's look, from its public fields: what every node it builds takes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Look {
    pub(crate) family: String,
    pub(crate) size: f32,
    pub(crate) line_height: f32,
    pub(crate) color: [u8; 4],
    pub(crate) background: [u8; 4],
    pub(crate) border: [u8; 4],
    pub(crate) border_width: f32,
}

impl Look {
    /// Every text property, so nothing inherits from a game's theme.
    fn text(&self, align: TextAlign) -> TextOverrides {
        TextOverrides {
            family: Some(self.family.clone()),
            size: Some(self.size),
            line_height: Some(self.line_height),
            weight: Some(400),
            italic: Some(false),
            color: Some(self.color),
            align: Some(align),
            wrap: Some(TextWrap::None),
            overflow: Some(TextOverflow::Clip),
        }
    }

    /// Every style property of a node of `fill` and `border` of `width`.
    fn node(
        &self,
        fill: [u8; 4],
        border: [u8; 4],
        width: f32,
        radius: f32,
        align: TextAlign,
    ) -> StyleOverrides {
        StyleOverrides {
            background: Some(fill),
            border: Some(border),
            border_width: Some(width),
            corner_radius: Some(radius),
            text: self.text(align),
        }
    }

    /// The root: the view's fill inside its border, which `outline_px` of 0
    /// (or less, or NaN) leaves out.
    fn root(&self) -> StyleOverrides {
        let width = if self.border_width > 0.0 {
            self.border_width
        } else {
            0.0
        };
        self.node(
            self.background,
            self.border,
            width,
            RADIUS,
            TextAlign::Start,
        )
    }

    /// Anything inside the window: no fill, no border.
    fn plain(&self, align: TextAlign) -> StyleOverrides {
        self.node(TRANSPARENT, TRANSPARENT, 0.0, 0.0, align)
    }
}

/// What a view's window holds besides its title and columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shape {
    /// Columns, each with its text alignment.
    pub(crate) columns: &'static [TextAlign],
    /// A line above the columns: the inspector's entity.
    pub(crate) header: bool,
    /// The frame-time graph below the columns: the HUD's.
    pub(crate) graph: bool,
    /// A scroll panel body capped at 45% of the viewport's height.
    pub(crate) scroll: bool,
}

/// What one view asks of its window this frame.
pub(crate) struct ViewSpec {
    /// Its key in the placements file.
    pub(crate) name: &'static str,
    /// The title bar's text.
    pub(crate) title: &'static str,
    pub(crate) enabled: bool,
    pub(crate) corner: HudCorner,
    pub(crate) padding: f32,
    pub(crate) look: Look,
    pub(crate) shape: Shape,
}

/// The window one view keeps in the UI tree.
#[derive(Debug)]
pub(crate) struct ViewWindow {
    pub(crate) root: WidgetId,
    pub(crate) body: WidgetId,
    /// The header line, when the shape has one.
    pub(crate) header: Option<WidgetId>,
    /// The column panels inside the body's row.
    pub(crate) columns: Vec<WidgetId>,
    /// The graph, when the shape has one.
    pub(crate) graph: Option<WidgetId>,
    look: Look,
    shape: Shape,
    /// Whether the window keeps a saved place, the file's or a drag's, over
    /// its corner.
    pub(crate) saved: bool,
    /// The corner placement this view last gave the root; another
    /// placement means a drag or the game moved it.
    placed: Option<Placement>,
    /// The logical viewport at the last update, and the one the saved place
    /// was last clamped in.
    seen: Option<Vec2>,
    clamped_in: Option<Vec2>,
}

/// The frame's state the views share.
pub(crate) struct ViewFrame<'a> {
    pub(crate) tree: &'a mut UiTree,
    placements: &'a mut WindowPlacements,
    /// The logical viewport the frame's layout uses.
    pub(crate) logical: Vec2,
    /// Physical pixels per UI unit.
    pub(crate) scale: f32,
    /// Roots whose drag ended at this frame's routing.
    moved: Vec<WidgetId>,
    /// Per corner, the height the unsaved windows placed there so far take.
    stacks: [f32; 4],
    save_warned: bool,
}

impl ViewFrame<'_> {
    /// Builds, keeps, rebuilds or removes `window` for `spec`, saves the end
    /// of a drag and places the window. Returns whether it is open: the
    /// caller then fills it.
    pub(crate) fn sync(&mut self, window: &mut Option<ViewWindow>, spec: &ViewSpec) -> bool {
        if !spec.enabled {
            if let Some(closed) = window.take() {
                self.tree.remove(closed.root);
            }
            return false;
        }
        let stale = window.as_ref().is_some_and(|open| {
            !self.tree.contains(open.root) || open.look != spec.look || open.shape != spec.shape
        });
        if stale && let Some(old) = window.take() {
            self.tree.remove(old.root);
        }
        let open = window.get_or_insert_with(|| self.build(spec));
        if self.moved.contains(&open.root) {
            open.saved = true;
            if let Some(Placement::Anchored { offset, .. }) = self
                .tree
                .layout_style(open.root)
                .map(|style| style.placement)
            {
                self.save(spec.name, offset);
            }
        }
        self.place(open, spec);
        true
    }

    /// A new window for `spec`: its root, title bar, body, columns and
    /// extras, every node styled; at its saved place when the file has one,
    /// else at its corner.
    fn build(&mut self, spec: &ViewSpec) -> ViewWindow {
        let look = &spec.look;
        let tree = &mut *self.tree;
        let root = tree.add_root(
            UiLayer::Debug,
            LayoutStyle {
                placement: corner_placement(spec.corner, Vec2::splat(spec.padding)),
                ..LayoutStyle::default()
            },
        );
        tree.set_overrides(root, look.root());
        tree.set_clip_children(root, true);
        tree.set_role(root, Role::Window);
        tree.set_accessible_name(root, Some(spec.title.to_string()));

        let title = tree.panel(
            root,
            LayoutStyle {
                direction: Direction::Row,
                padding: Edges::symmetric(INSET, INSET * 0.5),
                ..LayoutStyle::default()
            },
        );
        tree.set_overrides(
            title,
            look.node(TITLE_FILL, TRANSPARENT, 0.0, RADIUS, TextAlign::Start),
        );
        tree.set_drag_handle(title, true);
        let caption = tree.label(title, spec.title);
        tree.set_overrides(caption, look.plain(TextAlign::Start));

        let body_style = LayoutStyle {
            padding: Edges::all(INSET),
            ..LayoutStyle::default()
        };
        let body = if spec.shape.scroll {
            tree.scroll_panel(root, body_style)
        } else {
            tree.panel(root, body_style)
        };
        tree.set_overrides(body, look.plain(TextAlign::Start));
        let header = spec.shape.header.then(|| {
            let header = tree.label(body, "");
            tree.set_overrides(header, look.plain(TextAlign::Start));
            header
        });
        let row = tree.panel(body, LayoutStyle::row().with_gap(COLUMN_GAP));
        tree.set_overrides(row, look.plain(TextAlign::Start));
        let columns = spec
            .shape
            .columns
            .iter()
            .map(|align| {
                let column = tree.panel(row, LayoutStyle::column());
                tree.set_overrides(column, look.plain(*align));
                column
            })
            .collect();
        let graph = spec.shape.graph.then(|| {
            let graph = tree.mesh(
                body,
                LayoutStyle {
                    height: Sizing::Px(GRAPH_HEIGHT),
                    margin: Edges {
                        top: INSET,
                        ..Edges::default()
                    },
                    ..LayoutStyle::default()
                },
            );
            tree.set_overrides(graph, look.plain(TextAlign::Start));
            graph
        });

        let mut saved = false;
        if let Some(position) = self.placements.get(spec.name) {
            saved = true;
            tree.set_window_position(root, position);
        }
        ViewWindow {
            root,
            body,
            header,
            columns,
            graph,
            look: spec.look.clone(),
            shape: spec.shape,
            saved,
            placed: None,
            seen: None,
            clamped_in: None,
        }
    }

    /// Places `window`: an unsaved one at its corner, below or above the
    /// unsaved windows placed there before it; a saved one where it is,
    /// clamped again with its measured size after a layout at a viewport it
    /// was not clamped in. A window whose placement changed since the view
    /// last set it, a drag under way or the game's, keeps it from then on.
    /// Caps the root's width, and a scrolling body's height.
    fn place(&mut self, window: &mut ViewWindow, spec: &ViewSpec) {
        let current = self
            .tree
            .layout_style(window.root)
            .map(|style| style.placement);
        if !window.saved && window.placed.is_some() && current != window.placed {
            window.saved = true;
        }
        let padding = spec.padding.max(0.0);
        let max_width = (self.logical.x - 2.0 * padding).max(0.0);
        let stack = &mut self.stacks[corner_index(spec.corner)];
        let placement = (!window.saved)
            .then(|| corner_placement(spec.corner, Vec2::new(padding, padding + *stack)));
        if placement.is_some() {
            window.placed = placement;
        }
        restyle(self.tree, window.root, |style| {
            if let Some(placement) = placement {
                style.placement = placement;
            }
            style.max_width = Some(max_width);
        });
        if spec.shape.scroll {
            let max_height = (self.logical.y * BODY_SHARE).max(0.0);
            restyle(self.tree, window.body, |style| {
                style.max_height = Some(max_height);
            });
        }
        if window.saved {
            // The previous frame's layout ran at `logical` when the view saw
            // it then too: only that layout's size clamps right.
            if window.clamped_in != Some(self.logical)
                && window.seen == Some(self.logical)
                && self.tree.rect(window.root).is_some()
            {
                if let Some(Placement::Anchored { offset, .. }) = self
                    .tree
                    .layout_style(window.root)
                    .map(|style| style.placement)
                {
                    self.tree.set_window_position(window.root, offset);
                }
                window.clamped_in = Some(self.logical);
            }
        } else if let Some(rect) = self.tree.rect(window.root) {
            *stack += padding + rect.height();
        }
        window.seen = Some(self.logical);
    }

    /// Keeps `position` for `name` and writes the file; a failed write
    /// warns once per run and keeps the place in memory.
    fn save(&mut self, name: &str, position: Vec2) {
        if !self.placements.set(name, position) {
            return;
        }
        if let Err(err) = self.placements.save()
            && !self.save_warned
        {
            self.save_warned = true;
            log::warn!("Debug views: could not save the window places: {err}; they last this run");
        }
    }
}

/// Changes `id`'s layout style through `change`; an equal style marks
/// nothing.
fn restyle(tree: &mut UiTree, id: WidgetId, change: impl FnOnce(&mut LayoutStyle)) {
    let Some(mut style) = tree.layout_style(id).cloned() else {
        return;
    };
    change(&mut style);
    tree.set_layout_style(id, style);
}

/// The anchored placement of a window at `corner`, `offset` in from it.
fn corner_placement(corner: HudCorner, offset: Vec2) -> Placement {
    let anchor = match corner {
        HudCorner::TopLeft => Anchor::TopStart,
        HudCorner::TopRight => Anchor::TopEnd,
        HudCorner::BottomLeft => Anchor::BottomStart,
        HudCorner::BottomRight => Anchor::BottomEnd,
    };
    Placement::Anchored { anchor, offset }
}

fn corner_index(corner: HudCorner) -> usize {
    match corner {
        HudCorner::TopLeft => 0,
        HudCorner::TopRight => 1,
        HudCorner::BottomLeft => 2,
        HudCorner::BottomRight => 3,
    }
}

impl ViewWindow {
    /// A new label at the end of column `c`, in the view's look, its text
    /// in `color` when one is given.
    fn add_label(
        &self,
        tree: &mut UiTree,
        c: usize,
        text: &str,
        color: Option<[u8; 4]>,
    ) -> WidgetId {
        let label = tree.label(self.columns[c], text);
        let mut overrides = self.look.plain(self.shape.columns[c]);
        if let Some(color) = color {
            overrides.text.color = Some(color);
        }
        tree.set_overrides(label, overrides);
        label
    }
}

/// Writes `rows` into `window`'s columns, cell `c` of each row into column
/// `c`, a missing cell empty; adds or removes labels to match.
pub(crate) fn write_rows(tree: &mut UiTree, window: &ViewWindow, rows: &[Vec<String>]) {
    for (c, column) in window.columns.iter().enumerate() {
        let labels = tree.children(*column).to_vec();
        for (i, row) in rows.iter().enumerate() {
            let text = row.get(c).map_or("", String::as_str);
            if let Some(label) = labels.get(i) {
                tree.set_plain_text(*label, text);
            } else {
                window.add_label(tree, c, text, None);
            }
        }
        for label in labels.iter().skip(rows.len()) {
            tree.remove(*label);
        }
    }
}

/// The graph's triangles into `out`: one bar per interval, oldest left and
/// the newest at the right edge of [`GRAPH_SAMPLES`] slots, 6 vertices
/// each, rising from the bottom against the larger of 33.3 ms and the
/// tallest shown, amber over 16.7 ms.
pub(crate) fn graph_vertices(samples: &[f32], color: [u8; 4], out: &mut Vec<UiVertex>) {
    out.clear();
    let shown = &samples[samples.len().saturating_sub(GRAPH_SAMPLES)..];
    let top = shown.iter().copied().fold(GRAPH_FLOOR_MS, f32::max);
    let first = GRAPH_SAMPLES - shown.len();
    let width = 1.0 / GRAPH_SAMPLES as f32;
    for (i, ms) in shown.iter().enumerate() {
        let x0 = (first + i) as f32 * width;
        let x1 = x0 + width;
        let y0 = 1.0 - (ms / top).clamp(0.0, 1.0);
        let color = if *ms > GRAPH_SLOW_MS {
            GRAPH_SLOW
        } else {
            color
        };
        for (x, y) in [
            (x0, y0),
            (x1, y0),
            (x1, 1.0),
            (x0, y0),
            (x1, 1.0),
            (x0, 1.0),
        ] {
            out.push(UiVertex::new(Vec2::new(x, y), color));
        }
    }
}

/// [`update_with`] without the app's paint list, which only the UI
/// inspector's batch row reads.
#[cfg(test)]
pub(crate) fn update(world: &mut World) {
    update_with(world, None);
}

/// The views' frame (M57): the log console drains the logger's records,
/// shown or not; then each view builds, updates or removes its window and
/// fills it, and a drag's end saves the window's place. `paint` is the
/// app's last paint list. Past the drain, a world whose views are all off
/// and closed returns at once.
pub(crate) fn update_with(world: &mut World, paint: Option<&PaintList>) {
    if let Some(console) = world.get_resource_mut::<LogConsole>() {
        console.drain();
    }
    let active = world
        .get_resource::<DebugHud>()
        .is_some_and(|hud| hud.enabled || hud.window.is_some())
        || world
            .get_resource::<SystemTimingOverlay>()
            .is_some_and(|overlay| overlay.enabled || overlay.window.is_some())
        || world
            .get_resource::<InspectorState>()
            .is_some_and(|state| state.enabled || state.window.is_some())
        || world
            .get_resource::<LogConsole>()
            .is_some_and(|console| console.enabled || console.window.is_some())
        || world
            .get_resource::<UiInspector>()
            .is_some_and(|inspector| inspector.enabled || inspector.window.is_some());
    if !active {
        return;
    }
    let Some(mut tree) = world.remove_resource::<UiTree>() else {
        return;
    };
    let mut placements = world
        .remove_resource::<WindowPlacements>()
        .unwrap_or_default();
    let scale = world
        .get_resource::<ScaleFactor>()
        .map_or(1.0, ScaleFactor::get);
    let logical = world
        .get_resource::<WindowSize>()
        .map_or(Vec2::ZERO, |size| {
            Vec2::new(size.width as f32, size.height as f32) / scale
        });
    let moved = world
        .get_resource::<EventQueue<UiEvent>>()
        .map(|queue| {
            queue
                .iter()
                .filter_map(|event| match *event {
                    UiEvent::WindowMoved { window, .. } => Some(window),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let save_warned = world
        .get_resource::<ViewsState>()
        .is_some_and(|state| state.save_warned);
    let mut frame = ViewFrame {
        tree: &mut tree,
        placements: &mut placements,
        logical,
        scale,
        moved,
        stacks: [0.0; 4],
        save_warned,
    };
    if let Some(mut hud) = world.remove_resource::<DebugHud>() {
        hud.update_window(world, &mut frame);
        world.insert_resource(hud);
    }
    if let Some(mut overlay) = world.remove_resource::<SystemTimingOverlay>() {
        overlay.update_window(world, &mut frame);
        world.insert_resource(overlay);
    }
    if let Some(mut state) = world.remove_resource::<InspectorState>() {
        state.update_window(world, &mut frame);
        world.insert_resource(state);
    }
    if let Some(mut console) = world.remove_resource::<LogConsole>() {
        console.update_window(world, &mut frame);
        world.insert_resource(console);
    }
    if let Some(mut inspector) = world.remove_resource::<UiInspector>() {
        inspector.update_window(world, &mut frame, paint);
        world.insert_resource(inspector);
    }
    let save_warned = frame.save_warned;
    world.insert_resource(ViewsState { save_warned });
    world.insert_resource(placements);
    world.insert_resource(tree);
}

/// What the views keep between frames outside their resources.
#[derive(Debug, Default)]
struct ViewsState {
    /// Whether a failed save of the places has warned this run.
    save_warned: bool,
}

/// A view's look from its public fields.
macro_rules! look_of {
    ($view:expr) => {
        Look {
            family: $view.font_id.clone(),
            size: $view.font_size,
            line_height: $view.line_height,
            color: $view.color,
            background: $view.background,
            border: $view.outline_color,
            border_width: $view.outline_px,
        }
    };
}

/// The log console (M57): the engine logger's records, newest last, in a
/// window on the UI's debug layer; toggled by `engine_toggle_log_console`
/// (F8). It drains the logger every frame, shown or not, into a ring of
/// [`capacity`](Self::capacity) lines, oldest out. Its window shows `N
/// lines · D dropped` (records the logger's full channel dropped), or `no
/// engine logger` when the app installed none, over the lines, an `ERROR`
/// line red and a `WARN` line amber; its body follows the newest line
/// until the wheel moves it off, and again once the wheel reaches the end.
pub struct LogConsole {
    /// Whether the window shows; the drain runs either way.
    pub enabled: bool,
    /// The corner the window opens at when no place is saved for it.
    pub corner: HudCorner,
    /// The UI font family the window's text takes: the engine font's,
    /// [`ENGINE_FONT_ID`](crate::ENGINE_FONT_ID), by default.
    pub font_id: String,
    /// Text size in UI units.
    pub font_size: f32,
    /// Line height in UI units.
    pub line_height: f32,
    /// The lines' colour, sRGB; `ERROR` and `WARN` lines take their own.
    pub color: [u8; 4],
    /// The window's fill.
    pub background: [u8; 4],
    /// The window's border colour.
    pub outline_color: [u8; 4],
    /// The window's border width in UI units; `0.0` draws none.
    pub outline_px: f32,
    /// Space between the window and its corner, in UI units.
    pub padding_px: f32,
    /// Lines kept, oldest out; 512.
    pub capacity: usize,
    /// How often the window takes new lines, on the frame interval; 100 ms.
    pub refresh_interval_ms: f32,
    feed: Option<ConsoleFeed>,
    /// The kept lines, oldest first, each with its colour.
    lines: VecDeque<(String, Option<[u8; 4]>)>,
    /// Lines received since the start, and when the window last took new
    /// ones.
    received: u64,
    shown: u64,
    /// The labels showing the lines, oldest first, and the root they are in.
    labels: VecDeque<WidgetId>,
    labels_in: Option<WidgetId>,
    time_since_refresh_ms: f32,
    pub(crate) window: Option<ViewWindow>,
}

impl Default for LogConsole {
    /// Off, with no logger feed: the window says `no engine logger`.
    fn default() -> Self {
        Self {
            enabled: false,
            corner: HudCorner::TopLeft,
            font_id: crate::ENGINE_FONT_ID.to_string(),
            font_size: 28.0,
            line_height: 32.0,
            color: [240, 240, 240, 240],
            background: BACKGROUND,
            outline_color: [0, 0, 0, 220],
            outline_px: 1.5,
            padding_px: 12.0,
            capacity: 512,
            refresh_interval_ms: 100.0,
            feed: None,
            lines: VecDeque::new(),
            received: 0,
            shown: 0,
            labels: VecDeque::new(),
            labels_in: None,
            time_since_refresh_ms: f32::INFINITY,
            window: None,
        }
    }
}

/// The log console's columns: the lines.
const CONSOLE_COLUMNS: &[TextAlign] = &[TextAlign::Start];

impl LogConsole {
    /// A console draining `feed`, the engine logger's when the app
    /// installed one.
    pub(crate) fn with_feed(feed: Option<ConsoleFeed>) -> Self {
        Self {
            feed,
            ..Self::default()
        }
    }

    /// Shows or hides the window.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// The window's root while the console is shown.
    #[must_use]
    pub fn window(&self) -> Option<WidgetId> {
        self.window.as_ref().map(|window| window.root)
    }

    /// The kept lines, oldest first.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(|(line, _)| line.as_str())
    }

    /// Records the logger dropped on a full or closed channel; 0 without a
    /// logger.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.feed
            .as_ref()
            .map_or(0, |feed| feed.dropped.load(Ordering::Relaxed))
    }

    /// Takes the logger's new records into the ring, oldest out.
    fn drain(&mut self) {
        let Some(feed) = &self.feed else {
            return;
        };
        while let Ok(record) = feed.records.try_recv() {
            let color = level_color(&record);
            self.lines.push_back((record, color));
            self.received += 1;
        }
        while self.lines.len() > self.capacity {
            self.lines.pop_front();
        }
    }

    /// The console's frame: on its interval, the new lines into the window
    /// and the header; while it is shown.
    fn update_window(&mut self, world: &World, frame: &mut ViewFrame<'_>) {
        let spec = ViewSpec {
            name: "log",
            title: "Log",
            enabled: self.enabled,
            corner: self.corner,
            padding: self.padding_px,
            look: look_of!(self),
            shape: Shape {
                columns: CONSOLE_COLUMNS,
                header: true,
                graph: false,
                scroll: true,
            },
        };
        if !frame.sync(&mut self.window, &spec) {
            self.labels.clear();
            self.labels_in = None;
            self.time_since_refresh_ms = f32::INFINITY;
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if self.labels_in != Some(window.root) {
            // A new window: every kept line is new to it, and its body
            // follows the end.
            self.labels.clear();
            self.labels_in = Some(window.root);
            self.shown = self.received - self.lines.len() as u64;
            frame.tree.set_scroll(window.body, f32::INFINITY);
            self.time_since_refresh_ms = f32::INFINITY;
        }
        self.time_since_refresh_ms += frame_interval(world);
        if self.time_since_refresh_ms < self.refresh_interval_ms {
            return;
        }
        self.time_since_refresh_ms = 0.0;
        let new = usize::try_from(self.received - self.shown)
            .unwrap_or(usize::MAX)
            .min(self.lines.len());
        let keep = self.lines.len() - new;
        while self.labels.len() > keep {
            if let Some(label) = self.labels.pop_front() {
                frame.tree.remove(label);
            }
        }
        for (line, color) in self.lines.iter().skip(keep) {
            self.labels
                .push_back(window.add_label(frame.tree, 0, line, *color));
        }
        self.shown = self.received;
        if let Some(header) = window.header {
            let text = if self.feed.is_some() {
                format!("{} lines · {} dropped", self.lines.len(), self.dropped())
            } else {
                "no engine logger".to_string()
            };
            frame.tree.set_plain_text(header, &text);
        }
    }
}

/// A record's colour by the level token after the timestamp in
/// `env_logger`'s format, `[<time> <LEVEL> <target>] …`: red for `ERROR`,
/// amber for `WARN`, the view's for anything else.
pub(crate) fn level_color(record: &str) -> Option<[u8; 4]> {
    let head = record.strip_prefix('[')?;
    match head.split_whitespace().nth(1)? {
        "ERROR" => Some(ERROR_LINE),
        "WARN" => Some(GRAPH_SLOW),
        _ => None,
    }
}

/// The previous frame's interval, 0 when it has none.
fn frame_interval(world: &World) -> f32 {
    world
        .get_resource::<crate::telemetry::FrameTimings>()
        .and_then(|ft| ft.interval_ms)
        .filter(|ms| ms.is_finite() && *ms >= 0.0)
        .unwrap_or(0.0)
}

/// Toggles the log console on `engine_toggle_log_console` (F8).
pub(crate) fn log_console_toggle_system(world: &mut World) {
    if just_pressed(world, "engine_toggle_log_console")
        && let Some(console) = world.get_resource_mut::<LogConsole>()
    {
        console.toggle();
    }
}

/// Toggles the UI inspector on `engine_toggle_ui_inspector` (F7).
pub(crate) fn ui_inspector_toggle_system(world: &mut World) {
    if just_pressed(world, "engine_toggle_ui_inspector")
        && let Some(inspector) = world.get_resource_mut::<UiInspector>()
    {
        inspector.toggle();
    }
}

/// Whether `action` was just pressed in the raw input.
fn just_pressed(world: &World, action: &str) -> bool {
    let (Some(input), Some(actions)) = (
        world.get_resource::<InputState>(),
        world.get_resource::<ActionMap>(),
    ) else {
        return false;
    };
    actions.just_pressed(input, action)
}

/// The UI inspector (M57): the widget a middle click picked, on any layer,
/// debug windows included, in a window on the UI's debug layer; toggled by
/// `engine_toggle_ui_inspector` (F7). While it is shown, a press of
/// `engine_inspector_pick` in the raw input with the pointer over a widget
/// picks it; a press elsewhere picks nothing new, and a removed widget
/// clears the pick. Its rows: the widget, its layout and visual rects, its
/// look, a label's or button's text style, its state, its flags, the work
/// it waits on and the text batch of its run in the app's last paint list.
pub struct UiInspector {
    /// Whether the window shows and a middle click picks.
    pub enabled: bool,
    /// The corner the window opens at when no place is saved for it: below
    /// the HUD's by default.
    pub corner: HudCorner,
    /// The UI font family the window's text takes: the engine font's,
    /// [`ENGINE_FONT_ID`](crate::ENGINE_FONT_ID), by default.
    pub font_id: String,
    /// Text size in UI units.
    pub font_size: f32,
    /// Line height in UI units.
    pub line_height: f32,
    /// The text colour, sRGB.
    pub color: [u8; 4],
    /// The window's fill.
    pub background: [u8; 4],
    /// The window's border colour.
    pub outline_color: [u8; 4],
    /// The window's border width in UI units; `0.0` draws none.
    pub outline_px: f32,
    /// Space between the window and its corner, in UI units.
    pub padding_px: f32,
    /// Row refresh interval on the frame interval; a new pick refreshes at
    /// once.
    pub refresh_interval_ms: f32,
    /// The widget whose rows show.
    pub picked: Option<WidgetId>,
    shown_picked: Option<WidgetId>,
    time_since_refresh_ms: f32,
    /// Where the cursor ended the previous frame it was shown, physical
    /// pixels: a press with no move before it picks there.
    last_cursor: Option<(f32, f32)>,
    pub(crate) window: Option<ViewWindow>,
}

impl Default for UiInspector {
    fn default() -> Self {
        Self {
            enabled: false,
            corner: HudCorner::TopRight,
            font_id: crate::ENGINE_FONT_ID.to_string(),
            font_size: 28.0,
            line_height: 32.0,
            color: [240, 240, 240, 240],
            background: BACKGROUND,
            outline_color: [0, 0, 0, 220],
            outline_px: 1.5,
            padding_px: 12.0,
            refresh_interval_ms: 500.0,
            picked: None,
            shown_picked: None,
            time_since_refresh_ms: f32::INFINITY,
            last_cursor: None,
            window: None,
        }
    }
}

/// The UI inspector's columns: names right-aligned, then values.
const UI_INSPECTOR_COLUMNS: &[TextAlign] = &[TextAlign::End, TextAlign::Start];

impl UiInspector {
    /// Shows or hides the window.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// The window's root while the UI inspector is shown.
    #[must_use]
    pub fn window(&self) -> Option<WidgetId> {
        self.window.as_ref().map(|window| window.root)
    }

    /// The UI inspector's frame: the pick, then on its interval, or at once
    /// for a new pick, the rows; while it is shown.
    fn update_window(
        &mut self,
        world: &World,
        frame: &mut ViewFrame<'_>,
        paint: Option<&PaintList>,
    ) {
        let spec = ViewSpec {
            name: "ui_inspector",
            title: "UI",
            enabled: self.enabled,
            corner: self.corner,
            padding: self.padding_px,
            look: look_of!(self),
            shape: Shape {
                columns: UI_INSPECTOR_COLUMNS,
                header: true,
                graph: false,
                scroll: true,
            },
        };
        if !frame.sync(&mut self.window, &spec) {
            self.time_since_refresh_ms = f32::INFINITY;
            return;
        }
        self.pick(world, frame.tree, frame.scale);
        let Some(window) = self.window.as_ref() else {
            return;
        };
        self.time_since_refresh_ms += frame_interval(world);
        if self.time_since_refresh_ms < self.refresh_interval_ms && self.shown_picked == self.picked
        {
            return;
        }
        self.time_since_refresh_ms = 0.0;
        self.shown_picked = self.picked;
        let (header, rows) = match self.picked {
            Some(widget) => (
                format!("widget {}:{}", widget.index(), widget.generation()),
                widget_rows(frame.tree, widget, paint),
            ),
            None => ("middle-click a widget to pick".to_string(), Vec::new()),
        };
        if let Some(line) = window.header {
            frame.tree.set_plain_text(line, &header);
        }
        write_rows(frame.tree, window, &rows);
    }

    /// Clears a removed widget's pick, then picks the widget under a press
    /// of `engine_inspector_pick` in the raw input, at the press's position.
    fn pick(&mut self, world: &World, tree: &UiTree, scale: f32) {
        if self.picked.is_some_and(|widget| !tree.contains(widget)) {
            self.picked = None;
        }
        let (Some(input), Some(actions)) = (
            world.get_resource::<InputState>(),
            world.get_resource::<ActionMap>(),
        ) else {
            return;
        };
        let press = press_position(input, actions, self.last_cursor);
        self.last_cursor = input.cursor_position();
        let Some((x, y)) = press else {
            return;
        };
        let scale = if scale > 0.0 { scale } else { 1.0 };
        if let Some(widget) = tree.hit_test(Vec2::new(x, y) / scale) {
            self.picked = Some(widget);
        }
    }
}

/// Where this frame's first press of `engine_inspector_pick` in `input`'s
/// stream happened: the last pointer move before it, else `last_cursor`.
fn press_position(
    input: &InputState,
    actions: &ActionMap,
    last_cursor: Option<(f32, f32)>,
) -> Option<(f32, f32)> {
    let bindings = actions.bindings("engine_inspector_pick");
    let mut at = last_cursor;
    for event in input.raw_events() {
        match event.input {
            RawInput::PointerMoved { x, y, .. } => at = Some((x, y)),
            press if crate::app::presses(bindings, press) => return at,
            _ => {}
        }
    }
    None
}

/// `rect` as `(x, y) w×h`, to one decimal.
fn rect_text(rect: Option<Rect>) -> String {
    rect.map_or_else(
        || "-".to_string(),
        |rect| {
            format!(
                "({:.1}, {:.1}) {:.1}×{:.1}",
                rect.min.x,
                rect.min.y,
                rect.width(),
                rect.height()
            )
        },
    )
}

fn hex(color: [u8; 4]) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        color[0], color[1], color[2], color[3]
    )
}

/// The UI inspector's rows for `widget`.
fn widget_rows(tree: &UiTree, widget: WidgetId, paint: Option<&PaintList>) -> Vec<Vec<String>> {
    let row = |name: &str, value: String| vec![name.to_string(), value];
    let mut rows = Vec::new();
    let kind = tree.kind(widget);
    let root = tree.root_of(widget);
    let debug = |value: Option<String>| value.unwrap_or_else(|| "-".to_string());
    rows.push(row(
        "widget",
        format!(
            "{}:{} {} {} {} root {}",
            widget.index(),
            widget.generation(),
            debug(kind.map(|kind| format!("{kind:?}"))),
            debug(tree.role(widget).map(|role| format!("{role:?}"))),
            debug(tree.layer(widget).map(|layer| format!("{layer:?}"))),
            debug(root.map(|root| format!("{}:{}", root.index(), root.generation()))),
        ),
    ));
    rows.push(row("layout", rect_text(tree.rect(widget))));
    rows.push(row("visual", rect_text(tree.visual_rect(widget))));
    if let Some(style) = tree.resolved_style(widget) {
        rows.push(row(
            "look",
            format!(
                "bg {} border {} {:.1} r {:.1}",
                hex(style.background),
                hex(style.border),
                style.border_width,
                style.corner_radius
            ),
        ));
        if matches!(kind, Some(WidgetKind::Label | WidgetKind::Button)) {
            let text = &style.text;
            rows.push(row(
                "text",
                format!(
                    "{} {:.1}/{:.1} {}",
                    text.family,
                    text.size,
                    text.line_height,
                    hex(text.color)
                ),
            ));
        }
    }
    if let Some(state) = tree.widget_state(widget) {
        rows.push(row("state", format!("{state:?}")));
    }
    let mut flags = String::new();
    for (on, name) in [
        (tree.interactive(widget), "interactive"),
        (tree.focusable(widget), "focusable"),
        (tree.hit_testable(widget), "hit"),
        (tree.clips_children(widget), "clips"),
        (tree.scroll(widget).is_some(), "scroll"),
        (tree.drag_handle(widget), "drag"),
    ] {
        if on {
            if !flags.is_empty() {
                flags.push(' ');
            }
            flags.push_str(name);
        }
    }
    rows.push(row(
        "flags",
        if flags.is_empty() { "-".into() } else { flags },
    ));
    let mut pending = String::new();
    if let Some(dirty) = tree.widget_dirty(widget) {
        for (on, name) in [
            (dirty.style, "style"),
            (dirty.text, "text"),
            (dirty.layout, "layout"),
        ] {
            if on {
                let _ = write!(
                    pending,
                    "{}{name}",
                    if pending.is_empty() { "" } else { " " }
                );
            }
        }
    }
    rows.push(row(
        "pending",
        if pending.is_empty() {
            "none".into()
        } else {
            pending
        },
    ));
    rows.push(row("batch", text_batch(tree, widget, paint)));
    rows
}

/// The text batch of `widget`'s run in `paint`: a label's, a button's
/// label's; `none` for anything else or a run not in the list.
fn text_batch(tree: &UiTree, widget: WidgetId, paint: Option<&PaintList>) -> String {
    let holder = match tree.kind(widget) {
        Some(WidgetKind::Label) => Some(widget),
        Some(WidgetKind::Button) => tree.children(widget).first().copied(),
        _ => None,
    };
    let (Some(node), Some(paint)) = (holder.and_then(|label| tree.text_node(label)), paint) else {
        return "none".into();
    };
    let Some(at) = paint.commands.iter().position(
        |command| matches!(command, PaintCommand::Text { node: run, .. } if *run == node),
    ) else {
        return "none".into();
    };
    paint
        .text_batches()
        .iter()
        .position(|batch| batch.contains(&at))
        .map_or_else(|| "none".into(), |batch| format!("batch {batch}"))
}

#[cfg(test)]
#[path = "tests/debug_views.rs"]
mod tests;
