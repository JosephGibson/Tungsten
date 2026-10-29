//! The engine's debug views as windows on the core UI (M57, w01 §11 M5):
//! the HUD with its frame-time graph, the system timing overlay, the entity
//! inspector, the log console (F8) and the UI inspector (F7) each keep a
//! root on `UiLayer::Debug`, holding a title bar that drags the window, an
//! optional fixed header and a scrolling body holding a table of label
//! columns sized by layout. Every node a view builds sets every style and
//! text override, so a game's theme changes nothing in a view. The log
//! console shows the engine logger's records, which the logger's `Tee`
//! copies into a bounded channel; the UI inspector shows the widget a middle
//! click picked, on any layer.
//!
//! M58 polishes them (`D-168`): a neutral dark look from one palette whose
//! saturated colours carry meaning, the engine font's SemiBold for titles
//! and headers, body text 15 units on 20, a 1 px border that brightens
//! while the pointer is over the window, a soft shadow, a title bar holding
//! the view's toggle key and collapse and close buttons, windows at most
//! 45% of the viewport wide, and table cells that end in `…` on one line,
//! data rows striped in turn.
//!
//! The debug layers draw on the OS scale alone, times a zoom (Ctrl+=,
//! Ctrl+-, Ctrl+0) and a step down on small windows, whatever the player's
//! UI and text scales. A window opens at the place saved under its name in
//! the user folder's `debug_windows.json` (`WindowPlacements`), in
//! OS-logical units, else it docks at its view's corner: the HUD top left,
//! the systems overlay top right, the inspector bottom left, the UI
//! inspector bottom right and the log console as a strip along the bottom,
//! each given a fixed height before the layout so that no two overlap at a
//! debug viewport of 960×540 units or more. A saved place is read again
//! when the debug scale moves, clamped again with the window's measured
//! size after its first layout and whenever the viewport changes, and
//! snapped onto a margin line its edge lies near. The end of a drag saves
//! the window's place; only places and the zoom persist, not which views
//! are open or collapsed. `App::run_frame` runs [`update`] once a frame,
//! after the schedule and before the UI layout.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::Ordering;

use glam::Vec2;
use tungsten_core::input::{ActionMap, InputState, KeyCode, MouseButton, RawInput};
use tungsten_core::text::{EllipsisAt, StyledText, TextAlign, TextOverflow, TextWrap};
use tungsten_core::ui::{
    Align, Anchor, Direction, Edges, LayoutStyle, PaintCommand, PaintList, Placement, Rect, Role,
    Sizing, StyleOverrides, TextOverrides, UiLayer, UiShadow, UiTree, UiVertex, Visibility,
    WidgetId, WidgetKind, WindowPlacements,
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

// ---- the palette (M58): one table every view reads ----
//
// Surfaces and chrome stay neutral; a saturated colour carries one meaning
// in a view, and its number, name or label stays beside it. Every text
// colour keeps 4.5:1 against the panel, the title bar, a striped row and a
// level badge, composited in linear light as the overlay does, over a black
// and over a white scene.

/// A window's fill; each view's `background` default.
pub(crate) const PANEL: [u8; 4] = [17, 17, 20, 252];

/// The title bar, over the panel.
const TITLE_VEIL: [u8; 4] = [255, 255, 255, 5];

/// Every other data row, and the graph's backdrop.
pub(crate) const STRIPE_VEIL: [u8; 4] = [255, 255, 255, 2];

/// The 1 px border; each view's `outline_color` default.
pub(crate) const BORDER: [u8; 4] = [70, 70, 80, 255];

/// The border while the pointer is over the window or its title bar is
/// pressed.
pub(crate) const BORDER_HOT: [u8; 4] = [150, 150, 170, 255];

/// Every window's shadow: its colour, blur and offset in UI units.
pub(crate) const SHADOW: [u8; 4] = [0, 0, 0, 90];
const SHADOW_BLUR: f32 = 10.0;
const SHADOW_OFFSET: Vec2 = Vec2::new(0.0, 3.0);

/// Values, titles and headers; each view's `color` default.
pub(crate) const VALUE: [u8; 4] = [232, 232, 236, 255];

/// Labels, the toggle key and the buttons' glyphs.
pub(crate) const LABEL: [u8; 4] = [166, 166, 176, 255];

/// The budget ramp, by the share of the budget: up to 1, 1.5 and 2 times,
/// and above.
pub(crate) const GREEN: [u8; 4] = [86, 214, 120, 255];
pub(crate) const YELLOW: [u8; 4] = [232, 208, 72, 255];
pub(crate) const ORANGE: [u8; 4] = [245, 150, 60, 255];
pub(crate) const RED: [u8; 4] = [255, 128, 128, 255];

/// `ERROR` and `WARN` records' messages in the log console.
pub(crate) const ERROR_TEXT: [u8; 4] = [255, 170, 170, 255];
pub(crate) const WARN_TEXT: [u8; 4] = [250, 215, 150, 255];

/// `WARN` badges and entity references.
pub(crate) const AMBER: [u8; 4] = [245, 180, 50, 255];
/// `INFO` badges, numbers and the `Update` stage.
pub(crate) const CYAN: [u8; 4] = [80, 200, 230, 255];
/// `DEBUG` badges, booleans and the `FixedUpdate` stage.
pub(crate) const VIOLET: [u8; 4] = [184, 152, 255, 255];
/// The `PreUpdate` stage.
pub(crate) const BLUE: [u8; 4] = [124, 170, 255, 255];
/// The `PostUpdate` stage.
pub(crate) const MAGENTA: [u8; 4] = [245, 132, 208, 255];
/// A level badge's fill: the level's hue at this alpha.
pub(crate) const BADGE_ALPHA: u8 = 8;

/// No colour.
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

// ---- sizes (M58), in UI units ----

/// The window's and the title bar's corner radius.
const RADIUS: f32 = 4.0;

/// Space inside the title bar and around the header and the body.
const INSET: f32 = 6.0;

/// Space at each side of a table cell; there is no gap between columns,
/// so the stripes run unbroken.
const CELL_PAD: f32 = 6.0;

/// How much further a cell under a header row starts.
const INDENT: f32 = 12.0;

/// How much larger a title is than the body text, in size and line height.
const TITLE_STEP: f32 = 2.0;

/// A bar cell's track: its width and height, centred in a line (M58).
const BAR_WIDTH: f32 = 48.0;
const BAR_HEIGHT: f32 = 6.0;

/// The weight of titles, headers and key numbers: the engine font's
/// SemiBold.
pub(crate) const STRONG: u16 = 600;

/// The share of the debug viewport's width a corner window takes at most.
const WIDTH_SHARE: f32 = 0.45;

/// The share of the debug viewport's height a corner window takes at most;
/// its body scrolls past it.
const HEIGHT_SHARE: f32 = 0.6;

/// The share of the debug viewport's height the log console's strip takes
/// at most.
const STRIP_SHARE: f32 = 0.25;

/// Lines a docked window leaves each window still to dock below it on its
/// side.
const RESERVED_LINES: f32 = 6.0;

/// How near, in UI units, a saved window's edge snaps onto its margin line.
const SNAP_DISTANCE: f32 = 16.0;

/// The debug viewport the views step down to keep, in UI units (M58).
const FLOOR: Vec2 = Vec2::new(960.0, 540.0);

/// The views' steps down from 1, largest first: the last is the floor.
const STEPS: [f32; 3] = [1.0, 0.875, 0.75];

/// The zoom's range and step: an eighth from a half to twice.
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 2.0;
const ZOOM_STEP: f32 = 0.125;

/// The frame-time graph's width: a unit per interval (M58).
pub(crate) const GRAPH_WIDTH: f32 = 240.0;

/// The frame-time graph's height, in lines (M58).
const GRAPH_LINES: f32 = 4.0;

/// The graph's guide lines, in milliseconds: one and two frames at 60 Hz
/// (M58).
const GUIDES_MS: [f32; 2] = [16.7, 33.3];

/// The guide lines' colour: the label grey at half alpha (M58).
const GUIDE: [u8; 4] = [LABEL[0], LABEL[1], LABEL[2], 128];

/// A value not known yet: the HUD's before its first interval (M58).
pub(crate) const MISSING: &str = "\u{2013}";

/// The graph block's statistics, in order (M58).
const STATS: [&str; 4] = ["min", "avg", "max", "1% low"];

/// Frame intervals the graph shows.
pub(crate) const GRAPH_SAMPLES: usize = 240;

/// The graph's lowest top, in milliseconds: two frames at 60 Hz.
const GRAPH_FLOOR_MS: f32 = 33.3;

/// A frame at 60 Hz, in milliseconds: the budget the graph's bars and the
/// HUD's times ramp against, so the bands fall at 16.7, 25 and 33.3 ms.
pub(crate) const FRAME_BUDGET_MS: f32 = 1000.0 / 60.0;

/// The collapse button's glyph while the body shows, and while it does not;
/// the close button's.
const COLLAPSE: &str = "\u{2013}";
const EXPAND: &str = "+";
const CLOSE: &str = "\u{d7}";

/// `value`'s colour on the budget ramp (M58): green up to `budget`, yellow
/// up to 1.5 times it, orange up to twice it, red above.
pub(crate) fn ramp(value: f32, budget: f32) -> [u8; 4] {
    if value <= budget {
        GREEN
    } else if value <= budget * 1.5 {
        YELLOW
    } else if value <= budget * 2.0 {
        ORANGE
    } else {
        RED
    }
}

/// What a node's text takes besides the view's look (M58).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ink {
    pub(crate) color: [u8; 4],
    pub(crate) weight: u16,
    pub(crate) align: TextAlign,
    /// A title: two units over the body text's size and line height.
    pub(crate) title: bool,
}

