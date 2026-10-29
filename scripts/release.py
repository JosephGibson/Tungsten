#!/usr/bin/env python3
"""Release version consistency, changelog cut, notes and packaging (stdlib only).

  check [TAG]         Cargo.toml's workspace version is the newest CHANGELOG.md
                      release; `## [Unreleased]` comes first; release headings read
                      `## [X.Y.Z] - YYYY-MM-DD`, newest version and date first; and
                      the DESIGN.md "Workspace `vX.Y.Z`" status line agrees.
                      A tag whose version has a CHANGELOG section must equal
                      the workspace version and have notes; so must every vX.Y.Z tag.
                      A pre-release tag without a section (e.g. v0.0.0-test) is a
                      rehearsal: the tree must still agree, the tag isn't compared.
  version             Prints the workspace version of a consistent tree.
  cut VERSION         Moves the [Unreleased] body under `## [VERSION] - DATE`, leaves
                      an empty [Unreleased], and sets the workspace version and status
                      lines. `just release minor` also refreshes Cargo.lock.
  run KIND [SUMMARY]  `just release` (D-227). `minor`: on the branch named for the next
                      minor after the newest vX.Y.Z tag on HEAD's history, a clean tree
                      and a subject of 72 characters at most, the docs ratchet, the cut
                      and the lock refresh, then CHECKS in order; the four cut files are
                      committed as `chore(release): X.Y.Z SUMMARY` and the preflight
                      prints the agent's push and the owner's block. A failed check
                      leaves the cut uncommitted, and a rerun resumes at the checks.
                      `rehearsal`: a unique v0.0.0-test.YYYYMMDD.gSHA on HEAD, checked,
                      and the preflight's rehearsal block; no cut or commit. Other kinds
                      wait for the redo's 9.8. --dry-run prints the chain, writing nothing.
  notes TAG           Prints the tag's CHANGELOG section ([Unreleased] for a
                      rehearsal tag), optionally with absolute repository links.
  package TAG TARGET  Zips each CPU level's example and template builds (BIN_DIR/
                      <level>/TARGET/release, one cargo target dir per level) for players:
                      OUT/tungsten-examples-VERSION-PLATFORM.zip, e.g.
                      tungsten-examples-0.92.0-windows-x64.zip. Its top folder holds one
                      launcher per build under the build's player-facing name (`TITLES`),
                      README.txt, LICENSE.txt and THIRD-PARTY-NOTICES.txt; data/ holds
                      the builds under bin/<level>/, the files the examples read relative
                      to the working directory, and each folder game's own tungsten.json,
                      input.json and assets/ under its folder, which its launcher runs it
                      from: the templates and every example whose folder holds its own
                      tungsten.json. Linux binaries are stripped with objcopy; no debug
                      file ships.
  split-debug BINARY DEBUG
                      The Linux split alone: moves BINARY's debug info to DEBUG, links
                      it, and checks the build-ids and DEBUG's .debug_line.
  licenses TARGET     Collects the license data of the crates the shipped packages link
                      for TARGET (cargo tree's normal edges, cargo metadata's details,
                      the crates' license and notice files) and the toolchain's
                      standard-library notices into OUT (a build job's licenses.json).
  notices DATA        Writes THIRD-PARTY-NOTICES.txt from DATA into OUT, as `package`
                      does inside each archive: each crate's texts for its preferred
                      license alternative, its NOTICE files, licenses/clarifications.json,
                      the embedded works and, last, the standard library's notices as
                      plain text.

Rationale: D-071, D-072, D-074, D-079, D-120, D-122, D-186, D-196, D-227. Release steps: docs/releases.md.
"""

import argparse
import datetime
import html.parser
import importlib.util
import json
import re
import shlex
import shutil
import struct
import subprocess
import sys
import tempfile
import textwrap
from collections import namedtuple
from pathlib import Path

CARGO = "Cargo.toml"
CHANGELOG = "CHANGELOG.md"
# Human status lines that name the workspace version; `cut` rewrites them.
STATUS_FILES = ("DESIGN.md",)
STATUS_RE = re.compile(r"(Workspace `v)([^`]*)(`)")
# `just release` (D-227): the kinds switched on, the subject's limit, the paths the cut
# commits and the chain the cut must pass, in order.
KINDS = ("minor", "rehearsal")
SUBJECT_LIMIT = 72
CUT_PATHS = (CHANGELOG, CARGO, "Cargo.lock", *STATUS_FILES)
RATCHET = ("just", "docs-ratchet")
LOCK = ("cargo", "update", "--workspace", "--offline")
CHECKS = (("just", "check"), ("just", "ctx"), ("just", "repo-check"), ("just", "api-check"),
          ("just", "script-test"), ("just", "smoke"), ("just", "perf", "counters"), ("just", "notices"),
          ("just", "template-check"), ("git", "diff", "--check"))
UNRELEASED = "## [Unreleased]"
UNRELEASED_RE = re.compile(r"^## \[Unreleased\][ \t]*$", re.M)
RELEASE_RE = re.compile(r"^## \[([^\]]*)\] - (\d{4}-\d{2}-\d{2})$", re.ASCII)
# SemVer 2.0 without build metadata; groups: major, minor, patch, pre-release.
_ID = r"(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)"
SEMVER = r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(" + _ID + r"(?:\." + _ID + r")*))?"
SEMVER_RE = re.compile("^" + SEMVER + "$", re.ASCII)
# Groups: 1 version, 2-4 major/minor/patch, 5 pre-release.
TAG_RE = re.compile("^v(" + SEMVER + ")$", re.ASCII)
TABLE_RE = re.compile(r"^\s*\[\[?\s*([^\]\s]+)\s*\]\]?\s*(?:#.*)?$")
VERSION_RE = re.compile(r'^\s*version\s*=\s*"([^"]*)"')
NAME_RE = re.compile(r'^\s*name\s*=\s*"([^"]+)"')
TARGET_RE = re.compile(r"^[A-Za-z0-9_.-]+$")
# Relative Markdown link targets (no scheme, anchor or absolute path).
RELATIVE_LINK_RE = re.compile(r"\]\((?![A-Za-z][A-Za-z0-9+.-]*:|#|/)([^)\s]+)\)")
# What every example reads relative to the working directory.
SHARED_RUNTIME = ("tungsten.json", "input.json", "assets")
# What a folder game, a template or an example with its own tungsten.json, reads from its own
# folder, which its launcher runs it from (D-186, D-196).
FOLDER_RUNTIME = ("tungsten.json", "input.json", "assets")
ARCHIVE_PREFIX = "tungsten-examples"
# The archive folder that holds the builds and every file they read, beside the launchers; must
# match `DATA` in tools/launcher/src/main.rs (D-196).
DATA = "data"
# Archive name and README wording per target; another target keeps its triple.
PLATFORMS = {
    "x86_64-pc-windows-msvc": ("windows-x64", "Windows (x64)"),
    "x86_64-unknown-linux-gnu": ("linux-x64", "Linux (x64)"),
}
# What players see each shipped package as: its launcher's and builds' file name and its README
# line. A package missing here keeps its package name. Folder games' titles must match `FOLDERS`
# in tools/launcher/src/main.rs.
TITLES = {
    "example-01-platformer": ("Platformer", "a small platformer level"),
    "example-02-bench": ("Benchmark", "the engine's stress tests; Tab moves to the next one"),
    "tungsten-template-basic": ("Template", "the starting game every new Tungsten project gets"),
}
# CPU levels release archives may carry, fastest first; must match `LEVELS` in
# tools/launcher/src/main.rs. The portable baseline is mandatory (D-072).
LEVELS = ("x86-64-v4", "x86-64-v3", "x86-64-v2", "x86-64")
BASELINE = "x86-64"
LAUNCHER = "tungsten-launcher"
# Workspace examples no archive ships: the fixtures crate is test-only (D-193).
NOT_SHIPPED = frozenset({"example-03-fixtures"})

