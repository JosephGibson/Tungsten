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
fn a_pop_after_a_push_in_one_frame_waits_and_takes_the_pushed_spawns() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    stack_mut(&mut world).request_push(TestState::new("pause", hooks.clone(), true));
    stack_mut(&mut world).request_pop();
    dispatch(&mut world, 0.0);
    assert_eq!(stack(&world).active_id(), Some("pause"), "the pop waits");
    assert_eq!(count(&hooks, "pause:update"), 1);
    assert_eq!(scene_entity_count(&world, "pause"), 1);

    dispatch(&mut world, 0.0);
    assert_eq!(stack(&world).active_id(), Some("menu"));
    assert_eq!(count(&hooks, "pause:on_exit"), 1);
    assert_eq!(scene_entity_count(&world, "pause"), 0, "no spawn outlives");
}

#[test]
fn a_second_replace_in_one_frame_waits_with_the_commands_after_it() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    stack_mut(&mut world).request_replace(TestState::new("gameplay", hooks.clone(), true));
    stack_mut(&mut world).request_replace(TestState::new("pause", hooks.clone(), true));
    stack_mut(&mut world).request_push(TestState::new("menu", hooks.clone(), false));
    dispatch(&mut world, 0.0);
    assert_eq!(stack(&world).active_id(), Some("gameplay"));
    assert_eq!(stack(&world).depth(), 1);

    dispatch(&mut world, 0.0);
    assert_eq!(stack(&world).active_id(), Some("menu"), "order kept");
    assert_eq!(stack(&world).depth(), 2);
    assert_eq!(scene_entity_count(&world, "gameplay"), 0);
    assert_eq!(scene_entity_count(&world, "pause"), 1);
}

#[test]
fn a_pop_then_a_push_in_one_frame_both_apply() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    stack_mut(&mut world).request_pop();
    stack_mut(&mut world).request_push(TestState::new("gameplay", hooks.clone(), false));
    dispatch(&mut world, 0.0);
    assert_eq!(stack(&world).active_id(), Some("gameplay"));
    assert_eq!(stack(&world).depth(), 1);
}

#[test]
fn a_boundary_pop_after_a_plain_push_waits_a_frame() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = world_in_menu(&hooks);
    assert!(stack_mut(&mut world).request_pop_transition(fade(0.0)));
    stack_mut(&mut world).request_push(TestState::new("pause", hooks.clone(), true));
    dispatch(&mut world, 0.0);
    assert_eq!(stack(&world).active_id(), Some("pause"));
    assert!(!stack(&world).is_transitioning());

    dispatch(&mut world, 0.0);
    assert_eq!(stack(&world).active_id(), Some("menu"));
    assert_eq!(scene_entity_count(&world, "pause"), 0);
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

// ---- M56: state-owned UI roots ----

use std::cell::Cell;
use tungsten_core::ui::{LayoutStyle, UiLayer, UiTree, Visibility, WidgetId};

/// A state that adds and owns one root in `on_enter`, or two with `pair`
/// (a `Hide` root, then a `Show` one), recording whether its first root
/// was still in the tree at `on_exit`.
struct RootState {
    id: StateId,
    cover: RootCover,
    roots: Rc<RefCell<Vec<WidgetId>>>,
    present_at_exit: Rc<Cell<Option<bool>>>,
}

impl RootState {
    fn new(id: StateId, cover: RootCover) -> Self {
        Self {
            id,
            cover,
            roots: Rc::default(),
            present_at_exit: Rc::default(),
        }
    }
}

impl GameState for RootState {
    fn id(&self) -> StateId {
        self.id
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let root = ctx
            .world
            .resource_mut::<UiTree>()
            .add_root(UiLayer::Game, LayoutStyle::default());
        ctx.world
            .resource_mut::<StateStack>()
            .own_root(root, self.cover);
        self.roots.borrow_mut().push(root);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        let root = self.roots.borrow()[0];
        self.present_at_exit
            .set(Some(ctx.world.resource::<UiTree>().contains(root)));
    }

    fn update(&mut self, _world: &mut World) {}
}

fn ui_world() -> World {
    let mut world = make_world();
    world.insert_resource(UiTree::new());
    world
}

fn tree(world: &World) -> &UiTree {
    world.resource::<UiTree>()
}

/// Pushes `state` with a plain request and dispatches; returns its roots.
fn push_roots(world: &mut World, state: RootState) -> Rc<RefCell<Vec<WidgetId>>> {
    let roots = state.roots.clone();
    stack_mut(world).request_push(state);
    dispatch(world, 0.0);
    roots
}

