//! Short messages at the bottom of the screen, on a root no state owns, so
//! a screen change keeps them; each leaves after [`TOAST_SECS`] on the real
//! clock, so a pause does not hold it.

use tungsten::core::ui::{Align, Anchor, LayoutStyle, UiLayer, UiTree, WidgetId};
use tungsten::core::{Time, Timer, World};

use super::{theme, widgets};

/// How long a toast shows, in real seconds.
pub const TOAST_SECS: f32 = 2.5;

/// The toasts: a `World` resource.
#[derive(Debug, Default)]
pub struct Toasts {
    root: Option<WidgetId>,
    /// Each shown toast's panel, its label and its timer on the real clock.
    shown: Vec<(WidgetId, WidgetId, Timer)>,
    pending: Vec<String>,
}

impl Toasts {
    /// The texts on screen, oldest first.
    #[must_use]
    pub fn texts(&self, tree: &UiTree) -> Vec<String> {
        self.shown
            .iter()
            .map(|(_, label, _)| widgets::text_of(tree, *label))
            .collect()
    }
}

/// Shows `text` from the next frame.
pub fn toast(world: &mut World, text: impl Into<String>) {
    world.resource_mut::<Toasts>().pending.push(text.into());
}

/// Shows the pending toasts and ages the shown ones: part of `shell_frame`.
pub(crate) fn tick(world: &mut World) {
    let dt = world.resource::<Time>().real_delta();
    let Some(mut toasts) = world.remove_resource::<Toasts>() else {
        return;
    };
    if let Some(tree) = world.get_resource_mut::<UiTree>() {
        let root = *toasts.root.get_or_insert_with(|| {
            let mut style = LayoutStyle::column()
                .with_gap(theme::GAP)
                .anchored(Anchor::BottomCenter, [0.0, theme::MARGIN].into());
            style.align_items = Align::Center;
            let root = tree.add_root(UiLayer::GamePopup, style);
            tree.set_hit_testable(root, false);
            root
        });
        for (panel, _, timer) in &mut toasts.shown {
            if timer.tick(dt) > 0 {
                tree.remove(*panel);
            }
        }
        toasts.shown.retain(|(.., timer)| !timer.finished());
        for text in toasts.pending.drain(..) {
            let panel = tree.panel(root, LayoutStyle::default().with_padding(theme::GAP));
            let label = tree.label(panel, text.as_str());
            toasts.shown.push((panel, label, Timer::once(TOAST_SECS)));
        }
        if !toasts.shown.is_empty() {
            tree.raise(root);
        }
    }
    world.insert_resource(toasts);
}
