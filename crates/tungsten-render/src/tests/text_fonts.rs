use std::collections::{HashMap, HashSet};
use std::path::Path;

use glyphon::fontdb;
use tungsten_core::assets::ResolvedFontFamily;

use super::*;

const SANS: &str = "Inter/static/Inter-Regular.ttf";
const SANS_BOLD: &str = "Inter/static/Inter-Bold.ttf";
const MONO: &str = "JetBrainsMono/static/JetBrainsMono-Regular.ttf";
const SERIF: &str = "SourceSerif4/static/SourceSerif4-Regular.ttf";

fn font_bytes(relative: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fonts")
        .join(relative);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn families(entries: &[(&str, &[&str])]) -> HashMap<String, ResolvedFontFamily> {
    entries
        .iter()
        .map(|&(id, faces)| {
            let faces = faces.iter().map(|face| (*face).to_string()).collect();
            (id.to_string(), ResolvedFontFamily::new(faces))
        })
        .collect()
}

fn chain(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_string()).collect()
}

/// The shared manifest's faces, loaded in sorted ID order, its families and
/// its chain, with packaged fonts only.
fn shared_engine() -> TextEngine {
    let mut engine = TextEngine::new(FontSource::Packaged);
    for (id, path) in [("mono", MONO), ("sans", SANS), ("sans_bold", SANS_BOLD)] {
        engine.load_font(id, font_bytes(path));
    }
    engine.set_font_families(
        &families(&[("sans", &["sans", "sans_bold"]), ("mono", &["mono"])]),
        &chain(&["sans", "mono"]),
    );
    engine
}

/// The fontdb face registered under manifest ID `id`.
fn face_of(engine: &TextEngine, id: &str) -> fontdb::ID {
    engine.font_attrs[id].face_ids[0]
}

