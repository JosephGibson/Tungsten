//! The level editor on the headless harness over the rooms fixture's copy,
//! `cave` spawned at the origin: its cells are 16 px, so `crate_1` at
//! `[5, 19]` has its marker at (80, 304).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tungsten_core::assets::{ResolvedManifest, Room, RoomCommand, RoomJournal, RoomRegistry};
use tungsten_core::input::{InputState, KeyCode, Modifiers, MouseButton, RoutedInput};
use tungsten_core::save::{SaveId, SaveState};
use tungsten_core::ui::UiLayer;
use tungsten_core::{CameraState, Config, DebugDraw, Time, Transform, World};

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
    let dir = fixture_copy();
    prepare(&dir);
    let mut harness = UiHarness::new(App::new(config).unwrap());
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
    assert_eq!(state(&harness).status, "moved crate_1 by 1, 0");
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
    harness.world_mut().resource_mut::<LevelEditor>().view = None;
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
            .ends_with("the disk is full; nothing edited"),
        "{}",
        state(&harness).status
    );
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
            .starts_with("`run_a` is a generated room"),
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
        "moved crate_1 by 1, 0 (no file: kept live)"
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
    assert_eq!(state(&harness).status, "placed lamp_2 at 3, 19");
    assert!(
        state(&harness).findings.is_empty(),
        "it stands on the floor"
    );
}

#[test]
fn the_eyedropper_takes_a_prefab_from_an_object_and_a_tile_and_its_layer_from_a_cell() {
    let (_dir, mut harness) = editor();
    open(&mut harness);
    secondary_click(&mut harness, CRATE);
    assert_eq!(state(&harness).prefab.as_deref(), Some("crate"));
    secondary_click(&mut harness, cell(2, 21));
    let editor = state(&harness);
    assert_eq!(editor.tile.as_deref(), Some("rock"));
    assert_eq!(
        editor.layer.as_deref(),
        Some("solid"),
        "the last layer holding one"
    );
    assert_eq!(editor.status, "took rock from layer solid");
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
    assert_eq!(state(&harness).status, "painted 3 cells plank");
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
