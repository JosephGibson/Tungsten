//! The boot splash: the atom fits the window and builds with the load,
//! paced by the hold, the readouts count it, the tile's rows sit on whole
//! pixels, and the splash fades in and out.

use tungsten_template_basic::shell::boot::{self, FADE_SECS, Splash};

/// The manifest's hold.
const HOLD: f32 = 2.0;

fn at(window: [u32; 2], fraction: f32, secs: f32) -> Splash {
    boot::splash(window, fraction, 60, secs, HOLD)
}

fn texts(splash: &Splash) -> Vec<&str> {
    splash
        .text
        .iter()
        .map(|section| section.content.as_str())
        .collect()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn the_atom_fits_any_window() {
    let windows = [
        [1280, 720],
        [1920, 1080],
        [3840, 2160],
        [1024, 768],
        [720, 1280],
        [2560, 1080],
    ];
    for window in windows {
        let [width, height] = window.map(|side| side as f32);
        // Mid-build, as the wave runs and in the hold.
        for secs in [0.6, 1.25, 1.9] {
            for sprite in &at(window, 1.0, secs).sprites {
                let ([x, y], [w, h]) = (sprite.position, sprite.size);
                let inside = x >= 0.0 && y >= 0.0 && x + w <= width && y + h <= height;
                assert!(inside, "{window:?} at {secs} s: {sprite:?}");
            }
        }
    }
}

#[test]
fn the_atom_builds_with_the_load_over_the_hold() {
    let electrons = |fraction, secs| at([1280, 720], fraction, secs).electrons;
    // A fast load: the atom takes 60 % of the hold.
    assert_eq!(electrons(1.0, 0.0), 0);
    assert_eq!(electrons(1.0, 0.6), 37);
    assert_eq!(electrons(1.0, 1.2), 74);
    // A slow one: the load's own share.
    assert_eq!(electrons(0.25, 1.5), 19);
    // A script run holds nothing, so the atom shows the load as it is.
    assert_eq!(boot::splash([1280, 720], 0.5, 60, 0.0, 0.0).electrons, 37);
}

#[test]
fn the_readouts_count_the_load() {
    let building = at([1280, 720], 0.5, 5.0);
    let texts_now = texts(&building);
    // [Xe] lights over the first 54 electrons, so 37 light "[X" and "e" is next.
    for want in [
        "W · 74 · TUNGSTEN",
        "BUILDING",
        "37/74",
        "[X",
        "e",
        "0030/0060  050%",
    ] {
        assert!(texts_now.contains(&want), "{want:?} in {texts_now:?}");
    }
    let done = at([1280, 720], 1.0, 1.5);
    let texts_done = texts(&done);
    for want in ["STABLE", "74/74", "[Xe] 4f¹⁴ 5d⁴ 6s²", "0060/0060  100%"] {
        assert!(texts_done.contains(&want), "{want:?} in {texts_done:?}");
    }
}

#[test]
fn the_tiles_rows_sit_on_whole_pixels() {
    for window in [
        [1280, 720],
        [1366, 768],
        [1600, 900],
        [1920, 1080],
        [2560, 1440],
        [3840, 2160],
    ] {
        let splash = at(window, 1.0, 1.0);
        let frame: Vec<_> = splash
            .text
            .iter()
            .filter(|section| section.content.contains(['┌', '│', '└']))
            .collect();
        assert_eq!(frame.len(), 12, "{window:?}: a frame row per tile row");
        // Whole cells, so the frame's lines meet without a seam, and whole
        // rows a whole line apart, each overlapping the next by half a
        // pixel at least, so no block or line seams down at the row's edge.
        let whole = |v: f32| near(v, v.round());
        let line = frame[1].position[1] - frame[0].position[1];
        let overlap = 1.32 * frame[0].font_size - line;
        assert!(
            whole(line) && overlap >= 0.5,
            "{window:?}: {line}, {overlap}"
        );
        for (row, section) in frame.iter().enumerate() {
            let [x, y] = section.position;
            assert!(
                whole(x) && whole(y) && whole(0.6 * section.font_size),
                "{window:?}: {section:?}"
            );
            assert!(
                near(y - frame[0].position[1], row as f32 * line),
                "{window:?}: row {row}"
            );
        }
    }
}

#[test]
fn the_splash_fades_in_and_out() {
    let alpha = |fraction, secs| at([1280, 720], fraction, secs).alpha;
    // Whole while the load runs, however long: a load past the hold ends at
    // full opacity, cutting to the title.
    assert_eq!(alpha(0.9, 1.4), 1.0);
    assert_eq!(alpha(0.99, 3.0), 1.0);
    // Out over the hold's last 0.3 s once the load is done.
    assert_eq!(alpha(1.0, 1.0), 1.0);
    assert!(near(alpha(1.0, HOLD - FADE_SECS / 2.0), 0.5));
    assert_eq!(alpha(1.0, HOLD), 0.0);
    // The text fades in after the orbits start: none at the first frame,
    // all by 0.4 s.
    assert!(
        at([1280, 720], 0.0, 0.0)
            .text
            .iter()
            .all(|section| section.color[3] == 0)
    );
    let title = at([1280, 720], 0.5, 0.5);
    let title = title
        .text
        .iter()
        .find(|section| section.content == "W · 74 · TUNGSTEN");
    assert_eq!(title.map(|section| section.color[3]), Some(255));
}
