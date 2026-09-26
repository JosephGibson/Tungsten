#!/usr/bin/env python3
"""Release version consistency, changelog cut, notes and packaging (stdlib only).

  check [TAG]         Cargo.toml's workspace version is the newest CHANGELOG.md
                      release; `## [Unreleased]` comes first; release headings read
                      `## [X.Y.Z] - YYYY-MM-DD`, newest version and date first; and
                      the README.md/DESIGN.md "Workspace `vX.Y.Z`" status lines
                      agree. A tag whose version has a CHANGELOG section must equal
                      the workspace version and have notes; so must every vX.Y.Z tag.
                      A pre-release tag without a section (e.g. v0.0.0-test) is a
                      rehearsal: the tree must still agree, the tag isn't compared.
  cut VERSION         Moves the [Unreleased] body under `## [VERSION] - DATE`, leaves
                      an empty [Unreleased], and sets the workspace version and status
                      lines. `just release-cut` also refreshes Cargo.lock.
  notes TAG           Prints the tag's CHANGELOG section ([Unreleased] for a
                      rehearsal tag), optionally with absolute repository links.
  package TAG TARGET  Archives each CPU level's example builds (BIN_DIR/<level>/TARGET/
                      release, one cargo target dir per level) under bin/<level>/, one
                      launcher per example, and the files the examples read relative
                      to the working directory: OUT/tungsten-examples-TAG-TARGET.*

Rationale: D-071, D-072. Release steps: README.md "Releases".
"""

import argparse
import datetime
import re
import shutil
import sys
import tempfile
from collections import namedtuple
from pathlib import Path

CARGO = "Cargo.toml"
CHANGELOG = "CHANGELOG.md"
# Human status lines that name the workspace version; `cut` rewrites them.
STATUS_FILES = ("README.md", "DESIGN.md")
STATUS_RE = re.compile(r"(Workspace `v)([^`]*)(`)")
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
# What every example reads relative to the working directory, plus the license.
SHARED_RUNTIME = ("tungsten.json", "input.json", "assets", "LICENSE")
ARCHIVE_PREFIX = "tungsten-examples"
# CPU levels release archives may carry, fastest first; must match `LEVELS` in
# tools/launcher/src/main.rs. The portable baseline is mandatory (D-072).
LEVELS = ("x86-64-v4", "x86-64-v3", "x86-64-v2", "x86-64")
BASELINE = "x86-64"
LAUNCHER = "tungsten-launcher"

Release = namedtuple("Release", "version date body")
State = namedtuple("State", "errors version releases unreleased")


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


def check_tree(root):
    errors = []
    version = cargo_version(read(root, CARGO))
    if version is None:
        errors.append(f"{CARGO}: no version in [workspace.package]")
    elif not SEMVER_RE.match(version):
        errors.append(f"{CARGO}: workspace version {version!r} is not SemVer X.Y.Z[-pre]")

    sections = changelog_sections(read(root, CHANGELOG))
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
                      f"[{releases[0].version}]; bump both with `just release-cut`")

    for rel in STATUS_FILES:
        found = [match[2] for match in STATUS_RE.finditer(read(root, rel))]
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


def examples(root):
    """(package name, directory) for each `examples/*` workspace member."""
    members = re.search(r"^\s*members\s*=\s*\[(.*?)\]", read(root, CARGO), re.S | re.M)
    if not members:
        raise ValueError(f"{CARGO}: no workspace members list")
    result = []
    for pattern in re.findall(r'"([^"]+)"', re.sub(r"#[^\n]*", "", members[1])):
        if not pattern.startswith("examples/"):
            continue
        dirs = ([p.relative_to(root).as_posix() for p in sorted(root.glob(pattern)) if (p / CARGO).is_file()]
                if any(c in pattern for c in "*?[") else [pattern])
        for member in dirs:
            _, name = table_value(read(root, f"{member}/{CARGO}"), "package", NAME_RE)
            if name is None:
                raise ValueError(f"{member}/{CARGO}: no [package] name")
            result.append((name[1], member))
    if not result:
        raise ValueError(f"{CARGO}: no examples/* workspace members")
    return result


