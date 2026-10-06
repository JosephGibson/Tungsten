//! Named stages, named systems with stage-local ordering, and plugins: the
//! schedule the app drives each frame (W15a's design, spiked 2026-10-05).
//!
//! A system is still `FnMut(&mut World)`. Each has a name and a [`Stage`];
//! `before` and `after` constraints name other systems in the same stage. A
//! stable topological sort resolves each stage: among the systems whose
//! constraints are met, the one registered first runs first, so the order
//! is deterministic and a snapshot test can pin it. A duplicate name, an
//! unknown name in a constraint, or a cycle is a [`ScheduleError`] at
//! [`Schedule::resolve`], naming the systems involved.
//!
//! A [`Plugin`] adds systems, events, resources and inspector rows through
//! `&mut Schedule` and `&mut World` only, so a plugin in a crate that depends
//! on core alone (the kit, `D-007`) can do everything an engine plugin does
//! except contribute render data to the extract, which stays with the app.

use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use crate::ecs::World;

/// The stages of a frame that take systems, in the order they run. The
/// engine's own steps (command flush, event rotation, hot reload, extract,
/// render, audio, telemetry) run after `PostUpdate` and take no systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stage {
    /// Once, before the first frame's `PreUpdate`, after the manifests have
    /// loaded; the headless harness runs it on its first step.
    Startup,
    /// Input consumers, engine toggles and the state dispatcher.
    PreUpdate,
    /// Physics and opt-in gameplay. Once per frame until W3b's accumulator
    /// steps it at a fixed rate.
    FixedUpdate,
    /// Gameplay; where `App::add_system` registers.
    Update,
    /// Physics sync, hierarchy, animation, particles, tweens, game feel and
    /// the camera, before the command flush.
    PostUpdate,
}

impl Stage {
    /// Every stage, in frame order.
    pub const ALL: [Stage; 5] = [
        Stage::Startup,
        Stage::PreUpdate,
        Stage::FixedUpdate,
        Stage::Update,
        Stage::PostUpdate,
    ];

    /// The stage's name, as the snapshot and the systems overlay print it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Stage::Startup => "startup",
            Stage::PreUpdate => "pre_update",
            Stage::FixedUpdate => "fixed_update",
            Stage::Update => "update",
            Stage::PostUpdate => "post_update",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A tick system.
pub type SystemFn = Box<dyn FnMut(&mut World)>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Constraint {
    other: String,
    /// `true`: the other system must exist; `false`: the constraint applies
    /// only when it does (for a coupling across plugins a game may leave out).
    required: bool,
}

/// A system with its name and ordering constraints, built by [`system`].
pub struct SystemDesc {
    name: String,
    before: Vec<Constraint>,
    after: Vec<Constraint>,
    run: SystemFn,
}

/// Starts a system description: `system("camera_update", f).after("shake_tick")`.
pub fn system(name: impl Into<String>, run: impl FnMut(&mut World) + 'static) -> SystemDesc {
    SystemDesc {
        name: name.into(),
        before: Vec::new(),
        after: Vec::new(),
        run: Box::new(run),
    }
}

impl SystemDesc {
    /// The system's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Runs before `other`, which must be in the same stage.
    #[must_use]
    pub fn before(mut self, other: impl Into<String>) -> Self {
        self.before.push(Constraint {
            other: other.into(),
            required: true,
        });
        self
    }

    /// Runs after `other`, which must be in the same stage.
    #[must_use]
    pub fn after(mut self, other: impl Into<String>) -> Self {
        self.after.push(Constraint {
            other: other.into(),
            required: true,
        });
        self
    }

    /// Runs before `other` when `other` is registered in the same stage;
    /// no constraint otherwise.
    #[must_use]
    pub fn before_if_present(mut self, other: impl Into<String>) -> Self {
        self.before.push(Constraint {
            other: other.into(),
            required: false,
        });
        self
    }

    /// Runs after `other` when `other` is registered in the same stage; no
    /// constraint otherwise.
    #[must_use]
    pub fn after_if_present(mut self, other: impl Into<String>) -> Self {
        self.after.push(Constraint {
            other: other.into(),
            required: false,
        });
        self
    }
}

/// Why a schedule cannot be resolved. Each names the systems involved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleError {
    /// Two systems share a name, so a constraint on it would be ambiguous.
    #[error("system `{name}` is registered twice, in `{first}` and `{second}`")]
    DuplicateName {
        /// The name registered twice.
        name: String,
        /// The stage of the first registration.
        first: Stage,
        /// The stage of the second.
        second: Stage,
    },
    /// A `before` or `after` names a system that is not in the stage.
    #[error("system `{system}` in `{stage}` runs {relation} `{missing}`, which {reason}")]
    UnknownName {
        /// The system whose constraint names `missing`.
        system: String,
        /// Its stage.
        stage: Stage,
        /// `before` or `after`.
        relation: &'static str,
        /// The name the constraint gives.
        missing: String,
        /// Where `missing` is: "is in" with its stage, or "is not registered".
        reason: String,
    },
    /// The constraints of a stage contradict each other.
    #[error("stage `{stage}` has an ordering cycle: {}", path.join(" -> "))]
    Cycle {
        /// The stage whose constraints form the cycle.
        stage: Stage,
        /// The systems around the cycle in run order, the first repeated last.
        path: Vec<String>,
    },
}

