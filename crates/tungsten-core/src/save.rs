//! Save slots (`D-160`): the game's own serde types, each in a versioned
//! envelope in the user folder's `saves` folder, with the game's own
//! migrations.
//!
//! ```json
//! { "envelope": 1, "slot": "autosave", "schema": 2, "engine_version": "0.73.0", "saved_at_ms": 1760400000000, "data": { "…": "the game's type" } }
//! ```

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::GameConfig;
use crate::persist::{WriteError, write_atomic};

/// The JSON value [`SaveData::migrate`] rewrites, re-exported so a game
/// needs no `serde_json` dependency of its own to write a migration.
pub use serde_json::Value;

/// The envelope format this build reads and writes.
const ENVELOPE: u32 = 1;
const SLOT_NAME_MAX_LEN: usize = 64;

/// A game's saved data: its serde type, its schema number and its
/// migrations.
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use tungsten_core::SaveData;
/// use tungsten_core::save::Value;
///
/// #[derive(Serialize, Deserialize)]
/// struct Progress {
///     level: u32,
///     gems: u32,
/// }
///
/// impl SaveData for Progress {
///     const SCHEMA: u32 = 2;
///
///     fn migrate(
///         from: u32,
///         mut data: Value,
///     ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
///         // Schema 1 had no gems.
///         if from == 1 {
///             data["gems"] = 0.into();
///             return Ok(data);
///         }
///         Err(format!("no migration from schema {from}").into())
///     }
/// }
/// ```
pub trait SaveData: Serialize + DeserializeOwned {
    /// This type's schema; bump it when the type changes shape.
    const SCHEMA: u32;

    /// Rewrites `data`, saved under the older schema `from`, into this
    /// schema's shape. The default refuses every older schema.
    ///
    /// # Errors
    ///
    /// Whatever the game's migration reports; [`SaveSlots::load`] returns it
    /// as [`SaveError::Migration`].
    fn migrate(from: u32, data: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let _ = data;
        Err(format!("schema {from} has no migration to schema {}", Self::SCHEMA).into())
    }
}

/// The game's save slots in its user folder: a `World` resource `App`
/// inserts. Without a folder (smoke mode, `UserFolder::Off`, or no
/// `game.id`) every call returns [`SaveError::NoFolder`].
#[derive(Debug, Clone, Default)]
pub struct SaveSlots {
    dir: Option<PathBuf>,
}

/// One slot as [`SaveSlots::list`] reads it: the envelope's header.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SlotInfo {
    /// The slot's name, from its file name.
    pub name: String,
    /// The schema its data was saved under.
    pub schema: u32,
    /// The engine version that saved it.
    pub engine_version: String,
    /// When it was saved: Unix milliseconds, 0 for a clock before 1970.
    pub saved_at_ms: u64,
}

/// What [`SaveSlots::list`] found.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct SlotList {
    /// Every readable slot, by name.
    pub slots: Vec<SlotInfo>,
    /// One line per `.json` file skipped as unreadable.
    pub warnings: Vec<String>,
}

