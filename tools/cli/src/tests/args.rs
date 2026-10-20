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
        parse_str(&["check", "--release"]),
        Err(UsageError("unknown option `--release`".to_owned()))
    );
    assert_eq!(
        parse_str(&["new", "-x", "game"]),
        Err(UsageError("unknown option `-x`".to_owned()))
    );
}

#[test]
fn the_usage_names_every_command_and_option() {
    for word in [
        "new <path>",
        "check",
        "package",
        "-h, --help",
        "-V, --version",
    ] {
        assert!(USAGE.contains(word), "{word}");
    }
    assert!(!USAGE.ends_with('\n'));
    assert!(USAGE.lines().all(|line| line.len() <= 76));
}
