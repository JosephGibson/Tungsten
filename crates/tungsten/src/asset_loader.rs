use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glam::Vec2;
use tungsten_core::assets::{
    pack_shelf, AnimationData, AnimationRegistry, FilterMode, FontRegistry, PackInput,
    PackedSprite, ParticleConfig, ParticleConfigRegistry, ResolvedManifest, SceneData, SoundData,
    SoundRegistry, TextureHandle, TilemapData, TilemapRegistry, UvRect,
};
use tungsten_core::{
    ActionMap, ActionMapError, AssetRegistry, CommandBuffer, Sprite, Tag, Transform, Visibility,
    World,
};
use tungsten_render::Renderer;

use crate::state::{SceneEntity, StateId};

/// Umbrella-crate resource tracking live atlas-page handles and every sprite's
/// packed rect. Hot-reload paths consult this to decide between in-place
/// overwrite and full rebuild, and to drop stale page handles when a rebuild
/// shrinks the page count. Populated by `load_sprites` and kept in sync by
/// `rebuild_atlas_for_filter`.
#[derive(Debug, Default)]
pub struct AtlasRegistry {
    pub nearest_pages: Vec<TextureHandle>,
    pub linear_pages: Vec<TextureHandle>,
    pub packed: HashMap<String, PackedSprite>,
}

impl AtlasRegistry {
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

/// One decoded sprite awaiting packing. CPU-side; no GPU handle yet. The
/// filter class is carried by the caller partition — one `Vec<Decoded>` per
/// filter — so it is not stored per entry.
struct Decoded {
    id: String,
    path: PathBuf,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// Pack `decoded` (one filter class) into one or more atlas pages, allocate
/// page handles from `renderer`, upload each page canvas, register every
/// sprite in `registry` with its half-texel-inset UV, and record results in
/// `atlas_registry`. Returns the list of page handles — used by the caller to
/// drop stale handles when a rebuild shrinks the page count.
///
/// The half-texel UV inset plus the packer's 1 px transparent padding keep
/// bilinear samples entirely inside the drawn rect at non-mip sampling; if
/// mipmaps are enabled in a future change, the inset math needs revisiting.
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

    let mut canvases: Vec<Vec<u8>> = pack
        .pages
        .iter()
        .map(|p| vec![0u8; (p.width as usize) * (p.height as usize) * 4])
        .collect();

    for packed in &pack.sprites {
        let src = by_id[packed.id.as_str()];
        let page = &pack.pages[packed.page as usize];
        let canvas = &mut canvases[packed.page as usize];
        let page_stride = (page.width as usize) * 4;
        let sprite_stride = (packed.width as usize) * 4;
        for row in 0..(packed.height as usize) {
            let dst_y = (packed.y as usize) + row;
            let dst_start = dst_y * page_stride + (packed.x as usize) * 4;
            let dst_end = dst_start + sprite_stride;
            let src_start = row * sprite_stride;
            let src_end = src_start + sprite_stride;
            canvas[dst_start..dst_end].copy_from_slice(&src.rgba[src_start..src_end]);
        }
    }

    let mut page_handles: Vec<TextureHandle> = Vec::with_capacity(pack.pages.len());
    for (page_idx, page) in pack.pages.iter().enumerate() {
        let handle = renderer.allocate_texture_handle();
        renderer.upload_texture(handle, &canvases[page_idx], page.width, page.height, filter);
        page_handles.push(handle);
    }

    for packed in &pack.sprites {
        let src = by_id[packed.id.as_str()];
        let page = &pack.pages[packed.page as usize];
        let pw = page.width as f32;
        let ph = page.height as f32;
        let uv = UvRect {
            min: [(packed.x as f32 + 0.5) / pw, (packed.y as f32 + 0.5) / ph],
            max: [
                (packed.x as f32 + packed.width as f32 - 0.5) / pw,
                (packed.y as f32 + packed.height as f32 - 0.5) / ph,
            ],
        };
        let atlas = page_handles[packed.page as usize];
        registry.register_sprite(
            src.id.clone(),
            filter,
            src.width,
            src.height,
            src.path.clone(),
            atlas,
            uv,
        );
        atlas_registry.packed.insert(src.id.clone(), packed.clone());
    }

