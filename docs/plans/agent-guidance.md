# Agent guidance (7.16) — in progress

- **status:** in progress
- **goal:** Research how engines and repositories guide agents (instruction files, task guides, shipped skills) against 7.13g's evaluation, its 26 runs and its 94 seam rows and entries; decide what the template ships; land it in `templates/basic/`, so `tungsten new` writes it into every game and 8.5's evaluation meets it; show fresh agents in a new copy reaching it; and record the decision with that evidence (the [roadmap redo](roadmap-redo.md)'s 7.16 row, §2's "Guides" answer and §8; [phase 7](phase-7.md)'s Step 16, with its rows EV-40 and EV-63).
- **non-goals:**
  - Code: the engine, the kit, the CLI and `D-291`'s harness. A tool the verdicts name (a `tungsten` verb, an MCP server, §8's `new` scaffolds) is recorded with its home, 8.5's scaffolds or Later, and not built here.
  - A new evaluation pass or an A/B trial (Q1): 8.5's pass is the first to measure the shipped guidance, and Step 5's sessions show that agents reach it, functional only (`D-226`).
  - `D-291`'s harness, frames, briefs, fixtures and records, and the transcript archive: read only, hashed at Step 1 and checked at every step after, since 8.5 and 8.6 reuse them (`D-293`). A frame change for 8.5 is a recommendation to its plan (Q2), not an edit.
  - The repository's own instruction files, LLM index and skills, which guide 01's agents, past what Step 4's landing forces: the reading's findings on them go to Later or known issues (Q3).
  - Client configuration a game might ship (permission allow lists, hooks, MCP configuration): the survey records what others ship, and shipping any is Later's, as `docs/agent-setup.md` defers it for the repository.
  - 8.5's template final pass and 9.5's docs triage, which take this step's decision.
