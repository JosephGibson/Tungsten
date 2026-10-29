//! The umbrella's integration tests in one binary (redo 7.1), one module per
//! former binary: `asset_smoke` loads the repository's assets headless,
//! `atlas_integration` the extract's atlas batches, `camera` the camera
//! systems and `particles` the particle stage end to end.

mod asset_smoke;
mod atlas_integration;
mod camera;
mod particles;
