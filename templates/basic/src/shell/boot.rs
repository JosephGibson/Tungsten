//! The boot splash (`D-265`): a tungsten atom builds while the manifest
//! loads, for the `boot` group's `hold` at least. Six dotted orbits draw on
//! round the element's tile, typed in the engine's mono, and the 74
//! electrons land shell by shell, K first, as the load runs: magenta on the
//! K, M and O shells and white on the rest, each with a comet tail and a
//! soft glow, a magenta one round a white-hot core. The corners read the
//! element, the atom's state and its electrons, the configuration lighting
//! a character at a time and the load's units over a hairline. When the
//! last electron lands, a wave of light runs out through the rings and the
//! state turns STABLE.
//!
//! The atom builds over [`BUILD_SHARE`] of the hold at least, so a fast
//! load still shows it; the text fades in over [`FADE_SECS`], and
//! everything fades out over the hold's last once the load is done, while
//! a load longer than the hold ends at full opacity. The splash is laid
//! out on [`FRAME`] scaled to fit the window. Everything but the text is
//! drawn from the group's logo, the white disc `tools/boot.py` draws: a
//! glow or an electron from the whole disc, a rect from its solid middle.
//! Sprite tints are linear and blend in linear light, so the colours here,
//! written in sRGB, are decoded, and a glow's layers cover as much as reads
//! the same. `game::register` sets [`view`] as the app's boot view.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use tungsten::core::World;
use tungsten::render::{SpriteBatch, SpriteInstance, TextSection};
use tungsten::{BootProgress, BootView, ENGINE_FONT_ID};

/// The seconds the text takes to fade in, and everything to fade out.
pub const FADE_SECS: f32 = 0.3;
/// The share of the hold the atom takes to build at least.
pub const BUILD_SHARE: f32 = 0.6;
/// Tungsten's electrons by shell, K to P.
pub const SHELLS: [usize; 6] = [2, 8, 18, 32, 12, 2];
/// The frame the splash is laid out on, in pixels, scaled to fit.
pub const FRAME: [f32; 2] = [1280.0, 720.0];

/// The engine font's faces; `engine_mono_semibold` is a reserved font ID.
const REGULAR: &str = ENGINE_FONT_ID;
const SEMIBOLD: &str = "engine_mono_semibold";
/// The disc's half size over the radius where its edge is half clear,
/// `tools/boot.py`'s 16 px over 14.
const DISC: f32 = 16.0 / 14.0;
const BLACK: [u8; 3] = [0; 3];
const WHITE: [u8; 3] = [255; 3];
const LIGHT: [u8; 3] = [0xB0; 3];
const GREY: [u8; 3] = [0x6C; 3];
const DIM: [u8; 3] = [0x36; 3];
const MAGENTA: [u8; 3] = [0xFF, 0x1F, 0xFF];
const PINK: [u8; 3] = [0xFF, 0x4F, 0xA3];
/// The element's tile, 26 cells by 12.
const TILE: [&str; 12] = [
    "┌────────────────────────┐",
    "│ 74              183.84 │",
    "│                        │",
    "│       ██╗    ██╗       │",
    "│       ██║    ██║       │",
    "│       ██║ █╗ ██║       │",
    "│       ██║███╗██║       │",
    "│       ╚███╔███╔╝       │",
    "│        ╚══╝╚══╝        │",
    "│                        │",
    "│        TUNGSTEN        │",
    "└────────────────────────┘",
];
/// The tile's styles: its frame, the frame's shadow, the symbol's blocks
/// and, for any other character, a pink label.
const CELLS: [(&str, [u8; 3], &str); 4] = [
    ("┌┐└┘─│", LIGHT, REGULAR),
    ("╗║╔╝╚═", GREY, REGULAR),
    ("█", WHITE, REGULAR),
    ("", PINK, SEMIBOLD),
];
/// The ground-state configuration and its runs, (first character, end,
/// first electron, end): each run lights as its electrons land.
const CONFIG: &str = "[Xe] 4f¹⁴ 5d⁴ 6s²";
const RUNS: [(usize, usize, usize, usize); 4] = [
    (0, 4, 0, 54),
    (5, 9, 54, 68),
    (10, 13, 68, 72),
    (14, 17, 72, 74),
];

/// A run of text: its content, sRGB colour and font.
type Run<'a> = (&'a str, [u8; 3], &'a str);

