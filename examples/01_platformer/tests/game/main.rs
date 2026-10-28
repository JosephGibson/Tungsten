//! The game on the engine's headless harness, laid out after each frame: no
//! window or renderer, the frames stepped one call at a time. `seam` holds
//! the seam the shell calls, `level` the map and its routes, `player` the
//! courier's moves and death, `camera` the view, `hazards` the spikes and
//! fireballs, `fixed_step` the moves across frame rates, `presentation`
//! what draws, `regime` the step's events, `areas` the banners and
//! autosaves; `ball_pit`, `spawning`, `burning`, `brick`, `ice` and
//! `spells` the toys, their element rules and the spells that cast them;
//! `audio` the listener's falloff;
//! `combat` the damage model, `data` the constants that restate the data,
//! `cascades` the endings and their chains, `lightning` the strike and its
//! conduction, `gun` the gun and its shots, `dummy` the training dummy,
//! `enemies` the bestiary and `props` the crates and kegs; `hud` the bars;
//! `tomes` the books, their panel, the take and the strip. The shell's own tests run in the
//! template, on the same bytes, which `just repo-check` holds.

mod areas;
mod audio;
mod ball_pit;
mod brick;
mod burning;
mod camera;
mod cascades;
mod combat;
mod data;
mod dummy;
mod enemies;
mod fixed_step;
mod frame_time;
mod gun;
mod hazards;
mod hud;
mod ice;
mod level;
mod lightning;
mod player;
mod playthrough;
mod presentation;
mod props;
mod regime;
mod saves;
mod seam;
mod spawning;
mod spells;
mod tomes;

use std::path::PathBuf;

use example_01_platformer::features::enemies::components::{Enemy, EnemyKind};
use example_01_platformer::features::level::geyser::Geyser;
use example_01_platformer::features::player as courier;
use example_01_platformer::features::props::components::Stone;
use example_01_platformer::features::run::{DEATH_SECS, Run};
use example_01_platformer::features::spells as game_spells;
use example_01_platformer::features::spells::components::Spell;
use example_01_platformer::features::toys;
use example_01_platformer::game;
use example_01_platformer::shared::{Body, Burst, despawn_tree};
use example_01_platformer::shell::{self as game_shell, Difficulty, GameInfo, RunStart};
use glam::Vec2;
use tungsten::core::assets::UvRect;
use tungsten::core::ui::{Visibility, WidgetId};
use tungsten::core::{
    AssetRegistry, Bundle, CameraState, Collider, CollisionEvent, Config, Entity, EventQueue,
    FilterMode, ParticleConfigRegistry, ParticleEmitter, Position, PrevPosition, RigidBodyBundle,
    TextureHandle, Time, UserFolder, Velocity,
};
use tungsten::kit::CameraTarget;
use tungsten::testing::UiHarness;
use tungsten::{App, SceneEntity, StateId};

/// The game's config with its user folder set: `Off` unless a test checks
/// what persists, so tests never touch the developer's own files.
pub fn config(user_folder: UserFolder) -> Config {
    let mut config = Config::load("tungsten.json").expect("tungsten.json loads");
    config.game.user_folder = user_folder;
    config
}

