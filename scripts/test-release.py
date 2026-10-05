#!/usr/bin/env python3
"""Synthetic regression tests for scripts/release.py; no GPU, network or cargo."""

import contextlib
import datetime
import hashlib
import importlib.util
import io
import re
import struct
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
rel = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rel)
LAUNCHER = rel.LAUNCHER

CARGO = """\
[workspace.dependencies.decoy]
version = "9.9.9"

[workspace.package]
version = "0.26.0"
edition = "2024"

[workspace]
members = [
    "crates/core",
    "examples/01_demo",
    "examples/02_bare",
]
"""
CHANGELOG = """\
# Changelog

Intro linking [the index](docs/INDEX.md).

## [Unreleased]

### Added

- Release tooling, see [index](docs/INDEX.md), [site](https://example.com) and [anchor](#top).

## [0.26.0] - 2026-07-06

Summary: lighting ([plan](docs/plans/x.md)).

## [0.9.0] - 2026-04-15

- Ninth.

## [0.8.0-alpha] - 2026-04-15

- Alpha.

## [0.2.0-alpha.0] - 2026-04-12

- First.
"""
DATE = datetime.date(2026, 10, 1)
BUILD_ID = bytes(range(20))
GUID = bytes.fromhex("33221100554477668899aabbccddeeff")


def synthetic_elf(build_id=None, sections=()):
    """A 64-bit little-endian ELF: a GNU build-id note when given, then the named sections."""
    body = []
    if build_id is not None:
        note = struct.pack("<III", 4, len(build_id), 3) + b"GNU\0" + build_id
        body.append((".note.gnu.build-id", 7, note + b"\0" * (-len(note) % 4), 4))
    body += [(name, 1, b"\x01" * 16, 1) for name in sections]
    names, offsets = b"\0", []
    for name in [entry[0] for entry in body] + [".shstrtab"]:
        offsets.append(len(names))
        names += name.encode() + b"\0"
    body.append((".shstrtab", 3, names, 1))
    data = bytearray(64)
    headers = [bytes(64)]
    for (_, kind, content, align), name in zip(body, offsets):
        headers.append(struct.pack("<IIQQQQIIQQ", name, kind, 0, 0, len(data), len(content), 0, 0, align, 0))
        data += content
    shoff = len(data)
    data += b"".join(headers)
    struct.pack_into("<4sBB", data, 0, b"\x7fELF", 2, 1)
    struct.pack_into("<Q", data, 0x28, shoff)
    struct.pack_into("<HHH", data, 0x3A, 64, len(headers), len(headers) - 1)
    return bytes(data)


def synthetic_pe(guid, age, pdb_path):
    """A PE32+ file with one section holding its debug directory and a CodeView record."""
    pe, optional_size = 0x40, 240
    optional, sections = pe + 24, pe + 24 + optional_size
    name = pdb_path.encode() + b"\0"
    data = bytearray(0x434 + len(name))
    data[0:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, pe)
    data[pe:pe + 4] = b"PE\0\0"
    struct.pack_into("<HH", data, pe + 4, 0x8664, 1)
    struct.pack_into("<H", data, pe + 20, optional_size)
    struct.pack_into("<H", data, optional, 0x20B)
    struct.pack_into("<I", data, optional + 108, 16)
    struct.pack_into("<II", data, optional + 112 + 8 * 6, 0x1000, 28)
    data[sections:sections + 8] = b".rdata\0\0"
    struct.pack_into("<IIII", data, sections + 8, 0x200, 0x1000, 0x200, 0x400)
    struct.pack_into("<IIII", data, 0x400 + 12, 2, 24 + len(name), 0, 0x41C)
    data[0x41C:0x420] = b"RSDS"
    data[0x420:0x430] = guid
    struct.pack_into("<I", data, 0x430, age)
    data[0x434:] = name
    return bytes(data)


