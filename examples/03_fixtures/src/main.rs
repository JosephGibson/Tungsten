//! The fixtures crate's binary: runs the scene `TUNGSTEN_FIXTURE` names
//! ([`example_03_fixtures::run`]).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    example_03_fixtures::run()
}
