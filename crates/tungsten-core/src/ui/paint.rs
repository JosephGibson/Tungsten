//! The paint list (w01 §6, M2): what the UI draws, as plain data the render
//! crate consumes (`D-007`, `D-018`). [`UiTree::paint`] walks the layers low
//! to high and each root's nodes in order into one ordered [`PaintList`] of
//! rounded rectangles, images and text runs in physical pixels, clipped by
//! integer rectangles under one rounding rule (`D-127`). The partition rule
//! ([`PaintList::text_batches`]) splits the text into batches that draw in
//! paint order, so a rear window's text paints before a front window's
//! background. M56 adds the paint-only properties: opacity and a root's
//! fade multiply every colour's alpha, and translation and scale map each
//! subtree, text at a scale rounded down to a multiple of 1/32; and the
//! prewarm list, a root's text runs painted shown or hidden so the renderer
//! can rasterize them before the root first opens. M57 adds the mesh
//! command: a mesh widget's triangles, their vertices in
//! [`PaintList::vertices`]; and scroll panels, whose children map through
//! their offset and whose thumb paints after them. M58 paints a debug
//! root's units at the paint scale times the debug scale, its text runs at
//! that scale exactly, and a node's shadow as a mesh before its box.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::ops::Range;

use glam::Vec2;

use crate::assets::{AssetRegistry, FilterMode, SpriteAsset, SpriteAssetId, TextureHandle, UvRect};
use crate::text::TextNodeId;

use super::control::{self, Control};
use super::geometry::{Rect, UiTransform};
use super::id::WidgetId;
use super::node::{Node, UiLayer, UiShadow, UiVertex, Visibility, WidgetKind};
use super::scroll::{self, THUMB_WIDTH, children_transform};
use super::tree::{MIN_SCALE, UiTree};

/// The border colour of the layout-bounds overlay.
const BOUNDS_OUTLINE: [u8; 4] = [255, 64, 255, 255];

/// No colour: a part with no border.
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

/// An integer rectangle in physical pixels: a primitive's scissor and the
/// bounds a text run is trimmed to. Index 0 of [`PaintList::clips`] is the
/// viewport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ClipRect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width; zero or less is empty.
    pub width: i32,
    /// Height; zero or less is empty.
    pub height: i32,
}

impl ClipRect {
    /// Whether the rectangle covers no pixel.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    /// The pixels inside both; an empty result keeps a zero width or
    /// height. Edges are summed in `i64`, so no field values overflow.
    #[must_use]
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = edge_sum(self.x, self.width).min(edge_sum(other.x, other.width));
        let bottom = edge_sum(self.y, self.height).min(edge_sum(other.y, other.height));
        let extent =
            |far: i64, near: i32| i32::try_from((far - i64::from(near)).max(0)).unwrap_or(i32::MAX);
        Self {
            x,
            y,
            width: extent(right, x),
            height: extent(bottom, y),
        }
    }
}

/// `start + length` without overflow.
fn edge_sum(start: i32, length: i32) -> i64 {
    i64::from(start) + i64::from(length)
}

/// How far from the origin [`to_physical`] places an edge: within ±2^30
/// physical pixels, every width and edge sum fits in `i32`.
const PHYSICAL_EDGE_LIMIT: i64 = 1 << 30;

/// A physical edge from its rounded float, within the limit (NaN is 0).
fn physical_edge(value: f32) -> i32 {
    // The `as` cast saturates at the `i64` range.
    (value as i64).clamp(-PHYSICAL_EDGE_LIMIT, PHYSICAL_EDGE_LIMIT - 1) as i32
}

/// The one rounding rule (`D-127`): a box in UI units becomes the integer
/// physical rectangle from `floor(min · scale)` to `ceil(max · scale)`, so
/// it covers every pixel the box's primitives touch. Primitives and glyph
/// clips both come through here, so their edges cannot disagree. Edges stay
/// within ±2^30 physical pixels, far past any screen, so a box beyond that
/// is cut there rather than overflowing `i32`.
#[must_use]
pub fn to_physical(rect: Rect, scale: f32) -> ClipRect {
    let x = physical_edge((rect.min.x * scale).floor());
    let y = physical_edge((rect.min.y * scale).floor());
    let right = physical_edge((rect.max.x * scale).ceil());
    let bottom = physical_edge((rect.max.y * scale).ceil());
    ClipRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}

/// A box in UI units as the physical box its primitive draws.
fn scaled(rect: Rect, scale: f32) -> Rect {
    Rect::new(rect.min * scale, rect.max * scale)
}

