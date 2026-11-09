use super::*;

fn parse_str(args: &[&str]) -> Result<Command, UsageError> {
    parse(args.iter().map(OsString::from))
}

#[test]
fn help_anywhere_wins() {
    for args in [
        &["-h"][..],
        &["--help"],
        &["check", "--help"],
        &["new", "game", "-h"],
        &["frobnicate", "--help"],
        &["-V", "-h"],
        &["--bogus", "--help"],
    ] {
        assert_eq!(parse_str(args), Ok(Command::Help), "{args:?}");
    }
}

#[test]
fn version_anywhere_after_help() {
    for args in [
        &["-V"][..],
        &["--version"],
        &["package", "-V"],
        &["x", "y", "--version"],
    ] {
        assert_eq!(parse_str(args), Ok(Command::Version), "{args:?}");
    }
}

#[test]
fn the_three_commands_parse() {
    assert_eq!(
        parse_str(&["new", "games/my-game"]),
        Ok(Command::New(PathBuf::from("games/my-game")))
    );
    assert_eq!(parse_str(&["check"]), Ok(Command::Check(None)));
    assert_eq!(parse_str(&["package"]), Ok(Command::Package));
}

#[test]
fn no_command_is_a_usage_error() {
    assert_eq!(
        parse_str(&[]),
        Err(UsageError("no command given".to_owned()))
    );
}

#[test]
fn an_unknown_command_is_a_usage_error() {
    assert_eq!(
        parse_str(&["frobnicate"]),
        Err(UsageError("unknown command `frobnicate`".to_owned()))
    );
    // Commands are matched exactly.
    assert!(parse_str(&["Check"]).is_err());
}

#[test]
fn new_needs_exactly_one_path() {
    assert_eq!(
        parse_str(&["new"]),
        Err(UsageError("`new` needs the new game's <path>".to_owned()))
    );
    assert_eq!(
        parse_str(&["new", "a", "b"]),
        Err(UsageError(
            "`new` takes no further argument, given `b`".to_owned()
        ))
    );
}

#[test]
fn check_and_package_take_no_argument() {
    assert_eq!(
        parse_str(&["check", "here"]),
        Err(UsageError(
            "`check` takes no further argument, given `here`".to_owned()
        ))
    );
    assert_eq!(
        parse_str(&["package", "a", "b"]),
        Err(UsageError(
            "`package` takes no further argument, given `a` `b`".to_owned()
        ))
    );
}

#[test]
fn an_unknown_option_is_a_usage_error() {
    assert_eq!(
        parse_str(&["check", "--bogus"]),
        Err(UsageError("unknown option `--bogus`".to_owned()))
    );
    assert_eq!(
        parse_str(&["new", "-x", "game"]),
        Err(UsageError("unknown option `-x`".to_owned()))
    );
    assert_eq!(
        parse_str(&["run", "--script", "a.json", "--frames", "9"]),
        Err(UsageError("unknown option `--frames`".to_owned()))
    );
}

#[test]
fn run_takes_its_options_in_any_order() {
    assert_eq!(
        parse_str(&["run", "--script", "s.json"]),
        Ok(Command::Run(RunArgs {
            script: PathBuf::from("s.json"),
            report: None,
            user_dir: None,
            release: false,
        }))
    );
    assert_eq!(
        parse_str(&[
            "run",
            "--release",
            "--user-dir",
            "u",
            "--script",
            "s.json",
            "--report",
            "r.json"
        ]),
        Ok(Command::Run(RunArgs {
            script: PathBuf::from("s.json"),
            report: Some(PathBuf::from("r.json")),
            user_dir: Some(PathBuf::from("u")),
            release: true,
        }))
    );
}

#[test]
fn run_needs_its_script_and_each_value() {
    let error = |args: &[&str]| match parse_str(args) {
        Err(UsageError(reason)) => reason,
        other => panic!("{args:?}: {other:?}"),
    };
    assert_eq!(error(&["run"]), "`run` needs --script <file>");
    assert_eq!(error(&["run", "--script"]), "`--script` needs a <file>");
    assert_eq!(
        error(&["run", "--script", "--release"]),
        "`--script` needs a <file>"
    );
    assert_eq!(
        error(&[
            "run", "--report", "r.json", "--script", "a", "--report", "b"
        ]),
        "`--report` is given twice"
    );
    assert_eq!(
        error(&["run", "--script", "a", "extra"]),
        "`run` takes no further argument, given `extra`"
    );
}

