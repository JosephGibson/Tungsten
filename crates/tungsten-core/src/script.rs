//! Scripted runs (redo 7.2): the script a run follows, the report it writes,
//! and the hooks a game adds for scripts to call.
//!
//! A script is JSON at version [`SCRIPT_VERSION`]: `version`, `frames` (the
//! last frame; the run ends after it), an optional `resolution` (`[w, h]`,
//! the window's and the captures' size) and `steps`. A step has `frame`
//! (counting frame bodies from 1, skipped renders included), optional
//! `until` and `every` (default 1) that repeat it, and one verb:
//!
//! | Verb | Value |
//! | --- | --- |
//! | `press`, `release` | an action name |
//! | `cursor` | `[x, y]` in window pixels, or `{"world": [x, y]}` |
//! | `action` | a name registered in [`ScriptHooks`], with `args` |
//! | `spawn` | `{"prefab": id, "at": [x, y]}` |
//! | `scene` | a scene file, relative to the game's folder |
//! | `stage` | `{}`, or `{"ambient": [r, g, b]}` |
//! | `sheet` | `{"items": ["prefab:goblin", "sprite:icon"], "columns"?, "cell"?, "labels"?}` |
//! | `camera` | `{"center": [x, y], "zoom": z}`, `{"center", "view_height": h}`, `{"fit": "staged"}` or `null` |
//! | `overlay` | `{"physics": bool, "kit": bool}` |
//! | `ui` | a bool |
//! | `capture` | a `.png` path; with `until`, a folder of `NNNN.png` named by frame |
//! | `probe` | a name registered in [`ScriptHooks`], with `args` and `path` |
//! | `assert` | `{"probe", "args"?, "path"?, "op", "value", "tolerance"?}` |
//! | `exit` | `{}` |
//!
//! Unknown keys are refused and [`Script::parse`] names the field that
//! failed (`steps[2].captur`), so a tool rejects a bad script before it
//! builds anything. A `path` is a JSON pointer (RFC 6901) into a probe's
//! value. The umbrella crate runs scripts; core holds the formats, the
//! hooks and the engine's core probes (`frame`, `entities`, `count`,
//! `camera`).
//!
//! ```
//! use tungsten_core::script::{Op, Script, Verb};
//!
//! let script = Script::parse(
//!     r#"{
//!         "version": 1,
//!         "frames": 60,
//!         "steps": [
//!             { "frame": 2, "press": "jump" },
//!             { "frame": 10, "until": 30, "every": 10, "capture": "shots" },
//!             { "frame": 60, "assert": { "probe": "entities", "op": ">", "value": 0 } }
//!         ]
//!     }"#,
//! )
//! .unwrap();
//! let frames: Vec<u32> = script.schedule().iter().map(|at| at.frame).collect();
//! assert_eq!(frames, [2, 10, 20, 30, 60]);
//! assert!(matches!(&script.steps[2].verb, Verb::Assert(a) if a.op == Op::Gt));
//!
//! let err = Script::parse(r#"{ "version": 1, "frames": 5, "steps": [{ "frame": 1, "captur": "a.png" }] }"#)
//!     .unwrap_err();
//! assert_eq!(err.field, "steps[0].captur");
//! ```

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::camera::CameraState;
use crate::component_registry::ComponentRegistry;
use crate::ecs::World;

/// The script format's version; a field added or changed bumps it.
pub const SCRIPT_VERSION: u32 = 1;

/// The report format's version; a field added or changed bumps it.
pub const REPORT_VERSION: u32 = 1;

/// `~=`'s tolerance when an assert gives none.
pub const DEFAULT_TOLERANCE: f64 = 1e-6;

/// The largest `resolution` side a script may ask for.
const MAX_RESOLUTION: u32 = 16_384;

/// A run's script: what to do on which frame, and when to stop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    /// [`SCRIPT_VERSION`].
    pub version: u32,
    /// The last frame; the run ends after it.
    pub frames: u32,
    /// The window's and the captures' size, `[w, h]` in pixels; the game's
    /// own when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<[u32; 2]>,
    /// The steps, in file order.
    #[serde(default)]
    pub steps: Vec<Step>,
}

impl Script {
    /// Parses and checks a script. An error names the field that failed as
    /// `serde_path_to_error` gives it (`steps[2].captur`); a version other
    /// than [`SCRIPT_VERSION`] fails before any other field is read.
    pub fn parse(text: &str) -> Result<Self, ScriptError> {
        let value: Value = serde_json::from_str(text)
            .map_err(|err| ScriptError::new("", format!("not JSON: {err}")))?;
        match value.get("version") {
            None => {
                return Err(ScriptError::new(
                    "version",
                    format!("missing; this engine reads version {SCRIPT_VERSION}"),
                ));
            }
            Some(Value::Number(n)) if n.as_u64() == Some(u64::from(SCRIPT_VERSION)) => {}
            Some(other) => {
                return Err(ScriptError::new(
                    "version",
                    format!(
                        "version {other} is not supported; this engine reads version {SCRIPT_VERSION}"
                    ),
                ));
            }
        }
        let script: Script = serde_path_to_error::deserialize(value).map_err(|err| {
            let field = err.path().to_string();
            let field = if field == "." { String::new() } else { field };
            ScriptError::new(field, err.into_inner().to_string())
        })?;
        script.check()?;
        Ok(script)
    }

