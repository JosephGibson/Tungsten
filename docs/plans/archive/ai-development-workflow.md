# AI development workflow improvements — research and approved execution

- **status:** superseded (approved proposals 1–3 complete 2026-10-11; 8 landed in 0.66; 4–7 and 9 carried to `docs/known-issues.md` by the 0.66 project-health pass)
- **goal:** Reduce context reconstruction, avoidable interruptions and verification gaps in Tungsten development while preserving the current architecture and release controls.
- **non-goals:** Implementing proposals 4–9; large refactors, crate moves, renderer or ECS redesign, changing publication permissions, introducing an agent orchestration platform, or weakening hardware/performance checks.
- **files to touch:** Approved subset: `AGENTS.md`, DESIGN Status, `DECISIONS.md`, `docs/{README,DECISION_INDEX,LLM_INDEX,agent-setup,validation}.md`, `docs/perf/profiling-workflow.md`, `docs/plans/README.md`, `docs/plans/1.0/{README,workflow}.md`, `scripts/{roadmap,check-repo,test-roadmap,test-check-repo}.py`, `justfile`, `CHANGELOG.md`, and this record. Research proposals 4–9 remain unapproved.
- **ordered steps:** (1) Repair current guidance and add drift/link fixtures. (2) Shorten context, record the bounded-maintenance policy, verify navigation and measure bytes. (3) Centralize validation routing and add a read-only selector with regression fixtures. (4) Run the approved checks and record evidence. Proposals 4–9 retain their research status.
- **done-when:** Proposals 1–3 satisfy the approved acceptance checks recorded below; proposals 4–9 await a separate owner decision.

Written 2026-10-10 against clean branch `0.60`, commit `fc59112`. This is a workflow research plan, rather than a numbered 1.0 milestone candidate.

## Context digest

Tungsten already has the right foundations: three clear crate responsibilities, shared Claude/Codex instructions, task routing, focused skills, deterministic assets, a headless harness, strict CPU checks, local GPU checks and careful performance provenance. Preserve them. The strongest opportunities are keeping current guidance trustworthy, reducing the amount of history agents read, making required verification easier to select, and testing the experience of using the engine earlier. Compilation has already received measured improvements: the warm `just check` run in this audit passed in 7.5 seconds. That is one local observation, not a cold-build measurement or an engine-performance result.

## Research scope and evidence

The review inventoried 61 non-archive Markdown documents and examined repository instructions, all three crate entry points and source layout, agent configuration and skills, active 1.0 workstreams and their workflow, release and performance procedures, CI, repository checkers, the game template, asset authoring and representative verification code. Large design and decision documents were read by relevant section. The plan archive was excluded under the repository rule. This is a workflow audit, not a full runtime correctness audit.

Read-only checks completed:

| Check | Result |
| --- | --- |
| `just check` | Passed: formatting, strict workspace clippy, tests and doctests; 7.5 seconds on the warm tree |
| `python3 -B scripts/check-agent-context.py` | Passed |
| Context checker self-test | Passed |
| `python3 -B scripts/check-repo.py` | 0 errors; numerous informational notes |
| Repository-checker regression tests | 27 passed |
| `python3 -B scripts/release.py check` | Workspace, changelog and DESIGN agree on 0.60.0 |
| `python3 -B scripts/roadmap.py status` | Current open stop `w8a`, using local refs; last fetch 2026-10-08 |
| Local Markdown destination/fragment probe | No unexpected missing destinations or fragment candidates; the milestone skeleton deliberately uses its eventual destination's relative links |

GPU smoke, visual regression, release physics and performance captures were not run: this research changed no runtime code or assets. Claude Code was unavailable on this host, so current Claude behavior was researched through official documentation, not claimed as locally verified. Installed Codex was `0.162.0-alpha.2`, newer than the setup guide's recorded verification.

## Triage: highest value to lowest

Value combines expected frequency, avoided rework and implementation cost. Small means a focused patch; medium means a few related patches and verification. These are relative estimates, not measured savings.

