//! What an entity means to do: input read into an [`Intent`](action_input::Intent),
//! per-entity state machines, the cursor in world space, the direction it
//! aims in, the abilities it uses and their skill sheets.

pub(crate) mod ability;
pub(crate) mod action_input;
pub(crate) mod aim;
pub(crate) mod cursor;
pub(crate) mod sheet;
pub(crate) mod state_machine;
