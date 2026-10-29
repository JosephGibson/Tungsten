#!/usr/bin/env python3
"""Real temporary Git repositories; no network, cargo or GitHub."""

import importlib.util
import subprocess
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("status", Path(__file__).with_name("status.py"))
status = importlib.util.module_from_spec(spec)
spec.loader.exec_module(status)

ROADMAP = """# Roadmap

## Now

- `alpha`: the first item.
- `beta`: the second item.

## Next

- `gamma`: waits.

## Later

- `delta`: someday.
"""

PLAN = """# A plan (a-plan)

- **status:** in progress
- **ordered steps:** (1) First. (2) Second. (3) Third.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-29 | done | `a \\| b` counted | none |
| 2 | 2026-10-30 | skipped, a check failed | none | none |

## Follow-ups
"""


class Status(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        base = Path(temp.name)
        self.root = base / "work"
        self.root.mkdir()
        self.git("init", "-b", "main")
        self.git("config", "user.name", "Status Test")
        self.git("config", "user.email", "status@example.invalid")
        self.git("config", "commit.gpgSign", "false")
        self.git("config", "tag.gpgSign", "false")
        self.git("config", "core.hooksPath", str(base / "no-hooks"))
        self.write("Cargo.toml", '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "1.0.0"\n')
        self.write("ROADMAP.md", ROADMAP)
        self.commit("first")
        self.git("tag", "-a", "v1.0.0", "-m", "Release")

    def git(self, *args):
        result = subprocess.run(["git", *args], cwd=self.root, text=True, capture_output=True, check=True)
        return result.stdout.strip()

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def commit(self, message):
        self.git("add", ".")
        self.git("commit", "-m", message)
        return self.git("rev-parse", "--short", "HEAD")

    def report(self):
        return status.report(self.root).splitlines()

    def section(self, title):
        lines = self.report()
        start = next(i for i, line in enumerate(lines) if line.startswith(f"== {title}"))
        end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith("== ")), len(lines))
        return lines[start + 1:end]

    def test_six_sections_in_order(self):
        heads = [line for line in self.report() if line.startswith("== ")]
        self.assertEqual([head.split()[1] for head in heads],
                         ["git", "tree", "sessions", "frozen", "ROADMAP.md", "plans"])

    def test_commits_since_the_newest_final_tag_on_head(self):
        self.write("a.txt", "a\n")
        self.commit("feat: second")
        self.git("tag", "-a", "v1.1.0-rc.1", "-m", "Candidate")
        self.write("b.txt", "b\n")
        self.commit("fix: third")
        self.git("switch", "-q", "-c", "side", "HEAD~2")
        self.write("c.txt", "c\n")
        self.commit("feat: off history")
        self.git("tag", "-a", "v9.0.0", "-m", "Elsewhere")
        self.git("switch", "-q", "main")
        git = self.section("git")
        self.assertIn("newest final tag on HEAD's history: v1.0.0 · on origin/main: no origin/main (last fetch never)",
                      git)
        self.assertIn("commits since v1.0.0: 2", git)
        self.assertEqual([line.split(" ", 3)[3] for line in git if line.startswith("  ")],
                         ["fix: third", "feat: second"])
        self.assertIn("version: 1.0.0 (Cargo.toml)", git)

    def test_a_cut_waits_when_the_version_is_past_the_tag(self):
        self.write("Cargo.toml", '[workspace.package]\nversion = "1.1.0"\n')
        self.commit("chore(release): 1.1.0 second")
        self.assertIn("version: 1.1.0 (Cargo.toml) · a cut waits: the version is past v1.0.0", self.section("git"))

    def test_now_and_next_with_now_count(self):
        roadmap = self.section("ROADMAP.md")
        self.assertEqual(roadmap[-1], "Now items: 2")
        self.assertIn("- `gamma`: waits.", roadmap)
        self.assertNotIn("- `delta`: someday.", roadmap)

    def test_a_plans_rows_and_the_frozen_folder_left_out(self):
        self.write("docs/plans/a-plan.md", PLAN)
        self.write("docs/plans/README.md", "# Plans\n")
        self.write("docs/plans/1.0/old.md", PLAN)
        self.write("docs/plans/list-plan.md", "- **status:** draft\n- **ordered steps:**\n  1. One.\n    - a detail\n"
                   "  2. Two.\n- **done-when:** it works\n")
        self.commit("docs: plans")
        self.assertEqual(self.section("plans"), [
            "docs/plans/a-plan.md: in progress · steps: 3 · evidence rows: 2 · last: step 2, skipped, a check failed",
            "docs/plans/list-plan.md: draft · steps: 2 · evidence rows: 0",
        ])

    def test_a_frozen_record_edited_after_the_d215_commit(self):
        banner = "**Frozen 2026-10-25 (`D-215`):** ROADMAP.md replaces this.\n"
        self.write("docs/plans/1.0/register.md", banner + "Rows.\n")
        self.write("docs/plans/1.0/catalog.md", banner + "Stops.\n")
        self.write("DECISIONS.md", "# Decisions\n\n## D-215 — Operating rules\n")
        self.commit("docs: freeze the records (D-215)")
        self.assertIn("no edits since D-215", self.section("frozen"))
        self.write("docs/plans/1.0/register.md", banner + "Rows, and a new one.\n")
        edit = self.commit("docs: a register row")
        self.write("docs/plans/1.0/register.md", banner + "Rows, and two.\n")
        frozen = self.section("frozen")
        self.assertIn(f"docs/plans/1.0/register.md: tree M · commits since D-215: {edit}", frozen)
        self.assertFalse(any(line.startswith("docs/plans/1.0/catalog.md") for line in frozen))

    def test_a_dirty_file_and_its_age(self):
        self.write("ROADMAP.md", ROADMAP + "\n- `epsilon`: new.\n")
        self.write("new.txt", "new\n")
        tree = self.section("tree")
        self.assertEqual(tree[:2], [" M ROADMAP.md", "?? new.txt"])
        self.assertEqual(tree[2], "edited in the last 3m: ROADMAP.md new.txt")
        later = status.tree_section(self.root.resolve(), time.time() + 600)
        self.assertEqual(later[-1], "edited in the last 3m: none")

    def test_clean_tree(self):
        self.assertEqual(self.section("tree"), ["clean", "edited in the last 3m: none"])

    def test_a_missing_proc_prints_unavailable(self):
        with patch.object(status, "PROC", self.root / "no-proc"):
            sessions = self.section("sessions")
        self.assertEqual(sessions, ["other agent sessions in this tree: unavailable (no /proc)",
                                    "perf capture running: unavailable (no /proc)"])

    def test_cli_runs_inside_the_repository(self):
        result = subprocess.run(["python3", "-B", str(Path(__file__).with_name("status.py"))], cwd=self.root,
                                text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.startswith("== git\nbranch: main · upstream: none\n"))


if __name__ == "__main__":
    unittest.main()
