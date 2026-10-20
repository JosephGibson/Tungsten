//! The command line's arguments: the promised commands and options
//! (`D-103`, `D-182`), parsed by hand.

use std::ffi::OsString;
use std::path::PathBuf;

/// `tungsten --help`, which `api/tungsten-cli.txt` records.
pub(crate) const USAGE: &str = "\
Usage: tungsten <command>

Commands:
  new <path>  Make a game in the new folder <path> from the engine's
              template, named after the folder, with the engine pinned to
              this command line's release tag
  check       Load the game's tungsten.json, input.json and every file its
              manifests name with the engine's loaders, in the game's own
              build, and report every error with its file
  package     Build the game for release, check it, and write its players'
              archive and its debug archive to dist/ (Linux)

Options:
  -h, --help     Print this help
  -V, --version  Print the version

Run check and package in the game's folder. They need the game's engine
to be the version of this command line.";

/// What the arguments ask for.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// Print the usage.
    Help,
    /// Print `tungsten <version>`.
    Version,
    /// Make a game in the new folder.
    New(PathBuf),
    /// Check the game in the current folder.
    Check,
    /// Package the game in the current folder.
    Package,
}

/// Arguments that ask for no command: the reason, printed above the usage.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UsageError(pub(crate) String);

/// Parses the arguments after the program's name. `-h` or `--help`
/// anywhere asks for the usage, then `-V` or `--version` anywhere for the
/// version; any other argument starting with `-` is an unknown option.
pub(crate) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, UsageError> {
    let args: Vec<OsString> = args.into_iter().collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        return Ok(Command::Help);
    }
    if args.iter().any(|arg| arg == "-V" || arg == "--version") {
        return Ok(Command::Version);
    }
    if let Some(option) = args
        .iter()
        .find(|arg| arg.as_encoded_bytes().starts_with(b"-"))
    {
        return Err(UsageError(format!(
            "unknown option `{}`",
            option.to_string_lossy()
        )));
    }
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Err(UsageError("no command given".to_owned()));
    };
    let rest: Vec<OsString> = args.collect();
    let command = match command.to_str() {
        Some("new") => match <[OsString; 1]>::try_from(rest) {
            Ok([path]) => return Ok(Command::New(PathBuf::from(path))),
            Err(rest) if rest.is_empty() => {
                return Err(UsageError("`new` needs the new game's <path>".to_owned()));
            }
            Err(rest) => return Err(extra("new", &rest[1..])),
        },
        Some("check") => Command::Check,
        Some("package") => Command::Package,
        _ => {
            return Err(UsageError(format!(
                "unknown command `{}`",
                command.to_string_lossy()
            )));
        }
    };
    if rest.is_empty() {
        Ok(command)
    } else {
        let name = if command == Command::Check {
            "check"
        } else {
            "package"
        };
        Err(extra(name, &rest))
    }
}

/// The error for arguments past what `command` takes.
fn extra(command: &str, rest: &[OsString]) -> UsageError {
    let rest: Vec<String> = rest
        .iter()
        .map(|arg| format!("`{}`", arg.to_string_lossy()))
        .collect();
    UsageError(format!(
        "`{command}` takes no further argument, given {}",
        rest.join(" ")
    ))
}

#[cfg(test)]
#[path = "tests/args.rs"]
mod tests;
