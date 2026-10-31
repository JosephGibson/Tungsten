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
        terrain: TerrainProps::default(),
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

/// A 2 × 1 map of 16 px tiles whose tileset holds `tiles`, Tiled tiles
/// with their properties, and whose layers are `layers`, each filled with
/// gid 1.
fn map_with_tiles(tiles: &serde_json::Value, layers: &serde_json::Value) -> serde_json::Value {
    let layers: Vec<serde_json::Value> = layers
        .as_array()
        .unwrap()
        .iter()
        .map(|layer| {
            let mut layer = layer.clone();
            layer["type"] = "tilelayer".into();
            layer["data"] = serde_json::json!([1, 1]);
            layer
        })
        .collect();
    serde_json::json!({
        "tilewidth": 16, "tileheight": 16, "width": 2, "height": 1,
        "tilesets": [{ "firstgid": 1, "tiles": tiles }],
        "layers": layers
    })
}

/// A Tiled tile `id` drawing `sprite`, with `properties` beside its
/// `sprite_id`.
fn tiled_tile(id: u32, sprite: &str, properties: &serde_json::Value) -> serde_json::Value {
    let mut all =
        vec![serde_json::json!({ "name": "sprite_id", "type": "string", "value": sprite })];
    all.extend(properties.as_array().unwrap().iter().cloned());
    serde_json::json!({ "id": id, "properties": all })
}

#[test]
fn tile_properties_load_from_the_tileset() {
    let tiles = serde_json::json!([
        tiled_tile(
            0,
            "rock",
            &serde_json::json!([
                { "name": "material", "type": "string", "value": "stone" },
                { "name": "health", "type": "float", "value": 3 },
                { "name": "broken", "type": "string", "value": "rubble" },
                { "name": "style", "type": "string", "value": "cave" },
                { "name": "mask", "type": "int", "value": 5 },
                { "name": "weight", "type": "int", "value": 2 },
                { "name": "colour", "type": "color", "value": "#ff00ff00" }
            ])
        ),
        tiled_tile(
            1,
            "rubble",
            &serde_json::json!([
                { "name": "one_way", "type": "bool", "value": true },
                { "name": "material", "type": "string", "value": "gravel" }
            ])
        ),
        tiled_tile(2, "plain", &serde_json::json!([])),
        tiled_tile(
            3,
            "spike",
            &serde_json::json!([
                { "name": "health", "type": "float", "value": 0.5 }
            ])
        ),
        tiled_tile(
            4,
            "brush",
            &serde_json::json!([
                { "name": "style", "type": "string", "value": "cave" }
            ])
        ),
        tiled_tile(
            5,
            "rubble",
            &serde_json::json!([
                { "name": "material", "type": "string", "value": "dust" }
            ])
        )
    ]);
    let map = load_json(&map_with_tiles(
        &tiles,
        &serde_json::json!([{ "name": "ground" }]),
    ))
    .unwrap();
    let rock = map.tile_props(0);
    assert_eq!(
        (
            rock.material.as_deref(),
            rock.health,
            rock.broken,
            rock.one_way
        ),
        (Some("stone"), Some(3.0), 1, false),
        "`broken` names the first tile drawing `rubble`"
    );
    assert_eq!(
        (rock.style.as_deref(), rock.mask, rock.weight),
        (Some("cave"), Some(5), 2)
    );
    let rubble = map.tile_props(1);
    assert!(rubble.one_way);
    assert_eq!(
        (rubble.material.as_deref(), rubble.health, rubble.broken),
        (Some("gravel"), None, EMPTY_TILE)
    );
    assert_eq!(rubble.weight, 1, "a weight's default");
    assert_eq!(map.tile_props(2), &TileProps::default());
    assert_eq!(map.tile_props(3).health, Some(0.5));
    let brush = map.tile_props(4);
    assert_eq!(
        (brush.style.as_deref(), brush.mask),
        (Some("cave"), None),
        "a styled tile without a mask is the style's brush"
    );
    assert_eq!(map.tile_props(5).material.as_deref(), Some("dust"));
    for missing in [EMPTY_TILE, 6, 99] {
        assert_eq!(map.tile_props(missing), &TileProps::default(), "{missing}");
    }
    let default = TileProps::default();
    assert_eq!(
        (default.broken, default.weight, default.one_way),
        (EMPTY_TILE, 1, false)
    );
}

