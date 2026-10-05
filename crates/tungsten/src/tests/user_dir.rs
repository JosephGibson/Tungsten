use super::*;
use std::path::Path;

const HOME: (&str, &str) = ("HOME", "/home/u");
const APPDATA: (&str, &str) = ("APPDATA", r"C:\Users\u\AppData\Roaming");
const LOCALAPPDATA: (&str, &str) = ("LOCALAPPDATA", r"C:\Users\u\AppData\Local");
const OVERRIDE: (&str, &str) = (USER_DIR_ENV, "/tmp/tu");
const PLATFORMS: [Platform; 3] = [Platform::Unix, Platform::Windows, Platform::MacOs];

fn game(id: &str) -> GameConfig {
    let mut game = GameConfig::default();
    game.id = Some(id.to_string());
    game
}

fn env_of<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

fn on(platform: Platform, vars: &[(&str, &str)]) -> Option<UserDirs> {
    resolve(&game("my-game"), false, platform, env_of(vars))
}

fn home_fallbacks() -> UserDirs {
    UserDirs {
        settings: PathBuf::from("/home/u/.config/my-game"),
        saves: PathBuf::from("/home/u/.local/share/my-game/saves"),
        logs: PathBuf::from("/home/u/.local/state/my-game/logs"),
    }
}

#[test]
fn current_platform_matches_the_host() {
    let expected = if cfg!(windows) {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::MacOs
    } else {
        Platform::Unix
    };
    assert_eq!(Platform::current(), expected);
}

#[test]
fn linux_uses_absolute_xdg_variables() {
    let vars = [
        HOME,
        ("XDG_CONFIG_HOME", "/cfg"),
        ("XDG_DATA_HOME", "/data"),
        ("XDG_STATE_HOME", "/state"),
    ];
    let dirs = on(Platform::Unix, &vars).unwrap();
    assert_eq!(dirs.settings, Path::new("/cfg/my-game"));
    assert_eq!(dirs.saves, Path::new("/data/my-game/saves"));
    assert_eq!(dirs.logs, Path::new("/state/my-game/logs"));
}

#[test]
fn linux_falls_back_under_home_for_unset_empty_or_relative_xdg_variables() {
    assert_eq!(on(Platform::Unix, &[HOME]), Some(home_fallbacks()));
    let empty = [
        HOME,
        ("XDG_CONFIG_HOME", ""),
        ("XDG_DATA_HOME", ""),
        ("XDG_STATE_HOME", ""),
    ];
    assert_eq!(on(Platform::Unix, &empty), Some(home_fallbacks()));
    let relative = [
        HOME,
        ("XDG_CONFIG_HOME", "cfg"),
        ("XDG_DATA_HOME", "./data"),
        ("XDG_STATE_HOME", "state"),
    ];
    assert_eq!(on(Platform::Unix, &relative), Some(home_fallbacks()));
}

#[test]
fn linux_without_a_usable_home_has_no_folder() {
    assert_eq!(on(Platform::Unix, &[]), None);
    assert_eq!(on(Platform::Unix, &[("HOME", "")]), None);
    assert_eq!(on(Platform::Unix, &[("HOME", "home/u")]), None);
    // Two of three variables still need `HOME` for the third.
    let partial = [("XDG_CONFIG_HOME", "/cfg"), ("XDG_DATA_HOME", "/data")];
    assert_eq!(on(Platform::Unix, &partial), None);
    let all = [
        ("XDG_CONFIG_HOME", "/cfg"),
        ("XDG_DATA_HOME", "/data"),
        ("XDG_STATE_HOME", "/state"),
    ];
    assert!(on(Platform::Unix, &all).is_some());
}

#[test]
fn windows_uses_appdata_and_localappdata() {
    let dirs = on(Platform::Windows, &[HOME, APPDATA, LOCALAPPDATA]).unwrap();
    let roaming = Path::new(APPDATA.1).join("my-game");
    assert_eq!(dirs.settings, roaming);
    assert_eq!(dirs.saves, roaming.join("saves"));
    assert_eq!(
        dirs.logs,
        Path::new(LOCALAPPDATA.1).join("my-game").join("logs")
    );
    assert_eq!(on(Platform::Windows, &[HOME, APPDATA]), None);
    assert_eq!(
        on(Platform::Windows, &[("APPDATA", ""), LOCALAPPDATA]),
        None
    );
}

#[test]
fn macos_uses_library() {
    let dirs = on(Platform::MacOs, &[HOME, ("XDG_STATE_HOME", "/state")]).unwrap();
    assert_eq!(
        dirs.settings,
        Path::new("/home/u/Library/Application Support/my-game")
    );
    assert_eq!(
        dirs.saves,
        Path::new("/home/u/Library/Application Support/my-game/saves")
    );
    assert_eq!(dirs.logs, Path::new("/home/u/Library/Logs/my-game"));
    assert_eq!(on(Platform::MacOs, &[("HOME", "home/u")]), None);
}

#[test]
fn the_override_wins_on_every_platform() {
    let vars = [
        HOME,
        ("XDG_STATE_HOME", "/state"),
        APPDATA,
        LOCALAPPDATA,
        OVERRIDE,
    ];
    let base = Path::new(OVERRIDE.1);
    for platform in PLATFORMS {
        assert_eq!(
            on(platform, &vars),
            Some(UserDirs {
                settings: base.join("settings"),
                saves: base.join("saves"),
                logs: base.join("logs"),
            }),
            "{platform:?}"
        );
    }
}

#[test]
fn a_relative_override_stays_relative_to_the_working_directory() {
    let dirs = on(Platform::Unix, &[(USER_DIR_ENV, "run")]).unwrap();
    assert_eq!(dirs.logs, Path::new("run").join("logs"));
}

#[test]
fn an_empty_override_means_no_folder() {
    for platform in PLATFORMS {
        let vars = [HOME, APPDATA, LOCALAPPDATA, (USER_DIR_ENV, "")];
        assert_eq!(on(platform, &vars), None, "{platform:?}");
    }
}

#[test]
fn smoke_mode_has_no_folder_even_with_an_override() {
    for platform in PLATFORMS {
        let vars = [HOME, APPDATA, LOCALAPPDATA, OVERRIDE];
        let dirs = resolve(&game("my-game"), true, platform, env_of(&vars));
        assert_eq!(dirs, None, "{platform:?}");
    }
}

#[test]
fn no_id_has_no_platform_folder() {
    let none = GameConfig::default();
    for platform in PLATFORMS {
        let vars = [HOME, APPDATA, LOCALAPPDATA];
        assert_eq!(
            resolve(&none, false, platform, env_of(&vars)),
            None,
            "{platform:?}"
        );
    }
    // The override comes first and needs no id.
    let dirs = resolve(&none, false, Platform::Unix, env_of(&[OVERRIDE])).unwrap();
    assert_eq!(dirs.logs, Path::new(OVERRIDE.1).join("logs"));
}

#[test]
fn an_invalid_id_builds_no_path() {
    for id in ["../x", "/abs", "a/b", ".x", "con.txt", ""] {
        for platform in PLATFORMS {
            for vars in [
                &[HOME, APPDATA, LOCALAPPDATA][..],
                &[HOME, APPDATA, LOCALAPPDATA, OVERRIDE][..],
            ] {
                let dirs = resolve(&game(id), false, platform, env_of(vars));
                assert_eq!(dirs, None, "{id:?} on {platform:?}");
            }
        }
    }
}
