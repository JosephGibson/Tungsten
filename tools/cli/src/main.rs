//! `tungsten`, the engine's command line for a game project (`D-182`):
//! `new` starts a game from the engine's template, `check` loads every file
//! the game reads and reports every error, `package` builds the game's
//! release archives, `run` runs it from a script, `render` renders it to PNG
//! and `describe` prints its components, script hooks and asset IDs (redo
//! 7.2).

mod args;
mod cargo;
mod check;
mod describe;
mod elf;
mod new;
mod notices;
mod package;
mod render;
mod run;

/// The files `build.rs` embeds, by their path in the engine's repository.
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));

    /// The embedded file at `path`.
    pub(crate) fn file(path: &str) -> Option<&'static [u8]> {
        FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, data)| *data)
    }
}

use std::process::ExitCode;

use args::{Command, USAGE};

fn main() -> ExitCode {
    let command = match args::parse(std::env::args_os().skip(1)) {
        Ok(command) => command,
        Err(args::UsageError(reason)) => {
            eprintln!("tungsten: {reason}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match command {
        Command::Help => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Command::Version => {
            println!("tungsten {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::New(path) => finish("new", new::run(&path)),
        Command::Check => finish("check", current_dir().and_then(|dir| check::run(&dir))),
        Command::Package => finish("package", current_dir().and_then(|dir| package::run(&dir))),
        Command::Run(args) => finish("run", current_dir().and_then(|dir| run::run(&dir, &args))),
        Command::Render(args) => finish(
            "render",
            current_dir().and_then(|dir| render::run(&dir, &args)),
        ),
        Command::Describe(args) => finish(
            "describe",
            current_dir().and_then(|dir| describe::run(&dir, &args)),
        ),
    }
}

/// The current folder, the game's for every command but `new`.
fn current_dir() -> anyhow::Result<std::path::PathBuf> {
    std::env::current_dir().map_err(|e| anyhow::anyhow!("no current folder: {e}"))
}

/// A command's exit: 0 when it succeeded, 1 when it found errors or failed,
/// its failure printed first.
fn finish(command: &str, result: anyhow::Result<bool>) -> ExitCode {
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("tungsten {command}: {e:#}");
            ExitCode::FAILURE
        }
    }
}
