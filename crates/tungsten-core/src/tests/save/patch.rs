use serde_json::json;

use super::*;

/// Diffs `value` from `base`, then checks the patch merges back to it.
fn round_trip(base: &Value, value: &Value) -> Option<Value> {
    let patch = diff(base, value);
    let merged = match &patch {
        Some(patch) => merge(base.clone(), patch),
        None => base.clone(),
    };
    assert_eq!(&merged, value, "the patch {patch:?} merged over {base}");
    patch
}

#[test]
fn a_patch_merged_over_its_base_gives_back_the_value() {
    let base = json!({
        "speed": 3.0,
        "target": null,
        "path": [1, 2, 3],
        "brain": { "mood": "calm", "timer": { "left": 1.5, "loop": true }, "memo": 4 },
        "shield": 2
    });
    let value = json!({
        "speed": null,
        "target": 7,
        "path": [1, 2],
        "brain": { "mood": "calm", "timer": { "left": 0.5 }, "seen": [] },
        "extra": { "a": 1 }
    });
    let patch = round_trip(&base, &value).unwrap();
    assert_eq!(
        patch,
        json!({
            "speed": null,
            "target": 7,
            "path": [1, 2],
            "brain": { "timer": { "left": 0.5, "-": ["loop"] }, "seen": [], "-": ["memo"] },
            "extra": { "a": 1 },
            "-": ["shield"]
        }),
        "nulls written, arrays whole, removed fields under `-` at two depths"
    );
}

#[test]
fn an_unchanged_value_has_no_patch() {
    let value = json!({ "a": { "b": [1, 2] }, "c": null, "d": "x" });
    assert_eq!(round_trip(&value, &value), None);
    assert_eq!(round_trip(&json!([1.5, 2.0]), &json!([1.5, 2.0])), None);
    assert_eq!(round_trip(&json!(4), &json!(4)), None);
    assert_eq!(round_trip(&json!(null), &json!(null)), None);
}

#[test]
fn a_nested_change_patches_only_its_own_key() {
    let base = json!({
        "move_speed": { "base": 120.0, "min": 0.0, "modifiers": [] },
        "jump": { "base": 300.0, "min": 0.0, "modifiers": [] }
    });
    let value = json!({
        "move_speed": { "base": 120.0, "min": 0.0, "modifiers": [{ "add": 30.0 }] },
        "jump": { "base": 300.0, "min": 0.0, "modifiers": [] }
    });
    let patch = round_trip(&base, &value).unwrap();
    assert_eq!(
        patch,
        json!({ "move_speed": { "modifiers": [{ "add": 30.0 }] } })
    );

    let retuned = json!({
        "move_speed": { "base": 150.0, "min": 0.0, "modifiers": [] },
        "jump": { "base": 300.0, "min": 0.0, "modifiers": [] }
    });
    assert_eq!(
        merge(retuned, &patch)["move_speed"],
        json!({ "base": 150.0, "min": 0.0, "modifiers": [{ "add": 30.0 }] }),
        "a retuned base reaches the patched value"
    );
}

#[test]
fn a_value_that_is_no_object_patches_whole() {
    assert_eq!(
        round_trip(&json!([430.0, 344.0]), &json!([431.0, 344.0])),
        Some(json!([431.0, 344.0]))
    );
    assert_eq!(round_trip(&json!(3), &json!(4)), Some(json!(4)));
    assert_eq!(
        round_trip(&json!(true), &json!({ "a": 1 })),
        Some(json!({ "a": 1 }))
    );
    assert_eq!(
        round_trip(&json!({ "a": 1 }), &json!("gone")),
        Some(json!("gone"))
    );
    assert_eq!(
        round_trip(&json!({ "a": 1 }), &json!({ "a": [1] })),
        Some(json!({ "a": [1] }))
    );
}
