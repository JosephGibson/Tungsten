//! The boot splash (redo 7.11): while the manifest loads, and for its
//! `boot` group's `hold` at least, the group's logo at half its pixel size
//! × the boot's scale, centred [`LOGO_RISE`] units above the window's
//! middle, and its bar [`BAR_GAP`] units under the logo, [`BAR_HEIGHT`]
//! units tall and as wide as the logo × the load's progress. Both fade in
//! over the first [`FADE_SECS`] and out over the hold's last once the load
//! is done; a load longer than the hold ends at full opacity. The scale is
//! the UI's, which the shell's plugin sets before the boot. `game::register`
//! sets [`view`] as the app's boot view.

use tungsten::core::World;
use tungsten::render::{SpriteBatch, SpriteInstance};
use tungsten::{BootProgress, BootView};

/// The seconds each fade takes.
pub const FADE_SECS: f32 = 0.3;
/// How far the logo's centre sits above the window's middle, in units.
pub const LOGO_RISE: f32 = 24.0;
/// The gap from the logo's foot to the bar, and the bar's height, in
/// units.
pub const BAR_GAP: f32 = 20.0;
pub const BAR_HEIGHT: f32 = 4.0;

/// One boot frame's splash, in window pixels from the top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Splash {
    /// The logo's top-left corner and size.
    pub logo: [f32; 4],
    /// The bar's top-left corner and size, its width the progress's share
    /// of the logo's.
    pub bar: [f32; 4],
    /// Both sprites' opacity, 0 to 1.
    pub alpha: f32,
}

/// The splash on a `window` (pixels) at the boot's `scale`, for a logo
/// `logo` pixels in size: `fraction` of the load done, `secs` real seconds
/// since the first boot frame and the group's `hold`.
#[must_use]
pub fn layout(
    window: [u32; 2],
    scale: f32,
    logo: [u32; 2],
    fraction: f32,
    secs: f32,
    hold: f32,
) -> Splash {
    let [width, height] = window.map(|side| side as f32);
    let [w, h] = logo.map(|side| side as f32 * 0.5 * scale);
    let (x, y) = (
        (width - w) * 0.5,
        height * 0.5 - LOGO_RISE * scale - h * 0.5,
    );
    let fraction = fraction.clamp(0.0, 1.0);
    let bar = [x, y + h + BAR_GAP * scale, w * fraction, BAR_HEIGHT * scale];
    let fade_in = (secs / FADE_SECS).clamp(0.0, 1.0);
    let fade_out = if fraction < 1.0 {
        1.0
    } else {
        ((hold - secs) / FADE_SECS).clamp(0.0, 1.0)
    };
    Splash {
        logo: [x, y, w, h],
        bar,
        alpha: fade_in.min(fade_out),
    }
}

/// The boot view: the splash over the clear colour; nothing without the
/// group's logo.
#[must_use]
pub fn view(_world: &World, progress: &BootProgress<'_>) -> BootView {
    let mut view = BootView::default();
    let Some(logo) = &progress.assets.logo else {
        return view;
    };
    let splash = layout(
        progress.window,
        progress.scale,
        logo.size,
        progress.fraction(),
        progress.secs,
        progress.hold,
    );
    let tint = [255, 255, 255, (splash.alpha * 255.0).round() as u8];
    let bar = progress.assets.bar.as_ref().map(|bar| (bar, splash.bar));
    for (sprite, [x, y, w, h]) in [(logo, splash.logo)].into_iter().chain(bar) {
        let mut batch = SpriteBatch::new(sprite.texture, sprite.filter);
        let instance = SpriteInstance::whole([x, y], [w, h], 0.0, tint);
        batch.instances.push(instance);
        view.sprites.push(batch);
    }
    view
}
