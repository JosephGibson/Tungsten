//! `tungsten check`'s content stage (M70, `D-182`): every file the game's
//! manifests name, and every unlisted JSON file under a manifest's folder as
//! a scene, loaded with the engine's own per-file loaders into the game's
//! World, past every error. `App::run` calls it in place of the window when
//! `TUNGSTEN_CHECK` names a report file.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tungsten_core::assets::manifest::ResolvedSprite;
use tungsten_core::assets::{
    AnimationData, AnimationRegistry, MaterialRegistry, ParticleConfig, ParticleConfigRegistry,
    ParticleMeshRegistry, PrefabRegistry, ResolvedManifest, SceneData, SoundData, SoundRegistry,
    StagedLoad, StagedLoadError, TilemapData, TilemapRegistry, UvRect,
};
use tungsten_core::ui::{
    ScreenFile, ScreenFileError, ScreenNames, Theme, ThemeFile, UiBindings, UiScreenRegistry,
    UiThemeRegistry, UiTree, widgets,
};
use tungsten_core::{AssetRegistry, TextureHandle, World};

use super::atlas::decode_sprite_siblings;
use super::{
    UiScreenPaths, UiThemePaths, missing_particle_render_ref, prefab, unknown_font_tokens,
};
use crate::screens::ScreenRegistry;

/// One error the check found: the file to fix, relative to the game's
/// folder and `/`-separated, and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckError {
    pub(crate) file: PathBuf,
    pub(crate) message: String,
}

/// The handle a checked sprite is registered under: no renderer allocates
/// one (`D-048`), and nothing draws from a check's World.
const NO_TEXTURE: TextureHandle = TextureHandle(u32::MAX);

