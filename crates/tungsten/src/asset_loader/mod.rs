//! Manifest-driven asset loading and hot reload. `atlas` packs and reloads
//! sprites, `reload` holds the other hot-reload paths, `scene` loads and
//! spawns scene files and `prefab` loads and reloads prefab and data files
//! in two phases (M61); every public item is re-exported here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use tungsten_core::assets::{
    AnimationData, AnimationRegistry, FilterMode, FontRegistry, LoadedManifest, MaterialAssetId,
    MaterialRegistry, MaterialUniformDefaults, ParticleConfig, ParticleConfigRegistry,
    ParticleMeshAssetId, ParticleMeshRegistry, ParticleRender, ResolvedFontFamily,
    ResolvedManifest, ShaderAssetId, ShaderRegistry, SoundData, SoundRegistry, TextureHandle,
    TilemapData, TilemapRegistry, UvRect,
};
use tungsten_core::physics::{PhysicsLayers, PhysicsMaterials};
use tungsten_core::ui::{ScreenFile, Theme, ThemeFile, UiScreenRegistry, UiThemeRegistry};
use tungsten_core::{AssetRegistry, World};
use tungsten_render::Renderer;
use tungsten_render::sprite::TextureHandles;

mod atlas;
pub(crate) mod check;
mod prefab;
mod reload;
mod scene;

pub use atlas::{
    AtlasRegistry, load_sprites, rebuild_atlas_for_filter, reload_sprite, reload_sprite_file,
};
pub use prefab::{load_prefabs, reload_data, reload_prefab};
pub(crate) use reload::reload_json_files;
pub use reload::{
    reload_action_map, reload_animation, reload_font, reload_manifest, reload_material,
    reload_particle, reload_shader, reload_sound, reload_tilemap, reload_ui_screen,
    reload_ui_screens_from_manifest, reload_ui_theme, reload_ui_themes_from_manifest,
};
pub use scene::{load_scene, spawn_scene};
pub(crate) use scene::{
    register_extract_components, register_particle_emitter, register_squash_stretch,
};

/// The reserved sprite every load registers after the manifest's (redo 7.9):
/// the renderer's 1 × 1 white texel, which flat quads, segments and the
/// stock glow draw on. A manifest sprite of this name fails the load.
pub const ENGINE_WHITE: &str = "engine_white";

/// The reserved material every load registers after the manifest's (redo
/// 7.9): the stock glow, its shader `glow`, compiled in. A manifest material
/// of this name fails the load.
pub const ENGINE_GLOW: &str = "engine_glow";

/// The stock glow's shader, compiled in and listed by the root manifest.
const GLOW_SHADER: &str = "glow";

/// `engine_glow`'s defaults: white, falloff 2, no core boost or dither, peak
/// alpha 1 (`v0` and `f32s`, as its shader reads them).
#[must_use]
pub fn engine_glow_defaults() -> MaterialUniformDefaults {
    MaterialUniformDefaults {
        vec4: [[1.0; 4], [0.0; 4], [0.0; 4], [0.0; 4]],
        f32s: [2.0, 0.0, 0.0, 1.0],
        ..MaterialUniformDefaults::default()
    }
}

/// Allocates the reserved `engine_glow` in `materials`, after the
/// manifest's, and returns its ID.
pub(crate) fn allocate_engine_glow(materials: &mut MaterialRegistry) -> MaterialAssetId {
    materials.allocate(
        ENGINE_GLOW,
        PathBuf::new(),
        GLOW_SHADER.to_string(),
        engine_glow_defaults(),
    )
}

/// A manifest that names a reserved sprite or material.
fn check_reserved(manifest: &ResolvedManifest) -> anyhow::Result<()> {
    if manifest.sprites.contains_key(ENGINE_WHITE) {
        anyhow::bail!("sprite '{ENGINE_WHITE}' is reserved by the engine; rename it");
    }
    if manifest.materials.contains_key(ENGINE_GLOW) {
        anyhow::bail!("material '{ENGINE_GLOW}' is reserved by the engine; rename it");
    }
    Ok(())
}

/// Registers the reserved `engine_white` on the target's white texel, once.
fn register_engine_white(world: &mut World, target: &mut dyn LoadTarget) {
    let registry = world.resource_mut::<AssetRegistry>();
    if registry.get_sprite(ENGINE_WHITE).is_some() {
        return;
    }
    let texel = target.white_texture();
    registry.register_sprite(
        ENGINE_WHITE.to_string(),
        FilterMode::Nearest,
        1,
        1,
        PathBuf::new(),
        texel,
        UvRect::FULL,
        None,
        None,
        None,
    );
}

/// Theme file paths by `ui_themes` ID (M46, `D-148`): the hot reload's
/// reverse lookup, as [`FontRegistry`] is for fonts.
#[derive(Debug, Default)]
pub struct UiThemePaths {
    path_to_id: HashMap<PathBuf, String>,
}

