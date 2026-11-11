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
        assert_eq!(
            parse_str(args),
            Ok(Command::Help(HelpTopic::Top)),
            "{args:?}"
        );
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
            "unknown schema `prefab`: ui_screen, ui_theme, room, tileset, batch or script"
                .to_owned()
        ))
    );
    assert_eq!(
        parse_str(&["schema"]),
        Err(UsageError(
            "`schema` needs a format: ui_screen, ui_theme, room, tileset, batch or script"
                .to_owned()
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
            edit: ApplyEdit::Batch(PathBuf::from("batch.json")),
            inverse: Some(PathBuf::from("undo.json")),
            dry_run: false,
            check: false,
        }))
    );
    assert_eq!(
        level(&["apply", "cave", "--undo"]),
        Ok(Command::Level(LevelArgs::Apply {
            room: "cave".into(),
            edit: ApplyEdit::Undo,
            inverse: None,
            dry_run: false,
            check: false,
        }))
    );
    assert_eq!(
        level(&["apply", "cave", "--redo", "--dry-run"]),
        Ok(Command::Level(LevelArgs::Apply {
            room: "cave".into(),
            edit: ApplyEdit::Redo,
            inverse: None,
            dry_run: true,
            check: false,
        }))
    );
    assert_eq!(
        level(&["apply", "cave", "batch.json", "--check"]),
        Ok(Command::Level(LevelArgs::Apply {
            room: "cave".into(),
            edit: ApplyEdit::Batch(PathBuf::from("batch.json")),
            inverse: None,
            dry_run: false,
            check: true,
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
            "`level` needs a verb: new, apply, query, map, describe, check, generate or render"
                .to_owned()
        ))
    );
    assert_eq!(
        level(&["edit", "cave"]),
        Err(UsageError(
            "unknown level verb `edit`: new, apply, query, map, describe, check, generate or render"
                .to_owned()
        ))
    );
    assert_eq!(
        level(&["apply", "cave", "batch.json", "--undo"]),
        Err(UsageError("`--undo` takes no batch".to_owned()))
    );
    assert_eq!(
        level(&["apply", "cave"]),
        Err(UsageError(
            "`level apply` needs <room> <batch>, <room> --undo or <room> --redo".to_owned()
        ))
    );
    assert_eq!(
        level(&["apply", "cave", "batch.json", "--redo"]),
        Err(UsageError("`--redo` takes no batch".to_owned()))
    );
    assert_eq!(
        level(&["apply", "cave", "--undo", "--redo"]),
        Err(UsageError("give `--undo` or `--redo`, not both".to_owned()))
    );
    assert_eq!(
        level(&["apply", "cave", "--redo", "--inverse", "undo.json"]),
        Err(UsageError("`--redo` writes no --inverse".to_owned()))
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
            "`--seed` is an option of `check`, `level check` and `level generate`, not of `level describe`"
                .to_owned()
        ))
    );
}

#[test]
fn level_generate_takes_a_generator_and_its_options() {
    use crate::level::LevelArgs;
    let generate = |args: &[&str]| {
        let mut all = vec!["level", "generate"];
        all.extend_from_slice(args);
        parse_str(&all)
    };
    assert_eq!(
        generate(&["chain"]),
        Ok(Command::Level(LevelArgs::Generate {
            generator: "chain".into(),
            seed: 0,
            prefix: None,
            into: None,
            regenerate: false,
        }))
    );
    assert_eq!(
        generate(&[
            "chain",
            "--seed",
            "3",
            "--prefix",
            "run",
            "--into",
            "assets/levels",
            "--regenerate"
        ]),
        Ok(Command::Level(LevelArgs::Generate {
            generator: "chain".into(),
            seed: 3,
            prefix: Some("run".into()),
            into: Some("assets/levels".into()),
            regenerate: true,
        }))
    );
    assert_eq!(
        generate(&[]),
        Err(UsageError(
            "`level generate` needs a <generator>".to_owned()
        ))
    );
    assert_eq!(
        generate(&["chain", "--seed", "x"]),
        Err(UsageError("`--seed x`: expected a whole number".to_owned()))
    );
    assert_eq!(
        generate(&["chain", "--bogus"]),
        Err(UsageError("unknown option `--bogus`".to_owned()))
    );
    assert_eq!(
        generate(&["chain", "--undo"]),
        Err(UsageError(
            "`--undo` is an option of `level apply`, not of `level generate`".to_owned()
        ))
    );
    assert_eq!(
        generate(&["chain", "more"]),
        Err(UsageError(
            "`level generate` takes no further argument, given `more`".to_owned()
        ))
    );
}

