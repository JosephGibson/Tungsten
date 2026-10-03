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

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
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

/// A layout no section used for more than this many prepared frames leaves
/// the cache, so the map holds a few entries per live section.
const BUFFER_CACHE_MAX_UNUSED_FRAMES: u64 = 2;
/// Evicted buffers kept for reuse: text that changes every frame shapes into
/// recycled allocations.
const SPARE_BUFFER_CAP: usize = 256;

/// One section of the last prepared frame: its layout key and the draw values
/// glyphon baked into its vertices.
#[derive(Debug)]
struct PreparedSection {
    key: TextLayoutKey,
    position: [f32; 2],
    color: [u8; 4],
    clip: TextBounds,
}

/// Device-free half of the text pipeline: fonts, shaped buffers and the
/// signature of the last prepared frame.
struct TextLayoutCache {
    font_system: FontSystem,
    font_attrs: HashMap<String, StoredFontAttrs>,
    buffers: HashMap<TextLayoutKey, CachedTextBuffer>,
    /// Evicted entries, at most `SPARE_BUFFER_CAP`; a miss reuses their allocations.
    spare: Vec<(TextLayoutKey, Buffer)>,
    /// Counts prepared frames only: a skipped frame ages nothing.
    frame_counter: u64,
    /// Sections of the last prepared frame, in order.
    prepared: Vec<PreparedSection>,
    /// Viewport of the last prepared frame. `None` while glyphon's vertices
    /// don't match `prepared`: before the first frame, after a font change
    /// and after a failed prepare.
    prepared_viewport: Option<(u32, u32)>,
}