/// One boot frame's splash, in window pixels from the top-left.
#[derive(Debug, Clone, Default)]
pub struct Splash {
    /// Its shapes in drawing order, each on the disc's page.
    pub sprites: Vec<SpriteInstance>,
    /// The tile's rows and the readouts, over the shapes.
    pub text: Vec<TextSection>,
    /// The electrons landed, 0 to 74.
    pub electrons: usize,
    /// The fade out's opacity, 0 to 1.
    pub alpha: f32,
}

/// The splash on a `window` (pixels): `fraction` of the load's `total`
/// units done, `secs` real seconds since the first boot frame and the
/// group's `hold`.
#[must_use]
pub fn splash(window: [u32; 2], fraction: f32, total: usize, secs: f32, hold: f32) -> Splash {
    let [width, height] = window.map(|side| side as f32);
    let k = (width / FRAME[0]).min(height / FRAME[1]);
    let (build, fraction) = (BUILD_SHARE * hold, fraction.clamp(0.0, 1.0));
    let shown = fraction.min(if build > 0.0 { secs / build } else { 1.0 });
    let electrons = (shown * 74.0).round() as usize;
    let alpha = [((hold - secs) / FADE_SECS).clamp(0.0, 1.0), 1.0][usize::from(fraction < 1.0)];
    let centre = [width / 2.0, height / 2.0 - 8.0 * k];
    let mut scene = Scene {
        out: Splash {
            electrons,
            alpha,
            ..Splash::default()
        },
        size: [width, height],
        k,
        centre,
        secs,
        since: [f32::NEG_INFINITY, secs - build][usize::from(electrons == 74)],
        grid: grid(centre, k),
    };
    scene.rings();
    scene.electrons(build);
    scene.tile();
    scene.readouts(shown, total);
    scene.out
}

/// The boot view: the splash over the clear colour, its shapes drawn from
/// the group's logo, and only its text without one.
#[must_use]
pub fn view(_world: &World, p: &BootProgress<'_>) -> BootView {
    let splash = splash(p.window, p.fraction(), p.total, p.secs, p.hold);
    let sprites = p.assets.logo.as_ref().map(|disc| {
        let mut batch = SpriteBatch::new(disc.texture, disc.filter);
        batch.instances = splash.sprites;
        batch
    });
    BootView {
        sprites: sprites.into_iter().collect(),
        text: splash.text,
    }
}

/// The tile's left, top, advance and line, on whole pixels: a whole
/// advance, so the frame's lines meet without a seam across, and a line,
/// the mono's 1.32 em over its 0.6 em advance, half a pixel short and
/// rounded down, so each row overlaps the next by at least the part pixel
/// a glyph's edge leaves, however the rows round, and blocks and frame
/// show no seam down either.
fn grid([x, y]: [f32; 2], k: f32) -> [f32; 4] {
    let advance = (9.0 * k).round().max(1.0);
    let line = (2.2 * advance - 0.5).floor();
    [
        (x - 13.0 * advance).round(),
        (y - 6.0 * line).round(),
        advance,
        line,
    ]
}

/// A frame's splash as it is drawn.
struct Scene {
    out: Splash,
    size: [f32; 2],
    /// The frame's scale on the window.
    k: f32,
    /// The atom's middle.
    centre: [f32; 2],
    secs: f32,
    /// Seconds since the build's end once the atom is whole, and minus
    /// infinity before, so the wave and the flash wait for it.
    since: f32,
    /// The tile's [`grid`].
    grid: [f32; 4],
}

impl Scene {
    /// The light wave on shell `shell`.
    fn wave(&self, shell: usize) -> f32 {
        gauss(self.since - 0.07 * shell as f32, 0.06)
    }

    fn radii(&self, shell: usize) -> [f32; 2] {
        let ry = (148.0 + 32.0 * shell as f32) * self.k;
        [1.75 * ry, ry]
    }

    fn on_ring(&self, shell: usize, angle: f32) -> [f32; 2] {
        let ([rx, ry], [x, y]) = (self.radii(shell), self.centre);
        [x + rx * angle.cos(), y + ry * angle.sin()]
    }

    /// Whether `at` is on the tile or its margin, where nothing else draws.
    fn covers(&self, [x, y]: [f32; 2]) -> bool {
        let ([left, top, advance, line], m) = (self.grid, 6.0 * self.k);
        (left - m..=left + 26.0 * advance + m).contains(&x)
            && (top - m..=top + 12.0 * line + m).contains(&y)
    }

