# Test audit (`test-audit`) — in progress

- **status:** in progress
- **goal:** cut every low-value test in the workspace and fill its major coverage gaps without losing the coverage or the guarantees the suite gives today: each cut carries its evidence (a kept test covering the same lines and assertions, or why its assertion proves nothing), no library line or region covered at the baseline goes uncovered, and the top ten CPU-reachable uncovered branches by risk get tests
- **non-goals:** no count target; no library code changes (test code only, and the helpers and imports a cut leaves dead); no new tests in `examples/`; no tree change for a coverage recipe or gate (a lasting one is one Later line); pixel comparisons, GPU smoke, audio callbacks and windows stay out of the baseline, the gates and the gaps (reported as not run, or listed as not CPU-coverable), the close's `just template-check` aside (§2.1); protected tests (§2.3) stay even where low-value, reported as skipped; the Python script tests are reviewed and, being protected, not cut; no CI, `justfile` or tooling change; no decision (cargo-llvm-cov is a local tool, not a dependency)
- **files to touch:** Step 2: `crates/tungsten-core/src/tests/`, `crates/tungsten-core/tests/`. Step 3: `crates/tungsten/src/tests/`, `crates/tungsten/tests/`, `crates/tungsten/src/testing/guard.rs` (its test module only). Step 4: `crates/tungsten-kit/src/tests/`. Step 5: `crates/tungsten-render/src/tests/`, `crates/tungsten-render/tests/`, `crates/tungsten-render/src/counters.rs` (its test module only), `tools/cli/src/tests/`, `tools/cli/tests/`, `tools/crash-probe/tests/`, `tools/launcher/src/main.rs` (its test module only). Step 6: `examples/01_platformer/tests/`, `examples/02_bench/src/tests/`, `examples/02_bench/tests/`, `examples/03_fixtures/tests/`, `examples/03_fixtures/src/post.rs`, `examples/03_fixtures/src/transition/mod.rs`, `examples/03_fixtures/src/transition/states.rs` (their test modules only), `templates/basic/tests/`. Steps 2–6: `docs/LLM_INDEX.md` only when a test file it names is emptied and deleted, and a `#[cfg(test)]` helper in the crate's library files only when a cut leaves it unused. Step 7: test files under the folders of Steps 2–5 (existing files; a new file only beside its `mod` line's parent there). Step 8: `ROADMAP.md`, this plan (deleted)
- **ordered steps:** (1) review: nine read-only reviewers return rows; (2)–(6) cut core, the umbrella, the kit, render and the tools, then the examples and the template, each gated by coverage and a fresh-context verifier and committed; (7) gaps: the top ten filled, a commit per crate; (8) close
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes, and `just template-check` if `templates/basic/` changed; the close's before-and-after report (tests, test lines, nextest wall time, coverage per crate, cuts by category, protected rows skipped, verifier reverts, gaps filled, spend) in the last evidence row; one Later line for a lasting coverage recipe; the plan deleted and `test-audit` out of Now

Item `test-audit`, off-roadmap, from the owner's prompt of 2026-11-26 (planned and run in this session). Written 2026-11-26 at `5d1aaeca`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

The workspace's suite has grown with every Phase 7 plan to 5,363 tests in 317 files, 224,108 lines (47 % of the workspace's Rust), 58 s under nextest; nothing measured what it covers. The owner asked for one session that measures, plans, reviews, cuts and fills, at most 15 % of the weekly meter (`perf-runs/713-chain/spend.py`, checked at each phase boundary), with a workflow of at most 20 agents.

It stands on `D-226` (counter guards run in `just check`, so guards are the suite's perf checks), `D-070` (CPU CI; GPU, audio and perf stay local), `D-172` (the kit's recipe census in `kit_recipes/coverage.rs` is its admission rule), `D-312`'s fix rule (a failure names its answer, so a merged table test's failure names its case), `D-140` and `D-215` (one plan file, one critique round at xhigh) and `D-216` (push the version branch after each commit). Nothing later waits on it; Phase 8 inherits the suite. Machine-local records go to `perf-runs/20261126-test-audit/` (summaries and scripts) and `target/test-audit/` (coverage sets and logs).

## 1. Audit

