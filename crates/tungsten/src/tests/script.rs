use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use glam::Vec2;
use serde_json::{Value, json};
use tungsten_core::assets::{FilterMode, ResolvedManifest, ResolvedPrefab, TextureHandle, UvRect};
use tungsten_core::script::{Ending, SheetItem, SheetKind, SheetStep};
use tungsten_core::ui::{LayoutStyle, UiLayer};
use tungsten_core::{
    ActionMap, AssetRegistry, CameraState, Config, InputState, Stage, Time, system,
};

use super::*;
use crate::App;
use crate::testing::{Harness, UiHarness};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn app() -> App {
    App::new(Config::default()).expect("App::new failed")
}

fn script(frames: u32, steps: &Value) -> Script {
    Script::parse(&json!({ "version": 1, "frames": frames, "steps": steps }).to_string())
        .expect("a valid script")
}

fn tempdir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_script_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A 16 × 16 sprite `name`, registered with no atlas behind it.
fn register_sprite(world: &mut World, name: &str) {
    world.resource_mut::<AssetRegistry>().register_sprite(
        name.to_string(),
        FilterMode::Nearest,
        16,
        16,
        PathBuf::from(format!("__test__/{name}.png")),
        TextureHandle(0),
        UvRect::FULL,
        None,
        None,
        None,
    );
}

/// A state that spawns one entity it owns when it enters.
struct Owning(StateId);

impl GameState for Owning {
    fn id(&self) -> StateId {
        self.0
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let id = ctx.state_id;
        ctx.world.spawn_with((SceneEntity { state_id: id },));
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, _world: &mut World) {}
}

/// `owned` `{"state": id}`: how many live entities `id` owns.
fn add_owned_probe(world: &mut World) {
    world.add_script_probe("owned", "Entities a state owns.", |world, args| {
        let id = args["state"].as_str().ok_or("`state` names a state")?;
        let owned = world
            .query::<&SceneEntity>()
            .filter(|owner| owner.state_id == id)
            .count();
        Ok(json!(owned))
    });
}

/// An app whose `Startup` pushes the states `ids`, bottom first, each
/// owning one entity, as the shell pushes its title.
fn app_pushing(ids: &'static [StateId]) -> App {
    let mut app = app();
    add_owned_probe(app.world_mut());
    app.add_system_to(
        Stage::Startup,
        system("push_states", move |world: &mut World| {
            let stack = world.resource_mut::<StateStack>();
            for id in ids {
                stack.request_push(Owning(id));
            }
        }),
    );
    app
}

fn probes<'r>(report: &'r Report, name: &str) -> Vec<(u32, &'r Value)> {
    report
        .probes
        .iter()
        .filter(|probe| probe.name == name)
        .map(|probe| (probe.frame, &probe.value))
        .collect()
}

#[derive(Default)]
struct Edges(Vec<(bool, bool)>);

#[test]
fn a_press_and_a_release_reach_the_action_map_as_edges_on_their_frames() {
    let mut app = app();
    app.world_mut().insert_resource(Edges::default());
    app.add_system(|world| {
        let input = world.resource::<InputState>();
        let actions = world.resource::<ActionMap>();
        let edge = (
            actions.just_pressed(input, "jump"),
            actions.just_released(input, "jump"),
        );
        world.resource_mut::<Edges>().0.push(edge);
    });
    let mut harness = Harness::new(app);
    let report = harness.run_script(&script(
        5,
        &json!([
            { "frame": 2, "press": "jump" },
            { "frame": 4, "release": "jump" },
        ]),
    ));
    assert!(report.passed, "{report:#?}");
    assert_eq!(report.ended, Ending::Frames);
    assert_eq!(report.frames, 5);
    assert_eq!(
        harness.world().resource::<Edges>().0,
        [
            (false, false),
            (true, false),
            (false, false),
            (false, true),
            (false, false)
        ]
    );
}

