//! The shell's motion and sound in one place: the theme asset's ID, the
//! fades and slides each screen change takes, and the manifest IDs of the
//! shell's music. Colours, borders, radii, fonts, spacing, sizes, text
//! styles and the menu sounds live in `assets/ui/themes/shell.json`,
//! which `tungsten.json`'s `ui.theme` applies and which hot-reloads; a
//! screen reads its spacing and sizes as it builds
//! (`tungsten::core::ui::widgets::size`).

use tungsten::core::Easing;
use tungsten::core::ui::{UiTrack, UiTree, WidgetId};

/// The `ui_themes` ID `tungsten.json`'s `ui.theme` names.
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
/// The title's song, by manifest ID, and its fade in and out.
pub const TITLE_MUSIC: &str = "music_title";
pub const TITLE_FADE: f32 = 1.0;
/// Pause's loop, by manifest ID, and its fade in and out: each screen's
/// song plays while it is the top one naming music (`GameState::music`).
pub const PAUSE_MUSIC: &str = "music_pause";
pub const PAUSE_FADE: f32 = 0.3;
/// Game over's sting and music, by manifest ID, and the music's fade in and
/// out.
pub const GAME_OVER_SOUNDS: [&str; 2] = ["game_over", "music_over"];
pub const GAME_OVER_FADE: f32 = 1.2;
/// Game over's entrance, in real seconds: its band fades in while the
/// world pixelates to [`DIED_BLOCK_PX`] and dims to [`DIED_DIM`] under a
/// near-black crimson; then its buttons fade in over [`DIED_BUTTONS_SECS`].
pub const DIED_SECS: f32 = 1.6;
pub const DIED_BUTTONS_SECS: f32 = 0.3;
pub const DIED_BLOCK_PX: f32 = 48.0;
pub const DIED_DIM: f32 = 0.8;
pub const DIED_FADE: [f32; 4] = [0.03, 0.002, 0.008, 1.0];
/// Game over's band across the screen, and its heading's crimson, 3:1 or
/// more against the band in linear light, and letter spacing, in em.
pub const DIED_BAND: [u8; 4] = [0, 0, 0, 200];
pub const DIED_RED: [u8; 4] = [0xbe, 0x14, 0x20, 0xff];
pub const DIED_SPACING: f32 = 0.2;

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

/// Fades `id` in over `secs` on the real clock, easing out: game over's band
/// over [`DIED_SECS`], then its buttons over [`DIED_BUTTONS_SECS`].
pub fn fade_in(tree: &mut UiTree, id: WidgetId, secs: f32) {
    let track = UiTrack::opacity(0.0, 1.0, secs).with_easing(Easing::QuadOut);
    tree.animate(id, track);
}

/// Fades `root` out and slides it down [`SLIDE_UNITS`] over [`POP_SECS`]:
/// a closing screen's exit.
pub fn slide_out(tree: &mut UiTree, root: WidgetId) {
    tree.animate(root, UiTrack::opacity(1.0, 0.0, POP_SECS));
    let to = [0.0, SLIDE_UNITS].into();
    tree.animate(root, UiTrack::translation([0.0, 0.0].into(), to, POP_SECS));
}
