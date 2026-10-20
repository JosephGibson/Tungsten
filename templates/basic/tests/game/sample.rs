//! The sample's gameplay: a test per system, and one per kit component the
//! player prefab composes.

use std::path::Path;

use tungsten::core::assets::manifest::ResolvedManifest;
use tungsten::core::{
    ActionMap, AssetRegistry, AudioCommand, Binding, BodyKind, CameraController, CameraMode,
    CameraState, Entity, ParticleEmitter, Position, PrevPosition, RigidBody, SoundRegistry, Sprite,
    Time, Transform, UserFolder, Velocity, Voice, With,
};
use tungsten::kit::{
    Counters, Damaged, Effects, Hit, HitFlash, LinearMotion, Projectile, Stats, damage,
};
use tungsten::testing::UiHarness;
use tungsten::{App, SceneEntity, WindowSize};
use tungsten_template_basic::game;
use tungsten_template_basic::sample::components::Player;
use tungsten_template_basic::sample::{
    CAMERA_DEAD_ZONE, DUMMY_START, ENEMIES, ENEMY_RETURN_SECS, GEM_PREFAB, HAZARD_PREFAB,
    HAZARD_START, MUSIC_FADE, MUSIC_PAUSE, MUSIC_RUN, PAUSE_FADE, PICKUPS, PLAY_CENTRE,
    PLAYER_PREFAB, PLAYER_START, SampleRun,
};
use tungsten_template_basic::shell::options::GameOptions;
use tungsten_template_basic::shell::{Difficulty, theme};

use super::{
    act, config, harness, health, heard, player, player_entity, player_position, plays,
    release_and_settle, sound, start, start_at, stops, tap,
};

/// How far the player moves per second, in pixels: the prefab's
/// `top_down_mover.speed`.
const PLAYER_SPEED: f32 = 240.0;

/// How far one fixed step moves the player: the drawn position trails the
/// step by up to one, so a distance is checked within it.
const ONE_STEP: f32 = PLAYER_SPEED / 60.0;

/// How far the player covers in `steps` fixed steps from rest at up to
/// `top` px/s: the prefab's acceleration, 1,800 px/s², gains 30 px/s a
/// step.
fn ramp(steps: u32, top: f32) -> f32 {
    (1..=steps)
        .map(|step| (30.0 * step as f32).min(top) / 60.0)
        .sum()
}

/// The hazard: the first of the run's enemies.
fn hazard_entity(harness: &UiHarness) -> Entity {
    harness.world().resource::<SampleRun>().enemies[0].entity
}

/// The gems still in play: the run's gems not yet taken.
fn gem_entities(harness: &UiHarness) -> Vec<Entity> {
    let world = harness.world();
    let gems = world.resource::<SampleRun>().gems.iter();
    gems.map(|(_, gem)| *gem)
        .filter(|gem| world.is_alive(*gem))
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
    let expected = PLAYER_START[0] + ramp(30, PLAYER_SPEED);
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
    let expected = PLAYER_START[0] + ramp(30, PLAYER_SPEED);
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
    // Half the clock runs half the steps, each eased in the same.
    let (full, half) = (moved[0], moved[1]);
    assert!(
        (full - ramp(30, PLAYER_SPEED)).abs() <= ONE_STEP + 1e-3,
        "full {full}"
    );
    assert!(
        (half - ramp(15, PLAYER_SPEED)).abs() <= ONE_STEP + 1e-3,
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
        [("1".to_string(), "engine_mono".to_string())],
        "the labels cue rule floats the hit while the option is on"
    );
    let trauma = harness.world().resource::<CameraController>().shake_trauma;
    assert!(
        trauma > 0.55,
        "and the player's damaged cue shakes the camera hard: {trauma}"
    );
}

