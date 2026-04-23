use std::collections::{HashMap, HashSet};

use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, Style,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight,
};
use wgpu::{Device, MultisampleState, Queue, RenderPass, TextureFormat};

/// High-level text draw command.
#[derive(Debug, Clone)]
pub struct TextSection {
    pub content: String,
    pub font_id: String,
    pub font_size: f32,
    pub line_height: f32,
    /// RGBA color.
    pub color: [u8; 4],
    /// Screen-space top-left position.
    pub position: [f32; 2],
    /// Optional wrap/clip bounds.
    pub bounds: Option<[f32; 2]>,
}

struct StoredFontAttrs {
    family: String,
    weight: Weight,
    style: Style,
    /// fontdb faces loaded from this manifest entry.
    face_ids: Vec<glyphon::fontdb::ID>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextLayoutKey {
    content: String,
    font_id: String,
    font_size_bits: u32,
    line_height_bits: u32,
    buffer_width_bits: Option<u32>,
    buffer_height_bits: Option<u32>,
}

#[derive(Debug)]
struct CachedTextBuffer {
    buffer: Buffer,
    last_used_frame: u64,
}

const BUFFER_CACHE_PRUNE_INTERVAL_FRAMES: u64 = 120;
const BUFFER_CACHE_TTL_FRAMES: u64 = 360;

/// Glyphon text pipeline state.
pub struct TextPipeline {
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    font_attrs: HashMap<String, StoredFontAttrs>,
    buffer_cache: HashMap<TextLayoutKey, CachedTextBuffer>,
    frame_counter: u64,
}

impl TextPipeline {
    #[must_use]
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
            buffer_cache: HashMap::new(),
            frame_counter: 0,
        }
    }

    /// Load font bytes under manifest ID.
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
        let face_ids: Vec<_> = new_faces.iter().map(|f| f.id).collect();
        log::info!(
            "Registered font '{id}' -> family=\"{family}\", weight={weight:?}, style={style:?}",
        );
        self.font_attrs.insert(
            id.to_string(),
            StoredFontAttrs {
                family,
                weight,
                style,
                face_ids,
            },
        );
        // Fontdb changes can alter fallback/selection.
        self.buffer_cache.clear();
    }

    /// Hot-reload font bytes and evict dependent caches.
    pub fn reload_font(&mut self, id: &str, data: Vec<u8>) {
        if let Some(old) = self.font_attrs.remove(id) {
            let db = self.font_system.db_mut();
            for face_id in old.face_ids {
                db.remove_face(face_id);
            }
        }
        self.atlas.trim();
        self.buffer_cache.clear();
        self.load_font(id, data);
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
        self.frame_counter = self.frame_counter.wrapping_add(1);
        let frame_counter = self.frame_counter;

        let mut keys: Vec<TextLayoutKey> = Vec::with_capacity(sections.len());

        for section in sections {
            let mut buffer = Buffer::new(
                &mut self.font_system,
                Metrics::new(section.font_size, section.line_height),
            );

            let (buf_w, buf_h) = match section.bounds {
                Some([w, h]) => (Some(w), Some(h)),
                None => (Some(width as f32), None),
            };
            let key = TextLayoutKey::new(section, buf_w, buf_h);
            keys.push(key.clone());

            if let Some(cached) = self.buffer_cache.get_mut(&key) {
                cached.last_used_frame = frame_counter;
                continue;
            }

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
            self.buffer_cache.insert(
                key,
                CachedTextBuffer {
                    buffer,
                    last_used_frame: frame_counter,
                },
            );
        }

        if frame_counter.is_multiple_of(BUFFER_CACHE_PRUNE_INTERVAL_FRAMES) {
            self.buffer_cache.retain(|_, v| {
                frame_counter.saturating_sub(v.last_used_frame) <= BUFFER_CACHE_TTL_FRAMES
            });
        }

        let areas: Vec<TextArea<'_>> = sections
            .iter()
            .zip(keys.iter())
            .filter_map(|(section, key)| self.buffer_cache.get(key).map(|cached| (section, cached)))
            .map(|(section, cached)| {
                let [r, g, b, a] = section.color;
                let bounds = clip_bounds_for_section(section, width, height);
                TextArea {
                    buffer: &cached.buffer,
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

    /// Draw prepared text.
    pub fn render<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if let Err(e) = self.text_renderer.render(&self.atlas, &self.viewport, pass) {
            log::error!("Text render error: {e:?}");
        }
    }

    /// Trim unused atlas entries after presenting.
    pub fn post_frame(&mut self) {
        self.atlas.trim();
    }
}

impl TextLayoutKey {
    fn new(section: &TextSection, buffer_width: Option<f32>, buffer_height: Option<f32>) -> Self {
        Self {
            content: section.content.clone(),
            font_id: section.font_id.clone(),
            font_size_bits: section.font_size.to_bits(),
            line_height_bits: section.line_height.to_bits(),
            buffer_width_bits: buffer_width.map(f32::to_bits),
            buffer_height_bits: buffer_height.map(f32::to_bits),
        }
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
