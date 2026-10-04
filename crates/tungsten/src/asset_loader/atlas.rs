//! Sprite atlases: decode, pack and upload at load, in-place hot reload of a
//! sprite that fits its packed cell, and repack of one filter class.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tungsten_core::assets::{
    AtlasPage, FilterMode, PackInput, PackResult, PackedSprite, ResolvedManifest, TextureHandle,
    UvRect, pack_shelf,
};
use tungsten_core::{AssetRegistry, World};
use tungsten_render::Renderer;

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
struct Decoded {
    id: String,
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
}

/// Decoded M29 siblings of one sprite: the normal map as RGBA and the
/// emissive mask premultiplied by its alpha into opaque RGBA.
#[derive(Default)]
struct SpriteSiblings {
    normal: Option<Vec<u8>>,
    emissive: Option<Vec<u8>>,
}

/// Decode the normal-map and emissive-mask siblings of an albedo of `size`. A
/// sibling that fails to decode or has another size comes back `None`, and
/// `report` gets its manifest field (`normal_map`, `emissive_mask`) and why.
fn decode_sprite_siblings(
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
struct PageCanvases {
    albedo: Vec<Vec<u8>>,
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
    renderer: &mut Renderer,
    handles: &[TextureHandle],
    pack: &PackResult,
    canvases: &PageCanvases,
    filter: FilterMode,
) {
    for (index, page) in pack.pages.iter().enumerate() {
        let albedo = &canvases.albedo[index];
        renderer.upload_texture(handles[index], albedo, page.width, page.height, filter);
        if !canvases.normal.is_empty() {
            renderer.upload_lit_texture(
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

/// Build one filter-class atlas; half-texel UV inset assumes no mipmaps.
fn build_atlas_for_filter(
    filter: FilterMode,
    decoded: &[Decoded],
    renderer: &mut Renderer,
    registry: &mut AssetRegistry,
    atlas_registry: &mut AtlasRegistry,
    max_dim: u32,
) -> Vec<TextureHandle> {
    if decoded.is_empty() {
        atlas_registry.page_handles_mut(filter).clear();
        return Vec::new();
    }

    let inputs: Vec<PackInput<'_>> = decoded
        .iter()
        .map(|d| PackInput {
            id: d.id.as_str(),
            width: d.width,
            height: d.height,
        })
        .collect();
    let pack = pack_shelf(&inputs, max_dim, 1);
    let by_id: HashMap<&str, &Decoded> = decoded.iter().map(|d| (d.id.as_str(), d)).collect();

    let any_lit = decoded.iter().any(|d| d.normal_rgba.is_some());
    let canvases = blit_pages(&pack, &by_id, any_lit);
    let page_handles: Vec<TextureHandle> = pack
        .pages
        .iter()
        .map(|_| renderer.allocate_texture_handle())
        .collect();
    upload_pages(renderer, &page_handles, &pack, &canvases, filter);

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
    let mut decoded_nearest: Vec<Decoded> = Vec::new();
    let mut decoded_linear: Vec<Decoded> = Vec::new();
    for (id, sprite) in &manifest.sprites {
        let img = image::open(&sprite.path)
            .map_err(|e| anyhow::anyhow!("Failed to decode '{}': {}", sprite.path.display(), e))?
            .to_rgba8();
        let (width, height) = img.dimensions();
        let siblings = decode_sprite_siblings(
            sprite.normal_path.as_deref(),
            sprite.emissive_path.as_deref(),
            (width, height),
            |field, why| log::error!("sprite '{id}' {field} {why}; sprite stays unlit"),
        );
        // M29: emissive without normal stays unlit; lit-path requires a normal.
        let lit = siblings.normal.is_some();
        let entry = Decoded {
            id: id.clone(),
            path: sprite.path.clone(),
            width,
            height,
            rgba: img.into_raw(),
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
            emissive_rgba: if lit { siblings.emissive } else { None },
            normal_rgba: siblings.normal,
        };
        match sprite.filter {
            FilterMode::Nearest => decoded_nearest.push(entry),
            FilterMode::Linear => decoded_linear.push(entry),
        }
    }

    let max_dim = renderer.max_2d_texture_dimension();
    let n_sprites = decoded_nearest.len() + decoded_linear.len();

    // Borrow split: `AtlasRegistry` plus `AssetRegistry`.
    let mut atlas_registry = world
        .get_resource_mut::<AtlasRegistry>()
        .map(std::mem::take)
        .unwrap_or_default();

    let (nearest_handles, linear_handles) = {
        let registry = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        // Sorted-name interning gives a manifest the same sprite IDs every run;
        // the packer registers in `HashMap` order.
        registry.intern_sprites(manifest.sprites.keys().map(String::as_str));
        let n = build_atlas_for_filter(
            FilterMode::Nearest,
            &decoded_nearest,
            renderer,
            registry,
            &mut atlas_registry,
            max_dim,
        );
        let l = build_atlas_for_filter(
            FilterMode::Linear,
            &decoded_linear,
            renderer,
            registry,
            &mut atlas_registry,
            max_dim,
        );
        (n, l)
    };

    log::info!(
        "Packed {} sprites → {} atlas pages ({} nearest + {} linear)",
        n_sprites,
        nearest_handles.len() + linear_handles.len(),
        nearest_handles.len(),
        linear_handles.len(),
    );

    world.insert_resource(atlas_registry);
    Ok(())
}

/// Hot-reload sprite; D-031 requires between-frame drain before handle drops.
///
/// `path` may point to the albedo PNG, the M29 normal-map sibling, or the
/// emissive-mask sibling. The reverse `path_to_sprite_id` maps any of the
/// three paths back to `id`; this routine consults the registered asset
/// metadata to decode all three siblings on every reload so the lit bundle
/// stays consistent.
pub fn reload_sprite(
    id: &str,
    path: &Path,
    filter: FilterMode,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let (atlas_handle, old_w, old_h, packed_cell, albedo_path, normal_path, emissive_path, had_lit) = {
        let asset_reg = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        let Some(asset) = asset_reg.get_sprite(id) else {
            log::error!("Hot reload sprite '{id}': not found in registry");
            return Ok(());
        };
        let packed = world.get_resource::<AtlasRegistry>().and_then(|ar| {
            let cell = ar.packed.get(id)?;
            Some((cell.clone(), *ar.page_sizes.get(&asset.atlas)?))
        });
        (
            asset.atlas,
            asset.width,
            asset.height,
            packed,
            asset.path.clone(),
            asset.normal_path.clone(),
            asset.emissive_path.clone(),
            asset.lit_atlas.is_some(),
        )
    };

    let img = match image::open(&albedo_path) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            log::error!(
                "Hot reload sprite '{id}': failed to decode '{}': {e}",
                albedo_path.display()
            );
            return Ok(());
        }
    };
    let (new_w, new_h) = img.dimensions();
    let rgba = img.into_raw();

    // M29 sibling decode: required when the sprite carried lit data; failures
    // log and leave the previous lit bundle byte-identical.
    let siblings = if had_lit {
        decode_sprite_siblings(
            normal_path.as_deref(),
            emissive_path.as_deref(),
            (new_w, new_h),
            |field, why| log::error!("Hot reload sprite '{id}': {field} {why}"),
        )
    } else {
        SpriteSiblings::default()
    };
    if had_lit
        && (siblings.normal.is_none() || (emissive_path.is_some() && siblings.emissive.is_none()))
    {
        anyhow::bail!("Hot reload sprite '{id}': invalid lit sibling; keeping previous bundle");
    }
    log::trace!(
        "reload_sprite '{id}' triggered by '{}' (albedo='{}')",
        path.display(),
        albedo_path.display()
    );

    let Some((cell, page)) = packed_cell else {
        log::error!("Hot reload sprite '{id}': no atlas-registry entry; keeping previous state");
        return Ok(());
    };
    let (px, py, pw, ph) = (cell.x, cell.y, cell.width, cell.height);

    if new_w <= pw && new_h <= ph {
        // In-place shrink: the image at the cell's top-left, a transparent
        // tail, and a UV over the image only (`shrink_entry`).
        let cell_w = pw as usize;
        let cell_h = ph as usize;
        let nw = new_w as usize;
        let nh = new_h as usize;
        let mut canvas = vec![0u8; cell_w * cell_h * 4];
        blit_rows(&mut canvas, cell_w, (0, 0), &rgba, nw, nh);
        renderer.write_subtexture(atlas_handle, &canvas, px, py, pw, ph);

        if had_lit {
            // Build flat-default normal + zero emissive cell-sized canvases,
            // then overlay the new sibling pixels if decoded successfully.
            let mut normal_cell = flat_normal_canvas(cell_w * cell_h);
            let mut emissive_cell = vec![0u8; cell_w * cell_h * 4];
            if let Some(normal) = &siblings.normal {
                blit_rows(&mut normal_cell, cell_w, (0, 0), normal, nw, nh);
            }
            if let Some(emissive) = &siblings.emissive {
                blit_rows(&mut emissive_cell, cell_w, (0, 0), emissive, nw, nh);
            }
            renderer.write_subtexture_lit(
                atlas_handle,
                &canvas,
                &normal_cell,
                &emissive_cell,
                px,
                py,
                pw,
                ph,
            );
        }

        let (uv, width, height) = shrink_entry(&cell, page, new_w, new_h);
        world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing")
            .update_sprite_entry(id, atlas_handle, uv, width, height);
        log::info!("Hot-reloaded sprite '{id}' ({new_w}x{new_h}, {filter:?}) in-place");
        return Ok(());
    }

    log::warn!(
        "Sprite '{id}' grew ({old_w}x{old_h} → {new_w}x{new_h}); rebuilding {filter:?} atlas",
    );
    rebuild_atlas_for_filter(filter, world, renderer)?;
    Ok(())
}

/// Repack one filter class; decode failure keeps last-known-good atlas.
pub fn rebuild_atlas_for_filter(
    filter: FilterMode,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let entries: Vec<(String, PathBuf, Option<PathBuf>, Option<PathBuf>)> = {
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

    let mut decoded: Vec<Decoded> = Vec::with_capacity(entries.len());
    for (id, path, normal_path, emissive_path) in entries {
        let img = match image::open(&path) {
            Ok(i) => i.to_rgba8(),
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
        let (w, h) = img.dimensions();
        let siblings = decode_sprite_siblings(
            normal_path.as_deref(),
            emissive_path.as_deref(),
            (w, h),
            |field, why| {
                log::error!(
                    "Rebuild {filter:?} atlas: sprite '{id}' {field} {why}; keeping previous atlas"
                );
            },
        );
        if (normal_path.is_some() && siblings.normal.is_none())
            || (emissive_path.is_some() && siblings.emissive.is_none())
        {
            anyhow::bail!(
                "Rebuild {filter:?} atlas: sprite '{id}' has an invalid lit sibling; keeping previous atlas"
            );
        }
        let lit = siblings.normal.is_some();
        decoded.push(Decoded {
            id,
            path,
            width: w,
            height: h,
            rgba: img.into_raw(),
            normal_path: if lit { normal_path } else { None },
            emissive_path: if lit { emissive_path } else { None },
            emissive_rgba: if lit { siblings.emissive } else { None },
            normal_rgba: siblings.normal,
        });
    }

    let max_dim = renderer.max_2d_texture_dimension();
    let inputs: Vec<PackInput<'_>> = decoded
        .iter()
        .map(|d| PackInput {
            id: d.id.as_str(),
            width: d.width,
            height: d.height,
        })
        .collect();
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