# Third-party license notices (D-122): the build jobs' license data, the files each archive
# gains, and the repository's clarifications for what crate packages lack or misstate.
LICENSE_DATA = "licenses.json"
NOTICES = "THIRD-PARTY-NOTICES.txt"
STD_NOTICES_SOURCE = "share/doc/rust/COPYRIGHT-library.html"  # in the toolchain's sysroot
CLARIFICATIONS = "licenses/clarifications.json"
# Third-party works the engine compiles into every binary, each with the file holding its license.
EMBEDDED_WORKS = (
    ("SMAA lookup textures and shaders (tungsten-render)", "crates/tungsten-render/src/assets/smaa/ATTRIBUTION.md"),
    ("JetBrains Mono Regular and SemiBold, the engine font (tungsten, D-123)", "crates/tungsten/assets/fonts/OFL.txt"),
)
# Root-level package files that may hold a license text or notices, by name prefix in any case.
LICENSE_FILE_RE = re.compile(r"^(licen[cs]e|copying|copyright|unlicense|notice)", re.I)
NOTICE_FILE_RE = re.compile(r"^notice", re.I)
FONT_SUFFIXES = (".ttf", ".otf", ".ttc", ".woff", ".woff2")
TREE_LINE_RE = re.compile(r"^(\S+) v(\S+)")
# An expression's alternatives are tried in this order; the first whose licenses all have a text is used.
PREFERENCE = ("MIT", "Apache-2.0", "Zlib", "BSD-3-Clause", "BSD-2-Clause", "ISC", "Unlicense", "0BSD",
              "CC0-1.0", "Unicode-3.0", "MPL-2.0", "Apache-2.0 WITH LLVM-exception")
_APACHE = ("terms and conditions for use, reproduction, and distribution", "grant of copyright license")
_BSD = "redistribution and use in source and binary forms, with or without modification, are permitted"
# Phrases of each license's standard text that a file must hold, and for look-alikes must not,
# compared in lowercase with whitespace collapsed.
MARKERS = {
    "MIT": (("permission is hereby granted, free of charge, to any person obtaining a copy",
             "the above copyright notice and this permission notice shall be included in all copies"), ()),
    "Apache-2.0": (_APACHE, ()),
    "Zlib": (("permission is granted to anyone to use this software for any purpose",
              "altered source versions must be plainly marked as such"), ()),
    "BSD-3-Clause": ((_BSD, "neither the name of"), ()),
    "BSD-2-Clause": ((_BSD, "redistributions in binary form must reproduce the above copyright notice"),
                     ("endorse or promote products derived from this software",)),
    "ISC": (("with or without fee is hereby granted, provided that the above copyright notice and this "
             "permission notice appear in all copies",), ()),
    "Unlicense": (("this is free and unencumbered software released into the public domain",), ()),
    "0BSD": (("permission to use, copy, modify, and/or distribute this software for any purpose with or "
              "without fee is hereby granted.",), ("provided that the above copyright notice",)),
    "CC0-1.0": (("cc0 1.0 universal", "statement of purpose"), ()),
    "Unicode-3.0": (("unicode license v3", "permission is hereby granted, free of charge, to any person "
                     "obtaining a copy of data files"), ()),
    "MPL-2.0": (("mozilla public license version 2.0", "exhibit a - source code form license notice"), ()),
    "Apache-2.0 WITH LLVM-exception": (_APACHE + ("llvm exceptions to the apache 2.0 license",), ()),
}
RULE = "=" * 78

Release = namedtuple("Release", "version date body")
State = namedtuple("State", "errors version releases unreleased")
# A PE file's CodeView (RSDS) record: the PDB's GUID bytes, its age and the path it names.
CodeView = namedtuple("CodeView", "guid age name")

SHT_NOTE = 7
SHT_NOBITS = 8
NT_GNU_BUILD_ID = 3
MSF_MAGIC = b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\x00\x00\x00"


def read(root, rel):
    return (root / rel).read_text(encoding="utf-8")


def write(root, rel, text):
    with (root / rel).open("w", encoding="utf-8", newline="\n") as out:
        out.write(text)


def semver_key(version):
    """SemVer precedence: numeric identifiers sort before alphanumeric ones and a
    pre-release sorts before its release."""
    match = SEMVER_RE.match(version)
    core = (int(match[1]), int(match[2]), int(match[3]))
    if match[4] is None:
        return core + (1, ())
    ids = tuple((0, int(part), "") if part.isdigit() else (1, 0, part) for part in match[4].split("."))
    return core + (0, ids)


def find_release(state, version):
    return next((r for r in state.releases if r.version == version), None)


def is_rehearsal(state, tag):
    """A pre-release tag without its own CHANGELOG section (e.g. v0.0.0-test)."""
    match = TAG_RE.match(tag)
    return bool(match and match[5]) and find_release(state, match[1]) is None


def table_lines(text, table):
    """(index, line) pairs inside one TOML table, e.g. `workspace.package`."""
    current = None
    for index, line in enumerate(text.splitlines(keepends=True)):
        header = TABLE_RE.match(line)
        if header:
            current = header[1]
        elif current == table:
            yield index, line


def table_value(text, table, pattern):
    for index, line in table_lines(text, table):
        match = pattern.match(line)
        if match:
            return index, match
    return None, None


def cargo_version(text):
    _, match = table_value(text, "workspace.package", VERSION_RE)
    return match[1] if match else None


def set_cargo_version(text, version):
    index, match = table_value(text, "workspace.package", VERSION_RE)
    if match is None:
        raise ValueError(f"{CARGO}: no version in [workspace.package]")
    lines = text.splitlines(keepends=True)
    lines[index] = lines[index][:match.start(1)] + version + lines[index][match.end(1):]
    return "".join(lines)


def changelog_sections(text):
    """(heading, stripped body) for every level-2 heading."""
    lines = text.splitlines()
    heads = [i for i, line in enumerate(lines) if line.startswith("## ")]
    return [(lines[start].rstrip(), "\n".join(lines[start + 1:end]).strip())
            for start, end in zip(heads, heads[1:] + [len(lines)])]


def check_tree(root, read_file=None):
    """Check working files, or an explicit reader for a committed Git tree."""
    read_file = read_file or (lambda rel: read(root, rel))
    errors = []
    version = cargo_version(read_file(CARGO))
    if version is None:
        errors.append(f"{CARGO}: no version in [workspace.package]")
    elif not SEMVER_RE.match(version):
        errors.append(f"{CARGO}: workspace version {version!r} is not SemVer X.Y.Z[-pre]")

    sections = changelog_sections(read_file(CHANGELOG))
    headings = [heading for heading, _ in sections]
    unreleased = None
    if headings.count(UNRELEASED) != 1 or headings[0] != UNRELEASED:
        errors.append(f"{CHANGELOG}: must start with exactly one '{UNRELEASED}' section")
    elif sections:
        unreleased = sections[0][1]
    releases = []
    for heading, body in sections:
        if heading == UNRELEASED:
            continue
        match = RELEASE_RE.match(heading)
        if not match or not SEMVER_RE.match(match[1]):
            errors.append(f"{CHANGELOG}: heading {heading!r} is not '## [X.Y.Z] - YYYY-MM-DD'")
            continue
        try:
            date = datetime.date.fromisoformat(match[2])
        except ValueError:
            errors.append(f"{CHANGELOG}: heading {heading!r} has an invalid date")
            continue
        releases.append(Release(match[1], date, body))
    if not releases:
        errors.append(f"{CHANGELOG}: no release headings")
    for newer, older in zip(releases, releases[1:]):
        if semver_key(newer.version) <= semver_key(older.version):
            errors.append(f"{CHANGELOG}: [{newer.version}] must be above [{older.version}] (newest first)")
        if newer.date < older.date:
            errors.append(f"{CHANGELOG}: [{newer.version}] dated {newer.date} is before [{older.version}] ({older.date})")
    if version and releases and releases[0].version != version:
        errors.append(f"{CARGO} workspace version {version} != newest {CHANGELOG} release "
                      f"[{releases[0].version}]; bump both with `just release minor`")

    for rel in STATUS_FILES:
        found = [match[2] for match in STATUS_RE.finditer(read_file(rel))]
        if not found:
            errors.append(f"{rel}: no 'Workspace `vX.Y.Z`' status line")
        for value in found:
            if value != version:
                errors.append(f"{rel}: status line says v{value}, {CARGO} says {version}")
    return State(errors, version, releases, unreleased)


