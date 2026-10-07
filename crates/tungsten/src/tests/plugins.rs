use super::*;
use crate::App;
use crate::testing::Harness;
use tungsten_core::{CollisionEvent, Config, EventQueue, PluginSet, Stage, system};

/// The resolved schedule of `DefaultPlugins`: the snapshot W15a's done-when
/// asks for. An engine reorder shows up here as a diff.
const DEFAULT_SCHEDULE: &str = "\
startup: -
pre_update: physics_debug_toggle, systems_overlay_toggle, inspector_toggle, inspector_pick, hud_toggle, display_input, state_dispatcher
fixed_update: physics_prev_snapshot, physics_step
update: -
post_update: physics_sync, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, camera_update
";

#[test]
fn default_plugins_resolve_to_the_snapshot() {
    let mut app = App::new(Config::default()).unwrap();
    app.resolve_schedule().unwrap();
    assert_eq!(app.schedule().resolved_text(), DEFAULT_SCHEDULE);
    assert_eq!(DefaultPlugins::set().len(), 8);
}

#[test]
fn a_game_system_lands_in_update_between_the_engine_stages() {
    let mut app = App::new(Config::default()).unwrap();
    app.add_system_named("player_movement", |_: &mut World| {});
    app.add_system(|_: &mut World| {});
    app.resolve_schedule().unwrap();
    assert_eq!(
        app.schedule().resolved_text().lines().nth(3),
        Some("update: player_movement, system_0")
    );
}

#[test]
fn a_game_orders_its_post_update_system_against_an_engine_name() {
    let mut app = App::new(Config::default()).unwrap();
    app.add_system_to(
        Stage::PostUpdate,
        system("hierarchy", |_: &mut World| {})
            .after(PHYSICS_SYNC)
            .before(CAMERA_UPDATE),
    );
    app.add_system_to(
        Stage::PostUpdate,
        system("first", |_: &mut World| {}).before(PHYSICS_SYNC),
    );
    app.resolve_schedule().unwrap();
    assert_eq!(
        app.schedule().resolved_text().lines().nth(4),
        Some(
            "post_update: first, physics_sync, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, hierarchy, camera_update"
        )
    );
}

#[test]
fn leaving_a_plugin_out_drops_its_systems_and_events_and_still_resolves() {
    let set = DefaultPlugins::set()
        .without::<PhysicsPlugin>()
        .without::<GameFeelPlugin>();
    let mut app = App::with_plugins(Config::default(), set).unwrap();
    app.resolve_schedule().unwrap();
    let text = app.schedule().resolved_text();
    assert_eq!(text.lines().nth(2), Some("fixed_update: -"));
    assert_eq!(
        text.lines().nth(4),
        Some(
            "post_update: particle_count_refresh, particle_emit, particle_tick, tween_tick, camera_update"
        )
    );
    assert!(
        app.world_mut()
            .get_resource::<EventQueue<CollisionEvent>>()
            .is_none()
    );
}

#[test]
fn an_empty_plugin_set_runs_no_engine_system() {
    let mut app = App::with_plugins(Config::default(), PluginSet::new()).unwrap();
    app.resolve_schedule().unwrap();
    assert_eq!(
        app.schedule().resolved_text(),
        "startup: -\npre_update: -\nfixed_update: -\nupdate: -\npost_update: -\n"
    );
    // The resources every engine feature reads are there regardless.
    assert!(app.world_mut().get_resource::<crate::DebugHud>().is_some());
}

#[test]
fn an_unknown_name_fails_at_startup_naming_both_systems() {
    let mut app = App::new(Config::default()).unwrap();
    app.add_system_to(
        Stage::Update,
        system("walk", |_: &mut World| {}).after("physics_step"),
    );
    let err = app.resolve_schedule().unwrap_err().to_string();
    assert_eq!(
        err,
        "Schedule: system `walk` in `update` runs after `physics_step`, which is in `fixed_update`"
    );
}

#[test]
fn a_cycle_fails_at_startup_naming_its_path() {
    let mut app = App::new(Config::default()).unwrap();
    app.add_system_to(Stage::Update, system("a", |_: &mut World| {}).after("b"));
    app.add_system_to(Stage::Update, system("b", |_: &mut World| {}).after("a"));
    assert_eq!(
        app.resolve_schedule().unwrap_err().to_string(),
        "Schedule: stage `update` has an ordering cycle: a -> b -> a"
    );
}

#[test]
#[should_panic(expected = "Harness::new: Schedule: system `a` is registered twice")]
fn the_harness_refuses_a_schedule_that_does_not_resolve() {
    let mut app = App::new(Config::default()).unwrap();
    app.add_system_named("a", |_: &mut World| {});
    app.add_system_to(Stage::PostUpdate, system("a", |_: &mut World| {}));
    let _ = Harness::new(app);
}

/// Which stage each system saw itself run in, by the frame's running count.
#[derive(Default)]
struct Seen(Vec<&'static str>);

#[test]
fn a_frame_runs_startup_once_then_the_stages_in_order() {
    let mut app = App::new(Config::default()).unwrap();
    app.world_mut().insert_resource(Seen::default());
    let mark = |tag: &'static str| {
        move |world: &mut World| world.get_resource_mut::<Seen>().unwrap().0.push(tag)
    };
    app.add_system_to(Stage::Startup, system("boot", mark("startup")));
    app.add_system_to(Stage::PostUpdate, system("late", mark("post_update")));
    app.add_system_to(Stage::Update, system("mid", mark("update")));
    app.add_system_to(Stage::PreUpdate, system("early", mark("pre_update")));
    app.add_system_to(Stage::FixedUpdate, system("fixed", mark("fixed_update")));
    let mut harness = Harness::new(app);
    harness.step(2);
    assert_eq!(
        harness.world().get_resource::<Seen>().unwrap().0,
        vec![
            "startup",
            "pre_update",
            "fixed_update",
            "update",
            "post_update",
            "pre_update",
            "fixed_update",
            "update",
            "post_update",
        ]
    );
}

#[test]
fn every_engine_system_is_timed_under_its_public_name() {
    let mut app = App::new(Config::default()).unwrap();
    let mut harness = Harness::new(app);
    harness.step(1);
    let timings = harness
        .world()
        .get_resource::<crate::FrameTimings>()
        .unwrap()
        .system_timings
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    let expected: Vec<&str> = DEFAULT_SCHEDULE
        .lines()
        .flat_map(|line| line.split_once(": ").unwrap().1.split(", "))
        .filter(|name| *name != "-")
        .collect();
    assert_eq!(timings, expected);
    app = App::with_plugins(Config::default(), PluginSet::new()).unwrap();
    assert_eq!(app.schedule().len(), 0);
}
