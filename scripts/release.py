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
                      lines. `just release-cut` also refreshes Cargo.lock.
  notes TAG           Prints the tag's CHANGELOG section ([Unreleased] for a
                      rehearsal tag), optionally with absolute repository links.
  package TAG TARGET  Archives each CPU level's example builds (BIN_DIR/<level>/TARGET/
                      release, one cargo target dir per level) under bin/<level>/, one
                      launcher per example, and the files the examples read relative
                      to the working directory: OUT/tungsten-examples-TAG-TARGET.*.
                      Their debug files go to OUT/tungsten-debug-TAG-TARGET.* under the
                      same top folder: each Linux binary is split with objcopy, its
                      build-id checked, and each Windows example's PDB is matched to
                      its CodeView record.
  split-debug BINARY DEBUG
                      The Linux split alone: moves BINARY's debug info to DEBUG, links
                      it, and checks the build-ids and DEBUG's .debug_line.

Rationale: D-071, D-072, D-074, D-079, D-120. Release steps: docs/releases.md.
"""

import argparse
import datetime
import json
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from collections import namedtuple
from pathlib import Path

CARGO = "Cargo.toml"
CHANGELOG = "CHANGELOG.md"
# Human status lines that name the workspace version; `cut` rewrites them.
STATUS_FILES = ("DESIGN.md",)
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
# The debug files' archive, kept apart so the player archive stays small (D-120).
DEBUG_PREFIX = "tungsten-debug"
# CPU levels release archives may carry, fastest first; must match `LEVELS` in
# tools/launcher/src/main.rs. The portable baseline is mandatory (D-072).
LEVELS = ("x86-64-v4", "x86-64-v3", "x86-64-v2", "x86-64")
BASELINE = "x86-64"
LAUNCHER = "tungsten-launcher"

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
                      f"[{releases[0].version}]; bump both with `just release-cut`")

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


def readme(tag, target, members, suffix, levels, game=None):
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
    # Windows builds open no console (D-119), so only Linux shows the launcher's line.
    console = "" if suffix else ", names it on the console,"
    logs = ""
    if game:
        logs = (
            "The examples open no console. Each run writes a log, and a crash a crash\n"
            f"report, to %LOCALAPPDATA%\\{game}\\logs.\n"
            if suffix else
            "Each run writes a log, and a crash a crash report, to\n"
            f"$XDG_STATE_HOME/{game}/logs, by default ~/.local/state/{game}/logs.\n"
        ) + "TUNGSTEN_USER_DIR=<folder> moves them to <folder>/logs.\n\n"
    return (
        f"Tungsten {tag} examples for {target}\n\n"
        "Run an example from any folder:\n\n"
        f"{run}\n\n"
        "Each is a small launcher. It picks the fastest build in bin/ that this CPU\n"
        f"supports ({', '.join(levels)}){console} and runs it from\n"
        "this folder, where the examples read tungsten.json, input.json and assets/.\n"
        f"TUNGSTEN_CPU_LEVEL={BASELINE} forces the portable build.\n\n"
        f"{logs}"
        f"{platform}\n\n"
        "MIT license: LICENSE.\n"
    )


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


def run_tool(command):
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        raise ValueError(f"{' '.join(command)} failed: {result.stderr.strip()}")


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


def game_id(root):
    """`game.id` from the root tungsten.json (D-119), or None."""
    try:
        value = json.loads(read(root, "tungsten.json")).get("game", {}).get("id")
    except (OSError, ValueError, AttributeError):
        return None
    return value if isinstance(value, str) else None


def package(root, tag, target, out, bin_dir=None, splitter=objcopy_split):
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
        # Both archives share the top folder, so the debug one extracts over the player one.
        player, debug = Path(tmp) / "player", Path(tmp) / "debug"
        stage, debug_stage = player / name, debug / name
        stage.mkdir(parents=True)
        for binary, _ in members:
            launcher = stage / (binary + suffix)
            place(built(BASELINE, LAUNCHER), launcher)
            if not suffix:
                splitter(launcher, None)
            for level in levels:
                dest = stage / "bin" / level / (binary + suffix)
                place(built(level, binary), dest)
                debug_dir = debug_stage / "bin" / level
                debug_dir.mkdir(parents=True, exist_ok=True)
                if suffix:
                    pdb, pdb_name = matching_pdb(built(level, binary), release_dir(level))
                    shutil.copyfile(pdb, debug_dir / pdb_name)
                else:
                    splitter(dest, debug_dir / f"{binary}.debug")
                    dest.chmod(0o755)
        for rel in runtime:
            src, dest = root / rel, stage / rel
            if src.is_dir():
                shutil.copytree(src, dest)
            else:
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(src, dest)
        text = readme(tag, target, members, suffix, levels, game_id(root))
        (stage / "README.txt").write_text(text, encoding="utf-8")
        kind = "zip" if suffix else "gztar"
        archive = shutil.make_archive(str(out / name), kind, root_dir=player, base_dir=name)
        debug_archive = shutil.make_archive(str(out / f"{DEBUG_PREFIX}-{tag}-{target}"), kind,
                                            root_dir=debug, base_dir=name)
    return Path(archive), Path(debug_archive)


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
    command = commands.add_parser("notes", help="print release notes for TAG")
    command.add_argument("tag")
    command.add_argument("--link-base", help="prefix for relative links, e.g. https://github.com/o/r/blob/TAG")
    command = commands.add_parser("package", help="archive example binaries with runtime files")
    command.add_argument("tag")
    command.add_argument("target")
    command.add_argument("--bin-dir", type=Path, help="per-level cargo target dirs (default: target)")
    command.add_argument("--out", type=Path, default=Path("dist"))
    command = commands.add_parser("split-debug", help="move a Linux binary's debug info to a linked file")
    command.add_argument("binary", type=Path)
    command.add_argument("debug", type=Path)
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
                      f"(`just release-cut` does), review status prose, run the checks, hand off (docs/releases.md).")
        elif args.command == "notes":
            sys.stdout.write(notes(root, args.tag, args.link_base))
            errors = []
        elif args.command == "split-debug":
            objcopy_split(args.binary, args.debug)
            print(f"{args.binary}: debug info moved to {args.debug}, build-ids equal")
            errors = []
        else:
            for archive in package(root, args.tag, args.target, args.out, args.bin_dir):
                print(archive)
            errors = []
    except (OSError, ValueError) as exc:
        errors = [str(exc)]
    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
