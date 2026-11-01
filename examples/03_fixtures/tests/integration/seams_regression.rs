//! Opt-in GPU regression for the drawing seams of redo 7.9, on the fixtures
//! crate's `seams` scene: each piece checked by its pixels, a mean or every
//! pixel of a region against a bound, never a golden.
//!
//! Gate: `TUNGSTEN_VISUAL_REGRESSION=1` (needs GPU/display). Capture: 8 smoke
//! frames, frame 5 at 1280x720, once for the module; the capture,
//! `tungsten-seams-regression.png` in the temporary folder, is the drawing
//! sheet. The scene draws world pixels one to one, so a piece's quad spans
//! `PIECE` pixels about its centre.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use example_03_fixtures::seams::{
    BEAM_FROM, BEAM_TO, FLASH_AT, FLAT_AT, FLAT_SIZE, GLASS_AT, GLOW_AT, GLOW_SIDE, LIT_AT,
    OVER_STRIP_AT, PIECE, SEGMENT_FROM, SEGMENT_TO, SEGMENT_WIDTH, STRIP_AT, TWIN_AT,
    UNDER_STRIP_AT, UNLIT_AT,
};
use glam::Vec2;

/// The scene loads its manifests relative to the workspace root.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gated() -> bool {
    std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_ok()
}

/// The scene's frame 5, captured on first use.
fn capture() -> &'static Rgba {
    static CAPTURE: OnceLock<Rgba> = OnceLock::new();
    CAPTURE.get_or_init(|| {
        let path = std::env::temp_dir().join("tungsten-seams-regression.png");
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_example-03-fixtures"))
            .current_dir(workspace_root())
            .env("TUNGSTEN_FIXTURE", "seams")
            .env_remove("TUNGSTEN_CAPTURE_DIRECT")
            .env("TUNGSTEN_SMOKE_FRAMES", "8")
            .env("TUNGSTEN_CAPTURE_FRAME", "5")
            .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
            .env("TUNGSTEN_CAPTURE_PATH", &path)
            .status()
            .expect("run example-03-fixtures");
        assert!(
            status.success(),
            "example-03-fixtures exited with {status:?}"
        );
        let image = Rgba::read(&path);
        assert_eq!((image.width, image.height), (1280, 720));
        image
    })
}

/// The pixels of the piece at `centre` over its quad's columns `xs` and
/// rows `ys`, counted from its top-left corner.
fn region(
    image: &Rgba,
    centre: Vec2,
    xs: Range<usize>,
    ys: Range<usize>,
) -> impl Iterator<Item = [u8; 4]> + '_ {
    let corner = centre - Vec2::splat(PIECE * 0.5);
    let (left, top) = (corner.x as usize, corner.y as usize);
    ys.flat_map(move |y| xs.clone().map(move |x| image.at(left + x, top + y)))
}

/// The pixels over columns `xs` and rows `ys` from `corner`, in capture
/// pixels.
fn area(
    image: &Rgba,
    corner: Vec2,
    xs: Range<usize>,
    ys: Range<usize>,
) -> impl Iterator<Item = [u8; 4]> + '_ {
    let (left, top) = (corner.x as usize, corner.y as usize);
    ys.flat_map(move |y| xs.clone().map(move |x| image.at(left + x, top + y)))
}

/// The mean of red, green and blue over a region.
fn brightness(image: &Rgba, centre: Vec2, xs: Range<usize>, ys: Range<usize>) -> f32 {
    let (sum, count) = region(image, centre, xs, ys).fold((0u64, 0u64), |(sum, count), p| {
        (
            sum + u64::from(p[0]) + u64::from(p[1]) + u64::from(p[2]),
            count + 3,
        )
    });
    sum as f32 / count as f32
}

