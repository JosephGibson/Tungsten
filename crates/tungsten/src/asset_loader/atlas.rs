//! Sprite atlases: decode, pack and upload at load, in-place hot reload of a
//! sprite that fits its packed cell, and repack of one filter class. A file
//! decodes once however many sprites it backs: a sheet's frames are cut from
//! it as their own images would be (redo 7.9).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use tungsten_core::assets::manifest::ResolvedSprite;
use tungsten_core::assets::{
    AtlasPage, FilterMode, PackInput, PackResult, PackedSprite, ResolvedManifest, TextureHandle,
    UvRect, oversized, pack_shelf,
};
use tungsten_core::{AssetRegistry, World};
use tungsten_render::Renderer;

use super::LoadTarget;

/// Live atlas pages plus sprite packed rects for hot reload.
#[derive(Debug, Default)]
pub struct AtlasRegistry {
    pub nearest_pages: Vec<TextureHandle>,
    pub linear_pages: Vec<TextureHandle>,
    /// Pixel size of each live page; an in-place reload derives its UV from it.
    pub page_sizes: HashMap<TextureHandle, AtlasPage>,
    pub packed: HashMap<String, PackedSprite>,
}

impl AtlasRegistry {
    #[must_use]
    pub fn page_handles(&self, filter: FilterMode) -> &[TextureHandle] {
        match filter {
            FilterMode::Nearest => &self.nearest_pages,
            FilterMode::Linear => &self.linear_pages,
        }
    }

    pub fn page_handles_mut(&mut self, filter: FilterMode) -> &mut Vec<TextureHandle> {
        match filter {
            FilterMode::Nearest => &mut self.nearest_pages,
            FilterMode::Linear => &mut self.linear_pages,
        }
    }
}

/// Decoded CPU-side sprite awaiting atlas packing.
pub(super) struct Decoded {
    pub(super) id: String,
    path: PathBuf,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    /// M29 optional sibling normal-map path (for hot-reload + load-side decode).
    normal_path: Option<PathBuf>,
    /// M29 optional sibling emissive-mask path.
    emissive_path: Option<PathBuf>,
    /// M29 decoded sibling normal-map RGBA, dimensions == albedo.
    normal_rgba: Option<Vec<u8>>,
    /// M29 decoded sibling emissive RGB-promoted to RGBA, dimensions == albedo.
    emissive_rgba: Option<Vec<u8>>,
    /// M46 nine-slice insets from the manifest entry; `None` on a repack,
    /// which keeps the registered ones.
    nine_slice: Option<[u32; 4]>,
    /// Redo 7.9: the sheet frame and anchor from the manifest entry,
    /// registered with the sprite; `None` on a repack, as `nine_slice`.
    rect: Option<[u32; 4]>,
    anchor: Option<[f32; 2]>,
    /// Redo 7.10p: the named points from the manifest entry; empty on a
    /// repack, which keeps the registered ones.
    points: BTreeMap<String, [f32; 2]>,
}

impl Decoded {
    /// Its named points, as the packed sprite registers them.
    #[cfg(test)]
    pub(super) fn points(&self) -> &BTreeMap<String, [f32; 2]> {
        &self.points
    }
}

/// One image file decoded with its siblings; every sprite on it cuts its
/// pixels from it (redo 7.9).
pub(super) struct DecodedFile {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    /// The normal map, when it decoded at the image's size.
    normal: Option<Vec<u8>>,
    /// The emissive mask premultiplied into opaque RGBA, likewise.
    emissive: Option<Vec<u8>>,
}

/// A sprite's pixels, cut from its file: the image and the siblings that
/// decoded.
pub(super) struct Cut {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) rgba: Vec<u8>,
    pub(super) normal: Option<Vec<u8>>,
    pub(super) emissive: Option<Vec<u8>>,
}

impl DecodedFile {
    /// Decodes the image at `path` and its siblings; a sibling that fails
    /// goes to `report` with its manifest field, as in
    /// [`decode_sprite_siblings`], and comes back `None`.
    pub(super) fn open(
        path: &Path,
        normal: Option<&Path>,
        emissive: Option<&Path>,
        report: impl FnMut(&str, String),
    ) -> Result<Self, image::ImageError> {
        let image = image::open(path)?.to_rgba8();
        let (width, height) = image.dimensions();
        let siblings = decode_sprite_siblings(normal, emissive, (width, height), report);
        Ok(Self {
            width,
            height,
            rgba: image.into_raw(),
            normal: siblings.normal,
            emissive: siblings.emissive,
        })
    }

