//! The death screen. Losing the last heart or falling out of bounds pixelates
//! and dims the frame under an animated title; a
//! restart press covers the frame fully, rebuilds the world as a fresh
//! launch builds it (`setup::restart_world`) and uncovers the new run. The
//! dimming is the stock `fade` post pass on the example's `PostStack`.
use tungsten::TransitionEffect;
use tungsten::core::post::{PostPass, PostStack};
use tungsten::core::{ActionMap, Easing, Entity, InputState, Time, World};
use tungsten::physics::{Collider, Position, RigidBody, Velocity};

/// How far the death screen covers the frame while it waits for a restart.
pub(crate) const DEATH_DIM: f32 = 0.8;
/// Dimming time; the restart input is taken once the frame is dim.
pub(crate) const DIM_SECS: f32 = 0.9;
pub(crate) const COVER_SECS: f32 = 0.35;
pub(crate) const UNCOVER_SECS: f32 = 0.5;
/// A near-black crimson; the mix runs in linear light.
const FADE: TransitionEffect = TransitionEffect::Fade {
    color: [0.03, 0.002, 0.008, 1.0],
};
/// The same coarsening used when entering the Sprite Scene pause menu.
const PIXELATE: TransitionEffect = TransitionEffect::Pixelate { max_block_px: 48.0 };

/// Where the death screen is; `Alive` draws nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) enum DeathScreen {
    #[default]
    Alive,
    /// The player is dead; the frame dims under the title.
    Dying { elapsed: f32 },
    /// Restart pressed; the frame closes to full cover, then the world is rebuilt.
    Covering { elapsed: f32 },
    /// The new run shows through as the cover lifts.
    Uncovering { elapsed: f32 },
}

impl DeathScreen {
    /// How far the frame is covered, in [0, 1].
    pub(crate) fn cover(self) -> f32 {
        let eased = |elapsed: f32, secs: f32| Easing::QuadOut.apply((elapsed / secs).min(1.0));
        match self {
            Self::Alive => 0.0,
            Self::Dying { elapsed } => DEATH_DIM * eased(elapsed, DIM_SECS),
            Self::Covering { elapsed } => {
                DEATH_DIM + (1.0 - DEATH_DIM) * eased(elapsed, COVER_SECS)
            }
            Self::Uncovering { elapsed } => 1.0 - eased(elapsed, UNCOVER_SECS),
        }
    }

    /// Opacity of the title, in [0, 1]: it rises with the dimming and
    /// leaves with the cover.
    pub(crate) fn title_alpha(self) -> f32 {
        match self {
            Self::Dying { elapsed } => ((elapsed - 0.25) / 0.5).clamp(0.0, 1.0),
            Self::Covering { elapsed } => 1.0 - (elapsed / COVER_SECS).min(1.0),
            Self::Alive | Self::Uncovering { .. } => 0.0,
        }
    }

    /// Whether a restart press is taken now.
    pub(crate) fn accepts_restart(self) -> bool {
        matches!(self, Self::Dying { elapsed } if elapsed >= DIM_SECS)
    }

    /// Whether the old run's player is dead: it neither acts nor takes damage.
    pub(crate) fn player_dead(self) -> bool {
        matches!(self, Self::Dying { .. } | Self::Covering { .. })
    }
}

pub(crate) fn player_dead(world: &World) -> bool {
    world
        .get_resource::<DeathScreen>()
        .is_some_and(|screen| screen.player_dead())
}

/// The last heart is gone: the body leaves the physics where it fell, so
/// nothing collides with it and the camera holds there, and the screen starts.
pub(crate) fn kill_player(world: &mut World, player: Entity) {
    if player_dead(world) {
        return;
    }
    world.insert_resource(DeathScreen::Dying { elapsed: 0.0 });
    if world.get::<crate::gameplay::Health>(player).is_none() {
        world.insert(player, crate::gameplay::Health::default());
    }
    world
        .get_mut::<crate::gameplay::Health>(player)
        .unwrap()
        .hearts = 0;
    if let Some(state) = world.get_mut::<crate::state::PlayerPresentation>(player) {
        state.pending_effect = None;
    }
    world.remove_component::<Collider>(player);
    world.remove_component::<RigidBody>(player);
    world.remove_component::<Velocity>(player);
    if let Some(position) = world.get::<Position>(player).map(|p| p.0) {
        world.insert(player, tungsten::physics::PrevPosition(position));
        crate::systems::spawn_transient_effect(world, "ex10_double_jump", position);
    }
}

/// Advances the screen on real time, so a paused or scaled game clock
/// cannot hold it, takes the restart press and draws the fade.
pub(crate) fn death_screen_system(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::real_delta);
    let screen = world
        .get_resource::<DeathScreen>()
        .copied()
        .unwrap_or_default();
    let restart = screen.accepts_restart()
        && world
            .get_resource::<InputState>()
            .zip(world.get_resource::<ActionMap>())
            .is_some_and(|(input, actions)| actions.just_pressed(input, "restart"));
    let next = match screen {
        DeathScreen::Alive => DeathScreen::Alive,
        DeathScreen::Dying { .. } if restart => DeathScreen::Covering { elapsed: 0.0 },
        DeathScreen::Dying { elapsed } => DeathScreen::Dying {
            elapsed: elapsed + dt,
        },
        // The boundary frame draws fully covered, the rebuilt world under it.
        DeathScreen::Covering { elapsed } if elapsed + dt >= COVER_SECS => {
            crate::setup::restart_world(world);
            DeathScreen::Uncovering { elapsed: 0.0 }
        }
        DeathScreen::Covering { elapsed } => DeathScreen::Covering {
            elapsed: elapsed + dt,
        },
        DeathScreen::Uncovering { elapsed } if elapsed + dt >= UNCOVER_SECS => DeathScreen::Alive,
        DeathScreen::Uncovering { elapsed } => DeathScreen::Uncovering {
            elapsed: elapsed + dt,
        },
    };
    world.insert_resource(next);
    if let Some(stack) = world.get_resource_mut::<PostStack>() {
        stack
            .0
            .retain(|pass| !matches!(pass, PostPass::Fade(_) | PostPass::Pixelate(_)));
        let cover = next.cover();
        if cover > 0.0 {
            stack.push(PIXELATE.pass_at((cover / DEATH_DIM).min(1.0)));
            stack.push(FADE.pass_at(cover));
        }
    }
}
