# The QA pass (`qa-pass`) — draft

- **status:** draft
- **goal:** one QA and debug pass over the whole tree before Phase 7's close. Records: the frozen 1.0 records and the plans archive deleted (the redo's 9.7, pulled forward), every rule naming them updated, their citations repointed or left as notes, and every other in-scope tracked Markdown file kept, compressed, merged or deleted with each cut's reason. Bugs: every package's non-test code scanned for easy bugs, each fixed test-first in place or recorded as one known-issues row or Later line
- **non-goals:** `DECISIONS.md` and `CHANGELOG.md` unedited (the new decision appended alone); `docs/DECISION_INDEX.md` beyond the new decision's row and the repoint; the Phase 7 close's files beyond the redo's 9.7 row and one Follow-ups line in its plan (§2.1); asset files, `tools/` art (01's and the template's art tools included) and `perf-runs/`; test code (not scanned); vendored sources; a fix past §2.5's four conditions; perf sittings (no workload moves: the counter guards in `just check` stand, `D-226`); no CI, release or dependency change
- **files to touch:** Step 1: `docs/plans/1.0/` and `docs/plans/archive/` (deleted), `scripts/check-repo.py`, `scripts/test-check-repo.py`, `scripts/status.py`, `scripts/test-status.py`, `scripts/check-agent-context.py`, `.claude/settings.json`, `.ignore`, `justfile`, `AGENTS.md`, `docs/operating-rules.md`, `docs/README.md`, `docs/plans/README.md`, `docs/agent-setup.md`, `docs/LLM_INDEX.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/validation.md`, `docs/DECISION_INDEX.md`, `DECISIONS.md` (appended), `docs/plans/roadmap-redo.md` (the 9.7 row), `docs/plans/phase-7-close.md` (Follow-ups), this plan. Step 3: the documents of §2.3's five doc slices that triage keeps rows for, and the inbound links of a document it deletes. Step 4: the non-test files named by accepted bug rows, their packages' test files, `docs/known-issues.md`, `ROADMAP.md` (Later). Step 5: this plan (deleted), `ROADMAP.md`
- **ordered steps:** (1) records: the two folders deleted, the rules and citations updated, the decision; (2) audit: thirteen read-only auditors write rows; (3) docs: triage, apply, verify, commit by group; (4) bugs: triage, test-first fixes, verify, commit by package, the rest recorded; (5) close
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check`, `just repo-check`, `just ctx` and `git diff --check` pass, and `just template-check` if `templates/basic/` changed; the last evidence row reports tracked Markdown bytes before and after (in all and the top 20), documents deleted, merged and compressed, known-issues rows dropped, bugs found, fixed and deferred, verifier reverts and spend; the plan deleted and `qa-pass` out of Now

Item `qa-pass`, off-roadmap, from the owner's prompt of 2026-11-26 (planned and run in this session, as the test audit was). Written 2026-11-26 at `66225d81`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

Phase 7's close (Step 17, `docs/plans/phase-7-close.md`, draft) is next. Before it, the owner asked for one QA and debug pass over the whole tree, in one session, with a workflow of at most 20 agents and at most 15 % of the weekly meter, read with `perf-runs/713-chain/spend.py` at each phase boundary.

Records: the owner's answer of 2026-11-26 replaces "the old records stay until 9.7" (2026-10-16, R-Q11; the redo's §4 and its 9.7 row). `D-215` froze the 1.0 records behind a banner "until step 9.7 deletes them" and kept `docs/plans/archive/` unread by rule. Both folders go now, their last commit named in a new decision and tagged by the owner; `DECISIONS.md`, `CHANGELOG.md` and the Phase 7 close's files keep their citations, which `just repo-check` reads as notes. Then every other in-scope tracked Markdown file is audited and compressed: stale text (contradicted by the code, describing a replaced system, or a finished record) goes, and history, superseded dated numbers and repeats of a canonical source ([the doc map](../README.md)) are cut, each fact a reader needs kept once.

Bugs: the non-test code of every package (core, render and its WGSL, the umbrella, the kit, the CLI, the crash probe and the launcher, examples 01–03, the template, `scripts/*.py`) is scanned for easy bugs; a fix lands only test-first, local, off every save, format, schema, decision and pin, and unnamed by any decision (§2.5).

It stands on `D-215` (frozen records, one plan file, one critique round at xhigh), `D-216` (push the version branch after each commit), `D-140` (routes), `D-226` (counter guards as the perf check) and `D-070` (GPU checks local). Machine-local records go to `perf-runs/20261126-qa-pass/` (its scripts, briefs, rows, verdicts and fix outputs; quoted here, not linked).

## 1. Audit

Read at `66225d81`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Tracked Markdown: 191 files, 10,825,716 B. `docs/plans/archive/`: 117 files, 5,999,607 B (counted from `git ls-tree -l`, no name printed). `docs/plans/1.0/`: 26 files, 958,405 B (25 Markdown, 884,508 B, and `roadmap.json`). The other 49: 3,941,601 B, 2,926,159 B of it out of bounds (`DECISIONS.md` 1,402,776, `CHANGELOG.md` 488,168, the index 133,085, the close's seven plan files 902,130), about 1.0 MB in scope | `perf-runs/20261126-qa-pass/inventory/md-66225d81.tsv` (bytes, last commit, inbound links) |
| A2 | The largest in scope: `DESIGN.md` 257,786 (its Subsystems section 211,855), `docs/perf/benchmarks.md` 117,199, `examples/01_platformer/tools/README.md` 114,390, `docs/known-issues.md` 105,145, `examples/01_platformer/README.md` 67,976, `docs/getting-started.md` 67,186, `ROADMAP.md` 50,846 (Later 28,605 in 132 lines, the last three Phase 7, 8 and 9), `docs/perf/profiling-workflow.md` 30,955, `docs/levels.md` 25,920, `docs/releases.md` 25,188 | A1 |
| A3 | Rules naming the two folders: `AGENTS.md:13`–`:14`, `docs/operating-rules.md:10` (status's frozen-record edits) and `:35`, `docs/README.md:32`, `docs/plans/README.md:19`–`:20`, `:43`, `:47`, `:49`, `docs/agent-setup.md:25`, `:28`, `:84`, `docs/LLM_INDEX.md:98`, `docs/validation.md:27` (the retired W4 ledger) | `rg -n 'plans/archive\|plans/1\.0\|1\.0/\|frozen record\|the archive'` outside both folders |
| A4 | Script rules: `check-repo.py`'s `ARCHIVE` and `archive_path` (`:26`, `:53`), the done-plan message (`:257`), link and citation notes (`:306`, `:379`), the `FROZEN` banner notes (`:46`–`:48`, `:311`, `:319`), the archive deny and ignore checks (`:428`, `:433`); `check-agent-context.py`'s `ARCHIVE` and `in_archive` (`:19`, `:58`); `status.py`'s frozen section (`:21`, `:173`–`:190`) and the plans section's exclusion (`:253`–`:257`); their tests (`test-check-repo.py:223`–`:346`, `test-status.py:89`–`:148`); `.claude/settings.json`'s `Read(./docs/plans/archive/**)` deny and `.ignore:5`. `preflight.py:33`, `:328` skip the archive too; it is a close file (R7), so it stays (§2.1) | the files |
| A5 | Only the 1.0 files carry the `Frozen <date> (D-215` banner (six) besides two test fixtures, so the frozen-record logic has nothing left once 1.0/ goes | `rg -l 'Frozen [0-9-]+ \(.?D-215'` |
| A6 | Citations in maintained docs: `DESIGN.md:7` (the W4 ledger link), `docs/DECISION_INDEX.md:3` (archive paths) and `:235` (`D-194`'s design path), `docs/known-issues.md:3` (nine archived plan paths), `:84` and `:102` (the 1.x backlog link), `docs/validation.md:3` (the 1.0 workflow link) and `:27` | A3's search |
| A7 | `D-215`'s consequence: "Frozen behind a banner, until step 9.7 deletes them" (`DECISIONS.md:3421`); the redo's R-Q11 default "the old records deleted at the end with the tree tagged and `just history`" (`roadmap-redo.md:99`) and its 9.7 row (`:182`). The decision index test checks headings and IDs only, no marker lines | `crates/tungsten-core/tests/integration/decision_index.rs:32`, `:51` |
| A8 | Tags: `release.yml` runs on `v[0-9]+.[0-9]+.[0-9]+` and `v[0-9]+.[0-9]+.[0-9]+-*` only (`:21`–`:23`); `ci.yml` on branches `main` and `0.*` (no tags); `status.py:57` and `release.py:370` read `refs/tags/v*` only. A tag named `old-records` publishes nothing and moves no script's newest tag | the files |
| A9 | `just check` at HEAD: exit 0 in 1 min 16 s (fmt, clippy with `-D warnings`, the workspace's tests). TODO/FIXME/XXX/HACK in non-test code: none (one false hit, `preflight.py:224`'s word list) | `perf-runs/20261126-qa-pass/inventory/check-66225d81.log` |
| A10 | Non-test code: 246,892 Rust lines (core 72,809, kit 70,088, umbrella 30,767, 01 30,528, render 17,235, 02 8,204, CLI 6,775, 03 6,324, template 3,815, launcher 242, crash probe 105), 4,786 WGSL lines (render 2,173, `assets/shaders/` 2,070 with LYGIA's four vendored files, umbrella 210, 01 162, 03 86, 02 85), 7,076 in `scripts/*.py` outside the tests | `perf-runs/20261126-qa-pass/slices.json` |
| A11 | Link census: 461 relative links in tracked Markdown; outside 1.0/, 3 `missing` (the plan skeleton's, which resolve at its destination) and 20 into the deleted folders (`CHANGELOG.md` 8, `DECISIONS.md` 2, `DESIGN.md` 1, known issues 2, the plans README 1, the Phase 7 plan 1, the redo 3, validation 2); no broken fragment | `perf-runs/20261126-qa-pass/links.py`, `inventory/links-66225d81.tsv` |
| A12 | Budgets: the LLM index 12,281 of 12,288 B, the template's `AGENTS.md` 12,705 of 13,312, `AGENTS.md` 6,143 of 6,144, the renderer's 4,092 of 4,096 | `scripts/check-agent-context.py:22`–`:28` |
| A13 | The four root entries `0.119`, `0.120`, `v0.118.0`, `v0.119.0` are empty regular files, not folders: untracked, 0 B, made within 0.43 s of 2026-11-25 16:03:08Z, while the 0.119 release session (`3b30c4b2`) waited on the owner's tag block. Neither that block nor `scripts/release-preflight.py` writes a file; the cause was not cheap to find | `stat`, that session's transcript |
| A14 | Disk 50 GB free of 468 GB; the week ~41.3 % used at the session's start (15:28:08Z, `spend.py`); Codex 5 h at 0 %, its week at 45 % | `df -h`, `spend.py`, the critique's window read |

## 2. Design

### 2.1 Records (Step 1)

- **Deletion.** One Python call removes both folders (`shutil.rmtree`, nothing printed), then `git add docs/plans/1.0 docs/plans/archive` stages it (`git rm` is denied). The last commit holding both is Step 1's parent, `S0` (this plan's commit); the new decision names it, and `git show <S0>:<path>` reads any record.
- **The decision,** `D-31[4]`, by the tungsten-decision skill: appended to `DECISIONS.md` with its index row. It records that the frozen 1.0 records and the plans archive were deleted on 2026-11-26 ahead of 9.7 (the owner's answer, replacing R-Q11's timing), that `S0` holds them, tagged `old-records` by the owner and read with `just history <path>`, and that history and the plans that cite them keep those citations as notes (`check-repo.py`'s historical paths). It changes `D-215`'s "until step 9.7" consequence; `D-215` gets no marker line and its index row no note, since the owner's prompt holds `DECISIONS.md` and the index unedited past the new row (Q1).
- **`check-repo.py`:** `ARCHIVE` becomes `HISTORICAL = ("docs/plans/archive", "docs/plans/1.0")`, which `archive_path` reads, so a link or citation into either folder is a note ("a deleted record's link, a note (`D-31[4]`)"), in every checked document; the done-plan error says to delete the plan with its citations; the `FROZEN` banner and its notes go (A5); `check_agent_config` drops the archive deny and ignore checks; the docstring names the deleted paths. `test-check-repo.py` follows: fixtures under the deleted folders become note cases, the frozen-record test goes, the settings fixtures lose the deny.
- **`status.py`:** the frozen section goes (five sections); the plans section's title loses "outside the frozen 1.0 folder" and its folder filter; `test-status.py`'s section-order test reads five, the frozen-record test goes, the plans test keeps its program-folder case.
- **`check-agent-context.py`:** `in_archive` reads both folders as historical; its docstring follows.
- **`.claude/settings.json` and `.ignore`:** the archive's deny and ignore line go (Q3).
- **`justfile`:** `history path:` with one line, `git show old-records:{{path}}`.
- **Rules:** `AGENTS.md`'s archive bullet and frozen-records clause become one clause, within its 6,144 B; `docs/operating-rules.md`'s Records bullet and the status bullet's frozen-record edits; `docs/README.md:32`; `docs/plans/README.md`'s archive lines and program-folder example; `docs/agent-setup.md`'s archive lines; `docs/LLM_INDEX.md:98`; `docs/validation.md:27`'s W4 ledger clause (retired by `D-215`). Each says the folders are gone, names `D-31[4]` and `just history`, and keeps no "never read" rule for a path that no longer exists.
- **Repoints (A6):** a link or path into either folder in `DESIGN.md`, the index, known issues and validation becomes plain text naming the path at `S0` (for example "the W4 ledger, `docs/plans/1.0/w04-api-freeze.md` at `S0`"). Each edited passage (`DESIGN.md`: its section) is read whole first.
- **The close's files:** the redo's 9.7 row reads done ahead of its place, citing `D-31[4]`, `S0` and the tag; the close plan's Follow-ups gains one line naming each of its files that cites a deleted folder (the Phase 7 plan, the redo, the seam register, the genre jam, the paper designs) and `preflight.py`'s archive skip, with Step 1's commit, so its Step 1 recheck sees them. Their other text is untouched.
- **The four root files** (A13): removed with `os.remove` after `stat` shows each empty; untracked, so no commit carries it.

### 2.2 Rows and the applier

Auditors write rows, one file each, under `perf-runs/20261126-qa-pass/rows/<slice>/`, from the briefs in `briefs/`: `key: value` header lines, a `---` line, then a body. `rows.py` (same folder) parses them:

- **Doc rows** name `doc`, `lines` (`A-B` at the audit's base, `S1`, Step 1's commit), `first` and `last` (each line's opening 40+ characters), `action` (`compress`, `cut`, `fix`, `merge`, `delete-doc`), `reason` (`stale`, `history`, `repeat`, `done`, `fixed`, `fix`) and `evidence`; the body is the replacement for lines `A..B`, verbatim. Every cut names its reason and what a verifier checks: the contradicting `file:line` or the replacement for `stale`, the current fact's place for `history`, the canonical home for `repeat`, the finishing commit or decision for `done` and `fixed`.
- **Bug rows** name `kind` (`bug`, `doc-bug`, `note`), `file`, `line`, `fn`, `summary`, `failure`, `repro` (a CPU test sketch), `fix`, `size`, `touches` (`none` or `save`, `format`, `schema`, `decision`, `pin`, `public-api`), `decisions` (the index rows checked) and `confidence`. A `note` edits nothing, so it may name any file; a doc slice files notes only (a cross-document fact, a code bug seen in passing).
- `rows.py check --base S1` refuses a row outside its slice (`slices.json`: each doc slice's documents and line spans, each code slice's paths, so no row reaches an out-of-bounds file or ROADMAP.md's Now and Phase 7 and 8 lines), a span past its file, anchors that do not match their lines, overlapping rows, an empty compress or a non-empty cut, and missing fields. `rows.py apply --base S1 [--skip ids] SLICE…` refuses a document that differs from `S1`, then replaces spans bottom-up and logs each row in `applied.tsv`; `rows.py stats` counts rows and bytes; `rows.py ids` checks that Later keeps every register ID and sha it held at `S1`.
- **Coverage:** each code auditor keeps `coverage.tsv` (every file of its slice, `read`, `skimmed` or `unread`) and each doc auditor `disposition.tsv` (every document, `keep`, `compress`, `merge` or `delete`), written as they go; `rows.py coverage` exits 1 on a file with no line, so an empty or unfinished slice cannot pass Step 2, and a stopped audit's unread files are listed in the report as not audited.
- `links.py --baseline <saved run>` exits 1 on a missing file, a missing fragment or a deleted-folder link that the saved run did not have, the files that keep notes aside (A11).

### 2.3 Slices and the audit

Thirteen auditors, read-only (they write only their rows), in one workflow, at xhigh, from `S1`:

| Slice | Scope | Size |
| --- | --- | --- |
| `design-a` | `DESIGN.md` to the line before `### Core UI Model` (Status to Standalone Games) | ~126 KB |
| `design-b` | `DESIGN.md` from `### Core UI Model` to its end | ~132 KB |
| `issues-perf` | `docs/known-issues.md`, `docs/perf/benchmarks.md`, `docs/perf/profiling-workflow.md` | 253 KB |
| `games` | `examples/01_platformer/tools/README.md`, `examples/01_platformer/README.md`, `docs/getting-started.md` | 250 KB |
| `rest` | `ROADMAP.md`'s Later bar its Phase 7 and 8 lines, `docs/releases.md`, `validation.md`, `agent-setup.md`, `levels.md`, `assets.md`, `LLM_INDEX.md`, `operating-rules.md`, `docs/README.md`, `docs/plans/README.md`, `README.md`, `AGENTS.md`, the renderer's and the template's `AGENTS.md`, the template's `README.md` and `tools/README.md`, the eight skill files under `.claude/skills/`, the fixture READMEs of 02, 03's tools and core's audio, `assets/fonts/README.md` | ~227 KB |
| `core-a` | core's physics, ECS, saves, persist, workers, script, config, schedule, time, camera, tween and small modules | 34,045 lines |
| `core-b` | core's assets, UI, input, text, display, component registry, settings, components, post and audio | 38,764 |
| `kit-a` | the kit's damage, chemistry, react, lifecycle, transform, foundation and `lib.rs`, and its listing example `examples/components.rs` | 35,327 |
| `kit-b` | the kit's motion, overlap, intent and present | 34,761 |
| `umbrella` | `crates/tungsten/src/` with its WGSL, and `crates/tungsten/build.rs` | 30,767 |
| `render-tools` | the renderer with its WGSL, `assets/shaders/`, the CLI with `tools/cli/build.rs`, the crash probe, the launcher | 28,508 |
| `example-01` | `examples/01_platformer/src/` and its shaders (not `tools/`) | 30,609 |
| `games-scripts` | 02's and 03's `src/` and shaders, the template's `src/` and `build.rs`, `scripts/*.py` outside the tests | 25,590 |

Kept without a slice: the three `CLAUDE.md` imports and the SMAA attribution (licence text). Not scanned: the benches (test harnesses) and LYGIA's vendored helpers in both mirrored homes (`assets/shaders/stock/lygia/`, `crates/tungsten-render/src/shaders/stock/lygia/`); `gen_slices.py` asserts no vendored file and no file in two slices. The doc auditors read each document whole before a row on it (DESIGN.md by whole section), verify `stale`, `fixed` and `fix` claims with `rg` and the decision index, and keep every link they leave valid. The code auditors read every file of their slice at least to its function bodies, check the index before calling a choice a bug, and include known-issues rows in their slice that meet §2.5. No auditor runs cargo, just or a writing git command.

### 2.4 Docs: triage, apply, verify (Step 3)

- **Triage,** in the main session, by metadata and reading: `rows.py check` errors are fixed against `S1` or the row dropped; a row is rejected when it cuts a fact with no home, drops a heading other documents link (`links.py` names them), deletes a document whose inbound links it does not repoint, or reaches a fact the doc map makes this document canonical for. Each slice's five largest rows are read in full. `triage.tsv` records each row's verdict and reason.
- **Apply:** `rows.py apply --base S1 --skip <rejected> <slices>`; a deleted document's inbound links are repointed by hand, by row-sized edits listed in `triage.tsv`.
- **Verify:** three fresh doc verifiers in parallel (`briefs/doc-verify.md`), by group: `DESIGN.md`; known issues and the perf docs; the rest. Each checks every applied row's hunk against its reason and evidence and writes `verify/<group>.tsv`. An unsupported row is reverted by reapplying the group's documents from `S1` without it.
- **Commits,** by document group, each after its verifier: `docs(design): …`, `docs(known-issues): …`, `docs(perf): …`, `docs(example-01): …`, `docs: …` for the rest (several when their documents are independent).

### 2.5 Bugs: triage, fix, verify (Step 4)

A fix lands only when all hold, as the owner set:

1. A failing CPU test, or a reproduction, written first, shows the bug.
2. The fix stays in one function or file, about 30 lines.
3. It changes no save, format, schema, decision or counter-guard pin, and no public API beyond the fix.
4. No decision names the behaviour (`docs/DECISION_INDEX.md` checked first).

Triage keeps bug rows that read as meeting them after the cited function is read; the rest become one known-issues row each (a defect) or one Later line (a missing capability), a `note` or `doc-bug` row its own disposition, a low-confidence row with no traced input dropped and counted. Up to three code writers (`briefs/code-fix.md`), one per group of packages, run in parallel in the main tree, each touching only its rows' files and their packages' tests, never committing: a checkpoint of the row's files first (`fixes/<id>/pre/`), the test first (its failing output saved), the fix, the package's checks (Rust: its tests, clippy, fmt, and `just physics-release` for a fix under `crates/tungsten-core/src/physics/`, whose hashes are pinned in release only; a script: its `scripts/test-*.py` suite and `just script-test`), the row's own `fix.patch` (its non-test files only, from its checkpoint) and a summary. Two rows may share a file, so a row that stops (not reproduced, too big, moves a pin, named by a decision, needs the GPU) puts back its own checkpoint, never `HEAD`, and is recorded. Then one fresh code verifier (`briefs/code-verify.md`), alone in the tree, undoes each row's own patch (`patch -p1 -R`), reruns its test (it must fail), re-applies it (it must pass), and checks §2.5's conditions on the diff. An unsupported fix is undone by its own patch. A render or WGSL fix reads `crates/tungsten-render/AGENTS.md` first and lands only with its GPU check run here (`just smoke` or `just visual`, as validation says), else it waits as a known-issues row. Commits: `fix(<package>): …` per package, each with `just check` passing, then one `docs(known-issues): …` for the recorded rows and Later lines.

### 2.6 Agents and budget

At most 20 agents: thirteen auditors, three doc verifiers, up to three code writers and one code verifier, at xhigh. `python3 -B perf-runs/713-chain/spend.py --since 2026-11-26T15:28:08Z` is read at each phase boundary (the plan's commit, Step 1, the audit, each docs commit group, the fixes, the close) and every 15 minutes while a workflow runs. The ceiling is 15 % of the week; 2 % is reserved for the close, so no phase starts past 13 %; the audit's workflow is stopped past 9 % and the rows written so far are triaged (auditors write rows as they go). Past 15 %, nothing more launches: what is verified lands, and Step 5 runs.

## 3. Steps

### Step 1: Records

- **Files:** Step 1's in the header
- **Change:** status `in progress`; §2.1 in its order; the four root files removed
- **Done-when:** `test ! -e docs/plans/1.0 && test ! -e docs/plans/archive` → exit 0; `git diff --cached --shortstat -- docs/plans/1.0 docs/plans/archive` → 143 files changed, 0 insertions; `python3 -B scripts/test-check-repo.py`, `python3 -B scripts/test-status.py` → exit 0; `just ctx` → exit 0; `just repo-check` → exit 0; `just script-test` → exit 0; `python3 -B perf-runs/20261126-qa-pass/links.py --baseline perf-runs/20261126-qa-pass/inventory/links-66225d81.tsv` → exit 0, and `links.py --summary` → no `historical` row outside `CHANGELOG.md`, `DECISIONS.md` and the close's files; `rg -n 'plans/archive|plans/1\.0' DESIGN.md docs/known-issues.md docs/validation.md docs/DECISION_INDEX.md` → each hit names `S0`; `git diff --unified=0 66225d81 -- DECISIONS.md | rg -c '^-[^-]'` → 0; `just history docs/plans/1.0/README.md` → exits non-zero until the owner's tag, and `git show <S0>:docs/plans/1.0/README.md | head -1` → the README's title; `just status` → five sections
- **Commit:** `docs(records): delete the 1.0 records and the plans archive`
- **Moves:** the tracked Markdown bytes by A1's 6,884,115; `just status`'s sections; check-repo's notes

### Step 2: Audit

- **Files:** none in the tree; `perf-runs/20261126-qa-pass/rows/`
- **Change:** `gen_slices.py` at `S1`; one workflow of §2.3's thirteen auditors; then `rows.py check` and `rows.py stats`
- **Done-when:** `python3 -B perf-runs/20261126-qa-pass/rows.py check --base S1` → exit 0 (each refused row fixed against `S1` or dropped, and counted); `rows.py coverage` → exit 0, or, for an audit the budget stopped, its unread files listed and carried to the report as not audited; `rows.py stats --base S1` → counts by action, reason and kind, quoted; the spend read
- **Commit:** none (rows are machine-local; the evidence row rides Step 3's first commit)
- **Moves:** nothing

### Step 3: Docs

- **Files:** Step 3's in the header
- **Change:** §2.4
- **Done-when,** per commit group: `rows.py apply` → its count; the group's verifier → every remaining row supported (reverted rows counted); `just repo-check` → exit 0; `just ctx` → exit 0; `git diff --check` → exit 0; `links.py --baseline …links-<S1>.tsv` → exit 0 (the baseline retaken at `S1`); for `ROADMAP.md`, `rows.py ids --base S1` → lost 0; for the template's docs, `cargo test -p tungsten-cli --locked` after staging (its embedded-template test reads the index) → exit 0
- **Commit:** per group, as §2.4
- **Moves:** document bytes and structure; no code

### Step 4: Bugs

- **Files:** Step 4's in the header
- **Change:** §2.5
- **Done-when,** per package commit: each fix's `before.txt` a failure and `after.txt` a pass; the verifier → supported, its `verify-without.txt` a failure; for a Rust package `cargo fmt --all -- --check` → exit 0, `cargo clippy -p <package> --all-targets --locked -- -D warnings` → exit 0 and `just check` → exit 0, and for a physics fix `just physics-release` → exit 0; for a script, its `python3 -B scripts/test-<name>.py` → exit 0 and `just script-test` → exit 0; `git diff -- api/` empty; for a render or WGSL fix, its GPU check's exit code; for the recorded rows, `just repo-check` → exit 0
- **Commit:** `fix(<package>): …` per package; `docs(known-issues): record the QA pass's deferred bugs`
- **Moves:** the fixed functions' behaviour, as each row states; tests added

### Step 5: Close

- **Files:** this plan (deleted), `ROADMAP.md`
- **Change:** the checks; the report; open follow-ups to known issues or Later; the plan deleted and `qa-pass` out of Now
- **Done-when:** `just check` → exit 0; `just repo-check` → exit 0; `just ctx` → exit 0; `git diff --check <S0> HEAD` → exit 0; `just template-check` → exit 0 when `templates/basic/` changed (the one hardware check, on the display); the demo: `just status` prints five sections with no frozen-record section, `git show <S0>:docs/plans/1.0/README.md | head -1` prints that README's title, and `git cat-file -e <S0>:docs/plans/archive/phase4.md` → exit 0 (the archive kept, its text unopened); the report in the last evidence row
- **Ends with:** the commits, the evidence, what the owner should try, the hand-off: the Phase 7 close's run prompt and the owner's tag command (`git tag -a old-records <S0> -m '…' && git push origin old-records`)

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | `D-215`'s "until step 9.7" consequence: a marker line? | None: the prompt holds `DECISIONS.md` and the index unedited past the new row; the new decision names what it changes |
| Q2 | The tag's name? | `old-records`, R-Q11's "the old records"; it cannot match `release.yml`'s `v[0-9]+…` patterns (A8) |
| Q3 | The archive's Read deny and ignore line, once the folder is gone? | Removed with check-repo's two checks: a rule for a path that no longer exists |
| Q4 | `preflight.py`'s archive skip? | Kept: a close file (R7); a dead skip changes no result; named in the close plan's Follow-ups line |
| Q5 | A known-issues row whose bug meets §2.5? | A bug row like any other; its fix drops the row in the same commit |
| Q6 | A bug in an out-of-bounds file (the close's files, `tools/` art)? | One known-issues row, not fixed |
| Q7 | A doc's fact that belongs in another slice's document? | A `note` row; the main session moves it at triage or leaves it |

## 5. Invariants

- **May move:** tracked Markdown bytes (down) and structure (Steps 1, 3); check-repo's, status's and the context check's behaviour for the deleted folders (Step 1); the fixed functions' behaviour and the tests (Step 4)
- **Must not move:** `DECISIONS.md` and `CHANGELOG.md`'s existing text; the index beyond the new row and the repoint; the close's files beyond the 9.7 row and the Follow-ups line; ROADMAP.md's Now (bar this item) and its Phase 7 and 8 Later lines; asset files, `tools/` art, `perf-runs/` outside this pass's folder; saves, formats, schemas, counter-guard pins, `api/` snapshots, physics hashes and reference images; every register ID and sha in Later

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it.
- A fix that trips a guard or `just check` and can't stay local is reverted and becomes a known-issues row.
- Spend: as §2.6; past 15 % nothing more launches, the verified work lands, and Step 5 runs.
- Disk under 20 GB free: `just clean-target --apply` before the next build.

## 7. Critique

Round 1: `critique --model gpt-6-astra --effort xhigh --raw --repo --focus-file <the tools' note>` on the draft over `66225d81`, 2026-11-26 (`gpt-6.2-astra` skipped, refused on this account since 2026-11-18); the focus note pointed the critic at the machine-local tools and Step 1's scripts. Footer: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 363.2 s, tokens in 848357 (cached 763136), out 10084 (reasoning 5484), commands 46, output 241.3 KB (largest 16.8 KB), usage 5h 0% → 21%, week 45% → 48%. One round, as the owner's prompt set; a finding still open would become an open question.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1.1 | A stopped row restored whole files from `HEAD`, erasing an earlier row's fix in the same file; the verifier's whole-file revert proved the combined change, not the row's | `briefs/code-fix.md`, `briefs/code-verify.md` | Accepted | Per-row checkpoints (`fixes/<id>/pre/`) and patches (`fix.patch`, non-test files only); a stop restores its checkpoint, the verifier undoes and re-applies the row's own patch (§2.5, both briefs) |
| 1.2 | The physics hashes the invariants hold are pinned in release only, which `just check` never runs | `physics_determinism.rs:70`, `physics_containment.rs:254` (ignored in debug); `docs/validation.md:21` | Accepted | `just physics-release` for a fix under `crates/tungsten-core/src/physics/` (§2.5, Step 4, the writer's brief) |
| 1.3 | Python fixes fell into a Rust-only completion path | `briefs/code-fix.md`'s step 4; Step 4's done-when | Accepted | A script's path: its `scripts/test-*.py` suite and `just script-test` (§2.5, Step 4, the brief) |
| 1.4 | `note` rows, which the doc brief prescribes, failed `check_bug`'s code-ownership test in every doc slice | `rows.py` `check_bug` | Accepted | A `note` may name any file and a doc slice files notes only (`rows.py`, §2.2) |
| 1.5 | Build scripts were unassigned: `crates/tungsten/build.rs`, `templates/basic/build.rs`, `tools/cli/build.rs` (the template and licence embedding) | `git ls-files '*.rs'` outside `src/` and `tests/` | Accepted | Assigned to the umbrella, games-scripts and render-tools slices, with the kit's `examples/components.rs`; the benches stated as not scanned (§2.3, `gen_slices.py`) |
| 1.6 | LYGIA's mirror copies under `crates/tungsten-render/src/shaders/stock/lygia/` stayed in the render slice | `slices.json`; `crates/tungsten-render/AGENTS.md:8` | Accepted | Both homes excluded; `gen_slices.py` asserts no vendored file and no file in two slices (§2.3) |
| 1.7 | Nothing showed every assigned file was read, so an empty or stopped audit passed Step 2 | `rows.py` `load` and `check` | Accepted | `coverage.tsv` and `disposition.tsv` written as auditors go and `rows.py coverage` in Step 2's done-when; a stopped audit's unread files go to the report as not audited (§2.2, Step 2, both audit briefs) |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| Baseline | 2026-11-26 | measured | A1–A14; `just check` exit 0 (1 min 16 s); spend 0.4 % at the plan's commit, the week 41.3 % at the session's start | `perf-runs/20261126-qa-pass/inventory/` |

## Follow-ups
