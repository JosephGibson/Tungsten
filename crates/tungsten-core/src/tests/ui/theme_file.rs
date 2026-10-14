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
        r#"{ "kinds": { "slider": {} } }"#,
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
