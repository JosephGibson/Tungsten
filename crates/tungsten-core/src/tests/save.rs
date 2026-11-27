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
fn a_slot_that_cannot_be_read_is_io_not_not_found() {
    let slots = slots();
    // A folder where the slot's file goes: reading it fails, and not as a
    // missing file (a folder on Unix, access denied on Windows).
    let path = slots.dir().unwrap().join("x.json");
    std::fs::create_dir_all(&path).unwrap();
    let error = slots.load::<Progress>("x").unwrap_err();
    assert!(
        matches!(
            &error,
            SaveError::Io { path: failed, source }
                if *failed == path && source.kind() != std::io::ErrorKind::NotFound
        ),
        "{error}"
    );
}

#[test]
fn slots_whose_folder_is_a_file_fail_to_list_as_io() {
    let file = tempdir().join("saves");
    std::fs::write(&file, "a file").unwrap();
    let error = SaveSlots::new(Some(file.clone())).list().unwrap_err();
    assert!(
        matches!(
            &error,
            SaveError::Io { path, source }
                if *path == file && source.kind() != std::io::ErrorKind::NotFound
        ),
        "{error}"
    );
}

#[test]
fn data_that_does_not_serialize_is_serialize_and_writes_nothing() {
    let slots = slots();
    // `serde_json` refuses a map key that is not a string.
    let data = std::collections::BTreeMap::from([((1_u8, 2_u8), 3_u8)]);
    let error = slots
        .save_stamped("x", &data, &SaveStamp::new(1, "0.1.0", 5))
        .unwrap_err();
    assert!(
        matches!(&error, SaveError::Serialize { slot, .. } if slot == "x"),
        "{error}"
    );
    assert!(
        !slots.dir().unwrap().exists(),
        "it fails before the folder is made"
    );
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

/// Schema 3, its steps each adding one field from the last: schema 1 had
/// `a`, 2 added `b`, 3 added `c`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Stepped {
    a: u32,
    b: u32,
    c: u32,
}

impl SaveData for Stepped {
    const SCHEMA: u32 = 3;

    fn migrate_step(
        from: u32,
        mut data: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        match from {
            1 => data["b"] = (data["a"].as_u64().ok_or("no a")? + 1).into(),
            2 => data["c"] = (data["b"].as_u64().ok_or("no b")? * 10).into(),
            _ => return Err(format!("no step from {from}").into()),
        }
        Ok(data)
    }
}

/// Schema 3 with its own `migrate`, straight from 1, and no steps.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Jumped {
    a: u32,
    c: u32,
}

impl SaveData for Jumped {
    const SCHEMA: u32 = 3;

    fn migrate(
        from: u32,
        mut data: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        if from != 1 {
            return Err(format!("no migration from {from}").into());
        }
        data["c"] = 7.into();
        Ok(data)
    }
}

#[test]
fn migrate_steps_a_schema_at_a_time_by_default() {
    let slots = slots();
    slots
        .save_stamped(
            "one",
            &serde_json::json!({ "a": 1 }),
            &SaveStamp::new(1, "0.1.0", 5),
        )
        .unwrap();
    slots
        .save_stamped(
            "two",
            &serde_json::json!({ "a": 1, "b": 4 }),
            &SaveStamp::new(2, "0.1.0", 5),
        )
        .unwrap();
    assert_eq!(
        slots.load::<Stepped>("one").unwrap(),
        Stepped { a: 1, b: 2, c: 20 },
        "1 to 2, then 2 to 3"
    );
    assert_eq!(
        slots.load::<Stepped>("two").unwrap(),
        Stepped { a: 1, b: 4, c: 40 },
        "2 to 3 alone"
    );
    slots
        .save_stamped(
            "zero",
            &serde_json::json!({}),
            &SaveStamp::new(0, "0.1.0", 5),
        )
        .unwrap();
    let err = slots.load::<Stepped>("zero").unwrap_err();
    assert!(
        matches!(&err, SaveError::Migration { from: 0, .. }),
        "{err}"
    );
    assert!(err.to_string().contains("no step from 0"), "{err}");
    assert_eq!(
        Progress::migrate(1, serde_json::json!({}))
            .unwrap_err()
            .to_string(),
        "schema 1 has no migration to schema 2",
        "the default step refuses"
    );
}

