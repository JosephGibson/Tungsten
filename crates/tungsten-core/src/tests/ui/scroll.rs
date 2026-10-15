use super::*;
use crate::assets::AssetRegistry;
use crate::display::ScaleFactor;
use crate::ecs::{EventQueue, World};
use crate::input::{ActionMap, InputState, RoutedInput};
use crate::time::Time;
use crate::ui::{
    Anchor, Edges, FixedAdvanceMeasure, PaintCommand, PaintList, Sizing, StyleOverrides, UiDebug,
    UiEvent, UiLayer, UiRouter, ui_route_system,
};

/// The logical viewport every tree lays out in.
const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);
const DT: f32 = 1.0 / 32.0;

fn at(x: f32, y: f32, w: f32, h: f32) -> LayoutStyle {
    fixed(w, h).anchored(Anchor::TopStart, Vec2::new(x, y))
}

fn fixed(w: f32, h: f32) -> LayoutStyle {
    LayoutStyle::default().with_size(Sizing::Px(w), Sizing::Px(h))
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_pos_size(Vec2::new(x, y), Vec2::new(w, h))
}

/// 5 units of padding above and below the content.
fn padded() -> LayoutStyle {
    LayoutStyle {
        padding: Edges {
            top: 5.0,
            bottom: 5.0,
            ..Edges::default()
        },
        ..LayoutStyle::default()
    }
}

/// No fill and no border.
fn bare() -> StyleOverrides {
    StyleOverrides {
        background: Some([0, 0, 0, 0]),
        border_width: Some(0.0),
        ..StyleOverrides::default()
    }
}

fn layout(tree: &mut UiTree) {
    tree.layout(VIEWPORT, &mut FixedAdvanceMeasure::default());
}

fn paint(tree: &mut UiTree, scale: f32) -> PaintList {
    layout(tree);
    let mut list = PaintList::default();
    tree.paint(scale, &UiDebug::default(), &AssetRegistry::new(), &mut list);
    list
}

/// A bare root of 200 by 100 at (0, `top`) holding a padded scroll panel of
/// `count` rows, 100 by 30 each: content 5 + 30 · `count` + 5 tall, so ten
/// rows leave a maximum of 210. (tree, root, panel, rows), laid out.
fn panel_rows_at(top: f32, count: usize) -> (UiTree, WidgetId, WidgetId, Vec<WidgetId>) {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, at(0.0, top, 200.0, 100.0));
    tree.set_overrides(root, bare());
    let panel = tree.scroll_panel(root, padded());
    let rows = (0..count)
        .map(|_| tree.panel(panel, fixed(100.0, 30.0)))
        .collect();
    layout(&mut tree);
    (tree, root, panel, rows)
}

fn panel_rows(count: usize) -> (UiTree, WidgetId, WidgetId, Vec<WidgetId>) {
    panel_rows_at(0.0, count)
}

/// A hand-built world with what the router reads, a frame at a time.
struct Rig {
    world: World,
}

impl Rig {
    fn new(tree: UiTree) -> Self {
        let mut world = World::new();
        world.insert_resource(InputState::new());
        world.insert_resource(ActionMap::default_map());
        world.insert_resource(Time::new());
        world.insert_resource(ScaleFactor::default());
        world.insert_resource(UiRouter::default());
        world.insert_resource(RoutedInput::default());
        world.register_event::<UiEvent>();
        world.insert_resource(tree);
        Self { world }
    }

    /// One frame: `feed` gives the raw input, the UI routes, the tree lays
    /// out. Returns the routing's events.
    fn frame(&mut self, feed: impl FnOnce(&mut InputState)) -> Vec<UiEvent> {
        self.world.flush_events();
        let input = self.world.resource_mut::<InputState>();
        input.begin_frame();
        feed(input);
        self.world.resource_mut::<Time>().advance_frame(DT);
        ui_route_system(&mut self.world);
        layout(self.tree_mut());
        self.world
            .resource::<EventQueue<UiEvent>>()
            .iter_current()
            .copied()
            .collect()
    }