/// Loads `roots` and everything they name into `world`, in the harness's
/// order, each registry inserted before what checks against it, and returns
/// every error, in the order found. `game_dir` is the game's folder: every
/// root and entry must lie inside it, and every path the errors name is
/// made relative to it. The plugins have built, so prefabs, data entries,
/// map objects and scenes check against every registered component and
/// data kind; a new section loader gains its line here in the same change.
pub(crate) fn check_files(
    world: &mut World,
    roots: &[PathBuf],
    game_dir: &Path,
) -> Vec<CheckError> {
    let mut errors = Errors::default();

    // 1. The manifest roots, merged, each path inside the game's folder.
    let (manifest, issues) = ResolvedManifest::load_for_check(roots, game_dir);
    for issue in issues {
        errors.push(&issue.manifest, issue.error.to_string());
    }

    // 2. Sprites and their siblings, registered so every later reference,
    // and a game's own asset check, finds them.
    check_sprites(&manifest, world, &mut errors);

    // 3. Animations, then each frame's sprite.
    let mut animations = AnimationRegistry::new();
    for id in sorted(manifest.animations.keys()) {
        let path = &manifest.animations[id].path;
        let sprites = asset_registry(world);
        let data = match AnimationData::load(path, sprites) {
            Ok(data) => data,
            Err(e) => {
                errors.push(path, format!("Failed to load animation '{id}': {e}"));
                continue;
            }
        };
        for (i, frame) in data.frames.iter().enumerate() {
            if sprites.sprite(frame.sprite).is_none() {
                let name = sprites.sprite_name(frame.sprite).unwrap_or("?");
                errors.push(
                    path,
                    format!("Animation '{id}' frame {i} references unknown sprite ID '{name}'"),
                );
            }
        }
        animations.insert_with_path(id.clone(), data, path.clone());
    }
    world.insert_resource(animations);

    // 4. UI themes; each font token against the families.
    let mut themes = UiThemeRegistry::new();
    let mut theme_paths = UiThemePaths::default();
    for id in sorted(manifest.ui_themes.keys()) {
        let path = &manifest.ui_themes[id].path;
        let theme = match ThemeFile::load(path) {
            Ok(file) => file.into_theme(),
            Err(e) => {
                errors.push(path, format!("Failed to load ui theme '{id}': {e}"));
                continue;
            }
        };
        for (token, family) in unknown_font_tokens(&theme, &manifest) {
            errors.push(
                path,
                format!(
                    "ui theme '{id}': fonts.{token} names family '{family}', which no font_families entry declares"
                ),
            );
        }
        themes.insert(id.clone(), theme);
        theme_paths.register(id.clone(), path.clone());
    }
    world.insert_resource(themes);
    world.insert_resource(theme_paths);

    // 4b. Screen files (redo 7.20), each against the names the plugins
    // registered and the themes give.
    check_screens(&manifest, world, &mut errors);

    // 5. Materials and particle meshes, as metadata: no renderer uploads them.
    let mut materials = world
        .remove_resource::<MaterialRegistry>()
        .unwrap_or_default();
    for id in sorted(manifest.materials.keys()) {
        let material = &manifest.materials[id];
        materials.allocate(
            id,
            material.source_manifest.clone(),
            material.shader.clone(),
            material.uniform_defaults,
        );
    }
    world.insert_resource(materials);
    let mut meshes = world
        .remove_resource::<ParticleMeshRegistry>()
        .unwrap_or_default();
    for id in sorted(manifest.particle_meshes.keys()) {
        meshes.insert(id, manifest.particle_meshes[id].mesh.clone());
    }
    world.insert_resource(meshes);

    // 6. Sounds.
    let mut sounds = SoundRegistry::new();
    for id in sorted(manifest.sounds.keys()) {
        let entry = &manifest.sounds[id];
        match SoundData::decode(&entry.path) {
            Ok(data) => {
                let handle = sounds.register(id.clone(), data, entry.volume, entry.looping);
                sounds.set_bus(handle, entry.bus);
                sounds.set_max_voices(handle, entry.max_voices);
            }
            Err(e) => errors.push(&entry.path, format!("Failed to decode sound '{id}': {e}")),
        }
    }
    world.insert_resource(sounds);

    // 7. Tilemaps, then each tileset entry's sprite.
    let mut tilemaps = TilemapRegistry::new();
    for id in sorted(manifest.tilemaps.keys()) {
        let path = &manifest.tilemaps[id].path;
        let data = match TilemapData::load(path) {
            Ok(data) => data,
            Err(e) => {
                errors.push(path, format!("Failed to load tilemap '{id}': {e}"));
                continue;
            }
        };
        let sprites = asset_registry(world);
        for (i, sprite) in data.tileset.iter().enumerate() {
            if sprites.get_sprite(sprite).is_none() {
                errors.push(
                    path,
                    format!("Tilemap '{id}' tileset[{i}] references unknown sprite ID '{sprite}'"),
                );
            }
        }
        tilemaps.insert_with_path(id.clone(), data, path.clone());
    }
    world.insert_resource(tilemaps);

    // 8. Particle configs, then the sprite or mesh each draws with.
    let mut particles = ParticleConfigRegistry::new();
    let mut loaded: Vec<(&String, &PathBuf, std::sync::Arc<ParticleConfig>)> = Vec::new();
    for id in sorted(manifest.particles.keys()) {
        let path = &manifest.particles[id].path;
        match ParticleConfig::load(path) {
            Ok(config) => {
                particles.register(id.clone(), path.clone(), config.clone());
                loaded.push((id, path, config));
            }
            Err(e) => errors.push(path, format!("Failed to load particle config '{id}': {e}")),
        }
    }
    world.insert_resource(particles);
    for (id, path, config) in loaded {
        if let Some((kind, name)) = missing_particle_render_ref(&config, world) {
            errors.push(
                path,
                format!("Particle config '{id}' references unknown {kind} ID '{name}'"),
            );
        }
    }

    // 9. The physics layer names and materials, which bodies name; the
    // tilesets and rooms (redo 7.12), whose collision layers name them, each
    // room's map registered; then the data entries and prefabs, in two
    // phases, and each prefab's sprite.
    super::load_physics_names(&manifest, world);
    super::room::check_rooms(&manifest, world, |path, message| {
        errors.push(path, message);
    });
    check_prefabs_and_data(&manifest, world, &mut errors);

    // 10. Each map's and each room's objects against the prefabs, and each
    // room's tileset sprites.
    if let Some(maps) = world.get_resource::<TilemapRegistry>() {
        for id in sorted(manifest.tilemaps.keys()) {
            if let Some(data) = maps.get(id)
                && let Err(e) = world.check_map_objects(id, data)
            {
                errors.push(&manifest.tilemaps[id].path, e.to_string());
            }
        }
        for id in sorted(manifest.rooms.keys()) {
            let Some(data) = maps.get(id) else {
                continue;
            };
            let path = &manifest.rooms[id].path;
            if let Err(e) = world.check_map_objects(id, data) {
                errors.push(path, format!("room '{id}': {e}"));
            }
            let sprites = world.get_resource::<AssetRegistry>();
            for (i, sprite) in data.tileset.iter().enumerate() {
                if sprites.is_none_or(|sprites| sprites.get_sprite(sprite).is_none()) {
                    errors.push(
                        path,
                        format!(
                            "room '{id}': its tileset's tiles[{i}] references unknown sprite ID '{sprite}'"
                        ),
                    );
                }
            }
        }
    }

    // 11. Scenes: every JSON file under a root's folder that no manifest
    // names.
    check_scenes(roots, game_dir, world, &mut errors);

    errors.finish(game_dir)
}

