//! Combat's components: the statuses' particles.

use tungsten::core::Entity;

/// A status's particles on the target `owner`: chilled's flakes,
/// bleeding's drips, shocked's sparks, frozen's glints or burning's
/// embers, which follow it while it carries the status. A freezable
/// creature's flakes come from the first point of cold by its frost's
/// `stage`, 1 to 4, each stage an emitter of its own; 0 for every other.
/// Presentation, never saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusEmitter {
    pub owner: Entity,
    pub look: StatusLook,
    pub stage: u8,
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
