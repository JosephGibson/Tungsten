//! The game on the engine's headless harness: no window or renderer, the
//! frames stepped one call at a time. A test per system, and one per kit
//! component the player prefab composes.

use std::path::{Path, PathBuf};

use tungsten::core::assets::manifest::ResolvedManifest;
use tungsten::core::{
    ActionMap, AssetRegistry, Binding, CameraController, CameraMode, CameraState, Config, Entity,
    Position, PrevPosition, Settings, Sprite, Time, Transform, UserFolder, With,
};
use tungsten::kit::{
    ContactDamage, Counters, Damaged, Effects, Health, Hit, HitFlash, Pickup, Stats, damage,
};
use tungsten::testing::Harness;
use tungsten::{App, SceneEntity};
use tungsten_template_basic::components::Player;
use tungsten_template_basic::game::{self, active_state};
use tungsten_template_basic::states::{
    AUTOSAVE, GAMEPLAY, GEM_PREFAB, HAZARD_PREFAB, HAZARD_START, PAUSE, PLAYER_PREFAB,
    PLAYER_START, TITLE,
};

/// How far the player moves per second, in pixels: the prefab's
/// `top_down_mover.speed`.
const PLAYER_SPEED: f32 = 240.0;

/// How far one fixed step moves the player: the drawn position trails the
/// step by up to one, so a distance is checked within it.
const ONE_STEP: f32 = PLAYER_SPEED / 60.0;

/// The game's config with its user folder set: `Off` in tests, so they
/// never read or write the developer's own settings and saves.
fn config(user_folder: UserFolder) -> Config {
    let mut config = Config::load("tungsten.json").expect("tungsten.json loads");
    config.game.user_folder = user_folder;
    config
}

/// The game as `main.rs` builds it, with one frame stepped: its `Startup`
/// stage runs `setup`, which requests the title state, and the engine's
/// dispatcher applies it in the same frame's `pre_update`. The harness loads
/// the manifest's headless sections (the prefabs, the tags data and the
/// flash material's metadata), as a window's startup loads them.
fn harness() -> Harness {
    harness_with(UserFolder::Off)
}

fn harness_with(user_folder: UserFolder) -> Harness {
    let mut app = App::new(config(user_folder)).expect("App::new failed");
    game::register(&mut app);
    let mut harness = Harness::new(app);
    harness
        .load_manifest(game::MANIFEST)
        .expect("the manifest loads");
    harness.step(1);
    harness
}

/// Presses `action` for a frame, then steps the frame that applies the
/// state change it asked for.
fn tap(harness: &mut Harness, action: &str) {
    harness.press_action(action);
    harness.step(1);
    harness.release_action(action);
    harness.step(1);
}

fn player(harness: &Harness) -> Option<[f32; 2]> {
    harness
        .world()
        .query_filtered::<&Transform, With<Player>>()
        .next()
        .map(|transform| [transform.position.x, transform.position.y])
}

/// The player's body position: where the mover left it, which the drawn
/// `Transform` trails by up to a step.
fn player_position(harness: &Harness) -> Option<[f32; 2]> {
    harness
        .world()
        .query_filtered::<&Position, With<Player>>()
        .next()
        .map(|position| [position.0.x, position.0.y])
}

fn player_entity(harness: &Harness) -> Entity {
    harness
        .world()
        .query_filtered::<Entity, With<Player>>()
        .next()
        .expect("the player spawned")
}

/// The hazard: the entity whose `contact_damage` hurts the player.
fn hazard_entity(harness: &Harness) -> Entity {
    harness
        .world()
        .query_filtered::<Entity, With<ContactDamage>>()
        .next()
        .expect("the hazard spawned")
}

/// The gems still in play: the entities with a `pickup`.
fn gem_entities(harness: &Harness) -> Vec<Entity> {
    harness
        .world()
        .query_filtered::<Entity, With<Pickup>>()
        .collect()
}

/// The game's `gems` count, which each gem's `picked_up` cue adds to.
fn gems_counted(harness: &Harness) -> i64 {
    harness
        .world()
        .get_resource::<Counters>()
        .map_or(0, |counters| counters.get("gems"))
}

fn health(harness: &Harness, entity: Entity) -> f32 {
    harness.world().get::<Health>(entity).unwrap().current()
}

