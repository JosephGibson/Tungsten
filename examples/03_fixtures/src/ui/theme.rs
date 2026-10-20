//! One place for every look and timing value the lab's screens share: the
//! theme asset's ID and its apply (`D-148`), spacing and sizes, the colours
//! the theme file has no token for, and the motion constants every screen
//! change reads. A game copies this module to keep its numbers out of the
//! screens that use them. Colours, radii, kinds' states and text tokens
//! live in the theme file, `assets/ui/themes/ex05_default.json`, which
//! hot-reloads; layout is not themable (`D-124`), so spacing and sizes are
//! code constants here.
//!
//! The palette keeps text at 4.5:1 or more against its background and
//! focus borders, rarity stripes and control indicators at 3:1 or more
//! (WCAG 1.4.11), composited in linear light as the overlay blends.

use glam::Vec2;
use tungsten::core::ui::{UiThemeRegistry, UiTrack, UiTree, WidgetId};
use tungsten::core::{Easing, World};
use tungsten::{Transition, TransitionEffect};

use crate::ui::inventory::items::Rarity;

/// The `ui_themes` ID the lab applies.
pub const THEME_ID: &str = "ex05_default";

/// The fade every screen change takes, each way, in seconds: short
/// enough that a menu answers at once.
pub const FADE_SECS: f32 = 0.08;
/// How long a new screen slides up into place, in real seconds.
pub const SLIDE_SECS: f32 = 0.12;
/// How far below its place a new screen starts, in UI units.
pub const SLIDE_UNITS: f32 = 12.0;
/// The slide's easing.
pub const SLIDE_EASING: Easing = Easing::CubicOut;
/// How long a popup takes to fade in when it shows, in real seconds.
pub const POP_SECS: f32 = 0.08;

// ---- spacing and sizes, in UI units ----

/// The spacing steps every gap and padding takes.
pub const SPACE: [f32; 5] = [4.0, 8.0, 12.0, 16.0, 24.0];
/// Inside a window or a screen, all round.
pub const WINDOW_PADDING: f32 = SPACE[3];
/// Between two rows, and between a caption and its control.
pub const ROW_GAP: f32 = SPACE[1];
/// Between two sections of a window.
pub const SECTION_GAP: f32 = SPACE[2];
/// Between a HUD row or a corner window and the screen's edge.
pub const SCREEN_MARGIN: f32 = SPACE[3];
/// A button's padding: horizontal, vertical.
pub const BUTTON_PADDING: Vec2 = Vec2::new(14.0, 6.0);
/// A button's least width; a row of action buttons takes it exactly.
pub const BUTTON_MIN_WIDTH: f32 = 96.0;
/// A setting row's caption column.
pub const CAPTION_WIDTH: f32 = 120.0;
/// A setting row's control column: a slider's width.
pub const CONTROL_WIDTH: f32 = 140.0;
/// A heading's size and line height; body text is the theme's 16 on 22.
pub const HEADING_SIZE: f32 = 22.0;
pub const HEADING_LINE: f32 = 28.0;
/// A slot's side.
pub const SLOT: f32 = 44.0;
/// An item icon's side.
pub const ICON: f32 = 32.0;
/// The gap between two slots.
pub const SLOT_GAP: f32 = ROW_GAP;
/// The rarity stripe's height, and its inset from the slot's edges.
pub const STRIPE: f32 = 3.0;
pub const STRIPE_INSET: f32 = SPACE[0];
/// The inventory's detail pane: three action buttons, their gaps and its
/// padding.
pub const PANE_WIDTH: f32 = 3.0 * BUTTON_MIN_WIDTH + 2.0 * ROW_GAP + 2.0 * SECTION_GAP;

// ---- colours the theme file has no token for ----

/// The raised surface of slots, the detail pane and popups: a background
/// override over the theme's window surface.
pub const RAISED: [u8; 4] = [42, 43, 52, 255];
/// The "Equipped" badge's fill: the theme's accent.
pub const BADGE: [u8; 4] = [53, 99, 201, 255];
/// The badge's text: the theme's accent text.
pub const BADGE_TEXT: [u8; 4] = [255, 255, 255, 255];
/// Muted text: an empty slot's name in the pane.
pub const MUTED: [u8; 4] = [160, 162, 175, 255];

/// A rarity's colour: its slot stripe and its item's name.
#[must_use]
pub const fn rarity_color(rarity: Rarity) -> [u8; 4] {
    match rarity {
        Rarity::Common => [154, 160, 166, 255],
        Rarity::Uncommon => [92, 196, 110, 255],
        Rarity::Rare => [84, 150, 255, 255],
        Rarity::Epic => [184, 120, 255, 255],
        Rarity::Legendary => [245, 166, 50, 255],
    }
}

/// Applies the lab's theme from the registry to `tree`; without it the
/// built-in theme draws.
pub fn apply(world: &World, tree: &mut UiTree) {
    let theme = world
        .get_resource::<UiThemeRegistry>()
        .and_then(|registry| registry.get(THEME_ID).cloned());
    match theme {
        Some(theme) => tree.set_theme_asset(THEME_ID, theme),
        None => println!("ui theme '{THEME_ID}' is not loaded; the built-in theme draws"),
    }
}

/// A fade to black and back, [`FADE_SECS`] each way: every screen change's
/// transition (`D-093`).
#[must_use]
pub fn fade() -> Transition {
    Transition::new(
        TransitionEffect::Fade {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        FADE_SECS,
    )
}

/// Slides `root` up [`SLIDE_UNITS`] into place over [`SLIDE_SECS`] on the
/// real clock: a new screen's entrance (`D-163`).
pub fn slide_in(tree: &mut UiTree, root: WidgetId) {
    tree.animate(
        root,
        UiTrack::translation(Vec2::new(0.0, SLIDE_UNITS), Vec2::ZERO, SLIDE_SECS)
            .with_easing(SLIDE_EASING),
    );
}

/// Fades `root` in from transparent over [`POP_SECS`] on the real clock: a
/// popup's entrance as it shows (`D-163`).
pub fn pop_in(tree: &mut UiTree, root: WidgetId) {
    tree.animate(root, UiTrack::opacity(0.0, 1.0, POP_SECS));
}