/// A teleport of the player onto the hazard, a pixel right of its point:
/// `Position` and `PrevPosition`. The contact's push points from the
/// hazard's point to the player's, so the pixel gives it a way out.
fn place_on_hazard(harness: &mut UiHarness, player: Entity, hazard: Entity) {
    let mut at = harness.world().get::<Position>(hazard).unwrap().0;
    at.x += 1.0;
    let world = harness.world_mut();
    world.get_mut::<Position>(player).unwrap().0 = at;
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
    assert_eq!(
        labels(&harness),
        [("1".to_string(), "engine_mono".to_string())]
    );
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
    let hazard_at = |harness: &UiHarness| harness.world().get::<Position>(hazard).unwrap().0;
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

    // Down onto the hazard's line, eased in and out: 150 px in 41 steps
    // and 10 more sliding to a stop. Then wait for its next pass.
    harness.harness_mut().press_action("move_down");
    harness.step(41);
    release_and_settle(&mut harness, "move_down");
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
    // sideways: half a second from the second step covers 58.5 px, eased
    // in to the slowed 120 px/s.
    harness.harness_mut().press_action("move_up");
    harness.step(1);
    let from = player_position(&harness).unwrap();
    harness.step(30);
    let slowed = from[1] - player_position(&harness).unwrap()[1];
    let expected = ramp(31, 120.0) - ramp(1, 120.0);
    assert!(
        (slowed - expected).abs() < 1.0,
        "slowed: {slowed} px in half a second, {expected} expected"
    );
    // Past the effect's second and a half and the ease back up, the same
    // half second covers 120.
    harness.step(70);
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
fn the_player_accelerates_and_stops() {
    let mut harness = harness();
    start(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    let player = player_entity(&harness);
    let speed = |harness: &UiHarness| harness.world().get::<Velocity>(player).unwrap().0.length();
    assert_eq!(speed(&harness), 0.0, "at rest");
    harness.harness_mut().press_action("move_right");
    harness.step(1);
    assert!(
        speed(&harness) > 0.0 && speed(&harness) < PLAYER_SPEED,
        "eased in"
    );
    let mut steps = 1;
    while (speed(&harness) - PLAYER_SPEED).abs() > 1e-3 {
        harness.step(1);
        steps += 1;
        assert!(steps <= 9, "full speed within nine steps");
    }
    harness.harness_mut().release_action("move_right");
    let mut steps = 0;
    while speed(&harness) > 0.0 {
        harness.step(1);
        steps += 1;
        assert!(steps <= 7, "at rest within seven steps");
    }
}

/// The bolts in flight, with their velocities.
fn bolts(harness: &UiHarness) -> Vec<(Entity, [f32; 2])> {
    let world = harness.world();
    world
        .query_filtered::<(Entity, &LinearMotion), With<Projectile>>()
        .map(|(entity, motion)| (entity, motion.velocity().to_array()))
        .collect()
}

#[test]
fn a_click_shoots_a_bolt_toward_the_cursor() {
    let mut harness = harness();
    start(&mut harness);
    let at = player_position(&harness).unwrap();
    // The camera starts at the play space's top-left at zoom 1, so the
    // window's pixels are the world's; the aim leaves the sprite's middle.
    harness.pointer_to(at[0] + MIDDLE + 300.0, at[1] + MIDDLE);
    harness.step(1);
    harness.harness_mut().press_action("shoot");
    harness.step(2);
    harness.harness_mut().release_action("shoot");
    let fired = bolts(&harness);
    assert_eq!(fired.len(), 1, "a bolt within two steps");
    let [x, y] = fired[0].1;
    assert!(x > 470.0 && y.abs() < 1.0, "toward the cursor: {x}, {y}");
    let start_x = harness.world().get::<Position>(fired[0].0).unwrap().0.x;
    harness.step(3);
    let now_x = harness.world().get::<Position>(fired[0].0).unwrap().0.x;
    assert!(now_x > start_x, "it moves right");
    // A press inside the 0.2 s cooldown fires nothing; one after it fires.
    harness.harness_mut().press_action("shoot");
    harness.step(1);
    harness.harness_mut().release_action("shoot");
    harness.step(1);
    assert_eq!(bolts(&harness).len(), 1, "inside the cooldown");
    harness.step(12);
    harness.harness_mut().press_action("shoot");
    harness.step(2);
    harness.harness_mut().release_action("shoot");
    assert_eq!(bolts(&harness).len(), 2, "a second after its cooldown");
}

#[test]
fn the_dummy_takes_hits_shows_damage_and_never_dies() {
    let mut harness = harness();
    start(&mut harness);
    let dummy = harness.world().resource::<SampleRun>().dummy;
    for hit in 0..200 {
        assert_eq!(
            damage(harness.world_mut(), dummy, 1.0),
            Hit::Hurt,
            "hit {hit}"
        );
        harness.step(1);
        if hit == 0 {
            assert_eq!(
                labels(&harness),
                [("1".to_string(), "engine_mono".to_string())],
                "a hit floats its damage"
            );
        }
    }
    assert!(current(&harness, dummy) > 0.0, "it never dies");
    assert_eq!(current(&harness, dummy), 100.0, "each hit mended");
    harness
        .world_mut()
        .resource_mut::<GameOptions>()
        .damage_numbers = false;
    harness.step(60);
    damage(harness.world_mut(), dummy, 1.0);
    harness.step(1);
    assert!(labels(&harness).is_empty(), "none with the option off");
}

/// Puts the player on pickup `at` of [`PICKUPS`] and steps once.
fn take(harness: &mut UiHarness, at: usize) {
    let player = player_entity(harness);
    let spot = PICKUPS[at].1.into();
    let world = harness.world_mut();
    world.get_mut::<Position>(player).unwrap().0 = spot;
    world.get_mut::<PrevPosition>(player).unwrap().0 = spot;
    harness.step(2);
}

#[test]
fn the_heal_pickup_heals_two() {
    let mut harness = harness();
    start(&mut harness);
    let player = player_entity(&harness);
    assert_eq!(damage(harness.world_mut(), player, 3.0), Hit::Hurt);
    assert_eq!(current(&harness, player), 2.0);
    take(&mut harness, 0);
    assert_eq!(current(&harness, player), 4.0, "healed 2");
    let run = harness.world().resource::<SampleRun>();
    assert!(!harness.world().is_alive(run.pickups[0].1), "taken once");
}

#[test]
fn the_speed_pickup_speeds_the_player_for_four_seconds() {
    let mut harness = harness();
    start(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    let player = player_entity(&harness);
    let speed = |harness: &UiHarness| {
        harness
            .world()
            .get::<Stats>(player)
            .unwrap()
            .get("move_speed")
            .unwrap()
    };
    take(&mut harness, 1);
    harness.step(1);
    assert_eq!(speed(&harness), 360.0, "half again as fast");
    harness.step(3 * 60);
    assert_eq!(speed(&harness), 360.0, "still at 3 s");
    harness.step(60 + 5);
    assert_eq!(speed(&harness), PLAYER_SPEED, "over after 4 s");
}

#[test]
fn the_camera_frames_the_play_space_at_every_window_size() {
    for (window, zoom) in [([1280, 720], 1.0), ([3840, 2160], 3.0), ([2560, 1080], 1.5)] {
        let mut harness = harness();
        harness.world_mut().insert_resource(WindowSize {
            width: window[0],
            height: window[1],
        });
        start(&mut harness);
        let camera = *harness.world().resource::<CameraState>();
        assert!(
            (camera.zoom - zoom).abs() < 1e-4,
            "{window:?}: zoom {}",
            camera.zoom
        );
        let centre = [0, 1]
            .map(|axis| camera.position.to_array()[axis] + window[axis] as f32 / camera.zoom / 2.0);
        let off = [centre[0] - PLAY_CENTRE[0], centre[1] - PLAY_CENTRE[1]];
        assert!(
            off[0].abs() < 0.5 && off[1].abs() < 0.5,
            "{window:?}: the view centred on {centre:?}"
        );
    }
}

/// The volume changes among `commands`.
fn levels(commands: &[AudioCommand]) -> Vec<(Voice, f32, f32)> {
    commands
        .iter()
        .filter_map(|command| match command {
            AudioCommand::SetVoiceVolume {
                voice,
                volume,
                fade,
                ..
            } => Some((*voice, *volume, *fade)),
            _ => None,
        })
        .collect()
}

/// A run started as New Game starts it, with its frames' audio; returns
/// the run music's voice.
fn start_heard(harness: &mut UiHarness) -> (Voice, Vec<AudioCommand>) {
    tungsten_template_basic::shell::start_run(
        harness.world_mut(),
        tungsten_template_basic::shell::RunStart::default(),
        None,
    );
    let mut commands = heard(harness, 1);
    while tungsten_template_basic::shell::transitioning(harness.world()) {
        commands.extend(heard(harness, 1));
    }
    let run = plays(harness, &commands, MUSIC_RUN);
    assert_eq!(run.len(), 1, "one run music");
    (run[0].0, commands)
}

#[test]
fn a_shot_a_hit_and_a_pickup_play_their_sounds() {
    let mut harness = harness();
    start(&mut harness);
    let at = player_position(&harness).unwrap();
    harness.pointer_to(at[0] + 300.0, at[1]);
    harness.step(1);
    harness.harness_mut().press_action("shoot");
    let shot = heard(&mut harness, 3);
    harness.harness_mut().release_action("shoot");
    assert_eq!(plays(&harness, &shot, "shoot").len(), 1, "the shot");
    let dummy = harness.world().resource::<SampleRun>().dummy;
    damage(harness.world_mut(), dummy, 1.0);
    let hit = heard(&mut harness, 2);
    assert_eq!(plays(&harness, &hit, "hit").len(), 1, "the dummy's hit");
    let player = player_entity(&harness);
    let spot = PICKUPS[0].1.into();
    harness.world_mut().get_mut::<Position>(player).unwrap().0 = spot;
    harness
        .world_mut()
        .get_mut::<PrevPosition>(player)
        .unwrap()
        .0 = spot;
    let taken = heard(&mut harness, 3);
    assert_eq!(plays(&harness, &taken, "pickup").len(), 1, "the pickup");
    damage(harness.world_mut(), player, 1.0);
    let hurt = heard(&mut harness, 2);
    assert_eq!(plays(&harness, &hurt, "hurt").len(), 1, "the player's hurt");
}

#[test]
fn the_run_music_fades_in_with_a_run_and_out_at_its_end() {
    let mut harness = harness();
    let (run, started) = start_heard(&mut harness);
    assert_eq!(
        plays(&harness, &started, MUSIC_RUN),
        [(run, true, MUSIC_FADE)],
        "looping, faded in"
    );
    act(&mut harness, "state_pause");
    let quit = find_named(&harness, "Quit to Title");
    assert!(harness.focus(quit));
    act(&mut harness, "ui_accept");
    let answer = find_named(&harness, "Quit");
    assert!(harness.focus(answer));
    let ended = act(&mut harness, "ui_accept");
    assert!(
        stops(&ended).contains(&(run, MUSIC_FADE)),
        "faded out with the run: {ended:?}"
    );
}

/// The enabled control named `name` on the front-most screen.
fn find_named(harness: &UiHarness, name: &str) -> tungsten::core::ui::WidgetId {
    super::find(harness, name).unwrap_or_else(|| panic!("no control '{name}'"))
}

#[test]
fn pause_fades_the_run_music_under_its_loop_and_back() {
    let mut harness = harness();
    let (run, _) = start_heard(&mut harness);
    let paused = act(&mut harness, "state_pause");
    let looped = plays(&harness, &paused, MUSIC_PAUSE);
    assert_eq!(looped.len(), 1, "pause's loop starts");
    let (pause, looping, fade) = looped[0];
    assert!(looping && fade == PAUSE_FADE);
    assert_eq!(
        levels(&paused),
        [(run, 0.0, PAUSE_FADE)],
        "the run's voice falls silent"
    );
    assert!(stops(&paused).is_empty(), "and plays on");
    // A screen over pause keeps the loop.
    let settings = find_named(&harness, "Settings");
    assert!(harness.focus(settings));
    let over = act(&mut harness, "ui_accept");
    let back = act(&mut harness, "ui_cancel");
    for commands in [&over, &back] {
        assert!(stops(commands).is_empty() && levels(commands).is_empty());
        assert!(plays(&harness, commands, MUSIC_PAUSE).is_empty());
    }
    let resumed = act(&mut harness, "state_pause");
    assert_eq!(stops(&resumed), [(pause, PAUSE_FADE)], "the loop stops");
    let level = {
        let sounds = harness.world().resource::<SoundRegistry>();
        sounds.get_volume(sound(&harness, MUSIC_RUN))
    };
    assert_eq!(
        levels(&resumed),
        [(run, level, PAUSE_FADE)],
        "the run's voice is back"
    );
}

#[test]
fn quit_to_title_from_pause_stops_the_pause_loop() {
    let mut harness = harness();
    let (run, _) = start_heard(&mut harness);
    let paused = act(&mut harness, "state_pause");
    let pause = plays(&harness, &paused, MUSIC_PAUSE)[0].0;
    let quit = find_named(&harness, "Quit to Title");
    assert!(harness.focus(quit));
    act(&mut harness, "ui_accept");
    let answer = find_named(&harness, "Quit");
    assert!(harness.focus(answer));
    let ended = act(&mut harness, "ui_accept");
    let stopped = stops(&ended);
    assert!(
        stopped.contains(&(run, MUSIC_FADE)) && stopped.contains(&(pause, MUSIC_FADE)),
        "both voices stop: {stopped:?}"
    );
}

#[test]
fn game_over_plays_its_sting_and_music_over_the_silenced_run() {
    let mut harness = harness();
    let (run, _) = start_heard(&mut harness);
    let player = player_entity(&harness);
    damage(harness.world_mut(), player, 99.0);
    let mut over = heard(&mut harness, 1);
    while tungsten_template_basic::shell::transitioning(harness.world()) {
        over.extend(heard(&mut harness, 1));
    }
    over.extend(heard(&mut harness, 30));
    assert_eq!(
        tungsten_template_basic::shell::top(harness.world()),
        Some(tungsten_template_basic::shell::game_over::GAME_OVER)
    );
    let [sting, music] = theme::GAME_OVER_SOUNDS;
    assert_eq!(plays(&harness, &over, sting).len(), 1, "the sting, once");
    let played = plays(&harness, &over, music);
    assert_eq!(played.len(), 1, "game over's music");
    let (music, looping, fade) = played[0];
    assert!(looping && fade == theme::GAME_OVER_FADE);
    assert!(
        plays(&harness, &over, MUSIC_PAUSE).is_empty(),
        "no pause loop"
    );
    assert_eq!(
        levels(&over),
        [(run, 0.0, PAUSE_FADE)],
        "the run's music falls silent"
    );
    let quit = find_named(&harness, "Quit to Title");
    assert!(harness.focus(quit));
    let ended = act(&mut harness, "ui_accept");
    let stopped = stops(&ended);
    assert!(
        stopped.contains(&(music, theme::GAME_OVER_FADE)) && stopped.contains(&(run, MUSIC_FADE)),
        "both stop with the run: {stopped:?}"
    );
}

/// A sprite's middle from its entity's point: the engine draws a sprite's
/// top-left corner at the point, and every sprite is 32 px square.
const MIDDLE: f32 = 16.0;

/// The world labels the last frame drew: each one's text and the physical
/// pixel its pivot, the middle of its foot, landed on.
fn label_spots(harness: &UiHarness) -> Vec<(String, [f32; 2])> {
    let draw = harness.harness().draw().expect("a frame was drawn");
    let runs = draw.world_labels.iter();
    runs.map(|run| (run.content.clone(), run.position))
        .collect()
}

#[test]
fn shots_land_where_the_cursor_points_at_every_window_size() {
    for window in [[1280, 720], [3840, 2160]] {
        let mut harness = harness();
        harness.world_mut().insert_resource(WindowSize {
            width: window[0],
            height: window[1],
        });
        start(&mut harness);
        let dummy = harness.world().resource::<SampleRun>().dummy;
        // Down and left of the dummy, so the shot crosses at a slant.
        let from = [DUMMY_START[0] - 160.0, DUMMY_START[1] + 90.0];
        put_player(&mut harness, from);
        harness.step(1);
        let middle = [DUMMY_START[0] + MIDDLE, DUMMY_START[1] + MIDDLE];
        let camera = *harness.world().resource::<CameraState>();
        let spot = camera.world_to_screen(middle.into());
        harness.pointer_to(spot.x, spot.y);
        harness.step(1);
        harness.harness_mut().press_action("shoot");
        harness.step(2);
        harness.harness_mut().release_action("shoot");
        let fired = bolts(&harness);
        assert_eq!(fired.len(), 1, "{window:?}: a bolt");
        let [vx, vy] = fired[0].1;
        let [wx, wy] = [middle[0] - from[0] - MIDDLE, middle[1] - from[1] - MIDDLE];
        let cos = (vx * wx + vy * wy) / (vx.hypot(vy) * wx.hypot(wy));
        assert!(cos > 0.9999, "{window:?}: ({vx}, {vy}) toward ({wx}, {wy})");
        let mut struck = false;
        for _ in 0..30 {
            harness.step(1);
            let flash = harness.world().get::<HitFlash>(dummy);
            if flash.is_some_and(|flash| flash.is_active()) {
                struck = true;
                break;
            }
        }
        assert!(struck, "{window:?}: the dummy took the shot");
        // Its damage floats from the dummy's middle, centred on it.
        let camera = *harness.world().resource::<CameraState>();
        let centre = camera.world_to_screen(middle.into());
        let spots = label_spots(&harness);
        assert_eq!(spots.len(), 1, "{window:?}: {spots:?}");
        let (text, [x, y]) = &spots[0];
        assert_eq!(text, "1");
        assert!(
            (x - centre.x).abs() < 1.0,
            "{window:?}: x {x} at {}",
            centre.x
        );
        let above = centre.y - y;
        assert!(
            (-1.0..40.0).contains(&above),
            "{window:?}: y {y} at {}",
            centre.y
        );
    }
}

#[test]
fn bodies_keep_the_player_out_of_the_dummy_and_the_enemies() {
    let mut harness = harness();
    start(&mut harness);
    let (player, dummy) = {
        let run = harness.world().resource::<SampleRun>();
        (run.player, run.dummy)
    };
    let kind = |harness: &UiHarness, entity| {
        harness
            .world()
            .get::<RigidBody>(entity)
            .map(|body| body.kind)
    };
    assert_eq!(kind(&harness, player), Some(BodyKind::Kinematic));
    assert_eq!(kind(&harness, dummy), Some(BodyKind::Static));
    for enemy in enemies(&harness) {
        assert_eq!(kind(&harness, enemy), Some(BodyKind::Kinematic));
    }
    // Walked into the dummy from its left, the player stops against it:
    // the box's 12 and the dummy's circle's 14 keep their middles 26 apart.
    put_player(&mut harness, [DUMMY_START[0] - 70.0, DUMMY_START[1]]);
    harness.step(2);
    harness.harness_mut().press_action("move_right");
    harness.step(60);
    harness.harness_mut().release_action("move_right");
    let gap = DUMMY_START[0] - harness.world().get::<Position>(player).unwrap().0.x;
    assert!((25.0..=30.0).contains(&gap), "stopped against it: {gap}");
}

#[test]
fn a_bolt_hit_bursts_sparks_where_it_struck() {
    let mut harness = harness();
    start(&mut harness);
    let dummy = harness.world().resource::<SampleRun>().dummy;
    let from = [DUMMY_START[0] - 200.0, DUMMY_START[1]];
    put_player(&mut harness, from);
    harness.pointer_to(from[0] + MIDDLE + 300.0, from[1] + MIDDLE);
    harness.step(1);
    harness.harness_mut().press_action("shoot");
    harness.step(2);
    harness.harness_mut().release_action("shoot");
    let sparks = |harness: &UiHarness| -> Vec<[f32; 2]> {
        let world = harness.world();
        let found = world.query_filtered::<&Transform, With<ParticleEmitter>>();
        found.map(|at| at.position.to_array()).collect()
    };
    let mut burst = Vec::new();
    for _ in 0..30 {
        harness.step(1);
        burst = sparks(&harness);
        if !burst.is_empty() {
            break;
        }
    }
    assert!(
        harness
            .world()
            .get::<HitFlash>(dummy)
            .is_some_and(|flash| flash.is_active())
    );
    assert_eq!(burst.len(), 1, "one burst");
    // At the bolt's middle when it met the dummy: 20 px from the dummy's.
    let [x, y] = burst[0];
    let middle = [DUMMY_START[0] + MIDDLE, DUMMY_START[1] + MIDDLE];
    let off = (x - middle[0]).hypot(y - middle[1]);
    assert!((14.0..=26.0).contains(&off), "at the impact: {off}");
    harness.step(40);
    assert!(
        sparks(&harness).is_empty(),
        "the emitter goes after its burst"
    );
}

#[test]
fn enemies_turn_through_their_four_frames() {
    let mut harness = harness();
    start(&mut harness);
    let enemy = enemies(&harness)[1];
    let mut seen = Vec::new();
    for _ in 0..40 {
        let frame = harness.world().get::<Sprite>(enemy).unwrap().asset_id;
        if !seen.contains(&frame) {
            seen.push(frame);
        }
        harness.step(1);
    }
    assert_eq!(seen.len(), 4, "the spin's four frames");
}

#[test]
fn a_shot_flies_at_its_speed_and_ends_at_its_range() {
    let mut harness = harness();
    start(&mut harness);
    let at = player_position(&harness).unwrap();
    // Left, along an empty row.
    harness.pointer_to(at[0] + MIDDLE - 300.0, at[1] + MIDDLE);
    harness.step(1);
    harness.harness_mut().press_action("shoot");
    harness.step(2);
    harness.harness_mut().release_action("shoot");
    let fired = bolts(&harness);
    assert_eq!(fired.len(), 1);
    let [x, y] = fired[0].1;
    assert!(
        (x + 640.0).abs() < 1.0 && y.abs() < 1.0,
        "640 px/s: {x}, {y}"
    );
    let mut steps = 0;
    while !bolts(&harness).is_empty() {
        harness.step(1);
        steps += 1;
        assert!(steps < 120, "it ends");
    }
    // 760 px at 640 px/s: 71 steps of 1/60 s, give or take its first.
    assert!((68..=74).contains(&steps), "{steps} steps");
}

/// Puts the player at `at`, without a slide from where it was.
fn put_player(harness: &mut UiHarness, at: [f32; 2]) {
    let player = player_entity(harness);
    let world = harness.world_mut();
    world.get_mut::<Position>(player).unwrap().0 = at.into();
    world.get_mut::<PrevPosition>(player).unwrap().0 = at.into();
}

#[test]
fn a_bolt_hits_what_it_visibly_touches() {
    let mut harness = harness();
    start(&mut harness);
    let dummy = harness.world().resource::<SampleRun>().dummy;
    // A shot passing 17 px from the dummy's middle: the bolt's 5-px circle
    // and the dummy's 15-px one overlap there, as their sprites do.
    let from = [DUMMY_START[0] - 200.0, DUMMY_START[1] + 17.0];
    put_player(&mut harness, from);
    harness.pointer_to(from[0] + MIDDLE + 300.0, from[1] + MIDDLE);
    harness.step(1);
    harness.harness_mut().press_action("shoot");
    harness.step(2);
    harness.harness_mut().release_action("shoot");
    let mut struck = false;
    for _ in 0..40 {
        harness.step(1);
        struck |= harness
            .world()
            .get::<HitFlash>(dummy)
            .is_some_and(|flash| flash.is_active());
    }
    assert!(struck, "the dummy took the bolt");
    assert!(bolts(&harness).is_empty(), "and the bolt ended there");
}

/// The run's enemies, in [`ENEMIES`]' order.
fn enemies(harness: &UiHarness) -> Vec<Entity> {
    let run = harness.world().resource::<SampleRun>();
    run.enemies.iter().map(|enemy| enemy.entity).collect()
}

#[test]
fn enemies_move_take_three_hits_and_come_back() {
    let mut harness = harness();
    start(&mut harness);
    let spawned = enemies(&harness);
    assert_eq!(spawned.len(), ENEMIES.len());
    let at =
        |harness: &UiHarness, entity: Entity| harness.world().get::<Position>(entity).unwrap().0;
    let first: Vec<_> = spawned.iter().map(|&e| at(&harness, e)).collect();
    harness.step(30);
    for (entity, before) in spawned.iter().zip(&first) {
        assert!(at(&harness, *entity).distance(*before) > 1.0, "it moves");
    }
    // A bolt hits an enemy: one less health.
    let orbiter = spawned[1];
    let near = at(&harness, orbiter);
    harness
        .world_mut()
        .prepare_prefab("bolt", [near.x - 30.0, near.y])
        .unwrap()
        .spawn(harness.world_mut());
    harness.step(6);
    assert_eq!(current(&harness, orbiter), 2.0, "a bolt took one");
    for _ in 0..2 {
        damage(harness.world_mut(), orbiter, 1.0);
        harness.step(1);
    }
    harness.step(2);
    assert!(!harness.world().is_alive(orbiter), "three hits end it");
    let seconds = (ENEMY_RETURN_SECS * 60.0) as u32;
    harness.step(seconds - 10);
    let back = enemies(&harness)[1];
    assert_eq!(back, orbiter, "not yet back");
    harness.step(20);
    let back = enemies(&harness)[1];
    assert_ne!(back, orbiter, "back after its seconds");
    assert!(harness.world().is_alive(back));
    assert_eq!(current(&harness, back), 3.0, "whole");
    let start = ENEMIES[1].1;
    let distance = at(&harness, back).distance(start.into());
    assert!(distance < 70.0, "on its circle about its place: {distance}");
}
