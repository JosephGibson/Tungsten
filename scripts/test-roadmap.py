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

- **status:** {status}

### Step 1
### Step 2
### Step 3

## Open questions

## Evidence log

| Step | Verdict |
| --- | --- |
{rows}"""
README = """# Road to 1.0

## Now

{block}

Prose after the block.
"""
PROMPTS = """# prompts

## Session prompts

| Key | Step | Where | Prompt |
| --- | --- | --- | --- |
| `gate` | Gate | New | `Run the <gate>.` |
| `plan` | Plan | New | `Plan <candidate>.` |
| `qa-plan` | QA plan | New | `Write the QA plan.` |
| `run` | Run | New | `Run <plan>.` |
| `experiment` | Experiment | New | `Run the spike.` |
| `rc-checklist` | Release candidate checklist | New | `Run C3.` |
| `rc-fix` | Release candidate failure | New | `Fix the C3 failure.` |

## Flow

| Stop | Steps: key (where, status) |
| --- | --- |
| Release | `plan` (new, todo) → `run` (new, plan or run) → `ship` (you, ready or review) |
| QA pass | `qa-plan` (new, todo) → `run` (new, plan or run) → `ship` (you, ready or review) |
| Gate | `gate` (new, todo) |
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

    def test_summary_fields_written_at_the_cut_hold_through_the_merge(self):
        # D-152: a cut, tagged or merged stop is closed for the owned fields.
        state, stops = self.status()
        self.assertEqual(stops["s0"]["status"], "ready")
        want = {"Workspace": "0.42.0", "First open stop": "w14a", "Stop state": "todo", "Next milestone": "M32"}
        self.assertEqual(roadmap.summary_fields(state), want)
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            roadmap.main(["--root", str(self.root), "summary"])
        self.assertEqual(output.getvalue(), roadmap.summary_text(state) + "\n")
        self.git("tag", "v0.42.0")
        self.assertEqual(self.status()[1]["s0"]["status"], "review")
        self.assertEqual(roadmap.summary_fields(self.status()[0]), want)
        self.commit("Update 0.42: shipped")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        self.assertEqual(self.status()[1]["s0"]["status"], "done")
        self.assertEqual(roadmap.summary_fields(self.status()[0]), want)

    def test_summary_write_replaces_only_the_owned_block(self):
        self.write(roadmap.SUMMARY_DOC, README.format(block=f"{roadmap.SUMMARY_START}\n- **Workspace:** `old`\n{roadmap.SUMMARY_END}"))
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(roadmap.main(["--root", str(self.root), "summary", "--write"]), 0)
        self.assertEqual((self.root / roadmap.SUMMARY_DOC).read_text(),
                         README.format(block=roadmap.summary_text(self.status()[0])))
        self.write(roadmap.SUMMARY_DOC, "# Road to 1.0\n")
        err = io.StringIO()
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(err):
            self.assertEqual(roadmap.main(["--root", str(self.root), "summary", "--write"]), 1)
        self.assertIn("expected one owned roadmap summary block", err.getvalue())

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

    def test_plan_status_follows_its_run_not_an_approval(self):
        # D-150: planned until the run starts (its status header, or an evidence row).
        name = "phase5-milestone-32-headless-harness.md"
        self.write_plan(plan=f"[{name}]({name})")
        self.write(f"{roadmap.PROGRAM}/{name}", MILESTONE.format(status="draft", rows="")
                   + "\nApproved 2026-10-05 with the stated defaults.\n")
        state, stops = self.status()
        self.assertEqual((stops["w14a"]["status"], stops["w14a"]["milestone"], state["milestone"]), ("plan", "M32", "M33"))
        self.assertEqual(stops["w14a"]["plan"], f"{roadmap.PROGRAM}/{name}")
        self.assertIn("not started, 0 of 3 steps with evidence", stops["w14a"]["why"])
        self.write(f"{roadmap.PROGRAM}/{name}", MILESTONE.format(status="in progress", rows=""))
        self.assertEqual(self.status()[1]["w14a"]["status"], "run")
        self.write(f"{roadmap.PROGRAM}/{name}", MILESTONE.format(status="draft", rows="| 1 | pass |\n| Step 2 | pass |\n"))
        stops = self.status()[1]
        self.assertEqual((stops["w14a"]["status"], stops["w14a"]["plan_steps"], stops["w14a"]["evidence"]), ("run", 3, 2))

    def test_archived_plan_keeps_its_archive_path(self):
        self.write_plan(plan="[M32](../archive/1.0/phase5-milestone-32-headless-harness.md)")
        stops = self.status()[1]
        self.assertEqual(stops["w14a"]["plan"], "docs/plans/archive/1.0/phase5-milestone-32-headless-harness.md")
        self.assertEqual(stops["w14a"]["milestone"], "M32")
        self.assertIn("plan archived", stops["w14a"]["why"])
        self.assertNotIn("not in the tree", stops["w14a"]["why"])

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
        self.assertEqual([s["key"] for s in catalog["stops"][0]["stages"]], ["plan", "run", "ship"])
        self.assertEqual(catalog["sessions"], {"release": {"mode": "auto", "model": "Opus 5.5", "effort": "xhigh"},
                                               "verify": {"mode": "auto", "model": "Opus 5.5", "effort": "high"}})
        self.write(roadmap.PROMPTS, PROMPTS.replace("`run` (new, plan or run) → `ship`", "`run` (new, plan or run; once) → `ship`"))
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
        self.assertEqual(flow["Release"][-1],
                         {"key": "ship", "title": "Ship", "where": "you", "at": ["ready", "review"]})
        self.assertEqual(flow["Release"][1]["at"], ["plan", "run"])
        self.assertEqual(flow["Release candidates (C3)"][1]["title"], "Playthrough")
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
        release = ["plan", "run", "ship"]  # D-150: one row for every level
        for level in ("A", "B", "C", None):
            self.assertEqual(keys(kind="release", level=level), release, level)
        self.assertEqual(keys(kind="qa", level="A"), ["qa-plan", "run", "ship"])
        self.assertEqual(keys(kind="gate", gate="glyph gate and the frame-loop gate"), ["gate"])
        self.assertEqual(keys(kind="spike"), ["experiment"])
        self.assertEqual(keys(kind="rc"), ["rc-checklist", "playthrough", "rc-fix"])
        self.assertEqual(keys(kind="custom"), [])
        with self.assertRaisesRegex(ValueError, "x: no Flow row for kind 'tour'"):
            keys(kind="tour")
        del flow["QA pass"]
        with self.assertRaisesRegex(ValueError, "has no Flow row 'QA pass'"):
            keys(kind="qa")

    def test_tiers_by_stage_and_level(self):
        # D-151: Opus max only for a gate or a level C plan at complexity 2-4; D-136's Fable rule stands.
        cases = [
            ({"complexity": 4, "effort": "S"}, "run", ("fable", "xhigh")),
            ({"complexity": 3, "effort": "XL", "level": "C"}, "plan", ("fable", "xhigh")),
            ({"complexity": 4, "effort": "L", "level": "B"}, "plan", ("opus", "xhigh")),
            ({"complexity": 3, "effort": "XL", "level": "A"}, "run", ("opus", "xhigh")),
            ({"complexity": 3, "effort": "L", "level": "C"}, "plan", ("opus", "max")),
            ({"complexity": 3, "effort": "L", "level": "C"}, "run", ("opus", "xhigh")),
            ({"complexity": 2, "effort": "M", "level": "C"}, "plan", ("opus", "max")),
            ({"complexity": 3, "effort": "L", "level": "B"}, "plan", ("opus", "xhigh")),
            ({"complexity": 2, "effort": "L", "level": None}, "plan", ("opus", "xhigh")),
            ({"complexity": 2, "effort": "L", "level": "A"}, "qa-plan", ("opus", "xhigh")),
            ({"complexity": 2, "effort": "S"}, "gate", ("opus", "max")),
            ({"complexity": 1, "effort": "S"}, "gate", ("opus", "high")),
            ({"complexity": 2, "effort": "M"}, "rc-fix", ("opus", "xhigh")),
            ({"complexity": 1, "effort": "M"}, "plan", ("opus", "xhigh")),
            ({}, "run", ("opus", "xhigh")),
        ]
        for stop, key, want in cases:
            self.assertEqual(roadmap.tier(stop, key), want, (stop, key))
        # D-195: a stop's own tier comes first, for every scaled stage.
        own = {"id": "x", "complexity": 3, "effort": "XL", "level": "C", "tier": {"model": "opus", "effort": "max"}}
        for key in roadmap.SCALED:
            self.assertEqual(roadmap.tier(own, key), ("opus", "max"), key)
        self.assertEqual(roadmap.recommend(own, "run"), {"mode": "auto", "model": "Opus 5.5", "effort": "max"})
        self.assertEqual(roadmap.recommend(own, "verify"), {"mode": "auto", "model": "Opus 5.5", "effort": "high"})
        for bad in ({"model": "haiku", "effort": "max"}, {"model": "opus", "effort": "low"}, {"model": "opus"}, "max"):
            with self.assertRaisesRegex(ValueError, "x: tier"):
                roadmap.tier({**own, "tier": bad}, "plan")
        stop = {"id": "x", "complexity": 4, "effort": "L"}
        rec = lambda key: roadmap.recommend(stop, key)
        self.assertEqual(rec("run"), {"mode": "auto", "model": "Fable 5.1", "effort": "xhigh"})
        self.assertEqual(rec("gate"), {"mode": "auto", "model": "Fable 5.1", "effort": "xhigh"})
        self.assertEqual(rec("rc-checklist"), {"mode": "auto", "model": "Opus 5.5", "effort": "xhigh"})
        for key in ("ship", "playthrough", "compact", "approve", "run-c-rest", "game-spec", "graduate"):
            self.assertIsNone(rec(key), key)

    def test_no_stage_keeps_a_session_or_runs_in_plan_mode(self):
        # D-150: no same-session stages, no plan mode.
        flow = roadmap.flows(self.root)
        for kind in ("release", "qa", "gate", "spike", "rc"):
            for level in ("A", "B", "C", None):
                stop = {"id": "x", "kind": kind, "level": level, "complexity": 3, "effort": "L", "gate": "a gate"}
                for stage in roadmap.stages(stop, flow):
                    self.assertNotEqual(stage["where"], "same", stage)
                    rec = stage.get("rec") or {}
                    self.assertFalse({"keeps", "if_new"} & set(rec), stage)
                    self.assertEqual(rec.get("mode", "auto"), "auto", stage)


