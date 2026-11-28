//! The `items` scene (redo 8.1's second plan): the kit's items and the item
//! screens in one room, there for their feel. The warden walks (the arrows
//! or WASD) over a stack of flasks, a magic charm and a rare wand, each
//! labelled in its rarity's text style, which the scene adds to the
//! theme; while the chandler's prompt shows, [`INTERACT`] (E) opens her
//! shop, and [`INVENTORY`] (I) opens the warden's bag. The play state holds
//! the game clock under both screens.
//!
//! As `story` does, the bases, affixes, the shop and the prefabs are inline
//! JSON, which [`setup`] puts in the World's registries, and the shared
//! manifest lists the scene's screen files alone (`inventory`, `shop`,
//! `item_tooltip` and `cell_menu`); entries inserted in code skip the
//! load's world checks, so `tests/integration/items.rs` walks them.
//!
//! Script hooks ([`register_script`]): the probe `items` and the actions
//! `drag` and `hover`.

use std::path::PathBuf;

use glam::Vec2;
use serde_json::{Value, json};
use tungsten::App;
use tungsten::core::text::TextOutline;
use tungsten::core::ui::{NamedStyle, TextOverrides, UiKey, UiTree};
use tungsten::core::{
    ActionMap, Binding, CameraController, Config, DataRegistry, Entity, GamepadButton, InputState,
    KeyCode, MouseButton, PrefabData, PrefabRegistry, RoutedInput, ScaleFactor, Stage, World,
    system,
};
use tungsten::items::{ItemsUiPlugin, open_inventory};
use tungsten::kit::{
    Affix, Counters, Inventory, ItemBase, KitPlugins, Shop, ShopVisit, Tags, visit,
};
use tungsten::plugins::UI_ROUTE;
use tungsten::screens::{self, ScreensPlugin, popup};
use tungsten::{GameState, StateContext, StateId, StateStack};

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/03_fixtures/assets/manifest.json";

/// The scene's play state.
pub const PLAY: StateId = "items_play";
/// The action that talks to the chandler, E.
pub const INTERACT: &str = "interact";
/// The action that opens the inventory, I.
pub const INVENTORY: &str = "inventory";
/// The chandler's shop.
pub const SHOP: &str = "chandlery";
/// The script action that drags between two keyed widgets.
pub const DRAG: &str = "drag";
/// The script action that moves the pointer onto a keyed widget.
pub const HOVER: &str = "hover";
const MOVE_UP: &str = "move_up";
const MOVE_DOWN: &str = "move_down";
/// The system that drives a scripted drag, before the UI's routing.
const DRAG_DRIVE: &str = "items_drag";
/// Frames a scripted drag takes by default, the press's and the release's
/// included.
const DRAG_FRAMES: u32 = 6;

/// The room: a wall of tiles at its edge.
const TILE: f32 = 32.0;
const ROOM: Vec2 = Vec2::new(192.0, 96.0);
const ROOM_TILES: (usize, usize) = (28, 17);
/// Where the warden starts, and the chandler and the ground items lie.
pub const WARDEN_AT: Vec2 = Vec2::new(288.0, 352.0);
pub const CHANDLER_AT: Vec2 = Vec2::new(736.0, 352.0);
pub const FLASKS_AT: Vec2 = Vec2::new(448.0, 368.0);
pub const CHARM_AT: Vec2 = Vec2::new(480.0, 192.0);
pub const WAND_AT: Vec2 = Vec2::new(928.0, 368.0);
/// The game's coins and day at the start.
pub const COINS: i64 = 50;
const DAY: i64 = 4;

/// The rarities' label styles the scene adds to the theme: a name and its
/// colour.
const RARITIES: [(&str, [u8; 4]); 3] = [
    ("rarity.normal", [232, 228, 216, 255]),
    ("rarity.magic", [120, 160, 255, 255]),
    ("rarity.rare", [255, 220, 96, 255]),
];

/// Runs the `items` scene: the root config and both manifests, the kit, the
/// screens and the item screens ([`configure`]), the room at startup
/// ([`setup`]).
pub fn run() -> anyhow::Result<()> {
    let mut config = Config::load("tungsten.json")?;
    config.window.title = "Fixtures — items (redo 8.1)".to_string();
    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![
        PathBuf::from(ROOT_MANIFEST),
        PathBuf::from(LOCAL_MANIFEST),
    ]);
    configure(&mut app);
    app.on_startup(|world, _renderer| setup(world));
    app.run()
}

