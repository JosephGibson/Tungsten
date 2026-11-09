//! Room generators: seeds, templates, the checks a generation passes before
//! anything registers, and the record each room carries.

use glam::Vec2;
use serde_json::json;

use super::*;
use crate::assets::room::tests::FULL_TILESET;
use crate::assets::{PrefabData, PrefabRegistry, Room, RoomCommand, TilemapRegistry};
use crate::save::SaveId;

/// An 8 × 4 template with a floor, a crate and a door on each side.
const HALL: &str = r#"{ "version": 1, "cell": [16, 16], "size": [8, 4], "tileset": "cave",
  "layers": [{ "name": "solid", "kind": "collision" }],
  "shapes": [{ "id": "floor", "rect": [0, 3, 8, 1], "tile": "rock", "layers": ["solid"] }],
  "objects": [{ "id": "crate_1", "prefab": "crate", "at": [1, 2] }],
  "places": [{ "id": "door_west", "point": [0, 2] }, { "id": "door_east", "point": [7, 2] }] }"#;

/// A World with the cave tileset, the prefab `crate` and the template
/// `hall`, and the generator `chain` of three rooms registered.
fn world() -> World {
    let mut world = World::new();
    let mut prefabs = PrefabRegistry::new();
    let crate_prefab: PrefabData = serde_json::from_value(json!({ "tag": "crate" })).unwrap();
    prefabs.insert("crate".into(), crate_prefab);
    world.insert_resource(prefabs);
    let mut rooms = RoomRegistry::new();
    rooms.insert_tileset("cave", Tileset::from_json(FULL_TILESET).unwrap());
    world.insert_resource(rooms);
    world
        .insert_room("hall", Room::from_json(HALL).unwrap())
        .unwrap();
    world.register_generator("chain", Chain { rooms: 3 });
    world
}

fn batch(json: &str) -> Vec<RoomCommand> {
    RoomCommand::parse_batch(json).unwrap()
}

/// `rooms` copies of `hall` in a row, 128 px apart, each with a second
/// crate at a column from the seed and its doors linked to its neighbours'.
struct Chain {
    rooms: usize,
}

impl RoomGenerator for Chain {
    fn version(&self) -> u32 {
        1
    }

    fn generate(&self, generation: &mut Generate<'_>) -> Result<(), String> {
        let hall = generation.template("hall")?;
        for at in 0..self.rooms {
            let local = format!("r{at}");
            let mut room = hall.clone();
            let col = generation.rng().next_u32() % 6 + 1;
            let mut commands = vec![json!({ "place": { "objects":
                { "id": "crate_2", "prefab": "crate", "at": [col, 2] } } })];
            if at > 0 {
                let to = format!("{}.door_east", generation.id(&format!("r{}", at - 1)));
                commands.push(json!({ "link": { "id": "door_west", "to": to } }));
            }
            if at + 1 < self.rooms {
                let to = format!("{}.door_west", generation.id(&format!("r{}", at + 1)));
                commands.push(json!({ "link": { "id": "door_east", "to": to } }));
            }
            let commands = RoomCommand::batch_from_value(&json!(commands)).unwrap();
            generation.apply(&local, &mut room, &commands)?;
            generation.add(&local, room, [at as f32 * 128.0, 0.0])?;
        }
        Ok(())
    }
}

/// A generator running `run`.
struct With<F>(F);