    /// The pixels `rect` (`[x, y, width, height]`) covers, or the whole
    /// file's for `None`; `None` when the rect leaves the image.
    pub(super) fn cut(&self, rect: Option<[u32; 4]>) -> Option<Cut> {
        let [x, y, width, height] = rect.unwrap_or([0, 0, self.width, self.height]);
        if x.checked_add(width)? > self.width || y.checked_add(height)? > self.height {
            return None;
        }
        let crop = |src: &[u8]| {
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for row in y..y + height {
                let start = (row as usize * self.width as usize + x as usize) * 4;
                out.extend_from_slice(&src[start..start + width as usize * 4]);
            }
            out
        };
        Some(Cut {
            width,
            height,
            rgba: crop(&self.rgba),
            normal: self.normal.as_deref().map(crop),
            emissive: self.emissive.as_deref().map(crop),
        })
    }

    /// The whole file as one sprite's pixels, without a copy.
    fn into_cut(self) -> Cut {
        Cut {
            width: self.width,
            height: self.height,
            rgba: self.rgba,
            normal: self.normal,
            emissive: self.emissive,
        }
    }
}

/// The sprites of `manifest` by the file they decode from, an image and its
/// siblings: each file's sprites by ID, the files by path, so a load decodes
/// each once, in one order every run.
pub(super) fn sprites_by_file(manifest: &ResolvedManifest) -> Vec<Vec<(&String, &ResolvedSprite)>> {
    let mut files: HashMap<(&Path, Option<&Path>, Option<&Path>), Vec<_>> = HashMap::new();
    for (id, sprite) in &manifest.sprites {
        let key = (
            sprite.path.as_path(),
            sprite.normal_path.as_deref(),
            sprite.emissive_path.as_deref(),
        );
        files.entry(key).or_default().push((id, sprite));
    }
    let mut files: Vec<_> = files.into_iter().collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
        .into_iter()
        .map(|(_, mut sprites)| {
            sprites.sort_by(|a, b| a.0.cmp(b.0));
            sprites
        })
        .collect()
}

/// One file's sprites decoded: the file once, each sprite cut from it.
pub(super) fn decode_file_sprites(
    sprites: &[(&String, &ResolvedSprite)],
) -> anyhow::Result<Vec<Decoded>> {
    let (first_id, first) = sprites[0];
    let file = DecodedFile::open(
        &first.path,
        first.normal_path.as_deref(),
        first.emissive_path.as_deref(),
        |field, why| log::error!("sprite '{first_id}' {field} {why}; sprite stays unlit"),
    )
    .map_err(|e| anyhow::anyhow!("Failed to decode '{}': {}", first.path.display(), e))?;
    // M29: emissive without normal stays unlit; lit-path requires a normal.
    let lit = file.normal.is_some();
    let decoded = |id: &String, sprite: &ResolvedSprite, cut: Cut| Decoded {
        id: id.clone(),
        path: sprite.path.clone(),
        width: cut.width,
        height: cut.height,
        rgba: cut.rgba,
        normal_path: if lit {
            sprite.normal_path.clone()
        } else {
            None
        },
        emissive_path: if lit {
            sprite.emissive_path.clone()
        } else {
            None
        },
        emissive_rgba: if lit { cut.emissive } else { None },
        normal_rgba: cut.normal,
        nine_slice: sprite.nine_slice,
        rect: sprite.rect,
        anchor: sprite.anchor,
        points: sprite.points.clone(),
    };
    if let [(id, sprite)] = sprites
        && sprite.rect.is_none()
    {
        return Ok(vec![decoded(id, sprite, file.into_cut())]);
    }
    sprites
        .iter()
        .map(|&(id, sprite)| {
            let cut = file.cut(sprite.rect).ok_or_else(|| {
                anyhow::anyhow!(
                    "Sprite '{id}''s frame {:?} leaves '{}' ({}x{})",
                    sprite.rect,
                    sprite.path.display(),
                    file.width,
                    file.height
                )
            })?;
            Ok(decoded(id, sprite, cut))
        })
        .collect()
}

