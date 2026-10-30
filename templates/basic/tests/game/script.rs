//! The sample's script hooks through the engine's script runner (redo 7.2).

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
            { "frame": 30, "until": 90, "action": "place_player", "args": { "at": [960, 600] } },
            { "frame": 90, "probe": "states" },
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
fn place_player_with_no_run_fails_its_step() {
    let report = run(r#"{
        "version": 1,
        "frames": 2,
        "steps": [{ "frame": 2, "action": "place_player", "args": { "at": [0, 0] } }]
    }"#);
    assert!(!report.passed);
    assert_eq!(
        report.failed_steps[0].error,
        "action `place_player`: no run"
    );
}
