//! `tungsten check`'s content stage over seeded files (M70).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use serde::Deserialize;
use tungsten_core::{AssetRegistry, Config, MissingAsset, World};

use super::*;
use crate::App;
use crate::plugins::DefaultPlugins;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A temp folder of its own, removed on drop.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tungsten_check_{}_{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir.canonicalize().unwrap())
    }

    fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }

    fn png(&self, rel: &str, size: u32) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbaImage::new(size, size).save(&path).unwrap();
        path
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A test component with no asset check.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    #[serde(default)]
    #[allow(dead_code)]
    strength: f32,
}

/// A data kind that names a prefab.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Loot {
    prefab: String,
}

/// A World as a game's `main` builds it: the default and kit plugins, a
/// test component and a data kind naming prefabs.
fn game_world() -> App {
    let mut app = App::with_plugins(Config::default(), DefaultPlugins::set())
        .expect("App::with_plugins failed");
    #[cfg(feature = "kit")]
    app.add_plugins(crate::kit::KitPlugins::set());
    let world = app.world_mut();
    world.register_component::<Probe>("probe");
    world.register_data_kind_with_assets::<Loot>("drops", |loot, world| {
        world.check_prefab_id("prefab", &loot.prefab)
    });
    app
}

/// `text` as a JSON string, so a Windows path's `\` survives the parse.
fn json_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn tmj(sprite: &str) -> String {
    format!(
        r#"{{
            "tilewidth": 16, "tileheight": 16, "width": 1, "height": 1,
            "tilesets": [{{"firstgid": 1, "tiles": [
                {{"id": 0, "properties": [{{"name": "sprite_id", "value": "{sprite}"}}]}}
            ]}}],
            "layers": [{{"type": "tilelayer", "name": "bg", "data": [1]}}]
        }}"#
    )
}

fn particle(mesh: &str) -> String {
    format!(
        r#"{{
            "sprite": "hero", "render": {{"kind": "mesh", "mesh": "{mesh}"}},
            "max_alive": 10, "seed": 1, "blend": "alpha",
            "emission": {{"kind": "continuous", "rate_hz": 10.0}},
            "lifetime": {{"min": 0.5, "max": 1.0}},
            "initial_velocity": {{"kind": "radial", "speed": {{"min": 10.0, "max": 20.0}}}},
            "gravity": [0.0, 0.0], "drag_per_sec": 0.0,
            "angular_velocity": {{"min": 0.0, "max": 0.0}},
            "start_scale": {{"min": 1.0, "max": 1.0}},
            "scale_over_life": [[0.0, 1.0], [1.0, 1.0]],
            "color_over_life": [[0.0, [1.0, 1.0, 1.0, 1.0]], [1.0, [1.0, 1.0, 1.0, 1.0]]],
            "alpha_over_life": [[0.0, 1.0], [1.0, 1.0]],
            "tint": [1.0, 1.0, 1.0, 1.0]
        }}"#
    )
}

