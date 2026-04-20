//! Core-owned action map. Maps string action names to lists of `Binding`
//! (keys, mouse buttons, or discrete scroll directions) and resolves
//! edge/pressed queries against `InputState`. Loaded from an optional
//! workspace-root `input.json`; a built-in default map covers every action
//! consumed by the in-tree examples so missing bindings fall back gracefully.
//!
//! Frame-order invariant: `ActionMap` is a pure data resource. Queries are
//! read-only against `InputState` and can run anywhere in the update stage
//! without affecting the canonical system -> flush -> event -> hot reload ->
//! extract -> render order.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::input::{InputState, KeyCode, MouseButton, ScrollDirection};

/// A single physical input binding. `Key` fires against keyboard scan codes,
/// `Mouse` fires against mouse buttons, and `Scroll` fires as a one-frame
/// impulse for the matching wheel direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Binding {
    Key { code: KeyCode },
    Mouse { button: MouseButton },
    Scroll { direction: ScrollDirection },
}

/// Core-owned action map. The inner storage is a plain `HashMap<String,
/// Vec<Binding>>`. Queries take an `&InputState` so the resource stays
/// read-only during a frame.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionMap {
    #[serde(default)]
    actions: HashMap<String, Vec<Binding>>,
    #[serde(skip)]
    source_path: Option<PathBuf>,
    #[serde(skip)]
    source_text: Option<String>,
}

#[derive(Debug, Error)]
pub enum ActionMapError {
    #[error("failed to read action map '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("invalid action map in '{path}': {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("failed to write action map '{path}': {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("action map has no source path to persist")]
    MissingSourcePath,
}

impl ActionMapError {
    /// True when the underlying cause is a missing file. Callers use this to
    /// fall back to `ActionMap::default_map()` without treating the missing
    /// file as an error.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound)
    }
}

impl ActionMap {
    /// Build an empty map. Use `default_map` for the engine-default bindings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Engine default bindings. These cover every action consumed by the
    /// in-tree examples plus the engine-owned controls (`F4`, `F9`, `F11`,
    /// `Escape`) so the default behaviour survives deleting `input.json`.
    pub fn default_map() -> Self {
        let mut actions: HashMap<String, Vec<Binding>> = HashMap::new();
        actions.insert(
            "move_left".into(),
            vec![
                Binding::Key {
                    code: KeyCode::ArrowLeft,
                },
                Binding::Key {
                    code: KeyCode::KeyA,
                },
            ],
        );
        actions.insert(
            "move_right".into(),
            vec![
                Binding::Key {
                    code: KeyCode::ArrowRight,
                },
                Binding::Key {
                    code: KeyCode::KeyD,
                },
            ],
        );
        actions.insert(
            "jump".into(),
            vec![
                Binding::Key {
                    code: KeyCode::Space,
                },
                Binding::Mouse {
                    button: MouseButton::Left,
                },
            ],
        );
        actions.insert(
            "zoom_in".into(),
            vec![
                Binding::Key {
                    code: KeyCode::Equal,
                },
                Binding::Scroll {
                    direction: ScrollDirection::Up,
                },
            ],
        );
        actions.insert(
            "zoom_out".into(),
            vec![
                Binding::Key {
                    code: KeyCode::Minus,
                },
                Binding::Scroll {
                    direction: ScrollDirection::Down,
                },
            ],
        );
        actions.insert(
            "audio_toggle_music".into(),
            vec![
                Binding::Key {
                    code: KeyCode::KeyM,
                },
                Binding::Mouse {
                    button: MouseButton::Right,
                },
            ],
        );
        actions.insert(
            "audio_stop_all".into(),
            vec![
                Binding::Key {
                    code: KeyCode::KeyS,
                },
                Binding::Mouse {
                    button: MouseButton::Middle,
                },
            ],
        );
        actions.insert(
            "volume_preset_low".into(),
            vec![Binding::Key {
                code: KeyCode::Digit1,
            }],
        );
        actions.insert(
            "volume_preset_mid".into(),
            vec![Binding::Key {
                code: KeyCode::Digit2,
            }],
        );
        actions.insert(
            "volume_preset_high".into(),
            vec![Binding::Key {
                code: KeyCode::Digit3,
            }],
        );
        actions.insert(
            "engine_toggle_hud".into(),
            vec![Binding::Key { code: KeyCode::F4 }],
        );
        actions.insert(
            "engine_toggle_vsync".into(),
            vec![Binding::Key { code: KeyCode::F9 }],
        );
        actions.insert(
            "engine_toggle_fullscreen".into(),
            vec![Binding::Key { code: KeyCode::F11 }],
        );
        actions.insert(
            "engine_exit".into(),
            vec![Binding::Key {
                code: KeyCode::Escape,
            }],
        );
        actions.insert(
            "state_start".into(),
            vec![Binding::Key {
                code: KeyCode::Enter,
            }],
        );
        actions.insert(
            "state_pause".into(),
            vec![Binding::Key {
                code: KeyCode::KeyP,
            }],
        );
        actions.insert(
            "state_back".into(),
            vec![Binding::Key {
                code: KeyCode::Backspace,
            }],
        );
        Self {
            actions,
            source_path: None,
            source_text: None,
        }
    }

