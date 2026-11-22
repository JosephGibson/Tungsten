use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use super::atlas::UvRect;
use super::manifest::FilterMode;
use super::particle::AssetId;

/// GPU texture handle; core never sees `wgpu` types (D-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

/// What a sprite's alpha channel holds (redo 7.15), measured from its
/// decoded pixels when it loads and again when it reloads, so a draw can
/// tell a sprite whose texels hide what lies behind them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpriteAlpha {
    /// Every texel is opaque, alpha 255.
    Opaque,
    /// Every texel is clear or opaque, alpha 0 or 255.
    Cutout,
    /// Some texel lies between, or nothing measured the sprite: one
    /// registered without pixels, as a headless load's are.
    #[default]
    Blend,
}

impl SpriteAlpha {
    /// The class of `rgba`, RGBA8 pixels; no pixels are [`Self::Blend`].
    #[must_use]
    pub fn measure(rgba: &[u8]) -> Self {
        let pixels = rgba.as_chunks::<4>().0;
        // A pixel a word, its alpha the top byte. `all` keeps the bits every
        // alpha shares, 255 only while each is 255. `between` gathers, of
        // each alpha plus one, every bit but the lowest: none for 255, which
        // wraps to 0, none for 0, which becomes 1, and some for any alpha
        // between the two. No branch on the data and no early exit: the
        // loop is word arithmetic the compiler runs four pixels a step, and
        // a load pays it for every sprite (the `boot` row).
        let (mut all, mut between) = (u32::MAX, 0u32);
        for pixel in pixels {
            let word = u32::from_le_bytes(*pixel);
            all &= word;
            between |= word.wrapping_add(0x0100_0000) & 0xFE00_0000;
        }
        if between != 0 || pixels.is_empty() {
            Self::Blend
        } else if all >> 24 == 255 {
            Self::Opaque
        } else {
            Self::Cutout
        }
    }

    /// Whether a quad of the sprite, sampled through `filter` and tinted
    /// with an alpha of 255, draws only clear and opaque fragments: what a
    /// solid batch promises. A linear sampler mixes a cutout's edge texels
    /// into partial alpha, so only a nearest one qualifies.
    #[must_use]
    pub fn solid(self, filter: FilterMode) -> bool {
        self != Self::Blend && filter == FilterMode::Nearest
    }
}

/// Loaded sprite metadata; atlas handle may be shared.
#[derive(Debug, Clone)]
pub struct SpriteAsset {
    pub atlas: TextureHandle,
    pub uv: UvRect,
    pub filter: FilterMode,
    pub width: u32,
    pub height: u32,
    /// Source PNG path for hot reload.
    pub path: PathBuf,
    /// M29 sibling normal-map source PNG path (if registered with one).
    pub normal_path: Option<PathBuf>,
    /// M29 sibling emissive-mask source PNG path (if registered with one).
    pub emissive_path: Option<PathBuf>,
    /// M29 lit atlas marker. `Some(handle)` means the sprite shares its packed
    /// rect with a parallel normal/emissive bundle uploaded under `handle`
    /// (typically equal to `atlas` since lit pages reuse the albedo handle).
    pub lit_atlas: Option<TextureHandle>,
    /// M2 nine-slice insets `[left, top, right, bottom]` in source pixels;
    /// `None` draws the sprite stretched.
    pub nine_slice: Option<[u32; 4]>,
    /// A sheet frame's `[x, y, width, height]` in `path` (redo 7.9); `None`
    /// for a whole file.
    pub rect: Option<[u32; 4]>,
    /// The point drawn at the entity's position and turned about, `[x, y]`
    /// in source pixels; `None` draws from the top-left.
    pub anchor: Option<[f32; 2]>,
    /// Named points, `[x, y]` in source pixels (redo 7.10p), from the
    /// manifest's `points`.
    pub points: BTreeMap<String, [f32; 2]>,
    /// What its alpha channel holds (redo 7.15): [`SpriteAlpha::Blend`]
    /// until a load or a reload measures its pixels.
    pub alpha: SpriteAlpha,
}

