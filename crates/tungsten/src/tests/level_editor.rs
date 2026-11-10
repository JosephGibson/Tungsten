//! The level editor on the headless harness over the rooms fixture's copy,
//! `cave` spawned at the origin: its cells are 16 px, so `crate_1` at
//! `[5, 19]` has its marker at (80, 304).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tungsten_core::assets::{ResolvedManifest, Room, RoomCommand, RoomJournal, RoomRegistry};
use tungsten_core::input::{InputState, KeyCode, Modifiers, MouseButton, RoutedInput};
use tungsten_core::save::{SaveId, SaveState};
use tungsten_core::ui::{UiKey, UiLayer, WidgetId};
use tungsten_core::{CameraState, Config, DebugDraw, Time, Transform, World};

use super::overlay::Mark;
use super::*;
use crate::App;
use crate::asset_loader::room::tests::fixture_copy;
use crate::asset_loader::{HeadlessTarget, LoadPlan};
use crate::testing::UiHarness;
use crate::{DebugHud, InspectorState};

/// What the room spawns its objects with.
#[derive(Debug, Clone, Copy)]
struct Marker;

/// The fixture's copy, changed by `prepare` before it loads, loaded into
/// an app on `config`, and room `spawn` spawned at the origin.
fn editor_with(config: Config, prepare: impl FnOnce(&Path), spawn: &str) -> (PathBuf, UiHarness) {
    editor_on(App::new(config).unwrap(), prepare, spawn)
}

/// [`editor_with`] over `app`, which a test gave its own systems.
fn editor_on(app: App, prepare: impl FnOnce(&Path), spawn: &str) -> (PathBuf, UiHarness) {
    let dir = fixture_copy();
    prepare(&dir);
    let mut harness = UiHarness::new(app);
    let world = harness.world_mut();
    let manifest = ResolvedManifest::load(dir.join("manifest.json")).unwrap();
    let mut plan = LoadPlan::new(manifest, world);
    let mut target = HeadlessTarget::default();
    while plan.run_next_into(world, &mut target).unwrap() {}
    if spawn != "none" {
        world.spawn_room(spawn, [0.0, 0.0], &(Marker,)).unwrap();
    }
    harness.step(2);
    (dir, harness)
}

/// [`editor_with`] on the default config, `cave` spawned.
fn editor() -> (PathBuf, UiHarness) {
    editor_with(Config::default(), |_| {}, "cave")
}

fn open(harness: &mut UiHarness) {
    harness.tap_action(TOGGLE);
    assert!(state(harness).open, "the editor opened");
}

fn state(harness: &UiHarness) -> &LevelEditor {
    harness.world().resource::<LevelEditor>()
}

fn document(world: &World, room: &str) -> Room {
    world.resource::<RoomRegistry>().room(room).unwrap().clone()
}

fn at(world: &World, room: &str, id: &str) -> [f32; 2] {
    document(world, room)
        .objects
        .iter()
        .find(|object| object.id == id)
        .unwrap_or_else(|| panic!("no object `{id}`"))
        .at
}

fn file(dir: &Path) -> String {
    fs::read_to_string(dir.join("rooms/cave.json")).unwrap()
}

fn journal(world: &World, room: &str) -> (usize, usize) {
    world
        .get_resource::<RoomJournal>()
        .map_or((0, 0), |journal| {
            (journal.undo_len(room), journal.redo_len(room))
        })
}

/// A press at `from`, a move to `to` and the release, a frame each, in
/// window pixels.
fn drag(harness: &mut UiHarness, from: Vec2, to: Vec2) {
    harness.pointer_to(from.x, from.y);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(to.x, to.y);
    harness.step(1);
    harness.pointer_up();
    harness.step(1);
}

/// `crate_1`'s marker, and one cell right of it.
const CRATE: Vec2 = Vec2::new(80.0, 304.0);
const RIGHT: Vec2 = Vec2::new(96.0, 304.0);

/// Taps `key` with `modifiers` held.
fn chord(harness: &mut UiHarness, modifiers: Modifiers, key: KeyCode) {
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .set_modifiers(modifiers);
    harness.tap_key(key);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .set_modifiers(Modifiers::empty());
}

fn named(world: &World, name: &str) -> Option<Entity> {
    world
        .get_resource::<SaveState>()?
        .named_entity(SaveId::authored(name))
        .filter(|&entity| world.is_alive(entity))
}

/// A writer that refuses, as a full disk or a read-only file does.
fn refusing(_: &Path, _: &str) -> Result<(), String> {
    Err("the disk is full".to_owned())
}

#[test]
fn f10_opens_a_modal_editor_that_stops_the_clock_and_takes_the_games_input() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    assert!(harness.world().resource::<Time>().is_editing());
    let root = state(&harness).window.as_ref().unwrap().root;
    let tree = harness.tree();
    assert_eq!(tree.key_name(root), Some("level_editor"));
    assert!(tree.modal(root), "its root is modal");
    assert_eq!(tree.layer(root), Some(UiLayer::Debug));
    // The next frame's left press reaches no routed input.
    harness.pointer_to(400.0, 500.0);
    harness.pointer_down();
    harness.step(1);
    assert!(
        !harness
            .world()
            .resource::<RoutedInput>()
            .mouse_just_pressed(MouseButton::Left)
    );
    harness.pointer_up();
    harness.step(1);
    harness.tap_action(TOGGLE);
    assert!(!state(&harness).open);
    assert!(!harness.world().resource::<Time>().is_editing());
    assert!(state(&harness).window.is_none(), "its window removed");
}

#[test]
fn hot_reload_off_keeps_the_editor_shut() {
    let mut config = Config::default();
    config.assets.hot_reload = Some(false);
    let (_dir, mut harness) = editor_with(config, |_| {}, "cave");
    harness.tap_action(TOGGLE);
    assert!(!state(&harness).open);
    assert!(!harness.world().resource::<Time>().is_editing());
    assert!(state(&harness).window.is_none());
}

#[test]
fn a_drag_moves_an_object_by_whole_cells_and_writes_its_file() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    drag(&mut harness, CRATE, RIGHT);
    let world = harness.world();
    assert_eq!(at(world, "cave", "crate_1"), [6.0, 19.0]);
    let crate_1 = named(world, "cave.crate_1").unwrap();
    assert_eq!(
        world.get::<Transform>(crate_1).unwrap().position,
        RIGHT,
        "respawned there"
    );
    assert_eq!(journal(world, "cave"), (1, 0), "one batch");
    assert_eq!(file(&dir), document(world, "cave").to_canonical());
    assert_eq!(state(&harness).status, "Moved `crate_1` by 1, 0.");
    assert!(!state(&harness).refused);
    assert_eq!(state(&harness).selection.as_deref(), Some("crate_1"));
}

#[test]
fn ctrl_z_and_ctrl_y_take_the_room_and_its_file_back_and_forth() {
    let (dir, mut harness) = editor();
    let before = file(&dir);
    open(&mut harness);
    drag(&mut harness, CRATE, RIGHT);
    let after = file(&dir);
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    let world = harness.world();
    assert_eq!(at(world, "cave", "crate_1"), [5.0, 19.0]);
    assert_eq!(journal(world, "cave"), (0, 1));
    assert_eq!(file(&dir), document(world, "cave").to_canonical());
    assert_eq!(
        Room::from_json(&file(&dir)).unwrap(),
        Room::from_json(&before).unwrap()
    );
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyY);
    assert_eq!(at(harness.world(), "cave", "crate_1"), [6.0, 19.0]);
    assert_eq!(file(&dir), after);
    // Ctrl+Shift+Z redoes too.
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    chord(
        &mut harness,
        Modifiers::CONTROL.union(Modifiers::SHIFT),
        KeyCode::KeyZ,
    );
    assert_eq!(at(harness.world(), "cave", "crate_1"), [6.0, 19.0]);
    assert_eq!(journal(harness.world(), "cave"), (1, 0));
}

#[test]
fn delete_removes_the_pick_and_ctrl_d_copies_it_a_cell_right() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    drag(&mut harness, CRATE, CRATE);
    assert_eq!(state(&harness).selection.as_deref(), Some("crate_1"));
    assert_eq!(
        journal(harness.world(), "cave"),
        (0, 0),
        "a click edits nothing"
    );
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyD);
    let world = harness.world();
    assert_eq!(at(world, "cave", "crate_2"), [6.0, 19.0]);
    assert!(named(world, "cave.crate_2").is_some(), "spawned");
    assert_eq!(state(&harness).selection.as_deref(), Some("crate_2"));
    assert_eq!(file(&dir), document(world, "cave").to_canonical());
    harness.tap_key(KeyCode::Delete);
    let world = harness.world();
    assert!(
        document(world, "cave")
            .objects
            .iter()
            .all(|object| object.id != "crate_2")
    );
    assert!(named(world, "cave.crate_2").is_none(), "despawned");
    assert_eq!(state(&harness).selection, None);
    assert_eq!(file(&dir), document(world, "cave").to_canonical());
    assert_eq!(journal(world, "cave"), (2, 0));
}

#[test]
fn a_press_over_a_debug_window_edits_nothing() {
    let (dir, mut harness) = editor();
    let before = file(&dir);
    harness.tap_key(KeyCode::F4);
    open(&mut harness);
    harness.tap_key(KeyCode::F3);
    let world = harness.world();
    let roots = [
        world.resource::<DebugHud>().window().unwrap(),
        state(&harness).window.as_ref().unwrap().root,
        world.resource::<InspectorState>().window().unwrap(),
    ];
    for root in roots {
        let rect = harness.tree().visual_rect(root).unwrap();
        let centre = (rect.min + rect.max) * 0.5;
        // The view put so `crate_1`'s marker lies under the window.
        harness.world_mut().resource_mut::<LevelEditor>().view = Some(View {
            position: CRATE - centre,
            zoom: 1.0,
        });
        drag(&mut harness, centre, centre + Vec2::new(16.0, 0.0));
        let world = harness.world();
        assert_eq!(at(world, "cave", "crate_1"), [5.0, 19.0], "{root:?}");
        assert_eq!(state(&harness).selection, None, "{root:?}");
        assert_eq!(journal(world, "cave"), (0, 0));
    }
    assert_eq!(file(&dir), before);
    // With the HUD and the inspector closed, off the editor's window, the
    // game's view picks it.
    harness.tap_key(KeyCode::F4);
    harness.tap_key(KeyCode::F3);
    let editor = harness.world_mut().resource_mut::<LevelEditor>();
    editor.view = None;
    // The press on the editor's own window fell on its panel, whose
    // buttons act: Select again, whatever it took up.
    editor.tool = Tool::Select;
    drag(&mut harness, CRATE, RIGHT);
    assert_eq!(at(harness.world(), "cave", "crate_1"), [6.0, 19.0]);
}

#[test]
fn a_failed_write_puts_the_room_the_file_and_the_journal_back() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    // A redo branch: two moves, one undone.
    drag(&mut harness, CRATE, RIGHT);
    drag(&mut harness, RIGHT, RIGHT + Vec2::new(16.0, 0.0));
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    let text = file(&dir);
    let live = document(harness.world(), "cave");
    assert_eq!(journal(harness.world(), "cave"), (1, 1));
    harness.world_mut().resource_mut::<LevelEditor>().writer = refusing;
    drag(&mut harness, RIGHT, RIGHT + Vec2::new(0.0, -16.0));
    let world = harness.world();
    assert_eq!(document(world, "cave"), live, "the live room");
    assert_eq!(file(&dir), text, "the file");
    assert_eq!(journal(world, "cave"), (1, 1), "the journal, redo kept");
    let crate_1 = named(world, "cave.crate_1").unwrap();
    assert_eq!(world.get::<Transform>(crate_1).unwrap().position, RIGHT);
    assert!(
        state(&harness)
            .status
            .starts_with("Can't move `crate_1`: '")
            && state(&harness)
                .status
                .ends_with("cannot be written (the disk is full); nothing edited."),
        "{}",
        state(&harness).status
    );
    assert!(state(&harness).refused);
    // Undo and redo give what they gave before.
    harness.world_mut().resource_mut::<LevelEditor>().writer = write_atomic;
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyY);
    assert_eq!(at(harness.world(), "cave", "crate_1"), [7.0, 19.0]);
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    assert_eq!(at(harness.world(), "cave", "crate_1"), [5.0, 19.0]);
    // The action lands when repeated with the writer back.
    drag(&mut harness, CRATE, CRATE + Vec2::new(0.0, -16.0));
    assert_eq!(at(harness.world(), "cave", "crate_1"), [5.0, 18.0]);
    assert_eq!(file(&dir), document(harness.world(), "cave").to_canonical());
}