/// Every sprite of `manifest` decoded, each file once, by filter: nearest,
/// then linear.
pub(super) fn decode_sprites(manifest: &ResolvedManifest) -> anyhow::Result<[Vec<Decoded>; 2]> {
    let mut by_filter: [Vec<Decoded>; 2] = Default::default();
    for sprites in sprites_by_file(manifest) {
        for entry in decode_file_sprites(&sprites)? {
            let filter = manifest.sprites[&entry.id].filter;
            by_filter[usize::from(filter == FilterMode::Linear)].push(entry);
        }
    }
    Ok(by_filter)
}

/// Decoded M29 siblings of one sprite: the normal map as RGBA and the
/// emissive mask premultiplied by its alpha into opaque RGBA.
#[derive(Default)]
pub(crate) struct SpriteSiblings {
    normal: Option<Vec<u8>>,
    emissive: Option<Vec<u8>>,
}

/// Decode the normal-map and emissive-mask siblings of an albedo of `size`. A
/// sibling that fails to decode or has another size comes back `None`, and
/// `report` gets its manifest field (`normal_map`, `emissive_mask`) and why.
pub(crate) fn decode_sprite_siblings(
    normal_path: Option<&Path>,
    emissive_path: Option<&Path>,
    size: (u32, u32),
    mut report: impl FnMut(&str, String),
) -> SpriteSiblings {
    let mut decode = |field: &str, path: Option<&Path>| {
        let path = path?;
        match image::open(path) {
            Ok(img) => {
                let img = img.to_rgba8();
                if img.dimensions() == size {
                    Some(img.into_raw())
                } else {
                    report(
                        field,
                        format!("dimensions {:?} != albedo {size:?}", img.dimensions()),
                    );
                    None
                }
            }
            Err(e) => {
                report(field, format!("decode failed at '{}': {e}", path.display()));
                None
            }
        }
    };
    let normal = decode("normal_map", normal_path);
    let emissive = decode("emissive_mask", emissive_path).map(|raw| {
        let mut rgb = Vec::with_capacity(raw.len());
        for px in raw.as_chunks::<4>().0 {
            let alpha = px[3] as f32 / 255.0;
            let red = (px[0] as f32 * alpha) as u8;
            let green = (px[1] as f32 * alpha) as u8;
            let blue = (px[2] as f32 * alpha) as u8;
            rgb.extend_from_slice(&[red, green, blue, 255]);
        }
        rgb
    });
    SpriteSiblings { normal, emissive }
}

/// Flat-normal RGBA canvas of `texels` texels: tangent-space (0.5, 0.5, 1.0).
fn flat_normal_canvas(texels: usize) -> Vec<u8> {
    [128, 128, 255, 255].repeat(texels)
}

/// Copy `height` rows of `width` RGBA texels from the tightly packed `src`
/// into `dst`, a canvas `dst_width` texels wide, with the first texel at
/// (`x`, `y`).
fn blit_rows(
    dst: &mut [u8],
    dst_width: usize,
    (x, y): (usize, usize),
    src: &[u8],
    width: usize,
    height: usize,
) {
    let stride = width * 4;
    for row in 0..height {
        let dst_start = ((y + row) * dst_width + x) * 4;
        dst[dst_start..dst_start + stride].copy_from_slice(&src[row * stride..(row + 1) * stride]);
    }
}

/// RGBA canvases for the pages of a pack: albedo, and when any sprite is lit,
/// flat-normal and zero-emissive canvases holding the lit siblings.
pub(super) struct PageCanvases {
    pub(super) albedo: Vec<Vec<u8>>,
    /// Empty when no sprite of the pack is lit; so is `emissive`.
    normal: Vec<Vec<u8>>,
    emissive: Vec<Vec<u8>>,
}

/// Blit every packed sprite of `pack` into its page canvases.
fn blit_pages(pack: &PackResult, by_id: &HashMap<&str, &Decoded>, any_lit: bool) -> PageCanvases {
    let texels = |page: &AtlasPage| (page.width as usize) * (page.height as usize);
    let mut canvases = PageCanvases {
        albedo: pack
            .pages
            .iter()
            .map(|p| vec![0u8; texels(p) * 4])
            .collect(),
        normal: Vec::new(),
        emissive: Vec::new(),
    };
    if any_lit {
        canvases.normal = pack
            .pages
            .iter()
            .map(|p| flat_normal_canvas(texels(p)))
            .collect();
        canvases.emissive = pack
            .pages
            .iter()
            .map(|p| vec![0u8; texels(p) * 4])
            .collect();
    }
    for packed in &pack.sprites {
        let src = by_id[packed.id.as_str()];
        let page = packed.page as usize;
        let page_width = pack.pages[page].width as usize;
        let at = (packed.x as usize, packed.y as usize);
        let (width, height) = (packed.width as usize, packed.height as usize);
        blit_rows(
            &mut canvases.albedo[page],
            page_width,
            at,
            &src.rgba,
            width,
            height,
        );
        if let Some(normal) = &src.normal_rgba {
            blit_rows(
                &mut canvases.normal[page],
                page_width,
                at,
                normal,
                width,
                height,
            );
        }
        if let Some(emissive) = &src.emissive_rgba {
            blit_rows(
                &mut canvases.emissive[page],
                page_width,
                at,
                emissive,
                width,
                height,
            );
        }
    }
    canvases
}