#[test]
fn a_flipped_lit_sprite_keeps_its_lit_side() {
    if !gated() {
        return;
    }
    let image = capture();
    // Away from the red edge on either side, each half's mean: the light
    // stands at the left of both.
    for (centre, piece) in [(LIT_AT, "the lit sprite"), (TWIN_AT, "its flipped twin")] {
        let near = brightness(image, centre, 12..28, 8..56);
        let far = brightness(image, centre, 36..52, 8..56);
        assert!(
            near > far + 20.0,
            "{piece}: the side toward the light {near:.1}, the other {far:.1}"
        );
    }
    // The red edge marks the texture's right: the twin shows it on its left.
    let red = |centre: Vec2, xs: Range<usize>| {
        region(image, centre, xs, 8..56).all(|[r, g, ..]| r > 240 && g < 160)
    };
    assert!(
        red(LIT_AT, 57..63) && !red(LIT_AT, 1..7),
        "the sprite's red edge"
    );
    assert!(
        red(TWIN_AT, 1..7) && !red(TWIN_AT, 57..63),
        "the twin's red edge"
    );
}

#[test]
fn a_flashing_lit_sprite_whitens() {
    if !gated() {
        return;
    }
    let image = capture();
    // The block's flash lanes, white at full amount, cover the light, the
    // ambient and the red edge alike.
    for [r, g, b, _] in region(image, FLASH_AT, 2..62, 2..62) {
        assert!(r >= 250 && g >= 250 && b >= 250, "({r}, {g}, {b})");
    }
}

#[test]
fn an_unlit_sprite_ignores_the_light() {
    if !gated() {
        return;
    }
    let image = capture();
    // Beside the light, the albedo's grey 128 stands as drawn, edge
    // included: no light, ambient or emissive reaches the stock pipeline.
    for [r, g, b, _] in region(image, UNLIT_AT, 2..62, 2..62) {
        assert!(
            [r, g, b].iter().all(|&c| c.abs_diff(128) <= 2),
            "({r}, {g}, {b})"
        );
    }
}

/// A capture as RGBA bytes, read by the minimal PNG reader below, so the
/// suite needs no image crate of its own: 8-bit RGBA without interlace, as
/// the renderer writes captures; zlib's stored, fixed and dynamic blocks
/// (RFC 1950, RFC 1951) and the five scanline filters.
struct Rgba {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Rgba {
    fn read(path: &Path) -> Self {
        let bytes = std::fs::read(path).expect("read the capture");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "a PNG");
        let be = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let (mut at, mut width, mut height, mut packed) = (8, 0, 0, Vec::new());
        while at + 8 <= bytes.len() {
            let len = be(at);
            let data = &bytes[at + 8..at + 8 + len];
            match &bytes[at + 4..at + 8] {
                b"IHDR" => {
                    (width, height) = (be(at + 8), be(at + 12));
                    assert_eq!(data[8..13], [8, 6, 0, 0, 0], "8-bit RGBA, no interlace");
                }
                b"IDAT" => packed.extend_from_slice(data),
                _ => {}
            }
            at += 12 + len;
        }
        // Past zlib's two-byte header; its checksum is not read.
        let raw = inflate(&packed[2..]);
        let stride = width * 4;
        assert_eq!(raw.len(), (stride + 1) * height, "one filtered line a row");
        let mut pixels = vec![0u8; stride * height];
        for y in 0..height {
            let filter = raw[y * (stride + 1)];
            for x in 0..stride {
                let left = if x >= 4 {
                    pixels[y * stride + x - 4]
                } else {
                    0
                };
                let up = if y > 0 {
                    pixels[(y - 1) * stride + x]
                } else {
                    0
                };
                let corner = if x >= 4 && y > 0 {
                    pixels[(y - 1) * stride + x - 4]
                } else {
                    0
                };
                let predicted = match filter {
                    0 => 0,
                    1 => left,
                    2 => up,
                    3 => u8::midpoint(left, up),
                    4 => paeth(left, up, corner),
                    other => panic!("scanline filter {other}"),
                };
                pixels[y * stride + x] = raw[y * (stride + 1) + 1 + x].wrapping_add(predicted);
            }
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    fn at(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * self.width + x) * 4;
        self.pixels[i..i + 4].try_into().unwrap()
    }
}

fn paeth(left: u8, up: u8, corner: u8) -> u8 {
    let p = i16::from(left) + i16::from(up) - i16::from(corner);
    let (to_left, to_up, to_corner) = (
        (p - i16::from(left)).abs(),
        (p - i16::from(up)).abs(),
        (p - i16::from(corner)).abs(),
    );
    if to_left <= to_up && to_left <= to_corner {
        left
    } else if to_up <= to_corner {
        up
    } else {
        corner
    }
}

/// A deflate stream's bits, least significant first.
struct Bits<'a> {
    data: &'a [u8],
    at: usize,
    bit: u32,
}

