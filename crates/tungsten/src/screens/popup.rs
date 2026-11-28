//! Tooltips and popups (redo 8.1's second plan, `D-31[6]`). A widget of a
//! modal screen hovered, or holding visible focus, for
//! [`TooltipSettings::delay`] real seconds shows its tooltip beside it: a
//! screen file's `tooltip` ([`screen_build::tooltip`]) or one
//! [`set_tooltip`] gave a widget built in Rust. [`tick`] does it, once a
//! frame, from each `tungsten::screens` modal screen's update, so nothing
//! runs while no screen is open. [`open`] pushes a screen file beside a
//! control as a popup, which a press outside it or a cancel closes.

use glam::Vec2;
use tungsten_core::ui::{
    Anchor, LayoutStyle, ScreenFile, Tooltip, UiEvent, UiKey, UiLayer, UiRouter, UiScreenRegistry,
    UiTree, Visibility, WidgetId, screen_build, widgets,
};
use tungsten_core::{EventQueue, InputState, MouseButton, ScaleFactor, Time, World};

use super::{DataScreen, idle, insert_resources, push_boxed, toast};
use crate::state::{RootCover, StateStack};

/// The widest a text tooltip's panel grows, in UI units.
pub const TOOLTIP_WIDTH: f32 = 320.0;
/// The gap between a widget and its tooltip or popup, in UI units.
const GAP: f32 = 4.0;

/// How tooltips time: a `World` resource [`tick`] reads, its default when
/// the world has none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TooltipSettings {
    /// Real seconds a widget is hovered or focused before its tooltip
    /// shows; 0.5.
    pub delay: f32,
}

impl Default for TooltipSettings {
    fn default() -> Self {
        Self { delay: 0.5 }
    }
}

/// A tooltip on show: the widget, its key, the tooltip's root, whether it
/// is a screen file's, and whether a layout has placed it.
#[derive(Debug, Clone, Copy)]
struct Shown {
    widget: WidgetId,
    key: Option<UiKey>,
    root: WidgetId,
    screen: bool,
    placed: bool,
}

/// The tooltips' state, a `World` resource [`tick`] inserts once a widget
/// has a tooltip: the widget timed and its seconds, the widget a press, a
/// drag or a scroll quieted until another takes its place, the tooltip on
/// show, the tooltips [`set_tooltip`] gave and the screen files a tooltip
/// refused, warned once each.
#[derive(Debug, Default)]
pub struct Tooltips {
    timing: Option<(WidgetId, f32)>,
    quiet: Option<WidgetId>,
    shown: Option<Shown>,
    own: Vec<(WidgetId, Tooltip)>,
    warned: Vec<String>,
}

/// Gives `widget`, built in Rust, `tooltip`, replacing one it had; a
/// screen file's own `tooltip` comes first.
pub fn set_tooltip(world: &mut World, widget: WidgetId, tooltip: Tooltip) {
    let mut own = std::mem::take(&mut tooltips_mut(world).own);
    let tree = world.get_resource::<UiTree>();
    own.retain(|(id, _)| *id != widget && tree.is_some_and(|tree| tree.contains(*id)));
    own.push((widget, tooltip));
    tooltips_mut(world).own = own;
}

/// The widget whose tooltip shows and its key: what a tooltip screen's
/// bindings read.
#[must_use]
pub fn tooltip_anchor(world: &World) -> Option<(WidgetId, Option<UiKey>)> {
    let shown = world.get_resource::<Tooltips>()?.shown?;
    Some((shown.widget, shown.key))
}

/// The root of the tooltip on show, once a layout has placed it.
#[must_use]
pub fn shown_tooltip(world: &World) -> Option<WidgetId> {
    let shown = world.get_resource::<Tooltips>()?.shown?;
    shown.placed.then_some(shown.root)
}

fn tooltips_mut(world: &mut World) -> &mut Tooltips {
    if !world.has_resource::<Tooltips>() {
        world.insert_resource(Tooltips::default());
    }
    world.resource_mut::<Tooltips>()
}