#[test]
fn a_failed_write_over_a_full_journal_keeps_its_oldest_entry() {
    let (dir, mut harness) = editor();
    let original = document(harness.world(), "cave");
    let world = harness.world_mut();
    for _ in 0..RoomJournal::LIMIT {
        let batch =
            RoomCommand::parse_batch(r#"[{ "move": { "id": "ball_1", "by": [0.125, 0] } }]"#)
                .unwrap();
        world.apply_room_batch("cave", &batch).unwrap();
    }
    fs::write(
        dir.join("rooms/cave.json"),
        document(world, "cave").to_canonical(),
    )
    .unwrap();
    open(&mut harness);
    harness.world_mut().resource_mut::<LevelEditor>().writer = refusing;
    drag(&mut harness, CRATE, RIGHT);
    let world = harness.world_mut();
    assert_eq!(journal(world, "cave"), (RoomJournal::LIMIT, 0));
    for _ in 0..RoomJournal::LIMIT {
        world.undo_room_batch("cave").unwrap().unwrap();
    }
    assert_eq!(document(world, "cave"), original, "the oldest entry kept");
}

#[test]
fn a_room_sharing_its_file_with_another_refuses_edits() {
    let (dir, mut harness) = editor_with(
        Config::default(),
        |dir| {
            let path = dir.join("manifest.json");
            let mut manifest: Value =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            manifest["rooms"]["twin"] = json!({ "path": "rooms/cave.json" });
            fs::write(&path, manifest.to_string()).unwrap();
        },
        "cave",
    );
    let before = file(&dir);
    open(&mut harness);
    drag(&mut harness, CRATE, RIGHT);
    assert_eq!(at(harness.world(), "cave", "crate_1"), [5.0, 19.0]);
    assert_eq!(file(&dir), before);
    assert!(
        state(&harness)
            .status
            .contains("shares its file with room `twin`"),
        "{}",
        state(&harness).status
    );
}

#[test]
fn a_marked_objects_move_drops_its_mark() {
    let (dir, mut harness) = editor_with(
        Config::default(),
        |dir| {
            let path = dir.join("rooms/cave.json");
            let mut room: Value =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            room["objects"][0]["generator"] = json!("chain");
            let room = Room::from_json(&room.to_string()).unwrap();
            fs::write(&path, room.to_canonical()).unwrap();
        },
        "cave",
    );
    open(&mut harness);
    drag(&mut harness, CRATE, RIGHT);
    let crate_1 = document(harness.world(), "cave").objects[0].clone();
    assert_eq!((crate_1.at, crate_1.generator), ([6.0, 19.0], None));
    assert!(!file(&dir).contains("generator"), "written detached");
}

/// The fixture's `cave` as room `a`.
struct Copy;

impl tungsten_core::assets::RoomGenerator for Copy {
    fn version(&self) -> u32 {
        1
    }

    fn generate(&self, generation: &mut tungsten_core::assets::Generate<'_>) -> Result<(), String> {
        let cave = generation.template("cave")?;
        generation.add("a", cave, [0.0, 0.0])
    }
}

#[test]
fn a_generated_room_refuses_the_drag() {
    let (dir, mut harness) = editor_with(Config::default(), |_| {}, "none");
    let before = file(&dir);
    let world = harness.world_mut();
    world.register_generator("copy", Copy);
    world.generate_rooms("copy", 1, "run").unwrap();
    world.spawn_room("run_a", [0.0, 0.0], &(Marker,)).unwrap();
    harness.step(1);
    open(&mut harness);
    drag(&mut harness, CRATE, RIGHT);
    assert_eq!(at(harness.world(), "run_a", "crate_1"), [5.0, 19.0]);
    assert!(
        state(&harness)
            .status
            .starts_with("Can't move `crate_1`: `run_a` is a generated room"),
        "{}",
        state(&harness).status
    );
    assert_eq!(file(&dir), before);
}

#[test]
fn a_file_holding_another_document_refuses_the_edit() {
    let (dir, mut harness) = editor();
    let live = document(harness.world(), "cave");
    let mut other = live.clone();
    other.objects[2].at = [31.0, 12.0];
    fs::write(dir.join("rooms/cave.json"), other.to_canonical()).unwrap();
    open(&mut harness);
    drag(&mut harness, CRATE, RIGHT);
    assert_eq!(document(harness.world(), "cave"), live, "the live room");
    assert_eq!(file(&dir), other.to_canonical(), "the file");
    assert!(
        state(&harness)
            .status
            .contains("differs from the live `cave`"),
        "{}",
        state(&harness).status
    );
}

#[test]
fn a_room_with_no_file_is_edited_live_and_nothing_written() {
    let (dir, mut harness) = editor_with(Config::default(), |_| {}, "none");
    let before = file(&dir);
    let world = harness.world_mut();
    let cave = document(world, "cave");
    world.insert_room("nook", cave).unwrap();
    world.spawn_room("nook", [0.0, 0.0], &(Marker,)).unwrap();
    harness.step(1);
    open(&mut harness);
    drag(&mut harness, CRATE, RIGHT);
    assert_eq!(at(harness.world(), "nook", "crate_1"), [6.0, 19.0]);
    assert_eq!(
        state(&harness).status,
        "Moved `crate_1` by 1, 0. The room has no file, so the edit stays in the running game."
    );
    assert_eq!(file(&dir), before);
    assert!(!dir.join("rooms/nook.json").exists());
}

#[test]
fn the_arrows_and_the_wheel_move_the_drawn_camera_and_closing_gives_the_games_back() {
    let (_dir, mut harness) = editor();
    let game = *harness.world().resource::<CameraState>();
    open(&mut harness);
    harness.tap_key(KeyCode::ArrowRight);
    let camera = *harness.world().resource::<CameraState>();
    assert_eq!(camera.position, game.position + Vec2::new(16.0, 0.0));
    // A notch up at (640, 360) zooms ×1.25 about the point under it.
    let cursor = Vec2::new(640.0, 360.0);
    let under = camera.screen_to_world(cursor);
    harness.pointer_to(cursor.x, cursor.y);
    harness.wheel(1.0);
    harness.step(1);
    let camera = *harness.world().resource::<CameraState>();
    assert_eq!(camera.zoom, 1.25);
    assert!(camera.screen_to_world(cursor).distance(under) < 1e-3);
    harness.tap_key(KeyCode::Home);
    assert_eq!(*harness.world().resource::<CameraState>(), game);
    harness.tap_key(KeyCode::ArrowDown);
    assert_ne!(*harness.world().resource::<CameraState>(), game);
    harness.tap_action(TOGGLE);
    harness.step(1);
    assert_eq!(*harness.world().resource::<CameraState>(), game);
}

#[test]
fn a_closed_editor_queues_no_debug_draw_and_builds_no_window() {
    let (_dir, mut harness) = editor();
    let world = harness.world_mut();
    world.resource_mut::<DebugDraw>().clear();
    crate::debug_views::update_with(world, None);
    assert!(world.resource::<DebugDraw>().is_empty());
    assert!(world.resource::<LevelEditor>().window.is_none());
    // Open, it draws the room.
    world.resource_mut::<LevelEditor>().open = true;
    crate::debug_views::update_with(world, None);
    assert!(!world.resource::<DebugDraw>().is_empty());
    assert!(world.resource::<LevelEditor>().window.is_some());
}

#[test]
fn the_editor_fits_beside_the_five_views_at_the_owners_scale() {
    let (sink, feed) = crate::logging::console_channel(256);
    let (_dir, mut harness) = editor();
    {
        let world = harness.world_mut();
        *world.resource_mut::<crate::app::WindowSize>() = crate::app::WindowSize {
            width: 1280,
            height: 720,
        };
        world
            .resource_mut::<tungsten_core::ScaleFactor>()
            .set_os(1.75);
        world.resource_mut::<DebugHud>().enabled = true;
        world.resource_mut::<crate::SystemTimingOverlay>().enabled = true;
        world.resource_mut::<InspectorState>().enabled = true;
        world.resource_mut::<crate::UiInspector>().enabled = true;
        let mut console = crate::LogConsole::with_feed(Some(feed));
        console.enabled = true;
        console.refresh_interval_ms = 0.0;
        world.insert_resource(console);
    }
    for n in 0..60 {
        sink.send(format!("[2026-11-10T12:34:56Z INFO  game::run] line {n}\n").as_bytes());
    }
    open(&mut harness);
    harness.step(4);
    harness.assert_fits("the editor beside the five views at 1280×720 on 1.75");
}

/// The screen point at the middle of cave's cell `[col, row]`.
fn cell(col: u32, row: u32) -> Vec2 {
    Vec2::new(col as f32 * 16.0 + 8.0, row as f32 * 16.0 + 8.0)
}

/// A press at `cells[0]`, a move through the rest and the release, a
/// frame each.
fn stroke(harness: &mut UiHarness, cells: &[Vec2]) {
    harness.pointer_to(cells[0].x, cells[0].y);
    harness.pointer_down();
    harness.step(1);
    for at in &cells[1..] {
        harness.pointer_to(at.x, at.y);
        harness.step(1);
    }
    harness.pointer_up();
    harness.step(1);
}

fn secondary_click(harness: &mut UiHarness, at: Vec2) {
    harness.pointer_to(at.x, at.y);
    let input = harness.world_mut().resource_mut::<InputState>();
    input.mouse_down(MouseButton::Right);
    harness.step(1);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .mouse_up(MouseButton::Right);
    harness.step(1);
}

#[test]
fn place_puts_the_palettes_prefab_at_the_clicked_cells_corner() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    harness.tap_key(KeyCode::Digit2);
    // The prefabs sorted: ball, crate, lamp, walker.
    harness.tap_key(KeyCode::BracketRight);
    harness.tap_key(KeyCode::BracketRight);
    assert_eq!(state(&harness).prefab.as_deref(), Some("lamp"));
    drag(&mut harness, cell(3, 19), cell(3, 19));
    let world = harness.world();
    assert_eq!(
        at(world, "cave", "lamp_2"),
        [3.0, 19.0],
        "the cell's corner"
    );
    assert!(named(world, "cave.lamp_2").is_some(), "spawned");
    assert_eq!(state(&harness).selection.as_deref(), Some("lamp_2"));
    assert_eq!(file(&dir), document(world, "cave").to_canonical());
    assert_eq!(state(&harness).status, "Placed `lamp_2` at 3, 19.");
    assert!(
        state(&harness).findings.is_empty(),
        "it stands on the floor"
    );
}

/// A press of the secondary button at `cells[0]`, a move through the rest
/// and the release, a frame each.
fn secondary_stroke(harness: &mut UiHarness, cells: &[Vec2]) {
    harness.pointer_to(cells[0].x, cells[0].y);
    let input = harness.world_mut().resource_mut::<InputState>();
    input.mouse_down(MouseButton::Right);
    harness.step(1);
    for at in &cells[1..] {
        harness.pointer_to(at.x, at.y);
        harness.step(1);
    }
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .mouse_up(MouseButton::Right);
    harness.step(1);
}

/// Holds `modifiers` from here on; `Modifiers::empty()` lets go.
fn hold(harness: &mut UiHarness, modifiers: Modifiers) {
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .set_modifiers(modifiers);
}

#[test]
fn alt_click_takes_a_tile_and_a_prefab() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    // Alt with the button, in Select: the object's prefab and Place, and
    // no pick, drag or edit.
    hold(&mut harness, Modifiers::ALT);
    drag(&mut harness, CRATE, RIGHT);
    let editor = state(&harness);
    assert_eq!(editor.prefab.as_deref(), Some("crate"));
    assert_eq!(editor.tool, Tool::Place);
    assert_eq!(editor.selection, None);
    assert_eq!(
        editor.status,
        "Took prefab `crate` from `crate_1`: Place puts it down."
    );
    assert_eq!(journal(harness.world(), "cave"), (0, 0));
    // In Place, still held: a cell's topmost tile, its layer and Paint.
    drag(&mut harness, cell(2, 21), cell(2, 21));
    let editor = state(&harness);
    assert_eq!(editor.tile.as_deref(), Some("rock"));
    assert_eq!(
        editor.layer.as_deref(),
        Some("solid"),
        "the last layer holding one"
    );
    assert_eq!(editor.tool, Tool::Paint);
    assert_eq!(
        editor.status,
        "Took tile `rock` and layer `solid`: Paint writes there."
    );
    // The press that took the tile started no stroke.
    assert_eq!(journal(harness.world(), "cave"), (0, 0));
    assert_eq!(
        state(&harness).hint,
        "Click an object: take its prefab. Click a cell: take its tile and layer. At 2, 21 in `cave`."
    );
    hold(&mut harness, Modifiers::empty());

    // The Pick tool is the same eyedropper without the key, for one take.
    harness.tap_key(KeyCode::Digit5);
    assert_eq!(state(&harness).tool, Tool::Pick);
    drag(&mut harness, CRATE, CRATE);
    assert_eq!(state(&harness).tool, Tool::Place);
    harness.tap_key(KeyCode::Digit5);
    drag(&mut harness, cell(20, 2), cell(20, 2));
    assert_eq!(state(&harness).status, "Cell 20, 2 holds no tile to take.");
    assert_eq!(
        state(&harness).tool,
        Tool::Pick,
        "nothing taken, the tool stays"
    );
    assert_eq!(journal(harness.world(), "cave"), (0, 0));
}