/// The world labels the last frame drew, with their fonts.
fn labels(harness: &Harness) -> Vec<(String, String)> {
    harness
        .draw()
        .expect("a frame was drawn")
        .world_labels
        .iter()
        .map(|run| (run.content.clone(), run.font_id.clone()))
        .collect()
}

/// A fresh user folder for one persistence test.
fn user_folder(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tungsten-template-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The resolved schedule as `register` builds it: the kit's phases and
/// systems in their stages between the engine's, and the game's. A reorder
/// shows up here as a diff.
const SCHEDULE: &str = "\
startup: setup
pre_update: physics_debug_toggle, systems_overlay_toggle, inspector_toggle, hud_toggle, ui_debug_toggle, ui_inspector_toggle, log_console_toggle, display_input, ui_state_roots, ui_route, inspector_pick, ui_animate, state_dispatcher
fixed_update: physics_prev_snapshot, kit_tick, lifetime_tick, health_tick, shield_tick, stats_apply, cooldown_tick, effect_tick, ailment_tick, kit_intent, action_input, aim, ability_tick, kit_move, path_follow, linear_motion, oscillator, platformer_controller, top_down_mover, steering, follow, orbit, knockback, pickup_magnet, speed_limit, physics_step, kit_overlap, zone_tick, speed_limit_settle, hit_grid, hitbox_tick, contact_damage, projectile_tick, pickup_tick, despawn_outside, wrap, stay_in_view, kit_resolve, damage_resolve, ailment_feed, effect_resolve, kit_react, spawner_tick, spawn_children, level_tick, cues, kit_despawn, kit_trace_step
update: toggle_mute
post_update: physics_sync, transform_propagate, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, transform_propagate_late, kit_present, animation_tick, hit_flash_tick, camera_target, label_tick, oscillator_present, facing, character_animator, depth_sort, kit_debug_draw, camera_update
";

#[test]
fn the_schedule_matches_the_snapshot() {
    let mut app = App::new(config(UserFolder::Off)).expect("App::new failed");
    game::register(&mut app);
    app.resolve_schedule().expect("the schedule resolves");
    assert_eq!(app.schedule().resolved_text(), SCHEDULE);
}

#[test]
fn data_files_load() {
    let config = Config::load("tungsten.json").expect("tungsten.json loads");
    // Config::load validates the ID; copied games choose their own name.
    assert!(
        config.game.id.is_some(),
        "the game config needs an explicit ID"
    );
    let actions = ActionMap::load(Path::new("input.json")).expect("input.json loads");
    for action in ["move_left", "move_right", "move_up", "move_down"] {
        assert!(
            !actions.bindings(action).is_empty(),
            "{action} has no binding"
        );
        assert!(
            actions.bindings(action).iter().any(|binding| matches!(
                binding,
                Binding::GamepadAxis { .. } | Binding::GamepadButton { .. }
            )),
            "{action} has no gamepad binding"
        );
    }
    let manifest = ResolvedManifest::load(game::MANIFEST).expect("the manifest loads");
    assert!(manifest.sprites["player"].path.is_file());
    assert!(manifest.fonts[game::FONT].path.is_file());
    assert!(manifest.prefabs[PLAYER_PREFAB].path.is_file());
    assert!(manifest.prefabs[HAZARD_PREFAB].path.is_file());
    assert!(manifest.prefabs[GEM_PREFAB].path.is_file());
    assert!(manifest.data["tags"]["game"].path.is_file());
    assert!(manifest.shaders["damage_flash"].path.is_file());
    assert_eq!(manifest.materials["damage_flash"].shader, "damage_flash");
}

#[test]
fn state_start_starts_gameplay_with_the_player() {
    let mut harness = harness();
    assert_eq!(active_state(harness.world()), Some(TITLE));
    assert_eq!(player(&harness), None);
    tap(&mut harness, "state_start");
    assert_eq!(active_state(harness.world()), Some(GAMEPLAY));
    assert_eq!(player(&harness), Some(PLAYER_START));
    // The prefab's sprite, and gameplay owns the player.
    let world = harness.world();
    let (sprite, owner) = world
        .query_filtered::<(&Sprite, &SceneEntity), With<Player>>()
        .next()
        .expect("the player spawned");
    let assets = world.resource::<AssetRegistry>();
    assert_eq!(assets.sprite_name(sprite.asset_id), Some("player"));
    assert_eq!(owner.state_id, GAMEPLAY);
}

#[test]
fn player_moves_under_move_right_only_in_gameplay() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    harness.press_action("move_right");
    harness.step(30);
    let [x, y] = player(&harness).expect("the player spawned");
    let expected = PLAYER_START[0] + PLAYER_SPEED * 30.0 / 60.0;
    assert!(
        (x - expected).abs() <= ONE_STEP + 1e-3,
        "x = {x}, expected {expected}"
    );
    assert!((y - PLAYER_START[1]).abs() < f32::EPSILON);
    harness.release_action("move_right");

    tap(&mut harness, "state_pause");
    let paused_at = player(&harness).map(|[x, _]| x);
    harness.press_action("move_right");
    harness.step(10);
    assert_eq!(
        player(&harness).map(|[x, _]| x),
        paused_at,
        "paused, the clock holds and the player stays"
    );
}