| Rank | Proposal | Value | Effort | Confidence |
| --- | --- | --- | --- | --- |
| 1 | Repair conflicting current guidance and make status drift visible | Very high | Small–medium | High: reproduced contradictions |
| 2 | Reduce task context and scale planning to the size of the change | High | Small–medium | High on context size; savings need a pilot |
| 3 | Make validation selection complete and easy to follow | High | Medium | High on missing routing; speed gains conditional |
| 4 | Enforce a few expensive-to-violate rules mechanically | High | Small | High: current checks inspected |
| 5 | Exercise standalone and AI-assisted engine use earlier | High | Medium | High on existing coverage limits |
| 6 | Refresh Claude/Codex discovery checks and exact command permissions | Medium | Small–medium | Client verification required |
| 7 | Standardize behavior and visual evidence for feature reviews | Medium | Small–medium | High on existing facilities; payoff task-dependent |
| 8 | Reduce routine check output and human handoff repetition | Medium | Small | High on output and documented touchpoints |
| 9 | Add lightweight text-formatting and line-ending defaults | Low–medium | Small | High on configuration gaps |

### 1. Repair conflicting current guidance and make status drift visible

**Evidence.** [Agent setup](../../agent-setup.md), Permissions, still says agents commit plan work locally under `D-097`. [Plan execution rules](../README.md#executing-a-plan) and `D-105` require leaving it uncommitted until the release commit. The [1.0 README](../1.0/README.md#now) says 0.56 is awaiting its release PR, although current source and changelog are 0.60 and the roadmap helper recognizes W15c as done. The [profiling guide](../../perf/profiling-workflow.md#hotspots) describes sprite extraction with no culling and string comparisons despite `D-113` and `D-114`. Its text-path names also precede the text split. These contradictions coexist with passing repository QA.

**Proposal.** Correct the maintained operational instructions. Put current state before dated findings in planning documents, linking to the applicable decisions when the initial proposal has since landed. Preserve immutable decisions and dated evidence. Use the existing roadmap helper for the brief current-state summary instead of maintaining another release narrative by hand. Extend repository QA with an explicit check for that summary's owned fields and with local-link checks over active program documents. Avoid trying to infer completion from age or free-form prose.

**Potential files:** `docs/agent-setup.md`, `docs/plans/1.0/README.md`, relevant active workstream introductions, `docs/perf/profiling-workflow.md`, `scripts/roadmap.py`, `scripts/check-repo.py`, their existing tests.

**Acceptance:** All three cited contradictions are resolved; a fixture with a stale owned status field fails QA; active-document links are checked without opening archive destinations. The local-ref freshness caveat remains explicit. Checkers still distinguish proposals, implemented behavior and published state.

### 2. Reduce task context and scale planning to the size of the change

**Evidence.** Measured file sizes: root `AGENTS.md` 5,332 B; renderer instructions 4,066 B, leaving only 30 B of their budget; navigation index 10,990 B; decision index 43,894 B; implementation plan 81,791 B. DESIGN's Status section alone is 13,679 B, mostly release history. The 1.0 README's Phase bullet is 4,747 characters. Line-count limits therefore conceal substantial context costs. [Milestone planning](../../../.claude/skills/tungsten-milestone/SKILL.md) already scopes its audit, while the root audit instruction can be read as requiring a whole crate for any audit.

**Proposal.** Keep startup rules focused on unusual constraints and required boundaries. Make the decision index a short current takeaway and pointer per decision; leave measurements in their canonical sections. Make DESIGN Status and the README Now section a brief current snapshot with links to history. Read only the matching decision-index rows during routine work. Clarify that a crate correctness audit reads its full relevant surface, while a workflow or documentation audit follows its own scope.

Define a small-task route: a bounded bug fix or documentation correction uses a short scope, affected files and acceptance check in the session; a multi-session change gets a saved handoff plan; 1.0 milestone/API work retains its existing design review and critique. This preserves planning while avoiding a full milestone ceremony for ordinary maintenance. Any change to established milestone policy needs an owner-approved decision.

**Potential files:** `AGENTS.md`, `docs/DECISION_INDEX.md`, DESIGN's Status section, `docs/plans/1.0/README.md`, `docs/plans/README.md`, and narrowly relevant workflow/skill wording.

**Acceptance:** Navigation still reaches every decision and task; `just ctx` passes; no historical evidence is lost. Pilot the revised instructions on a small bug, a docs correction and a renderer task, recording files read, repeated exploration and owner corrections. Do not claim a token percentage from byte counts.

This direction follows Claude's guidance to keep persistent instructions concise and OpenAI's guidance on task-specific reading and focused skills. [Claude best practices](https://code.claude.com/docs/en/best-practices), [official OpenAI guidance](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra).

### 3. Make validation selection complete and easy to follow

**Evidence.** [The root test matrix](../../../AGENTS.md#tests), [setup tiers](../../agent-setup.md#local-check-tiers), the navigation index, release guide and authoring guide each contribute part of the validation contract. `just quick` runs context checks, repository QA including Rust integration tests, then an all-workspace type check. It is broader than a targeted edit-loop check. `just level-check` exists but is absent from the setup tier table and root matrix. Release-only physics checks are routed through the index. Visual tests return early without their environment switch and consequently appear passed in ordinary test output.

**Proposal.** Keep one detailed change-to-check matrix and link to it from the other operational documents. Add a small read-only selection mode to existing QA tooling that prints the checks required by supplied paths or the milestone's changed paths, including untracked files. It should recommend commands, rather than silently launch GPU/perf work. Include platformer generator sources and generated outputs, physics internals, shader mirrors, scripts/workflows, public API, template wiring and docs.

Document targeted crate/test commands for the edit loop; retain final `just check` and the appropriate release checks. Report required checks as passed, failed or not run, with hardware checks visibly separate. Do not reuse results across changed source or a release cut without an explicit validity rule.

**Potential files:** `docs/agent-setup.md`, `AGENTS.md`'s matrix/link, `docs/LLM_INDEX.md`, `justfile`, `scripts/check-repo.py` and tests. Changes to visual-test skip presentation can be a separate small follow-up if the reporting route is insufficient.

**Acceptance:** Path fixtures select `level-check` for generator work, release physics for solver work and GPU/visual checks for renderer work. Documentation-only paths do not imply a performance capture. Existing milestone/release requirements remain satisfied. A CPU-only result cannot be reported as having run visual comparisons.

The expected gain is fewer omitted checks and fewer repeated broad checks during iteration. With a warm CPU gate at 7.5 seconds, a complex incremental test cache would have poor justification.

### 4. Enforce a few expensive-to-violate rules mechanically

**Evidence.** [Dependency policy](../../../deny.toml)'s bans section currently warns about duplicate versions; it does not list the prohibited engine/ECS crates or async runtimes from `AGENTS.md`. The [decision-index tests](../../../crates/tungsten-core/tests/decision_index.rs) check sets of IDs anywhere in the index, so a mention can satisfy coverage without a proper row; the decision skill itself records this limitation. Core/render dependency direction is an instruction rather than a dedicated repository check.

**Proposal.** Add the existing prohibited dependency names to cargo-deny's bans, verifying them against the approved graph. Add a narrow manifest check for core's dependency direction and the prohibition on a direct `wgpu` dependency. Strengthen decision-index validation to require one actual row per decision, reject duplicate rows/headings and retain amendment references. Limit automation to these objective contracts; leave contextual design and dependency-admission judgments with the agent and owner.

**Potential files:** `deny.toml`, `scripts/check-repo.py` and tests, `crates/tungsten-core/tests/decision_index.rs`.

**Acceptance:** The current graph passes. Disposable fixtures containing a prohibited dependency, a core-to-render dependency or an ID mentioned only in prose fail for the intended reason. No new runtime dependency or broad custom Rust static analyzer is needed.

### 5. Exercise standalone and AI-assisted engine use earlier

**Evidence.** [The outside-copy check](../../../scripts/template-check.sh) shares the engine checkout, build profiles and lockfile. That is useful for checking self-containment but does not exercise the guide's first dependency resolution from a git tag. [Known issues](../../known-issues.md#follow-ups) already records this distinction. The fresh-Claude feature exercise, RC-A9, is scheduled at [C3](../1.0/release-checklist.md); the documented acceptance-game gap table is empty. The latter does not prove the game repository has no work or gaps.

**Proposal.** Add a periodic clean-consumer exercise after changes to the supported authoring API or guide: follow the published-tag instructions without copying the engine lockfile, then run the game's tests. Keep the inexpensive existing outside-copy check. Also pilot RC-A9 earlier on a tiny feature, such as a timed pickup that affects player movement, using only template instructions, the guide and rustdoc. Record engine-source lookups, missing documentation, workaround code and observed behavior. File gaps in the existing log; resolve them in normal scoped work.

**Potential files:** `scripts/template-check.sh` or an explicitly separate mode, `docs/getting-started.md`, `docs/plans/1.0/acceptance-game.md`, `docs/plans/1.0/workflow.md`, relevant template tests.

**Acceptance:** A released-tag consumer resolves and tests successfully; the task works in a fresh session without engine patches; every necessary source lookup becomes a documented gap. Network, disk and dependency-resolution conditions are recorded. Run periodically or after relevant changes, rather than on every edit.

OpenAI's skill-evaluation guidance supports testing actual task behavior, rather than judging instruction quality by prose alone. [Testing skills with evals](https://developers.openai.com/blog/eval-skills).

### 6. Refresh client discovery checks and exact command permissions

**Evidence.** Setup records Claude 2.1.110 and Codex `0.155.0-alpha.16.3`. Current Claude documentation says `AGENTS.md` support starts at 2.1.277 and an imported file is deduplicated, including when both instruction types are enabled. The existing import remains a supported compatibility route. The project allowlist omits exact `just api`, `api-check`, `notices`, `template-check` and `level-check` commands, although these are normal documented workflows. Whether that causes prompts depends on effective user settings and permission mode; it was not measured here.

**Proposal.** Update the version-specific guidance and run controlled instruction-discovery probes on the actual clients used by the owner, at the root, renderer directory and copied template. Retain the import unless testing establishes a reason to remove it. Provide a read-only prerequisite report using existing scripts/recipes: versions, missing tools, broken skill symlinks, hook availability and relevant disk/display/capture conditions. Audit `/permissions` before approving any additional exact recipes, and add only the vetted commands appropriate to the current policy. Keep hooks local as the existing policy requires.

**Potential files:** `docs/agent-setup.md`, `docs/known-issues.md`, `.claude/settings.json`, existing context-check tooling and `justfile`. Personal settings remain owner-controlled.

**Acceptance:** Discovery evidence identifies the rules actually loaded; all seven skills are discoverable where supported. Normal approved workflows have documented command coverage. Git/publication boundaries and the archive restriction remain intact. Missing tools produce one actionable report rather than failures discovered halfway through a task.

[Claude memory and instruction discovery](https://code.claude.com/docs/en/memory), [Claude permissions](https://code.claude.com/docs/en/permissions), [official Codex skill discovery](https://learn.chatgpt.com/docs/build-skills).

### 7. Standardize behavior and visual evidence for feature reviews

**Evidence.** The harness, template state tests, platformer frame-rate regime tests and GPU comparison facilities are already substantial. However, [showcase captures](../../showcase/README.md) mix historical images, recipe-only entries and previously inspected captures that are not retained. The platformer's authoring guide explicitly says CPU tests and startup smoke do not establish readability or particle appearance in motion. Recent effect work still needs audible/resized-window review.

**Proposal.** For a behavior change, choose the smallest observable scenario that fails before the fix and passes afterward through the real harness. Reuse the existing regime pattern when time/input/event order is involved. For visible features, attach fixed-frame before/after images with commit, backend and settings, plus a short motion capture when a still cannot demonstrate the effect. Label human audio/playthrough checks as pending until reviewed. Use existing capture facilities and preserve the golden-image approval rules.

**Potential files:** The affected feature's existing tests, `docs/showcase/README.md`, the platformer authoring guide and task evidence rows. No blanket test migration is proposed.

**Acceptance:** Each affected task supplies evidence of the behavior the user requested; frame-rate changes exercise zero/one/multiple fixed steps where relevant; intentional visual changes have an explained comparison. A shader-validation or startup-smoke pass is never presented as evidence of visual quality.

Claude's official guidance recommends observable verification criteria and visual evidence for UI changes. [Claude verification practices](https://code.claude.com/docs/en/best-practices).

### 8. Reduce routine check output and human handoff repetition

**Evidence.** A successful repository QA run prints many archive-citation, active-plan and asset-exception notes before its zero-error verdict. The [1.0 workflow](../1.0/workflow.md#9-owner-touchpoints) documents five owner touchpoints for a level B candidate, and `D-108` already allows eligible pairs to share them. The next-session prompts and milestone skeleton are already centralized, so another prompt framework would duplicate working machinery.

**Proposal.** Add an optional concise output mode: verdict and counts on success, complete actionable diagnostics on failure, with detailed notes still available. Reuse the existing final response format for scope, evidence, remaining owner action and the next prompt. Apply existing pairing and same-session approval rules consistently. For small maintenance tasks, use proposal 2's shorter route. Any broader reduction of milestone review/approval stages requires a separate owner-approved policy decision.

**Potential files:** `scripts/check-repo.py`, its tests, context reporting if needed, `docs/agent-setup.md`, and existing prompt/skill wording only where repetition is observed.

**Acceptance:** Concise output preserves failure detail and exit codes. A failed or unrun required check remains visible. Representative tasks need fewer repetitive owner messages while retaining design approval, API review and the human release handoff.

### 9. Add lightweight text-formatting and line-ending defaults

**Evidence.** Rust uses pinned rustfmt, edition 2024 and strict clippy; this baseline passes. Shell and workflow linting already exists. There is no tracked `.editorconfig` or `.gitattributes`. Windows CI explicitly disables Git's CRLF conversion; ordinary clones have no repository-level text-ending policy. Shader mirror equality and shell scripts are sensitive to inconsistent bytes.

**Proposal.** Define UTF-8, LF, final-newline and indentation defaults for maintained text, with explicit handling for binary assets. Keep rustfmt authoritative for Rust and preserve existing generated-file formatting. Adopt a scoped Git line-ending policy without a wholesale normalization commit. Consider additional Python/JSON/Markdown formatting only if a pilot demonstrates real review churn; do not add formatter dependencies and reformat the repository by default.

**Potential files:** `.editorconfig`, `.gitattributes`, a short setup note. No runtime source or binary-asset rewrite is part of this proposal.

**Acceptance:** A representative Windows checkout preserves shell/shader text as intended; generated outputs and shader mirrors remain byte-equal; `cargo fmt --check` and `git diff --check` pass; the patch contains no mass formatting changes.

## Suggested approval and execution order

Approve a first batch limited to proposals 1 and 2: fix guidance, shorten current-state/navigation material, and define the small-task route. These have the clearest recurring benefit and little runtime risk.

After reviewing that result, consider proposals 3 and 4 together: a complete validation route and a few objective guardrails. Proposal 6 can accompany them once the owner's Claude client is available for verification. Then pilot proposals 5 and 7 on one real feature. Proposal 8 is a small output/hand-off improvement; proposal 9 can wait until Windows/editor friction warrants it.

Use the existing decision workflow for changes to established policy. Preserve `D-105`'s uncommitted milestone work, `D-070`'s informational CPU CI, `D-079`'s human publication handoff, and the performance workflow's exclusive capture sittings. Keep each implementation patch within its approved proposal. Revisit no crate architecture or runtime design as part of this work.

## Evaluation and stop conditions

Evaluate the approved changes on five representative tasks: a docs correction, a core bug, a shader change, a generator change and a small game feature. Record time to the first relevant edit, repeated exploration, owner interruptions, required checks omitted or rerun, and review corrections. Use session usage data when available; bytes alone are not token measurements. Existing cross-model critique can assess a qualifying milestone plan, but introducing new agents or plugins is unnecessary for this pilot.

Keep a change only when the workflow becomes simpler or catches a meaningful failure. Stop and rescope if it grows into a new CLI/orchestration system, needs a runtime dependency, weakens check validity, or changes the release/commit boundaries without an approved decision. Do not promise a speedup percentage before this pilot supplies evidence.

## Approval

Approved 2026-10-11: proposals 1, 2 and 3, including bounded documentation, workflow policy and QA tooling. The owner explicitly waived the draft's pending-approval wording for this subset and requested implementation and validation without a release or Git publication. Proposals 4–9 remain unapproved. The preceding findings and suggested order are dated research from 2026-10-10, not a description of the resulting tree.

## Approved subset execution

Revalidated 2026-10-11 on branch `0.60`, `fc59112`, workspace 0.60.0. Initial tree: only this research draft untracked. Roadmap derivation: `w8a` first open, W15c done, local refs last fetched 2026-10-08; no fetch or publication performed.

Scope/API: documentation and existing Python/just tooling only. Add a small owned roadmap summary from `roadmap.derive`, check active local Markdown destinations, and add `check-repo.py --select-checks [PATH…] [--base REF]`. No runtime/API changes or dependencies. Acceptance: stale summaries and broken active links fail with explicit diagnostics; selection covers generator, solver, renderer, docs and mixed paths; all requested CPU checks pass; measured bytes and navigation are recorded. Real-task benefits remain unevaluated until a pilot.

| Proposal | Verdict and evidence |
| --- | --- |
| 1 | Complete: D-105 commit guidance reconciled; four roadmap fields derived by the existing helper; active inline/reference destinations and fragments checked. Stale-field and broken-link fixtures fail with their intended diagnostics; archive access guards and template-example fixtures pass. Profiling symbols/culling/IDs match current source. |
| 2 | Complete: concise decision rows/current snapshots, scoped audit distinction and maintenance/handoff routes recorded in D-140. Every existing index row and cross-reference retained; 140 decision headings resolve through one row each. Historical measurements remain in decisions; unique former-roadmap metadata is retained below. Byte measurements and navigation probes follow. |
| 3 | Complete: docs/validation.md owns the detailed matrix; other routing guides link it. Existing QA gains a read-only selector and just checks. Generator, solver, renderer, docs, mixed/API/template/manifest/workflow and temporary-Git fixtures pass, including untracked files, spaces, renames, deletions, explicit base and unborn HEAD. Hardware recommendations carry not-run labels; the selector executes no checks and cannot certify visual comparisons. |


### Historical roadmap metadata

The former Now section's release metadata is retained here as dated observations from the pre-change tree, not reverified publication evidence. Release contents stay in CHANGELOG, design evidence in the decisions and gate signatures in the register's gate records. The old summary ended with 0.56 cut on 2026-10-07, awaiting its merge; current local-ref derivation now sees W15c complete.

| Observed tag | Date recorded | Squash commit recorded | PR recorded |
| --- | --- | --- | --- |
| `v0.42.0` | 2026-10-04 | `28abcce` | — |
| `v0.43.0` | 2026-10-04 | `9f10746` | — |
| `v0.44.0` | 2026-10-04 | `d1a0665` | — |
| `v0.45.0` | 2026-10-04 | `6a61cc2` | — |
| `v0.46.0` | 2026-10-04 | `20bd160` | — |
| `v0.47.0` | 2026-10-04 | `71b00be` | — |
| `v0.48.0` | 2026-10-05 | `0a1f32f` | — |
| `v0.49.0` | 2026-10-05 | `346ca9d` | — |
| `v0.50.0` | 2026-10-05 | `01f9c55` | #50 |
| `v0.51.0` | 2026-10-06 | `764ef4e` | #51 |
| `v0.52.0` | 2026-10-06 | `36b766d` | #53 |
| `v0.53.0` | 2026-10-06 | `2103040` | #54 |
| `v0.54.0` | 2026-10-06 | `7bc0345` | #55 |
| `v0.55.0` | 2026-10-07 | `cc3ded9` | #56 |
| `v0.56.0` | 2026-10-07 (cut) | Pending in the old summary | Pending in the old summary |

### Validation results

| Check | Result |
| --- | --- |
| `python3 -B scripts/test-check-repo.py` | Passed: 43 regression fixtures/tests (27 before this change) |
| `python3 -B scripts/test-roadmap.py` | Passed: 14 tests |
| `just script-test` | Passed: perf/smoke helpers, ShellCheck, actionlint, QA/roadmap, release, crash and preflight regressions |
| `just ctx` | Passed: instruction/link/skill checks and all 10 self-test corruptions/valid cases |
| `just repo-check` | Passed: 0 QA errors, workspace/changelog/DESIGN at 0.60.0, 2 decision-index and 3 manifest tests |
| `just check` | Passed: formatting, strict workspace clippy, tests and doctests |
| Changed root instructions and DESIGN Status links/anchors | Passed, in addition to maintained/active documents covered by repository QA |
| `git diff --check` | Passed |
| GPU smoke, visual comparisons, audio/playthrough and perf captures | Not run: no runtime, shader, asset or hardware-workflow behavior changed. CPU visual-test skip paths are not pixel comparisons |

Representative selector output (commands are recommendations, not executed results):

| Input | Expected recommendations verified |
| --- | --- |
| `just checks docs/agent-setup.md` | `ctx`, `repo-check`, final `check` and `git diff --check`; no hardware or perf |
| `just checks examples/01_platformer/tools/level.json` | `level-check`, platformer route/gameplay tests, manifests/coverage and smoke; applicable runtime appearance/audio review |
| `just checks crates/tungsten-core/src/physics/step/solver.rs` | Core tests, `physics-release`, smoke; content-dependent API/perf review |
| `just checks crates/tungsten-render/src/renderer.rs assets/shaders/sprite.wgsl` | Render tests including shader coverage, manifests/coverage, smoke/visual; overlapping commands and reasons combined |
| `just checks` | All current staged/unstaged/untracked paths: documentation/QA CPU checks, no hardware |
| `just checks --base origin/main` | Explicit local comparison also includes earlier committed engine/example/API/dependency work; correctly adds its applicable hardware/generator requirements. Those are outside this patch's validation scope |
| Temporary Git repositories | Rename contributes old/new paths; deleted solver still selects release physics; untracked filename with spaces retained; ignored files excluded; explicit base includes committed generator work; unborn HEAD supported; invalid ref fails visibly |

No fetch, engine change, dependency addition, commit, push, tag, release cut or publication occurred. The starting untracked research draft is retained with its dated findings and unapproved proposals; execution evidence is confined to this approved-subset section.

### Context-size measurements

UTF-8 file bytes before/after this patch, measured from the actual starting tree and final content. Section counts include their heading and trailing blank lines. These are not tokens or total session context; no task-time, token-use or speedup percentage was measured. Growth in policy/validation guides is explicit and on demand.

| Document or section | Before bytes | After bytes |
| --- | --- | --- |
| AGENTS.md | 5,332 | 5,148 |
| crates/tungsten-render/AGENTS.md | 4,066 | 4,066 |
| docs/LLM_INDEX.md | 10,990 | 10,948 |
| docs/DECISION_INDEX.md | 43,894 | 17,634 |
| docs/agent-setup.md | 9,302 | 8,897 |
| docs/plans/README.md | 4,040 | 5,388 |
| docs/plans/1.0/README.md | 11,153 | 6,736 |
| docs/plans/1.0/workflow.md | 22,219 | 22,428 |
| docs/perf/profiling-workflow.md | 43,163 | 43,468 |
| DESIGN.md §Status | 13,689 | 952 |
| docs/plans/1.0/README.md §Now | 5,585 | 1,168 |
| docs/validation.md (new, on demand) | 0 | 11,315 |

### Navigation and deferred evaluation

Static navigation probes passed:

- Documentation correction: root rules → LLM documentation/validation rows → doc map, plan routes and validation; no crate correctness audit implied. All named files/local destinations exist.
- Core solver task: Physics row → step/solver, collision/broadphase and release-physics integration tests; D-063/D-081/D-092 headings reachable; selector recommends the debug-skipped release gate.
- Renderer shader task: scoped renderer rules → renderer/material/post routing and stock mirror pair/shader_coverage; D-023/D-057/D-091 headings reachable; selector keeps Naga validation separate from smoke and actual pixel evidence.

These are navigation/selection probes, not real feature pilots. Still deferred: docs correction, small core bug and renderer task pilots recording files actually read, repeated exploration, time to first edit, usage and owner corrections; the research's broader five-task evaluation remains a proposal. No unmeasured savings are claimed. The link checker handles repository inline/reference links, ATX headings and explicit HTML anchors, not every Markdown dialect or external URL. Selection is conservative by path; new/unclassified paths and content-dependent API/perf/manual acceptance require review. Local-ref summary consistency cannot prove publication or freshness. Proposals 4–9 remain unapproved; no unrelated findings were fixed.

### Changed files

- AGENTS.md
- CHANGELOG.md ([Unreleased] only)
- DECISIONS.md (D-140 appended; existing entries immutable)
- DESIGN.md (Status only)
- docs/DECISION_INDEX.md
- docs/LLM_INDEX.md
- docs/README.md
- docs/agent-setup.md
- docs/perf/profiling-workflow.md (Hotspots only)
- docs/plans/README.md
- docs/plans/1.0/README.md
- docs/plans/1.0/workflow.md
- docs/plans/ai-development-workflow.md
- docs/validation.md
- justfile
- scripts/check-repo.py
- scripts/roadmap.py
- scripts/test-check-repo.py
- scripts/test-roadmap.py
