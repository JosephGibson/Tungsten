//! The `story` scene (redo 8.1's first plan: P7-3, P8-1, P8-2, P8-5): the
//! kit's story systems in one room, there for their feel. A name card opens
//! first, a text field whose Enter writes the warden's name under its feet.
//! The warden walks (the arrows or WASD); while a prompt shows, [`INTERACT`]
//! (E) talks to the keeper, lights the lamp or reads the notice board, and
//! [`JOURNAL`] (J) opens the journal over the warden's quests. The keeper's
//! conversation offers [`QUEST`]; the lit lamp moves it to its report, and
//! telling the keeper ends it with its reward. The play state holds the
//! game clock under the box, the journal and the card.
//!
//! Every fixtures scene composes the one shared manifest and this one alone
//! registers the kit, so a kit data entry or prefab there would fail the
//! other scenes' loads: the speakers, conversations, quest and prefabs are
//! inline JSON here, which [`setup`] puts in the World's registries, and
//! the manifest lists the scene's screen files alone (`dialogue`, `journal`
//! and [`NAME_SCREEN`]), which a load parses and holds to no binding.
//! Entries inserted in code skip the load's world checks, so
//! `tests/integration/story.rs` walks them.
//!
//! Script hooks ([`register_script`]): the probe `story` and the action
//! `type`.

use std::path::PathBuf;

use glam::Vec2;
use serde_json::{Value, json};
use tungsten::App;
use tungsten::core::text::TextOutline;
use tungsten::core::ui::{UiAction, UiBindings, UiScreens, UiTree, screen_build};
use tungsten::core::{
    ActionMap, Binding, CameraController, Config, DataRegistry, Entity, GamepadButton, InputState,
    KeyCode, PrefabData, PrefabRegistry, RoutedInput, Tag, World, WorldLabel, WorldLabels, system,
};
use tungsten::kit::{
    Conversation, Counters, Dialogue, Flags, Interactor, KitPlugins, Phase, Quest, Quests, Speaker,
    Talk, TalkLine, centre_of,
};
use tungsten::screens::{self, ScreensPlugin};
use tungsten::story::{DialogueBoxView, StoryUiPlugin, open_journal};
use tungsten::{GameState, StateContext, StateId, StateStack};

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/03_fixtures/assets/manifest.json";

/// The play state's ID.
pub const PLAY: StateId = "story_play";
/// The name card's `ui_screens` ID.
pub const NAME_SCREEN: &str = "name_card";
/// The key of the card's text field.
pub const NAME_KEY: &str = "name";
/// The action the card's Enter runs: [`commit_name`].
pub const NAME_ACTION: &str = "story.name";
/// The name a blank card gives.
pub const DEFAULT_NAME: &str = "Warden";
/// Talks, lights or reads: E, the pad's West.
pub const INTERACT: &str = "interact";
/// Opens the journal: J, the pad's North.
pub const JOURNAL: &str = "journal";
const MOVE_UP: &str = "move_up";
const MOVE_DOWN: &str = "move_down";
/// The quest the keeper offers.
pub const QUEST: &str = "lamplight";
/// The game flag the lamp's cue sets.
pub const LAMP_LIT: &str = "lamp.lit";
/// The keeper's conversation.
pub const KEEPER_TALK: &str = "keeper.evening";
/// The notice board's.
pub const NOTICE_TALK: &str = "notice.board";
/// The system that draws the warden's name.
const NAME_LABEL: &str = "story_name_label";

/// The name's face, size, offset off the warden's centre in UI units, and
/// its colours, as the kit's barks draw. It hangs under the warden's feet,
/// clear of the prompts and barks over the heads beside it.
const FONT: &str = "engine_mono";
const NAME_SIZE: f32 = 14.0;
const NAME_OFFSET: Vec2 = Vec2::new(0.0, 28.0);
const NAME_COLOR: [u8; 4] = [255, 232, 160, 255];
const NAME_OUTLINE: [u8; 4] = [12, 12, 16, 255];

