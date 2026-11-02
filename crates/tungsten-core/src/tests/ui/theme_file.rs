use super::*;
use crate::text::{EllipsisAt, TextAlign, TextOverflow, TextWrap};
use crate::ui::WidgetKind;

fn theme_of(json: &str) -> Theme {
    ThemeFile::from_json(json)
        .unwrap_or_else(|e| panic!("{json}: {e}"))
        .into_theme()
}

#[test]
fn an_empty_file_is_the_built_in_theme() {
    assert_eq!(theme_of("{}"), Theme::default());
    assert_eq!(theme_of(r#"{"tokens": {}, "kinds": {}}"#), Theme::default());
}

#[test]
fn a_partial_file_overrides_the_defaults_and_the_kinds_follow_its_tokens() {
    let theme = theme_of(
        r##"{
          "tokens": { "colors": { "accent": "#5b8def", "surface": "#1e1e24" },
                      "spacing": { "md": 8 }, "radii": { "md": 4.5 },
                      "fonts": { "body": "serif" }, "text": { "size": 14, "line_height": 18 } },
          "kinds": { "button": { "normal": { "border_width": 2, "text": { "weight": 800 } },
                                 "focused": { "border": "#ffffff" },
                                 "disabled": { "text": { "color": "#808090" } } },
                     "image": { "normal": { "corner_radius": 3 } } }
        }"##,
    );
    let default = Theme::default();
    assert_eq!(theme.tokens.colors.accent, [0x5b, 0x8d, 0xef, 255]);
    assert_eq!(theme.tokens.colors.surface, [0x1e, 0x1e, 0x24, 255]);
    assert_eq!(theme.tokens.colors.text, default.tokens.colors.text);
    assert_eq!(theme.tokens.spacing.md, 8.0);
    assert_eq!(theme.tokens.spacing.lg, default.tokens.spacing.lg);
    assert_eq!(theme.tokens.radii.md, 4.5);
    assert_eq!(theme.tokens.fonts.body, "serif");
    assert_eq!(theme.tokens.fonts.mono, "mono");
    assert_eq!(theme.tokens.text.size, 14.0);
    assert_eq!(theme.tokens.text.line_height, 18.0);
    assert_eq!(theme.tokens.text.heading_size, 24.0);

    // The kind styles are drawn from the file's tokens, then overridden.
    let button = &theme.kinds.button;
    assert_eq!(button.normal.background, [0x5b, 0x8d, 0xef, 255]);
    assert_eq!(button.normal.border_width, 2.0);
    assert_eq!(button.normal.corner_radius, 4.5);
    assert_eq!(button.normal.text.weight, Some(800));
    assert_eq!(
        button.normal.text.color,
        Some(default.tokens.colors.accent_text)
    );
    assert_eq!(button.focused.border, Some([255, 255, 255, 255]));
    assert_eq!(
        button.focused.border_width,
        Some(2.0),
        "a variant merges over the built-in one"
    );
    assert_eq!(button.disabled.text.color, Some([0x80, 0x80, 0x90, 255]));
    assert_eq!(
        button.disabled.background,
        Some([0x1e, 0x1e, 0x24, 255]),
        "the built-in disabled surface, from the file's token"
    );
    assert_eq!(theme.kinds.panel.normal.background, [0x1e, 0x1e, 0x24, 255]);
    assert_eq!(theme.kinds.panel.normal.corner_radius, 4.5);
    assert_eq!(theme.kinds.image.normal.corner_radius, 3.0);
    assert_eq!(theme.kinds.image.normal.background, [0, 0, 0, 0]);
    assert_eq!(theme.kinds.label, default.kinds.label);
    assert_eq!(theme.kind(WidgetKind::Image), &theme.kinds.image);
    assert_eq!(theme.base_text().family, "serif");
    assert_eq!(theme.base_text().size, 14.0);
}

#[test]
fn toggle_and_slider_looks_parse_and_merge_over_the_built_in_ones() {
    let theme = theme_of(
        r##"{
          "tokens": { "colors": { "accent": "#40c070" } },
          "kinds": { "toggle": { "normal": { "corner_radius": 5, "text": { "color": "#ffffff" } },
                                 "pressed": { "background": "#101010" } },
                     "slider": { "normal": { "background": "#303030" },
                                 "hovered": { "text": { "color": "#80ff80" } },
                                 "focused": { "border_width": 3 } } }
        }"##,
    );
    let default = Theme::default();
    let toggle = &theme.kinds.toggle;
    assert_eq!(toggle.normal.corner_radius, 5.0);
    assert_eq!(toggle.normal.text.color, Some([255, 255, 255, 255]));
    assert_eq!(
        toggle.normal.background,
        default.kinds.toggle.normal.background
    );
    assert_eq!(toggle.pressed.background, Some([0x10, 0x10, 0x10, 255]));
    assert_eq!(toggle.hovered, default.kinds.toggle.hovered);
    assert_eq!(theme.kind(WidgetKind::Toggle), toggle);

    let slider = &theme.kinds.slider;
    assert_eq!(slider.normal.background, [0x30, 0x30, 0x30, 255]);
    assert_eq!(
        slider.normal.text.color,
        Some([0x40, 0xc0, 0x70, 255]),
        "the fill follows the file's accent"
    );
    assert_eq!(slider.hovered.text.color, Some([0x80, 0xff, 0x80, 255]));
    assert_eq!(slider.focused.border_width, Some(3.0));
    assert_eq!(
        slider.focused.border,
        Some(default.tokens.colors.focus),
        "a variant merges over the built-in one"
    );
    assert_eq!(theme.kind(WidgetKind::Slider), slider);
    assert_eq!(
        theme_of(r#"{ "kinds": { "toggle": {}, "slider": {} } }"#),
        default
    );
}

#[test]
fn text_fields_map_onto_the_text_overrides() {
    let theme = theme_of(
        r##"{ "kinds": { "label": { "normal": { "text": {
            "family": "mono", "size": 12, "line_height": 15, "weight": 700, "italic": true,
            "color": "#FF000080", "align": "center", "wrap": "none",
            "overflow": { "ellipsis": { "at": "middle", "lines": 2 } } } } },
          "panel": { "hovered": { "text": { "overflow": "clip", "wrap": "word_or_glyph" } } } } }"##,
    );
    let text = &theme.kinds.label.normal.text;
    assert_eq!(text.family.as_deref(), Some("mono"));
    assert_eq!(text.size, Some(12.0));
    assert_eq!(text.line_height, Some(15.0));
    assert_eq!(text.weight, Some(700));
    assert_eq!(text.italic, Some(true));
    assert_eq!(text.color, Some([255, 0, 0, 128]));
    assert_eq!(text.align, Some(TextAlign::Center));
    assert_eq!(text.wrap, Some(TextWrap::None));
    assert_eq!(
        text.overflow,
        Some(TextOverflow::Ellipsis {
            at: EllipsisAt::Middle,
            lines: Some(2)
        })
    );
    let hovered = &theme.kinds.panel.hovered.text;
    assert_eq!(hovered.overflow, Some(TextOverflow::Clip));
    assert_eq!(hovered.wrap, Some(TextWrap::WordOrGlyph));
    assert_eq!(hovered.align, None);

    let spaced = theme_of(
        r#"{ "kinds": { "label": { "normal": { "text": { "letter_spacing": 0.2 } } },
                       "button": { "focused": { "text": { "letter_spacing": -0.05 } } } } }"#,
    );
    assert_eq!(spaced.kinds.label.normal.text.letter_spacing, Some(0.2));
    assert_eq!(spaced.kinds.button.focused.text.letter_spacing, Some(-0.05));
    assert_eq!(
        spaced.kinds.button.normal,
        Theme::default().kinds.button.normal
    );
    assert!(
        ThemeFile::from_json(
            r#"{ "kinds": { "label": { "normal": { "text": { "tracking": 0.2 } } } } }"#
        )
        .is_err(),
        "an unknown text field still fails"
    );

    let open = theme_of(
        r#"{ "kinds": { "label": { "normal": { "text": { "overflow": { "ellipsis": { "at": "end" } } } } } } }"#,
    );
    assert_eq!(
        open.kinds.label.normal.text.overflow,
        Some(TextOverflow::Ellipsis {
            at: EllipsisAt::End,
            lines: None
        })
    );
}

