use std::collections::{HashMap, HashSet};

use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, Style,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight,
};
use wgpu::{Device, MultisampleState, Queue, RenderPass, TextureFormat};

/// High-level text draw command. Game code produces these; the renderer
/// converts them into glyphon buffers each frame.
#[derive(Debug, Clone)]
pub struct TextSection {
    pub content: String,
    pub font_id: String,
    pub font_size: f32,
    pub line_height: f32,
    /// RGBA color, each channel 0–255.
    pub color: [u8; 4],
    /// Screen-space position in pixels (left, top).
    pub position: [f32; 2],
    /// Optional (width, height): passed to the layout buffer for wrapping, and
    /// used with [`position`](Self::position) to build glyphon clip bounds
    /// (intersected with the viewport). Omit for full-viewport clipping only.
    pub bounds: Option<[f32; 2]>,
}

struct StoredFontAttrs {
    family: String,
    weight: Weight,
    style: Style,
}

/// Owns all glyphon state and provides prepare/render methods that sit
/// alongside the quad and sprite pipelines in the Renderer.
pub struct TextPipeline {
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    font_attrs: HashMap<String, StoredFontAttrs>,
}

impl TextPipeline {
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);

        Self {
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            font_attrs: HashMap::new(),
        }
    }

    /// Load a font from raw TTF/OTF bytes and associate it with a manifest ID.
    /// The font's family name and weight are auto-detected from the file.
    pub fn load_font(&mut self, id: &str, data: Vec<u8>) {
        let ids_before: HashSet<_> = self.font_system.db().faces().map(|f| f.id).collect();
        self.font_system.db_mut().load_font_data(data);

        let new_faces: Vec<_> = self
            .font_system
            .db()
            .faces()
            .filter(|f| !ids_before.contains(&f.id))
            .collect();

        if new_faces.is_empty() {
            log::warn!("Font '{id}': no face detected after loading data");
            return;
        }

        if new_faces.len() > 1 {
            log::warn!(
                "Font '{id}': TTF/OTF contains {} faces; using the first for manifest ID '{id}'",
                new_faces.len(),
            );
        }

        let face = new_faces[0];
        let family = face
            .families
            .first()
            .map(|(name, _)| name.clone())
            .unwrap_or_default();
        let weight = face.weight;
        let style = face.style;
        log::info!(
            "Registered font '{id}' -> family=\"{family}\", weight={weight:?}, style={style:?}",
        );
        self.font_attrs.insert(
            id.to_string(),
            StoredFontAttrs {
                family,
                weight,
                style,
            },
        );
    }

    /// Build glyphon Buffers from TextSections, upload glyphs, and prepare
    /// vertex data for rendering. Must be called before `render`.
    pub fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        sections: &[TextSection],
        width: u32,
        height: u32,
    ) {
        self.viewport.update(queue, Resolution { width, height });

        let mut text_areas: Vec<(Buffer, TextSection)> = Vec::with_capacity(sections.len());

        for section in sections {
            let mut buffer = Buffer::new(
                &mut self.font_system,
                Metrics::new(section.font_size, section.line_height),
            );

            let (buf_w, buf_h) = match section.bounds {
                Some([w, h]) => (Some(w), Some(h)),
                None => (Some(width as f32), None),
            };
            buffer.set_size(&mut self.font_system, buf_w, buf_h);

            let attrs = make_attrs(&self.font_attrs, &section.font_id);
            buffer.set_text(
                &mut self.font_system,
                &section.content,
                &attrs,
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(&mut self.font_system, false);

            text_areas.push((buffer, section.clone()));
        }

        let areas: Vec<TextArea<'_>> = text_areas
            .iter()
            .map(|(buffer, section)| {
                let [r, g, b, a] = section.color;
                let bounds = clip_bounds_for_section(section, width, height);
                TextArea {
                    buffer,
                    left: section.position[0],
                    top: section.position[1],
                    scale: 1.0,
                    bounds,
                    default_color: Color::rgba(r, g, b, a),
                    custom_glyphs: &[],
                }
            })
            .collect();

        if let Err(e) = self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash_cache,
        ) {
            log::error!("Text prepare error: {e:?}");
        }
    }

    /// Draw prepared text into an active render pass.
    pub fn render<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if let Err(e) = self.text_renderer.render(&self.atlas, &self.viewport, pass) {
            log::error!("Text render error: {e:?}");
        }
    }

    /// Trim unused atlas entries after presenting. Call once per frame.
    pub fn post_frame(&mut self) {
        self.atlas.trim();
    }
}

fn clip_bounds_for_section(section: &TextSection, width: u32, height: u32) -> TextBounds {
    let vw = width as i32;
    let vh = height as i32;
    match section.bounds {
        Some([bw, bh]) if bw > 0.0 && bh > 0.0 => {
            let left = section.position[0].max(0.0).floor() as i32;
            let top = section.position[1].max(0.0).floor() as i32;
            let right = (section.position[0] + bw).min(width as f32).floor() as i32;
            let bottom = (section.position[1] + bh).min(height as f32).floor() as i32;
            TextBounds {
                left,
                top,
                right: right.max(left),
                bottom: bottom.max(top),
            }
        }
        _ => TextBounds {
            left: 0,
            top: 0,
            right: vw,
            bottom: vh,
        },
    }
}

fn make_attrs<'a>(font_attrs: &'a HashMap<String, StoredFontAttrs>, font_id: &str) -> Attrs<'a> {
    if let Some(stored) = font_attrs.get(font_id) {
        Attrs::new()
            .family(Family::Name(&stored.family))
            .weight(stored.weight)
            .style(stored.style)
    } else {
        log::warn!("Unknown font ID '{font_id}', falling back to sans-serif");
        Attrs::new().family(Family::SansSerif)
    }
}
