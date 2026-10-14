use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::{
    DEBOUNCE_MS, Inbox, WatchMessage, accept_path, canonical_or_clone, classify,
    under_recursive_root,
};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn tempdir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_hotreload_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::File::create(path).unwrap();
}

#[test]
fn accept_extra_file_matches_exact_path() {
    let dir = tempdir();
    let input_json = dir.join("input.json");
    touch(&input_json);

    let mut extras = HashSet::new();
    extras.insert(canonical_or_clone(&input_json));
    let roots: Vec<PathBuf> = Vec::new();

    assert!(accept_path(&input_json, &roots, &extras));
}

#[test]
fn accept_recursive_root_matches_nested_file() {
    let dir = tempdir();
    let assets = dir.join("assets");
    fs::create_dir_all(&assets).unwrap();
    let nested = assets.join("sprites").join("hero.png");
    touch(&nested);

    let roots = vec![canonical_or_clone(&assets)];
    let extras = HashSet::new();

    assert!(
        accept_path(&nested, &roots, &extras),
        "files under a recursive root must be accepted"
    );
}

#[test]
fn reject_sibling_file_in_extra_file_parent() {
    // Extra-file watch is exact, not parent-wide.
    let dir = tempdir();
    let input_json = dir.join("input.json");
    let tungsten_json = dir.join("tungsten.json");
    touch(&input_json);
    touch(&tungsten_json);

    let mut extras = HashSet::new();
    extras.insert(canonical_or_clone(&input_json));
    let roots: Vec<PathBuf> = Vec::new();

    assert!(
        !accept_path(&tungsten_json, &roots, &extras),
        "sibling of an extra-file entry must be filtered out"
    );
}

#[test]
fn reject_parent_directory_of_recursive_root() {
    // Recursive root match is directional.
    let dir = tempdir();
    let assets = dir.join("assets");
    fs::create_dir_all(&assets).unwrap();
    let outside = dir.join("other.txt");
    touch(&outside);

    let roots = vec![canonical_or_clone(&assets)];
    let extras = HashSet::new();

    assert!(
        !accept_path(&outside, &roots, &extras),
        "a sibling of the watched root must not match"
    );
}

#[test]
fn extra_file_under_a_recursive_root_needs_no_watch_of_its_own() {
    // A root manifest inside a watched asset directory is delivered by that
    // watch; one outside it needs its parent watched.
    let dir = tempdir();
    let assets = dir.join("assets");
    let manifest = assets.join("manifest.json");
    let input_json = dir.join("input.json");
    touch(&manifest);
    touch(&input_json);

    let roots = vec![canonical_or_clone(&assets)];

    assert!(under_recursive_root(&canonical_or_clone(&manifest), &roots));
    assert!(!under_recursive_root(
        &canonical_or_clone(&input_json),
        &roots
    ));
}

/// The messages one watcher callback sends.
fn messages(result: notify::Result<notify::Event>) -> Vec<WatchMessage> {
    let mut sent = Vec::new();
    classify(result, |message| sent.push(message));
    sent
}

#[test]
fn watcher_events_become_paths_errors_or_a_rescan() {
    use notify::event::{AccessKind, CreateKind, Event, EventKind, Flag, ModifyKind};
    let path = PathBuf::from("a.png");
    let changed = vec![WatchMessage::Changed(path.clone())];
    let modify = Event::new(EventKind::Modify(ModifyKind::Any)).add_path(path.clone());
    assert_eq!(messages(Ok(modify)), changed);
    let create = Event::new(EventKind::Create(CreateKind::File)).add_path(path);
    assert_eq!(messages(Ok(create)), changed);
    // An inotify queue overflow arrives as `Other` with the rescan flag.
    let overflow = Event::new(EventKind::Other).set_flag(Flag::Rescan);
    assert_eq!(messages(Ok(overflow)), [WatchMessage::Rescan]);
    assert!(messages(Ok(Event::new(EventKind::Access(AccessKind::Any)))).is_empty());
    assert_eq!(
        messages(Err(notify::Error::generic("boom"))),
        [WatchMessage::Error("boom".to_string())]
    );
}

#[test]
fn reports_log_once_and_a_stopped_watcher_disconnects() {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    let dir = tempdir();
    let file = dir.join("hero.png");
    touch(&file);
    let (tx, rx) = mpsc::channel();
    let mut inbox = Inbox::new(rx, vec![canonical_or_clone(&dir)], HashSet::new());
    for _ in 0..3 {
        tx.send(WatchMessage::Error("boom".to_string())).unwrap();
        tx.send(WatchMessage::Rescan).unwrap();
    }
    tx.send(WatchMessage::Changed(file.clone())).unwrap();
    let now = Instant::now();
    assert!(
        inbox.drain(now).is_empty(),
        "the path waits out the debounce"
    );
    assert!(inbox.logged_error && inbox.logged_rescan);
    assert!(!inbox.disconnected);

    // The watcher thread ending hands over every path it sent, the ones
    // still inside their debounce too, since the app then drops the watcher.
    let other = dir.join("villain.png");
    touch(&other);
    tx.send(WatchMessage::Changed(other.clone())).unwrap();
    drop(tx);
    let soon = now + Duration::from_millis(DEBOUNCE_MS / 2);
    let mut flushed = inbox.drain(soon);
    flushed.sort();
    let mut expected = vec![file, other];
    expected.sort();
    assert_eq!(flushed, expected);
    assert!(inbox.disconnected);
    assert!(inbox.drain(soon).is_empty());
}