#[test]
fn a_gamepad_stick_moves_the_player() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    // The harness presses an action's first binding: make it the stick's.
    let actions = harness.world_mut().resource_mut::<ActionMap>();
    let stick: Vec<Binding> = actions
        .bindings("move_right")
        .iter()
        .copied()
        .filter(|binding| matches!(binding, Binding::GamepadAxis { .. }))
        .collect();
    assert_eq!(stick.len(), 1, "move_right has one stick binding");
    actions.replace_bindings("move_right", stick);
    harness.press_action("move_right");
    harness.step(30);
    let [x, y] = player(&harness).expect("the player spawned");
    let expected = PLAYER_START[0] + PLAYER_SPEED * 30.0 / 60.0;
    assert!(
        (x - expected).abs() <= ONE_STEP + 1e-3,
        "x = {x}, expected {expected}"
    );
    assert!((y - PLAYER_START[1]).abs() < f32::EPSILON);
}

#[test]
fn clock_half_scale_moves_the_player_half_as_far() {
    let mut moved = Vec::new();
    for scale in [1.0, 0.5] {
        let mut harness = harness();
        tap(&mut harness, "state_start");
        // A scale set between frames applies from the next one.
        harness
            .world_mut()
            .get_resource_mut::<Time>()
            .expect("the app inserts Time")
            .set_scale(scale);
        harness.press_action("move_right");
        harness.step(30);
        let [x, _] = player(&harness).expect("the player spawned");
        moved.push(x - PLAYER_START[0]);
    }
    let (full, half) = (moved[0], moved[1]);
    assert!(
        (full - PLAYER_SPEED * 30.0 / 60.0).abs() <= ONE_STEP + 1e-3,
        "full {full}"
    );
    assert!(
        (half - full / 2.0).abs() <= ONE_STEP + 1e-3,
        "half {half}, full {full}"
    );
}

#[test]
fn damage_lowers_the_health_starts_the_flash_and_sends_damaged() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    let entity = player_entity(&harness);
    assert_eq!(
        harness.world().get::<Health>(entity).unwrap().current(),
        5.0
    );
    assert_eq!(damage(harness.world_mut(), entity, 1.0), Hit::Hurt);
    let world = harness.world();
    assert_eq!(world.get::<Health>(entity).unwrap().current(), 4.0);
    assert!(
        world.get::<HitFlash>(entity).unwrap().is_active(),
        "the manifest's damage_flash material is in place"
    );
    assert_eq!(
        damage(harness.world_mut(), entity, 1.0),
        Hit::Ignored,
        "inside the invulnerability window"
    );
    harness.step(1);
    let sent: Vec<Damaged> = harness.events::<Damaged>().copied().collect();
    assert_eq!(sent, [Damaged::new(entity, 1.0, false)]);
    // The window is half a second: 30 steps later the next hit lands.
    harness.step(30);
    assert_eq!(damage(harness.world_mut(), entity, 1.0), Hit::Hurt);
}

