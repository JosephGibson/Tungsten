use super::*;
use crate::transition::{TransitionEffect, TransitionPhase};
use std::cell::RefCell;
use std::rc::Rc;

type Hooks = Rc<RefCell<Vec<&'static str>>>;

struct TestState {
    id: StateId,
    hooks: Hooks,
    spawn_on_enter: bool,
}

impl TestState {
    fn new(id: StateId, hooks: Hooks, spawn_on_enter: bool) -> Self {
        Self {
            id,
            hooks,
            spawn_on_enter,
        }
    }

    fn record(&self, label: &'static str) {
        self.hooks.borrow_mut().push(label);
    }
}

impl GameState for TestState {
    fn id(&self) -> StateId {
        self.id
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        self.record(hook_label(self.id, "on_enter"));
        if self.spawn_on_enter {
            let buf = ctx
                .world
                .get_resource_mut::<CommandBuffer>()
                .expect("CommandBuffer resource missing");
            let pending = buf.spawn();
            buf.insert_pending(pending, SceneEntity { state_id: self.id });
        }
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {
        self.record(hook_label(self.id, "on_exit"));
    }

    fn on_pause(&mut self, _ctx: &mut StateContext) {
        self.record(hook_label(self.id, "on_pause"));
    }

    fn on_resume(&mut self, _ctx: &mut StateContext) {
        self.record(hook_label(self.id, "on_resume"));
    }

    fn update(&mut self, _world: &mut World) {
        self.record(hook_label(self.id, "update"));
    }
}

fn hook_label(id: StateId, slot: &'static str) -> &'static str {
    match (id, slot) {
        ("menu", "on_enter") => "menu:on_enter",
        ("menu", "on_exit") => "menu:on_exit",
        ("menu", "on_pause") => "menu:on_pause",
        ("menu", "on_resume") => "menu:on_resume",
        ("menu", "update") => "menu:update",
        ("gameplay", "on_enter") => "gameplay:on_enter",
        ("gameplay", "on_exit") => "gameplay:on_exit",
        ("gameplay", "on_pause") => "gameplay:on_pause",
        ("gameplay", "on_resume") => "gameplay:on_resume",
        ("gameplay", "update") => "gameplay:update",
        ("pause", "on_enter") => "pause:on_enter",
        ("pause", "on_exit") => "pause:on_exit",
        ("pause", "on_pause") => "pause:on_pause",
        ("pause", "on_resume") => "pause:on_resume",
        ("pause", "update") => "pause:update",
        _ => "unknown",
    }
}

fn make_world() -> World {
    let mut world = World::new();
    world.insert_resource(StateStack::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(HudActiveState::default());
    world.insert_resource(Time::new());
    world
}

fn flush(world: &mut World) {
    let buf = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(buf);
    world.insert_resource(CommandBuffer::new());
}

fn scene_entity_count(world: &World, id: StateId) -> usize {
    world
        .query::<(Entity, &SceneEntity)>()
        .filter(|(_, marker)| marker.state_id == id)
        .count()
}

#[test]
fn push_fires_on_pause_then_on_enter() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = make_world();
    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_push(TestState::new("menu", hooks.clone(), false));
    state_dispatcher_system(&mut world);
    flush(&mut world);

    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_push(TestState::new("gameplay", hooks.clone(), false));
    hooks.borrow_mut().clear();
    state_dispatcher_system(&mut world);
    flush(&mut world);

    let recorded = hooks.borrow().clone();
    let enter_idx = recorded
        .iter()
        .position(|&s| s == "gameplay:on_enter")
        .expect("gameplay on_enter fired");
    let pause_idx = recorded
        .iter()
        .position(|&s| s == "menu:on_pause")
        .expect("menu on_pause fired");
    assert!(pause_idx < enter_idx, "on_pause must run before on_enter");
}

#[test]
fn pop_fires_on_exit_then_on_resume() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = make_world();
    {
        let stack = world.get_resource_mut::<StateStack>().unwrap();
        stack.request_push(TestState::new("menu", hooks.clone(), false));
        stack.request_push(TestState::new("gameplay", hooks.clone(), false));
    }
    state_dispatcher_system(&mut world);
    flush(&mut world);

    hooks.borrow_mut().clear();
    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_pop();
    state_dispatcher_system(&mut world);
    flush(&mut world);

    let recorded = hooks.borrow().clone();
    let exit_idx = recorded.iter().position(|&s| s == "gameplay:on_exit");
    let resume_idx = recorded.iter().position(|&s| s == "menu:on_resume");
    assert!(exit_idx.is_some(), "exit fired");
    assert!(resume_idx.is_some(), "resume fired");
    assert!(exit_idx.unwrap() < resume_idx.unwrap());
}

#[test]
fn replace_fires_on_exit_then_on_enter() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = make_world();
    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_push(TestState::new("menu", hooks.clone(), false));
    state_dispatcher_system(&mut world);
    flush(&mut world);

