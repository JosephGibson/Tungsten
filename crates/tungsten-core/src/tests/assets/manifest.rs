use super::*;
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
        },
    );

    let mut b = ResolvedManifest::default();
    b.sprites.insert(
        "enemy".into(),
        ResolvedSprite {
            path: "enemy.png".into(),
            filter: FilterMode::Linear,
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
        },
    );

    let mut b = ResolvedManifest::default();
    b.sprites.insert(
        "hero".into(),
        ResolvedSprite {
            path: "hero2.png".into(),
            filter: FilterMode::Nearest,
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