#[test]
fn an_actions_error_and_an_unknown_action_fail_their_steps_and_the_run() {
    let mut app = app();
    app.world_mut()
        .add_script_action("boom", "Always fails.", |_, _| Err("no".to_owned()));
    let mut harness = Harness::new(app);
    let report = harness.run_script(&script(
        3,
        &json!([
            { "frame": 1, "action": "boom" },
            { "frame": 2, "action": "vanish" },
            { "frame": 2, "press": "no_such_action" },
        ]),
    ));
    assert!(!report.passed);
    assert_eq!(report.ended, Ending::Frames);
    let failed: Vec<(u32, usize, &str, &str)> = report
        .failed_steps
        .iter()
        .map(|f| (f.frame, f.index, f.verb.as_str(), f.error.as_str()))
        .collect();
    assert_eq!(
        failed,
        [
            (1, 0, "action", "action `boom`: no"),
            (2, 1, "action", "unknown action `vanish`"),
            (2, 2, "press", "action `no_such_action` has no binding"),
        ]
    );
}

#[test]
fn a_failed_assert_reports_its_actual_value() {
    let mut harness = Harness::new(app());
    let report = harness.run_script(&script(
        4,
        &json!([
            { "frame": 3, "assert": { "probe": "frame", "op": "==", "value": 2 } },
            { "frame": 4, "assert": { "probe": "frame", "op": ">=", "value": 4 } },
        ]),
    ));
    assert!(!report.passed);
    let asserts: Vec<(u32, bool, &Value)> = report
        .asserts
        .iter()
        .map(|a| (a.frame, a.passed, &a.actual))
        .collect();
    assert_eq!(asserts, [(3, false, &json!(3)), (4, true, &json!(4))]);
}

#[test]
fn repeated_probes_land_on_their_frames() {
    let mut harness = Harness::new(app());
    let report = harness.run_script(&script(
        9,
        &json!([
            { "frame": 2, "until": 8, "every": 3, "probe": "frame" },
            { "frame": 9, "probe": "states" },
        ]),
    ));
    assert!(report.passed, "{report:#?}");
    assert_eq!(
        probes(&report, "frame"),
        [(2, &json!(2)), (5, &json!(5)), (8, &json!(8))]
    );
    assert_eq!(probes(&report, "states"), [(9, &json!([]))]);
}

#[test]
fn a_frame_one_stage_clears_a_startup_push_and_holds_its_sheet() {
    let mut app = app_pushing(&["title"]);
    register_sprite(app.world_mut(), "dot");
    let mut harness = Harness::new(app);
    let report = harness.run_script(&script(60, &json!([
            { "frame": 1, "stage": { "ambient": [0.5, 0.5, 0.5] } },
            { "frame": 1, "sheet": { "items": ["sprite:dot", "sprite:dot"] } },
            { "frame": 1, "until": 60, "probe": "states" },
            { "frame": 1, "until": 60, "probe": "owned", "args": { "state": "script_stage" } },
            { "frame": 3, "assert": { "probe": "owned", "args": { "state": "title" }, "op": "==", "value": 0 } },
        ]),
    ));
    assert!(report.passed, "{report:#?}");
    let states = probes(&report, "states");
    assert_eq!(
        states[0],
        (1, &json!(["title"])),
        "Startup's push applies first"
    );
    assert!(
        states[2..]
            .iter()
            .all(|(_, stack)| **stack == json!(["script_stage"])),
        "the stage alone from frame 3: {states:?}"
    );
    // The sheet spawns once the stage is alone, and its items live to frame
    // 60: an exit of the stage would have despawned them.
    let owned = probes(&report, "owned");
    assert_eq!(owned.len(), 60);
    assert!(
        owned
            .iter()
            .all(|(frame, n)| **n == json!(if *frame >= 3 { 2 } else { 0 })),
        "{owned:?}"
    );
    assert_eq!(
        harness.world().resource::<AmbientLight>().0,
        glam::Vec3::splat(0.5)
    );
}

#[test]
fn a_stage_over_two_states_leaves_itself_alone_and_their_entities_gone() {
    let mut harness = Harness::new(app_pushing(&["game", "pause"]));
    let report = harness.run_script(&script(
        4,
        &json!([
            { "frame": 1, "stage": {} },
            { "frame": 1, "probe": "states" },
            { "frame": 4, "probe": "states" },
            { "frame": 4, "probe": "owned", "args": { "state": "game" } },
            { "frame": 4, "probe": "owned", "args": { "state": "pause" } },
        ]),
    ));
    assert!(report.passed, "{report:#?}");
    assert_eq!(
        probes(&report, "states"),
        [
            (1, &json!(["game", "pause"])),
            (4, &json!(["script_stage"]))
        ]
    );
    assert_eq!(probes(&report, "owned"), [(4, &json!(0)), (4, &json!(0))]);
}

