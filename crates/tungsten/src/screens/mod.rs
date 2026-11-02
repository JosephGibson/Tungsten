//! Modal screens over the state stack (redo 7.17): the template shell's
//! screen plumbing in the engine. A [`ModalScreen`] opens in a state's
//! `on_enter` as a scrim and a card ([`widgets::screen`]), owned by its
//! state and slid in; its `update` pops it on its root's `Cancel`, so a
//! back button pops it, and hands the state each key activated on it.
//! [`push`], [`pop`] and [`reset`] change the stack one change at a time: a
//! push acts at once, a pop after its slide, a reset behind a fade in the
//! theme's background. [`Confirm`] asks a question with a safe answer,
//! [`toast`] shows a short message above every screen, and [`register`]
//! and [`open`] name screens for scripts and data; [`open`] also pushes a
//! `ui_screens` file as a [`DataScreen`] (redo 7.20), which runs the file's
//! actions.
//!
//! Opt in with [`ScreensPlugin`], which inserts the resources and adds no
//! system: each screen's own `update` drives its slide out and its
//! answers, and the UI's animation tracks time the toasts.

mod confirm;
mod data;
mod toast;

use tungsten_core::ui::{UiEvent, UiKey, UiScreenRegistry, UiTrack, UiTree, WidgetId, widgets};
use tungsten_core::{Easing, EventQueue, Plugin, Schedule, UiRouter, World};

use crate::state::{GameState, RootCover, StateCommand, StateStack};
use crate::transition::{Transition, TransitionEffect};

pub use confirm::{CONFIRM, Confirm, ask};
pub use data::DataScreen;
pub use toast::{TOAST_SECS, Toasts, toast};

/// How long a pushed screen fades and slides in, in real seconds.
pub const PUSH_SECS: f32 = 0.10;
/// How long a closing screen fades and slides out before it pops.
pub const POP_SECS: f32 = 0.08;
/// How far a screen slides, in UI units: up into place, down out of it.
pub const SLIDE_UNITS: f32 = 6.0;
/// A reset's fade out, before the stack changes under it, and its fade in.
pub const RESET_SECS: [f32; 2] = [0.10, 0.14];

/// The tag a closing screen's slide out ends with.
const POP_TAG: u32 = u32::from_le_bytes(*b"pop!");

/// The screens' state, a `World` resource [`ScreensPlugin`] inserts.
#[derive(Debug, Default)]
pub struct Screens {
    /// The root sliding out, which pops when its slide ends.
    closing: Option<WidgetId>,
}

/// A screen made by name: what [`register`] keeps.
pub type MakeScreen = fn() -> Box<dyn GameState>;

/// The screens a game named, a `World` resource [`ScreensPlugin`] inserts.
#[derive(Default)]
pub struct ScreenRegistry {
    screens: Vec<(&'static str, MakeScreen)>,
}

impl ScreenRegistry {
    /// The registered names, in registration order.
    #[must_use]
    pub fn ids(&self) -> Vec<&'static str> {
        self.screens.iter().map(|(id, _)| *id).collect()
    }
}

/// Opts a game into [`tungsten::screens`](self): inserts [`Screens`],
/// [`ScreenRegistry`] and [`Toasts`] when the world lacks them. It adds no
/// system.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, _schedule: &mut Schedule, world: &mut World) {
        insert_resources(world);
    }
}

/// Inserts [`Screens`], [`ScreenRegistry`] and [`Toasts`] where the world
/// lacks them.
fn insert_resources(world: &mut World) {
    if !world.has_resource::<Screens>() {
        world.insert_resource(Screens::default());
    }
    if !world.has_resource::<ScreenRegistry>() {
        world.insert_resource(ScreenRegistry::default());
    }
    if !world.has_resource::<Toasts>() {
        world.insert_resource(Toasts::default());
    }
}

/// Whether a screen change waits or runs: a stack change not yet applied or
/// a transition, or a screen sliding out.
#[must_use]
pub fn transitioning(world: &World) -> bool {
    world
        .get_resource::<Screens>()
        .is_some_and(|screens| screens.closing.is_some())
        || world
            .get_resource::<StateStack>()
            .is_some_and(StateStack::requests_waiting)
}

/// Whether a screen may act: no screen change waits or runs and no rebind
/// is capturing.
#[must_use]
pub fn idle(world: &World) -> bool {
    let capturing = world
        .get_resource::<UiRouter>()
        .is_some_and(|router| router.capturing().is_some());
    !(transitioning(world) || capturing)
}

/// Pushes `state` over the top one, at once; `false`, and the state
/// dropped, unless the screens are [`idle`].
pub fn push(world: &mut World, state: impl GameState) -> bool {
    push_boxed(world, Box::new(state))
}

