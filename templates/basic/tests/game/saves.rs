//! The save seam: the schema's migration, a continued run's restore, a
//! slot that fails to load, and the options' own store.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tungsten::StateStack;
use tungsten::core::ui::WidgetKind;
use tungsten::core::{SaveData, SaveSlots, UserFolder};
use tungsten::kit::{Counters, Health, Hit, damage};
use tungsten::testing::UiHarness;
use tungsten_template_basic::game;
use tungsten_template_basic::sample::{ENEMIES, HAZARD_START, PLAYER_START, SampleRun};
use tungsten_template_basic::save::{self, SaveGame};
use tungsten_template_basic::shell::confirm::CONFIRM;
use tungsten_template_basic::shell::game_over::GAME_OVER;
use tungsten_template_basic::shell::options::{self, GameOptions};
use tungsten_template_basic::shell::pause::PAUSE;
use tungsten_template_basic::shell::saves::SAVES;
use tungsten_template_basic::shell::toast::Toasts;
use tungsten_template_basic::shell::{
    self, AUTOSAVES, Difficulty, OLD_AUTOSAVE, RunState, request_autosave, widgets,
};

use super::{
    find, harness_with, health, named, player_entity, player_position, press, settle, shows,
    slot_file, start, tap, top, user_folder,
};

/// The template's save before schema 2: the player's position alone.
#[derive(Serialize, Deserialize)]
struct SaveV1 {
    player: [f32; 2],
}

impl SaveData for SaveV1 {
    const SCHEMA: u32 = 1;
}

/// The template's save before schema 3: no pickups.
#[derive(Serialize, Deserialize)]
struct SaveV2 {
    meta: shell::RunMeta,
    player: [f32; 2],
    health: tungsten::kit::Health,
    gems_left: Vec<u8>,
    gems: i64,
}

impl SaveData for SaveV2 {
    const SCHEMA: u32 = 2;
}

