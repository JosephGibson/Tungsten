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