#[test]
fn a_bare_dash_is_the_batch_on_stdin() {
    let level = |args: &[&str]| {
        let mut all = vec!["level"];
        all.extend_from_slice(args);
        parse_str(&all)
    };
    assert_eq!(
        level(&["apply", "cave", "-"]),
        Ok(Command::Level(LevelArgs::Apply {
            room: "cave".into(),
            edit: ApplyEdit::Batch(PathBuf::from("-")),
            inverse: None,
            dry_run: false,
            check: false,
        }))
    );
    // Its place among the options does not matter.
    assert_eq!(
        level(&["apply", "cave", "--dry-run", "-"]),
        Ok(Command::Level(LevelArgs::Apply {
            room: "cave".into(),
            edit: ApplyEdit::Batch(PathBuf::from("-")),
            inverse: None,
            dry_run: true,
            check: false,
        }))
    );
    // Any other argument starting with `-` is still an option.
    assert_eq!(
        level(&["apply", "cave", "-x"]),
        Err(UsageError("unknown option `-x`".to_owned()))
    );
    for word in ["--redo", "--dry-run", "--check", "- for stdin"] {
        assert!(USAGE.contains(word), "{word}");
    }
}

#[test]
fn level_new_and_map_read_their_options() {
    let level = |args: &[&str]| {
        let mut all = vec!["level"];
        all.extend_from_slice(args);
        parse_str(&all)
    };
    assert_eq!(
        level(&["new", "yard"]),
        Ok(Command::Level(LevelArgs::New(NewArgs {
            room: "yard".into(),
            size: [40, 24],
            cell: None,
            tileset: None,
            tiles: Vec::new(),
            dry_run: false,
        })))
    );
    assert_eq!(
        level(&[
            "new",
            "yard",
            "--size",
            "24x14",
            "--cell",
            "32",
            "--tileset",
            "tiles",
            "--tile",
            "rock",
            "--tile",
            "moss",
            "--dry-run",
        ]),
        Ok(Command::Level(LevelArgs::New(NewArgs {
            room: "yard".into(),
            size: [24, 14],
            cell: Some([32, 32]),
            tileset: Some("tiles".into()),
            tiles: vec!["rock".into(), "moss".into()],
            dry_run: true,
        })))
    );
    assert_eq!(
        level(&["new", "yard", "--cell", "16x8"]).map(|command| match command {
            Command::Level(LevelArgs::New(args)) => args.cell,
            _ => None,
        }),
        Ok(Some([16, 8]))
    );
    assert_eq!(
        level(&["new"]),
        Err(UsageError(
            "`level new` needs the new <room>'s ID".to_owned()
        ))
    );
    for size in ["24", "0x4", "24x5000", "axb"] {
        assert_eq!(
            level(&["new", "yard", "--size", size]),
            Err(UsageError(format!(
                "`--size {size}`: expected <cols>x<rows>, each 1 to 4096, as 40x24"
            ))),
            "{size}"
        );
    }
    assert_eq!(
        level(&["new", "yard", "--cell", "0"]),
        Err(UsageError(
            "`--cell 0`: expected <px> or <w>x<h> in pixels, as 16 or 16x8".to_owned()
        ))
    );
    assert_eq!(
        level(&["map", "cave", "--rect", "8,16,12,4"]),
        Ok(Command::Level(LevelArgs::Map {
            room: "cave".into(),
            rect: Some([8, 16, 12, 4]),
        }))
    );
    assert_eq!(
        level(&["map", "cave"]),
        Ok(Command::Level(LevelArgs::Map {
            room: "cave".into(),
            rect: None,
        }))
    );
    for rect in ["8,16,12", "8,16,12,x", "1,2,3,4,5"] {
        assert_eq!(
            level(&["map", "cave", "--rect", rect]),
            Err(UsageError(format!(
                "`--rect {rect}`: expected <col>,<row>,<width>,<height>, four whole numbers"
            ))),
            "{rect}"
        );
    }
    assert_eq!(
        level(&["map"]),
        Err(UsageError("`level map` needs a <room>".to_owned()))
    );
    for word in [
        "new <room>",
        "map <room>",
        "--tile <sprite>",
        "--rect <col>",
    ] {
        assert!(USAGE.contains(word), "{word}");
    }
}

