use serde_json::json;
use tungsten_core::script::{CaptureReport, Ending, FailedStep, Unreached};

use super::*;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tungsten_cli_run_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn args(script: &str) -> RunArgs {
    RunArgs {
        script: PathBuf::from(script),
        report: None,
        user_dir: None,
        release: false,
    }
}

#[test]
fn a_bad_script_fails_with_its_field_path_before_any_game_check() {
    // No Cargo.toml: the script's error comes first, so nothing builds.
    let dir = folder("bad");
    std::fs::write(
        dir.join("bad.json"),
        r#"{ "version": 1, "frames": 5, "steps": [{ "frame": 1, "captur": "a.png" }] }"#,
    )
    .unwrap();
    let err = format!("{:#}", run(&dir, &args("bad.json")).unwrap_err());
    assert!(err.starts_with("invalid script '"), "{err}");
    assert!(
        err.contains("steps[0].captur: unknown field `captur`"),
        "{err}"
    );

    let err = format!("{:#}", run(&dir, &args("missing.json")).unwrap_err());
    assert!(err.contains("cannot read the script"), "{err}");

    std::fs::write(dir.join("good.json"), r#"{ "version": 1, "frames": 5 }"#).unwrap();
    let err = format!("{:#}", run(&dir, &args("good.json")).unwrap_err());
    assert!(err.contains("no Cargo.toml here"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn failing_report() -> Report {
    let script = Script::parse(
        r#"{ "version": 1, "frames": 45, "steps": [
            { "frame": 45, "assert": { "probe": "player", "path": "/position", "op": "~=", "value": [10, 20], "tolerance": 0.5 } },
            { "frame": 45, "assert": { "probe": "area", "op": "==", "value": "cave" } }
        ] }"#,
    )
    .unwrap();
    let assert_of = |index: usize| match &script.steps[index].verb {
        tungsten_core::script::Verb::Assert(assert) => assert.clone(),
        _ => unreachable!(),
    };
    let mut report = Report::new("run.json");
    report.frames = 45;
    report
        .asserts
        .push(assert_of(0).evaluate(45, Ok(&json!({ "position": [12.0, 20.0] }))));
    report
        .asserts
        .push(assert_of(1).evaluate(45, Err("unknown probe `area`".to_owned())));
    report.captures.push(CaptureReport {
        frame: 10,
        path: "shots/a.png".to_owned(),
        written: false,
        error: Some("not written: the frame was skipped".to_owned()),
    });
    report.failed_steps.push(FailedStep {
        frame: 2,
        index: 0,
        verb: "action".to_owned(),
        error: "unknown action `start_run`".to_owned(),
    });
    for frame in 46..58 {
        report.unreached.push(Unreached {
            frame,
            index: 3,
            verb: "probe".to_owned(),
        });
    }
    report.finish(Ending::GameExit, None);
    report
}

#[test]
fn the_failures_name_each_assert_its_value_and_the_actual() {
    let report = failing_report();
    let lines = failures(&report);
    assert_eq!(
        lines[..4],
        [
            "frame 45: assert player/position ~= [10,20] failed: actual [12.0,20.0]",
            "frame 45: assert area == \"cave\" failed: unknown probe `area`",
            "frame 10: capture shots/a.png not written: not written: the frame was skipped",
            "frame 2: step 0 (action) failed: unknown action `start_run`",
        ]
    );
    assert_eq!(lines[4], "frame 46: step 3 (probe) unreached");
    assert_eq!(lines.len(), 4 + 10 + 1);
    assert_eq!(lines.last().unwrap(), "and 2 more unreached");
    assert_eq!(
        summary(&report),
        "tungsten run: 45 frames, 1 capture, 2 asserts: failed"
    );
}

#[test]
fn a_passing_report_prints_no_failure() {
    let mut report = Report::new("run.json");
    report.frames = 1;
    report.finish(Ending::Frames, None);
    assert!(failures(&report).is_empty());
    assert_eq!(
        summary(&report),
        "tungsten run: 1 frame, 0 captures, 0 asserts: passed"
    );
    let mut fatal = Report::new("run.json");
    fatal.finish(Ending::Failed, Some("render failed at frame 3".to_owned()));
    assert_eq!(failures(&fatal), ["error: render failed at frame 3"]);
}
