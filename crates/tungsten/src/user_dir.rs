//! The game's per-user folder (`D-119`): where its settings, saves and logs
//! live, resolved from `game.id`, smoke mode, the platform and the
//! environment.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use tungsten_core::GameConfig;

/// Replaces the platform folder; set but empty, the game has no folder.
pub(crate) const USER_DIR_ENV: &str = "TUNGSTEN_USER_DIR";

/// Whose folder rules apply. A parameter, so tests on one platform cover all
/// three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Platform {
    /// Linux and other Unix: the XDG base directories.
    Unix,
    Windows,
    MacOs,
}

impl Platform {
    pub(crate) const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Unix
        }
    }
}

/// The game's folders. Only `logs` is created by the engine so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UserDirs {
    #[allow(dead_code, reason = "settings (W11b) read and create it")]
    pub(crate) settings: PathBuf,
    #[allow(dead_code, reason = "save slots (W11c) read and create it")]
    pub(crate) saves: PathBuf,
    pub(crate) logs: PathBuf,
}

/// Resolves the game's folders, or `None`: in smoke mode, for an invalid
/// id, for an empty `TUNGSTEN_USER_DIR`, without an id and no override, or
/// when the platform's variables give no absolute base. `env` looks a
/// variable up, so tests need no `set_var`.
pub(crate) fn resolve(
    game: &GameConfig,
    smoke: bool,
    platform: Platform,
    env: impl Fn(&str) -> Option<OsString>,
) -> Option<UserDirs> {
    if smoke || game.validate().is_err() {
        return None;
    }
    if let Some(dir) = env(USER_DIR_ENV) {
        if dir.is_empty() {
            return None;
        }
        let dir = PathBuf::from(dir);
        return Some(UserDirs {
            settings: dir.join("settings"),
            saves: dir.join("saves"),
            logs: dir.join("logs"),
        });
    }
    let id = game.id.as_deref()?;
    match platform {
        Platform::Unix => {
            let home = unix_home(&env);
            let base = |var: &str, fallback: &str| match env(var) {
                Some(value) if is_unix_absolute(&value) => Some(PathBuf::from(value)),
                _ => home.as_ref().map(|home| home.join(fallback)),
            };
            Some(UserDirs {
                settings: base("XDG_CONFIG_HOME", ".config")?.join(id),
                saves: base("XDG_DATA_HOME", ".local/share")?
                    .join(id)
                    .join("saves"),
                logs: base("XDG_STATE_HOME", ".local/state")?
                    .join(id)
                    .join("logs"),
            })
        }
        Platform::Windows => {
            let roaming = env("APPDATA").filter(|value| !value.is_empty())?;
            let local = env("LOCALAPPDATA").filter(|value| !value.is_empty())?;
            let settings = PathBuf::from(roaming).join(id);
            Some(UserDirs {
                saves: settings.join("saves"),
                settings,
                logs: PathBuf::from(local).join(id).join("logs"),
            })
        }
        Platform::MacOs => {
            let home = unix_home(&env)?;
            let settings = home.join("Library/Application Support").join(id);
            Some(UserDirs {
                saves: settings.join("saves"),
                settings,
                logs: home.join("Library/Logs").join(id),
            })
        }
    }
}

/// `HOME` when it is set and absolute.
fn unix_home(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    env("HOME")
        .filter(|home| is_unix_absolute(home))
        .map(PathBuf::from)
}

/// A Unix path is absolute when it starts with `/`; checked by hand, since
/// `Path::is_absolute` follows the host and the tests also run on Windows.
fn is_unix_absolute(path: &OsStr) -> bool {
    path.as_encoded_bytes().first() == Some(&b'/')
}

#[cfg(test)]
#[path = "tests/user_dir.rs"]
mod tests;
