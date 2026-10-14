use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

fn tempdir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "tungsten_persist_test_{}_{}",
        std::process::id(),
        nonce
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Real writes; the rename fails with `rename_errors` in turn, then renames.
struct FakeOps {
    windows: bool,
    fail_write: bool,
    rename_errors: Vec<i32>,
    renames: u32,
    sleeps: Vec<Duration>,
}

impl FakeOps {
    fn new(windows: bool) -> Self {
        Self {
            windows,
            fail_write: false,
            rename_errors: Vec::new(),
            renames: 0,
            sleeps: Vec::new(),
        }
    }
}

impl Ops for FakeOps {
    fn windows(&self) -> bool {
        self.windows
    }

    fn write(&mut self, file: &mut File, contents: &[u8]) -> std::io::Result<()> {
        if self.fail_write {
            // Half the bytes reach the temp file, then the sync fails.
            file.write_all(&contents[..contents.len() / 2])?;
            return Err(std::io::Error::other("injected sync failure"));
        }
        RealOps.write(file, contents)
    }

    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        self.renames += 1;
        if !self.rename_errors.is_empty() {
            return Err(std::io::Error::from_raw_os_error(
                self.rename_errors.remove(0),
            ));
        }
        std::fs::rename(from, to)
    }

    fn sleep(&mut self, duration: Duration) {
        self.sleeps.push(duration);
    }
}

#[test]
fn a_failure_before_the_rename_leaves_the_old_file() {
    let dir = tempdir();
    let path = dir.join("settings.json");
    std::fs::write(&path, "old").unwrap();
    let mut ops = FakeOps::new(false);
    ops.fail_write = true;
    let error = write_atomic_with(&path, b"the new contents", &mut ops).unwrap_err();
    assert!(matches!(error, WriteError::Io { .. }), "{error}");
    assert_eq!(ops.renames, 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
    assert_eq!(entries(&dir), ["settings.json"], "the temp file is removed");
}

#[test]
fn a_failed_rename_leaves_the_old_file_and_no_temp() {
    let dir = tempdir();
    let path = dir.join("settings.json");
    std::fs::write(&path, "old").unwrap();
    let mut ops = FakeOps::new(false);
    ops.rename_errors = vec![13];
    let error = write_atomic_with(&path, b"new", &mut ops).unwrap_err();
    assert!(matches!(error, WriteError::Io { .. }), "{error}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
    assert_eq!(entries(&dir), ["settings.json"]);
}

#[test]
fn a_windows_sharing_failure_retries_then_succeeds() {
    let dir = tempdir();
    let path = dir.join("settings.json");
    std::fs::write(&path, "old").unwrap();
    let mut ops = FakeOps::new(true);
    ops.rename_errors = vec![32, 5];
    write_atomic_with(&path, b"new", &mut ops).unwrap();
    assert_eq!(ops.renames, 3);
    assert_eq!(
        ops.sleeps,
        [Duration::from_millis(20), Duration::from_millis(40)]
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
    assert_eq!(entries(&dir), ["settings.json"]);
}

#[test]
fn a_windows_sharing_failure_that_persists_is_busy() {
    let dir = tempdir();
    let path = dir.join("settings.json");
    std::fs::write(&path, "old").unwrap();
    let mut ops = FakeOps::new(true);
    ops.rename_errors = vec![33; 10];
    let error = write_atomic_with(&path, b"new", &mut ops).unwrap_err();
    let WriteError::Busy {
        attempts, source, ..
    } = &error
    else {
        panic!("expected Busy, got {error}");
    };
    assert_eq!(*attempts, 5);
    assert_eq!(source.raw_os_error(), Some(33));
    assert_eq!(ops.renames, 5);
    assert_eq!(
        ops.sleeps,
        [20, 40, 80, 160].map(Duration::from_millis),
        "four sleeps, injected"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
    assert_eq!(entries(&dir), ["settings.json"]);
}

#[test]
fn other_errors_and_unix_codes_are_not_retried() {
    for code in [5, 32, 33] {
        assert!(is_sharing_error(true, Some(code)));
        assert!(!is_sharing_error(false, Some(code)));
    }
    for raw in [Some(2), Some(13), Some(183), None] {
        assert!(!is_sharing_error(true, raw));
    }

    let dir = tempdir();
    let path = dir.join("input.json");
    let mut ops = FakeOps::new(false);
    ops.rename_errors = vec![32, 32];
    let error = write_atomic_with(&path, b"new", &mut ops).unwrap_err();
    assert!(matches!(error, WriteError::Io { .. }), "{error}");
    assert_eq!(ops.renames, 1);
    assert!(ops.sleeps.is_empty());

    let mut ops = FakeOps::new(true);
    ops.rename_errors = vec![2, 2];
    let error = write_atomic_with(&path, b"new", &mut ops).unwrap_err();
    assert!(matches!(error, WriteError::Io { .. }), "{error}");
    assert_eq!(ops.renames, 1);
    assert!(!path.exists());
    assert!(entries(&dir).is_empty());
}

#[test]
fn the_folder_is_created() {
    let dir = tempdir();
    let path = dir.join("user").join("settings").join("settings.json");
    write_atomic(&path, b"{}").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"{}");
    write_atomic(&path, b"{ }").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"{ }");
    assert_eq!(entries(path.parent().unwrap()), ["settings.json"]);
}

#[test]
fn quarantine_replaces_an_older_backup() {
    let dir = tempdir();
    let path = dir.join("settings.json");
    std::fs::write(dir.join("settings.json.bad"), "older").unwrap();
    std::fs::write(&path, "corrupt").unwrap();
    let backup = quarantine(&path).unwrap();
    assert_eq!(backup, dir.join("settings.json.bad"));
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "corrupt");
    assert!(!path.exists());
    assert_eq!(entries(&dir), ["settings.json.bad"]);
}