/// Two roots in `game` and the files they name, with each of fourteen
/// errors seeded when `broken`; `outside` holds a sprite outside the game's
/// folder. Returns the roots.
fn seed(game: &Folder, outside: &Folder, broken: bool) -> Vec<PathBuf> {
    let far = outside.png("outside.png", 16);
    game.png("assets/sprites/hero.png", 16);
    game.png("assets/sprites/lit.png", 16);
    game.png("assets/sprites/lit_n.png", if broken { 8 } else { 16 });
    game.png("levels/bonus.png", 16);
    let extra_sprites = if broken {
        format!(
            r#", "gone": {{"path": "sprites/gone.png"}}, "far": {{"path": {}}}"#,
            json_string(&far.display().to_string())
        )
    } else {
        String::new()
    };
    let root = game.write(
        "assets/manifest.json",
        &format!(
            r#"{{
                "sprites": {{
                    "hero": {{"path": "sprites/hero.png"}},
                    "lit": {{"path": "sprites/lit.png", "normal_map": "sprites/lit_n.png"}}
                    {extra_sprites}
                }},
                "animations": {{"walk": {{"path": "anim/walk.json"}}}},
                "ui_themes": {{"main": {{"path": "ui/theme.json"}}}},
                "tilemaps": {{"level": {{"path": "maps/level.tmj"}}}},
                "particle_meshes": {{"tri": {{"vertices": [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], "indices": [0, 1, 2]}}}},
                "particles": {{"spark": {{"path": "particles/spark.json"}}}},
                "prefabs": {{
                    "broken": {{"path": "prefabs/broken.json"}},
                    "ghost": {{"path": "prefabs/ghost.json"}}
                }},
                "data": {{"drops": {{
                    "a": {{"path": "data/drop_a.json"}},
                    "b": {{"path": "data/drop_b.json"}}
                }}}}
            }}"#
        ),
    );
    let second = game.write(
        "levels/manifest.json",
        if broken {
            r#"{"sprites": {"hero": {"path": "bonus.png"}}}"#
        } else {
            r#"{"sprites": {"bonus": {"path": "bonus.png"}}}"#
        },
    );
    let pick = |bad: &'static str, good: &'static str| if broken { bad } else { good };
    game.write(
        "assets/anim/walk.json",
        &format!(
            r#"{{"looping": true, "frames": [{{"sprite": "{}", "duration_ms": 100}}]}}"#,
            pick("ghost", "hero")
        ),
    );
    game.write(
        "assets/ui/theme.json",
        &format!(
            r#"{{"tokens": {{"fonts": {{"body": "engine_mono", "heading": "engine_mono", "mono": "{}"}}}}}}"#,
            pick("ghost", "engine_mono")
        ),
    );
    game.write("assets/maps/level.tmj", &tmj(pick("ghost", "hero")));
    game.write(
        "assets/particles/spark.json",
        &particle(pick("ghost_mesh", "tri")),
    );
    game.write(
        "assets/prefabs/broken.json",
        pick(
            r#"{"components": {"no_such_component": {}}}"#,
            r#"{"components": {"probe": {"strength": 1.0}}}"#,
        ),
    );
    game.write(
        "assets/prefabs/ghost.json",
        &format!(
            r#"{{"sprite": {{"asset_id": "{}"}}}}"#,
            pick("ghost", "hero")
        ),
    );
    game.write(
        "assets/data/drop_a.json",
        &format!(r#"{{"prefab": "{}"}}"#, pick("nope", "ghost")),
    );
    // Names the prefab that fails its check, which stays in place.
    game.write("assets/data/drop_b.json", r#"{"prefab": "broken"}"#);
    game.write(
        "assets/scenes/far.json",
        &format!(
            r#"{{"entities": [{{"transform": {{"position": [{}, 0.0]}}, "sprite": {{"asset_id": "hero"}}}}]}}"#,
            pick("1e30", "0.0")
        ),
    );
    game.write(
        "assets/scenes/typo.json",
        pick(r#"{"entites": []}"#, r#"{"entities": []}"#),
    );
    game.write("assets/notes.json", pick("not json", "{}"));
    // A hidden folder is skipped.
    game.write("assets/.cache/junk.json", "not json either");
    vec![root, second]
}

fn files(errors: &[CheckError]) -> Vec<String> {
    let mut files: Vec<String> = errors
        .iter()
        .map(|error| error.file.display().to_string())
        .collect();
    files.sort();
    files
}

#[test]
fn every_seeded_error_is_reported_with_its_file() {
    let game = Folder::new();
    let outside = Folder::new();
    let roots = seed(&game, &outside, true);
    let mut app = game_world();
    let errors = check_files(app.world_mut(), &roots, &game.0);
    let report: Vec<String> = errors
        .iter()
        .map(|e| format!("{}: {}", e.file.display(), e.message))
        .collect();
    assert_eq!(errors.len(), 14, "{report:#?}");
    assert_eq!(
        files(&errors),
        [
            "assets/anim/walk.json",
            "assets/data/drop_a.json",
            "assets/manifest.json",
            "assets/manifest.json",
            "assets/maps/level.tmj",
            "assets/notes.json",
            "assets/particles/spark.json",
            "assets/prefabs/broken.json",
            "assets/prefabs/ghost.json",
            "assets/scenes/far.json",
            "assets/scenes/typo.json",
            "assets/sprites/lit_n.png",
            "assets/ui/theme.json",
            "levels/manifest.json",
        ],
        "{report:#?}"
    );
    // Paths inside messages are relative too, but for the one outside.
    let game_dir = game.0.display().to_string();
    for error in &errors {
        assert!(
            !error.message.contains(&game_dir) || error.message.contains("outside"),
            "{}",
            error.message
        );
    }
    let message = |file: &str| {
        errors
            .iter()
            .find(|e| e.file == Path::new(file))
            .map(|e| e.message.clone())
            .unwrap()
    };
    assert!(
        message("assets/notes.json").starts_with("listed in no manifest, so loaded as a scene")
    );
    assert!(message("assets/data/drop_a.json").contains("unknown prefab `nope`"));
    assert!(message("assets/sprites/lit_n.png").contains("normal_map dimensions"));
    assert!(message("levels/manifest.json").contains("duplicate asset ID 'hero'"));
    assert!(message("assets/prefabs/ghost.json").contains("unknown sprite ID 'ghost'"));
    // No root is loaded as a scene, and the hidden folder is skipped.
    assert!(
        !errors
            .iter()
            .any(|e| e.message.contains("loaded as a scene") && e.file.ends_with("manifest.json"))
    );
    assert!(!errors.iter().any(|e| e.file.starts_with("assets/.cache")));
}

#[test]
fn the_same_files_with_every_error_fixed_report_none() {
    let game = Folder::new();
    let outside = Folder::new();
    let roots = seed(&game, &outside, false);
    let mut app = game_world();
    let errors = check_files(app.world_mut(), &roots, &game.0);
    assert!(errors.is_empty(), "{errors:#?}");
    let world = app.world_mut();
    assert!(
        world
            .resource::<AssetRegistry>()
            .get_sprite("bonus")
            .is_some()
    );
}

/// A component whose value names a sprite, checked in `AssetRegistry`.
#[derive(Debug, Deserialize)]
struct Badge {
    sprite: String,
}

#[test]
fn a_games_asset_check_finds_the_manifest_sprites() {
    let game = Folder::new();
    game.png("assets/hero.png", 16);
    game.write(
        "assets/good.json",
        r#"{"components": {"badge": {"sprite": "hero"}}}"#,
    );
    game.write(
        "assets/bad.json",
        r#"{"components": {"badge": {"sprite": "nobody"}}}"#,
    );
    let root = game.write(
        "assets/manifest.json",
        r#"{"sprites": {"hero": {"path": "hero.png"}},
            "prefabs": {"good": {"path": "good.json"}, "bad": {"path": "bad.json"}}}"#,
    );
    let mut app = App::with_plugins(Config::default(), DefaultPlugins::set()).unwrap();
    app.world_mut().register_component_with_assets(
        "badge",
        |badge: &Badge, world: &World| {
            let found = world
                .get_resource::<AssetRegistry>()
                .is_some_and(|sprites| sprites.get_sprite(&badge.sprite).is_some());
            if found {
                Ok(())
            } else {
                Err(MissingAsset::new("sprite", "sprite", &badge.sprite))
            }
        },
        |badge: Badge, _: &tungsten_core::SpawnContext<'_>| (badge,),
    );
    let errors = check_files(app.world_mut(), &[root], &game.0);
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert_eq!(errors[0].file, Path::new("assets/bad.json"));
    assert!(
        errors[0].message.contains("nobody"),
        "{}",
        errors[0].message
    );
}
