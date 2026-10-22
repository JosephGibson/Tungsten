//! The area banner: the name of the area the player enters, in the theme's
//! heading face at the top centre, one at a time. A new area replaces the
//! name and restarts its fade; the fade runs on the game clock, so a pause
//! holds it. The shell's toasts stay for everything else.

use tungsten::RootCover;
use tungsten::core::ui::{Anchor, LayoutStyle, UiLayer, UiTree, WidgetId};
use tungsten::core::{Time, World};

use crate::shell::{self, theme, widgets};

/// How long a name shows, and its fade in and out, in game seconds.
pub const BANNER_SECS: f32 = 2.5;
pub const BANNER_FADE: f32 = 0.35;

/// The banner's root and label, the name it shows and for how long it has.
#[derive(Debug, Clone)]
pub struct Banner {
    root: WidgetId,
    label: WidgetId,
    name: Option<String>,
    shown: f32,
}

/// Builds the run's banner, empty, its root owned by gameplay.
pub fn start(world: &mut World) {
    let tree = world.resource_mut::<UiTree>();
    let style = LayoutStyle::column().anchored(Anchor::TopCenter, [0.0, theme::MARGIN].into());
    let root = tree.add_root(UiLayer::Game, style);
    tree.set_hit_testable(root, false);
    let size = [theme::HEADING_SIZE, theme::HEADING_LINE];
    let label = widgets::display(tree, root, "", size);
    tree.set_opacity(root, 0.0);
    shell::stack(world).own_root(root, RootCover::Show);
    world.insert_resource(Banner {
        root,
        label,
        name: None,
        shown: 0.0,
    });
}

/// Shows `name`, in place of any name shown, its fade started again.
pub fn show(world: &mut World, name: &str) {
    let Some(banner) = world.get_resource_mut::<Banner>() else {
        return;
    };
    banner.name = Some(name.to_owned());
    banner.shown = 0.0;
    let label = banner.label;
    world.resource_mut::<UiTree>().set_plain_text(label, name);
    tick(world);
}

/// `run_banner`, each frame: the shown name's fade.
pub fn tick(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let Some(banner) = world.get_resource_mut::<Banner>() else {
        return;
    };
    if banner.name.is_none() {
        return;
    }
    banner.shown += dt;
    let left = BANNER_SECS - banner.shown;
    let opacity = (banner.shown / BANNER_FADE).min(left / BANNER_FADE);
    if left <= 0.0 {
        banner.name = None;
    }
    let root = banner.root;
    world
        .resource_mut::<UiTree>()
        .set_opacity(root, opacity.clamp(0.0, 1.0));
}

/// The name the banner shows, while it does.
#[must_use]
pub fn shown(world: &World) -> Option<&str> {
    world.get_resource::<Banner>()?.name.as_deref()
}
