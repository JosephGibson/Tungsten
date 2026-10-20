//! The shell's look, motion and sound in one place: the theme asset's ID
//! and its apply, the spacing and sizes every screen shares, the fades and
//! slides each screen change takes, and the menu sounds' manifest IDs. Colours, borders, radii and font
//! families live in `assets/ui/themes/shell.json`, which hot-reloads; layout
//! is not themable, so spacing and sizes are constants here.

use tungsten::core::ui::{UiThemeRegistry, UiTrack, UiTree, WidgetId};
use tungsten::core::{Easing, World};

/// The `ui_themes` ID the shell applies.
pub const THEME_ID: &str = "shell";
/// How long a pushed screen fades and slides in, in real seconds; it acts
/// on its first frame.
pub const PUSH_SECS: f32 = 0.10;
/// How long a closing screen fades and slides out before it pops.
pub const POP_SECS: f32 = 0.08;
/// How far a screen slides, in UI units: up into place, down out of it.
pub const SLIDE_UNITS: f32 = 6.0;
/// A run change's curtain: its fade in, before the stack changes under it,
/// and its fade out.
pub const CURTAIN_IN_SECS: f32 = 0.10;
pub const CURTAIN_OUT_SECS: f32 = 0.14;
/// The scrim's alpha over the screen below: the theme's background at 72 %.
pub const SCRIM_ALPHA: u8 = 184;
/// The menu sounds, by manifest ID: a focus move a navigation press made,
/// an activation, and a cancel or Back.
pub const MENU_SOUNDS: [&str; 3] = ["ui_move", "ui_select", "ui_back"];
/// The window height, in physical pixels, that lays the UI out at scale 1:
/// a taller window scales the UI up with it, a shorter one keeps the OS
/// scale (`D-184`).
pub const REFERENCE_HEIGHT: f32 = 720.0;
/// The smallest viewport the UI lays out in, in UI units, whatever the
/// scales: a 1280 × 720 window at the OS scale 1.75.
pub const MIN_VIEWPORT: [f32; 2] = [731.0, 411.0];

/// Inside a screen, all round, in UI units.
pub const PADDING: f32 = 16.0;
/// Between two rows.
pub const GAP: f32 = 8.0;
/// Between a screen's edge and the HUD.
pub const MARGIN: f32 = 16.0;
/// A button's padding: horizontal, vertical.
pub const BUTTON_PADDING: [f32; 2] = [14.0, 5.0];
/// A menu's width, and a wide screen's: a card's design width, which a
/// narrower viewport narrows.
pub const MENU_WIDTH: f32 = 260.0;
pub const WIDE_WIDTH: f32 = 600.0;
/// A slider's width and the least width of a setting's choice, which holds
/// no text or grows with it, and of a readout; a caption takes what is left
/// and wraps.
pub const CONTROL_WIDTH: f32 = 170.0;
pub const READOUT_WIDTH: f32 = 64.0;
/// A heading's size and line height, in the heading family; body text is
/// the theme's 16 on 22.
pub const HEADING_SIZE: f32 = 20.0;
pub const HEADING_LINE: f32 = 28.0;
/// The title's size and line height, in the heading family.
pub const TITLE_SIZE: f32 = 40.0;
pub const TITLE_LINE: f32 = 48.0;
/// Small print: the title's version and "Saving is off".
pub const SMALL: [f32; 2] = [13.0, 18.0];

/// Applies the shell's theme to `world`'s tree when it is not already on;
/// without the asset the built-in theme draws.
pub fn apply(world: &mut World) {
    let theme = world
        .get_resource::<UiThemeRegistry>()
        .and_then(|registry| registry.get(THEME_ID).cloned());
    let Some(tree) = world.get_resource_mut::<UiTree>() else {
        return;
    };
    match theme {
        Some(theme) if tree.theme_id() != Some(THEME_ID) => tree.set_theme_asset(THEME_ID, theme),
        Some(_) => {}
        None => log::warn!("ui theme '{THEME_ID}' is not loaded; the built-in theme draws"),
    }
}

/// Fades `root` in and slides it up [`SLIDE_UNITS`] into place over
/// [`PUSH_SECS`] on the real clock, with no overshoot: a new screen's
/// entrance.
pub fn slide_in(tree: &mut UiTree, root: WidgetId) {
    let ease = |track: UiTrack| track.with_easing(Easing::CubicOut);
    tree.animate(root, ease(UiTrack::opacity(0.0, 1.0, PUSH_SECS)));
    let from = [0.0, SLIDE_UNITS].into();
    tree.animate(
        root,
        ease(UiTrack::translation(from, [0.0, 0.0].into(), PUSH_SECS)),
    );
}

/// Fades `root` out and slides it down [`SLIDE_UNITS`] over [`POP_SECS`]:
/// a closing screen's exit.
pub fn slide_out(tree: &mut UiTree, root: WidgetId) {
    tree.animate(root, UiTrack::opacity(1.0, 0.0, POP_SECS));
    let to = [0.0, SLIDE_UNITS].into();
    tree.animate(root, UiTrack::translation([0.0, 0.0].into(), to, POP_SECS));
}