impl Ink {
    /// Body text of `color`, at weight 400, aligned to the start.
    pub(crate) const fn body(color: [u8; 4]) -> Self {
        Self {
            color,
            weight: 400,
            align: TextAlign::Start,
            title: false,
        }
    }

    /// This ink aligned to `align`.
    pub(crate) const fn aligned(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// This ink at the strong weight.
    pub(crate) const fn strong(mut self) -> Self {
        self.weight = STRONG;
        self
    }
}

/// A view's look, from its public fields: what every node it builds takes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Look {
    pub(crate) family: String,
    pub(crate) size: f32,
    pub(crate) line_height: f32,
    pub(crate) color: [u8; 4],
    pub(crate) background: [u8; 4],
    pub(crate) border: [u8; 4],
    /// The border's width in physical pixels (M58).
    pub(crate) border_px: f32,
}

impl Look {
    /// Every text property, so nothing inherits from a game's theme: the
    /// engine font in `ink`, unspaced, wrapped by glyph and held to one line
    /// that ends in `…` (M58).
    fn text(&self, ink: Ink) -> TextOverrides {
        let step = if ink.title { TITLE_STEP } else { 0.0 };
        TextOverrides {
            family: Some(self.family.clone()),
            size: Some(self.size + step),
            line_height: Some(self.line_height + step),
            weight: Some(ink.weight),
            italic: Some(false),
            color: Some(ink.color),
            align: Some(ink.align),
            wrap: Some(TextWrap::Glyph),
            overflow: Some(TextOverflow::Ellipsis {
                at: EllipsisAt::End,
                lines: Some(1),
            }),
            letter_spacing: Some(0.0),
        }
    }

    /// Every style property of a node of `fill` and `border` of `width`.
    fn node(
        &self,
        fill: [u8; 4],
        border: [u8; 4],
        width: f32,
        radius: f32,
        ink: Ink,
    ) -> StyleOverrides {
        StyleOverrides {
            background: Some(fill),
            border: Some(border),
            border_width: Some(width),
            corner_radius: Some(radius),
            text: self.text(ink),
        }
    }

    /// The root: the view's fill inside a `border` of `width` units, which
    /// `outline_px` of 0 (or less, or NaN) leaves out.
    fn root(&self, border: [u8; 4], width: f32) -> StyleOverrides {
        self.node(
            self.background,
            border,
            width,
            RADIUS,
            Ink::body(self.color),
        )
    }

    /// The border's width in UI units at `unit_px` physical pixels a unit:
    /// `outline_px` physical pixels, none for 0, less or NaN.
    fn border_width(&self, unit_px: f32) -> f32 {
        if self.border_px > 0.0 && unit_px > 0.0 {
            self.border_px / unit_px
        } else {
            0.0
        }
    }

    /// Anything inside the window with no fill of its own: no fill, no
    /// border.
    fn plain(&self, ink: Ink) -> StyleOverrides {
        self.node(TRANSPARENT, TRANSPARENT, 0.0, 0.0, ink)
    }

    /// A table cell: `fill` behind it, no border.
    fn cell(&self, fill: [u8; 4], ink: Ink) -> StyleOverrides {
        self.node(fill, TRANSPARENT, 0.0, 0.0, ink)
    }

    /// The colour a column of `tone` writes in.
    fn tone(&self, tone: Tone) -> [u8; 4] {
        match tone {
            Tone::Label => LABEL,
            Tone::Value => self.color,
        }
    }
}

/// How a table column takes width (M58, §2.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Width {
    /// As wide as its widest cell: short fixed text.
    Auto,
    /// As wide as its widest cell, at most this percent of the window's
    /// width cap: free text beside the `Fill` column.
    Capped(u8),
    /// The width the other columns leave: as wide as its widest cell, at
    /// most the window's width cap less the other columns' widths at the
    /// last layout, and at least what the title bar and the header leave
    /// them. Taffy sizes a window by its children's flex bases, so a
    /// growing column with no basis would leave the window narrower than
    /// its text (§2.7's fallback).
    Fill,
}

/// A column's text colour by default: the palette's grey for labels, the
/// view's colour for values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Label,
    Value,
}

/// A table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Column {
    pub(crate) align: TextAlign,
    pub(crate) width: Width,
    pub(crate) tone: Tone,
    /// Whether its cells are bars, not text (M58): the systems overlay's
    /// share of the frame.
    pub(crate) bars: bool,
}

impl Column {
    /// A column of `tone` text aligned to `align`, taking `width`.
    pub(crate) const fn new(align: TextAlign, width: Width, tone: Tone) -> Self {
        Self {
            align,
            width,
            tone,
            bars: false,
        }
    }

    /// A column of bars: each a track [`BAR_WIDTH`] wide holding a fill of
    /// its cell's fraction, [`BAR_HEIGHT`] tall and centred in a line.
    pub(crate) const fn bars() -> Self {
        Self {
            align: TextAlign::Start,
            width: Width::Auto,
            tone: Tone::Value,
            bars: true,
        }
    }
}

/// What a window holds between its title bar and its body: a fixed header
/// that never scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Header {
    None,
    /// One line: the inspectors' pick.
    Line,
    /// The HUD's frame-time graph.
    Graph,
    /// A line of this many buttons, then a status line: the log console's
    /// controls (M58).
    Controls(usize),
}

/// What a view's window holds besides its title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shape {
    /// The table's columns.
    pub(crate) columns: &'static [Column],
    pub(crate) header: Header,
}

/// What one view asks of its window this frame.
pub(crate) struct ViewSpec {
    /// Its key in the placements file.
    pub(crate) name: &'static str,
    /// The title bar's text.
    pub(crate) title: &'static str,
    /// The title bar's key: the first binding of the view's toggle action
    /// (M58); `None` while it is unbound.
    pub(crate) key: Option<Cow<'static, str>>,
    pub(crate) enabled: bool,
    pub(crate) corner: HudCorner,
    pub(crate) padding: f32,
    /// A strip along its corner's horizontal edge, as wide as the viewport
    /// less its margins: the log console (M58).
    pub(crate) strip: bool,
    pub(crate) look: Look,
    pub(crate) shape: Shape,
}

/// The first binding of `action` in the world's action map, as a prompt
/// names it: `F4` for `engine_toggle_hud`.
pub(crate) fn toggle_key(world: &World, action: &str) -> Option<Cow<'static, str>> {
    world
        .get_resource::<ActionMap>()?
        .bindings(action)
        .first()
        .map(|binding| binding.label())
}

/// The window one view keeps in the UI tree.
#[derive(Debug)]
pub(crate) struct ViewWindow {
    pub(crate) root: WidgetId,
    /// The title bar: the drag handle.
    pub(crate) title: WidgetId,
    /// The title bar's caption, and the empty panel that grows between it
    /// and the key, which keeps the key and the buttons at the right.
    pub(crate) caption: WidgetId,
    spacer: WidgetId,
    /// The title bar's toggle key, and its collapse and close buttons.
    pub(crate) key: WidgetId,
    pub(crate) collapse: WidgetId,
    pub(crate) close: WidgetId,
    /// The fixed header's panel, when the shape has one.
    pub(crate) fixed: Option<WidgetId>,
    /// The header line, when the shape's header is one, or the controls'
    /// status line.
    pub(crate) header: Option<WidgetId>,
    /// The controls' buttons, when the shape's header is a controls line
    /// (M58).
    pub(crate) controls: Vec<WidgetId>,
    /// The graph, when the shape's header is one, and the values of its
    /// block's four statistics (M58).
    pub(crate) graph: Option<WidgetId>,
    pub(crate) stats: Vec<WidgetId>,
    /// The header's content at its own width: the header line or the
    /// graph block, which the window keeps room for (M58).
    natural: Option<WidgetId>,
    /// The scroll panel holding the table.
    pub(crate) body: WidgetId,
    /// The column panels inside the table's row.
    pub(crate) columns: Vec<WidgetId>,
    /// Whether the collapse button has collapsed the header and the body;
    /// not saved (`D-166`).
    pub(crate) collapsed: bool,
    /// Whether a primary press holds the title bar: a drag under way.
    held: bool,
    look: Look,
    shape: Shape,
    /// The border's state the root last took: hot, and its width in units.
    border: Option<(bool, f32)>,
    /// The button whose glyph shows hovered; none at build.
    glyph_hovered: Option<WidgetId>,
    /// Whether the window keeps a saved place, the file's, a drag's or the
    /// game's, over its dock.
    pub(crate) saved: bool,
    /// The OS-logical units per UI unit its saved place was last placed at
    /// (M58): another means a zoom or a step, and the place is read again.
    units: Option<f32>,
    /// The debug viewport at the last update: a saved place clamps with
    /// the size a layout at that viewport measured.
    seen: Option<Vec2>,
}

/// A window the dock places this frame (M58): its root, where it docks,
/// the height it wants, its margin, line and title bar heights.
#[derive(Debug, Clone, Copy)]
struct Dock {
    root: WidgetId,
    corner: HudCorner,
    strip: bool,
    wanted: f32,
    margin: f32,
    line: f32,
    title: f32,
}