    hooks.borrow_mut().clear();
    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_replace(TestState::new("gameplay", hooks.clone(), false));
    state_dispatcher_system(&mut world);
    flush(&mut world);

    let recorded = hooks.borrow().clone();
    let exit_idx = recorded
        .iter()
        .position(|&s| s == "menu:on_exit")
        .expect("menu on_exit fired");
    let enter_idx = recorded
        .iter()
        .position(|&s| s == "gameplay:on_enter")
        .expect("gameplay on_enter fired");
    assert!(exit_idx < enter_idx);
}

#[test]
fn scene_entities_despawn_on_exit_through_command_buffer() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = make_world();
    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_push(TestState::new("gameplay", hooks.clone(), true));
    state_dispatcher_system(&mut world);
    flush(&mut world);
    assert_eq!(scene_entity_count(&world, "gameplay"), 1);

    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_pop();
    state_dispatcher_system(&mut world);
    flush(&mut world);
    assert_eq!(scene_entity_count(&world, "gameplay"), 0);
}

#[test]
fn push_does_not_despawn_paused_states_scene_entities() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = make_world();
    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_push(TestState::new("gameplay", hooks.clone(), true));
    state_dispatcher_system(&mut world);
    flush(&mut world);
    assert_eq!(scene_entity_count(&world, "gameplay"), 1);

    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_push(TestState::new("pause", hooks.clone(), true));
    state_dispatcher_system(&mut world);
    flush(&mut world);

    assert_eq!(scene_entity_count(&world, "gameplay"), 1);
    assert_eq!(scene_entity_count(&world, "pause"), 1);
}

#[test]
fn update_only_runs_on_top_state() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = make_world();
    {
        let stack = world.get_resource_mut::<StateStack>().unwrap();
        stack.request_push(TestState::new("gameplay", hooks.clone(), false));
        stack.request_push(TestState::new("pause", hooks.clone(), false));
    }
    state_dispatcher_system(&mut world);
    flush(&mut world);
    hooks.borrow_mut().clear();

    state_dispatcher_system(&mut world);
    flush(&mut world);

    let recorded = hooks.borrow().clone();
    assert!(recorded.contains(&"pause:update"));
    assert!(!recorded.contains(&"gameplay:update"));
}

#[test]
fn hud_active_state_mirrors_top_state_id() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = make_world();
    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_push(TestState::new("menu", hooks.clone(), false));
    state_dispatcher_system(&mut world);
    flush(&mut world);
    assert_eq!(world.get_resource::<HudActiveState>().unwrap().0, "menu");

    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_replace(TestState::new("gameplay", hooks.clone(), false));
    state_dispatcher_system(&mut world);
    flush(&mut world);
    assert_eq!(
        world.get_resource::<HudActiveState>().unwrap().0,
        "gameplay"
    );
}

#[test]
fn hud_active_state_cleared_when_stack_empty() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = make_world();
    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_push(TestState::new("menu", hooks.clone(), false));
    state_dispatcher_system(&mut world);
    flush(&mut world);

    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_pop();
    state_dispatcher_system(&mut world);
    flush(&mut world);

    assert!(world.get_resource::<HudActiveState>().unwrap().0.is_empty());
}

