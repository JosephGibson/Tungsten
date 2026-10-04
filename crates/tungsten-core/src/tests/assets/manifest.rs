use super::*;
use crate::assets::material::MaterialUniformDefaults;
use std::io::Write;

fn write_manifest(dir: &Path, content: &str) -> PathBuf {
    let path = dir.join("manifest.json");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    path
}

fn write_file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::File::create(&path).unwrap();
    path
}

#[test]
fn load_empty_manifest() {
    let tmp = tempdir();
    let path = write_manifest(&tmp, r#"{"sprites": {}, "animations": {}}"#);
    let m = ResolvedManifest::load(&path).unwrap();
    assert!(m.sprites.is_empty());
    assert!(m.animations.is_empty());
    assert!(m.fonts.is_empty());
}

#[test]
fn load_manifest_with_sprites() {
    let tmp = tempdir();
    write_file(&tmp, "hero.png");
    let path = write_manifest(
        &tmp,
        r#"{"sprites": {"hero": {"path": "hero.png", "filter": "nearest"}}}"#,
    );
    let m = ResolvedManifest::load(&path).unwrap();
    assert!(m.sprites.contains_key("hero"));
    assert_eq!(m.sprites["hero"].filter, FilterMode::Nearest);
}

#[test]
fn load_manifest_missing_sprite_file() {
    let tmp = tempdir();
    let path = write_manifest(&tmp, r#"{"sprites": {"hero": {"path": "missing.png"}}}"#);
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(matches!(err, ManifestError::MissingFile { .. }));
}

#[test]
fn load_manifest_missing_animation_file() {
    let tmp = tempdir();
    let path = write_manifest(&tmp, r#"{"animations": {"walk": {"path": "walk.json"}}}"#);
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(matches!(err, ManifestError::MissingAnimationFile { .. }));
}

#[test]
fn load_manifest_invalid_json() {
    let tmp = tempdir();
    let path = write_manifest(&tmp, "NOT JSON!");
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(matches!(err, ManifestError::Parse { .. }));
}

#[test]
fn load_manifest_nonexistent_file() {
    let err = ResolvedManifest::load("/nonexistent/manifest.json").unwrap_err();
    assert!(matches!(err, ManifestError::Io { .. }));
}

#[test]
fn merge_success() {
    let mut a = ResolvedManifest::default();
    a.sprites.insert(
        "hero".into(),
        ResolvedSprite {
            path: "hero.png".into(),
            filter: FilterMode::Nearest,
            normal_path: None,
            emissive_path: None,
        },
    );

    let mut b = ResolvedManifest::default();
    b.sprites.insert(
        "enemy".into(),
        ResolvedSprite {
            path: "enemy.png".into(),
            filter: FilterMode::Linear,
            normal_path: None,
            emissive_path: None,
        },
    );

    a.merge(b).unwrap();
    assert!(a.sprites.contains_key("hero"));
    assert!(a.sprites.contains_key("enemy"));
}

#[test]
fn merge_duplicate_sprite_is_error() {
    let mut a = ResolvedManifest::default();
    a.sprites.insert(
        "hero".into(),
        ResolvedSprite {
            path: "hero.png".into(),
            filter: FilterMode::Nearest,
            normal_path: None,
            emissive_path: None,
        },
    );

    let mut b = ResolvedManifest::default();
    b.sprites.insert(
        "hero".into(),
        ResolvedSprite {
            path: "hero2.png".into(),
            filter: FilterMode::Nearest,
            normal_path: None,
            emissive_path: None,
        },
    );

    let err = a.merge(b).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "hero"));
}

#[test]
fn merge_duplicate_animation_is_error() {
    let mut a = ResolvedManifest::default();
    a.animations.insert(
        "walk".into(),
        ResolvedAnimation {
            path: "walk.json".into(),
        },
    );

    let mut b = ResolvedManifest::default();
    b.animations.insert(
        "walk".into(),
        ResolvedAnimation {
            path: "walk2.json".into(),
        },
    );

    let err = a.merge(b).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "walk"));
}

