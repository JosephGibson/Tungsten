#!/usr/bin/env python3
"""Synthetic regression tests for repository QA; no GPU, network or archive reads."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("repo_check", Path(__file__).with_name("check-repo.py"))
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)


class RepoChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "examples").mkdir()
        self.write("assets/manifest.json", '{"sprites":{"hero":{"path":"sprites/hero.png","normal_map":"sprites/normal.png"}}}')
        self.write("assets/sprites/hero.png", "pixels")
        self.write("assets/sprites/normal.png", "pixels")
        self.write("docs/plans/active.md", "\n".join(f"- **{f}:** {('in progress' if f == 'status' else 'work')}" for f in qa.FIELDS))

    def write(self, rel, text):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    def assets(self):
        errors, notes = [], []
        qa.check_assets(self.root, errors, notes)
        return errors, notes

    def test_asset_coverage_and_missing_sibling(self):
        self.assertEqual(self.assets()[0], [])
        (self.root / "assets/sprites/normal.png").unlink()
        self.assertIn("missing file", " ".join(self.assets()[0]))

    def test_unlisted_backup_is_detected(self):
        self.write("assets/sprites/hero.png~", "backup")
        self.assertIn("unlisted asset", " ".join(self.assets()[0]))

    def test_nested_font_family_exception(self):
        self.write("assets/fonts/Family/unused-weight.ttf", "font")
        self.write("assets/fonts/README.md", "font inventory")
        self.assertEqual(self.assets()[0], [])

    def test_duplicate_json_key_is_not_silently_overwritten(self):
        self.write("assets/manifest.json", '{"sprites":{},"sprites":{}}')
        self.assertIn("duplicate JSON key", " ".join(self.assets()[0]))

    def test_missing_example_manifest_fails(self):
        self.write("examples/05_demo/assets/new.png", "pixels")
        self.assertIn("manifest.json", " ".join(self.assets()[0]))

    def test_directory_instead_of_asset_fails(self):
        self.write("assets/manifest.json", '{"sprites":{"x":{"path":"sprites"}}}')
        self.assertIn("missing file", " ".join(self.assets()[0]))

    def test_malformed_json_shape_fails(self):
        self.write("assets/manifest.json", '[]')
        self.assertTrue(self.assets()[0])

    def test_directory_symlinks_are_not_walked(self):
        (self.root / "assets/loop").symlink_to(self.root / "assets", target_is_directory=True)
        self.assertEqual(self.assets()[0], [])

    def test_active_and_yaml_headers(self):
        errors, notes = [], []
        qa.check_plans(self.root, errors, notes)
        self.assertEqual(errors, [])
        fields = qa.plan_fields('---\nstatus: draft\nfiles to touch:\n  - a.rs\n---\n')
        self.assertEqual(fields["status"], "draft")
        self.assertIn("files to touch", fields)

    def test_done_plan_and_missing_header_fail_without_archive_access(self):
        self.write("docs/plans/active.md", '- **status:** done (finished)\n')
        original = Path.read_text

        def guarded(path, *args, **kwargs):
            if "archive" in path.parts:
                raise AssertionError("archive read")
            return original(path, *args, **kwargs)

        with patch.object(Path, "read_text", guarded):
            errors, notes = [], []
            qa.check_plans(self.root, errors, notes)
        self.assertTrue(any("belongs in" in e for e in errors))
        self.assertTrue(any("missing plan header" in e for e in errors))

    def test_program_folder_plans_are_checked_and_archive_is_not_listed(self):
        self.write("docs/plans/1.0/README.md", "# Folder map, not a plan\n")
        self.write("docs/plans/1.0/finished.md", "\n".join(f"- **{f}:** {('done' if f == 'status' else 'work')}" for f in qa.FIELDS))
        self.write("docs/plans/1.0/headless.md", "- **status:** draft\n")
        self.write("docs/plans/1.0/deeper/nested.md", "not a plan\n")
        self.write("docs/plans/archive/old.md", "- **status:** done\n")
        self.write("docs/plans/archive/1.0/old.md", "- **status:** done\n")
        original_iterdir, original_glob = Path.iterdir, Path.glob

        def guarded_iterdir(path):
            if "archive" in path.parts:
                raise AssertionError("archive listed")
            return original_iterdir(path)

        def guarded_glob(path, pattern):
            if "archive" in path.parts or "**" in pattern:
                raise AssertionError("archive or recursive glob")
            return original_glob(path, pattern)

        with patch.object(Path, "iterdir", guarded_iterdir), patch.object(Path, "glob", guarded_glob):
            errors, notes = [], []
            qa.check_plans(self.root, errors, notes)
        self.assertIn("docs/plans/1.0/finished.md: done plan belongs in docs/plans/archive/1.0/", errors)
        self.assertTrue(any(e.startswith("docs/plans/1.0/headless.md: missing plan header") for e in errors))
        self.assertFalse(any("README" in e or "nested" in e or "archive/old" in e for e in errors))
        self.assertEqual(len(errors), 6)  # finished.md once, headless.md's five missing headers

    def test_agent_config_detects_filter_drift(self):
        self.write(".claude/settings.json", json.dumps({"env": {"CLAUDE_CODE_GLOB_NO_IGNORE": "false"}, "permissions": {"deny": ["Read(./docs/plans/archive/**)"]}}))
        self.write(".ignore", "docs/plans/archive/\n")
        errors = []
        qa.check_agent_config(self.root, errors)
        self.assertEqual(errors, [])
        self.write(".ignore", "target/\n")
        qa.check_agent_config(self.root, errors)
        self.assertTrue(errors)

    def test_agent_config_rejects_blanket_execution_allow(self):
        self.write(".claude/settings.json", json.dumps({"env": {"CLAUDE_CODE_GLOB_NO_IGNORE": "false"}, "permissions": {"deny": ["Read(./docs/plans/archive/**)"], "allow": ["Bash(just *)"]}}))
        self.write(".ignore", "docs/plans/archive/\n")
        errors = []
        qa.check_agent_config(self.root, errors)
        self.assertTrue(any("exact commands" in e for e in errors))

    def test_plan_citations_flag_missing_plans_but_not_archive_or_history(self):
        self.write("docs/plans/live.md", "plan")
        self.write("crates/demo/src/lib.rs", "// docs/plans/live.md, then docs/plans/gone.md.\n")
        self.write("DESIGN.md", "Archived at `docs/plans/archive/old.md`.\n")
        self.write("CHANGELOG.md", "Shipped docs/plans/gone.md.\n")
        self.write("scripts/test-demo.py", "FIXTURE = 'docs/plans/fixture.md'\n")
        original = Path.read_text

        def guarded(path, *args, **kwargs):
            if "archive" in path.parts:
                raise AssertionError("archive read")
            return original(path, *args, **kwargs)

        with patch.object(Path, "read_text", guarded):
            errors, notes = [], []
            qa.check_plan_citations(self.root, errors, notes)
        self.assertEqual(errors, ["crates/demo/src/lib.rs: cites missing plan docs/plans/gone.md"])
        self.assertIn("archive citation left unread: DESIGN.md: docs/plans/archive/old.md", notes)

    def test_docs_find_unknown_decisions_and_broken_links_skip_archive(self):
        self.write("DECISIONS.md", "## D-001 — test\n")
        for doc in qa.DOCS:
            self.write(doc, "# Doc\n")
        self.write("README.md", "[missing](missing.md) D-999\n[history](docs/plans/archive/unread.md)\n")
        self.write("scripts/check-agent-context.py", "def check(root): return []\n")
        errors, notes = [], []
        qa.check_docs(self.root, errors, notes)
        self.assertEqual(len(errors), 2)
        self.assertTrue(any("archive link left unread" in n for n in notes))


ROADMAP_PLAN = """# Plan

