//! The kit's phases: seven anchor systems in the engine's stages that every
//! kit system joins, so the kit's order is one table rather than a chain of
//! pairwise constraints (`D-158`, as the kit foundations amend it).

use tungsten_core::physics::{PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PHYSICS_SYNC};
use tungsten_core::schedule::names::{CAMERA_UPDATE, TWEEN_TICK};
use tungsten_core::{Schedule, Stage, SystemDesc, World, system};

use crate::transform::hierarchy::TRANSFORM_PROPAGATE_LATE;

/// [`Phase::Tick`]'s anchor: `FixedUpdate`, after the physics history
/// snapshot when it is registered.
pub const KIT_TICK: &str = "kit_tick";
/// [`Phase::Intent`]'s anchor: `FixedUpdate`, after [`KIT_TICK`].
pub const KIT_INTENT: &str = "kit_intent";
/// [`Phase::Move`]'s anchor: `FixedUpdate`, after [`KIT_INTENT`] and
/// before `physics_step` when it is registered.
pub const KIT_MOVE: &str = "kit_move";
/// [`Phase::Overlap`]'s anchor: `FixedUpdate`, after [`KIT_MOVE`] and
/// after `physics_step` when it is registered.
pub const KIT_OVERLAP: &str = "kit_overlap";
/// [`Phase::Resolve`]'s anchor: `FixedUpdate`, after [`KIT_OVERLAP`].
pub const KIT_RESOLVE: &str = "kit_resolve";
/// [`Phase::React`]'s anchor: `FixedUpdate`, after [`KIT_RESOLVE`], last
/// of the kit's.
pub const KIT_REACT: &str = "kit_react";
/// [`Phase::Present`]'s anchor: `PostUpdate`, after the physics sync, the
/// late propagation pass and the tween tick, and before the camera, each
/// when registered.
pub const KIT_PRESENT: &str = "kit_present";

/// Where a kit system runs: one of seven phases, each an anchor system
/// ([`ensure_phases`]) that its members are ordered after, and before the
/// next anchor of its stage. Inside a phase, systems run in registration
/// order; an item names another item's system only where it reads or
/// adds to that item's output in the same step: the path follower, the
/// linear motion and the oscillator before the controllers, so a rider
/// reads its platform's velocity; the follower and the orbit after the
/// movers their target moves by; the knockback after the drivers it adds
/// to, and the speed limit after everything it caps; the aim after the
/// input; the character animator after the facing and the animation tick;
/// the pickups' magnet after the oscillator it adds to; the hit items after
/// the hit grid and before the despawn outside, the wrap and the stay in
/// view, which judge or rewrite the step's motion; the kit's end-of-step
/// despawn after the spawns.
///
/// | Phase | Stage | What runs there |
/// | --- | --- | --- |
/// | `Tick` | `FixedUpdate`, after the history snapshot | timers, lifetimes, health windows and death delays, shields, stats, cooldowns, effects, the ailment drain |
/// | `Intent` | `FixedUpdate` | input, aim, abilities, state machines |
/// | `Move` | `FixedUpdate`, before `physics_step` | the controllers, the path follower, linear motion, the oscillator, steering, follow, orbit, knockback, the pickups' magnet, the speed limit |
/// | `Overlap` | `FixedUpdate`, after `physics_step` | zones, the speed limit's settle, the hit grid, hitboxes, contact damage, projectiles, pickups, despawn outside, wrap, stay in view |
/// | `Resolve` | `FixedUpdate` | the damage pipeline's queued requests and heals, the ailment feed, the effect requests |
/// | `React` | `FixedUpdate`, last | spawners, child spawns, level ups, cues, the kit's end-of-step despawn, the trace's step count |
/// | `Present` | `PostUpdate`, after the sync and the late propagation, before the camera | animation, the hit flash, the camera target, labels, sounds, the oscillator's rotation and scale, facing, the character animator, depth sort |
///
/// Stats sit in `Tick`, not `Resolve`, so an entity spawned at a flush
/// moves at its stats from its first step.
///
/// A game's own system joins a phase the same way: its plugin calls
/// [`ensure_phases`], then adds [`Phase::join`]'s description in the
/// phase's [`stage`](Self::stage).
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, World, system};
/// use tungsten_kit::{Phase, TopDownMoverPlugin, ensure_phases};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// ensure_phases(&mut schedule);
/// // A magnet that pulls pickups: motion, so it runs in the move phase.
/// schedule.add(
///     Stage::FixedUpdate,
///     Phase::Move.join(system("magnet", |_: &mut World| {})),
/// );
/// TopDownMoverPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
/// assert_eq!(
///     order,
///     ["kit_tick", "kit_intent", "kit_move", "magnet", "top_down_mover", "kit_overlap",
///      "kit_resolve", "kit_react"]
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Timers and windows, first in the step.
    Tick,
    /// What each entity means to do this step.
    Intent,
    /// Motion, before the physics step.
    Move,
    /// What the step's motion met, after the physics step.
    Overlap,
    /// Damage and effects.
    Resolve,
    /// Reactions: spawns and cues, last in the step.
    React,
    /// Presentation, once a frame after the physics sync.
    Present,
}