#[test]
fn a_collision_layers_physics_layers_load_from_its_properties() {
    let tiles = serde_json::json!([tiled_tile(0, "a", &serde_json::json!([]))]);
    let collision = |properties: serde_json::Value| {
        let mut all =
            vec![serde_json::json!({ "name": "kind", "type": "string", "value": "collision" })];
        all.extend(properties.as_array().unwrap().iter().cloned());
        serde_json::Value::Array(all)
    };
    let layers = serde_json::json!([
        { "name": "ground" },
        { "name": "walls", "properties": collision(serde_json::json!([
            { "name": "layers", "type": "string", "value": "1, 3" },
            { "name": "mask", "type": "string", "value": "0,2" }
        ])) },
        { "name": "glass", "properties": collision(serde_json::json!([
            { "name": "layers", "type": "string", "value": "" },
            { "name": "mask", "type": "string", "value": " " }
        ])) },
        { "name": "solid", "properties": collision(serde_json::json!([])) },
        { "name": "high", "properties": collision(serde_json::json!([
            { "name": "layers", "type": "string", "value": "31" },
            { "name": "mask", "type": "string", "value": "31,0,31" }
        ])) }
    ]);
    let map = load_json(&map_with_tiles(&tiles, &layers)).unwrap();
    let physics: Vec<_> = (0..6).map(|layer| map.layer_physics(layer)).collect();
    assert_eq!(
        physics,
        [
            LayerPhysics::DEFAULT,
            LayerPhysics {
                layers: 1 << 1 | 1 << 3,
                mask: 1 | 1 << 2
            },
            LayerPhysics { layers: 0, mask: 0 },
            LayerPhysics::DEFAULT,
            LayerPhysics {
                layers: 1 << 31,
                mask: 1 << 31 | 1
            },
            LayerPhysics::DEFAULT,
        ]
    );
    assert_eq!(
        LayerPhysics::DEFAULT,
        LayerPhysics {
            layers: 1,
            mask: u32::MAX
        },
        "layer 0, meeting every layer"
    );

    let cases = [
        (
            serde_json::json!({ "name": "ground", "properties": [
                { "name": "layers", "type": "string", "value": "1" }
            ] }),
            "layer 'ground': `layers` is a collision layer's property, and this layer's `kind` is not `collision`",
        ),
        (
            serde_json::json!({ "name": "ground", "properties": [
                { "name": "kind", "type": "string", "value": "render" },
                { "name": "mask", "type": "string", "value": "1" }
            ] }),
            "layer 'ground': `mask` is a collision layer's property, and this layer's `kind` is not `collision`",
        ),
        (
            serde_json::json!({ "name": "walls", "properties": collision(serde_json::json!([
                { "name": "layers", "type": "string", "value": "0, 32" }
            ])) }),
            "layer 'walls': `layers` holds layer 32; a layer must be 0 to 31",
        ),
        (
            serde_json::json!({ "name": "walls", "properties": collision(serde_json::json!([
                { "name": "mask", "type": "string", "value": "99999999999" }
            ])) }),
            "layer 'walls': `mask` holds layer 99999999999; a layer must be 0 to 31",
        ),
        (
            serde_json::json!({ "name": "walls", "properties": collision(serde_json::json!([
                { "name": "mask", "type": "int", "value": 3 }
            ])) }),
            "layer 'walls': `mask` must be a string of layer numbers separated by commas (got 3)",
        ),
    ];
    for (layer, expected) in cases {
        let err = load_json(&map_with_tiles(&tiles, &serde_json::json!([layer]))).unwrap_err();
        assert!(matches!(err, TilemapError::Invalid { .. }), "{err:?}");
        assert!(err.to_string().ends_with(expected), "{err}");
    }
    for list in ["1,,2", "a", "-1", "1;2", "1,", "1 2", "+1"] {
        let layer = serde_json::json!({ "name": "walls", "properties": collision(serde_json::json!([
            { "name": "layers", "type": "string", "value": list }
        ])) });
        let err = load_json(&map_with_tiles(&tiles, &serde_json::json!([layer]))).unwrap_err();
        assert!(
            err.to_string().ends_with(&format!(
                "layer 'walls': `layers` must be layer numbers 0 to 31 separated by commas (got \"{list}\")"
            )),
            "{err}"
        );
    }
}