impl SpriteAsset {
    /// The named point `name`, `[x, y]` in source pixels, if the sprite
    /// has it; [`sprite_point`](crate::sprite_point) reads where it is
    /// drawn.
    #[must_use]
    pub fn point(&self, name: &str) -> Option<[f32; 2]> {
        self.points.get(name).copied()
    }
}

/// Dense sprite handle, minted by [`AssetRegistry`]; stable for the registry's life.
///
/// An ID means something only in the registry that minted it: another registry
/// reads its own slot at that index, or `None` past its end.
pub type SpriteAssetId = AssetId<SpriteAsset>;

/// D-014 runtime asset registry resource.
///
/// Sprite names are interned to dense [`SpriteAssetId`]s. Interning is
/// append-only, and an interned name reads `None` until it is registered.
#[derive(Debug, Default)]
pub struct AssetRegistry {
    /// Indexed by `SpriteAssetId::index`; `None` until registered.
    sprites: Vec<Option<SpriteAsset>>,
    sprite_names: Vec<String>,
    sprite_id_by_name: HashMap<String, SpriteAssetId>,
    /// Every sprite on a source path, in registration order: a sheet's
    /// frames share one.
    path_to_sprite_id: HashMap<PathBuf, Vec<SpriteAssetId>>,
    /// Sizes and anchors noted for unregistered names; see
    /// [`Self::note_sprite_size`].
    noted: HashMap<SpriteAssetId, NotedSprite>,
    /// Rises on every change to a registered sprite; see [`Self::sprite_epoch`].
    sprite_epoch: u64,
}

/// A sprite's size and anchor noted with no texture.
#[derive(Debug, Clone, Copy)]
struct NotedSprite {
    width: u32,
    height: u32,
    anchor: Option<[f32; 2]>,
}