#[test]
fn the_secondary_button_erases_cells_and_deletes_an_object() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    // Under Paint it erases a stroke of cells, one undo step.
    harness.tap_key(KeyCode::Digit3);
    secondary_stroke(&mut harness, &[cell(1, 21), cell(2, 21)]);
    assert_eq!(state(&harness).status, "Erased 2 cells of art and solid.");
    assert_eq!(state(&harness).tool, Tool::Paint, "the tool stays");
    let room = document(harness.world(), "cave");
    assert_eq!(room.layers[1].cells.get("2,21"), Some(&None));
    assert_eq!(journal(harness.world(), "cave"), (1, 0));
    assert_eq!(file(&dir), room.to_canonical());
    // Under Select it deletes the object under the cursor, picked or not.
    harness.tap_key(KeyCode::Digit1);
    secondary_click(&mut harness, CRATE);
    assert_eq!(state(&harness).status, "Deleted `crate_1`.");
    let room = document(harness.world(), "cave");
    assert!(room.objects.iter().all(|object| object.id != "crate_1"));
    assert_eq!(named(harness.world(), "cave.crate_1"), None);
    assert_eq!(file(&dir), room.to_canonical());
    // Under Place too; on nothing it does nothing.
    harness.tap_key(KeyCode::Digit2);
    let lamp = Vec2::from(at(harness.world(), "cave", "lamp_1")) * 16.0 + Vec2::splat(2.0);
    secondary_click(&mut harness, lamp);
    assert_eq!(state(&harness).status, "Deleted `lamp_1`.");
    secondary_click(&mut harness, cell(20, 2));
    assert_eq!(journal(harness.world(), "cave"), (3, 0));
    // Each is undone as any edit is.
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    assert_eq!(at(harness.world(), "cave", "lamp_1"), [8.0, 19.0]);
}

#[test]
fn shift_makes_a_stroke_a_rectangle() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    click(&mut harness, "tool_paint");
    click(&mut harness, "item_plank");
    click(&mut harness, "layer_art");
    hold(&mut harness, Modifiers::SHIFT);
    harness.pointer_to(cell(1, 10).x, cell(1, 10).y);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(cell(4, 12).x, cell(4, 12).y);
    harness.step(1);
    assert_eq!(
        state(&harness).hint,
        "Release: paint a 4 \u{d7} 3 rectangle of art with `plank`. Esc drops it. At 4, 12 in `cave`."
    );
    // The overlay draws the rectangle as one box.
    let boxes = overlay(&harness).marks;
    assert!(boxes.iter().any(|mark| matches!(
        mark,
        Mark::Aabb(min, max, _) if *min == Vec2::new(16.0, 160.0) && *max == Vec2::new(80.0, 208.0)
    )));
    // Shift let go before the release changes nothing: the press chose.
    hold(&mut harness, Modifiers::empty());
    harness.pointer_up();
    harness.step(1);
    assert_eq!(
        state(&harness).status,
        "Painted a 4 \u{d7} 3 rectangle of art with `plank`."
    );
    let room = document(harness.world(), "cave");
    for col in 1..=4 {
        for row in 10..=12 {
            let key = format!("{col},{row}");
            assert_eq!(
                room.layers[0].cells.get(&key),
                Some(&Some("plank".to_owned()))
            );
        }
    }
    assert_eq!(room.layers[0].cells.get("5,12"), None);
    assert_eq!(journal(harness.world(), "cave"), (1, 0), "one batch");
    assert_eq!(file(&dir), room.to_canonical());

    // The secondary button's rectangle erases, and a corner dragged out of
    // the room stays inside it.
    hold(&mut harness, Modifiers::SHIFT);
    secondary_stroke(&mut harness, &[cell(38, 22), Vec2::new(900.0, 600.0)]);
    hold(&mut harness, Modifiers::empty());
    assert_eq!(
        state(&harness).status,
        "Erased a 2 \u{d7} 2 rectangle of art."
    );
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    let room = document(harness.world(), "cave");
    assert_eq!(
        room.layers[0].cells.get("1,10"),
        None,
        "the rectangle undone whole"
    );
}

#[test]
fn escape_drops_a_drag() {
    let (dir, mut harness) = editor();
    let before = file(&dir);
    open(&mut harness);
    harness.pointer_to(CRATE.x, CRATE.y);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(CRATE.x + 32.0, CRATE.y);
    harness.step(1);
    assert_eq!(
        state(&harness).hint,
        "Release: move `crate_1` by 2, 0. Esc drops it. At 7, 19 in `cave`."
    );
    harness.tap_key(KeyCode::Escape);
    assert!(
        state(&harness).open,
        "Escape is the gesture's, not the window's"
    );
    assert!(
        !harness.world().has_resource::<crate::app::Exiting>(),
        "nor the game's: the modal root keeps the key from `engine_exit`"
    );
    harness.pointer_up();
    harness.step(1);
    assert_eq!(at(harness.world(), "cave", "crate_1"), [5.0, 19.0]);
    assert_eq!(journal(harness.world(), "cave"), (0, 0));
    assert_eq!(state(&harness).status, "Dropped the move: nothing changed.");
    assert_eq!(state(&harness).selection.as_deref(), Some("crate_1"));
    // With no gesture under way it drops the pick.
    harness.tap_key(KeyCode::Escape);
    assert_eq!(state(&harness).selection, None);
    // A stroke is dropped the same way.
    harness.tap_key(KeyCode::Digit3);
    harness.pointer_to(cell(1, 10).x, cell(1, 10).y);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(cell(3, 10).x, cell(3, 10).y);
    harness.step(1);
    harness.tap_key(KeyCode::Escape);
    harness.pointer_up();
    harness.step(1);
    assert_eq!(
        state(&harness).status,
        "Dropped the stroke: nothing changed."
    );
    assert_eq!(journal(harness.world(), "cave"), (0, 0));
    assert_eq!(file(&dir), before);
}

#[test]
fn a_paint_stroke_is_one_batch_a_paint_per_layer_written_in_place() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    harness.tap_key(KeyCode::Digit3);
    // The tileset's order: rock, rock_45_br, rock_low_br, rock_high_br,
    // plank, arch.
    for _ in 0..4 {
        harness.tap_key(KeyCode::BracketRight);
    }
    assert_eq!(state(&harness).tile.as_deref(), Some("plank"));
    // Two frames' moves cross three cells, the middle one jumped over.
    stroke(&mut harness, &[cell(1, 10), cell(3, 10)]);
    let world = harness.world();
    assert_eq!(journal(world, "cave"), (1, 0), "one batch");
    let room = document(world, "cave");
    for layer in &room.layers {
        for key in ["1,10", "2,10", "3,10"] {
            assert_eq!(
                layer.cells.get(key),
                Some(&Some("plank".to_owned())),
                "{} {key}",
                layer.name
            );
        }
    }
    let map = world
        .resource::<tungsten_core::assets::TilemapRegistry>()
        .get("cave")
        .unwrap();
    let plank = map
        .tileset
        .iter()
        .position(|sprite| sprite == "plank")
        .unwrap();
    for col in 1..=3 {
        assert_eq!(map.tile_at(0, col, 10), Some(plank as i32), "in place");
    }
    assert_eq!(file(&dir), room.to_canonical());
    assert_eq!(
        state(&harness).status,
        "Painted 3 cells of art and solid with `plank`."
    );
    // Erase empties them.
    harness.tap_key(KeyCode::Digit4);
    stroke(&mut harness, &[cell(1, 10), cell(2, 10), cell(3, 10)]);
    let world = harness.world();
    let room = document(world, "cave");
    for layer in &room.layers {
        assert_eq!(layer.cells.get("2,10"), Some(&None), "{}", layer.name);
    }
    let map = world
        .resource::<tungsten_core::assets::TilemapRegistry>()
        .get("cave")
        .unwrap();
    assert_eq!(map.tile_at(1, 2, 10), Some(tungsten_core::EMPTY_TILE));
    assert_eq!(journal(world, "cave"), (2, 0));
}

#[test]
fn l_cycles_each_layer_then_all() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    // A click in the room makes it the current one.
    drag(&mut harness, cell(20, 6), cell(20, 6));
    assert_eq!(state(&harness).current.as_deref(), Some("cave"));
    let mut seen = Vec::new();
    for _ in 0..3 {
        harness.tap_key(KeyCode::KeyL);
        seen.push(state(&harness).layer.clone());
    }
    assert_eq!(
        seen,
        [Some("art".to_owned()), Some("solid".to_owned()), None]
    );
}

#[test]
fn a_lamp_placed_in_mid_air_shows_one_finding_and_its_undo_none() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.tap_key(KeyCode::Digit2);
    harness.world_mut().resource_mut::<LevelEditor>().prefab = Some("lamp".to_owned());
    drag(&mut harness, cell(10, 5), cell(10, 5));
    let findings = state(&harness).findings.clone();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(findings[0].contains("lamp_2"), "{findings:?}");
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    assert!(state(&harness).findings.is_empty());
}

/// The panel's widget keyed `key`.
fn keyed(harness: &UiHarness, key: &str) -> WidgetId {
    harness
        .find_key(UiKey::new(key))
        .unwrap_or_else(|| panic!("no widget keyed `{key}`"))
}

/// Clicks the panel's widget keyed `key`, and runs the frame that acts.
fn click(harness: &mut UiHarness, key: &str) {
    let widget = keyed(harness, key);
    harness.click(widget);
    harness.step(1);
}

/// The text of the panel's widget keyed `key`: a label's, or a button's.
fn text(harness: &UiHarness, key: &str) -> String {
    harness.tree().plain_text(keyed(harness, key))
}

/// The texts of the labels under the panel's list keyed `key`, in order,
/// a row's being its label's.
fn rows(harness: &UiHarness, key: &str) -> Vec<String> {
    let tree = harness.tree();
    tree.children(keyed(harness, key))
        .iter()
        .map(|&row| {
            let own = tree.plain_text(row);
            if own.is_empty() {
                (tree.children(row).iter())
                    .map(|&child| tree.plain_text(child))
                    .find(|text| !text.is_empty())
                    .unwrap_or_default()
            } else {
                own
            }
        })
        .collect()
}

/// Calls the `editor` script action with `args`; an error is the editor's
/// own, without the hooks' `action `editor`: ` before it.
fn script(harness: &mut UiHarness, args: &Value) -> Result<(), String> {
    tungsten_core::script::ScriptHooks::call_action(harness.world_mut(), "editor", args).map_err(
        |err| match err.strip_prefix("action `editor`: ") {
            Some(own) => own.to_owned(),
            None => err,
        },
    )
}

/// Reads the `editor` script probe.
fn probe(harness: &UiHarness) -> Value {
    tungsten_core::script::ScriptHooks::read_probe(harness.world(), "editor", &Value::Null).unwrap()
}

#[test]
fn every_action_has_a_button() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    // Each tool, and each thing a key does, is a keyed button of the panel.
    for tool in Tool::ALL {
        let button = keyed(&harness, &format!("tool_{}", tool.name()));
        assert_eq!(harness.tree().plain_text(button), tool.caption());
    }
    for key in [
        "undo", "redo", "delete", "copy", "fit", "home", "grid", "rename", "set", "link",
    ] {
        keyed(&harness, key);
    }
    // Every key's action is a tool's, a button's or a list row's.
    let keys = [
        (KeyCode::Digit1, false, false),
        (KeyCode::Digit2, false, false),
        (KeyCode::Digit3, false, false),
        (KeyCode::Digit4, false, false),
        (KeyCode::Digit5, false, false),
        (KeyCode::KeyG, false, false),
        (KeyCode::Digit6, false, false),
        (KeyCode::KeyR, false, false),
        (KeyCode::BracketLeft, false, false),
        (KeyCode::BracketRight, false, false),
        (KeyCode::KeyL, false, false),
        (KeyCode::KeyF, false, false),
        (KeyCode::Home, false, false),
        (KeyCode::Delete, false, false),
        (KeyCode::Backspace, false, false),
        (KeyCode::KeyD, true, false),
        (KeyCode::KeyZ, true, false),
        (KeyCode::KeyZ, true, true),
        (KeyCode::KeyY, true, false),
    ];
    for (code, control, shift) in keys {
        let action = key_action(code, control, shift).unwrap_or_else(|| panic!("{code:?}"));
        let on_the_panel = match action {
            Action::Tool(_)
            | Action::Undo
            | Action::Redo
            | Action::Delete
            | Action::Copy
            | Action::Fit
            | Action::GameView
            | Action::Grid
            | Action::Prompt(_) => true,
            // The palette's and the layer's rows are their steps' buttons.
            Action::PaletteStep(_) => !rows(&harness, "palette").is_empty(),
            Action::LayerStep => rows(&harness, "layers").len() == 3,
            Action::Room(_)
            | Action::Prefab(_)
            | Action::Tile(_)
            | Action::Layer(_)
            | Action::Select(_) => false,
        };
        assert!(on_the_panel, "{code:?} asks for {action:?}");
    }
    // A key that is no action asks for none; Ctrl turns a plain key off.
    assert_eq!(key_action(KeyCode::KeyQ, false, false), None);
    assert_eq!(key_action(KeyCode::Digit2, true, false), None);
    // The keys are written out on the panel.
    assert!(harness.shows("Del: delete   Ctrl+D: copy"));
    assert!(harness.shows("right button: erase cells, delete an object"));
    // Escape drops a gesture under way, which no button can: the one key
    // that is no action.
    assert_eq!(key_action(KeyCode::Escape, false, false), None);
    assert!(harness.shows("Esc: drop a drag or the pick   F10: close"));
}