struct Entry {
    name: String,
    stage: Stage,
    before: Vec<Constraint>,
    after: Vec<Constraint>,
    run: SystemFn,
}

/// The systems of a frame by stage, resolved into one deterministic order
/// per stage.
pub struct Schedule {
    entries: Vec<Entry>,
    /// Per stage, indices into `entries` in run order; valid while `resolved`.
    order: [Vec<usize>; Stage::ALL.len()],
    resolved: bool,
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

impl Schedule {
    /// An empty schedule.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            order: Default::default(),
            resolved: false,
        }
    }

    /// Adds `desc` to `stage`. The schedule needs resolving again before it
    /// runs.
    pub fn add(&mut self, stage: Stage, desc: SystemDesc) {
        self.entries.push(Entry {
            name: desc.name,
            stage,
            before: desc.before,
            after: desc.after,
            run: desc.run,
        });
        self.resolved = false;
    }

    /// `add(stage, system(name, run))`.
    pub fn add_fn(
        &mut self,
        stage: Stage,
        name: impl Into<String>,
        run: impl FnMut(&mut World) + 'static,
    ) {
        self.add(stage, system(name, run));
    }

    /// How many systems are registered, over every stage.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no system is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The stage `name` is registered in, if any.
    #[must_use]
    pub fn stage_of(&self, name: &str) -> Option<Stage> {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.stage)
    }

    /// Whether `name` is registered in any stage.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.stage_of(name).is_some()
    }

    /// Whether the run order is current.
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Resolves every stage's order. Idempotent once resolved; a later
    /// [`Schedule::add`] needs another call.
    ///
    /// # Errors
    ///
    /// A duplicate name, an unknown name in a required constraint, or a
    /// cycle, each naming the systems involved.
    pub fn resolve(&mut self) -> Result<(), ScheduleError> {
        if self.resolved {
            return Ok(());
        }
        let mut seen: HashMap<&str, Stage> = HashMap::with_capacity(self.entries.len());
        for entry in &self.entries {
            if let Some(&first) = seen.get(entry.name.as_str()) {
                return Err(ScheduleError::DuplicateName {
                    name: entry.name.clone(),
                    first,
                    second: entry.stage,
                });
            }
            seen.insert(&entry.name, entry.stage);
        }
        let mut order: [Vec<usize>; Stage::ALL.len()] = Default::default();
        for stage in Stage::ALL {
            order[stage.index()] = self.resolve_stage(stage, &seen)?;
        }
        self.order = order;
        self.resolved = true;
        Ok(())
    }

    fn resolve_stage(
        &self,
        stage: Stage,
        all: &HashMap<&str, Stage>,
    ) -> Result<Vec<usize>, ScheduleError> {
        let nodes: Vec<usize> = (0..self.entries.len())
            .filter(|&i| self.entries[i].stage == stage)
            .collect();
        let local: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(local, &i)| (self.entries[i].name.as_str(), local))
            .collect();
        let n = nodes.len();
        let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut indeg = vec![0usize; n];
        let mut add_edge = |from: usize, to: usize| {
            succ[from].push(to);
            pred[to].push(from);
            indeg[to] += 1;
        };
        for (i, &entry_index) in nodes.iter().enumerate() {
            let entry = &self.entries[entry_index];
            let resolve_other = |constraint: &Constraint, relation: &'static str| match local
                .get(constraint.other.as_str())
            {
                Some(&j) => Ok(Some(j)),
                None if !constraint.required => Ok(None),
                None => Err(ScheduleError::UnknownName {
                    system: entry.name.clone(),
                    stage,
                    relation,
                    missing: constraint.other.clone(),
                    reason: match all.get(constraint.other.as_str()) {
                        Some(other_stage) => format!("is in `{other_stage}`"),
                        None => "is not registered".to_string(),
                    },
                }),
            };
            for constraint in &entry.before {
                if let Some(j) = resolve_other(constraint, "before")? {
                    add_edge(i, j);
                }
            }
            for constraint in &entry.after {
                if let Some(j) = resolve_other(constraint, "after")? {
                    add_edge(j, i);
                }
            }
        }
        // Kahn's algorithm, taking the earliest-registered ready system each
        // time: ties keep registration order.
        let mut done = vec![false; n];
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            let Some(next) = (0..n).find(|&i| !done[i] && indeg[i] == 0) else {
                break;
            };
            done[next] = true;
            out.push(nodes[next]);
            for &j in &succ[next] {
                indeg[j] -= 1;
            }
        }
        if let Some(start) = (0..n).find(|&i| !done[i]) {
            return Err(ScheduleError::Cycle {
                stage,
                path: self.cycle_path(&nodes, &pred, &done, start),
            });
        }
        Ok(out)
    }

    /// A cycle among the unresolved nodes, as names in run order, the first
    /// repeated last. The walk goes from `start` through unresolved
    /// predecessors: every unresolved node has one (that is what left it
    /// unresolved), so the walk never runs dry and must revisit a node,
    /// which is on a cycle even when `start` itself merely waits behind one.
    fn cycle_path(
        &self,
        nodes: &[usize],
        pred: &[Vec<usize>],
        done: &[bool],
        start: usize,
    ) -> Vec<String> {
        let name = |&i: &usize| self.entries[nodes[i]].name.clone();
        let mut stack = vec![start];
        let mut position: Vec<Option<usize>> = vec![None; nodes.len()];
        position[start] = Some(0);
        loop {
            let top = stack[stack.len() - 1];
            let Some(next) = pred[top].iter().copied().find(|&j| !done[j]) else {
                // Unreachable by the invariant above; report the walk, not a panic.
                return stack.iter().rev().map(name).collect();
            };
            if let Some(from) = position[next] {
                // Predecessors were walked, so the cycle reads backwards from
                // `next`: reverse the tail into run order.
                let mut path = Vec::with_capacity(stack.len() - from + 1);
                path.push(name(&stack[from]));
                path.extend(stack[from + 1..].iter().rev().map(name));
                path.push(name(&stack[from]));
                return path;
            }
            position[next] = Some(stack.len());
            stack.push(next);
        }
    }

    /// The names in `stage`, in run order.
    ///
    /// # Panics
    ///
    /// When the schedule is not resolved.
    pub fn names(&self, stage: Stage) -> impl Iterator<Item = &str> {
        assert!(self.resolved, "Schedule::names on an unresolved schedule");
        self.order[stage.index()]
            .iter()
            .map(|&i| self.entries[i].name.as_str())
    }

    /// One line per stage, `stage: a, b, c` (`-` when empty), for snapshot
    /// tests and the systems overlay.
    ///
    /// # Panics
    ///
    /// When the schedule is not resolved.
    #[must_use]
    pub fn resolved_text(&self) -> String {
        let mut text = String::new();
        for stage in Stage::ALL {
            let names: Vec<&str> = self.names(stage).collect();
            text.push_str(stage.name());
            text.push_str(": ");
            if names.is_empty() {
                text.push('-');
            } else {
                text.push_str(&names.join(", "));
            }
            text.push('\n');
        }
        text
    }

    /// Runs `stage`'s systems in order, reporting each one's name and wall
    /// time to `observe`.
    ///
    /// # Panics
    ///
    /// When the schedule is not resolved.
    pub fn run_stage(
        &mut self,
        stage: Stage,
        world: &mut World,
        mut observe: impl FnMut(&str, Duration),
    ) {
        assert!(
            self.resolved,
            "Schedule::run_stage on an unresolved schedule"
        );
        for &i in &self.order[stage.index()] {
            let entry = &mut self.entries[i];
            let start = Instant::now();
            (entry.run)(world);
            observe(&entry.name, start.elapsed());
        }
    }
}