/// The scale a text run draws at under a composed scale `scale` (M56, Q4):
/// rounded down to a multiple of 1/32, so a scale track rasterizes a few
/// sizes and resting text is never larger than its box; below 1/32 the
/// scale itself.
pub(crate) fn text_scale_step(scale: f32) -> f32 {
    if scale < 1.0 / 32.0 {
        scale
    } else {
        (scale * 32.0).floor() / 32.0
    }
}

/// `rect` scaled by `ratio` about its centre.
fn shrink_about_centre(rect: Rect, ratio: f32) -> Rect {
    let centre = (rect.min + rect.max) * 0.5;
    let half = rect.size() * 0.5 * ratio;
    Rect::new(centre - half, centre + half)
}

/// `color` with its alpha times `alpha`, rounded.
fn faded(color: [u8; 4], alpha: f32) -> [u8; 4] {
    if alpha >= 1.0 {
        return color;
    }
    let [r, g, b, a] = color;
    [r, g, b, (f32::from(a) * alpha).round() as u8]
}

/// A node's own map in its root's units (M56): its scale about its layout
/// centre, then its translation rounded to whole physical pixels at
/// `paint_scale`, the physical pixels per unit of its root (M58).
pub(crate) fn node_transform(node: &Node, rect: Rect, paint_scale: f32) -> UiTransform {
    if node.scale == 1.0 && node.translation == Vec2::ZERO {
        return UiTransform::IDENTITY;
    }
    let translation = if paint_scale > 0.0 && paint_scale.is_finite() {
        (node.translation * paint_scale).round() / paint_scale
    } else {
        node.translation
    };
    UiTransform::about((rect.min + rect.max) * 0.5, node.scale, translation)
}

/// A label's text command under `transform`: its content box mapped, at
/// the composed scale rounded down to a 32nd and shrunk to match about its
/// centre, so the run never outgrows its box or its clips; `unit` is the
/// physical pixels per unit of its root, and `layer` its root's layer
/// scale (M58), which multiplies the run's scale unstepped.
fn text_command(
    node: &Node,
    rect: Rect,
    transform: UiTransform,
    unit: f32,
    layer: f32,
    color: [u8; 4],
    clip: u32,
) -> Option<PaintCommand> {
    let text_node = node.text_id?;
    let exact = transform.rect(node.content.unwrap_or(rect));
    let text_scale = text_scale_step(transform.scale);
    let text_rect = if text_scale == transform.scale {
        exact
    } else {
        shrink_about_centre(exact, text_scale / transform.scale)
    };
    Some(PaintCommand::Text {
        node: text_node,
        rect: scaled(text_rect, unit),
        color,
        scale: layer * text_scale,
        clip,
    })
}

/// What a node inherits from its ancestors in a paint walk.
#[derive(Debug, Clone, Copy)]
struct Inherited {
    clip: u32,
    alpha: f32,
    transform: UiTransform,
}

/// One entry of the paint walk's stack.
#[derive(Debug, Clone, Copy)]
enum PaintStep {
    /// A node to paint, with what it inherits.
    Node(WidgetId, Inherited),
    /// A command to push once the subtree queued above it has painted: a
    /// scroll panel's thumb (M57).
    After(PaintCommand),
}

/// One thing to draw, in physical pixels; `clip` indexes
/// [`PaintList::clips`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaintCommand {
    /// A rounded rectangle: `fill` inside, `border` of `border_width` pixels
    /// along the edge, corners of `corner_radius` pixels. A transparent
    /// colour draws nothing for that part.
    Rect {
        /// The box.
        rect: Rect,
        /// Fill colour, sRGB.
        fill: [u8; 4],
        /// Border colour, sRGB.
        border: [u8; 4],
        /// Border width in physical pixels; zero for none.
        border_width: f32,
        /// Corner radius in physical pixels.
        corner_radius: f32,
        /// The clip index.
        clip: u32,
    },
    /// A textured rectangle: `uv` of `texture`, sampled with `filter`, times
    /// `tint`. A nine-slice is nine of these.
    Image {
        /// The box.
        rect: Rect,
        /// The atlas page.
        texture: TextureHandle,
        /// The page region.
        uv: UvRect,
        /// How the page is sampled.
        filter: FilterMode,
        /// Multiplied into the sampled colour, sRGB.
        tint: [u8; 4],
        /// The clip index.
        clip: u32,
    },
    /// A text run: the committed layout of `node`, never a glyphon type
    /// (`D-126`), drawn in `color` at `rect` and `scale` and trimmed to
    /// `rect` ∩ the clip, so an unwrapped line never leaves the box the
    /// partition rule reasons about.
    Text {
        /// The text node the label shaped with.
        node: TextNodeId,
        /// The label's content box (M56: its box less padding and border).
        rect: Rect,
        /// Text colour, sRGB, its alpha times the label's opacity.
        color: [u8; 4],
        /// The scale the run draws at on top of [`PaintList::scale`]: the
        /// label's composed scale rounded down to a multiple of 1/32 (M56),
        /// times its debug root's debug scale, which is not rounded (M58);
        /// 1 for a label nothing scales.
        scale: f32,
        /// The clip index.
        clip: u32,
    },
    /// A mesh widget's triangles (M57): `count` vertices of
    /// [`PaintList::vertices`] from `first`, every three a triangle, in
    /// physical pixels with straight-alpha sRGB colours, no texture and no
    /// anti-aliasing. [`PaintList::mesh_vertices`] reads them.
    Mesh {
        /// The bounding box of the mapped vertices, which may leave the
        /// widget's box, so the partition rule sees every pixel a triangle
        /// can paint.
        rect: Rect,
        /// The first vertex.
        first: u32,
        /// The vertex count, a multiple of 3.
        count: u32,
        /// The clip index.
        clip: u32,
    },
}

