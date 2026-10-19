//! The sample's gameplay: a test per system, and one per kit component the
//! player prefab composes.

use std::path::Path;

use tungsten::core::assets::manifest::ResolvedManifest;
use tungsten::core::{
    ActionMap, AssetRegistry, Binding, CameraController, CameraMode, CameraState, Entity,
    PrevPosition, Settings, Sprite, Time, Transform, UserFolder, With,
};
use tungsten::kit::{
    ContactDamage, Counters, Damaged, Effects, Hit, HitFlash, Pickup, Stats, damage,
};
use tungsten::testing::UiHarness;
use tungsten::{App, SceneEntity};
use tungsten_template_basic::game;
use tungsten_template_basic::sample::components::Player;
use tungsten_template_basic::sample::{
    CAMERA_DEAD_ZONE, GEM_PREFAB, HAZARD_PREFAB, HAZARD_START, PLAYER_PREFAB, PLAYER_START,
};
use tungsten_template_basic::shell::{Difficulty, theme};

use super::{
    config, harness, harness_with, health, player, player_entity, player_position, start, start_at,
    tap, user_folder,
};

/// How far the player moves per second, in pixels: the prefab's
/// `top_down_mover.speed`.
const PLAYER_SPEED: f32 = 240.0;

/// How far one fixed step moves the player: the drawn position trails the
/// step by up to one, so a distance is checked within it.
const ONE_STEP: f32 = PLAYER_SPEED / 60.0;

/// The hazard: the entity whose `contact_damage` hurts the player.
fn hazard_entity(harness: &UiHarness) -> Entity {
    harness
        .world()
        .query_filtered::<Entity, With<ContactDamage>>()
        .next()
        .expect("the hazard spawned")
}

/// The gems still in play: the entities with a `pickup`.
fn gem_entities(harness: &UiHarness) -> Vec<Entity> {
    harness
        .world()
        .query_filtered::<Entity, With<Pickup>>()
        .collect()
}

/// The game's `gems` count, which each gem's `picked_up` cue adds to.
fn gems_counted(harness: &UiHarness) -> i64 {
    harness
        .world()
        .get_resource::<Counters>()
        .map_or(0, |counters| counters.get("gems"))
}

fn current(harness: &UiHarness, entity: Entity) -> f32 {
    health(harness, entity).0
}

/// The world labels the last frame drew, with their fonts.
fn labels(harness: &UiHarness) -> Vec<(String, String)> {
    harness
        .harness()
        .draw()
        .expect("a frame was drawn")
        .world_labels
        .iter()
        .map(|run| (run.content.clone(), run.font_id.clone()))
        .collect()
}

/// The resolved schedule as `register` builds it: the kit's phases and
/// systems in their stages between the engine's, the shell's and the
/// sample's. A reorder shows up here as a diff.
const SCHEDULE: &str = "\
startup: shell_start
pre_update: physics_debug_toggle, systems_overlay_toggle, inspector_toggle, hud_toggle, ui_debug_toggle, ui_inspector_toggle, log_console_toggle, display_input, ui_state_roots, ui_route, inspector_pick, ui_animate, state_dispatcher
fixed_update: physics_prev_snapshot, kit_tick, lifetime_tick, health_tick, shield_tick, stats_apply, cooldown_tick, effect_tick, ailment_tick, kit_intent, action_input, aim, ability_tick, kit_move, path_follow, linear_motion, oscillator, platformer_controller, top_down_mover, steering, follow, orbit, knockback, pickup_magnet, speed_limit, physics_step, kit_overlap, zone_tick, speed_limit_settle, hit_grid, hitbox_tick, contact_damage, projectile_tick, pickup_tick, despawn_outside, wrap, stay_in_view, kit_resolve, damage_resolve, ailment_feed, effect_resolve, kit_react, spawner_tick, spawn_children, level_tick, cues, kit_despawn, kit_trace_step, shell_checkpoint
update: shell_frame, shell_flow, sample_frame
post_update: physics_sync, transform_propagate, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, transform_propagate_late, kit_present, animation_tick, hit_flash_tick, camera_target, label_tick, oscillator_present, facing, character_animator, depth_sort, kit_debug_draw, camera_update
";

#[test]
fn the_schedule_matches_the_snapshot() {
    let mut app: App = super::app(UserFolder::Off);
    app.resolve_schedule().expect("the schedule resolves");
    assert_eq!(app.schedule().resolved_text(), SCHEDULE);
}