#[test]
fn a_tool_button_picks_its_tool_and_marks_it() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    let filled = |harness: &UiHarness, tool: Tool| {
        let button = keyed(harness, &format!("tool_{}", tool.name()));
        harness
            .tree()
            .overrides(button)
            .and_then(|style| style.background)
            .unwrap()
    };
    let active = filled(&harness, Tool::Select);
    for tool in [Tool::Place, Tool::Paint, Tool::Erase, Tool::Select] {
        click(&mut harness, &format!("tool_{}", tool.name()));
        assert_eq!(state(&harness).tool, tool);
        harness.step(1);
        for other in Tool::ALL {
            assert_eq!(
                filled(&harness, other) == active,
                other == tool,
                "{other:?} while {tool:?} is taken up"
            );
        }
    }
    // The click was the panel's: nothing in the world was picked or edited.
    assert_eq!(state(&harness).selection, None);
    assert_eq!(journal(harness.world(), "cave"), (0, 0));
}

#[test]
fn a_palette_row_picks_its_prefab_and_its_tile() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    // Select shows the prefabs, the first one chosen.
    assert_eq!(text(&harness, "palette").as_str(), "");
    assert_eq!(
        rows(&harness, "palette"),
        ["ball", "crate", "lamp", "walker"]
    );
    // A prefab's row takes it and takes up Place.
    click(&mut harness, "item_lamp");
    assert_eq!(state(&harness).prefab.as_deref(), Some("lamp"));
    assert_eq!(state(&harness).tool, Tool::Place);
    drag(&mut harness, cell(3, 19), cell(3, 19));
    assert_eq!(at(harness.world(), "cave", "lamp_2"), [3.0, 19.0]);
    // Paint shows the room's tiles; a tile's row takes it.
    click(&mut harness, "tool_paint");
    harness.step(1);
    assert_eq!(
        rows(&harness, "palette"),
        [
            "rock",
            "rock_45_br",
            "rock_low_br",
            "rock_high_br",
            "plank",
            "arch"
        ]
    );
    click(&mut harness, "item_plank");
    assert_eq!(state(&harness).tile.as_deref(), Some("plank"));
    assert_eq!(state(&harness).tool, Tool::Paint);
    stroke(&mut harness, &[cell(1, 10), cell(2, 10)]);
    let room = document(harness.world(), "cave");
    assert_eq!(
        room.layers[0].cells.get("2,10"),
        Some(&Some("plank".to_owned()))
    );
    assert_eq!(file(&dir), room.to_canonical());
    // Erase keeps the tiles in view; a tile's row there takes up Paint.
    click(&mut harness, "tool_erase");
    harness.step(1);
    assert_eq!(rows(&harness, "palette").len(), 6);
    click(&mut harness, "item_rock");
    assert_eq!(state(&harness).tool, Tool::Paint);
}

#[test]
fn a_layer_row_picks_the_target_layer() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    assert_eq!(
        rows(&harness, "layers"),
        ["all  every layer", "art  render", "solid  collision"]
    );
    click(&mut harness, "layer_solid");
    assert_eq!(state(&harness).layer.as_deref(), Some("solid"));
    click(&mut harness, "tool_erase");
    stroke(&mut harness, &[cell(1, 21), cell(2, 21)]);
    let room = document(harness.world(), "cave");
    assert_eq!(
        room.layers[1].cells.get("1,21"),
        Some(&None),
        "solid erased"
    );
    assert_eq!(room.layers[0].cells.get("1,21"), None, "art left");
    assert_eq!(state(&harness).status, "Erased 2 cells of solid.");
    click(&mut harness, "layer_all");
    assert_eq!(state(&harness).layer, None);
}

#[test]
fn a_refusal_reads_as_a_sentence_and_wraps() {
    let (dir, mut harness) = editor();
    let before = file(&dir);
    open(&mut harness);
    // A drag out of the room's bottom: the move is refused.
    drag(&mut harness, CRATE, CRATE + Vec2::new(0.0, 16.0 * 12.0));
    let editor = state(&harness);
    assert_eq!(
        editor.status,
        "Can't move `crate_1`: [5, 31] is past the room's 40 \u{d7} 24 cells."
    );
    assert!(editor.refused);
    assert_eq!(file(&dir), before);
    harness.step(1);
    // The panel shows the whole sentence, in the error colour, on lines of
    // its own width: no window grows to hold it on one.
    assert_eq!(text(&harness, "status"), editor_status(&harness));
    let status = keyed(&harness, "status");
    let tree = harness.tree();
    let style = tree.overrides(status).unwrap();
    assert_eq!(style.text.color, Some(crate::debug_views::ERROR_TEXT));
    let rect = tree.rect(status).unwrap();
    assert!(rect.width() <= 300.0, "{}", rect.width());
    assert!(rect.height() >= 2.0 * 20.0, "wrapped: {}", rect.height());
    // A button's refusal reads the same way.
    drag(&mut harness, cell(20, 6), cell(20, 6));
    click(&mut harness, "delete");
    assert_eq!(
        state(&harness).status,
        "Nothing is picked to delete: click an object with Select."
    );
    assert!(state(&harness).refused);
    // An edit that lands clears the refusal.
    drag(&mut harness, CRATE, RIGHT);
    assert!(!state(&harness).refused);
    harness.step(1);
    let style = harness.tree().overrides(keyed(&harness, "status")).unwrap();
    assert_eq!(style.text.color, Some(crate::debug_views::VALUE));
}

fn editor_status(harness: &UiHarness) -> String {
    state(harness).status.clone()
}

#[test]
fn no_live_room_says_so() {
    // Rooms listed, none spawned.
    let (_dir, mut harness) = editor_with(Config::default(), |_| {}, "none");
    open(&mut harness);
    harness.step(2);
    assert_eq!(state(&harness).current, None);
    let hint = text(&harness, "hint");
    assert_eq!(
        hint,
        "No room is live: the game has spawned none of its 2 rooms. Click one under Rooms to open it here."
    );
    assert_eq!(
        rows(&harness, "rooms"),
        ["cave  click to open", "hall  click to open"]
    );
    // A click in the world edits nothing and the buttons say why.
    drag(&mut harness, CRATE, RIGHT);
    assert_eq!(journal(harness.world(), "cave"), (0, 0));
    click(&mut harness, "undo");
    assert_eq!(state(&harness).status, "Nothing to undo: no room is live.");
    click(&mut harness, "fit");
    assert_eq!(state(&harness).status, "No room to fit: none is live.");

    // A game with no room at all.
    let mut harness = UiHarness::new(App::new(Config::default()).unwrap());
    harness.step(2);
    open(&mut harness);
    harness.step(2);
    assert_eq!(
        text(&harness, "hint"),
        "No room to edit: the game lists none. `tungsten level new <room>` makes one, and the game spawns it with `world.spawn_room`."
    );
}

#[test]
fn a_read_only_room_says_why() {
    let (_dir, mut harness) = editor_with(Config::default(), |_| {}, "none");
    let world = harness.world_mut();
    world.register_generator("copy", Copy);
    world.generate_rooms("copy", 1, "run").unwrap();
    world.spawn_room("run_a", [0.0, 0.0], &(Marker,)).unwrap();
    harness.step(1);
    open(&mut harness);
    harness.step(2);
    // Before any edit is tried: its row and the hint say it.
    assert_eq!(state(&harness).current.as_deref(), Some("run_a"));
    assert_eq!(
        rows(&harness, "rooms"),
        [
            "run_a  read only",
            "cave  click to open",
            "hall  click to open"
        ]
    );
    assert_eq!(
        text(&harness, "hint"),
        "`run_a` is read only: generator `copy` made it for this run. `tungsten level generate` writes rooms to edit."
    );
}

#[test]
fn the_editor_action_drives_every_control() {
    let (dir, mut harness) = editor();
    // Closed, it takes only `open`.
    assert_eq!(
        script(&mut harness, &json!({ "tool": "paint" })),
        Err("the level editor is closed: `{\"open\": true}` opens it".to_owned())
    );
    script(&mut harness, &json!({ "open": true })).unwrap();
    assert!(state(&harness).open);
    assert!(harness.world().resource::<Time>().is_editing());
    assert_eq!(
        state(&harness).current.as_deref(),
        Some("cave"),
        "current at once"
    );
    harness.step(1);
    // A tool, a palette pick, a layer and a pick, as the buttons give them.
    script(
        &mut harness,
        &json!({ "tool": "paint", "tile": "plank", "layer": "art" }),
    )
    .unwrap();
    let editor = state(&harness);
    assert_eq!(
        (editor.tool, editor.tile.as_deref(), editor.layer.as_deref()),
        (Tool::Paint, Some("plank"), Some("art"))
    );
    script(&mut harness, &json!({ "prefab": "lamp" })).unwrap();
    assert_eq!(state(&harness).tool, Tool::Place, "a prefab takes up Place");
    script(&mut harness, &json!({ "tool": "select", "prefab": "ball" })).unwrap();
    assert_eq!(
        state(&harness).tool,
        Tool::Select,
        "a tool named beside it stands"
    );
    script(&mut harness, &json!({ "layer": null, "select": "crate_1" })).unwrap();
    assert_eq!(state(&harness).layer, None);
    assert_eq!(state(&harness).selection.as_deref(), Some("crate_1"));
    // What its buttons do, by name: edits land in the file.
    script(&mut harness, &json!({ "do": "copy" })).unwrap();
    assert_eq!(at(harness.world(), "cave", "crate_2"), [6.0, 19.0]);
    script(&mut harness, &json!({ "do": "delete" })).unwrap();
    script(&mut harness, &json!({ "do": "undo" })).unwrap();
    assert_eq!(journal(harness.world(), "cave"), (1, 1));
    script(&mut harness, &json!({ "do": "redo" })).unwrap();
    assert_eq!(journal(harness.world(), "cave"), (2, 0));
    assert_eq!(file(&dir), document(harness.world(), "cave").to_canonical());
    script(&mut harness, &json!({ "do": "palette_next" })).unwrap();
    assert_eq!(state(&harness).prefab.as_deref(), Some("crate"));
    script(&mut harness, &json!({ "do": "layer_next" })).unwrap();
    assert_eq!(state(&harness).layer.as_deref(), Some("art"));
    script(&mut harness, &json!({ "do": "fit" })).unwrap();
    assert!(state(&harness).view.is_some());
    script(&mut harness, &json!({ "do": "game_view" })).unwrap();
    assert_eq!(state(&harness).view, None);
    script(&mut harness, &json!({ "room": "cave" })).unwrap();
    // What it refuses fails the step with its sentence.
    for (args, error) in [
        (
            json!({ "tool": "brush" }),
            "no tool `brush`: select, place, paint, erase, pick, mark",
        ),
        (json!({ "prefab": "ghost" }), "No prefab `ghost`."),
        (
            json!({ "tile": "grass" }),
            "Room `cave`'s tileset has no tile `grass`.",
        ),
        (json!({ "layer": "sky" }), "The room has no layer `sky`."),
        (
            json!({ "select": "crate_9" }),
            "The room has no object or place `crate_9`.",
        ),
        (
            json!({ "room": "attic" }),
            "No room `attic`: the game lists cave, hall.",
        ),
        (
            json!({ "do": "explode" }),
            "`do` names no action `explode`: undo, redo, delete, copy, fit, game_view, grid, palette_next, palette_previous, layer_next",
        ),
        (
            json!({ "select": null, "do": "delete" }),
            "Nothing is picked to delete: click an object with Select.",
        ),
    ] {
        assert_eq!(script(&mut harness, &args), Err(error.to_owned()), "{args}");
    }
    assert!(
        script(&mut harness, &json!({ "tol": "paint" }))
            .unwrap_err()
            .starts_with("args: unknown field `tol`")
    );
    script(&mut harness, &json!({ "open": false })).unwrap();
    assert!(!state(&harness).open);
    assert!(!harness.world().resource::<Time>().is_editing());
}

#[test]
fn the_editor_probe_reads_its_state() {
    let (_dir, mut harness) = editor();
    let closed = probe(&harness);
    assert_eq!(closed["available"], true);
    assert_eq!(closed["open"], false);
    open(&mut harness);
    drag(&mut harness, CRATE, RIGHT);
    harness.pointer_to(RIGHT.x, RIGHT.y);
    harness.step(1);
    let read = probe(&harness);
    assert_eq!(
        read,
        json!({
            "available": true,
            "open": true,
            "tool": "select",
            "room": "cave",
            "rooms": ["cave"],
            "opened": [],
            "layer": null,
            "prefab": "ball",
            "tile": "rock",
            "selection": "crate_1",
            "status": "Moved `crate_1` by 1, 0.",
            "refused": false,
            "hint": "Click: pick `crate_1`. Drag: move it by cells. Right click: delete it. At 6, 19 in `cave`.",
            "undo": 1,
            "redo": 0,
            "findings": [],
            "grid": true,
            "prompt": null,
            "view": null,
        })
    );
    harness.tap_key(KeyCode::ArrowRight);
    assert_eq!(
        probe(&harness)["view"],
        json!({ "position": [16.0, 0.0], "zoom": 1.0 })
    );
}

