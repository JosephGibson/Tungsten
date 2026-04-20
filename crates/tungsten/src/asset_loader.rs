use std::path::Path;

use tungsten_core::assets::{
    AnimationData, AnimationRegistry, FilterMode, FontRegistry, ResolvedManifest, SoundData,
    SoundRegistry, TextureHandle, TilemapData, TilemapRegistry,
};
use tungsten_core::{ActionMap, ActionMapError, AssetRegistry, World};
use tungsten_render::Renderer;

/// Load all sprite assets from a resolved manifest: decode PNGs to CPU bitmaps,
/// register in the asset registry, and upload textures to the GPU.
pub fn load_sprites(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let registry = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");

    let mut uploads: Vec<(TextureHandle, Vec<u8>, u32, u32)> = Vec::new();

    for (id, sprite) in &manifest.sprites {
        let img = image::open(&sprite.path)
            .map_err(|e| anyhow::anyhow!("Failed to decode '{}': {}", sprite.path.display(), e))?
            .to_rgba8();

        let (width, height) = img.dimensions();
        let handle = registry.register_sprite(
            id.clone(),
            sprite.filter,
            width,
            height,
            sprite.path.clone(),
        );

        log::info!(
            "Loaded sprite '{}' ({}x{}, {:?}) -> {:?}",
            id,
            width,
            height,
            sprite.filter,
            handle,
        );

        uploads.push((handle, img.into_raw(), width, height));
    }

    for (handle, rgba, width, height) in uploads {
        renderer.upload_texture(handle, &rgba, width, height);
    }

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

/// Load all assets (sprites + animations + fonts + sounds + tilemaps).
/// After loading, validates cross-references: animation frames must
/// name known sprites (D-009), and tileset entries must name known
/// sprites.
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

    Ok(())
}

// ---------------------------------------------------------------------------
// Hot-reload helpers — called by App::process_hot_reload each frame.
// All functions log errors and return Ok(()) to preserve last-known-good state.
// ---------------------------------------------------------------------------

/// Hot-reload a sprite: decode the new PNG and re-upload to the GPU behind
/// the same TextureHandle. If dimensions changed the wgpu texture is recreated
/// in-place (the old one is dropped and deferred-destroyed by wgpu).
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

    let handle = {
        let registry = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        match registry.get_sprite(id) {
            Some(a) => a.texture,
            None => {
                log::error!("Hot reload sprite '{id}': not found in registry");
                return Ok(());
            }
        }
    };

    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .update_sprite_dimensions(id, new_w, new_h);

    renderer.upload_texture(handle, &rgba, new_w, new_h);
    log::info!(
        "Hot-reloaded sprite '{id}' ({}x{}, {:?})",
        new_w,
        new_h,
        filter
    );
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

        let mut additions = ResolvedManifest::default();
        for (id, entry) in &new_manifest.sprites {
            if !existing.iter().any(|e| e == id) {
                additions.sprites.insert(id.clone(), entry.clone());
            }
        }
        if !additions.sprites.is_empty() {
            if let Err(e) = load_sprites(&additions, world, renderer) {
                log::error!("Manifest reload: new sprite error: {e}");
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
