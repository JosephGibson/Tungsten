//! The save seam: the schema's migration, a continued run's restore, a
//! slot that fails to load, and the options' own store.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tungsten::StateStack;
use tungsten::core::{SaveData, SaveSlots, UserFolder};
use tungsten::kit::{Counters, Hit, damage};
use tungsten::testing::UiHarness;
use tungsten_template_basic::game;
use tungsten_template_basic::sample::{HAZARD_START, PLAYER_START, SampleRun};
use tungsten_template_basic::save::{self, SaveGame};
use tungsten_template_basic::shell::confirm::CONFIRM;
use tungsten_template_basic::shell::options::{self, GameOptions};
use tungsten_template_basic::shell::pause::PAUSE;
use tungsten_template_basic::shell::saves::SAVES;
use tungsten_template_basic::shell::toast::Toasts;
use tungsten_template_basic::shell::{self, AUTOSAVE, Difficulty, widgets};

use super::{
    find, harness_with, health, named, player_entity, player_position, press, shows, slot_file,
    start, start_at, tap, top, user_folder,
};

/// The template's save before schema 2: the player's position alone.
#[derive(Serialize, Deserialize)]
struct SaveV1 {
    player: [f32; 2],
}

impl SaveData for SaveV1 {
    const SCHEMA: u32 = 1;
}

#[test]
fn a_schema_1_autosave_migrates() {
    let dir = user_folder("migrate");
    let slots = SaveSlots::new(Some(dir.join("saves")));
    slots
        .save(
            AUTOSAVE,
            &SaveV1 {
                player: [100.0, 200.0],
            },
        )
        .unwrap();
    let save = slots.load::<SaveGame>(AUTOSAVE).expect("schema 1 migrates");
    assert_eq!(save.player, [100.0, 200.0]);
    assert_eq!(save.meta.difficulty, Difficulty::Normal);
    assert_eq!(save.meta.game_version, "unknown");
    assert_eq!(save.meta.playtime_secs, 0.0);
    assert_eq!((save.health.current(), save.health.max()), (5.0, 5.0));
    assert_eq!(save.gems_left, [0, 1, 2]);
    assert_eq!(save.gems, 0);

    // Continued, it plays from there.
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Continue");
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

#[test]
fn a_restored_player_takes_a_first_step_hit_at_its_saved_health() {
    let dir = user_folder("first-step-hit");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start_at(&mut harness, Difficulty::Easy);
    let player = player_entity(&harness);
    assert_eq!(damage(harness.world_mut(), player, 2.0), Hit::Hurt);
    // Past the invulnerability window, so the save holds none of it.
    harness.step(40);
    let mut saved = save::capture(harness.world()).expect("a live run");
    assert_eq!((saved.health.current(), saved.health.max()), (6.0, 8.0));
    // Saved standing on the hazard's start: the first fixed step hits.
    saved.player = [HAZARD_START[0] + 1.0, HAZARD_START[1]];
    SaveSlots::new(Some(dir.join("saves")))
        .save(AUTOSAVE, &saved)
        .unwrap();

    let mut restarted = harness_with(UserFolder::At(dir.clone()));
    press(&mut restarted, "Continue");
    assert_eq!(
        health(&restarted, player_entity(&restarted)),
        (5.0, 8.0),
        "one hit on the saved 6 of 8, at Easy's max"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_slot_that_fails_to_load_is_never_overwritten() {
    let dir = user_folder("newer-save");
    let slot = slot_file(&dir, AUTOSAVE);
    std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
    // A save from a newer build of the game: schema 3.
    let newer = r#"{ "envelope": 1, "slot": "autosave", "schema": 3, "engine_version": "9.0.0", "saved_at_ms": 1, "data": { "player": [1.0, 2.0], "lives": 3 } }"#;
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

// ---- the save and load screen (step 3) ----

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

#[test]
fn the_load_screen_lists_the_autosave_and_three_slots_with_summaries() {
    let dir = user_folder("load-screen");
    let save = fresh_save();
    write_slot(&dir, AUTOSAVE, save.clone(), [10.0, 20.0], 1);
    write_slot(&dir, "slot_2", save, [30.0, 40.0], 2);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Load");
    assert_eq!(top(&harness), Some(SAVES));
    assert_eq!(
        row_text(&harness, "Autosave"),
        "Autosave · just now · 12:04 played · v0.1.0\nGems 1 / 3 · Health 5 / 5"
    );
    assert_eq!(row_text(&harness, "Slot 1"), "Slot 1 · Empty");
    assert_eq!(
        row_text(&harness, "Slot 2"),
        "Slot 2 · just now · 12:04 played · v0.1.0\nGems 2 / 3 · Health 5 / 5"
    );
    assert_eq!(row_text(&harness, "Slot 3"), "Slot 3 · Empty");
    assert!(find(&harness, "Autosave").is_some() && find(&harness, "Slot 2").is_some());
    assert_eq!(
        find(&harness, "Slot 1"),
        None,
        "an empty slot loads nothing"
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
    harness.harness_mut().release_action("move_right");
    harness.step(1);
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
    assert!(row_text(&harness, "Slot 1").starts_with("Slot 1 · just now"));
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
    assert_eq!(row_text(&harness, "Slot 2"), "Slot 2 · Empty");
    assert_eq!(find(&harness, "Delete Slot 2"), None);
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
    assert_eq!(
        row_text(&harness, "Slot 1"),
        "Slot 1 · Saved by a newer version"
    );
    assert_eq!(row_text(&harness, "Slot 3"), "Slot 3 · Damaged");
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
    // A read-only autosave: from a newer build.
    let dir = user_folder("load-asks-read-only");
    write_slot(&dir, "slot_1", fresh_save(), [50.0, 60.0], 0);
    let newer = r#"{ "envelope": 1, "slot": "autosave", "schema": 9, "engine_version": "9.0.0", "saved_at_ms": 1, "data": {} }"#;
    std::fs::write(slot_file(&dir, AUTOSAVE), newer).unwrap();
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    assert!(load_slot_1_asks(&mut harness), "a read-only autosave");
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
fn continue_loads_the_newest_slot() {
    let dir = user_folder("continue-newest");
    let save = fresh_save();
    write_slot(&dir, AUTOSAVE, save.clone(), [10.0, 20.0], 0);
    write_slot(&dir, "slot_2", save.clone(), [30.0, 40.0], 0);
    write_slot(&dir, "slot_3", save, [70.0, 80.0], 0);
    set_saved_at(&dir, AUTOSAVE, 2_000);
    set_saved_at(&dir, "slot_2", 3_000);
    set_saved_at(&dir, "slot_3", 1_000);
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Continue");
    assert_eq!(player_position(&harness), Some([30.0, 40.0]));
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
fn the_autosave_row_is_not_a_save_target() {
    let dir = user_folder("autosave-row");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Save");
    assert_eq!(
        row_text(&harness, "Autosave"),
        "Autosave · Written when you pause or quit"
    );
    assert_eq!(find(&harness, "Autosave"), None);
    tap(&mut harness, "ui_cancel");
    press(&mut harness, "Load");
    assert!(find(&harness, "Autosave").is_some(), "but a load source");
    let _ = std::fs::remove_dir_all(&dir);
}