    /// The checks that span fields: frames in range, a sane resolution.
    fn check(&self) -> Result<(), ScriptError> {
        if self.frames == 0 {
            return Err(ScriptError::new("frames", "must be at least 1"));
        }
        if let Some([w, h]) = self.resolution
            && (w == 0 || h == 0 || w > MAX_RESOLUTION || h > MAX_RESOLUTION)
        {
            return Err(ScriptError::new(
                "resolution",
                format!("each side must be 1 to {MAX_RESOLUTION}, got [{w}, {h}]"),
            ));
        }
        for (index, step) in self.steps.iter().enumerate() {
            let last = step.until.unwrap_or(step.frame);
            if last > self.frames {
                let field = if step.until.is_some() {
                    "until"
                } else {
                    "frame"
                };
                return Err(ScriptError::new(
                    format!("steps[{index}].{field}"),
                    format!("frame {last} is after the last frame, {}", self.frames),
                ));
            }
        }
        Ok(())
    }

    /// Every occurrence of every step, in frame order, the steps of one
    /// frame in file order: what a runner walks.
    #[must_use]
    pub fn schedule(&self) -> Vec<Occurrence> {
        let mut out: Vec<Occurrence> = self
            .steps
            .iter()
            .enumerate()
            .flat_map(|(step, s)| s.frames().map(move |frame| Occurrence { frame, step }))
            .collect();
        out.sort_by_key(|at| (at.frame, at.step));
        out
    }
}

/// One occurrence of a step: the frame it applies on and its index in
/// [`Script::steps`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Occurrence {
    /// The frame, from 1.
    pub frame: u32,
    /// The step's index in the script.
    pub step: usize,
}

/// A script that does not parse or check: the field that failed and why.
/// Prints "`<field>: <message>`", or the message alone when `field` is
/// empty.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{}{message}", field_prefix(.field))]
#[non_exhaustive]
pub struct ScriptError {
    /// The path to the field, as `serde_path_to_error` prints it
    /// (`steps[2].captur`), empty at the top.
    pub field: String,
    /// What is wrong.
    pub message: String,
}

impl ScriptError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

fn field_prefix(field: &str) -> String {
    if field.is_empty() {
        String::new()
    } else {
        format!("{field}: ")
    }
}

/// One step: a verb on a frame, repeated every `every` frames to `until`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawStep", into = "RawStep")]
pub struct Step {
    /// The first frame it applies on, from 1.
    pub frame: u32,
    /// The last frame it may repeat on.
    pub until: Option<u32>,
    /// The repeat's stride, at least 1.
    pub every: u32,
    /// What it does.
    pub verb: Verb,
}

impl Step {
    /// The frames this step applies on.
    pub fn frames(&self) -> impl Iterator<Item = u32> + use<> {
        let every = self.every.max(1) as usize;
        (self.frame..=self.until.unwrap_or(self.frame)).step_by(every)
    }

    /// Where a `capture` on `frame` writes: the step's path, or, for a
    /// repeated capture, `NNNN.png` named by frame in the step's folder.
    /// `None` for any other verb.
    #[must_use]
    pub fn capture_path(&self, frame: u32) -> Option<PathBuf> {
        let Verb::Capture(path) = &self.verb else {
            return None;
        };
        Some(if self.until.is_some() {
            PathBuf::from(path).join(format!("{frame:04}.png"))
        } else {
            PathBuf::from(path)
        })
    }
}

/// What a step does. See the [module docs](self) for where each applies.
#[derive(Debug, Clone, PartialEq)]
pub enum Verb {
    /// Presses an action's first binding.
    Press(String),
    /// Releases an action's first binding.
    Release(String),
    /// Moves the cursor.
    Cursor(Cursor),
    /// Calls a registered action.
    Action(ActionStep),
    /// Spawns a prefab.
    Spawn(SpawnStep),
    /// Spawns a scene file under the stage.
    Scene(String),
    /// Clears the game's states for an empty stage, held from its frame.
    Stage(StageStep),
    /// Stages and lays out items in a captioned grid.
    Sheet(SheetStep),
    /// Draws with another camera until `None`.
    Camera(Option<CameraView>),
    /// Turns debug overlays on or off.
    Overlay(OverlayStep),
    /// Whether the UI paints.
    Ui(bool),
    /// Captures the frame's render to a `.png` path, or with `until` to a
    /// folder.
    Capture(String),
    /// Reads a registered probe into the report.
    Probe(ProbeStep),
    /// Checks a probe's value.
    Assert(Assert),
    /// Ends the run after the frame.
    Exit,
}

impl Verb {
    /// The verb's key in the file: `"press"`, `"capture"`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Verb::Press(_) => "press",
            Verb::Release(_) => "release",
            Verb::Cursor(_) => "cursor",
            Verb::Action(_) => "action",
            Verb::Spawn(_) => "spawn",
            Verb::Scene(_) => "scene",
            Verb::Stage(_) => "stage",
            Verb::Sheet(_) => "sheet",
            Verb::Camera(_) => "camera",
            Verb::Overlay(_) => "overlay",
            Verb::Ui(_) => "ui",
            Verb::Capture(_) => "capture",
            Verb::Probe(_) => "probe",
            Verb::Assert(_) => "assert",
            Verb::Exit => "exit",
        }
    }
}

/// Where `cursor` moves the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
pub enum Cursor {
    /// `[x, y]` in window pixels.
    Window([f32; 2]),
    /// `{"world": [x, y]}`, mapped through the game's camera.
    World([f32; 2]),
}

impl TryFrom<Value> for Cursor {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WorldPoint {
            world: [f32; 2],
        }
        const SHAPE: &str = "expected [x, y] or {\"world\": [x, y]}";
        if value.is_array() {
            serde_json::from_value(value)
                .map(Cursor::Window)
                .map_err(|err| format!("{SHAPE}: {err}"))
        } else {
            serde_json::from_value::<WorldPoint>(value)
                .map(|point| Cursor::World(point.world))
                .map_err(|err| format!("{SHAPE}: {err}"))
        }
    }
}