/// A failed save-slot call; a runtime condition, never a panic (`D-022`).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SaveError {
    /// Smoke mode, `UserFolder::Off`, or no `game.id` and no override.
    #[error("the game has no user folder, so it has no save slots")]
    NoFolder,
    /// Not 1–64 of `a`–`z`, `0`–`9`, `.`, `_` and `-` starting with a letter
    /// or digit, or a Windows device name.
    #[error(
        "invalid slot name '{name}': expected 1-64 of a-z, 0-9, '.', '_' or '-', starting with a letter or digit, and not a Windows device name"
    )]
    InvalidName {
        /// The name given.
        name: String,
    },
    /// No such slot.
    #[error("no save slot '{slot}'")]
    NotFound {
        /// The slot.
        slot: String,
    },
    /// The file is not an envelope, or its data does not fit the type.
    #[error("save slot '{slot}' is corrupt: {source}")]
    Corrupt {
        /// The slot.
        slot: String,
        /// Why it failed to parse.
        source: serde_json::Error,
    },
    /// The game's type failed to serialize.
    #[error("save slot '{slot}' could not be serialized: {source}")]
    Serialize {
        /// The slot.
        slot: String,
        /// Why.
        source: serde_json::Error,
    },
    /// An envelope format this build does not know, from a newer engine.
    #[error("save slot '{slot}' has envelope {envelope}; this build reads envelope 1")]
    UnknownEnvelope {
        /// The slot.
        slot: String,
        /// The envelope it names.
        envelope: u32,
    },
    /// Saved under a schema newer than the type's, by a newer build.
    #[error("save slot '{slot}' has schema {found}, newer than this build's {supported}")]
    NewerSchema {
        /// The slot.
        slot: String,
        /// Its schema.
        found: u32,
        /// The type's [`SaveData::SCHEMA`].
        supported: u32,
    },
    /// The game's migration failed, or its result did not fit the type.
    #[error("save slot '{slot}' failed to migrate from schema {from}: {source}")]
    Migration {
        /// The slot.
        slot: String,
        /// The schema it was saved under.
        from: u32,
        /// Why.
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Reading, listing or deleting failed.
    #[error("save slot I/O on '{}' failed: {source}", path.display())]
    Io {
        /// The file or folder.
        path: PathBuf,
        /// The failure.
        source: std::io::Error,
    },
    /// The write failed; the slot's file is unchanged.
    #[error(transparent)]
    Write(#[from] WriteError),
}

#[derive(Serialize)]
struct EnvelopeOut<'a, T> {
    envelope: u32,
    slot: &'a str,
    schema: u32,
    engine_version: &'static str,
    saved_at_ms: u64,
    data: &'a T,
}

#[derive(Deserialize)]
struct EnvelopeVersion {
    envelope: u32,
}

#[derive(Deserialize)]
struct EnvelopeIn {
    schema: u32,
    data: Value,
}

#[derive(Deserialize)]
struct Header {
    schema: u32,
    engine_version: String,
    saved_at_ms: u64,
}