/// The tooltip of `widget`: its screen file's, else one [`set_tooltip`]
/// gave.
fn tooltip_of(world: &World, widget: WidgetId) -> Option<&Tooltip> {
    screen_build::tooltip(world, widget).or_else(|| {
        let tooltips = world.get_resource::<Tooltips>()?;
        let mut own = tooltips.own.iter();
        own.find(|(id, _)| *id == widget)
            .map(|(_, tooltip)| tooltip)
    })
}

/// The pointer in UI units, while the router has it in the window.
fn pointer(world: &World) -> Option<Vec2> {
    let router = world.get_resource::<UiRouter>()?;
    if !router.pointer_over_ui() {
        return None;
    }
    let (x, y) = world.get_resource::<InputState>()?.cursor_position()?;
    let scale = world
        .get_resource::<ScaleFactor>()
        .map_or(1.0, ScaleFactor::get);
    Some(Vec2::new(x, y) / scale)
}

/// The widget of `root` whose tooltip may show: none while a press or a
/// drag holds the pointer; the nearest at or above the widget under the
/// pointer with a tooltip; else the widget holding visible focus, when it
/// has one, wherever the pointer rests.
fn candidate(world: &World, root: WidgetId) -> Option<WidgetId> {
    let router = world.get_resource::<UiRouter>();
    if router.is_some_and(|router| router.captured().is_some() || router.dragging().is_some()) {
        return None;
    }
    let tree = world.get_resource::<UiTree>()?;
    let hit = pointer(world)
        .and_then(|point| tree.hit_test(point))
        .filter(|hit| tree.root_of(*hit) == Some(root));
    let mut at = hit;
    while let Some(id) = at {
        if tooltip_of(world, id).is_some() {
            return Some(id);
        }
        at = tree.parent(id);
    }
    tree.focused()
        .filter(|_| tree.focus_visible())
        .filter(|focused| tree.root_of(*focused) == Some(root))
        .filter(|focused| tooltip_of(world, *focused).is_some())
}

/// Whether this routing pressed, activated, dragged or scrolled.
fn hushed(world: &World) -> bool {
    world
        .get_resource::<EventQueue<UiEvent>>()
        .is_some_and(|events| {
            events.iter_current().any(|event| {
                matches!(
                    event,
                    UiEvent::Pressed { .. }
                        | UiEvent::Activated { .. }
                        | UiEvent::DragStarted { .. }
                        | UiEvent::Scrolled { .. }
                )
            })
        })
}

/// The screen at `root`'s tooltip turn, once a frame from its update
/// (`D-31[6]`): a press, an activation, a drag or a scroll this routing
/// hides the tooltip and quiets its widget until another takes its place;
/// the widget whose tooltip may show (the one under the pointer, else the
/// one holding visible focus) is timed on the real clock and shows its
/// tooltip once [`TooltipSettings::delay`] has passed; the tooltip follows
/// its widget, shown once placed, a screen file's refreshed; it goes when
/// another widget, or none, takes its widget's place, and while a screen
/// change waits or runs. A world where no widget has a tooltip is left
/// alone.
pub fn tick(world: &mut World, root: WidgetId) {
    let candidate = if idle(world) {
        candidate(world, root)
    } else {
        None
    };
    if candidate.is_none() && !world.has_resource::<Tooltips>() {
        return;
    }
    let dt = world.get_resource::<Time>().map_or(0.0, Time::real_delta);
    let delay = world
        .get_resource::<TooltipSettings>()
        .copied()
        .unwrap_or_default()
        .delay;
    let hushed = hushed(world);
    let tooltips = tooltips_mut(world);
    if hushed {
        tooltips.quiet = candidate;
    } else if tooltips.quiet != candidate {
        tooltips.quiet = None;
    }
    let wanted = candidate.filter(|widget| tooltips.quiet != Some(*widget));
    if let Some(shown) = tooltips.shown.filter(|shown| Some(shown.widget) != wanted) {
        tooltips.shown = None;
        world.resource_mut::<UiTree>().remove(shown.root);
    }
    let tooltips = tooltips_mut(world);
    let Some(widget) = wanted else {
        tooltips.timing = None;
        return;
    };
    let seconds = match tooltips.timing {
        Some((timed, seconds)) if timed == widget => seconds + dt,
        _ => 0.0,
    };
    tooltips.timing = Some((widget, seconds));
    if tooltips.shown.is_none() && seconds >= delay {
        show(world, widget);
    }
    follow(world);
}

