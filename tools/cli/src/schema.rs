//! `tungsten schema` (redo 7.20, rooms and tilesets since 7.12): a file
//! format's JSON Schema, draft 2020-12, from the command line's own engine,
//! printed or written; an editor validates and completes a screen, theme,
//! room or tileset file with it, and the schemas of a level batch and of a
//! run script, every field described and every verb with an example, are
//! those formats' reference.

use anyhow::Context;
use serde_json::Value;
use tungsten_core::assets::room;
use tungsten_core::script;
use tungsten_core::ui::{screen_file, theme_file};

use crate::args::SchemaArgs;

/// The formats with a schema.
pub(crate) const NAMES: [&str; 6] = [
    "ui_screen",
    "ui_theme",
    "room",
    "tileset",
    "batch",
    "script",
];

/// The formats as a usage error lists them: `a, b, c or d`.
pub(crate) fn listed() -> String {
    let (last, rest) = NAMES.split_last().expect("formats");
    format!("{} or {last}", rest.join(", "))
}

/// The schema of format `name`; `None` for one not in [`NAMES`].
pub(crate) fn schema(name: &str) -> Option<Value> {
    match name {
        "ui_screen" => Some(screen_file::json_schema()),
        "ui_theme" => Some(theme_file::json_schema()),
        "room" => Some(room::json_schema()),
        "tileset" => Some(room::tileset::json_schema()),
        "batch" => Some(room::batch_schema()),
        "script" => Some(script::json_schema()),
        _ => None,
    }
}

/// Prints the schema `args` names, or writes it to its file.
pub(crate) fn run(args: &SchemaArgs) -> anyhow::Result<bool> {
    let schema = schema(&args.name).context("no such schema")?;
    let text = format!("{}\n", serde_json::to_string_pretty(&schema)?);
    match &args.out {
        Some(path) => std::fs::write(path, text)
            .with_context(|| format!("cannot write '{}'", path.display()))?,
        None => print!("{text}"),
    }
    Ok(true)
}

#[cfg(test)]
#[path = "tests/schema.rs"]
mod tests;
