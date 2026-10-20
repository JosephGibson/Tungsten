//! Ported from `scripts/test-release.py`'s notice cases, with the engine's
//! crates and a game's own clarifications added.

use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;

use super::*;

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tungsten_notices_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// The game's `licenses/clarifications.json` with `entries`.
    fn clarify(&self, entries: &serde_json::Value) -> PathBuf {
        self.write(
            "licenses/clarifications.json",
            &json!({ "crates": entries }).to_string(),
        );
        self.0.join("licenses")
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn text(license: &str, who: &str) -> String {
    match license {
        "MIT" => format!(
            "Copyright (c) {who}\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software. The above copyright notice and this permission notice shall be included in all\ncopies or substantial portions of the Software.\n"
        ),
        "Apache-2.0" => format!(
            "Apache License\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n2. Grant of Copyright License. ({who})\n"
        ),
        "Zlib" => format!(
            "Copyright {who}\nPermission is granted to anyone to use this software for any purpose.\n2. Altered source versions must be plainly marked as such.\n"
        ),
        "Unicode-3.0" => format!(
            "UNICODE LICENSE V3\nPermission is hereby granted, free of charge, to any person obtaining a\ncopy of data files ({who})\n"
        ),
        "GPL-2.0-only" => format!("GNU GENERAL PUBLIC LICENSE, Version 2 ({who})\n"),
        other => panic!("{other}"),
    }
}

fn files(pairs: &[(&str, String)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(name, text)| ((*name).to_owned(), text.clone()))
        .collect()
}

fn record(name: &str, license: &str, files: BTreeMap<String, String>) -> CrateRecord {
    CrateRecord {
        name: name.to_owned(),
        version: "1.0.0".to_owned(),
        license: Some(license.to_owned()),
        repository: Some(format!("https://example.com/{name}")),
        files,
        fonts: Vec::new(),
    }
}

fn input(crates: Vec<CrateRecord>, game_licenses: Option<PathBuf>) -> NoticeInput {
    NoticeInput {
        game: "my-game".to_owned(),
        game_version: "0.3.1".to_owned(),
        engine_version: "0.90.0".to_owned(),
        target: "x86_64-unknown-linux-gnu".to_owned(),
        rustc: "rustc 1.0.0 (fake)".to_owned(),
        engine_crates: Vec::new(),
        crates,
        game_licenses,
    }
}

fn strings(alternatives: &[&[&str]]) -> Vec<Vec<String>> {
    alternatives
        .iter()
        .map(|a| a.iter().map(|s| (*s).to_owned()).collect())
        .collect()
}

#[test]
fn license_expressions() {
    for (expression, expected) in [
        ("MIT", strings(&[&["MIT"]])),
        ("MIT OR Apache-2.0", strings(&[&["MIT"], &["Apache-2.0"]])),
        ("MIT/Apache-2.0", strings(&[&["MIT"], &["Apache-2.0"]])),
        ("Unlicense / MIT", strings(&[&["Unlicense"], &["MIT"]])),
        (
            "MIT OR Apache-2.0 OR Zlib",
            strings(&[&["MIT"], &["Apache-2.0"], &["Zlib"]]),
        ),
        (
            "(MIT OR Apache-2.0) AND Unicode-3.0",
            strings(&[&["MIT", "Unicode-3.0"], &["Apache-2.0", "Unicode-3.0"]]),
        ),
        ("Apache-2.0 AND MIT", strings(&[&["Apache-2.0", "MIT"]])),
        (
            "Apache-2.0 OR GPL-2.0-only",
            strings(&[&["Apache-2.0"], &["GPL-2.0-only"]]),
        ),
        (
            "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
            strings(&[
                &["Apache-2.0 WITH LLVM-exception"],
                &["Apache-2.0"],
                &["MIT"],
            ]),
        ),
        ("MPL-2.0", strings(&[&["MPL-2.0"]])),
    ] {
        assert_eq!(alternatives(expression), Ok(expected), "{expression}");
    }
    for bad in [
        "",
        "MIT OR",
        "(MIT OR Apache-2.0",
        "MIT)",
        "AND MIT",
        "MIT WITH",
        "MIT Apache-2.0",
    ] {
        let err = alternatives(bad).unwrap_err();
        assert!(err.contains("malformed"), "{bad}: {err}");
    }
}

#[test]
fn the_preferred_alternative_with_a_text_is_used() {
    let mut all = files(&[
        ("LICENSE-APACHE", text("Apache-2.0", "a")),
        ("LICENSE-MIT", text("MIT", "a")),
        ("LICENSE-ZLIB", text("Zlib", "a")),
    ]);
    assert_eq!(
        choose("Zlib OR Apache-2.0 OR MIT", &all),
        Ok(Some((
            vec!["MIT".to_owned()],
            vec![("MIT".to_owned(), "LICENSE-MIT".to_owned())]
        )))
    );
    all.remove("LICENSE-MIT");
    assert_eq!(
        choose("Zlib OR Apache-2.0 OR MIT", &all),
        Ok(Some((
            vec!["Apache-2.0".to_owned()],
            vec![("Apache-2.0".to_owned(), "LICENSE-APACHE".to_owned())]
        )))
    );
}

#[test]
fn each_license_of_an_and_needs_a_text_and_the_most_specific_file_serves() {
    let mut both = files(&[
        ("LICENSE", text("MIT", "a") + &text("Apache-2.0", "a")),
        ("LICENSE-MIT", text("MIT", "a")),
        ("LICENSE-UNICODE", text("Unicode-3.0", "a")),
    ]);
    assert_eq!(
        choose("(MIT OR Apache-2.0) AND Unicode-3.0", &both),
        Ok(Some((
            vec!["MIT".to_owned(), "Unicode-3.0".to_owned()],
            vec![
                ("MIT".to_owned(), "LICENSE-MIT".to_owned()),
                ("Unicode-3.0".to_owned(), "LICENSE-UNICODE".to_owned())
            ]
        )))
    );
    both.remove("LICENSE-UNICODE");
    assert_eq!(
        choose("(MIT OR Apache-2.0) AND Unicode-3.0", &both),
        Ok(None)
    );
}

#[test]
fn a_pointer_is_no_text_and_a_license_outside_the_preference_is_never_used() {
    let pointer = files(&[("LICENSE", "Licensed under the MIT license.".to_owned())]);
    assert_eq!(choose("MIT", &pointer), Ok(None));
    let gpl = files(&[("COPYING", text("GPL-2.0-only", "a"))]);
    assert_eq!(choose("GPL-2.0-only", &gpl), Ok(None));
}

#[test]
fn notices_leave_out_other_alternatives_and_keep_notice_files() {
    let apache = text("Apache-2.0", "the authors");
    let notices = third_party_notices(&input(
        vec![
            record(
                "lib-a",
                "MIT OR Apache-2.0",
                files(&[
                    ("LICENSE-APACHE", apache.clone()),
                    ("NOTICE.md", "Lib A's notice.\n".to_owned()),
                ]),
            ),
            record("lib-b", "Apache-2.0", files(&[("LICENSE", apache.clone())])),
            record(
                "self_cell",
                "Apache-2.0 OR GPL-2.0-only",
                files(&[
                    ("LICENSE-APACHE", apache.clone()),
                    ("LICENSE-GPLv2", text("GPL-2.0-only", "x")),
                ]),
            ),
        ],
        None,
    ))
    .unwrap();
    assert!(!notices.contains("GNU GENERAL PUBLIC LICENSE"));
    assert!(notices.contains(
        "self_cell 1.0.0\n    License: Apache-2.0 OR GPL-2.0-only\n    Used under: Apache-2.0: [1]\n"
    ));
    assert!(notices.contains(
        "lib-a 1.0.0\n    License: MIT OR Apache-2.0\n    Used under: Apache-2.0: [1]\n    Notices: [2]\n"
    ));
    // A text that several crates share appears once, under all of them.
    assert_eq!(notices.matches("TERMS AND CONDITIONS FOR USE").count(), 1);
    assert!(
        notices.contains("[1] Apache-2.0\nUsed by lib-a 1.0.0, lib-b 1.0.0, self_cell 1.0.0\n")
    );
    assert!(notices.contains(&format!(
        "[2] NOTICE\nUsed by lib-a 1.0.0\n{}\nLib A's notice.\n",
        "-".repeat(78)
    )));
    assert!(
        notices.starts_with("Third-party notices: my-game 0.3.1 for x86_64-unknown-linux-gnu\n")
    );
    assert!(
        notices
            .replace('\n', " ")
            .contains("rustc 1.0.0 (fake); its notices are in THIRD-PARTY-NOTICES-rust-std.html")
    );
}

#[test]
fn the_engines_crates_are_listed_under_its_license_and_its_works_lead() {
    let mut notice_input = input(
        vec![record(
            "anyhow",
            "MIT OR Apache-2.0",
            files(&[("LICENSE-MIT", text("MIT", "anyhow"))]),
        )],
        None,
    );
    notice_input.engine_crates = vec!["tungsten".to_owned(), "tungsten-core".to_owned()];
    let notices = third_party_notices(&notice_input).unwrap();
    assert!(notices.contains(
        "tungsten 0.90.0\n    License: MIT\n    Used under: MIT: [1]\n    Source: https://github.com/JosephGibson/Tungsten/tree/v0.90.0/crates/tungsten\n"
    ));
    assert!(notices.contains("\ntungsten-core 0.90.0\n"));
    assert!(notices.contains("Used by tungsten 0.90.0, tungsten-core 0.90.0\n"));
    let license = read_any(embedded::file("LICENSE").unwrap());
    assert!(notices.contains(license.lines().next().unwrap()));
    let (title, _) = EMBEDDED_WORKS[0];
    assert!(notices.contains(&format!(
        "Works compiled into the engine\n{}\n\n{title}\n",
        "=".repeat(78)
    )));
    assert!(notices.find(title).unwrap() < notices.find("anyhow 1.0.0").unwrap());
    assert!(notices.contains("JetBrains Mono"));
    assert!(
        notices.contains("the 3 crates the binary links")
            || notices
                .replace('\n', " ")
                .contains("the 3 crates the binary links")
    );
}

#[test]
fn the_engines_clarification_supplies_an_embedded_text() {
    let mut dasp = record("dasp_sample", "MIT OR Apache-2.0", BTreeMap::new());
    dasp.version = "0.11.0".to_owned();
    let notices = third_party_notices(&input(vec![dasp.clone()], None)).unwrap();
    assert!(notices.contains("dasp_sample 0.11.0\n    License: MIT OR Apache-2.0\n    Used under: MIT: [1] (from https://github.com/RustAudio/dasp/"));
    dasp.version = "0.12.0".to_owned();
    let err = third_party_notices(&input(vec![dasp], None)).unwrap_err();
    assert!(
        err.contains("dasp_sample 0.12.0: licenses/clarifications.json checked it at 0.11.0"),
        "{err}"
    );
}

#[test]
fn a_games_clarification_supplies_texts_for_its_version_only() {
    let game = Folder::new();
    game.write("licenses/bare/LICENSE-MIT", &text("MIT", "bare upstream"));
    let licenses = game.clarify(&json!([{
        "name": "bare", "version": "0.11.0",
        "texts": [{ "license": "MIT", "file": "bare/LICENSE-MIT", "source": "https://example.com/bare/v0.11.0/LICENSE-MIT" }]
    }]));
    let mut bare = record("bare", "MIT OR Apache-2.0", BTreeMap::new());
    bare.version = "0.11.0".to_owned();
    let notices = third_party_notices(&input(vec![bare.clone()], Some(licenses.clone()))).unwrap();
    assert!(notices.contains(
        "    Used under: MIT: [1] (from https://example.com/bare/v0.11.0/LICENSE-MIT)\n"
    ));
    assert!(notices.contains("Copyright (c) bare upstream"));
    let mut newer = bare.clone();
    newer.version = "0.12.0".to_owned();
    let err = third_party_notices(&input(vec![newer], Some(licenses.clone()))).unwrap_err();
    assert!(
        err.contains("bare 0.12.0: licenses/clarifications.json checked it at 0.11.0"),
        "{err}"
    );
    game.clarify(&json!([{
        "name": "bare", "version": "0.11.0",
        "texts": [{ "license": "Zlib", "file": "bare/LICENSE-MIT", "source": "https://example.com" }]
    }]));
    let err = third_party_notices(&input(vec![bare.clone()], Some(licenses.clone()))).unwrap_err();
    assert!(
        err.contains("names Zlib, not an alternative of \"MIT OR Apache-2.0\""),
        "{err}"
    );
    game.clarify(&json!([{
        "name": "bare", "version": "0.11.0",
        "texts": [{ "license": "MIT", "file": "../outside", "source": "https://example.com" }]
    }]));
    let err = third_party_notices(&input(vec![bare], Some(licenses))).unwrap_err();
    assert!(err.contains("which its licenses/ lacks"), "{err}");
}

#[test]
fn every_crate_without_a_text_is_named_in_one_error() {
    let mut bare = record("bare", "MIT OR Apache-2.0", BTreeMap::new());
    bare.version = "0.11.0".to_owned();
    let other = record(
        "other",
        "MIT",
        files(&[("LICENSE", "See the MIT license.".to_owned())]),
    );
    let err = third_party_notices(&input(vec![bare, other], None)).unwrap_err();
    assert!(
        err.contains("2 crate(s)")
            && err.contains(
                "bare 0.11.0 (MIT OR Apache-2.0; files: none); other 1.0.0 (MIT; files: LICENSE)"
            ),
        "{err}"
    );
}

#[test]
fn a_bundled_work_is_written_under_its_crate() {
    let game = Folder::new();
    game.write(
        "licenses/decorations/COPYING",
        "Copyright (c) the font's authors\nSIL OPEN FONT LICENSE Version 1.1\n",
    );
    let licenses = game.clarify(&json!([{
        "name": "decorations", "version": "0.10.1",
        "bundled": [{ "work": "the Title font", "license": "OFL-1.1", "file": "decorations/COPYING", "source": "https://example.com/font/COPYING" }]
    }]));
    let mut decorations = record(
        "decorations",
        "MIT",
        files(&[("LICENSE", text("MIT", "decorations"))]),
    );
    decorations.version = "0.10.1".to_owned();
    decorations.fonts = vec!["src/title/Title.ttf".to_owned()];
    let notices = third_party_notices(&input(vec![decorations], Some(licenses))).unwrap();
    assert!(notices.contains(
        "decorations 0.10.1\n    License: MIT\n    Used under: MIT: [1]\n    Bundles the Title font, under OFL-1.1: [2] (from https://example.com/font/COPYING)\n"
    ));
    assert!(notices.contains("[2] OFL-1.1, the Title font\nUsed by decorations 0.10.1\n"));
    assert!(notices.contains("SIL OPEN FONT LICENSE Version 1.1"));
}

#[test]
fn font_files_need_a_bundled_entry_or_a_review() {
    let mut fonts = record("fonts-lib", "MIT", files(&[("LICENSE", text("MIT", "f"))]));
    fonts.fonts = vec!["tests/fonts/Test.ttf".to_owned()];
    let err = third_party_notices(&input(vec![fonts.clone()], None)).unwrap_err();
    assert!(
        err.contains("fonts-lib 1.0.0: carries font files (tests/fonts/Test.ttf) with neither a bundled entry nor a fonts_reviewed note"),
        "{err}"
    );
    let game = Folder::new();
    let licenses = game.clarify(&json!([{
        "name": "fonts-lib", "version": "1.0.0", "fonts_reviewed": "tests/fonts/ serves its tests."
    }]));
    let notices = third_party_notices(&input(vec![fonts], Some(licenses))).unwrap();
    assert!(notices.contains("fonts-lib 1.0.0\n"));
}

#[test]
fn every_embedded_work_and_license_file_is_embedded() {
    for (_, path) in EMBEDDED_WORKS {
        assert!(embedded::file(path).is_some(), "{path}");
    }
    assert!(embedded::file("LICENSE").is_some());
    assert!(embedded::file("licenses/clarifications.json").is_some());
}

#[test]
#[cfg_attr(
    windows,
    ignore = "python3 is not on every Windows host; the Linux runs hold the tables together"
)]
fn the_rules_match_release_py() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = r#"
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("release", sys.argv[1])
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
print(json.dumps({
    "preference": list(release.PREFERENCE),
    "markers": {name: [list(required), list(absent)] for name, (required, absent) in release.MARKERS.items()},
    "works": [path for _, path in release.EMBEDDED_WORKS],
}))
"#;
    let output = match std::process::Command::new("python3")
        .args(["-I", "-B", "-c", script])
        .arg(repo.join("scripts/release.py"))
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            eprintln!("skipped: no python3 to read scripts/release.py ({e})");
            return;
        }
    };
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let release: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(release["preference"], json!(PREFERENCE));
    let markers: serde_json::Map<String, serde_json::Value> = MARKERS
        .iter()
        .map(|(name, required, absent)| ((*name).to_owned(), json!([required, absent])))
        .collect();
    assert_eq!(release["markers"], serde_json::Value::Object(markers));
    let works: Vec<&str> = EMBEDDED_WORKS.iter().map(|(_, path)| *path).collect();
    assert_eq!(release["works"], json!(works));
}
