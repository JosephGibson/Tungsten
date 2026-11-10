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
    // Each 16 × 16 sprite's top-left, so the sprite centres on its cell.
    let mut cells: Vec<Vec2> = (0..3)
        .map(|index| layout.center(index) - Vec2::splat(8.0))
        .collect();
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

#[test]
fn the_startup_probe_reads_the_timings() {
    // Headless, no window started: null.
    let mut harness = Harness::new(app());
    let report = harness.run_script(&script(1, &json!([{ "frame": 1, "probe": "startup" }])));
    assert_eq!(probes(&report, "startup"), [(1, &Value::Null)]);

    let timings = crate::StartupTimings {
        load: 12.5,
        first_frame: Some(45.0),
        loaded: 40.0,
        ..Default::default()
    };
    let mut app = app();
    app.world_mut().insert_resource(timings);
    let mut harness = Harness::new(app);
    let report = harness.run_script(&script(
        1,
        &json!([
            { "frame": 1, "probe": "startup" },
            { "frame": 1, "assert": { "probe": "startup", "path": "/loaded", "op": "==", "value": 40.0 } },
        ]),
    ));
    assert!(report.passed, "{report:#?}");
    assert_eq!(probes(&report, "startup"), [(1, &json!(timings))]);
}

/// An app whose `ui_screens` are `files` (ID, JSON).
fn screen_app(files: &[(&str, &str)]) -> App {
    let mut app = app();
    let mut registry = tungsten_core::ui::UiScreenRegistry::new();
    for (id, json) in files {
        let file = tungsten_core::ui::ScreenFile::from_json(json).expect("a screen file");
        registry.insert(*id, file);
    }
    app.world_mut().insert_resource(registry);
    app
}

#[test]
fn a_screen_step_opens_a_screen_file() {
    let pause = r#"{"version": 1, "root": {"kind": "screen", "width": 260,
        "children": [{"kind": "button", "key": "resume", "text": "Resume", "on": "pop"}]}}"#;
    let mut harness = Harness::new(screen_app(&[("pause", pause)]));
    let report = harness.run_script(&script(
        20,
        &json!([
            { "frame": 2, "screen": "pause" },
            { "frame": 20, "assert": { "probe": "states", "op": "==", "value": ["pause"] } },
        ]),
    ));
    assert!(report.passed, "{report:#?}");
}

#[test]
fn a_refused_screen_step_fails_with_its_reason() {
    let hud = r#"{"version": 1, "root": {"kind": "root", "layer": "game"}}"#;
    let mut harness = Harness::new(screen_app(&[("hud", hud)]));
    let report = harness.run_script(&script(
        3,
        &json!([
            { "frame": 2, "screen": "hud" },
            { "frame": 3, "screen": "nowhere" },
        ]),
    ));
    assert!(!report.passed);
    let failed: Vec<(u32, &str, &str)> = report
        .failed_steps
        .iter()
        .map(|f| (f.frame, f.verb.as_str(), f.error.as_str()))
        .collect();
    assert_eq!(
        failed,
        [
            (
                2,
                "screen",
                "ui screen 'hud': its root is not modal on the popup layer"
            ),
            (
                3,
                "screen",
                "no registered screen or ui_screens file is named 'nowhere'"
            ),
        ]
    );
}

