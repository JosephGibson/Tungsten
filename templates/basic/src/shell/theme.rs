//! The shell's look and motion in one place: the theme asset's ID and its
//! apply, the spacing and sizes every screen shares, and the fade and slide
//! each screen change takes. Colours, borders and radii live in
//! `assets/ui/themes/shell.json`, which hot-reloads; layout is not themable,
//! so spacing and sizes are constants here.

use tungsten::core::ui::{UiThemeRegistry, UiTrack, UiTree, WidgetId};
use tungsten::core::{Easing, World};
use tungsten::{Transition, TransitionEffect};

/// The `ui_themes` ID the shell applies.
pub const THEME_ID: &str = "shell";
/// Each way of a screen change's fade, in seconds.
pub const FADE_SECS: f32 = 0.08;
/// How long a new screen slides into place, in real seconds.
pub const SLIDE_SECS: f32 = 0.12;
/// How far below its place a new screen starts, in UI units.
pub const SLIDE_UNITS: f32 = 8.0;

/// Inside a screen, all round, in UI units.
pub const PADDING: f32 = 16.0;
/// Between two rows.
pub const GAP: f32 = 8.0;
/// Between a screen's edge and the HUD.
pub const MARGIN: f32 = 16.0;
/// A button's padding: horizontal, vertical.
pub const BUTTON_PADDING: [f32; 2] = [14.0, 5.0];
/// A menu's width, and a wide screen's.
pub const MENU_WIDTH: f32 = 260.0;
pub const WIDE_WIDTH: f32 = 520.0;
/// A setting row's caption column and control column.
pub const CAPTION_WIDTH: f32 = 150.0;
pub const CONTROL_WIDTH: f32 = 170.0;
/// A heading's size and line height; body text is the theme's 16 on 22.
pub const HEADING_SIZE: f32 = 26.0;
pub const HEADING_LINE: f32 = 32.0;
/// The title's size and line height.
pub const TITLE_SIZE: f32 = 32.0;
pub const TITLE_LINE: f32 = 40.0;

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

/// A fade to black and back, [`FADE_SECS`] each way: a menu's push and pop.
#[must_use]
pub fn fade() -> Transition {
    Transition::new(
        TransitionEffect::Fade {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        FADE_SECS,
    )
}

/// Slides `root` up [`SLIDE_UNITS`] into place on the real clock, with no
/// overshoot: a new screen's entrance.
pub fn slide_in(tree: &mut UiTree, root: WidgetId) {
    tree.animate(
        root,
        UiTrack::translation([0.0, SLIDE_UNITS].into(), [0.0, 0.0].into(), SLIDE_SECS)
            .with_easing(Easing::CubicOut),
    );
}