#[test]
fn a_type_that_overrides_migrate_keeps_its_own() {
    let slots = slots();
    slots
        .save_stamped(
            "old",
            &serde_json::json!({ "a": 2 }),
            &SaveStamp::new(1, "0.1.0", 5),
        )
        .unwrap();
    assert_eq!(slots.load::<Jumped>("old").unwrap(), Jumped { a: 2, c: 7 });
    assert!(
        Jumped::migrate_step(1, serde_json::json!({})).is_err(),
        "its steps are the default's"
    );
}

#[test]
fn a_migration_whose_result_does_not_fit_the_type_is_migration() {
    let slots = slots();
    // Schema 2's step to 3 runs, and leaves an `a` that is no number.
    slots
        .save_stamped(
            "misfit",
            &serde_json::json!({ "a": "one", "b": 4 }),
            &SaveStamp::new(2, "0.1.0", 5),
        )
        .unwrap();
    let path = slots.dir().unwrap().join("misfit.json");
    let before = std::fs::read(&path).unwrap();
    let error = slots.load::<Stepped>("misfit").unwrap_err();
    // The type's own refusal, not a step's.
    assert!(
        matches!(
            &error,
            SaveError::Migration { slot, from: 2, source }
                if slot == "misfit" && source.is::<serde_json::Error>()
        ),
        "{error}"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the file is unchanged"
    );
}

#[test]
fn a_stamped_save_lists_and_loads_with_its_stamp() {
    let slots = slots();
    let stamp = SaveStamp::new(1, "0.50.0", 1_700_000_000_000);
    slots
        .save_stamped("older", &serde_json::json!({ "a": 3 }), &stamp)
        .unwrap();
    let list = slots.list().unwrap();
    assert_eq!(
        list.slots,
        [SlotInfo {
            name: "older".into(),
            schema: 1,
            engine_version: "0.50.0".into(),
            saved_at_ms: 1_700_000_000_000,
        }]
    );
    assert_eq!(
        slots.load::<Stepped>("older").unwrap(),
        Stepped { a: 3, b: 4, c: 40 }
    );

    slots
        .save_stamped(
            "newer",
            &serde_json::json!({ "a": 3 }),
            &SaveStamp::new(9, "9.0.0", 0),
        )
        .unwrap();
    assert!(matches!(
        slots.load::<Stepped>("newer"),
        Err(SaveError::NewerSchema {
            found: 9,
            supported: 3,
            ..
        })
    ));
}

// S-191: `list` reads a slot's header up to its `data` key.

/// A reader that counts the bytes it hands over.
struct Counting<R> {
    inner: R,
    handed: usize,
}

impl<R: std::io::Read> std::io::Read for Counting<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.handed += read;
        Ok(read)
    }
}

#[derive(Serialize, Deserialize)]
struct Heavy {
    level: u32,
    blob: String,
}

impl SaveData for Heavy {
    const SCHEMA: u32 = 5;
}

#[test]
fn a_slots_header_reads_without_its_data() {
    let slots = slots();
    let heavy = Heavy {
        level: 1,
        blob: "x".repeat(1 << 20),
    };
    slots.save("heavy", &heavy).unwrap();
    let path = slots.dir().unwrap().join("heavy.json");
    assert!(std::fs::metadata(&path).unwrap().len() > 1 << 20);

    let mut file = Counting {
        inner: std::fs::File::open(&path).unwrap(),
        handed: 0,
    };
    let header = header_before_data(&mut file).expect("an engine-written slot");
    assert_eq!(
        (header.schema, header.engine_version.as_str()),
        (5, env!("CARGO_PKG_VERSION"))
    );
    assert!(header.saved_at_ms > 0);
    assert!(
        file.handed > 0 && file.handed <= 16 * 1024,
        "{} bytes read of a slot over 1 MiB",
        file.handed
    );
    // `list` reads it the same way, and the data still loads.
    assert_eq!(
        slots.list().unwrap().slots,
        [SlotInfo {
            name: "heavy".into(),
            schema: 5,
            engine_version: env!("CARGO_PKG_VERSION").into(),
            saved_at_ms: header.saved_at_ms,
        }]
    );
    assert_eq!(slots.load::<Heavy>("heavy").unwrap().blob.len(), 1 << 20);
}