#[test]
fn level_help_lists_every_verb_with_an_example() {
    assert_eq!(
        parse_str(&["level", "--help"]),
        Ok(Command::Help(HelpTopic::Level))
    );
    assert_eq!(
        parse_str(&["level", "-h", "apply"]),
        Ok(Command::Help(HelpTopic::Level)),
        "the verb comes right after `level`"
    );
    assert_eq!(
        parse_str(&["level", "apply", "cave", "--help"]),
        Ok(Command::Help(HelpTopic::LevelVerb("apply".to_owned())))
    );
    assert_eq!(
        parse_str(&["level", "bogus", "--help"]),
        Ok(Command::Help(HelpTopic::Level))
    );
    let level = help(&HelpTopic::Level);
    assert!(level.starts_with("Usage: tungsten level <verb> [arguments]\n"));
    assert!(level.lines().all(|line| line.len() <= 76), "76 columns");
    assert!(!level.ends_with('\n'));
    // A worked example first: each command it runs is a verb the help
    // lists below it.
    let example: Vec<&str> = level
        .lines()
        .filter_map(|line| line.strip_prefix("  tungsten level "))
        .map(|rest| rest.split(' ').next().unwrap())
        .collect();
    assert_eq!(example, ["new", "apply", "map", "check", "render"]);
    for verb in LEVEL_VERBS {
        assert!(
            level.contains(&format!("\n  {verb} ")),
            "`{verb}` has its entry"
        );
        // Each verb's own help is its entry, and no other verb's.
        let own = help(&HelpTopic::LevelVerb(verb.to_owned()));
        assert!(
            own.starts_with(&format!("tungsten level {verb}:\n  {verb}")),
            "{own}"
        );
        let others = LEVEL_VERBS
            .iter()
            .filter(|other| **other != verb)
            .filter(|other| own.contains(&format!("\n  {other} ")))
            .count();
        assert_eq!(others, 0, "{own}");
        assert!(
            own.lines().count() < 40,
            "{verb}: {} lines",
            own.lines().count()
        );
    }
    // The batch commands sit in level's help and in apply's.
    let apply = help(&HelpTopic::LevelVerb("apply".to_owned()));
    for text in [&level, &apply] {
        for verb in [
            "place",
            "move",
            "set",
            "delete",
            "duplicate",
            "paint",
            "erase",
            "link",
            "rename",
        ] {
            assert!(text.contains(&format!("\n  {verb} ")), "`{verb}`");
        }
        assert!(text.contains("tungsten schema batch"));
    }
    assert!(apply.contains("--redo") && apply.contains("--dry-run"));
    // The whole usage still lists every verb and points at level's help.
    assert!(USAGE.contains("tungsten level --help"));
    assert_eq!(help(&HelpTopic::Top), USAGE);
}

#[test]
fn a_usage_error_prints_its_verbs_usage_alone() {
    let usage = |args: &[&str]| {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        usage_for(&args)
    };
    // A level verb's mistake: that verb's entry with its options.
    let apply = usage(&["level", "apply", "cave"]);
    assert!(apply.starts_with("  apply <room> <batch>\n"), "{apply}");
    assert!(
        apply.contains("--undo") && apply.contains("--redo"),
        "{apply}"
    );
    assert!(!apply.contains("generate <generator>"), "{apply}");
    assert!(apply.ends_with("`tungsten level --help` prints every verb with a worked example."));
    assert!(apply.lines().count() < 15, "{apply}");
    // No verb, or an unknown one: the verbs without their options.
    for args in [&["level"][..], &["level", "bogus"], &["level", "--undo"]] {
        let verbs = usage(args);
        assert!(
            verbs.starts_with("Verbs of level:\n  new <room>"),
            "{verbs}"
        );
        assert!(
            verbs.contains("--save-script and"),
            "render's whole description"
        );
        assert!(!verbs.contains("    --size"), "no option: {verbs}");
        assert!(verbs.lines().count() < 30, "{}", verbs.lines().count());
    }
    // A command's mistake: its entry and its options.
    let run = usage(&["run"]);
    assert!(run.starts_with("  run         Run the game"), "{run}");
    assert!(run.contains("Options of run:\n  --script <file>"), "{run}");
    assert!(
        !run.contains("Verbs of level") && !run.contains("package"),
        "{run}"
    );
    assert!(run.ends_with("`tungsten --help` prints every command."));
    let check = usage(&["check", "--json"]);
    assert!(
        check.contains("Options of check and level check, one of:\n  --seeds"),
        "{check}"
    );
    let render = usage(&["render"]);
    assert!(render.contains("Options of render,") && render.contains("--sheet <items>"));
    assert!(!render.contains("Options of run:"), "{render}");
    // A command with no options: its entry alone.
    let package = usage(&["package", "now"]);
    assert!(
        package.starts_with("  package     Build the game for release"),
        "{package}"
    );
    assert!(!package.contains("Options of"), "{package}");
    // No command, an unknown one or a leading option: the commands.
    for args in [&[][..], &["frobnicate"], &["--json"]] {
        let commands = usage(args);
        assert!(
            commands.starts_with("Commands:\n  new <path>"),
            "{commands}"
        );
        assert!(commands.contains("\n  level <verb>\n"), "{commands}");
        assert!(!commands.contains("Options of"), "{commands}");
    }
}

#[test]
fn level_render_keeps_its_script() {
    let command = parse_str(&[
        "level",
        "render",
        "cave",
        "-o",
        "cave.png",
        "--save-script",
        "cave-script.json",
    ]);
    let Ok(Command::Level(LevelArgs::Render { args })) = command else {
        panic!("{command:?}");
    };
    assert_eq!(args.save_script, Some(PathBuf::from("cave-script.json")));
    assert!(USAGE.contains("--save-script and"));
}
