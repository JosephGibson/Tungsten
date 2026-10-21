//! The game, one folder per part: each `mod.rs` holds the part's plugin,
//! state and constants, `components.rs` its components and resources and
//! `systems.rs` its systems. `game::register` adds the plugins.

pub mod audio;
pub mod elements;
pub mod level;
pub mod player;
pub mod run;
pub mod spells;
pub mod toys;
