//! What an entity means to do: input read into an [`Intent`](action_input::Intent),
//! per-entity state machines and the cursor in world space.

pub(crate) mod action_input;
pub(crate) mod cursor;
pub(crate) mod state_machine;
