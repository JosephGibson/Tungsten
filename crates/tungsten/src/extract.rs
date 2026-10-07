//! The frame's extract channels (`D-138`): [`Extracts`], through which a game
//! or a plugin adds sprites, quads and text to what the engine draws.
//!
//! Each channel is a base, its default or a game's replacement, followed by
//! the contributions in the order they were added. `App` calls the channels
//! after the stages with a shared `&World` (`D-018`, `D-128`), then adds its
//! own pieces: the HUD, the overlays, debug draw, lights and mesh particles.

use tungsten_core::World;
use tungsten_render::{QuadInstance, SpriteBatch, TextSection};

use crate::sprite_extract::extract_sprites_default;
use crate::tilemap_extract::extract_tilemaps;

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
/// replacement a game set. The sprite default draws the tilemaps, then the
/// `Sprite` entities; quads and text have no default.
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
/// caller's depth and is tested against the default's: its tiles sit at
/// 1.0, the far plane, and its sprites in [0, 1), the last drawn at 0.
///
/// Without the resource, which only its removal causes, the channels draw
/// nothing.
pub struct Extracts {
    sprites: Channel<SpriteBatch>,
    quads: Channel<QuadInstance>,
    text: Channel<TextSection>,
}

impl Default for Extracts {
    /// Sprites: the tilemaps at the far plane, then the `Sprite` entities.
    /// Quads and text: nothing.
    fn default() -> Self {
        Self {
            sprites: Channel::new(Some(Box::new(default_sprites))),
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

    /// Replaces the sprite channel's base, by default the tilemaps then the
    /// `Sprite` entities, with `f`; contributions stay. The last call wins.
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
        self.sprites.run(world)
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

/// The default sprite base: the tilemaps, then the `Sprite` entities. The
/// tiles go to `z_norm` 1.0, the far plane, so under the `gpu_depth` sort
/// every sprite still draws over them; the CPU sort has no depth test. With
/// no tiles in view this is the sprite extract's vector unchanged.
fn default_sprites(world: &World) -> Vec<SpriteBatch> {
    let mut batches = extract_tilemaps(world);
    if batches.is_empty() {
        return extract_sprites_default(world);
    }
    for batch in &mut batches {
        for tile in &mut batch.instances {
            tile.z_norm = 1.0;
        }
    }
    batches.extend(extract_sprites_default(world));
    batches
}

#[cfg(test)]
#[path = "tests/extract.rs"]
mod tests;