#[test]
fn describe_takes_a_name_and_json() {
    assert_eq!(
        parse_str(&["describe"]),
        Ok(Command::Describe(DescribeArgs {
            name: None,
            json: false
        }))
    );
    assert_eq!(
        parse_str(&["describe", "--json", "start_run"]),
        Ok(Command::Describe(DescribeArgs {
            name: Some("start_run".to_owned()),
            json: true
        }))
    );
    assert_eq!(
        parse_str(&["describe", "a", "b"]),
        Err(UsageError(
            "`describe` takes no further argument, given `b`".to_owned()
        ))
    );
}

#[test]
fn a_misplaced_option_names_its_command() {
    assert_eq!(
        parse_str(&["check", "--release"]),
        Err(UsageError(
            "`--release` is an option of `run` and `render`, not of `check`".to_owned()
        ))
    );
    assert_eq!(
        parse_str(&["run", "--script", "a", "--json"]),
        Err(UsageError(
            "`--json` is an option of `describe`, not of `run`".to_owned()
        ))
    );
    assert_eq!(
        parse_str(&["--script", "a.json", "run"]),
        Err(UsageError(
            "`--script` goes after its command: `run`".to_owned()
        ))
    );
    assert_eq!(
        parse_str(&["--bogus", "run"]),
        Err(UsageError("unknown option `--bogus`".to_owned()))
    );
}

#[test]
fn the_usage_names_every_command_and_option() {
    for word in [
        "new <path>",
        "check",
        "package",
        "run",
        "describe [<name>]",
        "-h, --help",
        "-V, --version",
        "--script <file>",
        "--report <file>",
        "--user-dir <dir>",
        "--release",
        "--json",
        "render",
        "--sheet <items>",
        "--scene <file>",
        "--at <x>,<y>",
        "--after <action>[=<json>]",
        "--zoom <z>",
        "--view <height>",
        "--size <w>x<h>",
        "--overlay <names>",
        "--no-ui",
        "--frame <n>",
        "--loop <from>..<to>/<every>",
        "--save-script <file>",
        "-o <path>",
    ] {
        assert!(USAGE.contains(word), "{word}");
    }
    assert!(!USAGE.ends_with('\n'));
    assert!(USAGE.lines().all(|line| line.len() <= 76));
}

#[test]
fn schema_takes_a_format_and_an_output() {
    assert_eq!(
        parse_str(&["schema", "ui_screen"]),
        Ok(Command::Schema(SchemaArgs {
            name: "ui_screen".to_owned(),
            out: None
        }))
    );
    assert_eq!(
        parse_str(&["schema", "ui_theme", "-o", "theme.schema.json"]),
        Ok(Command::Schema(SchemaArgs {
            name: "ui_theme".to_owned(),
            out: Some(PathBuf::from("theme.schema.json"))
        }))
    );
    assert_eq!(
        parse_str(&["schema", "prefab"]),
        Err(UsageError(
            "unknown schema `prefab`: ui_screen, ui_theme, room or tileset".to_owned()
        ))
    );
    assert_eq!(
        parse_str(&["schema"]),
        Err(UsageError(
            "`schema` needs a format: ui_screen, ui_theme, room or tileset".to_owned()
        ))
    );
    assert!(parse_str(&["schema", "ui_screen", "ui_theme"]).is_err());
}