impl<F: Fn(&mut Generate<'_>) -> Result<(), String> + 'static> RoomGenerator for With<F> {
    fn version(&self) -> u32 {
        1
    }

    fn generate(&self, generation: &mut Generate<'_>) -> Result<(), String> {
        (self.0)(generation)
    }
}

/// Each generated room's canonical document, by ID.
fn documents(world: &World, rooms: &GeneratedRooms) -> Vec<(String, String)> {
    let registry = world.resource::<RoomRegistry>();
    rooms
        .rooms()
        .map(|(id, _)| (id.to_owned(), registry.room(id).unwrap().to_canonical()))
        .collect()
}

/// Each generated room's object names.
fn object_names(world: &World, rooms: &GeneratedRooms) -> Vec<String> {
    let maps = world.resource::<TilemapRegistry>();
    rooms
        .rooms()
        .flat_map(|(id, _)| {
            maps.get(id)
                .unwrap()
                .object_layers
                .iter()
                .flat_map(|layer| layer.objects.iter().map(|object| object.name.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The registry's rooms and maps, sorted.
fn registered(world: &World) -> (Vec<String>, Vec<String>) {
    let rooms = world
        .resource::<RoomRegistry>()
        .ids()
        .map(str::to_owned)
        .collect();
    let mut maps: Vec<String> = world
        .resource::<TilemapRegistry>()
        .ids()
        .map(str::to_owned)
        .collect();
    maps.sort();
    (rooms, maps)
}

fn remove_all(world: &mut World, rooms: &GeneratedRooms) {
    for (id, _) in rooms.rooms() {
        world.remove_room(id).unwrap();
    }
}

#[test]
fn a_seed_gives_the_same_rooms_and_names_each_time() {
    let mut first = world();
    let mut second = world();
    let a = first.generate_rooms("chain", 7, "chain").unwrap();
    let b = second.generate_rooms("chain", 7, "chain").unwrap();
    assert_eq!(a, b);
    assert_eq!(documents(&first, &a), documents(&second, &b));
    assert_eq!(object_names(&first, &a), object_names(&second, &b));
    assert_eq!(
        object_names(&first, &a)[..2],
        ["chain_r0.crate_1", "chain_r0.crate_2"]
    );
    // Again in one World, once the run's rooms are removed.
    let before = documents(&first, &a);
    remove_all(&mut first, &a);
    let again = first.generate_rooms("chain", 7, "chain").unwrap();
    assert_eq!(documents(&first, &again), before);
    // Another seed puts the crates elsewhere.
    remove_all(&mut first, &again);
    let other = first.generate_rooms("chain", 8, "chain").unwrap();
    assert_ne!(documents(&first, &other), before);
}

#[test]
fn generate_rooms_registers_each_room_with_its_generation() {
    let mut world = world();
    let rooms = world.generate_rooms("chain", 7, "run").unwrap();
    let ids: Vec<(&str, Vec2)> = rooms.rooms().collect();
    assert_eq!(
        ids,
        [
            ("run_r0", Vec2::ZERO),
            ("run_r1", Vec2::new(128.0, 0.0)),
            ("run_r2", Vec2::new(256.0, 0.0))
        ]
    );
    let generation = rooms.generation();
    assert_eq!(
        (
            generation.generator.as_str(),
            generation.version,
            generation.seed,
            generation.prefix.as_str()
        ),
        ("chain", 1, 7, "run")
    );
    let registry = world.resource::<RoomRegistry>();
    for (id, _) in rooms.rooms() {
        assert_eq!(registry.generation(id), Some(generation), "{id}");
        assert!(registry.compiled(id).is_some() && registry.path(id).is_none());
        assert!(
            world.resource::<TilemapRegistry>().get(id).is_some(),
            "{id}"
        );
    }
    assert_eq!(registry.generation("hall"), None, "a template is authored");
    let spawned = world
        .spawn_room("run_r1", [128.0, 0.0], &(Marker,))
        .unwrap();
    assert_eq!(spawned.objects().count(), 2);
    let names: Vec<String> = spawned
        .objects()
        .map(|(_, entity)| {
            let id = world.get::<crate::save::Saved>(entity).unwrap().id();
            world
                .resource::<crate::save::SaveState>()
                .name_of(id)
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(names, ["run_r1.crate_1", "run_r1.crate_2"]);
}

/// What a spawn's `with` gives.
#[derive(Debug, Clone, Copy)]
struct Marker;

#[test]
fn a_generation_records_each_template_it_read() {
    let mut world = world();
    let mut den = Room::from_json(HALL).unwrap();
    den.size = [9, 4];
    world.insert_room("den", den).unwrap();
    world.register_generator(
        "pair",
        With(|generation: &mut Generate<'_>| {
            let hall = generation.template("hall")?;
            generation.template("den")?;
            generation.template("hall")?;
            generation.add("a", hall, [0.0, 0.0])
        }),
    );
    let rooms = world.generate_rooms("pair", 1, "pair").unwrap();
    let templates = &rooms.generation().templates;
    assert_eq!(templates.keys().collect::<Vec<_>>(), ["den", "hall"]);
    for hash in templates.values() {
        assert_eq!(hash.len(), 16, "{hash}");
        assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()), "{hash}");
    }
    assert_ne!(templates["den"], templates["hall"]);
    // The 64-bit FNV-1a of the template's canonical form.
    let canonical = Room::from_json(HALL).unwrap().to_canonical();
    let fnv = canonical
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
    assert_eq!(templates["hall"], format!("{fnv:016x}"));
    // An edited template gives the next generation another hash; the rooms
    // already generated keep theirs.
    let mut hall = Room::from_json(HALL).unwrap();
    hall.size = [8, 5];
    world.insert_room("hall", hall).unwrap();
    let next = world.generate_rooms("pair", 1, "next").unwrap();
    assert_ne!(next.generation().templates["hall"], templates["hall"]);
    assert_eq!(
        world.resource::<RoomRegistry>().generation("pair_a"),
        Some(rooms.generation())
    );
}

#[test]
fn a_template_that_is_not_registered_fails_the_generation() {
    let mut world = world();
    world.register_generator(
        "typo",
        With(|generation: &mut Generate<'_>| {
            let hall = generation.template("hall")?;
            // Swallowed, the refusal still fails the generation.
            let _ = generation.template("hal");
            generation.add("a", hall, [0.0, 0.0])
        }),
    );
    let before = registered(&world);
    let err = world.generate_rooms("typo", 3, "typo").unwrap_err();
    assert!(
        matches!(err, GenerateError::Template { ref template, .. } if template == "hal"),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "generator `typo` at seed 3: template `hal`: no room `hal`; did you mean `hall`?"
    );
    assert_eq!(registered(&world), before, "nothing registered");
    // A generated room is no template.
    world.generate_rooms("chain", 1, "chain").unwrap();
    world.register_generator(
        "nested",
        With(|generation: &mut Generate<'_>| {
            let room = generation.template("chain_r0")?;
            generation.add("a", room, [0.0, 0.0])
        }),
    );
    let err = world.generate_rooms("nested", 1, "nested").unwrap_err();
    assert_eq!(
        err.to_string(),
        "generator `nested` at seed 1: template `chain_r0`: `chain_r0` is a generated room; a template is a file's or `insert_room`'s"
    );
}

#[test]
fn a_generation_that_adds_no_room_fails() {
    let mut world = world();
    world.register_generator(
        "none",
        With(|generation: &mut Generate<'_>| {
            generation.template("hall")?;
            Ok(())
        }),
    );
    let before = registered(&world);
    let err = world.generate_rooms("none", 3, "none").unwrap_err();
    assert_eq!(
        err,
        GenerateError::Empty {
            generator: "none".into(),
            seed: 3
        }
    );
    assert_eq!(err.to_string(), "generator `none` at seed 3 added no room");
    assert_eq!(registered(&world), before);
}

#[test]
fn a_room_that_does_not_compile_registers_nothing() {
    let mut world = world();
    world.register_generator(
        "bad",
        With(|generation: &mut Generate<'_>| {
            let hall = generation.template("hall")?;
            let mut broken = hall.clone();
            broken.tileset = Some("cavern".into());
            generation.add("a", hall, [0.0, 0.0])?;
            generation.add("b", broken, [128.0, 0.0])
        }),
    );
    let before = registered(&world);
    let err = world.generate_rooms("bad", 3, "bad").unwrap_err();
    assert!(
        matches!(err, GenerateError::Room { ref room, .. } if room == "bad_b"),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "generated room `bad_b`: tileset: no tileset `cavern`; did you mean `cave`?"
    );
    assert_eq!(registered(&world), before, "not even `bad_a`");
}

#[test]
fn an_id_a_room_or_map_holds_is_taken() {
    let mut world = world();
    let run = world.generate_rooms("chain", 1, "chain").unwrap();
    let err = world.generate_rooms("chain", 2, "chain").unwrap_err();
    assert_eq!(
        err,
        GenerateError::Taken {
            room: "chain_r0".into()
        }
    );
    assert_eq!(
        err.to_string(),
        "generated room `chain_r0`: a room or a map holds the ID; remove it first"
    );
    // A map no room registered holds its ID too.
    let map = world
        .resource::<TilemapRegistry>()
        .get("hall")
        .unwrap()
        .clone();
    world
        .resource_mut::<TilemapRegistry>()
        .insert("other_r1".into(), map);
    let before = registered(&world);
    assert!(matches!(
        world.generate_rooms("chain", 2, "other"),
        Err(GenerateError::Taken { ref room }) if room == "other_r1"
    ));
    assert_eq!(registered(&world), before, "not even `other_r0`");
    // Once the run's rooms are removed, its prefix generates again.
    remove_all(&mut world, &run);
    assert!(world.generate_rooms("chain", 2, "chain").is_ok());
}

#[test]
fn two_names_sharing_a_save_id_fail_the_generation() {
    // Found once by a birthday search over random object IDs.
    let (a, b) = ("clash_a.jnzvero", "clash_b.xysfyis");
    assert_eq!(SaveId::authored(a), SaveId::authored(b));
    let mut world = world();
    world.register_generator(
        "clash",
        With(|generation: &mut Generate<'_>| {
            let hall = generation.template("hall")?;
            for (local, object) in [("a", "jnzvero"), ("b", "xysfyis")] {
                let mut room = hall.clone();
                let rename =
                    format!(r#"[{{ "rename": {{ "id": "crate_1", "to": "{object}" }} }}]"#);
                generation.apply(local, &mut room, &batch(&rename))?;
                generation.add(local, room, [0.0, 0.0])?;
            }
            Ok(())
        }),
    );
    let before = registered(&world);
    let err = world.generate_rooms("clash", 1, "clash").unwrap_err();
    assert_eq!(
        err,
        GenerateError::Clash {
            name: b.into(),
            other: a.into()
        }
    );
    assert_eq!(
        err.to_string(),
        "save name `clash_b.xysfyis` takes the ID of `clash_a.jnzvero` in the same generation; rename one"
    );
    assert_eq!(registered(&world), before);
}

#[test]
fn a_generators_error_names_it_and_its_seed() {
    let mut world = world();
    world.register_generator(
        "broken",
        With(|_: &mut Generate<'_>| Err("no exit fits".to_owned())),
    );
    let err = world.generate_rooms("broken", 42, "broken").unwrap_err();
    assert_eq!(
        err,
        GenerateError::Failed {
            generator: "broken".into(),
            seed: 42,
            message: "no exit fits".into()
        }
    );
    assert_eq!(
        err.to_string(),
        "generator `broken` at seed 42: no exit fits"
    );
    // A room added wrongly fails it too, even when the error is dropped.
    world.register_generator(
        "careless",
        With(|generation: &mut Generate<'_>| {
            let hall = generation.template("hall")?;
            generation.add("a", hall.clone(), [0.0, 0.0])?;
            let _ = generation.add("a", hall.clone(), [0.0, 0.0]);
            generation.add("b", hall, [0.0, 0.0])
        }),
    );
    let err = world.generate_rooms("careless", 5, "careless").unwrap_err();
    assert_eq!(
        err.to_string(),
        "generator `careless` at seed 5: room `a` was added already"
    );
    world.register_generator(
        "shouting",
        With(|generation: &mut Generate<'_>| {
            let hall = generation.template("hall")?;
            generation.add("R0", hall, [0.0, 0.0])
        }),
    );
    let err = world.generate_rooms("shouting", 5, "loud").unwrap_err();
    assert_eq!(
        err.to_string(),
        "generator `shouting` at seed 5: room `R0` is not `[a-z][a-z0-9_]*`"
    );
    // An unknown generator, and a prefix that is no ID.
    let err = world.generate_rooms("chan", 1, "chain").unwrap_err();
    assert_eq!(
        err.to_string(),
        "no room generator `chan`; did you mean `chain`?"
    );
    let err = world.generate_rooms("chain", 1, "Chain").unwrap_err();
    assert_eq!(
        err.to_string(),
        "generation prefix `Chain` is not `[a-z][a-z0-9_]*`"
    );
    assert_eq!(registered(&world).0, ["hall"]);
}