/// A unit of registration: systems by stage, the events they send,
/// resources and inspector rows. Engine features and kit items are plugins;
/// a game is one too.
pub trait Plugin: 'static {
    /// Registers the plugin's systems, events, resources and rows.
    fn build(&self, schedule: &mut Schedule, world: &mut World);

    /// The plugin's name, for diagnostics; the type name by default.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// An ordered set of plugins, one per type: a game starts from the engine's
/// default set and leaves out the plugins it replaces.
#[derive(Default)]
pub struct PluginSet {
    plugins: Vec<(TypeId, Box<dyn Plugin>)>,
}

impl PluginSet {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The set with `plugin` at the end, or with the one of the same type
    /// replaced in place.
    #[must_use]
    pub fn with<P: Plugin>(mut self, plugin: P) -> Self {
        let id = TypeId::of::<P>();
        match self.plugins.iter_mut().find(|(other, _)| *other == id) {
            Some(slot) => slot.1 = Box::new(plugin),
            None => self.plugins.push((id, Box::new(plugin))),
        }
        self
    }

    /// Removes the plugin of type `P`, if present.
    #[must_use]
    pub fn without<P: Plugin>(mut self) -> Self {
        let id = TypeId::of::<P>();
        self.plugins.retain(|(other, _)| *other != id);
        self
    }

    /// Whether a plugin of type `P` is in the set.
    #[must_use]
    pub fn contains<P: Plugin>(&self) -> bool {
        let id = TypeId::of::<P>();
        self.plugins.iter().any(|(other, _)| *other == id)
    }

    /// The plugins' names, in order.
    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|(_, p)| p.name()).collect()
    }

    /// How many plugins are in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Builds every plugin in order; a set is built once.
    pub fn build(self, schedule: &mut Schedule, world: &mut World) {
        for (_, plugin) in &self.plugins {
            plugin.build(schedule, world);
        }
    }
}

#[cfg(test)]
#[path = "tests/schedule.rs"]
mod tests;
