use super::*;

const MINIMAL: &str = r#"{
    "entities": [
        {
            "transform": { "position": [10.0, -4.0], "rotation": 0.5, "scale": [2.0, 3.0] },
            "sprite": { "asset_id": "hero", "color": [10, 20, 30, 255], "z_order": 5 },
            "visible": false,
            "tag": "player"
        }
    ]
}"#;

#[test]
fn load_parses_minimal_fixture() {
    let data: SceneData = serde_json::from_str(MINIMAL).expect("parse minimal");
    assert_eq!(data.entities.len(), 1);
    let entry = &data.entities[0];
    assert_eq!(entry.transform.position, [10.0, -4.0]);
    assert_eq!(entry.transform.rotation, 0.5);
    assert_eq!(entry.transform.scale, [2.0, 3.0]);
    let sprite = entry.sprite.as_ref().expect("sprite present");
    assert_eq!(sprite.asset_id, "hero");
    assert_eq!(sprite.color, [10, 20, 30, 255]);
    assert_eq!(sprite.z_order, 5);
    assert!(!entry.visible);
    assert_eq!(entry.tag.as_deref(), Some("player"));
}

#[test]
fn defaults_fill_missing_fields() {
    let src = r#"{
        "entities": [
            {
                "transform": { "position": [0.0, 0.0] },
                "sprite": { "asset_id": "s" }
            }
        ]
    }"#;
    let data: SceneData = serde_json::from_str(src).expect("parse defaults");
    let entry = &data.entities[0];
    assert_eq!(entry.transform.rotation, 0.0);
    assert_eq!(entry.transform.scale, [1.0, 1.0]);
    let sprite = entry.sprite.as_ref().unwrap();
    assert_eq!(sprite.color, [255, 255, 255, 255]);
    assert_eq!(sprite.z_order, 0);
    assert!(entry.visible);
    assert!(entry.tag.is_none());
}

#[test]
fn empty_entities_list_is_valid() {
    let data: SceneData = serde_json::from_str(r#"{ "entities": [] }"#).unwrap();
    assert!(data.entities.is_empty());
}

#[test]
fn round_trip_preserves_fields() {
    let data: SceneData = serde_json::from_str(MINIMAL).unwrap();
    let encoded = serde_json::to_string(&data).unwrap();
    let reparsed: SceneData = serde_json::from_str(&encoded).unwrap();
    assert_eq!(reparsed.entities.len(), data.entities.len());
    assert_eq!(
        reparsed.entities[0].transform.position,
        data.entities[0].transform.position
    );
}

const WITH_TWEEN: &str = r#"{
    "entities": [
        {
            "transform": { "position": [0.0, 0.0] },
            "sprite": { "asset_id": "overlay" },
            "tweens": [
                {
                    "duration": 0.4,
                    "easing": "cubic_out",
                    "repeat": "once",
                    "tag": "scene_fade_in",
                    "channels": [
                        { "kind": "color_a", "from": 0, "to": 255 },
                        { "kind": "position_x", "from": -100.0, "to": 0.0 }
                    ]
                }
            ]
        }
    ]
}"#;

#[test]
fn parses_scene_tween_entry() {
    let data: SceneData = serde_json::from_str(WITH_TWEEN).expect("parse");
    let entry = &data.entities[0];
    assert_eq!(entry.tweens.len(), 1);
    let t = &entry.tweens[0];
    assert_eq!(t.duration, 0.4);
    assert_eq!(t.easing, Easing::CubicOut);
    assert_eq!(t.channels.len(), 2);
    assert_eq!(t.tag.as_deref(), Some("scene_fade_in"));
}

#[test]
fn scene_tween_round_trips_through_json() {
    let data: SceneData = serde_json::from_str(WITH_TWEEN).unwrap();
    let encoded = serde_json::to_string(&data).unwrap();
    let reparsed: SceneData = serde_json::from_str(&encoded).unwrap();
    assert_eq!(reparsed.entities[0].tweens.len(), 1);
    assert_eq!(
        reparsed.entities[0].tweens[0].duration,
        data.entities[0].tweens[0].duration
    );
}

#[test]
fn scene_tween_into_tween_maps_fields() {
    let data: SceneData = serde_json::from_str(WITH_TWEEN).unwrap();
    let runtime = data.entities[0].tweens[0].into_tween();
    assert_eq!(runtime.channels.len(), 2);
    assert_eq!(runtime.duration, 0.4);
    assert_eq!(runtime.easing, Easing::CubicOut);
    assert_eq!(runtime.repeat, TweenRepeat::Once);
    assert_eq!(runtime.on_complete_tag.as_deref(), Some("scene_fade_in"));
}

#[test]
fn scene_tween_times_repeat_parses() {
    let src = r#"{ "entities": [ {
        "transform": { "position": [0.0, 0.0] },
        "tweens": [ {
            "duration": 0.5,
            "repeat": { "times": 4 },
            "channels": [ { "kind": "rotation", "from": 0.0, "to": 6.28 } ]
        } ]
    } ] }"#;
    let data: SceneData = serde_json::from_str(src).expect("parse times");
    let t = data.entities[0].tweens[0].into_tween();
    assert_eq!(t.repeat, TweenRepeat::Times(4));
}

#[test]
fn load_rejects_non_finite_duration() {
    use std::io::Write as _;
    let dir = std::env::temp_dir().join("tungsten-scene-bad-duration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scene.json");
    let bad = r#"{ "entities": [ {
        "transform": { "position": [0.0, 0.0] },
        "tweens": [ {
            "duration": 0.0,
            "channels": [ { "kind": "color_a", "from": 0, "to": 255 } ]
        } ]
    } ] }"#;
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(bad.as_bytes()).unwrap();
    }
    let err = SceneData::load(&path).expect_err("must reject zero duration");
    assert!(matches!(err, SceneError::Validation { .. }));
}

#[test]
fn load_rejects_empty_channels() {
    use std::io::Write as _;
    let dir = std::env::temp_dir().join("tungsten-scene-empty-channels");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scene.json");
    let bad = r#"{ "entities": [ {
        "transform": { "position": [0.0, 0.0] },
        "tweens": [ { "duration": 0.5, "channels": [] } ]
    } ] }"#;
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(bad.as_bytes()).unwrap();
    }
    let err = SceneData::load(&path).expect_err("must reject empty channels");
    assert!(matches!(err, SceneError::Validation { .. }));
}
