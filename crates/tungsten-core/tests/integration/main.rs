//! Core's integration tests in one binary (redo 7.1), one module per former
//! binary: `audio_decode` the decoders, `composition` the asset composition
//! contract, `decision_index` the decision records, `display` the config's
//! display section, `manifests` every manifest in the repository, and the
//! three physics harnesses, `physics_containment` and `physics_determinism`
//! in release only (`just physics-release`) and `physics_tunneling`; `common`
//! holds the first two's spawn helpers.
//!
//! Only `display` changes the environment, under its `ENV_LOCK`; a module
//! that reads the environment takes that lock too.

mod audio_decode;
mod common;
mod composition;
mod decision_index;
mod display;
mod manifests;
mod physics_containment;
mod physics_determinism;
mod physics_tunneling;
