use super::*;
use std::io::Write;

fn tiny_map() -> TilemapData {
    TilemapData {
        tile_width: 16,
        tile_height: 16,
        width: 3,
        height: 2,
        tileset: vec!["grass".into(), "dirt".into()],
        layers: vec![
            TilemapLayer {
                name: "bg".into(),
                kind: LayerKind::Render,
                tiles: vec![0, 0, 0, 1, 1, 1],
            },
            TilemapLayer {
                name: "fg".into(),
                kind: LayerKind::Render,
                tiles: vec![-1, 0, -1, -1, -1, -1],
            },
        ],
    }
}

#[test]
fn tile_at_reads_row_major() {
    let m = tiny_map();
    assert_eq!(m.tile_at(0, 0, 0), Some(0));
    assert_eq!(m.tile_at(0, 2, 0), Some(0));
    assert_eq!(m.tile_at(0, 0, 1), Some(1));
    assert_eq!(m.tile_at(1, 1, 0), Some(0));
    assert_eq!(m.tile_at(1, 0, 0), Some(-1));
}

#[test]
fn tile_at_out_of_bounds_is_none() {
    let m = tiny_map();
    assert_eq!(m.tile_at(0, 3, 0), None);
    assert_eq!(m.tile_at(0, 0, 2), None);
    assert_eq!(m.tile_at(2, 0, 0), None);
}

#[test]
fn pixel_size_multiplies_dimensions() {
    let m = tiny_map();
    assert_eq!(m.pixel_size(), Vec2::new(48.0, 32.0));
}

#[test]
fn validate_rejects_wrong_layer_length() {
    let mut m = tiny_map();
    m.layers[0].tiles.pop();
    assert!(m.validate().is_err());
}

#[test]
fn validate_rejects_out_of_range_index() {
    let mut m = tiny_map();
    m.layers[0].tiles[0] = 99;
    let err = m.validate().unwrap_err();
    assert!(err.contains("out of range"));
}

#[test]
fn validate_allows_negative_indices_as_empty() {
    let mut m = tiny_map();
    m.layers[0].tiles[0] = -1;
    m.layers[0].tiles[1] = -42;
    assert!(m.validate().is_ok());
}

#[test]
fn validate_rejects_zero_dims() {
    let mut m = tiny_map();
    m.width = 0;
    assert!(m.validate().is_err());
    let mut m = tiny_map();
    m.tile_width = 0;
    assert!(m.validate().is_err());
}

#[test]
fn load_parses_a_real_file() {
    // D-032: Tiled .tmj parsing.
    let dir = std::env::temp_dir().join(format!(
        "tungsten_tilemap_test_{}_{}",
        std::process::id(),
        std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("demo.tmj");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(
        br#"{
      "type": "map", "version": "1.10",
      "orientation": "orthogonal", "renderorder": "right-down",
      "tilewidth": 8, "tileheight": 8,
      "width": 2, "height": 2,
      "infinite": false,
      "tilesets": [{
        "firstgid": 1, "columns": 0, "name": "test",
        "spacing": 0, "margin": 0,
        "tilewidth": 8, "tileheight": 8, "tilecount": 2,
        "tiles": [
          { "id": 0, "image": "a.png",
            "properties": [{"name": "sprite_id", "type": "string", "value": "a"}] },
          { "id": 1, "image": "b.png",
            "properties": [{"name": "sprite_id", "type": "string", "value": "b"}] }
        ]
      }],
      "layers": [
        { "id": 1, "type": "tilelayer", "name": "ground",
          "x": 0, "y": 0, "width": 2, "height": 2,
          "data": [1, 2, 2, 1] },
        { "id": 2, "type": "tilelayer", "name": "solid",
          "x": 0, "y": 0, "width": 2, "height": 2,
          "properties": [{"name": "kind", "type": "string", "value": "collision"}],
          "data": [0, 1, 0, 0] }
      ]
    }"#,
    )
    .unwrap();
    drop(f);

    let m = TilemapData::load(&path).unwrap();
    assert_eq!(m.width, 2);
    assert_eq!(m.tileset, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(m.layers.len(), 2);
    assert_eq!(m.layers[0].tiles, vec![0, 1, 1, 0]);
    assert_eq!(m.layers[1].kind, LayerKind::Collision);
    assert_eq!(m.layers[1].tiles, vec![-1, 0, -1, -1]);
}

#[test]
fn layer_kind_defaults_to_render() {
    let json = r#"{"name": "x", "tiles": []}"#;
    let layer: TilemapLayer = serde_json::from_str(json).unwrap();
    assert_eq!(layer.kind, LayerKind::Render);
}

#[test]
fn registry_insert_and_lookup() {
    let mut reg = TilemapRegistry::new();
    reg.insert_with_path("demo".into(), tiny_map(), PathBuf::from("/tmp/demo.tmj"));
    assert!(reg.get("demo").is_some());
    assert_eq!(reg.id_for_path(Path::new("/tmp/demo.tmj")), Some("demo"),);
    let ids: Vec<&str> = reg.ids().collect();
    assert_eq!(ids, vec!["demo"]);
}
