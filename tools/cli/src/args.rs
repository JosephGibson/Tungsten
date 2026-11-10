//! The command line's arguments: the promised commands and options
//! (`D-103`, `D-182`), parsed by hand.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::check::Seeds;
use crate::level::{ApplyEdit, DEFAULT_SIZE, LevelArgs, NewArgs};
use crate::render::{self, RenderArgs};

/// The `level` verbs with their options, as `tungsten --help` and
/// `tungsten level --help` both print them.
macro_rules! level_verbs {
    () => {
        "\
Verbs of level, a <room> a rooms ID or a room file:
  new <room>        Make a room file with a render layer and a collision
                    layer, and list it in the manifest
    --size <cols>x<rows>
                    Its size in cells (default 40x24)
    --cell <px>     A cell's size, <px> or <w>x<h> (default: that of the
                    tileset's other rooms, else 16)
    --tileset <id>  Its tileset (default: the game's only one; with none,
                    a new one, tiles)
    --tile <sprite> A tile of a new tileset, by its sprite; repeatable
    --dry-run       Report what would be written; write nothing
  apply <room> <batch>
                    Apply a batch of commands (a file, or - for stdin),
                    write the room canonically and journal the batch
    --inverse <file>  Also write the batch that undoes it
    --undo          Undo the journal's newest batch instead of a batch
    --redo          Apply the newest undone batch again instead
    --dry-run       Check and report what would change; write nothing
    --check         Run level check on the room after the write
  query <room> [<id>]
                    Print every entry with its geometry, one entry by
                    its ID, or with --at what lies at a cell
    --at <col>,<row>  The cell
    --json          Print JSON
  map <room>        Print the room as text, a character a cell, with its
                    objects and places lettered and listed
    --rect <col>,<row>,<width>,<height>
                    The cells to print; a room past 200x100 needs it
  describe [<room>] Print each room's layers and named things by kind
    --json          Print JSON
  check [<room>...] Run the game's check and print the rooms' findings
                    and every room generator's, with --seeds and --seed
                    as check's
  generate <generator>
                    Write a generator's rooms as room files, every object
                    marked; a regenerated room keeps its author's objects
    --seed <seed>   The seed (default 0)
    --prefix <prefix> The rooms' ID prefix (default the generator's ID)
    --into <folder> Where new rooms go (default the manifest's rooms/)
    --regenerate    Rewrite the listed rooms it generates
  render <room>     Render the room on an empty stage to a PNG; with
                    -o, --overlay, --size, --frame, --save-script and
                    --release as render's
"
    };
}

