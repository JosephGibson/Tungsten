use std::path::Path;

use glyphon::fontdb;

use super::TextEngine as TextLayoutCache;
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
    let mut cache = TextLayoutCache::from_font_system(font_system);
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
        ..Default::default()
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

type Runs = Vec<(Vec<(u16, f32, f32)>, f32)>;

/// Each layout run of the first prepared section: its glyphs (ID, x offset,
/// advance) and its width.
fn runs(cache: &TextLayoutCache) -> Runs {
    let area = text_areas(&cache.prepared, &cache.buffers)
        .next()
        .expect("a prepared section");
    area.buffer
        .layout_runs()
        .map(|run| {
            let glyphs = run.glyphs.iter().map(|g| (g.glyph_id, g.x, g.w)).collect();
            (glyphs, run.line_w)
        })
        .collect()
}

/// Lays `section` out as a frame of its own and returns its runs.
fn laid_out(cache: &mut TextLayoutCache, section: TextSection) -> Runs {
    frame(cache, &[section]);
    runs(cache)
}

/// The glyph cosmic-text draws for an ellipsis in the mono face.
fn ellipsis_glyph(cache: &mut TextLayoutCache) -> u16 {
    laid_out(cache, section("\u{2026}", 0))[0].0[0].0
}

const LONG_LINE: &str = "a long line of words that runs far wider than its bounds";

#[test]
fn default_layout_lays_out_like_the_buffer_before_layouts() {
    let mut cache = mono_cache();
    let mut wrapped = section("0042 f000150 deadbeef 0badc0de wraps here", 0);
    wrapped.bounds = Some([200.0, 400.0]);

    // Recycle buffers that carried every non-default setting first.
    for index in 0..=BUFFER_CACHE_MAX_UNUSED_FRAMES + 1 {
        let mut filler = section(&format!("filler {index} {LONG_LINE}"), 0);
        filler.bounds = Some([120.0, 30.0]);
        filler.layout = TextLayout::default()
            .with_align(TextAlign::Center)
            .with_wrap(TextWrap::Glyph)
            .with_overflow(TextOverflow::Ellipsis {
                at: EllipsisAt::Middle,
                lines: None,
            })
            .with_hinting(TextHinting::Enabled)
            .with_letter_spacing(0.2)
            .with_features(FontFeatures::tabular_figures());
        frame(&mut cache, &[filler]);
    }
    assert!(frame(&mut cache, &[]));
    assert!(!cache.spare.is_empty());
    let drawn = laid_out(&mut cache, wrapped.clone());

    // Step 4's calls on a fresh buffer.
    let mut buffer = Buffer::new_empty(Metrics::new(wrapped.font_size, wrapped.line_height));
    buffer.set_size(Some(200.0), Some(400.0));
    let attrs = make_attrs(&cache.font_attrs, "mono");
    buffer.set_text(&wrapped.content, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut cache.font_system, false);
    let expected: Runs = buffer
        .layout_runs()
        .map(|run| {
            let glyphs = run.glyphs.iter().map(|g| (g.glyph_id, g.x, g.w)).collect();
            (glyphs, run.line_w)
        })
        .collect();

    assert!(expected.len() > 1, "the text wraps");
    assert_eq!(drawn, expected);
}