- **files to touch:**
  - Step 1: this plan (status, evidence row). Untracked: `perf-runs/716-guidance/` (gitignored).
  - Step 2: this plan. Untracked: `perf-runs/716-guidance/`.
  - Step 3: this plan (§2.4's table).
  - Step 4: of these, the ones Step 3's verdicts name: `templates/basic/AGENTS.md`, `templates/basic/README.md`, `templates/basic/.claude/skills/<name>/SKILL.md`, `templates/basic/.agents/skills/<name>/SKILL.md`, `docs/getting-started.md`, `docs/levels.md`, `docs/assets.md`, `scripts/check-agent-context.py`, `scripts/check-repo.py`, `scripts/test-check-repo.py`, `docs/validation.md`; always `docs/agent-setup.md` and this plan.
  - Step 5: this plan; for a revision or a withdrawal, Step 4's files. Untracked: `perf-runs/716-guidance/demo/`, `target/716-guidance/`, each try's engine repository and workspaces under `/home/joker/tungsten-eval/716/`.
  - Step 6: `DECISIONS.md`, `docs/DECISION_INDEX.md`, this plan.
  - Step 7: this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md`, `docs/known-issues.md`, the files that cite this plan.
- **ordered steps:** (1) The reading: the evaluation's read paths and seams. (2) The survey: how engines and repositories guide agents. (3) The verdicts. (4) The template's guidance. (5) The demo in a fresh copy. (6) The decision. (7) Close.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes and the demo works (Step 5: in fresh `tungsten new` copies outside the repository, on a Git dependency, Claude Code and Codex list the shipped guidance, its pointer to the engine follows the version the game pins, and a task session's delivered results hold each shipped form's marks before its first edit, a form none reaches withdrawn alone before the decision is recorded); what the template ships decided and recorded (phase 7's Step 16); EV-40 and EV-63 closed; the plan deleted with its citations and its line out of Now.

Item `agent-guidance` in [ROADMAP.md](../../ROADMAP.md), from phase 7's Step 16 (`docs/plans/phase-7.md:601-604`) and the redo's 7.16 row (`docs/plans/roadmap-redo.md:139`), with §2's Guides answer (`:72`) and §8 (`:523-548`). Written 2026-11-25 at `d188da18`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

- 7.16 is the redo's research-and-decision step for agent guidance. The owner answered "Guides: unsure" (§2), so the step researches before it decides. 8.5's template final pass carries "7.16's shipped guidance", and 9.5's docs triage uses it.
- It rests on 7.13g's evaluation (`D-291`, `D-293`): thirteen fresh headless agents a pass, two passes, in a `tungsten new` copy and in example 01; 101 and 88 friction rows, merged into EV-1 to EV-66 with 28 entries on existing IDs; of pass 1's 23 fixed seams, 15 gone in pass 2, 2 not met and 6 recurring. Two rows sit at 7.16, EV-40 and EV-63, both wrong turns kept "for 7.16's research".
- Today the template ships `AGENTS.md` (12,287 of `D-183`'s 12,288 bytes, its kit catalogue held complete by `D-192`'s check), a `CLAUDE.md` that imports it, `README.md` and `tools/README.md`, and no skills; it sends agents to the engine's guide and cookbook at the tag, on GitHub. The repository itself ships six skills, linked into `.agents/skills` for Codex (`D-068`).
- The evaluation's frame handed each agent the engine's local docs path; a game's own `AGENTS.md` gives none, though Cargo keeps the engine's tree at the pinned version on disk. The plan session's probe found every copy run naming engine files in its tool calls (the guide, `api/` listings, sources), no run calling a skill, the web or a subagent, and 13 reads cut to a 2 KB preview with no follow-up (A9, A17).
- The decision is recorded last, after the demo, so a form no fresh agent reaches is withdrawn alone rather than recorded and reversed; each try of the demo builds its own CLI from the commit it tests, and reach counts only a form's own marks in a delivered result.
- Decisions: `D-068`, `D-106`, `D-183` with `D-224`, `D-192` (budgets, skills, the catalogue check); `D-291`, `D-293` (the harness); `D-237` (xhigh); `D-226` (functional runs); `D-150` (no approval stops).

## 1. Audit

Read at `d188da18`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The template's instruction files: `AGENTS.md`, 12,287 bytes in 59 lines, against `just ctx`'s 12,288 and 100 (`D-183`); `CLAUDE.md` the import alone | `scripts/check-agent-context.py:27`, `:154-157`; `templates/basic/AGENTS.md` |
| A2 | It sends agents off the machine for the engine's docs: the cookbook "in the engine repository at `Cargo.toml`'s tag", the guide "on GitHub", the API by `cargo doc` | `templates/basic/AGENTS.md:41`, `:58`, `:59` |
| A3 | `D-192`'s check holds it to name every scene component, data kind and cue event that `api/tungsten-kit-components.txt` lists, which fills its "The kit" section | `scripts/check-repo.py:153-168`; `templates/basic/AGENTS.md:32-41` |
| A4 | `tungsten new` writes every file under `templates/basic/` but `target/` and `dist/`, dot-folders included; a test holds the embedded list to `git ls-files`; every text file's template names are rewritten. The walk follows a symlinked folder that `git ls-files` lists as one path, so a symlink in the template fails that test | `tools/cli/build.rs:54-71`, `:133-144`; `tools/cli/src/tests/new.rs:132-175`; `tools/cli/src/new.rs:15-17`, `:166-172` |
| A5 | `tungsten package` archives `tungsten.json`, `input.json`, `assets/` and `LICENSE*` alone, so no guidance file reaches players | `tools/cli/src/package.rs:255-263` |
| A6 | The repository's skills: `.claude/skills/<name>/SKILL.md` with a `.agents/skills/<name>` symlink for Codex; frontmatter `name` and a description of at most 300 characters, a body of at most 8 KiB. `just ctx` checks the root's skills alone; the template has none | `scripts/check-agent-context.py:44-47`, `:105-139`; `docs/agent-setup.md:17-21` |
| A7 | The CLI reads the game's Cargo graph (`cargo metadata` in its folder) and holds its engine to the CLI's own version; `just template-check` and the evaluation's harness rewrite a copy's engine lines from the tag to a path (EV-45), so neither meets a Git dependency's checkout | `tools/cli/src/cargo.rs:17-32`, `:38`; `scripts/template-check.sh:78` |
| A8 | The evaluation's copy frame names the guide's local path and "the engine paths those docs name"; `D-293`(4) keeps the frames for 8.5 and 8.6 | `perf-runs/713g-eval/harness/frame-copy.md:5`; `DECISIONS.md:4600` |
| A9 | The plan session's probe of the 26 runs' transcripts, counting tool calls whose input names a path (functional only; Step 1 recounts): of the 16 copy runs, 15 name the guide, 10 the `api/` listings, 12 engine sources and 5 the kit's fixtures; none of the 26 calls a skill, the web or a subagent; in the 14 copy runs that edited with Edit or Write, the first edit falls at the 18th to 51st tool call, 18 to 60 % of the run's calls (the two prefab runs edited by `sed -i`) | `~/.claude/projects/-tungsten-eval-archive/{1,2}/` |
| A10 | Of pass 1's 23 fixed seams, 15 were gone in pass 2, 2 not met and 6 recurring. Fixes where the agents searched held (EV-25, EV-32 and EV-34, each read at the fixed line); the recurrences include a fix off the reader's path (EV-16's, off 01's, as EV-47) and a guide sentence left unread (EV-10's, as EV-55) | `git show c184bb31:docs/plans/example-01-evaluation-rerun.md`, §2.10 |
| A11 | Both passes' doc fixes went into the template's `AGENTS.md`, the guide, `docs/levels.md` and 01's tools guide; `AGENTS.md` has sat at its cap since | `git show --stat 3afadd0c 3f2d95d2` |
| A12 | Phase 7 holds EV-40 and EV-63 at 7.16, open, each "a record for its research on what fresh agents trip on"; Step 16 still reads "Rows: none" | `docs/plans/phase-7.md:340`, `:363`, `:367`, `:603` |
| A13 | `docs/agent-setup.md` covers the repository's discovery and gives the template one line; its "Verifying a client" checks are the listing probes Step 5 reuses | `docs/agent-setup.md:8`, `:38-47` |
| A14 | The guide is 67,186 bytes and a maintained doc under `just repo-check`; its Read next names the API, assets, levels and design | `scripts/check-repo.py:20-25`; `docs/getting-started.md:331-336` |
| A15 | `perf-runs/` is gitignored. `D-291`'s harness and records sit in `perf-runs/713g-eval/` (584 MB), each run's record holding a copy of its transcripts; the archive is `~/.claude/projects/-tungsten-eval-archive/` (51 MB) | `.gitignore:31`; `perf-runs/713g-eval/runs/2/copy.status/a1/transcripts/` |
| A16 | The harness's passes are sequential: `setup -p N` moves pass N−1's export and builds aside, and its frame is one file per workspace kind, so Step 5 runs outside it | `perf-runs/713g-eval/harness/eval.py:362-366`, `:1008-1013` |
| A17 | A tool result past the client's limit arrives as a 2 KB preview, the rest saved to a file: pass 2's `copy.status` asked for the whole guide (63.5 KB) and got its first 2 KB. The plan session's probe counted 13 such previews in the 26 runs and no later read of a saved remainder | `perf-runs/713g-eval/runs/2/copy.status/a1/transcripts/997a6f57-4fdf-4bdf-ad61-729347b9ebc7.jsonl:47` |

## 2. Design

### 2.1 What is decided

Four questions, each answered by the verdicts (Step 3), proved by the landing and the demo (Steps 4 and 5) and recorded in the decision entry (Step 6):

1. What a game made by `tungsten new` ships for agents: which files, which clients load them, and each one's budget.
2. Where a guidance fix from a later evaluation goes (8.5's and 8.6's doc seams): the fix-placement rule (§2.6).
3. What 8.5 checks of the shipped guidance, and what 8.5's plan should weigh in `D-291`'s frame (Q2).
4. EV-40's and EV-63's dispositions (§2.9).

### 2.2 The reading: the evaluation's read paths and seams

Inputs, read only:

- the 26 runs' records, `perf-runs/713g-eval/runs/{1,2}/<task>/a1/`: `transcripts/` (the session's and any subagent's), `log.md` (the friction log), `counts.json`, `prompt.md`, `diff.txt`;
- the seam tables as the plans closed them, `git show 9bc05f39:docs/plans/example-01-evaluation.md` and `git show c184bb31:docs/plans/example-01-evaluation-rerun.md` (§2.11's rows, §2.12's entries, the rerun's §2.10), with phase 7's EV rows for the final statuses;
- the frames, `perf-runs/713g-eval/harness/frame-copy.md` and `frame-e01.md`.

`perf-runs/716-guidance/mine.py` (standard library) does the counting; the classes are judged and written to `seams.md`, which `mine.py check` validates.

**Hash.** First of all, `mine.py hash` writes `eval-hash.json`, a SHA-256 of every file under `perf-runs/713g-eval/` and `~/.claude/projects/-tungsten-eval-archive/`; `mine.py hash --check` exits 1 on any change, and every later step runs it.

**Reads** (`mine.py reads` → `reads.json`, `reads.md`), per run: each guidance source a tool call names (the game's instruction files; the engine's guide, by heading; `docs/levels.md`, `docs/assets.md`, `DESIGN.md`; `api/*.txt`; the kit's fixtures; rustdoc and `cargo doc`; engine sources by crate; 01's README and tools guide), with its call count and first minute; tool calls and minutes before the first edit (an Edit, a Write or a shell write to the game's files); Skill, web and subagent calls. Sessions and subagents are counted apart and summed, turns by `D-291`'s rule (assistant messages by message ID).

**Delivered, not requested.** A read is credited with what its tool result delivered: a guide heading is read when the result holds its text, not when the call asked for its range. A result cut to a preview (A17) credits the preview alone, and the saved remainder only when a later call reads that file; a search credits the lines its result shows. `reads.md` counts the previews, their sizes and their follow-ups apart, and no class or form count credits an answer the agent was never shown.

**Classes** (`seams.md`): one per EV row and per entry, with three cells beside it: where the answer stood at the run's base (`path:line`, or `nowhere`), what the agent had been shown at the moment (the delivered results of its calls before the friction's minute, or the row's evidence), and which of §2.4's forms would have put the answer there.

| Class | When |
| --- | --- |
| `none` | not guidance's: the machine, the session or the harness; a bug or gap whose fix is code; a choice the brief left open |
| `tool` | a command the task ran printed the answer, or would have with a better message |
| `rule` | one always-loaded line holds the answer, and it applies across tasks |
| `task` | the answer belongs to one kind of task (an enemy, a room, a sound, a status, a broken prefab, a test of one) and runs past a line |
| `item` | the answer belongs in one item's own text (its rustdoc, a kit field's description, a constant's doc), where the agent was reading that item |
| `read` | a tool result had delivered the answer's line to the agent, and it went another way: the wording failed, not the place |

An entry is new evidence on an existing ID, so the counts report rows and entries apart and by distinct ID.

**Verifier.** A fresh subagent re-judges every row and entry classed `rule`, `task` or `read`, and every fifth other one in order (EV-5, EV-10, …; entries 5, 10, … of each pass), from the row, its log rows and its transcript's delivered results, given the class table and not §2.4's forms. Its verdicts go to `verify-reading.md`; a changed class is taken with its reason.

**Summary** (`reading.md`): rows, entries, tasks hit and logged minutes per class; per form of §2.4, the rows it would have reached; the recurrences' classes (EV-44, EV-46, EV-47, EV-49, EV-54, EV-55) against where each pass-1 fix landed; orientation per run (calls and minutes before the first edit); the previews and what they cut; the frame's own share (reads of the paths the frame names, which a game's own guidance does not).

### 2.3 The survey: how engines and repositories guide agents

The questions are fixed here, so the reading's answer does not shape them; the reading may add questions, each logged with its reason.

| # | Question |
| --- | --- |
| Q-a | Instruction files: what Claude Code and Codex load from a project (files, order, imports, nesting, size limits), which other clients read `AGENTS.md`, and what changed since `docs/agent-setup.md`'s notes of 2026-10-12 and 10-29 |
| Q-b | Skills: the Agent Skills format; each client's project skill folders; whether a headless session in a folder it never trusted loads them; how a skill is invoked (by its description, its name, an instruction); measured invocation rates |
| Q-c | Generators and frameworks: what new-project generators and packages ship for agents (files, sizes, contents), and how they keep it to the version; at least three, found by search |
| Q-d | Game engines: what Godot, Unity, Unreal and Bevy, and any engine found shipping guidance in its templates, ship or document for agents (instruction files, skills, MCP servers, docs written for agents) |
| Q-e | Evidence: measured effects of context files, skills and docs indexes on agents' task success, cost or turns, each with its method and sample |
| Q-f | Versions: how shipped guidance stays true to the version a project builds against, and after it upgrades |

**Method.** Web search and fetch, or a research subagent, primary sources first (a vendor's docs, a paper, a repository's file at a commit), a secondary source marked as one. The installed clients' own help and versions count as sources for Q-a and Q-b. **Queries name no project, person, email or path from this machine**; they state the question in general terms. Each finding is a row of `survey.md`, `| V-n | Q | Claim | URL | Published | Accessed | Quote |`, the quote 25 words at most; a question with no source gets a `none found` row naming its searches.

**Verifier.** A fresh subagent fetches each URL and marks each claim `confirmed`, `not found` (the page fails or lacks the quote) or `contradicted`, in `verify-survey.md`. Only `confirmed` claims weigh in §2.4, and the decision cites no other.

### 2.4 Forms, criteria and the verdicts

The forms guidance can take; they combine, and today's guidance unchanged is an outcome too:

| Form | What ships |
| --- | --- |
| today | `AGENTS.md` with the rules and the kit catalogue, always loaded; the guide and cookbook at the tag on GitHub; rustdoc by `cargo doc` |
| index | `AGENTS.md`'s rules with a short map of where each kind of answer lives, pointing at the engine's own files at the version the game builds against, on disk: the guide's sections, the `api/` listings, the kit's fixtures |
| skills | Task guides as skills, one per §8 task kind, in each supported client's project skill folder |
| guide | Task guides as sections of the engine's guide, at its version, named from `AGENTS.md` |
| copy | Version-matched docs copied into the game by `tungsten new` |
| tool | A command that prints the answer or its path (a `tungsten` verb, an MCP server, a scaffold); recorded with its home, never built here |

| Criterion | Test |
| --- | --- |
| read path | It puts the answer where the reading's agents were looking at the moment of need, in a size a result delivers whole; its count is the rows it would have reached, by class |
| clients | Claude Code and Codex, which the repository supports (`D-068`), with no symlink (A4), and plain Markdown for any other agent |
| version | True for the engine the game builds against, offline, and still true once the game moves to a newer engine, since `tungsten new` copies once (A4) |
| drift | A command holds it to the engine, as `just ctx` and `D-192`'s check do (§8: tools before docs) |
| cost | Always-loaded bytes, paid every session; on-demand text, paid when used, at the survey's measured invocation rate |
| ownership | The sample's lines and the shell's stay separable, since a game deletes the sample and a later template replaces the shell whole (`D-181`) |
| evidence | A confirmed survey measurement or the reading's counts support it; a form the survey finds measured as unused ships only if Step 5 shows it reached |

The table, filled at Step 3 from `reading.md` and `verify-survey.md`, each cell a number or a claim ID:

| Form | Read path | Clients | Version | Drift | Cost | Ownership | Evidence | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| today | | | | | | | | |
| index | | | | | | | | |
| skills | | | | | | | | |
| guide | | | | | | | | |
| copy | | | | | | | | |
| tool | | | | | | | | |

**Rule.** A form is landed when its read-path count holds rows today's guidance missed, it meets the clients, version, drift, cost and ownership tests, and its evidence shows no measured harm; one that serves one client only is landed when the verdict says why. The tool form's verdict is a home. A verdict on part of a form (one skill, one index line) names the part, and the Verdict cells name the files Step 4 lands.

**The entry** is written at Step 6, after the demo, with the next free ID (`D-31[2]` at writing), rechecked against `DECISIONS.md` and `git status` just before the commit, since another session can take it. It states what ships, with budgets and clients, exactly the guidance files and sections the template holds after Step 5; what does not, and why, a withdrawn form with its demo rows; the evidence by claim ID and the reading's counts; the checks that hold it; the fix-placement rule; what 8.5 checks and what its plan should weigh in the frame; EV-40's and EV-63's dispositions. The entries it changes get their marker lines in the same change, by the tungsten-decision skill, and only where it changes them: `D-183` (the budget, `D-224`'s supersession kept), `D-192` (the catalogue's home), `D-068` (skills in the template).

### 2.5 The landing

Step 4 lands the verdicts' files under these rules:

- **No symlinks in the template.** A skill shipped for two clients is two byte-equal copies that `just ctx` holds equal, unless the survey confirms one folder both clients read (A4, A6).
- **Names.** Guidance names the game as "this game" or by its folder, never by the template's package, crate or title, which `new` rewrites (A4).
- **Budgets.** `just ctx` holds every shipped guidance file: `AGENTS.md` to `D-183`'s 12,288 bytes and 100 lines unless the verdicts set another; a template skill to the repository's skill rules (frontmatter `name` its folder, a description of 1 to 300 characters, a body of at most 8 KiB); the self-test fails a case for each new check.
- **The catalogue.** `D-192`'s `check_kit_catalogue` follows the kit catalogue to its new home; if the catalogue leaves the copy for the engine's `api/tungsten-kit-components.txt` at the game's version, the check is rewritten to hold the pointer instead, with the verdict's reason, and its test changes with it.
- **Versions.** A shipped line states a fact that stays true when the game moves to a newer engine, or points at the engine's own file at the version the game builds against, by a means that works for a Git dependency (Cargo's checkout), not a fixed path or the GitHub page.
- **Sections.** Each form's text in a file another form also changes (`AGENTS.md`, `docs/agent-setup.md`, the guide) sits under headings of its own, so the demo can revise or withdraw one form alone (§2.7).
- **Sizes.** A file a shipped pointer sends agents to whole is named with the way to read it in parts (its headings by search, then a section), since a whole read past the client's limit arrives as a 2 KB preview (A17).
- **`docs/agent-setup.md`** gains a section, "Games made from the template": what ships, how each client loads it, the budgets, the fix-placement rule, and how to verify a copy (§2.7's checks).
- **The sample.** Lines that describe the sample stay marked as the sample's, which the guide tells a new game to rewrite (`docs/getting-started.md:41`).

### 2.6 The fix-placement rule

From the reading's classes and the recurrences: a guidance fix goes where the agent was looking at the moment of need, by class (`tool` the command's message, `rule` the instruction file, `task` its task guide, `item` the item's doc, `read` a rewording in place), and a fix for one workspace's agents lands on that workspace's read path, as EV-47 shows (EV-16's fix landed off 01's path). The decision states the rule as the reading's evidence shapes it, and `docs/agent-setup.md`'s section carries it for 8.5's and 8.6's doc seams.

### 2.7 The demo

- **Tries.** A try tests one commit of the template's guidance: Step 4's commit first, then a revision's or a withdrawal's. Each try builds from its own commit, since `tungsten new` writes the template its CLI embedded at build time (`tools/cli/build.rs:85`, `tools/cli/src/new.rs:166`), and its rows record that commit.
- **The engine as a Git dependency.** A try's export of its commit (`git archive`, this plan left out) becomes a scratch Git repository of two commits at `/home/joker/tungsten-eval/716/try-<n>/engine-git/`, made through Python, as `D-291`'s harness makes its workspaces, since the shell's deny list refuses `git init`: commit A the export, commit B the same with one marker line added at the end of `docs/getting-started.md`. The try's CLI is built from that export into `target/716-guidance/try-<n>/cli`.
- **Copies** under `/home/joker/tungsten-eval/716/try-<n>/`, outside the repository, since Claude Code reads every `CLAUDE.md` above its folder: fresh `tungsten new` copies from the try's CLI, whose two engine lines read `git = "file:///home/joker/tungsten-eval/716/try-<n>/engine-git", rev = "<A>"` in place of the release tag, never a path, so Cargo checks the engine out under its own home as it does for a tag; each copy a Git repository of one commit, made the same way. One warm build a try, at `CARGO_TARGET_DIR=target/716-guidance/try-<n>/game`, fetches the engine (a `file://` source, no network); the sessions then run with `CARGO_NET_OFFLINE=true`.
- **The version check** (`demo/version.sh`), each try, when the shipped guidance points at the engine's files: in a copy at rev A, the pointer followed as the guidance writes it names a folder of Cargo's checkout of A whose guide lacks the marker; with the engine lines moved to rev B and `cargo metadata` run again, it names B's, whose guide holds it. A pointer that names a fixed path, the GitHub page or the CLI's own version fails.
- **Listing checks** in a fresh copy: `docs/agent-setup.md`'s "Verifying a client" with skills added, `claude -p 'Without tools, list the instruction files and skills loaded into your context.'` and `codex exec --sandbox read-only 'Without tools, list the AGENTS.md files and skills you were given.'`. A model's self-report can be wrong (`docs/agent-setup.md:47`), so each shipped skill also gets a session asked to use it by name, its transcript showing the call.
- **Task sessions:** `copy.status` and `copy.room` from `perf-runs/713g-eval/harness/briefs.json`, each in its own copy, under `D-291`'s isolation (the project's memory folder moved aside and a planted sentinel shown absent, the session's `CLAUDE*` variables dropped, no appended system prompt), at Opus 5.5 and xhigh (`D-237`), with the harness's launch flags (`perf-runs/713g-eval/harness/eval.py:460-462`), each capped at 45 minutes and USD 15. The frame is `D-291`'s copy frame with its Read sentence naming no doc path ("the game's folder, what its guidance leads to on this machine, rustdoc and the build folder"), its Never list kept, the scratch engine repository's history and the engine's `templates/` in it.
- **Marks and reach.** Step 4 writes each landed form's marks to `perf-runs/716-guidance/demo/marks.json`: a skill's name; a guide section's heading line; for an index line, the heading or line of the target it names. A form is reached when a session's delivered result holds one of its marks before the session's first edit (a skill by its call), by `mine.py reads --demo`. A read of the right file that holds no mark, such as a preview that stops short of the section (A17), does not count. Each landed form needs a session that reaches it.
- **Revise, then withdraw.** Each form's text in a shared file sits in sections of its own (§2.5), so one form can change or leave alone. A form no session reaches in try 1 gets one revision of its own sections and files, as each of H2's lean skills got one, committed, and try 2 runs on that commit with the same two briefs. A form still unreached is withdrawn: its own sections and files removed, and its part of `docs/agent-setup.md`'s section with them, in a `revert(template): …` commit. The forms left keep their sections byte for byte as they were tried (`mine.py sections --check <tried commit> <withdrawal commit>`), so their reach stands from the try that tested them, and a last try on the withdrawal commit runs the version and listing checks alone. Step 6 records a withdrawn form as not shipped, with its rows. So nothing ships unreached or untried.
- **Functional only** (`D-226`): two sessions measure nothing, and pass 2's `copy.status` and `copy.room` counts (9.4 and 7.6 minutes, USD 3.88 and 3.12) sit beside theirs as context alone.
- **When the verdicts land no new guidance** (today's kept): the listing checks run alone.

### 2.8 Records

`perf-runs/716-guidance/` (gitignored, A15) holds `mine.py`, the hash, the reading's and the survey's files and the verifiers' and the demo's records, kept for 8.5. The tracked record is the decision entry, `docs/agent-setup.md`'s section and this plan's evidence log, which quotes the numbers; phase 7's evidence row takes them at the close.

### 2.9 EV-40 and EV-63

Each gets the reading's class. At the close its status cell reads `landed <sha>` when a shipped line answers it, else `closed (research record, <class>)`, with the decision's reason.

### 2.10 Budget and time

- The reading and the survey each take a fresh verifier (the evaluation's verifiers ran 455k to 601k tokens each), the survey a research pass of about that size, and Step 5 at most three tries: two task sessions in the first and in a revision's, the listing checks and a skill session each in every try, an engine build each. Estimate: 4 to 9 % of the Claude week, in one session of about six to seven hours.
- Gates: no subagent or session starts past 90 % of the week (`perf-runs/713-chain/spend.py`'s estimate, `D-291`'s gate), and no Step 5 build or session with under 30 GB free.

## 3. Steps

Before each step, `just preflight docs/plans/agent-guidance.md --step N`, reading each file it lists. Each step commits its own paths and pushes (`git push origin 0.120`, the whole command).

### Step 1: The reading

- **Files:** this plan (`status: in progress`, the evidence row). Untracked: `perf-runs/716-guidance/mine.py`, `eval-hash.json`, `reads.json`, `reads.md`, `seams.md`, `verify-reading.md`, `reading.md`.
- **Change:** §2.2 in its order: the hash, the reads, the classes, the verifier, the summary.
- **Done-when:** `python3 -B perf-runs/716-guidance/mine.py hash` → exit 0, before any other command; `python3 -B perf-runs/716-guidance/mine.py reads` → exit 0, 26 run rows, 13 a pass, each with its previews and their follow-ups counted; `python3 -B perf-runs/716-guidance/mine.py check` → exit 0: `seams.md` holds EV-1 to EV-66 once each and the 28 entries (pass 1's 5, pass 2's 23), each with one class of §2.2's six and its three cells filled, and `verify-reading.md` a verdict for every one §2.2's rule picks; `python3 -B perf-runs/716-guidance/mine.py hash --check` → exit 0.
- **Commit:** `docs(plans): record 7.16's reading of the evaluation`
- **Moves:** none.

### Step 2: The survey

- **Files:** this plan (the evidence row). Untracked: `perf-runs/716-guidance/survey.md`, `verify-survey.md`.
- **Change:** §2.3: the reading's added questions logged, the survey, the verifier.
- **Done-when:** `python3 -B perf-runs/716-guidance/mine.py survey` → exit 0: every claim row parses with a URL, both dates and a quote of 25 words or fewer; each of Q-a to Q-f has a claim `verify-survey.md` marks `confirmed`, or a `none found` row naming its searches; every claim has a verdict; `python3 -B perf-runs/716-guidance/mine.py hash --check` → exit 0.
- **Commit:** `docs(plans): record 7.16's survey`
- **Moves:** none.

### Step 3: The verdicts

- **Files:** this plan (§2.4's table, the evidence row).
- **Change:** §2.4's table filled from `reading.md` and `verify-survey.md`, each Verdict cell naming the files Step 4 lands or the form's home.
- **Done-when:** §2.4's table has no empty cell; `python3 -B perf-runs/716-guidance/mine.py cited --table docs/plans/agent-guidance.md` → exit 0, every claim ID in the table `confirmed`; `python3 -B perf-runs/716-guidance/mine.py hash --check` → exit 0.
- **Commit:** `docs(plans): record 7.16's verdicts on the guidance forms`
- **Moves:** none.

### Step 4: The template's guidance

- **Files:** the ones Step 3's verdicts name of `templates/basic/AGENTS.md`, `templates/basic/README.md`, `templates/basic/.claude/skills/<name>/SKILL.md`, `templates/basic/.agents/skills/<name>/SKILL.md`, `docs/getting-started.md`, `docs/levels.md`, `docs/assets.md`, `scripts/check-agent-context.py`, `scripts/check-repo.py`, `scripts/test-check-repo.py` and `docs/validation.md`; always `docs/agent-setup.md` and this plan.
- **Change:** the verdicts' files by §2.5, each form in sections of its own, and `docs/agent-setup.md`'s section; each landed form's marks in `perf-runs/716-guidance/demo/marks.json` (untracked, §2.7); the evidence row names each file touched.
- **Done-when:** `just ctx` → exit 0, its self-test failing a case for each new check; `python3 -B scripts/test-check-repo.py` → exit 0 when `check-repo.py` changed, else not run; `cargo test -p tungsten-cli --locked` → exit 0 (the embedded template is the tracked one, and `new` writes each file); `just repo-check` → exit 0; `git diff --check` → exit 0; `just template-check` → exit 0 (GPU and display: `tungsten new`'s game, guidance and all, checks, builds, tests, smoke-runs and packages outside the repository); `python3 -B perf-runs/716-guidance/mine.py cited --file docs/agent-setup.md` → exit 0.
- **Commit:** `docs(template): ship the agent guidance the verdicts name`
- **Moves:** the template's guidance files and the budgets the verdicts set, their new sizes in the evidence row.

### Step 5: The demo in a fresh copy

- **Files:** this plan (the evidence row); for a revision or a withdrawal, Step 4's files, each in its own commit. Untracked: `perf-runs/716-guidance/demo/` (the frame, the prompts, results, transcripts, `reads --demo`'s rows, `version.sh`), `target/716-guidance/`, `/home/joker/tungsten-eval/716/`.
- **Change:** §2.7, a try at a time: the try's engine repository, CLI and copies, built from its commit; the version check; the listing checks; the task sessions and their reads; then a revision and its try, or a withdrawal and its last try.
- **Done-when:** the gates (§2.10) read and quoted; for each try, its commit named and `bash perf-runs/716-guidance/demo/version.sh <try>` → exit 0 when the shipped guidance points at the engine's files, else not run with that reason, and both listing checks' outputs quoted, naming each shipped file, each shipped skill's session showing its call; `python3 -B perf-runs/716-guidance/mine.py reads --demo` → exit 0, a row per session with its try, its delivered guidance reads, Skill calls, calls before its first edit and `reached` per form by its marks, every form still in the template reached in the try that tested its sections; a revision under Step 4's done-when; a withdrawal's `revert(template)` commit under `just ctx`, `cargo test -p tungsten-cli --locked`, `just repo-check` and `python3 -B perf-runs/716-guidance/mine.py sections --check <tried commit> <withdrawal commit>` → exit 0; each session ended inside its cap, marked functional only; `python3 -B perf-runs/716-guidance/mine.py hash --check` → exit 0.
- **Commit:** `docs(plans): record 7.16's demo`
- **Moves:** none past a revision's or a withdrawal's files.

### Step 6: The decision

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, this plan (the evidence row).
- **Change:** the entry of §2.4 and its index row, with the marker lines of the entries it amends, by the tungsten-decision skill.
- **Done-when:** `rg -c '^## <ID> — ' DECISIONS.md` → `1`, `<ID>` the one the recheck found; `python3 -B perf-runs/716-guidance/mine.py cited --entry <ID>` → exit 0, every URL the entry cites a `confirmed` claim; `python3 -B perf-runs/716-guidance/mine.py shipped --entry <ID>` → exit 0, the guidance files and sections the entry says ship equal the template's tracked guidance files (its instruction files and skill folders) and their sections, and those sections are byte for byte the ones a try reached; `just repo-check` → exit 0, the decision index's tests among its checks; `python3 -B perf-runs/716-guidance/mine.py hash --check` → exit 0.
- **Commit:** `docs(decisions): record what the template ships for agents`
- **Moves:** none.

### Step 7: Close

- **Files:** this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md`, `docs/known-issues.md`, the files that cite this plan.
- **Change:** EV-40's and EV-63's status cells (§2.9); phase 7's Step 16 gets its Plans and Closed lines, its "Rows: none" made EV-40 and EV-63, and its evidence row; the tool form's homes, a withdrawn form and the reading's findings outside this step (Q3) to Later lines or known issues; the plan deleted with its citations; its Now line out, and phase 7's line saying 7.16 landed and Step 17 is next.
- **Done-when:** `just check` → exit 0; `just ctx` and `just repo-check` → exit 0 after the deletion; `rg -n '^\| EV-(40|63) \| [^|]+ \| 7\.16 \| open \|' docs/plans/phase-7.md; test $? -eq 1` → exit 0; `python3 -B perf-runs/716-guidance/mine.py hash --check` → exit 0; Step 5's demo row quoted.
- **Ends with:** the commits, the evidence, what the owner should try (a `tungsten new` copy outside the repository, asked to list its instructions and skills, then given a small task), and the next prompt.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | An A/B trial before 8.5: the copy's briefs with and without the shipped guidance on `D-291`'s harness? | No. 8.5's pass measures the shipped guidance; a trial here would cost about two copy passes (pass 2's eight copy runs took 68.6 agent-minutes and USD 26.12 at API prices, the rerun plan's evidence row 2 at `c184bb31`) and measure a template 8.5 changes again. Step 5 shows the guidance reached, nothing more |
| Q2 | 8.5's frame names the guide's local path (`D-293`(4)), which a game's own guidance does not: should 8.5's plan change it? | The decision recommends that 8.5's plan weigh a frame naming no doc path, its cost the counts' comparability with 7.13's; no harness edit here |
| Q3 | Changes for 01's read path (the root `AGENTS.md`, the LLM index, 01's README and tools guide) | None here: the reading's findings on them go to Later or known issues, each with its rows |
| Q4 | Clients past Claude Code and Codex (Copilot, Cursor, Gemini) | Served where a file both supported clients already read serves them (`AGENTS.md`, a shared skill folder the survey confirms); no third copy |
| Q5 | The verdicts keep today's guidance | Step 4 lands `docs/agent-setup.md`'s section and the fixes the reading names that fit `AGENTS.md`'s budget, or nothing more; Step 5 runs the listing checks alone; Step 6 records that nothing new ships, and why |

## 5. Invariants

- **May move:** the template's guidance files, `just ctx`'s template budgets and checks and `check_kit_catalogue`'s rule, as the verdicts set them (Step 4, and Step 5's revision or withdrawal, the new values in their evidence rows); `docs/agent-setup.md` and the guide's sections the verdicts name (the same steps); the decision index (Step 6); phase 7's rows and Step 16 (Step 7).
- **Must not move:** code in `crates/`, `tools/` and `examples/`; the template's `src/`, `assets/`, `tests/`, `tools/`, `Cargo.toml` and line budgets (`D-199`); every counter guard pin, digest and `api/` snapshot; 01's shell copy check; each `CLAUDE.md`'s import-only form; `perf-runs/713g-eval/` and the transcript archive (the hash).

## 6. Stop conditions

- A done-when fails: restore the step's files from the last commit, mark the step skipped with its reason, and go on with the steps that don't depend on it. Step 3 depends on Steps 1 and 2, Step 4 on Step 3, Step 5 on Step 4, Step 6 on Step 5 (on Step 3 alone under Q5); a skipped step's dependents are skipped with it, and the close reports them and leaves the item in Now.
- A file outside a step's list stops the step; a landing that needs one lands what fits in Step 4's list, and the rest becomes a second 7.16 plan, named in Follow-ups.
- `mine.py hash --check` fails: stop the run and report the changed files, since 8.5 and 8.6 need those records as they are.
- A gate (§2.10) fails before a subagent or a session: wait for the week's reset or the disk, or record that part not run with the reading.
- A Step 5 session reaches its cap: stopped there, recorded, and its delivered reads still count toward `reached`.
- The survey leaves a question without a confirmed claim: the decision says so and weighs no claim for it.

## 7. Critique

Round 1: gpt-6-astra through Codex, 2026-11-25, at `d188da18` with this plan uncommitted; `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 348.6 s, tokens in 719154 (cached 647680), out 11116 (reasoning 6825), commands 21, output 343 KB (largest 83.2 KB), usage 5h 0% → 19%, week 28% → 31%`. Run on `gpt-6-astra` at once: the critic's default, `gpt-6.2-astra`, was refused on this account with no model call on 2026-11-18 and 11-19, and this session did not retry it.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | The reading maps requested line ranges to the guide's headings, but a result can be cut: pass 2's `copy.status` asked for the whole guide and got a 2 KB preview | The transcript's line 47 (`<persisted-output>`, 63.5 KB saved, 2 KB shown); a scan of the 26 runs' records: 13 previews, no later read of a saved remainder | Holds | §2.2 credits delivered text alone, previews and follow-ups counted apart; the `read` class needs a delivered line; A17; §2.5's sizes rule, so a pointer names how to read a large file in parts |
| 2 | Step 5's checks allow every session to miss, §2.7 sends a second miss to known issues and the close only quotes the row, against the done-when's working demo | This plan's Step 5, §2.7, Step 6 and header | Holds | Each landed form must be reached by one session; a miss gets one revision, then a withdrawal (`revert(template)`), and the decision entry moves after the demo (Step 6, the close Step 7), so it records only what was reached |
| 3 | The demo rewrites the tagged dependency to a path, as `scripts/template-check.sh:78` does, and tests one version, so a pointer that reads the path from `Cargo.toml` passes and fails in a real game | `scripts/template-check.sh:78`; this plan's §2.7 | Holds | The demo's copies depend on a scratch Git repository by `rev`, so Cargo checks the engine out as for a tag; `demo/version.sh` moves the pin from A to B and the pointer must follow; §2.5's versions rule names Cargo's checkout |

Round 2: the same critic at xhigh, 2026-11-25, on this plan without §7 and the evidence log (`--from "sed '/^## 7\./,/^## Fol/{/^## Fol/!d}' …"`), with a focus note naming round 1's three changes and no rejections; `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 255.6 s, tokens in 495024 (cached 431744), out 8072 (reasoning 5242), commands 16, output 212.6 KB (largest 29.4 KB), usage 5h 19% → 34%, week 31% → 33%`. Each of its three findings is a gap in a round-1 fix; no third round follows.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | A revision's rerun would use the first try's export and CLI, whose embedded template is Step 4's, so it reruns the old guidance | `tools/cli/build.rs:85` (each template file's bytes embedded at build), `tools/cli/src/new.rs:166` (`new` writes them); this plan's §2.7 | Holds | §2.7's tries: each builds its own export, engine repository, CLI and copies from the commit it tests, under `try-<n>/`, and records that commit; Step 5's done-when names each try's commit |
| 2.2 | A withdrawal restored whole files, so withdrawing one form erased another's lines in a shared file (`AGENTS.md`) and `docs/agent-setup.md`'s section, which the file-name check in Step 6 cannot see | This plan's §2.4 (forms combine), §2.7 and Step 6 | Holds | §2.5's sections rule: each form's text in a shared file under headings of its own; a withdrawal removes only its form's sections and files, `mine.py sections --check` holds the rest byte for byte as tried, and a last try runs the version and listing checks; Step 6's `shipped` check compares sections too |
| 2.3 | Reach counted a delivered read of the file a form points to, which a preview that stops short of the shipped section satisfies | `perf-runs/713g-eval/runs/2/copy.status/a1/transcripts/997a6f57-4fdf-4bdf-ad61-729347b9ebc7.jsonl:47`; this plan's §2.7 | Holds | §2.7's marks: Step 4 writes each form's marks (a skill's name, a guide section's heading, an index target's heading or line) to `demo/marks.json`, and a form is reached only when a delivered result holds one of them before the first edit |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-11-25 | done | `mine.py hash` → exit 0 first (663 files: 621 under `perf-runs/713g-eval/`, 42 in the archive); `reads` → exit 0, 26 runs (13 a pass), 2,163 tool calls, 0 Skill, web or subagent calls, 11 previews (the plan's 13 counted two twice, from a request blob replaying e01.sound's pair), 0 followed; first edit: copy after 4–50 calls (median 27), minute 0.5–7.0 (median 3.5), 01 after 21–85 (median 46), minute 2.5–12.3; no run called `cargo doc`; every engine line a copy run read (5–1,475) came by the frame's local path. Classes by four fresh judges, then a fresh verifier blind to them on 33 picked items: 30 confirmed, 3 changed (EV-31 and EV-59 `task` → `tool`, 2/22 `task` → `none`), taken. `check` → exit 0: 94 items (66 rows, 28 entries), `none` 58, `item` 14, `rule` 7, `tool` 6, `task` 5, `read` 4; guidance items 36 (copy runs 20, 01's 16); reach in copy runs: `index` 12, `today` 9, `guide` 8, `tool` 4, `skills` 2, `copy` 2. EV-40 `none`, EV-63 `task` (01's path). The survey's Q-g (tool-result limits) added from the previews. `hash --check` → exit 0. Functional only (`D-226`) | `perf-runs/716-guidance/` (`mine.py`, `eval-hash.json`, `reads.{json,md}`, `seams.md` from `seams-{A..D}.md`, `verify-raw.md`, `verify-reading.md`, `reading.md`) |
| 2 | 2026-11-25 | done | Questions Q-a to Q-f as fixed, Q-g (tool-result limits) added by the reading for its 11 previews; a fresh research subagent, web and the installed clients' help (Claude Code 2.1.332, Codex CLI 0.160.0), no query naming this machine. `mine.py survey` → exit 0: 70 claims (Q-a 16, Q-b 16, Q-c 6, Q-d 8, Q-e 8, Q-f 6, Q-g 10), each with a URL, both dates and a quote of 25 words or fewer; a fresh verifier fetched each: 67 `confirmed`, 3 `not found` (V-27, V-35, V-37, each a detail past its quote), 0 `contradicted`; every question has a confirmed claim (Q-c 4 of 6, Q-b 15 of 16). Among them: a docs skill never invoked in 56 % of Vercel's cases and an 8 KB docs index in `AGENTS.md` at 100 % (V-30, V-50, V-70); task-skill invocation 46.4–99.2 % across 18 harnesses, curated skills 33.9 → 50.5 % (V-31, V-52); context files no general gain at over 20 % more cost (V-47); Next.js's version-matched docs in the installed package with a managed `AGENTS.md` block pointing there (V-33, V-55, V-56); a Bash result past about 30,000 characters cut to a 2,000-character preview (V-61). `hash --check` → exit 0 | `perf-runs/716-guidance/survey.md`, `verify-survey.md` |

## Follow-ups
