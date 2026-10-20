//! A game's third-party notices (`D-122`'s rules for the release archives,
//! ported from `scripts/release.py` for `tungsten package`, `D-182`): the
//! crates the binary links, each under the first alternative of its licence
//! whose texts it ships, the engine's crates under the engine's `LICENSE`,
//! the works the engine compiles in, and every text once. A drift test
//! holds `PREFERENCE`, `MARKERS` and `EMBEDDED_WORKS` to `release.py`'s.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, bail};
use serde_json::Value;

use crate::cargo::REPOSITORY;
use crate::embedded;

/// The notices file each archive gains.
pub(crate) const NOTICES: &str = "THIRD-PARTY-NOTICES.txt";
/// The standard library's notices, copied from the toolchain.
pub(crate) const STD_NOTICES: &str = "THIRD-PARTY-NOTICES-rust-std.html";
const STD_NOTICES_SOURCE: &str = "share/doc/rust/COPYRIGHT-library.html";
/// Where the engine's clarifications sit, and a game's.
const CLARIFICATIONS: &str = "licenses/clarifications.json";
/// The engine's crates: always listed, under the engine's root `LICENSE`.
pub(crate) const ENGINE_CRATES: [&str; 4] = [
    "tungsten",
    "tungsten-core",
    "tungsten-kit",
    "tungsten-render",
];
/// Third-party works the engine compiles into every binary, each with the
/// file in the engine's repository holding its licence.
pub(crate) const EMBEDDED_WORKS: [(&str, &str); 2] = [
    (
        "SMAA lookup textures and shaders (tungsten-render)",
        "crates/tungsten-render/src/assets/smaa/ATTRIBUTION.md",
    ),
    (
        "JetBrains Mono Regular and SemiBold, the engine font (tungsten, D-123)",
        "crates/tungsten/assets/fonts/OFL.txt",
    ),
];
const FONT_SUFFIXES: [&str; 5] = [".ttf", ".otf", ".ttc", ".woff", ".woff2"];
/// An expression's alternatives are tried in this order; the first whose
/// licences all have a text is used.
pub(crate) const PREFERENCE: [&str; 12] = [
    "MIT",
    "Apache-2.0",
    "Zlib",
    "BSD-3-Clause",
    "BSD-2-Clause",
    "ISC",
    "Unlicense",
    "0BSD",
    "CC0-1.0",
    "Unicode-3.0",
    "MPL-2.0",
    "Apache-2.0 WITH LLVM-exception",
];
const APACHE: [&str; 2] = [
    "terms and conditions for use, reproduction, and distribution",
    "grant of copyright license",
];
const BSD: &str = "redistribution and use in source and binary forms, with or without modification, are permitted";
/// Phrases of each licence's standard text that a file must hold, and for
/// look-alikes must not, compared in lowercase with whitespace collapsed.
pub(crate) const MARKERS: [(&str, &[&str], &[&str]); 12] = [
    (
        "MIT",
        &[
            "permission is hereby granted, free of charge, to any person obtaining a copy",
            "the above copyright notice and this permission notice shall be included in all copies",
        ],
        &[],
    ),
    ("Apache-2.0", &APACHE, &[]),
    (
        "Zlib",
        &[
            "permission is granted to anyone to use this software for any purpose",
            "altered source versions must be plainly marked as such",
        ],
        &[],
    ),
    ("BSD-3-Clause", &[BSD, "neither the name of"], &[]),
    (
        "BSD-2-Clause",
        &[
            BSD,
            "redistributions in binary form must reproduce the above copyright notice",
        ],
        &["endorse or promote products derived from this software"],
    ),
    (
        "ISC",
        &[
            "with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies",
        ],
        &[],
    ),
    (
        "Unlicense",
        &["this is free and unencumbered software released into the public domain"],
        &[],
    ),
    (
        "0BSD",
        &[
            "permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted.",
        ],
        &["provided that the above copyright notice"],
    ),
    (
        "CC0-1.0",
        &["cc0 1.0 universal", "statement of purpose"],
        &[],
    ),
    (
        "Unicode-3.0",
        &[
            "unicode license v3",
            "permission is hereby granted, free of charge, to any person obtaining a copy of data files",
        ],
        &[],
    ),
    (
        "MPL-2.0",
        &[
            "mozilla public license version 2.0",
            "exhibit a - source code form license notice",
        ],
        &[],
    ),
    (
        "Apache-2.0 WITH LLVM-exception",
        &[
            APACHE[0],
            APACHE[1],
            "llvm exceptions to the apache 2.0 license",
        ],
        &[],
    ),
];
const RULE_WIDTH: usize = 78;

