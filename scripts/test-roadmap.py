#!/usr/bin/env python3
"""Fixture-tree tests for scripts/roadmap.py: catalog payload, stages, session recommendations and
status derivation (no network), plus the repository's own catalog."""

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
PROMPTS = """# prompts

## Session prompts

| Key | Step | Where | Prompt |
| --- | --- | --- | --- |
| `gate` | Gate | New | `Run the <gate>.` |
| `graduate` | Graduation | New | `Graduate <W>.` |
| `plan` | Plan | New | `Plan <candidate>.` |
| `qa-plan` | QA plan | New | `Write the QA plan.` |
| `approve` | Approve a plan | Same | `Approved.` |
| `run` | Run, levels A and B | New | `Run <plan>.` |
| `run-c1` | Run, level C, step 1 | New | `Run step 1.` |
| `run-c-rest` | Run, level C, the rest | Same | `API approved.` |
| `game-spec` | Game spec | New | `Agree the spec.` |
| `experiment` | Experiment | New | `Run the spike.` |
| `rc-checklist` | Release candidate checklist | New | `Run C3.` |
| `rc-fix` | Release candidate failure | New | `Fix the C3 failure.` |

## Flow

| Stop | Steps: key (where, status) |
| --- | --- |
| Release, level A or B | `plan` (new, todo) → `approve` (same, plan) → `run` (new, run) → `ship` (you, ready or review) |
| Release, level C | `plan` (new, todo) → `approve` (same, plan) → `run-c1` (new, run) → `run-c-rest` (same, run) → `ship` (you, ready or review) |
| QA pass | `qa-plan` (new, todo) → `approve` (same, plan) → `run` (new, run) → `ship` (you, ready or review) |
| Gate | `game-spec` (new, todo; the frame-loop gate only, until the pitch is agreed) → `gate` (new, todo) → `graduate` (new, todo; one per workstream the record graduates) |
| Experiments (Track B) | `experiment` (new, todo; one per spike) |
| Release candidates (C3) | `rc-checklist` (new, todo) → `playthrough` (you, run) → `rc-fix` (new, run; after a failure, then `rc-checklist` again) |

## Owner steps

**Ship** (`ship`), one sitting after the run session ends.

**Playthrough** (`playthrough`). Play the release candidate.
"""