def check_tag(state, tag):
    match = TAG_RE.match(tag)
    if not match:
        return [f"tag {tag!r} is not v<SemVer>, e.g. v0.27.0 or v0.27.0-rc.1"]
    if is_rehearsal(state, tag):
        return []
    if match[1] != state.version:
        return [f"tag {tag} does not match {CARGO} workspace version {state.version}"]
    release = find_release(state, match[1])
    if release is None:
        return [f"{CHANGELOG}: no [{match[1]}] section for tag {tag}"]
    if not release.body:
        return [f"{CHANGELOG}: [{match[1]}] has no notes for tag {tag}"]
    return []


def cut(root, version, date):
    """Returns errors; writes nothing unless the tree and the request are valid."""
    state = check_tree(root)
    if state.errors:
        return ["tree is inconsistent; fix it before cutting"] + state.errors
    errors = []
    if not SEMVER_RE.match(version):
        errors.append(f"{version!r} is not SemVer X.Y.Z[-pre] (no leading v)")
    elif semver_key(version) <= semver_key(state.version):
        errors.append(f"{version} must be above the current version {state.version}")
    if date < state.releases[0].date:
        errors.append(f"{date} is before the newest release date {state.releases[0].date}")
    if not state.unreleased:
        errors.append(f"{CHANGELOG}: {UNRELEASED} is empty; record the changes first")
    if errors:
        return errors

    heading = f"{UNRELEASED}\n\n## [{version}] - {date.isoformat()}"
    updates = {
        CHANGELOG: UNRELEASED_RE.sub(lambda _: heading, read(root, CHANGELOG), count=1),
        CARGO: set_cargo_version(read(root, CARGO), version),
    }
    for rel in STATUS_FILES:
        updates[rel] = STATUS_RE.sub(lambda m: m[1] + version + m[3], read(root, rel))
    for rel, text in updates.items():
        write(root, rel, text)
    return check_tree(root).errors


def git(root, *args):
    result = subprocess.run(["git", *args], cwd=root, text=True, capture_output=True)
    if result.returncode:
        raise ValueError(f"git {args[0]} failed: {result.stderr.strip()}")
    return result.stdout.rstrip("\n")  # porcelain lines keep their leading status column


def newest_final_tag(root):
    """The newest vX.Y.Z tag on HEAD's history; prerelease tags don't count."""
    names = git(root, "for-each-ref", "--merged", "HEAD", "--format=%(refname:short)", "refs/tags/v*").split()
    finals = [name for name in names if TAG_RE.match(name) and not TAG_RE.match(name)[5]]
    return max(finals, key=lambda name: semver_key(name[1:]), default=None)


def changed_paths(root):
    """Staged, unstaged and untracked paths, as `git status` lists them."""
    entries = git(root, "status", "--porcelain", "-z", "--untracked-files=normal").split("\0")
    paths, skip = [], False
    for entry in entries:
        if skip or not entry:
            skip = False
            continue
        paths.append(entry[3:])
        skip = entry[0] in "RC"  # a rename's or copy's old path follows
    return paths


def run_step(root, command):
    """Runs one step with its output shown; True when it passes."""
    print(f"$ {shlex.join(command)}", flush=True)
    return subprocess.run(command, cwd=root).returncode == 0


def run_preflight(root, version, rehearsal=False):
    """release-preflight.py on VERSION against origin's push destination; its exit code."""
    spec = importlib.util.spec_from_file_location("release_preflight", Path(__file__).with_name("release-preflight.py"))
    pre = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(pre)
    repo = pre.github_repository(git(root, "remote", "get-url", "--push", "origin"))
    return pre.main([version, "--repo", repo, "--root", str(root), *(["--rehearsal"] if rehearsal else [])])


def release_run(root, kind, summary=None, dry_run=False, date=None, step=run_step, preflight=run_preflight):
    """`just release KIND`; returns errors. Refuses before writing anything (D-227)."""
    if kind not in KINDS:
        return [f"release kind {kind!r} isn't switched on: `minor` cuts the next minor and `rehearsal` checks a "
                "rehearsal tag; the redo's 9.8 adds patch, major, release candidates and promote"]
    date = date or datetime.date.today()
    if kind == "rehearsal":
        return rehearse(root, dry_run, date, preflight)
    tag = newest_final_tag(root)
    if tag is None:
        return ["no vX.Y.Z tag on HEAD's history to release after"]
    major, minor = int(TAG_RE.match(tag)[2]), int(TAG_RE.match(tag)[3])
    version, branch = f"{major}.{minor + 1}.0", f"{major}.{minor + 1}"
    subject = f"chore(release): {version} {summary}" if summary else None
    errors = []
    current = git(root, "branch", "--show-current")
    if current != branch:
        errors.append(f"release {version} runs on branch {branch}, not {current or 'a detached HEAD'}")
    if summary is None and not dry_run:
        errors.append("a summary is required: just release minor '<summary>'")
    elif summary is not None and (not summary.strip() or "\n" in summary):
        errors.append("the summary is one line of text")
    elif subject and len(subject) > SUBJECT_LIMIT:
        errors.append(f"the commit subject is {len(subject)} characters, over {SUBJECT_LIMIT}: {subject}")
    state = check_tree(root)
    changed = changed_paths(root)
    stage = None
    if not changed:
        if state.version == tag[1:]:
            stage = "cut"
        elif state.version == version and git(root, "log", "-1", "--format=%s").startswith(f"chore(release): {version} "):
            stage = "commit"
        else:
            errors.append(f"{CARGO} says {state.version}: neither the newest release {tag[1:]} nor a committed {version} cut")
    elif set(changed) <= set(CUT_PATHS) and state.version == version and not state.errors:
        stage = "checks"
    else:
        errors.append(f"the tree is dirty ({', '.join(changed)}); commit or move those changes first")
    if errors:
        return errors

    def cut_step():
        problems = cut(root, version, date)
        for problem in problems:
            print(f"ERROR: {problem}", file=sys.stderr)
        return not problems

    def commit_step():
        paths = changed_paths(root)
        stray = [path for path in paths if path not in CUT_PATHS]
        if stray:
            print(f"ERROR: the checks left changes outside the cut: {', '.join(stray)}", file=sys.stderr)
            return False
        git(root, "add", "--", *[path for path in CUT_PATHS if path in paths])
        git(root, "commit", "-q", "-m", subject)
        print(f"Committed {git(root, 'rev-parse', '--short', 'HEAD')}: {subject}")
        return True

    chain = []
    if stage == "cut":
        chain += [(shlex.join(RATCHET), lambda: step(root, RATCHET)),
                  (f"release.py cut {version}", cut_step),
                  (shlex.join(LOCK), lambda: step(root, LOCK))]
    if stage in ("cut", "checks"):
        chain += [(shlex.join(command), lambda command=command: step(root, command)) for command in CHECKS]
        chain.append((f"git commit -m {shlex.quote(subject or f'chore(release): {version} <summary>')} -- "
                      + " ".join(CUT_PATHS), commit_step))
    chain.append((f"release-preflight.py {version} --repo <origin's repository>", lambda: preflight(root, version) == 0))
    print(f"Release {version} (minor) on {branch}, after {tag}: "
          + {"cut": "cutting.", "checks": "the cut is in place; resuming at the checks.",
             "commit": "the release commit is in place; running the preflight."}[stage])
    if dry_run:
        print("Dry run: nothing runs and nothing is written. The chain:")
        for label, _ in chain:
            print(f"  {label}")
        return []
    for label, action in chain:
        if not action():
            if label.startswith(("just docs-ratchet", "release.py cut")):
                return [f"{label} failed; nothing is committed"]
            if label.startswith("release-preflight"):
                return [f"the preflight failed; rerun `just release minor` to repeat it, or "
                        f"`just release-preflight {version} --repo OWNER/REPO`"]
            return [f"{label} failed; the cut stays uncommitted: fix it and rerun "
                    f"`just release minor {shlex.quote(summary)}`, which resumes at the checks"]
    return []


