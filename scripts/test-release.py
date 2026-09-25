#!/usr/bin/env python3
"""Synthetic regression tests for scripts/release.py; no GPU, network or cargo."""

import contextlib
import datetime
import importlib.util
import io
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
rel = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rel)

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


class Release(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.write("Cargo.toml", CARGO)
        self.write("CHANGELOG.md", CHANGELOG)
        self.write("README.md", "Workspace `v0.26.0`; current development branch `0.27`.\n")
        self.write("DESIGN.md", "Workspace `v0.26.0` on branch `0.26`.\n")
        self.write("examples/01_demo/Cargo.toml", '[package]\nname = "example-01-demo"\nversion.workspace = true\n')
        self.write("examples/01_demo/assets/manifest.json", "{}")
        self.write("examples/02_bare/Cargo.toml", '[package]\nname = "example-02-bare"\n')
        for path in ("tungsten.json", "input.json", "LICENSE", "assets/manifest.json", "assets/sprites/a.png"):
            self.write(path, path)

    def write(self, path, text):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)
        return target

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
        self.write("README.md", "No status here.\n")
        self.assertIn("README.md: no 'Workspace", self.errors())

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
        self.assertEqual(rel.cut(self.root, "0.27.0", DATE), [])
        text = (self.root / "CHANGELOG.md").read_text()
        self.assertIn("## [Unreleased]\n\n## [0.27.0] - 2026-10-01\n\n### Added\n\n- Release tooling", text)
        state = rel.check_tree(self.root)
        self.assertEqual((state.errors, state.version, state.unreleased), ([], "0.27.0", ""))
        self.assertIn("Release tooling", state.releases[0].body)
        self.assertIn('[workspace.dependencies.decoy]\nversion = "9.9.9"', (self.root / "Cargo.toml").read_text())
        self.assertIn("`v0.27.0`; current development branch `0.27`", (self.root / "README.md").read_text())
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

    def bins(self, target, suffix):
        directory = self.root / "target" / target / "release"
        for name in ("example-01-demo", "example-02-bare"):
            self.write(f"target/{target}/release/{name}{suffix}", "binary").chmod(0o644)
        self.write(f"target/{target}/release/example-01-demo.d", "dep-info")
        return directory

    def test_package_linux_archive_layout(self):
        self.bins("x86_64-unknown-linux-gnu", "")
        archive = rel.package(self.root, "v0.26.0", "x86_64-unknown-linux-gnu", self.root / "dist")
        self.assertEqual(archive.name, "tungsten-examples-v0.26.0-x86_64-unknown-linux-gnu.tar.gz")
        with tarfile.open(archive) as tar:
            members = {m.name.split("/", 1)[1]: m for m in tar.getmembers() if "/" in m.name}
        for path in ("example-01-demo", "example-02-bare", "tungsten.json", "input.json", "LICENSE",
                     "README.txt", "assets/manifest.json", "assets/sprites/a.png",
                     "examples/01_demo/assets/manifest.json"):
            self.assertIn(path, members)
        self.assertEqual(members["example-01-demo"].mode & 0o111, 0o111)
        self.assertNotIn("example-01-demo.d", members)
        self.assertNotIn("examples/02_bare/assets", members)

    def test_package_windows_zip_and_missing_binary(self):
        self.bins("x86_64-pc-windows-msvc", ".exe")
        archive = rel.package(self.root, "v0.0.0-test", "x86_64-pc-windows-msvc", self.root / "dist")
        self.assertEqual(archive.suffix, ".zip")
        with zipfile.ZipFile(archive) as zipped:
            names = set(zipped.namelist())
            readme = zipped.read("tungsten-examples-v0.0.0-test-x86_64-pc-windows-msvc/README.txt").decode()
        self.assertIn("tungsten-examples-v0.0.0-test-x86_64-pc-windows-msvc/example-02-bare.exe", names)
        self.assertIn("  .\\example-01-demo.exe", readme)
        with self.assertRaisesRegex(ValueError, "not a target triple"):
            rel.package(self.root, "v0.0.0-test", "../escape", self.root / "dist")
        (self.root / "target/x86_64-pc-windows-msvc/release/example-02-bare.exe").unlink()
        with self.assertRaisesRegex(ValueError, "missing example-02-bare.exe"):
            rel.package(self.root, "v0.0.0-test", "x86_64-pc-windows-msvc", self.root / "dist")

    def test_cli_exit_codes(self):
        def run(*argv):
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = rel.main(["--root", str(self.root), *argv])
            return code, out.getvalue() + err.getvalue()

        code, output = run("check")
        self.assertEqual(code, 0)
        self.assertIn("workspace 0.26.0", output)
        self.assertIn("rehearsal", run("check", "v0.0.0-test")[1])
        self.assertEqual(run("check", "v0.25.0")[0], 1)
        self.assertEqual(run("cut", "0.27.0", "--date", "2026-10-01")[0], 0)
        self.assertEqual(run("check", "v0.27.0")[0], 0)
        self.assertEqual(run("notes", "v9.9.9")[0], 1)
        self.assertEqual(run("package", "v0.27.0", "x86_64-unknown-linux-gnu", "--out", str(self.root / "d"))[0], 1)


if __name__ == "__main__":
    unittest.main()
