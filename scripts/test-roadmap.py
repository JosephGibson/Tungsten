#!/usr/bin/env python3
"""Fixture-tree tests for scripts/roadmap.py: catalog payload and status derivation (no network)."""

import contextlib
import importlib.util
import io
import json
import os
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("roadmap", Path(__file__).with_name("roadmap.py"))
roadmap = importlib.util.module_from_spec(spec)
spec.loader.exec_module(roadmap)

PLAN = """# Plan

## 10. Register

| Candidate | Milestone plan | Release | Status |
| --- | --- | --- | --- |
| Step 0 | — | 0.42 | Landed 2026-10-04; ships in `v0.42.0`, cut 2026-10-04 |
| W14a | {plan} | — | Next |
| Track B spikes | — | — | Not started |

## 11. Gate records

### Definition gate, 2026-10-03

Run on branch `0.41` at `56f08ca`.

Sign-off: Signed 2026-10-03.
{records}
## Revisions
"""
MILESTONE = """# W14a

### Step 1
### Step 2
### Step 3

## Open questions
{approval}
## Evidence log

| Step | Verdict |
| --- | --- |
| 1 | pass |
| Step 2 | pass |
"""


class Status(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q", "-b", "0.42")
        self.write("Cargo.toml", '[workspace.package]\nversion = "0.42.0"\n')
        self.write_plan()
        stop = {"group": "a", "level": "B", "src": [], "questions": []}
        self.write(roadmap.CATALOG, json.dumps({"schema": 1, "groups": [{"id": "a"}], "questions": [], "stops": [
            dict(stop, id="s040", row=None, kind="release", release="0.41"),
            dict(stop, id="gdef", row=None, kind="gate", gate="definition gate"),
            dict(stop, id="s0", row="Step 0", kind="custom"),
            dict(stop, id="w14a", row="W14a", kind="release"),
            dict(stop, id="trackb", row="Track B spikes", kind="spike"),
            dict(stop, id="gfl", row=None, kind="gate", gate="glyph gate and the frame-loop gate"),
        ]}))
        self.commit("Update 0.41: the previous release")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        self.fetched(time.time())

    def git(self, *args):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@example.com", "-C", str(self.root), *args],
                              check=True, capture_output=True, text=True).stdout.strip()

    def commit(self, subject):
        self.git("add", "-A")
        self.git("commit", "-q", "--allow-empty", "-m", subject)

    def write(self, rel, text):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def write_plan(self, plan="—", records=""):
        self.write(roadmap.PLAN, PLAN.format(plan=plan, records=records))

    def fetched(self, when):
        path = self.root / ".git/FETCH_HEAD"
        path.write_text("")
        os.utime(path, (when, when))

    def status(self):
        state = roadmap.derive(self.root)
        return state, {s["id"]: s for s in state["stops"]}

    def test_before_the_merge(self):
        state, stops = self.status()
        self.assertEqual({k: v["status"] for k, v in stops.items()},
                         {"s040": "done", "gdef": "done", "s0": "ready", "w14a": "todo", "trackb": "todo", "gfl": "todo"})
        self.assertEqual((stops["gdef"]["release"], stops["s0"]["release"]), ("0.41", "0.42"))
        self.assertEqual((state["release"], state["milestone"], state["stop"]), ("0.42", "M32", "s0"))

    def test_stale_fetch_reports_review_unverified(self):
        self.git("tag", "v0.42.0")
        self.fetched(time.time() - 3600)
        stops = self.status()[1]
        self.assertEqual(stops["s0"]["status"], "review")
        self.assertIn("unverified: fetch older than the tag", stops["s0"]["why"])
        self.fetched(time.time() + 3600)
        stops = self.status()[1]
        self.assertEqual(stops["s0"]["status"], "review")
        self.assertNotIn("unverified", stops["s0"]["why"])

    def test_merge_visible_on_origin_main_is_done(self):
        self.git("tag", "v0.42.0")
        self.commit("Update 0.42: graduation and Step 0 (#42)")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        state, stops = self.status()
        self.assertEqual(stops["s0"]["status"], "done")
        self.assertEqual(state["stop"], "w14a")

    def test_branch_that_is_not_a_release_uses_cargo_plus_one(self):
        self.git("checkout", "-q", "-b", "roadmap-work")
        self.assertEqual(self.status()[0]["release"], "0.43")
        self.git("checkout", "-q", "--detach")
        self.assertEqual(self.status()[0]["release"], "0.43")

    def test_plan_then_approval_line(self):
        name = "phase5-milestone-32-headless-harness.md"
        self.write_plan(plan=f"[{name}]({name})")
        self.write(f"{roadmap.PROGRAM}/{name}", MILESTONE.format(approval=""))
        state, stops = self.status()
        self.assertEqual((stops["w14a"]["status"], stops["w14a"]["milestone"], state["milestone"]), ("plan", "M32", "M33"))
        self.assertEqual(stops["w14a"]["plan"], f"{roadmap.PROGRAM}/{name}")
        self.write(f"{roadmap.PROGRAM}/{name}", MILESTONE.format(approval="\nApproved 2026-10-05 with the stated defaults.\n"))
        stops = self.status()[1]
        self.assertEqual((stops["w14a"]["status"], stops["w14a"]["plan_steps"], stops["w14a"]["evidence"]), ("run", 3, 2))

    def test_named_plan_missing_from_the_tree_stays_todo(self):
        self.write_plan(plan="`phase5-milestone-32-headless-harness.md`")
        stops = self.status()[1]
        self.assertEqual(stops["w14a"]["status"], "todo")
        self.assertIn("not in the tree", stops["w14a"]["why"])

    def test_gate_needs_every_record_signed_and_spike_follows_it(self):
        glyph = "\n### Glyph gate, 2026-11-01\n\nSign-off: Signed 2026-11-01.\n"
        self.write_plan(records=glyph + "\n### Frame-loop gate, 2026-11-01\n\nSign-off: owed\n")
        stops = self.status()[1]
        self.assertEqual((stops["gfl"]["status"], stops["trackb"]["status"]), ("todo", "todo"))
        self.assertIn("frame-loop gate: record unsigned", stops["gfl"]["why"])
        self.write_plan(records=glyph + "\n### Frame-loop gate, 2026-11-01\n\nSign-off: Signed 2026-11-02.\n")
        stops = self.status()[1]
        self.assertEqual((stops["gfl"]["status"], stops["trackb"]["status"]), ("done", "done"))

    def test_catalog_payload_and_hash(self):
        out = self.root / "catalog.json"
        with contextlib.redirect_stdout(io.StringIO()):
            roadmap.main(["--root", str(self.root), "catalog", str(out)])
        payload = json.loads(out.read_text())
        self.assertEqual(sorted(payload), ["catalog", "hash", "schema"])
        self.assertRegex(payload["hash"], r"^[0-9a-f]{16}$")
        self.assertEqual(payload["hash"], roadmap.catalog_payload(self.root)["hash"])
        catalog = payload["catalog"]
        catalog["stops"][0]["title"] = "Changed"
        self.write(roadmap.CATALOG, json.dumps(catalog))
        self.assertNotEqual(payload["hash"], roadmap.catalog_payload(self.root)["hash"])


if __name__ == "__main__":
    unittest.main()