impl UiThemePaths {
    /// Records that `path` loads as theme `id`.
    pub fn register(&mut self, id: String, path: PathBuf) {
        self.path_to_id.insert(path, id);
    }

    /// The theme a file path loads as.
    #[must_use]
    pub fn id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_id.get(path).map(String::as_str)
    }
}

/// Screen file paths by `ui_screens` ID (redo 7.20): the hot reload's
/// reverse lookup, inserted only when a manifest lists screens.
#[derive(Debug, Default)]
pub struct UiScreenPaths {
    path_to_id: HashMap<PathBuf, String>,
}

impl UiScreenPaths {
    /// Records that `path` loads as screen `id`.
    pub fn register(&mut self, id: String, path: PathBuf) {
        self.path_to_id.insert(path, id);
    }

    /// The screen a file path loads as.
    #[must_use]
    pub fn id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_id.get(path).map(String::as_str)
    }
}

/// Load the manifest's `ui_screens` into a [`UiScreenRegistry`] and their
/// paths into [`UiScreenPaths`] (redo 7.20), in sorted ID order; a file
/// that does not load is an error, as a theme's. A manifest without screens
/// inserts neither resource, and every reader takes their absence as none.
pub fn load_ui_screens(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    if manifest.ui_screens.is_empty() {
        return Ok(());
    }
    let mut registry = UiScreenRegistry::new();
    let mut paths = UiScreenPaths::default();
    let mut ids: Vec<&String> = manifest.ui_screens.keys().collect();
    ids.sort();
    for id in ids {
        let entry = &manifest.ui_screens[id];
        let file = ScreenFile::load(&entry.path)
            .map_err(|e| anyhow::anyhow!("Failed to load ui screen '{id}': {e}"))?;
        log::info!("Loaded ui screen '{id}' from '{}'", entry.path.display());
        registry.insert(id.clone(), file);
        paths.register(id.clone(), entry.path.clone());
    }
    world.insert_resource(registry);
    world.insert_resource(paths);
    Ok(())
}

/// Load the manifest's `ui_themes` into a [`UiThemeRegistry`] and their
/// paths into [`UiThemePaths`] (M46, `D-148`), in sorted ID order, each file
/// over the built-in theme. A theme whose font tokens name no
/// `font_families` entry warns per token; a file that does not load is an
/// error, as any other asset's.
pub fn load_ui_themes(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut registry = UiThemeRegistry::new();
    let mut paths = UiThemePaths::default();
    let mut ids: Vec<&String> = manifest.ui_themes.keys().collect();
    ids.sort();
    for id in ids {
        let entry = &manifest.ui_themes[id];
        let theme = ThemeFile::load(&entry.path)
            .map_err(|e| anyhow::anyhow!("Failed to load ui theme '{id}': {e}"))?
            .into_theme();
        warn_unknown_font_tokens(id, &theme, manifest);
        log::info!("Loaded ui theme '{id}' from '{}'", entry.path.display());
        registry.insert(id.clone(), theme);
        paths.register(id.clone(), entry.path.clone());
    }
    world.insert_resource(registry);
    world.insert_resource(paths);
    Ok(())
}

/// Warns for each font token of `theme` that names no `font_families` entry
/// of `manifest`: such text falls back through the chain at draw time.
pub(crate) fn warn_unknown_font_tokens(id: &str, theme: &Theme, manifest: &ResolvedManifest) {
    for (token, family) in unknown_font_tokens(theme, manifest) {
        log::warn!(
            "ui theme '{id}': fonts.{token} names family '{family}', which no font_families entry declares"
        );
    }
}

/// The font tokens of `theme` whose family neither `manifest` declares nor
/// the engine adds (`engine_mono`, M57), with their families.
pub(crate) fn unknown_font_tokens<'a>(
    theme: &'a Theme,
    manifest: &ResolvedManifest,
) -> Vec<(&'static str, &'a str)> {
    let fonts = &theme.tokens.fonts;
    [
        ("body", fonts.body.as_str()),
        ("heading", fonts.heading.as_str()),
        ("mono", fonts.mono.as_str()),
    ]
    .into_iter()
    .filter(|(_, family)| {
        *family != crate::ENGINE_FONT_ID && !manifest.font_families.contains_key(*family)
    })
    .collect()
}

/// Load animation data from manifest.
pub fn load_animations(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    world.insert_resource(AnimationRegistry::new());
    for id in sorted_keys(&manifest.animations) {
        load_animation_unit(manifest, id, world)?;
    }
    Ok(())
}

/// One animation file into the World's [`AnimationRegistry`], its frames'
/// sprite names interned.
fn load_animation_unit(
    manifest: &ResolvedManifest,
    id: &str,
    world: &mut World,
) -> anyhow::Result<()> {
    let anim_entry = &manifest.animations[id];
    let sprites = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    let data = AnimationData::load(&anim_entry.path, sprites)?;
    log::info!(
        "Loaded animation '{}' ({} frames, {}ms total, looping={})",
        id,
        data.frames.len(),
        data.total_duration_ms(),
        data.looping,
    );
    world
        .get_resource_mut::<AnimationRegistry>()
        .expect("AnimationRegistry resource missing")
        .insert_with_path(id.to_string(), data, anim_entry.path.clone());
    Ok(())
}