    /// The orbits: a dot every 4 frame pixels, shaded lighter below, drawn
    /// on clockwise from twelve o'clock, inner rings first.
    fn rings(&mut self) {
        for shell in 0..SHELLS.len() {
            let [rx, ry] = self.radii(shell);
            let perimeter = PI * (3.0 * (rx + ry) - ((3.0 * rx + ry) * (rx + 3.0 * ry)).sqrt());
            let dots = (perimeter / (4.0 * self.k)) as usize;
            let drawn = ease((self.secs - 0.05 * shell as f32) / 0.28);
            let base = mix([0x3C; 3], [0x2A; 3], shell as f32 / 5.0);
            let wave = self.wave(shell);
            for i in 0..dots {
                let angle = TAU * i as f32 / dots as f32;
                let at = self.on_ring(shell, angle);
                if (angle + FRAC_PI_2) % TAU / TAU >= drawn || self.covers(at) {
                    continue;
                }
                let shade = mix(BLACK, base, 0.78 + 0.22 * angle.sin());
                self.square(at, 2.0 * self.k, mix(shade, LIGHT, 0.85 * wave));
            }
        }
    }

    /// The landed electrons, each on its time in an even build; one that a
    /// slower load lands late arrives whole.
    fn electrons(&mut self, build: f32) {
        let spots = SHELLS.iter().enumerate();
        let spots = spots.flat_map(|(shell, &n)| (0..n).map(move |slot| (shell, slot, n)));
        for (i, (shell, slot, n)) in spots.take(self.out.electrons).enumerate() {
            let age = self.secs - (i as f32 + 0.5) / 74.0 * build;
            let magenta = shell % 2 == 0;
            let (color, tail) =
                [(WHITE, [0x7E; 3]), (MAGENTA, [0x7A, 0x10, 0x7A])][usize::from(magenta)];
            let turn = 1.5 / (1.0 + 0.55 * shell as f32) * [-1.0, 1.0][usize::from(magenta)];
            let angle = TAU * slot as f32 / n as f32 + turn * self.secs + 0.7 * shell as f32;
            // A comet tail, longer on the fast inner shells, grown as it lands.
            let length = (age / 0.025).clamp(0.0, 10.0) as usize;
            let step = turn.signum() * 0.18 / (shell as f32 + 3.0);
            for j in (1..=length).rev() {
                let at = self.on_ring(shell, angle - step * j as f32);
                let side = 3.2 * self.k * (1.0 - j as f32 / (length + 2) as f32);
                let fade = (j as f32 / (length + 1) as f32).powf(1.3);
                if !self.covers(at) {
                    self.square(at, side, mix(tail, BLACK, fade));
                }
            }
            let at = self.on_ring(shell, angle);
            if self.covers(at) {
                continue;
            }
            // It grows in under a bloom, and flares as the wave passes.
            let (grow, bloom, wave) = (ease(age / 0.12), 1.0 - ease(age / 0.45), self.wave(shell));
            let radius = 15.0 * self.k * (0.6 + 0.4 * grow) * (1.0 + 0.35 * bloom + 0.3 * wave);
            let peak = [0.315, 0.42][usize::from(magenta)] * (1.0 + 2.4 * bloom) * (1.0 + wave);
            self.glow(at, [radius; 2], peak * grow, color, 6);
            // A new one flashes white and settles; a magenta one keeps a
            // white-hot core, 0.75 over it as sRGB blends.
            let core = 4.2 * self.k * pop(age);
            self.disc(at, [core; 2], mix(WHITE, color, ease(age / 0.3)), grow);
            if magenta {
                self.disc(at, [0.42 * core; 2], WHITE, 0.53 * grow);
            }
        }
    }

    /// The tile: a black card inside its frame's lines, hiding the glows
    /// behind it, then a row of text per row and style, the frame flashing
    /// white as the atom turns stable.
    fn tile(&mut self) {
        let fade = ease((self.secs - 0.05) / FADE_SECS);
        let flash = gauss(self.since - 0.02, 0.05);
        let [left, top, advance, line] = self.grid;
        let size = advance / 0.6;
        let card = [left + advance / 2.0, top + (line / 2.0).floor()];
        self.rect(card, [25.0 * advance, 11.0 * line + 1.0], BLACK, 1.0);
        let class = |c: char| CELLS.iter().position(|s| s.0.contains(c)).unwrap_or(3);
        for (row, cells) in TILE.iter().enumerate() {
            let baseline = [left, top + row as f32 * line + 1.02 * size];
            for (style, &(_, color, font)) in CELLS.iter().enumerate() {
                let content = cells.replace(|c: char| class(c) != style, " ");
                let color = mix(color, WHITE, [flash, 0.0][usize::from(style > 0)]);
                self.line(
                    baseline,
                    false,
                    &[(content.trim_end(), color, font)],
                    size,
                    fade,
                );
            }
        }
    }

