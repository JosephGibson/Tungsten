//! D-046 state stack; D-039 scene despawns go through `CommandBuffer`.
//!
//! Push pauses old top; pop/replace exits and auto-despawns matching `SceneEntity`s.
//!
//! M60: under `StatePlugin` a prefab queued from a scene entity
//! (`CommandBuffer::spawn_prefab_from`) takes its `SceneEntity` at the
//! command flush, so it leaves with its state; one whose source an earlier
//! command despawned, as a state's exit does, is dropped.
//!
//! M31 (`D-093`): a `request_*_transition` call holds its command behind a
//! screen transition. The dispatcher runs the command on the frame the `Out`
//! phase completes, with the same hooks and despawns as a plain request.
//! Transitions advance on the real clock, so a fade finishes over a paused
//! game (`D-129`).
//!
//! M56: a state owns UI roots ([`StateStack::own_root`]), by its place on
//! the stack. Covered, a root hides or stays shown and frozen
//! ([`RootCover`]); uncovered, it is restored; its state's exit removes it.
//! While a transition is queued or active every owned root is frozen and
//! faded with the frame, since the UI draws after the transition pass.

use tungsten_core::post::PostPass;
use tungsten_core::ui::{UiTree, Visibility, WidgetId};
use tungsten_core::{CommandBuffer, Entity, Time, World};

use crate::debug_hud::HudActiveState;
use crate::transition::{Transition, TransitionState, TransitionStep};

/// Static state identifier.
pub type StateId = &'static str;

/// Lifecycle hook context.
pub struct StateContext<'a> {
    pub world: &'a mut World,
    pub state_id: StateId,
}

/// State-owned entity marker. Under [`StatePlugin`](crate::StatePlugin) a
/// prefab queued from an entity that carries one
/// ([`CommandBuffer::spawn_prefab_from`]) takes a copy, so it leaves with
/// the same state.
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

/// What a state's UI root does while another state covers its owner (M56).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RootCover {
    /// Hidden while covered; uncovered, it takes back the visibility it had
    /// when it was covered. For a screen the next one replaces.
    Hide,
    /// Shown and frozen while covered: it keeps its look, blocks the
    /// pointer and takes no input. For a game screen under a pause menu.
    Show,
}

/// A UI root a state owns, by the owner's place on the stack.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OwnedRoot {
    root: WidgetId,
    cover: RootCover,
    owner: usize,
    /// A `Hide` root's visibility when it was covered; `None` while not.
    hidden_from: Option<Visibility>,
}

/// State stack resource plus pending command queue and screen transition.
pub struct StateStack {
    pub(crate) stack: Vec<Box<dyn GameState>>,
    pub(crate) pending: Vec<StateCommand>,
    pub(crate) queued_transition: Option<QueuedTransition>,
    pub(crate) active_transition: Option<ActiveTransition>,
    /// The roots states own (M56).
    pub(crate) owned_roots: Vec<OwnedRoot>,
    /// The stack index of the state whose hook or `update` runs, off the
    /// stack; `None` outside the dispatcher.
    pub(crate) dispatching: Option<usize>,
}