#[test]
fn a_root_owned_in_on_enter_is_removed_after_on_exit_by_a_pop_and_a_replace() {
    let mut world = ui_world();
    let state = RootState::new("menu", RootCover::Hide);
    let (roots, present) = (state.roots.clone(), state.present_at_exit.clone());
    stack_mut(&mut world).request_push(state);
    dispatch(&mut world, 0.0);
    let root = roots.borrow()[0];
    assert!(tree(&world).contains(root));
    stack_mut(&mut world).request_pop();
    dispatch(&mut world, 0.0);
    assert_eq!(present.get(), Some(true), "on_exit still sees it");
    assert!(!tree(&world).contains(root), "then it goes");

    let state = RootState::new("menu", RootCover::Hide);
    let (roots, present) = (state.roots.clone(), state.present_at_exit.clone());
    push_roots(&mut world, state);
    let root = roots.borrow()[0];
    let next = RootState::new("gameplay", RootCover::Hide);
    let next_roots = next.roots.clone();
    stack_mut(&mut world).request_replace(next);
    dispatch(&mut world, 0.0);
    assert_eq!(present.get(), Some(true));
    assert!(!tree(&world).contains(root));
    let replacement = next_roots.borrow()[0];
    assert!(tree(&world).contains(replacement));
    assert!(!tree(&world).frozen(replacement), "the new top is live");
}

#[test]
fn push_hides_or_freezes_by_cover_and_pop_restores_each_visibility() {
    let mut world = ui_world();
    let play = push_roots(&mut world, RootState::new("gameplay", RootCover::Show));
    let shown = play.borrow()[0];
    // Two more roots for the top state, owned from outside any hook.
    let (hidden, game_hidden) = {
        let tree = world.resource_mut::<UiTree>();
        let a = tree.add_root(UiLayer::Game, LayoutStyle::default());
        let b = tree.add_root(UiLayer::Game, LayoutStyle::default());
        tree.set_visibility(b, Visibility::Hidden);
        (a, b)
    };
    stack_mut(&mut world).own_root(hidden, RootCover::Hide);
    stack_mut(&mut world).own_root(game_hidden, RootCover::Hide);
    dispatch(&mut world, 0.0);
    assert!(!tree(&world).frozen(shown));

    let pause = push_roots(&mut world, RootState::new("pause", RootCover::Hide));
    let pause_root = pause.borrow()[0];
    let ui = tree(&world);
    assert_eq!(ui.visibility(shown), Some(Visibility::Visible));
    assert!(ui.frozen(shown), "a Show root stays shown and frozen");
    assert_eq!(ui.visibility(hidden), Some(Visibility::Hidden));
    assert_eq!(ui.visibility(game_hidden), Some(Visibility::Hidden));
    assert!(!ui.frozen(pause_root));

    stack_mut(&mut world).request_pop();
    dispatch(&mut world, 0.0);
    let ui = tree(&world);
    assert!(!ui.contains(pause_root));
    assert!(!ui.frozen(shown));
    assert_eq!(ui.visibility(hidden), Some(Visibility::Visible));
    assert_eq!(
        ui.visibility(game_hidden),
        Some(Visibility::Hidden),
        "a root the game hid stays hidden"
    );
}

#[test]
fn a_root_owned_outside_any_hook_belongs_to_the_top_state() {
    let mut world = ui_world();
    push_roots(&mut world, RootState::new("menu", RootCover::Hide));
    push_roots(&mut world, RootState::new("pause", RootCover::Hide));
    let root = world
        .resource_mut::<UiTree>()
        .add_root(UiLayer::Game, LayoutStyle::default());
    stack_mut(&mut world).own_root(root, RootCover::Hide);
    assert_eq!(stack(&world).owned_roots.last().map(|o| o.owner), Some(1));
    stack_mut(&mut world).request_pop();
    dispatch(&mut world, 0.0);
    assert!(!tree(&world).contains(root), "it left with the top state");

    // An empty stack owns nothing.
    let mut empty = ui_world();
    let root = empty
        .resource_mut::<UiTree>()
        .add_root(UiLayer::Game, LayoutStyle::default());
    stack_mut(&mut empty).own_root(root, RootCover::Show);
    assert!(stack(&empty).owned_roots.is_empty());
}

#[test]
fn two_instances_of_one_state_id_each_lose_only_their_own_roots() {
    let mut world = ui_world();
    let first = push_roots(&mut world, RootState::new("menu", RootCover::Hide));
    let second = push_roots(&mut world, RootState::new("menu", RootCover::Hide));
    let (a, b) = (first.borrow()[0], second.borrow()[0]);
    assert_eq!(tree(&world).visibility(a), Some(Visibility::Hidden));
    stack_mut(&mut world).request_pop();
    dispatch(&mut world, 0.0);
    assert!(!tree(&world).contains(b));
    assert!(
        tree(&world).contains(a),
        "the lower instance keeps its root"
    );
    assert_eq!(tree(&world).visibility(a), Some(Visibility::Visible));
}