#[test]
fn list_answers_as_the_whole_parse_does() {
    let slots = slots();
    slots.save("a_first", &progress()).unwrap();
    slots
        .save_stamped("older", &progress(), &SaveStamp::new(1, "0.70.0", 123))
        .unwrap();
    let dir = slots.dir().unwrap().to_path_buf();
    // Whether the header reads before the data, and the file.
    let files = [
        (true, "a_first", None),
        (true, "older", None),
        (
            false,
            "data-first",
            Some(
                r#"{ "data": { "level": 3 }, "envelope": 1, "slot": "data-first", "schema": 2, "engine_version": "0.73.0", "saved_at_ms": 5 }"#,
            ),
        ),
        (
            false,
            "data-early",
            Some(
                r#"{ "envelope": 1, "schema": 2, "data": {}, "engine_version": "0.1.0", "saved_at_ms": 7 }"#,
            ),
        ),
        (
            false,
            "no-data",
            Some(r#"{ "envelope": 1, "schema": 2, "engine_version": "0.73.0", "saved_at_ms": 5 }"#),
        ),
        (
            true,
            "twice",
            Some(
                r#"{ "envelope": 1, "schema": 1, "schema": 3, "engine_version": "a", "saved_at_ms": 5, "data": {} }"#,
            ),
        ),
        (
            false,
            "newer",
            Some(
                r#"{ "envelope": 2, "slot": "newer", "schema": 2, "engine_version": "9.0.0", "saved_at_ms": 5, "data": {} }"#,
            ),
        ),
        (
            false,
            "newer-shape",
            Some(r#"{ "envelope": 2, "header": {}, "payload": [] }"#),
        ),
        (
            false,
            "mistyped",
            Some(
                r#"{ "envelope": 1, "schema": "two", "engine_version": "0.73.0", "saved_at_ms": 5, "data": {} }"#,
            ),
        ),
        (false, "no-json", Some("{ nope")),
        (false, "a-list", Some("[1, 2]")),
        (false, "empty", Some("")),
        (
            false,
            "Shouting",
            Some(
                r#"{ "envelope": 1, "schema": 2, "engine_version": "0.73.0", "saved_at_ms": 5, "data": {} }"#,
            ),
        ),
    ];
    for (_, name, text) in &files {
        if let Some(text) = text {
            write_slot(&slots, name, text);
        }
    }
    std::fs::write(dir.join(".a_first.json.1.0.tmp"), "half").unwrap();
    std::fs::write(dir.join(".hidden.json"), "{}").unwrap();
    std::fs::write(dir.join("notes.txt"), "not a slot").unwrap();

    let list = slots.list().unwrap();
    let whole = slots.list_by(read_header_whole).unwrap();
    assert_eq!(list.slots, whole.slots);
    assert_eq!(list.warnings, whole.warnings);
    let names: Vec<&str> = list.slots.iter().map(|info| info.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "a_first",
            "data-early",
            "data-first",
            "no-data",
            "older",
            "twice"
        ]
    );
    assert_eq!(list.slots[5].schema, 3, "a key given twice keeps its last");
    assert_eq!(list.warnings.len(), 7, "{:?}", list.warnings);
    for (before_data, name, _) in files {
        let file = std::fs::File::open(dir.join(format!("{name}.json"))).unwrap();
        // The read alone: an invalid name never reaches it.
        let read = header_before_data(file).is_some();
        assert_eq!(read, before_data || name == "Shouting", "{name}");
    }
}

#[test]
fn a_slot_with_a_whole_header_and_broken_data_lists_and_fails_to_load() {
    let slots = slots();
    // Torn in its data, and whole with something after it.
    for text in [
        r#"{ "envelope": 1, "slot": "torn", "schema": 2, "engine_version": "0.73.0", "saved_at_ms": 5, "data": { "level": 3, "name": "a"#,
        r#"{ "envelope": 1, "slot": "torn", "schema": 2, "engine_version": "0.73.0", "saved_at_ms": 5, "data": { "level": 3, "name": "ada" } } trailing"#,
    ] {
        write_slot(&slots, "torn", text);
        let list = slots.list().unwrap();
        assert!(list.warnings.is_empty(), "{:?}", list.warnings);
        assert_eq!(
            list.slots,
            [SlotInfo {
                name: "torn".into(),
                schema: 2,
                engine_version: "0.73.0".into(),
                saved_at_ms: 5,
            }]
        );
        // The whole parse skipped it, and `load` still reports it.
        let whole = slots.list_by(read_header_whole).unwrap();
        assert!(whole.slots.is_empty() && whole.warnings.len() == 1);
        let error = slots.load::<Progress>("torn").unwrap_err();
        assert!(matches!(error, SaveError::Corrupt { .. }), "{error}");
    }
}

// --- Saves written behind (`D-303`) ---

use std::sync::mpsc;
use std::time::Duration;

/// The longest a held writer waits for its test to let it go.
const HELD: Duration = Duration::from_secs(10);

/// The file system's own writes and renames, with the writer held where a
/// test asks: a sync reports itself on `syncing` and waits for a token on
/// `sync_gate`, then fails, or unwinds as a panic does (`panic_sync`); a
/// rename fails with `rename_errors` in turn; a sleep reports its length on
/// `sleeping` and waits for a token on `sleep_gate`.
struct HeldOps {
    syncing: Option<mpsc::Sender<()>>,
    sync_gate: Option<mpsc::Receiver<()>>,
    fail_sync: bool,
    panic_sync: bool,
    windows: bool,
    rename_errors: Vec<i32>,
    sleeping: Option<mpsc::Sender<Duration>>,
    sleep_gate: Option<mpsc::Receiver<()>>,
}

impl HeldOps {
    fn free() -> Self {
        Self {
            syncing: None,
            sync_gate: None,
            fail_sync: false,
            panic_sync: false,
            windows: false,
            rename_errors: Vec::new(),
            sleeping: None,
            sleep_gate: None,
        }
    }
}

impl Ops for HeldOps {
    fn windows(&self) -> bool {
        self.windows
    }

    fn write(&mut self, file: &mut std::fs::File, contents: &[u8]) -> std::io::Result<()> {
        RealOps.write(file, contents)
    }

    fn sync(&mut self, file: &mut std::fs::File) -> std::io::Result<()> {
        if let Some(syncing) = &self.syncing {
            let _ = syncing.send(());
        }
        if let Some(gate) = &self.sync_gate {
            let _ = gate.recv_timeout(HELD);
        }
        if self.panic_sync {
            // A panic's unwind without the panic hook, so the run prints no
            // panic message.
            std::panic::resume_unwind(Box::new("injected writer panic"));
        }
        if self.fail_sync {
            return Err(std::io::Error::other("injected sync failure"));
        }
        RealOps.sync(file)
    }

    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        if !self.rename_errors.is_empty() {
            return Err(std::io::Error::from_raw_os_error(
                self.rename_errors.remove(0),
            ));
        }
        std::fs::rename(from, to)
    }

    fn sleep(&mut self, duration: Duration) {
        if let Some(sleeping) = &self.sleeping {
            let _ = sleeping.send(duration);
        }
        if let Some(gate) = &self.sleep_gate {
            let _ = gate.recv_timeout(HELD);
        }
    }
}

