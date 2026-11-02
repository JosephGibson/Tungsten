use std::collections::BTreeSet;

use super::*;

fn read(json: &str) -> ScreenFile {
    ScreenFile::from_json(json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

fn error(json: &str) -> ScreenError {
    match ScreenFile::from_json(json) {
        Ok(file) => panic!("{json} read as {file:?}"),
        Err(error) => error,
    }
}

/// A file whose screen card holds `children`.
fn screen(children: &str) -> String {
    format!(
        r#"{{"version": 1, "root": {{"kind": "screen", "width": 260, "children": [{children}]}}}}"#
    )
}

/// The names serde lists after "expected" in `message`.
fn expected(message: &str) -> BTreeSet<String> {
    let after = message
        .split_once("expected")
        .unwrap_or_else(|| panic!("no expected list in {message}"))
        .1;
    after
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

#[test]
fn every_kind_reads_with_its_fields() {
    let file = read(
        r##"{
          "version": 1,
          "root": {
            "kind": "screen", "key": "menu", "width": "menu_width", "style": "body", "mono": true,
            "card": { "layout": { "max_width": "none", "align_items": "center", "gap": "spacing.lg" },
                      "overrides": { "background": "#00000000", "border_width": 0 } },
            "children": [
              { "kind": "group", "layout": { "direction": "row" }, "children": [
                { "kind": "label", "text": "A" } ] },
              { "kind": "row", "children": [ { "kind": "note", "text": "B" } ] },
              { "kind": "column", "children": [ { "kind": "small", "bind": { "text": "age" } } ] },
              { "kind": "panel", "overrides": { "border": "accent" }, "children": [] },
              { "kind": "heading", "text": "Paused", "centred": true },
              { "kind": "display", "text": "YOU DIED", "style": "died",
                "overrides": { "text": { "color": "#a01818", "letter_spacing": 0.3 } } },
              { "kind": "prompt", "template": "{action:state_pause} Pause" },
              { "kind": "button", "key": "save", "text": "Save", "danger": true,
                "bind": { "enabled": "saving_on", "visible": "shown" }, "on": { "action": "save" },
                "layout": { "padding": { "x": "button_padding_x", "y": 6 }, "align_self": "end" },
                "children": [ { "kind": "small", "text": "now" } ] },
              { "kind": "choice_row", "key": "mode", "caption": "Window mode", "bind": { "text": "mode" },
                "on": "pop" },
              { "kind": "toggle_row", "key": "vsync", "caption": "VSync", "bind": { "value": "vsync" },
                "on": { "setting": "vsync" } },
              { "kind": "slider_row", "key": "master", "caption": "Master",
                "range": { "min": 0, "max": 1, "step": 0.05 }, "readout": "master_text",
                "on": { "setting": "master_volume" } },
              { "kind": "tab_rail", "tabs": [ { "key": "display", "caption": "Display" },
                                               { "key": "audio", "caption": "Audio" } ],
                "bind": { "value": "tab" } },
              { "kind": "meter", "width": 120, "height": "spacing.sm", "color": "danger",
                "bind": { "fill": "health" } },
              { "kind": "mount", "key": "cards", "mount": "difficulty_cards" },
              { "kind": "repeat", "key": "slots", "rows": "slots",
                "row": { "kind": "button", "field": "name", "on": { "push": "confirm" },
                         "children": [ { "kind": "note", "key": "age", "field": "age" } ] } }
            ],
            "over": [
              { "kind": "column", "layout": { "anchor": { "corner": "bottom_start",
                                                          "offset": ["spacing.lg", 24] },
                                              "width": "50%", "min_height": 10 },
                "children": [ { "kind": "small", "text": "Version 1" } ] }
            ]
          }
        }"##,
    );
    assert_eq!(file.version, SCREEN_VERSION);
    assert_eq!(file.root.kind, Kind::Screen);
    assert_eq!(file.root.children.len(), 15);
    assert_eq!(file.root.over.len(), 1);
    assert_eq!(
        file.keys(),
        [
            "menu", "save", "mode", "vsync", "master", "display", "audio", "cards", "slots"
        ]
    );
    let uses = file.uses();
    let set = |names: &[&str]| names.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(uses.text, set(&["age", "mode", "master_text"]));
    assert_eq!(uses.value, set(&["vsync", "tab"]));
    assert_eq!(uses.fill, set(&["health"]));
    assert_eq!(uses.visible, set(&["shown"]));
    assert_eq!(uses.enabled, set(&["saving_on"]));
    assert_eq!(uses.rows, set(&["slots"]));
    assert_eq!(uses.mounts, set(&["difficulty_cards"]));
    assert_eq!(uses.actions, set(&["save"]));
    assert_eq!(uses.styles, set(&["body", "died"]));
    assert_eq!(uses.sizes, set(&["menu_width", "button_padding_x"]));
    assert_eq!(uses.screens, set(&["confirm"]));

    let bare = read(
        r#"{"version": 1, "root": {"kind": "root", "layer": "game_popup", "modal": true,
            "layout": {"width": "100%", "height": "100%", "justify_content": "center"},
            "children": [{"kind": "panel", "key": "band"}]}}"#,
    );
    assert_eq!(bare.root.kind, Kind::Root);
    assert_eq!(bare.root.layer, Some(LayerName::GamePopup));
    assert!(bare.root.modal);
    assert_eq!(bare.data_screen_refusal(), None);
    assert_eq!(file.data_screen_refusal(), None);
    let hud = read(r#"{"version": 1, "root": {"kind": "root", "layer": "game"}}"#);
    assert!(hud.data_screen_refusal().is_some());
}

#[test]
fn an_unknown_field_names_its_path() {
    let error = error(&screen(
        r#"{"kind": "label", "text": "a"}, {"kind": "label", "text": "b"},
           {"kind": "button", "captoin": "Save"}"#,
    ));
    assert_eq!(error.field, "root.children[2].captoin");
    assert!(
        error.message.starts_with("unknown field `captoin`"),
        "{error}"
    );

    // A field another kind takes names its path too, set to its default
    // or not.
    let error = error_of(&screen(r#"{"kind": "label", "text": "a", "children": []}"#));
    assert_eq!(error.field, "root.children[0].children");
    assert_eq!(error.message, "a label takes no children");
    let error = error_of(&screen(
        r#"{"kind": "label", "text": "a", "centred": false}"#,
    ));
    assert_eq!(error.field, "root.children[0].centred");

    // A value of the wrong form names its field.
    let error = error_of(&screen(
        r#"{"kind": "group", "layout": {"gap": "spacing.xxl"}}"#,
    ));
    assert_eq!(error.field, "root.children[0].layout.gap");
    let error = error_of(&screen(
        r#"{"kind": "group", "layout": {"padding": {"lft": 4}}}"#,
    ));
    assert_eq!(error.field, "root.children[0].layout.padding.lft");
    let error = error_of(&screen(
        r#"{"kind": "button", "text": "a", "on": {"setting": "gamma"}}"#,
    ));
    assert_eq!(error.field, "root.children[0].on.setting");
}

fn error_of(json: &str) -> ScreenError {
    error(json)
}

#[test]
fn a_version_other_than_1_fails_before_the_rest() {
    let error = error(r#"{"version": 2, "root": {"kind": "screen", "widht": 1}}"#);
    assert_eq!(error.field, "version");
    assert_eq!(
        error.message,
        "version 2 is not supported; this engine reads version 1"
    );
    let missing = error_of(r#"{"root": {"kind": "screen", "width": 1}}"#);
    assert_eq!(missing.field, "version");
    assert_eq!(error_of("[1]").message, "a screen file is a JSON object");
    assert!(error_of("{").message.starts_with("not JSON"));
}

#[test]
fn a_node_with_two_text_sources_fails() {
    let error = error(&screen(
        r#"{"kind": "label", "text": "a", "bind": {"text": "b"}}"#,
    ));
    assert_eq!(error.field, "root.children[0].text");
    assert_eq!(
        error.message,
        "a label takes exactly one of text, bind.text and field"
    );
    let none = error_of(&screen(r#"{"kind": "button", "key": "go"}"#));
    assert_eq!(none.field, "root.children[0].text");
    // A choice row's value is optional, one source at most.
    read(&screen(r#"{"kind": "choice_row", "caption": "Mode"}"#));
    let two = error_of(&screen(
        r#"{"kind": "choice_row", "caption": "Mode", "text": "a", "bind": {"text": "b"}}"#,
    ));
    assert_eq!(
        two.message,
        "a choice_row takes at most one of text, bind.text and field"
    );
}

#[test]
fn a_key_twice_in_a_file_fails() {
    let error = error(&screen(
        r#"{"kind": "button", "key": "save", "text": "Save"},
           {"kind": "group", "children": [{"kind": "button", "key": "save", "text": "Again"}]}"#,
    ));
    assert_eq!(error.field, "root.children[1].children[0].key");
    assert_eq!(error.message, "key 'save' is used twice in this file");
    // A tab's key is a file key.
    let tab = error_of(&screen(
        r#"{"kind": "button", "key": "audio", "text": "a"},
           {"kind": "tab_rail", "tabs": [{"key": "audio", "caption": "Audio"}]}"#,
    ));
    assert_eq!(tab.field, "root.children[1].tabs[0].key");
}

#[test]
fn a_key_twice_in_a_repeat_row_fails() {
    let error = error(&screen(
        r#"{"kind": "repeat", "key": "slots", "rows": "slots", "row": {"kind": "row", "children": [
              {"kind": "button", "key": "load", "field": "name"},
              {"kind": "button", "key": "load", "text": "Load"}]}}"#,
    ));
    assert_eq!(error.field, "root.children[0].row.children[1].key");
    assert_eq!(
        error.message,
        "key 'load' is used twice in this repeat's row"
    );
    // A row's keys are its own: a file key may repeat one, and two repeats'
    // rows may share them.
    read(&screen(
        r#"{"kind": "button", "key": "load", "text": "Load"},
           {"kind": "repeat", "key": "a", "rows": "a", "row": {"kind": "button", "key": "load", "text": "x"}},
           {"kind": "repeat", "key": "b", "rows": "b", "row": {"kind": "button", "key": "load", "text": "y"}}"#,
    ));
    let nested = error_of(&screen(
        r#"{"kind": "repeat", "key": "a", "rows": "a", "row": {"kind": "column", "children": [
              {"kind": "repeat", "key": "b", "rows": "b", "row": {"kind": "label", "text": "x"}}]}}"#,
    ));
    assert_eq!(nested.field, "root.children[0].row.children[0].kind");
}

#[test]
fn a_key_with_a_slash_fails() {
    let error = error(&screen(
        r#"{"kind": "button", "key": "items/save", "text": "a"}"#,
    ));
    assert_eq!(error.field, "root.children[0].key");
    assert_eq!(
        error.message,
        "key 'items/save' holds a '/', which only the keys a repeat builds hold"
    );
    let empty = error_of(&screen(r#"{"kind": "button", "key": "", "text": "a"}"#));
    assert_eq!(empty.message, "a name is not empty");
}

#[test]
fn a_field_outside_a_repeat_row_fails() {
    let error = error(&screen(r#"{"kind": "label", "field": "name"}"#));
    assert_eq!(error.field, "root.children[0].field");
    assert_eq!(error.message, "a field is read only inside a repeat's row");
}

#[test]
fn each_kind_needs_its_fields_and_sits_in_its_place() {
    for (children, field, message) in [
        (
            r#"{"kind": "display", "text": "a"}"#,
            "root.children[0].style",
            "a display needs its style",
        ),
        (
            r#"{"kind": "slider_row", "caption": "a"}"#,
            "root.children[0].range",
            "a slider_row needs its range",
        ),
        (
            r#"{"kind": "tab_rail", "tabs": []}"#,
            "root.children[0].tabs",
            "a tab_rail needs its tabs",
        ),
        (
            r#"{"kind": "meter", "width": 1}"#,
            "root.children[0].height",
            "a meter needs its height",
        ),
        (
            r#"{"kind": "repeat", "rows": "a", "row": {"kind": "label", "text": "a"}}"#,
            "root.children[0].key",
            "a repeat needs its key",
        ),
        (
            r#"{"kind": "screen", "width": 1}"#,
            "root.children[0].kind",
            "a screen is only a file's root",
        ),
        (
            r#"{"kind": "toggle_row", "caption": "a", "on": {"setting": "ui_scale"}}"#,
            "root.children[0].on.setting",
            "setting 'ui_scale' is written by a slider_row",
        ),
        (
            r#"{"kind": "label", "text": "a", "bind": {"value": "v"}}"#,
            "root.children[0].bind.value",
            "a label takes no value binding",
        ),
        (
            r#"{"kind": "slider_row", "caption": "a", "range": {"min": 1, "max": 1, "step": 1}}"#,
            "root.children[0].range",
            "min 1 is not below max 1",
        ),
        (
            r#"{"kind": "panel", "overrides": {"background": "accentt"}}"#,
            "root.children[0].overrides.background",
            "",
        ),
        (
            r#"{"kind": "label", "text": "a", "overrides": {"text": {"shrink": 0.5}}}"#,
            "root.children[0].overrides.text.shrink",
            "only a theme's named styles take shrink",
        ),
    ] {
        let error = error(&screen(children));
        assert_eq!(error.field, field, "{children}");
        if !message.is_empty() {
            assert_eq!(error.message, message, "{children}");
        }
    }
    let root = error_of(r#"{"version": 1, "root": {"kind": "label", "text": "a"}}"#);
    assert_eq!(
        (root.field.as_str(), root.message.as_str()),
        ("root.kind", "a file's root is a screen or a root")
    );
    let layer = error_of(r#"{"version": 1, "root": {"kind": "root"}}"#);
    assert_eq!(layer.field, "root.layer");
    let card = error_of(r#"{"version": 1, "root": {"kind": "root", "layer": "game", "card": {}}}"#);
    assert_eq!(card.field, "root.card");
}

#[test]
fn the_canonical_form_round_trips_and_is_idempotent() {
    let file = read(
        r##"{"root": {"children": [
              {"kind": "display", "text": "Ünïcode \"quoted\"", "style": "died",
               "overrides": {"text": {"letter_spacing": 0.3, "color": "#A01818FF", "size": 14.0,
                                      "overflow": {"ellipsis": {"at": "end"}}}}},
              {"kind": "slider_row", "caption": "Scale", "range": {"step": 0.25, "min": 0.5, "max": 2},
               "on": {"setting": "ui_scale"}, "readout": "scale"},
              {"kind": "group", "layout": {"width": "33.5%", "max_width": "none", "padding": 7.25,
                                           "margin": {"top": "title_space"}}},
              {"kind": "button", "text": "Back", "on": "pop", "mono": false}],
             "kind": "screen", "width": 1e3}, "version": 1}"##,
    );
    let canonical = file.to_canonical();
    assert_eq!(read(&canonical), file);
    assert_eq!(read(&canonical).to_canonical(), canonical);
    assert!(canonical.ends_with("}\n"));
    assert!(
        canonical.contains("\"letter_spacing\": 0.3,"),
        "{canonical}"
    );
    assert!(canonical.contains("\"color\": \"#a01818\""), "{canonical}");
    assert!(canonical.contains("\"width\": \"33.5%\""), "{canonical}");
    assert!(canonical.contains("\"width\": 1000"), "{canonical}");
    assert!(
        !canonical.contains("mono"),
        "defaults are left out: {canonical}"
    );
}

#[test]
fn the_canonical_form_sorts_keys_and_writes_whole_numbers_bare() {
    let file = read(
        r#"{"version": 1, "root": {"width": 260.0, "kind": "screen", "children": [
              {"text": "Paused", "kind": "heading", "centred": true},
              {"on": {"action": "save"}, "text": "Save", "key": "save", "kind": "button",
               "layout": {"gap": 2.5, "padding": 14}}]}}"#,
    );
    let expected = r#"{
  "root": {
    "children": [
      {
        "centred": true,
        "kind": "heading",
        "text": "Paused"
      },
      {
        "key": "save",
        "kind": "button",
        "layout": {
          "gap": 2.5,
          "padding": 14
        },
        "on": {
          "action": "save"
        },
        "text": "Save"
      }
    ],
    "kind": "screen",
    "width": 260
  },
  "version": 1
}
"#;
    assert_eq!(file.to_canonical(), expected);
}

#[test]
fn check_names_each_unknown_name_with_its_path() {
    let file = read(
        r#"{"version": 1, "root": {"kind": "screen", "width": "menu_width", "children": [
              {"kind": "display", "bind": {"text": "game_tile"}, "style": "title"},
              {"kind": "button", "text": "a", "style": "loud", "bind": {"enabled": "saving_on"},
               "layout": {"padding": {"y": "title_button_pad"}}, "on": {"push": "options"}},
              {"kind": "button", "text": "b", "on": {"push": "hud"}},
              {"kind": "button", "text": "c", "on": {"action": "sav"}},
              {"kind": "toggle_row", "caption": "a", "bind": {"value": "vsync"}},
              {"kind": "slider_row", "caption": "b", "range": {"min": 0, "max": 1, "step": 1},
               "readout": "volume_text"},
              {"kind": "meter", "width": 1, "height": 1, "bind": {"fill": "hp", "visible": "alive"}},
              {"kind": "mount", "mount": "cards"},
              {"kind": "repeat", "key": "slots", "rows": "slot_rows", "row": {"kind": "label", "field": "name"}}
            ]}}"#,
    );
    let names = |list: &[&str]| list.iter().map(|name| (*name).to_owned()).collect();
    let known = ScreenNames {
        text: names(&["game_title"]),
        enabled: names(&["saving_on"]),
        value: names(&["vsync"]),
        styles: names(&["title"]),
        sizes: names(&["menu_width"]),
        screens: names(&["settings", "hud"]),
        refused: names(&["hud"]),
        ..ScreenNames::default()
    };
    let issues: Vec<(String, String)> = file
        .check(&known)
        .into_iter()
        .map(|issue| (issue.field, issue.message))
        .collect();
    let expected = [
        ("root.children[0].bind.text", "no text binding 'game_tile'"),
        ("root.children[1].style", "no text style 'loud'"),
        ("root.children[1].on.push", "no screen to push 'options'"),
        ("root.children[1].layout.padding.y", "no size 'title_button_pad'"),
        (
            "root.children[2].on.push",
            "screen 'hud' is a file a data screen refuses: its root is not modal on the popup layer",
        ),
        ("root.children[3].on.action", "no action 'sav'"),
        ("root.children[5].readout", "no text binding 'volume_text'"),
        ("root.children[6].bind.fill", "no fill binding 'hp'"),
        ("root.children[6].bind.visible", "no visible binding 'alive'"),
        ("root.children[7].mount", "no mount 'cards'"),
        ("root.children[8].rows", "no row provider 'slot_rows'"),
    ]
    .map(|(field, message)| (field.to_owned(), message.to_owned()));
    assert_eq!(issues, expected);
    assert!(
        file.check(&file.uses()).is_empty(),
        "a file's own names pass"
    );
}

