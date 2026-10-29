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
  run         Run the game in its window from a script: inputs at frames,
              captures, probes and asserts; print what failed, and exit 0
              when the script passed
  describe [<name>]
              Print the game's components, script hooks and asset IDs, or
              only the entries named <name>

Options:
  -h, --help     Print this help
  -V, --version  Print the version

Options of run:
  --script <file>   The script to follow (required)
  --report <file>   Where the report goes (default: a temporary file)
  --user-dir <dir>  The game's user folder (default: a fresh temporary
                    folder, removed after)
  --release         Run the game's release build

Options of describe:
  --json            Print the description as JSON

Run every command but new in the game's folder. They need the game's
engine to be the version of this command line.";

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
    /// Run the game in the current folder from a script.
    Run(RunArgs),
    /// Describe the game in the current folder.
    Describe(DescribeArgs),
}

/// `run`'s options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunArgs {
    pub(crate) script: PathBuf,
    pub(crate) report: Option<PathBuf>,
    pub(crate) user_dir: Option<PathBuf>,
    pub(crate) release: bool,
}

/// `describe`'s name and option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DescribeArgs {
    pub(crate) name: Option<String>,
    pub(crate) json: bool,
}

/// Arguments that ask for no command: the reason, printed above the usage.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UsageError(pub(crate) String);

/// An option's name and the value it takes, if any (`<file>`).
type OptionSpec = (&'static str, Option<&'static str>);

/// Each command's options.
const OPTIONS: &[(&str, &[OptionSpec])] = &[
    (
        "run",
        &[
            ("--script", Some("<file>")),
            ("--report", Some("<file>")),
            ("--user-dir", Some("<dir>")),
            ("--release", None),
        ],
    ),
    ("describe", &[("--json", None)]),
];

/// Parses the arguments after the program's name. `-h` or `--help`
/// anywhere asks for the usage, then `-V` or `--version` anywhere for the
/// version; then the command, then its own arguments. An option of another
/// command, or one before the command, names the command it belongs to;
/// any other argument starting with `-` is an unknown option.
pub(crate) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, UsageError> {
    let args: Vec<OsString> = args.into_iter().collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        return Ok(Command::Help);
    }
    if args.iter().any(|arg| arg == "-V" || arg == "--version") {
        return Ok(Command::Version);
    }
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Err(UsageError("no command given".to_owned()));
    };
    if is_option(&command) {
        return Err(misplaced(&command, None));
    }
    let rest: Vec<OsString> = args.collect();
    let name = command.to_string_lossy().into_owned();
    let (positional, options) = match name.as_str() {
        "new" | "check" | "package" | "run" | "describe" => split(&name, rest)?,
        _ => return Err(UsageError(format!("unknown command `{name}`"))),
    };
    let flag = |option: &str| options.iter().any(|(o, _)| *o == option);
    let value = |option: &str| {
        options
            .iter()
            .find(|(o, _)| *o == option)
            .and_then(|(_, v)| v.clone())
            .map(PathBuf::from)
    };
    match name.as_str() {
        "new" => match <[OsString; 1]>::try_from(positional) {
            Ok([path]) => Ok(Command::New(PathBuf::from(path))),
            Err(rest) if rest.is_empty() => {
                Err(UsageError("`new` needs the new game's <path>".to_owned()))
            }
            Err(rest) => Err(extra("new", &rest[1..])),
        },
        "check" | "package" | "run" if !positional.is_empty() => Err(extra(&name, &positional)),
        "check" => Ok(Command::Check),
        "package" => Ok(Command::Package),
        "run" => Ok(Command::Run(RunArgs {
            script: value("--script")
                .ok_or_else(|| UsageError("`run` needs --script <file>".to_owned()))?,
            report: value("--report"),
            user_dir: value("--user-dir"),
            release: flag("--release"),
        })),
        _ => match <[OsString; 1]>::try_from(positional) {
            Err(rest) if rest.is_empty() => Ok(Command::Describe(DescribeArgs {
                name: None,
                json: flag("--json"),
            })),
            Ok([name]) => Ok(Command::Describe(DescribeArgs {
                name: Some(name.to_string_lossy().into_owned()),
                json: flag("--json"),
            })),
            Err(rest) => Err(extra("describe", &rest[1..])),
        },
    }
}

/// An option and its value.
type Given = (&'static str, Option<OsString>);

/// `command`'s arguments as its positionals and its options in order: each
/// of its own options once, with its value when it takes one.
fn split(command: &str, rest: Vec<OsString>) -> Result<(Vec<OsString>, Vec<Given>), UsageError> {
    let own: &[OptionSpec] = OPTIONS
        .iter()
        .find(|(name, _)| *name == command)
        .map_or(&[], |(_, options)| options);
    let mut positional = Vec::new();
    let mut options: Vec<Given> = Vec::new();
    let mut args = rest.into_iter();
    while let Some(arg) = args.next() {
        if !is_option(&arg) {
            positional.push(arg);
            continue;
        }
        let Some(&(option, takes)) = own.iter().find(|(name, _)| arg == *name) else {
            return Err(misplaced(&arg, Some(command)));
        };
        if options.iter().any(|(given, _)| *given == option) {
            return Err(UsageError(format!("`{option}` is given twice")));
        }
        let value = match takes {
            None => None,
            Some(what) => Some(
                args.next()
                    .filter(|value| !is_option(value))
                    .ok_or_else(|| UsageError(format!("`{option}` needs a {what}")))?,
            ),
        };
        options.push((option, value));
    }
    Ok((positional, options))
}

fn is_option(arg: &OsString) -> bool {
    arg.as_encoded_bytes().starts_with(b"-")
}

/// The error for `option` given to `command`, or before any command: the
/// command or commands it belongs to, or that it is unknown.
fn misplaced(option: &OsString, command: Option<&str>) -> UsageError {
    let option = option.to_string_lossy();
    let owners: Vec<String> = OPTIONS
        .iter()
        .filter(|(_, options)| options.iter().any(|(name, _)| *name == option))
        .map(|(name, _)| format!("`{name}`"))
        .collect();
    match (owners.is_empty(), command) {
        (true, _) => UsageError(format!("unknown option `{option}`")),
        (false, Some(command)) => UsageError(format!(
            "`{option}` is an option of {}, not of `{command}`",
            owners.join(" and ")
        )),
        (false, None) => UsageError(format!(
            "`{option}` goes after its command: {}",
            owners.join(" or ")
        )),
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
