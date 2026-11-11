//! The game's script hooks through the engine's script runner (redo 7.2):
//! the shell's `start_run`, the sample's `reload_run` and `player`, and the
//! kit's `place` and `tagged` on the player's tag.

use tungsten::core::script::{Report, Script};

use super::harness;

fn run(script: &str) -> Report {
    let script = Script::parse(script).expect("a valid script");
    harness().harness_mut().run_script(&script)
}

#[test]
fn a_script_starts_a_run_and_holds_the_player_where_it_puts_it() {
    let report = run(r#"{
        "version": 1,
        "frames": 90,
        "steps": [
            { "frame": 2, "action": "start_run", "args": { "difficulty": "hard" } },
            { "frame": 30, "until": 90, "action": "place", "args": { "tag": "team.player", "at": [960, 600] } },
            { "frame": 90, "probe": "states" },
            { "frame": 90, "assert": { "probe": "tagged", "args": { "tag": "team.player" }, "path": "/position", "op": "~=", "value": [960, 600], "tolerance": 0.5 } },
            { "frame": 90, "assert": { "probe": "player", "path": "/position", "op": "~=", "value": [960, 600], "tolerance": 0.5 } },
            { "frame": 90, "assert": { "probe": "player", "path": "/health/0", "op": ">", "value": 0 } }
        ]
    }"#);
    assert!(report.passed, "{report:#?}");
    assert_eq!(report.probes[0].value.to_string(), r#"["gameplay"]"#);
}

#[test]
fn a_frame_one_sheet_clears_the_title_and_spawns_its_items() {
    let report = run(r#"{
        "version": 1,
        "frames": 3,
        "steps": [
            { "frame": 1, "sheet": { "items": ["player", "prefab:gem", "dummy"] } },
            { "frame": 3, "assert": { "probe": "states", "op": "==", "value": ["script_stage"] } },
            { "frame": 3, "assert": { "probe": "count", "args": { "component": "player" }, "op": "==", "value": 1 } }
        ]
    }"#);
    assert!(report.passed, "{report:#?}");
}

#[test]
fn place_with_no_run_fails_its_step() {
    let report = run(r#"{
        "version": 1,
        "frames": 2,
        "steps": [{ "frame": 2, "action": "place", "args": { "tag": "team.player", "at": [0, 0] } }]
    }"#);
    assert!(!report.passed);
    assert_eq!(
        report.failed_steps[0].error,
        "action `place`: no entity holds `team.player`"
    );
}

#[test]
fn reload_run_keeps_the_run() {
    // The player placed once, then the run saved in memory and continued:
    // the loaded run's player stands where the save left it, not at the
    // start (624, 344).
    let report = run(r#"{
        "version": 1,
        "frames": 60,
        "steps": [
            { "frame": 2, "action": "start_run" },
            { "frame": 10, "action": "place", "args": { "tag": "team.player", "at": [300, 150] } },
            { "frame": 20, "action": "reload_run" },
            { "frame": 21, "probe": "states" },
            { "frame": 60, "assert": { "probe": "player", "path": "/position", "op": "~=", "value": [300, 150], "tolerance": 0.5 } },
            { "frame": 60, "assert": { "probe": "player", "path": "/health/0", "op": ">", "value": 0 } }
        ]
    }"#);
    assert!(report.passed, "{report:#?}");
}