#[test]
fn colours_are_rrggbb_or_rrggbbaa() {
    assert_eq!(parse_color("#1e1e24"), Ok([0x1e, 0x1e, 0x24, 255]));
    assert_eq!(parse_color("#1E1E24"), Ok([0x1e, 0x1e, 0x24, 255]));
    assert_eq!(parse_color("#ff000080"), Ok([255, 0, 0, 128]));
    assert_eq!(parse_color("#00000000"), Ok([0, 0, 0, 0]));
    for bad in [
        "red",
        "1e1e24",
        "#12345",
        "#1234567",
        "#gg0000",
        "#",
        "#ff00ff00ff",
        "#ééé",
    ] {
        let err = parse_color(bad).unwrap_err();
        assert!(err.contains(bad), "{bad}: {err}");
    }
    let err =
        ThemeFile::from_json(r#"{ "tokens": { "colors": { "text": "white" } } }"#).unwrap_err();
    assert!(err.to_string().contains("'white'"), "{err}");
}

#[test]
fn an_unknown_kind_token_or_field_is_rejected() {
    for bad in [
        r#"{ "kinds": { "progress": {} } }"#,
        r##"{ "tokens": { "colors": { "primary": "#000000" } } }"##,
        r#"{ "tokens": { "sizes": {} } }"#,
        r#"{ "kinds": { "button": { "active": {} } } }"#,
        r#"{ "kinds": { "button": { "normal": { "opacity": 1 } } } }"#,
        r#"{ "kinds": { "button": { "normal": { "text": { "font": "mono" } } } } }"#,
        r#"{ "kinds": { "label": { "normal": { "text": { "align": "left" } } } } }"#,
        r#"{ "name": "dark" }"#,
        "[]",
        "not json",
    ] {
        assert!(ThemeFile::from_json(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_line_height_must_be_finite_and_above_zero() {
    // The text engine cannot lay a line out at a zero line height
    // (cosmic-text asserts it), so the file is refused at load and a reload
    // keeps the previous theme (`D-148`).
    for bad in [
        r#"{ "tokens": { "text": { "line_height": 0 } } }"#,
        r#"{ "tokens": { "text": { "line_height": -2.5 } } }"#,
        r#"{ "tokens": { "text": { "line_height": 1e39 } } }"#,
        r#"{ "kinds": { "label": { "normal": { "text": { "line_height": 0 } } } } }"#,
        r#"{ "kinds": { "button": { "focused": { "text": { "line_height": 0.0 } } } } }"#,
    ] {
        let err = ThemeFile::from_json(bad).unwrap_err();
        assert!(err.to_string().contains("line_height"), "{bad}: {err}");
    }
    let theme = theme_of(
        r#"{ "tokens": { "text": { "line_height": 18.5 } },
             "kinds": { "label": { "normal": { "text": { "line_height": null } } } } }"#,
    );
    assert_eq!(theme.tokens.text.line_height, 18.5);
}

#[test]
fn load_reads_a_file_and_names_it_in_errors() {
    let dir = std::env::temp_dir().join(format!("tungsten_theme_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let good = dir.join("dark.json");
    std::fs::write(
        &good,
        r##"{ "tokens": { "colors": { "text": "#ffffff" } } }"##,
    )
    .unwrap();
    let theme = ThemeFile::load(&good).unwrap().into_theme();
    assert_eq!(theme.tokens.colors.text, [255, 255, 255, 255]);

    let bad = dir.join("bad.json");
    std::fs::write(&bad, "{").unwrap();
    let err = ThemeFile::load(&bad).unwrap_err();
    assert!(matches!(&err, ThemeFileError::Parse { path, .. } if path.ends_with("bad.json")));
    assert!(err.to_string().starts_with("invalid ui theme '"), "{err}");

    let missing = dir.join("missing.json");
    let err = ThemeFile::load(&missing).unwrap_err();
    assert!(matches!(&err, ThemeFileError::Io { path, .. } if path.ends_with("missing.json")));
    assert!(
        err.to_string().starts_with("failed to read ui theme '"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_registry_holds_themes_by_id() {
    let mut registry = UiThemeRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.insert("dark", Theme::default()), None);
    let mut light = Theme::default();
    light.tokens.colors.text = [0, 0, 0, 255];
    assert_eq!(registry.insert("light", light.clone()), None);
    assert_eq!(registry.len(), 2);
    assert_eq!(registry.ids(), ["dark", "light"]);
    assert!(registry.contains("light"));
    assert!(!registry.contains("high_contrast"));
    assert_eq!(registry.get("light"), Some(&light));
    assert_eq!(registry.get("high_contrast"), None);
    assert_eq!(registry.insert("light", Theme::default()), Some(light));
    assert_eq!(registry.get("light"), Some(&Theme::default()));
}

#[test]
fn danger_reads_from_colors_and_defaults() {
    assert_eq!(Theme::default().tokens.colors.danger, [255, 107, 107, 255]);
    let theme = theme_of(r##"{ "tokens": { "colors": { "danger": "#e0303080" } } }"##);
    assert_eq!(theme.tokens.colors.danger, [0xe0, 0x30, 0x30, 0x80]);
    let default = Theme::default();
    assert_eq!(
        theme.tokens.colors.text, default.tokens.colors.text,
        "the other colours keep their defaults"
    );
    // No built-in kind reads it, so the kinds are the default theme's.
    assert_eq!(theme.kinds, default.kinds);
}

#[test]
fn text_styles_sizes_and_sounds_read_from_a_theme_file() {
    let theme = theme_of(
        r##"{
          "tokens": { "colors": { "background": "#102030", "text_muted": "#808080" },
                      "fonts": { "heading": "serif" },
                      "text": { "size": 16, "line_height": 22, "heading_size": 26 } },
          "text_styles": { "heading": { "size": 20, "line_height": 28 },
                           "title": { "family": "serif", "size": 56, "line_height": 64,
                                      "color": "#ffffff" } },
          "sizes": { "menu_width": 260, "button_padding_x": 14.5 },
          "sounds": { "focus": "ui_move", "back": "ui_back" }
        }"##,
    );
    let heading = &theme.text_style("heading").expect("built in").text;
    assert_eq!(
        heading.family.as_deref(),
        Some("serif"),
        "from the file's tokens"
    );
    assert_eq!(
        (heading.size, heading.line_height),
        (Some(20.0), Some(28.0)),
        "the file's fields over the built-in style"
    );
    let title = &theme.text_style("title").expect("the file's own").text;
    assert_eq!(title.color, Some([255, 255, 255, 255]));
    assert_eq!(title.size, Some(56.0));
    let note = &theme.text_style("note").expect("built in").text;
    assert_eq!(
        note.color,
        Some([0x80, 0x80, 0x80, 255]),
        "the built-ins follow the file's tokens"
    );
    let small = &theme.text_style("small").expect("built in").text;
    assert_eq!((small.size, small.line_height), (Some(13.0), Some(18.0)));
    assert_eq!(theme.size("menu_width", 0.0), 260.0);
    assert_eq!(theme.size("button_padding_x", 0.0), 14.5);
    assert_eq!(theme.size("missing", 7.0), 7.0);
    assert_eq!(theme.sounds().focus.as_deref(), Some("ui_move"));
    assert_eq!(theme.sounds().activate, None);
    assert_eq!(theme.sounds().back.as_deref(), Some("ui_back"));
    assert_eq!(
        theme.tokens.colors.scrim,
        [0x10, 0x20, 0x30, 184],
        "an unset scrim follows the file's background"
    );
    assert_eq!(theme.tokens.colors.raised, theme.tokens.colors.surface);

    let built_in = Theme::default();
    let names: Vec<&str> = built_in
        .names
        .text_styles
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(names, ["danger", "heading", "mono", "note", "small"]);
    assert!(built_in.names.sizes.is_empty());
    assert_eq!(*built_in.sounds(), ThemeSounds::default());
    for bad in [
        r#"{ "sizes": { "gap": -1 } }"#,
        r#"{ "sizes": { "gap": 1e39 } }"#,
        r#"{ "sounds": { "hover": "ui_move" } }"#,
        r#"{ "text_styles": { "x": { "font": "mono" } } }"#,
        r#"{ "text_styles": { "x": { "line_height": 0 } } }"#,
    ] {
        assert!(ThemeFile::from_json(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_colour_field_takes_a_token_name() {
    let theme = theme_of(
        r##"{
          "tokens": { "colors": { "surface": "#15171d", "raised": "#1e2129", "focus": "accent" } },
          "kinds": { "button": { "normal": { "background": "surface", "text": { "color": "text" } },
                                 "hovered": { "background": "raised", "border": "scrim" } } },
          "text_styles": { "warning": { "color": "danger" } }
        }"##,
    );
    let colors = theme.tokens.colors;
    assert_eq!(colors.focus, colors.accent, "a token reads a literal token");
    let button = &theme.kinds.button;
    assert_eq!(button.normal.background, [0x15, 0x17, 0x1d, 255]);
    assert_eq!(button.normal.text.color, Some(colors.text));
    assert_eq!(button.hovered.background, Some([0x1e, 0x21, 0x29, 255]));
    assert_eq!(button.hovered.border, Some(colors.scrim));
    let warning = &theme.text_style("warning").expect("the file's own").text;
    assert_eq!(warning.color, Some(colors.danger));
    let literal =
        theme_of(r##"{ "kinds": { "button": { "normal": { "background": "#15171d" } } } }"##);
    let named = theme_of(
        r##"{ "tokens": { "colors": { "surface": "#15171d" } },
             "kinds": { "button": { "normal": { "background": "surface" } } } }"##,
    );
    assert_eq!(
        literal.kinds.button.normal.background,
        named.kinds.button.normal.background
    );
}

#[test]
fn an_unknown_token_name_is_refused_with_its_path() {
    for (bad, path) in [
        (
            r#"{ "kinds": { "button": { "hovered": { "background": "surfce" } } } }"#,
            "kinds.button.hovered.background",
        ),
        (
            r#"{ "kinds": { "slider": { "normal": { "text": { "color": "primary" } } } } }"#,
            "kinds.slider.normal.text.color",
        ),
        (
            r#"{ "text_styles": { "title": { "color": "gold" } } }"#,
            "text_styles.title.color",
        ),
        (
            r#"{ "tokens": { "colors": { "focus": "Accent" } } }"#,
            "tokens.colors.focus",
        ),
    ] {
        let err = ThemeFile::from_json(bad).unwrap_err().to_string();
        assert!(err.contains(path), "{bad}: {err}");
        assert!(err.contains("raised"), "the error lists the names: {err}");
    }
}