impl Dock {
    fn top(&self) -> bool {
        matches!(self.corner, HudCorner::TopLeft | HudCorner::TopRight)
    }

    fn left(&self) -> bool {
        matches!(self.corner, HudCorner::TopLeft | HudCorner::BottomLeft)
    }
}

/// The frame's state the views share.
pub(crate) struct ViewFrame<'a> {
    pub(crate) tree: &'a mut UiTree,
    placements: &'a mut WindowPlacements,
    /// The debug layer's viewport the frame's layout uses, in its units
    /// (M58).
    pub(crate) viewport: Vec2,
    /// Physical pixels per tree unit: `ScaleFactor::get()`.
    pub(crate) scale: f32,
    /// OS-logical units per debug unit: the tree's factor times the debug
    /// scale over the OS scale (M58, `D-184`), what a saved place is
    /// multiplied by.
    units: f32,
    /// The placements the dock gave its windows at the last frame (M58):
    /// another placement means a drag or the game moved the window.
    docked: Vec<(WidgetId, Placement)>,
    /// The windows to dock this frame, in the views' update order.
    docking: Vec<Dock>,
    /// Roots whose drag ended at this frame's routing.
    moved: Vec<WidgetId>,
    /// Widgets this frame's routing activated: the views' buttons (M58).
    activated: Vec<WidgetId>,
    /// Widgets a primary press took, and widgets whose press ended, at this
    /// frame's routing: the title bars (M58).
    pressed: Vec<WidgetId>,
    released: Vec<WidgetId>,
    /// The root under the pointer at the last layout (M58).
    hot: Option<WidgetId>,
    save_warned: bool,
}