#[test]
fn the_panel_fits_at_the_owners_scale() {
    for os in [1.0, 1.75] {
        let (_dir, mut harness) = editor();
        {
            let world = harness.world_mut();
            *world.resource_mut::<crate::app::WindowSize>() = crate::app::WindowSize {
                width: 1280,
                height: 720,
            };
            world
                .resource_mut::<tungsten_core::ScaleFactor>()
                .set_os(os);
        }
        open(&mut harness);
        harness.step(4);
        harness.assert_fits(&format!("the editor's panel at 1280\u{d7}720 on {os}"));
        // The whole panel, or all of it but what its body scrolls, shows:
        // the status, the tools and the palette are inside the window.
        let tree = harness.tree();
        let window = state(&harness).window.as_ref().unwrap();
        let root = tree.visual_rect(window.root).unwrap();
        for key in ["status", "hint", "tool_select", "undo", "link", "layers"] {
            let rect = tree.visual_rect(keyed(&harness, key)).unwrap();
            assert!(
                rect.min.y >= root.min.y && rect.max.y <= root.max.y,
                "`{key}` at {os}: {rect:?} in {root:?}"
            );
        }
        // The palette, which scrolls in itself, starts in view.
        let palette = tree.visual_rect(keyed(&harness, "palette")).unwrap();
        assert!(palette.min.y + 20.0 < root.max.y, "{palette:?} in {root:?}");
        // Taller than the other views' 60 %: it has the height to itself.
        let viewport = tree.layer_viewport(UiLayer::Debug).unwrap();
        assert!(
            root.height() > 0.6 * viewport.y,
            "{} of {viewport} at {os}",
            root.height()
        );
    }
}

#[test]
fn an_idle_open_editor_writes_no_node() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.pointer_to(RIGHT.x, RIGHT.y);
    harness.step(3);
    let writes = |harness: &UiHarness| state(harness).panel.as_ref().unwrap().writes;
    let before = writes(&harness);
    assert!(before > 0, "it was written once");
    harness.step(30);
    assert_eq!(writes(&harness), before, "30 idle frames");
    // Moving the cursor writes the hint, and nothing else.
    harness.pointer_to(RIGHT.x + 64.0, RIGHT.y);
    harness.step(1);
    assert_eq!(writes(&harness), before + 1);
}

/// What the editor draws in the world this frame.
fn overlay(harness: &UiHarness) -> super::overlay::Overlay {
    let world = harness.world();
    let editor = state(harness);
    let camera = editor.drawn(editor.game_view(world));
    let rooms = live_rooms(world, editor.current.as_deref());
    super::overlay::gather(editor, world, &rooms, camera)
}

/// The overlay's lines: the grid's.
fn lines_of(harness: &UiHarness) -> Vec<(Vec2, Vec2)> {
    (overlay(harness).marks.iter())
        .filter_map(|mark| match mark {
            Mark::Line(a, b, _) => Some((*a, *b)),
            _ => None,
        })
        .collect()
}

/// Whether the overlay holds a box from `min` to `max`.
fn boxed(harness: &UiHarness, min: Vec2, max: Vec2) -> bool {
    overlay(harness)
        .marks
        .iter()
        .any(|mark| matches!(mark, Mark::Aabb(a, b, _) if *a == min && *b == max))
}

#[test]
fn the_grid_draws_the_views_cells_alone() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.step(1);
    // The fixture's routes are lines too: the grid's are what G takes away.
    harness.tap_key(KeyCode::KeyG);
    assert!(!state(&harness).grid);
    let routes = lines_of(&harness).len();
    harness.tap_key(KeyCode::KeyG);
    let grid = |harness: &UiHarness| lines_of(harness).len() - routes;
    // The game's view holds the room whole, 40 by 24 cells of 16 px: the 39
    // and 23 lines between them, the room's edges being its bounds'.
    assert_eq!(grid(&harness), 39 + 23);
    // Zoomed in, the lines the view holds, each cut to it: 320 by 180 px
    // of the world at 4, so 20 columns' and 11 rows' lines.
    harness.world_mut().resource_mut::<LevelEditor>().view = Some(View {
        position: Vec2::ZERO,
        zoom: 4.0,
    });
    harness.step(1);
    assert_eq!(grid(&harness), 20 + 11);
    for (a, b) in lines_of(&harness).into_iter().take(31) {
        assert!(
            a.x <= 320.0 && b.x <= 320.0 && a.y <= 180.0 && b.y <= 180.0,
            "{a} {b}"
        );
    }
    // Zoomed out until a cell is under 6 px, none.
    harness.world_mut().resource_mut::<LevelEditor>().view = Some(View {
        position: Vec2::ZERO,
        zoom: 0.25,
    });
    harness.step(1);
    assert_eq!(grid(&harness), 0);
    // The Grid button is the key's, marked while the grid is on.
    harness.world_mut().resource_mut::<LevelEditor>().view = None;
    let fill = |harness: &UiHarness| {
        (harness.tree().overrides(keyed(harness, "grid")))
            .and_then(|style| style.background)
            .unwrap()
    };
    harness.step(1);
    let on = fill(&harness);
    click(&mut harness, "grid");
    assert!(!state(&harness).grid);
    harness.step(1);
    assert_ne!(fill(&harness), on);
    assert_eq!(grid(&harness), 0);
}

#[test]
fn a_ghost_shows_where_place_will_put_its_prefab() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    // Place: the prefab's sprite, 8 by 16 px for the lamp, from the cell's
    // top-left, where the object's spawn point will be.
    click(&mut harness, "item_lamp");
    harness.pointer_to(cell(3, 19).x, cell(3, 19).y);
    harness.step(1);
    assert_eq!(
        state(&harness).hint,
        "Click: place `lamp` at 3, 19. At 3, 19 in `cave`."
    );
    assert!(boxed(
        &harness,
        Vec2::new(48.0, 304.0),
        Vec2::new(56.0, 320.0)
    ));
    // Paint and Erase: the cell.
    click(&mut harness, "tool_paint");
    harness.pointer_to(cell(3, 19).x, cell(3, 19).y);
    harness.step(1);
    assert!(boxed(
        &harness,
        Vec2::new(48.0, 304.0),
        Vec2::new(64.0, 320.0)
    ));
    assert!(!boxed(
        &harness,
        Vec2::new(48.0, 304.0),
        Vec2::new(56.0, 320.0)
    ));
    // Select: no ghost, but a box and the ID of the object under the
    // cursor, and each room's ID at its corner.
    click(&mut harness, "tool_select");
    harness.pointer_to(cell(3, 19).x, cell(3, 19).y);
    harness.step(1);
    assert!(!boxed(
        &harness,
        Vec2::new(48.0, 304.0),
        Vec2::new(64.0, 320.0)
    ));
    let labels = |harness: &UiHarness| -> Vec<String> {
        (overlay(harness).labels.into_iter())
            .map(|(text, ..)| text)
            .collect()
    };
    assert_eq!(labels(&harness), ["cave"]);
    harness.pointer_to(CRATE.x + 4.0, CRATE.y + 4.0);
    harness.step(1);
    assert_eq!(labels(&harness), ["cave", "crate_1"]);
    assert!(boxed(
        &harness,
        Vec2::new(80.0, 304.0),
        Vec2::new(96.0, 320.0)
    ));
    // Off the room, and over the panel, nothing is promised.
    click(&mut harness, "tool_place");
    harness.pointer_to(700.0, 500.0);
    harness.step(1);
    assert_eq!(state(&harness).hint, "Click in a room: place `lamp`.");
    // The frame's labels reach the frame's draw.
    let drawn = &harness.harness().draw().unwrap().world_labels;
    assert!(
        drawn.iter().any(|label| label.content == "cave"),
        "{drawn:?}"
    );
}

#[test]
fn fit_shows_the_room_beside_the_panel() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    harness.tap_key(KeyCode::KeyF);
    let editor = state(&harness);
    let view = editor.view.expect("a view of its own");
    let (panel, _) = editor.panel_rect.expect("the panel's box");
    // The room, 640 by 384 px, whole in the strip left of the panel, at a
    // whole zoom.
    let camera = editor.drawn(CameraState::new());
    let min = camera.world_to_screen(Vec2::ZERO);
    let max = camera.world_to_screen(Vec2::new(640.0, 384.0));
    assert!(min.x >= 0.0 && min.y >= 0.0, "{min}");
    assert!(max.x <= panel.x && max.y <= 720.0, "{max} beside {panel}");
    assert_eq!(view.zoom, view.zoom.floor());
    assert!(view.zoom >= 1.0);
    // Centred in the strip.
    assert!((min.x - (panel.x - max.x)).abs() <= 1.0, "{min} {max}");
}

#[test]
fn a_room_of_thousands_draws_what_the_view_holds() {
    const OBJECTS: usize = 4000;
    let (_dir, mut harness) = editor_with(
        Config::default(),
        |dir| {
            // A prefab with a sprite and no body, and 4,000 of it over
            // the room's 40 by 24 cells.
            fs::write(
                dir.join("prefabs/peg.json"),
                r#"{ "sprite": { "asset_id": "ball" }, "tag": "peg" }"#,
            )
            .unwrap();
            let path = dir.join("manifest.json");
            let mut manifest: Value =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            manifest["prefabs"]["peg"] = json!({ "path": "prefabs/peg.json" });
            fs::write(&path, manifest.to_string()).unwrap();
            let path = dir.join("rooms/cave.json");
            let mut room: Value =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            let objects = room["objects"].as_array_mut().unwrap();
            for n in 0..OBJECTS {
                objects.push(json!({
                    "id": format!("peg_{}", n + 1),
                    "prefab": "peg",
                    "at": [n % 40, (n / 40) % 24],
                }));
            }
            fs::write(&path, room.to_string()).unwrap();
        },
        "cave",
    );
    open(&mut harness);
    harness.step(1);
    let boxes = |harness: &UiHarness| {
        (overlay(harness).marks.iter())
            .filter(|mark| matches!(mark, Mark::Aabb(..)))
            .count()
    };
    // The game's view holds the room: every object's marker.
    assert!(boxes(&harness) > OBJECTS);
    // Zoomed in to 160 by 90 px of it, with the margin about 14 by 10
    // cells of the 960: the markers there, a small share of the room's.
    harness.world_mut().resource_mut::<LevelEditor>().view = Some(View {
        position: Vec2::new(320.0, 192.0),
        zoom: 8.0,
    });
    harness.pointer_to(400.0, 300.0);
    harness.step(1);
    let near = boxes(&harness);
    assert!(near < OBJECTS / 4, "{near}");
    assert!(near > 100, "{near}");
    // The object under the cursor is still found, and a click picks it.
    assert!(state(&harness).hover.is_some());
    harness.pointer_down();
    harness.step(1);
    harness.pointer_up();
    harness.step(1);
    assert!(state(&harness).selection.is_some());
}

/// Whether room `id` has a live instance.
fn live(world: &World, id: &str) -> bool {
    world
        .resource::<RoomRegistry>()
        .live(id)
        .is_some_and(|instance| world.is_alive(instance))
}

#[test]
fn the_rooms_list_names_rooms_that_are_not_live() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    // The live room with its file, then the rest, greyed: a click away.
    assert_eq!(
        rows(&harness, "rooms"),
        ["cave  cave.json", "hall  click to open"]
    );
    let ink = |harness: &UiHarness, key: &str| {
        let tree = harness.tree();
        let label = tree.children(keyed(harness, key))[1];
        tree.overrides(label).and_then(|style| style.text.color)
    };
    assert_ne!(ink(&harness, "room_hall"), ink(&harness, "room_cave"));
    assert_eq!(ink(&harness, "room_hall"), Some(crate::debug_views::LABEL));
    assert_eq!(probe(&harness)["rooms"], json!(["cave"]));
}

