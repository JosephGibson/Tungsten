use super::{Plugin, PluginSet, Schedule, ScheduleError, Stage, system};
use crate::ecs::World;

/// What each system appended when it ran, in order.
#[derive(Default)]
struct Trace(Vec<&'static str>);

fn tracer(name: &'static str) -> impl FnMut(&mut World) {
    move |world: &mut World| {
        world.get_resource_mut::<Trace>().unwrap().0.push(name);
    }
}

fn run_all(schedule: &mut Schedule) -> Vec<&'static str> {
    let mut world = World::new();
    world.insert_resource(Trace::default());
    schedule.resolve().expect("resolves");
    for stage in Stage::ALL {
        schedule.run_stage(stage, &mut world, |_, _| {});
    }
    world.remove_resource::<Trace>().unwrap().0
}

#[test]
fn stages_run_in_frame_order_and_registration_order_within_a_stage() {
    let mut schedule = Schedule::new();
    schedule.add(Stage::PostUpdate, system("post_a", tracer("post_a")));
    schedule.add(Stage::Update, system("up_a", tracer("up_a")));
    schedule.add(Stage::PreUpdate, system("pre_a", tracer("pre_a")));
    schedule.add(Stage::Update, system("up_b", tracer("up_b")));
    schedule.add(Stage::Startup, system("start", tracer("start")));
    schedule.add(Stage::FixedUpdate, system("fixed", tracer("fixed")));
    assert_eq!(
        run_all(&mut schedule),
        vec!["start", "pre_a", "fixed", "up_a", "up_b", "post_a"]
    );
}

#[test]
fn before_and_after_reorder_within_a_stage_and_ties_keep_registration_order() {
    let mut schedule = Schedule::new();
    schedule.add(Stage::Update, system("a", tracer("a")));
    schedule.add(Stage::Update, system("b", tracer("b")));
    schedule.add(Stage::Update, system("c", tracer("c")).before("a"));
    schedule.add(Stage::Update, system("d", tracer("d")).after("c"));
    // a waits for c; b is the earliest ready system, then c, then a and d by registration.
    assert_eq!(run_all(&mut schedule), vec!["b", "c", "a", "d"]);
    assert_eq!(
        schedule.resolved_text(),
        "startup: -\npre_update: -\nfixed_update: -\nupdate: b, c, a, d\npost_update: -\n"
    );
}

#[test]
fn an_unknown_name_fails_to_resolve_and_names_both_systems() {
    let mut schedule = Schedule::new();
    schedule.add(Stage::Update, system("a", tracer("a")).after("ghost"));
    let err = schedule.resolve().unwrap_err();
    assert_eq!(
        err,
        ScheduleError::UnknownName {
            system: "a".into(),
            stage: Stage::Update,
            relation: "after",
            missing: "ghost".into(),
            reason: "is not registered".into(),
        }
    );
    assert_eq!(
        err.to_string(),
        "system `a` in `update` runs after `ghost`, which is not registered"
    );
}

#[test]
fn a_constraint_across_stages_is_unknown_and_says_where_the_other_is() {
    let mut schedule = Schedule::new();
    schedule.add(Stage::PostUpdate, system("camera", tracer("camera")));
    schedule.add(Stage::Update, system("a", tracer("a")).before("camera"));
    let err = schedule.resolve().unwrap_err();
    assert_eq!(
        err.to_string(),
        "system `a` in `update` runs before `camera`, which is in `post_update`"
    );
}

#[test]
fn a_cycle_fails_to_resolve_and_names_its_path() {
    let mut schedule = Schedule::new();
    schedule.add(Stage::Update, system("a", tracer("a")).after("c"));
    schedule.add(Stage::Update, system("b", tracer("b")).after("a"));
    schedule.add(Stage::Update, system("c", tracer("c")).after("b"));
    schedule.add(Stage::Update, system("free", tracer("free")));
    let err = schedule.resolve().unwrap_err();
    assert_eq!(
        err,
        ScheduleError::Cycle {
            stage: Stage::Update,
            path: vec!["a".into(), "b".into(), "c".into(), "a".into()],
        }
    );
    assert_eq!(
        err.to_string(),
        "stage `update` has an ordering cycle: a -> b -> c -> a"
    );
}

#[test]
fn a_system_waiting_behind_a_cycle_reports_the_cycle_without_a_panic() {
    // `sink` has no unresolved successor; a walker over successors would
    // run dry on it (2.1). The cycle it waits behind is what is reported.
    let mut schedule = Schedule::new();
    schedule.add(Stage::Update, system("sink", tracer("sink")).after("a"));
    schedule.add(Stage::Update, system("a", tracer("a")).after("b"));
    schedule.add(Stage::Update, system("b", tracer("b")).after("a"));
    let err = schedule.resolve().unwrap_err();
    assert_eq!(
        err,
        ScheduleError::Cycle {
            stage: Stage::Update,
            path: vec!["a".into(), "b".into(), "a".into()],
        }
    );
    assert_eq!(
        err.to_string(),
        "stage `update` has an ordering cycle: a -> b -> a"
    );
}

