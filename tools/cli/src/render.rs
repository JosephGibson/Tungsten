//! `tungsten render` (redo 7.2): a script written from the flags (a sheet, a
//! scene or a point of the game's world, the actions before it, the camera,
//! overlays, the UI and one capture or a loop) and run as `tungsten run`
//! runs one, so a review sheet, a loop or an area still is one command.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::{Value, json};
use tungsten_core::script::OverlayStep;

use crate::args::RunArgs;
use crate::run;

/// What the shot shows.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Subject {
    /// Items in a captioned grid on an empty stage.
    Sheet(Vec<String>),
    /// A scene file on an empty stage, the camera fitted to it.
    Scene(PathBuf),
    /// A point of the game's own world, the camera's centre.
    At([f32; 2]),
}

/// How much of the world an `--at` shot shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum View {
    /// Screen pixels per world pixel.
    Zoom(f32),
    /// World pixels from the view's top to its bottom.
    Height(f32),
}

/// One capture, or a loop of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shot {
    Frame(u32),
    Loop { from: u32, to: u32, every: u32 },
}

/// `render`'s flags, checked.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RenderArgs {
    pub(crate) subject: Subject,
    /// Actions run at frames 2, 3 and on, with their JSON arguments.
    pub(crate) after: Vec<(String, Option<Value>)>,
    pub(crate) view: Option<View>,
    pub(crate) size: [u32; 2],
    /// The overlays turned on; none when neither is given.
    pub(crate) overlay: OverlayStep,
    pub(crate) ui: bool,
    pub(crate) shot: Shot,
    pub(crate) save_script: Option<PathBuf>,
    pub(crate) release: bool,
    pub(crate) out: PathBuf,
}

/// The window's and the PNG's size when `--size` gives none.
pub(crate) const DEFAULT_SIZE: [u32; 2] = [1280, 720];

/// The frame captured when `--frame` gives none: one second in.
pub(crate) const DEFAULT_FRAME: u32 = 60;

/// Writes the script for `args` and runs it on the game in `dir`; `true`
/// when it passed, the PNGs written.
pub(crate) fn run(dir: &Path, args: &RenderArgs) -> anyhow::Result<bool> {
    let text = serde_json::to_string_pretty(&script(dir, args))? + "\n";
    let (path, temp) = match &args.save_script {
        Some(path) => (dir.join(path), false),
        None => (
            std::env::temp_dir().join(format!("tungsten-render-{}.json", std::process::id())),
            true,
        ),
    };
    std::fs::write(&path, text)
        .with_context(|| format!("cannot write the script '{}'", path.display()))?;
    let passed = run::run(
        dir,
        &RunArgs {
            script: path.clone(),
            report: None,
            user_dir: None,
            release: args.release,
        },
    );
    if temp {
        let _ = std::fs::remove_file(&path);
    }
    let passed = passed?;
    if passed {
        println!("wrote {}", dir.join(&args.out).display());
    }
    Ok(passed)
}

/// The script `args` asks for, its paths made absolute against `dir`: the
/// subject, overlays and `ui` at frame 1, each `--after` action a frame
/// from 2, then the capture or the loop.
pub(crate) fn script(dir: &Path, args: &RenderArgs) -> Value {
    let mut steps = Vec::new();
    match &args.subject {
        Subject::Sheet(items) => steps.push(json!({ "frame": 1, "sheet": { "items": items } })),
        Subject::Scene(path) => {
            steps.push(json!({ "frame": 1, "stage": {} }));
            steps.push(json!({ "frame": 1, "scene": dir.join(path) }));
            steps.push(json!({ "frame": 1, "camera": { "fit": "staged" } }));
        }
        Subject::At(center) => {
            let camera = match args.view {
                Some(View::Height(height)) => json!({ "center": center, "view_height": height }),
                Some(View::Zoom(zoom)) => json!({ "center": center, "zoom": zoom }),
                None => json!({ "center": center, "zoom": 1.0 }),
            };
            steps.push(json!({ "frame": 1, "camera": camera }));
        }
    }
    if args.overlay != OverlayStep::default() {
        steps.push(json!({ "frame": 1, "overlay": args.overlay }));
    }
    if !args.ui {
        steps.push(json!({ "frame": 1, "ui": false }));
    }
    for (frame, (action, value)) in (2..).zip(&args.after) {
        let mut step = json!({ "frame": frame, "action": action });
        if let Some(value) = value {
            step["args"] = value.clone();
        }
        steps.push(step);
    }
    let out = dir.join(&args.out);
    let last = match args.shot {
        Shot::Frame(frame) => {
            steps.push(json!({ "frame": frame, "capture": out }));
            frame
        }
        Shot::Loop { from, to, every } => {
            steps.push(json!({ "frame": from, "until": to, "every": every, "capture": out }));
            to
        }
    };
    let frames = last.max(args.after.len() as u32 + 1);
    json!({ "version": 1, "frames": frames, "resolution": args.size, "steps": steps })
}

