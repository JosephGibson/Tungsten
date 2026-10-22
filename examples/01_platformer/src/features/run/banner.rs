//! The area banner: the name of the area the player enters, as a title at
//! the top centre, one at a time: the heading face bold, large and spaced,
//! over no box, with a soft dark ring and a drop behind it so it stays
//! legible over bright scenery. A new area's name waits while the shown one
//! fades out quickly, then fades in from nothing; the fades run on the game
//! clock, so a pause holds them. The shell's toasts stay for everything
//! else.

use glam::Vec2;
use tungsten::RootCover;
use tungsten::core::ui::{Anchor, LayoutStyle, UiLayer, UiTree, WidgetId};
use tungsten::core::{Time, World};

use crate::shell::{self, theme, widgets};

/// How long a name shows, its fade in and its fade out, and how quickly a
/// shown name gives way to a new one, in game seconds.
pub const BANNER_SECS: f32 = 4.0;
pub const BANNER_FADE_IN: f32 = 0.8;
pub const BANNER_FADE_OUT: f32 = 1.2;
pub const BANNER_SWAP: f32 = 0.25;
/// The title's size and line height, its weight (the serif family's Bold
/// face), its letter spacing in em, and the most units its size may reach
/// as the text scale grows.
pub const TITLE_SIZE: [f32; 2] = [34.0, 42.0];
pub const TITLE_WEIGHT: u16 = 700;
pub const TITLE_SPACING: f32 = 0.12;
pub const TITLE_CAP: f32 = 52.0;
/// The shadow behind the title: a ring of copies two units out at a faint
/// dark, and a drop below and right at a deeper one, painted in that order
/// under the title.
const RING: f32 = 2.0;
const RING_COLOR: [u8; 4] = [6, 5, 10, 70];
const DROP: Vec2 = Vec2::new(1.0, 3.0);
const DROP_COLOR: [u8; 4] = [6, 5, 10, 130];

/// The banner's root and labels (the shadow's copies, then the title), the
/// newest name, for how long its labels have shown it, the opacity it is
/// drawn at, and the shown name's quick fade out while a new one waits:
/// its seconds so far and the opacity it began at.
#[derive(Debug, Clone)]
pub struct Banner {
    root: WidgetId,
    labels: Vec<WidgetId>,
    name: Option<String>,
    shown: f32,
    opacity: f32,
    swap: Option<(f32, f32)>,
}

/// Builds the run's banner, empty, its root owned by gameplay and drawn
/// with no fill or border.
pub fn start(world: &mut World) {
    let tree = world.resource_mut::<UiTree>();
    let style = LayoutStyle::column().anchored(Anchor::TopCenter, [0.0, theme::MARGIN].into());
    let root = tree.add_root(UiLayer::Game, style);
    tree.set_hit_testable(root, false);
    widgets::fill(tree, root, [0; 4]);
    let mut shadow: Vec<(Vec2, [u8; 4])> = [
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
    ]
    .into_iter()
    .map(|(x, y)| (Vec2::new(x, y) * RING, RING_COLOR))
    .collect();
    shadow.push((DROP, DROP_COLOR));
    let mut labels = Vec::new();
    for (offset, color) in shadow {
        let label = title(tree, root);
        tree.set_layout_style(
            label,
            LayoutStyle::default().anchored(Anchor::Center, offset),
        );
        widgets::restyle(tree, label, |text| text.color = Some(color));
        labels.push(label);
    }
    labels.push(title(tree, root));
    tree.set_opacity(root, 0.0);
    for &label in &labels {
        widgets::capped(world, label, TITLE_SIZE, TITLE_CAP);
    }
    shell::stack(world).own_root(root, RootCover::Show);
    world.insert_resource(Banner {
        root,
        labels,
        name: None,
        shown: 0.0,
        opacity: 0.0,
        swap: None,
    });
}

/// One of the banner's labels: the heading face, bold and spaced.
fn title(tree: &mut UiTree, root: WidgetId) -> WidgetId {
    let label = widgets::display(tree, root, "", TITLE_SIZE);
    widgets::restyle(tree, label, |text| {
        text.weight = Some(TITLE_WEIGHT);
        text.letter_spacing = Some(TITLE_SPACING);
    });
    label
}

/// `t` in `0..=1` eased in and out.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A name's opacity `shown` seconds into its own fade: eased in over
/// [`BANNER_FADE_IN`], held, and eased out over the last
/// [`BANNER_FADE_OUT`].
fn opacity(shown: f32) -> f32 {
    ease(shown / BANNER_FADE_IN).min(ease((BANNER_SECS - shown) / BANNER_FADE_OUT))
}

/// Shows `name` in place of any name shown: at once from nothing, or, while
/// one is drawn, once it has faded out over [`BANNER_SWAP`].
pub fn show(world: &mut World, name: &str) {
    let Some(banner) = world.get_resource_mut::<Banner>() else {
        return;
    };
    let drawn = banner.name.is_some() && banner.opacity > 0.0;
    banner.name = Some(name.to_owned());
    if drawn {
        let from = banner.swap.map_or(banner.opacity, |(_, from)| from);
        let elapsed = banner.swap.map_or(0.0, |(elapsed, _)| elapsed);
        banner.swap = Some((elapsed, from));
        return;
    }
    banner.shown = 0.0;
    banner.swap = None;
    let labels = banner.labels.clone();
    let tree = world.resource_mut::<UiTree>();
    for label in labels {
        tree.set_plain_text(label, name);
    }
}

/// `run_banner`, each frame: the shown name's fade, or its quick fade out
/// with the newest name put in its place at the end.
pub fn tick(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let Some(banner) = world.get_resource_mut::<Banner>() else {
        return;
    };
    let Some(name) = banner.name.clone() else {
        return;
    };
    let mut retext = false;
    banner.opacity = match banner.swap {
        Some((elapsed, from)) => {
            let elapsed = elapsed + dt;
            if elapsed >= BANNER_SWAP {
                banner.swap = None;
                banner.shown = 0.0;
                retext = true;
                0.0
            } else {
                banner.swap = Some((elapsed, from));
                from * (1.0 - ease(elapsed / BANNER_SWAP))
            }
        }
        None => {
            banner.shown += dt;
            if banner.shown >= BANNER_SECS {
                banner.name = None;
            }
            opacity(banner.shown)
        }
    };
    let (root, opacity, labels) = (banner.root, banner.opacity, banner.labels.clone());
    let tree = world.resource_mut::<UiTree>();
    if retext {
        for label in labels {
            tree.set_plain_text(label, &name);
        }
    }
    tree.set_opacity(root, opacity.clamp(0.0, 1.0));
}

/// The name the banner shows, while it does.
#[must_use]
pub fn shown(world: &World) -> Option<&str> {
    world.get_resource::<Banner>()?.name.as_deref()
}
