//! The boot splash's layout: the logo at half its size by the boot's scale,
//! the bar under it following the load, and the fades.

use tungsten::core::{ScaleFactor, UserFolder};
use tungsten_template_basic::shell::boot::{self, BAR_GAP, BAR_HEIGHT, FADE_SECS, LOGO_RISE};

/// The logo's size in pixels, `tools/logo.py`'s.
const LOGO: [u32; 2] = [768, 256];

/// The scale the boot reads on `window` at OS scale `os`: the UI's, which
/// the template's app sets before the boot, from its `tungsten.json` and the
/// shell's plugin, as `App::new` and the window do.
fn boot_scale(window: [u32; 2], os: f32) -> f32 {
    let mut app = super::app(UserFolder::Off);
    let scale = app.world_mut().resource_mut::<ScaleFactor>();
    scale.set_os(os);
    scale.set_window_size(window[0], window[1]);
    scale.get()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn the_logo_is_centred_at_half_size_by_the_boot_scale() {
    // The default UI scale, 0.75, on a 1280 × 720 window at OS scale 1.
    assert!(near(boot_scale([1280, 720], 1.0), 0.75));
    for (window, os, size) in [
        ([1280, 720], 1.0, [288.0, 96.0]),
        ([1280, 720], 1.75, [504.0, 168.0]),
        ([3840, 2160], 2.0, [864.0, 288.0]),
    ] {
        let scale = boot_scale(window, os);
        let [x, y, w, h] = boot::layout(window, scale, LOGO, 0.5, 1.0, 1.5).logo;
        let what = format!("{window:?} at OS scale {os}: {w} × {h} at ({x}, {y})");
        assert!(near(w, size[0]) && near(h, size[1]), "{what}");
        // Centred across, its middle LOGO_RISE units above the window's.
        let middle = window.map(|side| side as f32 / 2.0);
        assert!(near(x + w / 2.0, middle[0]), "{what}");
        assert!(near(middle[1] - (y + h / 2.0), LOGO_RISE * scale), "{what}");
    }
    // 1:1 at a scale of 2.
    let [.., w, h] = boot::layout([1920, 1080], 2.0, LOGO, 1.0, 0.0, 0.0).logo;
    assert_eq!([w, h], [768.0, 256.0]);
}

#[test]
fn the_bar_follows_the_progress() {
    let scale = boot_scale([1280, 720], 1.0);
    let at = |fraction| boot::layout([1280, 720], scale, LOGO, fraction, 1.0, 1.5);
    let [left, top, _, height] = at(0.0).logo;
    for (fraction, width) in [(0.0, 0.0), (0.5, 144.0), (1.0, 288.0), (1.5, 288.0)] {
        let [x, y, w, h] = at(fraction).bar;
        assert!(near(x, left), "under the logo's left edge: {x}");
        assert!(near(y, top + height + BAR_GAP * scale), "{y}");
        assert!(near(w, width), "{fraction}: {w}");
        assert!(near(h, BAR_HEIGHT * scale), "{h}");
    }
}

#[test]
fn the_splash_fades_in_and_out() {
    let hold = 1.5;
    let alpha = |fraction, secs| boot::layout([1280, 720], 0.75, LOGO, fraction, secs, hold).alpha;
    // In over the first 0.3 s.
    assert_eq!(alpha(0.0, 0.0), 0.0);
    assert!(near(alpha(0.2, FADE_SECS / 2.0), 0.5));
    assert_eq!(alpha(0.4, FADE_SECS), 1.0);
    // Whole while the load runs, however long: a load past the hold ends
    // at full opacity, cutting to the title.
    assert_eq!(alpha(0.9, 1.4), 1.0);
    assert_eq!(alpha(0.99, 3.0), 1.0);
    // Out over the hold's last 0.3 s once the load is done.
    assert_eq!(alpha(1.0, 1.0), 1.0);
    assert!(near(alpha(1.0, hold - FADE_SECS / 2.0), 0.5));
    assert_eq!(alpha(1.0, hold), 0.0);
}