#[test]
fn generated_rooms_link_by_their_ids() {
    let mut world = world();
    let rooms = world.generate_rooms("chain", 7, "chain").unwrap();
    let registry = world.resource::<RoomRegistry>();
    let to = |room: &str, place: &str| {
        registry
            .place(room, place)
            .and_then(|place| place.to.clone())
    };
    assert_eq!(
        to("chain_r0", "door_east").as_deref(),
        Some("chain_r1.door_west")
    );
    assert_eq!(
        to("chain_r1", "door_west").as_deref(),
        Some("chain_r0.door_east")
    );
    assert_eq!(
        to("chain_r2", "door_west").as_deref(),
        Some("chain_r1.door_east")
    );
    assert_eq!(to("chain_r0", "door_west"), None);
    for (id, _) in rooms.rooms() {
        let findings = world.room_findings(id, &[]);
        assert!(findings.is_empty(), "{id}: {findings:?}");
    }
    // A link to a room the generation did not make is a finding.
    world.register_generator(
        "dangling",
        With(|generation: &mut Generate<'_>| {
            let mut hall = generation.template("hall")?;
            let link = format!(
                r#"[{{ "link": {{ "id": "door_east", "to": "{}.door_west" }} }}]"#,
                generation.id("r1")
            );
            generation.apply("r0", &mut hall, &batch(&link))?;
            generation.add("r0", hall, [0.0, 0.0])
        }),
    );
    world.generate_rooms("dangling", 1, "dangling").unwrap();
    assert_eq!(world.room_findings("dangling_r0", &[]).len(), 1);
}

