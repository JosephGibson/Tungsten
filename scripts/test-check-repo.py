#!/usr/bin/env python3
"""Synthetic regression tests for repository QA; no GPU, network or archive reads."""

import importlib.util
import json
import contextlib
import io
import subprocess
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

    def test_list_section_names_no_files(self):
        self.write("assets/manifest.json", '{"sprites":{"hero":{"path":"sprites/hero.png","normal_map":"sprites/normal.png"}},'
                   '"font_families":{"sans":{"faces":["sans"]}},"font_fallback":["sans"]}')
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

    def test_template_assets_are_covered(self):
        self.write("templates/basic/assets/manifest.json", '{"sprites":{"player":{"path":"sprites/player.png"}}}')
        self.write("templates/basic/assets/sprites/player.png", "pixels")
        self.write("templates/basic/assets/fonts/Inter/LICENSE.txt", "license")
        self.assertEqual(self.assets()[0], [])
        self.write("templates/basic/assets/sprites/stray.png", "pixels")
        self.assertIn("unlisted asset: templates/basic/assets/sprites/stray.png", self.assets()[0])

    def templates(self):
        errors, notes = [], []
        qa.check_templates(self.root, errors, notes)
        return errors

    def test_template_pins_the_root_toolchain_and_rust_version(self):
        self.assertEqual(self.templates(), [])  # no templates/ folder
        pin = '[toolchain]\nchannel = "1.98.1"\n'
        self.write("rust-toolchain.toml", pin)
        self.write("Cargo.toml", '[workspace.package]\nrust-version = "1.98.1"\n\n[workspace]\nmembers = []\n')
        self.write("templates/basic/rust-toolchain.toml", pin)
        self.write("templates/basic/Cargo.toml", '[package]\nname = "basic"\nrust-version = "1.98.1"\n')
        self.assertEqual(self.templates(), [])
        self.write("templates/basic/rust-toolchain.toml", pin.replace("1.98.1", "1.99.0"))
        self.write("templates/basic/Cargo.toml", '[package]\nname = "basic"\nrust-version = "1.97"\n')
        self.assertEqual(self.templates(), [
            "templates/basic/rust-toolchain.toml: must equal the root's rust-toolchain.toml",
            "templates/basic/Cargo.toml: rust-version '1.97' differs from the workspace's '1.98.1'"])
        (self.root / "templates/basic/rust-toolchain.toml").unlink()
        self.assertIn("templates/basic/rust-toolchain.toml: must equal the root's rust-toolchain.toml", self.templates())

    def test_directory_symlinks_are_not_walked(self):
        (self.root / "assets/loop").symlink_to(self.root / "assets", target_is_directory=True)
        self.assertEqual(self.assets()[0], [])

    def data_manifest(self, data):
        self.write("assets/manifest.json", '{"sprites":{"hero":{"path":"sprites/hero.png","normal_map":"sprites/normal.png"}},'
                   f'"data":{data}}}')

    def test_data_entries_cover_their_files_two_levels_deep(self):
        self.data_manifest('{"effects":{"burning":{"path":"data/effects/burning.json"}}}')
        self.write("assets/data/effects/burning.json", "{}")
        self.assertEqual(self.assets()[0], [])
        self.write("assets/data/effects/frozen.json", "{}")
        self.assertEqual(self.assets()[0], ["unlisted asset: assets/data/effects/frozen.json"])

    def test_data_entry_missing_file_names_its_kind_and_id(self):
        self.data_manifest('{"effects":{"burning":{"path":"data/effects/burning.json"}}}')
        self.assertEqual(self.assets()[0], [
            "assets/manifest.json: data.effects.burning.path: missing file data/effects/burning.json"])

    def test_data_kind_that_is_not_an_object_fails(self):
        self.data_manifest('{"effects":["burning"]}')
        self.assertEqual(self.assets()[0], [
            "assets/manifest.json: data.effects: a data kind must be an object of entries by ID"])

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

    def test_active_program_links_and_fragments_fail_for_the_destination(self):
        self.write("DECISIONS.md", "## D-001 — test\n")
        for doc in qa.DOCS:
            self.write(doc, "# Doc\n")
        self.write("scripts/check-agent-context.py", "def check(root): return []\n")
        self.write("docs/plans/1.0/README.md", "[plan](work.md#acceptance)\n[missing](gone.md)\n")
        self.write("docs/plans/1.0/work.md", "# Work\n\n## Acceptance\n[bad](#old-checks)\n")
        errors, notes = [], []
        qa.check_docs(self.root, errors, notes)
        self.assertEqual(errors, ["docs/plans/1.0/README.md: broken link gone.md",
                                  "docs/plans/1.0/work.md: broken fragment #old-checks"])

    def test_links_skip_archive_and_template_examples_without_probing_them(self):
        self.write("DECISIONS.md", "## D-001 — test\n")
        for doc in qa.DOCS:
            self.write(doc, "# Doc\n")
        self.write("scripts/check-agent-context.py", "def check(root): return []\n")
        self.write("docs/plans/1.0/README.md", "[history](../archive/unread.md#anything)\n"
                   "```md\n[placeholder](<future file.md>)\n```\n")
        self.write(".claude/skills/tungsten-milestone/skeleton.md", "[rules](../README.md)\n")
        originals = {name: getattr(Path, name) for name in ("exists", "read_text", "glob", "iterdir")}

        def guard(name):
            def guarded(path, *args, **kwargs):
                if "archive" in path.parts:
                    raise AssertionError(f"archive {name}")
                return originals[name](path, *args, **kwargs)
            return guarded

        with patch.object(Path, "exists", guard("exists")), patch.object(Path, "read_text", guard("read_text")), \
                patch.object(Path, "glob", guard("glob")), patch.object(Path, "iterdir", guard("iterdir")):
            errors, notes = [], []
            qa.check_docs(self.root, errors, notes)
        self.assertEqual(errors, [])
        self.assertIn("archive link left unread: docs/plans/1.0/README.md: ../archive/unread.md#anything", notes)

    def test_reference_links_spaces_and_duplicate_heading_fragments(self):
        path = self.write("docs/a.md", "# A\n[space](<with space.md#two-words-1>)\n[ref][dest]\n"
                          "[dest]: with%20space.md#explicit\n")
        self.write("docs/with space.md", '# Two `words`\n## Two words\n<a id="explicit"></a>\n')
        errors, notes = [], []
        qa.check_local_links(self.root, path, path.read_text(), errors, notes, {})
        self.assertEqual(errors, [])


class SummaryChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / qa.roadmap.PROGRAM).mkdir(parents=True)
        (self.root / qa.roadmap.CATALOG).write_text("{}")
        self.state = {"workspace": "0.60.0", "stop": "w8a", "milestone": "M43",
                      "stops": [{"id": "w8a", "status": "todo"}]}
        self.doc = self.root / qa.roadmap.SUMMARY_DOC
        self.doc.write_text(qa.roadmap.summary_text(self.state))

    def check(self, refs=True):
        errors, notes = [], []
        with (patch.object(qa.roadmap, "derive", return_value=self.state),
              patch.object(qa, "has_release_refs", return_value=refs)):
            qa.check_summary(self.root, errors, notes)
        return errors

    def test_current_owned_fields_pass_and_dated_prose_is_not_interpreted(self):
        self.doc.write_text(self.doc.read_text() + "\nHistorical: 0.56 awaits a merge; 2020-01-01.\n")
        self.assertEqual(self.check(), [])

    def test_each_stale_owned_field_fails_with_its_expected_value(self):
        for field, value in qa.roadmap.summary_fields(self.state).items():
            with self.subTest(field=field):
                self.doc.write_text(qa.roadmap.summary_text(self.state).replace(f"`{value}`", "`stale`"))
                errors = self.check()
                self.assertEqual(len(errors), 1)
                self.assertIn(f"stale owned field {field!r}: 'stale', expected {value!r}", errors[0])

    def test_without_release_refs_only_tree_fields_are_compared(self):
        for field, value in qa.roadmap.summary_fields(self.state).items():
            with self.subTest(field=field):
                self.doc.write_text(qa.roadmap.summary_text(self.state).replace(f"`{value}`", "`stale`"))
                self.assertEqual(len(self.check(refs=False)), 0 if field in qa.REF_FIELDS else 1)

    def test_a_tree_outside_git_has_no_release_refs(self):
        self.assertFalse(qa.has_release_refs(self.root))

    def test_missing_duplicate_and_unknown_fields_are_not_accepted(self):
        for corrupt in (qa.roadmap.summary_text(self.state).replace("Workspace", "Unknown"),
                        qa.roadmap.summary_text(self.state).replace("Next milestone", "Workspace"),
                        qa.roadmap.SUMMARY_END + qa.roadmap.summary_text(self.state).replace(qa.roadmap.SUMMARY_END, ""),
                        "# Now\n"):
            self.doc.write_text(corrupt)
            self.assertTrue(self.check())