#[test]
fn a_click_opens_a_room_beside_the_live_ones() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    click(&mut harness, "room_hall");
    let world = harness.world();
    assert!(live(world, "hall"));
    // Two of its 16 px cells right of `cave`'s 640 px, at its top.
    let instance = world.resource::<RoomRegistry>().live("hall").unwrap();
    assert_eq!(
        world
            .get::<tungsten_core::assets::TilemapInstance>(instance)
            .unwrap()
            .origin,
        Vec2::new(672.0, 0.0)
    );
    // Under the top state, or the editor's own with none on the stack.
    assert!(world.get::<crate::state::SceneEntity>(instance).is_some());
    let editor = state(&harness);
    assert_eq!(editor.current.as_deref(), Some("hall"));
    assert_eq!(editor.opened, ["hall"]);
    assert!(editor.view.is_some(), "fitted");
    assert_eq!(
        editor.status,
        "Opened `hall` beside the live rooms. Closing the editor puts it away."
    );
    harness.step(1);
    assert_eq!(
        rows(&harness, "rooms"),
        ["cave  cave.json", "hall  hall.json"]
    );
    assert_eq!(probe(&harness)["opened"], json!(["hall"]));
    // It is edited as any live room is: checked, journaled, written.
    script(&mut harness, &json!({ "prefab": "crate" })).unwrap();
    let floor = state(&harness)
        .drawn(CameraState::new())
        .world_to_screen(Vec2::new(672.0 + 3.0 * 16.0 + 8.0, 9.0 * 16.0 + 8.0));
    drag(&mut harness, floor, floor);
    assert_eq!(state(&harness).status, "Placed `crate_1` at 3, 9.");
    assert_eq!(at(harness.world(), "hall", "crate_1"), [3.0, 9.0]);
    assert_eq!(
        fs::read_to_string(dir.join("rooms/hall.json")).unwrap(),
        document(harness.world(), "hall").to_canonical()
    );
    // The same from a script.
    let (_dir, mut harness) = self::editor();
    script(&mut harness, &json!({ "open": true, "room": "hall" })).unwrap();
    assert!(live(harness.world(), "hall"));
}

#[test]
fn a_room_with_saved_progress_is_not_opened() {
    // The game had `hall` live in this run and left it: its area is still
    // among the World's loaded ones.
    let (_dir, mut harness) = editor();
    let world = harness.world_mut();
    let hall = world.spawn_room("hall", [700.0, 0.0], &(Marker,)).unwrap();
    world.despawn(hall.instance());
    open(&mut harness);
    harness.step(2);
    assert_eq!(
        rows(&harness, "rooms"),
        ["cave  cave.json", "hall  not live: the game opens it"]
    );
    click(&mut harness, "room_hall");
    assert_eq!(
        state(&harness).status,
        "Can't open `hall`: the game has had it live in this run, and a spawn here would write over what changed. It opens once the game spawns it."
    );
    assert!(state(&harness).refused);
    assert!(!live(harness.world(), "hall"));
    assert!(state(&harness).opened.is_empty());

    // A loaded save holds `hall`, which this run has not entered.
    let (_dir, mut source) = editor();
    source
        .world_mut()
        .spawn_room("hall", [700.0, 0.0], &(Marker,))
        .unwrap();
    let save = tungsten_core::save::capture_world(source.world()).unwrap();
    let (_dir, mut harness) = editor();
    tungsten_core::save::restore_world(harness.world_mut(), &save, &(Marker,)).unwrap();
    assert!(harness.world().resource::<SaveState>().is_kept("hall"));
    open(&mut harness);
    harness.step(2);
    click(&mut harness, "room_hall");
    assert_eq!(
        state(&harness).status,
        "Can't open `hall`: a loaded save holds progress there, which a spawn here would write over. It opens once the game spawns it."
    );
    assert!(!live(harness.world(), "hall"));
    assert!(harness.world().resource::<SaveState>().is_kept("hall"));
}

#[test]
fn closing_puts_away_the_rooms_the_editor_opened() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    click(&mut harness, "room_hall");
    script(&mut harness, &json!({ "prefab": "crate" })).unwrap();
    let floor = state(&harness)
        .drawn(CameraState::new())
        .world_to_screen(Vec2::new(672.0 + 56.0, 152.0));
    drag(&mut harness, floor, floor);
    assert!(named(harness.world(), "hall.crate_1").is_some());
    harness.tap_action(TOGGLE);
    assert!(!state(&harness).open);
    let world = harness.world();
    assert!(!live(world, "hall"), "put away");
    assert_eq!(named(world, "hall.crate_1"), None);
    assert!(world.resource::<RoomRegistry>().room("hall").is_some());
    let saves = world.resource::<SaveState>();
    assert!(!saves.is_loaded("hall"), "no save of this run holds it");
    assert!(state(&harness).opened.is_empty());
    // The edit stays in its file and its journal, and it opens again.
    assert_eq!(journal(world, "hall"), (1, 0));
    open(&mut harness);
    harness.step(2);
    click(&mut harness, "room_hall");
    assert!(live(harness.world(), "hall"));
    assert!(named(harness.world(), "hall.crate_1").is_some());
}

#[test]
fn a_room_the_game_spawned_stays_when_the_editor_closes() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    click(&mut harness, "room_hall");
    // The game's room is made current again, edited, and left live.
    click(&mut harness, "room_cave");
    assert_eq!(state(&harness).current.as_deref(), Some("cave"));
    assert_eq!(state(&harness).opened, ["hall"]);
    harness.tap_action(TOGGLE);
    let world = harness.world();
    assert!(live(world, "cave"));
    assert!(named(world, "cave.crate_1").is_some());
    assert!(world.resource::<SaveState>().is_loaded("cave"));
    assert!(!live(world, "hall"));
}

#[test]
fn closing_gives_back_the_post_stack_and_the_ambient_light() {
    use tungsten_core::{AmbientLight, PostStack};
    // `hall` gives an ambient light and `den`, its copy, a post stack.
    let prepare = |dir: &Path| {
        let path = dir.join("rooms/hall.json");
        let mut hall: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let mut den = hall.clone();
        hall["ambient"] = json!([0.25, 0.5, 1.0]);
        fs::write(&path, hall.to_string()).unwrap();
        den["post"] = json!([{ "kind": "pixelate", "params": 4 }]);
        den["places"] = json!([]);
        fs::write(dir.join("rooms/den.json"), den.to_string()).unwrap();
        let path = dir.join("manifest.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        manifest["rooms"]["den"] = json!({ "path": "rooms/den.json" });
        fs::write(&path, manifest.to_string()).unwrap();
    };
    let look = |world: &World| {
        (
            world.get_resource::<PostStack>().map(|post| post.0.len()),
            world.get_resource::<AmbientLight>().map(|light| light.0),
        )
    };
    // What the game had comes back, whichever room set which.
    let (_dir, mut harness) = editor_with(Config::default(), prepare, "cave");
    let world = harness.world_mut();
    world.insert_resource(PostStack(Vec::new()));
    world.insert_resource(AmbientLight(glam::Vec3::splat(0.5)));
    let before = look(harness.world());
    open(&mut harness);
    harness.step(2);
    click(&mut harness, "room_hall");
    click(&mut harness, "room_den");
    assert_eq!(
        look(harness.world()),
        (Some(1), Some(glam::Vec3::new(0.25, 0.5, 1.0))),
        "the rooms' own while they are open"
    );
    harness.tap_action(TOGGLE);
    assert_eq!(look(harness.world()), before);

    // A World with neither ends with neither.
    let (_dir, mut harness) = editor_with(Config::default(), prepare, "cave");
    let world = harness.world_mut();
    world.remove_resource::<PostStack>();
    world.remove_resource::<AmbientLight>();
    open(&mut harness);
    harness.step(2);
    click(&mut harness, "room_den");
    click(&mut harness, "room_hall");
    assert_eq!(look(harness.world()).0, Some(1));
    harness.tap_action(TOGGLE);
    assert_eq!(look(harness.world()), (None, None));
}

/// Whether the World had `hall`'s area loaded in each last frame's
/// `PreUpdate`, where a game saves on quit.
#[derive(Debug, Default)]
struct SeenAtExit(Vec<bool>);

#[test]
fn a_last_frame_closes_the_editor_before_its_save() {
    use tungsten_core::schedule::{Stage, system};
    for close_the_window in [true, false] {
        let mut app = App::new(Config::default()).unwrap();
        app.add_system_to(
            Stage::PreUpdate,
            system("save_on_quit", |world: &mut World| {
                if world.has_resource::<crate::app::Exiting>() {
                    let loaded = world
                        .get_resource::<SaveState>()
                        .is_some_and(|saves| saves.is_loaded("hall"));
                    world.resource_mut::<SeenAtExit>().0.push(loaded);
                }
            }),
        );
        let (_dir, mut harness) = editor_on(app, |_| {}, "cave");
        harness.world_mut().insert_resource(SeenAtExit::default());
        open(&mut harness);
        harness.step(2);
        click(&mut harness, "room_hall");
        assert!(harness.world().resource::<SaveState>().is_loaded("hall"));
        if close_the_window {
            // The window's close: `Exiting` at once, the next frame the last.
            harness.harness_mut().request_close();
            harness.step(1);
        } else {
            // A game's Quit: the frame after the next is the last.
            crate::request_exit(harness.world_mut());
            harness.step(2);
        }
        let world = harness.world();
        assert_eq!(
            world.resource::<SeenAtExit>().0,
            [false],
            "close_the_window: {close_the_window}"
        );
        assert!(!state(&harness).open);
        assert!(!live(world, "hall"));
        assert!(live(world, "cave"));
        assert!(!world.resource::<Time>().is_editing());
    }
}

/// Types `text` into the open prompt, a key press a character, on the US
/// layout the prompt reads.
fn type_text(harness: &mut UiHarness, text: &str) {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA,
        KeyCode::KeyB,
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyE,
        KeyCode::KeyF,
        KeyCode::KeyG,
        KeyCode::KeyH,
        KeyCode::KeyI,
        KeyCode::KeyJ,
        KeyCode::KeyK,
        KeyCode::KeyL,
        KeyCode::KeyM,
        KeyCode::KeyN,
        KeyCode::KeyO,
        KeyCode::KeyP,
        KeyCode::KeyQ,
        KeyCode::KeyR,
        KeyCode::KeyS,
        KeyCode::KeyT,
        KeyCode::KeyU,
        KeyCode::KeyV,
        KeyCode::KeyW,
        KeyCode::KeyX,
        KeyCode::KeyY,
        KeyCode::KeyZ,
    ];
    const DIGITS: [KeyCode; 10] = [
        KeyCode::Digit0,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    let other = |code: winit::keyboard::KeyCode| KeyCode::Other(code as u32);
    for c in text.chars() {
        let (key, shift) = match c {
            'a'..='z' => (LETTERS[(c as u8 - b'a') as usize], false),
            'A'..='Z' => (LETTERS[(c as u8 - b'A') as usize], true),
            '0'..='9' => (DIGITS[(c as u8 - b'0') as usize], false),
            ' ' => (KeyCode::Space, false),
            '-' => (KeyCode::Minus, false),
            '_' => (KeyCode::Minus, true),
            '=' => (KeyCode::Equal, false),
            '[' => (KeyCode::BracketLeft, false),
            ']' => (KeyCode::BracketRight, false),
            '.' => (other(winit::keyboard::KeyCode::Period), false),
            ',' => (other(winit::keyboard::KeyCode::Comma), false),
            '\'' => (other(winit::keyboard::KeyCode::Quote), false),
            '"' => (other(winit::keyboard::KeyCode::Quote), true),
            _ => panic!("the prompt types no `{c}`"),
        };
        if shift {
            chord(harness, Modifiers::SHIFT, key);
        } else {
            harness.tap_key(key);
        }
    }
}

/// The open prompt's text.
fn prompt_text(harness: &UiHarness) -> Option<&str> {
    state(harness)
        .prompt
        .as_ref()
        .map(|prompt| prompt.text.as_str())
}

fn place<'a>(room: &'a Room, id: &str) -> &'a tungsten_core::assets::RoomPlace {
    room.places
        .iter()
        .find(|place| place.id == id)
        .unwrap_or_else(|| panic!("no place `{id}`"))
}

#[test]
fn the_prompt_types_backspaces_commits_and_cancels() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    drag(&mut harness, CRATE, CRATE);
    harness.tap_key(KeyCode::KeyR);
    // It opens on the pick's ID, shown in the hint with a caret.
    assert_eq!(prompt_text(&harness), Some("crate_1"));
    harness.step(1);
    assert_eq!(
        text(&harness, "hint"),
        "Rename `crate_1` to: crate_1_  (Enter commits, Esc cancels)"
    );
    for _ in 0..7 {
        harness.tap_key(KeyCode::Backspace);
    }
    assert_eq!(prompt_text(&harness), Some(""));
    harness.tap_key(KeyCode::Backspace);
    assert_eq!(prompt_text(&harness), Some(""), "nothing left to take");
    // Every character it types, by its US key.
    type_text(&mut harness, "aZ09 -_=[].,'\"");
    assert_eq!(prompt_text(&harness), Some("aZ09 -_=[].,'\""));
    // A key with no character types nothing: Shift+1, Tab, an arrow.
    chord(&mut harness, Modifiers::SHIFT, KeyCode::Digit1);
    harness.tap_key(KeyCode::Tab);
    harness.tap_key(KeyCode::ArrowLeft);
    assert_eq!(prompt_text(&harness), Some("aZ09 -_=[].,'\""));
    for _ in 0..14 {
        harness.tap_key(KeyCode::Backspace);
    }
    type_text(&mut harness, "guard_2");
    harness.tap_key(KeyCode::Enter);
    // Enter commits: one `rename`, written, the pick following the ID.
    let editor = state(&harness);
    assert_eq!(editor.prompt, None);
    assert_eq!(editor.status, "Renamed `crate_1` to `guard_2`.");
    assert_eq!(editor.selection.as_deref(), Some("guard_2"));
    assert_eq!(at(harness.world(), "cave", "guard_2"), [5.0, 19.0]);
    assert_eq!(journal(harness.world(), "cave"), (1, 0));
    assert_eq!(file(&dir), document(harness.world(), "cave").to_canonical());
    // Escape cancels: nothing renamed.
    harness.tap_key(KeyCode::KeyR);
    type_text(&mut harness, "x");
    assert_eq!(prompt_text(&harness), Some("guard_2x"));
    harness.tap_key(KeyCode::Escape);
    assert_eq!(state(&harness).prompt, None);
    assert_eq!(state(&harness).status, "Left as it was.");
    assert_eq!(state(&harness).selection.as_deref(), Some("guard_2"));
    assert_eq!(journal(harness.world(), "cave"), (1, 0));
    // The same ID again is no edit.
    harness.tap_key(KeyCode::KeyR);
    harness.tap_key(KeyCode::Enter);
    assert_eq!(state(&harness).status, "`guard_2` keeps its name.");
    assert_eq!(journal(harness.world(), "cave"), (1, 0));
    // It holds 96 characters.
    harness.tap_key(KeyCode::KeyR);
    for _ in 0..120 {
        harness.tap_key(KeyCode::KeyA);
    }
    assert_eq!(prompt_text(&harness).unwrap().len(), 96);
}