#[test]
fn a_room_step_spawns_the_room_and_the_place_probe_reads_a_marker() {
    use tungsten_core::assets::{PrefabData, PrefabRegistry, Room, RoomRegistry, Tileset};
    let mut app = app();
    add_owned_probe(app.world_mut());
    let world = app.world_mut();
    register_sprite(world, "rock");
    let crate_prefab: PrefabData = serde_json::from_value(json!({ "tag": "crate" })).unwrap();
    world
        .resource_mut::<PrefabRegistry>()
        .insert("crate".into(), crate_prefab);
    let mut rooms = RoomRegistry::new();
    rooms.insert_tileset(
        "cave",
        Tileset::from_json(r#"{ "version": 1, "tiles": [{ "sprite": "rock" }] }"#).unwrap(),
    );
    world.insert_resource(rooms);
    let room = Room::from_json(
        r#"{ "version": 1, "cell": [16, 16], "size": [10, 4], "tileset": "cave",
             "layers": [{ "name": "solid", "kind": "collision" }],
             "shapes": [{ "id": "floor", "rect": [0, 3, 10, 1], "tile": "rock", "layers": ["solid"] }],
             "objects": [{ "id": "crate_1", "prefab": "crate", "at": [2, 2] }],
             "places": [{ "id": "start", "point": [1.5, 2] }] }"#,
    )
    .unwrap();
    world.insert_room("cave", room).unwrap();
    let mut harness = Harness::new(app);
    let report = harness.run_script(&script(
        10,
        &json!([
            { "frame": 2, "stage": {} },
            { "frame": 2, "room": "cave" },
            { "frame": 2, "camera": { "fit": "staged" } },
            { "frame": 5, "probe": "place", "args": { "room": "cave", "id": "start" } },
            { "frame": 5, "assert": { "probe": "owned", "args": { "state": "script_stage" }, "op": "==", "value": 2 } },
            { "frame": 6, "assert": { "probe": "place", "args": { "room": "cave", "id": "start" }, "path": "/0", "op": "==", "value": 24 } }
        ]),
    ));
    assert!(report.passed, "{report:#?}");
    assert_eq!(
        probes(&report, "place"),
        [(5, &json!([24.0, 32.0]))],
        "the marker in world pixels"
    );
    // The room spawned once, its crate named for its room.
    let world = harness.world();
    let crates = world
        .query::<&tungsten_core::Tag>()
        .filter(|tag| tag.name == "crate")
        .count();
    assert_eq!(crates, 1);
    assert!(world.resource::<RoomRegistry>().live("cave").is_some());
    // An unknown room fails its step and the run.
    let mut harness = Harness::new(crate::App::new(Config::default()).unwrap());
    let report = harness.run_script(&script(
        4,
        &json!([{ "frame": 2, "stage": {} }, { "frame": 2, "room": "cove" }]),
    ));
    assert!(!report.passed);
}

/// An app with the sprite `rock`, the prefab `crate`, the cave tileset and
/// the template `cave`: a floor, a crate and the place `start`.
fn cave_app() -> crate::App {
    use tungsten_core::assets::{PrefabData, PrefabRegistry, Room, RoomRegistry, Tileset};
    let mut app = app();
    add_owned_probe(app.world_mut());
    let world = app.world_mut();
    register_sprite(world, "rock");
    let crate_prefab: PrefabData = serde_json::from_value(json!({ "tag": "crate" })).unwrap();
    world
        .resource_mut::<PrefabRegistry>()
        .insert("crate".into(), crate_prefab);
    let mut rooms = RoomRegistry::new();
    rooms.insert_tileset(
        "cave",
        Tileset::from_json(r#"{ "version": 1, "tiles": [{ "sprite": "rock" }] }"#).unwrap(),
    );
    world.insert_resource(rooms);
    let room = Room::from_json(
        r#"{ "version": 1, "cell": [16, 16], "size": [10, 4], "tileset": "cave",
             "layers": [{ "name": "solid", "kind": "collision" }],
             "shapes": [{ "id": "floor", "rect": [0, 3, 10, 1], "tile": "rock", "layers": ["solid"] }],
             "objects": [{ "id": "crate_1", "prefab": "crate", "at": [2, 2] }],
             "places": [{ "id": "start", "point": [1.5, 2] }] }"#,
    )
    .unwrap();
    world.insert_room("cave", room).unwrap();
    app
}

/// Two copies of `cave`, 160 px apart; failing at seed 3.
struct Pair;

impl tungsten_core::assets::RoomGenerator for Pair {
    fn version(&self) -> u32 {
        1
    }

    fn generate(&self, generation: &mut tungsten_core::assets::Generate<'_>) -> Result<(), String> {
        if generation.seed() == 3 {
            return Err("no exit fits".to_owned());
        }
        let cave = generation.template("cave")?;
        generation.add("a", cave.clone(), [0.0, 0.0])?;
        generation.add("b", cave, [160.0, 0.0])
    }
}

