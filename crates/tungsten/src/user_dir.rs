//! The game's per-user folder (`D-119`, `D-159`): where its settings, saves
//! and logs live, resolved from smoke mode, `game.id`, `game.user_folder`,
//! the environment and the platform; a script run's from the environment
//! alone (redo 7.2).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use tungsten_core::{GameConfig, UserFolder};

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

/// The game's folders. `logs` is created at startup, `settings` on the first
/// save of the settings or bindings, `saves` on the first slot saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UserDirs {
    pub(crate) settings: PathBuf,
    pub(crate) saves: PathBuf,
    pub(crate) logs: PathBuf,
}

impl UserDirs {
    /// `<dir>/settings`, `<dir>/saves` and `<dir>/logs`.
    fn under(dir: &Path) -> Self {
        Self {
            settings: dir.join("settings"),
            saves: dir.join("saves"),
            logs: dir.join("logs"),
        }
    }
}

/// A script run's folders (redo 7.2): `TUNGSTEN_USER_DIR`'s alone, over
/// code's `UserFolder`, and `None` without it (or set but empty) or with an
/// invalid id, so a script never touches the player's folder. `env` looks a
/// variable up, as [`resolve`]'s does.
pub(crate) fn resolve_scripted(
    game: &GameConfig,
    env: impl Fn(&str) -> Option<OsString>,
) -> Option<UserDirs> {
    if game.validate().is_err() {
        return None;
    }
    env(USER_DIR_ENV)
        .filter(|dir| !dir.is_empty())
        .map(|dir| UserDirs::under(Path::new(&dir)))
}

/// Resolves the game's folders, in this order: smoke mode, an invalid id and
/// `UserFolder::Off` give `None`; `UserFolder::At` its folder; then
/// `TUNGSTEN_USER_DIR` (set but empty, `None`); then without an id `None`;
/// then the platform's folders, `None` when its variables give no absolute
/// base. Code wins over the environment, so a test that pins a folder stays
/// pinned in a developer's shell; smoke mode wins over both. `env` looks a
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
    match &game.user_folder {
        UserFolder::Off => return None,
        UserFolder::At(dir) => return Some(UserDirs::under(dir)),
        _ => {}
    }
    if let Some(dir) = env(USER_DIR_ENV) {
        if dir.is_empty() {
            return None;
        }
        return Some(UserDirs::under(Path::new(&dir)));
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