/// Upload each page canvas under its handle, plus the lit bundle when the
/// pack has lit sprites.
fn upload_pages(
    target: &mut dyn LoadTarget,
    handles: &[TextureHandle],
    pack: &PackResult,
    canvases: &PageCanvases,
    filter: FilterMode,
) {
    for (index, page) in pack.pages.iter().enumerate() {
        let albedo = &canvases.albedo[index];
        target.upload_texture(handles[index], albedo, page.width, page.height, filter);
        if !canvases.normal.is_empty() {
            target.upload_lit_texture(
                handles[index],
                albedo,
                &canvases.normal[index],
                &canvases.emissive[index],
                page.width,
                page.height,
                filter,
            );
        }
    }
}

/// The pack inputs of `decoded`, and why they cannot pack when one is larger
/// than a page of `max_dim` with the 1 px padding holds.
fn pack_inputs(decoded: &[Decoded], max_dim: u32) -> (Vec<PackInput<'_>>, Option<String>) {
    let inputs: Vec<PackInput<'_>> = decoded
        .iter()
        .map(|d| PackInput {
            id: d.id.as_str(),
            width: d.width,
            height: d.height,
        })
        .collect();
    let too_large = oversized(&inputs, max_dim, 1).map(|input| {
        let side = max_dim.saturating_sub(2);
        format!(
            "sprite '{}' ({}x{}) is larger than an atlas page holds ({side}x{side} px)",
            input.id, input.width, input.height
        )
    });
    (inputs, too_large)
}

/// UV of a packed rect with the half-texel inset; assumes no mipmaps.
fn packed_uv(packed: &PackedSprite, page: AtlasPage) -> UvRect {
    let (pw, ph) = (page.width as f32, page.height as f32);
    UvRect {
        min: [(packed.x as f32 + 0.5) / pw, (packed.y as f32 + 0.5) / ph],
        max: [
            (packed.x as f32 + packed.width as f32 - 0.5) / pw,
            (packed.y as f32 + packed.height as f32 - 0.5) / ph,
        ],
    }
}

/// Registry entry of a sprite reloaded in place into its packed `cell` on a
/// page of size `page`: the UV and size the extract draws with. The image
/// sits at the cell's top-left, so the UV spans that `new_w × new_h` rect with
/// the half-texel inset of a fresh pack, and UV and size stay together (B3).
pub(super) fn shrink_entry(
    cell: &PackedSprite,
    page: AtlasPage,
    new_w: u32,
    new_h: u32,
) -> (UvRect, u32, u32) {
    let image = PackedSprite {
        width: new_w,
        height: new_h,
        ..cell.clone()
    };
    (packed_uv(&image, page), new_w, new_h)
}

/// The CPU half of one filter class's pack: `decoded` shelf-packed, and
/// each page's canvases with every sprite blitted in.
pub(super) fn pack_pages(decoded: &[Decoded], max_dim: u32) -> (PackResult, PageCanvases) {
    let (inputs, _) = pack_inputs(decoded, max_dim);
    let pack = pack_shelf(&inputs, max_dim, 1);
    let by_id: HashMap<&str, &Decoded> = decoded.iter().map(|d| (d.id.as_str(), d)).collect();
    let any_lit = decoded.iter().any(|d| d.normal_rgba.is_some());
    let canvases = blit_pages(&pack, &by_id, any_lit);
    (pack, canvases)
}