#[test]
fn a_hit_shakes_the_camera_and_shows_its_damage() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    let entity = player_entity(&harness);
    assert_eq!(
        harness.world().resource::<CameraController>().shake_trauma,
        0.0
    );
    assert_eq!(damage(harness.world_mut(), entity, 1.0), Hit::Hurt);
    harness.step(1);
    let labels: Vec<(&str, &str)> = harness
        .draw()
        .expect("a frame was drawn")
        .world_labels
        .iter()
        .map(|run| (run.content.as_str(), run.font_id.as_str()))
        .collect();
    assert_eq!(
        labels,
        [("1", "sans")],
        "the player's damaged cue floats the hit"
    );
    assert!(
        harness.world().resource::<CameraController>().shake_trauma > 0.0,
        "and shakes the camera"
    );
}

/// A teleport of the player onto the hazard, a pixel right of its point:
/// `Position` and `PrevPosition`. The contact's push points from the
/// hazard's point to the player's, so the pixel gives it a way out.
fn place_on_hazard(harness: &mut Harness, player: Entity, hazard: Entity) {
    let mut at = harness.world().get::<Position>(hazard).unwrap().0;
    at.x += 1.0;
    let world = harness.world_mut();
    world.get_mut::<Position>(player).unwrap().0 = at;
    world.get_mut::<PrevPosition>(player).unwrap().0 = at;
}

#[test]
fn a_player_kept_on_the_hazard_takes_one_hit_per_window() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    let (player, hazard) = (player_entity(&harness), hazard_entity(&harness));
    assert_eq!(
        health(&harness, player),
        5.0,
        "the idle player is never touched"
    );
    // The hazard is owned by gameplay, as the player is.
    assert_eq!(
        harness.world().get::<SceneEntity>(hazard).unwrap().state_id,
        GAMEPLAY
    );

    place_on_hazard(&mut harness, player, hazard);
    harness.step(1);
    assert_eq!(health(&harness, player), 4.0, "one hit");
    assert_eq!(
        labels(&harness),
        [("1".to_string(), "sans".to_string())],
        "the player's damaged cue floats the hit"
    );
    assert!(
        harness.world().resource::<CameraController>().shake_trauma > 0.0,
        "and shakes the camera"
    );

    // Kept on the hazard each frame, whatever the push and the patrol do
    // inside the step: the window is half a second, 30 steps, so none of
    // the next 29 touches lands, and the one after it does.
    for _ in 0..29 {
        place_on_hazard(&mut harness, player, hazard);
        harness.step(1);
    }
    assert_eq!(health(&harness, player), 4.0, "the window paces the hits");
    place_on_hazard(&mut harness, player, hazard);
    harness.step(1);
    assert_eq!(health(&harness, player), 3.0, "the next hit after it");
}

#[test]
fn a_killed_player_starts_gameplay_over() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    let first = player_entity(&harness);
    harness.press_action("move_right");
    harness.step(10);
    harness.release_action("move_right");
    assert_ne!(player(&harness), Some(PLAYER_START));

    assert_eq!(damage(harness.world_mut(), first, 5.0), Hit::Killed);
    // Gameplay's next update asks for the restart; the frame after applies
    // it.
    harness.step(2);
    assert_eq!(active_state(harness.world()), Some(GAMEPLAY));
    assert!(
        !harness.world().is_alive(first),
        "the dead player left with the old gameplay"
    );
    let player_now = player_entity(&harness);
    assert_eq!(health(&harness, player_now), 5.0);
    assert_eq!(player(&harness), Some(PLAYER_START));
}

/// The hazard's patrol: its start, the amplitude of its swing along x and
/// its period, from `prefabs/hazard.json`.
const HAZARD_SWING: f32 = 240.0;
const HAZARD_PERIOD_FRAMES: usize = 240;