/// What `main.rs` hands the shell: the window's title from `tungsten.json`.
pub fn info() -> GameInfo {
    GameInfo {
        title: config(UserFolder::Off).window.title,
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

/// The app as `main.rs` builds it, with no window.
pub fn app(user_folder: UserFolder) -> App {
    let mut app = App::new(config(user_folder)).expect("App::new failed");
    app.set_exit_on_escape(false);
    game::register(&mut app, info());
    app
}

/// The game with its manifest's headless sections loaded and one frame
/// stepped: the title on top.
pub fn harness() -> UiHarness {
    harness_with(UserFolder::Off)
}

pub fn harness_with(user_folder: UserFolder) -> UiHarness {
    let mut harness = UiHarness::new(app(user_folder));
    harness
        .harness_mut()
        .load_manifest(game::MANIFEST)
        .expect("the manifest loads");
    harness.step(1);
    harness
}

/// A fresh user folder for one persistence test.
pub fn user_folder(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("tungsten-platformer-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The state on top.
pub fn top(harness: &UiHarness) -> Option<StateId> {
    game_shell::top(harness.world())
}

/// Steps until no screen change runs: a push the dispatcher has not
/// applied, a closing screen, the curtain or an engine transition.
pub fn settle(harness: &mut UiHarness) {
    let mut frames = 0;
    while game_shell::transitioning(harness.world()) {
        if harness.step(1) == 0 {
            return;
        }
        frames += 1;
        assert!(frames < 200, "the transition never ended");
    }
}

/// Steps a dead player's fall, [`DEATH_SECS`] of game time and two frames
/// for gameplay's update to see it, then waits out game over's arrival.
pub fn fall_dead(harness: &mut UiHarness) {
    harness.step((DEATH_SECS * 60.0).ceil() as u32 + 2);
    settle(harness);
}

/// Presses `action` for a frame, releases it the next, then waits out the
/// transition it started.
pub fn tap(harness: &mut UiHarness, action: &str) {
    harness.harness_mut().press_action(action);
    harness.step(1);
    harness.harness_mut().release_action(action);
    harness.step(1);
    settle(harness);
}

/// The focusable control named `name` on the front-most shown screen.
pub fn find(harness: &UiHarness, name: &str) -> Option<WidgetId> {
    let tree = harness.tree();
    tree.roots().iter().rev().find_map(|root| {
        if tree.visibility(*root) != Some(Visibility::Visible) {
            return None;
        }
        let mut stack = vec![*root];
        while let Some(id) = stack.pop() {
            if tree.accessible_name(id) == Some(name) && tree.can_focus(id) {
                return Some(id);
            }
            stack.extend(tree.children(id).iter().rev().copied());
        }
        None
    })
}

/// Focuses the control named `name` and activates it with `ui_accept`.
pub fn press(harness: &mut UiHarness, name: &str) {
    let id = find(harness, name).unwrap_or_else(|| panic!("no enabled control '{name}'"));
    assert!(harness.focus(id), "'{name}' takes focus");
    tap(harness, "ui_accept");
}

/// Starts a run at `difficulty` as the title's New Game does, and steps
/// into it through the curtain.
pub fn start_at(harness: &mut UiHarness, difficulty: Difficulty) {
    let start = RunStart {
        difficulty,
        save: None,
    };
    game_shell::start_run(harness.world_mut(), start, None);
    settle(harness);
    assert_eq!(top(harness), Some(game::GAMEPLAY));
}

pub fn start(harness: &mut UiHarness) {
    start_at(harness, Difficulty::Normal);
}

/// The run's player.
pub fn player_entity(harness: &UiHarness) -> Entity {
    harness
        .world()
        .get_resource::<Run>()
        .expect("a run is live")
        .player
}

/// Takes the map's enemies, geysers and stones off, with what hangs from
/// them: a test that walks the level meets its terrain alone.
pub fn calm(harness: &mut UiHarness) {
    let world = harness.world_mut();
    let calmed: Vec<Entity> = world
        .query::<(Entity, &Enemy)>()
        .map(|(entity, _)| entity)
        .chain(world.query::<(Entity, &Geyser)>().map(|(entity, _)| entity))
        .chain(world.query::<(Entity, &Stone)>().map(|(entity, _)| entity))
        .collect();
    for entity in calmed {
        despawn_tree(world, entity);
    }
}

/// A New Game at Normal, stepped into, and its player.
pub fn run() -> (UiHarness, Entity) {
    let mut harness = harness();
    start(&mut harness);
    let player = player_entity(&harness);
    (harness, player)
}

/// Puts `entity`'s body centre at `centre`, its history too, so it is not
/// drawn sliding there, and stops it.
pub fn teleport(harness: &mut UiHarness, entity: Entity, centre: Vec2) {
    let world = harness.world_mut();
    let at = centre - body_offset();
    world.get_mut::<Position>(entity).expect("a body").0 = at;
    world.get_mut::<PrevPosition>(entity).expect("a body").0 = at;
    if let Some(velocity) = world.get_mut::<Velocity>(entity) {
        velocity.0 = Vec2::ZERO;
    }
}

/// The player's body centre.
pub fn centre(harness: &UiHarness, entity: Entity) -> Vec2 {
    courier::centre(harness.world(), entity).expect("the player has a body")
}

/// `actor`'s body as its data file holds it, which the game loads
/// ([`Body`]): `player`, `dummy` or an enemy kind's prefab's.
pub fn actor_body(actor: &str) -> Body {
    let path = format!("assets/data/bodies/{actor}.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// `kind`'s body ([`actor_body`]).
pub fn kind_body(kind: EnemyKind) -> Body {
    actor_body(kind.info().prefab)
}

/// The player's body's half size, and its centre from its sprite's corner.
pub fn player_half() -> Vec2 {
    actor_body("player").half
}

pub fn body_offset() -> Vec2 {
    actor_body("player").centre
}

/// The fixed steps the last frame ran.
pub fn steps_this_frame(harness: &UiHarness) -> u32 {
    harness.world().resource::<Time>().fixed_steps_this_frame()
}

/// A small ball or a bowling ball with its centre at `at`, at once.
pub fn ball(harness: &mut UiHarness, at: Vec2, small: bool) -> Entity {
    toys::spawn_ball(harness.world_mut(), at, small)
}

/// A static box about `centre`, owned by gameplay: a test's floor or wall
/// in the level's empty sky.
pub fn floor(harness: &mut UiHarness, centre: Vec2, half: Vec2) -> Entity {
    harness.world_mut().spawn_with(
        RigidBodyBundle::r#static(Position(centre), Collider::aabb(half)).with((SceneEntity {
            state_id: game::GAMEPLAY,
        },)),
    )
}

/// Sends a collision event between `a` and `b`, as the step's solver
/// would: `normal` the way `a` was pushed.
pub fn contact(harness: &mut UiHarness, a: Entity, b: Option<Entity>, normal: Vec2) {
    let world = harness.world_mut();
    let queue = world.resource_mut::<EventQueue<CollisionEvent>>();
    queue.send(CollisionEvent::new(a, b, normal, 0.1));
}

/// Runs the cascades waiting now, as the next step's overlap phase would:
/// a test's direct blast or crush ends its frozen things at once.
pub fn cascades(harness: &mut UiHarness) {
    example_01_platformer::features::elements::cascade::run(harness.world_mut());
}

/// The bursts of the particle config `name`.
pub fn bursts(harness: &UiHarness, name: &str) -> usize {
    let world = harness.world();
    let config = world.resource::<ParticleConfigRegistry>().id_for_name(name);
    world
        .query::<(&Burst, &ParticleEmitter)>()
        .filter(|(_, emitter)| Some(emitter.config) == config)
        .count()
}

/// A body's centre.
pub fn body(harness: &UiHarness, entity: Entity) -> Vec2 {
    let world = harness.world();
    let at = world.get::<Position>(entity).expect("a body").0;
    at + world
        .get::<Collider>(entity)
        .map_or(Vec2::ZERO, |c| c.offset)
}

/// Sets a body's velocity.
pub fn kick(harness: &mut UiHarness, entity: Entity, velocity: Vec2) {
    harness
        .world_mut()
        .get_mut::<Velocity>(entity)
        .expect("a body")
        .0 = velocity;
}

/// Registers `name` as a 64 × 64 sprite on atlas `handle`, drawn lit or
/// not, since the harness uploads no sprite.
pub fn mock_sprite(assets: &mut AssetRegistry, name: &str, handle: u32, lit: bool) {
    assets.register_sprite(
        name.into(),
        FilterMode::Nearest,
        64,
        64,
        PathBuf::new(),
        TextureHandle(handle),
        UvRect::FULL,
        None,
        None,
        lit.then_some(TextureHandle(handle)),
    );
}

/// Holds the camera with its view's corner at `corner`: the player no
/// longer leads it, so a cursor set on a world point stays on it.
pub fn pin_camera(harness: &mut UiHarness, corner: Vec2) {
    let player = player_entity(harness);
    let world = harness.world_mut();
    world.remove_component::<CameraTarget>(player);
    world.resource_mut::<CameraState>().position = corner;
}

/// Puts the cursor on the world point `at`, where the camera draws it.
pub fn aim_at(harness: &mut UiHarness, at: Vec2) {
    let screen = harness
        .world()
        .resource::<CameraState>()
        .world_to_screen(at);
    harness.harness_mut().set_cursor(screen.x, screen.y);
}

/// Selects the slot that holds `spell`, as its key does: pressed until a
/// fixed step has read it.
pub fn choose(harness: &mut UiHarness, spell: Spell) {
    let slot = game_spells::slot_of(spell).expect("a slot holds it");
    let action = game_spells::SLOT_ACTIONS[slot];
    harness.harness_mut().press_action(action);
    loop {
        harness.step(1);
        if steps_this_frame(harness) > 0 {
            break;
        }
    }
    harness.harness_mut().release_action(action);
}

/// The player's mana.
pub fn mana(harness: &UiHarness) -> f32 {
    game_spells::mana(harness.world(), player_entity(harness))
}

/// A static floor across the level's empty sky, x 0 to 2,400, its top at
/// `top`: a test's ground.
pub fn ground(harness: &mut UiHarness, top: f32) -> Entity {
    floor(
        harness,
        Vec2::new(1200.0, top + 50.0),
        Vec2::new(1200.0, 50.0),
    )
}

/// Stands `player` on a floor whose top is at `top`, its centre at `x`.
pub fn stand(harness: &mut UiHarness, player: Entity, x: f32, top: f32) {
    let centre = Vec2::new(x, top - player_half().y - 0.5);
    teleport(harness, player, centre);
}