impl Bits<'_> {
    fn take(&mut self, count: u32) -> usize {
        let mut value = 0;
        for i in 0..count {
            value |= usize::from(self.data[self.at] >> self.bit & 1) << i;
            self.bit += 1;
            if self.bit == 8 {
                (self.at, self.bit) = (self.at + 1, 0);
            }
        }
        value
    }
}

/// A canonical Huffman code: how many codes of each length, and the symbols
/// in code order.
struct Huffman {
    counts: [usize; 16],
    symbols: Vec<usize>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Self {
        let mut counts = [0; 16];
        for &length in lengths {
            counts[usize::from(length)] += 1;
        }
        counts[0] = 0;
        let mut next = [0; 16];
        for length in 1..16 {
            next[length] = next[length - 1] + counts[length - 1];
        }
        let mut symbols = vec![0; lengths.len()];
        for (symbol, &length) in lengths.iter().enumerate() {
            if length != 0 {
                symbols[next[usize::from(length)]] = symbol;
                next[usize::from(length)] += 1;
            }
        }
        Self { counts, symbols }
    }

    fn decode(&self, bits: &mut Bits<'_>) -> usize {
        let (mut code, mut first, mut index) = (0, 0, 0);
        for length in 1..16 {
            code |= bits.take(1);
            let count = self.counts[length];
            if code < first + count {
                return self.symbols[index + code - first];
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        panic!("no Huffman code");
    }
}

/// A raw deflate stream's bytes.
fn inflate(data: &[u8]) -> Vec<u8> {
    const LENGTH_BASE: [usize; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LENGTH_EXTRA: [u32; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DISTANCE_BASE: [usize; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DISTANCE_EXTRA: [u32; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    /// The order a dynamic block lists its code-length code's lengths in.
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut bits = Bits {
        data,
        at: 0,
        bit: 0,
    };
    let mut out = Vec::new();
    loop {
        let last = bits.take(1) == 1;
        let (literals, distances) = match bits.take(2) {
            0 => {
                // Stored: from the next byte, its length, the length's
                // complement and the bytes.
                if bits.bit != 0 {
                    (bits.at, bits.bit) = (bits.at + 1, 0);
                }
                let len = usize::from(u16::from_le_bytes([data[bits.at], data[bits.at + 1]]));
                out.extend_from_slice(&data[bits.at + 4..bits.at + 4 + len]);
                bits.at += 4 + len;
                if last {
                    break;
                }
                continue;
            }
            1 => {
                let mut lengths = [8u8; 288];
                lengths[144..256].fill(9);
                lengths[256..280].fill(7);
                (Huffman::new(&lengths), Huffman::new(&[5; 30]))
            }
            2 => {
                let literals = bits.take(5) + 257;
                let distances = bits.take(5) + 1;
                let mut code_lengths = [0u8; 19];
                for &index in &ORDER[..bits.take(4) + 4] {
                    code_lengths[index] = bits.take(3) as u8;
                }
                let code = Huffman::new(&code_lengths);
                let mut lengths = Vec::with_capacity(literals + distances);
                while lengths.len() < literals + distances {
                    let (length, repeat) = match code.decode(&mut bits) {
                        length @ 0..=15 => (length as u8, 1),
                        16 => (
                            *lengths.last().expect("a length to repeat"),
                            3 + bits.take(2),
                        ),
                        17 => (0, 3 + bits.take(3)),
                        _ => (0, 11 + bits.take(7)),
                    };
                    lengths.extend(std::iter::repeat_n(length, repeat));
                }
                (
                    Huffman::new(&lengths[..literals]),
                    Huffman::new(&lengths[literals..]),
                )
            }
            kind => panic!("deflate block type {kind}"),
        };
        loop {
            let symbol = literals.decode(&mut bits);
            match symbol {
                0..=255 => out.push(symbol as u8),
                256 => break,
                _ => {
                    let length = LENGTH_BASE[symbol - 257] + bits.take(LENGTH_EXTRA[symbol - 257]);
                    let index = distances.decode(&mut bits);
                    let distance = DISTANCE_BASE[index] + bits.take(DISTANCE_EXTRA[index]);
                    let start = out.len() - distance;
                    for i in 0..length {
                        out.push(out[start + i]);
                    }
                }
            }
        }
        if last {
            break;
        }
    }
    out
}

#[test]
fn a_z_layer_covers_the_sprite_below_it() {
    if !gated() {
        return;
    }
    let image = capture();
    let white = |[r, g, b, _]: [u8; 4]| r >= 250 && g >= 250 && b >= 250;
    let red = |[r, g, b, _]: [u8; 4]| r >= 200 && g + 60 < r && b + 60 < r;
    let yellow = |[r, g, b, _]: [u8; 4]| r >= 200 && g >= 180 && b + 60 < g;
    // The strip over the red sprite's lower half, and on its own: white.
    let red_left = (UNDER_STRIP_AT.x - STRIP_AT.x) as usize;
    assert!(
        area(image, STRIP_AT, red_left + 2..red_left + 30, 2..14).all(white),
        "the strip over the red sprite"
    );
    assert!(
        area(image, STRIP_AT, 2..14, 2..30).all(white),
        "the strip alone"
    );
    // Above the strip the red sprite shows; the yellow one's upper half lies
    // over the strip.
    assert!(
        area(image, UNDER_STRIP_AT, 2..30, 2..14).all(red),
        "the red sprite above the strip"
    );
    assert!(
        area(image, OVER_STRIP_AT, 2..30, 2..14).all(yellow),
        "the yellow sprite over the strip"
    );
}

#[test]
fn a_tinted_unlit_layer_ignores_the_light() {
    if !gated() {
        return;
    }
    let image = capture();
    // Two lit tiles beside the light, drawn flat: one colour across both
    // halves of each tile and its red edge, its green the albedo's.
    let pixels: Vec<[u8; 4]> = area(image, GLASS_AT, 2..62, 2..30).collect();
    for channel in 0..3 {
        let (low, high) = pixels.iter().fold((u8::MAX, 0), |(low, high), p| {
            (low.min(p[channel]), high.max(p[channel]))
        });
        assert!(high - low <= 2, "channel {channel} spans {low} to {high}");
    }
    let [r, g, b, _] = pixels[0];
    assert!(
        g.abs_diff(128) <= 2 && r + 25 < g && b + 25 < g,
        "({r}, {g}, {b})"
    );
}

#[test]
fn a_backdrop_fills_the_view() {
    if !gated() {
        return;
    }
    let image = capture();
    // Every corner and the empty middle show the backdrop's dark teal, not
    // the clear colour.
    let spots = [
        (2, 2),
        (1277, 2),
        (2, 717),
        (1277, 717),
        (1000, 400),
        (640, 650),
    ];
    let first = image.at(spots[0].0, spots[0].1);
    for (x, y) in spots {
        let [r, g, b, _] = image.at(x, y);
        assert!(
            [r, g, b]
                .iter()
                .zip(&first)
                .all(|(&c, &f)| c.abs_diff(f) <= 1),
            "({x}, {y}): ({r}, {g}, {b})"
        );
        assert!(g > r + 30 && b > r + 30, "({x}, {y}): ({r}, {g}, {b})");
    }
}

/// The capture's pixel at `at`, rounded down to whole pixels.
fn pixel(image: &Rgba, at: Vec2) -> [u8; 4] {
    image.at(at.x as usize, at.y as usize)
}

/// Whether `pixel`'s colour is `rgb` within 1 a channel.
fn is(pixel: [u8; 4], rgb: [u8; 3]) -> bool {
    pixel
        .iter()
        .zip(&rgb)
        .all(|(&c, &want)| c.abs_diff(want) <= 1)
}

#[test]
fn a_fragment_only_glow_draws_its_falloff() {
    if !gated() {
        return;
    }
    let image = capture();
    // Full strength at the centre: its colour (1, 0.6, 0.2) in sRGB.
    let [r, g, b, _] = pixel(image, GLOW_AT);
    assert!(
        r >= 250 && g.abs_diff(203) <= 3 && b.abs_diff(124) <= 3,
        "({r}, {g}, {b})"
    );
    // Fading out along a radius to the backdrop past its circle, which the
    // quad's corner shows.
    let radius = GLOW_SIDE * 0.5;
    let reds: Vec<u8> = [0.0, 0.25, 0.5, 0.75, 0.95]
        .iter()
        .map(|&d| pixel(image, GLOW_AT + Vec2::new(radius * d, 0.0))[0])
        .collect();
    assert!(reds.windows(2).all(|pair| pair[0] > pair[1]), "{reds:?}");
    let backdrop = image.at(2, 2);
    let corner = pixel(image, GLOW_AT + Vec2::splat(radius * 0.9));
    assert!(
        is(corner, [backdrop[0], backdrop[1], backdrop[2]]),
        "{corner:?}"
    );
    assert!(reds[4] > backdrop[0], "a trace of glow inside the circle");
}

#[test]
fn a_flat_quad_and_a_segment_draw_their_colour() {
    if !gated() {
        return;
    }
    let image = capture();
    let (width, height) = (FLAT_SIZE.x as usize, FLAT_SIZE.y as usize);
    assert!(
        area(image, FLAT_AT, 2..width - 2, 2..height - 2).all(|p| is(p, [255, 0, 255])),
        "the flat quad's magenta"
    );
    // Cyan along the segment, its ends included; off to its side, not.
    let along = SEGMENT_TO - SEGMENT_FROM;
    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let at = SEGMENT_FROM + along * t;
        assert!(
            is(pixel(image, at), [0, 255, 255]),
            "at {t}: {:?}",
            pixel(image, at)
        );
    }
    let side = along.perp().normalize() * (SEGMENT_WIDTH * 1.5);
    let off = pixel(image, SEGMENT_FROM + along * 0.5 + side);
    assert!(!is(off, [0, 255, 255]), "beside the segment: {off:?}");
}

#[test]
fn a_beam_spans_its_segment() {
    if !gated() {
        return;
    }
    let image = capture();
    let yellow = |p: [u8; 4]| is(p, [255, 255, 0]);
    let blue = |p: [u8; 4]| is(p, [0, 0, 255]);
    // Its strip from end to end: yellow then blue, 8 px each.
    let y = BEAM_FROM.y as usize;
    for x in (BEAM_FROM.x as usize + 1)..(BEAM_TO.x as usize - 1) {
        let p = image.at(x, y);
        let in_tile = (x - BEAM_FROM.x as usize) % 16;
        let want = if in_tile < 8 { yellow(p) } else { blue(p) };
        assert!(want, "x {x}: {p:?}");
    }
    // Nothing past either end.
    for x in [BEAM_FROM.x as usize - 3, BEAM_TO.x as usize + 3] {
        let p = image.at(x, y);
        assert!(!yellow(p) && !blue(p), "x {x}: {p:?}");
    }
}
