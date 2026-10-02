//! D-046 state stack; D-039 scene despawns go through `CommandBuffer`.
//!
//! Push pauses old top; pop/replace exits and auto-despawns matching `SceneEntity`s.
//!
//! M31 (`D-093`): a `request_*_transition` call holds its command behind a
//! screen transition. The dispatcher runs the command on the frame the `Out`
//! phase completes, with the same hooks and despawns as a plain request.

use tungsten_core::post::PostPass;
use tungsten_core::{CommandBuffer, DeltaTime, World};

use crate::debug_hud::HudActiveState;
use crate::transition::{Transition, TransitionState, TransitionStep};

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

/// A transition requested this frame, waiting for the dispatcher.
pub(crate) struct QueuedTransition {
    pub(crate) transition: Transition,
    pub(crate) command: StateCommand,
}

/// The running transition; `command` is taken on the boundary frame.
pub(crate) struct ActiveTransition {
    pub(crate) transition: Transition,
    pub(crate) command: Option<StateCommand>,
    pub(crate) state: TransitionState,
}

/// State stack resource plus pending command queue and screen transition.
pub struct StateStack {
    pub(crate) stack: Vec<Box<dyn GameState>>,
    pub(crate) pending: Vec<StateCommand>,
    pub(crate) queued_transition: Option<QueuedTransition>,
    pub(crate) active_transition: Option<ActiveTransition>,
}

impl StateStack {
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            pending: Vec::new(),
            queued_transition: None,
            active_transition: None,
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

    /// Queue `push` behind `transition`. `false`, and the state is dropped,
    /// when a transition is queued or active.
    pub fn request_push_transition(
        &mut self,
        state: impl GameState,
        transition: Transition,
    ) -> bool {
        self.queue_transition(StateCommand::Push(Box::new(state)), transition)
    }

    /// Queue `pop` behind `transition`. `false` when a transition is queued or
    /// active.
    pub fn request_pop_transition(&mut self, transition: Transition) -> bool {
        self.queue_transition(StateCommand::Pop, transition)
    }

    /// Queue `replace` behind `transition`. `false`, and the state is dropped,
    /// when a transition is queued or active.
    pub fn request_replace_transition(
        &mut self,
        state: impl GameState,
        transition: Transition,
    ) -> bool {
        self.queue_transition(StateCommand::Replace(Box::new(state)), transition)
    }

    fn queue_transition(&mut self, command: StateCommand, transition: Transition) -> bool {
        if self.is_transitioning() {
            return false;
        }
        self.queued_transition = Some(QueuedTransition {
            transition,
            command,
        });
        true
    }

    /// A transition is queued or active. States keep updating during one; a
    /// game that wants input frozen checks this.
    #[must_use]
    pub fn is_transitioning(&self) -> bool {
        self.queued_transition.is_some() || self.active_transition.is_some()
    }

    /// Phase and elapsed time of the active transition.
    #[must_use]
    pub fn transition_state(&self) -> Option<TransitionState> {
        self.active_transition.as_ref().map(|active| active.state)
    }

    /// How far the active transition covers the frame, in [0, 1]; 0.0 when
    /// idle. Screen-space text is not covered by the transition pass, so a
    /// game fades its own text with `1 - transition_cover()`.
    #[must_use]
    pub fn transition_cover(&self) -> f32 {
        self.active_transition
            .as_ref()
            .map_or(0.0, |active| active.transition.cover(active.state))
    }

    /// The post pass the active transition draws this frame.
    #[must_use]
    pub fn transition_pass(&self) -> Option<PostPass> {
        self.active_transition
            .as_ref()
            .map(|active| active.transition.pass(active.state))
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

/// Drain plain commands, advance the screen transition, update top state,
/// mirror active ID.
///
/// Order per frame: plain requests apply at once, even during a transition;
/// a queued transition activates when none is active; the active one advances
/// by `DeltaTime` and applies its command on the boundary frame; then the top
/// state updates (the old state during `Out`, the new one during `In`).
pub fn state_dispatcher_system(world: &mut World) {
    let pending: Vec<StateCommand> = match world.get_resource_mut::<StateStack>() {
        Some(stack) => std::mem::take(&mut stack.pending),
        None => return,
    };

    for cmd in pending {
        apply_command(world, cmd);
    }

    advance_transition(world);

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

/// Activate the queued transition when none is active, then advance the
/// active one. The stored command runs through `apply_command` on the frame
/// `Out` completes, so hooks, scene despawn and order follow `D-046`.
fn advance_transition(world: &mut World) {
    let dt = world.get_resource::<DeltaTime>().map_or(0.0, |d| d.dt);
    let Some(stack) = world.get_resource_mut::<StateStack>() else {
        return;
    };
    if stack.active_transition.is_none()
        && let Some(queued) = stack.queued_transition.take()
    {
        stack.active_transition = Some(ActiveTransition {
            transition: queued.transition,
            command: Some(queued.command),
            state: TransitionState::START,
        });
    }
    let Some(active) = stack.active_transition.as_mut() else {
        return;
    };

    let transition = active.transition;
    let mut step = transition.advance(&mut active.state, dt);
    let mut command = None;
    if step == TransitionStep::Boundary {
        // `In` starts before the hooks run, so they see a fully covered frame.
        step = transition.begin_in(&mut active.state);
        command = active.command.take();
    }

    if let Some(command) = command {
        apply_command(world, command);
    }
    if step == TransitionStep::Finished
        && let Some(stack) = world.get_resource_mut::<StateStack>()
    {
        stack.active_transition = None;
    }
}

fn apply_command(world: &mut World, cmd: StateCommand) {
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
