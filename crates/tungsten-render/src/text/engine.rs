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

use super::TextSection;
use super::fonts::{ChainFallback, FaceStyle, FamilyNames, FontSource, pick_face};

// Retained nodes live in `text/nodes.rs`, as a child of this module so their
// methods reach the engine's fonts.
#[path = "nodes.rs"]
mod nodes;

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

/// Device-free half of the text pipeline: the font system, the registered
/// faces, the font families and the fallback chain (`D-115`, `D-116`), the
/// shaped section layouts (`D-085`), the signature of the last prepared frame
/// and the retained text nodes that layout measures (`D-117`). It holds no GPU
/// state; [`super::TextPipeline`] owns one and draws what it lays out.
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
    /// Counts prepared frames only: a skipped frame ages nothing.
    frame_counter: u64,
    /// Sections of the last prepared frame, in order.
    prepared: Vec<PreparedSection>,
    /// Size of the last prepared frame. `None` while glyphon's vertices
    /// don't match `prepared`: before the first frame, after a font change
    /// and after a failed prepare.
    prepared_viewport: Option<(u32, u32)>,
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

    pub(crate) fn reload_font(&mut self, id: &str, data: Vec<u8>) {
        if let Some(old) = self.font_attrs.remove(id) {
            let db = self.font_system.db_mut();
            for face_id in old.face_ids {
                db.remove_face(face_id);
            }
        }
        self.register(id, data);
        self.font_changed();
    }

    /// Adds a file's faces to the database under manifest ID `id`; false when
    /// it holds none.
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
        if let Some(other) = self.font_attrs.iter().find(|(other, stored)| {
            other.as_str() != id
                && stored.family == family
                && stored.weight == weight
                && stored.style == style
        }) {
            log::warn!(
                "Fonts '{id}' and '{}' share family \"{family}\", weight and style; text draws one of them for both",
                other.0,
            );
        }
        self.font_attrs.insert(
            id.to_string(),
            StoredFontAttrs {
                family,
                weight,
                style,
                face_ids,
            },
        );
        true
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
    }

    /// Brings the cache up to this frame's sections and shapes the ones it
    /// lacks. Returns `false`, having touched nothing, when the frame equals
    /// the last prepared one.
    pub(crate) fn update(&mut self, sections: &[TextSection], width: u32, height: u32) -> bool {
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
            // Set every field: a recycled buffer keeps its last layout's.
            let layout = &section.layout;
            buffer.set_wrap(cosmic_wrap(layout.wrap));
            buffer.set_ellipsize(cosmic_ellipsize(layout.overflow, buf_h));
            buffer.set_hinting(cosmic_hinting(layout.hinting));
            let attrs = section_attrs(
                &self.font_attrs,
                &self.families,
                &self.chain,
                &section.font_id,
            );
            let attrs = with_layout(attrs, layout);
            buffer.set_text(
                &section.content,
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
        true
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
