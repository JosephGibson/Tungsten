#!/usr/bin/env python3
"""Tests for scripts/clean-target.py on synthetic target folders; no cargo."""

import contextlib
import importlib.util
import io
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(file))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


clean = load("clean_target", "clean-target.py")


def touch(path, age, size=1):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"x" * size)
    os.utime(path, (1_000_000 - age, 1_000_000 - age))


def hexhash(n):
    return f"{n:016x}"


class CleanTarget(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.target = Path(tmp.name)
        self.deps = self.target / "debug" / "deps"

    def run_main(self, *args):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), mock.patch.object(clean, "build_running", return_value=False):
            code = clean.main(["--target-dir", str(self.target), *args])
        return code, out.getvalue()

    def unit(self, name, n, age):
        """A test binary with its .d and two .dwo files, all of one age."""
        base = f"{name}-{hexhash(n)}"
        touch(self.deps / base, age, 100)
        touch(self.deps / f"{base}.d", age)
        touch(self.deps / f"{base}.0.rcgu.dwo", age)
        touch(self.deps / f"{base}.1.rcgu.dwo", age)
        return base

    def test_keeps_newest_units_per_name_and_removes_whole_older_units(self):
        units = [self.unit("example_02_bench", n, age=n * 10) for n in range(1, 5)]
        other = self.unit("tungsten_core", 9, age=500)
        code, out = self.run_main("--apply", "--keep", "2")
        self.assertEqual(code, 0)
        left = sorted(p.name for p in self.deps.iterdir())
        self.assertEqual(left, sorted(f"{base}{ext}" for base in (units[0], units[1], other)
                                       for ext in ("", ".d", ".0.rcgu.dwo", ".1.rcgu.dwo")))
        self.assertIn("debug/deps: 2 stale units, 8 entries", out)

    def test_dry_run_removes_nothing(self):
        for n in range(1, 4):
            self.unit("camera", n, age=n)
        before = sorted(self.deps.iterdir())
        code, out = self.run_main("--keep", "1")
        self.assertEqual(code, 0)
        self.assertEqual(sorted(self.deps.iterdir()), before)
        self.assertIn("2 stale units", out)
        self.assertIn("rerun with --apply", out)

    def test_unit_age_is_its_newest_file(self):
        old = self.unit("camera", 1, age=100)
        touch(self.deps / f"{old}.2.rcgu.dwo", age=1)  # one fresh file keeps the unit young
        self.unit("camera", 2, age=10)
        self.unit("camera", 3, age=20)
        self.run_main("--apply", "--keep", "2")
        names = {p.name.split(".", 1)[0] for p in self.deps.iterdir()}
        self.assertEqual(names, {old, f"camera-{hexhash(2)}"})

    def test_names_without_a_hash_are_never_touched(self):
        touch(self.deps / "libfoo.rlib", age=999)
        touch(self.deps / "notes-short.txt", age=999)
        self.unit("camera", 1, age=1)
        self.run_main("--apply", "--keep", "1")
        self.assertTrue((self.deps / "libfoo.rlib").exists())
        self.assertTrue((self.deps / "notes-short.txt").exists())

    def test_unit_directories_in_build_fingerprint_and_incremental(self):
        for sub, make in (("build", hexhash), (".fingerprint", hexhash), ("incremental", lambda n: f"{n:013x}")):
            for n, age in ((1, 30), (2, 20), (3, 10)):
                touch(self.target / "debug" / sub / f"tungsten_core-{make(n)}" / "file", age)
                os.utime(self.target / "debug" / sub / f"tungsten_core-{make(n)}", (1_000_000 - age,) * 2)
        self.run_main("--apply", "--keep", "1")
        for sub in ("build", ".fingerprint", "incremental"):
            left = [p.name for p in (self.target / "debug" / sub).iterdir()]
            self.assertEqual(len(left), 1, sub)
            self.assertTrue(left[0].startswith("tungsten_core-"))
            self.assertTrue(left[0].endswith(("3", hexhash(3))), left[0])

    def test_profiles_are_opt_in(self):
        release = self.target / "release" / "deps"
        for n, age in ((1, 20), (2, 10)):
            touch(release / f"camera-{hexhash(n)}", age)
        self.run_main("--apply", "--keep", "1")
        self.assertEqual(len(list(release.iterdir())), 2)
        self.run_main("--apply", "--keep", "1", "--profile", "release")
        self.assertEqual([p.name for p in release.iterdir()], [f"camera-{hexhash(2)}"])

    def test_hard_links_count_once(self):
        a = self.unit("camera", 1, age=20)
        self.unit("camera", 2, age=10)
        os.link(self.deps / a, self.target / "linked")
        seen = set()
        size = clean.disk_bytes([self.deps / a, self.target / "linked"], seen)
        self.assertEqual(size, clean.disk_bytes([self.deps / a], set()))

    def test_apply_refuses_while_a_build_runs(self):
        self.unit("camera", 1, age=20)
        self.unit("camera", 2, age=10)
        err = io.StringIO()
        with mock.patch.object(clean, "build_running", return_value=True), contextlib.redirect_stderr(err):
            code = clean.main(["--target-dir", str(self.target), "--apply", "--keep", "1"])
        self.assertEqual(code, 1)
        self.assertIn("cargo or rustc is running", err.getvalue())
        self.assertEqual(len(list(self.deps.iterdir())), 8)

    def test_keep_below_one_is_rejected(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            clean.main(["--target-dir", str(self.target), "--keep", "0"])


if __name__ == "__main__":
    unittest.main()