    /// The corners and the hairline: the element, the atom's state and its
    /// electrons at the top, the configuration lit to the electrons landed
    /// and the load's units at the bottom, over a hairline whose magenta
    /// tip glows at the load's front.
    fn readouts(&mut self, shown: f32, total: usize) {
        let ([width, height], k, electrons) = (self.size, self.k, self.out.electrons);
        let fade = ease((self.secs - 0.1) / FADE_SECS);
        let (size, margin) = (14.0 * k, 31.5 * k);
        let (top, right, bottom) = (margin + 0.75 * size, width - margin, height - margin);
        let title = [("W · 74 · TUNGSTEN", WHITE, SEMIBOLD)];
        self.line([margin, top], false, &title, size, fade);
        let state = if electrons == 74 {
            // A neon glow: rings of faint copies, brightest as it lands.
            let since = self.since.max(0.0);
            let strength = fade * (1.0 + 1.5 * (-since / 0.25).exp());
            for (radius, copies, opacity) in [(1.5, 8, 0.10), (3.0, 12, 0.05), (5.0, 16, 0.025)] {
                for i in 0..copies {
                    let turn = TAU * i as f32 / copies as f32 + radius;
                    let at = [
                        right - 3.6 * size + radius * k * turn.cos(),
                        top + radius * k * turn.sin(),
                    ];
                    let alpha = (opacity * strength).min(1.0).powf(1.5);
                    self.line(at, false, &[("STABLE", MAGENTA, SEMIBOLD)], size, alpha);
                }
            }
            ("STABLE", mix(WHITE, MAGENTA, ease(since / 0.3)))
        } else {
            ("BUILDING", WHITE)
        };
        let atom = [("ATOM  ", GREY, REGULAR), (state.0, state.1, SEMIBOLD)];
        self.line([right, top], true, &atom, size, fade);
        let count = format!("{electrons:02}/74");
        let count: [Run; 2] = [("ELECTRONS  ", GREY, REGULAR), (&count, WHITE, SEMIBOLD)];
        self.line([right, top + 22.75 * k], true, &count, size, fade);
        let next = RUNS.iter().find_map(|&(first, end, from, to)| {
            let lit = electrons.saturating_sub(from) * (end - first) / (to - from);
            (first + lit < end).then_some(first + lit)
        });
        let cut = |n: usize| {
            CONFIG
                .char_indices()
                .nth(n)
                .map_or(CONFIG.len(), |(i, _)| i)
        };
        let [a, b] = next.map_or([CONFIG.len(); 2], |n| [cut(n), cut(n + 1)]);
        let config: [Run; 3] = [
            (&CONFIG[..a], WHITE, REGULAR),
            (&CONFIG[a..b], MAGENTA, SEMIBOLD),
            (&CONFIG[b..], DIM, REGULAR),
        ];
        self.line([margin, bottom], false, &config, size, fade);
        let (units, percent) = (
            (shown * total as f32).round() as usize,
            (shown * 100.0).round(),
        );
        let units = format!("{units:04}/{total:04}  {percent:03}%");
        self.line(
            [right, bottom],
            true,
            &[(&units, LIGHT, REGULAR)],
            size,
            fade,
        );
        let (y, head) = (height - 54.25 * k, margin + (right - margin) * shown);
        self.rect([margin, y], [right - margin, 2.0 * k], [0x1E; 3], fade);
        self.rect([margin, y], [head - margin, 2.0 * k], WHITE, fade);
        if shown < 1.0 {
            let tip = [head - 4.0 * k, y + k];
            self.glow(tip, [20.0 * k, 9.0 * k], 0.5 * fade, MAGENTA, 5);
            self.rect([head - 8.0 * k, y - k], [8.0 * k, 4.0 * k], MAGENTA, fade);
        }
    }