#[test]
fn a_generate_step_spawns_each_room_at_its_origin() {
    use tungsten_core::assets::RoomRegistry;
    let mut app = cave_app();
    app.register_generator("pair", Pair);
    let mut harness = Harness::new(app);
    let report = harness.run_script(&script(
        10,
        &json!([
            { "frame": 2, "stage": {} },
            { "frame": 2, "generate": { "generator": "pair", "seed": 7 } },
            { "frame": 2, "camera": { "fit": "staged" } },
            { "frame": 5, "probe": "place", "args": { "room": "pair_b", "id": "start" } },
            { "frame": 5, "assert": { "probe": "owned", "args": { "state": "script_stage" }, "op": "==", "value": 4 } },
            { "frame": 6, "generate": { "generator": "pair", "seed": 8, "prefix": "deep" } },
            { "frame": 8, "probe": "place", "args": { "room": "deep_a", "id": "start" } },
            { "frame": 8, "assert": { "probe": "owned", "args": { "state": "script_stage" }, "op": "==", "value": 8 } }
        ]),
    ));
    assert!(report.passed, "{report:#?}");
    assert_eq!(
        probes(&report, "place"),
        [(5, &json!([184.0, 32.0])), (8, &json!([24.0, 32.0]))],
        "each room at its origin: `b` 160 px over"
    );
    let rooms = harness.world().resource::<RoomRegistry>();
    for room in ["pair_a", "pair_b", "deep_a", "deep_b"] {
        assert!(rooms.live(room).is_some(), "{room}");
        assert!(rooms.generation(room).is_some(), "{room}");
    }
}

#[test]
fn a_generate_step_that_fails_fails_the_script() {
    let mut app = cave_app();
    app.register_generator("pair", Pair);
    let mut harness = Harness::new(app);
    let report = harness.run_script(&script(
        6,
        &json!([
            { "frame": 2, "stage": {} },
            { "frame": 2, "generate": { "generator": "pair", "seed": 3 } },
            { "frame": 4, "generate": { "generator": "chain", "seed": 1 } }
        ]),
    ));
    assert!(!report.passed);
    let failed: Vec<(u32, &str, &str)> = report
        .failed_steps
        .iter()
        .map(|f| (f.frame, f.verb.as_str(), f.error.as_str()))
        .collect();
    assert_eq!(
        failed,
        [
            (2, "generate", "generator `pair` at seed 3: no exit fits"),
            (4, "generate", "no room generator `chain`"),
        ]
    );
    let rooms = harness
        .world()
        .resource::<tungsten_core::assets::RoomRegistry>();
    assert_eq!(
        rooms.ids().collect::<Vec<_>>(),
        ["cave"],
        "nothing registered"
    );
}