#[test]
fn the_hazard_patrols_its_line_and_a_player_walked_into_it_is_hit_once_and_pushed_away() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    let (player, hazard) = (player_entity(&harness), hazard_entity(&harness));
    let hazard_at = |harness: &Harness| harness.world().get::<Position>(hazard).unwrap().0;
    let first = hazard_at(&harness);
    let (mut left, mut right) = (f32::INFINITY, f32::NEG_INFINITY);
    for _ in 0..HAZARD_PERIOD_FRAMES {
        harness.step(1);
        let at = hazard_at(&harness);
        assert!(
            (at.y - HAZARD_START[1]).abs() < 1e-3,
            "it keeps to its line"
        );
        left = left.min(at.x);
        right = right.max(at.x);
    }
    assert!(
        (left - (HAZARD_START[0] - HAZARD_SWING)).abs() < 1.0,
        "{left}"
    );
    assert!(
        (right - (HAZARD_START[0] + HAZARD_SWING)).abs() < 1.0,
        "{right}"
    );
    assert!(
        (hazard_at(&harness).x - first.x).abs() < 1e-2,
        "a whole period"
    );
    assert_eq!(
        health(&harness, player),
        5.0,
        "the idle player is never touched"
    );

    // Down onto the hazard's line, then wait for its next pass.
    harness.press_action("move_down");
    harness.step(40);
    harness.release_action("move_down");
    let stood = player_position(&harness).unwrap();
    assert!(
        (stood[1] - HAZARD_START[1]).abs() <= ONE_STEP + 1e-3,
        "{stood:?}"
    );
    let mut hit = None;
    for frame in 0..HAZARD_PERIOD_FRAMES {
        harness.step(1);
        if health(&harness, player) < 5.0 {
            hit = Some(frame);
            break;
        }
    }
    assert!(hit.is_some(), "the hazard came by");
    assert_eq!(health(&harness, player), 4.0);
    let at_hit = player_position(&harness).unwrap();
    // The window is half a second: the pass is over before it ends, with
    // no second hit, and the push has thrown the player off its spot.
    harness.step(29);
    assert_eq!(health(&harness, player), 4.0, "one hit per window");
    let after = player_position(&harness).unwrap();
    let pushed = ((after[0] - at_hit[0]).powi(2) + (after[1] - at_hit[1]).powi(2)).sqrt();
    assert!(pushed > 30.0, "pushed {pushed} px away");
}

#[test]
fn a_hazard_hit_slows_the_player_for_its_effects_seconds() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    let (player, hazard) = (player_entity(&harness), hazard_entity(&harness));
    let speed = |harness: &Harness| {
        harness
            .world()
            .get::<Stats>(player)
            .unwrap()
            .get("move_speed")
            .unwrap()
    };
    assert_eq!(speed(&harness), 240.0, "the stat the mover follows");
    // The hit: the hazard's `hit` cue queues `slowed` on the player, which
    // lands at the next step's effect resolve.
    place_on_hazard(&mut harness, player, hazard);
    harness.step(1);
    assert_eq!(health(&harness, player), 4.0);
    harness.step(1);
    assert_eq!(speed(&harness), 120.0, "halved by the effect's modifier");
    assert!(
        harness
            .world()
            .get::<Effects>(player)
            .unwrap()
            .has("slowed")
    );
    // Walking up, away from the hazard's line, while the push runs out
    // sideways: half a second covers 60 px at the slowed speed.
    harness.press_action("move_up");
    harness.step(1);
    let from = player_position(&harness).unwrap();
    harness.step(30);
    let slowed = from[1] - player_position(&harness).unwrap()[1];
    assert!(
        (slowed - 60.0).abs() < 1.0,
        "slowed: {slowed} px in half a second"
    );
    // Past the effect's second and a half, the same half second covers 120.
    harness.step(60);
    assert_eq!(speed(&harness), 240.0, "the effect expired");
    assert!(
        !harness
            .world()
            .get::<Effects>(player)
            .unwrap()
            .has("slowed")
    );
    let from = player_position(&harness).unwrap();
    harness.step(30);
    let full = from[1] - player_position(&harness).unwrap()[1];
    assert!(
        (full - 120.0).abs() < 1.0,
        "full speed: {full} px in half a second"
    );
    harness.release_action("move_up");
}