/// Load fonts into renderer and path registry, in sorted ID order, then set
/// the font families and the fallback chain (`D-115`, `D-116`).
pub fn load_fonts(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    world.insert_resource(FontRegistry::new());
    for id in font_load_order(manifest) {
        load_font_unit(manifest, id, world, renderer)?;
    }
    set_font_families(manifest, renderer);
    Ok(())
}

/// One font face into the target and the World's [`FontRegistry`].
fn load_font_unit(
    manifest: &ResolvedManifest,
    id: &str,
    world: &mut World,
    target: &mut dyn LoadTarget,
) -> anyhow::Result<()> {
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
    target.load_font(id, data);
    world
        .get_resource_mut::<FontRegistry>()
        .expect("FontRegistry resource missing")
        .register(id.to_string(), font_entry.path.clone());
    Ok(())
}

/// The manifest's font families, with the engine's, and its fallback chain.
fn set_font_families(manifest: &ResolvedManifest, target: &mut dyn LoadTarget) {
    target.set_font_families(
        &crate::engine_font::with_engine_family(&manifest.font_families),
        &manifest.font_fallback,
    );
}

/// Font IDs in the order they load: sorted, so fontdb IDs, and with them the
/// fallback's ties, repeat from a fresh start (`D-116`).
pub(crate) fn font_load_order(manifest: &ResolvedManifest) -> Vec<&String> {
    let mut ids: Vec<&String> = manifest.fonts.keys().collect();
    ids.sort();
    ids
}

/// Decode sounds into `SoundRegistry`, each with its manifest volume, loop
/// flag, bus and voice limit.
pub fn load_sounds(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    world.insert_resource(SoundRegistry::new());
    for id in sorted_keys(&manifest.sounds) {
        load_sound_unit(manifest, id, world)?;
    }
    Ok(())
}

/// One sound decoded into the World's [`SoundRegistry`], with its manifest
/// volume, loop, bus and voice limit.
fn load_sound_unit(manifest: &ResolvedManifest, id: &str, world: &mut World) -> anyhow::Result<()> {
    let sound_entry = &manifest.sounds[id];
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
    let sound_registry = world
        .get_resource_mut::<SoundRegistry>()
        .expect("SoundRegistry resource missing");
    let handle = sound_registry.register(
        id.to_string(),
        data,
        sound_entry.volume,
        sound_entry.looping,
    );
    sound_registry.set_bus(handle, sound_entry.bus);
    sound_registry.set_max_voices(handle, sound_entry.max_voices);
    // Redo 7.9: an edit to its file reloads it.
    sound_registry.set_path(handle, sound_entry.path.clone());
    Ok(())
}

/// A section's IDs, sorted, so a load's order is every run's.
fn sorted_keys<T>(section: &HashMap<String, T>) -> Vec<&String> {
    let mut ids: Vec<&String> = section.keys().collect();
    ids.sort();
    ids
}

/// Load tilemaps; sprite-ID validation happens after sprite load.
pub fn load_tilemaps(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    world.insert_resource(TilemapRegistry::new());
    for id in sorted_keys(&manifest.tilemaps) {
        load_tilemap_unit(manifest, id, world)?;
    }
    Ok(())
}

/// One tilemap into the World's [`TilemapRegistry`].
fn load_tilemap_unit(
    manifest: &ResolvedManifest,
    id: &str,
    world: &mut World,
) -> anyhow::Result<()> {
    let entry = &manifest.tilemaps[id];
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
    world
        .get_resource_mut::<TilemapRegistry>()
        .expect("TilemapRegistry resource missing")
        .insert_with_path(id.to_string(), data, entry.path.clone());
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
    load_shaders_into(manifest, world, renderer);
    Ok(())
}

