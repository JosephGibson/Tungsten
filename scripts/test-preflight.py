#!/usr/bin/env python3
"""scripts/preflight.py on a fixture plan and tree in a temporary folder; no Git, cargo or network."""

import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

TOOL = Path(__file__).with_name("preflight.py")
spec = importlib.util.spec_from_file_location("preflight", TOOL)
preflight = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preflight)

PLAN = """# Gadgets (gadgets) — in progress

- **status:** in progress
- **goal:** A gadget widget, then its data kind.
- **non-goals:** none.
- **files to touch:** (kit paths under `crates/tungsten-kit/src/`, its tests under `crates/tungsten-kit/src/tests/`)
  - Step 1: `widgets/gadget.rs`; the `seams` fixture (§2.1): `examples/03_fixtures/src/seams.rs`, `examples/03_fixtures/tests/seams.rs`.
  - Step 2: `foundation/plugins.rs`, `lib.rs`, any `examples/01_platformer/tests/game/*.rs` the rule in §5 admits, the `seams` fixture's files, the follow-ups' homes.
- **ordered steps:** (1) The gadget. (2) Its data kind.
- **done-when:** the tests pass.

## 2. Design

### 2.1 The gadget (Step 1)

`GadgetState` holds a size.

### 2.2 The kind (Step 2)

The `widgets` kind registers in `KitPlugins::set()`.

## 3. Steps

### Step 1: The gadget

- **Files:** Step 1's list in the header.
- **Change:** 2.1.

### Step 2: Its data kind

- **Files:** Step 2's list in the header.
- **Change:** 2.2.
"""

# A plan whose names the tree holds nowhere and whose list feeds no pin and fires no rule.
QUIET = """# Quiet (quiet) — draft

- **status:** draft
- **files to touch:**
  - Step 1: `docs/quiet.md`.
- **ordered steps:** (1) Write it.
- **done-when:** it reads well.

## 3. Steps

### Step 1: Write it

- **Change:** `BrandNewThing` and `brand_new_thing`, which nothing names yet.
"""

# Its list is a document, and its text cites a kit source: a cite alone keys no pin.
CITING = QUIET.replace("which nothing names yet.", "beside `crates/tungsten-kit/src/widgets/gadget.rs:1`.")

TREE = {
    # In the plan's lists: excluded wherever they name it.
    "crates/tungsten-kit/src/widgets/gadget.rs": "pub struct GadgetState {\n    pub size: f32,\n}\n",
    "crates/tungsten-kit/src/foundation/plugins.rs": "pub struct KitPlugins;\n",
    "crates/tungsten-kit/src/lib.rs": "pub use widgets::gadget::GadgetState;\n",
    "crates/tungsten-core/src/lib.rs": "pub mod lighting;\n",
    "crates/tungsten-core/src/lighting.rs": "pub fn light_at() {}\n",
    "examples/03_fixtures/src/seams.rs": "pub struct Seams;\n",
    "examples/03_fixtures/tests/seams.rs": "use fixtures::Seams;\n",
    # Outside them: one file a group.
    "crates/tungsten-kit/src/tests/gadget_size.rs": "fn pinned() {\n    assert_eq!(std::mem::size_of::<GadgetState>(), 4);\n}\n",
    "crates/tungsten/src/tests/kit_recipes/coverage.rs": "fn every_kind_is_counted() {\n    assert_eq!(kinds.len(), 3);\n}\n",
    "crates/tungsten-kit/src/tests/describe.rs": "// GadgetState's description\nfn census() {\n    assert_eq!(kinds.len(), 3);\n}\n",
    "examples/01_platformer/src/game.rs": "use tungsten::kit::GadgetState;\n",
    "examples/01_platformer/tests/game/guards/pit.json": '{\n "allocated_bytes": 100,\n "allocations": 3\n}\n',
    "examples/01_platformer/tests/game/ball_pit.rs": "// The pit's walls hold a GadgetState.\n",
    "templates/basic/src/game.rs": "// The template's GadgetState\n",
    "docs/notes.md": "A note on `GadgetState`.\n",
    "docs/quiet.md": "Nothing yet.\n",
    # Never searched.
    "target/debug/GadgetState.txt": "GadgetState\n",
    "perf-runs/old/GadgetState.md": "GadgetState\n",
    "docs/plans/archive/old.md": "GadgetState\n",
}