class Output(unittest.TestCase):
    """Notes are counted unless --notes asks for them; errors always print."""

    def run_main(self, *args):
        output = io.StringIO()
        notes = lambda root, errors, notes: notes.extend(["second note", "first note", "first note"])
        failing = lambda root, errors, notes: errors.append("broken")
        quiet = lambda root, errors, notes: None
        with tempfile.TemporaryDirectory() as temp, contextlib.redirect_stdout(output), \
                patch.object(qa, "check_assets", notes), patch.object(qa, "check_templates", failing), \
                patch.object(qa, "check_plans", quiet), patch.object(qa, "check_docs", quiet), \
                patch.object(qa, "check_plan_citations", quiet), patch.object(qa, "check_roadmap", quiet), \
                patch.object(qa, "check_summary", quiet), patch.object(qa, "check_agent_config", lambda root, errors: None):
            code = qa.main(["--root", temp, *args])
        return code, output.getvalue().splitlines()

    def test_notes_are_counted_by_default(self):
        code, lines = self.run_main()
        self.assertEqual(code, 1)
        self.assertEqual(lines, ["ERROR: broken", "Repository QA: 1 error(s), 2 note(s) (--notes lists them)"])

    def test_notes_flag_lists_them_once_each(self):
        code, lines = self.run_main("--notes")
        self.assertEqual(lines, ["NOTE: first note", "NOTE: second note", "ERROR: broken", "Repository QA: 1 error(s), 2 note(s)"])