/// The face and glyph of every glyph `content` shapes to in a section of
/// face `font_id`.
fn shaped(engine: &mut TextEngine, font_id: &str, content: &str) -> Vec<(fontdb::ID, u16)> {
    let section = TextSection {
        content: content.to_string(),
        font_id: font_id.to_string(),
        font_size: 16.0,
        line_height: 20.0,
        ..Default::default()
    };
    assert!(engine.update(&[section], 1920, 1080));
    text_areas(&engine.prepared, &engine.buffers)
        .flat_map(|area| {
            area.buffer
                .layout_runs()
                .flat_map(|run| run.glyphs.iter().map(|g| (g.font_id, g.glyph_id)))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn packaged_database_holds_exactly_the_loaded_faces() {
    let engine = shared_engine();
    let in_db: HashSet<fontdb::ID> = engine.font_system.db().faces().map(|f| f.id).collect();
    let loaded: HashSet<fontdb::ID> = engine
        .font_attrs
        .values()
        .flat_map(|stored| stored.face_ids.iter().copied())
        .collect();
    assert_eq!(in_db.len(), 3);
    assert_eq!(in_db, loaded);
}

#[test]
fn glyph_in_no_packaged_face_shapes_to_glyph_zero() {
    let mut engine = shared_engine();
    let glyphs = shaped(&mut engine, "sans", "\u{4E00}");
    assert_eq!(glyphs.len(), 1);
    assert_eq!(glyphs[0].1, 0);
}

#[test]
fn box_drawing_in_sans_falls_back_to_jetbrains_mono() {
    let mut engine = shared_engine();
    let mono = face_of(&engine, "mono");
    let glyphs = shaped(&mut engine, "sans", "\u{2500}");
    assert_eq!(glyphs.len(), 1);
    assert_eq!(glyphs[0].0, mono);
    assert_ne!(glyphs[0].1, 0);
}

#[test]
fn chain_order_picks_the_fallback_face_whatever_the_load_order() {
    let faces = [("serif", SERIF), ("sans", SANS), ("mono", MONO)];
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for order in orders {
        for (fallback, expected) in [(["sans", "mono"], "sans"), (["mono", "sans"], "mono")] {
            let mut engine = TextEngine::new(FontSource::Packaged);
            for index in order {
                let (id, path) = faces[index];
                engine.load_font(id, font_bytes(path));
            }
            engine.set_font_families(
                &families(&[
                    ("sans", &["sans"]),
                    ("mono", &["mono"]),
                    ("serif", &["serif"]),
                ]),
                &chain(&fallback),
            );
            let expected_face = face_of(&engine, expected);
            // U+0166 is in Inter and JetBrains Mono, not in Source Serif 4,
            // and has no decomposition.
            let glyphs = shaped(&mut engine, "serif", "\u{0166}");
            assert_eq!(glyphs.len(), 1, "{order:?} {fallback:?}");
            assert_eq!(glyphs[0].0, expected_face, "{order:?} {fallback:?}");
            assert_ne!(glyphs[0].1, 0, "{order:?} {fallback:?}");
            // U+012C is not in Source Serif 4 either, but decomposes: the
            // serif face draws it as I and a combining breve, with no
            // fallback.
            let serif = face_of(&engine, "serif");
            let glyphs = shaped(&mut engine, "serif", "\u{012C}");
            assert_eq!(glyphs.len(), 2, "{order:?} {fallback:?}");
            assert!(
                glyphs
                    .iter()
                    .all(|&(face, glyph)| face == serif && glyph != 0)
            );
        }
    }
}

#[test]
fn packaged_face_replaces_a_system_face_of_the_same_style() {
    let mut system = fontdb::Database::new();
    system.load_font_data(font_bytes(SANS));
    system.load_font_data(font_bytes(SERIF));
    let mut engine =
        TextEngine::with_database(FontSource::PackagedThenSystem, "en-US".to_string(), system);
    engine.load_font("sans", font_bytes(SANS));

    let packaged = face_of(&engine, "sans");
    let inter_regular: Vec<fontdb::ID> = engine
        .font_system
        .db()
        .faces()
        .filter(|face| {
            face.families.iter().any(|(name, _)| name == "Inter")
                && face.weight == fontdb::Weight::NORMAL
                && face.style == fontdb::Style::Normal
        })
        .map(|face| face.id)
        .collect();
    assert_eq!(inter_regular, [packaged]);
    // A system face no packaged face stands for stays.
    assert_eq!(engine.font_system.db().len(), 2);
}

#[test]
fn family_resolves_a_weight_to_the_nearest_face_ties_heavier() {
    let mut engine = shared_engine();
    for (weight, expected) in [
        (400, "sans"),
        (500, "sans"),
        (550, "sans_bold"),
        (600, "sans_bold"),
        (700, "sans_bold"),
        (900, "sans_bold"),
    ] {
        assert_eq!(
            engine.resolve_face("sans", weight, false).as_deref(),
            Some(expected),
            "weight {weight}"
        );
    }
    // No italic face: the upright one, logged once.
    assert_eq!(
        engine.resolve_face("sans", 400, true).as_deref(),
        Some("sans")
    );
    // An unknown family takes the chain's head.
    assert_eq!(
        engine.resolve_face("missing", 700, false).as_deref(),
        Some("sans_bold")
    );
}

#[test]
fn shipped_strings_draw_from_their_own_face() {
    let mut engine = shared_engine();
    let content: String = (0x20u8..0x7f)
        .map(char::from)
        .chain("\u{00B7}\u{2014}\u{2190}\u{2192}".chars())
        .collect();
    for id in ["sans", "sans_bold", "mono"] {
        let face = face_of(&engine, id);
        // Ligatures such as `<=>` make fewer glyphs than characters.
        let glyphs = shaped(&mut engine, id, &content);
        assert!(!glyphs.is_empty(), "{id}");
        for (index, (glyph_face, glyph)) in glyphs.into_iter().enumerate() {
            assert_ne!(glyph, 0, "{id}: glyph {index}");
            assert_eq!(glyph_face, face, "{id}: glyph {index}");
        }
    }
}

/// JetBrains Mono Regular with a trailing byte: the same family, weight and
/// style from other bytes.
fn mono_variant() -> Vec<u8> {
    let mut data = font_bytes(MONO);
    data.push(0);
    data
}

#[test]
fn identical_faces_under_two_ids_do_not_clash_and_other_bytes_do() {
    let mut engine = TextEngine::new(FontSource::Packaged);
    engine.load_font("engine_mono", font_bytes(MONO));
    engine.load_font("mono", font_bytes(MONO));
    assert_eq!(engine.clash("mono", face_of(&engine, "mono")), None);
    assert_eq!(
        engine.clash("engine_mono", face_of(&engine, "engine_mono")),
        None
    );

    engine.load_font("mono_variant", mono_variant());
    assert!(
        engine
            .clash("mono_variant", face_of(&engine, "mono_variant"))
            .is_some_and(|other| other == "mono" || other == "engine_mono")
    );
}

#[test]
fn engine_face_alone_shapes_text() {
    // As the engine registers it: one face, no families and no chain.
    let mut engine = TextEngine::new(FontSource::Packaged);
    engine.load_font("engine_mono", font_bytes(MONO));
    let face = face_of(&engine, "engine_mono");
    let glyphs = shaped(&mut engine, "engine_mono", "FPS 60.0 | 16.7 ms");
    assert!(!glyphs.is_empty());
    assert!(
        glyphs
            .iter()
            .all(|&(glyph_face, glyph)| glyph_face == face && glyph != 0)
    );
}

#[test]
fn face_loaded_under_a_registered_id_replaces_it() {
    let mut engine = TextEngine::new(FontSource::Packaged);
    engine.load_font("engine_mono", font_bytes(MONO));
    let embedded = face_of(&engine, "engine_mono");

    // A manifest face under the same ID, same family, weight and style.
    engine.load_font("engine_mono", mono_variant());
    let replacement = face_of(&engine, "engine_mono");
    assert_ne!(replacement, embedded);
    assert!(engine.font_system.db().face(embedded).is_none());
    assert_eq!(engine.font_system.db().len(), 1);
    let glyphs = shaped(&mut engine, "engine_mono", "HUD");
    assert!(
        glyphs
            .iter()
            .all(|&(glyph_face, glyph)| glyph_face == replacement && glyph != 0)
    );

    // Data with no face keeps the registration.
    engine.load_font("engine_mono", b"not a font".to_vec());
    assert_eq!(face_of(&engine, "engine_mono"), replacement);
    assert!(engine.font_system.db().face(replacement).is_some());
}

#[test]
fn override_beside_an_identical_mono_reports_their_clash() {
    let mut engine = TextEngine::new(FontSource::Packaged);
    engine.load_font("engine_mono", font_bytes(MONO));
    engine.load_font("mono", font_bytes(MONO));
    assert_eq!(engine.clash("mono", face_of(&engine, "mono")), None);

    engine.load_font("engine_mono", mono_variant());
    assert_eq!(
        engine
            .clash("engine_mono", face_of(&engine, "engine_mono"))
            .as_deref(),
        Some("mono")
    );
    assert_eq!(
        engine.clash("mono", face_of(&engine, "mono")).as_deref(),
        Some("engine_mono")
    );
}

#[test]
fn epoch_rises_on_font_and_chain_changes_only() {
    let mut engine = TextEngine::new(FontSource::Packaged);
    let start = engine.font_epoch();

    engine.load_font("sans", font_bytes(SANS));
    let loaded = engine.font_epoch();
    assert!(loaded > start, "load");

    engine.reload_font("sans", font_bytes(SANS));
    let reloaded = engine.font_epoch();
    assert!(reloaded > loaded, "reload");

    engine.load_font("mono", font_bytes(MONO));
    let fams = families(&[("sans", &["sans"]), ("mono", &["mono"])]);
    engine.set_font_families(&fams, &chain(&["sans", "mono"]));
    let chained = engine.font_epoch();
    engine.set_font_families(&fams, &chain(&["sans", "mono"]));
    assert_eq!(engine.font_epoch(), chained, "unchanged chain");

    engine.set_font_families(&fams, &chain(&["mono", "sans"]));
    assert!(engine.font_epoch() > chained, "changed chain");
}