impl PaintCommand {
    /// The command's box.
    #[must_use]
    pub fn rect(&self) -> Rect {
        match self {
            Self::Rect { rect, .. }
            | Self::Image { rect, .. }
            | Self::Text { rect, .. }
            | Self::Mesh { rect, .. } => *rect,
        }
    }

    /// The command's clip index.
    #[must_use]
    pub fn clip(&self) -> u32 {
        match self {
            Self::Rect { clip, .. }
            | Self::Image { clip, .. }
            | Self::Text { clip, .. }
            | Self::Mesh { clip, .. } => *clip,
        }
    }

    /// Whether this is a text run.
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. })
    }
}

/// The ordered commands of one paint, their clips and the scale they were
/// painted at. The app retains one and repaints it when the tree or the
/// scale changed.
#[derive(Debug, Clone, PartialEq)]
pub struct PaintList {
    /// Physical pixels per UI unit.
    pub scale: f32,
    /// The clip rectangles commands index; index 0 is the viewport.
    pub clips: Vec<ClipRect>,
    /// The commands in paint order.
    pub commands: Vec<PaintCommand>,
    /// The vertices [`PaintCommand::Mesh`] commands index, in physical
    /// pixels (M57).
    pub vertices: Vec<UiVertex>,
}

impl Default for PaintList {
    /// Empty, at scale 1.
    fn default() -> Self {
        Self {
            scale: 1.0,
            clips: Vec::new(),
            commands: Vec::new(),
            vertices: Vec::new(),
        }
    }
}

impl PaintList {
    /// Drops the clips, commands and vertices, keeping their capacity and
    /// the scale.
    pub fn clear(&mut self) {
        self.clips.clear();
        self.commands.clear();
        self.vertices.clear();
    }

    /// The vertices a [`PaintCommand::Mesh`] draws; empty for any other
    /// command or a range past [`Self::vertices`].
    #[must_use]
    pub fn mesh_vertices(&self, command: &PaintCommand) -> &[UiVertex] {
        let PaintCommand::Mesh { first, count, .. } = *command else {
            return &[];
        };
        let start = first as usize;
        self.vertices
            .get(start..start.saturating_add(count as usize))
            .unwrap_or(&[])
    }

    /// Whether there is nothing to draw.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// The clip rectangle `command` draws inside.
    #[must_use]
    pub fn clip_of(&self, command: &PaintCommand) -> ClipRect {
        self.clips
            .get(command.clip() as usize)
            .copied()
            .unwrap_or_default()
    }

    /// The text batches, as ranges of command indices, by the partition rule
    /// (w01 §6, turned around in M46 so that text over its own panel stays in
    /// one batch): a batch opens at a text command and every later text
    /// command joins it; a non-text command whose box overlaps any of the
    /// batch's text boxes closes the batch before itself. Each range runs
    /// from the batch's first text command to one past its last; the batch
    /// draws where it closes, after the non-text commands inside its range,
    /// which overlap none of its text. The check is pairwise, a few thousand
    /// box tests for a screen of a hundred labels; a bounding box in its place
    /// split a row of buttons at every button.
    #[must_use]
    pub fn text_batches(&self) -> Vec<Range<usize>> {
        let mut batches = Vec::new();
        let mut start: Option<usize> = None;
        let mut end = 0;
        let mut texts: Vec<Rect> = Vec::new();
        for (i, command) in self.commands.iter().enumerate() {
            if let PaintCommand::Text { rect, .. } = command {
                if start.is_none() {
                    start = Some(i);
                }
                texts.push(*rect);
                end = i + 1;
            } else if let Some(open) = start
                && texts.iter().any(|text| overlaps(*text, command.rect()))
            {
                batches.push(open..end);
                start = None;
                texts.clear();
            }
        }
        if let Some(open) = start {
            batches.push(open..end);
        }
        batches
    }