#[test]
fn any_changed_layout_field_prepares_again() {
    let mut cache = mono_cache();
    let mut sections = vec![section("static line", 0)];
    assert!(frame(&mut cache, &sections));
    assert!(!frame(&mut cache, &sections));

    type Edit = (&'static str, fn(&mut TextLayout));
    let edits: [Edit; 6] = [
        ("align", |layout| layout.align = TextAlign::End),
        ("wrap", |layout| layout.wrap = TextWrap::None),
        ("overflow", |layout| {
            layout.overflow = TextOverflow::Ellipsis {
                at: EllipsisAt::End,
                lines: Some(1),
            };
        }),
        ("hinting", |layout| layout.hinting = TextHinting::Enabled),
        ("letter spacing", |layout| layout.letter_spacing = 0.05),
        ("features", |layout| {
            layout.features = FontFeatures::tabular_figures();
        }),
    ];
    for (field, edit) in edits {
        edit(&mut sections[0].layout);
        assert!(frame(&mut cache, &sections), "{field}");
        assert!(!frame(&mut cache, &sections), "{field}");
    }
}

#[test]
fn end_alignment_puts_the_last_glyph_at_the_right_edge() {
    let mut cache = mono_cache();
    let mut short = section("end", 0);
    short.bounds = Some([300.0, 40.0]);
    short.layout.align = TextAlign::End;
    let drawn = laid_out(&mut cache, short);
    let &(_, x, w) = drawn[0].0.last().expect("glyphs");
    assert!((x + w - 300.0).abs() < 0.5, "right edge at {}", x + w);
}

#[test]
fn no_wrap_keeps_a_long_line_on_one_line() {
    let mut cache = mono_cache();
    let mut long = section(LONG_LINE, 0);
    long.bounds = Some([120.0, 400.0]);
    assert!(laid_out(&mut cache, long.clone()).len() > 1);
    long.layout.wrap = TextWrap::None;
    assert_eq!(laid_out(&mut cache, long).len(), 1);
}

#[test]
fn ellipsis_line_limit_counts_lines_per_paragraph() {
    let mut cache = mono_cache();
    let ellipsis = ellipsis_glyph(&mut cache);
    let mut long = section(LONG_LINE, 0);
    long.bounds = Some([120.0, 400.0]);
    long.layout.overflow = TextOverflow::Ellipsis {
        at: EllipsisAt::End,
        lines: Some(1),
    };
    let drawn = laid_out(&mut cache, long.clone());
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].0.last().map(|g| g.0), Some(ellipsis));

    // cosmic-text applies the limit to each paragraph (Q10).
    long.content = "a\nb".to_string();
    let drawn = laid_out(&mut cache, long);
    assert_eq!(drawn.len(), 2);
    assert!(drawn.iter().all(|(glyphs, _)| glyphs.len() == 1));
}

#[test]
fn ellipsis_without_a_line_limit_fills_the_bounds_height() {
    let mut cache = mono_cache();
    let ellipsis = ellipsis_glyph(&mut cache);
    let mut long = section(LONG_LINE, 0);
    // One line high: line height 18.
    long.bounds = Some([120.0, 18.0]);
    long.layout.overflow = TextOverflow::Ellipsis {
        at: EllipsisAt::End,
        lines: None,
    };
    let drawn = laid_out(&mut cache, long);
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].0.last().map(|g| g.0), Some(ellipsis));
}

#[test]
fn letter_spacing_is_in_em() {
    let mut cache = mono_cache();
    let mut digits = section("0123456789", 0);
    digits.font_size = 20.0;
    digits.line_height = 24.0;
    let plain = laid_out(&mut cache, digits.clone())[0].1;
    digits.layout.letter_spacing = 0.1;
    let spaced = laid_out(&mut cache, digits)[0].1;
    let wider = spaced - plain;
    assert!((18.0..=20.001).contains(&wider), "{wider} px wider");
}

#[test]
fn tabular_figures_give_digits_one_advance() {
    let mut cache = mono_cache();
    cache.load_font("sans", font_bytes("Inter/static/Inter-Regular.ttf"));
    let mut digits = section("10", 0);
    digits.font_id = "sans".to_string();
    digits.font_size = 20.0;
    digits.line_height = 24.0;
    let proportional = laid_out(&mut cache, digits.clone())[0].0.clone();
    assert_ne!(proportional[0].2, proportional[1].2);
    digits.layout.features = FontFeatures::tabular_figures();
    let tabular = laid_out(&mut cache, digits)[0].0.clone();
    assert_eq!(tabular[0].2, tabular[1].2);
}
