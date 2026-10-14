use super::*;
use std::io::Write;

fn tiny_map() -> TilemapData {
    TilemapData {
        tile_width: 16,
        tile_height: 16,
        width: 3,
        height: 2,
        tileset: vec!["grass".into(), "dirt".into()],
        object_layers: Vec::new(),
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

#[test]
fn sparse_tiled_ids_resolve_and_invalid_gids_return_errors() {
    let path = std::env::temp_dir().join(format!(
        "tungsten-sparse-tiles-{}-{}.tmj",
        std::process::id(),
        std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()
    ));
    let mut json = serde_json::json!({
        "tilewidth": 8, "tileheight": 8, "width": 2, "height": 1,
        "tilesets": [{"firstgid": 10, "tiles": [
            {"id": 5, "properties": [{"name": "sprite_id", "value": "b"}]},
            {"id": 0, "properties": [{"name": "sprite_id", "value": "a"}]}
        ]}],
        "layers": [{"type": "tilelayer", "name": "ground", "data": [15, 10]}]
    });
    std::fs::write(&path, json.to_string()).unwrap();
    let map = TilemapData::load(&path).unwrap();
    assert_eq!(map.tileset, ["a", "b"]);
    assert_eq!(map.layers[0].tiles, [1, 0]);
    for gid in [1, 11, u32::MAX] {
        json["layers"][0]["data"][0] = gid.into();
        std::fs::write(&path, json.to_string()).unwrap();
        assert!(TilemapData::load(&path).is_err(), "gid={gid}");
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_group_layer_is_an_error() {
    // Tiled's usual way to organise layers is a group; a collision layer
    // inside one was skipped with the group, so the map loaded with no
    // collision and no word.
    let dir = std::env::temp_dir().join(format!(
        "tungsten_tilemap_group_{}_{}",
        std::process::id(),
        std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("grouped.tmj");
    std::fs::write(
        &path,
        br#"{
      "tilewidth": 8, "tileheight": 8, "width": 1, "height": 1,
      "tilesets": [{ "firstgid": 1, "tiles": [
        { "id": 0, "properties": [{"name": "sprite_id", "type": "string", "value": "a"}] }
      ]}],
      "layers": [
        { "type": "objectgroup", "name": "spawns", "objects": [] },
        { "type": "group", "name": "level", "layers": [
          { "type": "tilelayer", "name": "solid", "data": [1],
            "properties": [{"name": "kind", "type": "string", "value": "collision"}] }
        ]}
      ]
    }"#,
    )
    .unwrap();
    let err = TilemapData::load(&path).unwrap_err();
    assert!(
        matches!(&err, TilemapError::Invalid { message, .. } if message.contains("'level'")),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Writes `map` to a `.tmj` of its own and loads it.
fn load_json(map: &serde_json::Value) -> Result<TilemapData, TilemapError> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path =
        std::env::temp_dir().join(format!("tungsten-objects-{}-{n}.tmj", std::process::id()));
    std::fs::write(&path, map.to_string()).unwrap();
    let result = TilemapData::load(&path);
    std::fs::remove_file(&path).unwrap();
    result
}

/// A 2 × 2 map of one tile with `objects` in layer `things`, its offset
/// [8, 4], and `alignment` on the tileset when given.
fn map_with_objects(objects: &serde_json::Value, alignment: Option<&str>) -> serde_json::Value {
    let mut tileset = serde_json::json!({ "firstgid": 1, "tiles": [
        { "id": 0, "properties": [{ "name": "sprite_id", "type": "string", "value": "a" }] }
    ]});
    if let Some(alignment) = alignment {
        tileset["objectalignment"] = alignment.into();
    }
    serde_json::json!({
        "tilewidth": 16, "tileheight": 16, "width": 2, "height": 2,
        "tilesets": [tileset],
        "layers": [
            { "type": "tilelayer", "name": "ground", "data": [1, 1, 1, 1] },
            { "type": "objectgroup", "name": "things", "offsetx": 8, "offsety": 4,
              "objects": objects }
        ]
    })
}

#[test]
fn object_layers_load_in_order_with_offsets_classes_and_properties() {
    let mut map = map_with_objects(
        &serde_json::json!([
            { "id": 3, "name": "start", "type": "slime", "x": 32, "y": 16.5,
              "width": 8, "height": 4, "rotation": 0, "visible": false,
              "properties": [
                { "name": "health.max", "type": "int", "value": 9 },
                { "name": "label", "type": "string", "value": "boss" },
                { "name": "speed", "type": "float", "value": 1.5 },
                { "name": "angry", "type": "bool", "value": true },
                { "name": "target", "type": "object", "value": 12 },
                { "name": "rigid_body", "type": "class", "propertytype": "Body",
                  "value": { "kind": "kinematic", "shape": { "radius": 3 } } }
              ] },
            { "id": 4, "class": "bat", "x": 0, "y": 0 },
            { "id": 5, "x": 1, "y": 2, "rotation": 45, "point": true }
        ]),
        None,
    );
    map["layers"].as_array_mut().unwrap().extend([
        serde_json::json!({ "type": "imagelayer", "name": "sky", "image": "sky.png" }),
        serde_json::json!({ "type": "objectgroup", "name": "empty", "objects": [] }),
    ]);
    let data = load_json(&map).unwrap();
    assert_eq!(data.layers.len(), 1, "the image layer is skipped");
    let names: Vec<_> = data.object_layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["things", "empty"]);
    let objects = &data.object_layers[0].objects;
    assert_eq!(objects.len(), 3);
    let slime = &objects[0];
    assert_eq!(
        (slime.id, slime.name.as_str(), slime.class.as_str()),
        (3, "start", "slime")
    );
    assert_eq!(
        slime.position,
        Vec2::new(40.0, 20.5),
        "the layer's offset added"
    );
    assert_eq!(slime.size, Vec2::new(8.0, 4.0));
    assert_eq!(
        serde_json::Value::Object(slime.properties.clone().into_iter().collect()),
        serde_json::json!({
            "health.max": 9, "label": "boss", "speed": 1.5, "angry": true, "target": 12,
            "rigid_body": { "kind": "kinematic", "shape": { "radius": 3 } }
        })
    );
    assert_eq!(objects[1].class, "bat", "Tiled 1.9 writes `class`");
    assert_eq!(objects[1].position, Vec2::new(8.0, 4.0));
    let marker = &objects[2];
    assert_eq!((marker.class.as_str(), marker.name.as_str()), ("", ""));
    assert_eq!(marker.size, Vec2::ZERO);
    assert!(marker.properties.is_empty());
}

#[test]
fn a_tile_object_is_placed_by_its_tilesets_alignment() {
    let objects = serde_json::json!([
        { "id": 1, "type": "crate", "gid": 1, "x": 16, "y": 48, "width": 16, "height": 16 },
        { "id": 2, "type": "crate", "x": 16, "y": 48, "width": 16, "height": 16 }
    ]);
    // `y` 48 and the layer's offset 4: the bottom-left alignments move the
    // top up by the height, 16.
    for (alignment, top) in [
        (None, 36.0),
        (Some("unspecified"), 36.0),
        (Some("bottomleft"), 36.0),
        (Some("topleft"), 52.0),
    ] {
        let data = load_json(&map_with_objects(&objects, alignment)).unwrap();
        let found = &data.object_layers[0].objects;
        assert_eq!(found[0].position, Vec2::new(24.0, top), "{alignment:?}");
        assert_eq!(
            found[1].position,
            Vec2::new(24.0, 52.0),
            "not a tile object"
        );
    }
    let err = load_json(&map_with_objects(&objects, Some("center"))).unwrap_err();
    assert!(
        err.to_string().ends_with(
            "object layer 'things', object 1: is a tile object, and its tileset's object alignment `center` is not supported; use `bottomleft` or `topleft`"
        ),
        "{err}"
    );
    // An alignment no tile object uses does not matter.
    let plain = serde_json::json!([{ "id": 2, "x": 0, "y": 0 }]);
    assert!(load_json(&map_with_objects(&plain, Some("center"))).is_ok());
}

#[test]
fn each_object_rule_fails_the_load_naming_the_layer_and_the_object() {
    let cases = [
        (
            serde_json::json!({ "x": 0, "y": 0 }),
            "objects[0] has no `id`",
        ),
        (
            serde_json::json!({ "id": 7, "y": 0 }),
            "object 7: has no `x`",
        ),
        (
            serde_json::json!({ "id": 7, "x": 1e40, "y": 0 }),
            "object 7: `x` must be finite (got inf)",
        ),
        (
            serde_json::json!({ "id": 7, "x": 0, "y": 0, "height": -1e40 }),
            "object 7: `height` must be finite (got -inf)",
        ),
        (
            serde_json::json!({ "id": 7, "x": 0, "y": 0, "width": -2 }),
            "object 7: `width` must be at least 0 (got -2)",
        ),
        (
            serde_json::json!({ "id": 7, "x": 16_777_216, "y": 0 }),
            "object 7: position must be finite and within ±16777216 (got [16777224, 4])",
        ),
        (
            serde_json::json!({ "id": 7, "type": "slime", "class": "bat", "x": 0, "y": 0 }),
            "object 7: `type` `slime` and `class` `bat` differ; give the class once",
        ),
        (
            serde_json::json!({ "id": 7, "template": "slime.tx", "x": 0, "y": 0 }),
            "object 7: is an instance of template `slime.tx`, whose fields the loader does not read; detach it in Tiled",
        ),
        (
            serde_json::json!({ "id": 7, "type": "slime", "rotation": 90, "x": 0, "y": 0 }),
            "object 7: has class `slime` and a rotation of 90 degrees; a prefab spawns unrotated",
        ),
        (
            serde_json::json!({ "id": 7, "x": 0, "y": 0, "properties": [
                { "name": "hp", "type": "int", "value": 1 },
                { "name": "hp", "type": "int", "value": 2 }
            ] }),
            "object 7: property `hp` is given twice",
        ),
    ];
    for (object, expected) in cases {
        let err = load_json(&map_with_objects(&serde_json::json!([object]), None)).unwrap_err();
        assert!(matches!(err, TilemapError::Invalid { .. }), "{err:?}");
        let message = err.to_string();
        assert!(
            message.ends_with(&format!("object layer 'things', {expected}"))
                || message.ends_with(&format!("object layer 'things': {expected}")),
            "{message}"
        );
    }
    let mut map = map_with_objects(&serde_json::json!([]), None);
    map["layers"][1]["offsetx"] = serde_json::json!(1e40);
    let err = load_json(&map).unwrap_err();
    assert!(
        err.to_string()
            .ends_with("object layer 'things': offset must be finite (got [inf, 4])"),
        "{err}"
    );
    // A matching `type` and `class` pass, and a marker may rotate.
    let fine = serde_json::json!([
        { "id": 1, "type": "slime", "class": "slime", "x": 0, "y": 0 },
        { "id": 2, "type": "", "class": "bat", "x": 0, "y": 0 },
        { "id": 3, "rotation": 30, "x": 0, "y": 0 }
    ]);
    let data = load_json(&map_with_objects(&fine, None)).unwrap();
    let classes: Vec<_> = data.object_layers[0]
        .objects
        .iter()
        .map(|o| o.class.as_str())
        .collect();
    assert_eq!(classes, ["slime", "bat", ""]);
}

#[test]
fn the_builder_makes_a_map_cell_by_cell() {
    let mut map = TilemapData::new(16, 8, 3, 2);
    assert_eq!(
        (map.tile_width, map.tile_height, map.width, map.height),
        (16, 8, 3, 2)
    );
    assert!(map.tileset.is_empty() && map.layers.is_empty() && map.object_layers.is_empty());
    let grass = map.add_tile("grass");
    let dirt = map.add_tile(String::from("dirt"));
    assert_eq!((grass, dirt), (0, 1));
    let ground = map.add_layer("ground", LayerKind::Render);
    let solid = map.add_layer("solid", LayerKind::Collision);
    assert_eq!((ground, solid), (0, 1));
    assert_eq!(map.layers[solid].tiles, [EMPTY_TILE; 6]);
    assert_eq!(map.layers[solid].kind, LayerKind::Collision);
    map.fill(ground, grass);
    map.set_tile(ground, 2, 1, dirt);
    map.set_tile(solid, 0, 1, dirt);
    map.set_tile(solid, 0, 1, -5);
    map.set_tile(solid, 1, 0, grass);
    assert_eq!(map.layers[ground].tiles, [0, 0, 0, 0, 0, 1]);
    assert_eq!(
        map.layers[solid].tiles,
        [-1, 0, -1, -1, -1, -1],
        "a negative tile clears"
    );
    assert_eq!(map.tile_at(ground, 2, 1), Some(dirt));
    assert_eq!(map.pixel_size(), Vec2::new(48.0, 16.0));
    assert!(map.validate().is_ok());
    map.fill(solid, -1);
    assert_eq!(map.layers[solid].tiles, [EMPTY_TILE; 6]);
}

#[test]
#[should_panic(expected = "TilemapData::new: dimensions must be non-zero")]
fn the_builder_panics_on_a_zero_dimension() {
    let _ = TilemapData::new(16, 16, 0, 4);
}

#[test]
#[should_panic(expected = "TilemapData::set_tile: cell (3, 0) is outside the 3x2 map")]
fn set_tile_panics_outside_the_map() {
    let mut map = TilemapData::new(16, 16, 3, 2);
    map.add_tile("grass");
    let layer = map.add_layer("ground", LayerKind::Render);
    map.set_tile(layer, 3, 0, 0);
}

#[test]
#[should_panic(expected = "TilemapData::set_tile: tile 1 is past the tileset's 1 entries")]
fn set_tile_panics_past_the_tileset() {
    let mut map = TilemapData::new(16, 16, 3, 2);
    map.add_tile("grass");
    let layer = map.add_layer("ground", LayerKind::Render);
    map.set_tile(layer, 0, 0, 1);
}

#[test]
#[should_panic(expected = "TilemapData::fill: layer 2 is past the map's 1 layers")]
fn fill_panics_on_a_missing_layer() {
    let mut map = TilemapData::new(16, 16, 3, 2);
    map.add_tile("grass");
    map.add_layer("ground", LayerKind::Render);
    map.fill(2, 0);
}
