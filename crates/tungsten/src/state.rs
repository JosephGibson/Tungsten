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
//!
//! Redo 7.11: a state names its music ([`GameState::music`]), and the
//! top-most state on the stack that names music is heard, at its sound's
//! manifest level. Every other music state's voice plays on at 0, keeping
//! its place in the song; each change fades over the incoming state's fade,
//! and a state's voice stops when it exits, over its own. The dispatcher
//! applies the rule only on a frame whose commands change the stack.

use std::borrow::Cow;

use tungsten_core::assets::SoundRegistry;
use tungsten_core::post::PostPass;
use tungsten_core::ui::{UiTree, Visibility, WidgetId};
use tungsten_core::{AudioCommands, CommandBuffer, Entity, Time, Voice, World};

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

    /// Whether the game clock holds while another state covers this one
    /// (redo 7.17): gameplay under a pause screen. The stack sets `Time`'s
    /// hold, beside the game's own pause, at each dispatch.
    fn holds_clock(&self) -> bool {
        false
    }

    /// The music this state plays (redo 7.11): heard while it is the
    /// top-most state that names music, at 0 under another that does, and
    /// stopped at its exit. Read when the state enters; `None`, the default,
    /// leaves the music to the states below, as a settings screen over the
    /// title keeps the title's song.
    fn music(&self) -> Option<StateMusic> {
        None
    }
}

/// A state's music (redo 7.11): the manifest sound it plays and the
/// seconds each change to it fades over, which its exit's stop takes too.
#[derive(Debug, Clone, PartialEq)]
pub struct StateMusic {
    sound: Cow<'static, str>,
    fade: f32,
}

impl StateMusic {
    /// The manifest's sound `sound`, faded over `fade` seconds; a fade that
    /// is not finite or is negative is 0.
    #[must_use]
    pub fn new(sound: impl Into<Cow<'static, str>>, fade: f32) -> Self {
        Self {
            sound: sound.into(),
            fade: if fade.is_finite() { fade.max(0.0) } else { 0.0 },
        }
    }

    /// The sound's manifest ID.
    #[must_use]
    pub fn sound(&self) -> &str {
        &self.sound
    }

    /// The fade, in seconds.
    #[must_use]
    pub fn fade(&self) -> f32 {
        self.fade
    }
}

/// A music state's voice (redo 7.11), by its owner's place on the stack.
pub(crate) struct MusicVoice {
    owner: usize,
    /// `None` when the sound did not resolve: it plays nothing.
    voice: Option<Voice>,
    /// The sound's manifest level, which the heard voice plays at.
    level: f32,
    fade: f32,
    heard: bool,
}

/// A reset's hook, run between the exits and the enter.
type SwapHook = Box<dyn FnOnce(&mut World)>;

/// What removes an owned resource from the World.
pub(crate) type RemoveResource = fn(&mut World);

pub(crate) enum StateCommand {
    Push(Box<dyn GameState>),
    Pop,
    Replace(Box<dyn GameState>),
    /// Every state exits, top down, the hook runs, then the state enters
    /// (redo 7.17).
    Reset(Box<dyn GameState>, Option<SwapHook>),
}

impl StateCommand {
    /// Whether the command exits the top state.
    fn exits_top(&self) -> bool {
        matches!(self, Self::Pop | Self::Replace(_) | Self::Reset(..))
    }