impl From<Cursor> for Value {
    fn from(cursor: Cursor) -> Value {
        match cursor {
            Cursor::Window(at) => json!(at),
            Cursor::World(at) => json!({ "world": at }),
        }
    }
}

/// `action`: a registered action and its arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionStep {
    /// The action's name in [`ScriptHooks`].
    pub name: String,
    /// Its arguments, `null` when the step gives none.
    pub args: Value,
}

/// `probe`: a registered probe, its arguments and the field to keep.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeStep {
    /// The probe's name in [`ScriptHooks`].
    pub name: String,
    /// Its arguments, `null` when the step gives none.
    pub args: Value,
    /// A JSON pointer into its value; the whole value when absent.
    pub path: Option<String>,
}

/// `spawn`: a prefab at a point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnStep {
    /// The manifest's prefab ID.
    pub prefab: String,
    /// Where, in world pixels.
    pub at: [f32; 2],
}

/// `stage`: an empty stage, with an ambient light when given.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageStep {
    /// The stage's `AmbientLight` colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<[f32; 3]>,
}

/// `sheet`: items laid out in a captioned grid on the stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetStep {
    /// What to lay out, in order.
    pub items: Vec<SheetItem>,
    /// Columns in the grid; a near-square grid when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    /// A cell's size in world pixels; from the items' sprite sizes when
    /// absent, else 64 × 64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<[f32; 2]>,
    /// Whether each cell carries its item's name.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub labels: bool,
}

fn yes() -> bool {
    true
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes a reference"
)]
fn is_true(value: &bool) -> bool {
    *value
}

/// One item of a `sheet`: `"prefab:goblin"`, `"sprite:icon_fire"`,
/// `"animation:wisp_idle"`, `"particles:spark"`; a bare ID is a prefab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SheetItem {
    /// What the ID names.
    pub kind: SheetKind,
    /// The manifest ID.
    pub id: String,
}

/// What a [`SheetItem`]'s ID names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetKind {
    /// A prefab, spawned as it is.
    Prefab,
    /// A sprite, drawn alone.
    Sprite,
    /// An animation, played on a loop.
    Animation,
    /// A particle config, emitting.
    Particles,
}

impl SheetKind {
    const ALL: [(SheetKind, &'static str); 4] = [
        (SheetKind::Prefab, "prefab"),
        (SheetKind::Sprite, "sprite"),
        (SheetKind::Animation, "animation"),
        (SheetKind::Particles, "particles"),
    ];

    /// The kind's prefix in the file: `"prefab"`.
    #[must_use]
    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(kind, _)| *kind == self)
            .map_or("prefab", |(_, name)| name)
    }
}

impl TryFrom<String> for SheetItem {
    type Error = String;

    fn try_from(item: String) -> Result<Self, String> {
        let (kind, id) = match item.split_once(':') {
            None => (SheetKind::Prefab, item.as_str()),
            Some((prefix, id)) => {
                let kind = SheetKind::ALL
                    .iter()
                    .find(|(_, name)| *name == prefix)
                    .map(|(kind, _)| *kind)
                    .ok_or_else(|| {
                        format!(
                            "sheet item `{item}`: `{prefix}` is not one of prefab, sprite, animation, particles"
                        )
                    })?;
                (kind, id)
            }
        };
        if id.is_empty() {
            return Err(format!("sheet item `{item}` names no ID"));
        }
        Ok(SheetItem {
            kind,
            id: id.to_owned(),
        })
    }
}

impl From<SheetItem> for String {
    fn from(item: SheetItem) -> String {
        format!("{}:{}", item.kind.name(), item.id)
    }
}

/// `camera`: the view a run draws with in place of the game's.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawCamera", into = "RawCamera")]
pub enum CameraView {
    /// Centred on a world point at a zoom.
    Zoom {
        /// The view's centre in world pixels.
        center: [f32; 2],
        /// Screen pixels per world pixel.
        zoom: f32,
    },
    /// Centred on a world point, showing `view_height` world pixels.
    ViewHeight {
        /// The view's centre in world pixels.
        center: [f32; 2],
        /// World pixels from the view's top to its bottom.
        view_height: f32,
    },
    /// Fits what the stage holds.
    FitStaged,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCamera {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    center: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zoom: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    view_height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fit: Option<String>,
}

impl TryFrom<RawCamera> for CameraView {
    type Error = String;

    fn try_from(raw: RawCamera) -> Result<Self, String> {
        const SHAPE: &str = "give `center` with `zoom` or `view_height`, or `fit`: \"staged\"";
        match raw {
            RawCamera {
                center: None,
                zoom: None,
                view_height: None,
                fit: Some(fit),
            } if fit == "staged" => Ok(CameraView::FitStaged),
            RawCamera {
                center: Some(center),
                zoom: Some(zoom),
                view_height: None,
                fit: None,
            } => positive("zoom", zoom).map(|zoom| CameraView::Zoom { center, zoom }),
            RawCamera {
                center: Some(center),
                zoom: None,
                view_height: Some(height),
                fit: None,
            } => positive("view_height", height).map(|view_height| CameraView::ViewHeight {
                center,
                view_height,
            }),
            _ => Err(SHAPE.to_owned()),
        }
    }
}

impl From<CameraView> for RawCamera {
    fn from(view: CameraView) -> RawCamera {
        match view {
            CameraView::Zoom { center, zoom } => RawCamera {
                center: Some(center),
                zoom: Some(zoom),
                ..RawCamera::default()
            },
            CameraView::ViewHeight {
                center,
                view_height,
            } => RawCamera {
                center: Some(center),
                view_height: Some(view_height),
                ..RawCamera::default()
            },
            CameraView::FitStaged => RawCamera {
                fit: Some("staged".to_owned()),
                ..RawCamera::default()
            },
        }
    }
}

fn positive(field: &str, value: f32) -> Result<f32, String> {
    if value > 0.0 {
        Ok(value)
    } else {
        Err(format!("`{field}` must be above 0, got {value}"))
    }
}

/// `overlay`: debug overlays on or off; an absent one is left as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayStep {
    /// The physics overlay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physics: Option<bool>,
    /// The kit's debug overlay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kit: Option<bool>,
}

