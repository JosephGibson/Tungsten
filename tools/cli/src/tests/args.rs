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
    assert_eq!(parse_str(&["check"]), Ok(Command::Check));
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
            "`--release` is an option of `run`, not of `check`".to_owned()
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
    ] {
        assert!(USAGE.contains(word), "{word}");
    }
    assert!(!USAGE.ends_with('\n'));
    assert!(USAGE.lines().all(|line| line.len() <= 76));
}
