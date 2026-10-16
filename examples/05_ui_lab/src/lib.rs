//! Example 05, the UI lab: a reference for a game's UI on the engine's core
//! UI. Each module shows one pattern; copy the one you need.
//!
//! - [`theme`]: every look and timing value in one place, and the theme
//!   asset's apply (`D-148`).
//! - [`widgets`]: small builders over `UiTree` that the screens share.
//! - [`hud`]: pointer-transparent HUD rows written without allocating.
//! - [`labels`]: world-anchored text (`D-161`): a fixed set and damage
//!   numbers.
//! - [`controls`]: toggles and sliders read through `UiEvent::ValueChanged`
//!   (`D-162`).
//! - [`menus`]: state-owned screens with fades, prompts, rebinding and the
//!   settings apply (`D-163`, `D-164`, `D-159`).
//! - [`inventory`]: a slot grid, a detail pane and a drop confirmation
//!   driven by every device, with a modal screen's hotkeys.
//! - [`budget`]: the 100-widget menu the prewarm budget measures.
//! - [`screens`]: the paint list's batch partition, the inventory under its
//!   modal confirmation (`D-147`, `D-149`).
//! - [`fixtures`]: driving the UI from scripts for smoke, pixel tests and
//!   perf.
//!
//! The binary (`main.rs`) wires them into an `App`; the library lets the
//! headless tests build the same trees.

pub mod budget;
pub mod controls;
pub mod fixtures;
pub mod hud;
pub mod inventory;
pub mod labels;
pub mod menus;
pub mod screens;
pub mod theme;
pub mod widgets;