    /// The snapshot-test format: one `clip n (x, y) w×h` line per clip, then
    /// one line per command (`Rect (x, y) w×h fill #rrggbbaa border #rrggbbaa
    /// 1.0 r 4.0 clip 0`, `Image (x, y) w×h tex 3 uv (u, v)-(u, v) nearest
    /// tint #rrggbbaa clip 1`, `Text node 2:0 (x, y) w×h #rrggbbaa clip 1`,
    /// with ` x0.5` before the clip when the text scale is not 1, `Mesh (x,
    /// y) w×h verts 6 clip 0`) with a `-- batch n` line where
    /// [`Self::text_batches`] opens a batch.
    #[must_use]
    pub fn dump(&self) -> String {
        let mut out = String::new();
        for (i, clip) in self.clips.iter().enumerate() {
            let _ = writeln!(
                out,
                "clip {i} ({}, {}) {}×{}",
                clip.x, clip.y, clip.width, clip.height
            );
        }
        let batches = self.text_batches();
        let mut next_batch = 0;
        for (i, command) in self.commands.iter().enumerate() {
            if batches
                .get(next_batch)
                .is_some_and(|batch| batch.start == i)
            {
                let _ = writeln!(out, "-- batch {next_batch}");
                next_batch += 1;
            }
            match command {
                PaintCommand::Rect {
                    rect,
                    fill,
                    border,
                    border_width,
                    corner_radius,
                    clip,
                } => {
                    let _ = writeln!(
                        out,
                        "Rect {} fill {} border {} {border_width:.1} r {corner_radius:.1} clip {clip}",
                        rect_str(*rect),
                        hex(*fill),
                        hex(*border)
                    );
                }
                PaintCommand::Image {
                    rect,
                    texture,
                    uv,
                    filter,
                    tint,
                    clip,
                } => {
                    let filter = match filter {
                        FilterMode::Nearest => "nearest",
                        FilterMode::Linear => "linear",
                    };
                    let _ = writeln!(
                        out,
                        "Image {} tex {} uv ({:.3}, {:.3})-({:.3}, {:.3}) {filter} tint {} clip {clip}",
                        rect_str(*rect),
                        texture.0,
                        uv.min[0],
                        uv.min[1],
                        uv.max[0],
                        uv.max[1],
                        hex(*tint)
                    );
                }
                PaintCommand::Text {
                    node,
                    rect,
                    color,
                    scale,
                    clip,
                } => {
                    let scale = if *scale == 1.0 {
                        String::new()
                    } else {
                        format!(" x{scale}")
                    };
                    let _ = writeln!(
                        out,
                        "Text node {}:{} {} {}{scale} clip {clip}",
                        node.index(),
                        node.generation(),
                        rect_str(*rect),
                        hex(*color)
                    );
                }
                PaintCommand::Mesh {
                    rect, count, clip, ..
                } => {
                    let _ = writeln!(out, "Mesh {} verts {count} clip {clip}", rect_str(*rect));
                }
            }
        }
        out
    }
}

/// Whether `a` and `b` share any area; touching edges do not.
fn overlaps(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y
}

fn hex(color: [u8; 4]) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        color[0], color[1], color[2], color[3]
    )
}

fn rect_str(rect: Rect) -> String {
    format!(
        "({:.1}, {:.1}) {:.1}×{:.1}",
        rect.min.x,
        rect.min.y,
        rect.width(),
        rect.height()
    )
}

/// What the UI's debug actions set: a `World` resource the paint reads
/// (w01 §14; the engine actions `engine_toggle_ui_bounds` and
/// `engine_toggle_ui` flip it).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct UiDebug {
    /// Paint no `Game` or `GamePopup` root; the debug layers stay.
    pub hide_game_layers: bool,
    /// Append a one-pixel magenta outline of every painted node's box, after
    /// every other command.
    pub show_bounds: bool,
}

