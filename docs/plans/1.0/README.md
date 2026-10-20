# Road to 1.0

Planning for Tungsten 1.0: what it means, which workstreams reach it, in what order, how the work runs as Claude Code sessions, and how the release is checked. Every file here started as a draft on 2026-10-03; the definition gate moved the implementation plan, the workflow and the release checklist to in progress. General plan rules: [plans README](../README.md).

## Now

<!-- roadmap-summary:start -->
- **Workspace:** `0.90.0`
- **First open stop:** `w12qa`
- **Stop state:** `todo`
- **Next milestone:** `M71`
<!-- roadmap-summary:end -->

This is the local tree's snapshot, not proof of publication. `python3 -B scripts/roadmap.py summary` prints the four owned fields above, and `--write` replaces them; `just repo-check` checks them against workspace metadata and the existing status derivation. `python3 -B scripts/roadmap.py status` reports the branch, tags, stop evidence and last fetch without fetching. Refresh/review local refs before claiming a release is merged or published.

Phase 5 is complete in the local refs. The [queue](implementation-plan.md#6-critical-path-and-the-queue) and [register](implementation-plan.md#10-register) own the remaining order. Use `/tungsten-next` for the session prompt. A stop that is cut, tagged or merged counts as closed here, and `just release-cut` rewrites the block, so it holds through the merge (`D-152`). Dated release contents stay in [CHANGELOG](../../../CHANGELOG.md), each release's squash commit in `git log origin/main`, and gate answers and signatures in the [gate records](implementation-plan.md#11-gate-records).

## Reading order

1. This file.
2. [criteria.md](criteria.md) §1 (definition) and §3 (workstreams and tiers). It runs to about 400 lines, so find a section with `rg -n '^#{2,3} ' docs/plans/1.0/criteria.md` and read only that.
3. [implementation-plan.md](implementation-plan.md) for phases, gates, candidates and what starts next.
4. [workflow.md](workflow.md) before writing or executing a milestone plan, or running a gate.
5. The workstream file for the task, then the criteria section it points to. [w01](w01-ui-text-suite.md) is the full UI draft, about 550 lines: find its sections the same way.

## Files

| File | Holds |
| --- | --- |
| [criteria.md](criteria.md) | What 1.0 means, the gaps, the 16 workstreams with proposed tiers, owner questions, decisions and risks. The scoping source for every workstream that has not graduated |
| [implementation-plan.md](implementation-plan.md) | Phases, gates, candidates in order with cards for Phase 5, the queue, when each owner question is needed, amendments proposed to criteria.md, the definition gate agenda, the register and gate records |
| [workflow.md](workflow.md) | How the program runs as Claude Code sessions: session types, levels, what every milestone owes before its release, sharing the tree and the reference machine, owner touchpoints, Step 0's tooling |
| [release-checklist.md](release-checklist.md) | The checks that tag 1.0; C3 runs them on a release candidate |
| [acceptance-game.md](acceptance-game.md) | The game that tests definition A; its feature list cuts W5, W6 and W13 |
| [backlog-1.x.md](backlog-1.x.md) | What 1.0 defers, and what would bring each item back |
| [roadmap.json](roadmap.json) | The catalog the [roadmap page](https://claude.ai/artifact/EyM2iTgnnActfbMBZKcav9) shows: every stop in register order with its group, kind, level, title, summary, sources and ratings, and the owner questions with their states and answers (`D-109`). `just repo-check` compares it with the register, and gate and graduation sessions update it with the register |

| ID | Workstream | File |
| --- | --- | --- |
| W1 | UI, interface and text suite | [w01-ui-text-suite.md](w01-ui-text-suite.md) |
| W2 | Multi-core rendering pass | [w02-multi-core-rendering.md](w02-multi-core-rendering.md) |
| W3 | Frame loop v2 | [w03-frame-loop.md](w03-frame-loop.md) |
| W4 | API stabilization and freeze | [w04-api-freeze.md](w04-api-freeze.md) |
| W5 | Input | [w05-input.md](w05-input.md) |
| W6 | Gameplay systems | [w06-gameplay-systems.md](w06-gameplay-systems.md) |
| W7 | Platforms | [w07-platforms.md](w07-platforms.md) |
| W8 | Correctness burn-down | [w08-correctness.md](w08-correctness.md) |
| W9 | Distribution and documentation | [w09-distribution.md](w09-distribution.md) |
| W10 | Closing phases C1–C3 | [w10-closing-phases.md](w10-closing-phases.md) |
| W11 | Shipping basics | [w11-shipping-basics.md](w11-shipping-basics.md) |
| W12 | Template project | [w12-template.md](w12-template.md) |
| W13 | `tungsten-kit` | [w13-kit.md](w13-kit.md) |
| W14 | Tooling | [w14-tooling.md](w14-tooling.md) |
| W15 | Authoring API | [w15-authoring-api.md](w15-authoring-api.md) |
| W16 | Prefabs | [w16-prefabs.md](w16-prefabs.md) |

## Conventions

- **Names.** One `wNN-slug.md` per workstream. Milestone plans for 1.0 are written here too, named by the [plan naming rule](../README.md#naming) and numbered from M32 when they are next to start. The [register](implementation-plan.md#10-register) maps each candidate to its plan and release.
- **Headers.** Every file except this one carries the six plan header fields, which the repository check reads (below). Skeletons say `draft (skeleton)`.
- **Catalog.** [roadmap.json](roadmap.json) is data, not a plan: it carries no plan headers, and the repository check reads it against the register instead (`D-109`).
- **Graduation.** W2, W3, W4, W9, W11, W12, W14 and W15 graduated: their criteria sections moved verbatim into their files, which hold their candidates in order, each with a done-when, and criteria.md keeps a placement paragraph. W13 graduated on 2026-10-16 for its remaining candidates: [w13](w13-kit.md) holds the kit catalogue, the scoping source of W13e–W13k, while criteria.md §8.4 stays the record of the landed W13a–W13d. The rest stay in criteria.md, and no graduation session is scheduled (`D-150`): a milestone plan cites its workstream's criteria section as its scoping source. A workstream file that has not graduated holds placement only and never a copy of the scoping text, so each fact has one source.
- **Repository check.** `just repo-check` reads this folder too (Step 0a): it checks every file's headers but this one's and flags a finished file left here, naming `docs/plans/archive/1.0/` as its place.
- **Lifecycle.** A finished, abandoned or superseded file moves to `docs/plans/archive/1.0/` with the same basename, and the rest of the folder follows after 1.0. Agents never read the archive.
- **Former paths.** `docs/plans/1.0-criteria-draft.md` is now [criteria.md](criteria.md), and `docs/plans/ui-text-suite-draft.md` is now [w01-ui-text-suite.md](w01-ui-text-suite.md). The released sections of `CHANGELOG.md` and the 0.40 QA plan still name the old paths; `DESIGN.md` names the new one since 0.40 QA step 11.