## 3. Phase 5

| Candidate | Contents |
| --- | --- |
| W14a | Harness |

| Candidate | Touches | Level |
| --- | --- | --- |
| W14a | `app.rs` | B |
| W9a | `release.yml` | B |
| W1 glyph gate, M2, M3 | w01 | S, then C |

## 7. Questions by when

| Question (criteria §10) | Needed by |
| --- | --- |
| Q14 logs on by default | W11a starts |

## 10. Register

| Candidate | Milestone plan | Release | Status |
| --- | --- | --- | --- |
| W14a | — | — | Next |
| W9a with W12a | — | — | Not started |
| W1 M2, M3 | — | — | Wait |

## 11. Gate records
"""


class RoadmapChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.write("docs/plans/1.0/implementation-plan.md", ROADMAP_PLAN)
        self.write("docs/plans/1.0/criteria.md", "## 10. Questions\n\n- **14.** Logs?\n\n### Answered\n\n- **19.** Text.\n\n## 11. Next\n")
        self.write("docs/plans/1.0/w14-tooling.md", "plan")
        (self.root / "perf-runs").mkdir()
        stop = {"group": "a", "kind": "release", "level": "B", "src": [], "questions": []}
        self.catalog = {
            "schema": 1, "groups": [{"id": "a", "phase": 5}],
            "stops": [
                dict(stop, id="gdef", row=None, kind="gate", level=None, reads=["perf-runs/ (verdicts)"]),
                dict(stop, id="w14a", row="W14a", src=["docs/plans/1.0/w14-tooling.md"], questions=["q14"]),
                dict(stop, id="w9a", row="W9a with W12a"),
                dict(stop, id="m2", row="W1 M2, M3", level="C"),
                dict(stop, id="m3", row="W1 M2, M3", level="C"),
            ],
            "questions": [{"id": "q14"}, {"id": "q19"}],
        }

    def write(self, rel, text):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def check(self):
        self.write("docs/plans/1.0/roadmap.json", json.dumps(self.catalog))
        errors, notes = [], []
        qa.check_roadmap(self.root, errors, notes)
        return errors

    def stop(self, sid):
        return next(s for s in self.catalog["stops"] if s["id"] == sid)

    def test_catalog_matching_the_register_passes(self):
        self.assertEqual(self.check(), [])

    def test_rows_out_of_order_fail(self):
        stops = self.catalog["stops"]
        stops[1], stops[2] = stops[2], stops[1]
        self.assertEqual(self.check(), ["docs/plans/1.0/roadmap.json: row 1 is 'W9a with W12a', the register's (implementation plan §10) is 'W14a'"])

    def test_missing_row_fails(self):
        self.catalog["stops"] = [s for s in self.catalog["stops"] if s["id"] != "w9a"]
        self.assertIn("row 2 is 'W1 M2, M3'", " ".join(self.check()))

    def test_register_split_fails_until_the_catalog_splits(self):
        self.write("docs/plans/1.0/implementation-plan.md", ROADMAP_PLAN.replace("| W1 M2, M3 | — | — | Wait |", "| W1 M2 | — | — | Wait |\n| W1 M3 | — | — | Wait |"))
        self.assertIn("row 3 is 'W1 M2, M3'", " ".join(self.check()))
        self.stop("m2")["row"], self.stop("m3")["row"] = "W1 M2", "W1 M3"
        self.assertEqual(self.check(), [])

    def test_level_disagreeing_with_its_card_fails(self):
        self.stop("w14a")["level"] = "C"
        self.assertEqual(self.check(), ["docs/plans/1.0/roadmap.json: w14a: level 'C', its §3 card says 'B'"])

    def test_missing_src_and_reads_paths_fail(self):
        self.stop("w14a")["src"] = ["docs/plans/1.0/w99-gone.md §2"]
        self.stop("gdef")["reads"] = ["docs/plans/1.0/w14-tooling.md/ (a folder)"]
        errors = self.check()
        self.assertIn("docs/plans/1.0/roadmap.json: w14a: missing path docs/plans/1.0/w99-gone.md", errors)
        self.assertIn("docs/plans/1.0/roadmap.json: gdef: missing path docs/plans/1.0/w14-tooling.md/", errors)

    def test_unknown_question_ids_fail(self):
        self.catalog["questions"].append({"id": "q99"})
        self.stop("w9a")["questions"] = ["q15"]
        errors = self.check()
        self.assertIn("docs/plans/1.0/roadmap.json: question q99 is not in implementation plan §7 or criteria §10", errors)
        self.assertIn("docs/plans/1.0/roadmap.json: w9a: unknown question q15", errors)

    def test_duplicate_and_malformed_ids_fail(self):
        self.stop("m3")["id"] = "m2"
        self.catalog["questions"].append({"id": "Q14"})
        errors = self.check()
        self.assertIn("docs/plans/1.0/roadmap.json: duplicate id m2", errors)
        self.assertIn("docs/plans/1.0/roadmap.json: id 'Q14' is not lowercase letters and digits", errors)

    def test_duplicate_key_and_unknown_group_fail(self):
        self.stop("w9a")["group"] = "b"
        self.assertEqual(self.check(), ["docs/plans/1.0/roadmap.json: w9a: unknown group b"])
        self.write("docs/plans/1.0/roadmap.json", '{"stops": [], "stops": []}')
        errors, notes = [], []
        qa.check_roadmap(self.root, errors, notes)
        self.assertIn("duplicate JSON key", " ".join(errors))


if __name__ == "__main__":
    unittest.main()