/// One crate's licence data: cargo metadata's fields, the text of its
/// root-level licence and notice files and of its `license_file`, and the
/// font files anywhere in its package.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CrateRecord {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) license: Option<String>,
    pub(crate) repository: Option<String>,
    pub(crate) files: BTreeMap<String, String>,
    pub(crate) fonts: Vec<String>,
}

/// Everything the notices are written from.
#[derive(Debug, Clone, Default)]
pub(crate) struct NoticeInput {
    /// The game's package name and version.
    pub(crate) game: String,
    pub(crate) game_version: String,
    /// The engine's version, its crates' version.
    pub(crate) engine_version: String,
    pub(crate) target: String,
    /// `rustc --version`.
    pub(crate) rustc: String,
    /// The engine's crates the binary links.
    pub(crate) engine_crates: Vec<String>,
    /// Every other crate the binary links, the game's own left out.
    pub(crate) crates: Vec<CrateRecord>,
    /// The game's own `licenses/` folder, when it has a
    /// `clarifications.json`.
    pub(crate) game_licenses: Option<PathBuf>,
}

/// What `collect` finds: the input, less the game's name, plus the
/// standard library's notices.
pub(crate) struct Collected {
    pub(crate) input: NoticeInput,
    pub(crate) std_notices: String,
}

