//! Movable windows (M57): drag handles, whose root follows the pointer once
//! a primary press on them has moved past the router's drag threshold, the
//! root's place as an anchored top-left clamped to the viewport, and
//! [`WindowPlacements`], the file that keeps named windows' places between
//! runs. The router does the dragging; this module holds the flag, the
//! clamp and the file.

use std::collections::BTreeMap;
use std::path::PathBuf;

use glam::Vec2;

use super::id::WidgetId;
use super::style::{Anchor, Placement};
use super::tree::UiTree;
use crate::persist::{WriteError, quarantine, write_atomic};

/// How much of a root a viewport keeps showing when the root's size is not
/// known yet, in UI units: enough for a title bar.
const VISIBLE_EDGE: f32 = 48.0;

/// The file version [`WindowPlacements`] reads and writes.
const VERSION: u64 = 1;

impl UiTree {
    /// Makes `id` a drag handle, or a plain widget again (M57): a primary
    /// press on it that moves past the router's drag threshold moves its
    /// root with the pointer instead of clicking. A handle is a pointer
    /// target as an interactive widget is, shows [`CursorIcon::Move`]
    /// unless a cursor is set ([`Self::set_cursor`]), and is not focusable,
    /// so pressing it never moves focus. Marks nothing.
    ///
    /// [`CursorIcon::Move`]: super::CursorIcon::Move
    pub fn set_drag_handle(&mut self, id: WidgetId, on: bool) {
        if let Some(node) = self.node_mut(id) {
            node.drag_handle = on;
        }
    }

    /// Whether `id` is a drag handle; `false` for an invalid ID.
    #[must_use]
    pub fn drag_handle(&self, id: WidgetId) -> bool {
        self.node(id).is_some_and(|node| node.drag_handle)
    }

    /// Places a root's top-left at `position` in UI units: its placement
    /// becomes `Anchored` at `TopStart` with that offset, clamped to the
    /// last layout's viewport. With a laid-out size, each axis keeps the
    /// whole root inside when it fits and its top-left at the viewport's
    /// when it does not; before its first layout, each axis keeps the
    /// top-left within `0..=viewport − 48`, so a title bar shows, and before
    /// any layout, at least 0. Returns whether the placement changed, and
    /// then marks layout; `false`, changing nothing, for a widget that is
    /// not a root or a position that is not finite.
    pub fn set_window_position(&mut self, root: WidgetId, position: Vec2) -> bool {
        let Some(node) = self.node(root) else {
            return false;
        };
        if node.parent.is_some() || !position.is_finite() {
            return false;
        }
        let position = self.clamp_window(position, node.rect.map(|rect| rect.size()));
        let mut style = node.layout_style.clone();
        style.placement = Placement::Anchored {
            anchor: Anchor::TopStart,
            offset: position,
        };
        if style == node.layout_style {
            return false;
        }
        self.set_layout_style(root, style);
        true
    }

    /// A root's top-left after the last layout; `None` before it, for a
    /// collapsed root or for a widget that is not a root.
    #[must_use]
    pub fn window_position(&self, root: WidgetId) -> Option<Vec2> {
        let node = self.node(root)?;
        if node.parent.is_some() {
            return None;
        }
        node.rect.map(|rect| rect.min)
    }

    /// `position` clamped as [`Self::set_window_position`] clamps a root of
    /// `size`.
    fn clamp_window(&self, position: Vec2, size: Option<Vec2>) -> Vec2 {
        let Some(viewport) = self.viewport else {
            return position.max(Vec2::ZERO);
        };
        let axis = |at: f32, extent: f32, size: Option<f32>| match size {
            Some(size) if size <= extent => at.clamp(0.0, extent - size),
            Some(_) => 0.0,
            None => at.clamp(0.0, (extent - VISIBLE_EDGE).max(0.0)),
        };
        Vec2::new(
            axis(position.x, viewport.x, size.map(|size| size.x)),
            axis(position.y, viewport.y, size.map(|size| size.y)),
        )
    }
}

/// Named windows' top-left positions in UI units, kept between runs (M57)
/// in a JSON file: `{"version": 1, "windows": {"<name>": [x, y]}}`.
///
/// [`load`](Self::load) never fails. A missing file loads empty; a corrupt
/// or invalid one loads empty and is renamed `<file>.bad` beside itself, as
/// `Settings::load` does, and a file it cannot read is left where it is,
/// each with one warning; a newer version loads empty, leaves the file
/// alone and warns. An entry that is not two finite numbers is dropped with
/// a warning. Warnings wait in [`take_load_warnings`](Self::take_load_warnings).
/// The umbrella keeps its debug views' places in the user folder's
/// settings (`debug_windows.json`).
#[derive(Debug, Default)]
pub struct WindowPlacements {
    path: Option<PathBuf>,
    positions: BTreeMap<String, Vec2>,
    load_warnings: Vec<String>,
}

