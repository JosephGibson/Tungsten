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

    def test_tracked_deletion_candidate_is_reported_and_kept(self):
        candidate = next(iter(qa.DELETION_CANDIDATES))
        path = self.write(candidate, "pixels")
        self.write(str(Path(candidate).parent.parent / "manifest.json"), '{}')
        errors, notes = self.assets()
        self.assertEqual(errors, [])
        self.assertTrue(any("deletion candidate" in n for n in notes))
        self.assertTrue(path.is_file())

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


if __name__ == "__main__":
    unittest.main()
