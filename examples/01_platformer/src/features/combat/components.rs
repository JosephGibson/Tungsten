//! Combat's components, the reactions its hints follow and the hints a
//! save has shown.

use serde::{Deserialize, Serialize};
use tungsten::core::Entity;

/// A status's particles on the target `owner`: chilled's flakes,
/// bleeding's drips, shocked's sparks, frozen's glints or burning's
/// embers, which follow it while it carries the status. Presentation,
/// never saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusEmitter {
    pub owner: Entity,
    pub look: StatusLook,
}

/// A status a target shows with particles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLook {
    Chilled,
    Bleeding,
    Shocked,
    Frozen,
    Burning,
}

impl StatusLook {
    /// Every status shown with particles.
    pub const ALL: [StatusLook; 5] = [
        StatusLook::Chilled,
        StatusLook::Bleeding,
        StatusLook::Shocked,
        StatusLook::Frozen,
        StatusLook::Burning,
    ];

    /// The status tag it shows.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            StatusLook::Chilled => super::CHILLED,
            StatusLook::Bleeding => super::BLEEDING,
            StatusLook::Shocked => super::SHOCKED,
            StatusLook::Frozen => super::FROZEN,
            StatusLook::Burning => super::BURNING,
        }
    }

    /// Its particle config.
    #[must_use]
    pub const fn config(self) -> &'static str {
        match self {
            StatusLook::Chilled => "ex10_chill_flakes",
            StatusLook::Bleeding => "ex10_bleed_drips",
            StatusLook::Shocked => "ex10_shock_sparks",
            StatusLook::Frozen => "ex10_frozen_glints",
            StatusLook::Burning => "ex10_burn_embers",
        }
    }
}

/// A reaction somewhere in the run, an event the element rules and combat's
/// reactions send: a freeze, a shatter, a steam burst or a shock. The
/// hints toast each kind's first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reaction {
    Freeze,
    Shatter,
    Steam,
    Shock,
}

impl Reaction {
    /// Its hint's text.
    #[must_use]
    pub const fn hint(self) -> &'static str {
        match self {
            Reaction::Freeze => "Frozen: fire makes it explode, a hard hit shatters it",
            Reaction::Shatter => "Shattered: a hard hit breaks what is frozen",
            Reaction::Steam => "Steam: cold on fire bursts and puts it out",
            Reaction::Shock => "Shocked: it takes half again from every hit for a while",
        }
    }
}

/// The reactions whose hints a save has shown, in the order they were: a
/// World resource a run's start inserts, saved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    pub seen: Vec<Reaction>,
}