    *atlas_registry.page_handles_mut(filter) = page_handles.clone();
    page_handles
}

/// Decode every sprite PNG named by `manifest`, pack each filter class into
/// one or more atlas pages, upload the pages to the GPU, and register sprites
/// with their per-sprite UV. Populates the `AtlasRegistry` resource so the
/// hot-reload paths can distinguish in-place overwrite from full rebuild.
///
/// Called once at startup. Step 6 removes the additions-path call from
/// `reload_manifest`; until then, calling this with a partial manifest is
/// supported but leaves only the additions in the `AtlasRegistry` — pre-Step-6
/// hot-reload-add is the single caller and it does not rely on the resource.
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
        let entry = Decoded {
            id: id.clone(),
            path: sprite.path.clone(),
            width,
            height,
            rgba: img.into_raw(),
        };
        match sprite.filter {
            FilterMode::Nearest => decoded_nearest.push(entry),
            FilterMode::Linear => decoded_linear.push(entry),
        }
    }

    let max_dim = renderer.max_2d_texture_dimension();
    let n_sprites = decoded_nearest.len() + decoded_linear.len();

    // Pull the AtlasRegistry out of the world for the duration of the build
    // so we can hold &mut to it and &mut AssetRegistry at the same time.
    let mut atlas_registry = world
        .get_resource_mut::<AtlasRegistry>()
        .map(std::mem::take)
        .unwrap_or_default();

    let (nearest_handles, linear_handles) = {
        let registry = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
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

/// Load all animation data from a resolved manifest.
pub fn load_animations(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut anim_registry = AnimationRegistry::new();

    for (id, anim_entry) in &manifest.animations {
        let data = AnimationData::load(&anim_entry.path)?;
        log::info!(
            "Loaded animation '{}' ({} frames, {}ms total, looping={})",
            id,
            data.frames.len(),
            data.total_duration_ms(),
            data.looping,
        );
        anim_registry.insert_with_path(id.clone(), data, anim_entry.path.clone());
    }

    world.insert_resource(anim_registry);
    Ok(())
}

/// Load all font assets from a resolved manifest: read TTF bytes, register them
/// in the renderer's text pipeline, and store paths in `FontRegistry` for
/// hot-reload reverse lookup.
pub fn load_fonts(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let mut font_registry = FontRegistry::new();

    for (id, font_entry) in &manifest.fonts {
        let data = std::fs::read(&font_entry.path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read font '{}' at '{}': {}",
                id,
                font_entry.path.display(),
                e
            )
        })?;
        log::info!(
            "Loaded font '{}' ({} bytes) from '{}'",
            id,
            data.len(),
            font_entry.path.display(),
        );
        renderer.load_font(id, data);
        font_registry.register(id.clone(), font_entry.path.clone());
    }

    world.insert_resource(font_registry);
    Ok(())
}

/// Load all sound assets from a resolved manifest: decode audio files and
/// register them in the `SoundRegistry` resource.
pub fn load_sounds(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut sound_registry = SoundRegistry::new();

    for (id, sound_entry) in &manifest.sounds {
        let data = SoundData::decode(&sound_entry.path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to decode sound '{}' at '{}': {}",
                id,
                sound_entry.path.display(),
                e
            )
        })?;
        log::info!(
            "Loaded sound '{}' ({} samples, {}Hz, {} ch) from '{}'",
            id,
            data.samples.len(),
            data.sample_rate,
            data.channels,
            sound_entry.path.display(),
        );
        sound_registry.register(id.clone(), data, sound_entry.volume, sound_entry.looping);
    }

    world.insert_resource(sound_registry);
    Ok(())
}

/// Load all tilemap data from a resolved manifest. Sprite-ID validation
/// of each tilemap's `tileset` happens in `load_all` after sprites are
/// loaded (mirrors the animation-frame validation path).
pub fn load_tilemaps(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut tilemap_registry = TilemapRegistry::new();

    for (id, entry) in &manifest.tilemaps {
        let data = TilemapData::load(&entry.path)?;
        log::info!(
            "Loaded tilemap '{}' ({}x{} tiles @ {}x{}px, {} layers)",
            id,
            data.width,
            data.height,
            data.tile_width,
            data.tile_height,
            data.layers.len(),
        );
        tilemap_registry.insert_with_path(id.clone(), data, entry.path.clone());
    }

    world.insert_resource(tilemap_registry);
    Ok(())
}