#[test]
fn load_manifest_with_fonts() {
    let tmp = tempdir();
    write_file(&tmp, "sans.ttf");
    let path = write_manifest(&tmp, r#"{"fonts": {"sans": {"path": "sans.ttf"}}}"#);
    let m = ResolvedManifest::load(&path).unwrap();
    assert!(m.fonts.contains_key("sans"));
}

#[test]
fn load_manifest_missing_font_file() {
    let tmp = tempdir();
    let path = write_manifest(&tmp, r#"{"fonts": {"sans": {"path": "missing.ttf"}}}"#);
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(matches!(err, ManifestError::MissingFontFile { .. }));
}

#[test]
fn merge_duplicate_font_is_error() {
    let mut a = ResolvedManifest::default();
    a.fonts.insert(
        "sans".into(),
        ResolvedFont {
            path: "sans.ttf".into(),
        },
    );

    let mut b = ResolvedManifest::default();
    b.fonts.insert(
        "sans".into(),
        ResolvedFont {
            path: "sans2.ttf".into(),
        },
    );

    let err = a.merge(b).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "sans"));
}

#[test]
fn load_manifest_with_sounds() {
    let tmp = tempdir();
    write_file(&tmp, "blip.ogg");
    let path = write_manifest(
        &tmp,
        r#"{"sounds": {"sfx_blip": {"path": "blip.ogg", "looping": false, "volume": 0.8}}}"#,
    );
    let m = ResolvedManifest::load(&path).unwrap();
    assert!(m.sounds.contains_key("sfx_blip"));
    let s = &m.sounds["sfx_blip"];
    assert!(!s.looping);
    assert!((s.volume - 0.8).abs() < 1e-6);
}

#[test]
fn sound_defaults_looping_false_volume_one() {
    let tmp = tempdir();
    write_file(&tmp, "blip.ogg");
    let path = write_manifest(&tmp, r#"{"sounds": {"sfx_blip": {"path": "blip.ogg"}}}"#);
    let m = ResolvedManifest::load(&path).unwrap();
    let s = &m.sounds["sfx_blip"];
    assert!(!s.looping);
    assert!((s.volume - 1.0).abs() < 1e-6);
}

#[test]
fn load_manifest_missing_sound_file() {
    let tmp = tempdir();
    let path = write_manifest(&tmp, r#"{"sounds": {"sfx_blip": {"path": "missing.ogg"}}}"#);
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(matches!(err, ManifestError::MissingSoundFile { .. }));
}

#[test]
fn merge_duplicate_sound_is_error() {
    let mut a = ResolvedManifest::default();
    a.sounds.insert(
        "sfx_blip".into(),
        ResolvedSound {
            path: "blip.ogg".into(),
            looping: false,
            volume: 1.0,
        },
    );

    let mut b = ResolvedManifest::default();
    b.sounds.insert(
        "sfx_blip".into(),
        ResolvedSound {
            path: "blip2.ogg".into(),
            looping: false,
            volume: 1.0,
        },
    );

    let err = a.merge(b).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "sfx_blip"));
}

#[test]
fn load_manifest_with_tilemaps() {
    let tmp = tempdir();
    write_file(&tmp, "maps/demo.tmj");
    let path = write_manifest(&tmp, r#"{"tilemaps": {"demo": {"path": "maps/demo.tmj"}}}"#);
    let m = ResolvedManifest::load(&path).unwrap();
    assert!(m.tilemaps.contains_key("demo"));
}

#[test]
fn load_manifest_missing_tilemap_file() {
    let tmp = tempdir();
    let path = write_manifest(&tmp, r#"{"tilemaps": {"demo": {"path": "nope.tmj"}}}"#);
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(matches!(err, ManifestError::MissingTilemapFile { .. }));
}

#[test]
fn merge_duplicate_tilemap_is_error() {
    let mut a = ResolvedManifest::default();
    a.tilemaps.insert(
        "demo".into(),
        ResolvedTilemap {
            path: "demo.tmj".into(),
        },
    );
    let mut b = ResolvedManifest::default();
    b.tilemaps.insert(
        "demo".into(),
        ResolvedTilemap {
            path: "demo2.tmj".into(),
        },
    );
    let err = a.merge(b).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "demo"));
}