#[test]
fn a_schema_2_save_migrates() {
    let dir = user_folder("migrate-2");
    let fresh = fresh_save();
    let old = SaveV2 {
        meta: fresh.meta.clone(),
        player: [120.0, 220.0],
        health: fresh.health.clone(),
        gems_left: vec![1],
        gems: 2,
    };
    let slots = SaveSlots::new(Some(dir.join("saves")));
    slots.save("slot_4", &old).unwrap();
    let save = slots.load::<SaveGame>("slot_4").expect("schema 2 migrates");
    assert_eq!(
        (save.player, save.gems_left, save.gems),
        ([120.0, 220.0], vec![1], 2)
    );
    assert_eq!(save.pickups_left, [0, 1], "every pickup still there");
    assert_eq!(<SaveGame as SaveData>::SCHEMA, 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pickups_left_round_trip() {
    let dir = user_folder("pickups-left");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    let taken = harness.world().resource::<SampleRun>().pickups[0].1;
    harness.world_mut().despawn(taken);
    tap(&mut harness, "state_pause");
    harness.step(1);
    let saved = SaveSlots::new(Some(dir.join("saves")))
        .load::<SaveGame>(AUTOSAVES[1])
        .unwrap();
    assert_eq!(saved.pickups_left, [1], "the speed pickup alone");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Continue");
    let run = harness.world().resource::<SampleRun>();
    let left: Vec<u8> = run.pickups.iter().map(|(slot, _)| *slot).collect();
    assert_eq!(left, [1], "the heal stays taken");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_schema_1_autosave_migrates() {
    let dir = user_folder("migrate");
    let slots = SaveSlots::new(Some(dir.join("saves")));
    slots
        .save(
            AUTOSAVES[0],
            &SaveV1 {
                player: [100.0, 200.0],
            },
        )
        .unwrap();
    let save = slots
        .load::<SaveGame>(AUTOSAVES[0])
        .expect("schema 1 migrates");
    assert_eq!(save.player, [100.0, 200.0]);
    assert_eq!(save.meta.difficulty, Difficulty::Normal);
    assert_eq!(save.meta.game_version, "unknown");
    assert_eq!(save.meta.playtime_secs, 0.0);
    assert_eq!((save.health.current(), save.health.max()), (5.0, 5.0));
    assert_eq!(save.gems_left, [0, 1, 2]);
    assert_eq!(save.gems, 0);
    assert_eq!(save.pickups_left, [0, 1], "through schema 2's step");

    // Continued, after the question an unknown version asks, it plays from
    // there.
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Continue");
    assert_eq!(top(&harness), Some(CONFIRM));
    press(&mut harness, "Load");
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    assert_eq!(player_position(&harness), Some([100.0, 200.0]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn continue_restores_position_health_gems_and_count() {
    let dir = user_folder("continue");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    let player = player_entity(&harness);
    harness.harness_mut().press_action("move_right");
    harness.step(20);
    harness.harness_mut().release_action("move_right");
    assert_eq!(damage(harness.world_mut(), player, 1.0), Hit::Hurt);
    let gem = harness.world().resource::<SampleRun>().gems[1].1;
    harness.world_mut().despawn(gem);
    harness
        .world_mut()
        .resource_mut::<Counters>()
        .set("gems", 1);
    // Paused with the movement released: the next frame saves.
    tap(&mut harness, "state_pause");
    harness.step(1);
    let moved = player_position(&harness).unwrap();
    assert_ne!(moved, PLAYER_START);

    let mut restarted = harness_with(UserFolder::At(dir.clone()));
    press(&mut restarted, "Continue");
    assert_eq!(player_position(&restarted), Some(moved));
    assert_eq!(health(&restarted, player_entity(&restarted)), (4.0, 5.0));
    let gems: Vec<u8> = restarted
        .world()
        .resource::<SampleRun>()
        .gems
        .iter()
        .map(|(slot, _)| *slot)
        .collect();
    assert_eq!(gems, [0, 2], "the taken gem stays taken");
    assert_eq!(restarted.world().resource::<Counters>().get("gems"), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A Normal run's save at 1 of 5 health, its player moved onto `at`.
fn low_health_save(at: [f32; 2]) -> SaveGame {
    let mut harness = super::harness();
    start(&mut harness);
    let player = player_entity(&harness);
    assert_eq!(damage(harness.world_mut(), player, 4.0), Hit::Hurt);
    // Past the invulnerability window, so the save holds none of it.
    harness.step(40);
    let mut save = save::capture(harness.world()).expect("a live run");
    assert_eq!((save.health.current(), save.health.max()), (1.0, 5.0));
    save.player = at;
    save
}

/// 120 frames at 1/60 with no input: the player lives, gameplay on top.
fn survives_two_seconds(harness: &mut UiHarness, way: &str) {
    harness.step(120);
    let player = player_entity(harness);
    let dead = harness.world().get::<Health>(player).unwrap().is_dead();
    assert!(!dead, "{way}: the player died");
    assert_eq!(top(harness), Some(game::GAMEPLAY), "{way}");
}

#[test]
fn a_continued_run_survives_its_first_two_seconds() {
    // Where a load at low health died: on the hazard's line, 64 px right of
    // the orbiter's place, where it stands on the first step, and under the
    // patrol.
    let orbiter = ENEMIES[1].1;
    let places = [
        [HAZARD_START[0] + 1.0, HAZARD_START[1]],
        [orbiter[0] + 64.0, orbiter[1]],
        [640.0, 80.0],
    ];
    for (n, at) in places.into_iter().enumerate() {
        let save = low_health_save(at);
        let seeded = |way: &str| {
            let dir = user_folder(&format!("grace-{n}-{way}"));
            let slots = SaveSlots::new(Some(dir.join("saves")));
            slots.save(AUTOSAVES[0], &save).unwrap();
            slots.save("slot_1", &save).unwrap();
            dir
        };

        // Continue from the title, then Retry after a death in that run.
        let dir = seeded("continue");
        let mut harness = harness_with(UserFolder::At(dir.clone()));
        press(&mut harness, "Continue");
        survives_two_seconds(&mut harness, &format!("Continue at {at:?}"));
        let player = player_entity(&harness);
        assert_eq!(damage(harness.world_mut(), player, 99.0), Hit::Killed);
        harness.step(1);
        settle(&mut harness);
        assert_eq!(top(&harness), Some(GAME_OVER));
        press(&mut harness, "Retry");
        survives_two_seconds(&mut harness, &format!("Retry at {at:?}"));
        let _ = std::fs::remove_dir_all(&dir);

        // Load from the title's Load screen.
        let dir = seeded("title-load");
        let mut harness = harness_with(UserFolder::At(dir.clone()));
        press(&mut harness, "Load");
        press(&mut harness, "Slot 1");
        survives_two_seconds(&mut harness, &format!("the title's Load at {at:?}"));
        let _ = std::fs::remove_dir_all(&dir);

        // Load from pause over a live run.
        let dir = seeded("pause-load");
        let mut harness = harness_with(UserFolder::At(dir.clone()));
        start(&mut harness);
        tap(&mut harness, "state_pause");
        harness.step(1);
        press(&mut harness, "Load");
        press(&mut harness, "Slot 1");
        survives_two_seconds(&mut harness, &format!("pause's Load at {at:?}"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_slot_that_fails_to_load_is_never_overwritten() {
    let dir = user_folder("newer-save");
    let slot = slot_file(&dir, AUTOSAVES[0]);
    std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
    // A save from a newer build of the game: schema 9.
    let newer = r#"{ "envelope": 1, "slot": "autosave_1", "schema": 9, "engine_version": "9.0.0", "saved_at_ms": 1, "data": { "player": [1.0, 2.0], "lives": 3 } }"#;
    std::fs::write(&slot, newer).unwrap();

    let mut harness = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(find(&harness, "Continue"), None, "nothing to continue");
    press(&mut harness, "New Game");
    press(&mut harness, "Normal");
    assert_eq!(top(&harness), Some(game::GAMEPLAY), "no question: it stays");
    tap(&mut harness, "state_pause");
    harness.step(1);
    assert_eq!(top(&harness), Some(PAUSE));
    assert_eq!(std::fs::read_to_string(&slot).unwrap(), newer);

    // The last slot, newer too: listed, neither loaded nor overwritten.
    let last = slot_file(&dir, "slot_10");
    let newer_last = newer.replace("\"autosave_1\"", "\"slot_10\"");
    std::fs::write(&last, &newer_last).unwrap();
    press(&mut harness, "Save");
    assert_eq!(find(&harness, "Slot 10"), None, "not a save target");
    assert!(named(&harness, "Slot 10").is_some(), "but listed");
    tap(&mut harness, "ui_cancel");
    press(&mut harness, "Load");
    assert_eq!(find(&harness, "Slot 10"), None, "nor a load source");
    assert_eq!(std::fs::read_to_string(&last).unwrap(), newer_last);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn options_keep_their_own_folder_and_a_damaged_file_falls_back() {
    let dir = user_folder("options");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let world = harness.world_mut();
    world.resource_mut::<GameOptions>().damage_numbers = false;
    world.resource_mut::<GameOptions>().set_screen_shake(0.5);
    options::save(world);
    let file = dir.join("options").join("options.json");
    assert!(file.is_file());
    let listed = SaveSlots::new(Some(dir.join("saves"))).list().unwrap();
    assert!(listed.slots.is_empty(), "the save slots never list them");

    let restarted = harness_with(UserFolder::At(dir.clone()));
    let options = *restarted.world().resource::<GameOptions>();
    assert!(!options.damage_numbers);
    assert_eq!(options.screen_shake(), 0.5);

    std::fs::write(&file, "{ not json").unwrap();
    let mut damaged = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(
        *damaged.world().resource::<GameOptions>(),
        GameOptions::default()
    );
    damaged.step(1);
    let toasts = damaged.world().resource::<Toasts>().texts(damaged.tree());
    assert_eq!(toasts, ["Options could not be read; using the defaults"]);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- the save and load screen ----

/// `save` with `player` and `gems` written to `slot` in `dir`, played for
/// 12:04.
fn write_slot(dir: &Path, slot: &str, mut save: SaveGame, player: [f32; 2], gems: i64) {
    save.player = player;
    save.gems = gems;
    save.meta.playtime_secs = 724.0;
    SaveSlots::new(Some(dir.join("saves")))
        .save(slot, &save)
        .unwrap();
}

/// A save as a fresh Normal run captures it, from a run nobody keeps.
fn fresh_save() -> SaveGame {
    let mut harness = super::harness();
    start(&mut harness);
    save::capture(harness.world()).expect("a live run")
}

/// Sets `slot`'s save time in `dir`, so the newest slot is known.
fn set_saved_at(dir: &Path, slot: &str, ms: u64) {
    let file = slot_file(dir, slot);
    let text = std::fs::read_to_string(&file).unwrap();
    let at = text.find("\"saved_at_ms\": ").unwrap() + "\"saved_at_ms\": ".len();
    let end = at + text[at..].find(',').unwrap();
    std::fs::write(&file, format!("{}{ms}{}", &text[..at], &text[end..])).unwrap();
}

fn row_text(harness: &UiHarness, name: &str) -> String {
    let row = named(harness, name).unwrap_or_else(|| panic!("no row '{name}'"));
    widgets::text_of(harness.tree(), row)
}

/// The details under the row named `name`: its button's second label.
fn details(harness: &UiHarness, name: &str) -> String {
    let row = named(harness, name).unwrap_or_else(|| panic!("no row '{name}'"));
    let tree = harness.tree();
    widgets::text_of(tree, tree.children(row)[1])
}

#[test]
fn the_load_screen_lists_the_autosaves_and_ten_slots_and_scrolls() {
    let dir = user_folder("load-screen");
    let save = fresh_save();
    write_slot(&dir, AUTOSAVES[0], save.clone(), [10.0, 20.0], 1);
    write_slot(&dir, "slot_2", save.clone(), [30.0, 40.0], 2);
    write_slot(&dir, "slot_10", save, [50.0, 60.0], 0);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    // The smallest viewport, where thirteen rows outgrow the card.
    harness.world_mut().insert_resource(tungsten::WindowSize {
        width: 731,
        height: 411,
    });
    press(&mut harness, "Load");
    assert_eq!(top(&harness), Some(SAVES));
    // The name, then the details; this version's saves name no version.
    assert_eq!(row_text(&harness, "Autosave 1"), "Autosave 1");
    assert_eq!(
        details(&harness, "Autosave 1"),
        "just now · 12:04 played\nGems 1 / 3 · Health 5 / 5"
    );
    for name in ["Autosave 2", "Autosave 3"] {
        assert_eq!(details(&harness, name), "Empty");
    }
    assert_eq!(
        details(&harness, "Slot 2"),
        "just now · 12:04 played\nGems 2 / 3 · Health 5 / 5"
    );
    for number in [1, 3, 4, 5, 6, 7, 8, 9] {
        let name = format!("Slot {number}");
        assert_eq!(details(&harness, &name), "Empty");
        assert_eq!(find(&harness, &name), None, "an empty slot loads nothing");
    }
    assert!(details(&harness, "Slot 10").starts_with("just now"));
    let tree = harness.tree();
    let card = named(&harness, "Autosave 1").and_then(|row| {
        let mut id = row;
        while let Some(parent) = tree.parent(id) {
            if tree.scroll_max(parent).is_some() {
                return Some(parent);
            }
            id = parent;
        }
        None
    });
    let card = card.expect("the rows sit in a scroll panel");
    assert!(tree.scroll_max(card).unwrap() > 0.0, "the card scrolls");
    let last = find(&harness, "Slot 10").unwrap();
    assert!(harness.focus(last), "focus reaches the last row");
    harness.layout();
    let (rect, view) = (harness.rect(last).unwrap(), harness.rect(card).unwrap());
    let offset = harness.tree().scroll(card).unwrap();
    assert!(offset > 0.0, "focus scrolls it into view");
    assert!(
        rect.max.y - offset <= view.max.y + 0.01,
        "{rect:?} at {offset} in {view:?}"
    );
    press(&mut harness, "Slot 2");
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    assert_eq!(player_position(&harness), Some([30.0, 40.0]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_to_an_empty_slot_writes_it_and_toasts() {
    let dir = user_folder("save-empty");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    harness.harness_mut().press_action("move_right");
    harness.step(10);
    super::release_and_settle(&mut harness, "move_right");
    let walked = player_position(&harness).unwrap();
    tap(&mut harness, "state_pause");
    press(&mut harness, "Save");
    assert_eq!(top(&harness), Some(SAVES));
    press(&mut harness, "Slot 1");
    assert_eq!(top(&harness), Some(SAVES), "no question for an empty slot");
    let slots = SaveSlots::new(Some(dir.join("saves")));
    assert_eq!(slots.load::<SaveGame>("slot_1").unwrap().player, walked);
    harness.step(1);
    let toasts = harness.world().resource::<Toasts>().texts(harness.tree());
    assert_eq!(toasts, ["Saved to Slot 1"]);
    assert!(details(&harness, "Slot 1").starts_with("just now"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn overwriting_asks_first_and_cancel_keeps_the_old_save() {
    let dir = user_folder("overwrite");
    write_slot(&dir, "slot_1", fresh_save(), [1.0, 2.0], 0);
    let file = slot_file(&dir, "slot_1");
    let bytes = std::fs::read(&file).unwrap();
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Save");
    press(&mut harness, "Slot 1");
    assert_eq!(top(&harness), Some(CONFIRM));
    assert!(shows(&harness, "Overwrite Slot 1?"));
    press(&mut harness, "Cancel");
    assert_eq!(top(&harness), Some(SAVES));
    assert_eq!(std::fs::read(&file).unwrap(), bytes, "the old save stays");
    press(&mut harness, "Slot 1");
    press(&mut harness, "Overwrite");
    let slots = SaveSlots::new(Some(dir.join("saves")));
    assert_eq!(
        slots.load::<SaveGame>("slot_1").unwrap().player,
        PLAYER_START
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleting_asks_first_and_empties_the_slot() {
    let dir = user_folder("delete");
    write_slot(&dir, "slot_2", fresh_save(), [1.0, 2.0], 0);
    let file = slot_file(&dir, "slot_2");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Load");
    press(&mut harness, "Delete Slot 2");
    assert!(shows(&harness, "Delete Slot 2?"));
    press(&mut harness, "Cancel");
    assert!(file.is_file());
    press(&mut harness, "Delete Slot 2");
    press(&mut harness, "Delete");
    assert_eq!(top(&harness), Some(SAVES));
    assert!(!file.exists());
    assert_eq!(details(&harness, "Slot 2"), "Empty");
    assert_eq!(find(&harness, "Delete Slot 2"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[cfg_attr(not(unix), ignore = "Windows does not honor read-only on folders")]
fn a_delete_that_fails_keeps_the_slot_read_only_and_says_so() {
    let dir = user_folder("delete-fails");
    let damaged = "{ not json";
    let file = slot_file(&dir, "slot_3");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, damaged).unwrap();
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Save");
    press(&mut harness, "Delete Slot 3");
    // The answer's frame, with the saves folder read-only: the delete fails.
    let saves = dir.join("saves");
    let writable = std::fs::metadata(&saves).unwrap().permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(&saves, read_only).unwrap();
    let delete = find(&harness, "Delete").unwrap();
    assert!(harness.focus(delete));
    harness.harness_mut().press_action("ui_accept");
    harness.step(1);
    harness.harness_mut().release_action("ui_accept");
    std::fs::set_permissions(&saves, writable).unwrap();
    // Before the screen reads the slots again, the slot is still read-only.
    assert!(
        !shell::save_to(harness.world_mut(), "slot_3"),
        "not written"
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), damaged);
    harness.step(1);
    let toasts = harness.world().resource::<Toasts>().texts(harness.tree());
    let last = toasts.last().map(String::as_str);
    assert_eq!(last, Some("Slot 3 could not be deleted"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_damaged_or_newer_slot_shows_its_label_and_is_neither_loaded_nor_overwritten() {
    let dir = user_folder("damaged-newer");
    let newer = r#"{ "envelope": 1, "slot": "slot_1", "schema": 9, "engine_version": "9.0.0", "saved_at_ms": 1, "data": {} }"#;
    let damaged = "{ not json";
    std::fs::create_dir_all(dir.join("saves")).unwrap();
    std::fs::write(slot_file(&dir, "slot_1"), newer).unwrap();
    std::fs::write(slot_file(&dir, "slot_3"), damaged).unwrap();
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    harness.step(1);
    let toasts = harness.world().resource::<Toasts>().texts(harness.tree());
    assert_eq!(toasts, ["1 save file could not be read"]);
    press(&mut harness, "Load");
    assert_eq!(details(&harness, "Slot 1"), "Saved by a newer version");
    assert_eq!(details(&harness, "Slot 3"), "Damaged");
    assert_eq!(find(&harness, "Slot 1"), None);
    assert_eq!(find(&harness, "Slot 3"), None);
    tap(&mut harness, "ui_cancel");

    press(&mut harness, "New Game");
    press(&mut harness, "Normal");
    tap(&mut harness, "state_pause");
    press(&mut harness, "Save");
    assert_eq!(find(&harness, "Slot 1"), None, "nor overwritten");
    assert!(!shell::save_to(harness.world_mut(), "slot_1"));
    assert!(!shell::save_to(harness.world_mut(), "slot_3"));
    assert_eq!(
        std::fs::read_to_string(slot_file(&dir, "slot_1")).unwrap(),
        newer
    );
    assert_eq!(
        std::fs::read_to_string(slot_file(&dir, "slot_3")).unwrap(),
        damaged
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A save of a run hit once, at `player`, with one gem taken and counted.
fn hurt_save() -> SaveGame {
    let mut harness = super::harness();
    start(&mut harness);
    let player = player_entity(&harness);
    assert_eq!(damage(harness.world_mut(), player, 1.0), Hit::Hurt);
    harness.step(40);
    let mut save = save::capture(harness.world()).unwrap();
    save.gems_left = vec![0, 1];
    save
}

#[test]
fn loading_from_pause_leaves_one_gameplay_state() {
    let dir = user_folder("load-pause");
    write_slot(&dir, "slot_1", hurt_save(), [50.0, 60.0], 1);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    harness.step(1);
    press(&mut harness, "Load");
    press(&mut harness, "Slot 1");
    assert_eq!(
        top(&harness),
        Some(game::GAMEPLAY),
        "checkpointed: no question"
    );
    assert_eq!(harness.world().resource::<StateStack>().depth(), 1);
    assert_eq!(player_position(&harness), Some([50.0, 60.0]));
    assert_eq!(health(&harness, player_entity(&harness)), (4.0, 5.0));
    assert_eq!(harness.world().resource::<Counters>().get("gems"), 1);
    assert_eq!(harness.world().resource::<SampleRun>().gems.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Opens Load from pause in `harness` and picks Slot 1; whether it asked.
fn load_slot_1_asks(harness: &mut UiHarness) -> bool {
    tap(harness, "state_pause");
    harness.step(1);
    press(harness, "Load");
    press(harness, "Slot 1");
    top(harness) == Some(CONFIRM)
}

#[test]
fn loading_from_pause_asks_first_unless_the_run_is_checkpointed() {
    // Read-only autosaves: three from a newer build.
    let dir = user_folder("load-asks-read-only");
    write_slot(&dir, "slot_1", fresh_save(), [50.0, 60.0], 0);
    let newer = r#"{ "envelope": 1, "slot": "autosave_1", "schema": 9, "engine_version": "9.0.0", "saved_at_ms": 1, "data": {} }"#;
    for slot in AUTOSAVES {
        std::fs::write(slot_file(&dir, slot), newer).unwrap();
    }
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    assert!(load_slot_1_asks(&mut harness), "read-only autosaves");
    assert!(shows(
        &harness,
        "Load Slot 1? Progress since your last save will be lost."
    ));
    let _ = std::fs::remove_dir_all(&dir);

    // A write that fails: the saves folder turned read-only mid-run. Unix
    // only: Windows does not honor the read-only attribute on folders.
    if cfg!(unix) {
        let dir = user_folder("load-asks-failed");
        write_slot(&dir, "slot_1", fresh_save(), [50.0, 60.0], 0);
        let mut harness = harness_with(UserFolder::At(dir.clone()));
        start(&mut harness);
        let saves = dir.join("saves");
        let writable = std::fs::metadata(&saves).unwrap().permissions();
        let mut read_only = writable.clone();
        read_only.set_readonly(true);
        std::fs::set_permissions(&saves, read_only).unwrap();
        let asked = load_slot_1_asks(&mut harness);
        std::fs::set_permissions(&saves, writable).unwrap();
        assert!(asked, "the pause's autosave failed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Checkpointed: the pause's autosave succeeded.
    let dir = user_folder("load-asks-not");
    write_slot(&dir, "slot_1", fresh_save(), [50.0, 60.0], 0);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    assert!(!load_slot_1_asks(&mut harness), "a checkpointed run");
    assert_eq!(player_position(&harness), Some([50.0, 60.0]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn continue_loads_the_newest_of_thirteen_slots() {
    let dir = user_folder("continue-newest");
    let save = fresh_save();
    for (at, slot) in shell::SLOTS.iter().enumerate() {
        let x = at as f32 * 10.0;
        write_slot(&dir, slot, save.clone(), [x, x + 1.0], 0);
        set_saved_at(&dir, slot, 1_000 + at as u64);
    }
    set_saved_at(&dir, "slot_7", 9_000);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Continue");
    assert_eq!(player_position(&harness), Some([90.0, 91.0]), "slot_7");
    assert_eq!(shell::SLOTS.len(), 13);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_user_folder_save_and_load_are_off() {
    let mut harness = super::harness();
    assert_eq!(find(&harness, "Load"), None);
    assert!(shows(&harness, "Saving is off"));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    assert_eq!(find(&harness, "Save"), None);
    assert_eq!(find(&harness, "Load"), None);
    assert!(shows(&harness, "Saving is off"));
    tap(&mut harness, "ui_cancel");
    let player = player_entity(&harness);
    damage(harness.world_mut(), player, 99.0);
    harness.step(1);
    super::settle(&mut harness);
    assert_eq!(find(&harness, "Load"), None);
    assert!(find(&harness, "Retry").is_some());
}

#[test]
fn the_autosave_rows_are_not_save_targets() {
    let dir = user_folder("autosave-row");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Save");
    // Left out, with a note on when they are written.
    for name in ["Autosave 1", "Autosave 2", "Autosave 3"] {
        assert_eq!(find(&harness, name), None);
    }
    assert!(shows(
        &harness,
        "Written as you play, when you pause and when you quit."
    ));
    tap(&mut harness, "ui_cancel");
    press(&mut harness, "Load");
    assert!(find(&harness, "Autosave 1").is_some(), "but a load source");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The slot the run's last save wrote.
fn retry_from(harness: &UiHarness) -> Option<&'static str> {
    harness.world().resource::<RunState>().retry_from
}

/// `slot`'s save time in `dir`.
fn saved_at(dir: &Path, slot: &str) -> u64 {
    let list = SaveSlots::new(Some(dir.join("saves"))).list().unwrap();
    let info = list.slots.iter().find(|info| info.name == slot);
    info.unwrap_or_else(|| panic!("no {slot}")).saved_at_ms
}

/// Walks the player right a little, then asks for an autosave and runs the
/// frame that writes it; where the player stands.
fn moved_autosave(harness: &mut UiHarness) -> [f32; 2] {
    harness.harness_mut().press_action("move_right");
    harness.step(10);
    super::release_and_settle(harness, "move_right");
    request_autosave(harness.world_mut());
    harness.step(1);
    player_position(harness).unwrap()
}

#[test]
fn autosaves_rotate_through_three_slots_overwriting_the_oldest() {
    let dir = user_folder("rotate");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    assert_eq!(retry_from(&harness), Some(AUTOSAVES[0]), "the run's first");
    let read = |slot: &str| {
        let slots = SaveSlots::new(Some(dir.join("saves")));
        slots.load::<SaveGame>(slot).ok().map(|save| save.player)
    };
    let mut held = [read(AUTOSAVES[0]), None, None];
    assert!(held[0].is_some());
    for at in [1, 2, 0] {
        let walked = moved_autosave(&mut harness);
        held[at] = Some(walked);
        assert_eq!(retry_from(&harness), Some(AUTOSAVES[at]), "the next");
        assert_eq!(AUTOSAVES.map(read), held, "after the write to {at}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_autosave_fills_an_empty_slot_then_the_oldest() {
    let dir = user_folder("fill-then-oldest");
    let save = fresh_save();
    for (slot, at) in AUTOSAVES.into_iter().zip([200, 300, 100]) {
        write_slot(&dir, slot, save.clone(), [1.0, 2.0], 0);
        set_saved_at(&dir, slot, at);
    }
    // Autosave 1 deleted, then a restart: (empty, t 300, t 100).
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Load");
    press(&mut harness, "Delete Autosave 1");
    press(&mut harness, "Delete");
    assert!(!slot_file(&dir, AUTOSAVES[0]).exists());
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "New Game");
    press(&mut harness, "Normal");
    press(&mut harness, "Start");
    assert_eq!(retry_from(&harness), Some(AUTOSAVES[0]), "the empty one");
    moved_autosave(&mut harness);
    assert_eq!(retry_from(&harness), Some(AUTOSAVES[2]), "then the oldest");
    assert_eq!(saved_at(&dir, AUTOSAVES[1]), 300, "the newer one stays");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The names of the save rows the screen on top shows, in order.
fn row_names(harness: &UiHarness) -> Vec<String> {
    let tree = harness.tree();
    let root = super::shown_modal(harness).expect("a screen");
    let mut names = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if tree.visibility(id) != Some(tungsten::core::ui::Visibility::Visible) {
            continue;
        }
        let name = tree.accessible_name(id).unwrap_or_default();
        let row = name.starts_with("Autosave") || name.starts_with("Slot ");
        if row && tree.kind(id) == Some(WidgetKind::Button) {
            names.push(name.to_owned());
        }
        stack.extend(tree.children(id).iter().rev().copied());
    }
    names
}

#[test]
fn the_load_screen_lists_three_autosaves_newest_first() {
    let dir = user_folder("autosaves-newest-first");
    let save = fresh_save();
    for (slot, at) in AUTOSAVES.into_iter().zip([100, 300, 200]) {
        write_slot(&dir, slot, save.clone(), [1.0, 2.0], 0);
        set_saved_at(&dir, slot, at);
    }
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Load");
    let names = row_names(&harness);
    assert_eq!(names[..3], ["Autosave 2", "Autosave 3", "Autosave 1"]);
    let slots: Vec<String> = (1..=10).map(|n| format!("Slot {n}")).collect();
    assert_eq!(names[3..], slots);
    // A row loads the slot it names.
    write_slot(&dir, AUTOSAVES[2], save, [7.0, 8.0], 0);
    set_saved_at(&dir, AUTOSAVES[2], 200);
    tap(&mut harness, "ui_cancel");
    press(&mut harness, "Load");
    press(&mut harness, "Autosave 3");
    assert_eq!(player_position(&harness), Some([7.0, 8.0]));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A 0.93 autosave in `dir`: the old slot name, a save at (77, 88), saved
/// at 12,345 ms.
fn old_autosave(dir: &Path) {
    write_slot(dir, OLD_AUTOSAVE, fresh_save(), [77.0, 88.0], 0);
    set_saved_at(dir, OLD_AUTOSAVE, 12_345);
}

#[test]
fn a_093_autosave_becomes_one_of_the_three() {
    let dir = user_folder("old-autosave");
    old_autosave(&dir);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    assert!(!slot_file(&dir, OLD_AUTOSAVE).exists(), "renamed");
    assert_eq!(saved_at(&dir, AUTOSAVES[0]), 12_345, "its time kept");
    press(&mut harness, "Continue");
    assert_eq!(player_position(&harness), Some([77.0, 88.0]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[cfg_attr(not(unix), ignore = "Windows does not honor read-only on folders")]
fn a_093_autosave_in_a_read_only_folder_stays_listed() {
    let dir = user_folder("old-autosave-read-only");
    old_autosave(&dir);
    let saves = dir.join("saves");
    let writable = std::fs::metadata(&saves).unwrap().permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(&saves, read_only).unwrap();
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let stayed = slot_file(&dir, OLD_AUTOSAVE).exists();
    press(&mut harness, "Load");
    let names = row_names(&harness);
    press(&mut harness, "Autosave");
    let at = player_position(&harness);
    std::fs::set_permissions(&saves, writable).unwrap();
    assert!(stayed, "the rename failed");
    // The one ready autosave first, then the three empty ones.
    let autosaves = ["Autosave", "Autosave 1", "Autosave 2", "Autosave 3"];
    assert_eq!(names[..4], autosaves, "listed among the autosaves");
    assert_eq!(at, Some([77.0, 88.0]), "and it loads");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn older_versions_compare_by_their_numbers() {
    assert!(shell::older("0.9.0", "0.10.0"));
    assert!(!shell::older("0.10.0", "0.9.0"));
    assert!(!shell::older("0.10.0", "0.10.0"), "equal is not older");
    assert!(shell::older("unknown", "0.1.0"));
    assert!(shell::older("", "0.1.0"));
    assert!(
        !shell::older("0.1.0-rc.1", "0.1.0"),
        "the suffix is ignored"
    );
    assert!(shell::older("0.0.9-beta", "0.1.0"));
}

#[test]
fn an_older_save_names_its_version_and_asks_before_it_loads() {
    let dir = user_folder("older-save");
    let mut old = fresh_save();
    old.meta.game_version = "0.0.1".into();
    write_slot(&dir, "slot_1", old, [11.0, 12.0], 0);
    SaveSlots::new(Some(dir.join("saves")))
        .save(
            AUTOSAVES[0],
            &SaveV1 {
                player: [21.0, 22.0],
            },
        )
        .unwrap();
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Load");
    let slot = details(&harness, "Slot 1");
    assert!(slot.contains(" · Older version 0.0.1\n"), "{slot}");
    let autosave = details(&harness, "Autosave 1");
    assert!(autosave.contains(" · Unknown version\n"), "{autosave}");
    press(&mut harness, "Autosave 1");
    assert_eq!(top(&harness), Some(CONFIRM));
    let question = "Autosave 1 was saved by an older version, unknown. Load it?";
    assert!(shows(&harness, question), "the migrated one");
    press(&mut harness, "Cancel");
    press(&mut harness, "Slot 1");
    assert!(shows(
        &harness,
        "Slot 1 was saved by an older version, 0.0.1. Load it?"
    ));
    press(&mut harness, "Cancel");
    assert_eq!(top(&harness), Some(SAVES), "Cancel stays on the screen");
    press(&mut harness, "Slot 1");
    press(&mut harness, "Load");
    assert_eq!(top(&harness), Some(game::GAMEPLAY));
    assert_eq!(player_position(&harness), Some([11.0, 12.0]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_save_of_this_version_loads_without_asking() {
    let dir = user_folder("this-version");
    write_slot(&dir, "slot_1", fresh_save(), [31.0, 32.0], 0);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Load");
    press(&mut harness, "Slot 1");
    assert_eq!(top(&harness), Some(game::GAMEPLAY), "no question");
    assert_eq!(player_position(&harness), Some([31.0, 32.0]));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The group labels, the note and the rows a player sees on the screen on
/// top, in order.
fn layout_texts(harness: &UiHarness) -> Vec<String> {
    let tree = harness.tree();
    let root = super::shown_modal(harness).expect("a screen");
    let mut texts = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if tree.visibility(id) != Some(tungsten::core::ui::Visibility::Visible) {
            continue;
        }
        let parent = tree.parent(id).and_then(|parent| tree.kind(parent));
        match tree.kind(id) {
            Some(WidgetKind::Button) => texts.push(widgets::text_of(tree, id)),
            Some(WidgetKind::Label) if parent != Some(WidgetKind::Button) => {
                texts.push(widgets::text_of(tree, id));
            }
            _ => {}
        }
        stack.extend(tree.children(id).iter().rev().copied());
    }
    texts.retain(|text| text != "Delete" && text != "Back");
    texts
}

#[test]
fn the_load_screen_groups_autosaves_and_slots() {
    let dir = user_folder("groups");
    let save = fresh_save();
    write_slot(&dir, AUTOSAVES[0], save.clone(), [1.0, 2.0], 0);
    write_slot(&dir, "slot_1", save, [1.0, 2.0], 0);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Load");
    let slots = (1..=10).map(|n| format!("Slot {n}"));
    let mut want = vec!["Load".to_owned(), "Autosaves".to_owned()];
    want.extend(["Autosave 1", "Autosave 2", "Autosave 3", "Slots"].map(str::to_owned));
    want.extend(slots.clone());
    assert_eq!(layout_texts(&harness), want);
    tap(&mut harness, "ui_cancel");
    press(&mut harness, "Continue");
    tap(&mut harness, "state_pause");
    press(&mut harness, "Save");
    let note = "Written as you play, when you pause and when you quit.";
    let mut want: Vec<String> = ["Save", "Autosaves", note, "Slots"]
        .map(str::to_owned)
        .into();
    want.extend(slots);
    assert_eq!(layout_texts(&harness), want, "Save mode leaves them out");
    tap(&mut harness, "ui_cancel");
    harness.world_mut().resource_mut::<GameOptions>().autosave = false;
    press(&mut harness, "Save");
    assert!(shows(&harness, "Autosave is off."));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_is_quiet_until_its_row_has_focus() {
    let dir = user_folder("quiet-delete");
    let save = fresh_save();
    write_slot(&dir, "slot_1", save.clone(), [1.0, 2.0], 0);
    write_slot(&dir, "slot_2", save, [1.0, 2.0], 0);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Load");
    let colors = harness.tree().theme().tokens.colors;
    let color = |harness: &UiHarness, name: &str| {
        let delete = named(harness, name).unwrap();
        harness.tree().resolved_style(delete).unwrap().text.color
    };
    let focus = |harness: &mut UiHarness, name: &str| {
        let id = find(harness, name).unwrap();
        assert!(harness.focus(id));
        harness.step(1);
    };
    focus(&mut harness, "Slot 1");
    assert_eq!(color(&harness, "Delete Slot 1"), colors.danger, "its row");
    assert_eq!(color(&harness, "Delete Slot 2"), colors.text_muted);
    focus(&mut harness, "Delete Slot 2");
    assert_eq!(color(&harness, "Delete Slot 2"), colors.danger, "itself");
    assert_eq!(color(&harness, "Delete Slot 1"), colors.text_muted);
    focus(&mut harness, "Back");
    assert_eq!(color(&harness, "Delete Slot 1"), colors.text_muted);
    assert_eq!(color(&harness, "Delete Slot 2"), colors.text_muted);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_row_reads_quieter() {
    let dir = user_folder("quiet-empty");
    write_slot(&dir, "slot_1", fresh_save(), [1.0, 2.0], 0);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let style = |harness: &UiHarness, name: &str| {
        let tree = harness.tree();
        let row = named(harness, name).unwrap();
        let label = tree.children(row)[0];
        let background = tree.resolved_style(row).unwrap().background;
        (background, tree.resolved_style(label).unwrap().text.color)
    };
    press(&mut harness, "Load");
    harness.step(1);
    assert_eq!(style(&harness, "Slot 2").0, [0; 4], "no fill in Load mode");
    tap(&mut harness, "ui_cancel");
    press(&mut harness, "New Game");
    press(&mut harness, "Normal");
    tap(&mut harness, "state_pause");
    press(&mut harness, "Save");
    // Focus away from the rows, so each shows its own look.
    let back = find(&harness, "Back").unwrap();
    assert!(harness.focus(back));
    harness.step(1);
    let theme = harness.tree().theme().clone();
    let colors = theme.tokens.colors;
    let ready = (theme.kinds.button.normal.background, colors.text);
    assert_eq!(style(&harness, "Slot 1"), ready, "a ready row");
    assert_eq!(style(&harness, "Slot 2"), ([0; 4], colors.text_muted));
    let _ = std::fs::remove_dir_all(&dir);
}

/// What a build of schema 0 wrote to `slot` at `at`, which this build's
/// `migrate` refuses.
fn older_slot(dir: &Path, slot: &str, at: u64) {
    let text = format!(
        r#"{{ "envelope": 1, "slot": "{slot}", "schema": 0, "engine_version": "0.1.0", "saved_at_ms": {at}, "data": {{ "player": [1.0, 2.0] }} }}"#
    );
    std::fs::create_dir_all(dir.join("saves")).unwrap();
    std::fs::write(slot_file(dir, slot), text).unwrap();
}

#[test]
fn older_autosaves_take_the_next_autosaves() {
    // An upgrader's three autosaves, of a schema this build refuses: the
    // run's first autosave takes the oldest, the next two the others, and
    // then the three rotate as ever.
    let dir = user_folder("older-autosaves");
    for (slot, at) in AUTOSAVES.into_iter().zip([300, 100, 200]) {
        older_slot(&dir, slot, at);
    }
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Load");
    assert_eq!(details(&harness, "Autosave 1"), "Saved by an older version");
    tap(&mut harness, "ui_cancel");
    press(&mut harness, "New Game");
    press(&mut harness, "Normal");
    assert_eq!(retry_from(&harness), Some(AUTOSAVES[1]), "the oldest");
    for next in [AUTOSAVES[2], AUTOSAVES[0], AUTOSAVES[1]] {
        moved_autosave(&mut harness);
        assert_eq!(retry_from(&harness), Some(next));
    }
    let slots = SaveSlots::new(Some(dir.join("saves")));
    for slot in AUTOSAVES {
        assert!(
            slots.load::<SaveGame>(slot).is_ok(),
            "{slot} holds this run"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_newer_autosave_is_never_written() {
    // A newer version's autosave sits out every rotation; the other two
    // take turns.
    let dir = user_folder("newer-autosave");
    let newer = r#"{ "envelope": 1, "slot": "autosave_2", "schema": 9, "engine_version": "9.0.0", "saved_at_ms": 1, "data": {} }"#;
    std::fs::create_dir_all(dir.join("saves")).unwrap();
    std::fs::write(slot_file(&dir, AUTOSAVES[1]), newer).unwrap();
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "New Game");
    press(&mut harness, "Normal");
    let mut seen = vec![retry_from(&harness)];
    for _ in 0..3 {
        moved_autosave(&mut harness);
        seen.push(retry_from(&harness));
    }
    let (first, third) = (Some(AUTOSAVES[0]), Some(AUTOSAVES[2]));
    assert_eq!(seen, [first, third, first, third]);
    assert_eq!(
        std::fs::read_to_string(slot_file(&dir, AUTOSAVES[1])).unwrap(),
        newer
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn damaged_autosaves_give_way_only_when_nothing_else_can() {
    // Three damaged autosaves: the first is written, as the last resort,
    // and then holds the run's autosaves; the other two stay as they were.
    let dir = user_folder("damaged-autosaves");
    std::fs::create_dir_all(dir.join("saves")).unwrap();
    for slot in AUTOSAVES {
        std::fs::write(slot_file(&dir, slot), "{ not json").unwrap();
    }
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "New Game");
    press(&mut harness, "Normal");
    assert_eq!(retry_from(&harness), Some(AUTOSAVES[0]));
    for _ in 0..2 {
        moved_autosave(&mut harness);
        assert_eq!(retry_from(&harness), Some(AUTOSAVES[0]));
    }
    for slot in &AUTOSAVES[1..] {
        let text = std::fs::read_to_string(slot_file(&dir, slot)).unwrap();
        assert_eq!(text, "{ not json", "{slot} kept");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
