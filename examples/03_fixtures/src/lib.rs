//! Test-only fixtures: the scenes behind the smoke rows and the pixel tests
//! of `just visual` (`D-193`). None of it shows how to write a game, and no
//! game copies it: the template's shell is the UI reference.
//!
//! `TUNGSTEN_FIXTURE` picks the scene at startup; unset means `ui`. Each scene
//! keeps reading its own variables, which its module lists, and ignores the
//! others'.
//!
//! - `post` ([`post`]): the stock post stack, SMAA, bloom, game feel, a mesh
//!   particle trail and material pairs (`TUNGSTEN_POST_STACK_FIXTURE`,
//!   `TUNGSTEN_POST_AA_FIXTURE`, `TUNGSTEN_BLOOM_FIXTURE`,
//!   `TUNGSTEN_GAME_FEEL_FIXTURE`, `TUNGSTEN_MESH_TRAIL_FIXTURE`,
//!   `TUNGSTEN_MATERIAL_PAIR_FIXTURE`).
//! - `transition` ([`transition`]): state changes behind the screen
//!   transitions, and the frame-cap row's plain run
//!   (`TUNGSTEN_TRANSITION_FIXTURE`).
//! - `ui` ([`ui`]): the UI overlay's paint path, world labels, gameplay
//!   screens, debug views and the prewarm budget menu
//!   (`TUNGSTEN_UI_LAB_FIXTURE`, `TUNGSTEN_UI_LAB_POST`,
//!   `TUNGSTEN_UI_LAB_BOUNDS`, `TUNGSTEN_UI_LAB_MAXIMIZE_WAIT`).
//! - `lighting` ([`lighting`]): lit and unlit sprites under point and
//!   directional lights, the M29 smoke row; no variables of its own.
//!
//! Like `02_bench`, every scene runs from the repository root on the root
//! `tungsten.json` and `input.json`, and composes the root manifest with
//! `examples/03_fixtures/assets/manifest.json`.

pub mod lighting;
pub mod post;
pub mod transition;
pub mod ui;

/// The scenes `TUNGSTEN_FIXTURE` accepts.
const SCENES: &str = "post, transition, ui, lighting";

/// Runs the scene `TUNGSTEN_FIXTURE` names, `ui` when it is unset; any other
/// value fails with the list of scenes.
pub fn run() -> anyhow::Result<()> {
    let scene = std::env::var("TUNGSTEN_FIXTURE").unwrap_or_else(|_| "ui".to_owned());
    match scene.as_str() {
        "post" => post::run(),
        "transition" => transition::run(),
        "ui" => ui::run(),
        "lighting" => lighting::run(),
        _ => anyhow::bail!("invalid TUNGSTEN_FIXTURE='{scene}': expected one of {SCENES}"),
    }
}
