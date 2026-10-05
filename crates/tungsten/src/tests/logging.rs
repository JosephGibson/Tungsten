use super::*;
use log::{Level, Log, Metadata};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn tempdir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_logging_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn at(secs: u64, millis: u32) -> SystemTime {
    UNIX_EPOCH + Duration::new(secs, millis * 1_000_000)
}

fn enabled(logger: &env_logger::Logger, level: Level, target: &str) -> bool {
    logger.enabled(&Metadata::builder().level(level).target(target).build())
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn rust_log_replaces_the_level() {
    let logger = builder(Some("warn"), LevelFilter::Trace).build();
    assert!(enabled(&logger, Level::Warn, "game"));
    assert!(!enabled(&logger, Level::Info, "game"));

    // The runner's filter: its modules only, whatever the level says.
    let logger = builder(Some("tungsten::app=debug,bench=debug"), LevelFilter::Trace).build();
    assert!(enabled(&logger, Level::Debug, "tungsten::app"));
    assert!(enabled(&logger, Level::Debug, "bench"));
    assert!(!enabled(&logger, Level::Error, "game"));
}

#[test]
fn the_level_applies_while_rust_log_is_unset() {
    let logger = builder(None, LevelFilter::Debug).build();
    assert!(enabled(&logger, Level::Debug, "game"));
    assert!(!enabled(&logger, Level::Trace, "game"));
    let logger = builder(None, LevelFilter::Off).build();
    assert!(!enabled(&logger, Level::Error, "game"));
}

#[test]
fn sinks_follow_the_build_rust_log_and_the_folder() {
    let cases = [
        // (debug build, RUST_LOG set, folder) -> (stderr, file)
        ((true, false, false), (true, false)),
        ((true, false, true), (true, true)),
        ((true, true, false), (true, false)),
        ((true, true, true), (true, true)),
        ((false, false, false), (false, false)),
        ((false, false, true), (false, true)),
        ((false, true, false), (true, false)),
        ((false, true, true), (true, true)),
    ];
    for ((debug, rust_log, folder), (stderr, file)) in cases {
        assert_eq!(
            Sinks::new(debug, rust_log, folder),
            Sinks { stderr, file },
            "debug {debug}, RUST_LOG {rust_log}, folder {folder}"
        );
    }
}

#[test]
fn utc_stamps_at_known_instants() {
    assert_eq!(utc_stamp(UNIX_EPOCH), "19700101T000000.000Z");
    assert_eq!(utc_stamp(at(951_827_696, 789)), "20000229T123456.789Z");
    assert_eq!(utc_stamp(at(946_684_799, 999)), "19991231T235959.999Z");
    assert_eq!(utc_stamp(at(1_791_158_400, 0)), "20261005T000000.000Z");
}

#[test]
fn log_file_names_carry_stem_stamp_and_pid() {
    assert_eq!(
        log_file_name("example-03-scene-state", at(951_827_696, 789), 4242),
        "example-03-scene-state-20000229T123456.789Z-4242.log"
    );
}

#[test]
fn only_exact_log_names_of_the_stem_count() {
    assert!(is_log_of("game-20261005T000000.000Z-1.log", "game"));
    assert!(!is_log_of("game-helper-20261005T000000.000Z-1.log", "game"));
    assert!(is_log_of(
        "game-helper-20261005T000000.000Z-1.log",
        "game-helper"
    ));
    assert!(!is_log_of("game-20261005T000000.000Z-1-crash.txt", "game"));
    assert!(!is_log_of("game-20261005T000000.000Z-.log", "game"));
    assert!(!is_log_of("game-20261005T000000.000Z-1x.log", "game"));
    assert!(!is_log_of("game-20261005T0000000.000Z-1.log", "game"));
    assert!(!is_log_of("game.log", "game"));
}

#[test]
fn pruning_keeps_the_ten_newest_of_the_stem_and_nothing_else_moves() {
    let dir = tempdir();
    let games: Vec<String> = (0..12)
        .map(|n| log_file_name("game", at(1_791_158_400 + n, 0), 7))
        .collect();
    let others = [
        log_file_name("game-helper", at(1_791_158_300, 0), 7),
        log_file_name("game-helper", at(1_791_158_301, 0), 7),
        log_file_name("other", at(1_791_158_300, 0), 7),
        "game-20261004T000000.000Z-7-crash.txt".to_string(),
        "game-20261004T000001.000Z-7-crash.txt".to_string(),
    ];
    for name in games.iter().chain(&others) {
        fs::write(dir.join(name), b"x").unwrap();
    }

    prune(&dir, "game", KEPT_LOGS);

    let mut expected: Vec<String> = games[2..].iter().chain(&others).cloned().collect();
    expected.sort();
    assert_eq!(names_in(&dir), expected);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn open_creates_the_folder_and_the_file() {
    let dir = tempdir();
    let logs = dir.join("nested").join("logs");
    let file = LogFile::default();

    open(&file, &logs, "game", at(1_791_158_400, 5), 99);

    assert!(file.get().is_some());
    assert_eq!(names_in(&logs), ["game-20261005T000000.005Z-99.log"]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn open_prunes_the_stem_down_to_ten_counting_the_new_file() {
    let dir = tempdir();
    let logs = dir.join("logs");
    fs::create_dir_all(&logs).unwrap();
    for n in 0..10 {
        fs::write(
            logs.join(log_file_name("game", at(1_000 + n, 0), 1)),
            b"old",
        )
        .unwrap();
    }
    let file = LogFile::default();

    open(&file, &logs, "game", at(1_791_158_400, 5), 99);

    assert!(file.get().is_some());
    let names = names_in(&logs);
    assert_eq!(names.len(), KEPT_LOGS);
    assert_eq!(
        names.last().map(String::as_str),
        Some("game-20261005T000000.005Z-99.log")
    );
    assert!(!names.contains(&log_file_name("game", at(1_000, 0), 1)));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_that_cannot_be_created_leaves_no_file() {
    let dir = tempdir();
    let blocker = dir.join("not-a-folder");
    fs::write(&blocker, b"x").unwrap();
    let file = LogFile::default();

    open(
        &file,
        &blocker.join("logs"),
        "game",
        at(1_791_158_400, 0),
        1,
    );

    assert!(file.get().is_none());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_tee_writes_each_record_to_the_file() {
    let dir = tempdir();
    let path = dir.join("run.log");
    let file = LogFile::default();
    file.set(File::create(&path).unwrap()).unwrap();
    let mut tee = Tee {
        file: Arc::clone(&file),
        stderr: false,
    };

    tee.write_all(b"[INFO game] one\n").unwrap();
    tee.write_all(b"[WARN game] two\n").unwrap();
    tee.flush().unwrap();

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "[INFO game] one\n[WARN game] two\n"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_tee_without_a_file_drops_records() {
    let mut tee = Tee {
        file: LogFile::default(),
        stderr: false,
    };
    tee.write_all(b"[INFO game] dropped\n").unwrap();
}
