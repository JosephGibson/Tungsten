use super::*;
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
    world
}

fn flush(world: &mut World) {
    let buf = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(buf);
    world.insert_resource(CommandBuffer::new());
}

fn scene_entity_count(world: &World, id: StateId) -> usize {
    world
        .query::<SceneEntity>()
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
