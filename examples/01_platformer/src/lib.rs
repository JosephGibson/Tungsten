//! Example 01, a platformer sandbox on the template: `main.rs` builds the
//! app; `game` and `save` are the seam the shell calls; `shell` holds the
//! screens every game keeps, copied byte for byte from the template;
//! `features` holds the game, one folder per part, and `shared` what two
//! parts share. The library lets `tests/` drive the same code on the
//! engine's headless harness.

pub mod features;
pub mod game;
pub mod save;
pub mod shared;
pub mod shell;