/// Load every particle config referenced by the manifest. Each entry is
/// parsed, validated, and registered in the [`ParticleConfigRegistry`] under
/// a freshly minted [`AssetId`](tungsten_core::AssetId). Sprite cross-reference
/// validation lives in `load_all` so it runs after sprites are registered.
pub fn load_particles(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut registry = ParticleConfigRegistry::new();
    for (id, entry) in &manifest.particles {
        let cfg = ParticleConfig::load(&entry.path).map_err(|e| anyhow::anyhow!("{e}"))?;
        log::info!(
            "Loaded particle config '{}' -> sprite '{}' ({} max)",
            id,
            cfg.sprite,
            cfg.max_alive,
        );
        registry.register(id.clone(), entry.path.clone(), cfg);
    }
    world.insert_resource(registry);
    Ok(())
}

/// Load all assets (sprites + animations + fonts + sounds + tilemaps + particles).
/// After loading, validates cross-references: animation frames must
/// name known sprites (D-009), tileset entries must name known sprites,
/// and particle configs must name known sprites.
pub fn load_all(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    load_sprites(manifest, world, renderer)?;
    load_animations(manifest, world)?;
    load_fonts(manifest, world, renderer)?;
    load_sounds(manifest, world)?;
    load_tilemaps(manifest, world)?;
    load_particles(manifest, world)?;

    let registry = world
        .get_resource::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    let anim_registry = world
        .get_resource::<AnimationRegistry>()
        .expect("AnimationRegistry resource missing");

    for (anim_id, anim_data) in anim_registry.iter() {
        for (i, frame) in anim_data.frames.iter().enumerate() {
            if registry.get_sprite(&frame.sprite).is_none() {
                return Err(anyhow::anyhow!(
                    "Animation '{}' frame {} references unknown sprite ID '{}'",
                    anim_id,
                    i,
                    frame.sprite,
                ));
            }
        }
    }

    let tilemap_registry = world
        .get_resource::<TilemapRegistry>()
        .expect("TilemapRegistry resource missing");

    for (map_id, map_data) in tilemap_registry.iter() {
        for (i, sprite_id) in map_data.tileset.iter().enumerate() {
            if registry.get_sprite(sprite_id).is_none() {
                return Err(anyhow::anyhow!(
                    "Tilemap '{}' tileset[{}] references unknown sprite ID '{}'",
                    map_id,
                    i,
                    sprite_id,
                ));
            }
        }
    }

    let particle_registry = world
        .get_resource::<ParticleConfigRegistry>()
        .expect("ParticleConfigRegistry resource missing");
    for (id, entry) in &manifest.particles {
        let Some(asset_id) = particle_registry.id_for_path(&entry.path) else {
            continue;
        };
        let cfg = particle_registry
            .get(asset_id)
            .expect("registered asset id lost its config");
        if registry.get_sprite(&cfg.sprite).is_none() {
            return Err(anyhow::anyhow!(
                "Particle config '{}' references unknown sprite ID '{}'",
                id,
                cfg.sprite,
            ));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Hot-reload helpers — called by App::process_hot_reload each frame.
// All functions log errors and return Ok(()) to preserve last-known-good state.
// ---------------------------------------------------------------------------

/// Load a `scene.json` file. Thin wrapper over [`SceneData::load`] that
/// maps `SceneError` into `anyhow::Error` with the path attached.
pub fn load_scene(path: &Path) -> anyhow::Result<SceneData> {
    SceneData::load(path)
        .map_err(|source| anyhow::anyhow!("Failed to load scene '{}': {source}", path.display()))
}

/// Spawn every entity in `data` through the world's [`CommandBuffer`], with
/// each spawned entity tagged `SceneEntity { state_id }` so the state
/// dispatcher can auto-despawn them on exit.
///
/// Sprite IDs are not validated here — missing IDs fall through to the
/// sprite-extract warning path, matching how tilemaps handle unresolved
/// tile IDs (see `D-046`).
pub fn spawn_scene(world: &mut World, data: &SceneData, state_id: StateId) {
    let buf = world
        .get_resource_mut::<CommandBuffer>()
        .expect("CommandBuffer resource missing");
    for entry in &data.entities {
        let pending = buf.spawn();
        buf.insert_pending(
            pending,
            Transform {
                position: Vec2::from(entry.transform.position),
                rotation: entry.transform.rotation,
                scale: Vec2::from(entry.transform.scale),
            },
        );
        if let Some(sprite) = &entry.sprite {
            buf.insert_pending(
                pending,
                Sprite {
                    asset_id: sprite.asset_id.clone(),
                    color: sprite.color,
                    z_order: sprite.z_order,
                },
            );
        }
        buf.insert_pending(
            pending,
            Visibility {
                visible: entry.visible,
            },
        );
        if let Some(name) = &entry.tag {
            buf.insert_pending(pending, Tag::new(name.clone()));
        }
        buf.insert_pending(pending, SceneEntity { state_id });
    }
}

/// Hot-reload a single sprite. If the new decoded size fits inside the
/// sprite's pre-packed rect, overwrite the rect in place (no handle churn —
/// `SpriteBatch`es already bound in this frame stay valid). Otherwise rebuild
/// the entire filter-class atlas via `rebuild_atlas_for_filter`.
///
/// # Between-frames invariant (D-031)
///
/// Hot-reload events are drained on the main loop between frames, so a
/// rebuild's `drop_texture` never races a `SpriteBatch` built in the current
/// frame's extract. Any caller that wires a different drain point must keep
/// this invariant — otherwise a dropped handle could be bound in a pending
/// draw (UB).
pub fn reload_sprite(
    id: &str,
    path: &Path,
    filter: FilterMode,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let img = match image::open(path) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            log::error!(
                "Hot reload sprite '{id}': failed to decode '{}': {e}",
                path.display()
            );
            return Ok(());
        }
    };
    let (new_w, new_h) = img.dimensions();
    let rgba = img.into_raw();

    // Look up atlas handle + packed rect (may be absent for sprites registered
    // outside `load_sprites`, e.g. the sprite-stress high-load generated sprite).
    let (atlas_handle, uv, old_w, old_h, packed_xywh) = {
        let asset_reg = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        let asset = match asset_reg.get_sprite(id) {
            Some(a) => a,
            None => {
                log::error!("Hot reload sprite '{id}': not found in registry");
                return Ok(());
            }
        };
        let packed = world
            .get_resource::<AtlasRegistry>()
            .and_then(|ar| ar.packed.get(id))
            .map(|p| (p.x, p.y, p.width, p.height));
        (asset.atlas, asset.uv, asset.width, asset.height, packed)
    };

    let Some((px, py, pw, ph)) = packed_xywh else {
        log::error!("Hot reload sprite '{id}': no atlas-registry entry; keeping previous state");
        return Ok(());
    };

    if new_w <= pw && new_h <= ph {
        // In-place: build a packed-rect-sized canvas, new bitmap top-left,
        // transparent below/right if shrunk. UV stays pointing at the full
        // packed rect (shrink-with-transparent-tail — plan risk #5).
        let cell_w = pw as usize;
        let cell_h = ph as usize;
        let mut canvas = vec![0u8; cell_w * cell_h * 4];
        let nw = new_w as usize;
        for row in 0..(new_h as usize) {
            let dst_start = row * cell_w * 4;
            let dst_end = dst_start + nw * 4;
            let src_start = row * nw * 4;
            let src_end = src_start + nw * 4;
            canvas[dst_start..dst_end].copy_from_slice(&rgba[src_start..src_end]);
        }
        renderer.write_subtexture(atlas_handle, &canvas, px, py, pw, ph);

        world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing")
            .update_sprite_entry(id, atlas_handle, uv, new_w, new_h);
        log::info!(
            "Hot-reloaded sprite '{id}' ({}x{}, {:?}) in-place",
            new_w,
            new_h,
            filter
        );
        return Ok(());
    }

    log::warn!(
        "Sprite '{id}' grew ({old_w}x{old_h} → {new_w}x{new_h}); rebuilding {:?} atlas",
        filter,
    );
    rebuild_atlas_for_filter(filter, world, renderer)?;
    Ok(())
}

/// Re-pack and re-upload every sprite in a single filter class from disk.
/// Reuses old page handles where possible so pending `SpriteBatch`es stay
/// valid across the swap (see the between-frames invariant on `reload_sprite`).
///
/// On a decode error in any sprite belonging to this filter class, the entire
/// rebuild is abandoned and the previous atlas is kept — last-known-good
/// discipline, matching [`reload_sprite`].
pub fn rebuild_atlas_for_filter(
    filter: FilterMode,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    // Snapshot every sprite in this filter class via its path.
    let entries: Vec<(String, PathBuf)> = {
        let registry = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        registry
            .sprite_ids()
            .filter_map(|id| {
                let asset = registry.get_sprite(id)?;
                if asset.filter == filter {
                    Some((id.to_string(), asset.path.clone()))
                } else {
                    None
                }
            })
            .collect()
    };

    if entries.is_empty() {
        // Drop any orphan pages for the now-empty filter class.
        let stale: Vec<TextureHandle> = world
            .get_resource_mut::<AtlasRegistry>()
            .map(|ar| std::mem::take(ar.page_handles_mut(filter)))
            .unwrap_or_default();
        for h in stale {
            renderer.drop_texture(h);
        }
        return Ok(());
    }

    let mut decoded: Vec<Decoded> = Vec::with_capacity(entries.len());
    for (id, path) in entries {
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
        decoded.push(Decoded {
            id,
            path,
            width: w,
            height: h,
            rgba: img.into_raw(),
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
    }

    let by_id: HashMap<&str, &Decoded> = decoded.iter().map(|d| (d.id.as_str(), d)).collect();

    let mut canvases: Vec<Vec<u8>> = pack
        .pages
        .iter()
        .map(|p| vec![0u8; (p.width as usize) * (p.height as usize) * 4])
        .collect();
    for packed in &pack.sprites {
        let src = by_id[packed.id.as_str()];
        let page = &pack.pages[packed.page as usize];
        let canvas = &mut canvases[packed.page as usize];
        let page_stride = (page.width as usize) * 4;
        let sprite_stride = (packed.width as usize) * 4;
        for row in 0..(packed.height as usize) {
            let dst_y = (packed.y as usize) + row;
            let dst_start = dst_y * page_stride + (packed.x as usize) * 4;
            let dst_end = dst_start + sprite_stride;
            let src_start = row * sprite_stride;
            let src_end = src_start + sprite_stride;
            canvas[dst_start..dst_end].copy_from_slice(&src.rgba[src_start..src_end]);
        }
    }

    for (page_idx, page) in pack.pages.iter().enumerate() {
        renderer.upload_texture(
            new_handles[page_idx],
            &canvases[page_idx],
            page.width,
            page.height,
            filter,
        );
    }

    {
        let registry = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        for packed in &pack.sprites {
            let src = by_id[packed.id.as_str()];
            let page = &pack.pages[packed.page as usize];
            let pw = page.width as f32;
            let ph = page.height as f32;
            let uv = UvRect {
                min: [(packed.x as f32 + 0.5) / pw, (packed.y as f32 + 0.5) / ph],
                max: [
                    (packed.x as f32 + packed.width as f32 - 0.5) / pw,
                    (packed.y as f32 + packed.height as f32 - 0.5) / ph,
                ],
            };
            registry.update_sprite_entry(
                &src.id,
                new_handles[packed.page as usize],
                uv,
                src.width,
                src.height,
            );
        }
    }

    for packed in &pack.sprites {
        atlas_registry
            .packed
            .insert(packed.id.clone(), packed.clone());
    }
    *atlas_registry.page_handles_mut(filter) = new_handles;

    world.insert_resource(atlas_registry);
    Ok(())
}

/// Hot-reload an animation: reparse the JSON and replace the entry in
/// `AnimationRegistry`.
pub fn reload_animation(id: &str, path: &Path, world: &mut World) -> anyhow::Result<()> {
    let data = match AnimationData::load(path) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Hot reload animation '{id}': {e}");
            return Ok(());
        }
    };

    world
        .get_resource_mut::<AnimationRegistry>()
        .expect("AnimationRegistry resource missing")
        .insert(id.to_string(), data);

    log::info!("Hot-reloaded animation '{id}'");
    Ok(())
}

/// Hot-reload a tilemap: reparse the `.tmj` JSON and replace the entry
/// in `TilemapRegistry`. Failures are logged and the last-known-good
/// data is kept so a typo in the JSON doesn't crash the running example.
pub fn reload_tilemap(id: &str, path: &Path, world: &mut World) -> anyhow::Result<()> {
    let data = match TilemapData::load(path) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Hot reload tilemap '{id}': {e}");
            return Ok(());
        }
    };

    // Validate tileset sprite IDs before accepting the reload — a typo
    // in a newly-added tileset entry would otherwise silently empty out
    // parts of the map.
    {
        let registry = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        for (i, sprite_id) in data.tileset.iter().enumerate() {
            if registry.get_sprite(sprite_id).is_none() {
                log::error!(
                    "Hot reload tilemap '{id}': tileset[{i}] references unknown sprite '{sprite_id}' — keeping stale"
                );
                return Ok(());
            }
        }
    }

    world
        .get_resource_mut::<TilemapRegistry>()
        .expect("TilemapRegistry resource missing")
        .insert(id.to_string(), data);

    log::info!("Hot-reloaded tilemap '{id}'");
    Ok(())
}