/// The room's walls, the root manifest's 32 px `blue_square`: its top-left
/// and its size in tiles, inside a view 720 px high from the origin.
const TILE: f32 = 32.0;
const ROOM: Vec2 = Vec2::new(192.0, 96.0);
const ROOM_TILES: (usize, usize) = (28, 17);
/// Each entity's top-left: the engine draws a sprite from it.
pub const WARDEN_AT: Vec2 = Vec2::new(288.0, 384.0);
pub const KEEPER_AT: Vec2 = Vec2::new(672.0, 384.0);
pub const LAMP_AT: Vec2 = Vec2::new(608.0, 160.0);
pub const NOTICE_AT: Vec2 = Vec2::new(296.0, 160.0);
/// The steps by the lamp, the root manifest's `red_square`.
const STEPS: [Vec2; 3] = [
    Vec2::new(704.0, 224.0),
    Vec2::new(736.0, 192.0),
    Vec2::new(768.0, 160.0),
];

/// Runs the `story` scene: the root config and both manifests, the kit, the
/// screens and the box ([`configure`]), the room at startup ([`setup`]).
pub fn run() -> anyhow::Result<()> {
    let mut config = Config::load("tungsten.json")?;
    config.window.title = "Fixtures — story (redo 8.1)".to_string();
    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![
        PathBuf::from(ROOT_MANIFEST),
        PathBuf::from(LOCAL_MANIFEST),
    ]);
    configure(&mut app);
    app.on_startup(|world, _renderer| setup(world));
    app.run()
}

/// The scene's plugins and hooks: the kit, the screens and the story's box
/// and journal, the card's action, the script's probe and action, and the
/// name's label in the kit's present phase. A test builds the same, then
/// loads the manifest and calls [`setup`].
pub fn configure(app: &mut App) {
    app.add_plugins(KitPlugins::set());
    app.add_plugin(ScreensPlugin);
    app.add_plugin(StoryUiPlugin);
    let world = app.world_mut();
    UiBindings::of(world).action(NAME_ACTION, commit_name);
    register_script(world);
    app.add_system_to(
        Phase::Present.stage(),
        Phase::Present.join(system(NAME_LABEL, name_label)),
    );
}

/// The room, once the manifests have loaded: the scene's actions, its data
/// and prefabs from inline JSON, the walls, the steps, the notice board, the
/// lamp, the keeper and the warden, [`StoryScene`], and the play state,
/// which opens the name card.
///
/// # Panics
///
/// When an inline entry does not read as its kind or a prefab does not
/// spawn: the scene's own data is broken.
pub fn setup(world: &mut World) {
    add_bindings(world);
    insert_data(world);
    insert_prefabs(world);
    for column in 0..ROOM_TILES.0 {
        for row in 0..ROOM_TILES.1 {
            let edge =
                column == 0 || row == 0 || column + 1 == ROOM_TILES.0 || row + 1 == ROOM_TILES.1;
            if edge {
                let at = ROOM + Vec2::new(column as f32, row as f32) * TILE;
                spawn(world, "story_wall", at);
            }
        }
    }
    for at in STEPS {
        spawn(world, "story_step", at);
    }
    let scene = StoryScene {
        notice: spawn(world, "story_notice", NOTICE_AT),
        lamp: spawn(world, "story_lamp", LAMP_AT),
        keeper: spawn(world, "story_keeper", KEEPER_AT),
        warden: spawn(world, "story_warden", WARDEN_AT),
        name: String::new(),
    };
    world.insert_resource(scene);
    if let Some(controller) = world.get_resource_mut::<CameraController>() {
        controller.rig.view_height = Some(720.0);
    }
    world
        .resource_mut::<StateStack>()
        .request_push(StoryPlay::default());
}

/// The scene's entities and the warden's name, a World resource [`setup`]
/// inserts.
#[derive(Debug)]
pub struct StoryScene {
    pub warden: Entity,
    pub keeper: Entity,
    pub lamp: Entity,
    pub notice: Entity,
    /// The name the card committed; empty until it does.
    pub name: String,
}

/// The scene's actions, added to the action map once each, as the
/// inventory adds its own: the moves the root `input.json` lacks
/// (`move_up`, `move_down`, and `move_left` and `move_right` on a map
/// without it), [`INTERACT`] and [`JOURNAL`].
pub fn add_bindings(world: &mut World) {
    let Some(map) = world.get_resource_mut::<ActionMap>() else {
        return;
    };
    let key = |code| Binding::Key { code };
    let pad = |button| Binding::GamepadButton { button };
    for (action, wanted) in [
        ("move_left", [key(KeyCode::ArrowLeft), key(KeyCode::KeyA)]),
        ("move_right", [key(KeyCode::ArrowRight), key(KeyCode::KeyD)]),
        (MOVE_UP, [key(KeyCode::ArrowUp), key(KeyCode::KeyW)]),
        (MOVE_DOWN, [key(KeyCode::ArrowDown), key(KeyCode::KeyS)]),
        (INTERACT, [key(KeyCode::KeyE), pad(GamepadButton::West)]),
        (JOURNAL, [key(KeyCode::KeyJ), pad(GamepadButton::North)]),
    ] {
        let mut bindings = map.bindings(action).to_vec();
        for binding in wanted {
            if !bindings.contains(&binding) {
                bindings.push(binding);
            }
        }
        map.replace_bindings(action, bindings);
    }
}