#[test]
fn an_engine_exit_leaves_its_frames_assert_unreached_and_the_run_failed() {
    let mut harness = Harness::new(app());
    let report = harness.run_script(&script(
        5,
        &json!([
            { "frame": 2, "press": "engine_exit" },
            { "frame": 2, "assert": { "probe": "frame", "op": "==", "value": 99 } },
            { "frame": 4, "probe": "frame" },
        ]),
    ));
    assert_eq!(report.ended, Ending::GameExit);
    assert!(!report.passed);
    assert!(report.asserts.is_empty());
    let unreached: Vec<(u32, usize, &str)> = report
        .unreached
        .iter()
        .map(|u| (u.frame, u.index, u.verb.as_str()))
        .collect();
    assert_eq!(unreached, [(2, 1, "assert"), (4, 2, "probe")]);
    assert_eq!(report.frames, 2);
}

/// Three prefabs of the 16 × 16 sprite `dot` with dynamic bodies moving
/// apart, loaded headless.
fn app_with_moving_prefabs() -> App {
    let mut app = app();
    let world = app.world_mut();
    register_sprite(world, "dot");
    let dir = tempdir();
    let mut manifest = ResolvedManifest::default();
    for (id, velocity) in [("a", [120, 0]), ("b", [0, -90]), ("c", [-60, 60])] {
        let path = dir.join(format!("{id}.json"));
        let prefab = json!({
            "sprite": { "asset_id": "dot" },
            "components": {
                "rigid_body": {
                    "kind": "dynamic",
                    "shape": { "kind": "aabb", "half_extents": [4, 4] },
                    "velocity": velocity
                }
            }
        });
        fs::write(&path, prefab.to_string()).unwrap();
        manifest
            .prefabs
            .insert(id.to_string(), ResolvedPrefab::new(path));
    }
    asset_loader::load_prefabs(&manifest, world).expect("the prefabs load");
    app
}

fn drawn_positions(harness: &Harness) -> Vec<Vec2> {
    let mut positions: Vec<Vec2> = harness
        .draw()
        .expect("a frame ran")
        .sprites
        .iter()
        .flat_map(|batch| batch.instances.iter())
        .map(|instance| Vec2::from_array(instance.position))
        .collect();
    positions.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    positions
}

#[test]
fn a_sheet_holds_moving_prefabs_at_their_cells_with_interpolation_on_and_off() {
    let sheet = SheetStep {
        items: ["a", "b", "c"]
            .map(|id| SheetItem {
                kind: SheetKind::Prefab,
                id: id.to_owned(),
            })
            .to_vec(),
        columns: None,
        cell: None,
        labels: true,
    };
    let layout = SheetLayout::new(&sheet, &[Some(Vec2::splat(16.0)); 3]);
    let mut cells: Vec<Vec2> = (0..3).map(|index| layout.center(index)).collect();
    cells.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    for interpolate in [true, false] {
        let mut app = app_with_moving_prefabs();
        app.world_mut()
            .resource_mut::<Time>()
            .set_interpolate(interpolate);
        let mut harness = Harness::new(app);
        let report = harness.run_script(&script(
            60,
            &json!([{ "frame": 1, "sheet": { "items": ["a", "prefab:b", "c"] } }]),
        ));
        assert!(report.passed, "{report:#?}");
        assert_eq!(
            drawn_positions(&harness),
            cells,
            "interpolate {interpolate}"
        );
    }
}

