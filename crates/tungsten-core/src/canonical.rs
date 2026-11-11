//! The repository's canonical JSON form (`D-236`), shared by screen files
//! and, since redo 7.12, rooms and tilesets: a model serialized with its
//! defaults left out, then written with keys sorted, arrays in order,
//! two-space indents, whole numbers without a fraction and others in the
//! shortest form that reads back as the same number, and a final newline.
//! Rooms and tilesets also print an array of numbers on one line
//! (`[10, 20]`); screen files keep every array on lines of its own.

use serde::Serialize;
use serde_json::Value;

/// How arrays print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arrays {
    /// Every array an item a line: screen files.
    Lines,
    /// An array of numbers alone on one line, `[10, 20]`: rooms and
    /// tilesets.
    InlineNumbers,
}

/// `model` in the canonical form. Through text first: a model number is
/// often an `f32`, which serde_json writes in its shortest form and which
/// reads back as that decimal.
pub(crate) fn to_canonical<T: Serialize>(model: &T, arrays: Arrays) -> String {
    let text = serde_json::to_string(model).unwrap_or_default();
    let value: Value = serde_json::from_str(&text).unwrap_or_default();
    let mut out = String::new();
    write(&value, 0, arrays, &mut out);
    out.push('\n');
    out
}

/// Writes `value` canonically at `indent` spaces.
fn write(value: &Value, indent: usize, arrays: Arrays, out: &mut String) {
    let pad = |out: &mut String, indent: usize| out.extend(std::iter::repeat_n(' ', indent));
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(on) => out.push_str(if *on { "true" } else { "false" }),
        Value::Number(number) => out.push_str(&canonical_number(number)),
        Value::String(text) => out.push_str(&Value::String(text.clone()).to_string()),
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items)
            if arrays == Arrays::InlineNumbers && items.iter().all(Value::is_number) =>
        {
            out.push('[');
            for (at, item) in items.iter().enumerate() {
                if at > 0 {
                    out.push_str(", ");
                }
                write(item, indent, arrays, out);
            }
            out.push(']');
        }
        Value::Array(items) => {
            out.push_str("[\n");
            for (at, item) in items.iter().enumerate() {
                pad(out, indent + 2);
                write(item, indent + 2, arrays, out);
                out.push_str(if at + 1 < items.len() { ",\n" } else { "\n" });
            }
            pad(out, indent);
            out.push(']');
        }
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push_str("{\n");
            for (at, key) in keys.iter().enumerate() {
                pad(out, indent + 2);
                out.push_str(&Value::String((*key).clone()).to_string());
                out.push_str(": ");
                write(&map[*key], indent + 2, arrays, out);
                out.push_str(if at + 1 < keys.len() { ",\n" } else { "\n" });
            }
            pad(out, indent);
            out.push('}');
        }
    }
}

/// A whole number without a fraction, any other in the shortest form that
/// reads back as the same `f64`.
fn canonical_number(number: &serde_json::Number) -> String {
    if number.is_i64() || number.is_u64() {
        return number.to_string();
    }
    let value = number.as_f64().unwrap_or_default();
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}