/// The speakers, the two conversations and the quest, in the kit's data
/// kinds.
fn insert_data(world: &mut World) {
    let data = world.resource_mut::<DataRegistry>();
    for (id, value) in [
        (
            "keeper",
            json!({ "name": "Old Tamsin", "portrait": { "neutral": "green_circle", "worried": "red_square" }, "pace": 40 }),
        ),
        (
            "warden",
            json!({ "name": "Warden", "portrait": "walk_0", "pace": 50 }),
        ),
        ("notice", json!({ "name": "Notice board", "pace": 60 })),
    ] {
        let speaker = serde_json::from_value::<Speaker>(value)
            .unwrap_or_else(|e| panic!("the story's speaker `{id}`: {e}"));
        data.insert(id, speaker);
    }
    for (id, value) in [
        (KEEPER_TALK, keeper_evening()),
        (NOTICE_TALK, notice_board()),
    ] {
        let conversation = serde_json::from_value::<Conversation>(value)
            .unwrap_or_else(|e| panic!("the story's conversation `{id}`: {e}"));
        data.insert(id, conversation);
    }
    let quest = serde_json::from_value::<Quest>(lamplight())
        .unwrap_or_else(|e| panic!("the story's quest `{QUEST}`: {e}"));
    data.insert(QUEST, quest);
}

/// The keeper's evening: a greeting and the lamp's offer while the warden
/// holds no `lamplight`, a reminder while it waits on the lamp, the thanks
/// and the reward once the lamp burns, and a goodnight after.
fn keeper_evening() -> Value {
    let holds = |condition: Value| json!({ "quest": condition });
    json!({
        "speaker": "keeper",
        "start": "open",
        "nodes": {
            "open": { "branch": [
                { "when": holds(json!({ "id": QUEST, "is": "done", "target": "other" })), "next": "after" },
                { "when": holds(json!({ "id": QUEST, "stage": "report", "target": "other" })), "next": "lit" },
                { "when": holds(json!({ "id": QUEST, "target": "other" })), "next": "waiting" },
                { "next": "greet" }
            ] },
            "greet": {
                "lines": ["Evening, warden.", { "speaker": "warden", "text": "Evening." }],
                "next": "offer"
            },
            "offer": {
                "lines": [{ "text": "The lamp by the steps is out again.", "mood": "worried" }],
                "choices": [
                    { "text": "I'll light it.", "do": [{ "quest": { "id": QUEST, "target": "other" } }], "next": "thanks" },
                    {
                        "text": "Who keeps the lamps?",
                        "when": { "not": { "flag": { "name": "lamp.asked", "target": "game" } } },
                        "do": [{ "flag": { "name": "lamp.asked", "target": "game" } }],
                        "next": "keepers"
                    },
                    { "text": "Not tonight.", "back": true }
                ]
            },
            "keepers": { "lines": ["I do, when my knees let me."], "next": "offer" },
            "thanks": { "lines": ["Bless you. Mind the steps."] },
            "waiting": { "lines": [{ "text": "Still dark by the steps.", "mood": "worried" }] },
            "lit": {
                "do": [{ "quest": { "id": QUEST, "op": "advance", "target": "other" } }],
                "lines": ["It burns. Thank you, warden."]
            },
            "after": { "lines": ["Sleep well, warden."] }
        }
    })
}

/// The notice board's two lines.
fn notice_board() -> Value {
    json!({
        "speaker": "notice",
        "start": "read",
        "nodes": { "read": { "lines": [
            "By order of the Lamp Guild: every lamp lit by the ninth bell.",
            "Report a dark lamp to its keeper."
        ] } }
    })
}