/// Parses each `ui_screens` file, then holds every name it uses to the
/// World's ([`screen_names`]), each error naming its field; inserts the
/// registry and paths when the manifests list screens, as a load does.
fn check_screens(manifest: &ResolvedManifest, world: &mut World, errors: &mut Errors) {
    if manifest.ui_screens.is_empty() {
        return;
    }
    let mut registry = UiScreenRegistry::new();
    let mut paths = UiScreenPaths::default();
    for id in sorted(manifest.ui_screens.keys()) {
        let path = &manifest.ui_screens[id].path;
        match ScreenFile::load(path) {
            Ok(file) => {
                registry.insert(id.clone(), file);
                paths.register(id.clone(), path.clone());
            }
            Err(ScreenFileError::Parse { source, .. }) => {
                errors.push(path, format!("ui screen '{id}': {source}"));
            }
            Err(e) => errors.push(path, format!("Failed to load ui screen '{id}': {e}")),
        }
    }
    let names = screen_names(world, &registry);
    for id in registry.ids() {
        let path = &manifest.ui_screens[id].path;
        for issue in registry
            .get(id)
            .map(|file| file.check(&names))
            .unwrap_or_default()
        {
            errors.push(path, format!("ui screen '{id}': {issue}"));
        }
    }
    world.insert_resource(registry);
    world.insert_resource(paths);
}

/// The names a screen file may use in `world`: the bindings, providers,
/// mounts and actions its [`UiBindings`] holds; the text styles and sizes
/// the built-in theme, the tree's and every loaded theme name, with the
/// widgets' size defaults; and the screens a `push` opens, the registered
/// ones and `registry`'s files, those a data screen refuses apart.
pub(crate) fn screen_names(world: &World, registry: &UiScreenRegistry) -> ScreenNames {
    let mut names = world
        .get_resource::<UiBindings>()
        .map(UiBindings::names)
        .unwrap_or_default();
    let built_in = Theme::default();
    let mut themes = vec![&built_in];
    themes.extend(world.get_resource::<UiTree>().map(UiTree::theme));
    if let Some(loaded) = world.get_resource::<UiThemeRegistry>() {
        themes.extend(loaded.ids().into_iter().filter_map(|id| loaded.get(id)));
    }
    for theme in themes {
        names.styles.extend(theme.names.text_styles.keys().cloned());
        names.sizes.extend(theme.names.sizes.keys().cloned());
    }
    names
        .sizes
        .extend(widgets::SIZES.iter().map(|(name, _)| (*name).to_owned()));
    if let Some(screens) = world.get_resource::<ScreenRegistry>() {
        names
            .screens
            .extend(screens.ids().into_iter().map(str::to_owned));
    }
    for id in registry.ids() {
        names.screens.insert(id.to_owned());
        if registry
            .get(id)
            .and_then(ScreenFile::data_screen_refusal)
            .is_some()
        {
            names.refused.insert(id.to_owned());
        }
    }
    names
}

/// Decodes each sprite's file once and its siblings against its size, and
/// checks each sheet frame against the image (redo 7.9), then registers each
/// sprite that decodes with a placeholder handle.
fn check_sprites(manifest: &ResolvedManifest, world: &mut World, errors: &mut Errors) {
    let mut sizes: HashMap<&Path, Option<(u32, u32)>> = HashMap::new();
    for id in sorted(manifest.sprites.keys()) {
        let sprite = &manifest.sprites[id];
        let size = *sizes.entry(sprite.path.as_path()).or_insert_with(|| {
            let image = match image::open(&sprite.path) {
                Ok(image) => image,
                Err(e) => {
                    errors.push(
                        &sprite.path,
                        format!("Failed to decode '{}': {e}", sprite.path.display()),
                    );
                    return None;
                }
            };
            let size = (image.width(), image.height());
            drop(image);
            decode_sprite_siblings(
                sprite.normal_path.as_deref(),
                sprite.emissive_path.as_deref(),
                size,
                |field, why| {
                    let file = match field {
                        "normal_map" => sprite.normal_path.as_ref(),
                        _ => sprite.emissive_path.as_ref(),
                    };
                    errors.push(
                        file.unwrap_or(&sprite.path),
                        format!("sprite '{id}' {field} {why}"),
                    );
                },
            );
            Some(size)
        });
        let Some(size) = size else {
            continue;
        };
        if let Some([x, y, width, height]) = sprite.rect
            && (x + width > size.0 || y + height > size.1)
        {
            errors.push(
                &sprite.path,
                format!(
                    "sprite '{id}''s frame {width}x{height} at ({x}, {y}) leaves its {}x{} image",
                    size.0, size.1
                ),
            );
            continue;
        }
        register_metadata(asset_registry(world), id, sprite, size, NO_TEXTURE, false);
    }
}