/// Build one filter-class atlas; half-texel UV inset assumes no mipmaps.
fn build_atlas_for_filter(
    filter: FilterMode,
    decoded: &[Decoded],
    target: &mut dyn LoadTarget,
    registry: &mut AssetRegistry,
    atlas_registry: &mut AtlasRegistry,
    max_dim: u32,
) -> Vec<TextureHandle> {
    if decoded.is_empty() {
        atlas_registry.page_handles_mut(filter).clear();
        return Vec::new();
    }

    // `load_sprites` refused oversized sprites before calling this.
    let (pack, canvases) = pack_pages(decoded, max_dim);
    let by_id: HashMap<&str, &Decoded> = decoded.iter().map(|d| (d.id.as_str(), d)).collect();
    let page_handles: Vec<TextureHandle> = pack
        .pages
        .iter()
        .map(|_| target.allocate_texture_handle())
        .collect();
    upload_pages(target, &page_handles, &pack, &canvases, filter);

    for packed in &pack.sprites {
        let src = by_id[packed.id.as_str()];
        let uv = packed_uv(packed, pack.pages[packed.page as usize]);
        let atlas = page_handles[packed.page as usize];
        let lit_atlas = if src.normal_path.is_some() {
            Some(atlas)
        } else {
            None
        };
        registry.register_sprite(
            src.id.clone(),
            filter,
            src.width,
            src.height,
            src.path.clone(),
            atlas,
            uv,
            src.normal_path.clone(),
            src.emissive_path.clone(),
            lit_atlas,
        );
        // M46: the manifest's nine-slice insets, carried onto the asset.
        if src.nine_slice.is_some() {
            registry.set_sprite_nine_slice(&src.id, src.nine_slice);
        }
        // Redo 7.9: a sheet frame's place and the anchor.
        if src.rect.is_some() || src.anchor.is_some() {
            registry.set_sprite_frame(&src.id, src.rect, src.anchor);
        }
        // Redo 7.10p: its named points.
        if !src.points.is_empty() {
            registry.set_sprite_points(&src.id, src.points.clone());
        }
        atlas_registry.packed.insert(src.id.clone(), packed.clone());
    }

    for (handle, page) in page_handles.iter().zip(&pack.pages) {
        atlas_registry.page_sizes.insert(*handle, *page);
    }
    atlas_registry
        .page_handles_mut(filter)
        .clone_from(&page_handles);
    page_handles
}

/// Load sprites into filter-class atlases and update `AtlasRegistry`.
pub fn load_sprites(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let [decoded_nearest, decoded_linear] = decode_sprites(manifest)?;
    let max_dim = renderer.max_2d_texture_dimension();
    for decoded in [&decoded_nearest, &decoded_linear] {
        if let (_, Some(why)) = pack_inputs(decoded, max_dim) {
            anyhow::bail!("Failed to pack sprites: {why}");
        }
    }
    // Sorted-name interning gives a manifest the same sprite IDs every run;
    // the packer registers in `HashMap` order.
    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .intern_sprites(manifest.sprites.keys().map(String::as_str));
    let nearest = pack_filter(FilterMode::Nearest, &decoded_nearest, world, renderer)?;
    let linear = pack_filter(FilterMode::Linear, &decoded_linear, world, renderer)?;
    log::info!(
        "Packed {} sprites → {} atlas pages ({nearest} nearest + {linear} linear)",
        decoded_nearest.len() + decoded_linear.len(),
        nearest + linear,
    );
    Ok(())
}

/// One filter class's decoded sprites packed, uploaded and registered on
/// `AtlasRegistry`, refused when one is larger than a page; their names
/// must be interned. The pages' count. A load unit (redo 7.9).
pub(super) fn pack_filter(
    filter: FilterMode,
    decoded: &[Decoded],
    world: &mut World,
    target: &mut dyn LoadTarget,
) -> anyhow::Result<usize> {
    let max_dim = target.max_2d_texture_dimension();
    if let (_, Some(why)) = pack_inputs(decoded, max_dim) {
        anyhow::bail!("Failed to pack sprites: {why}");
    }
    // Borrow split: `AtlasRegistry` plus `AssetRegistry`.
    let mut atlas_registry = world
        .get_resource_mut::<AtlasRegistry>()
        .map(std::mem::take)
        .unwrap_or_default();
    let pages = {
        let registry = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        build_atlas_for_filter(
            filter,
            decoded,
            target,
            registry,
            &mut atlas_registry,
            max_dim,
        )
        .len()
    };
    world.insert_resource(atlas_registry);
    Ok(pages)
}