#[test]
fn the_game_camera_reads_the_same_after_an_overridden_run() {
    let steps = json!([
        { "frame": 1, "camera": { "center": [500.0, 300.0], "zoom": 3.0 } },
        { "frame": 5, "probe": "camera", "path": "/zoom" },
        { "frame": 5, "probe": "camera", "args": { "drawn": true }, "path": "/zoom" },
    ]);
    let mut overridden = Harness::new(app());
    let report = overridden.run_script(&script(10, &steps));
    assert!(report.passed, "{report:#?}");
    assert_eq!(
        probes(&report, "camera"),
        [(5, &json!(1.0)), (5, &json!(3.0))]
    );

    let mut plain = Harness::new(app());
    let report = plain.run_script(&script(10, &json!([])));
    assert!(report.passed, "{report:#?}");
    let camera = |harness: &Harness| {
        let camera = harness.world().resource::<CameraState>();
        (camera.position, camera.zoom, camera.rotation)
    };
    assert_eq!(camera(&overridden), camera(&plain));
}

#[test]
fn ui_false_leaves_the_frames_paint_list_empty() {
    let painted = |ui: bool| {
        let mut harness = UiHarness::new(app());
        let tree = harness.tree_mut();
        let root = tree.add_root(UiLayer::Game, LayoutStyle::column().with_padding(10.0));
        tree.button(root, "Start");
        harness.step(2);
        let report = harness
            .harness_mut()
            .run_script(&script(2, &json!([{ "frame": 1, "ui": ui }])));
        assert!(report.passed, "{report:#?}");
        assert!(harness.world().has_resource::<UiTree>(), "the tree is back");
        harness
            .harness()
            .draw()
            .expect("a frame ran")
            .ui
            .commands
            .len()
    };
    assert!(painted(true) > 0, "the menu paints with the UI on");
    assert_eq!(painted(false), 0);
}

#[test]
fn a_capture_in_the_harness_is_recorded_as_not_written() {
    let mut harness = Harness::new(app());
    let report = harness.run_script(&script(
        3,
        &json!([
            { "frame": 2, "capture": "shot.png" },
            { "frame": 1, "until": 3, "every": 2, "capture": "loop" },
        ]),
    ));
    assert!(!report.passed);
    let captures: Vec<(u32, &str, bool)> = report
        .captures
        .iter()
        .map(|c| (c.frame, c.path.as_str(), c.written))
        .collect();
    let loop_frame = |frame: &str| Path::new("loop").join(frame).display().to_string();
    let (first, last) = (loop_frame("0001.png"), loop_frame("0003.png"));
    assert_eq!(
        captures,
        [
            (1, first.as_str(), false),
            (2, "shot.png", false),
            (3, last.as_str(), false)
        ]
    );
}

#[test]
fn a_fatal_error_ends_the_report_failed_and_the_report_round_trips() {
    let script = script(3, &json!([{ "frame": 1, "probe": "frame" }]));
    let mut harness = Harness::new(app());
    let report = harness.run_script(&script);
    assert!(report.passed);
    assert_eq!(verdict(&report), "script 'harness' passed");

    // A render that fails after the script's own end still fails the run.
    let mut runner = ScriptRunner::new(script.clone(), "run.json", false);
    runner.ended = Some(Ending::Frames);
    let failed = runner.finish(
        harness.world_mut(),
        Ending::GameExit,
        Some("render failed at frame 3".to_owned()),
    );
    assert_eq!(failed.ended, Ending::Failed);
    assert!(!failed.passed);
    assert_eq!(
        verdict(&failed),
        "script 'run.json' did not pass: render failed at frame 3, 1 step unreached"
    );

    let dir = tempdir();
    let path = dir.join("out/report.json");
    write_report(&failed, &path).unwrap();
    let read: Report = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(read, failed);

    let setup = dir.join("setup.json");
    report_setup_failure(
        Path::new("run.json"),
        &setup,
        &anyhow::anyhow!("Invalid script 'run.json': steps[0].captur: unknown field"),
    );
    let read: Report = serde_json::from_str(&fs::read_to_string(&setup).unwrap()).unwrap();
    assert_eq!(
        (read.ended, read.passed, read.frames),
        (Ending::Failed, false, 0)
    );
    assert!(read.error.unwrap().contains("steps[0].captur"));

    let bad = dir.join("bad.json");
    fs::write(
        &bad,
        r#"{ "version": 1, "frames": 5, "steps": [{ "frame": 1, "captur": "a.png" }] }"#,
    )
    .unwrap();
    let err = load_script(&bad).unwrap_err().to_string();
    assert!(err.contains("steps[0].captur"), "{err}");
}
