//! What an entity means to do: input read into an [`Intent`](action_input::Intent),
//! per-entity state machines, the cursor in world space, the direction it
//! aims in and the abilities it uses.

pub(crate) mod ability;
pub(crate) mod action_input;
pub(crate) mod aim;
pub(crate) mod cursor;
pub(crate) mod state_machine;