class Repository(unittest.TestCase):
    def test_opus_max_only_where_judgment_is_final(self):
        # D-151 on the repository's own catalog: max if and only if a gate or a level C plan, at complexity 2-4,
        # outside D-136's Fable rule; re-rating or carding a stop at a gate keeps this true. A stop with its
        # own tier (D-195) takes it for every scaled stage.
        payload = roadmap.catalog_payload(Path(__file__).resolve().parent.parent)
        for stop in payload["catalog"]["stops"]:
            if "tier" in stop:
                for stage in stop["stages"]:
                    if stage["key"] in roadmap.SCALED:
                        with self.subTest(stop=stop["id"], stage=stage["key"]):
                            own = stop["tier"]
                            self.assertEqual(stage["rec"], {"mode": "auto", "model": roadmap.MODELS[own["model"]],
                                                            "effort": own["effort"]})
                continue
            c, e, level = stop.get("complexity"), stop.get("effort"), stop.get("level")
            fable = (c == 4 or (c == 3 and e == "XL")) and level not in ("A", "B")
            for stage in stop["stages"]:
                if "rec" not in stage:
                    continue
                with self.subTest(stop=stop["id"], stage=stage["key"]):
                    final = stage["key"] == "gate" or (stage["key"] == "plan" and level == "C")
                    want_max = not fable and c in (2, 3, 4) and final
                    self.assertEqual(stage["rec"]["effort"] == "max", want_max)
                    self.assertEqual(stage["rec"]["model"], "Fable 5.1" if fable and stage["key"] in roadmap.SCALED else "Opus 5.5")

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
