//! Short messages at the bottom of the screen (redo 7.17), on a root no
//! state owns, so a screen change, a reset's fade included, keeps them;
//! each shows [`TOAST_SECS`] on the real clock, so a pause does not hold
//! it, timed by its own animation track.

use tungsten_core::World;
use tungsten_core::ui::{Align, Anchor, LayoutStyle, UiLayer, UiTrack, UiTree, WidgetId};

/// How long a toast shows, in real seconds.
pub const TOAST_SECS: f32 = 2.5;

/// The toasts: a `World` resource [`ScreensPlugin`](super::ScreensPlugin)
/// inserts.
#[derive(Debug, Default)]
pub struct Toasts {
    root: Option<WidgetId>,
    /// Each toast's panel and label, oldest first.
    shown: Vec<(WidgetId, WidgetId)>,
}

impl Toasts {
    /// The texts on screen, oldest first: the toasts whose time is not up.
    #[must_use]
    pub fn texts(&self, tree: &UiTree) -> Vec<String> {
        self.shown
            .iter()
            .filter(|(panel, _)| showing(tree, *panel))
            .map(|(_, label)| tree.plain_text(*label))
            .collect()
    }
}

/// Whether a toast's panel still shows: its track runs, or its opacity
/// holds above 0.
fn showing(tree: &UiTree, panel: WidgetId) -> bool {
    tree.contains(panel)
        && (tree.is_animating(panel) || tree.opacity(panel).is_some_and(|alpha| alpha > 0.0))
}

/// Shows `text` at the bottom of the screen, above every screen, for
/// [`TOAST_SECS`] on the real clock; the toasts whose time is up leave the
/// tree here.
pub fn toast(world: &mut World, text: impl Into<String>) {
    let text = text.into();
    if !world.has_resource::<Toasts>() {
        world.insert_resource(Toasts::default());
    }
    let mut toasts = std::mem::take(world.resource_mut::<Toasts>());
    let tree = world.resource_mut::<UiTree>();
    toasts.shown.retain(|&(panel, _)| {
        let keep = showing(tree, panel);
        if !keep {
            tree.remove(panel);
        }
        keep
    });
    let spacing = tree.theme().tokens.spacing;
    let root = if let Some(root) = toasts.root.filter(|root| tree.contains(*root)) {
        root
    } else {
        let mut style = LayoutStyle::column()
            .with_gap(spacing.sm)
            .anchored(Anchor::BottomCenter, [0.0, spacing.lg].into());
        style.align_items = Align::Center;
        let root = tree.add_root(UiLayer::GamePopup, style);
        tree.set_bare(root, true);
        tree.set_hit_testable(root, false);
        root
    };
    toasts.root = Some(root);
    let panel = tree.panel(root, LayoutStyle::default().with_padding(spacing.sm));
    let label = tree.label(panel, text.as_str());
    let hold = UiTrack::opacity(1.0, 1.0, TOAST_SECS).then(UiTrack::opacity(0.0, 0.0, 0.0));
    tree.animate_chain(panel, hold);
    tree.raise(root);
    toasts.shown.push((panel, label));
    *world.resource_mut::<Toasts>() = toasts;
}

/// Raises the toasts over every root: after a screen opens.
pub(crate) fn raise(world: &mut World) {
    let Some(root) = world
        .get_resource::<Toasts>()
        .and_then(|toasts| toasts.root)
    else {
        return;
    };
    if let Some(tree) = world.get_resource_mut::<UiTree>() {
        tree.raise(root);
    }
}
