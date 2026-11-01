//! The frame's extract channels (`D-138`): [`Extracts`], through which a game
//! or a plugin adds sprites, quads and text to what the engine draws.
//!
//! Each channel is a base, its default or a game's replacement, followed by
//! the contributions in the order they were added. `App` calls the channels
//! after the stages with a shared `&World` (`D-018`, `D-128`), then adds its
//! own pieces: the world labels, the HUD, the overlays, debug draw, lights
//! and mesh particles.

use glam::Vec2;
use tungsten_core::{CameraState, ScaleFactor, UiDebug, World, WorldLabels};
use tungsten_render::{QuadInstance, SpriteBatch, TextSection, WorldLabelRun};

use crate::sprite_extract::{extract_backdrops, extract_sprites_among, extract_sprites_default};
use crate::tilemap_extract::extract_tiles_raising;

/// A channel's boxed closure: for sprites, quads and text, the
/// `ExtractSpritesFn`, `ExtractQuadsFn` and `ExtractTextFn` of `tungsten::app`.
type ExtractFn<T> = Box<dyn Fn(&World) -> Vec<T>>;

/// One channel: its base, then its contributions in the order they were
/// added.
struct Channel<T> {
    base: Option<ExtractFn<T>>,
    added: Vec<ExtractFn<T>>,
}

impl<T> Channel<T> {
    fn new(base: Option<ExtractFn<T>>) -> Self {
        Self {
            base,
            added: Vec::new(),
        }
    }

    /// The base's output, then each contribution's. With no contributions
    /// the base's vector comes back as it returned it.
    fn run(&self, world: &World) -> Vec<T> {
        let mut out = self.base.as_ref().map_or_else(Vec::new, |base| base(world));
        for add in &self.added {
            out.extend(add(world));
        }
        out
    }
}

/// The frame's sprite, quad and text channels, a `World` resource that `App`
/// always inserts. A channel draws its base, then its contributions in the
/// order they were added. The base is the channel's default or the
/// replacement a game set. The sprite default draws the backdrops, the
/// under-tile contributions, the tile layers without a `z`, the over-tile
/// contributions, then the `Sprite` entities with each layer that has a `z`
/// among them (redo 7.9); quads and text have no default.
///
/// A plugin adds in its `build`:
/// `world.resource_mut::<Extracts>().add_text(score_text)`. `App` inserts
/// the resource before it builds any plugin, so engine plugins contribute
/// before a game's. Replacing a base keeps the contributions; a game that
/// wants none of them leaves out the plugin that adds them.
///
/// Depth: a contribution's instances keep the `z_norm` its closure wrote.
/// Under the CPU sort, the channel's order is what draws on top. Under the
/// `gpu_depth` sort, an instance at `z_norm` 0, which hand-written extracts
/// usually write, passes the depth test against everything drawn before it,
/// so a contribution draws over the default. A nonzero `z_norm` is the
/// caller's depth and is tested against the default's: its backdrops,
/// under- and over-tile contributions and bottom tiles sit at 1.0, the far
/// plane, where a later draw wins, and its sprites and raised layers in
/// [0, 1), the last drawn at 0.
///
/// Without the resource, which only its removal causes, the channels draw
/// nothing.
pub struct Extracts {
    /// No base is the default base, which places the tile slots itself.
    sprites: Channel<SpriteBatch>,
    /// What draws under and over the default base's bottom tiles.
    under_tiles: Vec<ExtractFn<SpriteBatch>>,
    over_tiles: Vec<ExtractFn<SpriteBatch>>,
    quads: Channel<QuadInstance>,
    text: Channel<TextSection>,
}

impl Default for Extracts {
    /// Sprites: the backdrops and the bottom tiles at the far plane, then
    /// the `Sprite` entities among the raised layers. Quads and text:
    /// nothing.
    fn default() -> Self {
        Self {
            sprites: Channel::new(None),
            under_tiles: Vec::new(),
            over_tiles: Vec::new(),
            quads: Channel::new(None),
            text: Channel::new(None),
        }
    }
}

impl Extracts {
    /// Adds `f` to the sprite channel: its batches draw after the base's and
    /// after every sprite contribution added before it.
    pub fn add_sprites(&mut self, f: impl Fn(&World) -> Vec<SpriteBatch> + 'static) {
        self.sprites.added.push(Box::new(f));
    }

