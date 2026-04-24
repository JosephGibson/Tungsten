/// Headless asset smoke: manifest, animations, sounds; no GPU upload.
use std::path::Path;

use tungsten::asset_loader;
use tungsten_core::assets::ResolvedManifest;
use tungsten_core::{AssetRegistry, AudioCommands, SoundRegistry, World};

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/tungsten has a parent")
        .parent()
        .expect("crates/ has a parent (workspace root)")
        .to_path_buf()
}

#[test]
fn manifest_loads_and_all_files_exist() {
    let manifest_path = workspace_root().join("assets/manifest.json");
    ResolvedManifest::load(&manifest_path)
        .expect("manifest should parse and all referenced asset files should exist on disk");
}

#[test]
fn animations_decode_headless() {
    let manifest_path = workspace_root().join("assets/manifest.json");
    let manifest = ResolvedManifest::load(&manifest_path).unwrap();

    let mut world = World::new();
    world.insert_resource(AssetRegistry::new());

    asset_loader::load_animations(&manifest, &mut world)
        .expect("all animation JSON files should parse cleanly");
}

#[test]
fn sounds_decode_headless() {
    let manifest_path = workspace_root().join("assets/manifest.json");
    let manifest = ResolvedManifest::load(&manifest_path).unwrap();

    let mut world = World::new();
    world.insert_resource(SoundRegistry::new());
    world.insert_resource(AudioCommands::new());

    asset_loader::load_sounds(&manifest, &mut world).expect(
        "all sound files should decode without error — \
                 if this fails with 'unsupported codec', check that the \
                 required symphonia codec features are enabled in Cargo.toml",
    );
}