/// Slots that write behind, their writer committing through `ops`.
fn behind(ops: HeldOps) -> SaveSlots {
    let mut slots = slots();
    slots.write_behind();
    *lock(&slots.behind.as_ref().unwrap().ops) = Some(Box::new(ops));
    slots
}

/// The names in the slots' folder, temp files included.
fn files(slots: &SaveSlots) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(slots.dir().unwrap()) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn pending(slots: &SaveSlots) -> usize {
    lock(&slots.behind.as_ref().unwrap().shared.state)
        .pending
        .len()
}

#[test]
fn slots_write_behind_only_when_asked_and_only_with_a_folder() {
    let mut none = SaveSlots::new(None);
    none.write_behind();
    assert!(!none.writes_behind(), "no folder, nothing to write");
    none.flush().unwrap();

    let mut some = slots();
    assert!(
        !some.writes_behind(),
        "a save is whole before it returns unless asked"
    );
    some.flush().unwrap();
    some.write_behind();
    assert!(some.writes_behind());
    // A clone shares the writer, so it reads what the other saved.
    let clone = some.clone();
    some.save("slot-1", &progress()).unwrap();
    assert_eq!(clone.load::<Progress>("slot-1").unwrap(), progress());
    clone.flush().unwrap();
    assert_eq!(files(&some), ["slot-1.json"]);
}

