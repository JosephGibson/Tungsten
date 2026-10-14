//! The per-entity state machine: a state with enter, exit and update hooks,
//! visited once a fixed step.

use std::fmt;

use tungsten_core::physics::{PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP};
use tungsten_core::{Entity, Plugin, Schedule, Stage, Time, World, system};

use crate::health::HEALTH_TICK;
use crate::path::PATH_FOLLOW;
use crate::platformer::PLATFORMER_CONTROLLER;
use crate::top_down::TOP_DOWN_MOVER;

/// A hook a state runs: the world and the entity whose machine it is.
type Hook = fn(&mut World, Entity);

/// An entity's state, of the game's type `S`: a boss's phases, an enemy's
/// patrol and chase. [`StateMachinePlugin<S>`]'s system visits it once a
/// fixed step and runs the hooks the game registered for its states.
/// [`go`](Self::go) asks for the next state, which the next visit enters;
/// [`elapsed`](Self::elapsed) reads the game time spent in the current
/// one.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Entity, Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{StateMachine, StateMachinePlugin};
///
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// enum Boss {
///     Idle,
///     Charge,
/// }
///
/// /// How many charges the boss has started.
/// struct Charges(u32);
///
/// fn idle(world: &mut World, boss: Entity) {
///     let machine = world.get_mut::<StateMachine<Boss>>(boss).unwrap();
///     if machine.elapsed() >= 0.5 {
///         machine.go(Boss::Charge);
///     }
/// }
///
/// fn charge(world: &mut World, _boss: Entity) {
///     world.resource_mut::<Charges>().0 += 1;
/// }
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// world.insert_resource(Charges(0));
/// let mut schedule = Schedule::new();
/// StateMachinePlugin::<Boss>::new("boss_states")
///     .on_update(Boss::Idle, idle)
///     .on_enter(Boss::Charge, charge)
///     .build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let boss = world.spawn_with((StateMachine::new(Boss::Idle),));
/// for _ in 0..40 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// }
/// let machine = world.get::<StateMachine<Boss>>(boss).unwrap();
/// assert_eq!(machine.state(), Boss::Charge);
/// assert_eq!(machine.previous(), Some(Boss::Idle));
/// assert_eq!(world.resource::<Charges>().0, 1);
/// ```
#[derive(Debug, Clone)]
pub struct StateMachine<S> {
    state: S,
    previous: Option<S>,
    elapsed: f32,
    next: Option<S>,
    /// Its first visit has run.
    entered: bool,
}

impl<S: Copy + Eq + 'static> StateMachine<S> {
    /// A machine that enters `initial` at its first visit, unless
    /// [`go`](Self::go) asks for another state first.
    #[must_use]
    pub fn new(initial: S) -> Self {
        Self {
            state: initial,
            previous: None,
            elapsed: 0.0,
            next: None,
            entered: false,
        }
    }

    /// The current state.
    #[must_use]
    pub fn state(&self) -> S {
        self.state
    }

    /// The state before the last transition; `None` before the first.
    #[must_use]
    pub fn previous(&self) -> Option<S> {
        self.previous
    }

    /// The game time spent in the current state, in seconds, up to the end
    /// of its last visit.
    #[must_use]
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    /// Asks for `next`, which the next visit enters: the current state's
    /// exit hook runs, then `next`'s enter hook. A `go` made during a
    /// visit waits for the next step; the last call before a visit wins,
    /// and a `go` to the current state enters it again.
    pub fn go(&mut self, next: S) {
        self.next = Some(next);
    }
}

/// Registers the system that visits every [`StateMachine<S>`], under the
/// name the game passes, in `FixedUpdate`: after the history snapshot and
/// [`HEALTH_TICK`], and before [`PATH_FOLLOW`], both controllers and
/// `physics_step`, each when registered. So an update hook that writes a
/// body's `Velocity` moves it in the same step, and a paused game, which
/// runs no step, holds every machine; a step of 0 visits nothing.
///
/// Each step it visits the machines in entity order. A first visit enters
/// the state `go` asked for, else the initial one, and runs its enter hook.
/// A later visit with a pending `go` runs the old state's exit hook, then
/// the new state's enter hook, and resets `elapsed`. Every visit then runs
/// the current state's update hook and adds the step to `elapsed`. A hook
/// that despawns its entity or removes its machine ends that visit.
///
/// Hooks are plain `fn` pointers. See [`StateMachine`] for an example.
pub struct StateMachinePlugin<S> {
    name: &'static str,
    hooks: Hooks<S>,
}

/// A plugin's hooks, by state.
struct Hooks<S> {
    enter: Vec<(S, Hook)>,
    exit: Vec<(S, Hook)>,
    update: Vec<(S, Hook)>,
}

impl<S: Copy> Clone for Hooks<S> {
    fn clone(&self) -> Self {
        Self {
            enter: self.enter.clone(),
            exit: self.exit.clone(),
            update: self.update.clone(),
        }
    }
}