#[test]
fn a_level_step_patches_the_live_room_and_undo_restores_it() {
    use tungsten_core::assets::{
        PrefabData, PrefabRegistry, Room, RoomJournal, RoomRegistry, Tileset,
    };
    use tungsten_core::save::{SaveId, SaveState};
    let mut app = app();
    add_owned_probe(app.world_mut());
    let world = app.world_mut();
    world.add_script_probe("named_x", "A named entity's x.", |world, args| {
        let name = args["name"].as_str().ok_or("`name` names an entity")?;
        let entity = world
            .get_resource::<SaveState>()
            .and_then(|state| state.named_entity(SaveId::authored(name)))
            .filter(|&entity| world.is_alive(entity))
            .ok_or("no such entity")?;
        Ok(json!(world.get::<Transform>(entity).unwrap().position.x))
    });
    register_sprite(world, "rock");
    register_sprite(world, "plank");
    let crate_prefab: PrefabData = serde_json::from_value(json!({ "tag": "crate" })).unwrap();
    world
        .resource_mut::<PrefabRegistry>()
        .insert("crate".into(), crate_prefab);
    let mut rooms = RoomRegistry::new();
    rooms.insert_tileset(
        "cave",
        Tileset::from_json(
            r#"{ "version": 1, "tiles": [{ "sprite": "rock" }, { "sprite": "plank" }] }"#,
        )
        .unwrap(),
    );
    world.insert_resource(rooms);
    let room = Room::from_json(
        r#"{ "version": 1, "cell": [16, 16], "size": [10, 4], "tileset": "cave",
             "layers": [{ "name": "solid", "kind": "collision" }],
             "shapes": [{ "id": "floor", "rect": [0, 3, 10, 1], "tile": "rock", "layers": ["solid"] }],
             "objects": [{ "id": "crate_1", "prefab": "crate", "at": [2, 2] }] }"#,
    )
    .unwrap();
    world.insert_room("cave", room.clone()).unwrap();
    let mut harness = Harness::new(app);
    let crate_x = json!({ "name": "cave.crate_1" });
    let report = harness.run_script(&script(
        8,
        &json!([
            { "frame": 2, "stage": {} },
            { "frame": 2, "room": "cave" },
            { "frame": 3, "level": { "room": "cave", "apply": [
                { "move": { "id": "crate_1", "by": [3, 0] } },
                { "paint": { "layer": "solid", "cells": [[5, 1]], "tile": "plank" } }
            ] } },
            { "frame": 4, "probe": "named_x", "args": crate_x },
            { "frame": 4, "probe": "owned", "args": { "state": "script_stage" } },
            { "frame": 5, "level": { "room": "cave", "undo": true } },
            { "frame": 6, "probe": "named_x", "args": crate_x },
            { "frame": 7, "level": { "room": "cave", "undo": true } }
        ]),
    ));
    assert_eq!(
        probes(&report, "named_x"),
        [(4, &json!(80.0)), (6, &json!(32.0))],
        "moved by the batch, back by the undo"
    );
    assert_eq!(
        probes(&report, "owned"),
        [(4, &json!(2))],
        "the respawned crate takes the instance's state"
    );
    let failed: Vec<(u32, &str, &str)> = report
        .failed_steps
        .iter()
        .map(|f| (f.frame, f.verb.as_str(), f.error.as_str()))
        .collect();
    assert_eq!(failed, [(7, "level", "room `cave` has nothing to undo")]);
    let world = harness.world();
    assert_eq!(
        world.resource::<RoomRegistry>().room("cave"),
        Some(&room),
        "the document as it was"
    );
    let map = world.resource::<TilemapRegistry>().get("cave").unwrap();
    assert_eq!(map.tile_at(0, 5, 1), Some(-1), "the paint undone");
    let journal = world.resource::<RoomJournal>();
    assert_eq!((journal.undo_len("cave"), journal.redo_len("cave")), (0, 1));
}

#[test]
fn a_level_step_redoes_what_it_undid() {
    let mut harness = Harness::new(cave_app());
    let crate_at = json!({ "room": "cave", "id": "crate_1" });
    let report = harness.run_script(&script(
        10,
        &json!([
            { "frame": 2, "stage": {} },
            { "frame": 2, "room": "cave" },
            { "frame": 3, "level": { "room": "cave", "apply": [{ "move": { "id": "crate_1", "by": [3, 0] } }] } },
            { "frame": 4, "level": { "room": "cave", "undo": true } },
            { "frame": 5, "probe": "room", "args": crate_at, "path": "/entry/at" },
            { "frame": 6, "level": { "room": "cave", "redo": true } },
            { "frame": 7, "probe": "room", "args": crate_at, "path": "/entry/at" },
            { "frame": 7, "probe": "room", "args": { "room": "cave" }, "path": "/undo" },
            { "frame": 7, "probe": "room", "args": { "room": "cave" }, "path": "/redo" },
            { "frame": 8, "level": { "room": "cave", "redo": true } }
        ]),
    ));
    assert_eq!(
        probes(&report, "room"),
        [
            (5, &json!([2.0, 2.0])),
            (7, &json!([5.0, 2.0])),
            (7, &json!(1)),
            (7, &json!(0)),
        ],
        "back by the undo, moved again by the redo, the journal one batch deep"
    );
    let failed: Vec<(u32, &str, &str)> = report
        .failed_steps
        .iter()
        .map(|f| (f.frame, f.verb.as_str(), f.error.as_str()))
        .collect();
    assert_eq!(failed, [(8, "level", "room `cave` has nothing to redo")]);
}