/// The scene's plugins and hooks: the kit, the screens and the item
/// screens, the script's probe and action, and the scripted drag's driver
/// before the UI's routing. A test builds the same, then loads the manifest
/// and calls [`setup`].
pub fn configure(app: &mut App) {
    app.add_plugins(KitPlugins::set());
    app.add_plugin(ScreensPlugin);
    app.add_plugin(ItemsUiPlugin);
    register_script(app.world_mut());
    app.add_system_to(
        Stage::PreUpdate,
        system(DRAG_DRIVE, drive_drag).before(UI_ROUTE),
    );
}

/// The room, once the manifests have loaded: the scene's actions, its data
/// and prefabs from inline JSON, the rarities' styles, the walls, the
/// chandler, the warden and the ground items, the game's coins and day,
/// [`ItemsScene`], and the play state.
///
/// # Panics
///
/// When an inline entry does not read as its kind or a prefab does not
/// spawn: the scene's own data is broken.
pub fn setup(world: &mut World) {
    add_bindings(world);
    insert_data(world);
    insert_prefabs(world);
    add_rarity_styles(world);
    for column in 0..ROOM_TILES.0 {
        for row in 0..ROOM_TILES.1 {
            let edge =
                column == 0 || row == 0 || column + 1 == ROOM_TILES.0 || row + 1 == ROOM_TILES.1;
            if edge {
                let at = ROOM + Vec2::new(column as f32, row as f32) * TILE;
                spawn(world, "items_wall", at);
            }
        }
    }
    let scene = ItemsScene {
        chandler: spawn(world, "items_chandler", CHANDLER_AT),
        warden: spawn(world, "items_warden", WARDEN_AT),
        flasks: spawn(world, "items_flasks", FLASKS_AT),
        charm: spawn(world, "items_charm", CHARM_AT),
        wand: spawn(world, "items_wand", WAND_AT),
    };
    world.insert_resource(scene);
    if !world.has_resource::<Counters>() {
        world.insert_resource(Counters::new());
    }
    let counters = world.resource_mut::<Counters>();
    counters.set("coins", COINS);
    counters.set("day", DAY);
    if let Some(controller) = world.get_resource_mut::<CameraController>() {
        controller.rig.view_height = Some(720.0);
    }
    world.resource_mut::<StateStack>().request_push(ItemsPlay);
}

/// The scene's entities, a World resource [`setup`] inserts.
#[derive(Debug, Clone, Copy)]
pub struct ItemsScene {
    pub warden: Entity,
    pub chandler: Entity,
    pub flasks: Entity,
    pub charm: Entity,
    pub wand: Entity,
}

/// The scene's actions, added to the action map once each: the moves the
/// root `input.json` lacks, [`INTERACT`] and [`INVENTORY`].
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
        (INVENTORY, [key(KeyCode::KeyI), pad(GamepadButton::North)]),
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