/// The crates `cargo tree`'s normal edges reach from `package` on `target`,
/// with `cargo metadata`'s details, run in `dir`; the toolchain's standard
/// library notices.
pub(crate) fn collect(dir: &Path, target: &str, package: &str) -> anyhow::Result<Collected> {
    // cargo tree resolves features as the build does; cargo metadata's
    // resolve also keeps edges the build never activates, so it supplies
    // package details only.
    let tree = run(
        "cargo",
        &[
            "tree", "-e", "normal", "--target", target, "--prefix", "none", "-f", "{p}", "-p",
            package,
        ],
        dir,
    )?;
    let metadata: Value = serde_json::from_str(&run(
        "cargo",
        &[
            "metadata",
            "--format-version",
            "1",
            "--filter-platform",
            target,
        ],
        dir,
    )?)
    .context("`cargo metadata` printed no JSON")?;
    let members: BTreeSet<&str> = metadata["workspace_members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut packages: HashMap<(&str, &str), Vec<&Value>> = HashMap::new();
    for package in crate::cargo::packages(&metadata) {
        if let (Some(name), Some(version)) = (package["name"].as_str(), package["version"].as_str())
        {
            packages.entry((name, version)).or_default().push(package);
        }
    }
    let mut seen = BTreeSet::new();
    let mut engine = BTreeSet::new();
    let mut engine_version = String::new();
    let mut crates = Vec::new();
    for line in tree.lines() {
        let mut words = line.split_whitespace();
        let (Some(name), Some(version)) = (words.next(), words.next()) else {
            continue;
        };
        let Some(version) = version.strip_prefix('v') else {
            continue;
        };
        if !seen.insert((name.to_owned(), version.to_owned())) {
            continue;
        }
        let found = packages
            .get(&(name, version))
            .map_or(&[][..], Vec::as_slice);
        let [package] = found else {
            bail!(
                "cargo metadata lists {} packages for {name} v{version}",
                found.len()
            );
        };
        if ENGINE_CRATES.contains(&name) {
            engine.insert(name.to_owned());
            if name == "tungsten" {
                version.clone_into(&mut engine_version);
            }
            continue;
        }
        if package["id"]
            .as_str()
            .is_some_and(|id| members.contains(id))
        {
            continue;
        }
        crates.push(crate_record(package).map_err(anyhow::Error::msg)?);
    }
    crates.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    let sysroot = run("rustc", &["--print", "sysroot"], dir)?;
    let std = Path::new(sysroot.trim()).join(STD_NOTICES_SOURCE);
    let std_notices = std::fs::read_to_string(&std).with_context(|| {
        format!(
            "{}: missing; the toolchain's rustc component ships the standard library's notices",
            std.display()
        )
    })?;
    let rustc = run("rustc", &["--version"], dir)?.trim().to_owned();
    let game_licenses = dir.join("licenses");
    Ok(Collected {
        input: NoticeInput {
            engine_version,
            target: target.to_owned(),
            rustc,
            engine_crates: engine.into_iter().collect(),
            crates,
            game_licenses: game_licenses
                .join("clarifications.json")
                .is_file()
                .then_some(game_licenses),
            ..NoticeInput::default()
        },
        std_notices,
    })
}

/// `program args` in `dir`: its standard output, else an error naming the
/// program and its standard error.
pub(crate) fn run(program: &str, args: &[&str], dir: &Path) -> anyhow::Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("cannot run `{program}`: is it installed and on PATH?"))?;
    if !output.status.success() {
        bail!(
            "`{program} {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A crate's record from its `cargo metadata` package.
pub(crate) fn crate_record(package: &Value) -> Result<CrateRecord, String> {
    let name = package["name"].as_str().unwrap_or_default().to_owned();
    let version = package["version"].as_str().unwrap_or_default().to_owned();
    let label = format!("{name} {version}");
    let manifest = Path::new(package["manifest_path"].as_str().unwrap_or_default());
    let Some(folder) = manifest.parent().filter(|folder| folder.is_dir()) else {
        return Err(format!("{label}: no source at {}", manifest.display()));
    };
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .map_err(|e| format!("{label}: cannot list {}: {e}", folder.display()))?
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| is_license_file(name))
        .collect();
    names.sort();
    if let Some(file) = package["license_file"].as_str() {
        names.push(file.replace('\\', "/"));
    }
    let mut files = BTreeMap::new();
    for name in names {
        if files.contains_key(&name) {
            continue;
        }
        let path = folder.join(&name);
        let data = std::fs::read(&path).map_err(|_| {
            format!(
                "{label}: its license_file {name} is missing from {}",
                folder.display()
            )
        })?;
        files.insert(name, read_any(&data));
    }
    let mut fonts = Vec::new();
    font_files(folder, folder, &mut fonts);
    fonts.sort();
    Ok(CrateRecord {
        name,
        version,
        license: package["license"].as_str().map(str::to_owned),
        repository: package["repository"].as_str().map(str::to_owned),
        files,
        fonts,
    })
}

/// A root-level file that may hold a licence text or notices.
fn is_license_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        "license",
        "licence",
        "copying",
        "copyright",
        "unlicense",
        "notice",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn is_notice_file(name: &str) -> bool {
    name.to_lowercase().starts_with("notice")
}

/// The font files under `dir`, relative to `root`, `/`-separated.
fn font_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            font_files(root, &path, out);
        } else {
            let lower = path.to_string_lossy().to_lowercase();
            if FONT_SUFFIXES.iter().any(|suffix| lower.ends_with(suffix))
                && let Ok(rel) = path.strip_prefix(root)
            {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

/// A text file as UTF-8, else Latin-1, with a byte-order mark dropped and
/// line endings as LF.
pub(crate) fn read_any(data: &[u8]) -> String {
    let text = match std::str::from_utf8(data) {
        Ok(text) => text.to_owned(),
        Err(_) => data.iter().map(|&b| char::from(b)).collect(),
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// An SPDX-style expression's alternatives, each a list of licences: `/`
/// reads as OR, AND binds tighter than OR, and WITH stays with its licence.
pub(crate) fn alternatives(expression: &str) -> Result<Vec<Vec<String>>, String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    for c in expression.chars() {
        if c == '(' || c == ')' || c == '/' || c.is_whitespace() {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            if !c.is_whitespace() {
                tokens.push(c.to_string());
            }
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    let mut parser = Parser {
        tokens,
        at: 0,
        expression,
    };
    let result = parser.any_of()?;
    if parser.at != parser.tokens.len() {
        return Err(parser.malformed());
    }
    Ok(result)
}

struct Parser<'a> {
    tokens: Vec<String>,
    at: usize,
    expression: &'a str,
}

impl Parser<'_> {
    fn malformed(&self) -> String {
        format!("license expression {:?} is malformed", self.expression)
    }

    fn peek(&self) -> Option<String> {
        self.tokens.get(self.at).map(|token| token.to_uppercase())
    }

    fn take(&mut self) -> Result<String, String> {
        let token = self
            .tokens
            .get(self.at)
            .cloned()
            .ok_or_else(|| self.malformed())?;
        self.at += 1;
        Ok(token)
    }

    fn any_of(&mut self) -> Result<Vec<Vec<String>>, String> {
        let mut result = self.all_of()?;
        while matches!(self.peek().as_deref(), Some("OR" | "/")) {
            self.take()?;
            for alternative in self.all_of()? {
                if !result.contains(&alternative) {
                    result.push(alternative);
                }
            }
        }
        Ok(result)
    }

    fn all_of(&mut self) -> Result<Vec<Vec<String>>, String> {
        let mut result = self.one()?;
        while self.peek().as_deref() == Some("AND") {
            self.take()?;
            let right = self.one()?;
            let mut next = Vec::new();
            for left in &result {
                for other in &right {
                    let mut both: Vec<String> = Vec::new();
                    for license in left.iter().chain(other) {
                        if !both.contains(license) {
                            both.push(license.clone());
                        }
                    }
                    next.push(both);
                }
            }
            result = next;
        }
        Ok(result)
    }

    fn one(&mut self) -> Result<Vec<Vec<String>>, String> {
        let token = self.take()?;
        if token == "(" {
            let result = self.any_of()?;
            if self.take()? != ")" {
                return Err(self.malformed());
            }
            return Ok(result);
        }
        if token == ")"
            || token == "/"
            || matches!(token.to_uppercase().as_str(), "AND" | "OR" | "WITH")
        {
            return Err(self.malformed());
        }
        let token = if self.peek().as_deref() == Some("WITH") {
            self.take()?;
            format!("{token} WITH {}", self.take()?)
        } else {
            token
        };
        Ok(vec![vec![token]])
    }
}

fn markers(license: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    MARKERS
        .iter()
        .find(|(name, ..)| *name == license)
        .map(|(_, required, absent)| (*required, *absent))
}

/// Whether `text` holds `license`'s marker phrases and none of its
/// look-alikes'.
pub(crate) fn matches(license: &str, text: &str) -> bool {
    let Some((required, absent)) = markers(license) else {
        return false;
    };
    let flat = text
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    required.iter().all(|phrase| flat.contains(phrase))
        && !absent.iter().any(|phrase| flat.contains(phrase))
}

/// The licences of an alternative, and the file each is read from.
pub(crate) type Choice = (Vec<String>, Vec<(String, String)>);

/// The first of the expression's alternatives, in `PREFERENCE` order, whose
/// licences each match one of `files`, with the file each is read from; of
/// two matching files the more specific serves. `None` when none does.
pub(crate) fn choose(
    expression: &str,
    files: &BTreeMap<String, String>,
) -> Result<Option<Choice>, String> {
    let preference = |license: &String| PREFERENCE.iter().position(|p| p == license);
    let mut known: Vec<(Vec<usize>, Vec<String>)> = alternatives(expression)?
        .into_iter()
        .filter_map(|alternative| {
            let mut ranks: Vec<usize> =
                alternative.iter().map(preference).collect::<Option<_>>()?;
            ranks.sort_unstable();
            Some((ranks, alternative))
        })
        .collect();
    known.sort_by(|a, b| a.0.cmp(&b.0));
    'alternatives: for (_, alternative) in known {
        let mut picks = Vec::new();
        for license in &alternative {
            let found = files
                .iter()
                .filter(|(name, text)| !is_notice_file(name) && matches(license, text));
            let best = found.min_by_key(|(name, text)| {
                let count = MARKERS
                    .iter()
                    .filter(|(other, ..)| matches(other, text))
                    .count();
                (count, name.len(), (*name).clone())
            });
            let Some((name, _)) = best else {
                continue 'alternatives;
            };
            picks.push((license.clone(), name.clone()));
        }
        return Ok(Some((alternative, picks)));
    }
    Ok(None)
}

/// A clarification and the `licenses/` folder its files sit in.
struct Clarification {
    entry: Value,
    /// `None` for the engine's, embedded in this command line.
    folder: Option<PathBuf>,
}

/// The clarifications by crate name: the engine's, then the game's.
fn clarifications(
    game: Option<&Path>,
    errors: &mut Vec<String>,
) -> HashMap<String, Vec<Clarification>> {
    let mut by_name: HashMap<String, Vec<Clarification>> = HashMap::new();
    let mut add = |text: &str, folder: Option<PathBuf>, errors: &mut Vec<String>| {
        let parsed: Result<Value, _> = serde_json::from_str(text);
        let Ok(Value::Array(entries)) = parsed.map(|value| value["crates"].clone()) else {
            errors.push(format!("{CLARIFICATIONS}: not a clarifications file"));
            return;
        };
        for entry in entries {
            let name = entry["name"].as_str().unwrap_or_default().to_owned();
            by_name.entry(name).or_default().push(Clarification {
                entry,
                folder: folder.clone(),
            });
        }
    };
    match embedded::file(CLARIFICATIONS) {
        Some(data) => add(&read_any(data), None, errors),
        None => errors.push(format!("the engine's {CLARIFICATIONS} is not embedded")),
    }
    if let Some(folder) = game {
        match std::fs::read(folder.join("clarifications.json")) {
            Ok(data) => add(&read_any(&data), Some(folder.to_path_buf()), errors),
            Err(e) => errors.push(format!("the game's {CLARIFICATIONS}: {e}")),
        }
    }
    by_name
}

/// The text of a clarification item's `file`, or `None` after recording
/// why not.
fn clarified_text(
    clarification: &Clarification,
    item: &Value,
    errors: &mut Vec<String>,
    label: &str,
) -> Option<String> {
    let file = item["file"].as_str().unwrap_or_default();
    let escapes = Path::new(file)
        .components()
        .any(|part| !matches!(part, Component::Normal(_)));
    let data = if escapes {
        None
    } else {
        match &clarification.folder {
            None => embedded::file(&format!("licenses/{file}")).map(<[u8]>::to_vec),
            Some(folder) => std::fs::read(folder.join(file)).ok(),
        }
    };
    if data.is_none() {
        let whose = if clarification.folder.is_some() {
            "the game's"
        } else {
            "the engine's"
        };
        errors.push(format!(
            "{label}: {whose} {CLARIFICATIONS} names {file}, which its licenses/ lacks"
        ));
    }
    data.map(|data| read_any(&data))
}

/// The cited texts, each once, numbered in the order first cited.
#[derive(Default)]
struct Texts {
    order: Vec<(String, Vec<String>, Vec<String>)>,
    index: HashMap<String, usize>,
}

impl Texts {
    /// `[n]` for `text`, recording `kind` and `user` against it.
    fn cite(&mut self, text: &str, kind: &str, user: &str) -> String {
        let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
        let start = lines
            .iter()
            .position(|line| !line.is_empty())
            .unwrap_or(lines.len());
        let end = lines
            .iter()
            .rposition(|line| !line.is_empty())
            .map_or(start, |i| i + 1);
        let key = lines[start..end].join("\n");
        let next = self.order.len();
        let at = *self.index.entry(key.clone()).or_insert(next);
        if at == next {
            self.order.push((key, Vec::new(), Vec::new()));
        }
        let (_, kinds, users) = &mut self.order[at];
        if !kinds.iter().any(|k| k == kind) {
            kinds.push(kind.to_owned());
        }
        if !users.iter().any(|u| u == user) {
            users.push(user.to_owned());
        }
        format!("[{}]", at + 1)
    }
}

/// `THIRD-PARTY-NOTICES.txt` for `input`. The error names every crate
/// without a licence text, every stale clarification, every unreviewed
/// font and every missing file.
pub(crate) fn third_party_notices(input: &NoticeInput) -> Result<String, String> {
    let mut errors = Vec::new();
    let clarified = clarifications(input.game_licenses.as_deref(), &mut errors);
    let mut unmatched = Vec::new();
    let mut texts = Texts::default();
    let mut index: Vec<(String, String, String)> = Vec::new();

    let engine_license = embedded::file("LICENSE").map(read_any);
    for name in &input.engine_crates {
        let label = format!("{name} {}", input.engine_version);
        let Some(license) = &engine_license else {
            errors.push("the engine's LICENSE is not embedded".to_owned());
            break;
        };
        let cited = texts.cite(license, "MIT", &label);
        index.push((
            name.clone(),
            input.engine_version.clone(),
            format!(
                "{label}\n    License: MIT\n    Used under: MIT: {cited}\n    Source: {REPOSITORY}/tree/v{}/crates/{name}",
                input.engine_version
            ),
        ));
    }

    for record in &input.crates {
        let label = format!("{} {}", record.name, record.version);
        let expression = record.license.clone().unwrap_or_default();
        let entries = clarified.get(&record.name).map_or(&[][..], Vec::as_slice);
        let found = entries
            .iter()
            .find(|c| c.entry["version"].as_str() == Some(record.version.as_str()));
        if !entries.is_empty() && found.is_none() {
            let mut checked: Vec<String> = entries
                .iter()
                .map(|c| c.entry["version"].as_str().unwrap_or("none").to_owned())
                .collect();
            checked.sort();
            errors.push(format!(
                "{label}: {CLARIFICATIONS} checked it at {}; check upstream again and add an entry for this version to the game's {CLARIFICATIONS}",
                checked.join(", ")
            ));
            continue;
        }
        let empty = Value::Null;
        let entry = found.map_or(&empty, |c| &c.entry);
        let mut lines = vec![
            label.clone(),
            format!(
                "    License: {}",
                if expression.is_empty() {
                    "none declared"
                } else {
                    &expression
                }
            ),
        ];
        let entry_texts = entry["texts"].as_array().filter(|texts| !texts.is_empty());
        if let (Some(items), Some(clarification)) = (entry_texts, found) {
            let used: Vec<String> = items
                .iter()
                .map(|item| item["license"].as_str().unwrap_or_default().to_owned())
                .collect();
            if !expression.is_empty() {
                match alternatives(&expression) {
                    Ok(known) if known.contains(&used) => {}
                    Ok(_) => {
                        errors.push(format!(
                            "{label}: {CLARIFICATIONS} names {}, not an alternative of {expression:?}",
                            used.join(" AND ")
                        ));
                        continue;
                    }
                    Err(e) => {
                        errors.push(format!("{label}: {e}"));
                        continue;
                    }
                }
            }
            let mut cited = Vec::new();
            for item in items {
                let license = item["license"].as_str().unwrap_or_default();
                let Some(text) = clarified_text(clarification, item, &mut errors, &label) else {
                    continue;
                };
                if markers(license).is_some() && !matches(license, &text) {
                    errors.push(format!(
                        "{label}: {} does not read as {license}",
                        item["file"].as_str().unwrap_or_default()
                    ));
                }
                cited.push(texts.cite(&text, license, &label));
            }
            let sources: Vec<&str> = items
                .iter()
                .filter_map(|item| item["source"].as_str())
                .collect();
            lines.push(format!(
                "    Used under: {}: {} (from {})",
                used.join(" AND "),
                cited.join(", "),
                sources.join(", ")
            ));
        } else {
            let choice = if expression.is_empty() {
                Ok(None)
            } else {
                choose(&expression, &record.files)
            };
            match choice {
                Ok(Some((alternative, picks))) => {
                    let cited: Vec<String> = picks
                        .iter()
                        .map(|(license, name)| texts.cite(&record.files[name], license, &label))
                        .collect();
                    lines.push(format!(
                        "    Used under: {}: {}",
                        alternative.join(" AND "),
                        cited.join(", ")
                    ));
                }
                Ok(None) => {
                    let files: Vec<&str> = record.files.keys().map(String::as_str).collect();
                    unmatched.push(format!(
                        "{label} ({}; files: {})",
                        if expression.is_empty() {
                            "no license field"
                        } else {
                            &expression
                        },
                        if files.is_empty() {
                            "none".to_owned()
                        } else {
                            files.join(", ")
                        }
                    ));
                    continue;
                }
                Err(e) => {
                    errors.push(format!("{label}: {e}"));
                    continue;
                }
            }
        }
        let notices: Vec<String> = record
            .files
            .iter()
            .filter(|(name, _)| is_notice_file(name))
            .map(|(_, text)| texts.cite(text, "NOTICE", &label))
            .collect();
        if !notices.is_empty() {
            lines.push(format!("    Notices: {}", notices.join(", ")));
        }
        let bundled = entry["bundled"].as_array().map_or(&[][..], Vec::as_slice);
        if let Some(clarification) = found {
            for work in bundled {
                if let Some(text) = clarified_text(clarification, work, &mut errors, &label) {
                    let name = work["work"].as_str().unwrap_or_default();
                    let license = work["license"].as_str().unwrap_or_default();
                    let cited = texts.cite(&text, &format!("{license}, {name}"), &label);
                    lines.push(format!(
                        "    Bundles {name}, under {license}: {cited} (from {})",
                        work["source"].as_str().unwrap_or_default()
                    ));
                }
            }
        }
        if !record.fonts.is_empty() && bundled.is_empty() && entry["fonts_reviewed"].is_null() {
            errors.push(format!(
                "{label}: carries font files ({}) with neither a bundled entry nor a fonts_reviewed note in {CLARIFICATIONS}",
                record.fonts.join(", ")
            ));
        }
        lines.push(format!(
            "    https://crates.io/crates/{}/{}",
            record.name, record.version
        ));
        if let Some(repository) = &record.repository {
            lines.push(format!("    Repository: {repository}"));
        }
        index.push((
            record.name.clone(),
            record.version.clone(),
            lines.join("\n"),
        ));
    }

    let mut works = Vec::new();
    for (title, path) in EMBEDDED_WORKS {
        match embedded::file(path) {
            Some(data) => works.push(format!(
                "{title}\n{}\n{}\n",
                "-".repeat(RULE_WIDTH),
                read_any(data).trim()
            )),
            None => errors.push(format!(
                "{path}: not embedded, though EMBEDDED_WORKS names it for {title}"
            )),
        }
    }
    if !unmatched.is_empty() {
        errors.insert(
            0,
            format!(
                "no license text matches an alternative of {} crate(s); add each to the game's {CLARIFICATIONS} (the engine's docs/releases.md#license-notices): {}",
                unmatched.len(),
                unmatched.join("; ")
            ),
        );
    }
    if !errors.is_empty() {
        let mut message = vec!["third-party notices:".to_owned()];
        message.extend(errors);
        return Err(message.join("\n  "));
    }

    index.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    let intro = format!(
        "{} {} is built on the Tungsten engine {}, whose crates are under the MIT license. The game's own license files, if it has any, sit beside this file. Every binary links the Rust standard library, built by {}; its notices are in {STD_NOTICES}. This file lists the third-party works compiled into the engine, then the {} crates the binary links, each with the license it is used under and the numbers of its texts, then each text once with the crates that use it.",
        input.game,
        input.game_version,
        input.engine_version,
        input.rustc,
        index.len()
    );
    let rule = "=".repeat(RULE_WIDTH);
    let mut parts = vec![
        format!(
            "Third-party notices: {} {} for {}",
            input.game, input.game_version, input.target
        ),
        rule.clone(),
        String::new(),
        wrap(&intro),
        String::new(),
        String::new(),
        "Works compiled into the engine".to_owned(),
        rule.clone(),
        String::new(),
    ];
    parts.extend(works);
    parts.extend([
        String::new(),
        "Crates".to_owned(),
        rule.clone(),
        String::new(),
    ]);
    parts.extend(index.into_iter().map(|(_, _, item)| item + "\n"));
    parts.extend([
        String::new(),
        "License texts".to_owned(),
        rule,
        String::new(),
    ]);
    for (number, (text, kinds, users)) in texts.order.iter().enumerate() {
        // No-break spaces keep each crate's name and version on one line.
        let users: Vec<String> = users
            .iter()
            .map(|user| user.replace(' ', "\u{a0}"))
            .collect();
        parts.push(format!("[{}] {}", number + 1, kinds.join("; ")));
        parts.push(wrap(&format!("Used by {}", users.join(", "))).replace('\u{a0}', " "));
        parts.push("-".repeat(RULE_WIDTH));
        parts.push(text.clone());
        parts.push(String::new());
    }
    Ok(parts.join("\n"))
}

/// `text` filled to 78 columns, breaking only at ASCII whitespace.
fn wrap(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text
        .split(|c: char| c.is_ascii_whitespace())
        .filter(|w| !w.is_empty())
    {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > RULE_WIDTH {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines.join("\n")
}

#[cfg(test)]
#[path = "tests/notices.rs"]
mod tests;