/// The four edges of an axis split into two corners and a middle: `start`
/// to `end`, with `a` and `b` of the corners, shrunk in proportion when they
/// would not both fit.
fn split_axis(start: f32, end: f32, a: f32, b: f32) -> [f32; 4] {
    let length = end - start;
    let (a, b) = if a + b > length && a + b > 0.0 {
        let fit = length / (a + b);
        (a * fit, b * fit)
    } else {
        (a, b)
    };
    [start, start + a, end - b, end]
}

/// Pushes `sprite` into `rect`: one image, or nine when the sprite has
/// insets, the corners at source size times `scale` and the edges and the
/// centre stretched (no repeat, w01 §6). Slices with no area are skipped.
fn push_images(
    out: &mut PaintList,
    rect: Rect,
    sprite: &SpriteAsset,
    tint: [u8; 4],
    scale: f32,
    clip: u32,
) {
    let image = |rect: Rect, uv: UvRect| PaintCommand::Image {
        rect,
        texture: sprite.atlas,
        uv,
        filter: sprite.filter,
        tint,
        clip,
    };
    let Some([left, top, right, bottom]) = sprite.nine_slice else {
        out.commands.push(image(rect, sprite.uv));
        return;
    };
    if sprite.width == 0 || sprite.height == 0 {
        out.commands.push(image(rect, sprite.uv));
        return;
    }
    let (width, height) = (sprite.width as f32, sprite.height as f32);
    let uv = sprite.uv;
    let (uv_width, uv_height) = (uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]);
    let xs = split_axis(
        rect.min.x,
        rect.max.x,
        left as f32 * scale,
        right as f32 * scale,
    );
    let ys = split_axis(
        rect.min.y,
        rect.max.y,
        top as f32 * scale,
        bottom as f32 * scale,
    );
    let us = split_axis(
        uv.min[0],
        uv.max[0],
        left as f32 / width * uv_width,
        right as f32 / width * uv_width,
    );
    let vs = split_axis(
        uv.min[1],
        uv.max[1],
        top as f32 / height * uv_height,
        bottom as f32 / height * uv_height,
    );
    for row in 0..3 {
        for col in 0..3 {
            let slice = Rect::new(
                Vec2::new(xs[col], ys[row]),
                Vec2::new(xs[col + 1], ys[row + 1]),
            );
            if slice.width() <= 0.0 || slice.height() <= 0.0 {
                continue;
            }
            out.commands.push(image(
                slice,
                UvRect {
                    min: [us[col], vs[row]],
                    max: [us[col + 1], vs[row + 1]],
                },
            ));
        }
    }
}

