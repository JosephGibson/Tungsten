# Roadmap artifact rework — draft

- **status:** in progress
- **goal:** Make the owner's [1.0 roadmap page](https://claude.ai/artifact/EyM2iTgnnActfbMBZKcav9) a thin viewer over facts the repo already owns, so that no fact about the road to 1.0 is hand-kept in more than one place without a check, the page's source lives in the repo, a release needs no manual page work, and the page answers "where am I, what do I run next, what's coming" on one screen at desktop and phone width. Same URL, same `db` capability.
- **non-goals:** Changing the 1.0 order, cards, gates or workflow rules. New page features beyond the three questions above. Reading the repo live from the page (§3, option b). A second publisher: `/tungsten-next` stays the only writer of statuses. Engine code.
- **files to touch:** New: `docs/plans/1.0/roadmap.json`, `scripts/roadmap.py`, `scripts/test-roadmap.py`, `.claude/skills/tungsten-next/page.html`. Edited: `scripts/check-repo.py`, `scripts/test-check-repo.py`, `.claude/skills/tungsten-next/{SKILL.md,prompts.md,roadmap.md}`, `docs/plans/1.0/workflow.md` (§2, §3, §9), `docs/plans/1.0/README.md` (Files, Conventions), `docs/LLM_INDEX.md` (one row), `CHANGELOG.md` (one `[Unreleased]` line); `DECISIONS.md` and `docs/DECISION_INDEX.md` if Q9 below is accepted. Outside the repo: the artifact and its db (§7), the memory note `roadmap_artifact.md` and its `MEMORY.md` line.
- **ordered steps:** (1) catalog file; (2) its repository check; (3) one home for prompts; (4) sync helper; (5) skill; (6) page; (7) db migration and publish; (8) records; (9) close. Details in §9.
- **done-when:** Each prompt template exists once (in `prompts.md`); the page source holds no stop, question or prompt data; `just repo-check` fails when the register and the catalog disagree (tested); a `/tungsten-next` sync after a merge needs no manual edit; the page meets §6's targets (≤ 45 KB, no horizontal scroll at 390 px, WCAG AA text and control contrast, FCP ≤ 200 ms locally); the db holds `meta/catalog`, `meta/now` and `stops/*`, with `questions/*` gone; same URL. Checks in §10.

Drafted 2026-10-04 on branch `0.42` after the 0.42 cut (`HEAD` `db1177c`), from artifact version `1791086651-9707`.

## Context digest

- The page is one 198,761-byte HTML file (CSS, markup, JS and data inline) that exists only in the artifact. Its `db` holds `stops/<id>` `{status, release, milestone, plan, steps}` (3 documents: `s040`, `gdef`, `s0`) and `questions/<qid>` `{status, answer}` (7 documents, all from the definition gate).
- `/tungsten-next` (`.claude/skills/tungsten-next/`) reads the tree through `state.sh`, replies with the next prompt from `prompts.md`, and syncs `stops` per `roadmap.md`. `tungsten-milestone` already points to `prompts.md` for every session's closing prompt.
- The repo is the truth: the register (implementation plan §10), cards (§3–§5), gate records (§11), README "Now" lines. Under `D-105` the truth is the uncommitted tree for a whole milestone, so GitHub's `main` lags it by one release.
- The page cannot read the repo: `files` needs a claude.ai Claude Code project id (`chan_…`), which this local VSCode session lacks, and CSP blocks every fetch to GitHub. Data reaches the page only by republishing it or by `db` writes.
- 0.42 is cut (`Cargo.toml` 0.42.0, a dated `CHANGELOG` section) and waits for the ship sitting, so this plan runs after that merge by default (Q1).

## 1. The page today

| Measure | Value |
| --- | --- |
| Size | 198.8 KB: CSS 44.7 KB (5.2 KB of it two override layers, "Refinements" and "Final pass"), markup 27.7 KB (17.5 KB reference prose), JS 125.6 KB |
| Data versus code | Content 82.5 KB (41 %): data constants 55.6 KB, prompt and guide text 9.4 KB, reference prose 17.5 KB. Code about 60 KB of JS |
| DOM | 7,026 nodes at load: all six views rendered up front, every stop's detail list included |
| First paint, desktop (1440 px, local file, headless Chromium) | FCP 436 ms, DOMContentLoaded 420 ms, layout 170 ms. With Google Fonts blocked: FCP 184 ms, layout 87 ms. The three Red Hat families cost about 250 ms of render-blocking time |
| First paint, phone (390 px, 4× CPU) | FCP 556 ms (472 ms without fonts), layout 330 ms, script 91 ms |
| Re-render per db snapshot | `applyState` 3.7 ms desktop, 13.9 ms phone at 4×; it runs at load and again for each collection's snapshot |
| Route length | 6,924 px desktop, 8,629 px phone |
| Phone layout | The Overview is 814 px wide at 390 px: `.content` and `.panel` are grids with an implicit `auto` column, and the timeline's `min-width: 760px` passes up through its scroll wrapper. The other views fit |

These numbers exclude the claude.ai viewer's frame and runtime boot; they compare the page with itself. Measured with a Playwright script in the planning session's scratchpad (`measure.py`, `overflow.py`), which step 6 recreates.

Bugs found while reading the source:

- Search never lists reference sections: `paletteSource` selects `#reference .card`, but those sections have class `sec`.
- The risk map's heat tint never renders: `color-mix(…, var(--panel))` names an undefined token.
- `--p5`, `--p6` and `--p7` (phase colors) are undefined; `.tl-risk` ticks are rendered, then hidden by CSS; `.cbx {}` and `.colorkey .cbx` style nothing.
- Copies already disagree: the reference tab's gate table lists the frame-loop gate's questions as "Q5 Q6 Q7 Q13" without the game-spec item that `PATCH.gfl` added; the Plan prompt reads differently in `workflow.md` §3, `prompts.md` and the page; "Project facts at 0.41" names `D-104` as the latest decision while `D-108` exists; the register puts Track B spikes after W1 M1, the page after W2 R2.

## 2. Fact inventory

Class: **derived** (computable from the tree), **duplicated** (hand-kept in more than one place), **page-only** (nowhere else). "After" is where the fact lives once this plan lands.