def readme(tag, target, members, suffix, levels):
    prefix = ".\\" if suffix else "./"  # PowerShell also needs .\ for the current folder
    run = "\n".join(f"  {prefix}{name}{suffix}" for name, _ in members)
    platform = (
        "Double-clicking an .exe in Explorer also works. Needs a GPU with current DX12\n"
        "or Vulkan drivers; WGPU_BACKEND=dx12 or WGPU_BACKEND=vulkan overrides the\n"
        "automatic choice."
        if suffix else
        "Needs a GPU with a current Vulkan driver, ALSA (libasound2), libxkbcommon and\n"
        "X11 or Wayland libraries. Built on Ubuntu 24.04; much older distributions may\n"
        "lack a new enough glibc."
    )
    return (
        f"Tungsten {tag} examples for {target}\n\n"
        "Run an example from any folder:\n\n"
        f"{run}\n\n"
        "Each is a small launcher. It picks the fastest build in bin/ that this CPU\n"
        f"supports ({', '.join(levels)}), names it on the console, and runs it from\n"
        "this folder, where the examples read tungsten.json, input.json and assets/.\n"
        f"TUNGSTEN_CPU_LEVEL={BASELINE} forces the portable build.\n\n"
        f"{platform}\n\n"
        "MIT license: LICENSE.\n"
    )


def place(src, dest):
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(src, dest)
    dest.chmod(0o755)  # artifact transfers drop the executable bit


def package(root, tag, target, out, bin_dir=None):
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
    members = examples(root)
    needed = [(level, binary) for level in levels for binary, _ in members] + [(BASELINE, LAUNCHER)]
    missing = [f"{level}/{binary}{suffix}" for level, binary in needed if not built(level, binary).is_file()]
    if missing:
        raise ValueError(f"{bin_dir}: missing {', '.join(missing)}")
    runtime = list(SHARED_RUNTIME) + [f"{d}/assets" for _, d in members if (root / d / "assets").is_dir()]
    name = f"{ARCHIVE_PREFIX}-{tag}-{target}"
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        stage = Path(tmp) / name
        stage.mkdir()
        for binary, _ in members:
            place(built(BASELINE, LAUNCHER), stage / (binary + suffix))
            for level in levels:
                place(built(level, binary), stage / "bin" / level / (binary + suffix))
        for rel in runtime:
            src, dest = root / rel, stage / rel
            if src.is_dir():
                shutil.copytree(src, dest)
            else:
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(src, dest)
        (stage / "README.txt").write_text(readme(tag, target, members, suffix, levels), encoding="utf-8")
        archive = shutil.make_archive(str(out / name), "zip" if suffix else "gztar", root_dir=tmp, base_dir=name)
    return Path(archive)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("check", help="version, changelog and optional tag consistency")
    command.add_argument("tag", nargs="?")
    command = commands.add_parser("cut", help="move [Unreleased] to VERSION and bump the version")
    command.add_argument("version")
    command.add_argument("--date", type=datetime.date.fromisoformat, default=datetime.date.today())
    command = commands.add_parser("notes", help="print release notes for TAG")
    command.add_argument("tag")
    command.add_argument("--link-base", help="prefix for relative links, e.g. https://github.com/o/r/blob/TAG")
    command = commands.add_parser("package", help="archive example binaries with runtime files")
    command.add_argument("tag")
    command.add_argument("target")
    command.add_argument("--bin-dir", type=Path, help="per-level cargo target dirs (default: target)")
    command.add_argument("--out", type=Path, default=Path("dist"))
    args = parser.parse_args(argv)
    root = args.root.absolute()

    try:
        if args.command == "check":
            state = check_tree(root)
            errors = state.errors + (check_tag(state, args.tag) if args.tag else [])
            if not errors:
                latest = state.releases[0]
                print(f"Release consistency: workspace {state.version} = {CHANGELOG} [{latest.version}] - "
                      f"{latest.date}; {' and '.join(STATUS_FILES)} agree; [Unreleased] "
                      f"{'has entries' if state.unreleased else 'is empty'}.")
                if args.tag and is_rehearsal(state, args.tag):
                    print(f"Tag {args.tag}: rehearsal (pre-release without a section); version not "
                          "compared, notes from [Unreleased].")
                elif args.tag:
                    print(f"Tag {args.tag}: release; notes from [{state.version}].")
        elif args.command == "cut":
            errors = cut(root, args.version, args.date)
            if not errors:
                print(f"Cut [{args.version}] - {args.date}: {CHANGELOG}, {CARGO} and "
                      f"{', '.join(STATUS_FILES)} updated. Next: refresh Cargo.lock "
                      f"(`just release-cut` does), review status prose, commit, tag v{args.version}.")
        elif args.command == "notes":
            sys.stdout.write(notes(root, args.tag, args.link_base))
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
