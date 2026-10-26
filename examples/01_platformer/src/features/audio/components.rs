//! The audio's resources and the game's sound events.

use glam::Vec2;
use tungsten::core::{Entity, Voice};

/// The music playing: the exploration loop, which plays on at 0 under
/// pause and game over, and pause's loop while pause is on top; the wisps'
/// shared hum, whose volume follows the nearest wisp, the black holes', the
/// nearest open hole, and the burning crates' crackle, the nearest burning
/// crate; and the combat layer over the exploration while an enemy sees
/// the player.
#[derive(Debug, Clone, Copy, Default)]
pub struct Music {
    pub explore: Option<Voice>,
    pub pause: Option<Voice>,
    pub hum: Option<Voice>,
    /// The hum's volume last set.
    pub hum_level: f32,
    pub holes: Option<Voice>,
    /// The holes' hum's volume last set.
    pub holes_level: f32,
    pub crackle: Option<Voice>,
    /// The fires' crackle's volume last set.
    pub crackle_level: f32,
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
    /// A black hole closing, by its time or the cap.
    HoleCollapsed,
    /// An iron block landing hard.
    IronLanded,
    /// A cast with no room: a toy at its cap, a body that does not fit or
    /// no cursor.
    Fizzled,
    /// A strike's snap under its thunder.
    Snapped,
    /// A frozen target thawing.
    Thawed,
    /// A bowling ball landing hard.
    BallThudded,
    /// The trigger pulled on an empty magazine or during a reload.
    DryFired,
    /// A reload begun.
    Racked,
    /// The flamethrower let go.
    FireEnded,
    /// The courier's air jump.
    AirJumped,
    /// A cast refused for want of mana.
    NoMana,
    /// A spell or a summon's type chosen.
    Switched,
    /// A cast refused while its spell cools or has no charge.
    NotReady,
    /// An orb dropped by a kill.
    OrbDropped,
    /// A crate catching fire.
    CrateIgnited,
    /// A burnt crate falling in.
    CrateCollapsed,
    /// A keg's or a bomb's fuse lit.
    FuseLit,
    /// A keg's or a bomb's detonation.
    KegBlasted,
    /// A goblin's throw, a rock or a bomb.
    RockThrown,
    /// A goblin's rock clacking on a hard landing, or crumbling.
    RockHit,
    /// A goblin's cackle on its alert.
    GoblinCackled,
    /// A troll's swipe.
    TrollSwiped,
    /// A troll's grunt as it winds up its slam.
    TrollGrunted,
    /// An enemy's alert from its patrol or its way home: the sting.
    Alerted,
    /// A goblin's stab with its knife.
    GoblinStabbed,
    /// Lava taking what fell in: its sizzle.
    LavaTook,
    /// Lava's freeze starting under the ice spray, its crust cracking at
    /// each of its three stages, and its melt (01 (k)).
    LavaChilled,
    LavaCracked,
    LavaMelted,
    /// A tome taken (01 (l)): the pact, bounding and arcane power, each
    /// its own.
    TomePactTaken,
    TomeBoundingTaken,
    TomeArcaneTaken,
    /// An air jump paid for in mana under the tome of bounding.
    Bounded,
}

impl Sound {
    /// Every sound, in order.
    pub const ALL: [Sound; 73] = [
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
        Sound::HoleCollapsed,
        Sound::IronLanded,
        Sound::Fizzled,
        Sound::Snapped,
        Sound::Thawed,
        Sound::BallThudded,
        Sound::DryFired,
        Sound::Racked,
        Sound::FireEnded,
        Sound::AirJumped,
        Sound::NoMana,
        Sound::Switched,
        Sound::NotReady,
        Sound::OrbDropped,
        Sound::CrateIgnited,
        Sound::CrateCollapsed,
        Sound::FuseLit,
        Sound::KegBlasted,
        Sound::RockThrown,
        Sound::RockHit,
        Sound::GoblinCackled,
        Sound::TrollSwiped,
        Sound::TrollGrunted,
        Sound::Alerted,
        Sound::GoblinStabbed,
        Sound::LavaTook,
        Sound::LavaChilled,
        Sound::LavaCracked,
        Sound::LavaMelted,
        Sound::TomePactTaken,
        Sound::TomeBoundingTaken,
        Sound::TomeArcaneTaken,
        Sound::Bounded,
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
            Sound::HoleCollapsed => "hole_collapsed",
            Sound::IronLanded => "iron_landed",
            Sound::Fizzled => "fizzled",
            Sound::Snapped => "snapped",
            Sound::Thawed => "thawed",
            Sound::BallThudded => "ball_thudded",
            Sound::DryFired => "dry_fired",
            Sound::Racked => "racked",
            Sound::FireEnded => "fire_ended",
            Sound::AirJumped => "air_jumped",
            Sound::NoMana => "no_mana",
            Sound::Switched => "switched",
            Sound::NotReady => "not_ready",
            Sound::OrbDropped => "orb_dropped",
            Sound::CrateIgnited => "crate_ignited",
            Sound::CrateCollapsed => "crate_collapsed",
            Sound::FuseLit => "fuse_lit",
            Sound::KegBlasted => "keg_blasted",
            Sound::RockThrown => "rock_thrown",
            Sound::RockHit => "rock_hit",
            Sound::GoblinCackled => "goblin_cackled",
            Sound::TrollSwiped => "troll_swiped",
            Sound::TrollGrunted => "troll_grunted",
            Sound::Alerted => "alerted",
            Sound::GoblinStabbed => "goblin_stabbed",
            Sound::LavaTook => "lava_took",
            Sound::LavaChilled => "lava_chilled",
            Sound::LavaCracked => "lava_cracked",
            Sound::LavaMelted => "lava_melted",
            Sound::TomePactTaken => "tome_pact_taken",
            Sound::TomeBoundingTaken => "tome_bounding_taken",
            Sound::TomeArcaneTaken => "tome_arcane_taken",
            Sound::Bounded => "bounded",
        }
    }
}