#[test]
fn a_duplicate_name_fails_to_resolve() {
    let mut schedule = Schedule::new();
    schedule.add(Stage::Update, system("a", tracer("a")));
    schedule.add(Stage::PostUpdate, system("a", tracer("a")));
    assert_eq!(
        schedule.resolve().unwrap_err().to_string(),
        "system `a` is registered twice, in `update` and `post_update`"
    );
}

#[test]
fn an_if_present_constraint_applies_only_when_the_other_is_registered() {
    let mut schedule = Schedule::new();
    schedule.add(
        Stage::Update,
        system("camera", tracer("camera")).after_if_present("shake"),
    );
    assert_eq!(run_all(&mut schedule), vec!["camera"]);

    let mut schedule = Schedule::new();
    schedule.add(
        Stage::Update,
        system("camera", tracer("camera")).after_if_present("shake"),
    );
    schedule.add(Stage::Update, system("shake", tracer("shake")));
    assert_eq!(run_all(&mut schedule), vec!["shake", "camera"]);
}

#[test]
fn adding_after_resolve_needs_another_resolve() {
    let mut schedule = Schedule::new();
    schedule.add(Stage::Update, system("a", tracer("a")));
    schedule.resolve().unwrap();
    assert!(schedule.is_resolved());
    schedule.add(Stage::Update, system("b", tracer("b")).before("a"));
    assert!(!schedule.is_resolved());
    assert_eq!(run_all(&mut schedule), vec!["b", "a"]);
}

#[test]
fn run_stage_reports_each_system_by_name() {
    let mut schedule = Schedule::new();
    schedule.add(Stage::Update, system("a", tracer("a")));
    schedule.add(Stage::Update, system("b", tracer("b")));
    schedule.resolve().unwrap();
    let mut world = World::new();
    world.insert_resource(Trace::default());
    let mut seen = Vec::new();
    schedule.run_stage(Stage::Update, &mut world, |name, _| {
        seen.push(name.to_string());
    });
    assert_eq!(seen, vec!["a", "b"]);
    assert_eq!(schedule.stage_of("b"), Some(Stage::Update));
    assert!(!schedule.contains("c"));
    assert_eq!(schedule.len(), 2);
}

struct Resource(u32);

struct ResourcePlugin;

impl Plugin for ResourcePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.insert_resource(Resource(7));
        schedule.add(
            Stage::Update,
            system("bump", |world: &mut World| {
                world.get_resource_mut::<Resource>().unwrap().0 += 1;
            }),
        );
    }
}

struct EventPlugin;

#[derive(Debug, Clone, Copy)]
struct Ping;

impl Plugin for EventPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<Ping>();
        schedule.add(
            Stage::PostUpdate,
            system("ping", |world: &mut World| {
                world
                    .get_resource_mut::<crate::EventQueue<Ping>>()
                    .unwrap()
                    .send(Ping);
            }),
        );
    }
}

#[test]
fn a_plugin_set_builds_in_order_and_drops_a_removed_type() {
    let set = PluginSet::new().with(ResourcePlugin).with(EventPlugin);
    assert_eq!(set.len(), 2);
    assert!(set.contains::<EventPlugin>());
    assert_eq!(
        set.names(),
        vec![
            std::any::type_name::<ResourcePlugin>(),
            std::any::type_name::<EventPlugin>()
        ]
    );

    let set = set.without::<EventPlugin>();
    assert!(!set.contains::<EventPlugin>());
    let mut schedule = Schedule::new();
    let mut world = World::new();
    set.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    assert_eq!(
        schedule.resolved_text().lines().nth(3),
        Some("update: bump")
    );
    assert_eq!(
        schedule.resolved_text().lines().nth(4),
        Some("post_update: -")
    );
    schedule.run_stage(Stage::Update, &mut world, |_, _| {});
    assert_eq!(world.get_resource::<Resource>().unwrap().0, 8);
    assert!(!world.has_event::<Ping>());
}

#[test]
fn adding_a_plugin_twice_replaces_it_in_place() {
    let set = PluginSet::new()
        .with(ResourcePlugin)
        .with(EventPlugin)
        .with(ResourcePlugin);
    assert_eq!(set.len(), 2);
    assert_eq!(set.names()[0], std::any::type_name::<ResourcePlugin>());
}

#[test]
fn a_plugin_registers_an_event_the_world_rotates() {
    let mut schedule = Schedule::new();
    let mut world = World::new();
    PluginSet::new()
        .with(EventPlugin)
        .build(&mut schedule, &mut world);
    assert!(world.has_event::<Ping>());
    assert!(
        !world.register_event::<Ping>(),
        "a second registration is a no-op"
    );
    assert_eq!(world.registered_event_count(), 1);
    schedule.resolve().unwrap();
    schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    world.flush_events();
    let queue = world.get_resource::<crate::EventQueue<Ping>>().unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.iter_current().count(), 0);
    world.flush_events();
    assert!(
        world
            .get_resource::<crate::EventQueue<Ping>>()
            .unwrap()
            .is_empty()
    );
}
