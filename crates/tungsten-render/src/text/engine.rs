use std::collections::{HashMap, HashSet};

use glyphon::cosmic_text::{
    Align, Ellipsize, EllipsizeHeightLimit, FeatureTag, FontFeatures as CosmicFeatures, Hinting,
};
use glyphon::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, TextArea, TextBounds,
    Weight, Wrap, fontdb,
};
use tungsten_core::assets::ResolvedFontFamily;
use tungsten_core::text::{
    EllipsisAt, FontEpoch, FontFeatures, TextAlign, TextHinting, TextLayout, TextOverflow, TextWrap,
};

use super::fonts::{ChainFallback, FaceStyle, FamilyNames, FontSource, pick_face};
use super::{TextSection, WorldLabelRun};

// Retained nodes live in `text/nodes.rs`, as a child of this module so their
// methods reach the engine's fonts.
#[path = "nodes.rs"]
mod nodes;

pub(crate) use nodes::NodeAreas;
pub use nodes::TextNodes;

struct StoredFontAttrs {
    family: String,
    weight: Weight,
    style: Style,
    /// fontdb faces loaded from this manifest entry.
    face_ids: Vec<glyphon::fontdb::ID>,
}

impl StoredFontAttrs {
    fn attrs(&self) -> Attrs<'_> {
        Attrs::new()
            .family(Family::Name(&self.family))
            .weight(self.weight)
            .style(self.style)
    }

    fn face_style(&self) -> FaceStyle {
        FaceStyle {
            weight: self.weight.0,
            italic: self.style != Style::Normal,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
struct TextLayoutKey {
    content: String,
    font_id: String,
    font_size_bits: u32,
    line_height_bits: u32,
    buffer_width_bits: Option<u32>,
    buffer_height_bits: Option<u32>,
    layout: LayoutKey,
}

/// A [`TextLayout`] as a cache key: the float by its bits.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
struct LayoutKey {
    align: TextAlign,
    wrap: TextWrap,
    overflow: TextOverflow,
    hinting: TextHinting,
    letter_spacing_bits: u32,
    features: FontFeatures,
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

/// One world label of the last prepared frame: its layout key, the draw
/// values glyphon baked into its vertices, and the box's top-left that the
/// pivot placed (`D-161`).
#[derive(Debug)]
struct PreparedLabel {
    key: TextLayoutKey,
    position: [f32; 2],
    pivot: [f32; 2],
    scale: f32,
    color: [u8; 4],
    left: f32,
    top: f32,
}

/// Which of a frame's two groups changed: the screen-space sections, the
/// world labels. Each changed group prepares again.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FrameUpdate {
    pub(crate) sections: bool,
    pub(crate) labels: bool,
}

/// What a layout shapes from: a section's or a label's text and style.
struct ShapeInput<'a> {
    content: &'a str,
    font_id: &'a str,
    font_size: f32,
    line_height: f32,
    layout: &'a TextLayout,
    size: (Option<f32>, Option<f32>),
}

/// Which prepared group holds a layout key.
#[derive(Clone, Copy)]
enum Group {
    Section(usize),
    Label(usize),
}

/// Device-free half of the text pipeline: the font system, the registered
/// faces, the font families and the fallback chain (`D-115`, `D-116`), the
/// shaped layouts of the sections and the world labels (`D-085`, `D-161`), the
/// signature of the last prepared frame and the retained text nodes that
/// layout measures (`D-117`). It holds no GPU state;
/// [`super::TextPipeline`] owns one and draws what it lays out.
pub struct TextEngine {
    font_system: FontSystem,
    /// Where faces come from; a rebuilt font system keeps it.
    source: FontSource,
    font_attrs: HashMap<String, StoredFontAttrs>,
    /// Family ID -> the face IDs it groups.
    families: HashMap<String, Vec<String>>,
    /// Family IDs in fallback order.
    chain: Vec<String>,
    /// The fontdb family names the font system's fallback was built with.
    fallback_names: Vec<&'static str>,
    names: FamilyNames,
    epoch: FontEpoch,
    /// Messages already logged once: unknown families, missing styles,
    /// stale node IDs.
    logged: HashSet<String>,
    /// Retained text nodes (`D-117`).
    nodes: nodes::NodeStore,
    buffers: HashMap<TextLayoutKey, CachedTextBuffer>,
    /// Evicted entries, at most `SPARE_BUFFER_CAP`; a miss reuses their allocations.
    spare: Vec<(TextLayoutKey, Buffer)>,
    /// Counts frames in which either group changed: a skipped frame ages
    /// nothing (`D-161`).
    frame_counter: u64,
    /// Sections of the last prepared frame, in order.
    prepared: Vec<PreparedSection>,
    /// Size of the last prepared frame. `None` while glyphon's vertices
    /// don't match `prepared`: before the first frame, after a font change
    /// and after a failed prepare.
    prepared_viewport: Option<(u32, u32)>,
    /// World labels of the last update, in order.
    prepared_labels: Vec<PreparedLabel>,
    /// `prepared_viewport` for the labels.
    labels_viewport: Option<(u32, u32)>,
}

impl TextEngine {
    /// An engine with no faces yet. [`FontSource::Packaged`] starts from an
    /// empty font database with locale `en-US`;
    /// [`FontSource::PackagedThenSystem`] loads the system's faces and locale.
    #[must_use]
    pub fn new(source: FontSource) -> Self {
        match source {
            FontSource::Packaged => {
                Self::with_database(source, "en-US".to_string(), fontdb::Database::new())
            }
            FontSource::PackagedThenSystem => {
                let (locale, db) = FontSystem::new().into_locale_and_db();
                Self::with_database(source, locale, db)
            }
        }
    }

    /// An engine whose database starts as `db`: the system's faces under
    /// [`FontSource::PackagedThenSystem`].
    pub(crate) fn with_database(source: FontSource, locale: String, db: fontdb::Database) -> Self {
        let fallback = ChainFallback::new(&[], source);
        let font_system = FontSystem::new_with_locale_and_db_and_fallback(locale, db, fallback);
        let mut engine = Self::from_font_system(font_system);
        engine.source = source;
        engine
    }

    pub(crate) fn from_font_system(font_system: FontSystem) -> Self {
        Self {
            font_system,
            source: FontSource::Packaged,
            families: HashMap::new(),
            chain: Vec::new(),
            fallback_names: Vec::new(),
            names: FamilyNames::default(),
            epoch: FontEpoch::default(),
            logged: HashSet::new(),
            nodes: nodes::NodeStore::default(),
            font_attrs: HashMap::new(),
            buffers: HashMap::new(),
            spare: Vec::new(),
            frame_counter: 0,
            prepared: Vec::new(),
            prepared_viewport: None,
            prepared_labels: Vec::new(),
            labels_viewport: None,
        }
    }

    /// The count of font changes; layouts made under an older epoch are stale.
    #[must_use]
    pub fn font_epoch(&self) -> FontEpoch {
        self.epoch
    }

    pub(crate) fn load_font(&mut self, id: &str, data: Vec<u8>) {
        if self.register(id, data) {
            self.font_changed();
        }
    }

    /// Replaces `id`'s faces with the data's; false, changing nothing, when
    /// the data holds no face (a partial save): `register` adds the new
    /// faces before it drops the old ones.
    pub(crate) fn reload_font(&mut self, id: &str, data: Vec<u8>) -> bool {
        let registered = self.register(id, data);
        if registered {
            self.font_changed();
        }
        registered
    }

    /// Adds a file's faces to the database under manifest ID `id`; false when
    /// it holds none. Once they are in, the faces `id` had before leave the
    /// database, so a face loaded under a registered ID replaces it
    /// (`D-123`); data with no face leaves the old registration in place.
    fn register(&mut self, id: &str, data: Vec<u8>) -> bool {
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
            return false;
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
        if self.source == FontSource::PackagedThenSystem {
            self.replace_system_faces(&face_ids);
        }
        if let Some(other) = self.clash(id, face_ids[0]) {
            log::warn!(
                "Fonts '{id}' and '{other}' share family \"{family}\", weight and style; text draws one of them for both",
            );
        }
        let earlier = self.font_attrs.insert(
            id.to_string(),
            StoredFontAttrs {
                family,
                weight,
                style,
                face_ids,
            },
        );
        if let Some(earlier) = earlier {
            let db = self.font_system.db_mut();
            for face_id in earlier.face_ids {
                db.remove_face(face_id);
            }
        }
        true
    }

    /// Another registered ID whose face shares `face`'s family, weight and
    /// style but not its bytes: text draws one of the two for both
    /// (`D-116`). Byte-identical faces draw the same glyphs, so they don't
    /// clash (`D-123`).
    fn clash(&self, id: &str, face: fontdb::ID) -> Option<String> {
        let db = self.font_system.db();
        let info = db.face(face)?;
        let family = info.families.first().map(|(name, _)| name.as_str());
        self.font_attrs
            .iter()
            .find(|(other, stored)| {
                other.as_str() != id
                    && Some(stored.family.as_str()) == family
                    && stored.weight == info.weight
                    && stored.style == info.style
                    && stored
                        .face_ids
                        .first()
                        .is_some_and(|&other_face| !same_bytes(db, other_face, face))
            })
            .map(|(other, _)| other.clone())
    }

    /// Removes the system faces that `packaged` faces stand for: same first
    /// family name, weight and style (`D-116`). fontdb would otherwise pick
    /// whichever loaded first, and system faces load first.
    fn replace_system_faces(&mut self, packaged: &[fontdb::ID]) {
        let db = self.font_system.db();
        let registered: HashSet<fontdb::ID> = self
            .font_attrs
            .values()
            .flat_map(|stored| stored.face_ids.iter().copied())
            .chain(packaged.iter().copied())
            .collect();
        let registered = &registered;
        let shadowed: Vec<fontdb::ID> = packaged
            .iter()
            .filter_map(|&id| db.face(id))
            .flat_map(|new| {
                db.faces().filter(move |face| {
                    !registered.contains(&face.id)
                        && face.families.first().map(|(name, _)| name)
                            == new.families.first().map(|(name, _)| name)
                        && face.weight == new.weight
                        && face.style == new.style
                })
            })
            .map(|face| face.id)
            .collect();
        let db = self.font_system.db_mut();
        for id in shadowed {
            db.remove_face(id);
        }
    }

    /// Sets the font families and the fallback chain (`D-115`). Equal input
    /// changes nothing; anything else is a font change.
    pub(crate) fn set_font_families(
        &mut self,
        families: &HashMap<String, ResolvedFontFamily>,
        fallback: &[String],
    ) {
        let families: HashMap<String, Vec<String>> = families
            .iter()
            .map(|(id, family)| (id.clone(), family.faces.clone()))
            .collect();
        if families == self.families && fallback == self.chain.as_slice() {
            return;
        }
        self.families = families;
        self.chain = fallback.to_vec();
        self.font_changed();
    }

    /// The face ID a style draws with (`D-116`): the family's face for
    /// `weight` and `italic`, logged once per family and style when the style
    /// is missing. An unknown family takes the chain's head, logged once per
    /// ID.
    pub(crate) fn resolve_face(
        &mut self,
        family: &str,
        weight: u16,
        italic: bool,
    ) -> Option<String> {
        let family = if self.families.contains_key(family) {
            family.to_string()
        } else {
            if self.logged.insert(format!("family {family}")) {
                log::warn!("Unknown font family '{family}': using the fallback chain's head");
            }
            self.chain.first()?.clone()
        };
        let (face, restyled) =
            family_face(&self.families, &self.font_attrs, &family, weight, italic)?;
        let face = face.to_string();
        if restyled && self.logged.insert(format!("style {family} {italic}")) {
            let style = if italic { "italic" } else { "upright" };
            log::warn!("Font family '{family}' has no {style} face: using another style");
        }
        Some(face)
    }

    /// A new epoch: fallback rebuilt if the chain's names moved, and every
    /// section layout dropped (`D-085`).
    fn font_changed(&mut self) {
        self.epoch = self.epoch.next();
        self.refresh_fallback();
        // Fontdb changes can alter fallback/selection.
        self.clear_layouts();
    }

    /// Rebuilds the font system when the chain's fontdb family names changed.
    /// The database moves across whole, so every fontdb ID, and every glyph
    /// the GPU half cached under one, stays valid.
    fn refresh_fallback(&mut self) {
        let mut names: Vec<&'static str> = Vec::new();
        for family in &self.chain {
            for face in self.families.get(family).into_iter().flatten() {
                if let Some(stored) = self.font_attrs.get(face) {
                    let name = self.names.intern(&stored.family);
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
        if names == self.fallback_names {
            return;
        }
        let empty = FontSystem::new_with_locale_and_db(String::new(), fontdb::Database::new());
        let (locale, db) = std::mem::replace(&mut self.font_system, empty).into_locale_and_db();
        let fallback = ChainFallback::new(&names, self.source);
        self.font_system = FontSystem::new_with_locale_and_db_and_fallback(locale, db, fallback);
        self.fallback_names = names;
    }

    /// Drops every shaped buffer and the frame signature, so the next frame
    /// shapes and prepares from scratch.
    fn clear_layouts(&mut self) {
        self.buffers.clear();
        self.spare.clear();
        self.prepared.clear();
        self.prepared_viewport = None;
        self.prepared_labels.clear();
        self.labels_viewport = None;
    }

    /// [`Self::update_frame`] for sections alone, with no labels.
    #[cfg(test)]
    pub(crate) fn update(&mut self, sections: &[TextSection], width: u32, height: u32) -> bool {
        self.update_frame(sections, &[], width, height).sections
    }

    /// Brings the cache up to this frame's sections and world labels and
    /// shapes the layouts it lacks. A group changed when it differs from the
    /// last prepared one or that prepare did not complete; an empty label
    /// group after an empty one never changed. With neither changed it
    /// returns both false, having touched nothing. Otherwise the frame
    /// counter advances once, layouts unused for more than
    /// `BUFFER_CACHE_MAX_UNUSED_FRAMES` counted frames age out, and every
    /// layout of both groups is touched, so a static group's layouts stay
    /// while the other changes (`D-161`).
    pub(crate) fn update_frame(
        &mut self,
        sections: &[TextSection],
        labels: &[WorldLabelRun],
        width: u32,
        height: u32,
    ) -> FrameUpdate {
        let change = FrameUpdate {
            sections: !(self.prepared_viewport == Some((width, height))
                && self.prepared.len() == sections.len()
                && self
                    .prepared
                    .iter()
                    .zip(sections)
                    .all(|(prepared, section)| prepared.matches(section, width, height))),
            labels: !((labels.is_empty() && self.prepared_labels.is_empty())
                || (self.labels_viewport == Some((width, height))
                    && self.prepared_labels.len() == labels.len()
                    && self
                        .prepared_labels
                        .iter()
                        .zip(labels)
                        .all(|(prepared, label)| prepared.matches(label)))),
        };
        if change == FrameUpdate::default() {
            return change;
        }
        self.frame_counter += 1;
        let frame = self.frame_counter;

        // Age out before inserting: the map then peaks at the layouts of
        // this frame and of the two before it.
        for (key, cached) in self
            .buffers
            .extract_if(|_, cached| frame - cached.last_used_frame > BUFFER_CACHE_MAX_UNUSED_FRAMES)
        {
            if self.spare.len() < SPARE_BUFFER_CAP {
                self.spare.push((key, cached.buffer));
            }
        }

        if change.sections {
            self.prepared_viewport = None;
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
            }
        }
        for (index, section) in sections.iter().enumerate().take(self.prepared.len()) {
            let input = ShapeInput {
                content: &section.content,
                font_id: &section.font_id,
                font_size: section.font_size,
                line_height: section.line_height,
                layout: &section.layout,
                size: buffer_size(section, width),
            };
            self.ensure_layout(Group::Section(index), &input, frame);
        }

        let label_layout = TextLayout::default();
        if change.labels {
            self.labels_viewport = None;
            self.prepared_labels.truncate(labels.len());
            for (index, label) in labels.iter().enumerate() {
                if let Some(prepared) = self.prepared_labels.get_mut(index) {
                    prepared.assign(label, &label_layout);
                } else {
                    let mut key = TextLayoutKey::default();
                    key.assign_label(label, &label_layout);
                    self.prepared_labels.push(PreparedLabel {
                        key,
                        position: label.position,
                        pivot: label.pivot,
                        scale: label.scale,
                        color: label.color,
                        left: 0.0,
                        top: 0.0,
                    });
                }
            }
        }
        for (index, label) in labels.iter().enumerate().take(self.prepared_labels.len()) {
            let input = ShapeInput {
                content: &label.content,
                font_id: &label.font_id,
                font_size: label.font_size,
                line_height: label.line_height,
                layout: &label_layout,
                size: (None, None),
            };
            self.ensure_layout(Group::Label(index), &input, frame);
        }
        if change.labels {
            for prepared in &mut self.prepared_labels {
                let (width, height) = self
                    .buffers
                    .get(&prepared.key)
                    .map_or((0.0, 0.0), |cached| buffer_extent(&cached.buffer));
                prepared.left = prepared.position[0] - width * prepared.scale * prepared.pivot[0];
                prepared.top = prepared.position[1] - height * prepared.scale * prepared.pivot[1];
            }
        }
        change
    }

    /// Marks the layout under `group`'s key used in `frame`, shaping it
    /// from `input` when the map lacks it.
    fn ensure_layout(&mut self, group: Group, input: &ShapeInput<'_>, frame: u64) {
        let key = match group {
            Group::Section(index) => &self.prepared[index].key,
            Group::Label(index) => &self.prepared_labels[index].key,
        };
        if let Some(cached) = self.buffers.get_mut(key) {
            cached.last_used_frame = frame;
            return;
        }

        // Never `Buffer::new`: it shapes an empty line first.
        let metrics = layout_metrics(input.font_size, input.line_height);
        let (mut owned_key, mut buffer) = match self.spare.pop() {
            // `set_metrics` rejects a zero font size; `new_empty` takes one.
            Some((spare_key, mut spare_buffer)) if metrics.font_size != 0.0 => {
                spare_buffer.set_metrics(metrics);
                (spare_key, spare_buffer)
            }
            _ => (TextLayoutKey::default(), Buffer::new_empty(metrics)),
        };
        let (buf_w, buf_h) = input.size;
        buffer.set_size(buf_w, buf_h);
        // Set every field: a recycled buffer keeps its last layout's.
        let layout = input.layout;
        buffer.set_wrap(cosmic_wrap(layout.wrap));
        buffer.set_ellipsize(cosmic_ellipsize(layout.overflow, buf_h));
        buffer.set_hinting(cosmic_hinting(layout.hinting));
        let attrs = section_attrs(&self.font_attrs, &self.families, &self.chain, input.font_id);
        let attrs = with_layout(attrs, layout);
        buffer.set_text(
            input.content,
            &attrs,
            Shaping::Advanced,
            cosmic_align(layout.align),
        );
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

    /// Records that glyphon prepared the sections of the last `update`.
    pub(crate) fn mark_prepared(&mut self, width: u32, height: u32) {
        self.prepared_viewport = Some((width, height));
    }

    /// The font system and the text areas of the last `update`, in section
    /// order, for glyphon's prepare.
    pub(crate) fn prepare_parts(
        &mut self,
    ) -> (&mut FontSystem, impl Iterator<Item = TextArea<'_>>) {
        (
            &mut self.font_system,
            text_areas(&self.prepared, &self.buffers),
        )
    }

    /// Records that glyphon prepared the labels of the last update.
    pub(crate) fn mark_labels_prepared(&mut self, width: u32, height: u32) {
        self.labels_viewport = Some((width, height));
    }

    /// The font system and the text areas of the last update's labels, in
    /// order, bounded by the `width` × `height` viewport, for glyphon's
    /// prepare.
    pub(crate) fn label_prepare_parts(
        &mut self,
        width: u32,
        height: u32,
    ) -> (&mut FontSystem, impl Iterator<Item = TextArea<'_>>) {
        (
            &mut self.font_system,
            label_areas(&self.prepared_labels, &self.buffers, width, height),
        )
    }

    /// Glyphs in the last update's label areas, a label sharing a layout
    /// counted once per label.
    pub(crate) fn label_glyphs(&self) -> u32 {
        self.prepared_labels
            .iter()
            .filter_map(|label| self.buffers.get(&label.key))
            .map(|cached| {
                cached
                    .buffer
                    .layout_runs()
                    .map(|run| run.glyphs.len() as u32)
                    .sum::<u32>()
            })
            .sum()
    }

    /// Layouts in the map, both groups' together.
    pub(crate) fn layout_count(&self) -> usize {
        self.buffers.len()
    }
}

/// The metrics a buffer lays out with. cosmic-text asserts a non-zero line
/// height, so one that is not finite and above zero, as
/// `TextSection::default()`'s 0 is, takes the built-in theme's ratio of 1.25
/// times the font size, and at least 1.
fn layout_metrics(font_size: f32, line_height: f32) -> Metrics {
    let line_height = if line_height.is_finite() && line_height > 0.0 {
        line_height
    } else {
        let scaled = font_size * 1.25;
        if scaled.is_finite() && scaled > 1.0 {
            scaled
        } else {
            1.0
        }
    };
    Metrics::new(font_size, line_height)
}

impl TextLayoutKey {
    /// Overwrites the key with a label's, which lays out unbounded under
    /// `layout`, keeping its string allocations.
    fn assign_label(&mut self, label: &WorldLabelRun, layout: &TextLayout) {
        self.content.clone_from(&label.content);
        self.font_id.clone_from(&label.font_id);
        self.font_size_bits = label.font_size.to_bits();
        self.line_height_bits = label.line_height.to_bits();
        self.buffer_width_bits = None;
        self.buffer_height_bits = None;
        self.layout.assign(layout);
    }

    /// Whether `label` lays out under this key, the layout aside: a label's
    /// is always the default.
    fn matches_label(&self, label: &WorldLabelRun) -> bool {
        self.content == label.content
            && self.font_id == label.font_id
            && self.font_size_bits == label.font_size.to_bits()
            && self.line_height_bits == label.line_height.to_bits()
    }

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
        self.layout.assign(&section.layout);
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
            && self.layout.matches(&section.layout)
    }
}

impl LayoutKey {
    /// Overwrites the key in place, keeping the features' allocation.
    fn assign(&mut self, layout: &TextLayout) {
        self.align = layout.align;
        self.wrap = layout.wrap;
        self.overflow = layout.overflow;
        self.hinting = layout.hinting;
        self.letter_spacing_bits = layout.letter_spacing.to_bits();
        self.features.clone_from(&layout.features);
    }

    fn matches(&self, layout: &TextLayout) -> bool {
        self.align == layout.align
            && self.wrap == layout.wrap
            && self.overflow == layout.overflow
            && self.hinting == layout.hinting
            && self.letter_spacing_bits == layout.letter_spacing.to_bits()
            && self.features == layout.features
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

impl PreparedLabel {
    /// Overwrites the label in place; `left` and `top` follow its layout.
    fn assign(&mut self, label: &WorldLabelRun, layout: &TextLayout) {
        self.key.assign_label(label, layout);
        self.position = label.position;
        self.pivot = label.pivot;
        self.scale = label.scale;
        self.color = label.color;
    }

    /// True when `label` lays out and draws exactly as this one did.
    fn matches(&self, label: &WorldLabelRun) -> bool {
        self.position.map(f32::to_bits) == label.position.map(f32::to_bits)
            && self.pivot.map(f32::to_bits) == label.pivot.map(f32::to_bits)
            && self.scale.to_bits() == label.scale.to_bits()
            && self.color == label.color
            && self.key.matches_label(label)
    }
}

/// Text areas of the prepared labels, in order: each at its box's top-left
/// and scale, bounded by the viewport.
fn label_areas<'a>(
    prepared: &'a [PreparedLabel],
    buffers: &'a HashMap<TextLayoutKey, CachedTextBuffer>,
    width: u32,
    height: u32,
) -> impl Iterator<Item = TextArea<'a>> {
    let bounds = TextBounds {
        left: 0,
        top: 0,
        right: width as i32,
        bottom: height as i32,
    };
    prepared.iter().filter_map(move |label| {
        let cached = buffers.get(&label.key)?;
        let [r, g, b, a] = label.color;
        Some(TextArea {
            buffer: &cached.buffer,
            left: label.left,
            top: label.top,
            scale: label.scale,
            bounds,
            default_color: Color::rgba(r, g, b, a),
            custom_glyphs: &[],
        })
    })
}

/// The laid-out box of an unbounded buffer: its widest line and the bottom
/// of its last line, before any scale.
fn buffer_extent(buffer: &Buffer) -> (f32, f32) {
    buffer
        .layout_runs()
        .fold((0.0_f32, 0.0_f32), |(width, height), run| {
            (
                width.max(run.line_w),
                height.max(run.line_top + run.line_height),
            )
        })
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

/// cosmic-text's alignment: `None` is the line's start.
fn cosmic_align(align: TextAlign) -> Option<Align> {
    match align {
        TextAlign::Center => Some(Align::Center),
        TextAlign::End => Some(Align::End),
        TextAlign::Justified => Some(Align::Justified),
        _ => None,
    }
}

fn cosmic_wrap(wrap: TextWrap) -> Wrap {
    match wrap {
        TextWrap::None => Wrap::None,
        TextWrap::Glyph => Wrap::Glyph,
        TextWrap::Word => Wrap::Word,
        _ => Wrap::WordOrGlyph,
    }
}

/// cosmic-text's ellipsis: `lines: None` keeps the lines that fit `height`,
/// all of them when the height is unbounded.
fn cosmic_ellipsize(overflow: TextOverflow, height: Option<f32>) -> Ellipsize {
    let TextOverflow::Ellipsis { at, lines } = overflow else {
        return Ellipsize::None;
    };
    let limit = match lines {
        Some(lines) => EllipsizeHeightLimit::Lines(lines as usize),
        None => EllipsizeHeightLimit::Height(height.unwrap_or(f32::INFINITY)),
    };
    match at {
        EllipsisAt::Start => Ellipsize::Start(limit),
        EllipsisAt::Middle => Ellipsize::Middle(limit),
        _ => Ellipsize::End(limit),
    }
}

fn cosmic_hinting(hinting: TextHinting) -> Hinting {
    match hinting {
        TextHinting::Enabled => Hinting::Enabled,
        _ => Hinting::Disabled,
    }
}

/// `attrs` with the layout's letter spacing and features; a default layout
/// leaves them as they are.
fn with_layout<'a>(mut attrs: Attrs<'a>, layout: &TextLayout) -> Attrs<'a> {
    if layout.letter_spacing != 0.0 {
        attrs = attrs.letter_spacing(layout.letter_spacing);
    }
    if !layout.features.is_empty() {
        let mut features = CosmicFeatures::new();
        for feature in layout.features.as_slice() {
            features.set(FeatureTag::new(&feature.tag), u32::from(feature.value));
        }
        attrs = attrs.font_features(features);
    }
    attrs
}

/// The attrs of a registered face; generic sans-serif for any other ID.
fn make_attrs<'a>(font_attrs: &'a HashMap<String, StoredFontAttrs>, font_id: &str) -> Attrs<'a> {
    font_attrs.get(font_id).map_or_else(
        || Attrs::new().family(Family::SansSerif),
        StoredFontAttrs::attrs,
    )
}

