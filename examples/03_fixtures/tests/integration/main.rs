//! The fixtures crate's integration tests in one binary (redo 7.1), one
//! module per former binary: `screens` the `ui` scene on the headless
//! harness, and the opt-in GPU regressions `post_regression`,
//! `transition_regression` and `ui_regression`, gated on
//! `TUNGSTEN_VISUAL_REGRESSION=1`. Each regression module runs its captures
//! in parallel with no lock, so `just visual` runs one module at a time.

mod post_regression;
mod screens;
mod transition_regression;
mod ui_regression;