/// The scene's item bases: a flask, the chandler's hood, a charm and a
/// wand, their icons the fixtures' item sprites.
fn bases() -> [(&'static str, Value); 4] {
    [
        (
            "oil_flask",
            json!({
                "name": "Oil Flask", "icon": "ex05_item_draught", "tags": ["item.flask"],
                "stack": 5, "price": 3, "ground": "items_flasks",
                "use": [{ "counter": { "name": "oil", "add": 10 } }]
            }),
        ),
        (
            "oilskin_hood",
            json!({
                "name": "Oilskin Hood", "icon": "ex05_item_cap",
                "text": "Keeps the rain off a lantern's flame.", "tags": ["item.hood"],
                "slot": "hood", "price": 12, "grant": ["status.hooded"],
                "implicit": [{ "resist": "damage.blade", "modifier": { "add": -0.1 } }]
            }),
        ),
        (
            "brass_charm",
            json!({
                "name": "Brass Charm", "icon": "ex05_item_moonstone", "tags": ["item.charm"],
                "slot": "charm", "price": 20,
                "implicit": [{ "stat": "pickup_radius", "modifier": { "add": 8 } }]
            }),
        ),
        (
            "ashwood_wand",
            json!({
                "name": "Ashwood Wand", "icon": "ex05_item_bow",
                "tags": ["item.weapon", "item.wand"], "slot": "main_hand", "price": 40,
                "weapon": { "damage": { "elemental.fire": [3, 7] }, "use_time": 0.8, "crit_chance": 0.07 }
            }),
        ),
    ]
}

/// The scene's affixes: the charm's prefix and the wand's two.
fn affixes() -> [(&'static str, Value); 3] {
    [
        (
            "fine",
            json!({
                "kind": "prefix", "group": "fine",
                "weights": [{ "tag": "item.charm", "weight": 1000 }],
                "tiers": [{ "level": 1, "weight": 1000, "values": [[5, 10]] }],
                "lines": [{ "stat": "move_speed", "modifier": { "pct": { "value": 0, "times": 0.01 } } }]
            }),
        ),
        (
            "searing",
            json!({
                "kind": "prefix", "group": "fire_damage_pct",
                "weights": [{ "tag": "item.wand", "weight": 1000 }],
                "tiers": [{ "level": 1, "weight": 1000, "values": [[10, 16]] }],
                "lines": [{ "stat": "damage", "modifier": { "pct": { "value": 0, "times": 0.01 } },
                            "with": { "any": ["elemental.fire"] } }]
            }),
        ),
        (
            "quickened",
            json!({
                "kind": "suffix", "group": "cast_speed_pct",
                "weights": [{ "tag": "item.wand", "weight": 800 }],
                "tiers": [{ "level": 1, "weight": 1000, "values": [[5, 9]] }],
                "lines": [{ "stat": "use_speed", "modifier": { "pct": { "value": 0, "times": 0.01 } } }]
            }),
        ),
    ]
}

/// The chandler's shop: flasks without end and two hoods from day 4, the
/// day the game's counter holds.
fn chandlery() -> Value {
    json!({
        "name": "Tamsin's Chandlery",
        "stock": [
            { "item": "oil_flask" },
            { "item": "oilskin_hood", "count": 2,
              "when": { "counter": { "name": "day", "min": 4, "target": "game" } } }
        ],
        "buys": { "any": ["item.flask", "item.hood"] }
    })
}

/// The bases, the affixes and the shop, in the kit's data kinds.
fn insert_data(world: &mut World) {
    let data = world.resource_mut::<DataRegistry>();
    for (id, value) in bases() {
        let base = serde_json::from_value::<ItemBase>(value)
            .unwrap_or_else(|e| panic!("the items' base `{id}`: {e}"));
        data.insert(id, base);
    }
    for (id, value) in affixes() {
        let affix = serde_json::from_value::<Affix>(value)
            .unwrap_or_else(|e| panic!("the items' affix `{id}`: {e}"));
        data.insert(id, affix);
    }
    let shop = serde_json::from_value::<Shop>(chandlery())
        .unwrap_or_else(|e| panic!("the items' shop `{SHOP}`: {e}"));
    data.insert(SHOP, shop);
}

/// The scene's prefabs: the wall, the chandler, the warden and the three
/// ground items, each item sprite's pickup centred on it.
fn insert_prefabs(world: &mut World) {
    let ground = |sprite: &str, item: Value| {
        json!({
            "sprite": { "asset_id": sprite },
            "components": {
                "pickup": { "shape": { "circle": 10 }, "offset": [16, 16] },
                "ground_item": { "item": item, "offset": [16, -6] }
            }
        })
    };
    let mut flasks = ground("ex05_item_draught", json!("oil_flask"));
    flasks["components"]["ground_item"]["count"] = json!(3);
    let charm = json!({ "base": "brass_charm", "rarity": "magic",
        "affixes": [{ "id": "fine", "tier": 1, "values": [8] }] });
    let wand = json!({ "base": "ashwood_wand", "rarity": "rare",
        "affixes": [{ "id": "searing", "tier": 1, "values": [12] },
                    { "id": "quickened", "tier": 1, "values": [7] }] });
    let prefabs = [
        (
            "items_wall",
            json!({ "sprite": { "asset_id": "blue_square" } }),
        ),
        (
            "items_chandler",
            json!({
                "sprite": { "asset_id": "green_circle" },
                "tag": "chandler",
                "components": {
                    "interactable": { "verb": "Trade", "radius": 16, "offset": [32, -8] },
                    "cues": { "interacted": [{ "shop": SHOP }] }
                }
            }),
        ),
        (
            "items_warden",
            json!({
                "sprite": { "asset_id": "walk_0" },
                "tag": "warden",
                "components": {
                    "tags": [],
                    "top_down_mover": { "speed": 160 },
                    "action_input": {
                        "move": ["move_left", "move_right", MOVE_UP, MOVE_DOWN],
                        "slots": { "interact": INTERACT }
                    },
                    "interactor": { "reach": 64 },
                    "hurtbox": { "shape": { "circle": 16 }, "offset": [32, 32] },
                    "inventory": {
                        "cells": 24,
                        "slots": { "lantern": "hood", "coat": "coat", "boots": "boots",
                                   "charm_1": "charm", "charm_2": "charm" }
                    }
                }
            }),
        ),
        ("items_flasks", flasks),
        ("items_charm", ground("ex05_item_moonstone", charm)),
        ("items_wand", ground("ex05_item_bow", wand)),
    ];
    if !world.has_resource::<PrefabRegistry>() {
        world.insert_resource(PrefabRegistry::new());
    }
    let registry = world.resource_mut::<PrefabRegistry>();
    for (id, value) in prefabs {
        let prefab = serde_json::from_value::<PrefabData>(value)
            .unwrap_or_else(|e| panic!("the items' prefab `{id}`: {e}"));
        registry.insert(id.to_owned(), prefab);
    }
}

fn spawn(world: &mut World, id: &str, at: Vec2) -> Entity {
    world
        .spawn_prefab(id, at)
        .unwrap_or_else(|e| panic!("the items' prefab `{id}`: {e}"))
}

/// Adds the rarities' label styles to the tree's theme when it lacks them:
/// each a colour over the theme's base text with a dark outline.
fn add_rarity_styles(world: &mut World) {
    let Some(tree) = world.get_resource_mut::<UiTree>() else {
        return;
    };
    if RARITIES
        .iter()
        .all(|(name, _)| tree.theme().text_style(name).is_some())
    {
        return;
    }
    let mut theme = tree.theme().clone();
    for (name, color) in RARITIES {
        let style = NamedStyle {
            text: TextOverrides {
                color: Some(color),
                ..TextOverrides::default()
            },
            outline: Some(TextOutline::new([12, 12, 16, 255], 1.0)),
            ..NamedStyle::default()
        };
        theme.names.text_styles.insert(name.to_owned(), style);
    }
    tree.set_theme(theme);
}

/// The scene's play state: holds the game clock while a screen covers it,
/// keeps the rarities' styles, and opens the inventory on [`INVENTORY`].
#[derive(Debug, Default)]
struct ItemsPlay;

impl GameState for ItemsPlay {
    fn id(&self) -> StateId {
        PLAY
    }

    fn on_enter(&mut self, _ctx: &mut StateContext) {}

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn holds_clock(&self) -> bool {
        true
    }

    fn update(&mut self, world: &mut World) {
        add_rarity_styles(world);
        if !screens::idle(world) {
            return;
        }
        let pressed = world
            .get_resource::<ActionMap>()
            .zip(world.get_resource::<RoutedInput>())
            .is_some_and(|(map, routed)| map.just_pressed(routed, INVENTORY));
        if pressed && let Some(scene) = world.get_resource::<ItemsScene>().copied() {
            open_inventory(world, scene.warden);
        }
    }
}

/// Adds the scene's script hooks: the probe `items` and the actions `drag`
/// and `hover`.
pub fn register_script(world: &mut World) {
    world.add_script_probe(
        "items",
        "The items scene: `{cells: [[base, count] | null], worn: {slot: base | null}, coins, tags, shop, screen, tooltip}`.",
        probe,
    );
    world.add_script_action(
        DRAG,
        "Drags `{\"from\": key, \"to\": key, \"frames\": n}` between two keyed widgets over `frames` frames, 6 by default and at least 2: pressed at the first, released at the last.",
        drag,
    );
    world.add_script_action(
        HOVER,
        "Moves the pointer onto `{\"key\": key}`'s widget, pressing nothing.",
        hover,
    );
}

/// The probe `items`: the warden's cells as base and count, its worn
/// slots, the game's coins, the warden's tags, the visit's shop, the top
/// state and whether a tooltip shows.
fn probe(world: &World, _args: &Value) -> Result<Value, String> {
    let scene = world
        .get_resource::<ItemsScene>()
        .ok_or("the items scene is not set up")?;
    let bag = world
        .get::<Inventory>(scene.warden)
        .ok_or("the warden has no bag")?;
    let cells: Vec<Value> = bag
        .cells()
        .map(|item| item.map_or(Value::Null, |item| json!([item.base(), item.count()])))
        .collect();
    let worn: serde_json::Map<String, Value> = bag
        .slots()
        .map(|(name, slot)| {
            let base = slot.item().map_or(Value::Null, |item| json!(item.base()));
            (name.to_owned(), base)
        })
        .collect();
    let tags: Vec<&str> = world
        .get::<Tags>(scene.warden)
        .map(|tags| tags.iter().collect())
        .unwrap_or_default();
    Ok(json!({
        "cells": cells,
        "worn": worn,
        "coins": world.get_resource::<Counters>().map_or(0, |counters| counters.get("coins")),
        "tags": tags,
        "shop": visit(world).map(ShopVisit::shop),
        "screen": world.get_resource::<StateStack>().and_then(StateStack::active_id),
        "tooltip": popup::shown_tooltip(world).is_some(),
    }))
}

/// A scripted drag under way: the pointer's path in window pixels, its
/// frames and the frame it is at.
#[derive(Debug, Clone, Copy)]
struct Dragging {
    from: Vec2,
    to: Vec2,
    frames: u32,
    frame: u32,
}

/// The window-pixel centre of the widget keyed `key` in the shown roots,
/// the front-most first.
fn centre_of_key(world: &World, key: &str) -> Result<Vec2, String> {
    let tree = world.get_resource::<UiTree>().ok_or("no UiTree")?;
    let key = UiKey::new(key);
    let widget = tree
        .roots()
        .iter()
        .rev()
        .filter(|root| {
            tree.visibility(**root)
                .is_some_and(tungsten::core::ui::Visibility::is_visible)
        })
        .find_map(|root| tree.find_key(*root, key))
        .ok_or("no shown widget has that key")?;
    let rect = tree.visual_rect(widget).ok_or("the widget has no box")?;
    let scale = world
        .get_resource::<ScaleFactor>()
        .map_or(1.0, ScaleFactor::get);
    Ok((rect.min + rect.max) * 0.5 * scale)
}

/// The action `drag`: starts a drag from `from`'s widget to `to`'s, which
/// [`drive_drag`] runs over `frames` frames, [`DRAG_FRAMES`] by default.
fn drag(world: &mut World, args: &Value) -> Result<(), String> {
    let key = |name: &str| {
        args.get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("`drag` takes `{{\"from\": key, \"to\": key}}`; no `{name}`"))
    };
    let frames = match args.get("frames") {
        None => DRAG_FRAMES,
        Some(frames) => frames
            .as_u64()
            .and_then(|frames| u32::try_from(frames).ok())
            .filter(|frames| *frames >= 2)
            .ok_or("`drag`'s `frames` is a whole number of at least 2")?,
    };
    let from = centre_of_key(world, key("from")?).map_err(|e| format!("from: {e}"))?;
    let to = centre_of_key(world, key("to")?).map_err(|e| format!("to: {e}"))?;
    world.insert_resource(Dragging {
        from,
        to,
        frames,
        frame: 0,
    });
    Ok(())
}

/// The action `hover`: the pointer onto `key`'s widget at once.
fn hover(world: &mut World, args: &Value) -> Result<(), String> {
    let key = args
        .get("key")
        .and_then(Value::as_str)
        .ok_or("`hover` takes `{\"key\": key}`")?;
    let at = centre_of_key(world, key)?;
    world
        .get_resource_mut::<InputState>()
        .ok_or("no InputState")?
        .update_cursor_position(at.x, at.y);
    Ok(())
}

/// Moves a scripted drag's pointer one frame on, before the UI's routing:
/// pressed at the start, along the path, released at the end.
fn drive_drag(world: &mut World) {
    let Some(mut dragging) = world.get_resource::<Dragging>().copied() else {
        return;
    };
    let last = dragging.frames - 1;
    let t = dragging.frame as f32 / last as f32;
    let at = dragging.from.lerp(dragging.to, t);
    let Some(input) = world.get_resource_mut::<InputState>() else {
        return;
    };
    input.update_cursor_position(at.x, at.y);
    if dragging.frame == 0 {
        input.mouse_down(MouseButton::Left);
    }
    if dragging.frame == last {
        input.mouse_up(MouseButton::Left);
        world.remove_resource::<Dragging>();
        return;
    }
    dragging.frame += 1;
    world.insert_resource(dragging);
}