    /// Adds `f` under the default base's bottom tiles (redo 7.9): its batches
    /// draw after the backdrops and before the tile layers without a `z`,
    /// after every under-tile contribution added before it, at the far plane
    /// under `gpu_depth`. Under a replaced base they draw before it.
    pub fn add_sprites_under_tiles(&mut self, f: impl Fn(&World) -> Vec<SpriteBatch> + 'static) {
        self.under_tiles.push(Box::new(f));
    }

    /// Adds `f` over the default base's bottom tiles (redo 7.9): its batches
    /// draw after them and before the sprites, after every over-tile
    /// contribution added before it, at the far plane under `gpu_depth`.
    /// Under a replaced base they draw before it, after the under-tile ones.
    pub fn add_sprites_over_tiles(&mut self, f: impl Fn(&World) -> Vec<SpriteBatch> + 'static) {
        self.over_tiles.push(Box::new(f));
    }

    /// Adds `f` to the quad channel: its quads draw after the base's and
    /// after every quad contribution added before it.
    pub fn add_quads(&mut self, f: impl Fn(&World) -> Vec<QuadInstance> + 'static) {
        self.quads.added.push(Box::new(f));
    }

    /// Adds `f` to the text channel: its sections draw after the base's and
    /// after every text contribution added before it.
    pub fn add_text(&mut self, f: impl Fn(&World) -> Vec<TextSection> + 'static) {
        self.text.added.push(Box::new(f));
    }

    /// Replaces the sprite channel's base, by default the backdrops, the
    /// tilemaps and the `Sprite` entities, with `f`; contributions stay, the
    /// under- and over-tile ones drawing before it. The last call wins.
    pub fn replace_sprites(&mut self, f: impl Fn(&World) -> Vec<SpriteBatch> + 'static) {
        self.sprites.base = Some(Box::new(f));
    }

    /// Replaces the quad channel's base, by default nothing, with `f`;
    /// contributions stay. The last call wins.
    pub fn replace_quads(&mut self, f: impl Fn(&World) -> Vec<QuadInstance> + 'static) {
        self.quads.base = Some(Box::new(f));
    }

    /// Replaces the text channel's base, by default nothing, with `f`;
    /// contributions stay. The last call wins.
    pub fn replace_text(&mut self, f: impl Fn(&World) -> Vec<TextSection> + 'static) {
        self.text.base = Some(Box::new(f));
    }

    /// The sprite channel's batches for `world`'s frame.
    pub(crate) fn sprites(&self, world: &World) -> Vec<SpriteBatch> {
        let mut out = match &self.sprites.base {
            None => default_sprites(world, &self.under_tiles, &self.over_tiles),
            Some(base) if self.under_tiles.is_empty() && self.over_tiles.is_empty() => base(world),
            Some(base) => {
                let mut out = Vec::new();
                run_at_far_plane(&self.under_tiles, world, &mut out);
                run_at_far_plane(&self.over_tiles, world, &mut out);
                out.extend(base(world));
                out
            }
        };
        for add in &self.sprites.added {
            out.extend(add(world));
        }
        out
    }

    /// The quad channel's quads for `world`'s frame.
    pub(crate) fn quads(&self, world: &World) -> Vec<QuadInstance> {
        self.quads.run(world)
    }

    /// The text channel's sections for `world`'s frame.
    pub(crate) fn text(&self, world: &World) -> Vec<TextSection> {
        self.text.run(world)
    }
}

/// The default sprite base: the backdrops, the under-tile slot, the tile
/// layers without a `z`, the over-tile slot, then the `Sprite` entities with
/// each layer that has a `z` among them. All before the sprites go to
/// `z_norm` 1.0, the far plane, so under the `gpu_depth` sort every sprite
/// still draws over them; the CPU sort has no depth test. With no tiles in
/// view and none of the rest this is the sprite extract's vector unchanged.
fn default_sprites(
    world: &World,
    under: &[ExtractFn<SpriteBatch>],
    over: &[ExtractFn<SpriteBatch>],
) -> Vec<SpriteBatch> {
    let mut raised = Vec::new();
    let mut batches = extract_tiles_raising(world, &mut raised);
    let backdrops = extract_backdrops(world);
    if raised.is_empty() && backdrops.is_empty() && under.is_empty() && over.is_empty() {
        if batches.is_empty() {
            return extract_sprites_default(world);
        }
        at_far_plane(&mut batches);
        batches.extend(extract_sprites_default(world));
        return batches;
    }
    // Stable: layers of one `z` keep their maps' and files' order.
    raised.sort_by_key(|&(z, _)| z);
    let mut out = backdrops;
    run_at_far_plane(under, world, &mut out);
    at_far_plane(&mut batches);
    out.extend(batches);
    run_at_far_plane(over, world, &mut out);
    out.extend(extract_sprites_among(world, &mut raised));
    out
}