    /// How many states at the top of the stack the dispatch has entered once
    /// this command applies, from `entered` before it; a pop applies only
    /// when that is none.
    fn entered_after(&self, entered: usize) -> usize {
        match self {
            Self::Push(_) | Self::Replace(_) | Self::Reset(..) => entered + 1,
            Self::Pop => entered,
        }
    }
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
    /// The stack index and ID of the state whose hook or `update` runs, off
    /// the stack; `None` outside the dispatcher.
    pub(crate) dispatching: Option<(usize, StateId)>,
    /// The resources states own (redo 7.17), by the owner's place on the
    /// stack, each with what removes it.
    pub(crate) owned_resources: Vec<(usize, RemoveResource)>,
    /// The music states' voices (redo 7.11).
    pub(crate) music: Vec<MusicVoice>,
    /// The music IDs warned of as unknown, each once.
    pub(crate) music_warned: Vec<String>,
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
            owned_resources: Vec::new(),
            music: Vec::new(),
            music_warned: Vec::new(),
        }
    }

    /// Gives resource `T` to a state (redo 7.17): from a state's hook or
    /// `update`, the state being dispatched; elsewhere the top state. `T`
    /// leaves the World when that state exits, after its `on_exit`, as its
    /// owned roots do. With an empty stack this warns and does nothing.
    pub fn own_resource<T: 'static>(&mut self) {
        let owner = match self.dispatching {
            Some((index, _)) => index,
            None if !self.stack.is_empty() => self.stack.len() - 1,
            None => {
                log::warn!(
                    "StateStack::own_resource::<{}>: no state to own it",
                    std::any::type_name::<T>()
                );
                return;
            }
        };
        self.owned_resources.push((owner, remove_resource::<T>));
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
            Some((index, _)) => index,
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

    /// Queue a reset behind `transition` (redo 7.17): on its boundary frame
    /// every state exits, top down, as pops exit them (hooks, scene entities
    /// and owned roots and resources), then `state` enters. `false`, and the
    /// state is dropped, when a transition is queued or active.
    pub fn request_reset_transition(
        &mut self,
        state: impl GameState,
        transition: Transition,
    ) -> bool {
        self.queue_transition(StateCommand::Reset(Box::new(state), None), transition)
    }

    /// [`Self::request_reset_transition`], running `on_swap` between the
    /// exits and the enter: where a run change stages the next run.
    pub fn request_reset_transition_with(
        &mut self,
        state: impl GameState,
        transition: Transition,
        on_swap: impl FnOnce(&mut World) + 'static,
    ) -> bool {
        let command = StateCommand::Reset(Box::new(state), Some(Box::new(on_swap)));
        self.queue_transition(command, transition)
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

    /// Whether a request waits: a command not yet applied, or held from the
    /// last dispatch, or a transition queued or active. A script's stage
    /// asks for its states only when none waits (redo 7.2).
    pub(crate) fn requests_waiting(&self) -> bool {
        !self.pending.is_empty() || self.is_transitioning()
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

    /// Active state ID: inside a state's hook or `update`, which run with
    /// the state off the stack, that state's.
    #[must_use]
    pub fn active_id(&self) -> Option<StateId> {
        match self.dispatching {
            Some((_, id)) => Some(id),
            None => self.stack.last().map(|s| s.id()),
        }
    }

    /// Stack depth: inside a state's hook or `update`, that state's depth
    /// with it on the stack.
    #[must_use]
    pub fn depth(&self) -> usize {
        match self.dispatching {
            Some((index, _)) => index + 1,
            None => self.stack.len(),
        }
    }
}

impl Default for StateStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Queue despawn for entities owned by `id`, and the live particles whose
/// emitter is one of them (redo 7.9), so a title's stars leave with the
/// title; removal waits for command flush.
pub fn despawn_scene_entities(world: &mut World, id: StateId) {
    let mut targets: Vec<_> = world
        .query::<(Entity, &SceneEntity)>()
        .filter_map(|(entity, marker)| (marker.state_id == id).then_some(entity))
        .collect();
    if targets.is_empty() {
        return;
    }
    let owned: std::collections::HashSet<Entity> = targets.iter().copied().collect();
    targets.extend(
        world
            .query::<(Entity, &tungsten_core::Particle)>()
            .filter(|(entity, particle)| {
                !owned.contains(entity) && particle.emitter.is_some_and(|e| owned.contains(&e))
            })
            .map(|(entity, _)| entity),
    );
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
///
/// A command that would exit a state entered earlier in the same dispatch
/// (a push then a pop, two replaces) waits for the next frame, with every
/// command after it, so the entered state runs one frame and its spawns
/// through the `CommandBuffer` are live when its exit despawns them.
pub fn state_dispatcher_system(world: &mut World) {
    let pending: Vec<StateCommand> = match world.get_resource_mut::<StateStack>() {
        Some(stack) => std::mem::take(&mut stack.pending),
        None => return,
    };

    let mut entered = 0;
    let mut held = Vec::new();
    let mut changed = false;
    for cmd in pending {
        changed |= apply_or_hold(world, cmd, &mut entered, &mut held);
    }

    changed |= advance_transition(world, &mut entered, &mut held);

    if !held.is_empty()
        && let Some(stack) = world.get_resource_mut::<StateStack>()
    {
        // Ahead of the requests this dispatch's hooks made.
        held.append(&mut stack.pending);
        stack.pending = held;
    }

    if changed {
        sync_music(world);
    }
    sync_clock_hold(world);

    if let Some(mut top) = pop_top(world) {
        top.update(world);
        push_top(world, top);
    }

    sync_state_roots(world, 0);

    // Rewritten in place only when the top state changes (debug-pass), so a
    // frame that keeps its state allocates nothing here.
    let active = world
        .get_resource::<StateStack>()
        .and_then(|s| s.stack.last().map(|t| t.id()))
        .unwrap_or("");
    if let Some(slot) = world.get_resource_mut::<HudActiveState>() {
        if slot.0 != active {
            slot.0.clear();
            slot.0.push_str(active);
        }
    } else {
        world.insert_resource(HudActiveState(active.to_owned()));
    }
}

/// Applies `cmd`, or holds it for the next frame when a command is already
/// held or it would exit a state this dispatch entered; `entered` counts the
/// states at the top of the stack the dispatch has entered. Whether it
/// applied.
fn apply_or_hold(
    world: &mut World,
    cmd: StateCommand,
    entered: &mut usize,
    held: &mut Vec<StateCommand>,
) -> bool {
    if !held.is_empty() || (*entered > 0 && cmd.exits_top()) {
        held.push(cmd);
        return false;
    }
    *entered = cmd.entered_after(*entered);
    apply_command(world, cmd);
    true
}

/// Activate the queued transition when none is active, then advance the
/// active one. The stored command runs through `apply_or_hold` on the frame
/// `Out` completes, so hooks, scene despawn and order follow `D-046`.
/// Whether that command applied.
fn advance_transition(
    world: &mut World,
    entered: &mut usize,
    held: &mut Vec<StateCommand>,
) -> bool {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::real_delta);
    let Some(stack) = world.get_resource_mut::<StateStack>() else {
        return false;
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
        return false;
    };

    let transition = active.transition;
    let mut step = transition.advance(&mut active.state, dt);
    let mut command = None;
    if step == TransitionStep::Boundary {
        // `In` starts before the hooks run, so they see a fully covered frame.
        step = transition.begin_in(&mut active.state);
        command = active.command.take();
    }

    let applied = command.is_some_and(|command| apply_or_hold(world, command, entered, held));
    if step == TransitionStep::Finished
        && let Some(stack) = world.get_resource_mut::<StateStack>()
    {
        stack.active_transition = None;
    }
    applied
}

/// The music rule (redo 7.11), after a dispatch changed the stack: each
/// state that entered naming music starts its voice, and the heard state,
/// the top-most naming music, plays at its level while every other plays
/// at 0, each change over the heard state's fade. A sound the registry
/// lacks warns once by its ID and plays nothing. Exits stopped their voices
/// already ([`release_roots`]).
fn sync_music(world: &mut World) {
    let Some(stack) = world.get_resource_mut::<StateStack>() else {
        return;
    };
    let mut music = std::mem::take(&mut stack.music);
    let mut entered: Vec<(usize, StateMusic)> = Vec::new();
    for (index, state) in stack.stack.iter().enumerate() {
        if !music.iter().any(|voice| voice.owner == index)
            && let Some(named) = state.music()
        {
            entered.push((index, named));
        }
    }
    let owners = music.iter().map(|voice| voice.owner);
    let Some(heard) = owners.chain(entered.iter().map(|(index, _)| *index)).max() else {
        stack.music = music;
        return;
    };
    let fade = entered
        .iter()
        .find(|(index, _)| *index == heard)
        .map(|(_, named)| named.fade)
        .or_else(|| {
            music
                .iter()
                .find(|voice| voice.owner == heard)
                .map(|voice| voice.fade)
        })
        .unwrap_or(0.0);

    let sounds = world.get_resource::<SoundRegistry>();
    let resolved: Vec<_> = entered
        .into_iter()
        .map(|(index, named)| {
            let sound = sounds.and_then(|sounds| {
                let handle = sounds.get_by_id(named.sound())?;
                Some((
                    handle,
                    sounds.get_volume(handle),
                    sounds.get_looping(handle),
                ))
            });
            (index, named, sound)
        })
        .collect();

    let mut audio = world.get_resource_mut::<AudioCommands>();
    for voice in &mut music {
        let heard_now = voice.owner == heard;
        if voice.heard == heard_now {
            continue;
        }
        voice.heard = heard_now;
        if let (Some(playing), Some(audio)) = (voice.voice, audio.as_deref_mut()) {
            let level = if heard_now { voice.level } else { 0.0 };
            audio.set_voice_volume(playing, level, fade);
        }
    }
    let mut unknown = Vec::new();
    for (index, named, sound) in resolved {
        let heard_now = index == heard;
        let (voice, level) =
            if let (Some((handle, level, looping)), Some(audio)) = (sound, audio.as_deref_mut()) {
                let (volume, rise) = if heard_now {
                    (level, named.fade)
                } else {
                    (0.0, 0.0)
                };
                (Some(audio.play_faded(handle, volume, looping, rise)), level)
            } else {
                unknown.push(named.sound.clone());
                (None, 0.0)
            };
        music.push(MusicVoice {
            owner: index,
            voice,
            level,
            fade: named.fade,
            heard: heard_now,
        });
    }

    let Some(stack) = world.get_resource_mut::<StateStack>() else {
        return;
    };
    stack.music = music;
    for sound in unknown {
        if !stack.music_warned.iter().any(|warned| *warned == *sound) {
            log::warn!("A state's music '{sound}' is not a loaded sound; it plays nothing");
            stack.music_warned.push(sound.into_owned());
        }
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

/// Holds the game clock while a covered state holds its clock (redo
/// 7.17): `Time`'s hold, beside the game's pause, from the next frame on.
fn sync_clock_hold(world: &mut World) {
    let Some(stack) = world.get_resource::<StateStack>() else {
        return;
    };
    let covered = stack.stack.len().saturating_sub(1);
    let held = stack.stack[..covered]
        .iter()
        .any(|state| state.holds_clock());
    if let Some(time) = world.get_resource_mut::<Time>() {
        time.set_held(held);
    }
}

/// Removes resource `T`: what [`StateStack::own_resource`] keeps.
fn remove_resource<T: 'static>(world: &mut World) {
    world.remove_resource::<T>();
}

/// Removes the roots and resources the state at `index` owns, after its
/// exit, stops its music's voice over its fade (redo 7.11), and ends its
/// dispatch.
fn release_roots(world: &mut World, index: usize) {
    let Some(stack) = world.get_resource_mut::<StateStack>() else {
        return;
    };
    stack.dispatching = None;
    if let Some(at) = stack.music.iter().position(|voice| voice.owner == index) {
        let gone = stack.music.remove(at);
        if let (Some(voice), Some(audio)) = (gone.voice, world.get_resource_mut::<AudioCommands>())
        {
            audio.stop_voice(voice, gone.fade);
        }
    }
    let Some(stack) = world.get_resource_mut::<StateStack>() else {
        return;
    };
    if !stack.owned_resources.is_empty() {
        let mut removers = Vec::new();
        stack.owned_resources.retain(|&(owner, remove)| {
            if owner == index {
                removers.push(remove);
            }
            owner != index
        });
        for remove in removers {
            remove(world);
        }
    }
    let Some(stack) = world.get_resource_mut::<StateStack>() else {
        return;
    };
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

/// Marks the state `id`, about to enter at the top, as the one being
/// dispatched.
fn dispatch_next(world: &mut World, id: StateId) {
    let stack = world
        .get_resource_mut::<StateStack>()
        .expect("StateStack removed mid-dispatch");
    stack.dispatching = Some((stack.stack.len(), id));
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
            dispatch_next(world, new_id);
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
            dispatch_next(world, new_id);
            new_state.on_enter(&mut StateContext {
                world,
                state_id: new_id,
            });
            push_top(world, new_state);
        }
        StateCommand::Reset(mut new_state, on_swap) => {
            while let Some(mut old) = pop_top(world) {
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
            sync_state_roots(world, 0);
            if let Some(on_swap) = on_swap {
                on_swap(world);
            }
            let new_id = new_state.id();
            dispatch_next(world, new_id);
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
    stack.dispatching = Some((stack.stack.len(), top.id()));
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
