//! Scene/state system (M20, `D-046`).
//!
//! A single `state_dispatcher_system` drives a `StateStack` of
//! `Box<dyn GameState>`. Deferred `request_push` / `request_pop` /
//! `request_replace` requests queue on the stack and are drained by the
//! dispatcher once per frame, firing `on_pause` / `on_exit` / `on_enter` /
//! `on_resume` per the transition matrix in the M20 plan.
//!
//! Entities spawned with a `SceneEntity { state_id }` marker during
//! `on_enter` are auto-despawned (through `CommandBuffer`, per `D-039`) when
//! that state exits. Pause doesn't auto-despawn: `push` fires `on_pause` on
//! the old top, not `on_exit`.

use tungsten_core::{CommandBuffer, World};

use crate::debug_hud::HudActiveState;

/// Stable string identifier for a state. Static lifetime so trait objects
/// can return it without allocation.
pub type StateId = &'static str;

/// Argument passed to `on_enter` / `on_exit` / `on_pause` / `on_resume`.
/// Mirrors the state's id so hooks can spawn `SceneEntity { state_id }`
/// markers without capturing the state's `self`.
pub struct StateContext<'a> {
    pub world: &'a mut World,
    pub state_id: StateId,
}

/// Marker inserted on every entity owned by a state. The dispatcher walks
/// this component on state exit and queues a `CommandBuffer::despawn` for
/// each matching entity before the user's `on_exit` runs.
#[derive(Debug, Clone, Copy)]
pub struct SceneEntity {
    pub state_id: StateId,
}

/// Trait implemented by game states. `on_pause` / `on_resume` are no-op
/// defaults so Pause can overlay Gameplay without tearing the scene down.
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

/// World resource: the authoritative state stack plus a pending-command
/// queue that `request_*` methods append to. The dispatcher drains the
/// queue once per frame.
pub struct StateStack {
    pub(crate) stack: Vec<Box<dyn GameState>>,
    pub(crate) pending: Vec<StateCommand>,
}

impl StateStack {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// Queue a push. Fires `on_pause` on the current top, then `on_enter`
    /// on the new state.
    pub fn request_push(&mut self, state: impl GameState) {
        self.pending.push(StateCommand::Push(Box::new(state)));
    }

    /// Queue a pop. Fires `on_exit` on the current top (auto-despawning its
    /// `SceneEntity`s first), then `on_resume` on the uncovered state.
    pub fn request_pop(&mut self) {
        self.pending.push(StateCommand::Pop);
    }

    /// Queue a replace. Fires `on_exit` on the current top (auto-despawning
    /// its `SceneEntity`s first), then `on_enter` on the new state.
    pub fn request_replace(&mut self, state: impl GameState) {
        self.pending.push(StateCommand::Replace(Box::new(state)));
    }

    /// Identifier of the state on top of the stack, if any.
    pub fn active_id(&self) -> Option<StateId> {
        self.stack.last().map(|s| s.id())
    }

    /// Number of states on the stack.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

impl Default for StateStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Enqueue a `CommandBuffer::despawn` for every entity carrying a
/// `SceneEntity { state_id }` matching `id`. The actual removal happens at
/// the next command-buffer flush — callers relying on post-despawn queries
/// must run a flush between this call and the query.
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

/// Engine-owned system: drains `StateStack.pending`, fires lifecycle hooks,
/// calls `update` on the current top, and mirrors the active id into
/// `HudActiveState` so consumers (custom HUD rows, external panels) can
/// read the active state without reaching into `StateStack`.
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
