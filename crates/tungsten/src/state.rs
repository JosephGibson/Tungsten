//! D-046 state stack; D-039 scene despawns go through `CommandBuffer`.
//!
//! Push pauses old top; pop/replace exits and auto-despawns matching `SceneEntity`s.

use tungsten_core::{CommandBuffer, World};

use crate::debug_hud::HudActiveState;

/// Static state identifier.
pub type StateId = &'static str;

/// Lifecycle hook context.
pub struct StateContext<'a> {
    pub world: &'a mut World,
    pub state_id: StateId,
}

/// State-owned entity marker.
#[derive(Debug, Clone, Copy)]
pub struct SceneEntity {
    pub state_id: StateId,
}

/// Game state lifecycle trait.
pub trait GameState: 'static {
    fn id(&self) -> StateId;
    fn on_enter(&mut self, ctx: &mut StateContext);
    fn on_exit(&mut self, ctx: &mut StateContext);
    fn on_pause(&mut self, _ctx: &mut StateContext) {}
    fn on_resume(&mut self, _ctx: &mut StateContext) {}
    fn update(&mut self, world: &mut World);
}

pub(crate) enum StateCommand {
    Push(Box<dyn GameState>),
    Pop,
    Replace(Box<dyn GameState>),
}

/// State stack resource plus pending transition queue.
pub struct StateStack {
    pub(crate) stack: Vec<Box<dyn GameState>>,
    pub(crate) pending: Vec<StateCommand>,
}

impl StateStack {
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// Queue push: pause old top, enter new state.
    pub fn request_push(&mut self, state: impl GameState) {
        self.pending.push(StateCommand::Push(Box::new(state)));
    }

    /// Queue pop: exit old top, resume uncovered state.
    pub fn request_pop(&mut self) {
        self.pending.push(StateCommand::Pop);
    }

    /// Queue replace: exit old top, enter new state.
    pub fn request_replace(&mut self, state: impl GameState) {
        self.pending.push(StateCommand::Replace(Box::new(state)));
    }

    /// Active state ID.
    #[must_use]
    pub fn active_id(&self) -> Option<StateId> {
        self.stack.last().map(|s| s.id())
    }

    /// Stack depth.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

impl Default for StateStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Queue despawn for entities owned by `id`; removal waits for command flush.
pub fn despawn_scene_entities(world: &mut World, id: StateId) {
    let targets: Vec<_> = world
        .query::<SceneEntity>()
        .filter_map(|(entity, marker)| (marker.state_id == id).then_some(entity))
        .collect();
    if targets.is_empty() {
        return;
    }
    let buf = world
        .get_resource_mut::<CommandBuffer>()
        .expect("CommandBuffer resource missing");
    for entity in targets {
        buf.despawn(entity);
    }
}

/// Drain transitions, update top state, mirror active ID.
pub fn state_dispatcher_system(world: &mut World) {
    let pending: Vec<StateCommand> = match world.get_resource_mut::<StateStack>() {
        Some(stack) => std::mem::take(&mut stack.pending),
        None => return,
    };

    for cmd in pending {
        match cmd {
            StateCommand::Push(mut new_state) => {
                if let Some(mut old) = pop_top(world) {
                    let old_id = old.id();
                    old.on_pause(&mut StateContext {
                        world,
                        state_id: old_id,
                    });
                    push_top(world, old);
                }
                let new_id = new_state.id();
                new_state.on_enter(&mut StateContext {
                    world,
                    state_id: new_id,
                });
                push_top(world, new_state);
            }
            StateCommand::Pop => {
                if let Some(mut old) = pop_top(world) {
                    let old_id = old.id();
                    despawn_scene_entities(world, old_id);
                    old.on_exit(&mut StateContext {
                        world,
                        state_id: old_id,
                    });
                }
                if let Some(mut next) = pop_top(world) {
                    let next_id = next.id();
                    next.on_resume(&mut StateContext {
                        world,
                        state_id: next_id,
                    });
                    push_top(world, next);
                }
            }
            StateCommand::Replace(mut new_state) => {
                if let Some(mut old) = pop_top(world) {
                    let old_id = old.id();
                    despawn_scene_entities(world, old_id);
                    old.on_exit(&mut StateContext {
                        world,
                        state_id: old_id,
                    });
                }
                let new_id = new_state.id();
                new_state.on_enter(&mut StateContext {
                    world,
                    state_id: new_id,
                });
                push_top(world, new_state);
            }
        }
    }

    if let Some(mut top) = pop_top(world) {
        top.update(world);
        push_top(world, top);
    }

    let active = world
        .get_resource::<StateStack>()
        .and_then(|s| s.stack.last().map(|t| t.id()))
        .unwrap_or("")
        .to_string();
    if let Some(slot) = world.get_resource_mut::<HudActiveState>() {
        slot.0 = active;
    } else {
        world.insert_resource(HudActiveState(active));
    }
}

fn pop_top(world: &mut World) -> Option<Box<dyn GameState>> {
    world.get_resource_mut::<StateStack>()?.stack.pop()
}

fn push_top(world: &mut World, state: Box<dyn GameState>) {
    world
        .get_resource_mut::<StateStack>()
        .expect("StateStack removed mid-dispatch")
        .stack
        .push(state);
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