#[test]
fn load_manifest_with_particles() {
    let tmp = tempdir();
    write_file(&tmp, "particles/spark.json");
    let path = write_manifest(
        &tmp,
        r#"{"particles": {"spark": {"path": "particles/spark.json"}}}"#,
    );
    let m = ResolvedManifest::load(&path).unwrap();
    assert!(m.particles.contains_key("spark"));
}

#[test]
fn load_manifest_missing_particle_file() {
    let tmp = tempdir();
    let path = write_manifest(&tmp, r#"{"particles": {"spark": {"path": "nope.json"}}}"#);
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(matches!(err, ManifestError::MissingParticleFile { .. }));
}

#[test]
fn merge_duplicate_particle_is_error() {
    let mut a = ResolvedManifest::default();
    a.particles.insert(
        "spark".into(),
        ResolvedParticle {
            path: "spark.json".into(),
        },
    );
    let mut b = ResolvedManifest::default();
    b.particles.insert(
        "spark".into(),
        ResolvedParticle {
            path: "spark2.json".into(),
        },
    );
    let err = a.merge(b).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "spark"));
}

#[test]
fn load_manifest_with_shaders() {
    let tmp = tempdir();
    write_file(&tmp, "shaders/sprite.wgsl");
    let path = write_manifest(
        &tmp,
        r#"{"shaders": {"sprite": {"path": "shaders/sprite.wgsl"}}}"#,
    );
    let m = ResolvedManifest::load(&path).unwrap();
    assert!(m.shaders.contains_key("sprite"));
    assert!(m.shaders["sprite"].path.ends_with("sprite.wgsl"));
}

#[test]
fn load_manifest_missing_shader_file() {
    let tmp = tempdir();
    let path = write_manifest(&tmp, r#"{"shaders": {"sprite": {"path": "missing.wgsl"}}}"#);
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(matches!(err, ManifestError::MissingShaderFile { .. }));
}

#[test]
fn merge_duplicate_shader_is_error() {
    let mut a = ResolvedManifest::default();
    a.shaders.insert(
        "sprite".into(),
        ResolvedShader {
            path: "sprite.wgsl".into(),
        },
    );
    let mut b = ResolvedManifest::default();
    b.shaders.insert(
        "sprite".into(),
        ResolvedShader {
            path: "sprite2.wgsl".into(),
        },
    );
    let err = a.merge(b).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "sprite"));
}

#[test]
fn load_manifest_with_materials_resolves_shader_cross_ref() {
    let tmp = tempdir();
    write_file(&tmp, "shaders/flash.wgsl");
    let path = write_manifest(
        &tmp,
        r#"{
            "shaders": {"flash": {"path": "shaders/flash.wgsl"}},
            "materials": {"damage_flash": {"shader": "flash", "uniform_defaults": {"f32s": [0.5, 0.0, 0.0, 0.0]}}}
        }"#,
    );
    let m = ResolvedManifest::load(&path).unwrap();
    let material = m.materials.get("damage_flash").expect("material present");
    assert_eq!(material.shader, "flash");
    assert!((material.uniform_defaults.f32s[0] - 0.5).abs() < 1e-6);
}

#[test]
fn load_manifest_material_shader_missing_is_error() {
    let tmp = tempdir();
    let path = write_manifest(
        &tmp,
        r#"{"materials": {"damage_flash": {"shader": "nope"}}}"#,
    );
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(
        matches!(err, ManifestError::MaterialShaderMissing { id, shader } if id == "damage_flash" && shader == "nope")
    );
}

#[test]
fn merge_material_resolves_shader_from_sibling_manifest() {
    let mut a = ResolvedManifest::default();
    a.shaders.insert(
        "flash".into(),
        ResolvedShader {
            path: "flash.wgsl".into(),
        },
    );
    let mut b = ResolvedManifest::default();
    b.materials.insert(
        "damage_flash".into(),
        ResolvedMaterial {
            source_manifest: "b.json".into(),
            shader: "flash".into(),
            uniform_defaults: MaterialUniformDefaults::default(),
        },
    );
    a.merge(b).unwrap();
    assert!(a.materials.contains_key("damage_flash"));
}

