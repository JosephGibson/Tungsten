use std::panic::{AssertUnwindSafe, catch_unwind};

use glam::Vec2;
use serde::Deserialize;
use serde_json::json;

use super::*;
use crate::component_registry::SpawnContext;

fn parse(steps: &Value) -> Result<Script, ScriptError> {
    Script::parse(&json!({ "version": 1, "frames": 90, "steps": steps }).to_string())
}

fn assert_of(op: &str, value: &Value) -> Assert {
    serde_json::from_value(json!({ "probe": "p", "op": op, "value": value })).unwrap()
}

fn panic_message(f: impl FnOnce()) -> String {
    let payload = catch_unwind(AssertUnwindSafe(f)).expect_err("must panic");
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_default()
}

#[test]
fn an_unknown_key_fails_naming_its_path() {
    let err = parse(&json!([
        { "frame": 1, "press": "jump" },
        { "frame": 2, "release": "jump" },
        { "frame": 3, "captur": "a.png" },
    ]))
    .unwrap_err();
    assert_eq!(err.field, "steps[2].captur");
    assert!(
        err.to_string()
            .starts_with("steps[2].captur: unknown field `captur`")
    );

    let err = Script::parse(r#"{ "version": 1, "frames": 9, "frame": 3 }"#).unwrap_err();
    assert_eq!(err.field, "frame");

    let err =
        parse(&json!([{ "frame": 1, "sheet": { "items": ["a"], "colums": 2 } }])).unwrap_err();
    assert_eq!(err.field, "steps[0].sheet.colums");
}

#[test]
fn only_version_one_parses() {
    let err =
        Script::parse(r#"{ "version": 2, "frames": 9, "steps": [{ "frame": 1, "new_verb": 1 }] }"#)
            .unwrap_err();
    assert_eq!(err.field, "version");
    assert_eq!(
        err.to_string(),
        "version: version 2 is not supported; this engine reads version 1"
    );
    let err = Script::parse(r#"{ "frames": 9 }"#).unwrap_err();
    assert_eq!(err.field, "version");
    let err = Script::parse("{ not json").unwrap_err();
    assert_eq!(err.field, "");
    assert!(err.message.starts_with("not JSON"));
}

#[test]
fn until_and_every_expand_in_frame_then_file_order() {
    let script = parse(&json!([
        { "frame": 5, "until": 25, "every": 10, "capture": "loop" },
        { "frame": 10, "press": "jump" },
        { "frame": 2, "until": 4, "probe": "frame" },
        { "frame": 15, "release": "jump" },
    ]))
    .unwrap();
    let schedule: Vec<(u32, usize)> = script
        .schedule()
        .iter()
        .map(|at| (at.frame, at.step))
        .collect();
    assert_eq!(
        schedule,
        [
            (2, 2),
            (3, 2),
            (4, 2),
            (5, 0),
            (10, 1),
            (15, 0),
            (15, 3),
            (25, 0)
        ]
    );
    assert_eq!(
        script.steps[0].capture_path(15),
        Some(PathBuf::from("loop").join("0015.png"))
    );
    assert_eq!(script.steps[1].capture_path(10), None);
}

#[test]
fn frames_and_repeats_are_checked() {
    let cases = [
        (
            json!([{ "frame": 0, "press": "a" }]),
            "steps[0]",
            "counts from 1",
        ),
        (
            json!([{ "frame": 91, "press": "a" }]),
            "steps[0].frame",
            "after the last frame",
        ),
        (
            json!([{ "frame": 80, "until": 95, "press": "a" }]),
            "steps[0].until",
            "after the last frame",
        ),
        (
            json!([{ "frame": 9, "until": 3, "press": "a" }]),
            "steps[0]",
            "before `frame`",
        ),
        (
            json!([{ "frame": 1, "every": 3, "press": "a" }]),
            "steps[0]",
            "needs `until`",
        ),
        (
            json!([{ "frame": 1, "until": 5, "every": 0, "press": "a" }]),
            "steps[0]",
            "at least 1",
        ),
        (
            json!([{ "frame": 1, "until": 5, "exit": {} }]),
            "steps[0]",
            "does not repeat",
        ),
        (json!([{ "frame": 1 }]), "steps[0]", "needs a verb"),
        (
            json!([{ "frame": 1, "press": "a", "capture": "x.png" }]),
            "steps[0]",
            "one verb",
        ),
        (
            json!([{ "frame": 1, "press": "a", "args": 1 }]),
            "steps[0]",
            "`args` goes with",
        ),
        (
            json!([{ "frame": 1, "action": "go", "path": "/x" }]),
            "steps[0]",
            "`path` goes with",
        ),
        (
            json!([{ "frame": 1, "capture": "shot" }]),
            "steps[0]",
            "writes a .png",
        ),
        (
            json!([{ "frame": 1, "until": 9, "capture": "a.png" }]),
            "steps[0]",
            "names a folder",
        ),
        (
            json!([{ "frame": 1, "probe": "Bad" }]),
            "steps[0]",
            "must match",
        ),
        (
            json!([{ "frame": 1, "probe": "p", "path": "x" }]),
            "steps[0]",
            "JSON pointer",
        ),
        (
            json!([{ "frame": 1, "overlay": {} }]),
            "steps[0]",
            "`physics`, `kit`",
        ),
        (
            json!([{ "frame": 1, "sheet": { "items": [] } }]),
            "steps[0]",
            "at least one item",
        ),
        (
            json!([{ "frame": 1, "sheet": { "items": ["x:y"] } }]),
            "steps[0].sheet.items[0]",
            "not one of",
        ),
        (
            json!([{ "frame": 1, "camera": { "center": [0, 0] } }]),
            "steps[0].camera",
            "give `center`",
        ),
        (
            json!([{ "frame": 1, "camera": { "center": [0, 0], "zoom": 0 } }]),
            "steps[0].camera",
            "above 0",
        ),
        (
            json!([{ "frame": 1, "cursor": { "screen": [0, 0] } }]),
            "steps[0].cursor",
            "expected [x, y]",
        ),
        (
            json!([{ "frame": 1, "assert": { "probe": "p", "op": "==", "value": 1, "tolerance": 1 } }]),
            "steps[0]",
            "goes with `~=`",
        ),
    ];
    for (steps, field, message) in cases {
        let err = parse(&steps).unwrap_err();
        assert_eq!(err.field, field, "{steps}: {err}");
        assert!(err.message.contains(message), "{steps}: {err}");
    }
    let err = Script::parse(r#"{ "version": 1, "frames": 0 }"#).unwrap_err();
    assert_eq!(err.field, "frames");
    let err =
        Script::parse(r#"{ "version": 1, "frames": 9, "resolution": [0, 720] }"#).unwrap_err();
    assert_eq!(err.field, "resolution");
}

#[test]
fn every_verb_parses_and_round_trips() {
    let steps = json!([
        { "frame": 1, "press": "jump" },
        { "frame": 1, "release": "jump" },
        { "frame": 1, "cursor": [10.0, 20.0] },
        { "frame": 1, "cursor": { "world": [1.5, 2.5] } },
        { "frame": 1, "action": "start_run", "args": { "difficulty": "hard" } },
        { "frame": 1, "action": "pause" },
        { "frame": 1, "spawn": { "prefab": "goblin", "at": [4.0, 8.0] } },
        { "frame": 1, "scene": "assets/scenes/test.json" },
        { "frame": 1, "stage": {} },
        { "frame": 1, "stage": { "ambient": [0.5, 0.5, 0.5] } },
        { "frame": 1, "sheet": { "items": ["goblin", "sprite:icon", "animation:wisp", "particles:spark"], "columns": 2, "cell": [32.0, 48.0], "labels": false } },
        { "frame": 1, "camera": { "center": [1.0, 2.0], "zoom": 2.0 } },
        { "frame": 1, "camera": { "center": [1.0, 2.0], "view_height": 288.0 } },
        { "frame": 1, "camera": { "fit": "staged" } },
        { "frame": 2, "camera": null },
        { "frame": 1, "overlay": { "physics": true } },
        { "frame": 1, "ui": false },
        { "frame": 1, "capture": "a.png" },
        { "frame": 1, "until": 9, "every": 2, "capture": "loop" },
        { "frame": 1, "probe": "player", "args": { "id": 1 }, "path": "/position" },
        { "frame": 1, "assert": { "probe": "player", "path": "/position", "op": "~=", "value": [1.0, 2.0], "tolerance": 0.5 } },
        { "frame": 3, "exit": {} },
    ]);
    let script = parse(&steps).unwrap();
    let verbs: Vec<&str> = script.steps.iter().map(|step| step.verb.name()).collect();
    assert_eq!(
        verbs,
        [
            "press", "release", "cursor", "cursor", "action", "action", "spawn", "scene", "stage",
            "stage", "sheet", "camera", "camera", "camera", "camera", "overlay", "ui", "capture",
            "capture", "probe", "assert", "exit"
        ]
    );
    assert_eq!(
        script.steps[3].verb,
        Verb::Cursor(Cursor::World([1.5, 2.5]))
    );
    assert_eq!(
        script.steps[5].verb,
        Verb::Action(ActionStep {
            name: "pause".to_owned(),
            args: Value::Null
        })
    );
    let Verb::Sheet(sheet) = &script.steps[10].verb else {
        panic!("a sheet");
    };
    let kinds: Vec<SheetKind> = sheet.items.iter().map(|item| item.kind).collect();
    assert_eq!(
        kinds,
        [
            SheetKind::Prefab,
            SheetKind::Sprite,
            SheetKind::Animation,
            SheetKind::Particles
        ]
    );
    assert_eq!(sheet.items[1].id, "icon");
    assert!(!sheet.labels);
    assert_eq!(
        script.steps[12].verb,
        Verb::Camera(Some(CameraView::ViewHeight {
            center: [1.0, 2.0],
            view_height: 288.0
        }))
    );
    assert_eq!(script.steps[14].verb, Verb::Camera(None));

    let written = serde_json::to_string(&script).unwrap();
    assert_eq!(Script::parse(&written).unwrap(), script);
    // `null` stays a camera step; an absent key stays absent.
    assert!(written.contains(r#""camera":null"#));
    assert!(!written.contains(r#""every":1"#));
}

#[test]
fn each_op_holds_on_numbers() {
    let cases = [
        ("==", 2, true),
        ("==", 3, false),
        ("!=", 3, true),
        ("!=", 2, false),
        ("<", 3, true),
        ("<", 2, false),
        ("<=", 2, true),
        ("<=", 1, false),
        (">", 1, true),
        (">", 2, false),
        (">=", 2, true),
        (">=", 3, false),
    ];
    for (op, value, holds) in cases {
        assert_eq!(
            assert_of(op, &json!(value)).check(&json!(2)),
            Ok(holds),
            "2 {op} {value}"
        );
    }
    // Numbers compare by value, whatever their JSON form.
    assert_eq!(assert_of("==", &json!(2.0)).check(&json!(2)), Ok(true));
}

#[test]
fn each_op_holds_on_arrays_and_strings() {
    assert_eq!(
        assert_of("==", &json!([1, 2.0])).check(&json!([1.0, 2])),
        Ok(true)
    );
    assert_eq!(
        assert_of("==", &json!([1, 2])).check(&json!([1, 2, 3])),
        Ok(false)
    );
    assert_eq!(
        assert_of("!=", &json!([1, 2])).check(&json!([2, 1])),
        Ok(true)
    );
    assert_eq!(
        assert_of("==", &json!({ "a": [1] })).check(&json!({ "a": [1.0] })),
        Ok(true)
    );
    assert_eq!(
        assert_of("==", &json!("title")).check(&json!("title")),
        Ok(true)
    );
    assert_eq!(
        assert_of("!=", &json!("title")).check(&json!("run")),
        Ok(true)
    );
    assert_eq!(
        assert_of("<", &json!("run")).check(&json!("menu")),
        Ok(true)
    );
    assert_eq!(
        assert_of(">=", &json!("run")).check(&json!("menu")),
        Ok(false)
    );
    let err = assert_of("<", &json!([1])).check(&json!([0])).unwrap_err();
    assert!(err.contains("two numbers or two strings"), "{err}");
    let err = assert_of(">", &json!("a")).check(&json!(1)).unwrap_err();
    assert!(err.contains("a number and a string"), "{err}");
}

#[test]
fn near_holds_within_its_tolerance() {
    let near = |value: Value, tolerance: Option<f64>| Assert {
        tolerance,
        ..assert_of("~=", &value)
    };
    assert_eq!(near(json!(1.0), Some(0.5)).check(&json!(1.4)), Ok(true));
    assert_eq!(near(json!(1.0), Some(0.5)).check(&json!(1.6)), Ok(false));
    assert_eq!(near(json!(1.0), None).check(&json!(1.0)), Ok(true));
    assert_eq!(near(json!(1.0), None).check(&json!(1.001)), Ok(false));
    assert_eq!(
        near(json!([1.0, 2.0]), Some(0.5)).check(&json!([1.2, 1.7])),
        Ok(true)
    );
    assert_eq!(
        near(json!([1.0, 2.0]), Some(0.5)).check(&json!([1.2, 2.7])),
        Ok(false)
    );
    assert!(
        near(json!([1.0, 2.0]), Some(0.5))
            .check(&json!([1.0]))
            .is_err()
    );
    assert!(near(json!(["a"]), Some(0.5)).check(&json!(["a"])).is_err());
    assert!(near(json!("a"), Some(0.5)).check(&json!("a")).is_err());
}

#[test]
fn a_path_selects_a_field_and_a_missing_one_fails() {
    let player = json!({ "position": [10.0, 20.0], "health": 3 });
    let mut at = Assert {
        path: Some("/position".to_owned()),
        tolerance: Some(0.5),
        ..assert_of("~=", &json!([10.2, 19.9]))
    };
    assert_eq!(at.check(&player), Ok(true));
    at.path = Some("/position/1".to_owned());
    at.value = json!(20);
    assert_eq!(at.check(&player), Ok(true));

    at.path = Some("/speed".to_owned());
    let err = at.check(&player).unwrap_err();
    assert!(err.contains("path `/speed` finds nothing"), "{err}");
    let report = at.evaluate(9, Ok(&player));
    assert!(!report.passed);
    assert_eq!(report.actual, Value::Null);
    assert!(report.error.is_some());

    let health = Assert {
        path: Some("/health".to_owned()),
        ..assert_of(">", &json!(5))
    };
    let report = health.evaluate(9, Ok(&player));
    assert_eq!(
        (report.passed, report.actual.clone(), report.error.clone()),
        (false, json!(3), None)
    );
    let report = health.evaluate(9, Err("unknown probe `player`".to_owned()));
    assert_eq!(report.error.as_deref(), Some("unknown probe `player`"));
    assert!(!report.passed);
}

fn report_with(ended: Ending, f: impl FnOnce(&mut Report)) -> Report {
    let mut report = Report::new("run.json");
    f(&mut report);
    report.finish(ended, None);
    report
}

#[test]
fn report_passes_only_a_clean_ending() {
    let clean = report_with(Ending::Frames, |report| {
        report.frames = 45;
        report.captures.push(CaptureReport {
            frame: 10,
            path: "a.png".to_owned(),
            written: true,
            error: None,
        });
        report
            .asserts
            .push(assert_of("==", &json!(1)).evaluate(45, Ok(&json!(1))));
    });
    assert!(clean.passed);
    assert!(report_with(Ending::Exit, |_| {}).passed);
    assert!(report_with(Ending::GameExit, |_| {}).passed);

    // A fatal ending with nothing recorded.
    let mut failed = Report::new("run.json");
    failed.finish(Ending::Failed, Some("render failed".to_owned()));
    assert!(!failed.passed);
    assert_eq!(failed.error.as_deref(), Some("render failed"));
    assert!(!report_with(Ending::Failed, |_| {}).passed);

    // The game exits before its assert.
    let unreached = report_with(Ending::GameExit, |report| {
        report.unreached.push(Unreached {
            frame: 40,
            index: 2,
            verb: "assert".to_owned(),
        });
    });
    assert!(!unreached.passed);

    let missed_capture = report_with(Ending::Frames, |report| {
        report.captures.push(CaptureReport {
            frame: 10,
            path: "a.png".to_owned(),
            written: false,
            error: Some("frame skipped".to_owned()),
        });
    });
    assert!(!missed_capture.passed);
    let failed_step = report_with(Ending::Frames, |report| {
        report.failed_steps.push(FailedStep {
            frame: 2,
            index: 0,
            verb: "action".to_owned(),
            error: "unknown action `go`".to_owned(),
        });
    });
    assert!(!failed_step.passed);
    let failed_assert = report_with(Ending::Frames, |report| {
        report
            .asserts
            .push(assert_of("==", &json!(1)).evaluate(45, Ok(&json!(2))));
    });
    assert!(!failed_assert.passed);
    // Log lines only count.
    let logged = report_with(Ending::Frames, |report| report.log.errors = 3);
    assert!(logged.passed);

    let text = serde_json::to_string(&clean).unwrap();
    assert!(text.contains(r#""ended":"frames""#), "{text}");
    assert_eq!(serde_json::from_str::<Report>(&text).unwrap(), clean);
}

#[test]
fn hooks_register_once_and_run_by_name() {
    struct Score(u32);
    let mut world = World::new();
    world.insert_resource(Score(1));
    world.add_script_action("double", "Doubles the score.", |world, _| {
        world.resource_mut::<Score>().0 *= 2;
        Ok(())
    });
    world.add_script_action("fail", "Always fails.", |_, _| Err("no".to_owned()));
    world.add_script_probe("score", "The score.", |world, _| {
        Ok(json!(world.resource::<Score>().0))
    });

    ScriptHooks::call_action(&mut world, "double", &Value::Null).unwrap();
    assert_eq!(
        ScriptHooks::read_probe(&world, "score", &Value::Null),
        Ok(json!(2))
    );
    assert_eq!(
        ScriptHooks::call_action(&mut world, "fail", &Value::Null),
        Err("action `fail`: no".to_owned())
    );
    assert_eq!(
        ScriptHooks::call_action(&mut world, "halve", &Value::Null),
        Err("unknown action `halve`".to_owned())
    );
    assert_eq!(
        ScriptHooks::read_probe(&world, "lives", &Value::Null),
        Err("unknown probe `lives`".to_owned())
    );

    let hooks = world.resource::<ScriptHooks>();
    let actions: Vec<_> = hooks.actions().collect();
    assert_eq!(
        actions,
        [("double", "Doubles the score."), ("fail", "Always fails.")]
    );
    let probes: Vec<&str> = hooks.probes().map(|(name, _)| name).collect();
    assert_eq!(probes, ["camera", "count", "entities", "frame", "score"]);
    assert!(hooks.probes().all(|(_, doc)| !doc.is_empty()));

    let message = panic_message(|| {
        world.add_script_probe("score", "Again.", |_, _| Ok(Value::Null));
    });
    assert_eq!(
        message,
        "ScriptHooks::add_probe: `score` is already a probe"
    );
    let message = panic_message(|| {
        ScriptHooks::new().add_action("Go", "Bad name.", |_, _| Ok(()));
    });
    assert!(message.contains("must match"), "{message}");
}

#[derive(Deserialize)]
struct Enemy;

#[derive(Deserialize)]
struct LightDef {}

#[test]
fn core_probes_read_the_world() {
    let mut world = World::new();
    world.insert_resource(ScriptHooks::new());
    world.register_component::<Enemy>("enemy");
    world.register_component_with("glow", |_: LightDef, _: &SpawnContext<'_>| (Enemy,));
    world.spawn_with((Enemy,));
    world.spawn_with((Enemy,));
    world.spawn();
    let read = |world: &World, name: &str, args: Value| ScriptHooks::read_probe(world, name, &args);

    assert_eq!(read(&world, "entities", Value::Null), Ok(json!(3)));
    assert_eq!(read(&world, "frame", Value::Null), Ok(json!(0)));
    assert_eq!(
        read(&world, "count", json!({ "component": "enemy" })),
        Ok(json!(2))
    );
    let err = read(&world, "count", json!({ "component": "ghost" })).unwrap_err();
    assert!(err.contains("unknown component `ghost`"), "{err}");
    let err = read(&world, "count", json!({ "component": "glow" })).unwrap_err();
    assert!(err.contains("registered by a definition"), "{err}");
    assert!(read(&world, "count", json!({ "name": "enemy" })).is_err());

    let mut camera = CameraState::new();
    camera.position = Vec2::new(10.0, 20.0);
    camera.zoom = 2.0;
    world.insert_resource(camera);
    let game = json!({ "position": [10.0, 20.0], "zoom": 2.0, "rotation": 0.0 });
    assert_eq!(read(&world, "camera", Value::Null), Ok(game.clone()));
    // No override in force: the drawn view is the game's.
    assert_eq!(
        read(&world, "camera", json!({ "drawn": true })),
        Ok(game.clone())
    );
    let mut drawn = camera;
    drawn.zoom = 4.0;
    world.insert_resource(ScriptRun {
        frame: 12,
        drawn_camera: Some(drawn),
    });
    assert_eq!(read(&world, "frame", Value::Null), Ok(json!(12)));
    assert_eq!(read(&world, "camera", Value::Null), Ok(game));
    assert_eq!(
        read(&world, "camera", json!({ "drawn": true })),
        Ok(json!({ "position": [10.0, 20.0], "zoom": 4.0, "rotation": 0.0 }))
    );
}