/// A headless review of a hit in motion: for the label's 0.8 s after the
/// player is put on the hazard, each frame's label run, the camera's shake
/// trauma and offset and the drawn points of the player and the hazard (their
/// `Transform`s, which the default extract draws; headless, the sprites have
/// no texture), printed under `cargo test -- --nocapture`. The label rises
/// and fades, the shake decays to nothing and the two points part.
#[test]
fn a_hit_in_motion_floats_its_label_and_settles_its_shake() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    let (player, hazard) = (player_entity(&harness), hazard_entity(&harness));
    place_on_hazard(&mut harness, player, hazard);
    harness.step(1);
    assert_eq!(health(&harness, player), 4.0);
    let drawn = |harness: &Harness, entity: Entity| {
        harness.world().get::<Transform>(entity).unwrap().position
    };
    let mut rises = Vec::new();
    let mut alphas = Vec::new();
    let mut traumas = Vec::new();
    for frame in 0..48 {
        let draw = harness.draw().expect("a frame was drawn");
        let run = draw
            .world_labels
            .first()
            .map(|run| (run.position, run.color[3]));
        let camera = harness.world().resource::<CameraController>();
        let (trauma, shake) = (camera.shake_trauma, camera.shake_offset());
        let (player_at, hazard_at) = (drawn(&harness, player), drawn(&harness, hazard));
        println!(
            "{frame:2}: label {run:?}, trauma {trauma:.3}, shake ({:5.2}, {:5.2}), player ({:7.2}, {:7.2}), hazard ({:7.2}, {:7.2})",
            shake.x, shake.y, player_at.x, player_at.y, hazard_at.x, hazard_at.y
        );
        if let Some((position, alpha)) = run {
            rises.push(position[1]);
            alphas.push(alpha);
        }
        traumas.push(trauma);
        harness.step(1);
    }
    assert!(
        rises.len() > 40,
        "the label shows for most of its seconds: {}",
        rises.len()
    );
    assert!(
        rises.windows(2).all(|pair| pair[1] <= pair[0]),
        "it rises: {rises:?}"
    );
    assert!(rises[0] - rises[rises.len() - 1] > 10.0, "{rises:?}");
    assert!(
        alphas.windows(2).all(|pair| pair[1] <= pair[0]),
        "it fades: {alphas:?}"
    );
    assert!(alphas[alphas.len() - 1] < alphas[0] / 2, "{alphas:?}");
    assert!(traumas[0] > 0.0, "a shake follows the hit: {traumas:?}");
    assert!(
        traumas.windows(2).all(|pair| pair[1] <= pair[0]),
        "{traumas:?}"
    );
    assert_eq!(
        traumas[traumas.len() - 1],
        0.0,
        "and decays to nothing: {traumas:?}"
    );
    harness.step(2);
    assert!(
        harness.draw().unwrap().world_labels.is_empty(),
        "gone after its seconds"
    );
    let gap = drawn(&harness, player).distance(drawn(&harness, hazard));
    assert!(gap > 28.0, "the push and the patrol part them: {gap}");
}

#[test]
fn the_camera_follows_the_player() {
    let mut harness = harness();
    assert_eq!(
        harness.world().resource::<CameraController>().mode,
        CameraMode::Free,
        "no target on the title"
    );
    tap(&mut harness, "state_start");
    let entity = player_entity(&harness);
    assert_eq!(
        harness.world().resource::<CameraController>().mode,
        CameraMode::Follow(entity)
    );
    assert_eq!(
        harness
            .world()
            .resource::<CameraController>()
            .dead_zone_size
            .to_array(),
        game::CAMERA_DEAD_ZONE
    );
    let before = harness.world().resource::<CameraState>().position;
    harness.press_action("move_right");
    // 120 px: still inside the dead zone, so the player moves on screen.
    harness.step(30);
    let inside = harness.world().resource::<CameraState>().position;
    assert_eq!(inside, before, "the camera holds while the player roams");
    // 360 px: past the zone's edge, the camera follows.
    harness.step(60);
    let after = harness.world().resource::<CameraState>().position;
    assert!(after.x > before.x, "the camera moved with the player");
    assert!((after.y - before.y).abs() < 1e-3);
}

#[test]
fn state_pause_pauses_and_state_back_resumes() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    tap(&mut harness, "state_pause");
    assert_eq!(active_state(harness.world()), Some(PAUSE));
    assert!(harness.world().resource::<Time>().is_paused());
    // Gameplay and its player stay under the pause.
    assert!(player(&harness).is_some());
    tap(&mut harness, "state_back");
    assert_eq!(active_state(harness.world()), Some(GAMEPLAY));
    assert!(!harness.world().resource::<Time>().is_paused());
    assert_eq!(player(&harness), Some(PLAYER_START));
}