fn fade(secs: f32) -> Transition {
    Transition::new(
        TransitionEffect::Fade {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        secs,
    )
}

/// One frame: advance the clock by `dt`, run the dispatcher, flush commands.
fn dispatch(world: &mut World, dt: f32) {
    world.get_resource_mut::<Time>().unwrap().advance_frame(dt);
    state_dispatcher_system(world);
    flush(world);
}

fn stack(world: &World) -> &StateStack {
    world.get_resource::<StateStack>().unwrap()
}

fn stack_mut(world: &mut World) -> &mut StateStack {
    world.get_resource_mut::<StateStack>().unwrap()
}

fn count(hooks: &Hooks, label: &str) -> usize {
    hooks.borrow().iter().filter(|&&s| s == label).count()
}

/// A world whose stack holds `menu`, entered through a plain push.
fn world_in_menu(hooks: &Hooks) -> World {
    let mut world = make_world();
    stack_mut(&mut world).request_push(TestState::new("menu", hooks.clone(), false));
    dispatch(&mut world, 0.0);
    world
}

#[test]
fn transition_defers_command_until_out_completes() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    assert!(
        stack_mut(&mut world).request_replace_transition(
            TestState::new("gameplay", hooks.clone(), false),
            fade(0.5),
        )
    );
    assert_eq!(stack(&world).active_id(), Some("menu"));

    dispatch(&mut world, 0.25);
    assert_eq!(stack(&world).active_id(), Some("menu"));
    assert!(stack(&world).is_transitioning());
    assert_eq!(
        stack(&world).transition_state().map(|s| s.phase),
        Some(TransitionPhase::Out)
    );

    dispatch(&mut world, 0.125);
    assert_eq!(stack(&world).active_id(), Some("menu"));

    dispatch(&mut world, 0.125);
    assert_eq!(stack(&world).active_id(), Some("gameplay"));
    assert_eq!(
        stack(&world).transition_state().map(|s| s.phase),
        Some(TransitionPhase::In)
    );
}

#[test]
fn clock_a_transition_finishes_over_a_paused_clock() {
    // Transitions advance on the real clock (`D-129`): a paused game clock
    // still lets a fade finish.
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    world.get_resource_mut::<Time>().unwrap().pause();
    stack_mut(&mut world)
        .request_replace_transition(TestState::new("gameplay", hooks.clone(), false), fade(0.25));

    // Out, the boundary, In and two idle frames, all with no game time.
    for _ in 0..8 {
        dispatch(&mut world, 0.125);
        assert_eq!(world.get_resource::<Time>().unwrap().game_delta(), 0.0);
    }
    assert!(!stack(&world).is_transitioning());
    assert_eq!(stack(&world).active_id(), Some("gameplay"));
    assert_eq!(count(&hooks, "gameplay:on_enter"), 1);
}

#[test]
fn transition_applies_command_once_at_boundary() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    stack_mut(&mut world)
        .request_replace_transition(TestState::new("gameplay", hooks.clone(), false), fade(0.25));

    // Out, the boundary, In and two idle frames.
    for _ in 0..8 {
        dispatch(&mut world, 0.125);
    }
    assert!(!stack(&world).is_transitioning());
    assert_eq!(count(&hooks, "menu:on_exit"), 1);
    assert_eq!(count(&hooks, "gameplay:on_enter"), 1);
    assert_eq!(stack(&world).depth(), 1);
}

#[test]
fn replace_transition_fires_exit_then_enter_at_boundary() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    stack_mut(&mut world)
        .request_replace_transition(TestState::new("gameplay", hooks.clone(), false), fade(0.5));
    dispatch(&mut world, 0.25);
    assert_eq!(count(&hooks, "menu:on_exit"), 0);
    assert_eq!(count(&hooks, "gameplay:on_enter"), 0);

    hooks.borrow_mut().clear();
    dispatch(&mut world, 0.25);
    let recorded = hooks.borrow().clone();
    let exit_idx = recorded
        .iter()
        .position(|&s| s == "menu:on_exit")
        .expect("menu on_exit fired at the boundary");
    let enter_idx = recorded
        .iter()
        .position(|&s| s == "gameplay:on_enter")
        .expect("gameplay on_enter fired at the boundary");
    assert!(exit_idx < enter_idx);
}

#[test]
fn pop_transition_despawns_scene_entities_at_boundary() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    stack_mut(&mut world).request_push(TestState::new("gameplay", hooks.clone(), true));
    dispatch(&mut world, 0.0);
    assert_eq!(scene_entity_count(&world, "gameplay"), 1);

    assert!(stack_mut(&mut world).request_pop_transition(fade(0.5)));
    dispatch(&mut world, 0.25);
    assert_eq!(scene_entity_count(&world, "gameplay"), 1);
    assert_eq!(count(&hooks, "gameplay:on_exit"), 0);

    dispatch(&mut world, 0.25);
    assert_eq!(scene_entity_count(&world, "gameplay"), 0);
    assert_eq!(count(&hooks, "gameplay:on_exit"), 1);
    assert_eq!(count(&hooks, "menu:on_resume"), 1);
    assert_eq!(stack(&world).active_id(), Some("menu"));
}