#[test]
#[should_panic(expected = "register_generator: `chain` is already a room generator")]
fn a_second_generator_under_one_id_panics() {
    let mut world = world();
    world.register_generator("chain", Chain { rooms: 1 });
}

#[test]
fn a_seed_shows_the_same_variants_each_time() {
    use crate::assets::AssetRegistry;
    use crate::components::Sprite;
    /// A World of `world()` whose `crate` shows `crate`, `crate_b` or
    /// `crate_c`, and the names its run's crates show.
    fn run(seed: u64) -> Vec<(String, String)> {
        let mut world = world();
        let mut assets = AssetRegistry::new();
        for name in ["crate", "crate_b", "crate_c"] {
            assets.intern_sprite(name);
        }
        world.insert_resource(assets);
        let crate_prefab: PrefabData = serde_json::from_value(
            json!({ "sprite": { "asset_id": "crate", "variants": ["crate_b", "crate_c"] } }),
        )
        .unwrap();
        world
            .resource_mut::<PrefabRegistry>()
            .insert("crate".into(), crate_prefab);
        let rooms = world.generate_rooms("chain", seed, "chain").unwrap();
        let mut shown = Vec::new();
        for (id, origin) in rooms.rooms() {
            let spawned = world.spawn_room(id, origin, &(Marker,)).unwrap();
            for (object, entity) in spawned.objects() {
                let sprite = world.get::<Sprite>(entity).unwrap().asset_id;
                let pool = world
                    .get::<crate::components::SpriteVariants>(entity)
                    .unwrap();
                assert_eq!(
                    sprite,
                    pool.by_save_id(SaveId::authored(&format!("{id}.{object}"))),
                    "named before it is drawn"
                );
                let name = world
                    .resource::<AssetRegistry>()
                    .sprite_name(sprite)
                    .unwrap();
                shown.push((format!("{id}.{object}"), name.to_owned()));
            }
        }
        shown
    }
    let first = run(7);
    assert_eq!(first.len(), 6);
    assert_eq!(run(7), first, "the same seed, the same variants");
    let names: std::collections::BTreeSet<&str> =
        first.iter().map(|(_, name)| name.as_str()).collect();
    assert!(names.len() > 1, "{first:?}");
}