/// Builds `widget`'s tooltip, hidden until a layout places it: a text's
/// `small` label in a panel at most [`TOOLTIP_WIDTH`] wide, or a screen
/// file's bare root, which takes no hits; owned by the state showing it,
/// hidden while another covers it. A screen file whose root is not a bare,
/// non-modal root shows nothing, warned once.
fn show(world: &mut World, widget: WidgetId) {
    let Some(tooltip) = tooltip_of(world, widget).cloned() else {
        return;
    };
    let (root, screen) = match tooltip {
        Tooltip::Text(text) => {
            let tree = world.resource_mut::<UiTree>();
            let spacing = tree.theme().tokens.spacing;
            let style = LayoutStyle::column().anchored(Anchor::TopStart, Vec2::ZERO);
            let root = tree.add_root(UiLayer::GamePopup, style);
            tree.set_bare(root, true);
            let mut panel = LayoutStyle::default().with_padding(spacing.sm);
            panel.max_width = Some(TOOLTIP_WIDTH);
            let panel = tree.panel(root, panel);
            widgets::small(tree, panel, &text);
            (root, false)
        }
        Tooltip::Screen(id) => {
            let refusal = world
                .get_resource::<UiScreenRegistry>()
                .and_then(|registry| registry.get(&id))
                .map_or(
                    Some("no ui_screens file has this ID"),
                    ScreenFile::tooltip_refusal,
                );
            if let Some(why) = refusal {
                let tooltips = tooltips_mut(world);
                if !tooltips.warned.contains(&id) {
                    log::warn!("tooltip screen '{id}': {why}; no tooltip shows");
                    tooltips.warned.push(id);
                }
                return;
            }
            let Some(built) = screen_build::build(world, &id) else {
                return;
            };
            (built.root, true)
        }
        _ => return,
    };
    let tree = world.resource_mut::<UiTree>();
    no_hits(tree, root);
    // Anchored from the start, so the first layout gives its own size.
    tree.set_window_position(root, Vec2::ZERO);
    tree.set_visibility(root, Visibility::Hidden);
    let key = tree.key(widget);
    if let Some(stack) = world.get_resource_mut::<StateStack>() {
        stack.own_root(root, RootCover::Hide);
    }
    toast::raise(world);
    tooltips_mut(world).shown = Some(Shown {
        widget,
        key,
        root,
        screen,
        placed: false,
    });
}

/// `id` and its subtree take no hits.
fn no_hits(tree: &mut UiTree, id: WidgetId) {
    tree.set_hit_testable(id, false);
    for index in 0..tree.children(id).len() {
        let child = tree.children(id)[index];
        no_hits(tree, child);
    }
}

/// The tooltip on show follows its widget, shown once placed; a screen
/// file's refreshes, its rebuilt rows taking no hits either.
fn follow(world: &mut World) {
    let Some(mut shown) = world.get_resource::<Tooltips>().and_then(|tips| tips.shown) else {
        return;
    };
    if shown.screen {
        screen_build::refresh(world, shown.root);
        no_hits(world.resource_mut::<UiTree>(), shown.root);
    }
    let tree = world.resource_mut::<UiTree>();
    if place_beside(tree, shown.root, shown.widget) && !shown.placed {
        tree.set_visibility(shown.root, Visibility::Visible);
        shown.placed = true;
        tooltips_mut(world).shown = Some(shown);
    }
}