    /// The wheel turned `lines` at `point`, in physical pixels.
    fn wheel(&mut self, point: Vec2, lines: f32) -> Vec<UiEvent> {
        self.frame(|input| {
            input.update_cursor_position(point.x, point.y);
            input.add_scroll_line_delta(0.0, lines);
        })
    }

    fn tree(&self) -> &UiTree {
        self.world.resource::<UiTree>()
    }

    fn tree_mut(&mut self) -> &mut UiTree {
        self.world.resource_mut::<UiTree>()
    }
}

fn scrolled(events: &[UiEvent]) -> Vec<(WidgetId, f32)> {
    events
        .iter()
        .filter_map(|event| match *event {
            UiEvent::Scrolled { widget, offset } => Some((widget, offset)),
            _ => None,
        })
        .collect()
}

/// An outer scroll panel filling a 200 by 100 root: a 30-unit row, an
/// inner scroll panel 60 tall holding five 30-unit rows (maximum 90), then
/// three more rows (outer maximum 80). (tree, outer, inner).
fn nested() -> (UiTree, WidgetId, WidgetId) {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, at(0.0, 0.0, 200.0, 100.0));
    let outer = tree.scroll_panel(root, LayoutStyle::default());
    tree.panel(outer, fixed(100.0, 30.0));
    let inner = tree.scroll_panel(
        outer,
        LayoutStyle::default().with_size(Sizing::Auto, Sizing::Px(60.0)),
    );
    for _ in 0..5 {
        tree.panel(inner, fixed(100.0, 30.0));
    }
    for _ in 0..3 {
        tree.panel(outer, fixed(100.0, 30.0));
    }
    layout(&mut tree);
    (tree, outer, inner)
}

#[test]
fn a_panel_shrinks_to_its_parents_limit_and_its_maximum_matches_the_content() {
    let (mut tree, _root, panel, rows) = panel_rows(10);
    assert_eq!(
        tree.rect(panel),
        Some(rect(0.0, 0.0, 200.0, 100.0)),
        "310 of content in 100"
    );
    assert_eq!(tree.rect(rows[9]), Some(rect(0.0, 275.0, 100.0, 30.0)));
    assert_eq!(
        tree.scroll_max(panel),
        Some(210.0),
        "305 + 5 of padding − 100"
    );
    assert_eq!(tree.scroll(panel), Some(0.0));
    assert!(tree.clips_children(panel));

    let mut last = tree.layout_style(rows[9]).cloned().unwrap();
    last.margin.bottom = 7.0;
    tree.set_layout_style(rows[9], last);
    layout(&mut tree);
    assert_eq!(tree.scroll_max(panel), Some(217.0), "the margin counts");

    let (fits, _root, fitting, _rows) = panel_rows(2);
    assert_eq!(
        fits.rect(fitting),
        Some(rect(0.0, 0.0, 200.0, 70.0)),
        "content-sized"
    );
    assert_eq!(fits.scroll_max(fitting), Some(0.0));
    assert_eq!(tree.scroll(rows[0]), None, "not a scroll panel");
    assert_eq!(tree.scroll_max(rows[0]), None);
    assert!(!tree.set_scroll(rows[0], 10.0));
}