/// Hot-reload a particle config. Parses and validates the updated JSON, then
/// `Arc::swap`s the registry entry under the same `AssetId`. In-flight
/// emitters and particles keep the `Arc` snapshot they captured at spawn, so
/// the visible change is bounded to newly spawned particles (plan: in-flight
/// snapshot semantics). Parse failures log an error and preserve the
/// last-known-good config.
pub fn reload_particle(id: &str, path: &Path, world: &mut World) -> anyhow::Result<()> {
    let cfg = match ParticleConfig::load(path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Hot reload particle '{id}': {e}");
            return Ok(());
        }
    };

    // Validate sprite cross-reference.
    {
        let registry = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        if registry.get_sprite(&cfg.sprite).is_none() {
            log::error!(
                "Hot reload particle '{id}': sprite '{}' not registered — keeping stale",
                cfg.sprite
            );
            return Ok(());
        }
    }

    let particle_registry = world
        .get_resource_mut::<ParticleConfigRegistry>()
        .expect("ParticleConfigRegistry resource missing");
    let Some(asset_id) = particle_registry.id_for_name(id) else {
        log::warn!("Hot reload particle '{id}': unknown asset id — keeping stale");
        return Ok(());
    };
    particle_registry.replace(asset_id, cfg);

    log::info!("Hot-reloaded particle '{id}'");
    Ok(())
}

