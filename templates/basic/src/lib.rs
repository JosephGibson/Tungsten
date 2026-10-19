//! A Tungsten game: `main.rs` builds the app; `game` and `save` are the seam
//! the shell calls; `shell` holds the screens every game keeps, and
//! `sample` the demo a new game deletes. The library lets `tests/` drive the
//! same code on the engine's headless harness.

pub mod game;
pub mod sample;
pub mod save;
pub mod shell;