impl AssetRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns `name`: the same name gets the same ID, registered or not.
    ///
    /// An interned name draws nothing until [`Self::register_sprite`] fills its slot.
    pub fn intern_sprite(&mut self, name: &str) -> SpriteAssetId {
        match self.sprite_id(name) {
            Some(id) => id,
            None => self.intern_new_sprite(name.to_owned()),
        }
    }

    fn intern_new_sprite(&mut self, name: String) -> SpriteAssetId {
        let id = SpriteAssetId::new(self.sprites.len() as u32);
        self.sprites.push(None);
        self.sprite_names.push(name.clone());
        self.sprite_id_by_name.insert(name, id);
        id
    }

    /// Interns `names` in sorted order, so a name set gets the same IDs every run.
    ///
    /// Names already interned keep their IDs; duplicates in `names` are ignored.
    pub fn intern_sprites<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) {
        let mut names: Vec<&str> = names.into_iter().collect();
        names.sort_unstable();
        names.dedup();
        for name in names {
            self.intern_sprite(name);
        }
    }

    /// ID of an interned name; never interns.
    #[must_use]
    pub fn sprite_id(&self, name: &str) -> Option<SpriteAssetId> {
        self.sprite_id_by_name.get(name).copied()
    }

    /// Registered sprite for `id`, by index. `None` for an ID past the end or
    /// one interned and not yet registered. `id` must come from this registry.
    #[must_use]
    pub fn sprite(&self, id: SpriteAssetId) -> Option<&SpriteAsset> {
        self.sprites.get(id.index() as usize)?.as_ref()
    }

    /// Interns `name` and notes its size, `width` × `height` source pixels,
    /// and its anchor, with no texture: a headless run's record of a sprite
    /// it never draws. A noted sprite is not registered: [`Self::sprite`]
    /// and [`Self::get_sprite`] stay `None`, so nothing draws it, and
    /// [`Self::sprite_size`] and [`Self::sprite_anchor`] read the noted
    /// values until it registers, when its own win. Noting again replaces
    /// the note; it moves no [`Self::sprite_epoch`].
    pub fn note_sprite_size(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        anchor: Option<[f32; 2]>,
    ) -> SpriteAssetId {
        let id = self.intern_sprite(name);
        self.noted.insert(
            id,
            NotedSprite {
                width,
                height,
                anchor,
            },
        );
        id
    }

    /// The size of the sprite interned as `id`, `(width, height)` in source
    /// pixels: a registered sprite's own (a sheet frame's, its frame's), else
    /// the size noted for it, else `None`.
    #[must_use]
    pub fn sprite_size(&self, id: SpriteAssetId) -> Option<(u32, u32)> {
        match self.sprite(id) {
            Some(asset) => Some((asset.width, asset.height)),
            None => self.noted.get(&id).map(|noted| (noted.width, noted.height)),
        }
    }

    /// The anchor of the sprite interned as `id`, `[x, y]` in source pixels:
    /// a registered sprite's own, else the anchor noted for it; `None` when
    /// it has none (it draws from its top-left) or nothing is known of it.
    #[must_use]
    pub fn sprite_anchor(&self, id: SpriteAssetId) -> Option<[f32; 2]> {
        match self.sprite(id) {
            Some(asset) => asset.anchor,
            None => self.noted.get(&id).and_then(|noted| noted.anchor),
        }
    }

    /// Name interned as `id`; `None` for an ID past the end.
    #[must_use]
    pub fn sprite_name(&self, id: SpriteAssetId) -> Option<&str> {
        self.sprite_names
            .get(id.index() as usize)
            .map(String::as_str)
    }

    /// Register sprite with renderer-owned atlas handle and UV rect, filling the
    /// slot `id` is interned to (interning it if needed), and return its ID.
    ///
    /// # Panics
    /// Panics on duplicate sprite ID (D-017).
    #[allow(clippy::too_many_arguments)] // stable M22/M29 surface; see D-048
    pub fn register_sprite(
        &mut self,
        id: String,
        filter: FilterMode,
        width: u32,
        height: u32,
        path: PathBuf,
        atlas: TextureHandle,
        uv: UvRect,
        normal_path: Option<PathBuf>,
        emissive_path: Option<PathBuf>,
        lit_atlas: Option<TextureHandle>,
    ) -> SpriteAssetId {
        assert!(
            self.get_sprite(&id).is_none(),
            "duplicate sprite ID '{id}' — each sprite must be registered exactly once"
        );
        let sprite_id = match self.sprite_id(&id) {
            Some(sprite_id) => sprite_id,
            None => self.intern_new_sprite(id),
        };
        for source in std::iter::once(&path)
            .chain(&normal_path)
            .chain(&emissive_path)
        {
            let ids = self.path_to_sprite_id.entry(source.clone()).or_default();
            if !ids.contains(&sprite_id) {
                ids.push(sprite_id);
            }
        }
        self.sprites[sprite_id.index() as usize] = Some(SpriteAsset {
            atlas,
            uv,
            filter,
            width,
            height,
            path,
            normal_path,
            emissive_path,
            lit_atlas,
            nine_slice: None,
            rect: None,
            anchor: None,
            points: BTreeMap::new(),
            alpha: SpriteAlpha::Blend,
        });
        self.sprite_epoch += 1;
        sprite_id
    }

    #[must_use]
    pub fn get_sprite(&self, id: &str) -> Option<&SpriteAsset> {
        self.sprite(self.sprite_id(id)?)
    }

    fn get_sprite_mut(&mut self, id: &str) -> Option<&mut SpriteAsset> {
        let index = self.sprite_id(id)?.index() as usize;
        self.sprites[index].as_mut()
    }

    /// Names of the registered sprites, in ID order; interned-only names are left out.
    pub fn sprite_names(&self) -> impl Iterator<Item = &str> {
        self.sprite_names
            .iter()
            .zip(&self.sprites)
            .filter(|(_, asset)| asset.is_some())
            .map(|(name, _)| name.as_str())
    }

    /// Sprite name for a source path (albedo, normal or emissive): the
    /// first registered on it.
    #[must_use]
    pub fn sprite_name_for_path(&self, path: &Path) -> Option<&str> {
        self.sprite_name(*self.sprite_ids_for_path(path).first()?)
    }

    /// Every sprite registered on a source path (albedo, normal or
    /// emissive), in registration order: each frame of a sheet (redo 7.9).
    #[must_use]
    pub fn sprite_ids_for_path(&self, path: &Path) -> &[SpriteAssetId] {
        self.path_to_sprite_id.get(path).map_or(&[], Vec::as_slice)
    }

    /// Update atlas/UV/dimensions after hot reload. The entry's pixels are
    /// new, so what was measured of the old ones is dropped: the sprite is
    /// unmeasured again until the caller measures it, as the loader does.
    pub fn update_sprite_entry(
        &mut self,
        id: &str,
        atlas: TextureHandle,
        uv: UvRect,
        width: u32,
        height: u32,
    ) {
        if let Some(asset) = self.get_sprite_mut(id) {
            asset.atlas = atlas;
            asset.uv = uv;
            asset.width = width;
            asset.height = height;
            // The new pixels are unmeasured until the loader says.
            asset.alpha = SpriteAlpha::Blend;
            self.sprite_epoch += 1;
        }
    }

    /// Update the M29 lit atlas marker after an atlas (re)build.
    pub fn update_sprite_lit_atlas(&mut self, id: &str, lit_atlas: Option<TextureHandle>) {
        if let Some(asset) = self.get_sprite_mut(id) {
            asset.lit_atlas = lit_atlas;
            self.sprite_epoch += 1;
        }
    }

    /// Sets the M2 nine-slice insets of a registered sprite, as the loader
    /// carries them from the manifest; an unregistered ID does nothing.
    pub fn set_sprite_nine_slice(&mut self, id: &str, nine_slice: Option<[u32; 4]>) {
        if let Some(asset) = self.get_sprite_mut(id) {
            asset.nine_slice = nine_slice;
            self.sprite_epoch += 1;
        }
    }

    /// Sets a registered sprite's sheet frame and anchor (redo 7.9), as the
    /// loader carries them from the manifest; an unregistered ID does
    /// nothing.
    pub fn set_sprite_frame(&mut self, id: &str, rect: Option<[u32; 4]>, anchor: Option<[f32; 2]>) {
        if let Some(asset) = self.get_sprite_mut(id) {
            asset.rect = rect;
            asset.anchor = anchor;
            self.sprite_epoch += 1;
        }
    }

    /// Sets a registered sprite's named points (redo 7.10p), as the loader
    /// carries them from the manifest; an unregistered ID does nothing.
    pub fn set_sprite_points(&mut self, id: &str, points: BTreeMap<String, [f32; 2]>) {
        if let Some(asset) = self.get_sprite_mut(id) {
            asset.points = points;
            self.sprite_epoch += 1;
        }
    }

    /// Sets what a registered sprite's alpha channel holds (redo 7.15), as
    /// the loader measures it from the decoded pixels, at a load and at
    /// every reload; a game that registers a sprite from pixels of its own
    /// sets it with [`SpriteAlpha::measure`]. An unregistered ID, or the
    /// class it already holds, does nothing.
    pub fn set_sprite_alpha(&mut self, id: &str, alpha: SpriteAlpha) {
        if let Some(asset) = self.get_sprite_mut(id)
            && asset.alpha != alpha
        {
            asset.alpha = alpha;
            self.sprite_epoch += 1;
        }
    }

    /// A count that rises whenever a registered sprite changes: on
    /// [`Self::register_sprite`], [`Self::update_sprite_entry`],
    /// [`Self::update_sprite_lit_atlas`], [`Self::set_sprite_nine_slice`],
    /// [`Self::set_sprite_frame`], [`Self::set_sprite_points`] and
    /// [`Self::set_sprite_alpha`], never on interning. A retained UI paint list repaints when it moved,
    /// since a repack may have moved a UV.
    #[must_use]
    pub fn sprite_epoch(&self) -> u64 {
        self.sprite_epoch
    }
}

/// Font path reverse-lookup registry.
#[derive(Debug, Default)]
pub struct FontRegistry {
    path_to_id: HashMap<PathBuf, String>,
}

impl FontRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, id: String, path: PathBuf) {
        self.path_to_id.insert(path, id);
    }

    #[must_use]
    pub fn id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_id.get(path).map(String::as_str)
    }

    #[must_use]
    pub fn contains_id(&self, id: &str) -> bool {
        self.path_to_id.values().any(|v| v == id)
    }
}

#[cfg(test)]
#[path = "../tests/assets/registry.rs"]
mod tests;