/// A section's attrs: its face's, else, warned at each shaping, the regular
/// face of the chain's head (`D-116`).
fn section_attrs<'a>(
    font_attrs: &'a HashMap<String, StoredFontAttrs>,
    families: &HashMap<String, Vec<String>>,
    chain: &[String],
    font_id: &str,
) -> Attrs<'a> {
    if font_attrs.contains_key(font_id) {
        return make_attrs(font_attrs, font_id);
    }
    log::warn!("Unknown font ID '{font_id}', falling back to the fallback chain's head");
    let head = chain
        .first()
        .and_then(|head| family_face(families, font_attrs, head, 400, false))
        .map(|(face, _)| face);
    make_attrs(font_attrs, head.unwrap_or(font_id))
}

/// Whether two faces come from the same bytes: the same font data and face
/// index.
fn same_bytes(db: &fontdb::Database, a: fontdb::ID, b: fontdb::ID) -> bool {
    db.with_face_data(a, |a_data, a_index| {
        db.with_face_data(b, |b_data, b_index| a_index == b_index && a_data == b_data)
    })
    .flatten()
    .unwrap_or(false)
}

/// The registered face of `family` for `weight` and `italic`, and whether the
/// style had to change; `None` for an unknown or unloaded family.
fn family_face<'a>(
    families: &'a HashMap<String, Vec<String>>,
    font_attrs: &HashMap<String, StoredFontAttrs>,
    family: &str,
    weight: u16,
    italic: bool,
) -> Option<(&'a str, bool)> {
    let loaded: Vec<(&'a str, FaceStyle)> = families
        .get(family)?
        .iter()
        .filter_map(|face| Some((face.as_str(), font_attrs.get(face)?.face_style())))
        .collect();
    let styles: Vec<FaceStyle> = loaded.iter().map(|&(_, style)| style).collect();
    let (index, restyled) = pick_face(&styles, weight, italic)?;
    Some((loaded[index].0, restyled))
}

#[cfg(test)]
#[path = "../tests/text.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/text_fonts.rs"]
mod font_tests;
