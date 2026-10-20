//! `tungsten`, the engine's command line for a game project (`D-182`):
//! `new` starts a game from the engine's template, `check` loads every file
//! the game reads and reports every error, and `package` builds the game's
//! release archives.

mod args;
mod cargo;
mod check;
mod elf;
mod new;
mod notices;
mod package;

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
    }
}

/// The current folder, the game's for `check` and `package`.
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