| Fact | On the page | Also in | Class | Changes | After |
| --- | --- | --- | --- | --- | --- |
| Stop ids and road order (49) | `PHASES` | `roadmap.md` id list, memory note, register rows (coarser) | duplicated ×3 | per gate | `roadmap.json`; the register is checked against it |
| Phase, group, group notes | `PHASES`, `GROUP_DEFS` | §3 tracks for Phase 5; Phases 6–7 grouping is the page's | duplicated / page-only | per gate | `roadmap.json` |
| Candidate names and pairs (`D-108`) | `cand`, `pc`, `SHIPS_WITH` | register rows, §6 Pairs | duplicated | per gate | `roadmap.json` `row` = the register's cell text, checked |
| Plain titles, one-line summaries, notes | `title`, `notice`, `note`, `PATCH` | — | page-only | per gate | `roadmap.json` (`PATCH` folded in) |
| Kind and level | `k`, `level` | §3 cards' Level column | duplicated | per gate | `roadmap.json`, checked against the card where one exists |
| Scope source; plan slug | `src`; `slug` | `prompts.md` rule for sources; nowhere for slugs | duplicated; page-only | per graduation | `roadmap.json`; the check confirms each path exists |
| Card details: touches, decisions, API, checks, unblocks, needs | `d` | §3 cards, §4–§5 tables | duplicated | per gate | cut; the row names its card |
| Ratings: complexity, effort, your time, flags | `CLASS` | — | page-only, estimates of 2026-10-03 | never updated | `roadmap.json` keeps complexity, effort, owner roles and flags; a plan's step count replaces effort once it exists |
| "Today" code facts | `TODAY` | — (each plan's audit re-measures) | page-only, measured at `db1177c` | stale every release | cut |
| Known issues each stop closes | `CLOSES` | §3 follow-up table, `known-issues.md` | duplicated | per release | cut |
| Acceptance-game needs | `GAME`, reference table | `acceptance-game.md` Mechanics, Feature map | duplicated | per gate | cut |
| Gate checklists, prompts, graduations | gate `d`, `gatePrompt`, `grad`, reference gates table | §2, §9, §11, `prompts.md` Gate | duplicated, already drifted | per gate | `roadmap.json` keeps the gate's name, question ids and reading list; checklist cut (§2 owns it) |
| Track B experiments | `SPIKES` | §3 Track B, `prompts.md` Experiment | duplicated | per gate | cut; the skill reads §3 Track B when Track B is next |
| Owner questions: text, needed by, default | `QUESTIONS` | criteria §10, implementation plan §7 | duplicated | per gate | `roadmap.json`, ids checked against §7 and criteria §10 |
| Question states and answers | `ANSWERED`, db `questions/*` | §11 records, criteria's Answered list | duplicated, two writers | per gate | `roadmap.json`; db collection deleted |
| Status, release, milestone, plan per stop | `DEFAULTS`, db `stops/*` | register, plan files, tags, `origin/main` | derived, two writers (page and skill) | per session or release | db, written only by the skill, from the tree |
| Session ticks | db `stops/*.steps` | — | page-only | per session | db, written only by the owner on the page |
| Prompt templates (20) | `P`, `BTYPES`, `spikeText`, `gatePrompt` | `prompts.md`, `workflow.md` §3 | duplicated ×3, already drifted | per workflow change | `prompts.md` only (§4) |
| Sessions per status; new or same session | `sessionsFor`, `SAME_SESSION`, `GUIDE` | `roadmap.md` session keys, `prompts.md` Where column, SKILL §4 | duplicated | per workflow change | `prompts.md` Flow table; the skill fills `meta/now` |
| Status vocabulary | `FLOWS`, `ADVANCE`, `STATUS_LABEL` | `roadmap.md` Fields | duplicated | rare | `roadmap.md` defines it; the page keeps display labels only |
| Next release and milestone number, plan path | `nextVersion` (base 39), `nextMilestone` (base 31), `planPath` | register, README conventions (M32), `prompts.md` | derived, page guesses | per release | computed by the skill, written into `meta/now` |
| Reference: release flow, levels, what a milestone owes, files, rules, commands | `#reference` | workflow §1–§6, §9; AGENTS.md; 1.0 README; justfile; memory | duplicated | per workflow change | cut |
| Project facts at 0.41 | `#ref-facts` | — | page-only, already stale | per release | cut |
| Color key, rating scale | `#ref-color`, `#ref-ratings` | — | page-only | never | one legend line on the page |
| "Built from `db1177c`" | footer | — | page-only, stale | per release | "Synced at `<commit>`, `<time>`" from `meta/now` |
| Artifact URL | `ARTIFACT_URL` | SKILL.md, `roadmap.md`, memory | duplicated | never | SKILL.md and memory only |

Before: six hand-kept copies of road facts (page JS, `roadmap.md`, `prompts.md`, workflow §3, register, memory) with nothing that compares them. After: the register and `roadmap.json`, compared by `just repo-check`, and `prompts.md`.

## 3. Architecture

Options judged on drift points, work at each release and gate, how easily an agent reads and updates the data, and what breaks if a step is skipped.

| Option | Drift points | Per release | Per gate | Agent use | If a step is skipped |
| --- | --- | --- | --- | --- | --- |
| Today | Six copies, unchecked | Skill writes statuses; anything structural is a hand edit of a 200 KB page | Rewrite `PHASES`/`PATCH` in page JS, the id list in `roadmap.md`, the memory note; republish | Data in JS literals with prose overlays (`PATCH`) | Silent: the copies drift (§1 lists three that already have) |
| a1. Register made parseable (ID, kind, level, risk columns) | One file | None | Edit the register | Markdown table, but titles and summaries make it unreadable, and Phases 6–7 have no rows until their gates | Nothing to skip, but the page still needs a transport |
| a2. `roadmap.json` beside the register | Two files, checked against each other | None | Edit both in the gate's change | Plain JSON, stable ids, about 25 KB | `just repo-check` fails at the next `just check` or release |
| b. `files` capability reads `docs/plans/1.0` live | None in principle | None | None | The page parses prose Markdown at view time | Not available: needs a `chan_…` project id this session lacks, reads the project's synced files rather than the uncommitted tree, and asks every viewer for consent |
| c. Page source in the repo, republished by a skill step | The page's data stays in its JS | A republish per sync: the Artifact tool accepts it only after a read of the whole live page, and every open view reloads | Edit page data, republish | Page JS | A skipped republish leaves the page stale, unseen |
| d. a2 + c + `db` transport (recommended) | Register ↔ catalog, checked; prompts in one file | None by hand: the sync writes db documents | Edit `roadmap.json` beside the register; the next sync pushes it | JSON with stable ids; a helper writes payload files that `ArtifactData` sends by `file_path`, so no agent retypes data | `repo-check` fails; a skipped sync leaves the page showing its synced-at commit |
| e. Derive everything from the docs at each sync, no catalog | None | Skill parses §3–§5 and prose every run | None | Token-heavy, non-deterministic parsing | Loses titles, summaries and complexity ratings; Phases 6–7 show placeholder rows only |

**Recommendation: d.** The repo holds the catalog (`docs/plans/1.0/roadmap.json`) and the renderer (`.claude/skills/tungsten-next/page.html`, no data in it). `scripts/roadmap.py` turns the tree into db documents: `meta/catalog` when the catalog's hash changes, `stops/<id>` statuses derived from evidence, and the skill writes `meta/now` (the filled next prompt, the session flow, heads-up lines, synced-at). A sync never republishes; the page is republished only when `page.html` changes. Option b becomes worth revisiting only if the repo moves to a claude.ai cloud project, and it would still read the last pushed tree rather than the working one.

The cost of d: the page's first useful frame waits for `claude.use("db")` and the first snapshot. The page shows a designed loading state and caches the last snapshot in `localStorage` as a per-viewer convenience, so repeat visits render at once and refresh when the db answers; it renders correctly without the cache. The cache entry carries the page's schema number and the catalog hash: a schema the renderer doesn't know discards it, and a cached view is labelled as such until the db answers. Step 7 times five live loads from load to the first rendered row. If the median exceeds 1 s or any load exceeds 2 s, the fallback is to bake the catalog into the page at publish time (statuses stay in the db); releases still need no page work, but each gate then adds a republish.

### 3.1 Catalog shape

```json
{
  "schema": 1,
  "phases": [{ "num": 5, "name": "Foundations", "intro": "The ground everything else stands on …" }],
  "groups": [{ "id": "track-a", "phase": 5, "name": "Track A · Foundations", "note": "Independent of the frame-loop gate, in this order." }],
  "stops": [
    { "id": "w14a", "row": "W14a", "group": "track-a", "kind": "release", "level": "B",
      "title": "Headless test harness", "summary": "Tests drive the real frame loop without opening a window …",
      "src": "docs/plans/1.0/w14-tooling.md", "slug": "headless-harness",
      "complexity": 3, "effort": "M", "shows": "hood", "roles": [], "flags": ["perf", "api"], "questions": [] },
    { "id": "gfl", "row": null, "group": "gates", "kind": "gate", "title": "Glyph gate and frame-loop gate",
      "summary": "…", "reads": ["docs/plans/1.0/w01-ui-text-suite.md", "perf-runs/"], "questions": ["q5", "q6", "q7", "q13"],
      "complexity": 4, "effort": "S", "roles": ["decide"], "flags": [] }
  ],
  "questions": [{ "id": "q14", "text": "Logs and crash reports: on by default in App, or one opt-in call?",
                  "by": "W11a starts", "default": "Asked in the W11a plan", "state": "open", "answer": "" }]
}
```

- `row` is the register's Candidate cell exactly. A pair is one stop (`"row": "W9a with W12a"`, id `w9a`); stops a placeholder row covers share it (`"W1 M2, M3"`, `"Phase 6 (§4)"`). It is `null` for gates and for `s040`, released before the register began.
- Pairs fold the second stop into the first: `w12a` into `w9a`, `r2` into `m0b`. 47 stops remain (22 + 17 + 8). Neither second id has a db document, so nothing migrates.
- `complexity` 1–4 drives the gray→rose→red meter, `roles` the amber chips, `flags` the violet (`api`, `break`, `dep`) and cyan (`perf`, `visual`, `hash`, `ci`) chips. "Your time" is dropped: roles and kind already say it.

### 3.2 The check

`check_roadmap` in `scripts/check-repo.py`, sharing the register parser in `scripts/roadmap.py`, so both read the register one way:

- `roadmap.json` parses; ids are unique and match `^[a-z0-9]+$`; every `group` and question reference resolves.
- The ordered distinct non-null `row` values equal the register's Candidate cells, in order. This holds because the register already keeps each pair (`W11a with W7a`, `W9a with W12a`, `W1 M0b with W2 R2`) and each placeholder (`W1 M2, M3`, `Phase 6 (§4)`, `Phase 7 (§5)`) as one row (§10, `D-108`). When a change splits a register row (a pair that splits, a placeholder that gains its own rows at a gate), the check fails until the catalog splits the stop in the same change; that failure is the point.
- Where a §3 card's Candidate cell equals a stop's `row`, the card's Level starts with the stop's `level`.
- Every `src` and `reads` path exists (a trailing `/` means a folder).
- Every question id appears as `Q<n>` in implementation plan §7 or criteria §10.

## 4. Prompt templates: one home

`.claude/skills/tungsten-next/prompts.md` becomes the only copy. `tungsten-milestone` already cites it; SKILL.md reads it; the page receives filled prompts in `meta/now` and holds no templates.

- **Key column.** Each row gains a stable key (`gate`, `graduate`, `plan`, `qa-plan`, `approve`, `run`, `run-c1`, `run-c-rest`, `resume`, `release`, `verify`, `game-spec`, `experiment`, `records-lag`, `review-fixes`, `rc-checklist`, `rc-fix`). Tick keys and the Flow table use these keys, never display titles.
- **Literal templates.** Each template carries its own `Don't commit: leave the changes in the tree for the release commit.` where it applies, so what the table shows is what gets pasted. Placeholders stay in angle brackets; the Gate template takes `<reads>` from the gate's `reads` in the catalog.
- **Flow table.** The ordered keys per kind and level, with new or same session and the status each runs at: level A or B `plan → approve → run → ship`; level C `plan → approve → run-c1 → run-c-rest → ship`; QA `qa-plan → approve → run → ship`; gate `[game-spec] → gate → graduate…`; release candidates `rc-checklist → playthrough → rc-fix`. This replaces `sessionsFor`, `SAME_SESSION` and `roadmap.md`'s session keys.
- **Rows the page had and `prompts.md` lacks** move in: `rc-checklist`, `rc-fix`, the playthrough step, and `compact` (`/compact focus on the remaining steps of <plan>`). The page's `where` and `sync` prompts are dropped: `/tungsten-next` does both.
- **workflow.md §3** keeps its first paragraph (the chain of prompts) and replaces its table with a link to `prompts.md`.

## 5. Views and features

| Today | Decision | Why |
| --- | --- | --- |
| Next-session panel | Keep, rebuilt from `meta/now` | The page's job: where you are, the next prompt, where to run it |
| Route | Keep as the only list | What's coming |
| Timeline (49 nodes) | Replace with a three-phase progress strip with gate markers | 49 tab stops, hover-only tips, and its `min-width` broke the phone layout |
| All stops table | Cut | The route's rows again, sortable by stale ratings |
| Risk map | Cut; the complexity meter stays on each row | Estimates never updated; the heat tint never rendered |
| Prompt builder | Cut; recovery prompts (resume, compact, release, verify) go in a fold under Next, filled by the skill | 20 prompt types copied from `prompts.md` |
| Questions | Keep as a read-only section: open questions with "needed by" and default; answered ones collapsed | Owner decisions are what's coming for the owner; answers are recorded in gate sessions |
| Reference | Cut; one legend line, and repo paths in the footer | 17.5 KB copying workflow, AGENTS.md, README and justfile; its project facts are stale |
| Sidebar (tree, progress, six tabs), phase jump, six filters, Expand all | Cut; done stops collapse per group behind a "Show done" toggle | One column is short enough once details live in the repo |
| Search palette, ten single-key shortcuts, shortcut dialog | Cut; the browser's find works | 47 rows; single-key shortcuts without an off switch fail WCAG 2.1.4; reference search was broken |
| Status selects, release, milestone and plan inputs, Mark buttons | Cut | Statuses are derived from the tree; a second writer can contradict the register |
| Session ticks | Keep, owner only | Moves the up-next marker between syncs |
| Copy buttons, toast | Keep (toast for copy only) | |

## 6. UI and UX

Standing preferences hold: dark only, no gradients, professional rather than card-heavy, compact, one meaning per color (blue where you are or a session running, amber waiting on the owner, green done, gray→rose→red complexity, violet public API and decisions, cyan machine checks).

| Area | Before | After |
| --- | --- | --- |
| Hierarchy | Six views; a 252 px sidebar repeats "you are at"; the next prompt is one of six rows in a two-column panel, followed by four big-number tiles | One column, at most 960 px: a header line (release · stop · status · synced-at), the Next block, the phase strip, the route, open questions, the legend |
| Progressive disclosure | An opened stop shows about 13 facts and 4–6 full prompts (3–4 screens) | A row shows status, id, title, one-line summary and chips; opening it adds level, source path, card reference, questions and pair. Only the current stop has prompts |
| Scannability | Ratings in three meter columns; flags, roles, visibility and status scattered across each row | One line per stop: status dot, id in mono, title, then right-aligned chips (level, complexity meter, owner roles, API, checks, release). Done stops collapse to "Kickoff · 3 done" |
| Typography | Red Hat Display, Text and Mono (three families, render-blocking); body 13.5 px; 10 px uppercase labels; 11 px mono counts | System UI stack for text (no download, native hinting); JetBrains Mono for ids, numbers and prompts, the font the engine embeds for its HUD (Q19), loaded after first paint with `swap` and a `ui-monospace` fallback. Scale 12 / 13 / 14 / 16 / 20; nothing under 12 px; sentence-case labels |
| Contrast | `--faint` #6d7684 text 3.95–4.42:1 at 11 px; checkbox and input borders 1.36:1; to-do timeline nodes 1.82:1; dimmed rows (opacity 0.32) 1.7–2.6:1 | `--faint` #7f8998 (≥ 5.1:1 on every surface); control borders #5a6370 or lighter (≥ 3:1 on `--band`); no opacity on text: done rows use `--muted` (7.6:1); every pair checked in step 6 |
| Color as information | Effort bars in grays (reads as low complexity); "your time" in amber dots for every stop; the primary button and the sync dot used blue and green outside their meanings | Effort as text (`M`, then `6 steps` once a plan exists); amber only on owner roles and owner steps; neutral buttons; the sync state as text |
| Density | Route 6,924 px desktop, 8,629 px phone | About 47 rows of 40–48 px with done groups collapsed; the whole page under about 2,600 px on desktop |
| Phone | 814 px wide at 390 px; tabs wrap to two rows; long chips clip | One column with `minmax(0, 1fr)` tracks; chips wrap under the title; prompts wrap (`pre-wrap`) with a full-width Copy below; `scrollWidth` equals the viewport at 390 px |
| Keyboard, screen reader | Tablist without arrow keys; 49 focusable timeline buttons; a `role=checkbox` button drawn with "✓"; single-key shortcuts; no focus trap in dialogs | No tabs or dialogs; native checkboxes with labels; one `main`, an `h1`, an `h2` per section; rows as `<details>` disclosures; a polite live region for copy results; the 2 px blue focus ring (7.3:1) stays |
| Motion | Smooth `scrollIntoView` ignores reduced motion; a flash animation | No scripted scrolling or animation; 120 ms color transitions only, none under `prefers-reduced-motion` |
| Weight | 198.8 KB, three font families, 7,026 nodes | ≤ 45 KB, one mono family, ≤ 1,500 nodes, no render-blocking stylesheet |

Desktop layout, after:

```text
Tungsten 1.0 · 0.43 · W14a Headless test harness · plan not written         synced 7f3c2a1 · 5 Oct 14:02
┌ Next · new session ──────────────────────────────────────────────────────────────────────────┐
│ Write the milestone plan                                                              [Copy] │
│ Use the tungsten-milestone skill. Write the milestone plan for W14a: read …                  │
│ After that: approve (same session) · run (new session) · ship (you)                         │
│ Heads-up: …                                                       ▸ If something went wrong  │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
P5 Foundations  3/22 ◆ glyph+loop ◆   P6 Features  0/17 ◆ feature   P7 Release  0/8 ◆ freeze ◆ RC ◆ 1.0
Phase 5 · Foundations                                                            3 of 22 done
  Kickoff · 3 done                                                                   ▸ Show
  Track A · Foundations
  ● W14a        Headless test harness             B  ▮▮▮▯ High  M   new API · perf
  ○ W2 R0       Sprite IDs become numbers         C  ▮▮▮▯ High  L   API review · API break · perf · visual
  …
Open questions · 13                                                       needed by · default
  Q14  Logs and crash reports on by default?      W11a starts · asked in the W11a plan
Blue here · amber you · green done · gray→red complexity · violet API · cyan checks
```

## 7. Data model and db migration

| Document | Written by | Shape |
| --- | --- | --- |
| `meta/catalog` | Skill, from `scripts/roadmap.py catalog`, only when the hash changes | `{schema, hash, catalog}`; about 25 KB, under the 256 KiB document limit |
| `meta/now` | Skill, each sync | `{synced_at, commit, branch, release, stop, headline, next: {key, title, where, text}, flow: [{key, title, where, at}], after: [..], heads_up: [..], recovery: [{key, title, text}]}` |
| `stops/<id>` | Skill (`status`, `release`, `milestone`, `plan`, `plan_steps`, `evidence`); page (`steps` only) | Today's shape plus two counts. No document means `todo` |
| `questions/*` | Nobody | Deleted after the catalog carries their answers |

Ticks key as `<status>:<prompt key>` (`todo:plan`, `plan:approve`), so a status change hides the previous status's ticks, as today. Status comes from evidence alone, matched with the patterns `state.sh` and `roadmap.md` already use, so the reply and the page agree:

| Status | Evidence | Pattern |
| --- | --- | --- |
| `plan` | The register row's milestone plan file exists | the path in the row's Milestone plan cell |
| `run` | The plan's approval line | `^Approved` in the plan (the form `tungsten-milestone` writes) |
| `ready` | The register row names a cut | `cut` in the row's Status cell, with a Release |
| `review` | The release tag exists and the release is not on `origin/main` | `refs/tags/v0.NN.0`; no `^Update 0.NN:` in `origin/main`'s log |
| `done` | The release is on `origin/main`, or the gate's record is signed | `^Update 0.NN:` on `origin/main`; `^Sign-off: Signed` under the gate's `###` record |

Uncommitted evidence counts (`D-105` makes the tree the truth for a whole milestone). Remote evidence comes from local `origin/*` refs, which the skill never fetches (it is read-only); the owner's post-merge block runs `git fetch origin` before it creates the next branch, so a sync after it sees the merge. `status` prints the last fetch time, and when the newest local tag is not yet visible on `origin/main` it reports the stop as `review` with "unverified: fetch older than the tag" rather than guessing `done`.

The next release and milestone come from the tree, not from fixed bases: the release is the current branch's `0.NN` (the branch the post-merge block made), else `Cargo.toml`'s minor plus one; the milestone is one past the highest `M<n>` in the register's Milestone plan column, `M32` when there is none (README conventions).

Migration, in this order, so the live page never lacks data:

1. Write `meta/catalog` and `meta/now`, and refresh `stops/*`, in one `ArtifactData` batch (catalog by `file_path`, existing documents pinned with `if_version`). The current page ignores the unknown collection and the extra fields. `ArtifactData`'s limits, from its schema: `set`/`update` and batch entries accept `file_path`; each entry carries its own `if_version`, and a pinned batch is all-or-nothing; at most 50 entries and 1 MiB per batch; 256 KiB per document. A sync writes only changed documents, a handful per release; a write of more than 48 stops splits into batches of up to 50.
2. Read the live artifact, then publish `page.html` to the same URL, omitting `capabilities` so the stored `{"db":{}}` and contract 0.2.67 carry forward.
3. List `meta` and `stops`, open the page on desktop and phone, and confirm the current stop matches the register.
4. Save `questions/*` with `ArtifactData` `list` and `out_dir` to the execution session's scratchpad and quote it in the evidence log, then delete `questions/q1`, `q4`, `q8`, `q9`, `q11`, `q16`, `q24`, each pinned to its version, once the page shows those answers from the catalog. `s0`'s title-keyed ticks stay; the page ignores ticks on closed stops.

Rollback: until step 4, republishing the previous page restores everything, since it reads only `stops` and `questions`. Keep its source (`artifact-…-9707.html`) in the execution session's scratchpad until the owner accepts the new page.

## 8. Maintenance routine

| When | Who | Writes | If skipped |
| --- | --- | --- | --- |
| Each session | The session | Ends with the next prompt, as now | Nothing new |
| Between syncs | Owner, optional | Ticks on the page | The up-next marker waits for the next sync |
| After each merge | Owner runs `/tungsten-next` | Statuses, `meta/now`, the catalog if changed: no hand edits | The page shows the previous stop and its synced-at commit; the next sync recomputes from the tree, so nothing accumulates |
| Plan written or approved | Optional `/tungsten-next` | Status `plan` or `run` | As above |
| Gate or graduation | The gate or graduation session | `roadmap.json` in the same change as the register, cards and records: new stops, levels, pairs, groups, question states | `just repo-check` fails at the next `just check` or release |
| Prompt wording changes | The session that changes the workflow | `prompts.md` only | Nothing else to update |
| Page design changes | A session asked to | `page.html`, then a republish (read the live page first) | — |

## 9. Steps

Each step names its files, runs in one session, and leaves its work uncommitted (`D-105`).

1. **Catalog.** Write `docs/plans/1.0/roadmap.json` from the page's `PHASES`, `GROUP_DEFS`, `CLASS`, `PATCH`, `SHIPS_WITH`, `QUESTIONS` and `ANSWERED` and the db's question answers, per §3.1. Fold `PATCH` into each stop; drop `d`, `TODAY`, `CLOSES`, `GAME`, `SPIKES`, `custom` and `guide`. Order follows the register, not the page: Track B moves to after W1 M1. Done when it parses, holds 47 stops and 24 questions, and every id is unique.
2. **Check.** `check_roadmap` in `scripts/check-repo.py` (§3.2), with cases in `scripts/test-check-repo.py`: rows out of order, a missing row, a level that disagrees with its card, a missing `src` path, an unknown question id, a duplicate id. Done when `just repo-check` and `just script-test` pass and each case fails as intended.
3. **Prompts.** `prompts.md` gains the Key column, literal templates, the Flow table and the moved rows (§4); `workflow.md` §3's table becomes a link. Done when `rg -c 'audit only the code it touches' .claude docs` names one file.
4. **Helper.** `scripts/roadmap.py` (standard library only): `catalog` writes the `meta/catalog` payload with its hash to a given path; `status` derives each stop's status and counts from the register, plan files, approval lines, evidence rows, tags and `origin/main`, and prints the evidence for each. It also prints the next release and milestone (§7). `scripts/test-roadmap.py` covers both on a fixture tree, including a stale fetch (tag present, merge not visible), a branch that isn't `0.NN`, no milestone yet (`M32`), and an approval line in the plan. Done when `just script-test` passes and `status` on the tree gives `s040` and `gdef` done and `s0` ready before the 0.42 merge (done after), with every later stop `todo`.
5. **Skill.** SKILL.md §1 and §5 and `roadmap.md` describe the sync: run the helper, write one batch (catalog by `file_path` when its hash differs, changed `stops/*`, `meta/now` from the reply's own fields); never write ticks; republish only for a `page.html` change, after reading the live page. Drop `roadmap.md`'s id list and session keys. Done when `rg -n 's040, gdef' .claude` finds nothing and the skill's reply format is unchanged.
6. **Page.** `.claude/skills/tungsten-next/page.html`: renderer only, per §5 and §6, with the loading state and the per-viewer cache. Check it locally with Playwright against a stubbed `claude.use("db")` fed from the helper's payloads. Done when it is ≤ 45 KB; `rg -c 'w14a|Headless test harness|audit only' page.html` is 0; at 390 and 1440 px `scrollWidth` equals the viewport; FCP ≤ 200 ms at desktop locally; the DOM holds ≤ 1,500 nodes; every text color reaches ≥ 4.5:1 and every control border ≥ 3:1 on its surface; the console shows no errors.
7. **Db and publish.** Owner approves the writes. Migration steps 1–3 (§7); measure the live time from load to the first rendered row, and apply §3's fallback if it is over 1 s; then migration step 4. Done when `ArtifactData` lists `meta/catalog` with the same hash as the helper prints, the page shows the register's current stop, and the owner has checked desktop and phone.
8. **Records.** `docs/plans/1.0/README.md`: a Files row for `roadmap.json`, and a Conventions line saying it is data without plan headers. `workflow.md` §2: Gate and Graduation produce `roadmap.json` updates; §9: the `/tungsten-next` row. `docs/LLM_INDEX.md`: one row, "Road to 1.0 status, next prompt, roadmap page". D-109 if Q9 is accepted, with its index row. One `CHANGELOG.md` `[Unreleased]` line. Rewrite the memory note as a pointer: URL, db layout, page source in the repo, the sync rule. Done when `just ctx`, `just repo-check` and `just check` pass.
9. **Close.** `status: done`; archive per the plan rules.

## 10. Done-when checks

- `rg -l 'audit only the code it touches' .claude docs` lists `.claude/skills/tungsten-next/prompts.md` only.
- `page.html` contains no stop id, title, question or prompt text (step 6's `rg`), and is ≤ 45 KB.
- `just repo-check` fails on each case in step 2 and passes on the tree.
- A `/tungsten-next` run after the next merge changes the page with no file edits.
- Step 6's layout, timing, DOM and contrast numbers, quoted.
- `ArtifactData` lists `meta/catalog`, `meta/now` and `stops/*`; `questions` is empty; the URL is unchanged.

## 11. Open questions

Each has a default; "default" is a valid answer.

1. **When does this run?** Default: after 0.42 merges, on `0.43`, before W14a's plan. 0.42 is cut and its checks have run, so adding files now means rerunning them. Alternative: fold into 0.42 and rerun its release checks.
2. **Where does the catalog live?** Default: `docs/plans/1.0/roadmap.json`, beside the register that gate sessions already edit. Alternative: the skill folder.
3. **Ticks?** Default: keep them, owner only; the skill never writes them.
4. **Owner status edits on the page?** Default: removed; statuses come from the tree.
5. **Ratings?** Default: keep complexity (1–4), effort (S–XL), owner roles, flags and "shows"; drop "your time"; a plan's step count replaces effort.
6. **Fonts?** Default: system UI stack for text and JetBrains Mono for ids and prompts, loaded without blocking. Alternative: Red Hat Text and Mono, also loaded without blocking.
7. **Questions on the page?** Default: read-only; open ones listed, answered ones collapsed.
8. **Reference?** Default: cut, keeping the legend line.
9. **A decision entry?** Default: yes, D-109, short: the catalog with its check, `prompts.md` as the one prompt home, gate sessions owning catalog updates, the page as a db-fed viewer. Alternative: workflow §2 text only.
10. **Delete the `questions` collection?** Default: yes, after the new page is live and checked.

Approved 2026-10-04 with the stated defaults.

Changed 2026-10-04, after steps 1–6: Q1 takes its alternative. The work ships in 0.42: steps 1–6 already sit in the cut 0.42 tree, entangled with 0.42's own edits to `scripts/check-repo.py`, `scripts/test-check-repo.py`, `justfile` and `workflow.md`, so 0.42's release checks rerun after step 8, and step 8's `CHANGELOG.md` line goes in the 0.42 section. The owner approved step 7's db writes and publish, to run before the 0.42 merge.

## 12. Critique record

Critiqued on 2026-10-04 by `openai/gpt-5` through the user-level `critique` skill, against the first draft. Each finding was checked against the repo before it changed anything.

| # | Finding | Verdict | Change |
| --- | --- | --- | --- |
| 1 | [HIGH] The register-row equality check can never hold with folded pairs and shared placeholder rows | Rejected as stated: the register already keeps each pair and placeholder as one row. Checking it found a real drift: the page orders Track B after W2 R2, the register after W1 M1 | §3.2 says why the equality holds and that a register split must split the catalog; step 1 takes order from the register; §1 lists the drift |
| 2 | [HIGH] Status derivation mixes uncommitted state, approval lines, tags and possibly stale `origin/main` | Partly accepted. Restricting to committed state would contradict `D-105`; fetching would break the skill's read-only rule | §7 gains the evidence table with the exact patterns `state.sh` and `roadmap.md` use, the fetch rule (the post-merge block fetches), and an "unverified" `review` when the fetch is older than the tag; step 4's tests cover a stale fetch |
| 3 | [MEDIUM] The db write path assumes `file_path`, per-entry `if_version` and batch limits | Checked against `ArtifactData`'s schema: all three exist (50 entries, 1 MiB per batch, 256 KiB per document) | §7 records the limits and the split rule; `questions/*` is saved to the scratchpad and quoted before deletion |
| 4 | [MEDIUM] The `localStorage` cache has no schema or hash keying; the fallback adds work | Accepted | §3 keys the cache by schema and catalog hash, labels a cached view, and sets the threshold as five live loads (median ≤ 1 s, none over 2 s); it states that the fallback adds a republish per gate |
| 5 | [MEDIUM] Next release and milestone come from hard-coded bases | Misread (the plan drops the page's bases), but the derivation sources were unstated | §7 names them (branch `0.NN`, else `Cargo.toml` minor + 1; register `M<n>` + 1, else `M32`); step 4 tests them |

## Evidence log

| Step | Verdict | Numbers and paths |
| --- | --- | --- |
| 1 | pass | "Done when it parses, holds 47 stops and 24 questions, and every id is unique": parses; 47 stops (22 + 17 + 8); 24 questions (20 from `QUESTIONS`, Q17–Q19 and Q23 from `ANSWERED`); stop and question ids unique and disjoint. `docs/plans/1.0/roadmap.json`, 38,085 bytes, built from page version `1791086651-9707` and the 7 `questions/*` answers (source kept in the scratchpad as `artifact-7124cc1f-1791086651-9707.html`). Choices: `src` and `reads` are lists of `"<path>[ <section>]"` (pairs and W11a carry two sources); Track B follows W1 M1, so W8a and the M0b–R2 pair form group `track-a-late`; `c3` is kind `rc`, `g10` kind `gate`; gates carry `gate` (the prompt's name) and `reads` beyond §2 |
| 2 | pass | "Done when `just repo-check` and `just script-test` pass and each case fails as intended": `just repo-check` exit 0 ("Repository QA: 0 error(s)", both cargo test results ok); `just script-test` exit 0 (`test-check-repo.py` 24 tests OK, 9 of them `RoadmapChecks`). Cases in `scripts/test-check-repo.py`: rows out of order, a missing row, a register split until the catalog splits, a level against its card, missing `src` and `reads` paths, unknown question ids (not in §7 or criteria §10; a stop citing an undefined one), duplicate and malformed ids, a duplicate JSON key and an unknown group. The same six plan cases on a scratch copy of the real catalog each give exactly one error, e.g. `r0: level 'B', its §3 card says 'C'`. `check_roadmap` in `scripts/check-repo.py`; the register, card and question parsers in `scripts/roadmap.py`. Criteria §10 numbers questions `**<n>.**`, so the check accepts that form beside `Q<n>` |
| 3 | pass, this plan excluded | "Done when `rg -c 'audit only the code it touches' .claude docs` names one file": raw, it names `.claude/skills/tungsten-next/prompts.md:1` and this plan (`:2`, its own step 3 and §10 quote the phrase); with `--glob '!docs/plans/roadmap-artifact-rework.md'` only `prompts.md:1`. Once step 9 archives this plan, `.ignore` hides it and the raw command names one file. `prompts.md`: Key column (20 session keys, plus `ship` and `playthrough` for owner steps), every template literal with its `Don't commit` line except `answer`, `records-lag`, `compact`, `release`, `verify` and `game-feature`, a fill rule from `roadmap.json`, the Flow table, `rc-checklist`, `rc-fix`, `compact`, the `playthrough` owner step, and `game-feature` (workflow §3's acceptance-game row, which would otherwise be lost). `workflow.md` §3 keeps its first paragraph and links to `prompts.md`; the release row's skill links stay as a sentence. `just ctx` OK. Follow-up: Track B's first item (worker-thread capture rules) has no template; the page had one (`SPIKES.capture`) and §4 does not list it |
| 4 | pass | "Done when `just script-test` passes and `status` on the tree gives `s040` and `gdef` done and `s0` ready before the 0.42 merge (done after), with every later stop `todo`": `just script-test` exit 0 (`test-roadmap.py` 8 tests OK; `test-check-repo.py` 24 OK); `just repo-check` exit 0. `python3 -B scripts/roadmap.py status` on `0.42`: `s040 done 0.40 origin/main has "Update 0.40:"`, `gdef done 0.41 definition gate: signed`, `s0 ready 0.42 register 'Step 0'; cut for 0.42, tag v0.42.0 none`, the other 44 `todo`; next release 0.42, next milestone M32, last fetch 2026-10-03 21:56. `catalog` writes 29,453 bytes, hash `e26916c24b973c6d`. Tests cover a stale fetch (tag present, merge not visible: `review`, "unverified: fetch older than the tag"), a fresh fetch, the merge on `origin/main`, a branch that isn't `0.NN` and a detached head (Cargo minor + 1), no milestone yet (M32), a plan then its approval line (`plan` → `run`, 3 steps, 2 with evidence), a named plan missing from the tree, a two-record gate and the spike that feeds it, and the catalog hash. Additions outside the plan's file list: `"release": "0.40"` on `s040` in the catalog (it has no register row), and one `justfile` line so `script-test` runs `test-roadmap.py` |
| 5 | pass | "Done when `rg -n 's040, gdef' .claude` finds nothing and the skill's reply format is unchanged": `rg` exit 1, no output. Reply format: SKILL.md §6 untouched (the edits are in §1 and §5 only); the folder is untracked, so there is no `HEAD` copy to diff, and §6 reads the same 36 lines as at the session's start. SKILL §1 runs `scripts/roadmap.py status --json` and `catalog` into the scratchpad, lists `meta` and `stops`, and reads the named stop's catalog entry with `jq`; §5 summarises the sync. `roadmap.md` holds the db layout, the fields (statuses only from the helper; ticks `<status>:<key>`, never written by the sync), `meta/now` from the reply, the compare-and-batch sync, and the republish rule (only the session that changes `page.html`, after reading the live page). Its id list and session keys are gone. `just ctx` OK |
| 6 | pass | "Done when it is ≤ 45 KB; `rg -c 'w14a\|Headless test harness\|audit only' page.html` is 0; at 390 and 1440 px `scrollWidth` equals the viewport; FCP ≤ 200 ms at desktop locally; the DOM holds ≤ 1,500 nodes; every text color reaches ≥ 4.5:1 and every control border ≥ 3:1 on its surface; the console shows no errors": `page.html` 29,236 bytes; `rg -c` exit 1, no match (a scan for every catalog id, title and question and every prompt template finds only "Tungsten 1.0", the page's own heading, which is also `g10`'s title); `scrollWidth` 390/390 and 1440/1440 at load and with every disclosure open; FCP over five desktop loads 112, 116, 156, 184, 192 ms (median 156; an earlier run before the meter change gave 100–112); 884 elements, 1,405 nodes counting text (1,335 elements with every disclosure open); 893 visible text elements, minimum 5.83:1 (`--cx1` "Low"); 4 controls, minimum border 3.85:1 (button on `--band`); no console errors or page errors. Also checked: a tick writes `update stops/s0 {steps: {"ready:ship": true}}`; a repeat visit with no db answer renders the 31,541-character cache labelled "cached view · synced db1177c"; no db shows "This copy can't reach the roadmap"; a db that never answers shows the loading state. Harness: `measure.py` in the scratchpad, Playwright with the system Chromium 153 (no Playwright browser is installed), the page wrapped in the publish skeleton, `claude.use("db")` stubbed from `roadmap.py catalog`, `status --json` and a sample `meta/now` for today's state. Added on the way: a `label` per stop in `roadmap.json` (the candidate name the row shows; `row` is a placeholder for Phases 6 and 7). Not a done-when, and not met: §6's "whole page under about 2,600 px on desktop"; it is 4,387 px (7,475 at 390 px), the 13 open questions and the 44 open rows being most of it |
| 7 | pass (the five timed loads not given) | "Done when `ArtifactData` lists `meta/catalog` with the same hash as the helper prints, the page shows the register's current stop, and the owner has checked desktop and phone": hash `a793d3aed035c4a8` in the listed `meta/catalog` (44,559 bytes as saved, schema 1, 47 stops, 24 questions) and from `roadmap.py catalog` (30,251 bytes); the register's current stop is `s0` (Step 0, cut for 0.42, no tag), which `meta/now` names and a local render with the db stubbed from the same payloads shows in the header ("0.42 · Step 0 Graduation, amendment folds and Step 0 · ready to ship"); the owner accepted desktop and phone on 2026-10-04 ("Approved"). Migration step 1: one batch, committed atomically: `set meta/catalog` (by `file_path`) and `set meta/now`, both version 1; `stops/*` unchanged, since the helper's derived fields equal the listed documents (`s040` done 0.40 v2, `gdef` done 0.41 v2, `s0` ready 0.42 v8); 12 of 25,000 documents. Step 2: live version `1791086651-9707` (198,761 bytes, contract 0.2.67, `{"db":{}}`) read whole; the rollback copy is `~/.claude/projects/-home-joker-Projects-Tungsten/11fa8148-9423-4318-b5cc-2556a583cfef/tool-results/artifact-7124cc1f-1791086651-9707.html` (outside the tree, so the release commit skips it; the scratchpad copy is on tmpfs); `page.html` published without `capabilities` as version 14 (`1791090924-0a74`), then, with one `performance.mark("roadmap:rows:<source>")` per data source on its first rendered rows so live loads can be timed in the console, as version 15 (`1791091053-3d2e`), 29,440 bytes; the declaration and contract carried forward; same URL. Locally after the marks: `rg -c` exit 1, `scrollWidth` 1440/1440 and 390/390, no console errors, marks at 230–242 ms against a stub answering in 200 ms. Step 3: `meta` and `stops` listed as above. Open, handed to a new session: five timed live loads (median of `roadmap:rows:live` ≤ 1 s, none over 2 s, else §3's fallback), the owner's check, then migration step 4. Timing: after each reload, in the console with the artifact's frame selected (`document.title` is "Tungsten 1.0 Roadmap"), `performance.getEntriesByType("mark")` filtered to `roadmap:` names; `rows:live` is ms from the frame's start to the first db-fed rows, `rows:cache` the cached view from the second load on. Another Claude session (pid 709995) was open and idle in this tree throughout. Execution session, 2026-10-04: the owner approved the page without sending the five `roadmap:rows:live` times, so the median ≤ 1 s and none-over-2 s check has no numbers and §3's fallback was not applied; it stays checkable from the console marks. The helper still prints hash `a793d3aed035c4a8` (30,251 bytes); the rollback copy was present (194.1 K) and unused. Migration step 4: `questions/*` listed with `out_dir` into the scratchpad (`db/questions/`), 7 documents, each version 1: `q1` `{"answer":"A and B; one acceptance game: a top-down survivors-like auto-shooter (D-102)","status":"answered"}`; `q4` `{"answer":"R1 and R2; R4 to 1.x","status":"default"}`; `q8` `{"answer":"Sliders and checkboxes in as W1 BC; text fields and IME out","status":"default"}`; `q9` `{"answer":"In","status":"default"}`; `q11` `{"answer":"Hidden from the promise (D-103)","status":"default"}`; `q16` `{"answer":"In","status":"default"}`; `q24` `{"answer":"Inside the promise","status":"default"}`. Each equals its catalog entry's `state` and `answer`. One batch of seven `delete`s, each pinned to version 1, committed atomically. After: `meta` lists `catalog` (v1, 44,559 bytes, schema 1, hash `a793d3aed035c4a8`, 47 stops, 24 questions) and `now` (v1, 1,327 bytes, `0.42`, stop `s0`, commit `db1177c`); `stops` lists `gdef` v2, `s0` v8, `s040` v2; `questions`: "No documents matched". From here the rollback page has no question answers to read |
| 8 | pass | "Done when `just ctx`, `just repo-check` and `just check` pass": `just ctx` exit 0 ("Agent context checks: OK", self-test OK; `LLM_INDEX.md` 8,487 B of 12 KiB, root `AGENTS.md` 5,343 B); `just repo-check` exit 0 ("Repository QA: 0 error(s)", both cargo test results ok); `just check` exit 0 (27 `test result: ok`, no failure, error or warning). Edits: `docs/plans/1.0/README.md`, a Files row for `roadmap.json` and a Conventions "Catalog" line (data, no plan headers, checked against the register); `workflow.md` §2's Gate and Graduation rows produce `roadmap.json` updates with the register, §9's `/tungsten-next` row, and a revision line; `docs/LLM_INDEX.md`, the row "Road to 1.0 status, next prompt, roadmap page (`D-109`)"; `D-109` with the tungsten-decision skill (next free ID; amends no decision) and its `docs/DECISION_INDEX.md` row under Dependencies / Tooling; one `CHANGELOG.md` line at the end of 0.42's Changed (Q1's alternative, not `[Unreleased]`); the memory note `roadmap_artifact.md` rewritten as a pointer (URL, db layout, page source in the repo, the sync rule) and its `MEMORY.md` line. Left for the release session: 0.42's summary line and the 1.0 README "Now" lines name Step 0 only; the release session, 2026-10-04, added `D-109` and this plan to both and to DESIGN's status line |
| 9 | not run: §10's sync check waits for the 0.42 merge | §10 after step 8, 2026-10-04: (1) "`rg -l 'audit only the code it touches' .claude docs` lists `.claude/skills/tungsten-next/prompts.md` only": raw it also lists this plan, which quotes the phrase; with the plan excluded, `prompts.md` only; archiving the plan makes the raw command pass. (2) "`page.html` contains no stop id, title, question or prompt text (step 6's `rg`), and is ≤ 45 KB": `rg -c` exit 1, 29,440 bytes. (3) "`just repo-check` fails on each case in step 2 and passes on the tree": passes (step 8); `just script-test` exit 0, five suites OK (`test-check-repo.py` 24 tests, `test-roadmap.py` 8). (4) "A `/tungsten-next` run after the next merge changes the page with no file edits": open until 0.42 merges and the owner's post-merge block fetches. (5) "Step 6's layout, timing, DOM and contrast numbers, quoted": step 6's row. (6) "`ArtifactData` lists `meta/catalog`, `meta/now` and `stops/*`; `questions` is empty; the URL is unchanged": step 7's row. Remaining: the post-merge `/tungsten-next` sync (`s0` to `done`, `meta/now` naming W14a's plan) quoted for (4), then step 9: `status: done` and the archive move |