def synthetic_pdb(guid, age, block_size=512):
    """An MSF 7.0 file: superblock, two free-page maps, the block map, the directory, the info stream."""
    info = struct.pack("<III", 20000404, 0, age) + guid
    directory = struct.pack("<IIII", 2, 0, len(info), 5)
    blocks = [rel.MSF_MAGIC + struct.pack("<6I", block_size, 1, 6, len(directory), 0, 3),
              b"", b"", struct.pack("<I", 4), directory, info]
    return b"".join(block.ljust(block_size, b"\0") for block in blocks)


class Release(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.write("Cargo.toml", CARGO)
        self.write("CHANGELOG.md", CHANGELOG)
        self.write("README.md", "# Engine\n\nBuild and run the examples.\n")
        self.write("DESIGN.md", "Workspace `v0.26.0` on branch `0.26`.\n")
        self.write("examples/01_demo/Cargo.toml", '[package]\nname = "example-01-demo"\nversion.workspace = true\n')
        self.write("examples/01_demo/assets/manifest.json", "{}")
        self.write("examples/02_bare/Cargo.toml", '[package]\nname = "example-02-bare"\n')
        for path in ("tungsten.json", "input.json", "LICENSE", "assets/manifest.json", "assets/sprites/a.png"):
            self.write(path, path)

    def write(self, path, text):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(text, bytes):
            target.write_bytes(text)
        else:
            target.write_text(text)
        return target

    def fake_split(self, binary, debug):
        """Stands in for objcopy: marks the binary stripped and writes its debug file."""
        self.splits.append((binary.parent.name, binary.name, debug and debug.name))
        if debug is not None:
            debug.write_bytes(b"debug of " + binary.read_bytes())
        binary.write_bytes(binary.read_bytes() + b" (stripped)")

    def edit(self, path, old, new):
        text = (self.root / path).read_text()
        self.assertIn(old, text)
        self.write(path, text.replace(old, new, 1))

    def errors(self, tag=None):
        state = rel.check_tree(self.root)
        return " | ".join(state.errors + (rel.check_tag(state, tag) if tag else []))

    def snapshot(self):
        return {p: (self.root / p).read_text() for p in ("Cargo.toml", "CHANGELOG.md", "README.md", "DESIGN.md")}

    def test_consistent_tree_passes(self):
        state = rel.check_tree(self.root)
        self.assertEqual(state.errors, [])
        self.assertEqual(state.version, "0.26.0")
        self.assertEqual([r.version for r in state.releases], ["0.26.0", "0.9.0", "0.8.0-alpha", "0.2.0-alpha.0"])
        self.assertIn("Release tooling", state.unreleased)

    def test_version_drift_fails(self):
        self.edit("Cargo.toml", 'version = "0.26.0"', 'version = "0.27.0"')
        self.assertIn("!= newest CHANGELOG.md release [0.26.0]", self.errors())
        self.assertIn("status line says v0.26.0", self.errors())

    def test_decoy_version_outside_workspace_package_is_ignored(self):
        self.assertEqual(rel.cargo_version(CARGO), "0.26.0")
        self.assertNotIn("9.9.9", rel.set_cargo_version(CARGO, "1.0.0").split("[workspace.package]")[1])

    def test_status_line_drift_and_absence_fail(self):
        self.edit("DESIGN.md", "v0.26.0", "v0.25.0")
        self.assertIn("DESIGN.md: status line says v0.25.0", self.errors())
        self.write("DESIGN.md", "No status here.\n")
        self.assertIn("DESIGN.md: no 'Workspace", self.errors())

    def test_readme_is_not_part_of_release_state(self):
        self.write("README.md", "Workspace `v9.9.9` is historical prose.\n")
        before = (self.root / "README.md").read_bytes()
        self.assertEqual(rel.cut(self.root, "0.27.0", DATE), [])
        self.assertEqual((self.root / "README.md").read_bytes(), before)
        (self.root / "README.md").unlink()
        self.assertEqual(self.errors(), "")

    def test_unreleased_must_lead_once(self):
        self.edit("CHANGELOG.md", "## [Unreleased]\n", "")
        self.assertIn("exactly one '## [Unreleased]'", self.errors())
        self.setUp()
        self.edit("CHANGELOG.md", "## [0.9.0]", "## [Unreleased]\n\n## [0.9.0]")
        self.assertIn("exactly one", self.errors())
        self.setUp()
        self.edit("CHANGELOG.md", "## [Unreleased]\n", "")
        self.edit("CHANGELOG.md", "## [0.9.0]", "## [Unreleased]\n\n## [0.9.0]")
        self.assertIn("exactly one", self.errors())

    def test_malformed_headings_fail(self):
        malformed = "is not '## [X.Y.Z] - YYYY-MM-DD'"
        for new, message in (("## [0.9] - 2026-04-15", malformed),
                             ("## 0.9.0 - 2026-04-15", malformed),
                             ("## [0.9.0]", malformed),
                             ("## [unreleased]", malformed),
                             ("## [0.9.0] - 2026-02-30", "invalid date")):
            with self.subTest(new=new):
                self.setUp()
                self.edit("CHANGELOG.md", "## [0.9.0] - 2026-04-15", new)
                self.assertIn(f"{new!r} {message}" if message == malformed else message, self.errors())

    def test_order_and_dates_must_descend(self):
        self.edit("CHANGELOG.md", "## [0.9.0] - 2026-04-15", "## [0.26.0] - 2026-04-15")
        self.assertIn("must be above", self.errors())
        self.setUp()
        self.edit("CHANGELOG.md", "## [0.9.0] - 2026-04-15", "## [0.9.0] - 2026-08-01")
        self.assertIn("is before [0.9.0]", self.errors())

    def test_semver_precedence(self):
        ordered = ["0.2.0-alpha.0", "0.8.0-alpha", "1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta",
                   "1.0.0-beta.2", "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0", "1.2.0", "1.10.0"]
        self.assertEqual(sorted(ordered, key=rel.semver_key), ordered)
        for bad in ("1.0", "01.0.0", "1.0.0+build", "1.0.0-01", "v1.0.0", "1.0.\u0663", "1.0.0-\u0663"):
            self.assertIsNone(rel.SEMVER_RE.match(bad), bad)

    def test_release_tags(self):
        self.assertEqual(self.errors("v0.26.0"), "")
        self.assertIn("does not match", self.errors("v0.27.0"))
        for bad in ("0.26.0", "v0.26", "v0.26.0+b1", "release-0.26.0"):
            self.assertIn("is not v<SemVer>", self.errors(bad), bad)

    def test_release_tag_needs_notes(self):
        self.edit("CHANGELOG.md", "Summary: lighting ([plan](docs/plans/x.md)).\n", "")
        self.assertIn("has no notes", self.errors("v0.26.0"))

    def test_prerelease_tag_is_a_rehearsal_on_a_consistent_tree(self):
        self.assertEqual(self.errors("v0.0.0-test"), "")
        self.assertEqual(self.errors("v0.27.0-rc.1"), "")
        self.assertTrue(rel.is_rehearsal(rel.check_tree(self.root), "v0.0.0-test"))
        self.edit("Cargo.toml", 'version = "0.26.0"', 'version = "0.25.0"')
        self.assertIn("!= newest", self.errors("v0.0.0-test"))

    def test_cut_prerelease_version_is_a_release_not_a_rehearsal(self):
        self.assertEqual(rel.cut(self.root, "0.27.0-rc.1", DATE), [])
        self.assertFalse(rel.is_rehearsal(rel.check_tree(self.root), "v0.27.0-rc.1"))
        self.assertEqual(self.errors("v0.27.0-rc.1"), "")
        body = rel.notes(self.root, "v0.27.0-rc.1")
        self.assertTrue(body.startswith("### Added"), body)

    def test_prerelease_tag_with_its_own_section_must_match_the_version(self):
        self.edit("CHANGELOG.md", "## [0.8.0-alpha]", "## [0.9.0-rc.1] - 2026-04-15\n\n- RC.\n\n## [0.8.0-alpha]")
        self.assertEqual(self.errors(), "")
        self.assertIn("does not match", self.errors("v0.9.0-rc.1"))

    def test_cut_moves_unreleased_and_bumps_versions(self):
        readme = (self.root / "README.md").read_bytes()
        self.assertEqual(rel.cut(self.root, "0.27.0", DATE), [])
        text = (self.root / "CHANGELOG.md").read_text()
        self.assertIn("## [Unreleased]\n\n## [0.27.0] - 2026-10-01\n\n### Added\n\n- Release tooling", text)
        state = rel.check_tree(self.root)
        self.assertEqual((state.errors, state.version, state.unreleased), ([], "0.27.0", ""))
        self.assertIn("Release tooling", state.releases[0].body)
        self.assertIn('[workspace.dependencies.decoy]\nversion = "9.9.9"', (self.root / "Cargo.toml").read_text())
        self.assertEqual((self.root / "README.md").read_bytes(), readme)
        self.assertIn("Workspace `v0.27.0`", (self.root / "DESIGN.md").read_text())
        self.assertEqual(self.errors("v0.27.0"), "")
        # A second cut has nothing to release.
        self.assertIn("is empty", " ".join(rel.cut(self.root, "0.28.0", DATE)))

    def test_cut_refuses_without_writing(self):
        before = self.snapshot()
        for version, date, message in (("0.26.0", DATE, "must be above"),
                                       ("0.9.1", DATE, "must be above"),
                                       ("v0.27.0", DATE, "no leading v"),
                                       ("0.27.0", datetime.date(2026, 7, 1), "before the newest")):
            with self.subTest(version=version):
                self.assertIn(message, " ".join(rel.cut(self.root, version, date)))
                self.assertEqual(self.snapshot(), before)
        self.edit("DESIGN.md", "v0.26.0", "v0.25.0")
        before = self.snapshot()
        self.assertIn("inconsistent", " ".join(rel.cut(self.root, "0.27.0", DATE)))
        self.assertEqual(self.snapshot(), before)

    def test_notes_for_release_and_prerelease(self):
        body = rel.notes(self.root, "v0.26.0", "https://github.com/o/r/blob/v0.26.0/")
        self.assertEqual(body, "Summary: lighting ([plan](https://github.com/o/r/blob/v0.26.0/docs/plans/x.md)).\n")
        pre = rel.notes(self.root, "v0.0.0-test", "https://h/b/v0.0.0-test")
        self.assertTrue(pre.startswith("Pre-release build of unreleased changes (`v0.0.0-test`)."))
        self.assertIn("[index](https://h/b/v0.0.0-test/docs/INDEX.md)", pre)
        self.assertIn("[site](https://example.com)", pre)
        self.assertIn("[anchor](#top)", pre)
        with self.assertRaises(ValueError):
            rel.notes(self.root, "v0.3.0", None)

    def test_examples_follow_workspace_members(self):
        self.assertEqual(rel.examples(self.root),
                         [("example-01-demo", "examples/01_demo"), ("example-02-bare", "examples/02_bare")])
        self.edit("Cargo.toml", '    "examples/02_bare",\n', '    # "examples/02_bare",\n')
        self.assertEqual([d for _, d in rel.examples(self.root)], ["examples/01_demo"])
        self.edit("Cargo.toml", '    "examples/01_demo",\n', '    "examples/*",\n')
        self.write("examples/README.md", "not a package")
        self.assertEqual([d for _, d in rel.examples(self.root)], ["examples/01_demo", "examples/02_bare"])

    def bins(self, target, suffix, levels=("x86-64-v3", "x86-64")):
        """Fake builds per level; on Windows each example is a PE whose PDB sits beside it."""
        self.splits = []
        for level in levels:
            release = f"target/{level}/{target}/release"
            for name in ("example-01-demo", "example-02-bare", "tungsten-launcher"):
                content = f"{level} {name}".encode()
                if suffix and name != LAUNCHER:
                    guid, pdb = hashlib.md5(content).digest(), name.replace("-", "_") + ".pdb"
                    self.write(f"{release}/{pdb}", synthetic_pdb(guid, 1))
                    content = synthetic_pe(guid, 1, f"D:\\a\\{level}\\release\\deps\\{pdb}")
                self.write(f"{release}/{name}{suffix}", content).chmod(0o644)
            self.write(f"{release}/example-01-demo.d", "dep-info")

    def package(self, tag, target):
        return rel.package(self.root, tag, target, self.root / "dist", splitter=self.fake_split)

    def test_package_linux_archive_layout(self):
        linux = "x86_64-unknown-linux-gnu"
        self.bins(linux, "")
        archive, debug = self.package("v0.26.0", linux)
        top = f"tungsten-examples-v0.26.0-{linux}"
        self.assertEqual(archive.name, f"{top}.tar.gz")
        self.assertEqual(debug.name, f"tungsten-debug-v0.26.0-{linux}.tar.gz")
        with tarfile.open(archive) as tar:
            members = {m.name.split("/", 1)[1]: m for m in tar.getmembers() if "/" in m.name}
            launcher = tar.extractfile(f"{top}/example-01-demo").read()
            fast = tar.extractfile(f"{top}/bin/x86-64-v3/example-01-demo").read()
            readme = tar.extractfile(f"{top}/README.txt").read().decode()
        for path in ("example-01-demo", "example-02-bare", "bin/x86-64-v3/example-01-demo",
                     "bin/x86-64/example-02-bare", "tungsten.json", "input.json", "LICENSE", "README.txt",
                     "assets/manifest.json", "assets/sprites/a.png", "examples/01_demo/assets/manifest.json"):
            self.assertIn(path, members)
        # Launcher copies come from the portable build; each level keeps its own binaries.
        # Every copy is stripped; only the examples' debug info is kept.
        self.assertEqual(launcher, b"x86-64 tungsten-launcher (stripped)")
        self.assertEqual(fast, b"x86-64-v3 example-01-demo (stripped)")
        self.assertFalse([path for path in members if path.endswith((".debug", ".pdb"))])
        for path in ("example-01-demo", "bin/x86-64-v3/example-01-demo", "bin/x86-64/example-02-bare"):
            self.assertEqual(members[path].mode & 0o111, 0o111, path)
        for path in ("example-01-demo.d", "bin/x86-64/example-01-demo.d", "tungsten-launcher",
                     "bin/x86-64/tungsten-launcher", "examples/02_bare/assets"):
            self.assertNotIn(path, members)
        self.assertIn("(x86-64-v3, x86-64), names it on the console,", readme)
        self.assertIn("  ./example-01-demo", readme)

    def test_package_linux_debug_archive_extracts_over_the_player_archive(self):
        linux = "x86_64-unknown-linux-gnu"
        self.bins(linux, "")
        _, debug = self.package("v0.26.0", linux)
        top = f"tungsten-examples-v0.26.0-{linux}"
        with tarfile.open(debug) as tar:
            files = {m.name: m for m in tar.getmembers() if m.isfile()}
            content = tar.extractfile(f"{top}/bin/x86-64/example-02-bare.debug").read()
        self.assertEqual(sorted(files), sorted(f"{top}/bin/{level}/{name}.debug"
                                               for level in ("x86-64", "x86-64-v3")
                                               for name in ("example-01-demo", "example-02-bare")))
        self.assertEqual(content, b"debug of x86-64 example-02-bare")
        # Each launcher copy is stripped with no debug file; each example build is split.
        self.assertEqual(sorted(self.splits, key=str), sorted(
            [(top, name, None) for name in ("example-01-demo", "example-02-bare")]
            + [(level, name, f"{name}.debug") for level in ("x86-64-v3", "x86-64")
               for name in ("example-01-demo", "example-02-bare")], key=str))

    def test_package_windows_zip_and_missing_binary(self):
        windows = "x86_64-pc-windows-msvc"
        self.bins(windows, ".exe")
        archive, debug = self.package("v0.0.0-test", windows)
        top = f"tungsten-examples-v0.0.0-test-{windows}"
        self.assertEqual(archive.suffix, ".zip")
        with zipfile.ZipFile(archive) as zipped:
            names = set(zipped.namelist())
            readme = zipped.read(f"{top}/README.txt").decode()
        self.assertIn(f"{top}/example-02-bare.exe", names)
        self.assertIn(f"{top}/bin/x86-64-v3/example-02-bare.exe", names)
        self.assertFalse([name for name in names if name.endswith((".debug", ".pdb"))])
        self.assertIn("  .\\example-01-demo.exe", readme)
        self.assertNotIn("console,", readme)
        # Nothing is split on Windows: the PDBs already hold the debug info.
        self.assertEqual(self.splits, [])
        with zipfile.ZipFile(debug) as zipped:
            self.assertEqual(debug.name, f"tungsten-debug-v0.0.0-test-{windows}.zip")
            self.assertEqual(sorted(n for n in zipped.namelist() if not n.endswith("/")),
                             sorted(f"{top}/bin/{level}/{name}.pdb" for level in ("x86-64", "x86-64-v3")
                                    for name in ("example_01_demo", "example_02_bare")))
        with self.assertRaisesRegex(ValueError, "not a target triple"):
            rel.package(self.root, "v0.0.0-test", "../escape", self.root / "dist")
        (self.root / f"target/x86-64-v3/{windows}/release/example-02-bare.exe").unlink()
        with self.assertRaisesRegex(ValueError, "missing x86-64-v3/example-02-bare.exe"):
            self.package("v0.0.0-test", windows)

    def test_package_windows_needs_each_pdb_to_match_its_record(self):
        windows = "x86_64-pc-windows-msvc"
        self.bins(windows, ".exe")
        pdb = self.root / f"target/x86-64/{windows}/release/example_02_bare.pdb"
        pdb.write_bytes(synthetic_pdb(GUID, 1))
        with self.assertRaisesRegex(ValueError, "example_02_bare.pdb: GUID and age differ"):
            self.package("v0.0.0-test", windows)
        pdb.unlink()
        with self.assertRaisesRegex(ValueError, "names example_02_bare.pdb, which .* lacks"):
            self.package("v0.0.0-test", windows)

    def test_readme_names_the_log_folders(self):
        self.write("tungsten.json", '{ "game": { "id": "my-game" } }')
        for target, suffix, folder in (("x86_64-unknown-linux-gnu", "", "~/.local/state/my-game/logs"),
                                       ("x86_64-pc-windows-msvc", ".exe", "%LOCALAPPDATA%\\my-game\\logs")):
            with self.subTest(target=target):
                self.bins(target, suffix)
                archive, _ = self.package("v0.26.0", target)
                top = f"tungsten-examples-v0.26.0-{target}"
                if suffix:
                    with zipfile.ZipFile(archive) as zipped:
                        readme = zipped.read(f"{top}/README.txt").decode()
                else:
                    with tarfile.open(archive) as tar:
                        readme = tar.extractfile(f"{top}/README.txt").read().decode()
                self.assertIn(folder, readme)
                self.assertIn("TUNGSTEN_USER_DIR=<folder> moves them to <folder>/logs.", readme)
        # Without a game id the README names no folder.
        self.write("tungsten.json", "{}")
        self.assertNotIn("logs", rel.readme("v0.26.0", "t", [("example-01-demo", "")], "", ["x86-64"],
                                            rel.game_id(self.root)))

    def test_elf_build_id_and_split_checks(self):
        self.assertEqual(rel.elf_build_id(synthetic_elf(BUILD_ID, [".debug_line"])), BUILD_ID)
        self.assertIsNone(rel.elf_build_id(synthetic_elf(None, [".text"])))
        self.assertTrue(rel.has_section(synthetic_elf(None, [".debug_line"]), ".debug_line"))
        self.assertFalse(rel.has_section(synthetic_elf(BUILD_ID), ".debug_line"))
        binary = self.write("split/game", synthetic_elf(BUILD_ID, [".text"]))
        debug = self.write("split/game.debug", synthetic_elf(BUILD_ID, [".debug_info", ".debug_line"]))
        rel.check_split(binary, debug)
        self.write("split/game.debug", synthetic_elf(bytes(20), [".debug_line"]))
        with self.assertRaisesRegex(ValueError, r"split/game: build-id 000102.* differs from game\.debug's"):
            rel.check_split(binary, debug)
        self.write("split/game.debug", synthetic_elf(BUILD_ID, [".debug_info"]))
        with self.assertRaisesRegex(ValueError, r"no \.debug_line section; was game built stripped"):
            rel.check_split(binary, debug)
        self.write("split/game", synthetic_elf(None, [".text"]))
        with self.assertRaisesRegex(ValueError, "split/game: no GNU build-id"):
            rel.check_split(binary, debug)
        self.write("split/game", "#!/bin/sh\n")
        with self.assertRaisesRegex(ValueError, "not a 64-bit little-endian ELF"):
            rel.check_split(binary, debug)

    def test_pdb_identity_readers(self):
        path = "D:\\a\\x86-64\\release\\deps\\game.pdb"
        record = rel.codeview(synthetic_pe(GUID, 3, path))
        self.assertEqual(record, rel.CodeView(GUID, 3, path))
        self.assertIsNone(rel.codeview(b"MZ" + bytes(62)))
        self.assertIsNone(rel.codeview(synthetic_elf(BUILD_ID)))
        self.assertEqual(rel.pdb_info(synthetic_pdb(GUID, 3)), (GUID, 3))
        self.assertEqual(rel.pdb_info(synthetic_pdb(GUID, 3, block_size=4096)), (GUID, 3))
        with self.assertRaisesRegex(ValueError, "not an MSF 7.0 PDB"):
            rel.pdb_info(b"Microsoft C/C++ program database 2.00\r\n")
        exe = self.write("win/game.exe", synthetic_pe(GUID, 3, path))
        self.write("win/game.pdb", synthetic_pdb(GUID, 3))
        self.assertEqual(rel.matching_pdb(exe, self.root / "win"), (self.root / "win/game.pdb", "game.pdb"))
        for guid, age in ((GUID, 4), (bytes(16), 3)):
            with self.subTest(guid=guid.hex(), age=age):
                self.write("win/game.pdb", synthetic_pdb(guid, age))
                with self.assertRaisesRegex(ValueError, "game.pdb: GUID and age differ"):
                    rel.matching_pdb(exe, self.root / "win")
        self.write("win/game.exe", "MZ text")
        with self.assertRaisesRegex(ValueError, "no CodeView record"):
            rel.matching_pdb(exe, self.root / "win")

    def test_package_needs_the_portable_baseline_and_launcher(self):
        linux = "x86_64-unknown-linux-gnu"
        self.bins(linux, "", levels=("x86-64-v3",))
        with self.assertRaisesRegex(ValueError, "no x86-64/x86_64-unknown-linux-gnu/release build"):
            self.package("v0.26.0", linux)
        self.bins(linux, "", levels=("x86-64",))
        (self.root / f"target/x86-64/{linux}/release/tungsten-launcher").unlink()
        with self.assertRaisesRegex(ValueError, "missing x86-64/tungsten-launcher"):
            self.package("v0.26.0", linux)

    def test_levels_match_the_launcher(self):
        source = (Path(__file__).resolve().parent.parent / "tools/launcher/src/main.rs").read_text()
        levels = re.search(r"const LEVELS: \[&str; \d+\] = \[([^\]]*)\]", source)
        self.assertEqual(tuple(re.findall(r'"([^"]+)"', levels[1])), rel.LEVELS)
        self.assertEqual(rel.LEVELS[-1], rel.BASELINE)

    def test_cli_exit_codes(self):
        def run(*argv):
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = rel.main(["--root", str(self.root), *argv])
            return code, out.getvalue() + err.getvalue()

        code, output = run("check")
        self.assertEqual(code, 0)
        self.assertIn("workspace 0.26.0", output)
        self.assertEqual(run("version"), (0, "0.26.0\n"))
        self.assertIn("rehearsal", run("check", "v0.0.0-test")[1])
        self.assertEqual(run("check", "v0.25.0")[0], 1)
        self.assertEqual(run("cut", "0.27.0", "--date", "2026-10-01")[0], 0)
        self.assertEqual(run("check", "v0.27.0")[0], 0)
        self.assertEqual(run("notes", "v9.9.9")[0], 1)
        self.assertEqual(run("package", "v0.27.0", "x86_64-unknown-linux-gnu", "--out", str(self.root / "d"))[0], 1)


if __name__ == "__main__":
    unittest.main()
