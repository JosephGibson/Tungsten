//! `tungsten describe` (redo 7.2): the game run in its own build with
//! `TUNGSTEN_DESCRIBE` naming a file, which `App::run` fills with the
//! World's description in place of opening a window; printed here as text,
//! or as JSON, whole or the entries of one name.

use std::fmt::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Context;
use serde_json::{Map, Value, json};

use crate::args::DescribeArgs;
use crate::check;

/// The file the game writes its description to.
pub(crate) const DESCRIBE_ENV: &str = "TUNGSTEN_DESCRIBE";

/// The sections that list named entries, with their headings.
const SECTIONS: &[(&str, &str)] = &[
    ("components", "Components"),
    ("shapes", "Shapes"),
    ("kinds", "Data kinds"),
];

/// Describes the game in `dir`; `false` when a name matches nothing.
pub(crate) fn run(dir: &Path, args: &DescribeArgs) -> anyhow::Result<bool> {
    check::require_game(dir)?;
    let out = std::env::temp_dir().join(format!("tungsten-describe-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&out);
    // The game's own output goes to stderr, so stdout is the description.
    let mut command = Command::new("cargo");
    command.stdout(Stdio::from(std::io::stderr()));
    let status = check::run_game(dir, false, &[(DESCRIBE_ENV, &out)], &mut command)?;
    let Ok(text) = std::fs::read_to_string(&out) else {
        anyhow::bail!(
            "the game stopped ({status}) before describing itself; its output above says why"
        );
    };
    let _ = std::fs::remove_file(&out);
    let description: Value =
        serde_json::from_str(&text).context("the game's description is not JSON")?;
    let description = match &args.name {
        Some(name) => {
            let kept = named(&description, name);
            if is_empty(&kept) {
                eprintln!("tungsten describe: nothing in the game is named `{name}`");
                return Ok(false);
            }
            kept
        }
        None => description,
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&description)?);
    } else {
        print!("{}", render(&description));
    }
    Ok(true)
}

/// The entries of `description` named `name`: components, shapes, kinds,
/// script hooks and asset IDs; its errors kept whole.
pub(crate) fn named(description: &Value, name: &str) -> Value {
    let keep = |list: &Value| -> Vec<Value> {
        list.as_array()
            .into_iter()
            .flatten()
            .filter(|entry| entry["name"] == name)
            .cloned()
            .collect()
    };
    let ids = |list: &Value| -> Vec<Value> {
        list.as_array()
            .into_iter()
            .flatten()
            .filter(|id| *id == name)
            .cloned()
            .collect()
    };
    let mut assets = Map::new();
    for (section, list) in description["assets"].as_object().into_iter().flatten() {
        if section == "data" {
            let mut data = Map::new();
            for (kind, entries) in list.as_object().into_iter().flatten() {
                let kept = if kind == name {
                    entries.as_array().cloned().unwrap_or_default()
                } else {
                    ids(entries)
                };
                if !kept.is_empty() {
                    data.insert(kind.clone(), json!(kept));
                }
            }
            assets.insert(section.clone(), Value::Object(data));
        } else {
            assets.insert(section.clone(), json!(ids(list)));
        }
    }
    json!({
        "version": description["version"],
        "components": keep(&description["components"]),
        "shapes": keep(&description["shapes"]),
        "kinds": keep(&description["kinds"]),
        "script": {
            "actions": keep(&description["script"]["actions"]),
            "probes": keep(&description["script"]["probes"]),
        },
        "assets": assets,
        "errors": description["errors"],
    })
}

/// Whether `description` names nothing, its errors aside.
fn is_empty(description: &Value) -> bool {
    let none = |list: &Value| list.as_array().is_none_or(Vec::is_empty);
    SECTIONS.iter().all(|(key, _)| none(&description[key]))
        && none(&description["script"]["actions"])
        && none(&description["script"]["probes"])
        && description["assets"]
            .as_object()
            .into_iter()
            .flatten()
            .all(|(section, list)| {
                if section == "data" {
                    list.as_object().is_none_or(Map::is_empty)
                } else {
                    none(list)
                }
            })
}

/// The description as text, a heading per section that lists anything.
pub(crate) fn render(description: &Value) -> String {
    let mut out = String::new();
    for (key, heading) in SECTIONS {
        let list = description[key].as_array().cloned().unwrap_or_default();
        if list.is_empty() {
            continue;
        }
        let _ = writeln!(out, "{heading} ({}):", list.len());
        for entry in &list {
            entry_lines(&mut out, entry);
        }
    }
    for (key, heading) in [("actions", "Script actions"), ("probes", "Script probes")] {
        let list = description["script"][key]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if list.is_empty() {
            continue;
        }
        let _ = writeln!(out, "{heading} ({}):", list.len());
        for hook in &list {
            let _ = writeln!(out, "  {}: {}", text(&hook["name"]), text(&hook["doc"]));
        }
    }
    // The sections by name, then the data entries by kind.
    let mut assets = String::new();
    for (section, list) in description["assets"].as_object().into_iter().flatten() {
        if section != "data" {
            id_line(&mut assets, section, list);
        }
    }
    for (kind, entries) in description["assets"]["data"]
        .as_object()
        .into_iter()
        .flatten()
    {
        id_line(&mut assets, &format!("data {kind}"), entries);
    }
    if !assets.is_empty() {
        let _ = writeln!(out, "Assets:");
        out.push_str(&assets);
    }
    let errors = description["errors"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !errors.is_empty() {
        let _ = writeln!(out, "Errors ({}):", errors.len());
        for error in &errors {
            let _ = writeln!(
                out,
                "  {}: {}",
                text(&error["file"]),
                text(&error["message"])
            );
        }
    }
    out
}

/// A component, shape or kind: its name and summary, then a line per
/// field with its type, default and doc.
fn entry_lines(out: &mut String, entry: &Value) {
    let name = text(&entry["name"]);
    match entry["summary"].as_str() {
        Some(summary) => {
            let _ = writeln!(out, "  {name}: {summary}");
        }
        None => {
            let _ = writeln!(out, "  {name}");
        }
    }
    for field in entry["fields"].as_array().into_iter().flatten() {
        let _ = write!(
            out,
            "    {}: {}",
            text(&field["name"]),
            text(&field["type"])
        );
        if let Some(default) = field["default"].as_str() {
            let _ = write!(out, " = {default}");
        }
        let _ = writeln!(out, " \u{2014} {}", text(&field["doc"]));
    }
}

/// `  <section> (<n>): <id>, <id>…`, nothing for an empty section.
fn id_line(out: &mut String, section: &str, list: &Value) {
    let ids: Vec<&str> = list
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !ids.is_empty() {
        let _ = writeln!(out, "  {section} ({}): {}", ids.len(), ids.join(", "));
    }
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}

#[cfg(test)]
#[path = "tests/describe.rs"]
mod tests;