Read at `5d1aaeca`. Tooling: cargo-llvm-cov 0.9.2 (installed with `cargo install --locked`, a local tool), cargo-nextest 0.9.133, toolchain 1.98.1 with `llvm-tools`. Disk before: 77 GB free of 468 GB (`target/` 133 GB); the coverage target, `target/llvm-cov-target/` with `CARGO_INCREMENTAL=0`, took 6 GB.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | 5,363 `#[test]` fns in 317 files: core 1,869 (94 files), umbrella 1,221 (57), kit 1,084 (73), example 01 512 (30), render 276 (27), template 187 (10), CLI 126 (12), 03 43 (8), 02 33 (4), crash probe 7, launcher 5; 25 ignored (core 24, 02 1); two render tests share a name (`text.rs`, `text_nodes.rs`), which the inventory's time column folds into one | `perf-runs/20261126-test-audit/inventory/tests.tsv`, `files.tsv` |
| A2 | 224,315 test lines: 223,768 in test files under `tests/` folders, 547 in six inline `#[cfg(test)]` modules (`tungsten/src/testing/guard.rs` 338, `03_fixtures/src/transition/states.rs` 70, `launcher/src/main.rs` 68, `render/src/counters.rs` 29, `03_fixtures/src/post.rs` and `transition/mod.rs` 21 each, spans by `rows.py`'s lexer); 473,764 lines of Rust in all | `inventory/inline-spans.tsv` |
| A3 | nextest at HEAD: 5,338 run, 5,338 passed, 25 skipped; 58.3 s run (66.9 s with its build check) on 12 threads; 638.8 s of test time summed, 345.5 s of it in the 118 tests over 1 s; the slowest are the physics workers' lockstep tests (19.5, 12.4, 12.0 s), 01's full-pit chain bound (15.3 s) and forked burning pit (13.8 s), and the kit's spread flood bound (11.1 s) | `target/nextest/audit/junit-base.xml`, `inventory/tests.tsv` |
| A4 | Coverage of library code, run 1 (`cargo llvm-cov --workspace`, 5,338 passed, 7 min 16 s with its build; doctests, which need nightly under coverage, not in it): 114,662 of 133,794 lines (85.7 %) and 191,083 of 221,575 regions (86.2 %) in 428 files, after `guard.rs`'s test module tail was refiltered out (critique 1.3). Per crate, lines then regions: core 95.7 / 95.5 %, kit 95.1 / 94.4, umbrella 84.0 / 83.8, 01 88.9 / 86.9, render 36.4 / 43.1, 02 83.6 / 82.9, CLI 80.8 / 78.4, 03 28.6 / 30.8, template 96.6 / 96.6, launcher 31.9 / 26.8, crash probe 83.3 / 86.5 | `perf-runs/20261126-test-audit/coverage-base-1.tsv` |
| A5 | cargo-llvm-cov's default filter drops every file under a `tests/`, `examples/` or `benches/` folder, which hides the demo games' own code (`examples/*/src/`); the audit's export lifts it (`--no-default-ignore-filename-regex`) and keeps only workspace files | `cov.py` `snap`; cargo-llvm-cov's `src/report.rs:944-950` |
| A6 | Run 2, one package's tests at a time (5,338 passed, 13 min 29 s with eleven exports): 114,679 lines and 191,096 regions. Covered in both runs, the gate's `base`: 114,656 lines and 191,077 regions (85.7 % and 86.2 %, per crate as A4 to 0.1 %); 54 flaky entries covered in one run only (29 lines, 25 regions: `physics/events.rs` 16, 01's `tomes/components.rs` 12, the kit's `impact.rs`, `cap.rs`, `effects.rs` and `defense.rs` 6 each, `step/pool.rs` 2), outside the gate | `perf-runs/20261126-test-audit/coverage-base.tsv`, `target/test-audit/base.sets.json`, `flaky.tsv` |
| A7 | 458 test names appear in `DECISIONS.md`, 20 in guard pins (`guards.rs`, `guards/*.json`), 3 in `scripts/preflight.py`, 1 in `docs/validation.md`, 76 in other docs; 6 in other `.rs` files | `inventory/named.tsv` |
| A8 | 67 pairs of test bodies at a token ratio of 0.8 or more within a crate (string literals masked), 28 at 0.95: table-test candidates, not cuts by themselves | `inventory/similar.tsv` |
| A9 | Python script tests: 228 `def test_` in nine `scripts/test-*.py` (4,226 lines) and 47 in `examples/01_platformer/tools/test_generate.py` (744 lines), plus `scripts/test-smoke-examples.sh`; protected (§2.3) | `justfile` `script-test`, `level-check` |
| A10 | Inline test modules are the only test code inside library files; every other test sits in a `tests/` folder file, included by a `#[cfg(test)] mod` or as an integration target, so a cut leaves library line numbers unchanged outside those six files | A2 |

## 2. Design

### 2.1 Measurement and the gate

The scripts are machine-local, so they are quoted, not linked: `perf-runs/20261126-test-audit/cov.py`, `rows.py` and `slices.py`.

- **Library code** is every workspace member's `.rs` file outside a `tests/` or `benches/` folder, a crate's own `examples/` folder and `build.rs`, minus the inline `#[cfg(test)] mod` spans (braces matched by `rows.py`'s lexer, which masks comments, strings, raw strings and chars across lines). Example and template sources count: their tests cover them.
- **Sets.** `cov.py snap <name>` exports the current llvm-cov data as lcov (lines) and JSON (regions: code regions keyed by file, start and end, unioned over monomorphizations) and keeps the covered and coverable sets in `target/test-audit/<name>.sets.json`. `cov.py run <name> <pkg>…` cleans the profraw, runs only those packages' tests (`--exclude-from-test` for the rest, so every workspace file is still reported) and snaps; a run in which no test passed is never snapped, since a report with no fresh profraw reuses the last merged profdata.
- **Baseline.** `base` is what run 1 (the workspace) and run 2 (the union of eleven per-package runs) both cover (`cov.py stable`); entries covered in one run only are flaky and outside the gate. Run 2's per-package sets are each package's current set.
- **Gate.** After a step's cuts, `cov.py step <label> <pkg>…` reruns that step's packages, makes each one's new set its current set (`target/test-audit/current.json`), joins every package's current set and checks with `cov.py gate base cur-<label>` that every library line and region in `base` is covered, mapping lines through `git show <base>:<file>`'s diff for the six files with inline tests. Any loss prints `file:line`; exit 1. `current.json` moves to the new sets only when every run passed and the gate held, so a rejected or skipped step leaves the next step's gate on the last accepted sets. A loss is rerun once (flaky past run 2), then traced to the cut rows whose tests reached it, which are reverted.
- **Times.** `cargo nextest run --workspace --locked --profile audit --config-file <scratch>/nextest.toml` with a JUnit report (`fail-fast = false`), at the baseline and the close, on a quiet machine. Pixel comparisons and GPU smoke never run in the baseline or the gates: reported as not run. The one hardware check is the close's `just template-check`, which the owner's prompt asks for when `templates/basic/` changed; it builds, smoke-runs and packages a `tungsten new` game on the display (`:0` here), and its result is reported as that.

### 2.2 Low-value tests

A test is low-value when one of these holds and its row says which, with evidence:

| Category | Holds when | Evidence the row carries |
| --- | --- | --- |
| `duplicate` | another kept test runs the same library code on the same inputs and asserts the same | the kept test (`path::fn`), the shared calls and assertions |
| `subsumed` | a stronger kept test in the same crate reaches the same lines and asserts everything this one asserts, or more | the kept test, what it adds |
| `tautology` | the assertion cannot fail while the test compiles (a value against itself, a literal round-trip through no engine code) | why it cannot fail |
| `restated` | it restates a constant or a `Default`'s literal without a behaviour that reads it | the restated item; the tests that cover the code reading it |
| `dependency` | it tests std, a serde derive with no custom code, or a dependency's own behaviour | the derive or call; that no engine code is between |
| `near-identical` | two or more tests differ only in their inputs | the tests, what varies; they merge into one table test whose failure names its case (`D-312`'s fix rule) |
| `removed` | it tests behaviour the engine no longer has (an alias nothing reads, a path no caller reaches) | the removal (commit or decision) |

A test is not low-value for being long, old or slow. A slow test with a unique guarantee stays.

### 2.3 Protected tests

Protected, unless the decision behind it is superseded (its `docs/DECISION_INDEX.md` row says so, and the row cites the superseding entry): any test named by a decision, `docs/validation.md`, `scripts/preflight.py`'s pin tables or a guard pin; counter guards and perf pins (`size_of` asserts, allocation and byte pins, state hashes, bounded-work tests such as "stays under its bound"); save, migration and format tests (a format is a file the engine reads or writes: saves, the manifest and its asset files, rooms, prefabs, scenes, screen and theme files, `tungsten.json`, `input.json`, settings; a format test asserts what such a file reads or writes as, its versions and migrations, or its errors on bad input); shader coverage (`crates/tungsten-render/tests/shader_coverage.rs`); the kit's recipe census (`kit_recipes/coverage.rs`, `D-172`), the per-genre recipe tests it admits by (each genre's copy of a recipe is the admission rule's evidence, never another genre's duplicate) and the tests at the lines preflight's P1–P9 pin; repo-check and script tests (the Python suites, `manifests::` and `decision_index::`); `#[ignore]` tests run by hand. Kept as well: tests whose real check needs the GPU (pixel comparisons, GPU smoke), since their CPU run takes a skip path the gate cannot judge; and, so no maintained text goes stale, a test named in any other tracked doc, script or `.rs` comment outside its own file (`CHANGELOG.md` aside, which is history). Each candidate's name is looked up in `inventory/named.tsv` (built from every tracked file) and grepped in those places before it is called low-value; a protected candidate becomes a `protected` row, counted at the close as skipped.

### 2.4 Rows

Each reviewer writes `perf-runs/20261126-test-audit/rows/<slice>.jsonl`, one JSON object a line: `cut` (`test` as `path::fn`, `category`, `kept`, `evidence`), `merge` (`tests`, `into`, `cases`, `evidence`), `protected` (`test`, `category`, `protected_by`, `evidence`) and `gap` (`file`, `lines`, `fn`, `branching`, `risk`, `cpu`, `home`, `sketch`). `rows.py check` refuses a row whose test or kept test does not exist, a `duplicate` or `subsumed` row with no kept test, a test in two rows, and a cut or merge of a name `named.tsv` holds in any column (decisions, validation, preflight, guard pins, docs, scripts and other `.rs` files; `CHANGELOG.md` aside). `rows.py apply <slice>…` deletes each cut row's fn with its attributes and doc comment (a lexer-aware span, checked against all 5,363 tests at HEAD); merges are written by hand. `rows.py removed <commit>` lists the test fns gone since a commit, which must equal the step's cut and merged rows.

### 2.5 Slices and the review

Nine reviewers, read-only (they write only their own rows file), each with a data pack in `perf-runs/20261126-test-audit/slices/<slice>/` (`files.tsv`, `tests.tsv`, `named.tsv`, `similar.tsv` and `uncovered.tsv`, the slice's library lines uncovered at `base` and its uncovered regions on covered lines, such as an error closure on a line that ran, each with its enclosing fn):

| Slice | Tests | Test lines | Test files |
| --- | --- | --- | --- |
| `core-physics` | 380 | 23,372 | core's `src/tests/physics/`, `tests/integration/physics_*`, `src/tests/workers.rs` |
| `core-assets-saves` | 577 | 22,147 | core's `src/tests/{assets,save}/`, `save.rs`, `persist.rs`, the other integration files |
| `core-runtime` | 912 | 27,769 | the rest of core: ECS, UI, input, config, schedule, time, script and the small modules |
| `tungsten-content` | 505 | 25,035 | the umbrella's `kit_recipes/`, `kit.rs`, `level_editor.rs`, `prefabs.rs`, `rooms.rs`, `saves.rs`, `save_slots.rs`, `asset_loader.rs`, `check.rs`, `script.rs`, `describe.rs`, `hot_reload.rs`, `plugins.rs` |
| `tungsten-runtime` | 716 | 24,753 | the rest of the umbrella, its integration tests and `testing/guard.rs`'s module |
| `kit-a` | 537 | 24,592 | the kit's `src/tests/` files `ability.rs` to `hit.rs` |
| `kit-b` | 547 | 21,389 | the kit's files `impact.rs` to `zone.rs` |
| `render-tools` | 414 + 275 Python | 15,101 + 4,970 | render, the CLI, the crash probe, the launcher, `scripts/test-*` |
| `examples-template` | 775 | 41,945 | `examples/`, `templates/basic/` |

Each reviewer reads every test of its slice, at least enough to know what it asserts, and the library code it reaches; it returns one row per candidate, protected candidates included, and its slice's gap rows: CPU-reachable library code with real branching that no test covers, ranked by risk, with GPU, audio-callback and window code marked `cpu: no`. Reviewers run no cargo command (the target and the tree are shared) and run at xhigh (the operating rules' tiers).

### 2.6 Cuts, the verifier and reverts

One crate or group at a time, in the main tree, on one coverage target and the workspace's `target/debug`: `rows.py apply`, the merges by hand, `cargo clippy -p <pkg> --all-targets --locked -- -D warnings` (dead helpers and imports removed), the gate, then a fresh-context verifier. The verifier gets the step's diff, its rows and the gate's output, reads every removed test against its row (the kept test exists and asserts what the cut one did on the same code; a "proves nothing" reason holds; a merge keeps every case's inputs and assertions and names its case), checks no protected name went, and returns a verdict per row. An unsupported row's test is put back from `HEAD` (`git show HEAD:<file>`, the fn reinserted where it stood) and its row marked reverted; the gate reruns after any revert. A step commits only after its verifier and reverts, so `HEAD` is the step's base throughout: the diff, `rows.py removed HEAD` and every restore read it. Five verifiers: core, the umbrella, the kit, render and the tools, the examples and the template.

### 2.7 Gaps

A major gap is CPU-reachable library code with real branching (a branch, an error path, a loop exit), uncovered as lines or as regions on a covered line, that no test covers. Eligibility comes first: a row is a candidate only when its home is a test file of an engine crate or tool (core, the umbrella, the kit, render, the CLI, the crash probe, the launcher); example and template rows are reported separately as excluded, since no test goes in `examples/` and the template is game code its own sessions own. The candidates are ranked by risk: saves and migrations, ECS storage and queries, physics, the asset registry and loaders, config parsing, kit systems, then the rest; within a class by the size of the uncovered branching. The top ten CPU-coverable rows are filled, or as many as the budget allows, by at most five writers, one a crate, in parallel: each edits only its crate's existing test files, builds through `target/debug` (the cargo lock compiles them one at a time), keeps tests cross-platform (`Path::join`, no Unix tools, no `python3` without a Windows ignore) and names each case in its failure. No test goes in `examples/`. GPU, audio-callback and window rows go on the close's "not CPU-coverable" list. Each filled row's lines and the regions starting on them must read covered in the crate's rerun (`cov.py covers`).

### 2.8 Agents and budget

At most 19 agents: nine reviewers, five verifiers, up to five writers, each at xhigh. `python3 -B perf-runs/713-chain/spend.py --since 2026-11-26T07:18:21Z` reads the session's spend at each phase boundary (after the plan's commit, the review, each cut step and the gaps); it read 35.3 % of the week used at the session's start. So the ceiling holds, 2 % is reserved for the close: no phase starts past 13 %, the spend is read before each agent batch and every 15 minutes while the review runs (the workflow is stopped past 10 % and the rows written so far are used), and the writers are sized at about 0.5 % each against what remains under 13 %. Past 13 % the step in flight finishes and commits, the unreached steps are listed, and Step 8 runs.

## 3. Steps

### Step 1: Review

- **Files:** none in the tree; `perf-runs/20261126-test-audit/rows/<slice>.jsonl`
- **Change:** one workflow of nine reviewers over §2.5's slices; then `rows.py check` and the rows' counts
- **Done-when:** `python3 -B perf-runs/20261126-test-audit/rows.py check` → exit 0 (a row it refuses is fixed against the tree or dropped, and counted); `rows.py stats` → the counts by kind and category, quoted; the spend read
- **Commit:** none (rows are machine-local; the evidence row rides Step 2's commit)
- **Moves:** nothing

### Step 2: Cut core

- **Files:** `crates/tungsten-core/src/tests/`, `crates/tungsten-core/tests/`
- **Change:** the three core slices' cut and merge rows, then §2.6
- **Done-when:** `cargo fmt --all -- --check` → exit 0; `cargo clippy -p tungsten-core --all-targets --locked -- -D warnings` → exit 0; `cargo test -p tungsten-core --locked` → exit 0; `python3 -B perf-runs/20261126-test-audit/cov.py step cut-core tungsten-core` → exit 0 (core's tests pass under coverage; every other package keeps its current set; lost 0 lines and 0 regions against `base`); `rows.py removed HEAD` → exactly the step's applied cut and merged tests; the verifier → every remaining row supported
- **Commit:** `test(core): cut low-value tests and merge near-identical ones`
- **Moves:** test counts and lines only

### Step 3: Cut the umbrella

- **Files:** `crates/tungsten/src/tests/`, `crates/tungsten/tests/`, `crates/tungsten/src/testing/guard.rs` (its test module)
- **Change:** `tungsten-content` and `tungsten-runtime`'s rows, then §2.6
- **Done-when:** as Step 2's, for package `tungsten` (`cov.py step cut-umbrella tungsten`)
- **Commit:** `test(tungsten): cut low-value tests and merge near-identical ones`
- **Moves:** test counts and lines only

### Step 4: Cut the kit

- **Files:** `crates/tungsten-kit/src/tests/`
- **Change:** `kit-a` and `kit-b`'s rows, then §2.6
- **Done-when:** as Step 2's, for package `tungsten-kit` (`cov.py step cut-kit tungsten-kit`)
- **Commit:** `test(kit): cut low-value tests and merge near-identical ones`
- **Moves:** test counts and lines only

### Step 5: Cut render and the tools

- **Files:** `crates/tungsten-render/src/tests/`, `crates/tungsten-render/tests/`, `crates/tungsten-render/src/counters.rs` (its test module), `tools/cli/src/tests/`, `tools/cli/tests/`, `tools/crash-probe/tests/`, `tools/launcher/src/main.rs` (its test module)
- **Change:** `render-tools`' rows, then §2.6 once over the group, then a commit per package with cuts
- **Done-when:** as Step 2's, for each of `tungsten-render`, `tungsten-cli`, `tungsten-crash-probe` and `tungsten-launcher` with cuts (`cov.py step cut-tools <those packages>`); no script changes, so no `just script-test`
- **Commit:** `test(render): …`, `test(cli): …` as each has cuts
- **Moves:** test counts and lines only

### Step 6: Cut the examples and the template

- **Files:** `examples/01_platformer/tests/`, `examples/02_bench/src/tests/`, `examples/02_bench/tests/`, `examples/03_fixtures/tests/`, the test modules of `examples/03_fixtures/src/post.rs`, `src/transition/mod.rs` and `src/transition/states.rs`, `templates/basic/tests/`
- **Change:** `examples-template`'s rows, then §2.6 once over the group, then a commit per package with cuts
- **Done-when:** as Step 2's, for each of `example-01-platformer`, `example-02-bench`, `example-03-fixtures` and `tungsten-template-basic` with cuts (`cov.py step cut-games <those packages>`); `cargo test -p tungsten-cli --locked` → exit 0 when `templates/basic/` changed (the CLI embeds the template)
- **Commit:** `test(example-01): …`, `test(template): …` as each has cuts
- **Moves:** test counts and lines only

### Step 7: Gaps

- **Files:** existing test files under Steps 2–5's folders (the engine crates and tools), named per gap in the evidence row
- **Change:** §2.7: the top ten gap rows by risk, filled by up to five writers, one a crate, in one workflow; then each crate's checks
- **Done-when:** per crate with gaps: `cargo fmt --all -- --check`, `cargo clippy -p <pkg> --all-targets --locked -- -D warnings` and `cargo test -p <pkg> --locked` → exit 0; `cov.py step gaps <the packages with gaps>` → exit 0; `cov.py covers cur-gaps <file>:<a>-<b>…` → exit 0, each filled row's lines covered
- **Commit:** `test(<crate>): cover <what>`, one a crate
- **Moves:** test counts and coverage up

### Step 8: Close

- **Files:** this plan (deleted), `ROADMAP.md`
- **Change:** the checks, the fresh nextest and coverage runs and the report; a Later line for a lasting coverage recipe; the plan deleted and `test-audit` out of Now
- **Done-when:** `just check` → exit 0; `just template-check` → exit 0 when `templates/basic/` changed (the one hardware check, §2.1); a fresh `cargo llvm-cov --workspace --locked --no-report --no-fail-fast` → 0 failed, then `cov.py snap final` and `cov.py gate base final` → exit 0; the nextest run → 0 failed, its wall time quoted; `just repo-check` → exit 0
- **Ends with:** the commits, the evidence, what the owner should try, the next prompt

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Does a format test that duplicates another format test exactly count as protected? | Yes: protected as written; the row is `protected`, counted as skipped |
| Q2 | A test named only in `CHANGELOG.md`? | Not protected: the changelog is history |
| Q3 | A whole test file emptied by cuts? | Deleted with its `mod` line, unless a doc or script outside the test tree names it (then its last test stays) |
| Q4 | A merged table test's position and name? | Where the first merged test stood; a name for what the cases share |

## 5. Invariants

- **May move:** test counts, test lines, nextest wall time (Steps 2–7); coverage up (Step 7)
- **Must not move:** library code (no non-test line changes, checked by `git diff --stat` per step against the files to touch); `base`'s covered lines and regions (the gate); counter guards, pins, hashes, snapshots and reference images (protected, untouched)

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it; `current.json` already names the last accepted sets (§2.1).
- The session's spend reaches 15 % of the week: finish the step in flight, commit, list the unreached steps and run Step 8.
- Disk under 20 GB free: delete this session's coverage target (`target/llvm-cov-target/`) and rebuild one package at a time.

## 7. Critique

Round 1: `critique --model gpt-6-astra --effort xhigh --raw --repo` on the draft over `5d1aaeca`, 2026-11-26 (`gpt-6.2-astra` skipped, refused on this account since 2026-11-18). Footer: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 364.1 s, tokens in 621896 (cached 548864), out 9321 (reasoning 5278), commands 22, output 272.6 KB (largest 30.2 KB), usage 5h 0% → 17%, week 42% → 45%. One round, as the owner's prompt set; findings still open would become open questions.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1.1 | Steps 5 and 6 commit per package before the verifier, whose diff and restores read `HEAD` | `brief-verify.md`, `rows.py` `restore` (reads `HEAD`) | Accepted | §2.6: every step commits only after its verifier and reverts, so `HEAD` is the step's base throughout; Steps 5 and 6 commit after §2.6 |
| 1.2 | Gap lists from uncovered lines miss uncovered regions on covered lines, such as `save.rs:752`'s migration-error closure | `base.sets.json`: line 752 covered, regions `752:50:752:59` and `752:60:752:75` not | Accepted | `slices.py gaps` adds `regions` rows (uncovered regions on covered lines: 2,959 over the nine slices); §2.5, §2.7 and Step 7's `cov.py covers` check regions too |
| 1.3 | The inline-test span parser counted braces line by line, so `guard.rs:458`'s multi-line raw string ended the module early and its tail counted as library | `cov.py`'s per-line `strip`; `guard.rs`'s module spans 330–667 | Accepted | `test_spans` matches braces on the whole text through `rows.py`'s lexer; `cov.py refilter` dropped the tail from every set (451 entries a set, 902 where its tests ran), the baseline rebuilt; A2, A4 and A6 restated |
| 1.4 | `cov.py step` wrote `current.json` before the gate, so a rejected step's sets fed the next gate | `cov.py` `step` | Accepted | `step` is transactional: the state moves only when every run passed and the gate held, a rejected candidate kept as `current-<label>-rejected.json`; §2.1, §6 |
| 1.5 | `rows.py check` ignored `named.tsv`'s Rust references, which §2.3 protects | `rows.py` `protected_reason`; `render/src/tests/timing.rs:19` cites a test by name | Accepted | `protected_reason` includes the `rs_elsewhere` column; §2.4 |
| 1.6 | The ranking took gaps whose homes Step 7 forbids (example 01's save migrations) | §2.7 against Step 7's files | Accepted | Eligibility before ranking: homes in the engine crates and tools only; example and template rows reported as excluded; §2.7, Step 7's files |
| 1.7 | The 15 % ceiling was read only at phase ends, with nothing reserved for the close | §2.8 | Accepted | 2 % reserved for the close: no phase starts past 13 %, a reading before each agent batch and every 15 minutes during the review (stopped past 10 %), writers sized against the rest |
| 1.8 | `just template-check` runs a GPU smoke the plan said never runs | `scripts/template-check.sh`'s smoke run; the owner's prompt asks for it | Accepted in part | The prompt's close keeps it, so it is the one hardware check, needing the display, reported as such; the baseline and the gates stay CPU-only (§2.1) |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| Baseline | 2026-11-26 | measured | 5,363 tests, 224,315 test lines; nextest 5,338 passed, 25 skipped, 58.3 s; `base` 114,656 / 133,794 lines (85.7 %), 191,077 / 221,575 regions (86.2 %); 54 flaky entries; spend 0.3 % of the week at the critique's start, 35.3 % used at the session's start | `perf-runs/20261126-test-audit/`, `target/test-audit/` |
| 1 | 2026-11-26 | done | nine reviewers in two workflows (4.52 M subagent tokens; core's three 15.5 min, the rest 18.2 min), each reading 333–716 of its slice's tests in full; 382 rows: 102 cuts, 30 merges of 76 tests, 71 protected, 179 gaps; `rows.py check` → 0 errors (one warning: a render merge across two files); `rows.py cross` → 1 hit, a kept umbrella test that an umbrella merge keeps as a case; spend 4.5 % | `perf-runs/20261126-test-audit/rows/` |
| 2 | 2026-11-26 | done | 54 cuts (6 duplicate, 41 subsumed, 6 restated, 1 dependency) and 9 merges of 24 tests; 1 merge dropped by the main session (the layers pair, three inputs varying); verifier 63 of 65 supported, 2 reverted (`particle_budget_default_is_10k`: the emit system's fallback reads that default; the rising-platform merge: its helper's last assert names no case); fmt, clippy and `cargo test -p tungsten-core` exit 0 (1,803 passed with doctests); `cov.py step cut-core` → 1,776 passed, lost 0 lines and 0 regions; `rows.py removed HEAD` = the 78 applied tests; spend 4.9 % | `perf-runs/20261126-test-audit/steps/2/` |
| 3 | 2026-11-26 | done | 27 cuts (24 subsumed, 1 duplicate, 1 dependency, 1 tautology) and 11 merges of 28 tests; five imports and one test-only struct the cuts left unused removed; verifier 38 of 38 supported, 0 reverted (it confirmed core's kept parallax case lives on in the new table test); fmt, clippy and `cargo test -p tungsten` exit 0 (1,177 passed); `cov.py step cut-umbrella` → 1,177 passed, lost 0 lines and 0 regions; `rows.py removed HEAD` = the 55 applied tests; spend 5.1 % | `perf-runs/20261126-test-audit/steps/3/` |
| 4 | 2026-11-26 | done | 5 cuts (all subsumed) and 3 merges of 7 should_panic tests into catch_unwind tables (`cues.rs` gains a file-local `panic_message`); 1 merge dropped by the main session (`phases.rs`' engine-first and kit-first pair, already named by their case); verifier 8 of 10 supported, 2 reverted (`the_phases_name_their_anchors_and_stages`: Present's place in `Phase::ALL` asserted nowhere else; `a_count_of_zero_spawns_nothing`: the only `with_count(0)` case); fmt, clippy and `cargo test -p tungsten-kit` exit 0 (1,236 passed with doctests); `cov.py step cut-kit` → 1,075 passed, lost 0 lines and 0 regions; `rows.py removed HEAD` = the 12 applied tests; spend 5.2 % | `perf-runs/20261126-test-audit/steps/4/` |
| 5 | 2026-11-26 | done | render: 10 cuts (6 subsumed, 1 tautology, 1 restated, 2 dependency) and 3 merges (8 tests into 3 tables, the bloom mip table keeping `targets.rs`' name across two files), one import and one test-only const the cuts left unused removed; the CLI: 1 cut (subsumed); the crash probe and the launcher: none; verifier 14 of 14 supported, 0 reverted; fmt, clippy and tests exit 0 (render 261, CLI 125 passed); `cov.py step cut-tools tungsten-render tungsten-cli` → lost 0 lines and 0 regions; `rows.py removed HEAD` = the 18 applied tests; commits `bb11be4c` (render) and this one (CLI); spend 5.4 % | `perf-runs/20261126-test-audit/steps/5/` |

## Follow-ups