/// Every instance of `batches` at `z_norm` 1.0, the far plane.
fn at_far_plane(batches: &mut [SpriteBatch]) {
    for batch in batches {
        for instance in &mut batch.instances {
            instance.z_norm = 1.0;
        }
    }
}

/// Each of `slots`' batches, in order, at the far plane onto `out`.
fn run_at_far_plane(slots: &[ExtractFn<SpriteBatch>], world: &World, out: &mut Vec<SpriteBatch>) {
    for slot in slots {
        let mut batches = slot(world);
        at_far_plane(&mut batches);
        out.extend(batches);
    }
}

/// The step a world label's drawn scale snaps to (Q17 of M54): glyphon keys
/// a glyph by its physical size, so a continuous pop would rasterize new
/// glyphs every frame, while eighths rasterize a few sizes (`D-126`).
pub(crate) const LABEL_SCALE_STEP: f32 = 0.125;

/// What the world-label extract did with the frame's labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct WorldLabelCounts {
    /// Labels kept for drawing.
    pub(crate) kept: u32,
    /// Labels whose projected point lay outside the grown viewport, or whose
    /// drawn scale snapped to nothing.
    pub(crate) culled: u32,
    /// Labels past the cap, the latest pushes.
    pub(crate) dropped: u32,
}

/// Drains the frame's [`WorldLabels`] and projects each through the camera
/// onto the `viewport`, in physical pixels: the anchor's screen point plus
/// the offset times the scale factor. It culls a label whose point lies
/// outside the viewport grown by the cull margin, keeps the first `cap`
/// survivors in push order and snaps each one's scale to
/// [`LABEL_SCALE_STEP`]. While `UiDebug::hide_game_layers` is set it drains
/// and keeps nothing. Without the resource it does nothing.
pub(crate) fn extract_world_labels(
    world: &mut World,
    viewport: (f32, f32),
) -> (Vec<WorldLabelRun>, WorldLabelCounts) {
    let mut runs = Vec::new();
    let mut counts = WorldLabelCounts::default();
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let scale = world
        .get_resource::<ScaleFactor>()
        .map_or(1.0, ScaleFactor::get);
    let hidden = world
        .get_resource::<UiDebug>()
        .is_some_and(|debug| debug.hide_game_layers);
    let Some(labels) = world.get_resource_mut::<WorldLabels>() else {
        return (runs, counts);
    };
    if hidden {
        labels.clear();
        return (runs, counts);
    }
    let cap = labels.cap();
    let margin = labels.cull_margin() * scale;
    let (min, max) = (
        Vec2::splat(-margin),
        Vec2::new(viewport.0 + margin, viewport.1 + margin),
    );
    for label in labels.drain() {
        let point = camera.world_to_screen(label.anchor) + label.offset * scale;
        let drawn_scale = (label.scale * scale / LABEL_SCALE_STEP).round() * LABEL_SCALE_STEP;
        // Written so that a NaN point or scale culls.
        let inside = point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y;
        if !(inside && drawn_scale > 0.0 && drawn_scale.is_finite()) {
            counts.culled += 1;
            continue;
        }
        if runs.len() >= cap {
            counts.dropped += 1;
            continue;
        }
        let mut run = WorldLabelRun::default();
        run.content = label.text;
        run.font_id = label.font_id;
        run.font_size = label.font_size;
        run.line_height = label.line_height;
        run.position = point.to_array();
        run.pivot = label.pivot.to_array();
        run.scale = drawn_scale;
        run.color = label.color;
        runs.push(run);
    }
    counts.kept = runs.len() as u32;
    (runs, counts)
}

#[cfg(test)]
#[path = "tests/extract.rs"]
mod tests;