def rehearse(root, dry_run, date, preflight):
    """A unique v0.0.0-test.YYYYMMDD.gSHA on HEAD, checked, and the preflight's rehearsal block."""
    changed = changed_paths(root)
    if changed:
        return [f"the tree is dirty ({', '.join(changed)}); a rehearsal tags a committed HEAD"]
    stem = f"0.0.0-test.{date:%Y%m%d}.g{git(root, 'rev-parse', '--short=7', 'HEAD')}"
    taken = set(git(root, "for-each-ref", "--format=%(refname:short)", f"refs/tags/v{stem}*").split())
    if not dry_run:
        url = git(root, "remote", "get-url", "--push", "origin")
        taken |= {line.split("refs/tags/")[1].removesuffix("^{}")
                  for line in git(root, "ls-remote", "--tags", "--", url, f"refs/tags/v{stem}*").splitlines()}
    version = next(v for v in [stem] + [f"{stem}.{n}" for n in range(2, 100)] if f"v{v}" not in taken)
    tag = f"v{version}"
    print(f"Rehearsal tag {tag} on HEAD, a prerelease without a changelog section.")
    if dry_run:
        print("Dry run: nothing runs and nothing is written. The chain:")
        print(f"  release.py check {tag}")
        print(f"  release-preflight.py {version} --rehearsal --repo <origin's repository>")
        return []
    state = check_tree(root)
    errors = state.errors + check_tag(state, tag)
    if errors:
        return errors
    print(f"Release consistency: workspace {state.version}; {tag} is a rehearsal, notes from [Unreleased].")
    if preflight(root, version, rehearsal=True):
        return [f"the preflight failed; rerun `just release-preflight {version} --rehearsal --repo OWNER/REPO`"]
    return []


def notes(root, tag, link_base=None):
    match = TAG_RE.match(tag)
    if not match:
        raise ValueError(f"tag {tag!r} is not v<SemVer>")
    state = check_tree(root)
    if is_rehearsal(state, tag):
        body = state.unreleased or "No unreleased changes recorded."
        body = f"Pre-release build of unreleased changes (`{tag}`).\n\n{body}"
    else:
        release = find_release(state, match[1])
        if release is None:
            raise ValueError(f"{CHANGELOG}: no [{match[1]}] section")
        body = release.body
    if link_base:
        base = link_base.rstrip("/")
        body = RELATIVE_LINK_RE.sub(lambda m: f"]({base}/{m[1]})", body)
    return body + "\n"


def members(root, folder):
    """(package name, directory) for each `<folder>/*` workspace member."""
    members = re.search(r"^\s*members\s*=\s*\[(.*?)\]", read(root, CARGO), re.S | re.M)
    if not members:
        raise ValueError(f"{CARGO}: no workspace members list")
    result = []
    for pattern in re.findall(r'"([^"]+)"', re.sub(r"#[^\n]*", "", members[1])):
        if not pattern.startswith(f"{folder}/"):
            continue
        dirs = ([p.relative_to(root).as_posix() for p in sorted(root.glob(pattern)) if (p / CARGO).is_file()]
                if any(c in pattern for c in "*?[") else [pattern])
        for member in dirs:
            _, name = table_value(read(root, f"{member}/{CARGO}"), "package", NAME_RE)
            if name is None:
                raise ValueError(f"{member}/{CARGO}: no [package] name")
            result.append((name[1], member))
    return result


def examples(root):
    """(package name, directory) for each `examples/*` workspace member an
    archive ships: all but the test-only `NOT_SHIPPED` packages (D-193)."""
    result = [member for member in members(root, "examples") if member[0] not in NOT_SHIPPED]
    if not result:
        raise ValueError(f"{CARGO}: no examples/* workspace members")
    return result


def templates(root):
    """(package name, directory) for each `templates/*` workspace member: a
    game's starting point, archived beside the examples and run from its own
    folder (D-186); none is no error."""
    return members(root, "templates")


def folder_games(root):
    """(package name, directory) for each build its launcher runs from its own folder under `DATA`
    (D-186, D-196): every template, then each shipped example whose folder holds its own
    `tungsten.json`."""
    return templates(root) + [(name, folder) for name, folder in examples(root)
                              if (root / folder / "tungsten.json").is_file()]


def title(package):
    """The name players see `package` under (`TITLES`), else the package name."""
    return TITLES.get(package, (package, None))[0]


def readme(tag, target, members, suffix, levels, game=None, templates=()):
    """README.txt for players (D-196); `members` holds (package name, folder) for each shipped
    build and `templates` (package name, folder, game id) for each folder game, which logs under
    its own game id."""
    def wrapped(*sentences):
        return textwrap.fill(" ".join(sentences), 78, break_on_hyphens=False, break_long_words=False)

    def heading(text, rule):
        return f"{text}\n{rule * len(text)}"

    prefix, sep = ("", "\\") if suffix else ("./", "/")
    commands = [(f"{prefix}{title(name)}{suffix}", TITLES.get(name, (name, None))[1]) for name, _ in members]
    width = max(len(command) for command, _ in commands) + 3
    programs = "\n".join(f"  {command:<{width}}{blurb}".rstrip() if blurb else f"  {command}"
                         for command, blurb in commands)
    keep = "Keep the data folder beside them: it holds their builds and the files they read."
    if suffix:
        start = wrapped("Programs made with the Tungsten game engine. Double-click one to start it:")
        needs = wrapped("They need a GPU with current DirectX 12 or Vulkan drivers.", keep)
    else:
        start = wrapped("Programs made with the Tungsten game engine. Start one from a terminal in this folder:")
        needs = wrapped("They need a GPU with a current Vulkan driver, ALSA (libasound2), libudev, libxkbcommon "
                        "and X11 or Wayland libraries. Built on Ubuntu 24.04; much older distributions may lack "
                        "a new enough glibc.", keep)
    # Windows builds open no console (D-119), so only Linux shows the launcher's line.
    logs = ["The programs open no console."] if suffix else []
    if game:
        logs.append(f"Each run writes a log, and a crash a crash report, to %LOCALAPPDATA%\\{game}\\logs."
                    if suffix else
                    f"Each run writes a log, and a crash a crash report, to $XDG_STATE_HOME/{game}/logs, "
                    f"by default ~/.local/state/{game}/logs.")
    for name, _, template_game in templates:
        if template_game:
            folder = f"%LOCALAPPDATA%\\{template_game}\\logs" if suffix else f"~/.local/state/{template_game}/logs"
            logs.append(f"{title(name)}'s logs and crash reports go to {folder}.")
    if game or any(template_game for _, _, template_game in templates):
        logs.append("TUNGSTEN_USER_DIR=<folder> moves them to <folder>/logs.")
    builds = (f"Each program starts the fastest build in {DATA}{sep}bin{sep} that this CPU supports "
              f"({', '.join(levels)}){'' if suffix else ' and names it on the terminal'}; "
              f"TUNGSTEN_CPU_LEVEL={BASELINE} forces the portable one.")
    if suffix:
        builds = "WGPU_BACKEND=dx12 or WGPU_BACKEND=vulkan overrides the automatic choice of graphics API. " + builds
    trouble = [wrapped(*logs)] if logs else []
    parts = [
        heading(f"Tungsten {tag[1:]} examples for {PLATFORMS.get(target, (target, target))[1]}", "="),
        start, programs, needs,
        heading("If a program does not start", "-"), *trouble, wrapped(builds),
        heading("Licenses", "-"),
        wrapped(f"Tungsten is under the MIT license: LICENSE.txt. {NOTICES} lists the third-party "
                "software built into the programs, the Rust standard library among it, with its licenses."),
    ]
    return "\n\n".join(parts) + "\n"