#[test]
fn merge_material_missing_shader_is_error() {
    let mut a = ResolvedManifest::default();
    let mut b = ResolvedManifest::default();
    b.materials.insert(
        "damage_flash".into(),
        ResolvedMaterial {
            source_manifest: "b.json".into(),
            shader: "absent_shader".into(),
            uniform_defaults: MaterialUniformDefaults::default(),
        },
    );
    let err = a.merge(b).unwrap_err();
    assert!(
        matches!(err, ManifestError::MaterialShaderMissing { id, shader } if id == "damage_flash" && shader == "absent_shader")
    );
}

#[test]
fn merge_duplicate_material_is_error() {
    let mut a = ResolvedManifest::default();
    a.shaders.insert(
        "flash".into(),
        ResolvedShader {
            path: "flash.wgsl".into(),
        },
    );
    a.materials.insert(
        "damage_flash".into(),
        ResolvedMaterial {
            source_manifest: "a.json".into(),
            shader: "flash".into(),
            uniform_defaults: MaterialUniformDefaults::default(),
        },
    );
    let mut b = ResolvedManifest::default();
    b.materials.insert(
        "damage_flash".into(),
        ResolvedMaterial {
            source_manifest: "b.json".into(),
            shader: "flash".into(),
            uniform_defaults: MaterialUniformDefaults::default(),
        },
    );
    let err = a.merge(b).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "damage_flash"));
}

#[test]
fn particle_meshes_section_parses_inline_mesh() {
    let tmp = tempdir();
    let path = write_manifest(
        &tmp,
        r#"{"particle_meshes": {"tri": {"vertices": [[0.0, -7.0], [6.0, 7.0], [-6.0, 7.0]], "indices": [0, 1, 2]}}}"#,
    );
    let m = ResolvedManifest::load(&path).unwrap();
    let resolved = m.particle_meshes.get("tri").expect("mesh present");
    assert_eq!(
        resolved.mesh.vertices,
        vec![[0.0, -7.0], [6.0, 7.0], [-6.0, 7.0]]
    );
    assert_eq!(resolved.mesh.indices, vec![0, 1, 2]);
    assert_eq!(resolved.source_manifest, path.canonicalize().unwrap());
}

#[test]
fn invalid_particle_mesh_is_rejected() {
    let tmp = tempdir();
    let path = write_manifest(
        &tmp,
        r#"{"particle_meshes": {"tri": {"vertices": [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], "indices": [0, 1, 5]}}}"#,
    );
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(
        matches!(&err, ManifestError::InvalidParticleMesh { id, reason } if id == "tri" && reason.contains("out of range")),
        "got: {err}"
    );
}

#[test]
fn duplicate_particle_mesh_id_across_manifests_is_fatal() {
    let content = r#"{"particle_meshes": {"tri": {"vertices": [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], "indices": [0, 1, 2]}}}"#;
    let a = write_manifest(&tempdir(), content);
    let b = write_manifest(&tempdir(), content);
    let err = ResolvedManifest::load_and_merge_many(&[a, b]).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "tri"));
}