/// `render`'s options, by name, made [`RenderArgs`]; the error says which
/// flag is wrong and why.
pub(crate) fn args(
    value: impl Fn(&str) -> Option<String>,
    values: impl Fn(&str) -> Vec<String>,
    flag: impl Fn(&str) -> bool,
) -> Result<RenderArgs, String> {
    let subjects: Vec<Subject> = [
        value("--sheet").map(|items| sheet(&items)).transpose()?,
        value("--scene").map(|path| Subject::Scene(PathBuf::from(path))),
        value("--at")
            .map(|at| pair(&at, ',', "--at").map(Subject::At))
            .transpose()?,
    ]
    .into_iter()
    .flatten()
    .collect();
    let subject = match <[Subject; 1]>::try_from(subjects) {
        Ok([subject]) => subject,
        Err(given) if given.is_empty() => {
            return Err("`render` needs one of --sheet, --scene and --at".to_owned());
        }
        Err(_) => return Err("give one of --sheet, --scene and --at, not several".to_owned()),
    };
    let view = match (value("--zoom"), value("--view")) {
        (Some(_), Some(_)) => return Err("give --zoom or --view, not both".to_owned()),
        (Some(zoom), None) => Some(View::Zoom(positive(&zoom, "--zoom")?)),
        (None, Some(height)) => Some(View::Height(positive(&height, "--view")?)),
        (None, None) => None,
    };
    if view.is_some() && !matches!(subject, Subject::At(_)) {
        return Err("--zoom and --view go with --at; a sheet or scene is fitted".to_owned());
    }
    let size = match value("--size") {
        Some(size) => {
            let [w, h] = pair(&size, 'x', "--size")?;
            let side =
                |v: f32| (v.fract() == 0.0 && (1.0..=16384.0).contains(&v)).then_some(v as u32);
            match (side(w), side(h)) {
                (Some(w), Some(h)) => [w, h],
                _ => return Err(format!("`--size {size}`: each side is a whole 1 to 16384")),
            }
        }
        None => DEFAULT_SIZE,
    };
    let mut overlay = OverlayStep::default();
    if let Some(names) = value("--overlay") {
        for name in names.split(',') {
            match name {
                "physics" => overlay.physics = Some(true),
                "kit" => overlay.kit = Some(true),
                other => {
                    return Err(format!("`--overlay`: `{other}` is not physics or kit"));
                }
            }
        }
    }
    let shot = match (value("--frame"), value("--loop")) {
        (Some(_), Some(_)) => return Err("give --frame or --loop, not both".to_owned()),
        (Some(frame), None) => Shot::Frame(whole(&frame, "--frame")?),
        (None, Some(spec)) => looped(&spec)?,
        (None, None) => Shot::Frame(DEFAULT_FRAME),
    };
    let out = PathBuf::from(value("-o").ok_or("`render` needs -o <path>")?);
    let png = out
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
    match (shot, png) {
        (Shot::Frame(_), false) => return Err("-o names the .png file to write".to_owned()),
        (Shot::Loop { .. }, true) => {
            return Err("with --loop, -o names the folder of its frames, not a .png".to_owned());
        }
        _ => {}
    }
    let after = values("--after")
        .into_iter()
        .map(|spec| match spec.split_once('=') {
            None => Ok((spec, None)),
            Some((action, text)) => serde_json::from_str(text)
                .map(|value| (action.to_owned(), Some(value)))
                .map_err(|err| format!("`--after {spec}`: its arguments are not JSON: {err}")),
        })
        .collect::<Result<_, _>>()?;
    Ok(RenderArgs {
        subject,
        after,
        view,
        size,
        overlay,
        ui: !flag("--no-ui"),
        shot,
        save_script: value("--save-script").map(PathBuf::from),
        release: flag("--release"),
        out,
    })
}

fn sheet(items: &str) -> Result<Subject, String> {
    let items: Vec<String> = items.split(',').map(str::to_owned).collect();
    if items.iter().any(String::is_empty) {
        return Err(format!("`--sheet {}`: an item is empty", items.join(",")));
    }
    Ok(Subject::Sheet(items))
}

/// `<a><sep><b>` as two finite numbers.
fn pair(text: &str, sep: char, flag: &str) -> Result<[f32; 2], String> {
    let numbers = text.split_once(sep).and_then(|(a, b)| {
        let (a, b) = (a.trim().parse::<f32>().ok()?, b.trim().parse::<f32>().ok()?);
        (a.is_finite() && b.is_finite()).then_some([a, b])
    });
    numbers.ok_or_else(|| format!("`{flag} {text}`: expected two numbers split by `{sep}`"))
}

fn positive(text: &str, flag: &str) -> Result<f32, String> {
    match text.parse::<f32>() {
        Ok(v) if v.is_finite() && v > 0.0 => Ok(v),
        _ => Err(format!("`{flag} {text}`: expected a number above 0")),
    }
}

fn whole(text: &str, flag: &str) -> Result<u32, String> {
    match text.parse::<u32>() {
        Ok(v) if v >= 1 => Ok(v),
        _ => Err(format!("`{flag} {text}`: expected a whole number from 1")),
    }
}

/// `<from>..<to>/<every>`.
fn looped(spec: &str) -> Result<Shot, String> {
    let bad =
        || format!("`--loop {spec}`: expected <from>..<to>/<every>, from 1, to at or after from");
    let (range, every) = spec.split_once('/').ok_or_else(bad)?;
    let (from, to) = range.split_once("..").ok_or_else(bad)?;
    let [from, to, every] = [from, to, every].map(|n| n.parse::<u32>().ok());
    match (from, to, every) {
        (Some(from), Some(to), Some(every)) if from >= 1 && to >= from && every >= 1 => {
            Ok(Shot::Loop { from, to, every })
        }
        _ => Err(bad()),
    }
}

#[cfg(test)]
#[path = "tests/render.rs"]
mod tests;