class Status(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q", "-b", "0.42")
        self.write("Cargo.toml", '[workspace.package]\nversion = "0.42.0"\n')
        self.write_plan()
        self.write(roadmap.PROMPTS, PROMPTS)
        stop = {"group": "a", "level": "B", "src": [], "questions": []}
        self.write(roadmap.CATALOG, json.dumps({"schema": roadmap.SCHEMA, "groups": [{"id": "a"}], "questions": [], "stops": [
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
        self.assertEqual((payload["schema"], payload["catalog"]["schema"]), (2, 2))
        self.assertRegex(payload["hash"], r"^[0-9a-f]{16}$")
        self.assertEqual(payload["hash"], roadmap.catalog_payload(self.root)["hash"])
        catalog = payload["catalog"]
        self.assertEqual([s["key"] for s in catalog["stops"][0]["stages"]], ["plan", "approve", "run", "ship"])
        self.assertEqual(catalog["sessions"], {"release": {"mode": "auto", "model": "Opus 5.5", "effort": "xhigh"},
                                               "verify": {"mode": "auto", "model": "Opus 5.5", "effort": "high"}})
        self.write(roadmap.PROMPTS, PROMPTS.replace("`run` (new, run) → `ship`", "`run` (new, run; once) → `ship`"))
        self.assertNotEqual(payload["hash"], roadmap.catalog_payload(self.root)["hash"])
        self.write(roadmap.PROMPTS, PROMPTS)
        source = json.loads((self.root / roadmap.CATALOG).read_text())
        source["stops"][0]["title"] = "Changed"
        self.write(roadmap.CATALOG, json.dumps(source))
        self.assertNotEqual(payload["hash"], roadmap.catalog_payload(self.root)["hash"])
        self.write(roadmap.CATALOG, json.dumps(dict(source, schema=1)))
        with self.assertRaisesRegex(ValueError, "schema 1"):
            roadmap.catalog_payload(self.root)

    def test_flow_table_steps(self):
        flow = roadmap.flows(self.root)
        self.assertEqual(flow["Release, level A or B"][-1],
                         {"key": "ship", "title": "Ship", "where": "you", "at": ["ready", "review"]})
        self.assertEqual(flow["Release candidates (C3)"][1]["title"], "Playthrough")
        self.assertEqual(flow["Gate"][0]["note"], "the frame-loop gate only, until the pitch is agreed")
        self.assertEqual(flow["Release candidates (C3)"][2]["note"], "after a failure, then `rc-checklist` again")
        self.write(roadmap.PROMPTS, PROMPTS.replace("`gate` (new, todo)", "`gate` (later)"))
        with self.assertRaisesRegex(ValueError, "Flow row 'Gate': can't read"):
            roadmap.flows(self.root)
        self.write(roadmap.PROMPTS, PROMPTS.replace("| `qa-plan` | QA plan | New | `Write the QA plan.` |\n", ""))
        with self.assertRaisesRegex(ValueError, "no Step title for 'qa-plan'"):
            roadmap.flows(self.root)
        self.write(roadmap.PROMPTS, PROMPTS.replace("| Experiments (Track B) | `experiment` (new, todo; one per spike) |", "| Experiments (Track B) |"))
        with self.assertRaisesRegex(ValueError, "fewer than two cells"):
            roadmap.flows(self.root)

    def test_stages_by_kind_and_level(self):
        flow = roadmap.flows(self.root)
        keys = lambda **stop: [s["key"] for s in roadmap.stages(dict({"id": "x"}, **stop), flow)]
        ab, c = ["plan", "approve", "run", "ship"], ["plan", "approve", "run-c1", "run-c-rest", "ship"]
        self.assertEqual((keys(kind="release", level="B"), keys(kind="release", level=None)), (ab, ab))
        self.assertEqual(keys(kind="release", level="C"), c)
        self.assertEqual(keys(kind="qa", level="A"), ["qa-plan", "approve", "run", "ship"])
        self.assertEqual(keys(kind="gate", gate="glyph gate and the frame-loop gate"), ["game-spec", "gate", "graduate"])
        self.assertEqual(keys(kind="gate", gate="definition gate"), ["gate", "graduate"])
        self.assertEqual(keys(kind="spike"), ["experiment"])
        self.assertEqual(keys(kind="rc"), ["rc-checklist", "playthrough", "rc-fix"])
        self.assertEqual(keys(kind="custom"), [])
        with self.assertRaisesRegex(ValueError, "x: no Flow row for kind 'tour'"):
            keys(kind="tour")
        del flow["QA pass"]
        with self.assertRaisesRegex(ValueError, "has no Flow row 'QA pass'"):
            keys(kind="qa")

    def test_recommendations(self):
        tiers = {(4, "S"): ("fable", "xhigh"), (3, "XL"): ("fable", "xhigh"), (3, "L"): ("opus", "max"),
                 (2, "XL"): ("opus", "max"), (2, "M"): ("opus", "max"), (1, "S"): ("opus", "high"),
                 (1, "M"): ("opus", "xhigh"), (None, None): ("opus", "xhigh")}
        for (c, e), want in tiers.items():
            self.assertEqual(roadmap.tier({"complexity": c, "effort": e}), want, (c, e))
        for level, want in {"A": ("opus", "max"), "B": ("opus", "max"), "C": ("fable", "xhigh"), None: ("fable", "xhigh")}.items():
            stop = {"complexity": 4, "effort": "L", "level": level}  # D-136: level A and B releases take Opus
            self.assertEqual(roadmap.tier(stop), want, level)
        self.assertEqual(roadmap.tier({"complexity": 3, "effort": "XL", "level": "B"}), ("opus", "max"))
        stop = {"id": "x", "complexity": 4, "effort": "L"}
        rec = lambda key: roadmap.recommend(stop, key)
        self.assertEqual(rec("run"), {"mode": "auto", "model": "Fable 5.1", "effort": "xhigh"})
        self.assertEqual(rec("gate"), {"mode": "plan", "model": "Fable 5.1", "effort": "xhigh"})
        self.assertEqual(rec("game-spec"), {"mode": "plan", "model": "Opus 5.5", "effort": "max"})
        self.assertEqual(rec("graduate"), {"mode": "auto", "model": "Opus 5.5", "effort": "xhigh"})
        self.assertEqual(rec("rc-checklist"), {"mode": "auto", "model": "Opus 5.5", "effort": "xhigh"})
        self.assertEqual((rec("ship"), rec("playthrough"), rec("compact")), (None, None, None))
        with self.assertRaisesRegex(ValueError, "approve has no earlier plan or qa-plan step"):
            rec("approve")

    def test_same_session_steps_keep_their_session(self):
        flow = roadmap.flows(self.root)
        stop = {"id": "x", "kind": "release", "level": "C", "complexity": 3, "effort": "XL"}
        recs = {s["key"]: s.get("rec") for s in roadmap.stages(stop, flow)}
        fable = {"mode": "auto", "model": "Fable 5.1", "effort": "xhigh"}
        self.assertEqual(recs["approve"], {"keeps": "plan", **fable,
                                           "if_new": {"mode": "auto", "model": "Opus 5.5", "effort": "high"}})
        self.assertEqual(recs["run-c-rest"], {"keeps": "run-c1", **fable, "if_new": fable})
        qa = {s["key"]: s.get("rec") for s in roadmap.stages({"id": "q", "kind": "qa", "complexity": 2}, flow)}
        self.assertEqual(qa["approve"]["keeps"], "qa-plan")


class Repository(unittest.TestCase):
    def test_every_stop_derives_its_stages(self):
        payload = roadmap.catalog_payload(Path(__file__).resolve().parent.parent)
        for stop in payload["catalog"]["stops"]:
            with self.subTest(stop=stop["id"]):
                self.assertEqual(bool(stop["stages"]), stop["kind"] != "custom")
                for stage in stop["stages"]:
                    self.assertEqual("rec" in stage, stage["where"] != "you", stage["key"])
        size = len(json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode())
        self.assertLess(size, 256 * 1024)  # one db document, roadmap.md


if __name__ == "__main__":
    unittest.main()