/// Registers sprite `id`'s metadata with no pixels behind it, as `tungsten
/// check` and the headless harness do (redo 7.9): its size (its frame's, or
/// `file`'s for a whole file), filter, siblings, nine-slice, frame, anchor
/// and named points, on `atlas` with full UVs, and lit on it when `lit`.
pub(crate) fn register_metadata(
    registry: &mut AssetRegistry,
    id: &str,
    sprite: &ResolvedSprite,
    file: (u32, u32),
    atlas: TextureHandle,
    lit: bool,
) {
    let (width, height) = sprite
        .rect
        .map_or(file, |[_, _, width, height]| (width, height));
    registry.register_sprite(
        id.to_string(),
        sprite.filter,
        width,
        height,
        sprite.path.clone(),
        atlas,
        UvRect::FULL,
        sprite.normal_path.clone(),
        sprite.emissive_path.clone(),
        lit.then_some(atlas),
    );
    if sprite.nine_slice.is_some() {
        registry.set_sprite_nine_slice(id, sprite.nine_slice);
    }
    if sprite.rect.is_some() || sprite.anchor.is_some() {
        registry.set_sprite_frame(id, sprite.rect, sprite.anchor);
    }
    if !sprite.points.is_empty() {
        registry.set_sprite_points(id, sprite.points.clone());
    }
}

/// Stages every data file by kind, then ID, and every prefab file by ID,
/// commits them for a check, then checks each prefab's sprite.
fn check_prefabs_and_data(manifest: &ResolvedManifest, world: &mut World, errors: &mut Errors) {
    let mut load = StagedLoad::new();
    for (kind, entries) in &manifest.data {
        for (id, entry) in entries {
            if let Err(e) = load.add_data(world, kind, id.clone(), entry.path.clone()) {
                errors.push(
                    &entry.path,
                    format!("Failed to load data `{kind}` entry `{id}`: {e}"),
                );
            }
        }
    }
    for id in sorted(manifest.prefabs.keys()) {
        let path = &manifest.prefabs[id].path;
        if let Err(e) = load.add_prefab(id.clone(), path.clone()) {
            errors.push(path, format!("Failed to load prefab '{id}': {e}"));
        }
    }
    for failure in load.commit_for_check(world) {
        let path = match &failure {
            StagedLoadError::Prefab { id, .. } => manifest.prefabs.get(id).map(|e| &e.path),
            StagedLoadError::Data { kind, id, .. } => manifest
                .data
                .get(kind)
                .and_then(|entries| entries.get(id))
                .map(|e| &e.path),
            _ => None,
        };
        let file = path.cloned().unwrap_or_default();
        errors.push(&file, prefab::failure(&failure));
    }
    let Some(prefabs) = world.get_resource::<PrefabRegistry>() else {
        return;
    };
    let sprites = world.get_resource::<AssetRegistry>();
    for id in sorted(manifest.prefabs.keys()) {
        let Some(sprite) = prefabs.get(id).and_then(|prefab| prefab.sprite.as_ref()) else {
            continue;
        };
        if sprites.is_none_or(|sprites| sprites.get_sprite(&sprite.asset_id).is_none()) {
            errors.push(
                &manifest.prefabs[id].path,
                format!(
                    "prefab '{id}' references unknown sprite ID '{}'",
                    sprite.asset_id
                ),
            );
        }
    }
}

/// Loads every `.json` file under each root's folder, hidden folders
/// skipped, that is neither a root nor named by a manifest entry, as a
/// scene (`D-046`), then checks each entry's sprite. Each root is resolved
/// alone for the files it names, so an entry the merge left out as a
/// repeated ID still counts as listed.
fn check_scenes(roots: &[PathBuf], game_dir: &Path, world: &World, errors: &mut Errors) {
    let mut listed: HashSet<PathBuf> = HashSet::new();
    let mut folders: Vec<PathBuf> = Vec::new();
    for root in roots {
        listed.insert(canonical(root));
        let (alone, _) = ResolvedManifest::load_for_check(std::slice::from_ref(root), game_dir);
        listed.extend(manifest_files(&alone).map(|path| canonical(path)));
        let folder = match root.parent() {
            Some(folder) if !folder.as_os_str().is_empty() => folder,
            _ => Path::new("."),
        };
        folders.push(canonical(folder));
    }
    let mut scenes: Vec<PathBuf> = Vec::new();
    for folder in &folders {
        json_files(folder, &mut scenes);
    }
    scenes.sort();
    scenes.dedup();
    let sprites = world.get_resource::<AssetRegistry>();
    for path in scenes.into_iter().filter(|path| !listed.contains(path)) {
        let scene = match SceneData::load(&path, world) {
            Ok(scene) => scene,
            Err(e) => {
                errors.push(
                    &path,
                    format!("listed in no manifest, so loaded as a scene: {e}"),
                );
                continue;
            }
        };
        for (i, entry) in scene.entities.iter().enumerate() {
            let Some(sprite) = &entry.sprite else {
                continue;
            };
            if sprites.is_none_or(|sprites| sprites.get_sprite(&sprite.asset_id).is_none()) {
                errors.push(
                    &path,
                    format!(
                        "scene entity {i} references unknown sprite ID '{}'",
                        sprite.asset_id
                    ),
                );
            }
        }
    }
}