/// Hot-reload sprite; D-031 requires between-frame drain before handle drops.
///
/// `path` may point to the albedo PNG, the M29 normal-map sibling, or the
/// emissive-mask sibling. Every sprite on the file reloads with `id`, as
/// [`reload_sprite_file`] reloads them; `filter` is `id`'s, kept for
/// callers.
pub fn reload_sprite(
    id: &str,
    path: &Path,
    filter: FilterMode,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    log::trace!(
        "reload_sprite '{id}' ({filter:?}) triggered by '{}'",
        path.display()
    );
    reload_sprite_file(path, world, renderer)
}

/// What an in-place reload of one sprite needs from the registries.
struct ReloadTarget {
    id: String,
    filter: FilterMode,
    atlas: TextureHandle,
    size: (u32, u32),
    rect: Option<[u32; 4]>,
    cell: Option<(PackedSprite, AtlasPage)>,
}

/// Hot-reloads every sprite on the file `path` (an albedo, normal map or
/// emissive mask) from one decode (redo 7.9): each sprite, or sheet frame,
/// that still fits its packed cell is written in place; when one grew, its
/// filter class repacks. The registry's metadata decides which siblings
/// decode, so the lit bundle stays consistent; a failed decode or an
/// invalid lit sibling keeps the previous pixels.
pub fn reload_sprite_file(
    path: &Path,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let (targets, albedo_path, normal_path, emissive_path, had_lit) = {
        let asset_reg = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        let atlas_reg = world.get_resource::<AtlasRegistry>();
        let mut targets = Vec::new();
        let mut files = None;
        for &sprite_id in asset_reg.sprite_ids_for_path(path) {
            let (Some(name), Some(asset)) = (
                asset_reg.sprite_name(sprite_id),
                asset_reg.sprite(sprite_id),
            ) else {
                continue;
            };
            files.get_or_insert_with(|| {
                (
                    asset.path.clone(),
                    asset.normal_path.clone(),
                    asset.emissive_path.clone(),
                    asset.lit_atlas.is_some(),
                )
            });
            targets.push(ReloadTarget {
                id: name.to_string(),
                filter: asset.filter,
                atlas: asset.atlas,
                size: (asset.width, asset.height),
                rect: asset.rect,
                cell: atlas_reg.and_then(|ar| {
                    Some((
                        ar.packed.get(name)?.clone(),
                        *ar.page_sizes.get(&asset.atlas)?,
                    ))
                }),
            });
        }
        let Some((albedo, normal, emissive, lit)) = files else {
            log::error!(
                "Hot reload sprite file '{}': no sprite registered",
                path.display()
            );
            return Ok(());
        };
        (targets, albedo, normal, emissive, lit)
    };
    let names = || {
        targets
            .iter()
            .map(|target| target.id.as_str())
            .collect::<Vec<_>>()
            .join("', '")
    };

    let rects: Vec<Option<[u32; 4]>> = targets.iter().map(|target| target.rect).collect();
    let (normal, emissive) = if had_lit {
        (normal_path.as_deref(), emissive_path.as_deref())
    } else {
        (None, None)
    };
    let cuts = match reload_cuts(&albedo_path, normal, emissive, &rects, |field, why| {
        log::error!("Hot reload sprite '{}': {field} {why}", names());
    }) {
        Ok(cuts) => cuts,
        Err(e) => {
            log::error!(
                "Hot reload sprite '{}': failed to decode '{}': {e}",
                names(),
                albedo_path.display()
            );
            return Ok(());
        }
    };
    log::trace!(
        "reload_sprite_file '{}' triggered by '{}'",
        albedo_path.display(),
        path.display()
    );

    let mut grew = Vec::new();
    for (target, cut) in targets.iter().zip(cuts) {
        let id = target.id.as_str();
        let Some(cut) = cut else {
            log::error!(
                "Hot reload sprite '{id}': its frame leaves '{}'",
                albedo_path.display()
            );
            continue;
        };
        // M29 sibling decode: required when the sprite carried lit data; failures
        // log and leave the previous lit bundle byte-identical.
        if had_lit && (cut.normal.is_none() || (emissive_path.is_some() && cut.emissive.is_none()))
        {
            anyhow::bail!("Hot reload sprite '{id}': invalid lit sibling; keeping previous bundle");
        }
        let Some((cell, page)) = &target.cell else {
            log::error!(
                "Hot reload sprite '{id}': no atlas-registry entry; keeping previous state"
            );
            continue;
        };
        if cut.width <= cell.width && cut.height <= cell.height {
            write_in_place(renderer, target.atlas, cell, &cut, had_lit);
            let (uv, width, height) = shrink_entry(cell, *page, cut.width, cut.height);
            world
                .get_resource_mut::<AssetRegistry>()
                .expect("AssetRegistry resource missing")
                .update_sprite_entry(id, target.atlas, uv, width, height);
            log::info!(
                "Hot-reloaded sprite '{id}' ({}x{}, {:?}) in-place",
                cut.width,
                cut.height,
                target.filter
            );
        } else {
            let (old_w, old_h) = target.size;
            log::warn!(
                "Sprite '{id}' grew ({old_w}x{old_h} → {}x{}); rebuilding {:?} atlas",
                cut.width,
                cut.height,
                target.filter
            );
            if !grew.contains(&target.filter) {
                grew.push(target.filter);
            }
        }
    }
    for filter in grew {
        rebuild_atlas_for_filter(filter, world, renderer)?;
    }
    Ok(())
}