#[test]
fn each_tile_property_rule_fails_the_load_naming_the_tile() {
    let style = serde_json::json!({ "name": "style", "type": "string", "value": "cave" });
    let mask = serde_json::json!({ "name": "mask", "type": "int", "value": 3 });
    let health = serde_json::json!({ "name": "health", "type": "float", "value": 1 });
    let property =
        |name: &str, value: serde_json::Value| serde_json::json!({ "name": name, "value": value });
    let cases = [
        (
            vec![property("material", 5.into())],
            "`material` must be a string (got 5)",
        ),
        (
            vec![property("material", "".into())],
            "`material` must not be empty",
        ),
        (
            vec![property("health", "3".into())],
            "`health` must be a number (got \"3\")",
        ),
        (
            vec![property("health", 0.into())],
            "`health` must be finite and above 0 (got 0)",
        ),
        (
            vec![property("health", (-2.5).into())],
            "`health` must be finite and above 0 (got -2.5)",
        ),
        (
            vec![property("health", 1e40.into())],
            "`health` must be finite and above 0 (got inf)",
        ),
        (
            vec![health.clone(), property("broken", 3.into())],
            "`broken` must be a string (got 3)",
        ),
        (
            vec![health.clone(), property("broken", "nothing".into())],
            "`broken` names sprite `nothing`, which no tile of the tileset has",
        ),
        (
            vec![property("broken", "base".into())],
            "`broken` needs `health`",
        ),
        (
            vec![property("one_way", "yes".into())],
            "`one_way` must be a bool (got \"yes\")",
        ),
        (
            vec![property("style", 1.into())],
            "`style` must be a string (got 1)",
        ),
        (
            vec![style.clone(), property("mask", 1.5.into())],
            "`mask` must be an integer (got 1.5)",
        ),
        (
            vec![style.clone(), property("mask", 16.into())],
            "`mask` must be 0 to 15 (got 16)",
        ),
        (
            vec![style.clone(), property("mask", (-1).into())],
            "`mask` must be 0 to 15 (got -1)",
        ),
        (vec![mask.clone()], "`mask` needs `style`"),
        (
            vec![style.clone(), mask.clone(), property("weight", 0.into())],
            "`weight` must be 1 to 4294967295 (got 0)",
        ),
        (
            vec![style.clone(), mask.clone(), property("weight", 2.5.into())],
            "`weight` must be an integer (got 2.5)",
        ),
        (
            vec![style.clone(), property("weight", 2.into())],
            "`weight` needs `mask`",
        ),
        (
            vec![style.clone(), property("weight", 1.into())],
            "`weight` needs `mask`",
        ),
        (
            vec![
                property("material", "a".into()),
                property("material", "b".into()),
            ],
            "`material` is given twice",
        ),
    ];
    for (properties, expected) in cases {
        let tiles = serde_json::json!([
            tiled_tile(0, "base", &serde_json::json!([])),
            tiled_tile(7, "a", &serde_json::Value::Array(properties.clone()))
        ]);
        let err = load_json(&map_with_tiles(
            &tiles,
            &serde_json::json!([{ "name": "ground" }]),
        ))
        .unwrap_err();
        assert!(matches!(err, TilemapError::Invalid { .. }), "{err:?}");
        assert!(
            err.to_string().ends_with(&format!("tile id=7: {expected}")),
            "{properties:?}: {err}"
        );
    }
}

#[test]
fn a_map_built_in_code_takes_tile_properties_and_layer_physics() {
    let mut map = TilemapData::new(16, 16, 4, 2);
    let rock = map.add_tile("rock");
    let rubble = map.add_tile("rubble");
    let plain = map.add_tile("plain");
    let ground = map.add_layer("ground", LayerKind::Render);
    let walls = map.add_layer("walls", LayerKind::Collision);
    let glass = map.add_layer("glass", LayerKind::Collision);
    assert_eq!(map.tile_props(rock), &TileProps::default());
    assert_eq!(map.layer_physics(walls), LayerPhysics::DEFAULT);

    let stone = TileProps {
        material: Some("stone".into()),
        health: Some(4.0),
        broken: rubble,
        ..TileProps::default()
    };
    map.set_tile_props(rock, stone.clone());
    let ledge = TileProps {
        one_way: true,
        style: Some("ledge".into()),
        mask: Some(10),
        weight: 3,
        ..TileProps::default()
    };
    map.set_tile_props(rubble, ledge.clone());
    assert_eq!(map.tile_props(rock), &stone);
    assert_eq!(map.tile_props(rubble), &ledge);
    assert_eq!(map.tile_props(plain), &TileProps::default());
    map.set_tile_props(rubble, TileProps::default());
    assert_eq!(map.tile_props(rubble), &TileProps::default());
    assert_eq!(map.tile_props(rock), &stone);

    let glassy = LayerPhysics {
        layers: 1 << 2,
        mask: 0,
    };
    map.set_layer_physics(glass, glassy);
    assert_eq!(map.layer_physics(glass), glassy);
    assert_eq!(map.layer_physics(walls), LayerPhysics::DEFAULT);
    assert_eq!(map.layer_physics(ground), LayerPhysics::DEFAULT);
    // A bulk write to the public fields reads the defaults.
    map.layers.push(TilemapLayer {
        name: "more".into(),
        kind: LayerKind::Collision,
        tiles: vec![EMPTY_TILE; 8],
    });
    map.tileset.push("extra".into());
    assert_eq!(map.layer_physics(3), LayerPhysics::DEFAULT);
    assert_eq!(map.tile_props(3), &TileProps::default());
    assert!(map.validate().is_ok());
}