#[test]
fn no_editor_key_acts_while_the_prompt_is_open() {
    let (dir, mut harness) = editor();
    let before = file(&dir);
    open(&mut harness);
    drag(&mut harness, CRATE, CRATE);
    harness.tap_key(KeyCode::KeyR);
    // A tool's key, the grid's, the palette's and the layer's type or do
    // nothing; Delete, the arrows and Ctrl+Z do nothing.
    for key in [
        KeyCode::Digit3,
        KeyCode::KeyG,
        KeyCode::KeyL,
        KeyCode::KeyF,
        KeyCode::BracketRight,
        KeyCode::Delete,
        KeyCode::ArrowRight,
        KeyCode::Home,
        KeyCode::KeyR,
    ] {
        harness.tap_key(key);
    }
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyD);
    let editor = state(&harness);
    assert_eq!(prompt_text(&harness), Some("crate_13glf]r"));
    assert_eq!(editor.tool, Tool::Select);
    assert!(editor.grid);
    assert_eq!(editor.layer, None);
    assert_eq!(editor.view, None);
    assert_eq!(editor.selection.as_deref(), Some("crate_1"));
    assert_eq!(journal(harness.world(), "cave"), (0, 0));
    // A press in the world waits for the prompt too: nothing moves.
    drag(&mut harness, CRATE, RIGHT);
    secondary_click(&mut harness, CRATE);
    assert_eq!(at(harness.world(), "cave", "crate_1"), [5.0, 19.0]);
    assert_eq!(file(&dir), before);
    assert!(state(&harness).prompt.is_some());
    // A button of the panel ends it, its text dropped, and acts.
    click(&mut harness, "tool_paint");
    assert_eq!(state(&harness).prompt, None);
    assert_eq!(state(&harness).tool, Tool::Paint);
    assert_eq!(file(&dir), before);
    // Closing the editor ends it.
    click(&mut harness, "tool_select");
    drag(&mut harness, CRATE, CRATE);
    harness.tap_key(KeyCode::KeyR);
    assert!(state(&harness).prompt.is_some());
    harness.tap_action(TOGGLE);
    assert!(!state(&harness).open);
    assert_eq!(state(&harness).prompt, None);
}

#[test]
fn r_renames_the_pick() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    // With nothing picked the key and the button say what to pick.
    harness.tap_key(KeyCode::KeyR);
    assert_eq!(
        state(&harness).status,
        "Nothing is picked to rename: click an object or a place with Select."
    );
    assert!(state(&harness).refused);
    assert_eq!(state(&harness).prompt, None);
    // The Rename button is the key's, dimmed until something is picked.
    let dim = |harness: &UiHarness, key: &str| {
        let tree = harness.tree();
        let label = tree.children(keyed(harness, key))[0];
        tree.overrides(label).and_then(|style| style.text.color) == Some(crate::debug_views::LABEL)
    };
    assert!(dim(&harness, "rename") && dim(&harness, "set") && dim(&harness, "link"));
    drag(&mut harness, CRATE, CRATE);
    harness.step(1);
    assert!(!dim(&harness, "rename") && !dim(&harness, "set"));
    assert!(dim(&harness, "link"), "an object has no link");
    click(&mut harness, "rename");
    assert_eq!(prompt_text(&harness), Some("crate_1"));
    harness.tap_key(KeyCode::Backspace);
    type_text(&mut harness, "7");
    harness.tap_key(KeyCode::Enter);
    assert_eq!(at(harness.world(), "cave", "crate_7"), [5.0, 19.0]);
    // A place renames the same way, and the route that names it follows.
    script(
        &mut harness,
        &json!({ "select": "exit", "prompt": "rename", "text": "east_door" }),
    )
    .unwrap();
    let room = document(harness.world(), "cave");
    assert_eq!(place(&room, "east_door").point, Some([39.0, 19.0]));
    assert_eq!(
        serde_json::to_value(&place(&room, "main").route).unwrap(),
        json!(["player_start", [16.0, 19.0], "east_door"])
    );
    assert_eq!(state(&harness).selection.as_deref(), Some("east_door"));
    assert_eq!(file(&dir), room.to_canonical());
}

#[test]
fn set_writes_and_removes_an_override() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    script(&mut harness, &json!({ "select": "ball_1" })).unwrap();
    click(&mut harness, "set");
    assert_eq!(prompt_text(&harness), Some(""));
    harness.step(1);
    assert_eq!(
        text(&harness, "hint"),
        "Set on `ball_1`, as path = value: _  (Enter commits, Esc cancels)"
    );
    type_text(&mut harness, "rigid_body.shape.radius = 5");
    harness.tap_key(KeyCode::Enter);
    assert_eq!(
        state(&harness).status,
        "Set `rigid_body.shape.radius` on `ball_1` to 5."
    );
    let room = document(harness.world(), "cave");
    let ball = room.objects.iter().find(|o| o.id == "ball_1").unwrap();
    assert_eq!(ball.set.get("rigid_body.shape.radius"), Some(&json!(5)));
    assert_eq!(file(&dir), room.to_canonical());
    harness.step(1);
    assert!(
        harness.shows("set rigid_body.shape.radius = 5"),
        "the pick lists it"
    );
    // An empty value takes it off again.
    script(
        &mut harness,
        &json!({ "prompt": "set", "text": "rigid_body.shape.radius =" }),
    )
    .unwrap();
    assert_eq!(
        state(&harness).status,
        "Took `rigid_body.shape.radius` off `ball_1`."
    );
    let room = document(harness.world(), "cave");
    let ball = room.objects.iter().find(|o| o.id == "ball_1").unwrap();
    assert!(ball.set.is_empty());
    assert_eq!(journal(harness.world(), "cave"), (2, 0));
    // A place takes no override.
    assert_eq!(
        script(&mut harness, &json!({ "select": "arena", "prompt": "set" })),
        Err("`arena` is a place: Set writes an object's override.".to_owned())
    );
    assert_eq!(state(&harness).prompt, None);
}

#[test]
fn link_points_a_place_at_another_rooms() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    // It opens on the place's link, and an empty text unlinks.
    script(&mut harness, &json!({ "select": "exit" })).unwrap();
    harness.step(1);
    assert!(harness.shows("linked to `hall.west_door`"));
    click(&mut harness, "link");
    assert_eq!(prompt_text(&harness), Some("hall.west_door"));
    for _ in 0..14 {
        harness.tap_key(KeyCode::Backspace);
    }
    harness.tap_key(KeyCode::Enter);
    assert_eq!(state(&harness).status, "Unlinked `exit`.");
    assert_eq!(place(&document(harness.world(), "cave"), "exit").to, None);
    // A point links by `room.place`.
    script(&mut harness, &json!({ "select": "player_start" })).unwrap();
    click(&mut harness, "link");
    assert_eq!(prompt_text(&harness), Some(""));
    type_text(&mut harness, "hall.west_door");
    harness.tap_key(KeyCode::Enter);
    assert_eq!(
        state(&harness).status,
        "Linked `player_start` to `hall.west_door`."
    );
    let room = document(harness.world(), "cave");
    assert_eq!(
        place(&room, "player_start").to.as_deref(),
        Some("hall.west_door")
    );
    assert_eq!(file(&dir), room.to_canonical());
    // An object and a route have no link.
    for (id, error) in [
        (
            "crate_1",
            "`crate_1` is an object: Link joins a point or an area to another room's place.",
        ),
        (
            "main",
            "`main` is a route: Link joins a point or an area to another room's place.",
        ),
    ] {
        assert_eq!(
            script(&mut harness, &json!({ "select": id, "prompt": "link" })),
            Err(error.to_owned())
        );
    }
    assert_eq!(
        script(&mut harness, &json!({ "prompt": "retitle" })),
        Err("no prompt `retitle`: rename, set, link".to_owned())
    );
}

#[test]
fn mark_places_a_point_and_an_area() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    harness.tap_key(KeyCode::Digit6);
    assert_eq!(state(&harness).tool, Tool::Mark);
    harness.pointer_to(cell(3, 5).x, cell(3, 5).y);
    harness.step(1);
    assert_eq!(
        state(&harness).hint,
        "Click: mark a point at 3, 5. Drag: mark an area. At 3, 5 in `cave`."
    );
    // A click is a point place at the cell, picked.
    drag(&mut harness, cell(3, 5), cell(3, 5));
    let room = document(harness.world(), "cave");
    assert_eq!(place(&room, "point_1").point, Some([3.0, 5.0]));
    assert_eq!(
        state(&harness).status,
        "Marked point `point_1` at 3, 5. Rename names it."
    );
    assert_eq!(state(&harness).selection.as_deref(), Some("point_1"));
    // A drag is an area from its first cell to its last.
    harness.pointer_to(cell(5, 5).x, cell(5, 5).y);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(cell(8, 7).x, cell(8, 7).y);
    harness.step(1);
    assert_eq!(
        state(&harness).hint,
        "Release: mark a 4 \u{d7} 3 area. Esc drops it. At 8, 7 in `cave`."
    );
    assert!(boxed(
        &harness,
        Vec2::new(80.0, 80.0),
        Vec2::new(144.0, 128.0)
    ));
    harness.pointer_up();
    harness.step(1);
    let room = document(harness.world(), "cave");
    assert_eq!(place(&room, "area_1").rect, Some([5.0, 5.0, 4.0, 3.0]));
    assert_eq!(
        state(&harness).status,
        "Marked area `area_1`, 4 \u{d7} 3 at 5, 5. Rename names it."
    );
    assert_eq!(journal(harness.world(), "cave"), (2, 0));
    assert_eq!(file(&dir), room.to_canonical());
    // Escape drops a mark under way.
    harness.pointer_to(cell(10, 5).x, cell(10, 5).y);
    harness.pointer_down();
    harness.step(1);
    harness.tap_key(KeyCode::Escape);
    harness.pointer_up();
    harness.step(1);
    assert_eq!(state(&harness).status, "Dropped the mark: nothing changed.");
    assert_eq!(journal(harness.world(), "cave"), (2, 0));
    // The pick shows what it is.
    script(&mut harness, &json!({ "select": "area_1" })).unwrap();
    harness.step(1);
    assert!(harness.shows("`area_1`, an area"));
    assert!(harness.shows("4 \u{d7} 3 at 5, 5"));
    assert!(harness.shows("not linked"));
}

#[test]
fn select_moves_and_deletes_a_place() {
    let (dir, mut harness) = editor();
    open(&mut harness);
    // `exit`, a point at (39, 19): picked within reach of its marker, and
    // dragged by cells.
    let exit = Vec2::new(39.0 * 16.0, 19.0 * 16.0);
    harness.pointer_to(exit.x - 4.0, exit.y);
    harness.step(1);
    assert_eq!(
        state(&harness).hint,
        "Click: pick `exit`. Drag: move it by cells. Right click: delete it. At 38, 19 in `cave`."
    );
    drag(&mut harness, exit, exit - Vec2::new(32.0, 0.0));
    assert_eq!(state(&harness).status, "Moved `exit` by -2, 0.");
    assert_eq!(state(&harness).selection.as_deref(), Some("exit"));
    let room = document(harness.world(), "cave");
    assert_eq!(place(&room, "exit").point, Some([37.0, 19.0]));
    assert_eq!(file(&dir), room.to_canonical());
    // `arena`, an area: picked by its outline, not by its inside.
    drag(
        &mut harness,
        Vec2::new(400.0, 150.0),
        Vec2::new(400.0, 150.0),
    );
    assert_eq!(
        state(&harness).selection,
        None,
        "a press inside is the room's"
    );
    drag(&mut harness, Vec2::new(400.0, 65.0), Vec2::new(400.0, 65.0));
    assert_eq!(state(&harness).selection.as_deref(), Some("arena"));
    harness.step(1);
    assert!(boxed(
        &harness,
        Vec2::new(320.0, 64.0),
        Vec2::new(576.0, 256.0)
    ));
    // An object at a place's spot wins the press.
    drag(&mut harness, Vec2::new(34.0, 306.0), Vec2::new(34.0, 306.0));
    assert_eq!(state(&harness).selection.as_deref(), Some("walker_1"));
    // Ctrl+D copies a place and Delete removes it.
    drag(&mut harness, Vec2::new(400.0, 65.0), Vec2::new(400.0, 65.0));
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyD);
    assert_eq!(
        state(&harness).status,
        "Copied `arena` as `arena_1`, a cell right."
    );
    let room = document(harness.world(), "cave");
    assert_eq!(place(&room, "arena_1").rect, Some([21.0, 4.0, 16.0, 12.0]));
    harness.tap_key(KeyCode::Delete);
    assert_eq!(state(&harness).status, "Deleted `arena_1`.");
    let room = document(harness.world(), "cave");
    assert!(room.places.iter().all(|place| place.id != "arena_1"));
    assert_eq!(file(&dir), room.to_canonical());
}