/// An `assert` step's check of a probe's value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assert {
    /// The probe's name in [`ScriptHooks`].
    pub probe: String,
    /// The probe's arguments, `null` when absent.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub args: Value,
    /// A JSON pointer into the probe's value; the whole value when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// How the value compares.
    pub op: Op,
    /// What it compares against.
    pub value: Value,
    /// `~=`'s tolerance, [`DEFAULT_TOLERANCE`] when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f64>,
}

/// An assert's comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    /// `==`: equal; numbers compare by value, arrays and objects by member.
    #[serde(rename = "==")]
    Eq,
    /// `!=`: not `==`.
    #[serde(rename = "!=")]
    Ne,
    /// `<`: two numbers, or two strings by bytes.
    #[serde(rename = "<")]
    Lt,
    /// `<=`.
    #[serde(rename = "<=")]
    Le,
    /// `>`.
    #[serde(rename = ">")]
    Gt,
    /// `>=`.
    #[serde(rename = ">=")]
    Ge,
    /// `~=`: two numbers, or two arrays of numbers of one length, within
    /// the tolerance.
    #[serde(rename = "~=")]
    Near,
}

impl Op {
    /// The op as the file writes it: `"=="`.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Op::Eq => "==",
            Op::Ne => "!=",
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Near => "~=",
        }
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

impl Assert {
    /// Whether `probed`, the probe's whole value, holds: the `path`'s field
    /// selected, then compared by `op`. `Err` when the path finds nothing
    /// or the op cannot compare the two values.
    pub fn check(&self, probed: &Value) -> Result<bool, String> {
        let actual = select(probed, self.path.as_deref())?;
        match self.op {
            Op::Eq => Ok(values_equal(actual, &self.value)),
            Op::Ne => Ok(!values_equal(actual, &self.value)),
            Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                let ordering = order(actual, &self.value).ok_or_else(|| {
                    format!(
                        "`{}` compares two numbers or two strings, not {} and {}",
                        self.op,
                        kind_of(actual),
                        kind_of(&self.value)
                    )
                })?;
                Ok(match self.op {
                    Op::Lt => ordering == Ordering::Less,
                    Op::Le => ordering != Ordering::Greater,
                    Op::Gt => ordering == Ordering::Greater,
                    _ => ordering != Ordering::Less,
                })
            }
            Op::Near => near(
                actual,
                &self.value,
                self.tolerance.unwrap_or(DEFAULT_TOLERANCE),
            ),
        }
    }

    /// The report's entry for this assert on `frame`: `probed` is the
    /// probe's value, or the error that kept it from reading.
    #[must_use]
    pub fn evaluate(&self, frame: u32, probed: Result<&Value, String>) -> AssertReport {
        let mut report = AssertReport {
            frame,
            probe: self.probe.clone(),
            path: self.path.clone(),
            op: self.op,
            value: self.value.clone(),
            actual: Value::Null,
            passed: false,
            error: None,
        };
        match probed {
            Err(err) => report.error = Some(err),
            Ok(probed) => {
                if let Ok(actual) = select(probed, self.path.as_deref()) {
                    report.actual = actual.clone();
                }
                match self.check(probed) {
                    Ok(holds) => report.passed = holds,
                    Err(err) => report.error = Some(err),
                }
            }
        }
        report
    }
}

/// The field of `value` a JSON pointer names, or the whole value for
/// `None`. `Err` names the pointer when it finds nothing.
pub fn select<'v>(value: &'v Value, path: Option<&str>) -> Result<&'v Value, String> {
    match path {
        None => Ok(value),
        Some(path) => value
            .pointer(path)
            .ok_or_else(|| format!("path `{path}` finds nothing in {value}")),
    }
}

/// `==` on JSON values, numbers compared by value.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| values_equal(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(key, x)| y.get(key).is_some_and(|y| values_equal(x, y)))
        }
        _ => a == b,
    }
}