impl ViewFrame<'_> {
    /// Whether this frame's routing activated `widget` (M58): a view's own
    /// controls.
    pub(crate) fn activated(&self, widget: WidgetId) -> bool {
        self.activated.contains(&widget)
    }

    /// Whether this frame's routing activated `window`'s close button
    /// (M58): the view then turns itself off, as its key does.
    pub(crate) fn closes(&self, window: Option<&ViewWindow>) -> bool {
        window.is_some_and(|window| self.activated.contains(&window.close))
    }

    /// Builds, keeps, rebuilds or removes `window` for `spec`, saves the end
    /// of a drag, keeps the chrome and places the window. Returns whether
    /// it is open: the caller then fills it.
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
        self.chrome(open, spec);
        self.place(open, spec);
        if self.moved.contains(&open.root) {
            open.saved = true;
            if let Some(Placement::Anchored { offset, .. }) = self
                .tree
                .layout_style(open.root)
                .map(|style| style.placement)
            {
                // The snapped place, in OS-logical units (M58).
                self.save(spec.name, offset * self.units);
                open.units = Some(self.units);
            }
        }
        true
    }

    /// A new window for `spec`: its root, title bar, fixed header, body,
    /// table and columns, every node styled; at its saved place when the
    /// file has one, else at its corner.
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
        tree.set_overrides(root, look.root(look.border, 0.0));
        tree.set_clip_children(root, true);
        tree.set_role(root, Role::Window);
        tree.set_accessible_name(root, Some(spec.title.to_string()));
        tree.set_shadow(
            root,
            Some(UiShadow::new(SHADOW, SHADOW_BLUR, SHADOW_OFFSET)),
        );

        let title = tree.panel(
            root,
            LayoutStyle {
                direction: Direction::Row,
                padding: Edges::symmetric(INSET, INSET * 0.5),
                gap: INSET,
                align_items: Align::Center,
                ..LayoutStyle::default()
            },
        );
        tree.set_overrides(
            title,
            look.node(TITLE_VEIL, TRANSPARENT, 0.0, RADIUS, Ink::body(look.color)),
        );
        tree.set_drag_handle(title, true);
        let caption = tree.label(title, spec.title);
        tree.set_overrides(
            caption,
            look.plain(Ink {
                title: true,
                ..Ink::body(look.color).strong()
            }),
        );
        let spacer = tree.panel(
            title,
            LayoutStyle {
                width: Sizing::Fill,
                ..LayoutStyle::default()
            },
        );
        tree.set_overrides(spacer, look.plain(Ink::body(look.color)));
        let key = tree.label(title, "");
        tree.set_overrides(key, look.plain(Ink::body(LABEL)));
        let collapse = title_button(tree, look, title, COLLAPSE);
        let close = title_button(tree, look, title, CLOSE);

        let mut stats = Vec::new();
        let mut controls = Vec::new();
        let (fixed, header, graph, natural) = match spec.shape.header {
            Header::None => (None, None, None, None),
            Header::Line => {
                let fixed = tree.panel(
                    root,
                    LayoutStyle {
                        padding: Edges {
                            top: INSET * 0.5,
                            ..Edges::default()
                        },
                        ..LayoutStyle::default()
                    },
                );
                tree.set_overrides(fixed, look.plain(Ink::body(look.color)));
                let line = tree.label(fixed, "");
                // At its own width, so the window can read what it needs.
                tree.set_layout_style(
                    line,
                    LayoutStyle {
                        padding: Edges::symmetric(CELL_PAD, 0.0),
                        align_self: Some(Align::Start),
                        ..LayoutStyle::default()
                    },
                );
                tree.set_overrides(line, look.plain(Ink::body(look.color).strong()));
                (Some(fixed), Some(line), None, Some(line))
            }
            Header::Graph => {
                let fixed = tree.panel(
                    root,
                    LayoutStyle {
                        padding: Edges {
                            start: INSET,
                            end: INSET,
                            top: INSET,
                            bottom: 0.0,
                        },
                        ..LayoutStyle::default()
                    },
                );
                tree.set_overrides(fixed, look.plain(Ink::body(look.color)));
                // The graph at a fixed size, its four statistics beside it.
                let block = tree.panel(
                    fixed,
                    LayoutStyle {
                        direction: Direction::Row,
                        gap: INSET,
                        align_items: Align::Start,
                        align_self: Some(Align::Start),
                        ..LayoutStyle::default()
                    },
                );
                tree.set_overrides(block, look.plain(Ink::body(look.color)));
                let graph = tree.mesh(
                    block,
                    LayoutStyle::default().with_size(
                        Sizing::Px(GRAPH_WIDTH),
                        Sizing::Px(GRAPH_LINES * look.line_height),
                    ),
                );
                tree.set_overrides(graph, look.cell(STRIPE_VEIL, Ink::body(look.color)));
                let names = tree.panel(block, LayoutStyle::column());
                tree.set_overrides(names, look.plain(Ink::body(LABEL)));
                let values = tree.panel(block, LayoutStyle::column());
                tree.set_overrides(values, look.plain(Ink::body(look.color).strong()));
                for name in STATS {
                    let label = tree.label(names, name);
                    tree.set_overrides(label, look.plain(Ink::body(LABEL)));
                    let value = tree.label(values, MISSING);
                    tree.set_overrides(value, look.plain(Ink::body(look.color).strong()));
                    stats.push(value);
                }
                (Some(fixed), None, Some(graph), Some(block))
            }
            Header::Controls(buttons) => {
                let fixed = tree.panel(
                    root,
                    LayoutStyle {
                        padding: Edges {
                            start: INSET,
                            end: INSET,
                            top: INSET * 0.5,
                            bottom: 0.0,
                        },
                        ..LayoutStyle::default()
                    },
                );
                tree.set_overrides(fixed, look.plain(Ink::body(look.color)));
                let line = tree.panel(
                    fixed,
                    LayoutStyle {
                        direction: Direction::Row,
                        gap: INSET,
                        align_items: Align::Center,
                        align_self: Some(Align::Start),
                        ..LayoutStyle::default()
                    },
                );
                tree.set_overrides(line, look.plain(Ink::body(look.color)));
                for _ in 0..buttons {
                    let button = tree.button(line, "");
                    tree.set_focusable(button, false);
                    tree.set_overrides(button, look.plain(Ink::body(LABEL)));
                    if let Some(&label) = tree.children(button).first() {
                        tree.set_overrides(label, look.plain(Ink::body(LABEL)));
                    }
                    controls.push(button);
                }
                let status = tree.label(line, "");
                tree.set_overrides(status, look.plain(Ink::body(LABEL)));
                (Some(fixed), Some(status), None, Some(line))
            }
        };

        let body = tree.scroll_panel(
            root,
            LayoutStyle {
                padding: Edges {
                    top: INSET * 0.5,
                    bottom: INSET * 0.5,
                    ..Edges::default()
                },
                ..LayoutStyle::default()
            },
        );
        tree.set_overrides(body, look.plain(Ink::body(look.color)));
        let table = tree.panel(body, LayoutStyle::row());
        tree.set_overrides(table, look.plain(Ink::body(look.color)));
        let columns = spec
            .shape
            .columns
            .iter()
            .map(|column| {
                let panel = tree.panel(table, LayoutStyle::column());
                tree.set_overrides(
                    panel,
                    look.plain(Ink::body(look.tone(column.tone)).aligned(column.align)),
                );
                panel
            })
            .collect();

        let mut saved = false;
        let mut units = None;
        if let Some(position) = self.placements.get(spec.name) {
            saved = true;
            units = Some(self.units);
            tree.set_window_position(root, position / self.units);
        }
        ViewWindow {
            root,
            title,
            caption,
            spacer,
            key,
            collapse,
            close,
            fixed,
            header,
            controls,
            graph,
            stats,
            natural,
            body,
            columns,
            collapsed: false,
            held: false,
            look: spec.look.clone(),
            shape: spec.shape,
            border: None,
            glyph_hovered: None,
            saved,
            units,
            seen: None,
        }
    }

    /// The window's chrome this frame (M58): a collapse its button asked
    /// for, the border's colour while the pointer is over the window or a
    /// primary press holds its title bar and its width in physical pixels,
    /// the toggle key, and the buttons' glyphs, `VALUE` while hovered.
    fn chrome(&mut self, window: &mut ViewWindow, spec: &ViewSpec) {
        let tree = &mut *self.tree;
        if self.activated.contains(&window.collapse) {
            window.collapsed = !window.collapsed;
            let shown = if window.collapsed {
                Visibility::Collapsed
            } else {
                Visibility::Visible
            };
            for part in window.fixed.into_iter().chain([window.body]) {
                tree.set_visibility(part, shown);
            }
            tree.set_text(
                window.collapse,
                if window.collapsed { EXPAND } else { COLLAPSE },
            );
        }
        if self.pressed.contains(&window.title) {
            window.held = true;
        }
        if self.released.contains(&window.title) {
            window.held = false;
        }
        let hot = self.hot == Some(window.root) || window.held;
        let width = window.look.border_width(self.scale * tree.debug_scale());
        if window.border != Some((hot, width)) {
            window.border = Some((hot, width));
            let border = if hot { BORDER_HOT } else { window.look.border };
            tree.set_overrides(window.root, window.look.root(border, width));
        }
        match &spec.key {
            Some(key) => {
                tree.set_plain_text(window.key, key);
                tree.set_visibility(window.key, Visibility::Visible);
            }
            None => tree.set_visibility(window.key, Visibility::Collapsed),
        }
        let hovered = tree
            .hovered()
            .filter(|hovered| [window.collapse, window.close].contains(hovered));
        if window.glyph_hovered != hovered {
            window.glyph_hovered = hovered;
            for button in [window.collapse, window.close] {
                let color = if hovered == Some(button) {
                    window.look.color
                } else {
                    LABEL
                };
                if let Some(&glyph) = tree.children(button).first() {
                    tree.set_overrides(glyph, glyph_look(&window.look, color));
                }
            }
        }
    }

    /// Places `window` (M58): caps a corner window at 45% of the debug
    /// viewport's width and 60% of its height, or makes the log console a
    /// strip as wide as the viewport less its margins and at most 25% of
    /// its height; a window with no saved place docks after every view has
    /// run ([`Self::dock`]). A saved one sits at its place, read again from
    /// its OS-logical units when the debug scale moved, clamped again with
    /// the size the last layout measured, and snapped onto a margin line its
    /// edge lies within 16 units of.
    /// A window whose placement changed since the dock last set it, a drag
    /// under way or the game's, keeps it from then on.
    fn place(&mut self, window: &mut ViewWindow, spec: &ViewSpec) {
        let current = self
            .tree
            .layout_style(window.root)
            .map(|style| style.placement);
        let docked = self
            .docked
            .iter()
            .find(|(root, _)| *root == window.root)
            .map(|(_, placement)| *placement);
        if !window.saved && docked.is_some() && current != docked {
            window.saved = true;
            window.units = Some(self.units);
        }
        let margin = spec.padding.max(0.0);
        let viewport = self.viewport;
        let (width, max_width, cap) = if spec.strip {
            let width = (viewport.x - 2.0 * margin).max(0.0);
            (Sizing::Px(width), width, viewport.y * STRIP_SHARE)
        } else {
            let width = viewport.x * WIDTH_SHARE;
            (Sizing::Auto, width, viewport.y * HEIGHT_SHARE)
        };
        let saved = window.saved;
        restyle(self.tree, window.root, |style| {
            style.width = width;
            style.max_width = Some(max_width);
            if saved {
                style.height = Sizing::Auto;
                style.max_height = Some(cap.max(0.0));
            } else {
                style.max_height = None;
            }
        });
        self.cap_width(window, max_width, spec.strip);
        if window.saved {
            self.place_saved(window, spec, margin);
        } else {
            let title = self
                .tree
                .rect(window.title)
                .map_or(0.0, |rect| rect.height());
            let wanted = if window.collapsed {
                title
            } else {
                match (
                    self.tree.rect(window.root),
                    self.tree.rect(window.body),
                    self.tree.scroll_max(window.body),
                ) {
                    (Some(root), Some(body), Some(more)) => body.max.y + more - root.min.y,
                    _ => cap,
                }
            };
            self.docking.push(Dock {
                root: window.root,
                corner: spec.corner,
                strip: spec.strip,
                wanted,
                margin,
                line: window.look.line_height,
                title,
            });
        }
        window.seen = Some(viewport);
    }

    /// A saved window's place this frame: read again when the debug scale
    /// moved, clamped with the last layout's size, then snapped.
    fn place_saved(&mut self, window: &mut ViewWindow, spec: &ViewSpec, margin: f32) {
        if window.units != Some(self.units)
            && let Some(logical) = self.placements.get(spec.name)
        {
            self.tree
                .set_window_position(window.root, logical / self.units);
        }
        window.units = Some(self.units);
        // The previous frame's layout ran at this viewport when the view saw
        // it then too: only that layout's size clamps right. A window whose
        // size grew is clamped again too.
        if window.seen == Some(self.viewport)
            && self.tree.rect(window.root).is_some()
            && let Some(offset) = anchored_offset(self.tree, window.root)
        {
            self.tree.set_window_position(window.root, offset);
        }
        let (Some(offset), Some(rect)) = (
            anchored_offset(self.tree, window.root),
            self.tree.rect(window.root),
        ) else {
            return;
        };
        let snap = |at: f32, size: f32, extent: f32| {
            if (at - margin).abs() <= SNAP_DISTANCE {
                margin
            } else if (at + size - (extent - margin)).abs() <= SNAP_DISTANCE {
                extent - margin - size
            } else {
                at
            }
        };
        let snapped = Vec2::new(
            snap(offset.x, rect.width(), self.viewport.x),
            snap(offset.y, rect.height(), self.viewport.y),
        );
        if snapped != offset {
            self.tree.set_window_position(window.root, snapped);
        }
    }

    /// Docks this frame's windows with no saved place (M58), before the
    /// layout that lays them out, so they never overlap: each takes a fixed
    /// height. A strip takes at most 25% of the viewport's height at its
    /// edge and pushes that edge's windows away; a side's windows, the left
    /// corners' or the right's, stack from the top corner down and from the
    /// bottom corner up in update order, each at most 60% of the height and
    /// what the windows docked before it left, less six lines and a margin
    /// for each window still to dock on that side. Returns the placements
    /// given, which the next frame's views compare.
    fn dock(&mut self) -> Vec<(WidgetId, Placement)> {
        let docks = std::mem::take(&mut self.docking);
        let height = self.viewport.y;
        let margin = docks.iter().map(|dock| dock.margin).fold(0.0, f32::max);
        let mut given = vec![0.0; docks.len()];
        let (mut top, mut bottom) = (0.0, 0.0);
        for (i, dock) in docks.iter().enumerate().filter(|(_, dock)| dock.strip) {
            given[i] = dock.wanted.min(height * STRIP_SHARE).max(dock.title);
            if dock.top() {
                top += given[i] + margin;
            } else {
                bottom += given[i] + margin;
            }
        }
        let room = height - 2.0 * margin - top - bottom;
        for left in [true, false] {
            let side: Vec<usize> = (0..docks.len())
                .filter(|i| !docks[*i].strip && docks[*i].left() == left)
                .collect();
            let mut used = 0.0;
            for (k, &i) in side.iter().enumerate() {
                let reserved: f32 = side[k + 1..]
                    .iter()
                    .map(|&j| RESERVED_LINES * docks[j].line + margin)
                    .sum();
                given[i] = docks[i]
                    .wanted
                    .min(height * HEIGHT_SHARE)
                    .min(room - used - reserved)
                    .max(docks[i].title);
                used += given[i] + margin;
            }
        }
        let mut placed = Vec::with_capacity(docks.len());
        let (mut strip_top, mut strip_bottom) = (margin, margin);
        let mut stacks = [margin + top, margin + top, margin + bottom, margin + bottom];
        for (dock, given) in docks.iter().zip(given) {
            let (corner, offset) = if dock.strip {
                let at = if dock.top() {
                    &mut strip_top
                } else {
                    &mut strip_bottom
                };
                let corner = if dock.top() {
                    HudCorner::TopLeft
                } else {
                    HudCorner::BottomLeft
                };
                let offset = Vec2::new(margin, *at);
                *at += given + margin;
                (corner, offset)
            } else {
                let at = &mut stacks[corner_index(dock.corner)];
                let offset = Vec2::new(margin, *at);
                *at += given + margin;
                (dock.corner, offset)
            };
            let placement = corner_placement(corner, offset);
            restyle(self.tree, dock.root, |style| {
                style.placement = placement;
                style.height = Sizing::Px(given);
            });
            placed.push((dock.root, placement));
        }
        placed
    }

    /// Caps the caption and the columns inside a window at most
    /// `max_width` wide (M58, §2.7): a capped column at its share of it; the
    /// caption, and the `Fill` column, at what the rest of the title bar,
    /// and the other columns, left at the last layout, and the `Fill`
    /// column at least as wide as the title bar and the header need beyond
    /// the other columns, so the stripes reach the window's right edge; in
    /// a `fixed` window, the strip, exactly that wide.
    fn cap_width(&mut self, window: &ViewWindow, max_width: f32, fixed: bool) {
        let tree = &mut *self.tree;
        // The border paints inside the box, which the layout does not see.
        let inner = max_width;
        let width = |tree: &UiTree, id: WidgetId| tree.rect(id).map(|rect| rect.width());
        let title = width(tree, window.title)
            .zip(width(tree, window.caption))
            .zip(width(tree, window.spacer))
            .map(|((title, caption), spacer)| (title - spacer, title - caption - spacer));
        restyle(tree, window.caption, |style| {
            style.max_width = title.map(|(_, rest)| (inner - rest).max(0.0));
        });
        // The header's content, and the fixed panel's padding around it.
        let header = window
            .natural
            .and_then(|content| width(tree, content))
            .map(|content| {
                let padding = window
                    .fixed
                    .and_then(|fixed| tree.layout_style(fixed))
                    .map_or(0.0, |style| style.padding.start + style.padding.end);
                content + padding
            });
        let chrome = title
            .map_or(0.0, |(natural, _)| natural)
            .max(header.unwrap_or(0.0));
        let others: f32 = window
            .columns
            .iter()
            .zip(window.shape.columns)
            .filter(|(_, column)| column.width != Width::Fill)
            .filter_map(|(panel, _)| width(tree, *panel))
            .sum();
        let measured = window
            .columns
            .iter()
            .all(|panel| tree.rect(*panel).is_some());
        for (panel, column) in window.columns.iter().zip(window.shape.columns) {
            restyle(tree, *panel, |style| {
                (style.min_width, style.max_width) = match column.width {
                    Width::Capped(percent) => (None, Some(max_width * f32::from(percent) / 100.0)),
                    Width::Fill if measured => {
                        let most = (inner - others).max(0.0);
                        // A strip's width is fixed: its column fills it.
                        let least = if fixed {
                            most
                        } else {
                            (chrome - others).clamp(0.0, most)
                        };
                        (Some(least), Some(most))
                    }
                    Width::Auto | Width::Fill => (None, None),
                };
            });
        }
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

/// A title bar button holding `glyph` (M58): interactive, not focusable, a
/// title line square, every override set on it and on its label.
fn title_button(tree: &mut UiTree, look: &Look, title: WidgetId, glyph: &str) -> WidgetId {
    let button = tree.button(title, glyph);
    tree.set_focusable(button, false);
    tree.set_layout_style(
        button,
        LayoutStyle {
            width: Sizing::Px(look.line_height + TITLE_STEP),
            ..LayoutStyle::default()
        },
    );
    tree.set_overrides(button, look.plain(Ink::body(LABEL)));
    if let Some(&label) = tree.children(button).first() {
        tree.set_overrides(label, glyph_look(look, LABEL));
    }
    button
}

/// A button glyph's look: title-sized, centred, in `color`.
fn glyph_look(look: &Look, color: [u8; 4]) -> StyleOverrides {
    look.plain(Ink {
        title: true,
        ..Ink::body(color).aligned(TextAlign::Center)
    })
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

/// A root's anchored offset as its layout style holds it now.
fn anchored_offset(tree: &UiTree, root: WidgetId) -> Option<Vec2> {
    match tree.layout_style(root)?.placement {
        Placement::Anchored { offset, .. } => Some(offset),
        _ => None,
    }
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

/// One table cell (M58): its text, a colour over its column's, whether it
/// is strong, spans that style parts of it in place of the text, and in a
/// bar column the bar's fraction and colour.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Cell {
    pub(crate) text: String,
    pub(crate) color: Option<[u8; 4]>,
    pub(crate) strong: bool,
    pub(crate) styled: Option<StyledText>,
    pub(crate) bar: Option<(f32, [u8; 4])>,
    /// A fill over the row's stripe: a level's badge (M58).
    pub(crate) fill: Option<[u8; 4]>,
}

impl Cell {
    /// A cell of `text` in its column's colour.
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
            strong: false,
            styled: None,
            bar: None,
            fill: None,
        }
    }

    /// This cell in `color`.
    #[must_use]
    pub(crate) fn colored(mut self, color: [u8; 4]) -> Self {
        self.color = Some(color);
        self
    }

    /// A bar cell of `fraction`, 0 to 1, in `color`.
    pub(crate) fn bar(fraction: f32, color: [u8; 4]) -> Self {
        Self {
            bar: Some((fraction, color)),
            ..Self::new("")
        }
    }

    /// A cell of styled `spans` (M58): the fps row's coloured time.
    pub(crate) fn styled(spans: StyledText) -> Self {
        Self {
            styled: Some(spans),
            ..Self::new("")
        }
    }
}