impl Phase {
    /// Every phase, in order.
    pub const ALL: [Phase; 7] = [
        Phase::Tick,
        Phase::Intent,
        Phase::Move,
        Phase::Overlap,
        Phase::Resolve,
        Phase::React,
        Phase::Present,
    ];

    /// The phase's anchor system name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Phase::Tick => KIT_TICK,
            Phase::Intent => KIT_INTENT,
            Phase::Move => KIT_MOVE,
            Phase::Overlap => KIT_OVERLAP,
            Phase::Resolve => KIT_RESOLVE,
            Phase::React => KIT_REACT,
            Phase::Present => KIT_PRESENT,
        }
    }

    /// The stage the phase runs in: `FixedUpdate` for all but `Present`.
    #[must_use]
    pub fn stage(self) -> Stage {
        match self {
            Phase::Present => Stage::PostUpdate,
            _ => Stage::FixedUpdate,
        }
    }

    /// The next anchor of the phase's stage.
    fn next(self) -> Option<Phase> {
        match self {
            Phase::Tick => Some(Phase::Intent),
            Phase::Intent => Some(Phase::Move),
            Phase::Move => Some(Phase::Overlap),
            Phase::Overlap => Some(Phase::Resolve),
            Phase::Resolve => Some(Phase::React),
            Phase::React | Phase::Present => None,
        }
    }

    /// `desc` ordered inside this phase: after its anchor and before the
    /// next anchor of its stage. A `Move` member is also before
    /// `physics_step` and a `Present` member before `camera_update`, each
    /// when registered: the resolver takes the earliest-registered ready
    /// system, and the engine's system, registered first, would otherwise
    /// run ahead of a member whose only bound is the next anchor. The
    /// schedule fails to resolve when the anchor is missing: call
    /// [`ensure_phases`] first.
    #[must_use]
    pub fn join(self, desc: SystemDesc) -> SystemDesc {
        let mut desc = desc.after(self.name());
        if let Some(next) = self.next() {
            desc = desc.before(next.name());
        }
        match self {
            Phase::Move => desc.before_if_present(PHYSICS_STEP),
            Phase::Present => desc.before_if_present(CAMERA_UPDATE),
            _ => desc,
        }
    }
}

/// Adds every anchor [`Schedule::contains`] does not find: a no-op system
/// per [`Phase`], ordered after every earlier anchor of its stage and
/// against the engine's names as the [`Phase`] table says, each of those
/// constraints applying when the engine's system is registered. Every kit
/// plugin calls it first in `build`, and a game's plugin that joins a phase
/// calls it too; calling it again adds nothing.
pub fn ensure_phases(schedule: &mut Schedule) {
    for phase in Phase::ALL {
        if schedule.contains(phase.name()) {
            continue;
        }
        let mut desc = system(phase.name(), |_: &mut World| {});
        for earlier in Phase::ALL
            .into_iter()
            .take_while(|earlier| *earlier != phase)
            .filter(|earlier| earlier.stage() == phase.stage())
        {
            desc = desc.after(earlier.name());
        }
        desc = match phase {
            Phase::Tick => desc.after_if_present(PHYSICS_PREV_SNAPSHOT),
            Phase::Move => desc.before_if_present(PHYSICS_STEP),
            Phase::Overlap => desc.after_if_present(PHYSICS_STEP),
            Phase::Present => desc
                .after_if_present(PHYSICS_SYNC)
                .after_if_present(TRANSFORM_PROPAGATE_LATE)
                .after_if_present(TWEEN_TICK)
                .before_if_present(CAMERA_UPDATE),
            Phase::Intent | Phase::Resolve | Phase::React => desc,
        };
        schedule.add(phase.stage(), desc);
    }
}

#[cfg(test)]
#[path = "../tests/phases.rs"]
mod tests;