#[test]
fn a_fade_transition_freezes_every_owned_root_and_fades_it_with_the_cover() {
    let mut world = ui_world();
    let play = push_roots(&mut world, RootState::new("gameplay", RootCover::Show));
    let shown = play.borrow()[0];
    let unowned = world
        .resource_mut::<UiTree>()
        .add_root(UiLayer::Game, LayoutStyle::default());
    let pause = RootState::new("pause", RootCover::Hide);
    let pause_roots = pause.roots.clone();
    assert!(stack_mut(&mut world).request_push_transition(pause, fade(1.0)));
    // Queued only: frozen at fade 1 from the next sync.
    ui_state_roots_system(&mut world);
    assert!(tree(&world).frozen(shown));
    assert_eq!(tree(&world).fade(shown), Some(1.0));
    let mut saw_pause = false;
    for _ in 0..100 {
        dispatch(&mut world, 1.0 / 32.0);
        let cover = stack(&world).transition_cover();
        let transitioning = stack(&world).is_transitioning();
        let mut owned = vec![shown];
        owned.extend(pause_roots.borrow().iter().copied());
        saw_pause |= !pause_roots.borrow().is_empty();
        for root in owned {
            let ui = tree(&world);
            if transitioning {
                assert!(ui.frozen(root));
                assert_eq!(ui.fade(root), Some(1.0 - cover));
            } else {
                assert_eq!(ui.fade(root), Some(1.0));
            }
        }
        assert!(!tree(&world).frozen(unowned), "a root no state owns");
        assert_eq!(tree(&world).fade(unowned), Some(1.0));
    }
    assert!(saw_pause);
    assert!(!stack(&world).is_transitioning());
    let pause_root = pause_roots.borrow()[0];
    assert!(!tree(&world).frozen(pause_root), "the new top thaws");
    assert!(
        tree(&world).frozen(shown),
        "the covered Show root stays frozen"
    );
}

#[test]
fn with_no_owned_roots_a_dispatch_leaves_the_tree_untouched() {
    let hooks: Hooks = Rc::new(RefCell::new(Vec::new()));
    let mut world = ui_world();
    let root = world
        .resource_mut::<UiTree>()
        .add_root(UiLayer::Game, LayoutStyle::default());
    world.resource_mut::<UiTree>().clear_paint_dirty();
    stack_mut(&mut world).request_push(TestState::new("menu", hooks.clone(), false));
    stack_mut(&mut world)
        .request_push_transition(TestState::new("pause", hooks.clone(), false), fade(0.25));
    for _ in 0..12 {
        dispatch(&mut world, 1.0 / 32.0);
        let ui = tree(&world);
        assert!(!ui.is_paint_dirty());
        assert!(!ui.frozen(root));
        assert_eq!(ui.fade(root), Some(1.0));
    }
    assert!(stack(&world).owned_roots.is_empty());
}

type Reads = Rc<RefCell<Vec<(&'static str, Option<StateId>, usize)>>>;

/// Records what `active_id` and `depth` read inside each of its hooks.
struct ReadingState {
    id: StateId,
    reads: Reads,
}

impl ReadingState {
    fn read(&self, hook: &'static str, world: &World) {
        let stack = world.get_resource::<StateStack>().unwrap();
        self.reads
            .borrow_mut()
            .push((hook, stack.active_id(), stack.depth()));
    }
}

impl GameState for ReadingState {
    fn id(&self) -> StateId {
        self.id
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        self.read("on_enter", ctx.world);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        self.read("on_exit", ctx.world);
    }

    fn on_pause(&mut self, ctx: &mut StateContext) {
        self.read("on_pause", ctx.world);
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        self.read("on_resume", ctx.world);
    }

    fn update(&mut self, world: &mut World) {
        self.read("update", world);
    }
}

#[test]
fn a_hook_reads_its_own_state_as_active_at_its_own_depth() {
    let reads: Reads = Rc::new(RefCell::new(Vec::new()));
    let mut world = make_world();
    let state = |id| ReadingState {
        id,
        reads: reads.clone(),
    };
    stack_mut(&mut world).request_push(state("menu"));
    dispatch(&mut world, 0.0);
    stack_mut(&mut world).request_push(state("pause"));
    dispatch(&mut world, 0.0);
    stack_mut(&mut world).request_pop();
    dispatch(&mut world, 0.0);

    assert_eq!(
        *reads.borrow(),
        vec![
            ("on_enter", Some("menu"), 1),
            ("update", Some("menu"), 1),
            ("on_pause", Some("menu"), 1),
            ("on_enter", Some("pause"), 2),
            ("update", Some("pause"), 2),
            ("on_exit", Some("pause"), 2),
            ("on_resume", Some("menu"), 1),
            ("update", Some("menu"), 1),
        ]
    );
    assert_eq!(stack(&world).active_id(), Some("menu"));
    assert_eq!(stack(&world).depth(), 1);
}