/// Every file `manifest`'s entries name; a new file section joins here.
fn manifest_files(manifest: &ResolvedManifest) -> impl Iterator<Item = &PathBuf> {
    let sprites = manifest.sprites.values().flat_map(|sprite| {
        std::iter::once(&sprite.path)
            .chain(sprite.normal_path.as_ref())
            .chain(sprite.emissive_path.as_ref())
    });
    let data = manifest
        .data
        .values()
        .flat_map(|entries| entries.values().map(|entry| &entry.path));
    sprites
        .chain(manifest.animations.values().map(|e| &e.path))
        .chain(manifest.fonts.values().map(|e| &e.path))
        .chain(manifest.sounds.values().map(|e| &e.path))
        .chain(manifest.tilemaps.values().map(|e| &e.path))
        .chain(manifest.particles.values().map(|e| &e.path))
        .chain(manifest.shaders.values().map(|e| &e.path))
        .chain(manifest.ui_themes.values().map(|e| &e.path))
        .chain(manifest.ui_screens.values().map(|e| &e.path))
        .chain(manifest.prefabs.values().map(|e| &e.path))
        .chain(manifest.tilesets.values().map(|e| &e.path))
        .chain(manifest.rooms.values().map(|e| &e.path))
        .chain(data)
}

/// The `.json` files under `dir`, recursively, canonical, skipping folders
/// whose name starts with `.`.
fn json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() || (kind.is_symlink() && path.is_dir()) {
            if !hidden {
                json_files(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "json") {
            out.push(canonical(&path));
        }
    }
}

/// The World's `AssetRegistry`, inserted when it has none.
fn asset_registry(world: &mut World) -> &mut AssetRegistry {
    if !world.has_resource::<AssetRegistry>() {
        world.insert_resource(AssetRegistry::new());
    }
    world.resource_mut::<AssetRegistry>()
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn sorted<'a>(ids: impl Iterator<Item = &'a String>) -> Vec<&'a String> {
    let mut ids: Vec<&String> = ids.collect();
    ids.sort();
    ids
}

/// The errors found so far, with their files as the loaders name them.
#[derive(Default)]
struct Errors(Vec<(PathBuf, String)>);

impl Errors {
    fn push(&mut self, file: &Path, message: String) {
        self.0.push((file.to_path_buf(), message));
    }

    /// Each error with its file and every path in its message relative to
    /// `game_dir`, `/`-separated, and the message on one line.
    fn finish(self, game_dir: &Path) -> Vec<CheckError> {
        let mut prefixes: Vec<String> = Vec::new();
        for dir in [canonical(game_dir), game_dir.to_path_buf()] {
            let prefix = format!("{}{}", dir.display(), std::path::MAIN_SEPARATOR);
            if !prefixes.contains(&prefix) {
                prefixes.push(prefix);
            }
        }
        let relative = |text: &str| {
            let mut text = text.to_owned();
            for prefix in &prefixes {
                text = text.replace(prefix.as_str(), "");
            }
            text
        };
        self.0
            .into_iter()
            .map(|(file, message)| {
                let file = relative(&file.display().to_string());
                let message = slashes(&relative(&message)).replace(['\n', '\t'], " ");
                CheckError {
                    file: PathBuf::from(slashes(&file)),
                    message,
                }
            })
            .collect()
    }
}

/// `text`'s paths with `\` as `/` and no `\\?\` verbatim prefix on
/// Windows; unchanged elsewhere.
fn slashes(text: &str) -> String {
    if std::path::MAIN_SEPARATOR == '\\' {
        text.replace(r"\\?\", "").replace('\\', "/")
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
#[path = "../tests/check.rs"]
mod tests;
