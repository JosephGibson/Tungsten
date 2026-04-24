//! Core-owned action map; pure data over read-only `InputState` queries.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::input::{InputState, KeyCode, MouseButton, ScrollDirection};

/// Physical input binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Binding {
    Key { code: KeyCode },
    Mouse { button: MouseButton },
    Scroll { direction: ScrollDirection },
}

/// String action names mapped to physical bindings.
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
    /// Missing-file error check for default fallback.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound)
    }
}

impl ActionMap {
    /// Empty action map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Engine default bindings for examples and engine-owned controls.
    #[must_use]
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
            "engine_toggle_physics_debug".into(),
            vec![Binding::Key { code: KeyCode::F1 }],
        );
        actions.insert(
            "engine_toggle_systems_overlay".into(),
            vec![Binding::Key { code: KeyCode::F2 }],
        );
        actions.insert(
            "engine_toggle_inspector".into(),
            vec![Binding::Key { code: KeyCode::F3 }],
        );
        actions.insert(
            "engine_inspector_pick".into(),
            vec![Binding::Mouse {
                button: MouseButton::Middle,
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

    /// Set path for later persistence.
    pub fn set_source_path(&mut self, path: impl Into<PathBuf>) {
        self.source_path = Some(path.into());
    }

    /// Load action map JSON.
    pub fn load(path: &Path) -> Result<Self, ActionMapError> {
        let contents = std::fs::read_to_string(path).map_err(|source| ActionMapError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_json(&contents, path)
    }

    /// Parse action map JSON text.
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

    /// Merge loaded map over defaults; empty list disables action.
    #[must_use]
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

    /// Persist to remembered source path via temp-file rename.
    pub fn persist(&mut self) -> Result<(), ActionMapError> {
        let path = self
            .source_path
            .clone()
            .ok_or(ActionMapError::MissingSourcePath)?;
        self.persist_to(&path)
    }

    /// Persist to path and remember it.
    pub fn persist_to(&mut self, path: &Path) -> Result<(), ActionMapError> {
        let rendered = self.render_for_save()?;
        atomic_write(path, &rendered)?;
        self.source_path = Some(path.to_path_buf());
        self.source_text = Some(rendered);
        Ok(())
    }

    /// Bindings for action; unknown returns empty slice.
    pub fn bindings(&self, action: &str) -> &[Binding] {
        self.actions.get(action).map_or(&[], Vec::as_slice)
    }

    /// Replace action bindings; call `persist` to save.
    pub fn replace_bindings(&mut self, action: impl Into<String>, bindings: Vec<Binding>) {
        let mut bindings = bindings;
        dedupe_in_place(&mut bindings);
        self.actions.insert(action.into(), bindings);
    }

    /// Replace bindings and persist immediately.
    pub fn replace_bindings_and_persist(
        &mut self,
        action: impl Into<String>,
        bindings: Vec<Binding>,
    ) -> Result<(), ActionMapError> {
        self.replace_bindings(action, bindings);
        self.persist()
    }

    /// Any binding currently held.
    #[must_use]
    pub fn is_pressed(&self, input: &InputState, action: &str) -> bool {
        self.bindings(action).iter().any(|binding| match *binding {
            Binding::Key { code } => input.is_pressed(code),
            Binding::Mouse { button } => input.is_mouse_pressed(button),
            Binding::Scroll { direction } => input.is_scroll_active(direction),
        })
    }

    /// Any binding transitioned pressed this frame.
    #[must_use]
    pub fn just_pressed(&self, input: &InputState, action: &str) -> bool {
        self.bindings(action).iter().any(|binding| match *binding {
            Binding::Key { code } => input.just_pressed(code),
            Binding::Mouse { button } => input.mouse_just_pressed(button),
            Binding::Scroll { direction } => input.scroll_just_pressed(direction),
        })
    }

    /// Any binding transitioned released this frame.
    #[must_use]
    pub fn just_released(&self, input: &InputState, action: &str) -> bool {
        self.bindings(action).iter().any(|binding| match *binding {
            Binding::Key { code } => input.just_released(code),
            Binding::Mouse { button } => input.mouse_just_released(button),
            Binding::Scroll { direction } => input.scroll_just_released(direction),
        })
    }

    /// Iterate `(action, bindings)`; order unstable.
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
    let line_start = source[..index].rfind('\n').map_or(0, |pos| pos + 1);
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

/// Resolve path for logging.
#[must_use]
pub fn resolve_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
#[path = "../tests/input/action_map.rs"]
mod tests;
