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
    assert_eq!(schema("room"), Some(room::json_schema()));
    assert_eq!(schema("tileset"), Some(room::tileset::json_schema()));
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

#[test]
fn schema_prints_room_and_tileset() {
    let dir = std::env::temp_dir().join(format!("tungsten-schema-rooms-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, title) in [("room", "Tungsten room"), ("tileset", "Tungsten tileset")] {
        let out = dir.join(format!("{name}.schema.json"));
        let args = SchemaArgs {
            name: name.to_owned(),
            out: Some(out.clone()),
        };
        assert!(run(&args).unwrap());
        let text = std::fs::read_to_string(&out).unwrap();
        let written: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(written, schema(name).unwrap());
        assert_eq!(written["title"], title);
        assert_eq!(
            written["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
    }
    assert_eq!(
        listed(),
        "ui_screen, ui_theme, room, tileset, batch or script"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn schema_names_batch_and_script() {
    assert_eq!(schema("batch"), Some(room::batch_schema()));
    assert_eq!(schema("script"), Some(script::json_schema()));
    assert_eq!(
        listed(),
        "ui_screen, ui_theme, room, tileset, batch or script"
    );
    for name in ["batch", "script"] {
        let schema = schema(name).unwrap();
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert!(schema["description"].is_string(), "{name}");
    }
    // The usage's schema line names every format.
    for name in NAMES {
        assert!(crate::args::USAGE.contains(name), "{name}");
    }
}