/// One table row (M58): a data row stripes in turn; a header row is
/// strong and unstriped, and the stripes start again below it; an indented
/// row's first cell starts [`INDENT`] further in.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Row {
    pub(crate) cells: Vec<Cell>,
    pub(crate) header: bool,
    pub(crate) indent: bool,
}

impl Row {
    /// A data row of `cells`.
    pub(crate) fn data(cells: Vec<Cell>) -> Self {
        Self {
            cells,
            header: false,
            indent: false,
        }
    }
}

/// How one row's cells look: striped, strong, indented.
#[derive(Debug, Clone, Copy)]
struct RowLook {
    striped: bool,
    strong: bool,
    indent: bool,
}

/// The looks of `rows` in turn: data rows striped every other one, counted
/// again from each header row.
fn row_looks(rows: &[Row]) -> Vec<RowLook> {
    let mut data = 0_usize;
    rows.iter()
        .map(|row| {
            if row.header {
                data = 0;
                RowLook {
                    striped: false,
                    strong: true,
                    indent: row.indent,
                }
            } else {
                data += 1;
                RowLook {
                    striped: data.is_multiple_of(2),
                    strong: false,
                    indent: row.indent,
                }
            }
        })
        .collect()
}

impl ViewWindow {
    /// A cell's layout: padded [`CELL_PAD`] a side, the first column's
    /// [`INDENT`] further when indented.
    fn cell_style(c: usize, indent: bool) -> LayoutStyle {
        let start = if indent && c == 0 {
            CELL_PAD + INDENT
        } else {
            CELL_PAD
        };
        LayoutStyle {
            padding: Edges {
                start,
                end: CELL_PAD,
                top: 0.0,
                bottom: 0.0,
            },
            ..LayoutStyle::default()
        }
    }

    /// A cell's look in column `c`: a header row's in the view's colour,
    /// as headers are, unless the cell names its own.
    fn cell_look(&self, c: usize, cell: Option<&Cell>, row: RowLook) -> StyleOverrides {
        let column = self.shape.columns[c];
        let tone = if row.strong { Tone::Value } else { column.tone };
        let color = cell
            .and_then(|cell| cell.color)
            .unwrap_or_else(|| self.look.tone(tone));
        let strong = row.strong || cell.is_some_and(|cell| cell.strong);
        let ink = Ink {
            color,
            weight: if strong { STRONG } else { 400 },
            align: column.align,
            title: false,
        };
        let stripe = if row.striped {
            STRIPE_VEIL
        } else {
            TRANSPARENT
        };
        let fill = cell.and_then(|cell| cell.fill).unwrap_or(stripe);
        self.look.cell(fill, ink)
    }

    /// A new cell at the end of column `c`, in `row`'s look (the log
    /// console appends its lines this way).
    fn add_cell(&self, tree: &mut UiTree, c: usize, cell: &Cell, row: RowLook) -> WidgetId {
        let label = tree.label(self.columns[c], cell.text.as_str());
        tree.set_layout_style(label, Self::cell_style(c, row.indent));
        tree.set_overrides(label, self.cell_look(c, Some(cell), row));
        label
    }
}

/// Writes `rows` into `window`'s table, cell `c` of each row into column
/// `c`, a missing cell empty; adds or removes cells to match, and restyles
/// each for its row (M58).
pub(crate) fn write_table(tree: &mut UiTree, window: &ViewWindow, rows: &[Row]) {
    let looks = row_looks(rows);
    for (c, column) in window.columns.iter().enumerate() {
        if window.shape.columns[c].bars {
            window.write_bars(tree, c, rows, &looks);
            continue;
        }
        let labels = tree.children(*column).to_vec();
        for (i, (row, look)) in rows.iter().zip(&looks).enumerate() {
            let cell = row.cells.get(c);
            let text = cell.map_or("", |cell| cell.text.as_str());
            let label = match labels.get(i) {
                Some(label) => *label,
                None => tree.label(*column, text),
            };
            match cell.and_then(|cell| cell.styled.as_ref()) {
                Some(styled) => {
                    tree.set_text(label, styled.clone());
                }
                None => {
                    tree.set_plain_text(label, text);
                }
            }
            tree.set_layout_style(label, ViewWindow::cell_style(c, look.indent));
            tree.set_overrides(label, window.cell_look(c, cell, *look));
        }
        for label in labels.iter().skip(rows.len()) {
            tree.remove(*label);
        }
    }
}