fn order(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64()?.partial_cmp(&y.as_f64()?),
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

fn near(a: &Value, b: &Value, tolerance: f64) -> Result<bool, String> {
    let shape = || {
        format!(
            "`~=` compares two numbers or two arrays of numbers of one length, not {} and {}",
            kind_of(a),
            kind_of(b)
        )
    };
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            Ok((x.as_f64().ok_or_else(shape)? - y.as_f64().ok_or_else(shape)?).abs() <= tolerance)
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            let mut holds = true;
            for (x, y) in x.iter().zip(y) {
                let (Some(x), Some(y)) = (x.as_f64(), y.as_f64()) else {
                    return Err(shape());
                };
                holds &= (x - y).abs() <= tolerance;
            }
            Ok(holds)
        }
        _ => Err(shape()),
    }
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a bool",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The step as the file writes it: every verb a key, one present.
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStep {
    frame: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    until: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    every: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    press: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    release: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    spawn: Option<SpawnStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scene: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stage: Option<StageStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sheet: Option<SheetStep>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[expect(clippy::option_option, reason = "an absent key differs from `null`")]
    camera: Option<Option<CameraView>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    overlay: Option<OverlayStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ui: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    probe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assert: Option<Assert>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exit: Option<ExitStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    args: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

/// `exit`'s value: `{}`.
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExitStep {}

/// A present key, `null` included: `Some(None)` for `"camera": null`.
#[expect(clippy::option_option, reason = "an absent key differs from `null`")]
fn present<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

impl TryFrom<RawStep> for Step {
    type Error = String;

    fn try_from(raw: RawStep) -> Result<Self, String> {
        if raw.frame == 0 {
            return Err("`frame` counts from 1".to_owned());
        }
        if let Some(until) = raw.until
            && until < raw.frame
        {
            return Err(format!("`until` {until} is before `frame` {}", raw.frame));
        }
        if raw.every.is_some() && raw.until.is_none() {
            return Err("`every` needs `until`".to_owned());
        }
        let every = raw.every.unwrap_or(1);
        if every == 0 {
            return Err("`every` must be at least 1".to_owned());
        }
        let mut verbs: Vec<(&str, Verb)> = Vec::new();
        if let Some(name) = raw.press {
            verbs.push(("press", Verb::Press(name)));
        }
        if let Some(name) = raw.release {
            verbs.push(("release", Verb::Release(name)));
        }
        if let Some(cursor) = raw.cursor {
            verbs.push(("cursor", Verb::Cursor(cursor)));
        }
        if let Some(name) = raw.action {
            let args = raw.args.clone().unwrap_or(Value::Null);
            verbs.push(("action", Verb::Action(ActionStep { name, args })));
        }
        if let Some(spawn) = raw.spawn {
            verbs.push(("spawn", Verb::Spawn(spawn)));
        }
        if let Some(scene) = raw.scene {
            verbs.push(("scene", Verb::Scene(scene)));
        }
        if let Some(stage) = raw.stage {
            verbs.push(("stage", Verb::Stage(stage)));
        }
        if let Some(sheet) = raw.sheet {
            verbs.push(("sheet", Verb::Sheet(sheet)));
        }
        if let Some(camera) = raw.camera {
            verbs.push(("camera", Verb::Camera(camera)));
        }
        if let Some(overlay) = raw.overlay {
            verbs.push(("overlay", Verb::Overlay(overlay)));
        }
        if let Some(ui) = raw.ui {
            verbs.push(("ui", Verb::Ui(ui)));
        }
        if let Some(capture) = raw.capture {
            verbs.push(("capture", Verb::Capture(capture)));
        }
        if let Some(name) = raw.probe {
            let args = raw.args.clone().unwrap_or(Value::Null);
            let path = raw.path.clone();
            verbs.push(("probe", Verb::Probe(ProbeStep { name, args, path })));
        }
        if let Some(assert) = raw.assert {
            verbs.push(("assert", Verb::Assert(assert)));
        }
        if raw.exit.is_some() {
            verbs.push(("exit", Verb::Exit));
        }
        let verb = match verbs.len() {
            0 => return Err("a step needs a verb".to_owned()),
            1 => verbs.pop().map_or(Verb::Exit, |(_, verb)| verb),
            _ => {
                let names: Vec<String> =
                    verbs.iter().map(|(name, _)| format!("`{name}`")).collect();
                return Err(format!("a step takes one verb, not {}", names.join(", ")));
            }
        };
        if raw.args.is_some() && !matches!(verb, Verb::Action(_) | Verb::Probe(_)) {
            return Err(format!(
                "`args` goes with `action` or `probe`, not `{}`",
                verb.name()
            ));
        }
        if raw.path.is_some() && !matches!(verb, Verb::Probe(_)) {
            return Err(format!("`path` goes with `probe`, not `{}`", verb.name()));
        }
        if raw.until.is_some() && matches!(verb, Verb::Exit | Verb::Stage(_) | Verb::Sheet(_)) {
            return Err(format!("`{}` does not repeat; drop `until`", verb.name()));
        }
        check_verb(&verb, raw.until.is_some())?;
        Ok(Step {
            frame: raw.frame,
            until: raw.until,
            every,
            verb,
        })
    }
}

/// The checks of one verb's value beyond its shape.
fn check_verb(verb: &Verb, repeats: bool) -> Result<(), String> {
    match verb {
        Verb::Press(name) | Verb::Release(name) | Verb::Scene(name) => non_empty(name),
        Verb::Action(ActionStep { name, .. }) => hook_name(name),
        Verb::Probe(ProbeStep { name, path, .. }) => {
            hook_name(name)?;
            pointer(path.as_deref())
        }
        Verb::Assert(assert) => {
            hook_name(&assert.probe)?;
            pointer(assert.path.as_deref())?;
            match assert.tolerance {
                Some(_) if assert.op != Op::Near => Err("`tolerance` goes with `~=`".to_owned()),
                Some(t) if t.is_nan() || t < 0.0 => {
                    Err(format!("`tolerance` must be 0 or above, got {t}"))
                }
                _ => Ok(()),
            }
        }
        Verb::Spawn(spawn) => non_empty(&spawn.prefab),
        Verb::Sheet(sheet) => {
            if sheet.items.is_empty() {
                return Err("`sheet` needs at least one item".to_owned());
            }
            if sheet.columns == Some(0) {
                return Err("`columns` must be at least 1".to_owned());
            }
            if let Some([w, h]) = sheet.cell {
                positive("cell", w.min(h))?;
            }
            Ok(())
        }
        Verb::Capture(path) => {
            let png = path.to_ascii_lowercase().ends_with(".png");
            match (repeats, png) {
                (false, false) => Err(format!("`capture` writes a .png file, not `{path}`")),
                (true, true) => Err(format!(
                    "a repeated `capture` names a folder for its NNNN.png frames, not `{path}`"
                )),
                _ => Ok(()),
            }
        }
        Verb::Overlay(overlay) if overlay.physics.is_none() && overlay.kit.is_none() => {
            Err("`overlay` names `physics`, `kit` or both".to_owned())
        }
        _ => Ok(()),
    }
}

fn non_empty(value: &str) -> Result<(), String> {
    if value.is_empty() {
        Err("must not be empty".to_owned())
    } else {
        Ok(())
    }
}

fn hook_name(name: &str) -> Result<(), String> {
    if valid_name(name) {
        Ok(())
    } else {
        Err(format!("`{name}` must match [a-z][a-z0-9_]*"))
    }
}

fn pointer(path: Option<&str>) -> Result<(), String> {
    match path {
        Some(path) if !path.is_empty() && !path.starts_with('/') => Err(format!(
            "`path` is a JSON pointer, `/position` or `/0`, not `{path}`"
        )),
        _ => Ok(()),
    }
}

/// `[a-z][a-z0-9_]*`.
fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl From<Step> for RawStep {
    fn from(step: Step) -> RawStep {
        let mut raw = RawStep {
            frame: step.frame,
            until: step.until,
            every: (step.every != 1).then_some(step.every),
            ..RawStep::default()
        };
        match step.verb {
            Verb::Press(name) => raw.press = Some(name),
            Verb::Release(name) => raw.release = Some(name),
            Verb::Cursor(cursor) => raw.cursor = Some(cursor),
            Verb::Action(ActionStep { name, args }) => {
                raw.action = Some(name);
                raw.args = (!args.is_null()).then_some(args);
            }
            Verb::Spawn(spawn) => raw.spawn = Some(spawn),
            Verb::Scene(scene) => raw.scene = Some(scene),
            Verb::Stage(stage) => raw.stage = Some(stage),
            Verb::Sheet(sheet) => raw.sheet = Some(sheet),
            Verb::Camera(camera) => raw.camera = Some(camera),
            Verb::Overlay(overlay) => raw.overlay = Some(overlay),
            Verb::Ui(ui) => raw.ui = Some(ui),
            Verb::Capture(path) => raw.capture = Some(path),
            Verb::Probe(ProbeStep { name, args, path }) => {
                raw.probe = Some(name);
                raw.args = (!args.is_null()).then_some(args);
                raw.path = path;
            }
            Verb::Assert(assert) => raw.assert = Some(assert),
            Verb::Exit => raw.exit = Some(ExitStep {}),
        }
        raw
    }
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ending {
    /// After the script's last frame.
    Frames,
    /// On an `exit` step.
    Exit,
    /// On the game's own exit.
    GameExit,
    /// On a fatal error: a failed frame or setup.
    Failed,
}

/// What a run did, written on every ending at [`REPORT_VERSION`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// [`REPORT_VERSION`].
    pub version: u32,
    /// The script's path as the run was given it.
    pub script: String,
    /// Frames run.
    pub frames: u32,
    /// How the run ended.
    pub ended: Ending,
    /// The fatal error, for a `failed` ending.
    pub error: Option<String>,
    /// Whether the run passed: [`Report::finish`]'s verdict.
    pub passed: bool,
    /// Each capture, written or not.
    pub captures: Vec<CaptureReport>,
    /// Each probe's value.
    pub probes: Vec<ProbeReport>,
    /// Each assert.
    pub asserts: Vec<AssertReport>,
    /// The steps that failed.
    pub failed_steps: Vec<FailedStep>,
    /// The occurrences the run never settled.
    pub unreached: Vec<Unreached>,
    /// The run's log lines at warning and error level.
    pub log: LogReport,
}

impl Report {
    /// An empty report of the script at `script`, ended `failed` until
    /// [`finish`](Self::finish) says otherwise.
    #[must_use]
    pub fn new(script: impl Into<String>) -> Self {
        Self {
            version: REPORT_VERSION,
            script: script.into(),
            frames: 0,
            ended: Ending::Failed,
            error: None,
            passed: false,
            captures: Vec::new(),
            probes: Vec::new(),
            asserts: Vec::new(),
            failed_steps: Vec::new(),
            unreached: Vec::new(),
            log: LogReport::default(),
        }
    }

    /// Records the ending and the fatal error, and sets
    /// [`passed`](Self::passed): the run ended in `frames`, `exit` or
    /// `game_exit`, every assert and capture passed, no step failed and
    /// nothing went unreached. Log lines only count.
    pub fn finish(&mut self, ended: Ending, error: Option<String>) {
        self.ended = ended;
        self.error = error;
        self.passed = ended != Ending::Failed
            && self.error.is_none()
            && self.asserts.iter().all(|a| a.passed)
            && self.captures.iter().all(|c| c.written)
            && self.failed_steps.is_empty()
            && self.unreached.is_empty();
    }
}

/// One capture in a [`Report`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureReport {
    /// The frame it was armed on.
    pub frame: u32,
    /// Where it was to be written.
    pub path: String,
    /// Whether the PNG was written.
    pub written: bool,
    /// Why not, when it was not.
    pub error: Option<String>,
}

