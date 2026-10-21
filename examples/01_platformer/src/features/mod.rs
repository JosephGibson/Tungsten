//! The game, one folder per part: each `mod.rs` holds the part's plugin,
//! state and constants, `components.rs` its components and resources and
//! `systems.rs` its systems. `game::register` adds the plugins.

pub mod audio;
pub mod combat;
pub mod dummy;
pub mod elements;
pub mod enemies;
pub mod gun;
pub mod level;
pub mod lightning;
pub mod player;
pub mod run;
pub mod spells;
pub mod toys;
