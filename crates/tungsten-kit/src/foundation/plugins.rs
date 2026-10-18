//! [`KitPlugins`]: every kit plugin that takes no game type, as one set.

use tungsten_core::PluginSet;

use crate::damage::effects::EffectsPlugin;
use crate::damage::health::HealthPlugin;
use crate::foundation::debug::KitDebugPlugin;
use crate::foundation::source::SourcePlugin;
use crate::foundation::stats::StatsPlugin;
use crate::foundation::tags::TagsPlugin;
use crate::intent::ability::AbilitiesPlugin;
use crate::intent::action_input::ActionInputPlugin;
use crate::intent::aim::AimPlugin;
use crate::lifecycle::children::SpawnChildrenPlugin;
use crate::lifecycle::cooldown::CooldownPlugin;
use crate::lifecycle::despawn::DespawnOutsidePlugin;
use crate::lifecycle::lifetime::LifetimePlugin;
use crate::lifecycle::spawner::SpawnerPlugin;
use crate::motion::follow::FollowPlugin;
use crate::motion::knockback::KnockbackPlugin;
use crate::motion::linear::LinearMotionPlugin;
use crate::motion::orbit::OrbitPlugin;
use crate::motion::oscillator::OscillatorPlugin;
use crate::motion::path::PathFollowerPlugin;
use crate::motion::platformer::PlatformerControllerPlugin;
use crate::motion::speed_limit::SpeedLimitPlugin;
use crate::motion::stay_in_view::StayInViewPlugin;
use crate::motion::steering::SteeringPlugin;
use crate::motion::top_down::TopDownMoverPlugin;
use crate::motion::wrap::WrapPlugin;
use crate::overlap::contact::ContactDamagePlugin;
use crate::overlap::hit::HitPlugin;
use crate::overlap::pickup::PickupPlugin;
use crate::overlap::projectile::ProjectilePlugin;
use crate::overlap::zone::ZonePlugin;
use crate::present::animation::AnimationPlugin;
use crate::present::camera::CameraTargetPlugin;
use crate::present::character::CharacterAnimatorPlugin;
use crate::present::depth::DepthSortPlugin;
use crate::present::facing::FacingPlugin;
use crate::react::counters::CountersPlugin;
use crate::react::cues::CuesPlugin;
use crate::react::level::LevelPlugin;
use crate::transform::hierarchy::HierarchyPlugin;

/// Every kit plugin that takes no game type, in the kit's order, as a
/// `PluginSet` a game builds after the engine's `DefaultPlugins`
/// (`App::add_plugins`); `PluginSet::without` drops one. The plugins that
/// take a game type, [`SoundOnEvent`](crate::SoundOnEvent),
/// [`TriggerZone`](crate::TriggerZone),
/// [`StateMachinePlugin`](crate::StateMachinePlugin) and
/// [`CuesOnEvent`](crate::CuesOnEvent), a game adds itself.
///
/// # Examples
///
/// ```
/// use tungsten_core::{PluginSet, Schedule, World};
/// use tungsten_kit::{KitPlugins, TagsPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// KitPlugins::set().build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// assert!(schedule.contains("kit_tick") && schedule.contains("top_down_mover"));
///
/// // A game that declares its tags at build drops the default set's
/// // `TagsPlugin` for its own.
/// let plugins = KitPlugins::set()
///     .without::<TagsPlugin>()
///     .with(TagsPlugin::new().with_tags(["team.player"]));
/// assert_eq!(plugins.len(), KitPlugins::set().len());
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct KitPlugins;

impl KitPlugins {
    /// Tags, source, hierarchy, animation, lifetime, despawn outside,
    /// spawner, health, path follower, the platformer controller, the
    /// top-down mover, zone, action input, the camera target, stats,
    /// counters, cooldown, spawn children, cues, linear motion, the
    /// oscillator, steering, follow, orbit, knockback, the speed limit,
    /// wrap, stay in view, aim, facing, the character animator, depth sort,
    /// the hit shapes, contact damage, projectiles, pickups, effects,
    /// abilities, levels and the debug drawing, in that order.
    #[must_use]
    pub fn set() -> PluginSet {
        PluginSet::new()
            .with(TagsPlugin::new())
            .with(SourcePlugin)
            .with(HierarchyPlugin)
            .with(AnimationPlugin)
            .with(LifetimePlugin)
            .with(DespawnOutsidePlugin)
            .with(SpawnerPlugin)
            .with(HealthPlugin)
            .with(PathFollowerPlugin)
            .with(PlatformerControllerPlugin)
            .with(TopDownMoverPlugin)
            .with(ZonePlugin)
            .with(ActionInputPlugin)
            .with(CameraTargetPlugin)
            .with(StatsPlugin::new())
            .with(CountersPlugin)
            .with(CooldownPlugin)
            .with(SpawnChildrenPlugin)
            .with(CuesPlugin)
            .with(LinearMotionPlugin)
            .with(OscillatorPlugin)
            .with(SteeringPlugin)
            .with(FollowPlugin)
            .with(OrbitPlugin)
            .with(KnockbackPlugin)
            .with(SpeedLimitPlugin)
            .with(WrapPlugin)
            .with(StayInViewPlugin)
            .with(AimPlugin)
            .with(FacingPlugin)
            .with(CharacterAnimatorPlugin)
            .with(DepthSortPlugin)
            .with(HitPlugin)
            .with(ContactDamagePlugin)
            .with(ProjectilePlugin)
            .with(PickupPlugin)
            .with(EffectsPlugin)
            .with(AbilitiesPlugin)
            .with(LevelPlugin)
            .with(KitDebugPlugin)
    }
}
