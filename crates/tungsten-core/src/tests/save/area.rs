//! Areas under their level references: spawned with their objects named,
//! kept while not loaded, entered later, and their tiles.

use serde_json::{Value, json};

use super::*;
use crate::assets::tilemap::{LayerKind, MapObject, ObjectLayer, TileProps};
use crate::assets::{PrefabData, PrefabRegistry, TilemapData};
use crate::save::{
    RefCodec, RefReader, RefWriter, SaveOptions, Saved, WorldSave, capture_world, restore_world,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Health {
    current: f32,
}

/// A hit activation and the targets it has struck.
#[derive(Debug, Clone, PartialEq)]
struct Hits {
    activation: u32,
    targets: Vec<Entity>,
}

#[derive(Serialize, Deserialize)]
struct HitsForm {
    activation: u32,
}

impl From<&Hits> for HitsForm {
    fn from(hits: &Hits) -> Self {
        Self {
            activation: hits.activation,
        }
    }
}

impl From<HitsForm> for Hits {
    fn from(form: HitsForm) -> Self {
        Self {
            activation: form.activation,
            targets: Vec::new(),
        }
    }
}

fn save_hits(hits: &Hits, out: &mut RefWriter<'_>) {
    for &target in &hits.targets {
        out.put(&hits.activation.to_string(), target);
    }
}

fn load_hits(hits: &mut Hits, refs: &RefReader<'_>) {
    let targets = refs.get(&hits.activation.to_string()).to_vec();
    hits.targets.extend(targets);
}

fn hits_keys(hits: &Hits) -> Vec<String> {
    vec![hits.activation.to_string()]
}

#[derive(Debug, Clone, Copy)]
struct Scene;

fn object(id: u32, name: &str, at: [f32; 2]) -> MapObject {
    MapObject {
        id,
        name: name.to_owned(),
        class: "bat".to_owned(),
        position: Vec2::from(at),
        size: Vec2::ZERO,
        properties: BTreeMap::new(),
    }
}

/// A `width` × `height` map of `tiles` (the first with health 10) with
/// layers `ground`, filled with the first, and `walls`, holding `objects`.
fn sized(
    width: u32,
    height: u32,
    tiles: &[&str],
    layers: [&str; 2],
    objects: Vec<MapObject>,
) -> TilemapData {
    let mut map = TilemapData::new(16, 16, width, height);
    for tile in tiles {
        map.add_tile(*tile);
    }
    map.set_tile_props(
        0,
        TileProps {
            health: Some(10.0),
            ..TileProps::default()
        },
    );
    let ground = map.add_layer(layers[0], LayerKind::Render);
    map.add_layer(layers[1], LayerKind::Collision);
    map.fill(ground, 0);
    map.object_layers.push(ObjectLayer {
        name: "things".to_owned(),
        objects,
    });
    map
}

/// An 8 × 4 map of `rock` and `moss`.
fn map(objects: Vec<MapObject>) -> TilemapData {
    sized(8, 4, &["rock", "moss"], ["ground", "walls"], objects)
}

const GROUND: usize = 0;
const WALLS: usize = 1;
const MOSS: TileIndex = 1;

fn world_with(health: SaveOptions<Health>) -> World {
    let mut world = World::new();
    world.register_component::<Health>("health");
    world.register_saved_with("health", health);
    world.register_saved_with(
        "hits",
        SaveOptions::<Hits>::form::<HitsForm>().refs(RefCodec {
            save: save_hits,
            load: load_hits,
            keys: hits_keys,
        }),
    );
    let mut prefabs = PrefabRegistry::new();
    let bat: PrefabData =
        serde_json::from_value(json!({ "components": { "health": { "current": 10.0 } } })).unwrap();
    prefabs.insert("bat".into(), bat);
    world.insert_resource(prefabs);
    let mut maps = TilemapRegistry::new();
    maps.insert(
        "north".into(),
        map(vec![
            object(1, "n_bat", [16.0, 16.0]),
            object(2, "", [32.0, 16.0]),
        ]),
    );
    maps.insert("south".into(), map(vec![object(1, "s_bat", [16.0, 16.0])]));
    maps.insert("east".into(), map(vec![object(1, "e_bat", [16.0, 16.0])]));
    world.insert_resource(maps);
    world
}

fn world() -> World {
    world_with(SaveOptions::new())
}

fn named(world: &World, name: &str) -> Entity {
    let entity = world
        .resource::<SaveState>()
        .named_entity(SaveId::authored(name))
        .unwrap();
    assert!(world.is_alive(entity), "`{name}` is alive");
    entity
}

fn health(world: &World, name: &str) -> f32 {
    world.get::<Health>(named(world, name)).unwrap().current
}

fn set_health(world: &mut World, name: &str, current: f32) {
    let entity = named(world, name);
    world.get_mut::<Health>(entity).unwrap().current = current;
}

fn text(save: &WorldSave) -> String {
    serde_json::to_string(save).unwrap()
}

fn value(save: &WorldSave) -> Value {
    serde_json::to_value(save).unwrap()
}

fn maps(world: &mut World) -> &mut TilemapRegistry {
    world.resource_mut::<TilemapRegistry>()
}

fn tile(world: &World, map: &str, layer: usize, col: u32, row: u32) -> Option<TileIndex> {
    world
        .resource::<TilemapRegistry>()
        .get(map)
        .unwrap()
        .tile_at(layer, col, row)
}

/// A run with `north` and `south` loaded, `n_bat`'s hits holding `s_bat`.
fn north_hits_south() -> WorldSave {
    let mut a = world();
    a.spawn_area("north", [0.0, 0.0]).unwrap();
    a.spawn_area("south", [200.0, 0.0]).unwrap();
    let target = named(&a, "s_bat");
    let bat = named(&a, "n_bat");
    a.insert(
        bat,
        Hits {
            activation: 1,
            targets: vec![target],
        },
    );
    set_health(&mut a, "s_bat", 4.0);
    capture_world(&a).unwrap()
}

#[test]
fn three_areas_restore_each_ones_changes_onto_its_own_objects() {
    let mut a = world();
    for (map, x) in [("north", 0.0), ("south", 200.0), ("east", 400.0)] {
        a.spawn_area(map, [x, 0.0]).unwrap();
    }
    for (bat, current) in [("n_bat", 1.0), ("s_bat", 2.0), ("e_bat", 3.0)] {
        set_health(&mut a, bat, current);
    }
    let saved = capture_world(&a).unwrap();
    assert_eq!(
        saved.area_names().collect::<Vec<_>>(),
        ["east", "north", "south"]
    );

    let mut b = world();
    b.spawn();
    for (map, x) in [("east", 400.0), ("north", 0.0), ("south", 200.0)] {
        b.spawn_area(map, [x, 0.0]).unwrap();
    }
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    for (bat, current) in [("n_bat", 1.0), ("s_bat", 2.0), ("e_bat", 3.0)] {
        assert_eq!(health(&b, bat), current, "{bat}");
        let area = match &bat[..1] {
            "n" => "north",
            "s" => "south",
            _ => "east",
        };
        assert_eq!(b.get::<Saved>(named(&b, bat)).unwrap().area(), Some(area));
    }
    assert_eq!(text(&capture_world(&b).unwrap()), text(&saved));
}

// A migration step's signature.
#[allow(clippy::unnecessary_wraps)]
fn times_ten(mut value: Value) -> Result<Value, String> {
    if let Some(current) = value.get("current").and_then(Value::as_f64) {
        value["current"] = json!(current * 10.0);
    }
    Ok(value)
}

#[test]
fn a_kept_area_is_written_back_at_the_registrys_versions() {
    let mut a = world();
    a.spawn_area("north", [0.0, 0.0]).unwrap();
    a.spawn_area("south", [200.0, 0.0]).unwrap();
    set_health(&mut a, "s_bat", 2.0);
    let saved = capture_world(&a).unwrap();

    let mut b = world_with(SaveOptions::new().version(2).step(1, times_ten));
    b.spawn_area("north", [0.0, 0.0]).unwrap();
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    let again = value(&capture_world(&b).unwrap());
    assert_eq!(again["versions"]["components"]["health"], json!(2));
    assert_eq!(
        again["areas"]["south"]["named"]["s_bat"]["components"]["health"],
        json!({ "current": 20.0 }),
        "kept, migrated to version 2"
    );

    b.spawn_area("south", [200.0, 0.0]).unwrap();
    let report = enter_area(&mut b, "south", &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(health(&b, "s_bat"), 20.0);
}

#[test]
fn entering_a_kept_area_applies_it_and_resolves_references_into_it() {
    let saved = north_hits_south();
    let mut b = world();
    b.spawn_area("north", [0.0, 0.0]).unwrap();
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    let bat = named(&b, "n_bat");
    assert_eq!(b.get::<Hits>(bat).unwrap().targets, [], "its target waits");

    b.spawn_area("south", [200.0, 0.0]).unwrap();
    assert_eq!(health(&b, "s_bat"), 10.0, "fresh until entered");
    let report = enter_area(&mut b, "south", &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(health(&b, "s_bat"), 4.0);
    assert_eq!(
        b.get::<Hits>(bat).unwrap().targets,
        [named(&b, "s_bat")],
        "resolved into the entered area"
    );
    let again = value(&capture_world(&b).unwrap());
    assert_eq!(
        again["areas"]["north"]["named"]["n_bat"]["refs"],
        json!({ "hits": { "1": [SaveId::authored("s_bat").0] } })
    );
}

#[test]
fn a_reference_into_a_kept_area_survives_a_capture_before_it_loads() {
    let saved = north_hits_south();
    let mut b = world();
    b.spawn_area("north", [0.0, 0.0]).unwrap();
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    let kept = capture_world(&b).unwrap();
    assert_eq!(text(&kept), text(&saved), "written as it came");

    let mut c = world();
    c.spawn_area("north", [0.0, 0.0]).unwrap();
    restore_world(&mut c, &kept, &(Scene,)).unwrap();
    c.spawn_area("south", [200.0, 0.0]).unwrap();
    enter_area(&mut c, "south", &(Scene,)).unwrap();
    assert_eq!(
        c.get::<Hits>(named(&c, "n_bat")).unwrap().targets,
        [named(&c, "s_bat")]
    );
}

#[test]
fn a_pending_reference_drops_once_its_key_is_gone() {
    let saved = north_hits_south();
    let rearm = |world: &mut World| {
        let bat = named(world, "n_bat");
        *world.get_mut::<Hits>(bat).unwrap() = Hits {
            activation: 2,
            targets: Vec::new(),
        };
    };

    // Rearmed before a second capture: the capture leaves it out.
    let mut b = world();
    b.spawn_area("north", [0.0, 0.0]).unwrap();
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    rearm(&mut b);
    let again = value(&capture_world(&b).unwrap());
    assert_eq!(
        again["areas"]["north"]["named"]["n_bat"]["components"]["hits"],
        json!({ "activation": 2 })
    );
    assert_eq!(again["areas"]["north"]["named"]["n_bat"].get("refs"), None);

    // Rearmed before the entry: the entry drops it.
    let mut c = world();
    c.spawn_area("north", [0.0, 0.0]).unwrap();
    restore_world(&mut c, &saved, &(Scene,)).unwrap();
    rearm(&mut c);
    c.spawn_area("south", [200.0, 0.0]).unwrap();
    enter_area(&mut c, "south", &(Scene,)).unwrap();
    assert_eq!(c.get::<Hits>(named(&c, "n_bat")).unwrap().targets, []);
    assert_eq!(health(&c, "s_bat"), 4.0, "the area itself still applied");
}

#[test]
fn an_object_the_map_dropped_is_reported_and_one_it_gained_starts_fresh() {
    let mut a = world();
    a.spawn_area("north", [0.0, 0.0]).unwrap();
    set_health(&mut a, "n_bat", 3.0);
    let saved = capture_world(&a).unwrap();

    let mut b = world();
    maps(&mut b).insert("north".into(), map(vec![object(3, "n_owl", [48.0, 16.0])]));
    b.spawn_area("north", [0.0, 0.0]).unwrap();
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert_eq!(
        report.dropped,
        ["`n_bat` in area `north`: the level no longer has it"]
    );
    assert_eq!(health(&b, "n_owl"), 10.0);
}

#[test]
fn a_maps_edits_and_damage_come_back_on_the_map_as_loaded() {
    let mut a = world();
    a.spawn_area("north", [0.0, 0.0]).unwrap();
    assert!(maps(&mut a).set_tile("north", WALLS, 2, 1, MOSS));
    assert!(maps(&mut a).set_tile("north", GROUND, 3, 0, EMPTY_TILE));
    maps(&mut a).damage_tile("north", GROUND, 5, 2, 4.0);
    let saved = capture_world(&a).unwrap();
    assert_eq!(
        value(&saved)["areas"]["north"]["tiles"],
        json!({ "north": {
            "layers": ["ground", "walls"],
            "edited": [[0, 3, 0, null], [1, 2, 1, "moss"]],
            "damaged": [[0, 5, 2, 6.0]] } })
    );

    let mut b = world();
    b.spawn_area("north", [0.0, 0.0]).unwrap();
    assert_eq!(tile(&b, "north", WALLS, 2, 1), Some(EMPTY_TILE));
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(tile(&b, "north", WALLS, 2, 1), Some(MOSS));
    assert_eq!(tile(&b, "north", GROUND, 3, 0), Some(EMPTY_TILE));
    assert_eq!(
        b.resource::<TilemapRegistry>()
            .health("north", GROUND, 5, 2),
        Some(6.0)
    );
    assert_eq!(text(&capture_world(&b).unwrap()), text(&saved));
}

#[test]
fn an_edit_past_its_map_or_on_a_renamed_layer_is_dropped_before_anything_changes() {
    let mut a = world();
    maps(&mut a).insert(
        "north".into(),
        sized(8, 4, &["rock", "moss", "ice"], ["ground", "walls"], vec![]),
    );
    a.spawn_area("north", [0.0, 0.0]).unwrap();
    for (layer, col, row, tile) in [
        (WALLS, 7, 3, MOSS),
        (WALLS, 1, 1, 0),
        (GROUND, 2, 2, MOSS),
        (WALLS, 2, 0, 2),
    ] {
        maps(&mut a).set_tile("north", layer, col, row, tile);
    }
    let saved = capture_world(&a).unwrap();

    let mut b = world();
    maps(&mut b).insert(
        "north".into(),
        sized(6, 3, &["rock", "moss"], ["floor", "walls"], vec![]),
    );
    b.spawn_area("north", [0.0, 0.0]).unwrap();
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert_eq!(
        report.dropped,
        [
            "map `north`'s edit at layer 0, (2, 2): the map has no layer `ground`",
            "map `north`'s edit at layer 1, (2, 0): the tileset has no `ice`",
            "map `north`'s edit at layer 1, (7, 3): the map is 6 × 3 cells",
        ]
    );
    assert_eq!(
        tile(&b, "north", WALLS, 1, 1),
        Some(0),
        "the edit that fits"
    );
}

#[test]
fn an_edit_names_its_tile_so_a_tile_added_before_it_restores_the_same_one() {
    let mut a = world();
    a.spawn_area("north", [0.0, 0.0]).unwrap();
    maps(&mut a).set_tile("north", WALLS, 1, 1, MOSS);
    let saved = capture_world(&a).unwrap();

    let mut b = world();
    maps(&mut b).insert(
        "north".into(),
        sized(8, 4, &["rock", "ice", "moss"], ["ground", "walls"], vec![]),
    );
    b.spawn_area("north", [0.0, 0.0]).unwrap();
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    let written = tile(&b, "north", WALLS, 1, 1).unwrap();
    assert_eq!(written, 2);
    assert_eq!(
        b.resource::<TilemapRegistry>()
            .get("north")
            .unwrap()
            .tileset[written as usize],
        "moss"
    );
}

#[test]
fn a_new_runs_area_spawns_on_the_map_as_loaded() {
    let mut world = world();
    let first = world.spawn_area("north", [0.0, 0.0]).unwrap();
    maps(&mut world).set_tile("north", WALLS, 1, 1, MOSS);
    for entity in first {
        world.despawn(entity);
    }
    world.spawn_area("north", [0.0, 0.0]).unwrap();
    assert_eq!(tile(&world, "north", WALLS, 1, 1), Some(EMPTY_TILE));
    assert_eq!(
        world
            .resource::<TilemapRegistry>()
            .edited_cells("north")
            .count(),
        0
    );
    assert_eq!(health(&world, "n_bat"), 10.0, "named again");
}

#[test]
fn an_unnamed_classed_object_spawns_unsaved() {
    let mut world = world();
    let spawned = world.spawn_area("north", [0.0, 0.0]).unwrap();
    assert_eq!(spawned.len(), 3, "the instance, then both objects");
    assert_eq!(
        world.get::<TilemapInstance>(spawned[0]).unwrap().id,
        "north"
    );
    assert!(!world.has::<Saved>(spawned[0]));
    let bat = world.get::<Saved>(spawned[1]).unwrap();
    assert_eq!(
        (bat.id(), bat.area()),
        (SaveId::authored("n_bat"), Some("north"))
    );
    assert!(world.has::<Health>(spawned[2]));
    assert!(!world.has::<Saved>(spawned[2]), "no name, no identity");
    assert!(matches!(
        world.spawn_area("north", [0.0, 0.0]),
        Err(AreaError::Name { .. })
    ));
}

/// `world` with room `cave`: two bats on a grid with no layers.
fn with_cave(mut world: World) -> World {
    let room = crate::assets::Room::from_json(
        r#"{ "version": 1, "cell": [16, 16], "size": [8, 4],
             "objects": [{ "id": "bat_1", "prefab": "bat", "at": [1, 1] },
                         { "id": "bat_2", "prefab": "bat", "at": [4, 1] }] }"#,
    )
    .unwrap();
    world.insert_room("cave", room).unwrap();
    world
}

#[test]
fn a_rooms_objects_restore_by_their_names() {
    let mut a = with_cave(world());
    a.spawn_room("cave", [0.0, 0.0], &(Scene,)).unwrap();
    set_health(&mut a, "cave.bat_2", 2.5);
    let saved = capture_world(&a).unwrap();
    assert_eq!(
        value(&saved)["areas"]["cave"]["named"]["cave.bat_2"]["components"],
        json!({ "health": { "current": 2.5 } }),
        "saved by its room name, in the room's area"
    );

    let mut b = with_cave(world());
    b.spawn();
    let spawned = b.spawn_room("cave", [64.0, 0.0], &(Scene,)).unwrap();
    let report = restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(health(&b, "cave.bat_2"), 2.5);
    assert_eq!(health(&b, "cave.bat_1"), 10.0, "unchanged, from its prefab");
    assert_eq!(named(&b, "cave.bat_2"), spawned.object("bat_2").unwrap());
    assert_eq!(text(&capture_world(&b).unwrap()), text(&saved));
}
