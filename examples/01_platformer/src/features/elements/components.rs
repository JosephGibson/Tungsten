//! The element rules' components.

use glam::Vec2;

/// A lit small ball's flame: whether nothing rests on it, so it burns on
/// the pile's surface. Presentation, which `flame_exposure` refreshes each
/// step; the fire itself is the ball's `ball_burn` and `kindled` statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flame {
    pub exposed: bool,
}

/// Where the cold last met a body, so its freeze's crystals show on that
/// face: presentation, which `chill` writes and the `freeze` handler reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColdFace(pub Vec2);

/// One emitter of the burning surface's bounded pool.
#[derive(Debug, Clone, Copy)]
pub struct FireEmitter;

/// A brief icy ring where iron froze or shattered, about `at`, `size` its
/// base across: presentation, the ring's own entity, a tweened sprite its
/// tween ends, with its glow beside it ([`PulseGlow`]).
#[derive(Debug, Clone, Copy)]
pub struct IcePulse {
    pub at: Vec2,
    pub size: f32,
    pub shatter: bool,
}

impl IcePulse {
    /// How long it shows.
    #[must_use]
    pub fn duration(self) -> f32 {
        if self.shatter { 0.32 } else { 0.24 }
    }

    /// How fast its ring turns, radians a second: a shatter's back fast.
    #[must_use]
    pub fn turn_rate(self) -> f32 {
        if self.shatter { -1.5 } else { 0.5 }
    }
}

/// A cold pulse's glow, a tweened sprite of its own beside its ring, the
/// pulse it lights: drawn just before it, both ended by their tweens.
#[derive(Debug, Clone, Copy)]
pub struct PulseGlow(pub tungsten::core::Entity);

/// The bowling balls' strikes whose bodies still touch: each a ball and
/// the enemy it struck, kept until they part, so one contact stuns once
/// (`ball_strikes`) and thuds none (`ball_impacts`). A World resource the
/// plugin inserts; not saved, so a load mid-contact may strike once more.
#[derive(Debug, Default)]
pub struct Strikes(pub Vec<(tungsten::core::Entity, tungsten::core::Entity)>);

impl Strikes {
    /// Whether `ball` struck `foe` and still touches it.
    #[must_use]
    pub fn touching(&self, ball: tungsten::core::Entity, foe: tungsten::core::Entity) -> bool {
        self.0.contains(&(ball, foe))
    }
}
