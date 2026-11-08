use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use tungsten_core::{Config, SaveSlots, Settings};

use super::*;
use crate::App;
use crate::plugins::DefaultPlugins;

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn tempdir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_describe_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// A game folder with one sprite, one prefab and a manifest naming both.
fn game_folder() -> PathBuf {
    let dir = tempdir();
    let png = dir.join("assets/sprites/dot.png");
    fs::create_dir_all(png.parent().unwrap()).unwrap();
    image::RgbaImage::new(8, 8).save(&png).unwrap();
    write(
        &dir.join("assets/prefabs/goblin.json"),
        r#"{ "sprite": { "asset_id": "dot" }, "components": { "lifetime": { "seconds": 2.0 } } }"#,
    );
    write(
        &dir.join("assets/manifest.json"),
        r#"{
            "sprites": { "dot": { "path": "sprites/dot.png" } },
            "prefabs": { "goblin": { "path": "prefabs/goblin.json" } }
        }"#,
    );
    dir
}

#[cfg(feature = "kit")]
#[test]
fn a_description_lists_components_with_kit_fields_hooks_and_manifest_ids() {
    let dir = game_folder();
    let mut app = App::new(Config::default()).unwrap();
    app.add_plugins(tungsten_kit::KitPlugins::set());
    app.world_mut()
        .add_script_action("start_run", "Starts a run.", |_, _| Ok(()));
    app.set_manifest_roots(vec![dir.join("assets/manifest.json")]);
    let out = dir.join("describe.json");
    app.run_describe(&out, &dir).unwrap();
    let description: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();

    assert_eq!(description["version"], 3);
    assert_eq!(description["errors"], json!([]), "{description:#}");
    let components = description["components"].as_array().unwrap();
    let named = |name: &str| components.iter().find(|c| c["name"] == name).cloned();
    // An engine component, with no kit description: its name alone.
    assert_eq!(named("rigid_body"), Some(json!({ "name": "rigid_body" })));
    let lifetime = named("lifetime").expect("the kit's lifetime");
    assert!(!lifetime["summary"].as_str().unwrap().is_empty());
    let fields = lifetime["fields"].as_array().unwrap();
    assert!(
        fields
            .iter()
            .any(|f| f["name"] == "seconds" && f["type"].as_str().unwrap().starts_with("f32")),
        "{lifetime:#}"
    );
    assert!(!description["shapes"].as_array().unwrap().is_empty());
    assert!(!description["kinds"].as_array().unwrap().is_empty());

    let probes = description["script"]["probes"].as_array().unwrap();
    let names: Vec<&str> = probes.iter().map(|p| p["name"].as_str().unwrap()).collect();
    for name in [
        "camera", "count", "entities", "frame", "log", "states", "tagged",
    ] {
        assert!(names.contains(&name), "{names:?}");
    }
    assert!(
        probes
            .iter()
            .all(|p| !p["doc"].as_str().unwrap().is_empty())
    );
    assert_eq!(
        description["script"]["actions"],
        json!([
            {
                "name": "place",
                "doc": "Sets the first entity holding `{\"tag\": t}` at rest at `\"at\": [x, y]`; fails with none."
            },
            { "name": "start_run", "doc": "Starts a run." }
        ])
    );
    assert_eq!(description["assets"]["sprites"], json!(["dot"]));
    assert_eq!(description["assets"]["prefabs"], json!(["goblin"]));
    assert_eq!(description["assets"]["sounds"], json!([]));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_bad_file_is_listed_not_fatal() {
    let dir = game_folder();
    write(&dir.join("assets/prefabs/goblin.json"), "{ not json");
    let mut app = App::new(Config::default()).unwrap();
    app.set_manifest_roots(vec![dir.join("assets/manifest.json")]);
    let out = dir.join("describe.json");
    app.run_describe(&out, &dir).unwrap();
    let description: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    let errors = description["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0]["file"], "assets/prefabs/goblin.json");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn describe_lists_each_prefab_resolved_at_version_3() {
    let dir = game_folder();
    write(
        &dir.join("assets/prefabs/goblin.json"),
        r#"{ "sprite": { "asset_id": "dot", "z_order": 2 }, "tag": "goblin" }"#,
    );
    write(
        &dir.join("assets/prefabs/goblin_red.json"),
        r#"{ "extends": "goblin", "tag": "red" }"#,
    );
    write(
        &dir.join("assets/manifest.json"),
        r#"{
            "sprites": { "dot": { "path": "sprites/dot.png" } },
            "prefabs": {
                "goblin": { "path": "prefabs/goblin.json" },
                "goblin_red": { "path": "prefabs/goblin_red.json" }
            }
        }"#,
    );
    let mut app = App::new(Config::default()).unwrap();
    app.set_manifest_roots(vec![dir.join("assets/manifest.json")]);
    let out = dir.join("describe.json");
    app.run_describe(&out, &dir).unwrap();
    let description: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();

    assert_eq!(description["version"], 3);
    assert_eq!(description["errors"], json!([]), "{description:#}");
    let sprite = json!({ "asset_id": "dot", "color": [255, 255, 255, 255], "z_order": 2 });
    assert_eq!(
        description["prefabs"],
        json!([
            {
                "name": "goblin",
                "extends": null,
                "prefab": { "sprite": sprite, "tag": "goblin", "visible": true }
            },
            {
                "name": "goblin_red",
                "extends": "goblin",
                "prefab": { "sprite": sprite, "tag": "red", "visible": true }
            }
        ])
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `name` = `value` pairs as an environment.
fn env_of(vars: Vec<(&'static str, OsString)>) -> impl Fn(&str) -> Option<OsString> {
    move |name| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.clone())
    }
}

#[test]
fn a_describe_run_resolves_no_user_folder_and_leaves_a_damaged_settings_file() {
    let user = tempdir();
    let settings = user.join("settings").join(Settings::FILE);
    let bad = user
        .join("settings")
        .join(format!("{}.bad", Settings::FILE));
    let mut config = Config::default();
    config.game.id = Some("describe-game".to_owned());

    let describing = env_of(vec![
        ("TUNGSTEN_DESCRIBE", OsString::from("describe.json")),
        ("TUNGSTEN_USER_DIR", user.clone().into_os_string()),
    ]);
    write(&settings, "{ not json");
    let app = App::build(config.clone(), DefaultPlugins::set(), &describing).unwrap();
    assert_eq!(app.world().resource::<SaveSlots>().dir(), None);
    assert_eq!(fs::read_to_string(&settings).unwrap(), "{ not json");
    assert!(!bad.exists());

    // The same folder without the describe variable is the game's, and
    // its damaged file is quarantined.
    let playing = env_of(vec![("TUNGSTEN_USER_DIR", user.clone().into_os_string())]);
    let app = App::build(config, DefaultPlugins::set(), &playing).unwrap();
    assert!(app.world().resource::<SaveSlots>().dir().is_some());
    assert!(bad.exists(), "the control run quarantines the damaged file");
    let _ = fs::remove_dir_all(&user);
}

fn title(_: &World) -> String {
    "Game".to_owned()
}

fn saving_on(_: &World) -> bool {
    true
}

fn save(_: &mut World, _: tungsten_core::ui::UiAction) {}

/// A registered screen, for the description's names.
struct Blank;

impl crate::GameState for Blank {
    fn id(&self) -> crate::StateId {
        "settings"
    }

    fn on_enter(&mut self, _ctx: &mut crate::StateContext) {}

    fn on_exit(&mut self, _ctx: &mut crate::StateContext) {}

    fn update(&mut self, _world: &mut World) {}
}

fn blank() -> Box<dyn crate::GameState> {
    Box::new(Blank)
}

#[test]
fn a_description_lists_screens_and_the_names_they_may_use() {
    let dir = game_folder();
    fs::remove_file(dir.join("assets/prefabs/goblin.json")).unwrap();
    write(
        &dir.join("assets/ui/screens/pause.json"),
        r#"{"version": 1, "root": {"kind": "screen", "width": "menu_width", "children": [
            {"kind": "display", "style": "heading", "bind": {"text": "title"}},
            {"kind": "button", "key": "save", "text": "Save", "bind": {"enabled": "saving_on"},
             "on": {"action": "save"}},
            {"kind": "button", "key": "settings", "text": "Settings", "on": {"push": "settings"}}
        ]}}"#,
    );
    write(
        &dir.join("assets/manifest.json"),
        r#"{
            "sprites": { "dot": { "path": "sprites/dot.png" } },
            "ui_screens": { "pause": { "path": "ui/screens/pause.json" } }
        }"#,
    );
    let mut app = App::new(Config::default()).unwrap();
    let world = app.world_mut();
    tungsten_core::ui::UiBindings::of(world)
        .text("title", title)
        .enabled("saving_on", saving_on)
        .action("save", save);
    crate::screens::register(world, "settings", blank);
    app.set_manifest_roots(vec![dir.join("assets/manifest.json")]);
    let out = dir.join("describe.json");
    app.run_describe(&out, &dir).unwrap();
    let description: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();

    assert_eq!(description["version"], 3);
    assert_eq!(description["errors"], json!([]), "{description:#}");
    assert_eq!(description["assets"]["ui_screens"], json!(["pause"]));
    let ui = &description["ui"];
    assert_eq!(ui["text"], json!(["title"]));
    assert_eq!(ui["enabled"], json!(["saving_on"]));
    assert_eq!(ui["actions"], json!(["save"]));
    assert_eq!(ui["value"], json!([]));
    assert_eq!(ui["screens"], json!(["settings"]));
    assert_eq!(
        description["screens"],
        json!([{
            "id": "pause",
            "file": "assets/ui/screens/pause.json",
            "keys": ["save", "settings"],
            "uses": {
                "text": ["title"], "value": [], "fill": [], "visible": [],
                "enabled": ["saving_on"], "rows": [], "mounts": [], "actions": ["save"],
                "styles": ["heading"], "sizes": ["menu_width"], "screens": ["settings"]
            }
        }])
    );
    let _ = fs::remove_dir_all(&dir);
}
