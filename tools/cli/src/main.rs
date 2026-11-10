//! `tungsten`, the engine's command line for a game project (`D-182`):
//! `new` starts a game from the engine's template, `check` loads every file
//! the game reads and reports every error, `package` builds the game's
//! release archives, `run` runs it from a script, `render` renders it to PNG
//! and `describe` prints its components, script hooks and asset IDs (redo
//! 7.2); `schema` prints a file format's JSON Schema (redo 7.20); `level`
//! edits, reads, checks and renders its rooms (redo 7.12).

mod args;
mod cargo;
mod check;
mod describe;
mod elf;
mod level;
mod new;
mod notices;
mod package;
mod render;
mod run;
mod schema;

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

use args::Command;

/// The exit code of a command whose reader closed the pipe: 128 + SIGPIPE,
/// as a tool the signal ends exits.
const CLOSED_PIPE: i32 = 141;

/// Ends the command quietly when its reader closes the pipe early
/// (`tungsten level map cave | head`): `println!` panics on a closed
/// stdout, and this hook exits in the panic's place, with no message.
fn quiet_on_a_closed_pipe() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let closed = info
            .payload()
            .downcast_ref::<String>()
            .is_some_and(|message| message.starts_with("failed printing to stdout"));
        if closed {
            std::process::exit(CLOSED_PIPE);
        }
        default(info);
    }));
}

fn main() -> ExitCode {
    quiet_on_a_closed_pipe();
    let arguments: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let command = match args::parse(arguments.clone()) {
        Ok(command) => command,
        Err(args::UsageError(reason)) => {
            // The reason, then its command's own usage: a mistake is
            // answered in a few lines, and `--help` prints the rest.
            eprintln!("tungsten: {reason}\n\n{}", args::usage_for(&arguments));
            return ExitCode::from(2);
        }
    };
    match command {
        Command::Help(topic) => {
            println!("{}", args::help(&topic));
            ExitCode::SUCCESS
        }
        Command::Version => {
            println!("tungsten {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::New(path) => finish("new", new::run(&path)),
        Command::Check(seeds) => finish(
            "check",
            current_dir().and_then(|dir| check::run(&dir, seeds)),
        ),
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
        Command::Schema(args) => finish("schema", schema::run(&args)),
        Command::Level(args) => finish(
            "level",
            current_dir().and_then(|dir| level::run(&dir, &args)),
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
