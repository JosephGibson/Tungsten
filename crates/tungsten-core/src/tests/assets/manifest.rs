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
            nine_slice: None,
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
            nine_slice: None,
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
            nine_slice: None,
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
            nine_slice: None,
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
fn a_sound_volume_must_be_finite_and_not_negative() {
    // The mixer multiplies by it: 1e39 narrows to an infinite f32, and a
    // negative gain inverts the waveform.
    let tmp = tempdir();
    write_file(&tmp, "blip.ogg");
    for volume in ["1e39", "-2", "-0.5"] {
        let path = write_manifest(
            &tmp,
            &format!(r#"{{"sounds": {{"sfx_blip": {{"path": "blip.ogg", "volume": {volume}}}}}}}"#),
        );
        let err = ResolvedManifest::load(&path).unwrap_err();
        assert!(
            matches!(err, ManifestError::Parse { .. }),
            "{volume}: {err}"
        );
        assert!(err.to_string().contains("volume"), "{volume}: {err}");
    }
    let path = write_manifest(
        &tmp,
        r#"{"sounds": {"sfx_blip": {"path": "blip.ogg", "volume": 0}}}"#,
    );
    assert_eq!(
        ResolvedManifest::load(&path).unwrap().sounds["sfx_blip"].volume,
        0.0
    );
}

/// The sound `sfx_blip` loaded from a manifest naming `fields` beside its
/// path, or the load's error.
fn load_sound(tmp: &Path, fields: &str) -> Result<ResolvedSound, ManifestError> {
    write_file(tmp, "blip.ogg");
    let path = write_manifest(
        tmp,
        &format!(r#"{{"sounds": {{"sfx_blip": {{"path": "blip.ogg"{fields}}}}}}}"#),
    );
    ResolvedManifest::load(&path).map(|manifest| manifest.sounds["sfx_blip"].clone())
}

#[test]
fn a_sound_bus_is_music_or_effects_and_defaults_to_effects() {
    let tmp = tempdir();
    assert_eq!(load_sound(&tmp, "").unwrap().bus, AudioBus::Effects);
    assert_eq!(
        load_sound(&tmp, r#", "bus": "music""#).unwrap().bus,
        AudioBus::Music
    );
    assert_eq!(
        load_sound(&tmp, r#", "bus": "effects""#).unwrap().bus,
        AudioBus::Effects
    );
    for bus in [r#""voice""#, r#""Music""#, "3"] {
        let err = load_sound(&tmp, &format!(r#", "bus": {bus}"#)).unwrap_err();
        assert!(matches!(err, ManifestError::Parse { .. }), "{bus}: {err}");
        let message = err.to_string();
        assert!(message.contains("sound `sfx_blip`"), "{bus}: {message}");
    }
}

#[test]
fn a_sound_max_voices_is_1_to_64_and_defaults_to_64() {
    let tmp = tempdir();
    assert_eq!(load_sound(&tmp, "").unwrap().max_voices, 64);
    for limit in [1, 4, 64] {
        let sound = load_sound(&tmp, &format!(r#", "max_voices": {limit}"#)).unwrap();
        assert_eq!(sound.max_voices, limit);
    }
    for limit in ["0", "65", "-1", "2.5", r#""four""#] {
        let err = load_sound(&tmp, &format!(r#", "max_voices": {limit}"#)).unwrap_err();
        assert!(matches!(err, ManifestError::Parse { .. }), "{limit}: {err}");
        let message = err.to_string();
        assert!(message.contains("sound `sfx_blip`"), "{limit}: {message}");
    }
    let message = load_sound(&tmp, r#", "max_voices": 65"#)
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("max_voices 65 is outside 1..=64"),
        "{message}"
    );
}

#[test]
fn a_repeated_id_inside_one_section_is_an_error() {
    // `D-017`: duplicate IDs are fatal. serde's map kept the last entry,
    // so a repeat inside one manifest passed while one across two failed.
    let tmp = tempdir();
    write_file(&tmp, "a.png");
    write_file(&tmp, "b.png");
    write_file(&tmp, "blip.ogg");
    for json in [
        r#"{"sprites": {"hero": {"path": "a.png"}, "hero": {"path": "b.png"}}}"#,
        r#"{"sounds": {"blip": {"path": "blip.ogg"}, "blip": {"path": "blip.ogg"}}}"#,
    ] {
        let path = write_manifest(&tmp, json);
        let err = ResolvedManifest::load(&path).unwrap_err();
        assert!(matches!(err, ManifestError::Parse { .. }), "{json}: {err}");
        assert!(err.to_string().contains("duplicate ID"), "{json}: {err}");
    }
    let path = write_manifest(
        &tmp,
        r#"{"sprites": {"hero": {"path": "a.png"}, "villain": {"path": "b.png"}}}"#,
    );
    assert_eq!(ResolvedManifest::load(&path).unwrap().sprites.len(), 2);
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
            bus: AudioBus::Effects,
            max_voices: 64,
        },
    );

    let mut b = ResolvedManifest::default();
    b.sounds.insert(
        "sfx_blip".into(),
        ResolvedSound {
            path: "blip2.ogg".into(),
            looping: false,
            volume: 1.0,
            bus: AudioBus::Effects,
            max_voices: 64,
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

#[test]
fn load_manifest_with_ui_themes_and_nine_slice() {
    let tmp = tempdir();
    write_file(&tmp, "frame.png");
    write_file(&tmp, "icon.png");
    write_file(&tmp, "ui/themes/dark.json");
    let path = write_manifest(
        &tmp,
        r#"{"sprites": {"frame": {"path": "frame.png", "nine_slice": [4, 6, 4, 6]},
                        "icon": {"path": "icon.png"}},
            "ui_themes": {"dark": {"path": "ui/themes/dark.json"}}}"#,
    );
    let m = ResolvedManifest::load(&path).unwrap();
    assert_eq!(m.sprites["frame"].nine_slice, Some([4, 6, 4, 6]));
    assert_eq!(m.sprites["icon"].nine_slice, None);
    assert_eq!(m.ui_themes.len(), 1);
    assert!(m.ui_themes["dark"].path.ends_with("dark.json"));
    assert!(m.ui_themes["dark"].path.is_absolute());
}

#[test]
fn load_manifest_missing_ui_theme_file() {
    let tmp = tempdir();
    let path = write_manifest(
        &tmp,
        r#"{"ui_themes": {"dark": {"path": "ui/themes/dark.json"}}}"#,
    );
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(
        matches!(&err, ManifestError::MissingUiThemeFile { id, path } if id == "dark" && path.ends_with("dark.json")),
        "{err:?}"
    );
    assert!(
        err.to_string()
            .starts_with("ui theme 'dark' references missing file")
    );
}

#[test]
fn merge_duplicate_ui_theme_is_error() {
    let theme = ResolvedUiTheme {
        path: "dark.json".into(),
    };
    let mut a = ResolvedManifest::default();
    a.ui_themes.insert("dark".into(), theme.clone());
    let mut b = ResolvedManifest::default();
    b.ui_themes.insert("dark".into(), theme.clone());
    let err = a.merge(b).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "dark"));

    let mut c = ResolvedManifest::default();
    c.ui_themes.insert("light".into(), theme);
    a.merge(c).unwrap();
    assert_eq!(a.ui_themes.len(), 2);
}

#[test]
fn load_manifest_with_prefabs() {
    let tmp = tempdir();
    write_file(&tmp, "prefabs/slime.json");
    let path = write_manifest(
        &tmp,
        r#"{"prefabs": {"slime": {"path": "prefabs/slime.json"}}}"#,
    );
    let m = ResolvedManifest::load(&path).unwrap();
    assert_eq!(m.prefabs.len(), 1);
    assert!(m.prefabs["slime"].path.ends_with("prefabs/slime.json"));
    assert!(m.prefabs["slime"].path.is_absolute(), "canonicalized");
}

#[test]
fn load_manifest_missing_prefab_file() {
    let tmp = tempdir();
    let path = write_manifest(
        &tmp,
        r#"{"prefabs": {"slime": {"path": "prefabs/slime.json"}}}"#,
    );
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(
        matches!(&err, ManifestError::MissingPrefabFile { id, path } if id == "slime" && path.ends_with("slime.json")),
        "{err:?}"
    );
    assert!(
        err.to_string()
            .starts_with("prefab 'slime' references missing file: ")
    );
}

#[test]
fn a_repeated_prefab_id_fails_in_one_section_and_across_manifests() {
    let tmp = tempdir();
    write_file(&tmp, "a.json");
    let path = write_manifest(
        &tmp,
        r#"{"prefabs": {"slime": {"path": "a.json"}, "slime": {"path": "a.json"}}}"#,
    );
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(matches!(err, ManifestError::Parse { .. }), "{err}");
    assert!(err.to_string().contains("duplicate ID `slime`"), "{err}");

    let prefab = ResolvedPrefab::new(PathBuf::from("a.json"));
    let mut a = ResolvedManifest::default();
    a.prefabs.insert("slime".into(), prefab.clone());
    let mut b = ResolvedManifest::default();
    b.prefabs.insert("slime".into(), prefab.clone());
    let err = a.merge(b).unwrap_err();
    assert!(matches!(err, ManifestError::DuplicateId { id } if id == "slime"));
    // `bat` has a file of its own: one file backs one prefab (M61).
    let mut c = ResolvedManifest::default();
    c.prefabs
        .insert("bat".into(), ResolvedPrefab::new(PathBuf::from("b.json")));
    a.merge(c).unwrap();
    assert_eq!(a.prefabs.len(), 2);
}

#[test]
fn load_manifest_with_data_entries() {
    let tmp = tempdir();
    write_file(&tmp, "data/effects/burning.json");
    write_file(&tmp, "data/items/potion.json");
    let path = write_manifest(
        &tmp,
        r#"{"data": {
            "effects": {"burning": {"path": "data/effects/burning.json"}},
            "items": {"potion": {"path": "data/items/potion.json"}},
            "tags": {}
        }}"#,
    );
    let m = ResolvedManifest::load(&path).unwrap();
    assert_eq!(
        m.data.keys().collect::<Vec<_>>(),
        ["effects", "items", "tags"]
    );
    let burning = &m.data["effects"]["burning"].path;
    assert!(burning.ends_with("data/effects/burning.json"));
    assert!(burning.is_absolute(), "canonicalized");
    assert!(m.data["items"]["potion"].path.ends_with("potion.json"));
    assert!(m.data["tags"].is_empty());
}

#[test]
fn no_data_key_gives_an_empty_section() {
    let tmp = tempdir();
    let path = write_manifest(&tmp, r#"{"sprites": {}}"#);
    let m = ResolvedManifest::load(&path).unwrap();
    assert!(m.data.is_empty());
}

#[test]
fn a_repeated_data_kind_or_id_in_one_manifest_is_a_parse_error() {
    let tmp = tempdir();
    write_file(&tmp, "a.json");
    for (content, needle) in [
        (
            r#"{"data": {"effects": {"a": {"path": "a.json"}}, "effects": {}}}"#,
            "duplicate data kind `effects` in one manifest",
        ),
        (
            r#"{"data": {"effects": {"a": {"path": "a.json"}, "a": {"path": "a.json"}}}}"#,
            "duplicate ID `a` in one section",
        ),
    ] {
        let path = write_manifest(&tmp, content);
        let err = ResolvedManifest::load(&path).unwrap_err();
        assert!(matches!(err, ManifestError::Parse { .. }), "{err}");
        assert!(err.to_string().contains(needle), "{err}");
    }
}

#[test]
fn load_manifest_missing_data_file() {
    let tmp = tempdir();
    let path = write_manifest(
        &tmp,
        r#"{"data": {"effects": {"burning": {"path": "data/burning.json"}}}}"#,
    );
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(
        matches!(&err, ManifestError::MissingDataFile { kind, id, path }
            if kind == "effects" && id == "burning" && path.ends_with("burning.json")),
        "{err:?}"
    );
    assert!(
        err.to_string()
            .starts_with("data `effects` entry 'burning' references missing file: ")
    );
}

#[test]
fn a_data_id_is_unique_within_its_kind_across_roots() {
    let entry = |path: &str| ResolvedDataEntry::new(PathBuf::from(path));
    let mut a = ResolvedManifest::default();
    a.data
        .entry("effects".into())
        .or_default()
        .insert("burning".into(), entry("a.json"));
    let mut b = ResolvedManifest::default();
    b.data
        .entry("effects".into())
        .or_default()
        .insert("burning".into(), entry("b.json"));
    let err = a.clone().merge(b).unwrap_err();
    assert!(
        matches!(&err, ManifestError::DuplicateId { id } if id == "data.effects.burning"),
        "{err}"
    );

    // Two kinds, or a prefab and a kind, may share an ID.
    let mut c = ResolvedManifest::default();
    c.data
        .entry("items".into())
        .or_default()
        .insert("burning".into(), entry("c.json"));
    c.prefabs.insert(
        "burning".into(),
        ResolvedPrefab::new(PathBuf::from("d.json")),
    );
    a.merge(c).unwrap();
    assert_eq!(a.data["effects"].len() + a.data["items"].len(), 2);
}

#[test]
fn one_file_backs_one_prefab_or_data_entry() {
    let tmp = tempdir();
    write_file(&tmp, "shared.json");
    write_file(&tmp, "other.json");
    let path = write_manifest(
        &tmp,
        r#"{"prefabs": {"slime": {"path": "shared.json"}, "bat": {"path": "other.json"}},
            "data": {"effects": {"ooze": {"path": "shared.json"}, "acid": {"path": "shared.json"}}}}"#,
    );
    let err = ResolvedManifest::load(&path).unwrap_err();
    assert!(
        matches!(&err, ManifestError::SharedFile { path, first, second }
            if path.ends_with("shared.json") && first == "data.effects.acid" && second == "data.effects.ooze"),
        "the smallest pair: {err:?}"
    );
    assert!(err.to_string().ends_with(
        "backs both data.effects.acid and data.effects.ooze; a file backs one prefab or data entry"
    ));

    // Across two roots, a prefab in one and a data entry in the other.
    let one = tempdir();
    let two = tempdir();
    write_file(&one, "shared.json");
    write_manifest(&one, r#"{"prefabs": {"slime": {"path": "shared.json"}}}"#);
    // The sibling root names the same file through `..`.
    let sibling = one.file_name().unwrap().to_str().unwrap();
    write_manifest(
        &two,
        &format!(
            r#"{{"data": {{"effects": {{"ooze": {{"path": "../{sibling}/shared.json"}}}}}}}}"#
        ),
    );
    let roots = [one.join("manifest.json"), two.join("manifest.json")];
    let err = ResolvedManifest::load_and_merge_many(&roots).unwrap_err();
    assert!(
        matches!(&err, ManifestError::SharedFile { first, second, .. }
            if first == "data.effects.ooze" && second == "prefabs.slime"),
        "{err:?}"
    );
    let mut a = ResolvedManifest::load(&roots[0]).unwrap();
    let b = ResolvedManifest::load(&roots[1]).unwrap();
    assert!(matches!(a.merge(b), Err(ManifestError::SharedFile { .. })));
}