impl StateStack {
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            pending: Vec::new(),
            queued_transition: None,
            active_transition: None,
            owned_roots: Vec::new(),
            dispatching: None,
        }
    }

    /// Gives `root` to a state (M56): from a state's hook or `update`, the
    /// state being dispatched; elsewhere the top state. Ownership is by the
    /// state's place on the stack, so two instances of one `StateId` keep
    /// their own roots. While another state covers the owner, `cover`
    /// decides whether the root hides or stays shown and frozen; uncovered,
    /// it is restored and thawed; the owner's pop or replace removes it from
    /// the tree after its `on_exit`. While a transition is queued or active
    /// every owned root is frozen and its fade is `1 - transition_cover()`.
    /// Owning a root again moves it. With an empty stack this warns and
    /// does nothing; a widget that is not a root warns at the next sync and
    /// is dropped.
    pub fn own_root(&mut self, root: WidgetId, cover: RootCover) {
        let owner = match self.dispatching {
            Some(index) => index,
            None if self.stack.is_empty() => {
                log::warn!("StateStack::own_root({root:?}): no state to own it; ignored");
                return;
            }
            None => self.stack.len() - 1,
        };
        self.owned_roots.retain(|owned| owned.root != root);
        self.owned_roots.push(OwnedRoot {
            root,
            cover,
            owner,
            hidden_from: None,
        });
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
        .query::<(Entity, &SceneEntity)>()
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
/// sync the owned UI roots, mirror active ID.
///
/// Order per frame: plain requests apply at once, even during a transition;
/// a queued transition activates when none is active; the active one advances
/// by the real dt ([`Time::real_delta`]) and applies its command on the
/// boundary frame; then the top state updates (the old state during `Out`,
/// the new one during `In`); then the owned roots take this frame's freeze
/// and fade (M56).
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

    sync_state_roots(world, 0);

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
    let dt = world.get_resource::<Time>().map_or(0.0, Time::real_delta);
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

/// Syncs the owned UI roots (M56) for the UI's routing: a transition an
/// `Update` system queued after the dispatcher freezes them before the next
/// frame's routing. The umbrella's `UiPlugin` registers it as
/// `ui_state_roots`, before `ui_route`; the dispatcher runs the same sync at
/// its end. Returns at once when no state owns a root.
pub fn ui_state_roots_system(world: &mut World) {
    sync_state_roots(world, 0);
}

/// Applies the cover, restore, freeze and fade rules to every owned root,
/// as though `incoming` more states sat on the stack (1 while a push's
/// covered state has paused and the new one has not entered), and drops
/// the entries whose root is gone or is not a root.
fn sync_state_roots(world: &mut World, incoming: usize) {
    let Some(stack) = world.get_resource_mut::<StateStack>() else {
        return;
    };
    if stack.owned_roots.is_empty() {
        return;
    }
    let transitioning = stack.is_transitioning();
    let fade = 1.0 - stack.transition_cover();
    let depth = stack.stack.len() + incoming;
    let mut owned = std::mem::take(&mut stack.owned_roots);
    if let Some(tree) = world.get_resource_mut::<UiTree>() {
        owned.retain_mut(|entry| sync_root(tree, entry, depth, transitioning, fade));
    }
    if let Some(stack) = world.get_resource_mut::<StateStack>() {
        stack.owned_roots = owned;
    }
}

/// One owned root's sync; `false` drops the entry.
fn sync_root(
    tree: &mut UiTree,
    entry: &mut OwnedRoot,
    depth: usize,
    transitioning: bool,
    fade: f32,
) -> bool {
    let root = entry.root;
    if !tree.contains(root) {
        return false;
    }
    if tree.parent(root).is_some() {
        log::warn!("StateStack::own_root({root:?}): not a root; ignored");
        return false;
    }
    let covered = entry.owner + 1 < depth;
    if entry.cover == RootCover::Hide {
        if covered && entry.hidden_from.is_none() {
            entry.hidden_from = tree.visibility(root);
            tree.set_visibility(root, Visibility::Hidden);
        } else if !covered && let Some(visibility) = entry.hidden_from.take() {
            tree.set_visibility(root, visibility);
        }
    }
    tree.set_frozen(root, transitioning || covered);
    tree.set_fade(root, fade);
    true
}

/// Removes the roots the state at `index` owns, after its exit, and ends
/// its dispatch.
fn release_roots(world: &mut World, index: usize) {
    let Some(stack) = world.get_resource_mut::<StateStack>() else {
        return;
    };
    stack.dispatching = None;
    if stack.owned_roots.is_empty() {
        return;
    }
    let mut released = Vec::new();
    stack.owned_roots.retain(|owned| {
        let gone = owned.owner == index;
        if gone {
            released.push(owned.root);
        }
        !gone
    });
    if let Some(tree) = world.get_resource_mut::<UiTree>() {
        for root in released {
            tree.remove(root);
        }
    }
}

/// Marks the state about to enter at the top as the one being dispatched.
fn dispatch_next(world: &mut World) -> usize {
    let stack = world
        .get_resource_mut::<StateStack>()
        .expect("StateStack removed mid-dispatch");
    let index = stack.stack.len();
    stack.dispatching = Some(index);
    index
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
                sync_state_roots(world, 1);
            }
            let new_id = new_state.id();
            dispatch_next(world);
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
                let index = world
                    .get_resource::<StateStack>()
                    .map_or(0, |s| s.stack.len());
                release_roots(world, index);
                sync_state_roots(world, 0);
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
                let index = world
                    .get_resource::<StateStack>()
                    .map_or(0, |s| s.stack.len());
                release_roots(world, index);
            }
            let new_id = new_state.id();
            dispatch_next(world);
            new_state.on_enter(&mut StateContext {
                world,
                state_id: new_id,
            });
            push_top(world, new_state);
        }
    }
}

/// Takes the top state off the stack for a hook or its `update`, which
/// dispatches it at the index it held.
fn pop_top(world: &mut World) -> Option<Box<dyn GameState>> {
    let stack = world.get_resource_mut::<StateStack>()?;
    let top = stack.stack.pop()?;
    stack.dispatching = Some(stack.stack.len());
    Some(top)
}

/// Puts a state back on top and ends its dispatch.
fn push_top(world: &mut World, state: Box<dyn GameState>) {
    let stack = world
        .get_resource_mut::<StateStack>()
        .expect("StateStack removed mid-dispatch");
    stack.stack.push(state);
    stack.dispatching = None;
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
