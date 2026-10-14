//! Save slots through `App`: a slot saved by one run, loaded by the next
//! with the game's migration (`D-160`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tungsten_core::save::{SaveError, Value};
use tungsten_core::{Config, SaveData, SaveSlots, UserFolder};

use crate::App;

fn user_folder(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("tungsten_save_slots_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn app_at(dir: &Path) -> App {
    let mut config = Config::default();
    config.game.user_folder = UserFolder::At(dir.to_path_buf());
    App::new(config).unwrap()
}

#[derive(Serialize, Deserialize)]
struct ProgressV1 {
    level: u32,
}

impl SaveData for ProgressV1 {
    const SCHEMA: u32 = 1;
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ProgressV2 {
    level: u32,
    gems: u32,
}

impl SaveData for ProgressV2 {
    const SCHEMA: u32 = 2;

    fn migrate(
        from: u32,
        mut data: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        match from {
            1 => {
                data["gems"] = 0.into();
                Ok(data)
            }
            _ => Err(format!("no migration from schema {from}").into()),
        }
    }
}

/// Schema 2 whose migration from 1 fails.
#[derive(Debug, Serialize, Deserialize)]
struct ProgressBroken {
    level: u32,
}

impl SaveData for ProgressBroken {
    const SCHEMA: u32 = 2;

    fn migrate(
        _from: u32,
        _data: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err("the level table changed".into())
    }
}

#[test]
fn a_schema_bump_loads_an_old_slot_through_the_games_migration() {
    let dir = user_folder("bump");
    let mut first = app_at(&dir);
    let slots = first.world_mut().resource::<SaveSlots>().clone();
    assert_eq!(slots.dir(), Some(dir.join("saves").as_path()));
    slots.save("slot-1", &ProgressV1 { level: 4 }).unwrap();

    let mut next = app_at(&dir);
    let loaded: ProgressV2 = next
        .world_mut()
        .resource::<SaveSlots>()
        .load("slot-1")
        .unwrap();
    assert_eq!(loaded, ProgressV2 { level: 4, gems: 0 });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_migration_is_a_typed_error() {
    let dir = user_folder("failed");
    let mut app = app_at(&dir);
    let slots = app.world_mut().resource::<SaveSlots>();
    slots.save("slot-1", &ProgressV1 { level: 4 }).unwrap();
    let before = std::fs::read(dir.join("saves/slot-1.json")).unwrap();
    let error = slots.load::<ProgressBroken>("slot-1").unwrap_err();
    assert!(
        matches!(&error, SaveError::Migration { from: 1, slot, .. } if slot == "slot-1"),
        "{error}"
    );
    assert!(
        error.to_string().contains("the level table changed"),
        "{error}"
    );
    assert_eq!(
        std::fs::read(dir.join("saves/slot-1.json")).unwrap(),
        before
    );
    let _ = std::fs::remove_dir_all(&dir);
}