#[test]
fn cover_rises_through_out_and_falls_through_in() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    assert_eq!(stack(&world).transition_cover(), 0.0);
    stack_mut(&mut world)
        .request_replace_transition(TestState::new("gameplay", hooks.clone(), false), fade(0.5));

    let mut covers = Vec::new();
    for dt in [0.0, 0.25, 0.25, 0.25, 0.25] {
        dispatch(&mut world, dt);
        covers.push(stack(&world).transition_cover());
    }
    // Out at 0 and 0.5, the fully covered boundary frame, In at 0.5, idle.
    assert_eq!(covers, [0.0, 0.5, 1.0, 0.5, 0.0]);
    assert!(!stack(&world).is_transitioning());
    assert_eq!(stack(&world).transition_state(), None);
}

#[test]
fn transition_pass_is_none_when_idle() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    assert_eq!(stack(&world).transition_pass(), None);

    // Queued but not yet activated: nothing to draw.
    stack_mut(&mut world)
        .request_replace_transition(TestState::new("gameplay", hooks.clone(), false), fade(0.25));
    assert!(stack(&world).is_transitioning());
    assert_eq!(stack(&world).transition_pass(), None);

    dispatch(&mut world, 0.125);
    match stack(&world).transition_pass() {
        Some(PostPass::Fade(params)) => assert_eq!(params.progress, 0.5),
        other => panic!("expected a fade pass, got {other:?}"),
    }

    dispatch(&mut world, 0.125);
    dispatch(&mut world, 0.25);
    assert_eq!(stack(&world).transition_pass(), None);
}

#[test]
fn second_transition_request_is_rejected() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    assert!(
        stack_mut(&mut world).request_replace_transition(
            TestState::new("gameplay", hooks.clone(), false),
            fade(0.25),
        )
    );
    // Rejected while the first is queued.
    assert!(
        !stack_mut(&mut world)
            .request_push_transition(TestState::new("pause", hooks.clone(), false), fade(0.25),)
    );

    // Rejected while it is active, in both phases.
    dispatch(&mut world, 0.125);
    assert!(!stack_mut(&mut world).request_pop_transition(fade(0.25)));
    dispatch(&mut world, 0.125);
    assert!(
        !stack_mut(&mut world)
            .request_replace_transition(TestState::new("pause", hooks.clone(), false), fade(0.25),)
    );

    dispatch(&mut world, 0.25);
    assert!(!stack(&world).is_transitioning());
    assert_eq!(stack(&world).active_id(), Some("gameplay"));
    assert_eq!(
        count(&hooks, "pause:on_enter"),
        0,
        "dropped states never enter"
    );

    // Accepted again once idle.
    assert!(stack_mut(&mut world).request_pop_transition(fade(0.25)));
}

#[test]
fn plain_request_applies_during_transition() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    stack_mut(&mut world)
        .request_replace_transition(TestState::new("gameplay", hooks.clone(), false), fade(1.0));
    dispatch(&mut world, 0.25);

    stack_mut(&mut world).request_push(TestState::new("pause", hooks.clone(), false));
    dispatch(&mut world, 0.25);
    assert_eq!(stack(&world).active_id(), Some("pause"));
    assert_eq!(stack(&world).depth(), 2);
    assert_eq!(
        stack(&world).transition_state().map(|s| s.phase),
        Some(TransitionPhase::Out),
        "the transition keeps running"
    );
}

#[test]
fn zero_duration_transition_applies_on_first_dispatch() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    stack_mut(&mut world)
        .request_replace_transition(TestState::new("gameplay", hooks.clone(), false), fade(0.0));
    dispatch(&mut world, 0.0);

    assert_eq!(stack(&world).active_id(), Some("gameplay"));
    assert!(!stack(&world).is_transitioning());
    assert_eq!(stack(&world).transition_pass(), None);
    assert_eq!(count(&hooks, "gameplay:on_enter"), 1);
}

#[test]
fn top_state_updates_during_transition() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    stack_mut(&mut world)
        .request_replace_transition(TestState::new("gameplay", hooks.clone(), false), fade(0.5));

    // Out: the old state keeps updating.
    hooks.borrow_mut().clear();
    dispatch(&mut world, 0.25);
    assert_eq!(*hooks.borrow(), ["menu:update"]);

    // The boundary frame updates the new state.
    hooks.borrow_mut().clear();
    dispatch(&mut world, 0.25);
    assert_eq!(
        *hooks.borrow(),
        ["menu:on_exit", "gameplay:on_enter", "gameplay:update"]
    );

    // In: the new state keeps updating.
    hooks.borrow_mut().clear();
    dispatch(&mut world, 0.125);
    assert_eq!(*hooks.borrow(), ["gameplay:update"]);
}