impl ViewWindow {
    /// Writes column `c`'s bars (M58): each row's cell a panel one line
    /// tall, striped as its row, holding a track that holds the fill; a row
    /// with no bar hides its track.
    fn write_bars(&self, tree: &mut UiTree, c: usize, rows: &[Row], looks: &[RowLook]) {
        let column = self.columns[c];
        let cells = tree.children(column).to_vec();
        let ink = Ink::body(self.look.color);
        for (i, (row, look)) in rows.iter().zip(looks).enumerate() {
            let cell = if let Some(cell) = cells.get(i) {
                *cell
            } else {
                let cell = tree.panel(
                    column,
                    LayoutStyle {
                        direction: Direction::Row,
                        height: Sizing::Px(self.look.line_height),
                        padding: Edges::symmetric(CELL_PAD, 0.0),
                        align_items: Align::Center,
                        ..LayoutStyle::default()
                    },
                );
                let track = tree.panel(
                    cell,
                    LayoutStyle::row().with_size(Sizing::Px(BAR_WIDTH), Sizing::Px(BAR_HEIGHT)),
                );
                tree.set_overrides(track, self.look.cell(BORDER, ink));
                let part = tree.panel(track, LayoutStyle::default());
                tree.set_overrides(part, self.look.cell(TRANSPARENT, ink));
                cell
            };
            let fill = if look.striped {
                STRIPE_VEIL
            } else {
                TRANSPARENT
            };
            tree.set_overrides(cell, self.look.cell(fill, ink));
            let Some(&track) = tree.children(cell).first() else {
                continue;
            };
            let bar = row.cells.get(c).and_then(|cell| cell.bar);
            tree.set_visibility(
                track,
                if bar.is_some() {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            );
            if let (Some((fraction, color)), Some(&part)) = (bar, tree.children(track).first()) {
                let width = BAR_WIDTH * fraction.clamp(0.0, 1.0);
                tree.set_layout_style(
                    part,
                    LayoutStyle::default().with_size(Sizing::Px(width), Sizing::Fill),
                );
                tree.set_overrides(part, self.look.cell(color, ink));
            }
        }
        for cell in cells.iter().skip(rows.len()) {
            tree.remove(*cell);
        }
    }
}

/// [`write_table`] of plain data rows, each cell in its column's colour.
pub(crate) fn write_rows(tree: &mut UiTree, window: &ViewWindow, rows: &[Vec<String>]) {
    let rows: Vec<Row> = rows
        .iter()
        .map(|cells| Row::data(cells.iter().map(Cell::new).collect()))
        .collect();
    write_table(tree, window, &rows);
}

/// The graph's triangles into `out`: one bar per interval, oldest left and
/// the newest at the right edge of [`GRAPH_SAMPLES`] slots, 6 vertices
/// each, rising from the bottom against the larger of 33.3 ms and the
/// tallest shown, in the budget ramp's colour against a 60 Hz frame; then,
/// over them, a guide line one unit thick at 16.7 and at 33.3 ms for a
/// graph `height` units tall (M58).
pub(crate) fn graph_vertices(samples: &[f32], height: f32, out: &mut Vec<UiVertex>) {
    out.clear();
    let shown = &samples[samples.len().saturating_sub(GRAPH_SAMPLES)..];
    let top = shown.iter().copied().fold(GRAPH_FLOOR_MS, f32::max);
    let first = GRAPH_SAMPLES - shown.len();
    let width = 1.0 / GRAPH_SAMPLES as f32;
    let mut quad = |x0: f32, y0: f32, x1: f32, y1: f32, color: [u8; 4]| {
        for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y0), (x1, y1), (x0, y1)] {
            out.push(UiVertex::new(Vec2::new(x, y), color));
        }
    };
    for (i, ms) in shown.iter().enumerate() {
        let x0 = (first + i) as f32 * width;
        let y0 = 1.0 - (ms / top).clamp(0.0, 1.0);
        quad(x0, y0, x0 + width, 1.0, ramp(*ms, FRAME_BUDGET_MS));
    }
    let thickness = if height > 0.0 { 1.0 / height } else { 0.0 };
    for ms in GUIDES_MS {
        let y = 1.0 - (ms / top).clamp(0.0, 1.0);
        let y0 = (y - thickness * 0.5).clamp(0.0, 1.0 - thickness);
        quad(0.0, y0, 1.0, y0 + thickness, GUIDE);
    }
}

/// The graph block's statistics over `samples`'s shown intervals (M58):
/// their least, mean and most, and the mean of the slowest 1% of them, at
/// least one; `None` with none.
pub(crate) fn graph_stats(samples: &[f32]) -> Option<[f32; 4]> {
    let shown = &samples[samples.len().saturating_sub(GRAPH_SAMPLES)..];
    if shown.is_empty() {
        return None;
    }
    let mut sorted = shown.to_vec();
    sorted.sort_by(f32::total_cmp);
    let count = sorted.len();
    let slowest = (count / 100).max(1);
    let low = sorted[count - slowest..].iter().sum::<f32>() / slowest as f32;
    let mean = sorted.iter().sum::<f32>() / count as f32;
    Some([sorted[0], mean, sorted[count - 1], low])
}