    /// Record where this action map should be written when persisted. Useful
    /// when the map was synthesized from defaults because `input.json` was
    /// absent at startup.
    pub fn set_source_path(&mut self, path: impl Into<PathBuf>) {
        self.source_path = Some(path.into());
    }

    /// Load an action map from a JSON file. Parse errors carry the path so the
    /// caller can surface a clear startup-fatal message.
    pub fn load(path: &Path) -> Result<Self, ActionMapError> {
        let contents = std::fs::read_to_string(path).map_err(|source| ActionMapError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_json(&contents, path)
    }

    /// Parse an action map from a JSON string. Used by `load` and tests.
    pub fn from_json(contents: &str, path: &Path) -> Result<Self, ActionMapError> {
        let mut map: Self =
            serde_json::from_str(contents).map_err(|source| ActionMapError::Parse {
                path: path.display().to_string(),
                source,
            })?;
        map.deduplicate_bindings();
        map.source_path = Some(path.to_path_buf());
        map.source_text = Some(contents.to_string());
        Ok(map)
    }

    /// Merge `loaded` on top of the engine defaults. User-supplied entries
    /// override the default for that action (even an empty list disables the
    /// action). Actions present only in defaults are preserved.
    pub fn merged_with_defaults(loaded: Self) -> Self {
        let ActionMap {
            actions,
            source_path,
            source_text,
        } = loaded;
        let mut merged = Self::default_map();
        for (action, bindings) in actions {
            merged.actions.insert(action, bindings);
        }
        merged.deduplicate_bindings();
        merged.source_path = source_path;
        merged.source_text = source_text;
        merged
    }

    /// Persist the current map back to its remembered source path. Uses an
    /// atomic temp-file + rename path inside the source directory.
    pub fn persist(&mut self) -> Result<(), ActionMapError> {
        let path = self
            .source_path
            .clone()
            .ok_or(ActionMapError::MissingSourcePath)?;
        self.persist_to(&path)
    }

    /// Persist the current map to `path`, updating the remembered source path
    /// and raw source text on success.
    pub fn persist_to(&mut self, path: &Path) -> Result<(), ActionMapError> {
        let rendered = self.render_for_save()?;
        atomic_write(path, &rendered)?;
        self.source_path = Some(path.to_path_buf());
        self.source_text = Some(rendered);
        Ok(())
    }

    /// Return the bindings for an action, or `&[]` if the action is unknown.
    /// Borrowed slice: no allocation on the query path.
    pub fn bindings(&self, action: &str) -> &[Binding] {
        self.actions.get(action).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Replace every binding for `action`. Used by runtime rebind flows
    /// (debug tooling, eventual settings menu). Call `persist` afterwards if
    /// the new binding should be written back to `input.json`.
    pub fn replace_bindings(&mut self, action: impl Into<String>, bindings: Vec<Binding>) {
        let mut bindings = bindings;
        dedupe_in_place(&mut bindings);
        self.actions.insert(action.into(), bindings);
    }

    /// Convenience for engine-side runtime rebind flows: replace the action's
    /// bindings, then immediately write the updated map back to disk.
    pub fn replace_bindings_and_persist(
        &mut self,
        action: impl Into<String>,
        bindings: Vec<Binding>,
    ) -> Result<(), ActionMapError> {
        self.replace_bindings(action, bindings);
        self.persist()
    }

    /// True if any binding for `action` is currently held. Unknown or empty
    /// actions return `false`.
    pub fn is_pressed(&self, input: &InputState, action: &str) -> bool {
        self.bindings(action).iter().any(|binding| match *binding {
            Binding::Key { code } => input.is_pressed(code),
            Binding::Mouse { button } => input.is_mouse_pressed(button),
            Binding::Scroll { direction } => input.is_scroll_active(direction),
        })
    }

    /// True if any binding for `action` transitioned from released to pressed
    /// this frame.
    pub fn just_pressed(&self, input: &InputState, action: &str) -> bool {
        self.bindings(action).iter().any(|binding| match *binding {
            Binding::Key { code } => input.just_pressed(code),
            Binding::Mouse { button } => input.mouse_just_pressed(button),
            Binding::Scroll { direction } => input.scroll_just_pressed(direction),
        })
    }

    /// True if any binding for `action` transitioned from pressed to released
    /// this frame.
    pub fn just_released(&self, input: &InputState, action: &str) -> bool {
        self.bindings(action).iter().any(|binding| match *binding {
            Binding::Key { code } => input.just_released(code),
            Binding::Mouse { button } => input.mouse_just_released(button),
            Binding::Scroll { direction } => input.scroll_just_released(direction),
        })
    }

    /// Iterator over `(action, bindings)` pairs. Stable ordering is not
    /// guaranteed (backed by `HashMap`).
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[Binding])> {
        self.actions
            .iter()
            .map(|(name, bindings)| (name.as_str(), bindings.as_slice()))
    }

    fn deduplicate_bindings(&mut self) {
        for bindings in self.actions.values_mut() {
            dedupe_in_place(bindings);
        }
    }

    fn render_for_save(&self) -> Result<String, ActionMapError> {
        let actions_object = canonical_actions_object(&self.actions)?;
        if let Some(existing) = &self.source_text {
            if let Some(patched) = replace_top_level_actions_object(existing, &actions_object) {
                return Ok(patched);
            }
        }
        canonical_document(&self.actions)
    }
}

fn dedupe_in_place(bindings: &mut Vec<Binding>) {
    let mut seen: Vec<Binding> = Vec::with_capacity(bindings.len());
    bindings.retain(|binding| {
        if seen.contains(binding) {
            false
        } else {
            seen.push(*binding);
            true
        }
    });
}

fn canonical_actions_map(
    actions: &HashMap<String, Vec<Binding>>,
) -> BTreeMap<String, Vec<Binding>> {
    actions
        .iter()
        .map(|(action, bindings)| (action.clone(), bindings.clone()))
        .collect()
}

fn canonical_actions_object(
    actions: &HashMap<String, Vec<Binding>>,
) -> Result<String, ActionMapError> {
    serde_json::to_string_pretty(&canonical_actions_map(actions)).map_err(|source| {
        ActionMapError::Parse {
            path: "<persist>".into(),
            source,
        }
    })
}

fn canonical_document(actions: &HashMap<String, Vec<Binding>>) -> Result<String, ActionMapError> {
    #[derive(Serialize)]
    struct SerializableActionMap {
        actions: BTreeMap<String, Vec<Binding>>,
    }

    serde_json::to_string_pretty(&SerializableActionMap {
        actions: canonical_actions_map(actions),
    })
    .map_err(|source| ActionMapError::Parse {
        path: "<persist>".into(),
        source,
    })
}

fn replace_top_level_actions_object(source: &str, replacement: &str) -> Option<String> {
    let (value_start, value_end, indent) = find_top_level_actions_value(source)?;
    let replacement = indent_multiline(replacement, &indent);
    let mut rendered = String::with_capacity(source.len() + replacement.len());
    rendered.push_str(&source[..value_start]);
    rendered.push_str(&replacement);
    rendered.push_str(&source[value_end..]);
    Some(rendered)
}

fn find_top_level_actions_value(source: &str) -> Option<(usize, usize, String)> {
    let bytes = source.as_bytes();
    let mut i = 0usize;
    let mut depth = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let end = scan_string_end(bytes, i)?;
                if depth == 1 && &source[i..=end] == "\"actions\"" {
                    let mut cursor = skip_ws(bytes, end + 1);
                    if bytes.get(cursor) != Some(&b':') {
                        i = end + 1;
                        continue;
                    }
                    cursor = skip_ws(bytes, cursor + 1);
                    if bytes.get(cursor) != Some(&b'{') {
                        i = end + 1;
                        continue;
                    }
                    let value_end = scan_matching_brace(bytes, cursor)? + 1;
                    return Some((cursor, value_end, line_indent(source, i)));
                }
                i = end + 1;
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    None
}

fn scan_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

fn scan_matching_brace(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    let mut depth = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => i = scan_string_end(bytes, i)? + 1,
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1)?;
                i += 1;
                if depth == 0 {
                    return Some(i - 1);
                }
            }
            _ => i += 1,
        }
    }
    None
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while matches!(bytes.get(index), Some(b' ' | b'\n' | b'\r' | b'\t')) {
        index += 1;
    }
    index
}