/// A manifest in a fresh directory declaring `faces` as fonts, with `rest`
/// appended to its top-level object.
fn font_manifest(faces: &[&str], rest: &str) -> PathBuf {
    let tmp = tempdir();
    let fonts: Vec<String> = faces
        .iter()
        .map(|face| {
            write_file(&tmp, &format!("{face}.ttf"));
            format!(r#""{face}": {{"path": "{face}.ttf"}}"#)
        })
        .collect();
    let separator = if rest.is_empty() { "" } else { ", " };
    write_manifest(
        &tmp,
        &format!(r#"{{"fonts": {{{}}}{separator}{rest}}}"#, fonts.join(", ")),
    )
}

#[test]
fn font_families_and_chain_load() {
    let path = font_manifest(
        &["sans", "sans_bold", "mono"],
        r#""font_families": {"sans": {"faces": ["sans", "sans_bold"]}, "mono": {"faces": ["mono"]}},
           "font_fallback": ["sans", "mono"]"#,
    );
    let m = ResolvedManifest::load(&path).unwrap();
    assert_eq!(
        m.font_families["sans"],
        ResolvedFontFamily::new(vec!["sans".into(), "sans_bold".into()])
    );
    assert_eq!(m.font_fallback, ["sans", "mono"]);
}

#[test]
fn font_family_with_missing_face_is_error() {
    let path = font_manifest(
        &["sans"],
        r#""font_families": {"sans": {"faces": ["sans", "sans_bold"]}, "a_first": {"faces": ["zz"]}}"#,
    );
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(
        matches!(&err, ManifestError::FontFamilyFaceMissing { family, face } if family == "a_first" && face == "zz"),
        "got: {err}"
    );
}

#[test]
fn unknown_fallback_family_is_error() {
    let path = font_manifest(
        &["sans"],
        r#""font_families": {"sans": {"faces": ["sans"]}}, "font_fallback": ["sans", "serif", "mono"]"#,
    );
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(
        matches!(&err, ManifestError::UnknownFallbackFamily { family } if family == "mono"),
        "got: {err}"
    );
}

#[test]
fn duplicate_font_family_across_roots_is_fatal() {
    let a = font_manifest(&["a"], r#""font_families": {"sans": {"faces": ["a"]}}"#);
    let b = font_manifest(&["b"], r#""font_families": {"sans": {"faces": ["b"]}}"#);
    let err = ResolvedManifest::load_and_merge_many(&[a, b]).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "sans"));
}

#[test]
fn font_family_faces_may_come_from_another_root() {
    // The family's root loads first; its faces arrive with the second root.
    let families = write_manifest(
        &tempdir(),
        r#"{"font_families": {"sans": {"faces": ["sans", "sans_bold"]}}, "font_fallback": ["sans"]}"#,
    );
    let faces = font_manifest(&["sans", "sans_bold"], "");
    let merged = ResolvedManifest::load_and_merge_many(&[families.clone(), faces]).unwrap();
    assert_eq!(merged.font_families["sans"].faces, ["sans", "sans_bold"]);
    // Alone, the family's root names faces nobody declares.
    let err = ResolvedManifest::load_and_merge_many(&[families]).unwrap_err();
    assert!(matches!(err, ManifestError::FontFamilyFaceMissing { .. }));
}

#[test]
fn fallback_chains_concatenate_in_root_order() {
    let a = font_manifest(
        &["a"],
        r#""font_families": {"a": {"faces": ["a"]}}, "font_fallback": ["a"]"#,
    );
    let b = font_manifest(
        &["b"],
        r#""font_families": {"b": {"faces": ["b"]}}, "font_fallback": ["b", "a"]"#,
    );
    let merged = ResolvedManifest::load_and_merge_many(&[a.clone(), b.clone()]).unwrap();
    assert_eq!(merged.font_fallback, ["a", "b"]);
    let merged = ResolvedManifest::load_and_merge_many(&[b, a]).unwrap();
    assert_eq!(merged.font_fallback, ["b", "a"]);
}

#[test]
fn merge_checks_font_family_faces() {
    let mut graph = ResolvedManifest::default();
    let mut other = ResolvedManifest::default();
    other
        .font_families
        .insert("sans".into(), ResolvedFontFamily::new(vec!["sans".into()]));
    let err = graph.merge(other).unwrap_err();
    assert!(
        matches!(err, ManifestError::FontFamilyFaceMissing { family, face } if family == "sans" && face == "sans")
    );
}

#[test]
fn default_filter_is_nearest() {
    let tmp = tempdir();
    write_file(&tmp, "hero.png");
    let path = write_manifest(&tmp, r#"{"sprites": {"hero": {"path": "hero.png"}}}"#);
    let m = ResolvedManifest::load(&path).unwrap();
    assert_eq!(m.sprites["hero"].filter, FilterMode::Nearest);
}

use std::sync::atomic::{AtomicU32, Ordering};
static COUNTER: AtomicU32 = AtomicU32::new(0);

fn tempdir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_test_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
