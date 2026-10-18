//! Damage: health, hits, the hit flash, the pipeline that resolves damage
//! and heal requests, the defences between a request and health, the
//! damage types a game declares, the effects a hit or a cue leaves, and
//! the ailment ladders a type's hits climb.

pub(crate) mod ailments;
pub(crate) mod defense;
pub(crate) mod effects;
pub(crate) mod health;
pub(crate) mod kinds;
pub(crate) mod pipeline;
