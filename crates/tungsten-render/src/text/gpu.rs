use std::collections::HashMap;

use glyphon::{Cache, Resolution, SwashCache, TextAtlas, TextRenderer, Viewport};
use tungsten_core::assets::ResolvedFontFamily;
use wgpu::{Device, MultisampleState, Queue, RenderPass, TextureFormat};

use super::{FontSource, TextEngine, TextSection};

/// Glyphon text pipeline state: the [`TextEngine`] that lays text out, and
/// the atlas, renderer, viewport and swash cache that draw it.
pub struct TextPipeline {
    engine: TextEngine,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    /// Whether this frame's `prepare` reached glyphon; `post_frame` reads it.
    prepared_this_frame: bool,
}

impl TextPipeline {
    /// Text renders in its own overlay pass after the post stack, which always
    /// targets a single-sample color texture with no depth, so the pipeline
    /// bakes those attachment bits whatever the scene's MSAA and depth config.
    ///
    /// Text draws with packaged fonts only ([`FontSource::Packaged`]).
    #[must_use]
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat) -> Self {
        Self::with_font_source(device, queue, format, FontSource::Packaged)
    }

    /// [`Self::new`] with the faces `source` allows (`D-116`).
    #[must_use]
    pub fn with_font_source(
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
        source: FontSource,
    ) -> Self {
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer = TextRenderer::new(
            &mut atlas,
            device,
            MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            crate::quad::passthrough_depth_stencil(false),
        );

        Self {
            engine: TextEngine::new(source),
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            prepared_this_frame: false,
        }
    }

    /// Load font bytes under manifest ID.
    pub fn load_font(&mut self, id: &str, data: Vec<u8>) {
        self.engine.load_font(id, data);
    }

    /// Hot-reload font bytes and evict dependent caches.
    pub fn reload_font(&mut self, id: &str, data: Vec<u8>) {
        self.atlas.trim();
        self.engine.reload_font(id, data);
    }

    pub(crate) fn engine(&self) -> &TextEngine {
        &self.engine
    }

    /// Crate-private: the atlas and swash cache key glyphs by the engine's
    /// fontdb IDs, so the engine must not be swapped under them.
    pub(crate) fn engine_mut(&mut self) -> &mut TextEngine {
        &mut self.engine
    }

    /// Set the font families and the fallback chain (`D-115`).
    pub(crate) fn set_font_families(
        &mut self,
        families: &HashMap<String, ResolvedFontFamily>,
        fallback: &[String],
    ) {
        self.engine.set_font_families(families, fallback);
    }

    /// Prepare glyph buffers and atlas for render.
    pub fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        sections: &[TextSection],
        width: u32,
        height: u32,
    ) {
        self.viewport.update(queue, Resolution { width, height });

        // Unchanged since the last prepare: glyphon keeps its vertex buffer
        // and atlas between prepares, and nothing else writes the atlas.
        self.prepared_this_frame = self.engine.update(sections, width, height);
        if !self.prepared_this_frame {
            return;
        }

        let (font_system, areas) = self.engine.prepare_parts();
        match self.text_renderer.prepare(
            device,
            queue,
            font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash_cache,
        ) {
            Ok(()) => self.engine.mark_prepared(width, height),
            Err(e) => log::error!("Text prepare error: {e:?}"),
        }
    }

    /// Draw prepared text.
    pub fn render<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if let Err(e) = self.text_renderer.render(&self.atlas, &self.viewport, pass) {
            log::error!("Text render error: {e:?}");
        }
    }

    /// Trim unused atlas entries after presenting.
    pub fn post_frame(&mut self) {
        // `trim` only clears glyphon's in-use set, which a skipped prepare
        // never filled.
        if std::mem::take(&mut self.prepared_this_frame) {
            self.atlas.trim();
        }
    }
}