#[test]
#[should_panic(expected = "TilemapData::set_tile_props: tile 0: `mask` needs `style`")]
fn set_tile_props_panics_on_a_broken_rule() {
    let mut map = TilemapData::new(16, 16, 2, 2);
    let tile = map.add_tile("rock");
    let props = TileProps {
        mask: Some(3),
        ..TileProps::default()
    };
    map.set_tile_props(tile, props);
}

#[test]
#[should_panic(expected = "TilemapData::set_tile_props: tile 1 is past the tileset's 1 entries")]
fn set_tile_props_panics_past_the_tileset() {
    let mut map = TilemapData::new(16, 16, 2, 2);
    map.add_tile("rock");
    map.set_tile_props(1, TileProps::default());
}

#[test]
#[should_panic(
    expected = "TilemapData::set_layer_physics: layer 0 ('ground') is not a collision layer"
)]
fn set_layer_physics_panics_on_a_render_layer() {
    let mut map = TilemapData::new(16, 16, 2, 2);
    let ground = map.add_layer("ground", LayerKind::Render);
    map.set_layer_physics(ground, LayerPhysics::DEFAULT);
}

#[test]
fn slope_tiles_load_and_refuse_one_way() {
    let slope =
        |text: &str| serde_json::json!([{ "name": "slope", "type": "string", "value": text }]);
    let names = [
        ("45 br", SlopeRise::Full, SlopeCorner::BottomRight),
        ("45 bl", SlopeRise::Full, SlopeCorner::BottomLeft),
        ("2:1 low tr", SlopeRise::Low, SlopeCorner::TopRight),
        ("2:1 high tl", SlopeRise::High, SlopeCorner::TopLeft),
    ];
    let tiles: Vec<serde_json::Value> = names
        .iter()
        .enumerate()
        .map(|(id, (text, ..))| tiled_tile(id as u32, "ramp", &slope(text)))
        .collect();
    let map = load_json(&map_with_tiles(
        &serde_json::Value::Array(tiles),
        &serde_json::json!([{ "name": "ground" }]),
    ))
    .unwrap();
    for (tile, (text, rise, corner)) in names.into_iter().enumerate() {
        assert_eq!(
            map.tile_props(tile as TileIndex).slope,
            Some(Slope { rise, corner }),
            "{text}"
        );
    }
    assert_eq!(TileProps::default().slope, None);
    // Every kind's code is its own, 1 to 12.
    let rises = [SlopeRise::Full, SlopeRise::Low, SlopeRise::High];
    let corners = [
        SlopeCorner::BottomRight,
        SlopeCorner::BottomLeft,
        SlopeCorner::TopRight,
        SlopeCorner::TopLeft,
    ];
    let mut codes: Vec<u8> = rises
        .iter()
        .flat_map(|&rise| {
            corners
                .iter()
                .map(move |&corner| Slope { rise, corner }.code())
        })
        .collect();
    codes.sort_unstable();
    assert_eq!(codes, (1..=12).collect::<Vec<u8>>());

    let one_way = serde_json::json!({ "name": "one_way", "type": "bool", "value": true });
    let cases = [
        (
            vec![slope("45 br")[0].clone(), one_way],
            "a `slope` tile cannot be `one_way`",
        ),
        (
            slope("45").as_array().unwrap().clone(),
            "`slope` must be a rise and a corner, as \"45 br\" (got \"45\")",
        ),
        (
            slope("30 br").as_array().unwrap().clone(),
            "`slope`'s rise must be \"45\", \"2:1 low\" or \"2:1 high\" (got \"30\")",
        ),
        (
            slope("2:1 low up").as_array().unwrap().clone(),
            "`slope`'s corner must be br, bl, tr or tl (got \"up\")",
        ),
        (
            vec![serde_json::json!({ "name": "slope", "value": 45 })],
            "`slope` must be a string (got 45)",
        ),
    ];
    for (properties, expected) in cases {
        let tiles = serde_json::json!([tiled_tile(7, "a", &serde_json::Value::Array(properties))]);
        let err = load_json(&map_with_tiles(
            &tiles,
            &serde_json::json!([{ "name": "ground" }]),
        ))
        .unwrap_err();
        assert!(
            err.to_string().ends_with(&format!("tile id=7: {expected}")),
            "{err}"
        );
    }
    let mut map = TilemapData::new(16, 16, 2, 2);
    let ramp = map.add_tile("ramp");
    let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        map.set_tile_props(
            ramp,
            TileProps {
                slope: Some(Slope::parse("45 br").unwrap()),
                one_way: true,
                ..TileProps::default()
            },
        );
    }));
    assert!(refused.is_err(), "the builder refuses a one-way slope");
}