/// The reload's half before the upload (redo 7.9): the file at `path`
/// decoded once with the siblings given, and each sprite's pixels cut from
/// it by its frame (`None`, the whole file), `None` for a frame that leaves
/// the image. A sibling that fails goes to `report`.
pub(super) fn reload_cuts(
    path: &Path,
    normal: Option<&Path>,
    emissive: Option<&Path>,
    rects: &[Option<[u32; 4]>],
    report: impl FnMut(&str, String),
) -> Result<Vec<Option<Cut>>, image::ImageError> {
    let file = DecodedFile::open(path, normal, emissive, report)?;
    Ok(rects.iter().map(|&rect| file.cut(rect)).collect())
}

/// Writes `cut` into its packed `cell` on page `atlas`: the image at the
/// cell's top-left and a transparent tail (`shrink_entry`), and with
/// `lit`, flat-default normal and zero emissive cells under the siblings.
fn write_in_place(
    renderer: &mut Renderer,
    atlas: TextureHandle,
    cell: &PackedSprite,
    cut: &Cut,
    lit: bool,
) {
    let (px, py, pw, ph) = (cell.x, cell.y, cell.width, cell.height);
    let cell_w = pw as usize;
    let cell_h = ph as usize;
    let nw = cut.width as usize;
    let nh = cut.height as usize;
    let mut canvas = vec![0u8; cell_w * cell_h * 4];
    blit_rows(&mut canvas, cell_w, (0, 0), &cut.rgba, nw, nh);
    renderer.write_subtexture(atlas, &canvas, px, py, pw, ph);
    if lit {
        let mut normal_cell = flat_normal_canvas(cell_w * cell_h);
        let mut emissive_cell = vec![0u8; cell_w * cell_h * 4];
        if let Some(normal) = &cut.normal {
            blit_rows(&mut normal_cell, cell_w, (0, 0), normal, nw, nh);
        }
        if let Some(emissive) = &cut.emissive {
            blit_rows(&mut emissive_cell, cell_w, (0, 0), emissive, nw, nh);
        }
        renderer.write_subtexture_lit(atlas, &canvas, &normal_cell, &emissive_cell, px, py, pw, ph);
    }
}

