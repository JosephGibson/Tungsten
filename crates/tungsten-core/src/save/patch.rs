//! Changes over a fresh spawn (S-3): a named entity's component writes,
//! as a patch, only what its run changed from the value it was named with.
//!
//! For two objects, a patch holds each key whose value differs, an object
//! key's own patch recursively, and under [`REMOVED`] the keys the base had
//! and the value no longer writes. Arrays and every other value go whole
//! when they differ. A component whose serde form writes a key `"-"` of its
//! own cannot be patched apart from it.

use serde_json::{Map, Value};

/// The key a patch lists removed keys under.
pub(crate) const REMOVED: &str = "-";

/// What `value` changed from `base`; `None` when nothing did.
pub(crate) fn diff(base: &Value, value: &Value) -> Option<Value> {
    match (base, value) {
        (Value::Object(base), Value::Object(value)) => {
            let mut patch = Map::new();
            for (key, new) in value {
                match base.get(key) {
                    Some(old) if old == new => {}
                    Some(old @ Value::Object(_)) if new.is_object() => {
                        if let Some(inner) = diff(old, new) {
                            patch.insert(key.clone(), inner);
                        }
                    }
                    _ => {
                        patch.insert(key.clone(), new.clone());
                    }
                }
            }
            let removed: Vec<Value> = base
                .keys()
                .filter(|key| !value.contains_key(*key))
                .map(|key| Value::String(key.clone()))
                .collect();
            if !removed.is_empty() {
                patch.insert(REMOVED.to_owned(), Value::Array(removed));
            }
            (!patch.is_empty()).then_some(Value::Object(patch))
        }
        _ => (base != value).then(|| value.clone()),
    }
}

/// `patch` laid over `base`: what [`diff`] gave back from the value it
/// read. A base that is no object takes the patch whole.
pub(crate) fn merge(base: Value, patch: &Value) -> Value {
    let (Value::Object(mut base), Value::Object(patch)) = (base, patch) else {
        return patch.clone();
    };
    for (key, new) in patch {
        if key == REMOVED {
            continue;
        }
        let merged = match base.remove(key) {
            Some(old @ Value::Object(_)) if new.is_object() => merge(old, new),
            _ => new.clone(),
        };
        base.insert(key.clone(), merged);
    }
    if let Some(Value::Array(removed)) = patch.get(REMOVED) {
        for key in removed.iter().filter_map(Value::as_str) {
            base.remove(key);
        }
    }
    Value::Object(base)
}

#[cfg(test)]
#[path = "../tests/save/patch.rs"]
mod tests;
