use std::path::{Path, PathBuf};

use tungsten_core::assets::ResolvedManifest;
use tungsten_core::ui::ScreenFile;

use super::*;

/// The repository's root, two folders above this crate.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn each_name_has_its_engines_schema() {
    assert_eq!(schema("ui_screen"), Some(screen_file::json_schema()));
    assert_eq!(schema("ui_theme"), Some(theme_file::json_schema()));
    assert_eq!(schema("prefab"), None);
    for name in NAMES {
        let schema = schema(name).unwrap();
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
    }
}

/// The screen files the repository keeps are canonical, the template's and
/// example 01's copy alike: each reads back as itself.
#[test]
fn the_repositorys_screen_files_are_canonical() {
    for manifest in [
        "templates/basic/assets/manifest.json",
        "examples/01_platformer/assets/manifest.json",
    ] {
        let manifest = ResolvedManifest::load(repo().join(manifest)).expect("the manifest loads");
        for (id, entry) in &manifest.ui_screens {
            let text = std::fs::read_to_string(&entry.path).expect("the file reads");
            let file =
                ScreenFile::from_json(&text).unwrap_or_else(|e| panic!("ui screen '{id}': {e}"));
            assert_eq!(
                file.to_canonical(),
                text,
                "ui screen '{id}' ({}) is not canonical",
                entry.path.display()
            );
        }
    }
}
