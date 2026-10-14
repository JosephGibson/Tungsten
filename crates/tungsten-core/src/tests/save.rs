use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

fn tempdir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "tungsten_save_test_{}_{}",
        std::process::id(),
        nonce
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Slots in a `saves` folder not yet created.
fn slots() -> SaveSlots {
    SaveSlots::new(Some(tempdir().join("saves")))
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Progress {
    level: u32,
    name: String,
}

impl SaveData for Progress {
    const SCHEMA: u32 = 2;
}

fn progress() -> Progress {
    Progress {
        level: 3,
        name: "ada".into(),
    }
}

fn write_slot(slots: &SaveSlots, slot: &str, text: &str) -> PathBuf {
    let dir = slots.dir().unwrap();
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(format!("{slot}.json"));
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn save_then_load_round_trips() {
    let slots = slots();
    slots.save("slot-1", &progress()).unwrap();
    assert_eq!(slots.load::<Progress>("slot-1").unwrap(), progress());

    let text = std::fs::read_to_string(slots.dir().unwrap().join("slot-1.json")).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(envelope["envelope"], 1);
    assert_eq!(envelope["slot"], "slot-1");
    assert_eq!(envelope["schema"], 2);
    assert_eq!(envelope["engine_version"], env!("CARGO_PKG_VERSION"));
    assert!(envelope["saved_at_ms"].as_u64().unwrap() > 0);
    assert_eq!(
        envelope["data"],
        serde_json::json!({ "level": 3, "name": "ada" })
    );
    assert_eq!(now_ms(UNIX_EPOCH - std::time::Duration::from_secs(1)), 0);
}

#[test]
fn list_reads_headers_sorted_and_skips_temp_files() {
    let slots = slots();
    assert!(
        slots.list().unwrap().slots.is_empty(),
        "no folder yet, no slots"
    );
    slots.save("b.second", &progress()).unwrap();
    slots.save("a_first", &progress()).unwrap();
    let dir = slots.dir().unwrap();
    std::fs::write(dir.join(".a_first.json.1.0.tmp"), "half").unwrap();
    std::fs::write(dir.join(".hidden.json"), "{}").unwrap();
    std::fs::write(dir.join("notes.txt"), "not a slot").unwrap();
    std::fs::write(dir.join("broken.json"), "{ nope").unwrap();

    let list = slots.list().unwrap();
    let names: Vec<&str> = list.slots.iter().map(|info| info.name.as_str()).collect();
    assert_eq!(names, ["a_first", "b.second"]);
    let first = &list.slots[0];
    assert_eq!(first.schema, 2);
    assert_eq!(first.engine_version, env!("CARGO_PKG_VERSION"));
    assert!(first.saved_at_ms > 0);
    assert_eq!(list.warnings.len(), 1, "{:?}", list.warnings);
    assert!(list.warnings[0].contains("broken"), "{:?}", list.warnings);
}

#[test]
fn a_missing_slot_is_not_found() {
    let slots = slots();
    assert!(matches!(
        slots.load::<Progress>("nothing"),
        Err(SaveError::NotFound { slot }) if slot == "nothing"
    ));
}

#[test]
fn a_corrupt_slot_is_corrupt_and_left_unchanged() {
    let slots = slots();
    for text in [
        "{ \"envelope\": 1, \"schema\": ",
        "[1, 2]",
        r#"{ "envelope": 1, "slot": "x", "schema": 2, "engine_version": "0.73.0", "saved_at_ms": 5, "data": { "level": "three" } }"#,
    ] {
        let path = write_slot(&slots, "x", text);
        let error = slots.load::<Progress>("x").unwrap_err();
        assert!(matches!(error, SaveError::Corrupt { .. }), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
}

#[test]
fn a_newer_schema_is_newer_schema() {
    let slots = slots();
    let text = r#"{ "envelope": 1, "slot": "x", "schema": 7, "engine_version": "9.0.0", "saved_at_ms": 5, "data": {} }"#;
    let path = write_slot(&slots, "x", text);
    let error = slots.load::<Progress>("x").unwrap_err();
    assert!(
        matches!(
            error,
            SaveError::NewerSchema {
                found: 7,
                supported: 2,
                ..
            }
        ),
        "{error}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}

#[test]
fn an_unknown_envelope_is_unknown_envelope() {
    let slots = slots();
    write_slot(
        &slots,
        "x",
        r#"{ "envelope": 2, "header": {}, "payload": [] }"#,
    );
    let error = slots.load::<Progress>("x").unwrap_err();
    assert!(
        matches!(error, SaveError::UnknownEnvelope { envelope: 2, .. }),
        "{error}"
    );
    assert_eq!(slots.list().unwrap().warnings.len(), 1);
}

#[test]
fn invalid_names_are_refused() {
    let slots = slots();
    let long = "a".repeat(65);
    for name in [
        "Slot1",
        "a/b",
        r"a\b",
        "..",
        ".x",
        "con",
        "con.save",
        long.as_str(),
        "",
    ] {
        let invalid = |result: Result<(), SaveError>| matches!(result, Err(SaveError::InvalidName { name: given }) if given == name);
        assert!(invalid(slots.save(name, &progress())), "{name:?}");
        assert!(invalid(slots.load::<Progress>(name).map(drop)), "{name:?}");
        assert!(invalid(slots.delete(name)), "{name:?}");
    }
    assert!(slots.save(&"a".repeat(64), &progress()).is_ok());
    assert!(slots.save("0-autosave_2.v1", &progress()).is_ok());
}

#[test]
fn without_a_folder_every_call_is_no_folder() {
    let slots = SaveSlots::default();
    assert_eq!(slots.dir(), None);
    assert!(matches!(
        slots.save("a", &progress()),
        Err(SaveError::NoFolder)
    ));
    assert!(matches!(
        slots.load::<Progress>("a"),
        Err(SaveError::NoFolder)
    ));
    assert!(matches!(slots.list(), Err(SaveError::NoFolder)));
    assert!(matches!(slots.delete("a"), Err(SaveError::NoFolder)));
}

#[test]
fn delete_removes_the_slot() {
    let slots = slots();
    slots.save("gone", &progress()).unwrap();
    slots.save("kept", &progress()).unwrap();
    slots.delete("gone").unwrap();
    assert!(matches!(
        slots.load::<Progress>("gone"),
        Err(SaveError::NotFound { .. })
    ));
    assert!(matches!(
        slots.delete("gone"),
        Err(SaveError::NotFound { .. })
    ));
    let names: Vec<String> = slots
        .list()
        .unwrap()
        .slots
        .into_iter()
        .map(|i| i.name)
        .collect();
    assert_eq!(names, ["kept"]);
}