#[test]
fn data_files_load() {
    let config = config(UserFolder::Off);
    // Config::load validates the ID; copied games choose their own name.
    assert!(
        config.game.id.is_some(),
        "the game config needs an explicit ID"
    );
    let actions = ActionMap::load(Path::new("input.json")).expect("input.json loads");
    for action in [
        "move_left",
        "move_right",
        "move_up",
        "move_down",
        "state_pause",
    ] {
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
    assert!(manifest.data["cue_rules"]["labels"].path.is_file());
    assert!(manifest.ui_themes[theme::THEME_ID].path.is_file());
    assert!(manifest.shaders["damage_flash"].path.is_file());
    assert_eq!(manifest.materials["damage_flash"].shader, "damage_flash");
}

#[test]
fn a_new_run_spawns_the_player_owned_by_gameplay() {
    let mut harness = harness();
    assert_eq!(player(&harness), None);
    start(&mut harness);
    assert_eq!(player(&harness), Some(PLAYER_START));
    let world = harness.world();
    let (sprite, owner) = world
        .query_filtered::<(&Sprite, &SceneEntity), With<Player>>()
        .next()
        .expect("the player spawned");
    let assets = world.resource::<AssetRegistry>();
    assert_eq!(assets.sprite_name(sprite.asset_id), Some("player"));
    assert_eq!(owner.state_id, game::GAMEPLAY);
}

#[test]
fn player_moves_under_move_right_only_in_gameplay() {
    let mut harness = harness();
    start(&mut harness);
    harness.harness_mut().press_action("move_right");
    harness.step(30);
    let [x, y] = player(&harness).expect("the player spawned");
    let expected = PLAYER_START[0] + PLAYER_SPEED * 30.0 / 60.0;
    assert!(
        (x - expected).abs() <= ONE_STEP + 1e-3,
        "x = {x}, expected {expected}"
    );
    assert!((y - PLAYER_START[1]).abs() < f32::EPSILON);
    harness.harness_mut().release_action("move_right");

    tap(&mut harness, "state_pause");
    let paused_at = player(&harness).map(|[x, _]| x);
    harness.harness_mut().press_action("move_right");
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
    start(&mut harness);
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
    harness.harness_mut().press_action("move_right");
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
        start(&mut harness);
        // A scale set between frames applies from the next one.
        harness.world_mut().resource_mut::<Time>().set_scale(scale);
        harness.harness_mut().press_action("move_right");
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
    start(&mut harness);
    let entity = player_entity(&harness);
    assert_eq!(current(&harness, entity), 5.0);
    assert_eq!(damage(harness.world_mut(), entity, 1.0), Hit::Hurt);
    assert_eq!(current(&harness, entity), 4.0);
    assert!(
        harness.world().get::<HitFlash>(entity).unwrap().is_active(),
        "the manifest's damage_flash material is in place"
    );
    assert_eq!(
        damage(harness.world_mut(), entity, 1.0),
        Hit::Ignored,
        "inside the invulnerability window"
    );
    harness.step(1);
    let sent: Vec<Damaged> = harness.harness().events::<Damaged>().copied().collect();
    assert_eq!(sent, [Damaged::new(entity, 1.0, false)]);
    // The window is half a second: 30 steps later the next hit lands.
    harness.step(30);
    assert_eq!(damage(harness.world_mut(), entity, 1.0), Hit::Hurt);
}

#[test]
fn a_hit_shakes_the_camera_and_shows_its_damage() {
    let mut harness = harness();
    start(&mut harness);
    let entity = player_entity(&harness);
    assert_eq!(
        harness.world().resource::<CameraController>().shake_trauma,
        0.0
    );
    assert_eq!(damage(harness.world_mut(), entity, 1.0), Hit::Hurt);
    harness.step(1);
    assert_eq!(
        labels(&harness),
        [("1".to_string(), "sans".to_string())],
        "the labels cue rule floats the hit while the option is on"
    );
    assert!(
        harness.world().resource::<CameraController>().shake_trauma > 0.0,
        "and the player's damaged cue shakes the camera"
    );
}

/// A teleport of the player onto the hazard, a pixel right of its point:
/// `Position` and `PrevPosition`. The contact's push points from the
/// hazard's point to the player's, so the pixel gives it a way out.
fn place_on_hazard(harness: &mut UiHarness, player: Entity, hazard: Entity) {
    let mut at = harness
        .world()
        .get::<tungsten::core::Position>(hazard)
        .unwrap()
        .0;
    at.x += 1.0;
    let world = harness.world_mut();
    world.get_mut::<tungsten::core::Position>(player).unwrap().0 = at;
    world.get_mut::<PrevPosition>(player).unwrap().0 = at;
}

#[test]
fn a_player_kept_on_the_hazard_takes_one_hit_per_window() {
    let mut harness = harness();
    start(&mut harness);
    let (player, hazard) = (player_entity(&harness), hazard_entity(&harness));
    assert_eq!(
        current(&harness, player),
        5.0,
        "the idle player is never touched"
    );
    assert_eq!(
        harness.world().get::<SceneEntity>(hazard).unwrap().state_id,
        game::GAMEPLAY
    );

    place_on_hazard(&mut harness, player, hazard);
    harness.step(1);
    assert_eq!(current(&harness, player), 4.0, "one hit");
    assert_eq!(labels(&harness), [("1".to_string(), "sans".to_string())]);
    assert!(harness.world().resource::<CameraController>().shake_trauma > 0.0);

    // Kept on the hazard each frame: the window is half a second, 30 steps,
    // so none of the next 29 touches lands, and the one after it does.
    for _ in 0..29 {
        place_on_hazard(&mut harness, player, hazard);
        harness.step(1);
    }
    assert_eq!(current(&harness, player), 4.0, "the window paces the hits");
    place_on_hazard(&mut harness, player, hazard);
    harness.step(1);
    assert_eq!(current(&harness, player), 3.0, "the next hit after it");
}

/// The hazard's patrol: the amplitude of its swing along x and its period,
/// from `prefabs/hazard.json`.
const HAZARD_SWING: f32 = 240.0;
const HAZARD_PERIOD_FRAMES: usize = 240;

#[test]
fn the_hazard_patrols_its_line_and_a_player_walked_into_it_is_hit_once_and_pushed_away() {
    let mut harness = harness();
    start(&mut harness);
    let (player, hazard) = (player_entity(&harness), hazard_entity(&harness));
    let hazard_at = |harness: &UiHarness| {
        harness
            .world()
            .get::<tungsten::core::Position>(hazard)
            .unwrap()
            .0
    };
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
    assert_eq!(current(&harness, player), 5.0);

    // Down onto the hazard's line, then wait for its next pass.
    harness.harness_mut().press_action("move_down");
    harness.step(40);
    harness.harness_mut().release_action("move_down");
    let stood = player_position(&harness).unwrap();
    assert!(
        (stood[1] - HAZARD_START[1]).abs() <= ONE_STEP + 1e-3,
        "{stood:?}"
    );
    let mut hit = None;
    for frame in 0..HAZARD_PERIOD_FRAMES {
        harness.step(1);
        if current(&harness, player) < 5.0 {
            hit = Some(frame);
            break;
        }
    }
    assert!(hit.is_some(), "the hazard came by");
    assert_eq!(current(&harness, player), 4.0);
    let at_hit = player_position(&harness).unwrap();
    // The window is half a second: the pass is over before it ends, with
    // no second hit, and the push has thrown the player off its spot.
    harness.step(29);
    assert_eq!(current(&harness, player), 4.0, "one hit per window");
    let after = player_position(&harness).unwrap();
    let pushed = ((after[0] - at_hit[0]).powi(2) + (after[1] - at_hit[1]).powi(2)).sqrt();
    assert!(pushed > 30.0, "pushed {pushed} px away");
}

#[test]
fn a_hazard_hit_slows_the_player_for_its_effects_seconds() {
    let mut harness = harness();
    start(&mut harness);
    let (player, hazard) = (player_entity(&harness), hazard_entity(&harness));
    let speed = |harness: &UiHarness| {
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
    assert_eq!(current(&harness, player), 4.0);
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
    harness.harness_mut().press_action("move_up");
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
    let from = player_position(&harness).unwrap();
    harness.step(30);
    let full = from[1] - player_position(&harness).unwrap()[1];
    assert!(
        (full - 120.0).abs() < 1.0,
        "full speed: {full} px in half a second"
    );
    harness.harness_mut().release_action("move_up");
}

/// A headless review of a hit in motion: for the label's 0.8 s after the
/// player is put on the hazard, each frame's label run, the camera's shake
/// and the drawn points of the player and the hazard, printed under
/// `cargo test -- --nocapture`. The label wobbles with the camera while the
/// shake runs, then rises; it fades, the shake decays to nothing and the two
/// points part.
#[test]
fn a_hit_in_motion_floats_its_label_and_settles_its_shake() {
    let mut harness = harness();
    start(&mut harness);
    let (player, hazard) = (player_entity(&harness), hazard_entity(&harness));
    place_on_hazard(&mut harness, player, hazard);
    harness.step(1);
    assert_eq!(current(&harness, player), 4.0);
    let drawn = |harness: &UiHarness, entity: Entity| {
        harness.world().get::<Transform>(entity).unwrap().position
    };
    let mut rises = Vec::new();
    let mut alphas = Vec::new();
    let mut traumas = Vec::new();
    let mut settled = None;
    for frame in 0..48 {
        let draw = harness.harness().draw().expect("a frame was drawn");
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
            if trauma == 0.0 && settled.is_none() {
                settled = Some(rises.len());
            }
            rises.push(position[1]);
            alphas.push(alpha);
        }
        traumas.push(trauma);
        harness.step(1);
    }
    assert!(rises.len() > 40, "the label shows: {}", rises.len());
    let settled = settled.expect("the shake settles while the label shows");
    assert!(
        rises[settled..].windows(2).all(|pair| pair[1] <= pair[0]),
        "it rises once the shake has settled at {settled}: {rises:?}"
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
    assert_eq!(traumas[traumas.len() - 1], 0.0, "{traumas:?}");
    harness.step(2);
    assert!(
        harness.harness().draw().unwrap().world_labels.is_empty(),
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
    start(&mut harness);
    let entity = player_entity(&harness);
    let camera = harness.world().resource::<CameraController>();
    assert_eq!(camera.mode, CameraMode::Follow(entity));
    assert_eq!(camera.dead_zone_size.to_array(), CAMERA_DEAD_ZONE);
    let before = harness.world().resource::<CameraState>().position;
    harness.harness_mut().press_action("move_right");
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
fn a_gem_drifts_to_the_player_and_counts_once() {
    let mut harness = harness();
    start(&mut harness);
    let player = player_entity(&harness);
    let gems = gem_entities(&harness);
    assert_eq!(gems.len(), 3, "gameplay spawns three");
    assert_eq!(gems_counted(&harness), 0);
    let gem = gems[0];
    let start = harness
        .world()
        .get::<tungsten::core::Position>(gem)
        .unwrap()
        .0;
    // Within the player's reach of 96 px, out of touch.
    let mut near = start;
    near.x += 60.0;
    let world = harness.world_mut();
    world.get_mut::<tungsten::core::Position>(player).unwrap().0 = near;
    world.get_mut::<PrevPosition>(player).unwrap().0 = near;
    harness.step(1);
    let drifted = harness
        .world()
        .get::<tungsten::core::Position>(gem)
        .unwrap()
        .0;
    assert!(
        drifted.x > start.x,
        "it drifts toward the player: {drifted:?}"
    );
    harness.step(30);
    assert!(!harness.world().is_alive(gem), "taken");
    assert_eq!(gems_counted(&harness), 1);
    harness.step(30);
    assert_eq!(gems_counted(&harness), 1, "once");
    assert_eq!(gem_entities(&harness).len(), 2, "the others wait");
}

#[test]
fn a_new_run_resets_the_gem_count() {
    let mut harness = harness();
    start(&mut harness);
    harness
        .world_mut()
        .resource_mut::<Counters>()
        .set("gems", 2);
    start(&mut harness);
    assert_eq!(gems_counted(&harness), 0, "a fresh run counts from 0");
    assert_eq!(gem_entities(&harness).len(), 3);
}

#[test]
fn difficulty_sets_the_players_max_health() {
    for (difficulty, full) in [
        (Difficulty::Easy, 8.0),
        (Difficulty::Normal, 5.0),
        (Difficulty::Hard, 3.0),
    ] {
        let mut harness = harness();
        start_at(&mut harness, difficulty);
        // The first fixed step's `stats_apply` has run.
        assert_eq!(
            health(&harness, player_entity(&harness)),
            (full, full),
            "{difficulty:?}"
        );
    }
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