#[test]
fn a_scroll_moves_paint_hits_and_visual_rects_but_not_layout() {
    let (mut tree, _root, panel, rows) = panel_rows(10);
    let before = paint(&mut tree, 1.0);
    assert_eq!(tree.hit_test(Vec2::new(50.0, 20.0)), Some(rows[0]));

    assert!(tree.set_scroll(panel, 45.0));
    assert!(tree.is_paint_dirty());
    assert!(!tree.is_layout_dirty(), "paint only");
    let stats = tree.layout(VIEWPORT, &mut FixedAdvanceMeasure::default());
    assert_eq!(stats.roots_laid_out, 0, "no layout work");
    assert_eq!(
        tree.rect(rows[2]),
        Some(rect(0.0, 65.0, 100.0, 30.0)),
        "layout rects stay"
    );
    assert_eq!(
        tree.visual_rect(rows[2]),
        Some(rect(0.0, 20.0, 100.0, 30.0))
    );
    assert_eq!(
        tree.visual_rect(panel),
        tree.rect(panel),
        "the panel itself stays"
    );
    assert_eq!(tree.hit_test(Vec2::new(50.0, 10.0)), Some(rows[1]));

    let after = paint(&mut tree, 1.0);
    // The panel's box first, then its rows, 45 units up.
    assert_eq!(after.commands[0], before.commands[0]);
    for (old, new) in before.commands[1..11].iter().zip(&after.commands[1..11]) {
        assert_eq!(new.rect().min, old.rect().min - Vec2::new(0.0, 45.0));
    }

    // At 1.5 the offset rounds to whole physical pixels: 45.3 · 1.5 = 67.95.
    tree.set_scroll(panel, 45.3);
    let list = paint(&mut tree, 1.5);
    assert!(
        (list.commands[3].rect().min.y - 29.5).abs() < 1e-4,
        "65 · 1.5 − 68"
    );
    let shift = 68.0 / 1.5;
    assert_eq!(tree.visual_rect(rows[2]).unwrap().min.y, 65.0 - shift);
}

#[test]
fn offsets_clamp_ignore_nan_and_follow_the_end_through_a_growing_layout() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, at(0.0, 0.0, 200.0, 100.0));
    let panel = tree.scroll_panel(root, padded());
    let mut rows: Vec<WidgetId> = (0..10)
        .map(|_| tree.panel(panel, fixed(100.0, 30.0)))
        .collect();
    assert!(tree.set_scroll(panel, 50.0), "kept before a layout");
    assert_eq!(
        tree.scroll(panel),
        Some(0.0),
        "clamped to the unknown maximum"
    );
    layout(&mut tree);
    assert_eq!(tree.scroll(panel), Some(50.0));

    assert!(tree.set_scroll(panel, 1000.0));
    assert_eq!(tree.scroll(panel), Some(210.0), "clamped");
    rows.push(tree.panel(panel, fixed(100.0, 30.0)));
    layout(&mut tree);
    assert_eq!(tree.scroll(panel), Some(240.0), "past the end follows it");

    assert!(tree.set_scroll(panel, f32::NAN));
    assert_eq!(tree.scroll(panel), Some(0.0));
    assert!(!tree.set_scroll(panel, f32::NEG_INFINITY), "also 0");
    assert!(!tree.set_scroll(panel, -5.0), "and so is a negative offset");

    assert!(tree.set_scroll(panel, 100.0));
    rows.push(tree.panel(panel, fixed(100.0, 30.0)));
    layout(&mut tree);
    assert_eq!(tree.scroll(panel), Some(100.0), "short of the end it stays");

    assert!(tree.set_scroll(panel, 270.0));
    for row in rows.drain(..3) {
        tree.remove(row);
    }
    layout(&mut tree);
    assert_eq!(tree.scroll_max(panel), Some(180.0));
    assert_eq!(tree.scroll(panel), Some(180.0), "at the end it follows");
    rows.push(tree.panel(panel, fixed(100.0, 30.0)));
    layout(&mut tree);
    assert_eq!(tree.scroll(panel), Some(210.0));
}

#[test]
fn the_innermost_panel_moves_first_and_an_outer_one_at_the_inners_end() {
    let (tree, outer, inner) = nested();
    assert_eq!(tree.scroll_max(outer), Some(80.0));
    assert_eq!(tree.scroll_max(inner), Some(90.0));
    let mut rig = Rig::new(tree);
    let over_inner = Vec2::new(50.0, 40.0);
    for expected in [40.0, 80.0, 90.0] {
        let events = rig.wheel(over_inner, -1.0);
        assert_eq!(scrolled(&events), [(inner, expected)]);
    }
    let events = rig.wheel(over_inner, -1.0);
    assert_eq!(
        scrolled(&events),
        [(outer, 40.0)],
        "the inner one is at its end"
    );
    assert_eq!(rig.tree().scroll(inner), Some(90.0));
    let events = rig.wheel(over_inner, 1.0);
    assert_eq!(
        scrolled(&events),
        [(inner, 50.0)],
        "up: the inner one can move"
    );
    assert_eq!(rig.tree().scroll(outer), Some(40.0));
}