#[test]
fn a_save_behind_returns_before_its_commit_and_reads_back_at_once() {
    let (release, sync_gate) = mpsc::channel();
    let slots = behind(HeldOps {
        sync_gate: Some(sync_gate),
        ..HeldOps::free()
    });
    let stamp = SaveStamp::new(2, "0.1.0", 1_760_400_000_000);
    slots.save_stamped("slot-1", &progress(), &stamp).unwrap();

    // The writer is held at its sync: the save is a temp file, and the
    // slot's own file is not there yet.
    let slot_file = slots.dir().unwrap().join("slot-1.json");
    assert!(!slot_file.exists());
    assert_eq!(files(&slots).len(), 1);
    assert_eq!(pending(&slots), 1);
    // It reads back all the same, and lists with its header.
    assert_eq!(slots.load::<Progress>("slot-1").unwrap(), progress());
    let list = slots.list().unwrap();
    assert!(list.warnings.is_empty(), "{:?}", list.warnings);
    assert_eq!(list.slots.len(), 1);
    assert_eq!(list.slots[0].name, "slot-1");
    assert_eq!(list.slots[0].schema, 2);
    assert_eq!(list.slots[0].saved_at_ms, 1_760_400_000_000);

    release.send(()).unwrap();
    slots.flush().unwrap();
    assert_eq!(
        files(&slots),
        ["slot-1.json"],
        "in place, its temp file gone"
    );
    assert_eq!(pending(&slots), 0);
    assert_eq!(slots.load::<Progress>("slot-1").unwrap(), progress());
    assert_eq!(slots.list().unwrap().slots.len(), 1);

    // The file is the one a save written before it returns leaves.
    let whole = self::slots();
    whole.save_stamped("slot-1", &progress(), &stamp).unwrap();
    assert_eq!(
        std::fs::read(&slot_file).unwrap(),
        std::fs::read(whole.dir().unwrap().join("slot-1.json")).unwrap()
    );
}

#[test]
fn a_save_that_cannot_be_staged_fails_at_once_and_queues_nothing() {
    // The slots' folder is a file: it cannot be made.
    let blocked = tempdir().join("saves");
    std::fs::write(&blocked, "a file").unwrap();
    let mut slots = SaveSlots::new(Some(blocked));
    slots.write_behind();
    let error = slots.save("slot-1", &progress()).unwrap_err();
    assert!(
        matches!(error, SaveError::Write(WriteError::Io { .. })),
        "{error}"
    );
    // And what failed before it could be serialized fails as it did.
    assert!(matches!(
        slots.save("Not A Name", &progress()),
        Err(SaveError::InvalidName { .. })
    ));
    assert_eq!(pending(&slots), 0);
    slots.flush().unwrap();
}

#[test]
fn a_commit_that_fails_keeps_the_old_file_and_is_the_flushs_error() {
    let slots = behind(HeldOps {
        fail_sync: true,
        ..HeldOps::free()
    });
    let old = Progress {
        level: 1,
        name: "old".into(),
    };
    SaveSlots::new(slots.dir().map(Path::to_path_buf))
        .save("slot-1", &old)
        .unwrap();

    slots.save("slot-1", &progress()).unwrap();
    let error = slots.flush().unwrap_err();
    assert!(
        matches!(error, SaveError::Write(WriteError::Io { .. })),
        "{error}"
    );
    assert_eq!(slots.load::<Progress>("slot-1").unwrap(), old);
    assert_eq!(files(&slots), ["slot-1.json"], "the temp file is removed");
    assert_eq!(pending(&slots), 0);
    slots.flush().expect("a failure is reported once");
}

#[test]
fn delete_cancels_a_save_not_yet_in_place() {
    let (syncing, in_sync) = mpsc::channel();
    let (release, sync_gate) = mpsc::channel();
    let slots = behind(HeldOps {
        syncing: Some(syncing),
        sync_gate: Some(sync_gate),
        ..HeldOps::free()
    });
    slots.save("slot-1", &progress()).unwrap();
    // The writer holds the save, inside its sync: the delete reaches it
    // between the sync and the rename.
    in_sync
        .recv_timeout(HELD)
        .expect("the writer syncs the save");
    slots
        .delete("slot-1")
        .expect("a slot saved and not yet in place is there to delete");
    assert!(matches!(
        slots.load::<Progress>("slot-1"),
        Err(SaveError::NotFound { .. })
    ));
    assert!(slots.list().unwrap().slots.is_empty());

    release.send(()).unwrap();
    slots.flush().unwrap();
    assert!(
        files(&slots).is_empty(),
        "the writer dropped the cancelled save"
    );
    assert!(matches!(
        slots.delete("slot-1"),
        Err(SaveError::NotFound { .. })
    ));
}