/// `lamplight`: light the lamp, then tell the keeper, for five coins.
fn lamplight() -> Value {
    json!({
        "title": "Lamplight",
        "start": "light",
        "stages": {
            "light": {
                "journal": "Light the lamp by the steps.",
                "objectives": [{ "text": "Lamp lit", "when": { "flag": { "name": LAMP_LIT, "target": "game" } } }],
                "next": "report"
            },
            "report": {
                "journal": "Tell Tamsin it burns.",
                "rewards": [{ "counter": { "name": "coins", "add": 5, "target": "game" } }]
            }
        }
    })
}

/// The scene's prefabs: the wall and the step, the notice board, the lamp,
/// the keeper and the warden.
fn insert_prefabs(world: &mut World) {
    let prompt =
        |verb: &str, offset: f32| json!({ "verb": verb, "radius": 16, "offset": [0, offset] });
    let mut lamp = prompt("Light", -40.0);
    lamp["when"] = json!({ "not": { "flag": { "name": LAMP_LIT, "target": "game" } } });
    let prefabs = [
        (
            "story_wall",
            json!({ "sprite": { "asset_id": "blue_square" } }),
        ),
        (
            "story_step",
            json!({ "sprite": { "asset_id": "red_square" } }),
        ),
        (
            "story_notice",
            json!({
                "sprite": { "asset_id": "ex05_frame" },
                "tag": "notice",
                "components": {
                    "interactable": prompt("Read", -32.0),
                    "cues": { "interacted": [{ "dialogue": NOTICE_TALK }] }
                }
            }),
        ),
        (
            "story_lamp",
            json!({
                "sprite": { "asset_id": "ex05_item_lantern" },
                "scale": [2, 2],
                "tag": "lamp",
                "components": {
                    "interactable": lamp,
                    "cues": { "interacted": [
                        { "flag": { "name": LAMP_LIT, "target": "game" } },
                        { "say": "It catches." }
                    ] }
                }
            }),
        ),
        (
            "story_keeper",
            json!({
                "sprite": { "asset_id": "green_circle" },
                "tag": "keeper",
                "components": {
                    "interactable": prompt("Talk", -40.0),
                    "cues": { "interacted": [{ "dialogue": KEEPER_TALK }] }
                }
            }),
        ),
        (
            "story_warden",
            json!({
                "sprite": { "asset_id": "walk_0" },
                "tag": "warden",
                "components": {
                    "top_down_mover": { "speed": 160 },
                    "action_input": {
                        "move": ["move_left", "move_right", MOVE_UP, MOVE_DOWN],
                        "slots": { "interact": INTERACT }
                    },
                    "interactor": { "reach": 64 },
                    "quests": {}
                }
            }),
        ),
    ];
    if !world.has_resource::<PrefabRegistry>() {
        world.insert_resource(PrefabRegistry::new());
    }
    let registry = world.resource_mut::<PrefabRegistry>();
    for (id, value) in prefabs {
        let prefab = serde_json::from_value::<PrefabData>(value)
            .unwrap_or_else(|e| panic!("the story's prefab `{id}`: {e}"));
        registry.insert(id.to_owned(), prefab);
    }
}

fn spawn(world: &mut World, id: &str, at: Vec2) -> Entity {
    world
        .spawn_prefab(id, at)
        .unwrap_or_else(|e| panic!("the story's prefab `{id}`: {e}"))
}

/// The scene's play state: holds the game clock while the box, the journal
/// or the card covers it, opens the card at its first update and the
/// journal on [`JOURNAL`].
#[derive(Debug, Default)]
struct StoryPlay {
    asked: bool,
}

impl GameState for StoryPlay {
    fn id(&self) -> StateId {
        PLAY
    }

    fn on_enter(&mut self, _ctx: &mut StateContext) {}

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn holds_clock(&self) -> bool {
        true
    }

    fn update(&mut self, world: &mut World) {
        if !screens::idle(world) {
            return;
        }
        if !self.asked {
            self.asked = true;
            screens::open(world, NAME_SCREEN);
            return;
        }
        let journal = world
            .get_resource::<ActionMap>()
            .zip(world.get_resource::<RoutedInput>())
            .is_some_and(|(map, routed)| map.just_pressed(routed, JOURNAL));
        if journal {
            let warden = world.get_resource::<StoryScene>().map(|scene| scene.warden);
            open_journal(world, warden);
        }
    }
}

