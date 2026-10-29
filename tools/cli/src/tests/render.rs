use std::ffi::OsString;

use tungsten_core::script::Script;

use super::*;
use crate::args::{Command, UsageError, parse};

fn render(args: &[&str]) -> Result<RenderArgs, String> {
    let mut all = vec!["render"];
    all.extend_from_slice(args);
    match parse(all.iter().map(OsString::from)) {
        Ok(Command::Render(args)) => Ok(args),
        Ok(other) => panic!("{other:?}"),
        Err(UsageError(reason)) => Err(reason),
    }
}

/// The script `args` make in `/game`, checked by core's parser.
fn script_of(args: &[&str]) -> Value {
    let script = script(Path::new("/game"), &render(args).expect("valid flags"));
    Script::parse(&script.to_string()).expect("core parses the script");
    script
}

#[test]
fn a_sheet_is_one_step_and_a_capture_at_frame_60() {
    assert_eq!(
        script_of(&["--sheet", "goblin,sprite:icon_fire", "-o", "shots/foes.png"]),
        json!({
            "version": 1, "frames": 60, "resolution": [1280, 720],
            "steps": [
                { "frame": 1, "sheet": { "items": ["goblin", "sprite:icon_fire"] } },
                { "frame": 60, "capture": "/game/shots/foes.png" }
            ]
        })
    );
}

#[test]
fn a_loop_captures_a_folder_by_frame() {
    assert_eq!(
        script_of(&[
            "--sheet",
            "animation:wisp",
            "--loop",
            "2..61/3",
            "--size",
            "640x360",
            "--no-ui",
            "-o",
            "/tmp/loop"
        ]),
        json!({
            "version": 1, "frames": 61, "resolution": [640, 360],
            "steps": [
                { "frame": 1, "sheet": { "items": ["animation:wisp"] } },
                { "frame": 1, "ui": false },
                { "frame": 2, "until": 61, "every": 3, "capture": "/tmp/loop" }
            ]
        })
    );
}

#[test]
fn a_point_takes_its_actions_view_and_overlays() {
    assert_eq!(
        script_of(&[
            "--after",
            "start_run",
            "--after",
            r#"place_player={"at": [10, -20]}"#,
            "--at",
            "-64,480.5",
            "--view",
            "288",
            "--overlay",
            "physics,kit",
            "--frame",
            "120",
            "-o",
            "area.png"
        ]),
        json!({
            "version": 1, "frames": 120, "resolution": [1280, 720],
            "steps": [
                { "frame": 1, "camera": { "center": [-64.0, 480.5], "view_height": 288.0 } },
                { "frame": 1, "overlay": { "physics": true, "kit": true } },
                { "frame": 2, "action": "start_run" },
                { "frame": 3, "action": "place_player", "args": { "at": [10, -20] } },
                { "frame": 120, "capture": "/game/area.png" }
            ]
        })
    );
    assert_eq!(
        script_of(&["--at", "5,6", "--zoom", "2", "-o", "a.png"])["steps"][0],
        json!({ "frame": 1, "camera": { "center": [5.0, 6.0], "zoom": 2.0 } })
    );
    assert_eq!(
        script_of(&["--at", "5,6", "-o", "a.png"])["steps"][0]["camera"]["zoom"],
        json!(1.0)
    );
}

#[test]
fn a_scene_stages_loads_and_fits() {
    assert_eq!(
        script_of(&[
            "--scene",
            "assets/scenes/arena.json",
            "--frame",
            "5",
            "-o",
            "s.png"
        ])["steps"],
        json!([
            { "frame": 1, "stage": {} },
            { "frame": 1, "scene": "/game/assets/scenes/arena.json" },
            { "frame": 1, "camera": { "fit": "staged" } },
            { "frame": 5, "capture": "/game/s.png" }
        ])
    );
}

#[test]
fn many_actions_lengthen_the_run_past_the_capture() {
    let after: Vec<String> = (0..4).map(|i| format!("a{i}")).collect();
    let mut args: Vec<&str> = Vec::new();
    for action in &after {
        args.extend(["--after", action.as_str()]);
    }
    args.extend(["--at", "0,0", "--frame", "3", "-o", "a.png"]);
    assert_eq!(script_of(&args)["frames"], json!(5));
}

#[test]
fn each_bad_flag_is_a_usage_error() {
    let cases: &[(&[&str], &str)] = &[
        (&["-o", "a.png"], "needs one of --sheet, --scene and --at"),
        (
            &["--sheet", "a", "--at", "1,2", "-o", "a.png"],
            "not several",
        ),
        (&["--sheet", "a"], "needs -o <path>"),
        (&["--sheet", "a", "-o", "shots"], "names the .png file"),
        (
            &["--sheet", "a", "--loop", "1..5/1", "-o", "a.png"],
            "the folder of its frames",
        ),
        (&["--sheet", "a,,b", "-o", "a.png"], "an item is empty"),
        (&["--at", "1", "-o", "a.png"], "two numbers split by `,`"),
        (&["--at", "1,x", "-o", "a.png"], "two numbers"),
        (&["--at", "1,2", "--zoom", "0", "-o", "a.png"], "above 0"),
        (
            &["--at", "1,2", "--zoom", "2", "--view", "9", "-o", "a.png"],
            "not both",
        ),
        (
            &["--sheet", "a", "--view", "9", "-o", "a.png"],
            "go with --at",
        ),
        (
            &["--sheet", "a", "--size", "1280", "-o", "a.png"],
            "split by `x`",
        ),
        (
            &["--sheet", "a", "--size", "0x720", "-o", "a.png"],
            "whole 1 to 16384",
        ),
        (
            &["--sheet", "a", "--overlay", "lights", "-o", "a.png"],
            "not physics or kit",
        ),
        (
            &["--sheet", "a", "--frame", "0", "-o", "a.png"],
            "whole number from 1",
        ),
        (
            &[
                "--sheet", "a", "--frame", "5", "--loop", "1..2/1", "-o", "d",
            ],
            "not both",
        ),
        (
            &["--sheet", "a", "--loop", "5..2/1", "-o", "d"],
            "<from>..<to>/<every>",
        ),
        (
            &["--sheet", "a", "--loop", "1..9", "-o", "d"],
            "<from>..<to>/<every>",
        ),
        (
            &["--at", "0,0", "--after", "go={bad", "-o", "a.png"],
            "not JSON",
        ),
        (
            &["--sheet", "a", "--sheet", "b", "-o", "a.png"],
            "given twice",
        ),
        (&["--sheet", "a", "-o"], "`-o` needs a <path>"),
        (
            &["--sheet", "a", "--json", "-o", "a.png"],
            "an option of `describe`",
        ),
        (
            &["--sheet", "a", "extra", "-o", "a.png"],
            "takes no further argument",
        ),
    ];
    for (args, message) in cases {
        let err = render(args).unwrap_err();
        assert!(err.contains(message), "{args:?}: {err}");
    }
}