#[test]
fn of_two_saves_of_one_slot_the_newer_stands() {
    let (syncing, in_sync) = mpsc::channel();
    let (release, sync_gate) = mpsc::channel();
    let slots = behind(HeldOps {
        syncing: Some(syncing),
        sync_gate: Some(sync_gate),
        ..HeldOps::free()
    });
    let first = Progress {
        level: 1,
        name: "first".into(),
    };
    slots.save("slot-1", &first).unwrap();
    // The writer holds the first save inside its sync when the second is
    // made, so the first is dropped after its sync, never renamed in.
    in_sync
        .recv_timeout(HELD)
        .expect("the writer syncs the first save");
    assert_eq!(slots.load::<Progress>("slot-1").unwrap(), first);
    slots.save("slot-1", &progress()).unwrap();
    assert_eq!(slots.load::<Progress>("slot-1").unwrap(), progress());
    assert_eq!(files(&slots).len(), 2, "two temp files, no slot file yet");

    release.send(()).unwrap();
    in_sync
        .recv_timeout(HELD)
        .expect("the writer syncs the second save");
    // The first save's temp file went with it: the slot's file was never
    // the first save's.
    assert_eq!(files(&slots).len(), 1);
    assert!(!slots.dir().unwrap().join("slot-1.json").exists());
    release.send(()).unwrap();
    slots.flush().unwrap();
    assert_eq!(slots.load::<Progress>("slot-1").unwrap(), progress());
    assert_eq!(
        files(&slots),
        ["slot-1.json"],
        "the older save's temp file is gone"
    );
}

#[test]
fn a_load_returns_while_a_renames_retry_sleeps() {
    let (sleeping, slept) = mpsc::channel();
    let (wake, sleep_gate) = mpsc::channel();
    let slots = behind(HeldOps {
        windows: true,
        rename_errors: vec![32],
        sleeping: Some(sleeping),
        sleep_gate: Some(sleep_gate),
        ..HeldOps::free()
    });
    slots.save("slot-1", &progress()).unwrap();
    let backoff = slept
        .recv_timeout(HELD)
        .expect("the refused rename sleeps before its retry");
    assert_eq!(backoff, Duration::from_millis(20));

    // The writer sleeps outside the lock a read takes.
    let reader = slots.clone();
    let (done, loaded) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(reader.load::<Progress>("slot-1"));
    });
    let loaded = loaded
        .recv_timeout(Duration::from_secs(5))
        .expect("a load returns while the retry sleeps");
    assert_eq!(loaded.unwrap(), progress());

    wake.send(()).unwrap();
    slots.flush().unwrap();
    assert_eq!(files(&slots), ["slot-1.json"]);
}

#[test]
fn dropping_the_slots_commits_what_is_staged() {
    let mut slots = slots();
    slots.write_behind();
    let dir = slots.dir().unwrap().to_path_buf();
    for slot in ["a", "b", "c"] {
        slots.save(slot, &progress()).unwrap();
    }
    let clone = slots.clone();
    drop(slots);
    // The last clone's drop waits for the writer.
    drop(clone);
    let whole = SaveSlots::new(Some(dir));
    assert_eq!(files(&whole), ["a.json", "b.json", "c.json"]);
    assert_eq!(whole.load::<Progress>("b").unwrap(), progress());
}

#[test]
fn slots_whose_writer_cannot_start_write_each_save_before_it_returns() {
    let mut slots = slots();
    slots.write_behind();
    Arc::get_mut(slots.behind.as_mut().unwrap())
        .unwrap()
        .no_thread = true;
    slots.save("slot-1", &progress()).unwrap();
    assert_eq!(files(&slots), ["slot-1.json"]);
    assert_eq!(pending(&slots), 0);
    assert_eq!(slots.load::<Progress>("slot-1").unwrap(), progress());
    slots.flush().unwrap();
}