class Preflight(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        plans = {"docs/plans/gadgets.md": PLAN, "docs/plans/quiet.md": QUIET}
        for rel, text in {**TREE, **plans}.items():
            path = self.root / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)

    def run_tool(self, plan="docs/plans/gadgets.md", *args, env=None):
        result = subprocess.run(["python3", "-B", str(TOOL), str(self.root / plan), "--root", str(self.root), *args],
                                capture_output=True, text=True, env={**os.environ, **(env or {})})
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout

    def report(self, *args, plan="docs/plans/gadgets.md"):
        return json.loads(self.run_tool(plan, "--json", *args))

    def files(self, report):
        return {entry["path"]: entry for entry in report["files"]}

    def test_each_file_outside_the_list_lands_in_its_group(self):
        groups = {path: entry["group"] for path, entry in self.files(self.report()).items()}
        self.assertEqual(groups, {
            "crates/tungsten-kit/src/tests/gadget_size.rs": "guard pins",
            "examples/01_platformer/tests/game/guards/pit.json": "guard pins",
            "crates/tungsten/src/tests/kit_recipes/coverage.rs": "coverage counts",
            "crates/tungsten-kit/src/tests/describe.rs": "describe and genre tests",
            "examples/01_platformer/src/game.rs": "01's names",
            "examples/01_platformer/tests/game/ball_pit.rs": "01's names",
            "templates/basic/src/game.rs": "the template",
            "docs/notes.md": "the rest",
        })

    def test_the_lists_files_and_the_skipped_folders_are_left_out(self):
        report = self.report()
        listed = set(self.files(report))
        for path in ("crates/tungsten-kit/src/widgets/gadget.rs", "crates/tungsten-kit/src/lib.rs",
                     "crates/tungsten-kit/src/foundation/plugins.rs", "examples/03_fixtures/tests/seams.rs",
                     "docs/plans/gadgets.md", "target/debug/GadgetState.txt", "perf-runs/old/GadgetState.md",
                     "docs/plans/archive/old.md"):
            self.assertNotIn(path, listed)
        self.assertEqual(report["files_to_touch"]["files"], [
            "crates/tungsten-kit/src/foundation/plugins.rs", "crates/tungsten-kit/src/lib.rs",
            "crates/tungsten-kit/src/widgets/gadget.rs", "examples/03_fixtures/src/seams.rs",
            "examples/03_fixtures/tests/seams.rs"])

    def test_a_name_hit_gives_its_path_line_and_text(self):
        out = self.run_tool()
        self.assertIn("names searched (1): `GadgetState` 6\n", out)
        self.assertIn("  examples/01_platformer/src/game.rs:1  use tungsten::kit::GadgetState;  [`GadgetState`]", out)
        self.assertIn("  crates/tungsten-kit/src/tests/gadget_size.rs:2  assert_eq!(std::mem::size_of::<GadgetState>(), 4);"
                      "  [R4, `GadgetState`]", out)
        self.assertIn("8 files outside the list: 6 by name, 1 by rule, 3 by the pin table, 2 of them by more than one.",
                      out)

    def test_the_pin_table_lists_derived_values_that_name_nothing_from_the_plan(self):
        report = self.report()
        files = self.files(report)
        coverage = files["crates/tungsten/src/tests/kit_recipes/coverage.rs"]
        self.assertEqual((coverage["by"], coverage["names"], coverage["pins"]), (["pin"], [], ["P4"]))
        self.assertEqual([(hit["line"], hit["text"]) for hit in coverage["hits"]], [(2, "assert_eq!(kinds.len(), 3);")])
        pit = files["examples/01_platformer/tests/game/guards/pit.json"]
        self.assertEqual(([hit["line"] for hit in pit["hits"]], pit["pins"]), ([2, 3], ["P1"]))
        fired = {pin["id"]: pin for pin in report["pins"]["fired"]}
        self.assertEqual(fired["P4"]["why"], ["step 1: touches crates/tungsten-kit/src/widgets/gadget.rs (kit)",
                                              "step 2: touches crates/tungsten-kit/src/foundation/plugins.rs (kit)"])
        self.assertIn("coverage.rs:85", fired["P4"]["stops"])
        # No fixture or recipe in the lists: the genre fixtures' P6 stays quiet.
        self.assertEqual(sorted(fired), ["P1", "P3", "P4", "P5"])
        out = self.run_tool()
        self.assertIn("  P4 the recipes' counts and hash: step 1: touches crates/tungsten-kit/src/widgets/gadget.rs (kit);"
                      " step 2: touches crates/tungsten-kit/src/foundation/plugins.rs (kit) -> 1 files", out)
        self.assertIn("  crates/tungsten/src/tests/kit_recipes/coverage.rs:2  assert_eq!(kinds.len(), 3);  [P4]", out)

    def test_a_cited_source_alone_feeds_no_pin(self):
        elsewhere = tempfile.TemporaryDirectory()
        self.addCleanup(elsewhere.cleanup)
        plan = Path(elsewhere.name) / "citing.md"
        plan.write_text(CITING)
        citing = json.loads(subprocess.run(["python3", "-B", str(TOOL), str(plan), "--root", str(self.root), "--json"],
                                           capture_output=True, text=True, check=True).stdout)
        self.assertEqual(citing["pins"]["fired"], [])
        self.assertNotIn("crates/tungsten/src/tests/kit_recipes/coverage.rs", self.files(citing))

    def test_a_step_takes_its_own_names_rules_and_list(self):
        step1 = self.report("--step", "1")
        files1 = self.files(step1)
        # lib.rs is in step 2's list, not step 1's: it names step 1's type.
        self.assertEqual(files1["crates/tungsten-kit/src/lib.rs"]["names"], ["GadgetState"])
        self.assertEqual([rule["id"] for rule in step1["rules"]["fired"]], ["R4", "R10"])
        self.assertEqual(step1["files_to_touch"]["unresolved"], [])
        step2 = self.report("--step", "2")
        self.assertEqual(step2["files_to_touch"]["files"][:2],
                         ["crates/tungsten-kit/src/foundation/plugins.rs", "crates/tungsten-kit/src/lib.rs"])
        self.assertNotIn("examples/01_platformer/src/game.rs", self.files(step2))
        self.assertNotIn("R4", [rule["id"] for rule in step2["rules"]["fired"]])

    def test_json_marks_each_hit_by_its_name_its_rule_or_its_pin(self):
        files = self.files(self.report())
        describe = files["crates/tungsten-kit/src/tests/describe.rs"]
        self.assertEqual(describe["by"], ["name", "pin"])
        by_line = {hit["line"]: hit["sources"] for hit in describe["hits"]}
        self.assertEqual(by_line[1], [{"source": "name", "name": "GadgetState", "kind": "type"}])
        self.assertEqual([(s["source"], s["pin"]) for s in by_line[3]], [("pin", "P5")])
        size = files["crates/tungsten-kit/src/tests/gadget_size.rs"]
        self.assertEqual((size["by"], size["rules"]), (["name", "rule"], ["R4"]))
        tables = self.report()
        self.assertEqual(sorted({rule["id"] for rule in tables["rules"]["table"]}),
                         ["R10", "R15", "R20", "R4", "R6", "R7", "R8", "R9"])
        self.assertEqual(sorted(tables["pins"]["table"][0]), ["feeds", "holds", "id", "name", "stops"])

    def test_an_entry_that_resolves_to_no_path_is_reported_not_dropped(self):
        touch = self.report("--step", "2")["files_to_touch"]
        self.assertEqual(touch["unresolved"], ["the follow-ups' homes"])
        # Another step's "the `seams` fixture (§2.1): …" resolves "the `seams` fixture's files".
        self.assertEqual(touch["aliases"], [{"entry": "the `seams` fixture's files", "name": "seams",
                                             "paths": ["examples/03_fixtures/src/seams.rs",
                                                       "examples/03_fixtures/tests/seams.rs"]}])
        out = self.run_tool("docs/plans/gadgets.md", "--step", "2")
        self.assertIn("list entries that resolve to no path (1): the follow-ups' homes", out)
        self.assertIn("list entry from another step's list: the `seams` fixture's files -> "
                      "examples/03_fixtures/src/seams.rs, examples/03_fixtures/tests/seams.rs", out)

    def test_a_conditional_entry_excludes_nothing_and_marks_its_files(self):
        entry = "any `examples/01_platformer/tests/game/*.rs` the rule in §5 admits"
        report = self.report()
        self.assertEqual(report["files_to_touch"]["conditional"],
                         [{"entry": entry, "paths": ["examples/01_platformer/tests/game/*.rs"]}])
        pit = self.files(report)["examples/01_platformer/tests/game/ball_pit.rs"]
        self.assertEqual((pit["names"], pit["conditional"]), (["GadgetState"], entry))
        self.assertIsNone(self.files(report)["examples/01_platformer/src/game.rs"]["conditional"])
        out = self.run_tool()
        self.assertIn("conditional list entries, which exclude nothing (1): " + entry, out)
        self.assertIn("  examples/01_platformer/tests/game/ball_pit.rs:1  // The pit's walls hold a GadgetState."
                      f"  [`GadgetState`]  (conditional entry: {entry})", out)

    def test_a_plan_with_no_hits(self):
        report = self.report(plan="docs/plans/quiet.md")
        self.assertEqual((report["files"], report["rules"]["fired"], report["pins"]["fired"]), ([], [], []))
        self.assertEqual(sorted(name["name"] for name in report["names"]["absent"]), ["BrandNewThing", "brand_new_thing"])
        out = self.run_tool("docs/plans/quiet.md")
        self.assertTrue(out.endswith("no files outside the list name what the plan changes.\n"), out)

    def test_it_writes_nothing(self):
        before = {p: p.stat().st_mtime_ns for p in self.root.rglob("*")}
        self.run_tool()
        self.run_tool("docs/plans/gadgets.md", "--json", "--step", "2")
        self.assertEqual({p: p.stat().st_mtime_ns for p in self.root.rglob("*")}, before)

    @unittest.skipUnless(shutil.which("rg"), "rg not installed")
    def test_rg_and_the_pure_python_search_agree(self):
        self.assertEqual(self.run_tool("docs/plans/gadgets.md", "--json", env={"PREFLIGHT_RG": "0"}),
                         self.run_tool("docs/plans/gadgets.md", "--json", env={"PREFLIGHT_RG": "1"}))

    def test_an_unknown_step_fails_with_the_steps_named(self):
        result = subprocess.run(["python3", "-B", str(TOOL), str(self.root / "docs/plans/gadgets.md"),
                                 "--root", str(self.root), "--step", "9"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn("has no step 9 (steps: 1, 2)", result.stderr)

    def test_list_forms_resolve_against_the_headers_bases(self):
        resolver = preflight.Resolver(preflight.Plan("docs/plans/x.md", PLAN), preflight.Corpus(self.root))
        touch = resolver.resolve_list("`widgets/{gadget.rs,spring.rs (new)}`; kit tests `describe.rs`, "
                                      "`examples/01_platformer/src/`, `docs/*.md`, `docs/notes.md:1`")
        self.assertEqual(touch.summary()["files"], ["crates/tungsten-kit/src/tests/describe.rs",
                                                    "crates/tungsten-kit/src/widgets/gadget.rs", "docs/notes.md"])
        self.assertEqual(touch.summary()["new"], ["crates/tungsten-kit/src/widgets/spring.rs"])
        self.assertEqual(touch.summary()["folders"], ["examples/01_platformer/src/"])
        self.assertEqual(touch.summary()["globs"], ["docs/*.md"])
        self.assertTrue(touch.covers("docs/quiet.md"))

    def test_an_ambiguous_or_new_path_goes_to_its_groups_crate(self):
        # Core is defined first, but the plan's unqualified paths are the kit's.
        plan = preflight.Plan("docs/plans/x.md", "- **files to touch:** (core paths under `crates/tungsten-core/src/`; "
                              "kit paths under `crates/tungsten-kit/src/`)\n"
                              "  - Step 1: `lighting.rs`, `lib.rs`.\n"
                              "  - Step 2: `fresh/{mod.rs,queue.rs}` (new), `lib.rs`; `widgets/gadget.rs`.\n")
        resolver = preflight.Resolver(plan, preflight.Corpus(self.root))
        step1 = resolver.resolve_list("`lighting.rs`, `lib.rs`").summary()
        self.assertEqual(step1["files"], ["crates/tungsten-core/src/lib.rs", "crates/tungsten-core/src/lighting.rs"])
        step2 = resolver.resolve_list("`fresh/{mod.rs,queue.rs}` (new), `lib.rs`; `widgets/gadget.rs`").summary()
        self.assertEqual(step2["files"], ["crates/tungsten-kit/src/lib.rs", "crates/tungsten-kit/src/widgets/gadget.rs"])
        self.assertEqual(step2["new"], ["crates/tungsten-kit/src/fresh/mod.rs", "crates/tungsten-kit/src/fresh/queue.rs"])


if __name__ == "__main__":
    unittest.main()