#[test]
fn text_follows_the_state_on_top() {
    let mut harness = harness();
    let texts = |harness: &Harness| -> Vec<String> {
        game::text(harness.world())
            .into_iter()
            .map(|section| section.content)
            .collect()
    };
    assert!(texts(&harness)[0].contains("Tungsten template"));
    tap(&mut harness, "state_start");
    assert!(texts(&harness)[0].contains("WASD"));
    tap(&mut harness, "state_pause");
    assert_eq!(texts(&harness)[0], "Paused");
    let draw = harness.draw().expect("a completed frame");
    assert!(
        draw.text
            .iter()
            .any(|section| section.font_id == game::FONT)
    );
}

#[test]
fn mute_survives_a_restart() {
    let dir = user_folder("mute");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(harness.world().resource::<Settings>().master_volume(), None);
    tap(&mut harness, "toggle_mute");
    assert_eq!(
        harness.world().resource::<Settings>().master_volume(),
        Some(0.0)
    );
    assert!(dir.join("settings/settings.json").is_file());

    let mut restarted = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(
        restarted.world().resource::<Settings>().master_volume(),
        Some(0.0),
        "the mute is back after a restart"
    );
    tap(&mut restarted, "toggle_mute");
    assert_eq!(
        restarted.world().resource::<Settings>().master_volume(),
        None
    );
    let restarted = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(
        restarted.world().resource::<Settings>().master_volume(),
        None
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn continue_from_the_autosave() {
    let dir = user_folder("autosave");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    tap(&mut harness, "state_start");
    harness.press_action("move_right");
    harness.step(30);
    // Paused with the movement still held: the frame that pushes the
    // pause runs the step it owed, and the next frame, with no step, saves
    // where the body stopped.
    tap(&mut harness, "state_pause");
    harness.step(1);
    let moved = player_position(&harness).expect("the player spawned");
    assert_ne!(moved, PLAYER_START);
    assert!(dir.join("saves").join(format!("{AUTOSAVE}.json")).is_file());

    let mut restarted = harness_with(UserFolder::At(dir.clone()));
    tap(&mut restarted, "state_start");
    assert_eq!(
        player_position(&restarted),
        Some(moved),
        "gameplay continues where the body stopped"
    );
    assert_eq!(player(&restarted), Some(moved), "drawn there too");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_slot_that_fails_to_load_is_never_overwritten() {
    let dir = user_folder("newer-save");
    let slot = dir.join("saves").join(format!("{AUTOSAVE}.json"));
    std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
    // A save from a newer build of the game: schema 2.
    let newer = r#"{ "envelope": 1, "slot": "autosave", "schema": 2, "engine_version": "9.0.0", "saved_at_ms": 1, "data": { "player": [1.0, 2.0], "lives": 3 } }"#;
    std::fs::write(&slot, newer).unwrap();

    let mut harness = harness_with(UserFolder::At(dir.clone()));
    tap(&mut harness, "state_start");
    assert_eq!(player(&harness), Some(PLAYER_START));
    tap(&mut harness, "state_pause");
    harness.step(1);
    assert_eq!(active_state(harness.world()), Some(PAUSE));
    assert_eq!(
        std::fs::read_to_string(&slot).unwrap(),
        newer,
        "the slot is untouched"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_gem_drifts_to_the_player_and_counts_once() {
    let mut harness = harness();
    tap(&mut harness, "state_start");
    let player = player_entity(&harness);
    let gems = gem_entities(&harness);
    assert_eq!(gems.len(), 3, "gameplay spawns three");
    assert_eq!(gems_counted(&harness), 0);
    let gem = gems[0];
    let start = harness.world().get::<Position>(gem).unwrap().0;
    // Within the player's reach of 96 px, out of touch.
    let mut near = start;
    near.x += 60.0;
    let world = harness.world_mut();
    world.get_mut::<Position>(player).unwrap().0 = near;
    world.get_mut::<PrevPosition>(player).unwrap().0 = near;
    harness.step(1);
    let drifted = harness.world().get::<Position>(gem).unwrap().0;
    assert!(
        drifted.x > start.x,
        "it drifts toward the player: {drifted:?}"
    );
    harness.step(30);
    assert!(!harness.world().is_alive(gem), "taken");
    assert_eq!(gems_counted(&harness), 1);
    harness.step(30);
    assert_eq!(gems_counted(&harness), 1, "once");
    assert_eq!(
        gem_entities(&harness).len(),
        2,
        "the others wait out of reach"
    );
}
