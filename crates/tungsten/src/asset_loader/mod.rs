//! Manifest-driven asset loading and hot reload. `atlas` packs and reloads
//! sprites, `reload` holds the other hot-reload paths and `scene` loads and
//! spawns scene files; every public item is re-exported here.

use std::path::Path;

use tungsten_core::assets::{
    AnimationData, AnimationRegistry, FontRegistry, LoadedManifest, MaterialRegistry,
    ParticleConfig, ParticleConfigRegistry, ParticleMeshRegistry, ParticleRender, ResolvedManifest,
    ShaderRegistry, SoundData, SoundRegistry, TilemapData, TilemapRegistry,
};
use tungsten_core::{AssetRegistry, World};
use tungsten_render::Renderer;

mod atlas;
mod reload;
mod scene;

pub use atlas::{AtlasRegistry, load_sprites, rebuild_atlas_for_filter, reload_sprite};
pub use reload::{
    reload_action_map, reload_animation, reload_font, reload_manifest, reload_material,
    reload_particle, reload_shader, reload_tilemap,
};
pub use scene::{load_scene, spawn_scene};

/// Load animation data from manifest.
pub fn load_animations(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut anim_registry = AnimationRegistry::new();
    let sprites = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");

    for (id, anim_entry) in &manifest.animations {
        let data = AnimationData::load(&anim_entry.path, sprites)?;
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

/// Load fonts into renderer and path registry, in sorted ID order, then set
/// the font families and the fallback chain (`D-115`, `D-116`).
pub fn load_fonts(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let mut font_registry = FontRegistry::new();

    for id in font_load_order(manifest) {
        let font_entry = &manifest.fonts[id];
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
    renderer.set_font_families(&manifest.font_families, &manifest.font_fallback);

    world.insert_resource(font_registry);
    Ok(())
}

/// Font IDs in the order they load: sorted, so fontdb IDs, and with them the
/// fallback's ties, repeat from a fresh start (`D-116`).
pub(crate) fn font_load_order(manifest: &ResolvedManifest) -> Vec<&String> {
    let mut ids: Vec<&String> = manifest.fonts.keys().collect();
    ids.sort();
    ids
}

/// Decode sounds into `SoundRegistry`.
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

/// Load tilemaps; sprite-ID validation happens after sprite load.
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

/// Register manifest-tracked shaders with the renderer + `ShaderRegistry`.
///
/// Upload is byte-equal short-circuited by `Renderer::upload_shader`: when
/// the on-disk WGSL matches the compile-time default already seeded at
/// `Renderer::new`, no pipeline rebuild happens. This keeps the default
/// config on the seeded pipelines and avoids a first-frame stall.
pub fn load_shaders(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let mut registry = world
        .remove_resource::<ShaderRegistry>()
        .unwrap_or_default();

    for (id, entry) in &manifest.shaders {
        let source = match std::fs::read_to_string(&entry.path) {
            Ok(s) => s,
            Err(e) => {
                log::error!(
                    "load_shaders '{id}': failed to read '{}': {e}; keeping compile-time default",
                    entry.path.display()
                );
                continue;
            }
        };
        registry.allocate(id, entry.path.clone());
        match renderer.upload_shader(id, source) {
            Ok(_) => {
                log::info!("Loaded shader '{id}' from '{}'", entry.path.display());
            }
            Err(e) => {
                log::error!(
                    "load_shaders '{id}': validation failed ({e}); keeping compile-time default"
                );
            }
        }
    }

    world.insert_resource(registry);
    Ok(())
}

/// Register manifest-tracked materials: allocate `MaterialAssetId`, resolve
/// the shader cross-ref, and build the `MaterialPipeline` on the renderer.
/// Call after `load_shaders` so shader-id resolution can succeed.
pub fn load_materials(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let mut registry = world
        .remove_resource::<MaterialRegistry>()
        .unwrap_or_default();

    for (id, material) in &manifest.materials {
        let asset_id = registry.allocate(
            id,
            material.source_manifest.clone(),
            material.shader.clone(),
            material.uniform_defaults,
        );
        if let Err(e) =
            renderer.upload_material(asset_id, id, &material.shader, material.uniform_defaults)
        {
            log::error!("load_materials '{id}': {e}; keeping last-known-good");
            continue;
        }
        log::info!("Loaded material '{id}' (shader={})", material.shader);
    }

    world.insert_resource(registry);
    Ok(())
}

/// Register manifest `particle_meshes` (M31, `D-093`) and upload their
/// geometry. The registry is kept across calls, so a mesh keeps its ID when it
/// loads again.
pub fn load_particle_meshes(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let mut registry = world
        .remove_resource::<ParticleMeshRegistry>()
        .unwrap_or_default();

    // Sorted, so the IDs do not depend on map order.
    let mut ids: Vec<&String> = manifest.particle_meshes.keys().collect();
    ids.sort();
    for id in ids {
        let mesh = &manifest.particle_meshes[id].mesh;
        let asset_id = registry.insert(id, mesh.clone());
        renderer.upload_particle_mesh(asset_id, &mesh.vertices, &mesh.indices);
        log::info!(
            "Loaded particle mesh '{id}' ({} vertices, {} indices)",
            mesh.vertices.len(),
            mesh.indices.len(),
        );
    }

    world.insert_resource(registry);
    Ok(())
}

/// Changes a manifest makes to the registered particle meshes; each list is
/// sorted.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ParticleMeshDiff {
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
}

/// Compare the registry with a manifest's `particle_meshes` section.
pub(crate) fn diff_particle_meshes(
    registry: &ParticleMeshRegistry,
    manifest: &ResolvedManifest,
) -> ParticleMeshDiff {
    let mut diff = ParticleMeshDiff::default();
    for (id, entry) in &manifest.particle_meshes {
        match registry
            .id_for_name(id)
            .and_then(|asset_id| registry.get(asset_id))
        {
            None => diff.added.push(id.clone()),
            Some(mesh) if *mesh != entry.mesh => diff.changed.push(id.clone()),
            Some(_) => {}
        }
    }
    for name in registry.names() {
        if !manifest.particle_meshes.contains_key(name) {
            diff.removed.push(name.to_string());
        }
    }
    diff.added.sort();
    diff.changed.sort();
    diff.removed.sort();
    diff
}

/// The asset a particle config draws with, when it is not registered: the
/// sprite of a quad config or the mesh of a mesh config, as `(kind, id)`.
fn missing_particle_render_ref<'a>(
    cfg: &'a ParticleConfig,
    world: &World,
) -> Option<(&'static str, &'a str)> {
    match &cfg.render {
        ParticleRender::Quad => {
            let registry = world
                .get_resource::<AssetRegistry>()
                .expect("AssetRegistry resource missing");
            registry
                .get_sprite(&cfg.sprite)
                .is_none()
                .then_some(("sprite", cfg.sprite.as_str()))
        }
        ParticleRender::Mesh { mesh } => {
            let registered = world
                .get_resource::<ParticleMeshRegistry>()
                .is_some_and(|registry| registry.id_for_name(mesh).is_some());
            (!registered).then_some(("particle mesh", mesh.as_str()))
        }
    }
}

/// Load particle configs; sprite and mesh references are validated after
/// both have loaded.
pub fn load_particles(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut registry = ParticleConfigRegistry::new();
    for (id, entry) in &manifest.particles {
        let cfg = ParticleConfig::load(&entry.path).map_err(|e| anyhow::anyhow!("{e}"))?;
        match &cfg.render {
            ParticleRender::Quad => log::info!(
                "Loaded particle config '{}' -> sprite '{}' ({} max)",
                id,
                cfg.sprite,
                cfg.max_alive,
            ),
            ParticleRender::Mesh { mesh } => log::info!(
                "Loaded particle config '{}' -> mesh '{}' ({} max)",
                id,
                mesh,
                cfg.max_alive,
            ),
        }
        registry.register(id.clone(), entry.path.clone(), cfg);
    }
    world.insert_resource(registry);
    Ok(())
}

/// Load all assets and validate sprite and particle-mesh cross-references (D-009).
pub fn load_all(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    load_sprites(manifest, world, renderer)?;
    load_animations(manifest, world)?;
    load_fonts(manifest, world, renderer)?;
    load_shaders(manifest, world, renderer)?;
    load_materials(manifest, world, renderer)?;
    load_sounds(manifest, world)?;
    load_tilemaps(manifest, world)?;
    load_particle_meshes(manifest, world, renderer)?;
    load_particles(manifest, world)?;

    let registry = world
        .get_resource::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    let anim_registry = world
        .get_resource::<AnimationRegistry>()
        .expect("AnimationRegistry resource missing");

    for (anim_id, anim_data) in anim_registry.iter() {
        for (i, frame) in anim_data.frames.iter().enumerate() {
            if registry.sprite(frame.sprite).is_none() {
                return Err(anyhow::anyhow!(
                    "Animation '{}' frame {} references unknown sprite ID '{}'",
                    anim_id,
                    i,
                    registry.sprite_name(frame.sprite).unwrap_or("?"),
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
                    "Tilemap '{map_id}' tileset[{i}] references unknown sprite ID '{sprite_id}'",
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
        if let Some((kind, name)) = missing_particle_render_ref(cfg, world) {
            return Err(anyhow::anyhow!(
                "Particle config '{id}' references unknown {kind} ID '{name}'",
            ));
        }
    }

    Ok(())
}

/// D-052 composition entry; D-017 duplicate IDs fatal.
pub fn load_all_merged(
    roots: &[impl AsRef<Path>],
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let merged = ResolvedManifest::load_and_merge_many(roots)?;
    load_all(&merged, world, renderer)?;
    world.insert_resource(LoadedManifest::new(merged));
    Ok(())
}

#[cfg(test)]
#[path = "../tests/asset_loader.rs"]
mod tests;