/// Repack one filter class; decode failure keeps last-known-good atlas.
pub fn rebuild_atlas_for_filter(
    filter: FilterMode,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    #[allow(clippy::type_complexity)] // a sprite's files and its frame
    let entries: Vec<(
        String,
        PathBuf,
        Option<PathBuf>,
        Option<PathBuf>,
        Option<[u32; 4]>,
    )> = {
        let registry = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        registry
            .sprite_names()
            .filter_map(|id| {
                let asset = registry.get_sprite(id)?;
                if asset.filter == filter {
                    Some((
                        id.to_string(),
                        asset.path.clone(),
                        asset.normal_path.clone(),
                        asset.emissive_path.clone(),
                        asset.rect,
                    ))
                } else {
                    None
                }
            })
            .collect()
    };

    if entries.is_empty() {
        let stale: Vec<TextureHandle> = world
            .get_resource_mut::<AtlasRegistry>()
            .map(|ar| {
                let stale = std::mem::take(ar.page_handles_mut(filter));
                for h in &stale {
                    ar.page_sizes.remove(h);
                }
                stale
            })
            .unwrap_or_default();
        for h in stale {
            renderer.drop_texture(h);
        }
        return Ok(());
    }

    // Redo 7.9: each file decodes once, its sprites cut from it.
    let mut decoded: Vec<Decoded> = Vec::with_capacity(entries.len());
    let mut files: HashMap<(PathBuf, Option<PathBuf>, Option<PathBuf>), DecodedFile> =
        HashMap::new();
    for (id, path, normal_path, emissive_path, rect) in entries {
        let key = (path.clone(), normal_path.clone(), emissive_path.clone());
        if !files.contains_key(&key) {
            let file = match DecodedFile::open(
                &path,
                normal_path.as_deref(),
                emissive_path.as_deref(),
                |field, why| {
                    log::error!(
                        "Rebuild {filter:?} atlas: sprite '{id}' {field} {why}; keeping previous atlas"
                    );
                },
            ) {
                Ok(file) => file,
                Err(e) => {
                    log::error!(
                        "Rebuild {:?} atlas: decode '{}' ({}) failed: {e}; keeping previous atlas",
                        filter,
                        id,
                        path.display()
                    );
                    return Ok(());
                }
            };
            files.insert(key.clone(), file);
        }
        let Some(cut) = files[&key].cut(rect) else {
            log::error!(
                "Rebuild {filter:?} atlas: sprite '{id}''s frame leaves '{}'; keeping previous atlas",
                path.display()
            );
            return Ok(());
        };
        if (normal_path.is_some() && cut.normal.is_none())
            || (emissive_path.is_some() && cut.emissive.is_none())
        {
            anyhow::bail!(
                "Rebuild {filter:?} atlas: sprite '{id}' has an invalid lit sibling; keeping previous atlas"
            );
        }
        let lit = cut.normal.is_some();
        decoded.push(Decoded {
            id,
            path,
            width: cut.width,
            height: cut.height,
            rgba: cut.rgba,
            normal_path: if lit { normal_path } else { None },
            emissive_path: if lit { emissive_path } else { None },
            emissive_rgba: if lit { cut.emissive } else { None },
            normal_rgba: cut.normal,
            nine_slice: None,
            rect: None,
            anchor: None,
            points: BTreeMap::new(),
        });
    }
    drop(files);

    let max_dim = renderer.max_2d_texture_dimension();
    let (inputs, too_large) = pack_inputs(&decoded, max_dim);
    if let Some(why) = too_large {
        log::error!("Rebuild {filter:?} atlas: {why}; keeping previous atlas");
        return Ok(());
    }
    let pack = pack_shelf(&inputs, max_dim, 1);

    let mut atlas_registry = world
        .get_resource_mut::<AtlasRegistry>()
        .map(std::mem::take)
        .unwrap_or_default();
    let old_handles: Vec<TextureHandle> = atlas_registry.page_handles(filter).to_vec();

    let mut new_handles: Vec<TextureHandle> = Vec::with_capacity(pack.pages.len());
    for i in 0..pack.pages.len() {
        if i < old_handles.len() {
            new_handles.push(old_handles[i]);
        } else {
            new_handles.push(renderer.allocate_texture_handle());
        }
    }
    for &h in old_handles.iter().skip(new_handles.len()) {
        renderer.drop_texture(h);
        atlas_registry.page_sizes.remove(&h);
    }

    let by_id: HashMap<&str, &Decoded> = decoded.iter().map(|d| (d.id.as_str(), d)).collect();

    let any_lit = decoded.iter().any(|d| d.normal_rgba.is_some());
    let canvases = blit_pages(&pack, &by_id, any_lit);
    upload_pages(renderer, &new_handles, &pack, &canvases, filter);

    {
        let registry = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        for packed in &pack.sprites {
            let src = by_id[packed.id.as_str()];
            let uv = packed_uv(packed, pack.pages[packed.page as usize]);
            registry.update_sprite_entry(
                &src.id,
                new_handles[packed.page as usize],
                uv,
                src.width,
                src.height,
            );
            // Rebuild lit-atlas marker after a repack.
            let lit_atlas = if src.normal_rgba.is_some() {
                Some(new_handles[packed.page as usize])
            } else {
                None
            };
            registry.update_sprite_lit_atlas(&src.id, lit_atlas);
        }
    }

    for packed in &pack.sprites {
        atlas_registry
            .packed
            .insert(packed.id.clone(), packed.clone());
    }
    for (handle, page) in new_handles.iter().zip(&pack.pages) {
        atlas_registry.page_sizes.insert(*handle, *page);
    }
    *atlas_registry.page_handles_mut(filter) = new_handles;

    world.insert_resource(atlas_registry);
    Ok(())
}