#[test]
fn a_line_is_scroll_line_units_and_a_pixel_delta_is_divided_by_the_scale() {
    let (tree, _root, panel, _rows) = panel_rows(10);
    let mut rig = Rig::new(tree);
    let over = Vec2::new(50.0, 50.0);
    rig.world
        .resource_mut::<UiRouter>()
        .settings_mut()
        .scroll_line = 25.0;
    rig.wheel(over, -1.0);
    assert_eq!(rig.tree().scroll(panel), Some(25.0));
    rig.world
        .resource_mut::<UiRouter>()
        .settings_mut()
        .scroll_line = f32::NAN;
    rig.wheel(over, -1.0);
    assert_eq!(rig.tree().scroll(panel), Some(65.0), "NaN acts as 40");
    rig.world
        .resource_mut::<UiRouter>()
        .settings_mut()
        .scroll_line = -3.0;
    rig.wheel(over, -0.5);
    assert_eq!(
        rig.tree().scroll(panel),
        Some(85.0),
        "so does a value below 0"
    );

    rig.world.resource_mut::<ScaleFactor>().set_os(2.0);
    let events = rig.frame(|input| {
        input.update_cursor_position(100.0, 100.0);
        input.add_scroll_pixel_delta(0.0, -30.0);
    });
    assert_eq!(
        scrolled(&events),
        [(panel, 100.0)],
        "30 physical pixels at 2"
    );
    let events = rig.frame(|input| input.add_scroll_pixel_delta(0.0, 50.0));
    assert_eq!(
        scrolled(&events),
        [(panel, 75.0)],
        "a positive y moves the content down"
    );
}

#[test]
fn a_delta_that_is_not_finite_moves_nothing() {
    let (tree, _root, panel, _rows) = panel_rows(10);
    let mut rig = Rig::new(tree);
    let over = Vec2::new(50.0, 50.0);
    for lines in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e38] {
        let events = rig.wheel(over, lines);
        assert!(scrolled(&events).is_empty(), "{lines}");
        assert_eq!(rig.tree().scroll(panel), Some(0.0));
    }
    let events = rig.frame(|input| input.add_scroll_pixel_delta(0.0, f32::NEG_INFINITY));
    assert!(scrolled(&events).is_empty());
    assert_eq!(
        rig.world.resource::<RoutedInput>().scroll_line_delta(),
        (0.0, 0.0),
        "the wheel over the UI stays the UI's"
    );
}

#[test]
fn a_routing_sends_one_scrolled_per_panel_it_moved() {
    let (tree, outer, inner) = nested();
    let mut rig = Rig::new(tree);
    let events = rig.frame(|input| {
        input.update_cursor_position(50.0, 40.0);
        input.add_scroll_line_delta(0.0, -1.0);
        input.add_scroll_line_delta(0.0, -1.0);
    });
    assert_eq!(
        scrolled(&events),
        [(inner, 80.0)],
        "one, with the offset it left"
    );
    let events = rig.frame(|input| {
        input.add_scroll_line_delta(0.0, -1.0);
        input.add_scroll_line_delta(0.0, -1.0);
    });
    assert_eq!(
        scrolled(&events),
        [(inner, 90.0), (outer, 40.0)],
        "in the order they moved"
    );
    assert_eq!(events.len(), 2, "rows take no other events: {events:?}");

    assert!(rig.tree_mut().set_scroll(outer, 0.0));
    let events = rig.frame(|_| {});
    assert!(scrolled(&events).is_empty(), "the setter sends none");
}