#[test]
fn the_registry_holds_files_by_id_and_interns_their_ids_once() {
    let mut registry = UiScreenRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.intern("pause"), None, "only loaded IDs intern");
    let file = read(&screen(""));
    assert!(registry.insert("pause", file.clone()).is_none());
    registry.insert("over", file.clone());
    assert_eq!(registry.ids(), ["over", "pause"]);
    assert_eq!(registry.get("pause"), Some(&file));
    let first = registry.intern("pause").unwrap();
    assert_eq!(first, "pause");
    assert!(std::ptr::eq(first, registry.intern("pause").unwrap()));
    assert!(registry.insert("pause", file).is_some());
    assert!(std::ptr::eq(first, registry.intern("pause").unwrap()));
    assert_eq!(registry.len(), 2);
}

#[test]
fn the_screen_schema_lists_every_field_the_model_reads() {
    let schema = json_schema();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    let defs = schema["$defs"].as_object().unwrap();
    let properties = |def: &Value| -> BTreeSet<String> {
        def["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{def} has no properties"))
            .keys()
            .cloned()
            .collect()
    };
    let kinds: Vec<&Value> = Kind::ALL.iter().map(|kind| &defs[kind.name()]).collect();
    let probes: [(&str, BTreeSet<String>); 14] = [
        (
            r#"{"version": 1, "root": {"kind": "screen", "width": 1}, "zz": 1}"#,
            properties(&schema),
        ),
        (
            r#"{"version": 1, "root": {"kind": "screen", "width": 1, "zz": 1}}"#,
            kinds.iter().flat_map(|def| properties(def)).collect(),
        ),
        (
            r#"{"version": 1, "root": {"kind": "screen", "width": 1, "layout": {"zz": 1}}}"#,
            properties(&defs["layout"]),
        ),
        (
            r#"{"version": 1, "root": {"kind": "screen", "width": 1, "layout": {"anchor": {"corner": "center", "zz": 1}}}}"#,
            properties(&defs["anchor"]),
        ),
        (
            r#"{"version": 1, "root": {"kind": "screen", "width": 1, "layout": {"padding": {"zz": 1}}}}"#,
            properties(&defs["edges"]["anyOf"][1]),
        ),
        (
            r#"{"version": 1, "root": {"kind": "screen", "width": 1, "bind": {"zz": "a"}}}"#,
            properties(&defs["bind"]),
        ),
        (
            r#"{"version": 1, "root": {"kind": "screen", "width": 1, "card": {"zz": 1}}}"#,
            properties(&defs["card"]),
        ),
        (
            r#"{"version": 1, "root": {"kind": "screen", "width": 1, "overrides": {"zz": 1}}}"#,
            properties(&defs["overrides"]),
        ),
        (
            r#"{"version": 1, "root": {"kind": "screen", "width": 1, "overrides": {"text": {"zz": 1}}}}"#,
            properties(&defs["text"]),
        ),
        (
            r##"{"version": 1, "root": {"kind": "screen", "width": 1, "overrides": {"text": {"outline": {"color": "#000000", "width": 1, "zz": 1}}}}}"##,
            properties(&defs["outline"]),
        ),
        (
            r##"{"version": 1, "root": {"kind": "screen", "width": 1, "overrides": {"text": {"shadow": {"color": "#000000", "offset": [1, 1], "zz": 1}}}}}"##,
            properties(&defs["shadow"]),
        ),
        (
            r#"{"version": 1, "root": {"kind": "screen", "width": 1, "overrides": {"text": {"overflow": {"ellipsis": {"at": "end", "zz": 1}}}}}}"#,
            properties(&defs["ellipsis"]),
        ),
        (
            &screen(
                r#"{"kind": "slider_row", "caption": "a", "range": {"min": 0, "max": 1, "step": 1, "zz": 1}}"#,
            ),
            properties(&defs["range"]),
        ),
        (
            &screen(r#"{"kind": "tab_rail", "tabs": [{"key": "a", "caption": "A", "zz": 1}]}"#),
            properties(&defs["tab"]),
        ),
    ];
    for (json, schema_fields) in probes {
        let error = error(json);
        assert!(
            error.message.contains("unknown field `zz`"),
            "{json}: {error}"
        );
        assert_eq!(expected(&error.message), schema_fields, "{json}");
    }

    // The kinds, the actions and the settings are the schema's.
    let error = error(r#"{"version": 1, "root": {"kind": "zz"}}"#);
    let consts: BTreeSet<String> = kinds
        .iter()
        .map(|def| {
            def["properties"]["kind"]["const"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(expected(&error.message), consts);
    let action = &defs["action"]["oneOf"];
    let error = error_of(&screen(
        r#"{"kind": "button", "text": "a", "on": {"zz": "a"}}"#,
    ));
    let mut actions: BTreeSet<String> = BTreeSet::from(["pop".to_owned()]);
    for each in action.as_array().unwrap().iter().skip(1) {
        actions.extend(properties(each));
    }
    assert_eq!(expected(&error.message), actions);
    let error = error_of(&screen(
        r#"{"kind": "toggle_row", "caption": "a", "on": {"setting": "zz"}}"#,
    ));
    let settings = &action[3]["properties"]["setting"]["enum"];
    let settings: BTreeSet<String> = settings
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(expected(&error.message), settings);
    for kind in Kind::ALL {
        let required = defs[kind.name()]["required"].as_array().unwrap();
        assert_eq!(required.len(), 1 + kind.required().len(), "{}", kind.name());
    }
}