#[test]
fn a_refused_prompt_keeps_its_text_and_says_why() {
    let (dir, mut harness) = editor();
    let before = file(&dir);
    open(&mut harness);
    drag(&mut harness, CRATE, CRATE);
    harness.tap_key(KeyCode::KeyR);
    for _ in 0..7 {
        harness.tap_key(KeyCode::Backspace);
    }
    type_text(&mut harness, "lamp_1");
    harness.tap_key(KeyCode::Enter);
    // The ID is taken: the status says so, and the prompt stays open on
    // what was typed.
    let editor = state(&harness);
    assert!(editor.refused);
    assert!(
        editor
            .status
            .starts_with("Can't rename `crate_1` to `lamp_1`: "),
        "{}",
        editor.status
    );
    assert!(editor.status.contains("lamp_1"), "{}", editor.status);
    assert_eq!(prompt_text(&harness), Some("lamp_1"));
    assert_eq!(file(&dir), before);
    harness.step(1);
    assert_eq!(
        text(&harness, "hint"),
        "Rename `crate_1` to: lamp_1_  (Enter commits, Esc cancels)"
    );
    // Mended, it lands.
    harness.tap_key(KeyCode::Backspace);
    type_text(&mut harness, "9");
    harness.tap_key(KeyCode::Enter);
    assert_eq!(state(&harness).status, "Renamed `crate_1` to `lamp_9`.");
    assert!(!state(&harness).refused);
    assert_eq!(state(&harness).prompt, None);
    // An ID that is no ID, and a Set with no `=`, are refused the same way.
    harness.tap_key(KeyCode::KeyR);
    type_text(&mut harness, " X");
    harness.tap_key(KeyCode::Enter);
    assert!(state(&harness).refused, "{}", state(&harness).status);
    assert_eq!(prompt_text(&harness), Some("lamp_9 X"));
    harness.tap_key(KeyCode::Escape);
    click(&mut harness, "set");
    type_text(&mut harness, "mass 5");
    harness.tap_key(KeyCode::Enter);
    assert_eq!(
        state(&harness).status,
        "Can't set: write it as path = value, as `health = 5`. An empty value takes the override off."
    );
    assert_eq!(prompt_text(&harness), Some("mass 5"));
    // From a script a refusal fails the step and leaves no prompt open.
    harness.tap_key(KeyCode::Escape);
    let refused = script(
        &mut harness,
        &json!({ "prompt": "rename", "text": "lamp_1" }),
    );
    assert!(
        refused
            .unwrap_err()
            .starts_with("Can't rename `lamp_9` to `lamp_1`: ")
    );
    assert_eq!(state(&harness).prompt, None);
    assert_eq!(
        script(&mut harness, &json!({ "text": "guard" })),
        Err("`text` is a prompt's: name it with `prompt`".to_owned())
    );
}

#[test]
fn the_panels_buttons_stay_put_as_the_cursor_comes_for_them() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    let top = |harness: &UiHarness, key: &str| {
        harness
            .tree()
            .visual_rect(keyed(harness, key))
            .unwrap()
            .min
            .y
    };
    // Over an object the hint is at its longest.
    harness.pointer_to(CRATE.x + 4.0, CRATE.y + 4.0);
    harness.step(2);
    let hint = state(&harness).hint.clone();
    assert!(hint.starts_with("Click: pick `crate_1`."), "{hint}");
    let over_the_world = (top(&harness, "tool_select"), top(&harness, "rename"));
    assert!(boxed(
        &harness,
        Vec2::new(80.0, 304.0),
        Vec2::new(96.0, 320.0)
    ));
    // The cursor comes onto the panel for a button: the hint stays what
    // it was, so nothing under it moves, and the world's hover box goes.
    let button = harness
        .tree()
        .visual_rect(keyed(&harness, "rename"))
        .unwrap();
    let scale = harness
        .world()
        .resource::<tungsten_core::ScaleFactor>()
        .get();
    let centre = (button.min + button.max) * 0.5 * scale;
    harness.pointer_to(centre.x, centre.y);
    harness.step(2);
    assert_eq!(state(&harness).hint, hint);
    assert_eq!(
        (top(&harness, "tool_select"), top(&harness, "rename")),
        over_the_world
    );
    assert!(!boxed(
        &harness,
        Vec2::new(80.0, 304.0),
        Vec2::new(96.0, 320.0)
    ));
    // A click there changes the hint to the click's: the tool's.
    click(&mut harness, "tool_paint");
    harness.step(1);
    assert!(state(&harness).hint.starts_with("Drag: paint `rock`"));
    // A one-line status after a three-line welcome keeps them there too.
    let before = top(&harness, "tool_select");
    click(&mut harness, "undo");
    harness.step(1);
    assert_eq!(state(&harness).status, "Nothing to undo in `cave`.");
    assert_eq!(top(&harness, "tool_select"), before);
}

#[test]
fn a_pick_stays_in_its_room() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    // `hall`, opened beside `cave`, gets a `crate_1` of its own.
    click(&mut harness, "room_hall");
    script(&mut harness, &json!({ "prefab": "crate" })).unwrap();
    let hall = |harness: &UiHarness, col: f32, row: f32| {
        state(harness)
            .drawn(CameraState::new())
            .world_to_screen(Vec2::new(672.0 + col * 16.0 + 8.0, row * 16.0 + 8.0))
    };
    let spot = hall(&harness, 3.0, 9.0);
    drag(&mut harness, spot, spot);
    assert_eq!(at(harness.world(), "hall", "crate_1"), [3.0, 9.0]);
    // `cave`'s `crate_1` is picked, then a stroke starts in `hall`: the
    // pick is dropped with the room, so nothing acts on `hall`'s crate.
    script(
        &mut harness,
        &json!({ "room": "cave", "select": "crate_1", "tool": "paint" }),
    )
    .unwrap();
    assert_eq!(state(&harness).selection.as_deref(), Some("crate_1"));
    script(&mut harness, &json!({ "do": "game_view" })).unwrap();
    harness.world_mut().resource_mut::<LevelEditor>().view = Some(View {
        position: Vec2::ZERO,
        zoom: 1.0,
    });
    let cell_in_hall = Vec2::new(672.0 + 5.0 * 16.0 + 8.0, 2.0 * 16.0 + 8.0);
    stroke(&mut harness, &[cell_in_hall, cell_in_hall]);
    let editor = state(&harness);
    assert_eq!(editor.current.as_deref(), Some("hall"));
    assert_eq!(editor.selection, None);
    harness.tap_key(KeyCode::Delete);
    assert_eq!(
        state(&harness).status,
        "Nothing is picked to delete: click an object with Select."
    );
    assert_eq!(at(harness.world(), "hall", "crate_1"), [3.0, 9.0]);
    assert_eq!(at(harness.world(), "cave", "crate_1"), [5.0, 19.0]);
}

#[test]
fn the_eyedropper_takes_the_authored_tile_not_what_play_left() {
    let (_dir, mut harness) = editor();
    // Play changes a cell of the live map: the room's own tile there is
    // still `rock`.
    let maps = harness
        .world_mut()
        .resource_mut::<tungsten_core::assets::TilemapRegistry>();
    let plank = maps
        .get("cave")
        .unwrap()
        .tileset
        .iter()
        .position(|sprite| sprite == "plank")
        .unwrap();
    assert!(maps.set_tile("cave", 1, 2, 21, plank as _));
    open(&mut harness);
    hold(&mut harness, Modifiers::ALT);
    drag(&mut harness, cell(2, 21), cell(2, 21));
    hold(&mut harness, Modifiers::empty());
    assert_eq!(state(&harness).tile.as_deref(), Some("rock"));
    assert_eq!(
        state(&harness).status,
        "Took tile `rock` and layer `solid`: Paint writes there."
    );
}

#[test]
fn a_gesture_ends_with_its_tool() {
    let (_dir, mut harness) = editor();
    let before = document(harness.world(), "cave");
    open(&mut harness);
    harness.step(2);
    // Alt pressed during a drag does not make the release the
    // eyedropper's: the move lands as it would have.
    harness.pointer_to(CRATE.x, CRATE.y);
    harness.pointer_down();
    harness.step(1);
    hold(&mut harness, Modifiers::ALT);
    harness.pointer_to(RIGHT.x, RIGHT.y);
    harness.step(1);
    harness.pointer_up();
    harness.step(1);
    hold(&mut harness, Modifiers::empty());
    assert_eq!(state(&harness).status, "Moved `crate_1` by 1, 0.");
    assert_eq!(state(&harness).tool, Tool::Select);
    chord(&mut harness, Modifiers::CONTROL, KeyCode::KeyZ);
    assert_eq!(document(harness.world(), "cave"), before);
    // A palette row clicked, or a script's `tile`, while a mark is under
    // way takes up its tool and drops the mark: no box is left drawn and
    // the new tool's ghost shows.
    harness.tap_key(KeyCode::Digit6);
    harness.pointer_to(cell(5, 5).x, cell(5, 5).y);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(cell(8, 7).x, cell(8, 7).y);
    harness.step(1);
    assert!(boxed(
        &harness,
        Vec2::new(80.0, 80.0),
        Vec2::new(144.0, 128.0)
    ));
    script(&mut harness, &json!({ "tile": "plank" })).unwrap();
    harness.pointer_up();
    harness.step(1);
    assert_eq!(state(&harness).tool, Tool::Paint);
    assert!(!boxed(
        &harness,
        Vec2::new(80.0, 80.0),
        Vec2::new(144.0, 128.0)
    ));
    assert!(
        boxed(&harness, Vec2::new(128.0, 112.0), Vec2::new(144.0, 128.0)),
        "Paint's ghost"
    );
    assert_eq!(
        document(harness.world(), "cave"),
        before,
        "no place was marked"
    );
    // Under Erase the keys step what the panel shows: the tiles.
    harness.tap_key(KeyCode::Digit4);
    harness.tap_key(KeyCode::BracketRight);
    assert_eq!(state(&harness).tile.as_deref(), Some("arch"));
    assert_eq!(state(&harness).prefab, None, "the prefabs are not showing");
}

#[test]
fn a_scripts_prompt_never_commits_one_left_open() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    harness.step(2);
    // A Link prompt left open on `exit` by an earlier step.
    script(&mut harness, &json!({ "select": "exit", "prompt": "link" })).unwrap();
    assert_eq!(prompt_text(&harness), Some("hall.west_door"));
    // A Set, which a place refuses, with a text: the step fails with
    // Set's reason, and the text never reaches the link.
    assert_eq!(
        script(
            &mut harness,
            &json!({ "prompt": "set", "text": "hall.door" })
        ),
        Err("`exit` is a place: Set writes an object's override.".to_owned())
    );
    assert_eq!(state(&harness).prompt, None);
    assert_eq!(
        place(&document(harness.world(), "cave"), "exit")
            .to
            .as_deref(),
        Some("hall.west_door")
    );
    assert_eq!(journal(harness.world(), "cave"), (0, 0));
    // At the panel a refused button ends the open prompt too.
    script(&mut harness, &json!({ "prompt": "rename" })).unwrap();
    click(&mut harness, "set");
    assert_eq!(state(&harness).prompt, None);
    assert!(state(&harness).refused);
}

#[test]
fn the_games_camera_keeps_its_rotation() {
    let (_dir, mut harness) = editor();
    harness.world_mut().resource_mut::<CameraState>().rotation = 0.25;
    harness.step(1);
    open(&mut harness);
    harness.step(1);
    // The editor's own view is level.
    harness.tap_key(KeyCode::KeyF);
    harness.step(2);
    assert_eq!(harness.world().resource::<CameraState>().rotation, 0.0);
    // Home goes back to the game's view, turned as it was.
    harness.tap_key(KeyCode::Home);
    harness.step(2);
    assert_eq!(harness.world().resource::<CameraState>().rotation, 0.25);
    // And closing from the editor's view gives it back too.
    harness.tap_key(KeyCode::KeyF);
    harness.step(2);
    harness.tap_action(TOGGLE);
    harness.step(2);
    assert_eq!(harness.world().resource::<CameraState>().rotation, 0.25);
}