#[test]
fn level_verbs_parse_and_a_misplaced_option_fails() {
    use crate::level::LevelArgs;
    let level = |args: &[&str]| {
        let mut all = vec!["level"];
        all.extend_from_slice(args);
        parse_str(&all)
    };
    assert_eq!(
        level(&["apply", "cave", "batch.json", "--inverse", "undo.json"]),
        Ok(Command::Level(LevelArgs::Apply {
            room: "cave".into(),
            batch: Some(PathBuf::from("batch.json")),
            inverse: Some(PathBuf::from("undo.json")),
            undo: false,
        }))
    );
    assert_eq!(
        level(&["apply", "cave", "--undo"]),
        Ok(Command::Level(LevelArgs::Apply {
            room: "cave".into(),
            batch: None,
            inverse: None,
            undo: true,
        }))
    );
    assert_eq!(
        level(&["query", "assets/rooms/cave.json", "--at", "7,19", "--json"]),
        Ok(Command::Level(LevelArgs::Query {
            room: "assets/rooms/cave.json".into(),
            id: None,
            at: Some([7, 19]),
            json: true,
        }))
    );
    assert_eq!(
        level(&["query", "cave", "crate_1"]),
        Ok(Command::Level(LevelArgs::Query {
            room: "cave".into(),
            id: Some("crate_1".into()),
            at: None,
            json: false,
        }))
    );
    assert_eq!(
        level(&["describe", "--json"]),
        Ok(Command::Level(LevelArgs::Describe {
            room: None,
            json: true,
        }))
    );
    assert_eq!(
        level(&["check", "cave", "hall"]),
        Ok(Command::Level(LevelArgs::Check {
            rooms: vec!["cave".into(), "hall".into()],
            seeds: None,
        }))
    );
    assert!(matches!(
        level(&["render", "cave", "-o", "cave.png", "--overlay", "physics"]),
        Ok(Command::Level(LevelArgs::Render { .. }))
    ));
    // A misplaced option names the verb it belongs to.
    assert_eq!(
        level(&["apply", "cave", "batch.json", "--at", "1,1"]),
        Err(UsageError(
            "`--at` is an option of `render` and `level query`, not of `level apply`".to_owned()
        ))
    );
    assert_eq!(
        level(&["check", "--json"]),
        Err(UsageError(
            "`--json` is an option of `describe`, `level query` and `level describe`, not of `level check`"
                .to_owned()
        ))
    );
    assert_eq!(
        level(&["render", "cave", "-o", "cave.png", "--loop", "1..5/1"]),
        Err(UsageError(
            "`--loop` is an option of `render`, not of `level render`".to_owned()
        ))
    );
    assert_eq!(
        level(&[]),
        Err(UsageError(
            "`level` needs a verb: apply, query, describe, check or render".to_owned()
        ))
    );
    assert_eq!(
        level(&["edit", "cave"]),
        Err(UsageError(
            "unknown level verb `edit`: apply, query, describe, check or render".to_owned()
        ))
    );
    assert_eq!(
        level(&["apply", "cave", "batch.json", "--undo"]),
        Err(UsageError("`--undo` takes no batch".to_owned()))
    );
    assert_eq!(
        level(&["apply", "cave"]),
        Err(UsageError(
            "`level apply` needs <room> <batch>, or <room> --undo".to_owned()
        ))
    );
    assert_eq!(
        level(&["query", "cave", "--at", "7"]),
        Err(UsageError(
            "`--at 7`: expected <col>,<row>, two whole numbers".to_owned()
        ))
    );
}

#[test]
fn check_takes_seeds_or_one_seed() {
    let seeds = |first, count| Some(Seeds { first, count });
    assert_eq!(
        parse_str(&["check", "--seeds", "500"]),
        Ok(Command::Check(seeds(0, 500)))
    );
    assert_eq!(
        parse_str(&["check", "--seed", "7"]),
        Ok(Command::Check(seeds(7, 1)))
    );
    assert_eq!(
        parse_str(&["check", "--seeds", "3", "--seed", "7"]),
        Err(UsageError(
            "give `--seeds` or `--seed`, not both".to_owned()
        ))
    );
    for bad in ["0", "-2", "many"] {
        assert_eq!(
            parse_str(&["check", "--seeds", bad]),
            Err(UsageError(format!(
                "`--seeds {bad}`: expected a count above 0"
            ))),
            "{bad}"
        );
    }
    assert_eq!(
        parse_str(&["check", "--seed", "-1"]),
        Err(UsageError(
            "`--seed -1`: expected a whole number".to_owned()
        ))
    );
    assert_eq!(
        parse_str(&["check", "--seeds"]),
        Err(UsageError("`--seeds` needs a <count>".to_owned()))
    );
    assert_eq!(
        parse_str(&["run", "--seeds", "5"]),
        Err(UsageError(
            "`--seeds` is an option of `check`, not of `run`".to_owned()
        ))
    );
    for word in ["--seeds <count>", "--seed <seed>"] {
        assert!(USAGE.contains(word), "{word}");
    }
}

#[test]
fn level_check_takes_seeds_or_one_seed() {
    use crate::level::LevelArgs;
    let check = |args: &[&str]| {
        let mut all = vec!["level", "check"];
        all.extend_from_slice(args);
        parse_str(&all)
    };
    assert_eq!(
        check(&["cave", "--seeds", "500"]),
        Ok(Command::Level(LevelArgs::Check {
            rooms: vec!["cave".into()],
            seeds: Some(Seeds {
                first: 0,
                count: 500
            }),
        }))
    );
    assert_eq!(
        check(&["--seed", "9"]),
        Ok(Command::Level(LevelArgs::Check {
            rooms: Vec::new(),
            seeds: Some(Seeds { first: 9, count: 1 }),
        }))
    );
    assert_eq!(
        check(&["--seed", "1", "--seeds", "2"]),
        Err(UsageError(
            "give `--seeds` or `--seed`, not both".to_owned()
        ))
    );
    assert_eq!(
        parse_str(&["level", "describe", "--seed", "1"]),
        Err(UsageError(
            "`--seed` is an option of `check` and `level check`, not of `level describe`"
                .to_owned()
        ))
    );
}