def place(src, dest):
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(src, dest)
    dest.chmod(0o755)  # artifact transfers drop the executable bit


def elf_sections(data):
    """{name: (type, offset, size, addralign)} from a 64-bit little-endian ELF's section headers."""
    if data[:4] != b"\x7fELF" or data[4:6] != b"\x02\x01":
        raise ValueError("not a 64-bit little-endian ELF file")
    try:
        (shoff,) = struct.unpack_from("<Q", data, 0x28)
        shentsize, shnum, shstrndx = struct.unpack_from("<HHH", data, 0x3A)
        headers = [struct.unpack_from("<IIQQQQIIQQ", data, shoff + i * shentsize) for i in range(shnum)]
        names = headers[shstrndx][4]
        sections = {}
        for name, kind, _flags, _addr, offset, size, _link, _info, align, _entsize in headers:
            label = data[names + name:data.index(b"\0", names + name)].decode("ascii", "replace")
            sections[label] = (kind, offset, size, align)
        return sections
    except (struct.error, IndexError, ValueError) as exc:
        raise ValueError(f"malformed ELF section headers: {exc}") from None


def elf_build_id(data):
    """The GNU build-id from an ELF's note sections, or None."""
    for kind, offset, size, align in elf_sections(data).values():
        if kind != SHT_NOTE:
            continue
        notes, step, at = data[offset:offset + size], 8 if align == 8 else 4, 0
        while at + 12 <= len(notes):
            name_size, desc_size, note_type = struct.unpack_from("<III", notes, at)
            desc = at + 12 + -(-name_size // step) * step
            if note_type == NT_GNU_BUILD_ID and notes[at + 12:at + 12 + name_size] == b"GNU\0":
                return notes[desc:desc + desc_size]
            at = desc + -(-desc_size // step) * step
    return None


def has_section(data, name):
    """Whether the ELF has section `name` with contents."""
    section = elf_sections(data).get(name)
    return section is not None and section[0] != SHT_NOBITS and section[2] > 0


def check_split(binary, debug):
    """`debug` belongs to `binary` (equal GNU build-ids) and holds line tables, else ValueError (D-120)."""
    binary_id = elf_build_id(Path(binary).read_bytes())
    if not binary_id:
        raise ValueError(f"{binary}: no GNU build-id; link with -Wl,--build-id")
    data = Path(debug).read_bytes()
    debug_id = elf_build_id(data)
    if debug_id != binary_id:
        found = debug_id.hex() if debug_id else "none"
        raise ValueError(f"{binary}: build-id {binary_id.hex()} differs from {Path(debug).name}'s ({found})")
    if not has_section(data, ".debug_line"):
        raise ValueError(f"{debug}: no .debug_line section; was {Path(binary).name} built stripped?")


def run_tool(command, cwd=None):
    """`command`'s standard output, else ValueError with its standard error."""
    result = subprocess.run(command, capture_output=True, encoding="utf-8", errors="replace", cwd=cwd)
    if result.returncode != 0:
        raise ValueError(f"{' '.join(command)} failed: {result.stderr.strip()}")
    return result.stdout


# rustc's GDB pretty-printer hint: an allocated section that --strip-debug keeps. Removing it
# drops only its header; its bytes stay in the read-only segment, which keeps its layout.
GDB_SCRIPTS = "--remove-section=.debug_gdb_scripts"


def objcopy_split(binary, debug=None):
    """Moves `binary`'s debug info into `debug`, compressed, and links it; with no `debug`, drops it."""
    if debug is None:
        run_tool(["objcopy", "--strip-debug", GDB_SCRIPTS, str(binary)])
        return
    run_tool(["objcopy", "--only-keep-debug", "--compress-debug-sections=zlib", str(binary), str(debug)])
    run_tool(["objcopy", "--strip-debug", GDB_SCRIPTS, f"--add-gnu-debuglink={debug}", str(binary)])
    check_split(binary, debug)


def codeview(data):
    """The CodeView record that a PE file's debug directory names, or None."""
    try:
        if data[:2] != b"MZ":
            return None
        (pe,) = struct.unpack_from("<I", data, 0x3C)
        if data[pe:pe + 4] != b"PE\0\0":
            return None
        section_count, optional_size = struct.unpack_from("<H", data, pe + 6)[0], struct.unpack_from("<H", data, pe + 20)[0]
        optional = pe + 24
        directories = optional + {0x10B: 96, 0x20B: 112}[struct.unpack_from("<H", data, optional)[0]]
        if struct.unpack_from("<I", data, directories - 4)[0] <= 6:
            return None
        rva, size = struct.unpack_from("<II", data, directories + 8 * 6)
        table = optional + optional_size
        for index in range(section_count):
            virtual_size, address, raw_size, raw_offset = struct.unpack_from("<IIII", data, table + 40 * index + 8)
            if address <= rva < address + max(virtual_size, raw_size):
                start = rva - address + raw_offset
                break
        else:
            return None
        for index in range(size // 28):
            kind, record_size, _address, pointer = struct.unpack_from("<IIII", data, start + 28 * index + 12)
            if kind == 2 and data[pointer:pointer + 4] == b"RSDS":
                (age,) = struct.unpack_from("<I", data, pointer + 20)
                name = data[pointer + 24:pointer + record_size].split(b"\0", 1)[0].decode("utf-8", "replace")
                return CodeView(data[pointer + 4:pointer + 20], age, name)
        return None
    except (struct.error, KeyError):
        return None


def pdb_info(data):
    """(GUID bytes, age) from a PDB's info stream, stream 1 of its MSF 7.0 container."""
    if data[:len(MSF_MAGIC)] != MSF_MAGIC:
        raise ValueError("not an MSF 7.0 PDB file")
    try:
        block_size, _fpm, _blocks, directory_size, _unknown, block_map = struct.unpack_from("<6I", data, 32)

        def stream(blocks, size):
            return b"".join(data[n * block_size:(n + 1) * block_size] for n in blocks)[:size]

        count = -(-directory_size // block_size)
        directory = stream(struct.unpack_from(f"<{count}I", data, block_map * block_size), directory_size)
        (streams,) = struct.unpack_from("<I", directory, 0)
        sizes = [0 if size == 0xFFFFFFFF else size for size in struct.unpack_from(f"<{streams}I", directory, 4)]
        at = 4 + 4 * streams
        for index, size in enumerate(sizes):
            count = -(-size // block_size)
            blocks = struct.unpack_from(f"<{count}I", directory, at)
            at += 4 * count
            if index == 1:
                info = stream(blocks, size)
                return info[12:28], struct.unpack_from("<I", info, 8)[0]
    except struct.error as exc:
        raise ValueError(f"malformed PDB file: {exc}") from None
    raise ValueError("PDB file has no info stream")


def matching_pdb(exe, search):
    """The PDB in `search` that `exe`'s CodeView record names, checked against its GUID and age (D-120)."""
    record = codeview(Path(exe).read_bytes())
    if record is None:
        raise ValueError(f"{exe}: no CodeView record naming a PDB")
    name = re.split(r"[\\/]", record.name)[-1]
    pdb = Path(search) / name
    if not pdb.is_file():
        raise ValueError(f"{exe}: names {name}, which {search} lacks")
    if pdb_info(pdb.read_bytes()) != (record.guid, record.age):
        raise ValueError(f"{pdb}: GUID and age differ from the CodeView record in {exe}")
    return pdb, name


def game_id(root, folder="."):
    """`game.id` from `folder`'s tungsten.json, the root's by default (D-119), or None."""
    try:
        value = json.loads(read(root, f"{folder}/tungsten.json")).get("game", {}).get("id")
    except (OSError, ValueError, AttributeError):
        return None
    return value if isinstance(value, str) else None


def read_any(path):
    """A text file as UTF-8, else Latin-1, with a byte-order mark dropped and line endings as LF."""
    data = Path(path).read_bytes()
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        text = data.decode("latin-1")
    return text.removeprefix("﻿").replace("\r\n", "\n").replace("\r", "\n")


def write_text(path, text):
    with Path(path).open("w", encoding="utf-8", newline="\n") as out:
        out.write(text)


def shipped_packages(root):
    """The packages whose binaries an archive ships: the examples, the templates
    and the launcher."""
    return [name for name, _ in examples(root) + templates(root)] + [LAUNCHER]


def crate_record(package):
    """A crate's license data: cargo metadata's fields, the text of its root-level license and
    notice files and of its `license_file`, and the font files anywhere in its package."""
    label = f"{package['name']} {package['version']}"
    folder = Path(package["manifest_path"]).parent
    if not folder.is_dir():
        raise ValueError(f"{label}: no source at {folder}; run cargo fetch --locked")
    names = sorted(path.name for path in folder.iterdir() if path.is_file() and LICENSE_FILE_RE.match(path.name))
    if package.get("license_file"):
        names.append(Path(package["license_file"]).as_posix())
    files = {}
    for name in dict.fromkeys(names):
        if not (folder / name).is_file():
            raise ValueError(f"{label}: its license_file {name} is missing from {folder}")
        files[name] = read_any(folder / name)
    fonts = sorted(path.relative_to(folder).as_posix() for path in folder.rglob("*")
                   if path.suffix.lower() in FONT_SUFFIXES and path.is_file())
    return {"name": package["name"], "version": package["version"], "license": package.get("license"),
            "license_file": package.get("license_file"), "repository": package.get("repository"),
            "files": files, "fonts": fonts}


def collect_licenses(root, target, run=run_tool):
    """License data for `target` (D-122): the crates that cargo tree's normal edges reach from the
    shipped packages, with cargo metadata's details and their files, and the standard library's
    notices from the toolchain. `run(command, cwd)` returns a command's output; tests fake it."""
    if not TARGET_RE.match(target):
        raise ValueError(f"target {target!r} is not a target triple")
    run(["cargo", "fetch", "--locked", "--target", target], cwd=root)
    # cargo tree resolves features as the build does; cargo metadata's resolve also keeps edges
    # that the build never activates, so it supplies package details only.
    roots = [arg for name in shipped_packages(root) for arg in ("-p", name)]
    tree = run(["cargo", "tree", "--locked", "-e", "normal", "--target", target, "--prefix", "none",
                "-f", "{p}", *roots], cwd=root)
    metadata = json.loads(run(["cargo", "metadata", "--locked", "--format-version", "1",
                               "--filter-platform", target], cwd=root))
    members = set(metadata["workspace_members"])
    packages = {}
    for package in metadata["packages"]:
        packages.setdefault((package["name"], package["version"]), []).append(package)
    crates = {}
    for line in tree.splitlines():
        match = TREE_LINE_RE.match(line)
        if not match or match.groups() in crates:
            continue
        found = packages.get(match.groups(), [])
        if len(found) != 1:
            raise ValueError(f"cargo metadata lists {len(found)} packages for {match[1]} v{match[2]}")
        if found[0]["id"] not in members:
            crates[match.groups()] = crate_record(found[0])
    std = Path(run(["rustc", "--print", "sysroot"], cwd=root).strip()) / STD_NOTICES_SOURCE
    if not std.is_file():
        raise ValueError(f"{std}: missing; the toolchain's rustc component ships the standard library's notices")
    return {"target": target, "rustc": run(["rustc", "--version"], cwd=root).strip(),
            "rust_std_notices": std.read_text(encoding="utf-8"),
            "crates": [crates[key] for key in sorted(crates)]}


def alternatives(expression):
    """An SPDX-style expression's alternatives, each a tuple of licenses: `/` reads as OR, AND binds
    tighter than OR, and WITH stays with its license."""
    tokens = re.findall(r"[()/]|[^\s()/]+", expression)
    at = 0

    def malformed():
        return ValueError(f"license expression {expression!r} is malformed")

    def peek():
        return tokens[at].upper() if at < len(tokens) else None

    def take():
        nonlocal at
        if at == len(tokens):
            raise malformed()
        at += 1
        return tokens[at - 1]

    def any_of():
        result = all_of()
        while peek() in ("OR", "/"):
            take()
            result += [alternative for alternative in all_of() if alternative not in result]
        return result

    def all_of():
        result = one()
        while peek() == "AND":
            take()
            right = one()
            result = [tuple(dict.fromkeys(left + other)) for left in result for other in right]
        return result

    def one():
        token = take()
        if token == "(":
            result = any_of()
            if take() != ")":
                raise malformed()
            return result
        if token in (")", "/") or token.upper() in ("AND", "OR", "WITH"):
            raise malformed()
        if peek() == "WITH":
            take()
            token = f"{token} WITH {take()}"
        return [(token,)]

    result = any_of()
    if at != len(tokens):
        raise malformed()
    return result


def matches(license, text):
    """Whether `text` holds `license`'s marker phrases and none of its look-alikes'."""
    required, absent = MARKERS[license]
    flat = " ".join(text.lower().split())
    return all(phrase in flat for phrase in required) and not any(phrase in flat for phrase in absent)


def choose(expression, files):
    """(alternative, [(license, file name)]): the first of the expression's alternatives, in
    PREFERENCE order, whose licenses each match one of `files`; None when none does."""
    known = [alternative for alternative in alternatives(expression) if all(lic in MARKERS for lic in alternative)]
    for alternative in sorted(known, key=lambda alternative: sorted(PREFERENCE.index(lic) for lic in alternative)):
        picks = []
        for lic in alternative:
            found = [name for name in sorted(files) if not NOTICE_FILE_RE.match(name) and matches(lic, files[name])]
            if not found:
                break
            # The most specific file: the one that matches the fewest licenses, then the shortest name.
            picks.append((lic, min(found, key=lambda name: (sum(matches(other, files[name]) for other in MARKERS),
                                                           len(name), name))))
        else:
            return alternative, picks
    return None


def clarifications(root):
    """{crate name: [entries]} from licenses/clarifications.json; none when the file is absent."""
    path = root / CLARIFICATIONS
    entries = {}
    if path.is_file():
        for entry in json.loads(path.read_text(encoding="utf-8"))["crates"]:
            entries.setdefault(entry["name"], []).append(entry)
    return entries


def clarified_text(root, item, errors, label):
    """The text of a clarification's `file` under licenses/, or None after recording why not."""
    path = (root / CLARIFICATIONS).parent / item["file"]
    if ".." in Path(item["file"]).parts or not path.is_file():
        errors.append(f"{label}: {CLARIFICATIONS} names {item['file']}, which licenses/ lacks")
        return None
    return read_any(path)


class _PlainText(html.parser.HTMLParser):
    """The text of the toolchain's COPYRIGHT-library.html: each block on its own lines, `<pre>`
    verbatim, nested `<div>` boxes indented, an outside link's address after its text."""

    BLOCKS = {"p", "div", "ul", "li", "details", "summary", "pre", "br", "h1", "h2", "h3", "h4"}
    GAPS = {"p", "div", "ul", "details", "pre", "h1", "h2", "h3", "h4"}
    RULES = {"h1": "=", "h2": "-"}
    SKIP = {"head", "script", "style"}

    def __init__(self):
        super().__init__()
        self.lines, self.line, self.pre, self.links = [], [], None, []
        self.depth = self.skip = 0

    def indent(self):
        return "    " * max(self.depth - 1, 0)

    def gap(self):
        if self.lines and self.lines[-1]:
            self.lines.append("")

    def flush(self):
        text = re.sub(r" +", " ", "".join(self.line)).strip()
        self.line = []
        if text:
            indent = self.indent()
            self.lines += textwrap.wrap(text, 78, initial_indent=indent,
                                        subsequent_indent=indent + ("  " if text.startswith("- ") else ""),
                                        break_on_hyphens=False, break_long_words=False)
        return text

    def handle_starttag(self, tag, attrs):
        if tag in self.SKIP:
            self.skip += 1
        if self.skip or self.pre is not None:
            return
        if tag in self.BLOCKS:
            self.flush()
        if tag in self.GAPS:
            self.gap()
        if tag == "div":
            self.depth += 1
        elif tag == "li":
            self.line.append("- ")
        elif tag == "pre":
            self.pre = []
        elif tag == "a":
            self.links.append((dict(attrs).get("href") or "", len(self.line)))

    def handle_endtag(self, tag):
        if tag in self.SKIP:
            self.skip = max(self.skip - 1, 0)
            return
        if self.skip:
            return
        if tag == "pre" and self.pre is not None:
            lines = [line.rstrip() for line in "".join(self.pre).split("\n")]
            while lines and not lines[0]:
                lines.pop(0)
            while lines and not lines[-1]:
                lines.pop()
            self.lines += [self.indent() + line if line else "" for line in lines]
            self.pre = None
            self.gap()
            return
        if self.pre is not None:
            return
        if tag == "a" and self.links:
            href, start = self.links.pop()
            text = "".join(self.line[start:]).strip()
            if href.startswith(("http://", "https://")) and href != text:
                self.line.append(f" <{href}>")
            return
        if tag in self.BLOCKS:
            text = self.flush()
            if text and tag in self.RULES:
                self.lines.append(self.indent() + self.RULES[tag] * len(text))
        if tag == "div":
            self.depth = max(self.depth - 1, 0)
        if tag in self.GAPS:
            self.gap()

    def handle_data(self, data):
        if self.skip:
            return
        if self.pre is not None:
            self.pre.append(data)
        else:
            self.line.append(re.sub(r"\s+", " ", data))


def plain_text(source):
    """`source`, an HTML page such as the standard library's notices, as plain text (D-196)."""
    parser = _PlainText()
    parser.feed(source)
    parser.close()
    parser.flush()
    return "\n".join(parser.lines).strip() + "\n"


def third_party_notices(root, data):
    """THIRD-PARTY-NOTICES.txt from a build job's license data (D-122), the standard library's
    notices last as plain text (D-196). ValueError names every crate without an entry, every stale
    clarification and unreviewed font, and missing files."""
    clarified, errors, unmatched, index, texts = clarifications(root), [], [], [], {}

    def cite(text, kind, user):
        lines = [line.rstrip() for line in text.splitlines()]
        while lines and not lines[0]:
            lines.pop(0)
        while lines and not lines[-1]:
            lines.pop()
        entry = texts.setdefault("\n".join(lines), {"number": len(texts) + 1, "kinds": [], "users": []})
        for key, value in (("kinds", kind), ("users", user)):
            if value not in entry[key]:
                entry[key].append(value)
        return f"[{entry['number']}]"

    for crate in data["crates"]:
        label, expression, files = f"{crate['name']} {crate['version']}", crate.get("license") or "", crate["files"]
        entries = clarified.get(crate["name"], [])
        entry = next((item for item in entries if item.get("version") == crate["version"]), None)
        if entries and entry is None:
            checked = ", ".join(sorted(str(item.get("version")) for item in entries))
            errors.append(f"{label}: {CLARIFICATIONS} checked it at {checked}; check upstream again and update the entry")
            continue
        entry = entry or {}
        lines = [label, f"    License: {expression or 'none declared'}"]
        if entry.get("texts"):
            used = tuple(item["license"] for item in entry["texts"])
            if expression and used not in alternatives(expression):
                errors.append(f"{label}: {CLARIFICATIONS} names {' AND '.join(used)}, not an alternative of {expression!r}")
                continue
            cited = []
            for item in entry["texts"]:
                text = clarified_text(root, item, errors, label)
                if text is not None and item["license"] in MARKERS and not matches(item["license"], text):
                    errors.append(f"{label}: {item['file']} does not read as {item['license']}")
                if text is not None:
                    cited.append(cite(text, item["license"], label))
            sources = ", ".join(item["source"] for item in entry["texts"])
            lines.append(f"    Used under: {' AND '.join(used)}: {', '.join(cited)} (from {sources})")
        else:
            choice = choose(expression, files) if expression else None
            if choice is None:
                unmatched.append(f"{label} ({expression or 'no license field'}; files: {', '.join(sorted(files)) or 'none'})")
                continue
            alternative, picks = choice
            lines.append(f"    Used under: {' AND '.join(alternative)}: "
                         f"{', '.join(cite(files[name], lic, label) for lic, name in picks)}")
        notice_files = [name for name in sorted(files) if NOTICE_FILE_RE.match(name)]
        if notice_files:
            lines.append(f"    Notices: {', '.join(cite(files[name], 'NOTICE', label) for name in notice_files)}")
        for work in entry.get("bundled", []):
            text = clarified_text(root, work, errors, label)
            if text is not None:
                lines.append(f"    Bundles {work['work']}, under {work['license']}: "
                             f"{cite(text, work['license'] + ', ' + work['work'], label)} (from {work['source']})")
        if crate.get("fonts") and not (entry.get("bundled") or entry.get("fonts_reviewed")):
            errors.append(f"{label}: carries font files ({', '.join(crate['fonts'])}) with neither a bundled "
                          f"entry nor a fonts_reviewed note in {CLARIFICATIONS}")
        lines.append(f"    https://crates.io/crates/{crate['name']}/{crate['version']}")
        if crate.get("repository"):
            lines.append(f"    Repository: {crate['repository']}")
        index.append("\n".join(lines))
    works = []
    for title, rel in EMBEDDED_WORKS:
        if (root / rel).is_file():
            works.append(f"{title}\n{'-' * 78}\n{read_any(root / rel).strip()}\n")
        else:
            errors.append(f"{rel}: missing; EMBEDDED_WORKS in scripts/release.py names it for {title}")
    if unmatched:
        errors.insert(0, f"no license text matches an alternative of {len(unmatched)} crate(s); add each to "
                         f"{CLARIFICATIONS} (docs/releases.md): " + "; ".join(unmatched))
    if errors:
        raise ValueError("\n  ".join(["third-party notices:"] + errors))

    def wrapped(text):
        return textwrap.fill(text, 78, break_on_hyphens=False, break_long_words=False)

    intro = (f"The Tungsten examples' own code is under the MIT license: LICENSE.txt. The fonts under "
             f"data/assets/fonts/ have their licenses beside them. This file lists the third-party works "
             f"compiled into the engine, then the {len(index)} crates the binaries link, each with the license "
             f"it is used under and the numbers of its texts, then each text once with the crates that use it. "
             f"Every binary also links the Rust standard library, built by {data['rustc']}; its notices close "
             f"the file.")
    parts = [f"Third-party notices: Tungsten examples for {data['target']}", RULE, "", wrapped(intro), "",
             "", "Works compiled into the engine", RULE, "", *works, "", "Crates", RULE, ""]
    parts += [item + "\n" for item in index]
    parts += ["", "License texts", RULE, ""]
    for text, entry in texts.items():
        # No-break spaces keep each crate's name and version on one line.
        users = ", ".join(user.replace(" ", " ") for user in entry["users"])
        parts += [f"[{entry['number']}] {'; '.join(entry['kinds'])}",
                  wrapped("Used by " + users).replace(" ", " "), "-" * 78, text, ""]
    parts += ["", f"The Rust standard library ({data['rustc']})", RULE, "", plain_text(data["rust_std_notices"])]
    return "\n".join(parts)


def load_license_data(path, target):
    """A build job's licenses.json for `target`, else ValueError."""
    if not Path(path).is_file():
        raise ValueError(f"{path}: missing; the build job's `release.py licenses` step writes it (D-122)")
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    if target is not None and data.get("target") != target:
        raise ValueError(f"{path}: license data for {data.get('target')}, not {target}")
    return data


def write_notices(root, data, folder):
    """Writes THIRD-PARTY-NOTICES.txt into `folder` and returns its path."""
    text = third_party_notices(root, data)
    Path(folder).mkdir(parents=True, exist_ok=True)
    write_text(Path(folder) / NOTICES, text)
    return Path(folder) / NOTICES


def strip_debug(binary):
    """Drops a Linux binary's debug info in place: `objcopy_split` with no debug file."""
    objcopy_split(binary, None)


def package(root, tag, target, out, bin_dir=None, strip=strip_debug):
    """Zips the players' archive for `target` (D-196) and returns its path. `strip(binary)` drops a
    Linux binary's debug info; tests fake it."""
    if not TAG_RE.match(tag):
        raise ValueError(f"tag {tag!r} is not v<SemVer>")
    if not TARGET_RE.match(target):
        raise ValueError(f"target {target!r} is not a target triple")
    suffix = ".exe" if "windows" in target else ""
    bin_dir = bin_dir or root / "target"

    def release_dir(level):
        return bin_dir / level / target / "release"

    def built(level, binary):
        return release_dir(level) / (binary + suffix)

    levels = [level for level in LEVELS if release_dir(level).is_dir()]
    if BASELINE not in levels:
        raise ValueError(f"{bin_dir}: no {BASELINE}/{target}/release build; the launcher needs the portable fallback")
    starts = templates(root)
    members = examples(root) + starts
    games = folder_games(root)
    in_folder = {d for _, d in games}
    needed = [(level, binary) for level in levels for binary, _ in members] + [(BASELINE, LAUNCHER)]
    missing = [f"{level}/{binary}{suffix}" for level, binary in needed if not built(level, binary).is_file()]
    if missing:
        raise ValueError(f"{bin_dir}: missing {', '.join(missing)}")
    licenses = load_license_data(bin_dir / LICENSE_DATA, target)
    notices = third_party_notices(root, licenses)
    # The examples read the shared root files and their own assets; a folder game, its folder's.
    runtime = list(SHARED_RUNTIME) + [f"{d}/assets" for _, d in examples(root)
                                      if d not in in_folder and (root / d / "assets").is_dir()]
    runtime += [f"{d}/{rel}" for _, d in games for rel in FOLDER_RUNTIME if (root / d / rel).exists()]
    name = f"{ARCHIVE_PREFIX}-{tag[1:]}-{PLATFORMS.get(target, (target, target))[0]}"
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        stage = Path(tmp) / name
        data = stage / DATA
        stage.mkdir(parents=True)

        def ship(src, dest):
            place(src, dest)
            # Windows builds keep their debug info in PDBs, which no archive ships.
            if not suffix:
                strip(dest)
                dest.chmod(0o755)

        # Each launcher runs the build of its own file name from data/bin/<level>/.
        for binary, _ in members:
            shown = title(binary) + suffix
            ship(built(BASELINE, LAUNCHER), stage / shown)
            for level in levels:
                ship(built(level, binary), data / "bin" / level / shown)
        for rel in runtime:
            src, dest = root / rel, data / rel
            if src.is_dir():
                shutil.copytree(src, dest)
            else:
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(src, dest)
        shutil.copyfile(root / "LICENSE", stage / "LICENSE.txt")
        logs = [(binary, d, game_id(root, d)) for binary, d in games]
        write_text(stage / "README.txt", readme(tag, target, members, suffix, levels, game_id(root), logs))
        write_text(stage / NOTICES, notices)
        archive = shutil.make_archive(str(out / name), "zip", root_dir=tmp, base_dir=name)
    return Path(archive)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("check", help="version, changelog and optional tag consistency")
    command.add_argument("tag", nargs="?")
    commands.add_parser("version", help="print the workspace version of a consistent tree")
    command = commands.add_parser("cut", help="move [Unreleased] to VERSION and bump the version")
    command.add_argument("version")
    command.add_argument("--date", type=datetime.date.fromisoformat, default=datetime.date.today())
    command = commands.add_parser("run", help="`just release KIND`: minor cuts, checks and commits; rehearsal checks a tag")
    command.add_argument("kind", help="minor or rehearsal; the redo's 9.8 adds the rest")
    command.add_argument("summary", nargs="?", help="the release commit's summary, after `chore(release): X.Y.Z`")
    command.add_argument("--dry-run", action="store_true", help="print the version and the chain; write nothing")
    command.add_argument("--date", type=datetime.date.fromisoformat, default=datetime.date.today())
    command = commands.add_parser("notes", help="print release notes for TAG")
    command.add_argument("tag")
    command.add_argument("--link-base", help="prefix for relative links, e.g. https://github.com/o/r/blob/TAG")
    command = commands.add_parser("package", help="zip the example and template builds with their files for players")
    command.add_argument("tag")
    command.add_argument("target")
    command.add_argument("--bin-dir", type=Path, help="per-level cargo target dirs (default: target)")
    command.add_argument("--out", type=Path, default=Path("dist"))
    command = commands.add_parser("split-debug", help="move a Linux binary's debug info to a linked file")
    command.add_argument("binary", type=Path)
    command.add_argument("debug", type=Path)
    command = commands.add_parser("licenses", help="collect the shipped crates' license data for TARGET")
    command.add_argument("target")
    command.add_argument("--out", type=Path, default=Path("target") / LICENSE_DATA)
    command = commands.add_parser("notices", help="write THIRD-PARTY-NOTICES.txt from license data")
    command.add_argument("data", type=Path)
    command.add_argument("--out", type=Path, default=Path("target/notices"))
    args = parser.parse_args(argv)
    root = args.root.absolute()

    try:
        if args.command == "check":
            state = check_tree(root)
            errors = state.errors + (check_tag(state, args.tag) if args.tag else [])
            if not errors:
                latest = state.releases[0]
                print(f"Release consistency: workspace {state.version} = {CHANGELOG} [{latest.version}] - "
                      f"{latest.date}; status consistent in {', '.join(STATUS_FILES)}; [Unreleased] "
                      f"{'has entries' if state.unreleased else 'is empty'}.")
                if args.tag and is_rehearsal(state, args.tag):
                    print(f"Tag {args.tag}: rehearsal (pre-release without a section); version not "
                          "compared, notes from [Unreleased].")
                elif args.tag:
                    print(f"Tag {args.tag}: release; notes from [{state.version}].")
        elif args.command == "version":
            state = check_tree(root)
            errors = state.errors
            if not errors:
                print(state.version)
        elif args.command == "cut":
            errors = cut(root, args.version, args.date)
            if not errors:
                print(f"Cut [{args.version}] - {args.date}: {CHANGELOG}, {CARGO} and "
                      f"{', '.join(STATUS_FILES)} updated. Next: refresh Cargo.lock "
                      f"(`just release minor` does), review status prose, run the checks (docs/releases.md).")
        elif args.command == "run":
            errors = release_run(root, args.kind, args.summary, args.dry_run, args.date)
        elif args.command == "notes":
            sys.stdout.write(notes(root, args.tag, args.link_base))
            errors = []
        elif args.command == "split-debug":
            objcopy_split(args.binary, args.debug)
            print(f"{args.binary}: debug info moved to {args.debug}, build-ids equal")
            errors = []
        elif args.command == "licenses":
            data = collect_licenses(root, args.target)
            args.out.parent.mkdir(parents=True, exist_ok=True)
            write_text(args.out, json.dumps(data, indent=1) + "\n")
            print(f"{args.target}: license data of {len(data['crates'])} crates and {data['rustc']}'s "
                  f"standard library in {args.out}")
            errors = []
        elif args.command == "notices":
            data = load_license_data(args.data, None)
            path = write_notices(root, data, args.out)
            print(f"{data['target']}: notices of {len(data['crates'])} crates, {len(EMBEDDED_WORKS)} "
                  f"embedded work(s) and the standard library in {path}")
            errors = []
        else:
            print(package(root, args.tag, args.target, args.out, args.bin_dir))
            errors = []
    except (OSError, ValueError) as exc:
        errors = [str(exc)]
    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