impl ViewWindow {
    /// Writes the graph block's statistics (M58): each in ms at the strong
    /// weight in the budget ramp's colour, or [`MISSING`] before any.
    pub(crate) fn write_stats(&self, tree: &mut UiTree, stats: Option<[f32; 4]>) {
        for (i, label) in self.stats.iter().enumerate() {
            let (text, color) = match stats {
                Some(values) => (
                    format!("{:.2}ms", values[i]),
                    ramp(values[i], FRAME_BUDGET_MS),
                ),
                None => (MISSING.to_string(), self.look.color),
            };
            tree.set_plain_text(*label, &text);
            tree.set_overrides(*label, self.look.plain(Ink::body(color).strong()));
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
/// shown or not; while a view is shown, the zoom keys move the zoom (M58);
/// the tree takes the debug scale, zoom × step / the player's UI scale, so
/// the debug layers draw at the OS scale times zoom and step (M58); then
/// each view builds, updates or removes its window and fills it, a drag's
/// end saves the window's place, and the windows with no saved place dock
/// (M58); a window's close and collapse buttons act on this frame's
/// activations. `paint` is the app's last paint list. Past the debug scale,
/// a world whose views are all off and closed returns at once.
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
    if active {
        zoom_keys(world);
    }
    let factor = debug_factor(world);
    if let Some(tree) = world.get_resource_mut::<UiTree>() {
        tree.set_debug_scale(factor);
    }
    if !active {
        return;
    }
    let Some(mut tree) = world.remove_resource::<UiTree>() else {
        return;
    };
    let mut placements = world
        .remove_resource::<WindowPlacements>()
        .unwrap_or_default();
    let (scale, os) = world
        .get_resource::<ScaleFactor>()
        .map_or((1.0, 1.0), |scale| (scale.get(), scale.os()));
    let debug = tree.debug_scale();
    let viewport = world
        .get_resource::<WindowSize>()
        .map_or(Vec2::ZERO, |size| {
            Vec2::new(size.width as f32, size.height as f32) / (scale * debug)
        });
    let queue = world.get_resource::<EventQueue<UiEvent>>();
    let moved = queue
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
    // The frame's events rotated into the previous window before the
    // views run, so each is read once.
    let events = |pick: fn(&UiEvent) -> Option<WidgetId>| -> Vec<WidgetId> {
        queue.map_or_else(Vec::new, |queue| queue.iter().filter_map(pick).collect())
    };
    let activated = events(|event| match *event {
        UiEvent::Activated { widget, .. } => Some(widget),
        _ => None,
    });
    let pressed = events(|event| match *event {
        UiEvent::Pressed {
            widget,
            button: MouseButton::Left,
            ..
        } => Some(widget),
        _ => None,
    });
    let released = events(|event| match *event {
        UiEvent::Released {
            widget,
            button: MouseButton::Left,
            ..
        }
        | UiEvent::PressCancelled { widget, .. } => Some(widget),
        _ => None,
    });
    let hot = world
        .get_resource::<InputState>()
        .and_then(InputState::cursor_position)
        .filter(|_| scale > 0.0)
        .and_then(|(x, y)| tree.hit_test(Vec2::new(x, y) / scale))
        .and_then(|hit| tree.root_of(hit));
    let (save_warned, docked) = world
        .get_resource_mut::<ViewsState>()
        .map_or((false, Vec::new()), |state| {
            (state.save_warned, std::mem::take(&mut state.docked))
        });
    let mut frame = ViewFrame {
        tree: &mut tree,
        placements: &mut placements,
        viewport,
        scale,
        units: scale * debug / os,
        docked,
        docking: Vec::new(),
        moved,
        activated,
        pressed,
        released,
        hot,
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
    let docked = frame.dock();
    let save_warned = frame.save_warned;
    world.insert_resource(ViewsState {
        save_warned,
        docked,
    });
    world.insert_resource(placements);
    world.insert_resource(tree);
}

/// What the views keep between frames outside their resources.
#[derive(Debug, Default)]
struct ViewsState {
    /// Whether a failed save of the places, or of the zoom, has warned
    /// this run.
    save_warned: bool,
    /// The placements the dock gave its windows at the last frame (M58).
    docked: Vec<(WidgetId, Placement)>,
}

/// `zoom` on the zoom's eighths, within a half and twice (M58).
fn snap_zoom(zoom: f32) -> f32 {
    ((zoom / ZOOM_STEP).round() * ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX)
}

/// The views' step at an OS-logical viewport (M58): the largest of 1, 7/8
/// and 3/4 at which it divided by the step holds 960×540 units, else 3/4.
pub(crate) fn step_for(logical: Vec2) -> f32 {
    STEPS
        .into_iter()
        .find(|step| logical.x / step >= FLOOR.x && logical.y / step >= FLOOR.y)
        .unwrap_or(STEPS[STEPS.len() - 1])
}

/// The debug layers' scale this frame (M58): the zoom kept with the window
/// places times the step for the OS-logical viewport, times the OS scale
/// over the tree's factor, which already holds the player's UI scale and
/// any window-relative base a game set (`D-184`), so the views keep their
/// OS-relative size; 1 step without a window size.
fn debug_factor(world: &World) -> f32 {
    let zoom = world
        .get_resource::<WindowPlacements>()
        .and_then(WindowPlacements::zoom)
        .map_or(1.0, snap_zoom);
    let (os, factor) = world
        .get_resource::<ScaleFactor>()
        .map_or((1.0, 1.0), |scale| (scale.os(), scale.get()));
    let step = world.get_resource::<WindowSize>().map_or(1.0, |size| {
        step_for(Vec2::new(size.width as f32, size.height as f32) / os)
    });
    zoom * step * os / factor
}

/// The zoom keys (M58), read from the frame's raw key presses, OS repeats
/// included, while a view is shown: Ctrl+= (Shift allowed) an eighth up,
/// Ctrl+- an eighth down, Ctrl+0 back to 1. A change is kept with the
/// window places and saved at once; a failed save warns once a run. The
/// keys still reach the game.
fn zoom_keys(world: &mut World) {
    let Some(input) = world.get_resource::<InputState>() else {
        return;
    };
    let start = world
        .get_resource::<WindowPlacements>()
        .and_then(WindowPlacements::zoom)
        .map_or(1.0, snap_zoom);
    let mut zoom = start;
    for event in input.raw_events() {
        let RawInput::Key {
            code,
            pressed: true,
            modifiers,
            ..
        } = event.input
        else {
            continue;
        };
        if !modifiers.control() {
            continue;
        }
        zoom = match code {
            KeyCode::Equal => (zoom + ZOOM_STEP).min(ZOOM_MAX),
            KeyCode::Minus => (zoom - ZOOM_STEP).max(ZOOM_MIN),
            KeyCode::Digit0 => 1.0,
            _ => zoom,
        };
    }
    if zoom == start {
        return;
    }
    if !world.has_resource::<WindowPlacements>() {
        world.insert_resource(WindowPlacements::default());
    }
    let placements = world.resource_mut::<WindowPlacements>();
    placements.set_zoom(zoom);
    let saved = placements.save();
    if let Err(err) = saved {
        if !world.has_resource::<ViewsState>() {
            world.insert_resource(ViewsState::default());
        }
        let state = world.resource_mut::<ViewsState>();
        if !state.save_warned {
            state.save_warned = true;
            log::warn!("Debug views: could not save the zoom: {err}; it lasts this run");
        }
    }
}

/// A view's look from its public fields.
macro_rules! look_of {
    ($view:expr) => {
        $crate::debug_views::Look {
            family: $view.font_id.clone(),
            size: $view.font_size,
            line_height: $view.line_height,
            color: $view.color,
            background: $view.background,
            border: $view.outline_color,
            border_px: $view.outline_px,
        }
    };
}
pub(crate) use look_of;

/// The log console (M57): the engine logger's records, newest last, in a
/// window on the UI's debug layer; toggled by `engine_toggle_log_console`
/// (F8). It drains the logger every frame, shown or not, into a ring of
/// [`capacity`](Self::capacity) records, oldest out, as the logger wrote
/// them. From M58 its rows read each record's time, level, short target and
/// message's first line when it shows them: the level on a badge in its
/// hue, an `ERROR` or `WARN` message tinted to match, a record in another
/// shape whole as its message. Its controls line, which stays in view,
/// holds a filter per level, `pause` and `clear`, then `N lines · D
/// dropped` (records the logger's full channel dropped), `paused · N new`,
/// or `no engine logger` when the app installed none. Its body follows the
/// newest row until the wheel moves it off, and again once the wheel
/// reaches the end.
pub struct LogConsole {
    /// Whether the window shows; the drain runs either way.
    pub enabled: bool,
    /// The edge the window docks along as a strip when no place is saved
    /// for it: a bottom corner the bottom, by default (M58), a top corner
    /// the top.
    pub corner: HudCorner,
    /// The UI font family the window's text takes: the engine font's,
    /// [`ENGINE_FONT_ID`](crate::ENGINE_FONT_ID), by default.
    pub font_id: String,
    /// Text size in UI units; 15 (M58), titles two more.
    pub font_size: f32,
    /// Line height in UI units; 20 (M58).
    pub line_height: f32,
    /// The messages' colour, sRGB; `ERROR` and `WARN` messages take their
    /// own.
    pub color: [u8; 4],
    /// The window's fill.
    pub background: [u8; 4],
    /// The window's border colour; brighter while the pointer is over the
    /// window (M58).
    pub outline_color: [u8; 4],
    /// The window's border width in physical pixels (M58); `0.0` draws
    /// none.
    pub outline_px: f32,
    /// Space between the window and its corner, in UI units.
    pub padding_px: f32,
    /// Records kept, oldest out; 512.
    pub capacity: usize,
    /// How often the window takes new records, on the frame interval;
    /// 100 ms.
    pub refresh_interval_ms: f32,
    feed: Option<ConsoleFeed>,
    /// The kept records as the logger wrote them, oldest first.
    lines: VecDeque<String>,
    /// Records received since the start, and how many of them the window
    /// has taken.
    received: u64,
    taken: u64,
    /// Each shown row's record, by its count since the start, oldest first
    /// (M58).
    rows: VecDeque<u64>,
    /// Rows appended since the window last rebuilt them: every other one
    /// stripes.
    appended: u64,
    /// The root the rows are in.
    rows_in: Option<WidgetId>,
    /// Which levels show, `ERROR` to `TRACE` (M58).
    levels: [bool; 5],
    /// Whether the window holds still (M58), and how many records it had
    /// taken when it stopped.
    paused: Option<u64>,
    /// Whether the rows rebuild at the next update: a filter, a clear.
    rebuild: bool,
    time_since_refresh_ms: f32,
    pub(crate) window: Option<ViewWindow>,
}

impl Default for LogConsole {
    /// Off, with no logger feed: the window says `no engine logger`.
    fn default() -> Self {
        Self {
            enabled: false,
            corner: HudCorner::BottomLeft,
            font_id: crate::ENGINE_FONT_ID.to_string(),
            font_size: 15.0,
            line_height: 20.0,
            color: VALUE,
            background: PANEL,
            outline_color: BORDER,
            outline_px: 1.0,
            padding_px: 8.0,
            capacity: 512,
            refresh_interval_ms: 100.0,
            feed: None,
            lines: VecDeque::new(),
            received: 0,
            taken: 0,
            rows: VecDeque::new(),
            appended: 0,
            rows_in: None,
            levels: [true; 5],
            paused: None,
            rebuild: false,
            time_since_refresh_ms: f32::INFINITY,
            window: None,
        }
    }
}

/// The log console's columns (M58): time, level, short target, message.
const CONSOLE_COLUMNS: &[Column] = &[
    Column::new(TextAlign::Start, Width::Auto, Tone::Label),
    Column::new(TextAlign::Center, Width::Auto, Tone::Label),
    Column::new(TextAlign::Start, Width::Capped(20), Tone::Label),
    Column::new(TextAlign::Start, Width::Fill, Tone::Value),
];

/// The levels the console filters, in its controls' order (M58).
const LEVELS: [log::Level; 5] = [
    log::Level::Error,
    log::Level::Warn,
    log::Level::Info,
    log::Level::Debug,
    log::Level::Trace,
];

/// The controls after the filters: pause and clear.
const PAUSE: usize = LEVELS.len();
const CLEAR: usize = LEVELS.len() + 1;

/// A level's hue (M58): `ERROR` red, `WARN` amber, `INFO` cyan, `DEBUG`
/// violet, `TRACE` grey.
pub(crate) fn level_hue(level: log::Level) -> [u8; 4] {
    match level {
        log::Level::Error => RED,
        log::Level::Warn => AMBER,
        log::Level::Info => CYAN,
        log::Level::Debug => VIOLET,
        log::Level::Trace => LABEL,
    }
}

/// One record's cells (M58): its time, its level on a badge of its hue,
/// its short target and its message's first line, ` …` when more follow,
/// tinted for `ERROR` and `WARN`; a record in another shape whole as its
/// message.
fn record_cells(record: &str) -> [Cell; 4] {
    let Some(fields) = crate::logging::record_fields(record) else {
        return [
            Cell::new(""),
            Cell::new(""),
            Cell::new(""),
            Cell::new(record),
        ];
    };
    let hue = level_hue(fields.level);
    let mut badge = Cell::new(fields.level.as_str());
    badge.color = Some(hue);
    badge.strong = true;
    badge.fill = Some([hue[0], hue[1], hue[2], BADGE_ALPHA]);
    let mut message = Cell::new(if fields.more {
        format!("{} \u{2026}", fields.message)
    } else {
        fields.message.to_string()
    });
    message.color = match fields.level {
        log::Level::Error => Some(ERROR_TEXT),
        log::Level::Warn => Some(WARN_TEXT),
        _ => None,
    };
    [
        Cell::new(fields.time),
        badge,
        Cell::new(fields.target),
        message,
    ]
}

impl LogConsole {
    /// Detaches the logger's feed, whose records then wait in it. A counter
    /// guard counts its window without it: the feed carries every thread's
    /// records in a test binary, not the guarded app's alone.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn take_feed(&mut self) -> Option<ConsoleFeed> {
        self.feed.take()
    }

    /// Attaches a feed [`take_feed`](Self::take_feed) took.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn restore_feed(&mut self, feed: Option<ConsoleFeed>) {
        if feed.is_some() {
            self.feed = feed;
        }
    }

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

    /// The kept records, oldest first, as the logger wrote them.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(String::as_str)
    }

