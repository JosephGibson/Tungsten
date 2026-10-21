//! The audio's resources and the game's sound events.

use glam::Vec2;
use tungsten::core::{Entity, Voice};

/// The music playing: the exploration loop, which plays on at 0 under
/// pause and game over, and pause's loop while pause is on top.
#[derive(Debug, Clone, Copy, Default)]
pub struct Music {
    pub explore: Option<Voice>,
    pub pause: Option<Voice>,
    pub ducked: bool,
}

/// One of the game's one-shot sounds, sent as a cue event of its own name
/// about the player, who hears it: `cue_rules/sounds.json` plays each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sounded {
    pub sound: Sound,
    pub at: Vec2,
    pub subject: Entity,
}

/// The game's one-shot sounds, by their cue events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    /// A fireball's blast.
    Blasted,
    /// Iron crushing balls, the player or frozen iron.
    Crushed,
    /// The black hole putting out burning balls.
    Doused,
    /// Iron frozen through.
    Froze,
    /// Frozen iron shattering.
    Shattered,
    /// A black hole opening.
    HoleOpened,
    /// A cold spray starting and ending.
    ColdStarted,
    ColdEnded,
    /// A fire spray starting.
    FireStarted,
    /// A bowling ball tossed.
    Tossed,
    /// An iron block or small balls set down.
    Placed,
    /// A fire missile cast.
    Cast,
    /// A force push.
    Pushed,
}

impl Sound {
    /// Every sound, in order.
    pub const ALL: [Sound; 13] = [
        Sound::Blasted,
        Sound::Crushed,
        Sound::Doused,
        Sound::Froze,
        Sound::Shattered,
        Sound::HoleOpened,
        Sound::ColdStarted,
        Sound::ColdEnded,
        Sound::FireStarted,
        Sound::Tossed,
        Sound::Placed,
        Sound::Cast,
        Sound::Pushed,
    ];

    /// Its cue event's name.
    #[must_use]
    pub fn event(self) -> &'static str {
        match self {
            Sound::Blasted => "blasted",
            Sound::Crushed => "crushed",
            Sound::Doused => "doused",
            Sound::Froze => "froze",
            Sound::Shattered => "shattered",
            Sound::HoleOpened => "hole_opened",
            Sound::ColdStarted => "cold_started",
            Sound::ColdEnded => "cold_ended",
            Sound::FireStarted => "fire_started",
            Sound::Tossed => "tossed",
            Sound::Placed => "placed",
            Sound::Cast => "cast",
            Sound::Pushed => "pushed",
        }
    }
}