class CheckSelection(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def selection(self, *paths):
        return qa.select_checks(self.root, list(paths))

    def test_documentation_only_needs_cpu_qa_without_perf_or_hardware(self):
        commands, review, unknown = self.selection("docs/agent-setup.md")
        self.assertEqual(set(commands), {"just check", "git diff --check", "just ctx", "just repo-check"})
        self.assertEqual((review, unknown), ([], []))
        self.assertFalse(any(c["kind"] == "Hardware" for c in commands.values()))

    def test_generator_inputs_and_generated_outputs_select_consistency_and_routes(self):
        for path in ("examples/01_platformer/tools/level.json", "examples/01_platformer/tools/actors.py",
                     "examples/01_platformer/tools/shaders/soft_glow.wgsl", "examples/01_platformer/src/level_layout.rs"):
            commands, review, unknown = self.selection(path)
            self.assertTrue({"just level-check", "cargo test -p example-01-platformer --locked",
                             "cargo test -p tungsten-core --test manifests --locked", "just smoke"} <= commands.keys())
            self.assertEqual(unknown, [])
        path = self.root / "examples/01_platformer/tools/outputs.json"
        path.parent.mkdir(parents=True)
        path.write_text('["assets/tilemaps/ruin.tmj"]')
        self.assertIn("just level-check", self.selection("examples/01_platformer/assets/tilemaps/ruin.tmj")[0])
        self.assertNotIn("just level-check", self.selection("examples/01_platformer/assets/sounds/black_hole.ogg")[0])
        self.assertNotIn("just level-check", self.selection("examples/01_platformer/tools/README.md")[0])

    def test_solver_needs_release_checks_that_debug_tests_skip(self):
        commands, review, unknown = self.selection("crates/tungsten-core/src/physics/step/solver.rs")
        self.assertTrue({"cargo test -p tungsten-core --locked", "just physics-release", "just smoke"} <= commands.keys())
        self.assertEqual(commands["just physics-release"]["kind"], "CPU release")
        self.assertTrue(any("ignored in debug" in r for r in commands["just physics-release"]["reasons"]))
        self.assertEqual(unknown, [])

    def test_renderer_and_shader_mirror_need_actual_hardware_comparisons(self):
        commands, review, unknown = self.selection("crates/tungsten-render/src/renderer.rs",
                                                 "crates/tungsten-render/src/shaders/stock/bloom.wgsl",
                                                 "assets/shaders/stock/bloom.wgsl")
        self.assertTrue({"cargo test -p tungsten-render --locked", "just smoke", "just visual"} <= commands.keys())
        self.assertEqual(commands["just visual"]["kind"], "Hardware")
        self.assertNotIn("cargo test -p tungsten-render --test shader_coverage --locked", commands)
        self.assertTrue(any("mirror" in r for r in commands["cargo test -p tungsten-render --locked"]["reasons"]))
        self.assertTrue(any("mirrors" in note for note in review))
        self.assertEqual(unknown, [])

    def test_kit_sources_select_their_tests_and_the_umbrellas_harness_tests(self):
        for path in ("crates/tungsten-kit/src/hierarchy.rs", "crates/tungsten-kit/src/tests/sound.rs"):
            with self.subTest(path=path):
                commands, review, unknown = self.selection(path)
                self.assertTrue({"cargo test -p tungsten-kit --locked", "cargo test -p tungsten --locked",
                                 "just smoke"} <= commands.keys())
                self.assertTrue(any("harness" in r for r in commands["cargo test -p tungsten --locked"]["reasons"]))
                self.assertTrue(any("Public API" in note for note in review))
                self.assertEqual(unknown, [])

    def test_mixed_changes_merge_reasons_and_leave_uncertainty_visible(self):
        paths = ["docs/agent-setup.md", "scripts/check-repo.py", "assets/shaders/sprite.wgsl", "unmapped.data"]
        commands, review, unknown = self.selection(*paths)
        self.assertTrue({"just ctx", "just repo-check", "just script-test", "just visual"} <= commands.keys())
        self.assertEqual(len(commands["just repo-check"]["reasons"]), 3)
        self.assertEqual(unknown, ["unmapped.data"])
        output = io.StringIO()
        with contextlib.redirect_stdout(output), patch.object(qa.subprocess, "run", side_effect=AssertionError("executed a check")):
            self.assertEqual(qa.main(["--root", str(self.root), "--select-checks", *paths]), 0)
        text = output.getvalue()
        self.assertIn("no checks executed, no results certified", text)
        self.assertIn("Hardware (not run", text)
        self.assertIn("Visual comparisons are NOT RUN by CPU tests", text)
        self.assertIn("UNCLASSIFIED: unmapped.data", text)
        self.assertNotIn("passed", text)

    def test_manifest_seam_public_api_template_and_workflow_routes(self):
        cases = {
            "assets/manifest.json": {"cargo test -p tungsten-core --test manifests --locked", "just smoke"},
            "crates/tungsten/src/asset_loader/mod.rs": {"cargo test -p tungsten-core --test manifests --locked", "just repo-check"},
            "api/tungsten.txt": {"just api-check"},
            "templates/basic/src/game.rs": {"just template-check", "cargo test -p tungsten-template-basic --locked"},
            ".github/workflows/ci.yml": {"just script-test", "just repo-check"},
            "Cargo.lock": {"just deps", "just smoke"},
        }
        for path, wanted in cases.items():
            with self.subTest(path=path):
                commands, review, unknown = self.selection(path)
                self.assertTrue(wanted <= commands.keys())
                self.assertEqual(unknown, [])

    def test_explicit_deleted_paths_spaces_and_root_boundary(self):
        self.assertEqual(qa.normalize_paths(self.root, ["docs/with space.md", "docs/../docs/gone.md", "docs/gone.md"]),
                         ["docs/gone.md", "docs/with space.md"])
        with self.assertRaisesRegex(ValueError, "outside repository"):
            qa.normalize_paths(self.root, ["../outside.md"])
        self.assertEqual(qa.normalize_paths(self.root, [str(self.root / "docs/a.md")]), ["docs/a.md"])


class GitCheckSelection(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q", "-b", "maintenance")
        self.write("docs/old.md", "old")
        self.write("crates/tungsten-core/src/physics/step/solver.rs", "solver")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "initial fixture")

    def git(self, *args):
        return subprocess.run(["git", "-c", "user.name=fixture", "-c", "user.email=fixture@example.com",
                               "-C", str(self.root), *args], check=True, capture_output=True, text=True).stdout.strip()

    def write(self, rel, text):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def test_git_includes_staged_unstaged_untracked_renames_and_deletions(self):
        self.write("assets/shaders/.keep", "")
        self.git("mv", "docs/old.md", "assets/shaders/new.wgsl")
        solver = "crates/tungsten-core/src/physics/step/solver.rs"
        (self.root / solver).unlink()
        self.write("docs/untracked space.md", "new")
        self.write(".gitignore", "ignored.md\n")
        self.write("ignored.md", "ignored")
        paths = qa.changed_paths(self.root)
        self.assertTrue({"docs/old.md", "assets/shaders/new.wgsl", solver, "docs/untracked space.md"} <= set(paths))
        self.assertNotIn("ignored.md", paths)
        commands, review, unknown = qa.select_checks(self.root, paths)
        self.assertTrue({"just visual", "just physics-release", "just ctx"} <= commands.keys())

    def test_base_adds_committed_work_and_never_fetches_or_writes(self):
        self.write("examples/01_platformer/tools/level.json", "{}")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "generator fixture")
        self.assertEqual(qa.changed_paths(self.root), [])
        before = self.git("status", "--porcelain=v1")
        paths = qa.changed_paths(self.root, "HEAD~1")
        self.assertEqual(paths, ["examples/01_platformer/tools/level.json"])
        self.assertIn("just level-check", qa.select_checks(self.root, paths)[0])
        self.assertEqual(self.git("status", "--porcelain=v1"), before)
        self.assertFalse((self.root / ".git/FETCH_HEAD").exists())
        with self.assertRaises(ValueError):
            qa.changed_paths(self.root, "nonexistent-ref")

    def test_unborn_repository_selects_new_files_without_head(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            subprocess.run(["git", "init", "-q", str(root)], check=True, capture_output=True)
            (root / "new doc.md").write_text("new")
            self.assertEqual(qa.changed_paths(root), ["new doc.md"])


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