fn load_shaders_into(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut dyn LoadTarget,
) {
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
        // The renderer mints the ID; core records the same one, even for a
        // shader that fails validation, so a later edit still reloads it.
        let asset = renderer.resolve_or_allocate_shader_id(id);
        registry.register(id, asset, entry.path.clone());
        match renderer.upload_shader(id, source) {
            Ok(()) => {
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
}

/// Register manifest-tracked materials: allocate `MaterialAssetId`, resolve
/// the shader cross-ref, and build the `MaterialPipeline` on the renderer.
/// Call after `load_shaders` so shader-id resolution can succeed.
pub fn load_materials(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    load_materials_into(manifest, world, renderer);
    Ok(())
}

fn load_materials_into(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut dyn LoadTarget,
) {
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
    // The stock glow, after the manifest's, so their IDs stay.
    let glow = allocate_engine_glow(&mut registry);
    if let Err(e) = renderer.upload_material(glow, ENGINE_GLOW, GLOW_SHADER, engine_glow_defaults())
    {
        log::error!("load_materials '{ENGINE_GLOW}': {e}");
    }

    world.insert_resource(registry);
}

/// Register manifest `particle_meshes` (M31, `D-093`) and upload their
/// geometry. The registry is kept across calls, so a mesh keeps its ID when it
/// loads again.
pub fn load_particle_meshes(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    load_particle_meshes_into(manifest, world, renderer);
    Ok(())
}

fn load_particle_meshes_into(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut dyn LoadTarget,
) {
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

/// Puts the manifest's `physics_layers` and `physics_materials` in the World
/// as [`PhysicsLayers`] and [`PhysicsMaterials`], replacing those it holds:
/// the startup loads them before the prefabs, which name them, and a
/// manifest reload replaces them, the materials' version moving on.
pub fn load_physics_names(manifest: &ResolvedManifest, world: &mut World) {
    world.insert_resource(PhysicsLayers::new(
        manifest
            .physics_layers
            .iter()
            .map(|(name, &layer)| (name.clone(), layer)),
    ));
    let materials = manifest
        .physics_materials
        .iter()
        .map(|(name, &material)| (name.clone(), material));
    match world.get_resource_mut::<PhysicsMaterials>() {
        Some(existing) => existing.replace(materials),
        None => world.insert_resource(PhysicsMaterials::new(materials)),
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

/// Load all assets and validate sprite and particle-mesh cross-references
/// (D-009), then every map's objects against the loaded prefabs (M51). The
/// prefabs and data entries load together, in two phases (M61), after the
/// particle configs and tilemaps their components name. Every unit of a
/// [`LoadPlan`] at once (redo 7.9).
pub fn load_all(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    LoadPlan::new(manifest.clone(), world)
        .run(world, renderer)
        .map(drop)
}

/// `load_all`'s last unit: animation frames, tilesets and particle configs
/// against the sprites and meshes, then every map's objects (`D-009`).
fn cross_check(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let registry = world
        .get_resource::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    let anim_registry = world
        .get_resource::<AnimationRegistry>()
        .expect("AnimationRegistry resource missing");

    let sounds = world.get_resource::<SoundRegistry>();
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
            // Redo 7.9: a frame's sound names a loaded sound.
            if let Some(sound) = &frame.sound
                && sounds.is_none_or(|sounds| sounds.get_by_id(sound).is_none())
            {
                return Err(anyhow::anyhow!(
                    "Animation '{anim_id}' frame {i} references unknown sound ID '{sound}'"
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
        // Redo 7.9: its clip and its stop's layer names, the physics names
        // loaded.
        if let Some(clip) = &cfg.clip
            && anim_registry.get(clip).is_none()
        {
            return Err(anyhow::anyhow!(
                "Particle config '{id}' references unknown clip ID '{clip}'"
            ));
        }
        let layers = world.get_resource::<PhysicsLayers>();
        for layer in cfg.stop.iter().flat_map(|stop| &stop.mask) {
            if let tungsten_core::assets::particle::LayerRef::Name(name) = layer
                && layers.is_none_or(|layers| layers.layer(name).is_none())
            {
                return Err(anyhow::anyhow!(
                    "Particle config '{id}' stop names unknown physics layer '{name}'"
                ));
            }
        }
    }

    prefab::check_all_map_objects(world)
}

/// What a load's units need from the renderer (redo 7.9): the
/// [`Renderer`], or [`HeadlessTarget`], which mints handles and uploads
/// nothing.
pub(crate) trait LoadTarget {
    fn max_2d_texture_dimension(&self) -> u32;
    fn allocate_texture_handle(&mut self) -> TextureHandle;
    fn upload_texture(
        &mut self,
        handle: TextureHandle,
        rgba: &[u8],
        width: u32,
        height: u32,
        filter: FilterMode,
    );
    #[allow(clippy::too_many_arguments)] // `Renderer::upload_lit_texture`'s
    fn upload_lit_texture(
        &mut self,
        handle: TextureHandle,
        albedo: &[u8],
        normal: &[u8],
        emissive: &[u8],
        width: u32,
        height: u32,
        filter: FilterMode,
    );
    fn load_font(&mut self, id: &str, data: Vec<u8>);
    fn set_font_families(
        &mut self,
        families: &HashMap<String, ResolvedFontFamily>,
        fallback: &[String],
    );
    fn resolve_or_allocate_shader_id(&mut self, id: &str) -> ShaderAssetId;
    /// The 1 × 1 white texel's handle, made on first use.
    fn white_texture(&mut self) -> TextureHandle;
    fn upload_shader(&mut self, id: &str, source: String) -> anyhow::Result<()>;
    fn upload_material(
        &mut self,
        asset: MaterialAssetId,
        id: &str,
        shader: &str,
        defaults: MaterialUniformDefaults,
    ) -> anyhow::Result<()>;
    fn upload_particle_mesh(
        &mut self,
        asset: ParticleMeshAssetId,
        vertices: &[[f32; 2]],
        indices: &[u16],
    );
}

impl LoadTarget for Renderer {
    fn max_2d_texture_dimension(&self) -> u32 {
        Renderer::max_2d_texture_dimension(self)
    }
    fn allocate_texture_handle(&mut self) -> TextureHandle {
        Renderer::allocate_texture_handle(self)
    }
    fn upload_texture(
        &mut self,
        handle: TextureHandle,
        rgba: &[u8],
        width: u32,
        height: u32,
        filter: FilterMode,
    ) {
        Renderer::upload_texture(self, handle, rgba, width, height, filter);
    }
    fn upload_lit_texture(
        &mut self,
        handle: TextureHandle,
        albedo: &[u8],
        normal: &[u8],
        emissive: &[u8],
        width: u32,
        height: u32,
        filter: FilterMode,
    ) {
        Renderer::upload_lit_texture(
            self, handle, albedo, normal, emissive, width, height, filter,
        );
    }
    fn load_font(&mut self, id: &str, data: Vec<u8>) {
        Renderer::load_font(self, id, data);
    }
    fn set_font_families(
        &mut self,
        families: &HashMap<String, ResolvedFontFamily>,
        fallback: &[String],
    ) {
        Renderer::set_font_families(self, families, fallback);
    }
    fn resolve_or_allocate_shader_id(&mut self, id: &str) -> ShaderAssetId {
        Renderer::resolve_or_allocate_shader_id(self, id)
    }
    fn white_texture(&mut self) -> TextureHandle {
        Renderer::white_texture(self)
    }
    fn upload_shader(&mut self, id: &str, source: String) -> anyhow::Result<()> {
        Renderer::upload_shader(self, id, source)
            .map(|_| ())
            .map_err(anyhow::Error::from)
    }
    fn upload_material(
        &mut self,
        asset: MaterialAssetId,
        id: &str,
        shader: &str,
        defaults: MaterialUniformDefaults,
    ) -> anyhow::Result<()> {
        Renderer::upload_material(self, asset, id, shader, defaults).map_err(anyhow::Error::from)
    }
    fn upload_particle_mesh(
        &mut self,
        asset: ParticleMeshAssetId,
        vertices: &[[f32; 2]],
        indices: &[u16],
    ) {
        Renderer::upload_particle_mesh(self, asset, vertices, indices);
    }
}

/// The target of [`LoadPlan::run_headless`], which runs only the units that
/// reach no device.
struct NoDevice;

impl LoadTarget for NoDevice {
    fn max_2d_texture_dimension(&self) -> u32 {
        unreachable!("a headless load packs nothing")
    }
    fn allocate_texture_handle(&mut self) -> TextureHandle {
        unreachable!("a headless load packs nothing")
    }
    fn upload_texture(&mut self, _: TextureHandle, _: &[u8], _: u32, _: u32, _: FilterMode) {
        unreachable!("a headless load uploads nothing")
    }
    fn upload_lit_texture(
        &mut self,
        _: TextureHandle,
        _: &[u8],
        _: &[u8],
        _: &[u8],
        _: u32,
        _: u32,
        _: FilterMode,
    ) {
        unreachable!("a headless load uploads nothing")
    }
    fn load_font(&mut self, _: &str, _: Vec<u8>) {
        unreachable!("a headless load loads no font")
    }
    fn set_font_families(&mut self, _: &HashMap<String, ResolvedFontFamily>, _: &[String]) {
        unreachable!("a headless load loads no font")
    }
    fn resolve_or_allocate_shader_id(&mut self, _: &str) -> ShaderAssetId {
        unreachable!("a headless load loads no shader")
    }
    fn white_texture(&mut self) -> TextureHandle {
        unreachable!("a headless load packs nothing")
    }
    fn upload_shader(&mut self, _: &str, _: String) -> anyhow::Result<()> {
        unreachable!("a headless load loads no shader")
    }
    fn upload_material(
        &mut self,
        _: MaterialAssetId,
        _: &str,
        _: &str,
        _: MaterialUniformDefaults,
    ) -> anyhow::Result<()> {
        unreachable!("a headless load uploads nothing")
    }
    fn upload_particle_mesh(&mut self, _: ParticleMeshAssetId, _: &[[f32; 2]], _: &[u16]) {
        unreachable!("a headless load uploads nothing")
    }
}

/// A load target with no device (redo 7.9): handles from a counter at 0,
/// as a renderer's, shader IDs by name, and every upload counted, not
/// made.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct HeadlessTarget {
    handles: TextureHandles,
    white: Option<TextureHandle>,
    shader_ids: HashMap<String, ShaderAssetId>,
    /// The handles uploaded to, in order.
    pub(crate) uploads: Vec<TextureHandle>,
}

#[cfg(test)]
impl LoadTarget for HeadlessTarget {
    fn max_2d_texture_dimension(&self) -> u32 {
        8192
    }
    fn allocate_texture_handle(&mut self) -> TextureHandle {
        self.handles.allocate()
    }
    fn upload_texture(&mut self, handle: TextureHandle, _: &[u8], _: u32, _: u32, _: FilterMode) {
        self.uploads.push(handle);
    }
    fn upload_lit_texture(
        &mut self,
        handle: TextureHandle,
        _: &[u8],
        _: &[u8],
        _: &[u8],
        _: u32,
        _: u32,
        _: FilterMode,
    ) {
        self.uploads.push(handle);
    }
    fn load_font(&mut self, _: &str, _: Vec<u8>) {}
    fn set_font_families(&mut self, _: &HashMap<String, ResolvedFontFamily>, _: &[String]) {}
    fn resolve_or_allocate_shader_id(&mut self, id: &str) -> ShaderAssetId {
        let next = ShaderAssetId(self.shader_ids.len() as u32);
        *self.shader_ids.entry(id.to_string()).or_insert(next)
    }
    fn white_texture(&mut self) -> TextureHandle {
        if let Some(white) = self.white {
            return white;
        }
        let white = self.handles.allocate();
        self.uploads.push(white);
        self.white = Some(white);
        white
    }
    fn upload_shader(&mut self, _: &str, _: String) -> anyhow::Result<()> {
        Ok(())
    }
    fn upload_material(
        &mut self,
        _: MaterialAssetId,
        _: &str,
        _: &str,
        _: MaterialUniformDefaults,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn upload_particle_mesh(&mut self, _: ParticleMeshAssetId, _: &[[f32; 2]], _: &[u16]) {}
}

/// One unit of a [`LoadPlan`] (redo 7.9), in `load_all`'s order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LoadUnit {
    /// One image file's sprites decoded: a sprite's file, or a sheet's,
    /// its frames cut from it.
    Sprites(PathBuf),
    /// One filter class's atlas packed, uploaded and registered.
    Pack(FilterMode),
    /// One animation file.
    Animation(String),
    /// One font face.
    Font(String),
    /// The font families and the fallback chain.
    FontFamilies,
    /// The UI themes, then the screen files (redo 7.20).
    Themes,
    /// The shaders.
    Shaders,
    /// The materials.
    Materials,
    /// One sound decoded.
    Sound(String),
    /// One tilemap.
    Tilemap(String),
    /// The particle meshes.
    ParticleMeshes,
    /// The particle configs.
    Particles,
    /// The physics layer names and materials.
    PhysicsNames,
    /// The prefabs and data entries, in two phases.
    Prefabs,
    /// Animation frames, tilesets and particle configs against the sprites
    /// and meshes, then every map's objects.
    CrossChecks,
}

/// What a plan's units took so far, in milliseconds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[non_exhaustive]
pub struct LoadTimings {
    /// Every unit.
    pub units: f64,
    /// The sprite units: decoding, and cutting sheet frames.
    pub sprites: f64,
    /// The sound units' decoding.
    pub sounds: f64,
    /// The pack units: packing, uploads and registering.
    pub pack: f64,
}

/// A merged manifest's load as units (redo 7.9), in [`load_all`]'s order:
/// each image file's sprites decoded, each filter's pack and upload, each
/// animation file, each font, the families, the themes, the shaders, the
/// materials, each sound, each tilemap, the particle meshes, the particle
/// configs, the physics names, the prefabs and data, the cross-checks. A
/// window runs them in slices between boot frames; [`load_all`] runs them
/// at once. A unit's error is the load's.
pub struct LoadPlan {
    manifest: ResolvedManifest,
    units: Vec<LoadUnit>,
    next: usize,
    /// Each sprite unit's sprite IDs, in its unit's order.
    files: Vec<Vec<String>>,
    /// Decoded sprites awaiting their filter's pack: nearest, then linear.
    decoded: [Vec<atlas::Decoded>; 2],
    timings: LoadTimings,
}

impl LoadPlan {
    /// The plan of `manifest`'s load into `world`: every sprite name, sheet
    /// frames included, interned in sorted order, and fresh animation,
    /// font, sound and tilemap registries, which their units fill.
    pub fn new(manifest: ResolvedManifest, world: &mut World) -> Self {
        world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing")
            .intern_sprites(manifest.sprites.keys().map(String::as_str));
        // Redo 7.9: the reserved white texel after the manifest's names, so
        // theirs stay.
        world
            .resource_mut::<AssetRegistry>()
            .intern_sprite(ENGINE_WHITE);
        world.insert_resource(AnimationRegistry::new());
        world.insert_resource(FontRegistry::new());
        world.insert_resource(SoundRegistry::new());
        world.insert_resource(TilemapRegistry::new());

        let files: Vec<Vec<String>> = atlas::sprites_by_file(&manifest)
            .into_iter()
            .map(|sprites| sprites.into_iter().map(|(id, _)| id.clone()).collect())
            .collect();
        let mut units: Vec<LoadUnit> = files
            .iter()
            .map(|ids| LoadUnit::Sprites(manifest.sprites[&ids[0]].path.clone()))
            .collect();
        units.extend([
            LoadUnit::Pack(FilterMode::Nearest),
            LoadUnit::Pack(FilterMode::Linear),
        ]);
        let named = |section: Vec<&String>, unit: fn(String) -> LoadUnit| {
            section.into_iter().cloned().map(unit).collect::<Vec<_>>()
        };
        units.extend(named(
            sorted_keys(&manifest.animations),
            LoadUnit::Animation,
        ));
        units.extend(named(font_load_order(&manifest), LoadUnit::Font));
        units.extend([
            LoadUnit::FontFamilies,
            LoadUnit::Themes,
            LoadUnit::Shaders,
            LoadUnit::Materials,
        ]);
        units.extend(named(sorted_keys(&manifest.sounds), LoadUnit::Sound));
        units.extend(named(sorted_keys(&manifest.tilemaps), LoadUnit::Tilemap));
        units.extend([
            LoadUnit::ParticleMeshes,
            LoadUnit::Particles,
            LoadUnit::PhysicsNames,
            LoadUnit::Prefabs,
            LoadUnit::CrossChecks,
        ]);
        Self {
            manifest,
            units,
            next: 0,
            files,
            decoded: Default::default(),
            timings: LoadTimings::default(),
        }
    }

    /// Every unit, in the order they run.
    #[must_use]
    pub fn units(&self) -> &[LoadUnit] {
        &self.units
    }

    /// Units run so far.
    #[must_use]
    pub fn done(&self) -> usize {
        self.next
    }

    /// Whether every unit has run.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.next == self.units.len()
    }

    /// The manifest it loads.
    #[must_use]
    pub fn manifest(&self) -> &ResolvedManifest {
        &self.manifest
    }

    /// What its units took so far.
    #[must_use]
    pub fn timings(&self) -> LoadTimings {
        self.timings
    }

    /// Runs the next unit into `world` and `renderer`; `Ok(false)` when
    /// none was left.
    ///
    /// # Errors
    ///
    /// The unit's load error, as [`load_all`] returns it.
    pub fn run_next(&mut self, world: &mut World, renderer: &mut Renderer) -> anyhow::Result<bool> {
        self.run_next_into(world, renderer)
    }

    /// Runs every unit left and gives the manifest back.
    ///
    /// # Errors
    ///
    /// The first unit's error.
    pub fn run(
        mut self,
        world: &mut World,
        renderer: &mut Renderer,
    ) -> anyhow::Result<ResolvedManifest> {
        while self.run_next(world, renderer)? {}
        Ok(self.manifest)
    }

    /// The manifest it loads, given back.
    #[must_use]
    pub fn into_manifest(self) -> ResolvedManifest {
        self.manifest
    }

    /// Runs every unit left with no renderer (redo 7.9), as the `boot` row's
    /// counter guard measures a load: sprite files decode and the sounds,
    /// animations, themes, maps, particles, physics names and prefabs load
    /// as a window's load does, while the packs, fonts, shaders, materials
    /// and particle meshes, which need a device, are skipped, so no sprite
    /// registers, and with them the cross-checks, which read the packed
    /// sprites. It gives the manifest back.
    ///
    /// # Errors
    ///
    /// The first unit's error.
    pub fn run_headless(mut self, world: &mut World) -> anyhow::Result<ResolvedManifest> {
        while let Some(unit) = self.units.get(self.next) {
            if matches!(
                unit,
                LoadUnit::Pack(_)
                    | LoadUnit::Font(_)
                    | LoadUnit::FontFamilies
                    | LoadUnit::Shaders
                    | LoadUnit::Materials
                    | LoadUnit::ParticleMeshes
                    | LoadUnit::CrossChecks
            ) {
                self.next += 1;
                continue;
            }
            self.run_next_into(world, &mut NoDevice)?;
        }
        Ok(self.manifest)
    }

    /// `run_next` into any load target.
    pub(crate) fn run_next_into(
        &mut self,
        world: &mut World,
        target: &mut dyn LoadTarget,
    ) -> anyhow::Result<bool> {
        let Some(unit) = self.units.get(self.next).cloned() else {
            return Ok(false);
        };
        let start = Instant::now();
        let manifest = &self.manifest;
        if self.next == 0 {
            check_reserved(manifest)?;
        }
        match &unit {
            LoadUnit::Sprites(_) => {
                let file = self.units[..self.next]
                    .iter()
                    .filter(|unit| matches!(unit, LoadUnit::Sprites(_)))
                    .count();
                let sprites: Vec<_> = self.files[file]
                    .iter()
                    .map(|id| (id, &manifest.sprites[id]))
                    .collect();
                for decoded in atlas::decode_file_sprites(&sprites)? {
                    let filter = manifest.sprites[&decoded.id].filter;
                    self.decoded[usize::from(filter == FilterMode::Linear)].push(decoded);
                }
            }
            LoadUnit::Pack(filter) => {
                let decoded =
                    std::mem::take(&mut self.decoded[usize::from(*filter == FilterMode::Linear)]);
                let pages = atlas::pack_filter(*filter, &decoded, world, target)?;
                log::info!(
                    "Packed {} {filter:?} sprites → {pages} atlas pages",
                    decoded.len()
                );
                // The last pack: the reserved white texel after every page.
                if *filter == FilterMode::Linear {
                    register_engine_white(world, target);
                }
            }
            LoadUnit::Animation(id) => load_animation_unit(manifest, id, world)?,
            LoadUnit::Font(id) => load_font_unit(manifest, id, world, target)?,
            LoadUnit::FontFamilies => set_font_families(manifest, target),
            LoadUnit::Themes => {
                load_ui_themes(manifest, world)?;
                load_ui_screens(manifest, world)?;
            }
            LoadUnit::Shaders => load_shaders_into(manifest, world, target),
            LoadUnit::Materials => load_materials_into(manifest, world, target),
            LoadUnit::Sound(id) => load_sound_unit(manifest, id, world)?,
            LoadUnit::Tilemap(id) => load_tilemap_unit(manifest, id, world)?,
            LoadUnit::ParticleMeshes => load_particle_meshes_into(manifest, world, target),
            LoadUnit::Particles => load_particles(manifest, world)?,
            LoadUnit::PhysicsNames => load_physics_names(manifest, world),
            LoadUnit::Prefabs => load_prefabs(manifest, world)?,
            LoadUnit::CrossChecks => cross_check(manifest, world)?,
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        self.timings.units += ms;
        match unit {
            LoadUnit::Sprites(_) => self.timings.sprites += ms,
            LoadUnit::Sound(_) => self.timings.sounds += ms,
            LoadUnit::Pack(_) => self.timings.pack += ms,
            _ => {}
        }
        self.next += 1;
        Ok(true)
    }
}

/// One boot group sprite on a page of its own (redo 7.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BootSprite {
    /// Its page, under a reserved handle.
    pub texture: TextureHandle,
    /// Its filter.
    pub filter: FilterMode,
    /// Its UVs on the page: the whole page.
    pub uv: UvRect,
    /// Its size in source pixels.
    pub size: [u32; 2],
}

/// A manifest's boot group loaded ahead of its plan (redo 7.9): its logo
/// and bar each on a page of its own under handles the renderer's counter
/// never mints, and its font's ID once loaded.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BootAssets {
    /// The logo, when the group names one.
    pub logo: Option<BootSprite>,
    /// The bar, when the group names one.
    pub bar: Option<BootSprite>,
    /// The font, when the group names one.
    pub font: Option<String>,
}

impl BootAssets {
    /// The pages it uploaded, which drop once the plan ends.
    #[must_use]
    pub fn pages(&self) -> Vec<TextureHandle> {
        [self.logo, self.bar]
            .into_iter()
            .flatten()
            .map(|sprite| sprite.texture)
            .collect()
    }
}

/// Loads `manifest`'s boot group before any unit (redo 7.9): each sprite
/// decoded, cut from its sheet when it is a frame, and uploaded alone under
/// a handle from `reserved`'s range, and the font into `target`.
pub(crate) fn load_boot_group(
    manifest: &ResolvedManifest,
    target: &mut dyn LoadTarget,
    reserved: &mut TextureHandles,
) -> anyhow::Result<BootAssets> {
    let boot = &manifest.boot;
    let mut sprite = |id: &Option<String>| -> anyhow::Result<Option<BootSprite>> {
        let Some(id) = id else {
            return Ok(None);
        };
        let entry = &manifest.sprites[id];
        let file = atlas::DecodedFile::open(&entry.path, None, None, |_, _| {})
            .map_err(|e| anyhow::anyhow!("Failed to decode '{}': {e}", entry.path.display()))?;
        let cut = file
            .cut(entry.rect)
            .ok_or_else(|| anyhow::anyhow!("Boot sprite '{id}''s frame leaves its image"))?;
        let texture = reserved.allocate_reserved();
        target.upload_texture(texture, &cut.rgba, cut.width, cut.height, entry.filter);
        Ok(Some(BootSprite {
            texture,
            filter: entry.filter,
            uv: UvRect::FULL,
            size: [cut.width, cut.height],
        }))
    };
    let logo = sprite(&boot.logo)?;
    let bar = sprite(&boot.bar)?;
    let font = match &boot.font {
        Some(id) => {
            let entry = &manifest.fonts[id];
            let data = std::fs::read(&entry.path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to read font '{id}' at '{}': {e}",
                    entry.path.display()
                )
            })?;
            target.load_font(id, data);
            Some(id.clone())
        }
        None => None,
    };
    Ok(BootAssets { logo, bar, font })
}

/// D-052 composition entry; D-017 duplicate IDs fatal.
pub fn load_all_merged(
    roots: &[impl AsRef<Path>],
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let merged = ResolvedManifest::load_and_merge_many(roots)?;
    let merged = LoadPlan::new(merged, world).run(world, renderer)?;
    world.insert_resource(LoadedManifest::new(merged));
    Ok(())
}

#[cfg(test)]
#[path = "../tests/asset_loader.rs"]
mod tests;