/// Hot-reload a font: read new bytes and replace the face data in the
/// renderer's text pipeline via `TextPipeline::reload_font`.
pub fn reload_font(id: &str, path: &Path, renderer: &mut Renderer) -> anyhow::Result<()> {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            log::error!(
                "Hot reload font '{id}': failed to read '{}': {e}",
                path.display()
            );
            return Ok(());
        }
    };

    renderer.reload_font(id, data);
    log::info!("Hot-reloaded font '{id}'");
    Ok(())
}

/// Hot-reload the workspace-root `input.json` action map. Loads the new
/// bindings, merges them with the engine defaults, and swaps the
/// `ActionMap` resource. On load failure the previous map is preserved
/// and the error is returned to the caller (the app layer logs and
/// continues).
pub fn reload_action_map(path: &Path, world: &mut World) -> Result<(), ActionMapError> {
    let loaded = ActionMap::load(path)?;
    let merged = ActionMap::merged_with_defaults(loaded);
    if let Some(map) = world.get_resource_mut::<ActionMap>() {
        *map = merged;
    } else {
        world.insert_resource(merged);
    }
    log::info!("Hot-reloaded action map from '{}'", path.display());
    Ok(())
}

/// Hot-reload the manifest: load the new version, register any new asset IDs,
/// warn about removed IDs (they stay stale — no removal), and log errors on
/// conflicts. Never crashes — all errors are logged and kept last-known-good.
pub fn reload_manifest(
    manifest_path: &Path,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let new_manifest = match ResolvedManifest::load(manifest_path) {
        Ok(m) => m,
        Err(e) => {
            log::error!(
                "Hot reload manifest: failed to parse '{}': {e}",
                manifest_path.display()
            );
            return Ok(());
        }
    };

    // --- Sprites: warn on removals, load additions ---
    {
        // Collect existing IDs to compare.
        let existing: Vec<String> = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing")
            .sprite_ids()
            .map(|s| s.to_string())
            .collect();

        for id in &existing {
            if !new_manifest.sprites.contains_key(id.as_str()) {
                log::warn!("Manifest reload: sprite '{id}' removed — keeping stale");
            }
        }

        // An added sprite is a growth event for its filter class: register
        // the new ID with placeholder atlas/UV, then rebuild the whole filter
        // class from disk (existing + added). `rebuild_atlas_for_filter`
        // re-decodes every sprite by path and calls `update_sprite_entry`
        // with real dimensions + UV, so the placeholders are always overwritten
        // unless the rebuild bails (decode error); in that last-known-good
        // case the orphan entry has width/height = 0 and will be cleaned on
        // the next successful reload.
        let mut gained_nearest = false;
        let mut gained_linear = false;
        {
            let registry = world
                .get_resource_mut::<AssetRegistry>()
                .expect("AssetRegistry resource missing");
            for (id, entry) in &new_manifest.sprites {
                if existing.iter().any(|e| e == id) {
                    continue;
                }
                registry.register_sprite(
                    id.clone(),
                    entry.filter,
                    0,
                    0,
                    entry.path.clone(),
                    TextureHandle(0),
                    UvRect::FULL,
                );
                match entry.filter {
                    FilterMode::Nearest => gained_nearest = true,
                    FilterMode::Linear => gained_linear = true,
                }
                log::info!("Manifest reload: staging new sprite '{id}'");
            }
        }
        if gained_nearest {
            if let Err(e) = rebuild_atlas_for_filter(FilterMode::Nearest, world, renderer) {
                log::error!("Manifest reload: nearest atlas rebuild failed: {e}");
            }
        }
        if gained_linear {
            if let Err(e) = rebuild_atlas_for_filter(FilterMode::Linear, world, renderer) {
                log::error!("Manifest reload: linear atlas rebuild failed: {e}");
            }
        }
    }

    // --- Animations: warn on removals, load additions ---
    {
        let existing: Vec<String> = world
            .get_resource::<AnimationRegistry>()
            .map(|ar| ar.ids().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        for id in &existing {
            if !new_manifest.animations.contains_key(id.as_str()) {
                log::warn!("Manifest reload: animation '{id}' removed — keeping stale");
            }
        }

        let mut additions = ResolvedManifest::default();
        for (id, entry) in &new_manifest.animations {
            if !existing.iter().any(|e| e == id) {
                additions.animations.insert(id.clone(), entry.clone());
            }
        }
        if !additions.animations.is_empty() {
            // load_animations replaces the whole registry resource; merge instead.
            for (id, entry) in additions.animations {
                match AnimationData::load(&entry.path) {
                    Ok(data) => {
                        if let Some(ar) = world.get_resource_mut::<AnimationRegistry>() {
                            ar.insert_with_path(id.clone(), data, entry.path.clone());
                            log::info!("Manifest reload: loaded new animation '{id}'");
                        }
                    }
                    Err(e) => log::error!("Manifest reload: new animation '{id}': {e}"),
                }
            }
        }
    }

    // --- Tilemaps: warn on removals, load additions ---
    {
        let existing: Vec<String> = world
            .get_resource::<TilemapRegistry>()
            .map(|tr| tr.ids().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        for id in &existing {
            if !new_manifest.tilemaps.contains_key(id.as_str()) {
                log::warn!("Manifest reload: tilemap '{id}' removed — keeping stale");
            }
        }

        for (id, entry) in &new_manifest.tilemaps {
            if existing.iter().any(|e| e == id) {
                continue;
            }
            match TilemapData::load(&entry.path) {
                Ok(data) => {
                    // Validate tileset sprite IDs before inserting.
                    let all_known = {
                        let registry = world
                            .get_resource::<AssetRegistry>()
                            .expect("AssetRegistry resource missing");
                        data.tileset
                            .iter()
                            .all(|sid| registry.get_sprite(sid).is_some())
                    };
                    if !all_known {
                        log::error!(
                            "Manifest reload: new tilemap '{id}' references unknown sprite IDs — skipping"
                        );
                        continue;
                    }
                    if let Some(tr) = world.get_resource_mut::<TilemapRegistry>() {
                        tr.insert_with_path(id.clone(), data, entry.path.clone());
                        log::info!("Manifest reload: loaded new tilemap '{id}'");
                    }
                }
                Err(e) => log::error!("Manifest reload: new tilemap '{id}': {e}"),
            }
        }
    }

    // --- Fonts: warn on removals, load additions ---
    {
        for (id, entry) in &new_manifest.fonts {
            let already_loaded = world
                .get_resource::<FontRegistry>()
                .map(|fr| fr.contains_id(id))
                .unwrap_or(false);

            if !already_loaded {
                match std::fs::read(&entry.path) {
                    Ok(data) => {
                        renderer.load_font(id, data);
                        if let Some(fr) = world.get_resource_mut::<FontRegistry>() {
                            fr.register(id.clone(), entry.path.clone());
                            log::info!("Manifest reload: loaded new font '{id}'");
                        }
                    }
                    Err(e) => log::error!(
                        "Manifest reload: new font '{id}' at '{}': {e}",
                        entry.path.display()
                    ),
                }
            }
        }
    }

    log::info!("Manifest reloaded from '{}'", manifest_path.display());
    Ok(())
}