/// The batch commands in brief, as `tungsten level --help` and `tungsten
/// level apply --help` print them.
macro_rules! batch_commands {
    () => {
        "\
Batch commands, each an object with one verb; a batch is a JSON array:
  place      {\"objects\" | \"shapes\" | \"places\" | \"layers\": <entry>,
              \"index\"?}: an entry as the room file writes it
  move       {\"id\", \"by\": [dx, dy]} or {\"id\", \"to\": [x, y]}
  set        {\"id\" | \"layer\"?, \"path\", \"value\"}; an object's override
             is \"path\": \"set.<component>.<field>\"; null removes it
  delete     {\"id\"} or {\"layer\"}
  duplicate  {\"id\", \"as\", \"by\"?}
  paint      {\"layer\", \"rect\": [col, row, w, h] | \"cells\": [[col, row]],
              \"tile\"}: a tile by its sprite, over cells of a layer
  erase      {\"layer\", \"rect\" | \"cells\"}
  link       {\"id\", \"to\": \"<room>.<place>\" | null}
  rename     {\"id\", \"to\"}
Positions are in cells from the room's top-left, x right and y down.
`tungsten schema batch` prints every field with its description and an
example, and `tungsten schema room` the room file's.
"
    };
}

/// `tungsten --help`, which `api/tungsten-cli.txt` records.
pub(crate) const USAGE: &str = concat!(
    "\
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
  render      Render the game to PNG through a script made from its flags:
              a contact sheet, a scene or a point of its world, once or as
              a loop of frames
  schema <name>
              Print the JSON Schema of a file format, from this command
              line's engine: ui_screen, ui_theme, room, tileset, batch (a
              level batch) or script (a run script)
  level <verb>
              Make, edit, read, check and render the game's rooms: new,
              apply, query, map, describe, check, generate or render,
              below

Options:
  -h, --help     Print this help
  -V, --version  Print the version

Options of run:
  --script <file>   The script to follow (required)
  --report <file>   Where the report goes (default: a temporary file)
  --user-dir <dir>  The game's user folder (default: a fresh temporary
                    folder, removed after)
  --release         Run the game's release build

Options of check and level check, one of:
  --seeds <count>   Generate each room generator's rooms at seeds 0 to
                    <count> - 1 and check them (default: 16 seeds)
  --seed <seed>     Generate and check them at <seed> alone

Options of describe:
  --json            Print the description as JSON

Options of render, one of --sheet, --scene, --at and --screen, and -o:
  --sheet <items>   Items in a captioned grid on an empty stage, by commas:
                    prefab IDs, sprite:<id>, animation:<id>, particles:<id>
  --scene <file>    A scene file on an empty stage
  --at <x>,<y>      A point of the game's world, the camera's centre
  --screen <id>     A screen file or a registered screen, opened after the
                    --after actions
  --after <action>[=<json>]
                    An action the game registered, with its arguments, at
                    frame 2, the next at 3, and so on; repeatable
  --zoom <z>        With --at: screen pixels per world pixel (default 1)
  --view <height>   With --at: world pixels from top to bottom
  --size <w>x<h>    The window's and the PNG's size (default 1280x720)
  --overlay <names> Debug overlays on: physics, kit, by commas
  --no-ui           Draw no UI
  --frame <n>       The frame to capture (default 60)
  --loop <from>..<to>/<every>
                    Capture every <every>th frame into the folder -o names
  --save-script <file>
                    Keep the script, to edit and run with `run`
  --release         Run the game's release build
  -o <path>         The PNG, or with --loop the folder of NNNN.png frames

Options of schema:
  -o <path>         Write the schema to <path> instead

",
    level_verbs!(),
    "
`tungsten level --help` adds the batch commands and a worked example.

Run every command but new and schema in the game's folder. They need
the game's engine to be the version of this command line. A command
that runs the game prints its warnings and errors and leaves out its
INFO log lines; RUST_LOG=info shows them."
);

/// `tungsten level --help`, which `api/tungsten-cli.txt` records too: a
/// worked example first, then the verbs and the batch commands.
pub(crate) const LEVEL_USAGE: &str = concat!(
    "\
Usage: tungsten level <verb> [arguments]

Make, edit, read, check and render the game's rooms. A room is a JSON file
listed in the manifest's rooms. Every edit is a batch of commands, checked
whole before anything is written, journaled, and undone and redone.

A room from nothing, on a new game's own sprites and prefabs:
  tungsten level new yard --size 24x14 --cell 32 --tile dummy
  tungsten level apply yard - <<'EOF'
  [{ \"place\": { \"shapes\": {
       \"id\": \"floor\", \"rect\": [0, 12, 24, 2], \"tile\": \"dummy\",
       \"layers\": [\"art\", \"solid\"] } } },
   { \"place\": { \"objects\": {
       \"id\": \"gem_1\", \"prefab\": \"gem\", \"at\": [5, 11] } } }]
  EOF
  tungsten level map yard
  tungsten level check yard
  tungsten level render yard -o yard.png

",
    level_verbs!(),
    "\n",
    batch_commands!(),
    "
Exit codes: 0 done, 1 an error or a finding, 2 a usage mistake."
);

/// The `level` verbs, in the order the usage lists them.
pub(crate) const LEVEL_VERBS: [&str; 8] = [
    "new", "apply", "query", "map", "describe", "check", "generate", "render",
];

/// What `--help` is asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HelpTopic {
    /// Every command: `tungsten --help`.
    Top,
    /// The level verbs: `tungsten level --help`.
    Level,
    /// One level verb: `tungsten level apply --help`.
    LevelVerb(String),
}

/// What `--help` prints for `topic`, without a final newline.
pub(crate) fn help(topic: &HelpTopic) -> String {
    match topic {
        HelpTopic::Top => USAGE.to_owned(),
        HelpTopic::Level => LEVEL_USAGE.to_owned(),
        HelpTopic::LevelVerb(verb) => {
            let mut text = format!("tungsten level {verb}:\n{}", level_verb(verb));
            if verb == "apply" {
                text.push('\n');
                text.push_str(batch_commands!());
            }
            text.push_str("\n`tungsten level --help` prints every verb and a worked example.");
            text
        }
    }
}

/// The usage a mistake in `args` is answered with: the part of the usage
/// its command, or its `level` verb, owns, then the help that prints the
/// rest, so an error costs its reader a few lines and not the whole usage.
pub(crate) fn usage_for(args: &[OsString]) -> String {
    let word = |at: usize| {
        args.get(at)
            .filter(|arg| !is_option(arg))
            .map(|arg| arg.to_string_lossy().into_owned())
    };
    match word(0).as_deref() {
        Some("level") => {
            if let Some(verb) = word(1).filter(|verb| LEVEL_VERBS.contains(&verb.as_str())) {
                return format!(
                    "{}\n`tungsten level --help` prints every verb with a worked example.",
                    level_verb(&verb)
                );
            }
            let verbs: String = LEVEL_VERBS
                .iter()
                .map(|verb| described(level_verb(verb)))
                .collect();
            format!(
                "Verbs of level:\n{verbs}\n`tungsten level --help` prints their options and a worked example."
            )
        }
        Some(command) if command_entry(command).is_some() => {
            let mut text = command_entry(command).unwrap_or_default().to_owned();
            if let Some(options) = options_of(command) {
                text.push('\n');
                text.push_str(options);
            }
            text.push_str("\n`tungsten --help` prints every command.");
            text
        }
        _ => format!(
            "{}\n`tungsten --help` prints every command's options.",
            section(USAGE, "Commands:")
        ),
    }
}

/// The lines of `text` from the one that is `heading` through the last
/// before a blank line, each with its newline.
fn section(text: &'static str, heading: &str) -> &'static str {
    let Some(start) = text
        .match_indices(heading)
        .map(|(at, _)| at)
        .find(|&at| at == 0 || text.as_bytes()[at - 1] == b'\n')
    else {
        return "";
    };
    let rest = &text[start..];
    let end = rest.find("\n\n").map_or(rest.len(), |at| at + 1);
    &rest[..end]
}

/// The lines an entry of a usage list owns: the line that starts with two
/// spaces and then `name` as a whole word, and the deeper lines after it.
fn entry(list: &'static str, name: &str) -> Option<&'static str> {
    let mut start = None;
    let mut at = 0;
    for line in list.split_inclusive('\n') {
        let own = line
            .strip_prefix("  ")
            .filter(|rest| !rest.starts_with(' '));
        match (start, own) {
            (None, Some(rest)) => {
                let named = rest
                    .strip_prefix(name)
                    .is_some_and(|after| after.is_empty() || after.starts_with([' ', '\n']));
                if named {
                    start = Some(at);
                }
            }
            // The next entry, or a line that is no part of the list.
            (Some(from), Some(_)) => return Some(&list[from..at]),
            (Some(from), None) if !line.starts_with("    ") => return Some(&list[from..at]),
            _ => {}
        }
        at += line.len();
    }
    start.map(|from| &list[from..])
}

/// A top-level command's entry in the usage's `Commands:` list.
fn command_entry(command: &str) -> Option<&'static str> {
    entry(section(USAGE, "Commands:"), command)
}

/// A top-level command's `Options of …` block in the usage, when it has
/// one: the block whose heading names it as a word.
fn options_of(command: &str) -> Option<&'static str> {
    USAGE
        .lines()
        .filter(|line| line.starts_with("Options of "))
        .find(|line| {
            line.split(|c: char| !c.is_ascii_alphanumeric())
                .skip(2)
                .take_while(|word| *word != "level")
                .any(|word| word == command)
        })
        .map(|heading| section(USAGE, heading))
}

/// A level verb's lines in the usage: its arguments, what it does and its
/// options.
fn level_verb(verb: &str) -> &'static str {
    entry(level_verbs!(), verb).unwrap_or_default()
}

/// A usage entry without its options: the lines before its first option,
/// which sits four spaces in.
fn described(entry: &'static str) -> &'static str {
    let mut end = 0;
    for line in entry.split_inclusive('\n') {
        if line.starts_with("    -") {
            break;
        }
        end += line.len();
    }
    &entry[..end]
}

/// What the arguments ask for.
#[derive(Debug, PartialEq)]
pub(crate) enum Command {
    /// Print the usage, or the part of it `--help` was asked about.
    Help(HelpTopic),
    /// Print `tungsten <version>`.
    Version,
    /// Make a game in the new folder.
    New(PathBuf),
    /// Check the game in the current folder, the room generators over the
    /// seeds given (redo 7.12).
    Check(Option<Seeds>),
    /// Package the game in the current folder.
    Package,
    /// Run the game in the current folder from a script.
    Run(RunArgs),
    /// Describe the game in the current folder.
    Describe(DescribeArgs),
    /// Render the game in the current folder to PNG.
    Render(RenderArgs),
    /// Print or write a file format's JSON Schema.
    Schema(SchemaArgs),
    /// Edit, read, check or render the game's rooms (redo 7.12).
    Level(LevelArgs),
}

/// `schema`'s format and option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SchemaArgs {
    /// The format: one of [`crate::schema::NAMES`].
    pub(crate) name: String,
    /// The file to write it to; stdout when `None`.
    pub(crate) out: Option<PathBuf>,
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
        "check",
        &[("--seeds", Some("<count>")), ("--seed", Some("<seed>"))],
    ),
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
    (
        "render",
        &[
            ("--sheet", Some("<items>")),
            ("--scene", Some("<file>")),
            ("--at", Some("<x>,<y>")),
            ("--screen", Some("<id>")),
            ("--after", Some("<action>")),
            ("--zoom", Some("<z>")),
            ("--view", Some("<height>")),
            ("--size", Some("<w>x<h>")),
            ("--overlay", Some("<names>")),
            ("--no-ui", None),
            ("--frame", Some("<n>")),
            ("--loop", Some("<from>..<to>/<every>")),
            ("--save-script", Some("<file>")),
            ("--release", None),
            ("-o", Some("<path>")),
        ],
    ),
    ("schema", &[("-o", Some("<path>"))]),
    (
        "level new",
        &[
            ("--size", Some("<cols>x<rows>")),
            ("--cell", Some("<px>")),
            ("--tileset", Some("<id>")),
            ("--tile", Some("<sprite>")),
            ("--dry-run", None),
        ],
    ),
    (
        "level map",
        &[("--rect", Some("<col>,<row>,<width>,<height>"))],
    ),
    (
        "level apply",
        &[
            ("--inverse", Some("<file>")),
            ("--undo", None),
            ("--redo", None),
            ("--dry-run", None),
            ("--check", None),
        ],
    ),
    (
        "level query",
        &[("--at", Some("<col>,<row>")), ("--json", None)],
    ),
    ("level describe", &[("--json", None)]),
    (
        "level check",
        &[("--seeds", Some("<count>")), ("--seed", Some("<seed>"))],
    ),
    (
        "level generate",
        &[
            ("--seed", Some("<seed>")),
            ("--prefix", Some("<prefix>")),
            ("--into", Some("<folder>")),
            ("--regenerate", None),
        ],
    ),
    (
        "level render",
        &[
            ("-o", Some("<path>")),
            ("--overlay", Some("<names>")),
            ("--size", Some("<w>x<h>")),
            ("--frame", Some("<n>")),
            ("--save-script", Some("<file>")),
            ("--release", None),
        ],
    ),
];

/// The options a command may give more than once.
const REPEATABLE: &[&str] = &["--after", "--tile"];

/// Parses the arguments after the program's name. `-h` or `--help`
/// anywhere asks for the usage (after `level`, for level's own, and after
/// one of its verbs for that verb's), then `-V` or `--version` anywhere for the
/// version; then the command, then its own arguments. An option of another
/// command, or one before the command, names the command it belongs to;
/// any other argument starting with `-` is an unknown option.
pub(crate) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, UsageError> {
    let args: Vec<OsString> = args.into_iter().collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        // `level --help` and `level <verb> --help` print level's own.
        let word = |at: usize| args.get(at).map(|arg| arg.to_string_lossy().into_owned());
        let topic = match (word(0).as_deref(), word(1)) {
            (Some("level"), Some(verb)) if LEVEL_VERBS.contains(&verb.as_str()) => {
                HelpTopic::LevelVerb(verb)
            }
            (Some("level"), _) => HelpTopic::Level,
            _ => HelpTopic::Top,
        };
        return Ok(Command::Help(topic));
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
    if name == "level" {
        return level(rest);
    }
    let (positional, options) = match name.as_str() {
        "new" | "check" | "package" | "run" | "describe" | "render" | "schema" => {
            split(&name, rest)?
        }
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
    let given = |option: &str| {
        options
            .iter()
            .find(|(o, _)| *o == option)
            .and_then(|(_, v)| v.as_ref())
            .map(|v| v.to_string_lossy().into_owned())
    };
    match name.as_str() {
        "new" => match <[OsString; 1]>::try_from(positional) {
            Ok([path]) => Ok(Command::New(PathBuf::from(path))),
            Err(rest) if rest.is_empty() => {
                Err(UsageError("`new` needs the new game's <path>".to_owned()))
            }
            Err(rest) => Err(extra("new", &rest[1..])),
        },
        "check" | "package" | "run" | "render" if !positional.is_empty() => {
            Err(extra(&name, &positional))
        }
        "check" => Ok(Command::Check(seeds(&given)?)),
        "package" => Ok(Command::Package),
        "render" => {
            let text = |option: &str| {
                options
                    .iter()
                    .find(|(o, _)| *o == option)
                    .and_then(|(_, v)| v.as_ref())
                    .map(|v| v.to_string_lossy().into_owned())
            };
            let texts = |option: &str| -> Vec<String> {
                options
                    .iter()
                    .filter(|(o, _)| *o == option)
                    .filter_map(|(_, v)| v.as_ref())
                    .map(|v| v.to_string_lossy().into_owned())
                    .collect()
            };
            render::args(text, texts, flag)
                .map(Command::Render)
                .map_err(UsageError)
        }
        "schema" => match <[OsString; 1]>::try_from(positional) {
            Ok([format]) => {
                let name = format.to_string_lossy().into_owned();
                if !crate::schema::NAMES.contains(&name.as_str()) {
                    return Err(UsageError(format!(
                        "unknown schema `{name}`: {}",
                        crate::schema::listed()
                    )));
                }
                Ok(Command::Schema(SchemaArgs {
                    name,
                    out: value("-o"),
                }))
            }
            Err(rest) if rest.is_empty() => Err(UsageError(format!(
                "`schema` needs a format: {}",
                crate::schema::listed()
            ))),
            Err(rest) => Err(extra("schema", &rest[1..])),
        },
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

/// `level <verb>`'s arguments.
fn level(rest: Vec<OsString>) -> Result<Command, UsageError> {
    const VERBS: &str = "new, apply, query, map, describe, check, generate or render";
    let mut rest = rest.into_iter();
    let Some(verb) = rest.next() else {
        return Err(UsageError(format!("`level` needs a verb: {VERBS}")));
    };
    if is_option(&verb) {
        return Err(misplaced(&verb, Some("level")));
    }
    let verb = verb.to_string_lossy().into_owned();
    if !LEVEL_VERBS.contains(&verb.as_str()) {
        return Err(UsageError(format!("unknown level verb `{verb}`: {VERBS}")));
    }
    let command = format!("level {verb}");
    let (positional, options) = split(&command, rest.collect())?;
    let positional: Vec<String> = positional
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let flag = |option: &str| options.iter().any(|(o, _)| *o == option);
    let text = |option: &str| {
        options
            .iter()
            .find(|(o, _)| *o == option)
            .and_then(|(_, v)| v.as_ref())
            .map(|v| v.to_string_lossy().into_owned())
    };
    let too_many = |taken: usize| {
        extra(
            &command,
            &positional[taken..]
                .iter()
                .map(OsString::from)
                .collect::<Vec<_>>(),
        )
    };
    let args = match verb.as_str() {
        "new" => {
            let size = match text("--size") {
                None => DEFAULT_SIZE,
                Some(size) => pair(&size)
                    .filter(|[w, h]| (1..=4096).contains(w) && (1..=4096).contains(h))
                    .ok_or_else(|| {
                        UsageError(format!(
                            "`--size {size}`: expected <cols>x<rows>, each 1 to 4096, as 40x24"
                        ))
                    })?,
            };
            let cell = match text("--cell") {
                None => None,
                Some(cell) => Some(
                    pair(&cell)
                        .or_else(|| cell.trim().parse().ok().map(|px| [px, px]))
                        .filter(|[w, h]| *w >= 1 && *h >= 1)
                        .ok_or_else(|| {
                            UsageError(format!(
                                "`--cell {cell}`: expected <px> or <w>x<h> in pixels, as 16 or 16x8"
                            ))
                        })?,
                ),
            };
            let tiles: Vec<String> = options
                .iter()
                .filter(|(option, _)| *option == "--tile")
                .filter_map(|(_, value)| value.as_ref())
                .map(|value| value.to_string_lossy().into_owned())
                .collect();
            match positional.as_slice() {
                [room] => LevelArgs::New(NewArgs {
                    room: room.clone(),
                    size,
                    cell,
                    tileset: text("--tileset"),
                    tiles,
                    dry_run: flag("--dry-run"),
                }),
                [] => {
                    return Err(UsageError(
                        "`level new` needs the new <room>'s ID".to_owned(),
                    ));
                }
                _ => return Err(too_many(1)),
            }
        }
        "map" => {
            let rect = match text("--rect") {
                None => None,
                Some(rect) => {
                    let numbers: Vec<u32> = rect
                        .split(',')
                        .filter_map(|part| part.trim().parse().ok())
                        .collect();
                    match (<[u32; 4]>::try_from(numbers), rect.split(',').count()) {
                        (Ok(rect), 4) => Some(rect),
                        _ => {
                            return Err(UsageError(format!(
                                "`--rect {rect}`: expected <col>,<row>,<width>,<height>, four whole numbers"
                            )));
                        }
                    }
                }
            };
            match positional.as_slice() {
                [room] => LevelArgs::Map {
                    room: room.clone(),
                    rect,
                },
                [] => return Err(UsageError("`level map` needs a <room>".to_owned())),
                _ => return Err(too_many(1)),
            }
        }
        "apply" => {
            let (undo, redo) = (flag("--undo"), flag("--redo"));
            if undo && redo {
                return Err(UsageError("give `--undo` or `--redo`, not both".to_owned()));
            }
            let history = if undo { "--undo" } else { "--redo" };
            let (room, edit) = match (positional.as_slice(), undo || redo) {
                ([room, batch], false) => (room, ApplyEdit::Batch(PathBuf::from(batch))),
                ([room], true) if undo => (room, ApplyEdit::Undo),
                ([room], true) => (room, ApplyEdit::Redo),
                ([_, _], true) => {
                    return Err(UsageError(format!("`{history}` takes no batch")));
                }
                (more, _) if more.len() > 2 => return Err(too_many(2)),
                _ => {
                    return Err(UsageError(
                        "`level apply` needs <room> <batch>, <room> --undo or <room> --redo"
                            .to_owned(),
                    ));
                }
            };
            let inverse = text("--inverse").map(PathBuf::from);
            if inverse.is_some() && (undo || redo) {
                return Err(UsageError(format!("`{history}` writes no --inverse")));
            }
            LevelArgs::Apply {
                room: room.clone(),
                edit,
                inverse,
                dry_run: flag("--dry-run"),
                check: flag("--check"),
            }
        }
        "query" => {
            let at = text("--at")
                .map(|at| cell(&at))
                .transpose()
                .map_err(UsageError)?;
            match positional.as_slice() {
                [room] => LevelArgs::Query {
                    room: room.clone(),
                    id: None,
                    at,
                    json: flag("--json"),
                },
                [room, id] => LevelArgs::Query {
                    room: room.clone(),
                    id: Some(id.clone()),
                    at,
                    json: flag("--json"),
                },
                [] => return Err(UsageError("`level query` needs a <room>".to_owned())),
                _ => return Err(too_many(2)),
            }
        }
        "describe" => match positional.as_slice() {
            [] => LevelArgs::Describe {
                room: None,
                json: flag("--json"),
            },
            [room] => LevelArgs::Describe {
                room: Some(room.clone()),
                json: flag("--json"),
            },
            _ => return Err(too_many(1)),
        },
        "check" => LevelArgs::Check {
            seeds: seeds(&text)?,
            rooms: positional,
        },
        "generate" => {
            let seed = match text("--seed") {
                None => 0,
                Some(seed) => seed
                    .trim()
                    .parse()
                    .map_err(|_| UsageError(format!("`--seed {seed}`: expected a whole number")))?,
            };
            match positional.as_slice() {
                [generator] => LevelArgs::Generate {
                    generator: generator.clone(),
                    seed,
                    prefix: text("--prefix"),
                    into: text("--into").map(PathBuf::from),
                    regenerate: flag("--regenerate"),
                },
                [] => {
                    return Err(UsageError(
                        "`level generate` needs a <generator>".to_owned(),
                    ));
                }
                _ => return Err(too_many(1)),
            }
        }
        _ => match positional.as_slice() {
            [room] => LevelArgs::Render {
                args: render::room_args(room, text, flag).map_err(UsageError)?,
            },
            [] => {
                return Err(UsageError("`level render` needs a <room>".to_owned()));
            }
            _ => return Err(too_many(1)),
        },
    };
    Ok(Command::Level(args))
}

/// `--seeds <count>`, seeds 0 to `count` - 1, or `--seed <seed>` alone;
/// `None` for neither.
fn seeds(text: &dyn Fn(&str) -> Option<String>) -> Result<Option<Seeds>, UsageError> {
    match (text("--seeds"), text("--seed")) {
        (Some(_), Some(_)) => Err(UsageError(
            "give `--seeds` or `--seed`, not both".to_owned(),
        )),
        (Some(count), None) => match count.trim().parse::<u64>() {
            Ok(count) if count > 0 => Ok(Some(Seeds { first: 0, count })),
            _ => Err(UsageError(format!(
                "`--seeds {count}`: expected a count above 0"
            ))),
        },
        (None, Some(seed)) => match seed.trim().parse::<u64>() {
            Ok(first) => Ok(Some(Seeds { first, count: 1 })),
            Err(_) => Err(UsageError(format!(
                "`--seed {seed}`: expected a whole number"
            ))),
        },
        (None, None) => Ok(None),
    }
}

/// `<a>x<b>` as two whole numbers.
fn pair(text: &str) -> Option<[u32; 2]> {
    let (a, b) = text.split_once('x')?;
    Some([a.trim().parse().ok()?, b.trim().parse().ok()?])
}

/// `<col>,<row>` as two whole numbers.
fn cell(text: &str) -> Result<[u32; 2], String> {
    text.split_once(',')
        .and_then(|(col, row)| Some([col.trim().parse().ok()?, row.trim().parse().ok()?]))
        .ok_or_else(|| format!("`--at {text}`: expected <col>,<row>, two whole numbers"))
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
        if !REPEATABLE.contains(&option) && options.iter().any(|(given, _)| *given == option) {
            return Err(UsageError(format!("`{option}` is given twice")));
        }
        let value = match takes {
            None => None,
            // A value may start with `-` (`--at -64,0`); one of the
            // command's own option names is never a value.
            Some(what) => Some(
                args.next()
                    .filter(|value| !own.iter().any(|(name, _)| value == *name))
                    .ok_or_else(|| UsageError(format!("`{option}` needs a {what}")))?,
            ),
        };
        options.push((option, value));
    }
    Ok((positional, options))
}

/// Whether `arg` is an option: it starts with `-` and is more than a bare
/// `-`, which names stdin where a file goes (`level apply <room> -`).
fn is_option(arg: &OsString) -> bool {
    let bytes = arg.as_encoded_bytes();
    bytes.starts_with(b"-") && bytes.len() > 1
}

/// The error for `option` given to `command`, or before any command: the
/// command or commands it belongs to, or that it is unknown.
fn misplaced(option: &OsString, command: Option<&str>) -> UsageError {
    let option = option.to_string_lossy();
    let all: Vec<&str> = OPTIONS
        .iter()
        .filter(|(_, options)| options.iter().any(|(name, _)| *name == option))
        .map(|(name, _)| *name)
        .collect();
    // A command's error names the commands it is likely meant for: a `level`
    // verb's every owner, another command's top-level owners, else all.
    let level = command.is_some_and(|command| command.starts_with("level"));
    let top: Vec<&str> = all
        .iter()
        .copied()
        .filter(|name| !name.starts_with("level"))
        .collect();
    let owners: Vec<String> = if level || top.is_empty() { &all } else { &top }
        .iter()
        .map(|name| format!("`{name}`"))
        .collect();
    match (owners.is_empty(), command) {
        (true, _) => UsageError(format!("unknown option `{option}`")),
        (false, Some(command)) => UsageError(format!(
            "`{option}` is an option of {}, not of `{command}`",
            listed(&owners, "and")
        )),
        (false, None) => UsageError(format!(
            "`{option}` goes after its command: {}",
            listed(&owners, "or")
        )),
    }
}

/// `a`, `a and b`, or `a, b and c`.
fn listed(items: &[String], last: &str) -> String {
    match items.split_last() {
        Some((only, [])) => only.clone(),
        Some((final_item, rest)) => format!("{} {last} {final_item}", rest.join(", ")),
        None => String::new(),
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