fn push_boxed(world: &mut World, state: Box<dyn GameState>) -> bool {
    if !idle(world) {
        return false;
    }
    world
        .resource_mut::<StateStack>()
        .pending
        .push(StateCommand::Push(state));
    true
}

/// Closes the front-most shown modal screen: its root fades and slides out
/// over [`POP_SECS`] on the real clock, and its [`ModalScreen::update`]
/// pops its state when the slide ends. Without a modal root the top state
/// pops at once. `false` unless the screens are [`idle`].
pub fn pop(world: &mut World) -> bool {
    if !idle(world) {
        return false;
    }
    let tree = world.resource_mut::<UiTree>();
    let root = tree.roots().iter().rev().copied().find(|root| {
        tree.modal(*root)
            && tree
                .visibility(*root)
                .is_some_and(tungsten_core::ui::Visibility::is_visible)
    });
    let Some(root) = root else {
        world.resource_mut::<StateStack>().request_pop();
        return true;
    };
    tree.animate(root, UiTrack::opacity(1.0, 0.0, POP_SECS).with_tag(POP_TAG));
    let to = [0.0, SLIDE_UNITS].into();
    tree.animate(root, UiTrack::translation([0.0, 0.0].into(), to, POP_SECS));
    if let Some(screens) = world.get_resource_mut::<Screens>() {
        screens.closing = Some(root);
    }
    true
}

/// Resets the stack to `state` behind a fade in the theme's background,
/// [`RESET_SECS`] out and in: on the fully covered frame every state exits,
/// top down, `on_swap` runs, then `state` enters. Owned roots fade with it;
/// toasts, unowned, stay above. `false`, and the state dropped, unless the
/// screens are [`idle`].
pub fn reset(
    world: &mut World,
    state: impl GameState,
    on_swap: impl FnOnce(&mut World) + 'static,
) -> bool {
    if !idle(world) {
        return false;
    }
    let [r, g, b, a] = world.resource::<UiTree>().theme().tokens.colors.background;
    let channel = |value: u8| f32::from(value) / 255.0;
    let effect = TransitionEffect::Fade {
        color: [channel(r), channel(g), channel(b), channel(a)],
    };
    let mut transition = Transition::new(effect, RESET_SECS[0]);
    transition.in_secs = RESET_SECS[1];
    world
        .resource_mut::<StateStack>()
        .request_reset_transition_with(state, transition, on_swap)
}

/// Names a screen `id` for [`open`], a script's `screen` step and
/// `tungsten render --screen`; callable from a plugin's `build`. A second
/// registration of an ID replaces the first.
pub fn register(world: &mut World, id: &'static str, make: MakeScreen) {
    if !world.has_resource::<ScreenRegistry>() {
        world.insert_resource(ScreenRegistry::default());
    }
    let registry = world.resource_mut::<ScreenRegistry>();
    registry.screens.retain(|(name, _)| *name != id);
    registry.screens.push((id, make));
}

/// Pushes the screen registered as `id`, else the `ui_screens` file `id`
/// as a [`DataScreen`]; `false` when [`refusal`] gives a reason, a file a
/// data screen refuses warning with it. Inserts [`Screens`],
/// [`ScreenRegistry`] and [`Toasts`] where the world lacks them, as
/// [`ScreensPlugin`] does, so a script opens a file in a game without the
/// plugin.
pub fn open(world: &mut World, id: &str) -> bool {
    insert_resources(world);
    match opening(world, id) {
        Ok(Some(make)) => push_boxed(world, make()),
        Ok(None) => {
            let state = world.resource_mut::<UiScreenRegistry>().intern(id);
            state.is_some_and(|state| push_boxed(world, Box::new(DataScreen::new(state))))
        }
        Err(reason) => {
            let file = world.get_resource::<UiScreenRegistry>();
            if file.is_some_and(|registry| registry.contains(id)) {
                log::warn!("{reason}");
            }
            false
        }
    }
}

/// Why [`open`] would not open `id` now: neither a registered screen nor a
/// loaded `ui_screens` file, a file whose root is not modal on the popup
/// layer, which screens close front-most first, or a screen change that
/// waits or runs. `None` when it would open.
#[must_use]
pub fn refusal(world: &World, id: &str) -> Option<String> {
    match opening(world, id) {
        Err(reason) => Some(reason),
        Ok(_) if !idle(world) => Some("a screen change waits or runs".to_owned()),
        Ok(_) => None,
    }
}

