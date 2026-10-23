//! The audio's resources and the game's sound events.

use glam::Vec2;
use tungsten::core::{Entity, Voice};

/// The music playing: the exploration loop, which plays on at 0 under
/// pause and game over, and pause's loop while pause is on top; the wisps'
/// shared hum, whose volume follows the nearest wisp; and the combat layer
/// over the exploration while an enemy sees the player.
#[derive(Debug, Clone, Copy, Default)]
pub struct Music {
    pub explore: Option<Voice>,
    pub pause: Option<Voice>,
    pub hum: Option<Voice>,
    /// The hum's volume last set.
    pub hum_level: f32,
    pub combat: Option<Voice>,
    /// The combat layer's volume last set.
    pub combat_level: f32,
    pub ducked: bool,
}

/// The areas' ambience loops playing, each its manifest ID, its voice and
/// the volume last set: every loop the map's areas name, started silent
/// with the run, the current area's raised.
#[derive(Debug, Clone, Default)]
pub struct Ambience {
    pub loops: Vec<(String, Voice, f32)>,
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
    /// A steam burst: cold on fire, or a fire hazard put out.
    Steamed,
    /// Thermal shock: a frozen thing exploding.
    FrostBlasted,
    /// A lightning strike's thunder.
    Thundered,
    /// A conduction jump's crackle.
    Crackled,
    /// A gun's shot.
    Shot,
    /// The gun's magazine come back whole.
    Reloaded,
    /// A bullet turned off iron.
    Ricocheted,
    /// A dummy or an enemy summoned.
    Summoned,
    /// A wisp's alert: its flame flaring.
    WispFlare,
    /// A crawler's tell before its lunge.
    CrawlerHiss,
    /// A crawler's lunge.
    CrawlerLunge,
    /// A beetle's paw before its charge.
    BeetlePaw,
    /// A beetle's charge.
    BeetleCharge,
    /// A bat's screech with its white flash before its dive.
    BatScreech,
    /// A bat's dive.
    BatDive,
    /// A troll's foot landing as it walks.
    TrollStep,
    /// A troll's roar on sight.
    TrollRoar,
    /// A troll's slam.
    TrollSlam,
    /// A troll's throw.
    TrollThrow,
    /// A shot through a wooden deck.
    Splintered,
    /// The player hurt by a blow, never by a tick (X1).
    PlayerHurt,
    /// The staff light guttering out for want of mana.
    StaffGuttered,
    /// A crawler shrinking from the light's edge.
    CrawlerScared,
    /// The player killed, by a blow or a tick.
    PlayerDied,
    /// Lava crusting over under the ice spray.
    LavaSet,
    /// The mine's elevator setting off, and arriving.
    LiftStarted,
    LiftStopped,
}

impl Sound {
    /// Every sound, in order.
    pub const ALL: [Sound; 40] = [
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
        Sound::Steamed,
        Sound::FrostBlasted,
        Sound::Thundered,
        Sound::Crackled,
        Sound::Shot,
        Sound::Reloaded,
        Sound::Ricocheted,
        Sound::Summoned,
        Sound::WispFlare,
        Sound::CrawlerHiss,
        Sound::CrawlerLunge,
        Sound::BeetlePaw,
        Sound::BeetleCharge,
        Sound::BatScreech,
        Sound::BatDive,
        Sound::TrollStep,
        Sound::TrollRoar,
        Sound::TrollSlam,
        Sound::TrollThrow,
        Sound::Splintered,
        Sound::PlayerHurt,
        Sound::StaffGuttered,
        Sound::CrawlerScared,
        Sound::PlayerDied,
        Sound::LavaSet,
        Sound::LiftStarted,
        Sound::LiftStopped,
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
            Sound::Steamed => "steamed",
            Sound::FrostBlasted => "frost_blasted",
            Sound::Thundered => "thundered",
            Sound::Crackled => "crackled",
            Sound::Shot => "shot",
            Sound::Reloaded => "reloaded",
            Sound::Ricocheted => "ricocheted",
            Sound::Summoned => "summoned",
            Sound::WispFlare => "wisp_flared",
            Sound::CrawlerHiss => "crawler_hissed",
            Sound::CrawlerLunge => "crawler_lunged",
            Sound::BeetlePaw => "beetle_pawed",
            Sound::BeetleCharge => "beetle_charged",
            Sound::BatScreech => "bat_screeched",
            Sound::BatDive => "bat_dived",
            Sound::TrollStep => "troll_stepped",
            Sound::TrollRoar => "troll_roared",
            Sound::TrollSlam => "troll_slammed",
            Sound::TrollThrow => "troll_threw",
            Sound::Splintered => "splintered",
            Sound::PlayerHurt => "player_hurt",
            Sound::StaffGuttered => "staff_guttered",
            Sound::CrawlerScared => "crawler_scared",
            Sound::PlayerDied => "player_died",
            Sound::LavaSet => "lava_set",
            Sound::LiftStarted => "lift_started",
            Sound::LiftStopped => "lift_stopped",
        }
    }
}
