use std::path::Path;

use glyphon::fontdb;

use super::*;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

fn font_bytes(relative: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fonts")
        .join(relative);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// A cache whose font database holds only the bundled mono font: no system
/// fonts and no GPU.
fn mono_cache() -> TextLayoutCache {
    let font_system =
        FontSystem::new_with_locale_and_db("en-US".to_string(), fontdb::Database::new());
    let mut cache = TextLayoutCache::new(font_system);
    cache.load_font(
        "mono",
        font_bytes("JetBrainsMono/static/JetBrainsMono-Regular.ttf"),
    );
    cache
}

fn section(content: &str, row: u32) -> TextSection {
    TextSection {
        content: content.to_string(),
        font_id: "mono".to_string(),
        font_size: 14.0,
        line_height: 18.0,
        color: [230, 236, 255, 255],
        position: [16.0, 16.0 + 20.0 * row as f32],
        bounds: None,
    }
}

/// One frame as the pipeline runs it: `update`, then glyphon's prepare, which
/// `mark_prepared` stands in for. Returns whether glyphon would have run.
fn frame(cache: &mut TextLayoutCache, sections: &[TextSection]) -> bool {
    let changed = cache.update(sections, WIDTH, HEIGHT);
    if changed {
        cache.mark_prepared(WIDTH, HEIGHT);
    }
    changed
}

/// Glyph ID, x offset and advance of every laid-out glyph, per text area.
fn glyphs(cache: &TextLayoutCache) -> Vec<Vec<(u16, f32, f32)>> {
    text_areas(&cache.prepared, &cache.buffers)
        .map(|area| {
            area.buffer
                .layout_runs()
                .flat_map(|run| run.glyphs.iter().map(|g| (g.glyph_id, g.x, g.w)))
                .collect()
        })
        .collect()
}

#[test]
fn cache_stays_bounded_while_text_changes_every_frame() {
    const SECTIONS: u32 = 24;
    let mut cache = mono_cache();
    for index in 0..1_000u32 {
        let sections: Vec<_> = (0..SECTIONS)
            .map(|row| {
                section(
                    &format!("{row:04} f{index:06} {:08x}", row * 7919 + index),
                    row,
                )
            })
            .collect();
        assert!(frame(&mut cache, &sections), "frame {index}");
        assert!(
            cache.buffers.len() <= 3 * SECTIONS as usize,
            "frame {index}: {} layouts",
            cache.buffers.len()
        );
        assert!(cache.spare.len() <= SPARE_BUFFER_CAP, "frame {index}");
    }
    // Steady state: each frame's evictions feed its misses.
    assert_eq!(cache.buffers.len(), 3 * SECTIONS as usize);
    assert!(cache.spare.is_empty());
    assert_eq!(glyphs(&cache).len(), SECTIONS as usize);
}

#[test]
fn spare_list_is_capped() {
    let sections_per_frame = SPARE_BUFFER_CAP as u32 + 40;
    let mut cache = mono_cache();
    for index in 0..4u32 {
        let sections: Vec<_> = (0..sections_per_frame)
            .map(|row| section(&format!("{index} {row}"), row))
            .collect();
        frame(&mut cache, &sections);
    }
    assert!(cache.spare.is_empty());
    // One section of the last frame stays: a whole frame of layouts ages out
    // and none is taken back.
    assert!(frame(&mut cache, &[section("3 0", 0)]));
    assert_eq!(cache.spare.len(), SPARE_BUFFER_CAP);
}

#[test]
fn identical_sections_share_one_layout() {
    let mut cache = mono_cache();
    let mut twin = section("same text", 0);
    twin.position = [400.0, 300.0];
    assert!(frame(&mut cache, &[section("same text", 0), twin]));
    assert_eq!(cache.buffers.len(), 1);
    let drawn = glyphs(&cache);
    assert_eq!(drawn.len(), 2);
    assert_eq!(drawn[0], drawn[1]);
}

#[test]
fn unchanged_frame_skips_prepare() {
    let mut cache = mono_cache();
    let sections = [section("static line", 0), section("another line", 1)];

    assert!(cache.update(&sections, WIDTH, HEIGHT));
    // Glyphon has not prepared these sections yet (or failed to): no skip.
    assert!(cache.update(&sections, WIDTH, HEIGHT));
    cache.mark_prepared(WIDTH, HEIGHT);

    let prepared_frames = cache.frame_counter;
    for _ in 0..10 {
        assert!(!cache.update(&sections, WIDTH, HEIGHT));
    }
    // Skipped frames age nothing.
    assert_eq!(cache.frame_counter, prepared_frames);
    assert_eq!(cache.buffers.len(), 2);
    assert_eq!(glyphs(&cache).len(), 2);
}

#[test]
fn any_changed_draw_value_prepares_again() {
    let mut cache = mono_cache();
    let mut sections = vec![section("static line", 0), section("another line", 1)];
    assert!(frame(&mut cache, &sections));
    assert!(!frame(&mut cache, &sections));

    sections[1].position[0] += 1.0;
    assert!(frame(&mut cache, &sections), "position");
    assert!(!frame(&mut cache, &sections));

    sections[0].color = [255, 0, 0, 255];
    assert!(frame(&mut cache, &sections), "color");
    assert!(!frame(&mut cache, &sections));

    sections[0].content.push('!');
    assert!(frame(&mut cache, &sections), "content");
    assert!(!frame(&mut cache, &sections));

    sections[1].bounds = Some([200.0, 40.0]);
    assert!(frame(&mut cache, &sections), "bounds");
    assert!(!frame(&mut cache, &sections));

    sections[1].font_size = 15.0;
    assert!(frame(&mut cache, &sections), "font size");
    assert!(!frame(&mut cache, &sections));

    sections.pop();
    assert!(frame(&mut cache, &sections), "section count");
    assert!(!frame(&mut cache, &sections));

    assert!(cache.update(&sections, WIDTH, HEIGHT - 1), "viewport");
    cache.mark_prepared(WIDTH, HEIGHT - 1);
    assert!(!cache.update(&sections, WIDTH, HEIGHT - 1));
    assert!(cache.update(&sections, WIDTH, HEIGHT));
}

#[test]
fn evicted_text_lays_out_again_to_the_same_glyphs() {
    let mut cache = mono_cache();
    let shown = [section("0042 f000150 deadbeef 0badc0de", 0)];
    assert!(frame(&mut cache, &shown));
    let first = glyphs(&cache);
    assert_eq!(first.len(), 1);
    assert!(!first[0].is_empty());

    // Other text, with other metrics and bounds, for longer than an unused
    // layout is kept.
    for index in 0..=BUFFER_CACHE_MAX_UNUSED_FRAMES + 1 {
        let mut filler = section(&format!("filler line number {index}"), 0);
        filler.font_size = 22.0;
        filler.line_height = 28.0;
        filler.bounds = Some([300.0, 60.0]);
        assert!(frame(&mut cache, &[filler]));
    }
    assert!(
        cache
            .buffers
            .keys()
            .all(|key| key.content != shown[0].content)
    );
    // A frame without text ages a filler layout out with no miss to take it.
    assert!(frame(&mut cache, &[]));
    assert!(!cache.spare.is_empty());

    // The text now shapes into a recycled buffer and must come out as it did
    // from an empty one.
    assert!(frame(&mut cache, &shown));
    assert_eq!(glyphs(&cache), first);
}

#[test]
fn zero_font_size_shapes_into_a_fresh_buffer() {
    let mut cache = mono_cache();
    for index in 0..=BUFFER_CACHE_MAX_UNUSED_FRAMES + 1 {
        frame(&mut cache, &[section(&format!("warm {index}"), 0)]);
    }
    assert!(frame(&mut cache, &[]));
    assert!(!cache.spare.is_empty());

    // `Buffer::set_metrics` panics on a zero font size; a recycled buffer
    // must not be given one.
    let mut zero = section("zero", 0);
    zero.font_size = 0.0;
    assert!(frame(&mut cache, &[zero]));
}

#[test]
fn font_reload_empties_every_cache() {
    let mut cache = mono_cache();
    let changing = |index: u32| -> Vec<TextSection> {
        (0..4)
            .map(|row| section(&format!("{row} {index}"), row))
            .collect()
    };
    let fill = |cache: &mut TextLayoutCache| {
        for index in 0..4 {
            frame(cache, &changing(index));
        }
        // Fewer sections than layouts aging out: some stay spare.
        frame(cache, &[section("tail", 0)]);
        assert!(!cache.buffers.is_empty());
        assert!(!cache.spare.is_empty());
        assert!(!cache.prepared.is_empty());
        assert!(cache.prepared_viewport.is_some());
    };
    let assert_empty = |cache: &TextLayoutCache| {
        assert!(cache.buffers.is_empty());
        assert!(cache.spare.is_empty());
        assert!(cache.prepared.is_empty());
        assert_eq!(cache.prepared_viewport, None);
    };

    fill(&mut cache);
    cache.reload_font(
        "mono",
        font_bytes("JetBrainsMono/static/JetBrainsMono-Regular.ttf"),
    );
    assert_empty(&cache);
    // The frame it last drew prepares again, against the reloaded face.
    let tail = [section("tail", 0)];
    assert!(frame(&mut cache, &tail));
    assert!(!glyphs(&cache)[0].is_empty());

    fill(&mut cache);
    cache.load_font("sans", font_bytes("Inter/static/Inter-Regular.ttf"));
    assert_empty(&cache);
    assert!(frame(&mut cache, &tail));
}