#[test]
fn the_room_probe_reads_a_live_rooms_entries() {
    let mut harness = Harness::new(cave_app());
    let all = json!({ "room": "cave" });
    let report = harness.run_script(&script(
        8,
        &json!([
            { "frame": 1, "probe": "room", "args": all, "path": "/live" },
            { "frame": 2, "stage": {} },
            { "frame": 2, "room": "cave" },
            { "frame": 4, "probe": "room", "args": all },
            { "frame": 4, "probe": "room", "args": { "room": "cave", "id": "crate_1" } },
            { "frame": 4, "probe": "room", "args": { "room": "cave", "id": "start" } },
            { "frame": 4, "probe": "room", "args": { "room": "cave", "id": "floor" }, "path": "/kind" },
            { "frame": 5, "level": { "room": "cave", "apply": [
                { "place": { "objects": { "id": "crate_2", "prefab": "crate", "at": [6, 2] } } }
            ] } },
            { "frame": 6, "assert": { "probe": "room", "args": { "room": "cave", "id": "crate_2" }, "path": "/entry/world", "op": "==", "value": [96, 32] } },
            { "frame": 6, "assert": { "probe": "room", "args": { "room": "cave", "id": "crate_2" }, "path": "/entry/alive", "op": "==", "value": true } },
            { "frame": 7, "probe": "room", "args": { "room": "cave", "id": "crate_9" } },
            { "frame": 7, "assert": { "probe": "room", "args": { "room": "cave", "id": "crate_9" }, "path": "/kind", "op": "==", "value": null } },
            { "frame": 7, "assert": { "probe": "room", "args": { "room": "cave", "id": "crate_9" }, "path": "/entry/at", "op": "==", "value": [2, 2] } },
            { "frame": 7, "probe": "room", "args": { "room": "cove" } }
        ]),
    ));
    let read = probes(&report, "room");
    assert_eq!(read[0], (1, &json!(false)), "registered, not live yet");
    assert_eq!(
        read[1],
        (
            4,
            &json!({
                "room": "cave",
                "live": true,
                "origin": [0.0, 0.0],
                "cell": [16, 16],
                "size": [10, 4],
                "tileset": "cave",
                "layers": [{ "name": "solid", "kind": "collision", "cells": 0 }],
                "shapes": [{ "id": "floor", "rect": [0, 3, 10, 1], "tile": "rock", "layers": ["solid"] }],
                "objects": [{ "id": "crate_1", "prefab": "crate", "at": [2.0, 2.0], "world": [32.0, 32.0], "alive": true }],
                "places": [{ "id": "start", "point": [1.5, 2.0] }],
                "undo": 0,
                "redo": 0,
            })
        )
    );
    assert_eq!(
        read[2],
        (
            4,
            &json!({ "kind": "object", "index": 0, "entry": {
                "id": "crate_1", "prefab": "crate", "at": [2.0, 2.0], "world": [32.0, 32.0], "alive": true
            } })
        )
    );
    assert_eq!(
        read[3],
        (
            4,
            &json!({ "kind": "place", "index": 0, "entry": { "id": "start", "point": [1.5, 2.0] } })
        )
    );
    assert_eq!(read[4], (4, &json!("shape")));
    // An ID the room lacks reads `null` under each key, with the nearest
    // ID: a script asserts an entry is gone by `/kind == null`, and a
    // misspelt ID's assert fails showing what it may have meant.
    assert_eq!(
        read[5],
        (
            7,
            &json!({ "kind": null, "index": null, "entry": null, "nearest": "crate_1" })
        )
    );
    let passed: Vec<bool> = report.asserts.iter().map(|assert| assert.passed).collect();
    assert_eq!(passed, [true, true, true, false], "{report:#?}");
    let failed: Vec<&str> = report
        .failed_steps
        .iter()
        .map(|f| f.error.as_str())
        .collect();
    assert_eq!(
        failed,
        ["probe `room`: unknown room `cove`; did you mean `cave`?"]
    );
    let misspelt = report.asserts[3].error.as_deref().unwrap_or_default();
    assert!(
        misspelt.contains("/entry/at") && misspelt.contains("\"nearest\":\"crate_1\""),
        "{misspelt}"
    );
}