/// Glyphon text pipeline state.
pub struct TextPipeline {
    layout: TextLayoutCache,
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
    #[must_use]
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat) -> Self {
        let font_system = FontSystem::new();
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
            layout: TextLayoutCache::new(font_system),
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            prepared_this_frame: false,
        }
    }

    /// Load font bytes under manifest ID.
    pub fn load_font(&mut self, id: &str, data: Vec<u8>) {
        self.layout.load_font(id, data);
    }

    /// Hot-reload font bytes and evict dependent caches.
    pub fn reload_font(&mut self, id: &str, data: Vec<u8>) {
        self.atlas.trim();
        self.layout.reload_font(id, data);
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
        self.prepared_this_frame = self.layout.update(sections, width, height);
        if !self.prepared_this_frame {
            return;
        }

        let areas = text_areas(&self.layout.prepared, &self.layout.buffers);
        match self.text_renderer.prepare(
            device,
            queue,
            &mut self.layout.font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash_cache,
        ) {
            Ok(()) => self.layout.mark_prepared(width, height),
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

impl TextLayoutCache {
    fn new(font_system: FontSystem) -> Self {
        Self {
            font_system,
            font_attrs: HashMap::new(),
            buffers: HashMap::new(),
            spare: Vec::new(),
            frame_counter: 0,
            prepared: Vec::new(),
            prepared_viewport: None,
        }
    }

    fn load_font(&mut self, id: &str, data: Vec<u8>) {
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
        self.clear_layouts();
    }

    fn reload_font(&mut self, id: &str, data: Vec<u8>) {
        if let Some(old) = self.font_attrs.remove(id) {
            let db = self.font_system.db_mut();
            for face_id in old.face_ids {
                db.remove_face(face_id);
            }
        }
        self.clear_layouts();
        self.load_font(id, data);
    }

    /// Drops every shaped buffer and the frame signature, so the next frame
    /// shapes and prepares from scratch.
    fn clear_layouts(&mut self) {
        self.buffers.clear();
        self.spare.clear();
        self.prepared.clear();
        self.prepared_viewport = None;
    }

    /// Brings the cache up to this frame's sections and shapes the ones it
    /// lacks. Returns `false`, having touched nothing, when the frame equals
    /// the last prepared one.
    fn update(&mut self, sections: &[TextSection], width: u32, height: u32) -> bool {
        if self.prepared_viewport == Some((width, height))
            && self.prepared.len() == sections.len()
            && self
                .prepared
                .iter()
                .zip(sections)
                .all(|(prepared, section)| prepared.matches(section, width, height))
        {
            return false;
        }
        self.prepared_viewport = None;
        self.frame_counter += 1;
        let frame = self.frame_counter;

        // Age out before inserting: the map then peaks at the sections of
        // this frame and of the two before it.
        for (key, cached) in self
            .buffers
            .extract_if(|_, cached| frame - cached.last_used_frame > BUFFER_CACHE_MAX_UNUSED_FRAMES)
        {
            if self.spare.len() < SPARE_BUFFER_CAP {
                self.spare.push((key, cached.buffer));
            }
        }

        self.prepared.truncate(sections.len());
        for (index, section) in sections.iter().enumerate() {
            let (buf_w, buf_h) = buffer_size(section, width);
            let clip = clip_bounds_for_section(section, width, height);
            if let Some(prepared) = self.prepared.get_mut(index) {
                prepared.key.assign(section, buf_w, buf_h);
                prepared.position = section.position;
                prepared.color = section.color;
                prepared.clip = clip;
            } else {
                let mut key = TextLayoutKey::default();
                key.assign(section, buf_w, buf_h);
                self.prepared.push(PreparedSection {
                    key,
                    position: section.position,
                    color: section.color,
                    clip,
                });
            }
            let key = &self.prepared[index].key;

            if let Some(cached) = self.buffers.get_mut(key) {
                cached.last_used_frame = frame;
                continue;
            }

            // Never `Buffer::new`: it shapes an empty line first.
            let metrics = Metrics::new(section.font_size, section.line_height);
            let (mut owned_key, mut buffer) = match self.spare.pop() {
                // `set_metrics` rejects a zero font size; `new_empty` takes one.
                Some((spare_key, mut spare_buffer)) if metrics.font_size != 0.0 => {
                    spare_buffer.set_metrics(metrics);
                    (spare_key, spare_buffer)
                }
                _ => (TextLayoutKey::default(), Buffer::new_empty(metrics)),
            };
            buffer.set_size(buf_w, buf_h);
            let attrs = make_attrs(&self.font_attrs, &section.font_id);
            buffer.set_text(&section.content, &attrs, Shaping::Advanced, None);
            buffer.shape_until_scroll(&mut self.font_system, false);
            owned_key.clone_from(key);
            self.buffers.insert(
                owned_key,
                CachedTextBuffer {
                    buffer,
                    last_used_frame: frame,
                },
            );
        }
        true
    }

    /// Records that glyphon prepared the sections of the last `update`.
    fn mark_prepared(&mut self, width: u32, height: u32) {
        self.prepared_viewport = Some((width, height));
    }
}

impl TextLayoutKey {
    /// Overwrites the key in place, keeping its string allocations.
    fn assign(
        &mut self,
        section: &TextSection,
        buffer_width: Option<f32>,
        buffer_height: Option<f32>,
    ) {
        self.content.clone_from(&section.content);
        self.font_id.clone_from(&section.font_id);
        self.font_size_bits = section.font_size.to_bits();
        self.line_height_bits = section.line_height.to_bits();
        self.buffer_width_bits = buffer_width.map(f32::to_bits);
        self.buffer_height_bits = buffer_height.map(f32::to_bits);
    }

    fn matches(
        &self,
        section: &TextSection,
        buffer_width: Option<f32>,
        buffer_height: Option<f32>,
    ) -> bool {
        self.content == section.content
            && self.font_id == section.font_id
            && self.font_size_bits == section.font_size.to_bits()
            && self.line_height_bits == section.line_height.to_bits()
            && self.buffer_width_bits == buffer_width.map(f32::to_bits)
            && self.buffer_height_bits == buffer_height.map(f32::to_bits)
    }
}

impl PreparedSection {
    /// True when `section` lays out and draws exactly as this one did.
    /// Compares in place: nothing is cloned.
    fn matches(&self, section: &TextSection, width: u32, height: u32) -> bool {
        let (buf_w, buf_h) = buffer_size(section, width);
        self.position.map(f32::to_bits) == section.position.map(f32::to_bits)
            && self.color == section.color
            && self.clip == clip_bounds_for_section(section, width, height)
            && self.key.matches(section, buf_w, buf_h)
    }
}

/// Text areas of the prepared sections, in section order.
fn text_areas<'a>(
    prepared: &'a [PreparedSection],
    buffers: &'a HashMap<TextLayoutKey, CachedTextBuffer>,
) -> impl Iterator<Item = TextArea<'a>> {
    prepared.iter().filter_map(|section| {
        let cached = buffers.get(&section.key)?;
        let [r, g, b, a] = section.color;
        Some(TextArea {
            buffer: &cached.buffer,
            left: section.position[0],
            top: section.position[1],
            scale: 1.0,
            bounds: section.clip,
            default_color: Color::rgba(r, g, b, a),
            custom_glyphs: &[],
        })
    })
}

/// Layout size of a section's buffer: its bounds, else the viewport width
/// with no height limit.
fn buffer_size(section: &TextSection, width: u32) -> (Option<f32>, Option<f32>) {
    match section.bounds {
        Some([w, h]) => (Some(w), Some(h)),
        None => (Some(width as f32), None),
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

#[cfg(test)]
#[path = "tests/text.rs"]
mod tests;