#[test]
fn a_writer_that_dies_leaves_its_queue_to_the_flush_and_later_saves_commit_at_once() {
    let (syncing, in_sync) = mpsc::channel();
    let (release, sync_gate) = mpsc::channel();
    let slots = behind(HeldOps {
        syncing: Some(syncing),
        sync_gate: Some(sync_gate),
        panic_sync: true,
        ..HeldOps::free()
    });
    let dir = slots.dir().unwrap().to_path_buf();
    let old = Progress {
        level: 1,
        name: "old".into(),
    };
    SaveSlots::new(Some(dir.clone())).save("a", &old).unwrap();
    // A folder, not empty, where `d`'s file goes: its rename fails.
    let blocked = dir.join("d.json");
    std::fs::create_dir_all(&blocked).unwrap();
    std::fs::write(blocked.join("keep"), "").unwrap();

    slots.save("a", &progress()).unwrap();
    in_sync.recv_timeout(HELD).expect("the writer syncs `a`");
    // Staged behind the save the writer holds: `b` twice, the first
    // superseded, `d`, and `e`, deleted before its turn.
    let first = Progress {
        level: 2,
        name: "first".into(),
    };
    slots.save("b", &first).unwrap();
    slots.save("b", &progress()).unwrap();
    slots.save("d", &progress()).unwrap();
    slots.save("e", &progress()).unwrap();
    slots.delete("e").unwrap();
    assert_eq!(pending(&slots), 3, "a, b and d");

    // The writer unwinds inside `a`'s sync; the flush commits what it left,
    // in order, and reports `d`'s failure once.
    release.send(()).unwrap();
    let error = slots.flush().unwrap_err();
    assert!(
        matches!(
            &error,
            SaveError::Write(WriteError::Io { path, .. } | WriteError::Busy { path, .. })
                if *path == blocked
        ),
        "{error}"
    );
    slots.flush().expect("a failure is reported once");
    assert_eq!(pending(&slots), 0);
    assert_eq!(
        slots.load::<Progress>("a").unwrap(),
        old,
        "the held save is lost with its temp file, and the old file stands"
    );
    assert_eq!(
        slots.load::<Progress>("b").unwrap(),
        progress(),
        "the newer save of `b` stands"
    );
    std::fs::remove_dir_all(&blocked).unwrap();
    assert_eq!(
        files(&slots),
        ["a.json", "b.json"],
        "no temp file is left, and the deleted `e` is not written"
    );
    let list = slots.list().unwrap();
    assert!(list.warnings.is_empty(), "{:?}", list.warnings);
    let names: Vec<&str> = list.slots.iter().map(|info| info.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);

    // From here on a save is in place when it returns.
    slots.save("c", &progress()).unwrap();
    assert_eq!(pending(&slots), 0);
    assert_eq!(files(&slots), ["a.json", "b.json", "c.json"]);
    drop(slots);
    let whole = SaveSlots::new(Some(dir));
    assert_eq!(files(&whole), ["a.json", "b.json", "c.json"]);
    assert_eq!(whole.load::<Progress>("c").unwrap(), progress());
}

#[test]
fn a_save_made_after_the_writer_dies_supersedes_the_slots_staged_save() {
    let (syncing, in_sync) = mpsc::channel();
    let (release, sync_gate) = mpsc::channel();
    let slots = behind(HeldOps {
        syncing: Some(syncing),
        sync_gate: Some(sync_gate),
        panic_sync: true,
        ..HeldOps::free()
    });
    let old = Progress {
        level: 1,
        name: "old".into(),
    };
    slots.save("a", &progress()).unwrap();
    in_sync.recv_timeout(HELD).expect("the writer syncs `a`");
    // `b`'s save is staged behind the one the writer holds.
    slots.save("b", &old).unwrap();

    // The writer unwinds inside `a`'s sync and leaves `b`'s save staged.
    release.send(()).unwrap();
    let start = std::time::Instant::now();
    while !lock(&slots.behind.as_ref().unwrap().shared.state).dead {
        assert!(start.elapsed() < HELD, "the writer dies inside `a`'s sync");
        std::thread::sleep(Duration::from_millis(1));
    }

    // A newer save of `b`, committed where it is made, takes the staged
    // one's place.
    slots.save("b", &progress()).unwrap();
    assert_eq!(
        slots.load::<Progress>("b").unwrap(),
        progress(),
        "a load after the in-place save reads it, not the older staged save"
    );
    slots.flush().unwrap();
    assert_eq!(
        slots.load::<Progress>("b").unwrap(),
        progress(),
        "the flush does not rename the older staged save over the newer"
    );
    assert_eq!(
        files(&slots),
        ["b.json"],
        "the older save's temp file is gone, and `a`'s was lost with it"
    );
}

#[test]
fn the_slots_keep_their_auto_traits_with_a_writer_inside() {
    fn auto<T: Send + Sync + std::panic::UnwindSafe + std::panic::RefUnwindSafe>() {}
    auto::<SaveSlots>();
}