/// One probe's value in a [`Report`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeReport {
    /// The frame it read after.
    pub frame: u32,
    /// The probe's name.
    pub name: String,
    /// The pointer it kept, when the step gave one.
    pub path: Option<String>,
    /// The value, or the pointer's field of it.
    pub value: Value,
}

/// One assert in a [`Report`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssertReport {
    /// The frame it checked after.
    pub frame: u32,
    /// The probe it read.
    pub probe: String,
    /// The pointer into the probe's value.
    pub path: Option<String>,
    /// The comparison.
    pub op: Op,
    /// The expected value.
    pub value: Value,
    /// The value the probe gave at `path`, `null` when it gave none.
    pub actual: Value,
    /// Whether it held.
    pub passed: bool,
    /// Why it could not compare: an unknown probe, a probe's error, a path
    /// that finds nothing, values the op cannot compare.
    pub error: Option<String>,
}

/// A step that failed in a [`Report`]: an action's error, an unknown
/// action, probe or prefab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedStep {
    /// The frame.
    pub frame: u32,
    /// The step's index in the script.
    pub index: usize,
    /// The step's verb, as the file writes it.
    pub verb: String,
    /// What went wrong.
    pub error: String,
}

/// An occurrence the run never settled in a [`Report`]: a step after the
/// run's end, or one after render on a frame the game exited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unreached {
    /// The frame.
    pub frame: u32,
    /// The step's index in the script.
    pub index: usize,
    /// The step's verb, as the file writes it.
    pub verb: String,
}