fn line_indent(source: &str, index: usize) -> String {
    let line_start = source[..index].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
    source[line_start..index]
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect()
}

fn indent_multiline(text: &str, indent: &str) -> String {
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };

    let mut rendered = String::new();
    rendered.push_str(first);
    for line in lines {
        rendered.push('\n');
        rendered.push_str(indent);
        rendered.push_str(line);
    }
    rendered
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), ActionMapError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|source| ActionMapError::Write {
        path: path.display().to_string(),
        source,
    })?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("input.json");
    let temp_path = next_temp_path(parent, file_name);

    let mut file = File::create(&temp_path).map_err(|source| ActionMapError::Write {
        path: path.display().to_string(),
        source,
    })?;
    file.write_all(contents.as_bytes())
        .map_err(|source| ActionMapError::Write {
            path: path.display().to_string(),
            source,
        })?;
    file.sync_all().map_err(|source| ActionMapError::Write {
        path: path.display().to_string(),
        source,
    })?;
    drop(file);

    if let Err(source) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(ActionMapError::Write {
            path: path.display().to_string(),
            source,
        });
    }

    Ok(())
}

fn next_temp_path(parent: &Path, file_name: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce))
}

/// Convenience: resolve a relative or absolute path to an absolute `PathBuf`.
/// Used by call sites that want to report a canonical path in log messages.
pub fn resolve_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
        "actions": {
            "move_left":   [{ "kind": "key", "code": "ArrowLeft" }, { "kind": "key", "code": "KeyA" }],
            "jump":        [{ "kind": "key", "code": "Enter" }, { "kind": "mouse", "button": "button4" }],
            "zoom_in":     [{ "kind": "scroll", "direction": "up" }],
            "fire":        [{ "kind": "mouse", "button": "left" }]
        }
    }"#;

    #[test]
    fn default_map_has_platformer_and_engine_actions() {
        let map = ActionMap::default_map();
        for action in [
            "move_left",
            "move_right",
            "jump",
            "audio_toggle_music",
            "audio_stop_all",
            "volume_preset_low",
            "volume_preset_mid",
            "volume_preset_high",
            "zoom_in",
            "zoom_out",
            "engine_toggle_hud",
            "engine_toggle_vsync",
            "engine_toggle_fullscreen",
            "engine_exit",
            "state_start",
            "state_pause",
            "state_back",
        ] {
            assert!(
                !map.bindings(action).is_empty(),
                "default map missing action '{action}'"
            );
        }
        assert!(map.bindings("jump").contains(&Binding::Mouse {
            button: MouseButton::Left
        }));
        assert!(map.bindings("zoom_in").contains(&Binding::Scroll {
            direction: ScrollDirection::Up
        }));
    }

    #[test]
    fn unknown_action_returns_false() {
        let map = ActionMap::default_map();
        let input = InputState::new();
        assert!(!map.is_pressed(&input, "dance"));
        assert!(!map.just_pressed(&input, "dance"));
        assert!(!map.just_released(&input, "dance"));
        assert!(map.bindings("dance").is_empty());
    }

    #[test]
    fn merged_with_defaults_preserves_user_overrides() {
        let loaded: ActionMap =
            ActionMap::from_json(SAMPLE_JSON, Path::new("<test>")).expect("parse sample");
        let merged = ActionMap::merged_with_defaults(loaded);

        let jump = merged.bindings("jump");
        assert_eq!(
            jump,
            &[
                Binding::Key {
                    code: KeyCode::Enter
                },
                Binding::Mouse {
                    button: MouseButton::Other(4)
                }
            ]
        );

        assert!(merged
            .bindings("engine_toggle_hud")
            .contains(&Binding::Key { code: KeyCode::F4 }));

        assert_eq!(
            merged.bindings("fire"),
            &[Binding::Mouse {
                button: MouseButton::Left
            }]
        );
    }

    #[test]
    fn load_parses_sample_input_json() {
        let map = ActionMap::from_json(SAMPLE_JSON, Path::new("<sample>")).unwrap();
        assert_eq!(
            map.bindings("move_left"),
            &[
                Binding::Key {
                    code: KeyCode::ArrowLeft
                },
                Binding::Key {
                    code: KeyCode::KeyA
                }
            ]
        );
        assert_eq!(
            map.bindings("jump"),
            &[
                Binding::Key {
                    code: KeyCode::Enter
                },
                Binding::Mouse {
                    button: MouseButton::Other(4)
                }
            ]
        );
        assert_eq!(
            map.bindings("zoom_in"),
            &[Binding::Scroll {
                direction: ScrollDirection::Up
            }]
        );
    }

    #[test]
    fn load_invalid_json_is_error() {
        let err = ActionMap::from_json("{ not json", Path::new("<bad>")).unwrap_err();
        assert!(matches!(err, ActionMapError::Parse { .. }));
    }

    #[test]
    fn query_respects_multiple_bindings() {
        let map = ActionMap::default_map();
        let mut input = InputState::new();
        input.key_down(KeyCode::KeyA);
        assert!(map.is_pressed(&input, "move_left"));
        input.key_up(KeyCode::KeyA);
        input.begin_frame();
        input.key_down(KeyCode::ArrowLeft);
        assert!(map.is_pressed(&input, "move_left"));
    }

    #[test]
    fn edge_query_just_pressed_any_binding() {
        let map = ActionMap::default_map();
        let mut input = InputState::new();
        input.key_down(KeyCode::ArrowLeft);
        assert!(map.just_pressed(&input, "move_left"));
        input.begin_frame();
        assert!(!map.just_pressed(&input, "move_left"));
    }

    #[test]
    fn mouse_button_binding_dispatches() {
        let map = ActionMap::default_map();
        let mut input = InputState::new();
        input.mouse_down(MouseButton::Left);
        assert!(map.is_pressed(&input, "jump"));
        assert!(map.just_pressed(&input, "jump"));
        input.begin_frame();
        input.mouse_up(MouseButton::Left);
        assert!(map.just_released(&input, "jump"));
    }

    #[test]
    fn scroll_binding_dispatches() {
        let map = ActionMap::default_map();
        let mut input = InputState::new();
        input.add_scroll_line_delta(0.0, 1.0);
        assert!(map.is_pressed(&input, "zoom_in"));
        assert!(map.just_pressed(&input, "zoom_in"));
        input.begin_frame();
        assert!(!map.is_pressed(&input, "zoom_in"));
        assert!(map.just_released(&input, "zoom_in"));
    }

    #[test]
    fn replace_bindings_rebinds_runtime() {
        let mut map = ActionMap::default_map();
        map.replace_bindings(
            "jump",
            vec![
                Binding::Key {
                    code: KeyCode::Enter,
                },
                Binding::Mouse {
                    button: MouseButton::Other(5),
                },
            ],
        );
        let mut input = InputState::new();
        input.key_down(KeyCode::Enter);
        assert!(map.is_pressed(&input, "jump"));
        input.key_up(KeyCode::Enter);
        input.mouse_down(MouseButton::Other(5));
        assert!(map.is_pressed(&input, "jump"));
        input.mouse_up(MouseButton::Other(5));
        input.key_down(KeyCode::Space);
        assert!(!map.is_pressed(&input, "jump"));
    }

    #[test]
    fn duplicate_bindings_are_deduplicated() {
        let dupes = r#"{ "actions": { "jump": [
            { "kind": "key", "code": "Space" },
            { "kind": "key", "code": "Space" },
            { "kind": "scroll", "direction": "up" },
            { "kind": "scroll", "direction": "up" }
        ] } }"#;
        let map = ActionMap::from_json(dupes, Path::new("<dupe>")).unwrap();
        assert_eq!(
            map.bindings("jump"),
            &[
                Binding::Key {
                    code: KeyCode::Space
                },
                Binding::Scroll {
                    direction: ScrollDirection::Up
                }
            ]
        );
    }

    #[test]
    fn binding_round_trip_json() {
        let bindings = vec![
            Binding::Key {
                code: KeyCode::ArrowLeft,
            },
            Binding::Mouse {
                button: MouseButton::Other(4),
            },
            Binding::Scroll {
                direction: ScrollDirection::Down,
            },
        ];
        let json = serde_json::to_string(&bindings).unwrap();
        let parsed: Vec<Binding> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, bindings);
    }

    #[test]
    fn persist_round_trips_rebound_actions_and_preserves_other_top_level_fields() {
        let dir = tempdir();
        let path = dir.join("input.json");
        std::fs::write(
            &path,
            "{\n  \"note\": \"keep\",\n  \"actions\": {\n    \"jump\": [{ \"kind\": \"key\", \"code\": \"Space\" }]\n  }\n}\n",
        )
        .unwrap();

        let mut map = ActionMap::load(&path).unwrap();
        map.replace_bindings(
            "jump",
            vec![Binding::Key {
                code: KeyCode::Enter,
            }],
        );
        map.persist().unwrap();

        let persisted = std::fs::read_to_string(&path).unwrap();
        assert!(persisted.contains("\"note\": \"keep\""));
        assert!(persisted.contains("\"Enter\""));

        let reparsed = ActionMap::load(&path).unwrap();
        assert_eq!(
            reparsed.bindings("jump"),
            &[Binding::Key {
                code: KeyCode::Enter
            }]
        );
    }

    #[test]
    fn persist_can_create_missing_input_json_from_defaults() {
        let dir = tempdir();
        let path = dir.join("input.json");
        let mut map = ActionMap::default_map();
        map.set_source_path(&path);
        map.persist().unwrap();

        let persisted = std::fs::read_to_string(&path).unwrap();
        assert!(persisted.contains("\"engine_toggle_hud\""));
        assert!(persisted.contains("\"zoom_in\""));
    }

    fn tempdir() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "tungsten_action_map_test_{}_{}",
            std::process::id(),
            nonce
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
