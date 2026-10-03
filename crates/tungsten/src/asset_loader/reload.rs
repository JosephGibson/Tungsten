//! Hot reload of single assets and of the manifest graph; every path keeps
//! the last-known-good state on failure.

use std::path::{Path, PathBuf};

use tungsten_core::assets::{
    AnimationData, AnimationRegistry, FilterMode, FontRegistry, LoadedManifest, MaterialRegistry,
    ParticleConfig, ParticleConfigRegistry, ParticleMeshRegistry, ResolvedManifest, SoundRegistry,
    TextureHandle, TilemapData, TilemapRegistry, UvRect,
};
use tungsten_core::{ActionMap, ActionMapError, AssetRegistry, World};
use tungsten_render::Renderer;

use super::{diff_particle_meshes, missing_particle_render_ref, rebuild_atlas_for_filter};

/// Hot-reload a manifest-tracked material: look up by name, re-upload under
/// the current `ResolvedMaterial` entry. Validation failure keeps the prior
/// live pipeline (last-known-good).
pub fn reload_material(id: &str, world: &mut World, renderer: &mut Renderer) -> anyhow::Result<()> {
    let (asset_id, shader_name, defaults) = {
        let Some(registry) = world.get_resource::<MaterialRegistry>() else {
            log::error!("reload_material '{id}': no MaterialRegistry");
            return Ok(());
        };
        let Some(asset_id) = registry.get(id) else {
            log::error!("reload_material '{id}': unknown material id");
            return Ok(());
        };
        let Some(shader_name) = registry
            .shader_name_for_id(asset_id)
            .map(ToString::to_string)
        else {
            log::error!("reload_material '{id}': no shader name on record");
            return Ok(());
        };
        let defaults = registry.defaults_for_id(asset_id).unwrap_or_default();
        (asset_id, shader_name, defaults)
    };

    // Prefer the current `LoadedManifest` view so authored `uniform_defaults`
    // edits flow through without a separate material JSON file.
    let live_defaults = world
        .get_resource::<LoadedManifest>()
        .and_then(|m| {
            m.as_resolved()
                .materials
                .get(id)
                .map(|rm| rm.uniform_defaults)
        })
        .unwrap_or(defaults);

    if let Err(e) = renderer.upload_material(asset_id, id, &shader_name, live_defaults) {
        log::error!("reload_material '{id}': {e}; keeping last-known-good");
        return Ok(());
    }
    // Update the registry's defaults cache so subsequent reloads start from
    // the live value.
    if let Some(reg) = world.get_resource_mut::<MaterialRegistry>() {
        reg.allocate(
            id,
            reg.path_for_id(asset_id)
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            shader_name.clone(),
            live_defaults,
        );
    }
    log::info!("Hot-reloaded material '{id}'");
    Ok(())
}

/// Hot-reload animation JSON.
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

/// Hot-reload tilemap; failures keep last-known-good data.
pub fn reload_tilemap(id: &str, path: &Path, world: &mut World) -> anyhow::Result<()> {
    let data = match TilemapData::load(path) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Hot reload tilemap '{id}': {e}");
            return Ok(());
        }
    };

    // Validate sprite IDs before accepting tilemap reload.
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

/// Hot-reload particle config; in-flight `Arc` snapshots keep old config.
pub fn reload_particle(id: &str, path: &Path, world: &mut World) -> anyhow::Result<()> {
    let cfg = match ParticleConfig::load(path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Hot reload particle '{id}': {e}");
            return Ok(());
        }
    };

    if let Some((kind, name)) = missing_particle_render_ref(&cfg, world) {
        log::error!("Hot reload particle '{id}': {kind} '{name}' not registered — keeping stale");
        return Ok(());
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

/// Hot-reload WGSL shader; validation or rebuild failure logs and keeps the
/// previous `ShaderModule` + pipeline intact.
pub fn reload_shader(
    id: &str,
    path: &Path,
    _world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            log::error!(
                "Hot reload shader '{id}': failed to read '{}': {e}",
                path.display()
            );
            return Ok(());
        }
    };
    if let Err(e) = renderer.reload_shader(id, source) {
        log::error!("Hot reload shader '{id}': {e}");
    }
    Ok(())
}