/// A run's log lines at warning and error level in a [`Report`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LogReport {
    /// Error lines.
    pub errors: u32,
    /// Warning lines.
    pub warnings: u32,
    /// The first five error lines.
    pub first_errors: Vec<String>,
}

/// What a running script shows the World: its frame and the camera it
/// draws with. The runner keeps it; the `frame` and `camera` probes read it.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScriptRun {
    /// The frame, from 1.
    pub frame: u32,
    /// The `camera` step's view in force, as the frame drew it; `None`
    /// while the game's camera draws.
    pub drawn_camera: Option<CameraState>,
}

/// A script action: changes the World before the frame's `PreUpdate`. The
/// `Err` fails the step.
pub type ScriptAction = dyn Fn(&mut World, &Value) -> Result<(), String>;

/// A script probe: reads the World after the frame. The `Err` fails the
/// step.
pub type ScriptProbe = dyn Fn(&World, &Value) -> Result<Value, String>;

struct Hook<F: ?Sized> {
    doc: &'static str,
    run: Rc<F>,
}

/// The actions and probes scripts call by name, each with a one-line doc:
/// a `World` resource the engine inserts with its own probes, and that a
/// game or plugin adds to in `build`.
///
/// ```
/// use serde_json::{Value, json};
/// use tungsten_core::World;
/// use tungsten_core::script::ScriptHooks;
///
/// struct Score(u32);
///
/// let mut world = World::new();
/// world.insert_resource(Score(0));
/// world.add_script_action("add_score", "Adds `{\"points\": n}` to the score.", |world, args| {
///     let points = args["points"].as_u64().ok_or("`points` must be a number")?;
///     world.resource_mut::<Score>().0 += u32::try_from(points).map_err(|e| e.to_string())?;
///     Ok(())
/// });
/// world.add_script_probe("score", "The score.", |world, _| Ok(json!(world.resource::<Score>().0)));
///
/// ScriptHooks::call_action(&mut world, "add_score", &json!({ "points": 3 })).unwrap();
/// assert_eq!(ScriptHooks::read_probe(&world, "score", &Value::Null), Ok(json!(3)));
/// assert!(ScriptHooks::read_probe(&world, "lives", &Value::Null).is_err());
/// ```
pub struct ScriptHooks {
    actions: BTreeMap<String, Hook<ScriptAction>>,
    probes: BTreeMap<String, Hook<ScriptProbe>>,
}

impl Default for ScriptHooks {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptHooks {
    /// The core probes: `frame`, `entities`, `count` and `camera`.
    #[must_use]
    pub fn new() -> Self {
        let mut hooks = Self {
            actions: BTreeMap::new(),
            probes: BTreeMap::new(),
        };
        hooks.add_probe("frame", "The script's frame, from 1.", |world, _| {
            Ok(json!(
                world.get_resource::<ScriptRun>().map_or(0, |run| run.frame)
            ))
        });
        hooks.add_probe("entities", "How many entities are alive.", |world, _| {
            Ok(json!(world.entity_count()))
        });
        hooks.add_probe(
            "count",
            "`{\"component\": name}`: how many entities hold a registered component.",
            probe_count,
        );
        hooks.add_probe(
            "camera",
            "The game's camera `{position, zoom, rotation}`; `{\"drawn\": true}` for the view the frame drew.",
            probe_camera,
        );
        hooks
    }

    /// Registers the action `name`, documented by `doc` (one line).
    ///
    /// # Panics
    ///
    /// When `name` is not `[a-z][a-z0-9_]*` or is already an action.
    #[track_caller]
    pub fn add_action(
        &mut self,
        name: &str,
        doc: &'static str,
        run: impl Fn(&mut World, &Value) -> Result<(), String> + 'static,
    ) {
        assert!(
            valid_name(name),
            "ScriptHooks::add_action: `{name}` must match [a-z][a-z0-9_]*"
        );
        assert!(
            !self.actions.contains_key(name),
            "ScriptHooks::add_action: `{name}` is already an action"
        );
        self.actions.insert(
            name.to_owned(),
            Hook {
                doc,
                run: Rc::new(run),
            },
        );
    }