/// What [`open`] makes of `id`: a registered screen's maker, `None` for a
/// screen file a data screen opens, or why neither.
fn opening(world: &World, id: &str) -> Result<Option<MakeScreen>, String> {
    let made = world.get_resource::<ScreenRegistry>().and_then(|registry| {
        registry
            .screens
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, make)| *make)
    });
    if made.is_some() {
        return Ok(made);
    }
    let file = world
        .get_resource::<UiScreenRegistry>()
        .and_then(|registry| registry.get(id));
    let Some(file) = file else {
        return Err(format!(
            "no registered screen or ui_screens file is named '{id}'"
        ));
    };
    match file.data_screen_refusal() {
        Some(why) => Err(format!("ui screen '{id}': {why}")),
        None => Ok(None),
    }
}

/// A state's modal screen (redo 7.17): its root and card, and the control
/// that opened the screen above it.
#[derive(Debug, Clone, Copy)]
pub struct ModalScreen {
    /// The scrim root, owned by the state.
    pub root: WidgetId,
    /// The card to build into.
    pub card: WidgetId,
    /// The widget last activated on it, which takes focus back.
    opener: Option<WidgetId>,
}

impl ModalScreen {
    /// Opens a modal screen `width` units wide from a state's `on_enter`:
    /// [`widgets::screen`]'s scrim and card, [adopted](Self::adopt). Build
    /// into [`Self::card`], then call [`Self::resume`] to focus the first
    /// control.
    pub fn open(world: &mut World, width: f32) -> Self {
        let tree = world.resource_mut::<UiTree>();
        let (root, card) = widgets::screen(tree, width);
        Self::adopt(world, root, card)
    }

    /// Adopts a modal root built elsewhere, a screen file's say, and its
    /// card: owned by the state and shown while covered
    /// ([`RootCover::Show`]), faded and slid in over [`PUSH_SECS`], with the
    /// toasts raised above it.
    pub fn adopt(world: &mut World, root: WidgetId, card: WidgetId) -> Self {
        let tree = world.resource_mut::<UiTree>();
        let ease = |track: UiTrack| track.with_easing(Easing::CubicOut);
        tree.animate(root, ease(UiTrack::opacity(0.0, 1.0, PUSH_SECS)));
        let from = [0.0, SLIDE_UNITS].into();
        tree.animate(
            root,
            ease(UiTrack::translation(from, [0.0, 0.0].into(), PUSH_SECS)),
        );
        world
            .resource_mut::<StateStack>()
            .own_root(root, RootCover::Show);
        toast::raise(world);
        Self {
            root,
            card,
            opener: None,
        }
    }

    /// The screen's turn, from its state's `update`: pops the state once
    /// its slide out ends; `None` while the screens are not [`idle`]; pops
    /// on its root's `Cancel`, so a back button pops it; else the key of
    /// the control on it activated at this routing, which becomes the
    /// opener.
    pub fn update(&mut self, world: &mut World) -> Option<UiKey> {
        if !self.ready(world) {
            return None;
        }
        let events = world.resource::<EventQueue<UiEvent>>();
        let tree = world.resource::<UiTree>();
        let activated = events.iter_current().find_map(|event| match *event {
            UiEvent::Activated {
                widget,
                key: Some(key),
                ..
            } if tree.root_of(widget) == Some(self.root) => Some((widget, key)),
            _ => None,
        });
        let (widget, key) = activated?;
        self.opener = Some(widget);
        Some(key)
    }

    /// Whether the screen may act at this routing: not while its slide out
    /// runs, whose end pops its state, nor while the screens are not
    /// [`idle`]; a `Cancel` on its root pops it instead.
    fn ready(&self, world: &mut World) -> bool {
        let events = world.resource::<EventQueue<UiEvent>>();
        let closing = world
            .get_resource::<Screens>()
            .is_some_and(|screens| screens.closing == Some(self.root));
        if closing {
            if events.animation_finished(self.root, POP_TAG) {
                world.resource_mut::<Screens>().closing = None;
                world.resource_mut::<StateStack>().request_pop();
            }
            return false;
        }
        if !idle(world) {
            return false;
        }
        let cancelled = events
            .iter_current()
            .any(|event| matches!(event, UiEvent::Cancel { scope, .. } if *scope == self.root));
        if cancelled {
            pop(world);
            return false;
        }
        true
    }

    /// Focuses the control that opened the screen above, else the first
    /// that can take focus: from the state's `on_resume`, and after
    /// building the screen.
    pub fn resume(&self, world: &mut World) {
        let tree = world.resource_mut::<UiTree>();
        let first = self
            .opener
            .filter(|opener| tree.can_focus(*opener))
            .or_else(|| first_focusable(tree, self.root));
        if let Some(widget) = first {
            tree.focus(widget);
        }
    }
}

/// The first widget under `root`, depth first, that can take focus.
fn first_focusable(tree: &UiTree, root: WidgetId) -> Option<WidgetId> {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if tree.can_focus(id) {
            return Some(id);
        }
        stack.extend(tree.children(id).iter().rev());
    }
    None
}

#[cfg(test)]
#[path = "../tests/screens.rs"]
mod tests;