/// Places `root` beside `widget`: below it, above when its layer's
/// viewport has no room below, shifted inside the viewport by its last
/// laid-out size ([`UiTree::set_window_position`]). `false`, placing
/// nothing, before both have a box.
pub(super) fn place_beside(tree: &mut UiTree, root: WidgetId, widget: WidgetId) -> bool {
    let (Some(anchor), Some(size)) = (tree.visual_rect(widget), tree.rect(root).map(|r| r.size()))
    else {
        return false;
    };
    let layer = tree.layer(root).unwrap_or(UiLayer::GamePopup);
    let bottom = tree
        .layer_viewport(layer)
        .map_or(f32::INFINITY, |view| view.y);
    let below = anchor.max.y + GAP;
    let y = if below + size.y > bottom {
        anchor.min.y - GAP - size.y
    } else {
        below
    };
    tree.set_window_position(root, Vec2::new(anchor.min.x, y));
    true
}

/// The controls open popups opened beside, and their keys, the front-most
/// last: a `World` resource a popup's `on_enter` pushes to and its
/// `on_exit` pops.
#[derive(Debug, Default)]
pub struct PopupAnchors(Vec<(WidgetId, Option<UiKey>)>);

/// The control the front-most popup opened beside and its key: what its
/// bindings and actions read.
#[must_use]
pub fn popup_anchor(world: &World) -> Option<(WidgetId, Option<UiKey>)> {
    world.get_resource::<PopupAnchors>()?.0.last().copied()
}

/// Pushes the `ui_screens` file `id` as a popup beside `beside`
/// (`D-31[6]`): a data screen whose root, a bare `root` modal on the popup
/// layer, is placed as a tooltip is and popped by a press outside it or a
/// cancel; its actions run as a data screen's, reading [`popup_anchor`].
/// `false` while the screens are not [`idle`](super::idle), and for a file
/// that is missing or whose root does not fit, warned.
pub fn open(world: &mut World, id: &str, beside: WidgetId) -> bool {
    insert_resources(world);
    let refusal = world
        .get_resource::<UiScreenRegistry>()
        .and_then(|registry| registry.get(id))
        .map_or(
            Some("no ui_screens file has this ID"),
            ScreenFile::popup_refusal,
        );
    if let Some(why) = refusal {
        log::warn!("popup screen '{id}': {why}; it does not open");
        return false;
    }
    if !idle(world) {
        return false;
    }
    let Some(state) = world.resource_mut::<UiScreenRegistry>().intern(id) else {
        return false;
    };
    let key = world
        .get_resource::<UiTree>()
        .and_then(|tree| tree.key(beside));
    push_boxed(world, Box::new(DataScreen::popup(state, beside, key)))
}

/// A popup's `on_enter`: its anchor goes on [`PopupAnchors`].
pub(super) fn anchored(world: &mut World, beside: WidgetId, key: Option<UiKey>) {
    if !world.has_resource::<PopupAnchors>() {
        world.insert_resource(PopupAnchors::default());
    }
    world.resource_mut::<PopupAnchors>().0.push((beside, key));
}

/// A popup's `on_exit`: its anchor leaves [`PopupAnchors`].
pub(super) fn closed(world: &mut World) {
    if let Some(anchors) = world.get_resource_mut::<PopupAnchors>() {
        anchors.0.pop();
    }
}

/// Whether the primary button went down this frame outside `root`'s box,
/// once `root` has one.
pub(super) fn pressed_outside(world: &World, root: WidgetId) -> bool {
    let pressed = world
        .get_resource::<InputState>()
        .is_some_and(|input| input.mouse_just_pressed(MouseButton::Left));
    let Some(rect) = world
        .get_resource::<UiTree>()
        .and_then(|tree| tree.visual_rect(root))
    else {
        return false;
    };
    pressed && !pointer(world).is_some_and(|point| rect.contains(point))
}

#[cfg(test)]
#[path = "../tests/popup.rs"]
mod tests;
