use std::path::{Path, PathBuf};

use super::*;
use serde_json::{Value, json};
use tungsten_core::{ComponentRegistry, EventQueue, Plugin, Schedule, Stage, StagedLoad, Time};

use crate::damage::health::{DamagedPart, Health, HealthPlugin};
use crate::damage::pipeline::{DamagePart, DamageRequest};
use crate::foundation::tags::TagsPlugin;

/// A World with health and `declared` as tags.
fn rig(declared: &[String]) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(declared.iter().cloned())
        .build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

/// `t00` to `t<count - 1>`, tag names that sort as they count.
fn numbered(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("t{i:02}")).collect()
}

/// A folder of data files for one test, removed when it drops.
struct Files(PathBuf);

impl Files {
    fn new(test: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("tungsten_kit_kinds_{}_{test}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// Writes `types`, an entry with these names and no fields, as `file`.
    fn write(&self, file: &str, types: &[String]) -> PathBuf {
        let entry: serde_json::Map<String, Value> =
            types.iter().map(|name| (name.clone(), json!({}))).collect();
        let path = self.0.join(file);
        std::fs::write(&path, Value::Object(entry).to_string()).unwrap();
        path
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Loads each `(id, file)` as a `damage_types` entry in one commit.
fn load(world: &mut World, entries: &[(&str, &Path)]) -> Result<(), String> {
    let mut staged = StagedLoad::new();
    for &(id, path) in entries {
        staged
            .add_data(world, DAMAGE_TYPES, id, path)
            .map_err(|e| e.to_string())?;
    }
    staged.commit(world).map_err(|e| e.to_string())
}

fn bit(world: &World, name: &str) -> Option<u8> {
    world.resource::<DamageTypeTable>().bit(name)
}

/// The kinds of each part sent so far, by name.
fn parts_sent(world: &World) -> Vec<Vec<String>> {
    world
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .map(|part| {
            part.kinds
                .names(world)
                .into_iter()
                .map(str::to_owned)
                .collect()
        })
        .collect()
}

fn typed(amount: f32, name: &str) -> DamagePart {
    DamagePart::new(amount).with_tags(Tags::new([name]))
}

#[test]
fn bits_follow_sorted_names_at_the_first_read_and_append_after() {
    let files = Files::new("bits");
    let (mut world, _) = rig(&names(&["ice", "fire", "acid", "zap", "beam"]));
    let game = files.write("game.json", &names(&["ice", "fire", "acid"]));
    load(&mut world, &[("game", &game)]).unwrap();
    assert_eq!(bit(&world, "acid"), None, "no read yet");
    merge_types(&mut world);
    assert_eq!(
        [bit(&world, "acid"), bit(&world, "fire"), bit(&world, "ice")],
        [Some(0), Some(1), Some(2)]
    );
    let more = files.write("more.json", &names(&["zap", "beam"]));
    load(&mut world, &[("more", &more)]).unwrap();
    merge_types(&mut world);
    assert_eq!(
        [bit(&world, "beam"), bit(&world, "zap")],
        [Some(3), Some(4)],
        "appended, sorted among themselves"
    );
    assert_eq!(bit(&world, "acid"), Some(0), "the first bits stay");
    let table = world.resource::<DamageTypeTable>();
    let kinds = table.mask(&Tags::new(["zap", "acid"]));
    assert_eq!(kinds.names(&world), ["acid", "zap"]);
    merge_types(&mut world);
    assert_eq!(
        world.resource::<DamageTypeTable>().len(),
        5,
        "a merge adds once"
    );
}

#[test]
fn a_reload_keeps_a_queued_masks_meaning_and_a_removed_types_bit() {
    let files = Files::new("reload");
    let (mut world, mut schedule) = rig(&names(&["fire", "ice", "acid"]));
    let game = files.write("game.json", &names(&["fire", "ice"]));
    load(&mut world, &[("game", &game)]).unwrap();
    let golem = world.spawn_with((Health::new(100.0),));
    assert!(
        DamageRequest::from_parts(golem, [typed(1.0, "ice")]).queue(&mut world),
        "queued before the reload"
    );
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    assert_eq!(parts_sent(&world), [names(&["ice"])]);
    let ice = bit(&world, "ice").unwrap();

    // The reload drops `ice` and adds `acid`; the event sent before it is
    // read after it.
    let game = files.write("game.json", &names(&["fire", "acid"]));
    load(&mut world, &[("game", &game)]).unwrap();
    DamageRequest::from_parts(golem, [typed(1.0, "acid")]).apply(&mut world);
    assert_eq!(
        parts_sent(&world),
        [names(&["ice"]), names(&["acid"])],
        "the queued mask still reads `ice`"
    );
    assert_eq!(bit(&world, "ice"), Some(ice), "its bit stays reserved");
    assert_eq!(bit(&world, "acid"), Some(2));
    // A part of the removed type still carries its bit, as a queued mask
    // would.
    DamageRequest::from_parts(golem, [typed(1.0, "ice")]).apply(&mut world);
    assert_eq!(parts_sent(&world)[2], names(&["ice"]));
}

#[test]
fn sixty_four_types_load_and_a_sixty_fifth_fails() {
    let files = Files::new("capacity");
    let mut declared = numbered(65);
    declared.extend(names(&["a", "z", "zz"]));

    // One entry of 65.
    let (mut world, _) = rig(&declared);
    let all = files.write("all.json", &numbered(65));
    let error = load(&mut world, &[("all", &all)]).unwrap_err();
    assert!(error.contains("entry `all`"), "{error}");
    assert!(
        error.ends_with("unknown damage type (a World holds 64) `t00`"),
        "{error}"
    );
    assert!(
        world
            .resource::<DataRegistry>()
            .ids(DAMAGE_TYPES)
            .next()
            .is_none()
    );

    // Two entries of 40 and 25.
    let (mut world, _) = rig(&declared);
    let names65 = numbered(65);
    let first = files.write("first.json", &names65[..40]);
    let second = files.write("second.json", &names65[40..]);
    let error = load(&mut world, &[("first", &first), ("second", &second)]).unwrap_err();
    assert!(error.contains("(a World holds 64)"), "{error}");

    // 64 load, and the last name takes bit 63.
    let (mut world, _) = rig(&declared);
    let full = files.write("full.json", &numbered(64));
    load(&mut world, &[("full", &full)]).unwrap();
    merge_types(&mut world);
    assert_eq!(bit(&world, "t63"), Some(63));

    // All 64 taken: a reload replacing the last-sorted name with an earlier
    // one fails the replacement's entry and keeps the previous entries.
    let mut replaced = numbered(63);
    replaced.push("a".to_owned());
    let full = files.write("full.json", &replaced);
    let error = load(&mut world, &[("full", &full)]).unwrap_err();
    assert!(error.contains("entry `full`"), "{error}");
    assert!(error.ends_with("(a World holds 64) `a`"), "{error}");
    let data = world.resource::<DataRegistry>();
    assert!(data.get::<DamageTypes>("full").unwrap().defines("t63"));

    // 63 taken: an unmerged entry holding `z` loses its file to a new entry
    // holding `a` and `zz`, which fails, and the old entry keeps its file.
    let (mut world, _) = rig(&declared);
    let main = files.write("main.json", &numbered(63));
    load(&mut world, &[("main", &main)]).unwrap();
    merge_types(&mut world);
    let shared = files.write("shared.json", &names(&["z"]));
    load(&mut world, &[("late", &shared)]).unwrap();
    let shared = files.write("shared.json", &names(&["a", "zz"]));
    let error = load(&mut world, &[("next", &shared)]).unwrap_err();
    assert!(error.contains("entry `next`"), "{error}");
    assert!(error.ends_with("(a World holds 64) `a`"), "{error}");
    let data = world.resource::<DataRegistry>();
    assert!(data.get::<DamageTypes>("next").is_none());
    assert_eq!(
        data.entry_for_path(&shared.canonicalize().unwrap()),
        Some((DAMAGE_TYPES, "late"))
    );
}

#[test]
fn a_typed_apply_right_after_a_load_carries_its_mask() {
    let files = Files::new("apply");
    let (mut world, mut schedule) = rig(&names(&["fire", "ice"]));
    let golem = world.spawn_with((Health::new(100.0),));
    let game = files.write("game.json", &names(&["fire"]));
    load(&mut world, &[("game", &game)]).unwrap();
    DamageRequest::from_parts(golem, [typed(1.0, "fire")]).apply(&mut world);
    assert_eq!(
        parts_sent(&world),
        [names(&["fire"])],
        "merged by the apply"
    );

    let more = files.write("more.json", &names(&["ice"]));
    load(&mut world, &[("more", &more)]).unwrap();
    assert!(DamageRequest::from_parts(golem, [typed(1.0, "ice")]).queue(&mut world));
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    assert_eq!(
        parts_sent(&world)[1],
        names(&["ice"]),
        "merged by the step's resolve"
    );
}

#[test]
fn a_type_two_entries_define_fails_the_load() {
    let files = Files::new("twice");
    let (mut world, _) = rig(&names(&["fire", "ice"]));
    let fantasy = files.write("fantasy.json", &names(&["fire"]));
    let scifi = files.write("scifi.json", &names(&["fire", "ice"]));
    let error = load(&mut world, &[("fantasy", &fantasy), ("scifi", &scifi)]).unwrap_err();
    assert!(error.contains("entry `fantasy`"), "{error}");
    assert!(error.ends_with("`fire, also defined by scifi`"), "{error}");
    assert!(
        world
            .resource::<DataRegistry>()
            .ids(DAMAGE_TYPES)
            .next()
            .is_none()
    );

    // An undeclared type fails as a tag.
    let acid = files.write("acid.json", &names(&["acid"]));
    let error = load(&mut world, &[("acid", &acid)]).unwrap_err();
    assert!(error.ends_with("unknown tag `acid`"), "{error}");
    assert!(serde_json::from_value::<DamageTypes>(json!({ "Fire": {} })).is_err());
    assert!(
        serde_json::from_value::<DamageTypes>(json!({ "fire": { "colour": [1, 0, 0, 1] } }))
            .is_err()
    );
}

#[test]
fn penetration_and_conversion_refuse_bad_names_and_bounds() {
    for bad in [
        json!({ "Fire": 0.5 }),
        json!({ "fire": 1.5 }),
        json!({ "fire": -0.1 }),
        json!({ "fire": "half" }),
    ] {
        assert!(
            serde_json::from_value::<Penetration>(bad.clone()).is_err(),
            "{bad}"
        );
        assert!(
            serde_json::from_value::<PenetrationDef>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    assert!(serde_json::from_value::<Penetration>(json!({ "fire": 1 })).is_ok());
    for bad in [
        json!([{ "from": "fire", "to": "fire", "pct": 0.5 }]),
        json!([{ "from": "fire", "to": "ice", "pct": 1.5 }]),
        json!([{ "from": "fire", "to": "ice", "pct": -0.5 }]),
        json!([{ "from": "Fire", "to": "ice", "pct": 0.5 }]),
        json!([{ "from": "fire", "to": "ice", "pct": 0.5, "chain": true }]),
        json!([{ "from": "fire", "pct": 0.5 }]),
    ] {
        assert!(
            serde_json::from_value::<Conversion>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    let panics = |f: fn()| std::panic::catch_unwind(f).is_err();
    assert!(panics(|| {
        let _ = Penetration::new().with_points("Fire", 0.5);
    }));
    assert!(panics(|| {
        let _ = Penetration::new().with_points("fire", f32::NAN);
    }));
    assert!(panics(|| {
        let _ = Conversion::new().with_rule("fire", "fire", 0.5);
    }));
    assert!(panics(|| {
        let _ = Conversion::new().with_rule("fire", "ice", 2.0);
    }));

    // The scene forms' names: penetration keys and `from` may be ancestors,
    // `to` must be a type.
    let (mut world, _) = rig(&names(&["elemental.fire", "elemental.cold", "physical"]));
    let entry: DamageTypes =
        serde_json::from_value(json!({ "elemental.fire": {}, "elemental.cold": {} })).unwrap();
    world.resource_mut::<DataRegistry>().insert("game", entry);
    let components = world.resource::<ComponentRegistry>();
    let check = |name: &str, value: Value| {
        components
            .check_in(name, &value, &world)
            .map_err(|e| e.to_string())
    };
    assert!(
        check(
            PENETRATION,
            json!({ "elemental": 0.5, "elemental.fire": 1 })
        )
        .is_ok()
    );
    assert_eq!(
        check(PENETRATION, json!({ "physical": 0.5 })).unwrap_err(),
        "component `penetration`, field `physical`: unknown damage type `physical`"
    );
    assert!(
        check(
            CONVERSION,
            json!([{ "from": "elemental", "to": "elemental.cold", "pct": 0.5 }])
        )
        .is_ok()
    );
    assert_eq!(
        check(
            CONVERSION,
            json!([{ "from": "elemental.fire", "to": "elemental", "pct": 0.5 }])
        )
        .unwrap_err(),
        "component `conversion`, field `0.to`: unknown damage type `elemental`"
    );
    assert_eq!(
        check(
            CONVERSION,
            json!([{ "from": "physical", "to": "elemental.fire", "pct": 0.5 }])
        )
        .unwrap_err(),
        "component `conversion`, field `0.from`: unknown damage type `physical`"
    );
}

#[test]
fn penetration_and_conversion_have_rows_and_round_trip_as_data() {
    let pen = Penetration::new().with_points("elemental.fire", 0.5);
    assert_eq!(
        serde_json::to_value(&pen).unwrap(),
        json!({ "elemental.fire": 0.5 })
    );
    assert_eq!(
        pen.inspect_rows(),
        [("penetration", "elemental.fire 0.5".to_string())]
    );
    let frost = Conversion::new()
        .with_rule("physical", "elemental.cold", 0.5)
        .with_rule("elemental", "chaos", 1.0);
    assert_eq!(
        serde_json::from_value::<Conversion>(serde_json::to_value(&frost).unwrap()).unwrap(),
        frost
    );
    assert_eq!(
        frost.inspect_rows(),
        [(
            "conversion",
            "physical → elemental.cold × 0.5, elemental → chaos × 1".to_string()
        )]
    );
    assert!(DamageKinds::NONE.names(&World::new()).is_empty());
    assert!(!DamageKinds::NONE.has(&World::new(), "fire"));
}

#[test]
fn a_parts_definition_is_its_deepest_type_ties_to_the_first_name() {
    let (mut world, _) = rig(&names(&["a.b.c", "z.y", "m.b", "m.a"]));
    let entry: DamageTypes = serde_json::from_value(
        json!({ "a": {}, "a.b": {}, "a.b.c": {}, "z.y": {}, "m.b": {}, "m.a": {} }),
    )
    .unwrap();
    world.resource_mut::<DataRegistry>().insert("game", entry);
    merge_types(&mut world);
    let table = world.resource::<DamageTypeTable>();
    let deep = table.mask(&Tags::new(["a.b.c", "z.y"]));
    let tie = table.mask(&Tags::new(["m.b", "m.a"]));
    assert_eq!(deep.names(&world), ["a", "a.b", "a.b.c", "z.y"]);
    assert_eq!(definition(&world, deep), Some("a.b.c"), "the deepest");
    assert_eq!(
        definition(&world, tie),
        Some("m.a"),
        "the first by name at one depth"
    );
    assert_eq!(definition(&world, DamageKinds::NONE), None);
}

#[test]
fn a_rung_with_needs_and_no_window_fails_the_load() {
    let (mut world, mut schedule) = rig(&names(&["elemental.cold", "status.chilled"]));
    crate::EffectsPlugin.build(&mut schedule, &mut world);
    world
        .resource_mut::<tungsten_core::DataRegistry>()
        .insert("chilled", crate::Effect::new().with_duration(1.0));
    let files = Files::new("rung");
    let write = |file: &str, text: &str| {
        let path = files.0.join(file);
        std::fs::write(&path, text).unwrap();
        path
    };
    let no_window = write(
        "no_window.json",
        r#"{ "elemental.cold": { "ailments": [{ "effect": "chilled", "needs": 0.5 }] } }"#,
    );
    let error = load(&mut world, &[("cold", &no_window)]).unwrap_err();
    assert!(error.contains("window: required with `needs`"), "{error}");
    let missing = write(
        "missing.json",
        r#"{ "elemental.cold": { "ailments": [{ "effect": "frosted", "window": 1 }] } }"#,
    );
    let error = load(&mut world, &[("cold", &missing)]).unwrap_err();
    assert!(
        error.contains("effects `frosted`") && error.contains("elemental.cold.ailments.0.effect"),
        "{error}"
    );
    let undeclared = write(
        "undeclared.json",
        r#"{ "elemental.cold": { "ailments": [{ "effect": "chilled", "requires": ["status.wet"] }] } }"#,
    );
    let error = load(&mut world, &[("cold", &undeclared)]).unwrap_err();
    assert!(
        error.contains("tag `status.wet`") && error.contains("ailments.0.requires.0"),
        "{error}"
    );
    let good = write(
        "good.json",
        r#"{ "elemental.cold": { "ailments": [{ "effect": "chilled", "needs": 0.5, "requires": ["status.chilled"], "window": 4 }] } }"#,
    );
    assert!(load(&mut world, &[("cold", &good)]).is_ok());
    merge_types(&mut world);
    let (name, rungs) = ladder(
        &world,
        world
            .resource::<DamageTypeTable>()
            .mask_of("elemental.cold"),
    )
    .unwrap();
    assert_eq!((name, rungs.len()), ("elemental.cold", 1));
    assert_eq!(rungs[0].window(), Some(4.0));
    assert!(ladder_of(&world, "elemental").is_none());
}