impl WindowPlacements {
    /// Reads the placements at `path`, which [`save`](Self::save) then
    /// writes. [`Default`] has no path and saves nothing.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut placements = Self {
            path: Some(path.clone()),
            ..Self::default()
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return placements,
            Err(error) => {
                placements.load_warnings.push(format!(
                    "failed to read window placements '{}': {error}; starting with none",
                    path.display()
                ));
                return placements;
            }
        };
        match parse(&bytes) {
            Ok(File::Windows(entries)) => {
                for (name, entry) in entries {
                    match entry {
                        Some(position) => {
                            placements.positions.insert(name, position);
                        }
                        None => placements.load_warnings.push(format!(
                            "window placement '{name}' in '{}' is not [x, y] of two finite numbers; dropped",
                            path.display()
                        )),
                    }
                }
            }
            Ok(File::Newer(version)) => placements.load_warnings.push(format!(
                "window placements '{}' are version {version}, newer than {VERSION}; starting with none and leaving the file as it is",
                path.display()
            )),
            Err(reason) => {
                let warning = match quarantine(&path) {
                    Ok(backup) => format!(
                        "window placements '{}' are invalid ({reason}); starting with none, the file kept as '{}'",
                        path.display(),
                        backup.display()
                    ),
                    Err(error) => format!(
                        "window placements '{}' are invalid ({reason}); starting with none, and the file could not be moved aside: {error}",
                        path.display()
                    ),
                };
                placements.load_warnings.push(warning);
            }
        }
        placements
    }

    /// The position kept for `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Vec2> {
        self.positions.get(name).copied()
    }

    /// Keeps `position` for `name`; returns `false`, keeping nothing, when
    /// it is not finite.
    pub fn set(&mut self, name: &str, position: Vec2) -> bool {
        if !position.is_finite() {
            return false;
        }
        self.positions.insert(name.to_owned(), position);
        true
    }

    /// Writes every kept position through [`write_atomic`]; with no path,
    /// writes nothing and succeeds.
    ///
    /// # Errors
    ///
    /// The write's error; the file on disk is then unchanged.
    pub fn save(&self) -> Result<(), WriteError> {
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        write_atomic(path, self.to_json().as_bytes())
    }

    /// Takes the warnings [`load`](Self::load) kept, for the caller to log.
    /// Later calls return none.
    pub fn take_load_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.load_warnings)
    }

    /// The file's text: the version, then the windows by name.
    fn to_json(&self) -> String {
        let windows: serde_json::Map<String, serde_json::Value> = self
            .positions
            .iter()
            .map(|(name, position)| (name.clone(), serde_json::json!([position.x, position.y])))
            .collect();
        let mut json = serde_json::to_string_pretty(&serde_json::json!({
            "version": VERSION,
            "windows": windows,
        }))
        .expect("placements serialize: numbers under string keys");
        json.push('\n');
        json
    }
}

/// What a readable file holds.
enum File {
    /// Version 1's entries by name: a position, or `None` for a bad entry.
    Windows(Vec<(String, Option<Vec2>)>),
    /// A version this build does not read.
    Newer(u64),
}

/// The file, or why the whole of it falls back.
fn parse(bytes: &[u8]) -> Result<File, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("not UTF-8: {error}"))?;
    let json = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: serde_json::Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let object = value.as_object().ok_or("expected an object")?;
    let version = object
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .filter(|version| *version >= 1)
        .ok_or("expected a whole version of 1 or more")?;
    if version > VERSION {
        return Ok(File::Newer(version));
    }
    let entries = match object.get("windows") {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::Object(windows)) => windows
            .iter()
            .map(|(name, entry)| (name.clone(), position(entry)))
            .collect(),
        Some(_) => return Err("expected 'windows' to be an object".into()),
    };
    Ok(File::Windows(entries))
}

/// An entry as a position: an array of two numbers, finite as `f32`.
fn position(entry: &serde_json::Value) -> Option<Vec2> {
    let [x, y] = entry.as_array()?.as_slice() else {
        return None;
    };
    let position = Vec2::new(x.as_f64()? as f32, y.as_f64()? as f32);
    position.is_finite().then_some(position)
}

#[cfg(test)]
#[path = "../tests/ui/window.rs"]
mod tests;