#[test]
fn focus_scrolls_into_view_from_focus_next_and_from_a_games_focus() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, at(0.0, 0.0, 200.0, 100.0));
    let panel = tree.scroll_panel(root, padded());
    let buttons: Vec<WidgetId> = (0..10)
        .map(|_| {
            let button = tree.button(panel, "x");
            tree.set_layout_style(button, fixed(100.0, 30.0));
            button
        })
        .collect();
    assert!(tree.focus(buttons[5]));
    assert_eq!(
        tree.scroll(panel),
        Some(0.0),
        "nothing moves before a layout"
    );
    layout(&mut tree);
    assert_eq!(tree.scroll(panel), Some(0.0), "nor at the layout");

    tree.blur();
    tree.focus_next();
    assert_eq!(tree.focused(), Some(buttons[0]));
    assert_eq!(tree.scroll(panel), Some(0.0), "already in view");
    for _ in 0..3 {
        tree.focus_next();
    }
    assert_eq!(tree.focused(), Some(buttons[3]));
    assert_eq!(
        tree.scroll(panel),
        Some(30.0),
        "its bottom, 125, at the view's, 95"
    );
    assert!(tree.is_paint_dirty() && !tree.is_layout_dirty());

    assert!(tree.focus(buttons[9]));
    assert_eq!(
        tree.scroll(panel),
        Some(210.0),
        "the game's focus, to the end"
    );
    assert!(tree.focus(buttons[0]));
    assert_eq!(tree.scroll(panel), Some(0.0), "its top at the view's");
    assert!(!tree.scroll_into_view(buttons[0]), "nothing to move");

    // Nested: the outer panel fits the box where the inner one leaves it.
    let (mut nested, outer, inner) = nested();
    let last = *nested.children(inner).last().unwrap();
    assert!(nested.scroll_into_view(last));
    assert_eq!(nested.scroll(inner), Some(90.0));
    assert_eq!(
        nested.scroll(outer),
        Some(0.0),
        "the inner view is inside the outer one"
    );
}

#[test]
fn the_thumb_paints_after_the_children_in_the_panels_clip() {
    let (mut tree, _root, panel, children) = panel_rows(10);
    let list = paint(&mut tree, 1.0);
    assert_eq!(list.commands.len(), 12, "the panel, ten rows, the thumb");
    // Inside the 1-unit border: a track of 98, a third of it visible.
    let track = 98.0;
    let length = track * (100.0 / 310.0);
    let thumb = |top: f32, alpha: u8| PaintCommand::Rect {
        rect: Rect::new(Vec2::new(195.0, top), Vec2::new(199.0, top + length)),
        fill: [235, 235, 240, alpha],
        border: [0, 0, 0, 0],
        border_width: 0.0,
        corner_radius: 2.0,
        clip: 0,
    };
    assert_eq!(
        list.commands[11],
        thumb(1.0, 128),
        "the text colour at half its alpha"
    );
    assert_eq!(
        list.commands[1].clip(),
        1,
        "the rows paint in the panel's clip"
    );

    // An opaque row over the thumb's place still paints before it.
    tree.set_layout_style(children[0], fixed(200.0, 30.0));
    tree.set_scroll(panel, f32::INFINITY);
    tree.set_opacity(panel, 0.5);
    let list = paint(&mut tree, 1.0);
    assert_eq!(
        list.commands[11],
        thumb(1.0 + (track - length), 64),
        "at the end, faded"
    );

    let (mut fits, _root, _panel, _rows) = panel_rows(2);
    assert_eq!(
        paint(&mut fits, 1.0).commands.len(),
        3,
        "no thumb while it fits"
    );
}

#[test]
fn a_child_scrolled_out_of_the_clip_is_not_hit() {
    let (mut tree, _root, panel, rows) = panel_rows_at(50.0, 10);
    assert_eq!(tree.hit_test(Vec2::new(50.0, 60.0)), Some(rows[0]));
    tree.set_scroll(panel, 45.0);
    // Row 0 paints at 10..40, above the panel's top at 50.
    assert_eq!(
        tree.visual_rect(rows[0]),
        Some(rect(0.0, 10.0, 100.0, 30.0))
    );
    assert_eq!(tree.hit_test(Vec2::new(50.0, 20.0)), None);
    // Row 1 paints at 40..70: hit below the clip's top edge only.
    assert_eq!(tree.hit_test(Vec2::new(50.0, 45.0)), None);
    assert_eq!(tree.hit_test(Vec2::new(50.0, 60.0)), Some(rows[1]));
}