    /// Runs of text on the mono's advance from `x`, or ending at it when
    /// `right`, on the baseline `y`.
    fn line(&mut self, [x, y]: [f32; 2], right: bool, runs: &[Run<'_>], size: f32, alpha: f32) {
        let advance = |text: &str| text.chars().count() as f32 * 0.6 * size;
        let width: f32 = runs.iter().map(|run| advance(run.0)).sum();
        let mut x = x - if right { width } else { 0.0 };
        for &(content, color, font) in runs {
            if !content.trim().is_empty() {
                let color = rgba(color, alpha * self.out.alpha);
                self.out
                    .text
                    .push(section(content, font, size, [x, y - 1.02 * size], color));
            }
            x += advance(content);
        }
    }

    /// A rect from the disc's solid middle.
    fn rect(&mut self, position: [f32; 2], size: [f32; 2], color: [u8; 3], alpha: f32) {
        let color = tint(color, alpha * self.out.alpha);
        let mut sprite = SpriteInstance::whole(position, size, 0.0, color);
        sprite.uv_min = [0.5; 2];
        sprite.uv_size = [0.0; 2];
        self.out.sprites.push(sprite);
    }

    fn square(&mut self, [x, y]: [f32; 2], side: f32, color: [u8; 3]) {
        self.rect([x - side / 2.0, y - side / 2.0], [side; 2], color, 1.0);
    }

    /// The whole disc, its edge on `radii` round `at`.
    fn disc(&mut self, [x, y]: [f32; 2], radii: [f32; 2], color: [u8; 3], alpha: f32) {
        let [w, h] = radii.map(|radius| radius * DISC);
        let color = tint(color, alpha * self.out.alpha);
        let sprite = SpriteInstance::whole([x - w, y - h], [2.0 * w, 2.0 * h], 0.0, color);
        self.out.sprites.push(sprite);
    }

    /// A soft glow, `layers` discs whose stack falls off from `peak` at the
    /// middle as a Gaussian reads in sRGB: each layer's coverage is the
    /// sRGB reading's to the power 2.2, since sprites blend in linear light.
    fn glow(&mut self, at: [f32; 2], radii: [f32; 2], peak: f32, color: [u8; 3], layers: usize) {
        let (peak, mut below) = (peak.min(0.95), 0.0);
        for i in 0..layers {
            let share = (layers - i) as f32 / layers as f32;
            let cover = (peak * gauss(share - 0.5 / layers as f32, 0.42)).powf(2.2);
            let opacity = 1.0 - (1.0 - cover) / (1.0 - below);
            self.disc(at, radii.map(|radius| radius * share), color, opacity);
            below = cover;
        }
    }
}

/// A cubic ease out of `x`, clamped to 0 to 1.
fn ease(x: f32) -> f32 {
    1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3)
}

fn gauss(x: f32, sigma: f32) -> f32 {
    (-x * x / (2.0 * sigma * sigma)).exp()
}

/// An arrival's size: up to 1.35 in 0.12 s, back to 1 by 0.3 s.
fn pop(age: f32) -> f32 {
    1.35 * ease(age / 0.12) - 0.35 * ease((age - 0.12) / 0.18)
}

/// `a` to `b` by `t`, in sRGB.
fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|i| {
        (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t).round() as u8
    })
}

/// A text colour: text takes sRGB colours.
fn rgba([r, g, b]: [u8; 3], alpha: f32) -> [u8; 4] {
    [r, g, b, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8]
}

/// A sprite's tint: the sprite shader multiplies its linear texels by the
/// tint as it is, so an sRGB colour is decoded first; its alpha rounds up,
/// so a glow's faint outer layers still draw.
fn tint(color: [u8; 3], alpha: f32) -> [u8; 4] {
    let decode = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = color.map(|c| (decode(c) * 255.0).round() as u8);
    [r, g, b, (alpha.clamp(0.0, 1.0) * 255.0).ceil() as u8]
}

/// A line of text at `position`, its line 1.32 em, the mono's ascent and
/// descent, so its baseline sits 1.02 em down.
fn section(
    content: &str,
    font: &str,
    size: f32,
    position: [f32; 2],
    color: [u8; 4],
) -> TextSection {
    TextSection {
        content: content.to_string(),
        font_id: font.to_string(),
        font_size: size,
        line_height: 1.32 * size,
        color,
        position,
        ..TextSection::default()
    }
}