    /// Registers the probe `name`, documented by `doc` (one line).
    ///
    /// # Panics
    ///
    /// When `name` is not `[a-z][a-z0-9_]*` or is already a probe.
    #[track_caller]
    pub fn add_probe(
        &mut self,
        name: &str,
        doc: &'static str,
        run: impl Fn(&World, &Value) -> Result<Value, String> + 'static,
    ) {
        assert!(
            valid_name(name),
            "ScriptHooks::add_probe: `{name}` must match [a-z][a-z0-9_]*"
        );
        assert!(
            !self.probes.contains_key(name),
            "ScriptHooks::add_probe: `{name}` is already a probe"
        );
        self.probes.insert(
            name.to_owned(),
            Hook {
                doc,
                run: Rc::new(run),
            },
        );
    }

    /// Whether `name` is an action.
    #[must_use]
    pub fn has_action(&self, name: &str) -> bool {
        self.actions.contains_key(name)
    }

    /// Whether `name` is a probe.
    #[must_use]
    pub fn has_probe(&self, name: &str) -> bool {
        self.probes.contains_key(name)
    }

    /// The actions' names and docs, sorted by name.
    pub fn actions(&self) -> impl Iterator<Item = (&str, &'static str)> {
        self.actions
            .iter()
            .map(|(name, hook)| (name.as_str(), hook.doc))
    }

    /// The probes' names and docs, sorted by name.
    pub fn probes(&self) -> impl Iterator<Item = (&str, &'static str)> {
        self.probes
            .iter()
            .map(|(name, hook)| (name.as_str(), hook.doc))
    }

    /// Runs the action `name` of `world`'s hooks on `world`. `Err` for an
    /// unknown action or the action's own error.
    pub fn call_action(world: &mut World, name: &str, args: &Value) -> Result<(), String> {
        let run = world
            .get_resource::<ScriptHooks>()
            .and_then(|hooks| hooks.actions.get(name))
            .map(|hook| Rc::clone(&hook.run))
            .ok_or_else(|| format!("unknown action `{name}`"))?;
        run(world, args).map_err(|err| format!("action `{name}`: {err}"))
    }

    /// Reads the probe `name` of `world`'s hooks. `Err` for an unknown
    /// probe or the probe's own error.
    pub fn read_probe(world: &World, name: &str, args: &Value) -> Result<Value, String> {
        let hook = world
            .get_resource::<ScriptHooks>()
            .and_then(|hooks| hooks.probes.get(name))
            .ok_or_else(|| format!("unknown probe `{name}`"))?;
        (hook.run)(world, args).map_err(|err| format!("probe `{name}`: {err}"))
    }
}

impl fmt::Debug for ScriptHooks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScriptHooks")
            .field("actions", &self.actions.keys().collect::<Vec<_>>())
            .field("probes", &self.probes.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// A hook's arguments as `T`, serde's error as the message.
fn hook_args<T: DeserializeOwned>(args: &Value) -> Result<T, String> {
    T::deserialize(args).map_err(|err| format!("args: {err}"))
}

fn probe_count(world: &World, args: &Value) -> Result<Value, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Args {
        component: String,
    }
    let Args { component } = hook_args(args)?;
    let registry = world
        .get_resource::<ComponentRegistry>()
        .filter(|registry| registry.contains(&component))
        .ok_or_else(|| format!("unknown component `{component}`"))?;
    registry.count(&component, world).map(|n| json!(n)).ok_or_else(|| {
        format!(
            "component `{component}` is registered by a definition, so its spawned types are its build's; count it with a game probe"
        )
    })
}

fn probe_camera(world: &World, args: &Value) -> Result<Value, String> {
    #[derive(Deserialize, Default)]
    #[serde(deny_unknown_fields)]
    struct Args {
        #[serde(default)]
        drawn: bool,
    }
    let Args { drawn } = if args.is_null() {
        Args::default()
    } else {
        hook_args(args)?
    };
    let game = world
        .get_resource::<CameraState>()
        .copied()
        .ok_or("the World has no CameraState")?;
    let camera = if drawn {
        world
            .get_resource::<ScriptRun>()
            .and_then(|run| run.drawn_camera)
            .unwrap_or(game)
    } else {
        game
    };
    Ok(json!({
        "position": [camera.position.x, camera.position.y],
        "zoom": camera.zoom,
        "rotation": camera.rotation,
    }))
}

impl World {
    /// [`ScriptHooks::add_action`] on this World's hooks, which it inserts
    /// first when the World has none, so a plugin built on a bare World
    /// registers.
    ///
    /// # Panics
    ///
    /// As [`ScriptHooks::add_action`].
    #[track_caller]
    pub fn add_script_action(
        &mut self,
        name: &str,
        doc: &'static str,
        run: impl Fn(&mut World, &Value) -> Result<(), String> + 'static,
    ) {
        hooks_mut(self).add_action(name, doc, run);
    }

    /// [`ScriptHooks::add_probe`] on this World's hooks, which it inserts
    /// first when the World has none.
    ///
    /// # Panics
    ///
    /// As [`ScriptHooks::add_probe`].
    #[track_caller]
    pub fn add_script_probe(
        &mut self,
        name: &str,
        doc: &'static str,
        run: impl Fn(&World, &Value) -> Result<Value, String> + 'static,
    ) {
        hooks_mut(self).add_probe(name, doc, run);
    }
}

fn hooks_mut(world: &mut World) -> &mut ScriptHooks {
    if !world.has_resource::<ScriptHooks>() {
        world.insert_resource(ScriptHooks::new());
    }
    world.resource_mut::<ScriptHooks>()
}

#[cfg(test)]
#[path = "tests/script.rs"]
mod tests;