/// Pushes a mesh's triangles, when it has any, as one command: each vertex
/// at its fraction of `rect`, mapped by `transform`, in physical pixels at
/// `scale`, its colour's alpha times `alpha`; the command's box bounds them.
fn push_mesh(
    out: &mut PaintList,
    mesh: &[UiVertex],
    rect: Rect,
    transform: UiTransform,
    scale: f32,
    alpha: f32,
    clip: u32,
) {
    if mesh.is_empty() {
        return;
    }
    let first = out.vertices.len();
    let size = rect.size();
    let mut bounds = Rect::new(Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
    for vertex in mesh {
        let position = transform.point(rect.min + vertex.position * size) * scale;
        bounds = Rect::new(bounds.min.min(position), bounds.max.max(position));
        out.vertices
            .push(UiVertex::new(position, faded(vertex.color, alpha)));
    }
    out.commands.push(PaintCommand::Mesh {
        rect: bounds,
        first: first as u32,
        count: mesh.len() as u32,
        clip,
    });
}

/// Pushes `shadow` under a node of box `rect` as one mesh (M58): the box
/// moved by the offset, in the colour, and a ring `blur` wide around it
/// fading to transparent, as nine quads of two triangles (54 vertices),
/// mapped by `transform` into physical pixels at `scale`, the colour's alpha
/// times `alpha`. Nothing when the blur is not finite and positive, the
/// offset is not finite or the colour's alpha is 0.
fn push_shadow(
    out: &mut PaintList,
    rect: Rect,
    shadow: UiShadow,
    transform: UiTransform,
    scale: f32,
    alpha: f32,
    clip: u32,
) {
    let blur = shadow.blur;
    if !blur.is_finite() || blur <= 0.0 || !shadow.offset.is_finite() || shadow.color[3] == 0 {
        return;
    }
    let inner = faded(shadow.color, alpha);
    let outer = [inner[0], inner[1], inner[2], 0];
    let (min, max) = (rect.min + shadow.offset, rect.max + shadow.offset);
    let xs = [min.x - blur, min.x, max.x, max.x + blur];
    let ys = [min.y - blur, min.y, max.y, max.y + blur];
    let corner = |i: usize, j: usize| {
        let color = if (1..=2).contains(&i) && (1..=2).contains(&j) {
            inner
        } else {
            outer
        };
        UiVertex::new(transform.point(Vec2::new(xs[i], ys[j])) * scale, color)
    };
    let first = out.vertices.len();
    for row in 0..3 {
        for col in 0..3 {
            let [a, b, c, d] = [
                corner(col, row),
                corner(col + 1, row),
                corner(col + 1, row + 1),
                corner(col, row + 1),
            ];
            // The top-right and bottom-left corners split along the
            // diagonal through their one inner vertex, as the others do.
            let triangles = if col != row && col != 1 && row != 1 {
                [a, b, d, b, c, d]
            } else {
                [a, b, c, a, c, d]
            };
            out.vertices.extend(triangles);
        }
    }
    let outline = Rect::new(Vec2::new(xs[0], ys[0]), Vec2::new(xs[3], ys[3]));
    out.commands.push(PaintCommand::Mesh {
        rect: scaled(transform.rect(outline), scale),
        first: first as u32,
        count: (out.vertices.len() - first) as u32,
        clip,
    });
}

/// Pushes a rectangle when its fill or its border shows.
fn push_box(
    out: &mut PaintList,
    rect: Rect,
    fill: [u8; 4],
    border: [u8; 4],
    border_width: f32,
    corner_radius: f32,
    clip: u32,
) {
    if fill[3] > 0 || (border[3] > 0 && border_width > 0.0) {
        out.commands.push(PaintCommand::Rect {
            rect,
            fill,
            border,
            border_width,
            corner_radius,
            clip,
        });
    }
}

impl UiTree {
    /// Paints the tree into `out` at `scale` physical pixels per UI unit:
    /// layers low to high, roots and children in order, each node's shadow
    /// (M58) and its own box when its fill or border is visible, then a
    /// label's text run, an image's sprite (`sprites` resolves it; an
    /// unregistered sprite warns once per ID and draws nothing), a toggle's
    /// mark when it is on or a mesh's triangles; a slider paints its track,
    /// fill and thumb in place of its box. Then its children, inside the
    /// intersection of its box and the enclosing clip when it clips them,
    /// skipped when that is empty. A debug root's units paint at `scale`
    /// times the debug scale of the last layout, and its translations and
    /// scroll offsets round to whole pixels there (M58). Clip 0 is the
    /// viewport of the last [`Self::layout`]. `Hidden` and `Collapsed`
    /// subtrees paint nothing; with `debug.hide_game_layers` neither do the
    /// `Game` and `GamePopup` layers, and `debug.show_bounds` appends an
    /// outline per painted node. Clears the paint-dirty flag.
    pub fn paint(
        &mut self,
        scale: f32,
        debug: &UiDebug,
        sprites: &AssetRegistry,
        out: &mut PaintList,
    ) {
        out.clear();
        out.scale = scale;
        self.paint_scale = Some(scale);
        let viewport = self
            .viewport
            .map_or(Rect::new(Vec2::ZERO, Vec2::ZERO), |size| {
                Rect::from_pos_size(Vec2::ZERO, size)
            });
        out.clips.push(to_physical(viewport, scale));

        let mut warned = std::mem::take(&mut self.warned_sprites);
        let mut bounds = Vec::new();
        let mut stack: Vec<PaintStep> = Vec::new();
        for i in 0..self.roots.len() {
            let root = self.roots[i];
            if debug.hide_game_layers
                && self
                    .layer(root)
                    .is_some_and(|layer| matches!(layer, UiLayer::Game | UiLayer::GamePopup))
            {
                continue;
            }
            stack.push(PaintStep::Node(
                root,
                Inherited {
                    clip: 0,
                    alpha: 1.0,
                    transform: UiTransform::IDENTITY,
                },
            ));
            while let Some(step) = stack.pop() {
                match step {
                    PaintStep::Node(id, inherited) => self.paint_node(
                        id,
                        inherited,
                        scale,
                        sprites,
                        out,
                        &mut bounds,
                        &mut warned,
                        &mut stack,
                    ),
                    PaintStep::After(command) => out.commands.push(command),
                }
            }
        }
        if debug.show_bounds {
            for rect in bounds {
                out.commands.push(PaintCommand::Rect {
                    rect,
                    fill: [0, 0, 0, 0],
                    border: BOUNDS_OUTLINE,
                    border_width: 1.0,
                    corner_radius: 0.0,
                    clip: 0,
                });
            }
        }
        self.warned_sprites = warned;
        self.paint_dirty = false;
    }

    /// Paints one node and queues its children, last first, on `stack`,
    /// under a scroll panel's thumb. The node's alpha is `inherited.alpha`
    /// times its opacity (and a root's fade); its map is the inherited one
    /// after its own, and its children's that after a scroll panel's
    /// offset, in its root's units, which paint at `scale` times its
    /// layer's scale. A subtree at alpha 0 or under a composed scale below
    /// 1/64 paints nothing.
    #[allow(clippy::too_many_arguments)] // The walk's state, passed rather than kept on the tree.
    #[allow(clippy::too_many_lines)] // One arm per kind, each a few pushes.
    fn paint_node(
        &self,
        id: WidgetId,
        inherited: Inherited,
        scale: f32,
        sprites: &AssetRegistry,
        out: &mut PaintList,
        bounds: &mut Vec<Rect>,
        warned: &mut HashSet<SpriteAssetId>,
        stack: &mut Vec<PaintStep>,
    ) {
        let Some(node) = self.node(id) else {
            return;
        };
        if node.visibility != Visibility::Visible {
            return;
        }
        let (Some(rect), Some(resolved)) = (node.rect, node.resolved.as_ref()) else {
            return;
        };
        let fade = if node.parent.is_none() {
            node.fade
        } else {
            1.0
        };
        let alpha = inherited.alpha * node.opacity * fade;
        // Physical pixels per unit of this node's root: a debug root's units
        // span the debug scale (M58).
        let layer = self.layer_scale(node.layer);
        let unit = scale * layer;
        let transform = inherited
            .transform
            .compose(node_transform(node, rect, unit));
        if alpha <= 0.0 || transform.scale < MIN_SCALE {
            return;
        }
        let clip = inherited.clip;
        // Lengths in physical pixels per UI unit, under this node's scale.
        let length = unit * transform.scale;
        let physical = scaled(transform.rect(rect), unit);
        let paint_rect = |part: Rect| scaled(transform.rect(part), unit);
        bounds.push(physical);
        if let Some(shadow) = node.shadow {
            push_shadow(out, rect, shadow, transform, unit, alpha, clip);
        }
        if node.kind != WidgetKind::Slider {
            push_box(
                out,
                physical,
                faded(resolved.background, alpha),
                faded(resolved.border, alpha),
                resolved.border_width * length,
                resolved.corner_radius * length,
                clip,
            );
        }
        match node.kind {
            WidgetKind::Label => {
                let color = faded(resolved.text.color, alpha);
                out.commands.extend(text_command(
                    node, rect, transform, unit, layer, color, clip,
                ));
            }
            WidgetKind::Image => {
                if let Some(image) = node.image {
                    match sprites.sprite(image.sprite) {
                        Some(sprite) => {
                            push_images(
                                out,
                                physical,
                                sprite,
                                faded(image.tint, alpha),
                                length,
                                clip,
                            );
                        }
                        None => {
                            if warned.insert(image.sprite) {
                                log::warn!(
                                    "UiTree: image {id:?} names sprite {} ({:?}), which is not registered; it draws nothing",
                                    image.sprite.index(),
                                    sprites.sprite_name(image.sprite).unwrap_or("unknown")
                                );
                            }
                        }
                    }
                }
            }
            WidgetKind::Toggle => {
                if let Some(Control::Toggle(true)) = node.control {
                    let (mark, radius) = control::toggle_mark(rect, resolved.corner_radius);
                    push_box(
                        out,
                        paint_rect(mark),
                        faded(resolved.text.color, alpha),
                        TRANSPARENT,
                        0.0,
                        radius * length,
                        clip,
                    );
                }
            }
            WidgetKind::Slider => {
                if let Some(Control::Slider { value, range }) = node.control {
                    let parts =
                        control::slider_parts(rect, range.fraction(value), resolved.corner_radius);
                    let track_radius = parts.track_radius * length;
                    for (part, fill) in [
                        (parts.track, resolved.background),
                        (parts.fill, resolved.text.color),
                    ] {
                        push_box(
                            out,
                            paint_rect(part),
                            faded(fill, alpha),
                            TRANSPARENT,
                            0.0,
                            track_radius,
                            clip,
                        );
                    }
                    push_box(
                        out,
                        paint_rect(parts.thumb),
                        faded(resolved.text.color, alpha),
                        faded(resolved.border, alpha),
                        resolved.border_width * length,
                        resolved.corner_radius * length,
                        clip,
                    );
                }
            }
            WidgetKind::Mesh => {
                push_mesh(out, &node.mesh, rect, transform, unit, alpha, clip);
            }
            WidgetKind::Panel | WidgetKind::Button => {}
        }
        if node.children.is_empty() {
            return;
        }
        let child_clip = if node.clip_children {
            let inner = out
                .clips
                .get(clip as usize)
                .copied()
                .unwrap_or_default()
                .intersect(to_physical(transform.rect(rect), unit));
            if inner.is_empty() {
                return;
            }
            out.clips.push(inner);
            (out.clips.len() - 1) as u32
        } else {
            clip
        };
        if let Some(thumb) = node
            .scroll
            .and_then(|state| scroll::thumb(rect, state, resolved.border_width))
        {
            stack.push(PaintStep::After(PaintCommand::Rect {
                rect: paint_rect(thumb),
                fill: faded(resolved.text.color, 0.5 * alpha),
                border: TRANSPARENT,
                border_width: 0.0,
                corner_radius: THUMB_WIDTH * 0.5 * length,
                clip,
            }));
        }
        let inherited = Inherited {
            clip: child_clip,
            alpha,
            transform: transform.compose(children_transform(node, unit)),
        };
        for child in node.children.iter().rev() {
            stack.push(PaintStep::Node(*child, inherited));
        }
    }

    /// Asks for a one-shot raster of `root`'s text runs, shown or hidden,
    /// once it is laid out (M56): the next [`Self::paint_prewarm`] after its
    /// first layout paints them, and the app hands them to the renderer
    /// before the frame's prepare, so a menu's first open finds its glyphs
    /// in the atlas. A collapsed or removed root drops the request; a
    /// widget that is not a root does nothing. A declared character set,
    /// such as a HUD's digits, is a hidden label in a prewarm root.
    pub fn prewarm(&mut self, root: WidgetId) {
        if self.node(root).is_some_and(|node| node.parent.is_none())
            && !self.prewarm_roots.contains(&root)
        {
            self.prewarm_roots.push(root);
        }
    }

    /// Whether a prewarm request waits.
    #[must_use]
    pub fn prewarm_pending(&self) -> bool {
        !self.prewarm_roots.is_empty()
    }

    /// Paints into `out`, at `scale`, the text commands of the roots
    /// [`Self::prewarm`] asked for that are laid out, at their current
    /// transforms and whatever their visibility, opacity or freeze, under
    /// the viewport clip; clears those requests and drops those of
    /// collapsed and removed roots, keeping the rest for their first
    /// layout. Returns whether it painted any run.
    pub fn paint_prewarm(&mut self, scale: f32, out: &mut PaintList) -> bool {
        out.clear();
        out.scale = scale;
        if self.prewarm_roots.is_empty() {
            return false;
        }
        let viewport = self
            .viewport
            .map_or(Rect::new(Vec2::ZERO, Vec2::ZERO), |size| {
                Rect::from_pos_size(Vec2::ZERO, size)
            });
        out.clips.push(to_physical(viewport, scale));
        let requests = std::mem::take(&mut self.prewarm_roots);
        let mut stack: Vec<(WidgetId, UiTransform)> = Vec::new();
        for root in requests {
            let Some(node) = self.node(root) else {
                continue;
            };
            if node.visibility == Visibility::Collapsed {
                continue;
            }
            if node.rect.is_none() {
                self.prewarm_roots.push(root);
                continue;
            }
            stack.push((root, UiTransform::IDENTITY));
            while let Some((id, parent)) = stack.pop() {
                self.prewarm_node(id, parent, scale, out, &mut stack);
            }
        }
        !out.commands.is_empty()
    }

    /// One node of the prewarm walk: its text run, then its children.
    fn prewarm_node(
        &self,
        id: WidgetId,
        parent: UiTransform,
        scale: f32,
        out: &mut PaintList,
        stack: &mut Vec<(WidgetId, UiTransform)>,
    ) {
        let Some(node) = self.node(id) else {
            return;
        };
        let (Some(rect), Some(resolved)) = (node.rect, node.resolved.as_ref()) else {
            return;
        };
        let layer = self.layer_scale(node.layer);
        let unit = scale * layer;
        let transform = parent.compose(node_transform(node, rect, unit));
        if transform.scale < MIN_SCALE {
            return;
        }
        if node.kind == WidgetKind::Label {
            out.commands.extend(text_command(
                node,
                rect,
                transform,
                unit,
                layer,
                resolved.text.color,
                0,
            ));
        }
        let children = transform.compose(children_transform(node, unit));
        for child in node.children.iter().rev() {
            stack.push((*child, children));
        }
    }
}

#[cfg(test)]
#[path = "../tests/ui/paint.rs"]
mod tests;