/// [`NAME_ACTION`], the card's Enter: the field's trimmed value, else
/// [`DEFAULT_NAME`], becomes the warden's name, and the card closes.
fn commit_name(world: &mut World, _call: UiAction) {
    let root = world.get_resource::<UiScreens>().and_then(|built| {
        built
            .roots()
            .filter(|(_, id)| *id == NAME_SCREEN)
            .map(|(root, _)| root)
            .last()
    });
    let typed = root
        .and_then(|root| screen_build::node(world, root, NAME_KEY))
        .and_then(|field| world.resource::<UiTree>().field_value(field))
        .map(|value| value.trim().to_owned())
        .unwrap_or_default();
    let name = if typed.is_empty() {
        DEFAULT_NAME.to_owned()
    } else {
        typed
    };
    if let Some(scene) = world.get_resource_mut::<StoryScene>() {
        scene.name = name;
    }
    screens::pop(world);
}

/// Draws the warden's name under its feet once the card has named it,
/// outlined as a bark is.
fn name_label(world: &mut World) {
    let Some(scene) = world.get_resource::<StoryScene>() else {
        return;
    };
    if scene.name.is_empty() || !world.has_resource::<WorldLabels>() {
        return;
    }
    let Some(anchor) = centre_of(world, scene.warden) else {
        return;
    };
    let label = WorldLabel::new(scene.name.clone(), anchor, FONT.to_owned(), NAME_SIZE)
        .with_offset(NAME_OFFSET)
        .with_pivot(Vec2::new(0.5, 0.0))
        .with_color(NAME_COLOR)
        .with_outline(TextOutline::new(NAME_OUTLINE, 1.0));
    world.resource_mut::<WorldLabels>().push(label);
}

/// Adds the scene's script hooks: the probe `story` and the action `type`.
pub fn register_script(world: &mut World) {
    world.add_script_probe(
        "story",
        "The story scene: `{conversation, node, speaker, line, revealing, choices, quest: {status, stage}, focus, name, lamp_lit, coins, screen}`, null where none.",
        probe,
    );
    world.add_script_action(
        "type",
        "Types `{\"text\": \"…\"}` as the window's characters would: a focused text field takes them.",
        type_text,
    );
}

/// The probe `story`: the talk's conversation, node, speaker and line, the
/// characters of it the box shows while it reveals, the shown choices,
/// [`QUEST`]'s status and stage on the warden, the tag of the warden's
/// focus, its name, the lamp's flag, the game's coins and the top state.
fn probe(world: &World, _args: &Value) -> Result<Value, String> {
    let scene = world
        .get_resource::<StoryScene>()
        .ok_or("the story scene is not set up")?;
    let talk = world.get_resource::<Dialogue>().and_then(Dialogue::current);
    let line = talk.and_then(Talk::line);
    let choices: Vec<&str> = talk
        .map(|talk| talk.choices().map(|(_, text)| text).collect())
        .unwrap_or_default();
    let quest = world
        .get::<Quests>(scene.warden)
        .and_then(|quests| quests.get(QUEST))
        .map(|state| json!({ "status": state.status().name(), "stage": state.stage() }));
    let focus = world
        .get::<Interactor>(scene.warden)
        .and_then(Interactor::focus)
        .and_then(|entity| world.get::<Tag>(entity))
        .map(|tag| tag.name.as_str());
    Ok(json!({
        "conversation": talk.map(Talk::conversation),
        "node": talk.map(Talk::node),
        "speaker": line.map(TalkLine::name),
        "line": line.map(TalkLine::text),
        "revealing": world.get_resource::<DialogueBoxView>().and_then(DialogueBoxView::revealing),
        "choices": choices,
        "quest": quest,
        "focus": focus,
        "name": scene.name,
        "lamp_lit": world.get_resource::<Flags>().is_some_and(|flags| flags.has(LAMP_LIT)),
        "coins": world.get_resource::<Counters>().map_or(0, |counters| counters.get("coins")),
        "screen": world.get_resource::<StateStack>().and_then(StateStack::active_id),
    }))
}

/// The action `type`: records its `text` as typed characters, as the
/// window's bridge records a key's text.
fn type_text(world: &mut World, args: &Value) -> Result<(), String> {
    let text = args
        .get("text")
        .and_then(Value::as_str)
        .ok_or("`type` takes `{\"text\": \"…\"}`")?;
    world
        .get_resource_mut::<InputState>()
        .ok_or("no InputState")?
        .type_text(text);
    Ok(())
}