/// Hot-reload font bytes.
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

/// Hot-reload `input.json`; load failure preserves previous map.
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

/// Hot-reload manifest additions; removals stay stale and last-known-good wins.
///
/// `roots` is the whole composition (`D-052`), whichever root changed: the
/// registries hold the merged graph, so the reload rebuilds and diffs the
/// merged graph too (`D-089`).
pub fn reload_manifest(
    roots: &[PathBuf],
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let root_list = roots
        .iter()
        .map(|root| format!("'{}'", root.display()))
        .collect::<Vec<_>>()
        .join(", ");
    let new_manifest = match ResolvedManifest::load_and_merge_many(roots) {
        Ok(m) => m,
        Err(e) => {
            log::error!("Hot reload manifest: failed to load {root_list}: {e}");
            return Ok(());
        }
    };

    {
        let existing: Vec<String> = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing")
            .sprite_ids()
            .map(ToString::to_string)
            .collect();

        for id in &existing {
            if !new_manifest.sprites.contains_key(id.as_str()) {
                log::warn!("Manifest reload: sprite '{id}' removed — keeping stale");
            }
        }
        // M29: warn when an existing sprite drops its normal/emissive
        // sibling. Aux pages stay last-known-good until the next full repack.
        {
            let registry = world
                .get_resource::<AssetRegistry>()
                .expect("AssetRegistry resource missing");
            for (id, entry) in &new_manifest.sprites {
                if let Some(asset) = registry.get_sprite(id) {
                    if asset.normal_path.is_some() && entry.normal_path.is_none() {
                        log::warn!(
                            "Manifest reload: sprite '{id}' normal_map removed — keeping stale lit page"
                        );
                    }
                    if asset.emissive_path.is_some() && entry.emissive_path.is_none() {
                        log::warn!(
                            "Manifest reload: sprite '{id}' emissive_mask removed — keeping stale lit page"
                        );
                    }
                }
            }
        }

        // Added sprite stages placeholder, then filter-class rebuild overwrites it.
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
                    entry.normal_path.clone(),
                    entry.emissive_path.clone(),
                    None,
                );
                match entry.filter {
                    FilterMode::Nearest => gained_nearest = true,
                    FilterMode::Linear => gained_linear = true,
                }
                log::info!("Manifest reload: staging new sprite '{id}'");
            }
        }
        if gained_nearest
            && let Err(e) = rebuild_atlas_for_filter(FilterMode::Nearest, world, renderer)
        {
            log::error!("Manifest reload: nearest atlas rebuild failed: {e}");
        }
        if gained_linear
            && let Err(e) = rebuild_atlas_for_filter(FilterMode::Linear, world, renderer)
        {
            log::error!("Manifest reload: linear atlas rebuild failed: {e}");
        }
    }

    {
        let existing: Vec<String> = world
            .get_resource::<AnimationRegistry>()
            .map(|ar| ar.ids().map(ToString::to_string).collect())
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

    {
        let existing: Vec<String> = world
            .get_resource::<TilemapRegistry>()
            .map(|tr| tr.ids().map(ToString::to_string).collect())
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

    {
        for (id, entry) in &new_manifest.fonts {
            let already_loaded = world
                .get_resource::<FontRegistry>()
                .is_some_and(|fr| fr.contains_id(id));

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

    // M31 particle meshes: register and upload new ones, re-upload changed
    // ones under the same ID. Before the particle block, so a config added by
    // the same edit may name a mesh added with it.
    {
        let mut registry = world
            .remove_resource::<ParticleMeshRegistry>()
            .unwrap_or_default();
        let diff = diff_particle_meshes(&registry, &new_manifest);

        for id in &diff.removed {
            log::warn!("Manifest reload: particle mesh '{id}' removed — keeping stale");
        }
        let added = diff.added.iter().map(|id| (id, "loaded new"));
        let changed = diff.changed.iter().map(|id| (id, "reloaded"));
        for (id, verb) in added.chain(changed) {
            let mesh = &new_manifest.particle_meshes[id].mesh;
            let asset_id = registry.insert(id, mesh.clone());
            renderer.upload_particle_mesh(asset_id, &mesh.vertices, &mesh.indices);
            log::info!("Manifest reload: {verb} particle mesh '{id}'");
        }
        world.insert_resource(registry);
    }

    {
        let existing: Vec<String> = world
            .get_resource::<ParticleConfigRegistry>()
            .map(|pr| pr.names().map(ToString::to_string).collect())
            .unwrap_or_default();

        for id in &existing {
            if !new_manifest.particles.contains_key(id.as_str()) {
                log::warn!("Manifest reload: particle '{id}' removed — keeping stale");
            }
        }

        for (id, entry) in &new_manifest.particles {
            if existing.iter().any(|e| e == id) {
                continue;
            }
            match ParticleConfig::load(&entry.path) {
                Ok(cfg) => {
                    if let Some((kind, name)) = missing_particle_render_ref(&cfg, world) {
                        log::error!(
                            "Manifest reload: new particle '{id}' references unknown {kind} '{name}' — skipping"
                        );
                        continue;
                    }
                    if let Some(pr) = world.get_resource_mut::<ParticleConfigRegistry>() {
                        pr.register(id.clone(), entry.path.clone(), cfg);
                        log::info!("Manifest reload: loaded new particle '{id}'");
                    }
                }
                Err(e) => log::error!("Manifest reload: new particle '{id}': {e}"),
            }
        }
    }

    // D-053: audio session-static; mixer owns cloned PCM.
    {
        let existing_count = world
            .get_resource::<SoundRegistry>()
            .map_or(0, |sr| sr.iter().count());
        let new_count = new_manifest.sounds.len();
        if existing_count != new_count {
            log::debug!(
                "Manifest reload: sound list changed ({existing_count} -> {new_count}) but audio is session-static; restart to pick up changes"
            );
        }
    }

    // M26 materials: register new, refresh existing. Materials live inside
    // the manifest graph, so every manifest edit flows through this branch.
    {
        let existing: Vec<String> = world
            .get_resource::<MaterialRegistry>()
            .map(|mr| mr.iter().map(|(name, _)| name.to_string()).collect())
            .unwrap_or_default();

        // Warn on removals (last-known-good).
        for id in &existing {
            if !new_manifest.materials.contains_key(id.as_str()) {
                log::warn!("Manifest reload: material '{id}' removed — keeping stale");
            }
        }

        for (id, entry) in &new_manifest.materials {
            if existing.iter().any(|e| e == id) {
                // Refresh existing pipeline against new defaults / shader pointer.
                // Stash the updated entry into LoadedManifest below so
                // `reload_material` picks it up.
                continue;
            }
            // Newly-added material: allocate an id and upload.
            let mut registry = world
                .remove_resource::<MaterialRegistry>()
                .unwrap_or_default();
            let asset_id = registry.allocate(
                id,
                entry.source_manifest.clone(),
                entry.shader.clone(),
                entry.uniform_defaults,
            );
            world.insert_resource(registry);
            if let Err(e) =
                renderer.upload_material(asset_id, id, &entry.shader, entry.uniform_defaults)
            {
                log::error!(
                    "Manifest reload: new material '{id}' upload failed: {e}; keeping stale"
                );
            } else {
                log::info!("Manifest reload: loaded new material '{id}'");
            }
        }
    }

    // Keep diagnostics/reload diff graph current.
    world.insert_resource(tungsten_core::assets::LoadedManifest::new(
        new_manifest.clone(),
    ));

    // Re-upload every existing material against the current LoadedManifest so
    // body-only uniform_defaults edits land. This mirrors the particle and
    // tilemap re-load patterns (D-053).
    let existing_ids: Vec<String> = world
        .get_resource::<MaterialRegistry>()
        .map(|mr| mr.iter().map(|(name, _)| name.to_string()).collect())
        .unwrap_or_default();
    for id in existing_ids {
        if new_manifest.materials.contains_key(&id)
            && let Err(e) = reload_material(&id, world, renderer)
        {
            log::error!("Manifest reload: material '{id}' refresh failed: {e}");
        }
    }

    log::info!("Manifest reloaded from {root_list}");
    Ok(())
}