    /// Empties the kept records, and the window at its next update (M58);
    /// the drain goes on.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.rebuild = true;
    }

    /// Records the logger dropped on a full or closed channel; 0 without a
    /// logger.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.feed
            .as_ref()
            .map_or(0, |feed| feed.dropped.load(Ordering::Relaxed))
    }

    /// Takes the logger's new records into the ring, oldest out; nothing
    /// reads them here (M58).
    fn drain(&mut self) {
        let Some(feed) = &self.feed else {
            return;
        };
        while let Ok(record) = feed.records.try_recv() {
            self.lines.push_back(record);
            self.received += 1;
        }
        while self.lines.len() > self.capacity {
            self.lines.pop_front();
        }
    }

    /// Whether `record` passes the level filters: one in another shape
    /// always does (M58).
    fn passes(&self, record: &str) -> bool {
        crate::logging::record_fields(record).is_none_or(|fields| {
            LEVELS
                .iter()
                .position(|level| *level == fields.level)
                .is_none_or(|i| self.levels[i])
        })
    }

    /// The console's frame: its controls' presses, then on its interval
    /// the new records into the window and the controls line; while it is
    /// shown.
    fn update_window(&mut self, world: &World, frame: &mut ViewFrame<'_>) {
        if frame.closes(self.window.as_ref()) {
            self.enabled = false;
        }
        let spec = ViewSpec {
            name: "log",
            title: "Log",
            key: toggle_key(world, "engine_toggle_log_console"),
            enabled: self.enabled,
            corner: self.corner,
            padding: self.padding_px,
            strip: true,
            look: look_of!(self),
            shape: Shape {
                columns: CONSOLE_COLUMNS,
                header: Header::Controls(LEVELS.len() + 2),
            },
        };
        if !frame.sync(&mut self.window, &spec) {
            self.rows.clear();
            self.rows_in = None;
            self.time_since_refresh_ms = f32::INFINITY;
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let pressed: Vec<usize> = (0..window.controls.len())
            .filter(|i| frame.activated(window.controls[*i]))
            .collect();
        let root = window.root;
        let body = window.body;
        for i in pressed {
            match i {
                PAUSE => {
                    self.paused = match self.paused {
                        Some(_) => None,
                        None => Some(self.taken),
                    };
                }
                CLEAR => self.clear(),
                level => {
                    self.levels[level] = !self.levels[level];
                    self.rebuild = true;
                }
            }
            self.time_since_refresh_ms = f32::INFINITY;
        }
        if self.rows_in != Some(root) {
            // A new window: every kept record is new to it, and its body
            // follows the end.
            self.rows.clear();
            self.rows_in = Some(root);
            self.rebuild = true;
            frame.tree.set_scroll(body, f32::INFINITY);
            self.time_since_refresh_ms = f32::INFINITY;
        }
        self.time_since_refresh_ms += frame_interval(world);
        if self.time_since_refresh_ms < self.refresh_interval_ms && !self.rebuild {
            return;
        }
        self.time_since_refresh_ms = 0.0;
        let window = self.window.as_ref().expect("synced above");
        let first = self.received - self.lines.len() as u64;
        if std::mem::take(&mut self.rebuild) {
            for column in &window.columns {
                for cell in frame.tree.children(*column).to_vec() {
                    frame.tree.remove(cell);
                }
            }
            self.rows.clear();
            self.appended = 0;
            self.taken = first;
        }
        while self.rows.front().is_some_and(|seq| *seq < first) {
            self.rows.pop_front();
            for column in &window.columns {
                if let Some(&cell) = frame.tree.children(*column).first() {
                    frame.tree.remove(cell);
                }
            }
        }
        let until = self.paused.unwrap_or(self.received).max(first);
        for seq in self.taken.max(first)..until {
            let record = &self.lines[(seq - first) as usize];
            if !self.passes(record) {
                continue;
            }
            let row = RowLook {
                striped: !self.appended.is_multiple_of(2),
                strong: false,
                indent: false,
            };
            for (c, cell) in record_cells(record).iter().enumerate() {
                window.add_cell(frame.tree, c, cell, row);
            }
            self.rows.push_back(seq);
            self.appended += 1;
        }
        self.taken = self.taken.max(until);
        self.write_controls(window, frame.tree);
    }

    /// The controls line (M58): each filter `[x] LEVEL` in its hue while
    /// it shows, `[ ] LEVEL` in grey while it hides; `pause`, or `resume`
    /// while paused; `clear`; then the status.
    fn write_controls(&self, window: &ViewWindow, tree: &mut UiTree) {
        let ink = |color: [u8; 4]| window.look.plain(Ink::body(color));
        for (i, button) in window.controls.iter().enumerate() {
            let (text, color) = match i {
                PAUSE => (
                    if self.paused.is_some() {
                        "resume".to_string()
                    } else {
                        "pause".to_string()
                    },
                    LABEL,
                ),
                CLEAR => ("clear".to_string(), LABEL),
                level => {
                    let name = LEVELS[level].as_str();
                    if self.levels[level] {
                        (format!("[x] {name}"), level_hue(LEVELS[level]))
                    } else {
                        (format!("[ ] {name}"), LABEL)
                    }
                }
            };
            tree.set_text(*button, text.as_str());
            if let Some(&label) = tree.children(*button).first() {
                tree.set_overrides(label, ink(color));
            }
        }
        if let Some(status) = window.header {
            let text = match (&self.feed, self.paused) {
                (None, _) => "no engine logger".to_string(),
                (Some(_), Some(at)) => {
                    format!("paused \u{b7} {} new", self.received.saturating_sub(at))
                }
                (Some(_), None) => {
                    format!(
                        "{} lines \u{b7} {} dropped",
                        self.lines.len(),
                        self.dropped()
                    )
                }
            };
            tree.set_plain_text(status, &text);
        }
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
    /// The corner the window docks at when no place is saved for it: the
    /// bottom right (M58).
    pub corner: HudCorner,
    /// The UI font family the window's text takes: the engine font's,
    /// [`ENGINE_FONT_ID`](crate::ENGINE_FONT_ID), by default.
    pub font_id: String,
    /// Text size in UI units; 15 (M58), titles two more.
    pub font_size: f32,
    /// Line height in UI units; 20 (M58).
    pub line_height: f32,
    /// The values' and the title's colour, sRGB; names take the palette's
    /// grey (M58).
    pub color: [u8; 4],
    /// The window's fill.
    pub background: [u8; 4],
    /// The window's border colour; brighter while the pointer is over the
    /// window (M58).
    pub outline_color: [u8; 4],
    /// The window's border width in physical pixels (M58); `0.0` draws
    /// none.
    pub outline_px: f32,
    /// Space between the window and its corner, in UI units.
    pub padding_px: f32,
    /// Row refresh interval on the frame interval; a new pick, and a pick's
    /// first layout, refresh at once.
    pub refresh_interval_ms: f32,
    /// The widget whose rows show.
    pub picked: Option<WidgetId>,
    shown_picked: Option<WidgetId>,
    /// Whether the shown pick had a layout rect at the last refresh: its
    /// first layout refreshes at once (M59).
    shown_laid_out: bool,
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
            corner: HudCorner::BottomRight,
            font_id: crate::ENGINE_FONT_ID.to_string(),
            font_size: 15.0,
            line_height: 20.0,
            color: VALUE,
            background: PANEL,
            outline_color: BORDER,
            outline_px: 1.0,
            padding_px: 8.0,
            refresh_interval_ms: 500.0,
            picked: None,
            shown_picked: None,
            shown_laid_out: false,
            time_since_refresh_ms: f32::INFINITY,
            last_cursor: None,
            window: None,
        }
    }
}

/// The UI inspector's columns: names right-aligned, then values.
const UI_INSPECTOR_COLUMNS: &[Column] = &[
    Column::new(TextAlign::End, Width::Capped(40), Tone::Label),
    Column::new(TextAlign::Start, Width::Fill, Tone::Value),
];

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
    /// for a new pick or a pick's first layout, the rows; while it is shown.
    fn update_window(
        &mut self,
        world: &World,
        frame: &mut ViewFrame<'_>,
        paint: Option<&PaintList>,
    ) {
        if frame.closes(self.window.as_ref()) {
            self.enabled = false;
        }
        let spec = ViewSpec {
            name: "ui_inspector",
            title: "UI",
            key: toggle_key(world, "engine_toggle_ui_inspector"),
            enabled: self.enabled,
            corner: self.corner,
            padding: self.padding_px,
            strip: false,
            look: look_of!(self),
            shape: Shape {
                columns: UI_INSPECTOR_COLUMNS,
                header: Header::Line,
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
        let laid_out = self
            .picked
            .is_some_and(|widget| frame.tree.rect(widget).is_some());
        if self.time_since_refresh_ms < self.refresh_interval_ms
            && self.shown_picked == self.picked
            && self.shown_laid_out == laid_out
        {
            return;
        }
        self.time_since_refresh_ms = 0.0;
        self.shown_picked = self.picked;
        self.shown_laid_out = laid_out;
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