impl<S: Copy + Eq + 'static> StateMachinePlugin<S> {
    /// A plugin with no hooks, its system named `name`.
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            hooks: Hooks {
                enter: Vec::new(),
                exit: Vec::new(),
                update: Vec::new(),
            },
        }
    }

    /// The plugin, running `hook` when a machine enters `state`.
    ///
    /// # Panics
    ///
    /// When `state` already has an enter hook.
    #[must_use]
    #[track_caller]
    pub fn on_enter(mut self, state: S, hook: fn(&mut World, Entity)) -> Self {
        add_hook(&mut self.hooks.enter, state, hook, "on_enter");
        self
    }

    /// The plugin, running `hook` when a machine leaves `state`.
    ///
    /// # Panics
    ///
    /// When `state` already has an exit hook.
    #[must_use]
    #[track_caller]
    pub fn on_exit(mut self, state: S, hook: fn(&mut World, Entity)) -> Self {
        add_hook(&mut self.hooks.exit, state, hook, "on_exit");
        self
    }

    /// The plugin, running `hook` at each visit of a machine in `state`.
    ///
    /// # Panics
    ///
    /// When `state` already has an update hook.
    #[must_use]
    #[track_caller]
    pub fn on_update(mut self, state: S, hook: fn(&mut World, Entity)) -> Self {
        add_hook(&mut self.hooks.update, state, hook, "on_update");
        self
    }
}

#[track_caller]
fn add_hook<S: Eq>(hooks: &mut Vec<(S, Hook)>, state: S, hook: Hook, method: &str) {
    assert!(
        hooks.iter().all(|(other, _)| *other != state),
        "StateMachinePlugin::{method}: the state already has a hook of this kind"
    );
    hooks.push((state, hook));
}

impl<S: Copy> Clone for StateMachinePlugin<S> {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            hooks: self.hooks.clone(),
        }
    }
}

impl<S> fmt::Debug for StateMachinePlugin<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StateMachinePlugin")
            .field("name", &self.name)
            .field("enter_hooks", &self.hooks.enter.len())
            .field("exit_hooks", &self.hooks.exit.len())
            .field("update_hooks", &self.hooks.update.len())
            .finish()
    }
}

impl<S: Copy + Eq + 'static> Plugin for StateMachinePlugin<S> {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        let hooks = self.hooks.clone();
        let mut entities = Vec::new();
        schedule.add(
            Stage::FixedUpdate,
            system(self.name, move |world: &mut World| {
                visit_all(world, &hooks, &mut entities);
            })
            .after_if_present(PHYSICS_PREV_SNAPSHOT)
            .after_if_present(HEALTH_TICK)
            .before_if_present(PATH_FOLLOW)
            .before_if_present(PLATFORMER_CONTROLLER)
            .before_if_present(TOP_DOWN_MOVER)
            .before_if_present(PHYSICS_STEP),
        );
    }
}

/// One step's visits; `entities` is the system's scratch list.
fn visit_all<S: Copy + Eq + 'static>(
    world: &mut World,
    hooks: &Hooks<S>,
    entities: &mut Vec<Entity>,
) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    if dt <= 0.0 {
        return;
    }
    entities.clear();
    entities.extend(
        world
            .query::<(Entity, &StateMachine<S>)>()
            .map(|(entity, _)| entity),
    );
    entities.sort_unstable_by_key(|entity| entity.id());
    for &entity in entities.iter() {
        visit(world, hooks, entity, dt);
    }
}

/// One machine's visit.
fn visit<S: Copy + Eq + 'static>(world: &mut World, hooks: &Hooks<S>, entity: Entity, dt: f32) {
    let Some(machine) = world.get_mut::<StateMachine<S>>(entity) else {
        return;
    };
    let next = machine.next.take();
    if !machine.entered {
        machine.entered = true;
        if let Some(next) = next {
            machine.state = next;
        }
        let state = machine.state;
        if !run_hook(world, &hooks.enter, state, entity) {
            return;
        }
    } else if let Some(next) = next {
        let old = machine.state;
        if !run_hook(world, &hooks.exit, old, entity) {
            return;
        }
        let Some(machine) = world.get_mut::<StateMachine<S>>(entity) else {
            return;
        };
        machine.previous = Some(old);
        machine.state = next;
        machine.elapsed = 0.0;
        if !run_hook(world, &hooks.enter, next, entity) {
            return;
        }
    }
    let Some(state) = world
        .get::<StateMachine<S>>(entity)
        .map(StateMachine::state)
    else {
        return;
    };
    if !run_hook(world, &hooks.update, state, entity) {
        return;
    }
    if let Some(machine) = world.get_mut::<StateMachine<S>>(entity) {
        machine.elapsed += dt;
    }
}

/// Runs `state`'s hook, if it has one; whether the entity still has its
/// machine after it.
fn run_hook<S: Copy + Eq + 'static>(
    world: &mut World,
    hooks: &[(S, Hook)],
    state: S,
    entity: Entity,
) -> bool {
    if let Some(&(_, hook)) = hooks.iter().find(|(other, _)| *other == state) {
        hook(world, entity);
    }
    world.has::<StateMachine<S>>(entity)
}

#[cfg(test)]
#[path = "tests/state_machine.rs"]
mod tests;