impl SaveSlots {
    /// Slots in `dir`, the user folder's `saves`; `None` for none.
    #[must_use]
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self { dir }
    }

    /// The folder slots live in; created on the first save.
    #[must_use]
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Saves `data` in `slot` through `write_atomic`, so a crash mid-save
    /// leaves the old slot.
    ///
    /// # Errors
    ///
    /// [`SaveError::NoFolder`], [`SaveError::InvalidName`],
    /// [`SaveError::Serialize`] or [`SaveError::Write`].
    pub fn save<T: SaveData>(&self, slot: &str, data: &T) -> Result<(), SaveError> {
        let path = self.slot_path(slot)?;
        let envelope = EnvelopeOut {
            envelope: ENVELOPE,
            slot,
            schema: T::SCHEMA,
            engine_version: env!("CARGO_PKG_VERSION"),
            saved_at_ms: now_ms(SystemTime::now()),
            data,
        };
        let mut json =
            serde_json::to_vec_pretty(&envelope).map_err(|source| SaveError::Serialize {
                slot: slot.to_string(),
                source,
            })?;
        json.push(b'\n');
        write_atomic(&path, &json)?;
        Ok(())
    }

    /// Loads `slot` as `T`: data saved under `T::SCHEMA` directly, under an
    /// older schema through [`SaveData::migrate`] once. A failed load never
    /// panics and never changes the file.
    ///
    /// # Errors
    ///
    /// [`SaveError::NoFolder`], [`SaveError::InvalidName`],
    /// [`SaveError::NotFound`], [`SaveError::Io`], [`SaveError::Corrupt`],
    /// [`SaveError::UnknownEnvelope`], [`SaveError::NewerSchema`] or
    /// [`SaveError::Migration`].
    pub fn load<T: SaveData>(&self, slot: &str) -> Result<T, SaveError> {
        let path = self.slot_path(slot)?;
        let bytes = std::fs::read(&path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                SaveError::NotFound {
                    slot: slot.to_string(),
                }
            } else {
                SaveError::Io {
                    path: path.clone(),
                    source,
                }
            }
        })?;
        let corrupt = |source| SaveError::Corrupt {
            slot: slot.to_string(),
            source,
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(corrupt)?;
        let version = EnvelopeVersion::deserialize(&value).map_err(corrupt)?;
        if version.envelope != ENVELOPE {
            return Err(SaveError::UnknownEnvelope {
                slot: slot.to_string(),
                envelope: version.envelope,
            });
        }
        let envelope = EnvelopeIn::deserialize(value).map_err(corrupt)?;
        if envelope.schema == T::SCHEMA {
            return T::deserialize(envelope.data).map_err(corrupt);
        }
        if envelope.schema > T::SCHEMA {
            return Err(SaveError::NewerSchema {
                slot: slot.to_string(),
                found: envelope.schema,
                supported: T::SCHEMA,
            });
        }
        let from = envelope.schema;
        let migration = |source| SaveError::Migration {
            slot: slot.to_string(),
            from,
            source,
        };
        let migrated = T::migrate(from, envelope.data).map_err(migration)?;
        T::deserialize(migrated).map_err(|error| migration(Box::new(error)))
    }

    /// Every readable slot, sorted by name, from each envelope's header. A
    /// `.json` file that is not a readable slot is skipped with a line in
    /// [`SlotList::warnings`], and [`load`](Self::load) reports it; the
    /// write's temp files are dotfiles and are not listed.
    ///
    /// # Errors
    ///
    /// [`SaveError::NoFolder`], or [`SaveError::Io`] when the folder cannot
    /// be read. A folder not yet created lists no slots.
    pub fn list(&self) -> Result<SlotList, SaveError> {
        let dir = self.dir.as_deref().ok_or(SaveError::NoFolder)?;
        let io_error = |source| SaveError::Io {
            path: dir.to_path_buf(),
            source,
        };
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SlotList::default());
            }
            Err(error) => return Err(io_error(error)),
        };
        let mut names = Vec::new();
        for entry in entries {
            let file_name = entry.map_err(io_error)?.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if let Some(stem) = file_name.strip_suffix(".json")
                && !file_name.starts_with('.')
            {
                names.push(stem.to_string());
            }
        }
        names.sort_unstable();

        let mut list = SlotList::default();
        for name in names {
            match read_header(dir, &name) {
                Ok(header) => list.slots.push(SlotInfo {
                    name,
                    schema: header.schema,
                    engine_version: header.engine_version,
                    saved_at_ms: header.saved_at_ms,
                }),
                Err(reason) => list
                    .warnings
                    .push(format!("save slot '{name}' skipped: {reason}")),
            }
        }
        Ok(list)
    }

    /// Deletes `slot`.
    ///
    /// # Errors
    ///
    /// [`SaveError::NoFolder`], [`SaveError::InvalidName`],
    /// [`SaveError::NotFound`] or [`SaveError::Io`].
    pub fn delete(&self, slot: &str) -> Result<(), SaveError> {
        let path = self.slot_path(slot)?;
        std::fs::remove_file(&path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                SaveError::NotFound {
                    slot: slot.to_string(),
                }
            } else {
                SaveError::Io { path, source }
            }
        })
    }

    fn slot_path(&self, slot: &str) -> Result<PathBuf, SaveError> {
        if !is_valid_slot_name(slot) {
            return Err(SaveError::InvalidName {
                name: slot.to_string(),
            });
        }
        let dir = self.dir.as_deref().ok_or(SaveError::NoFolder)?;
        Ok(dir.join(format!("{slot}.json")))
    }
}

/// `game.id`'s rule, lowercase only, so no two names collide on a
/// case-insensitive file system.
fn is_valid_slot_name(name: &str) -> bool {
    let game = GameConfig {
        id: Some(name.to_string()),
        ..GameConfig::default()
    };
    !name.is_empty()
        && name.len() <= SLOT_NAME_MAX_LEN
        && !name.bytes().any(|b| b.is_ascii_uppercase())
        && game.validate().is_ok()
}

fn read_header(dir: &Path, name: &str) -> Result<Header, String> {
    let bytes = std::fs::read(dir.join(format!("{name}.json"))).map_err(|e| e.to_string())?;
    if !is_valid_slot_name(name) {
        return Err("not a valid slot name".into());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let version = EnvelopeVersion::deserialize(&value).map_err(|e| e.to_string())?;
    if version.envelope != ENVELOPE {
        return Err(format!("envelope {}", version.envelope));
    }
    Header::deserialize(&value).map_err(|e| e.to_string())
}

/// Unix milliseconds; 0 for a clock before 1970.
fn now_ms(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH).map_or(0, |since| {
        u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
    })
}

#[cfg(test)]
#[path = "tests/save.rs"]
mod tests;
