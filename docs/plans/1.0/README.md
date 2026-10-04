# Road to 1.0

Planning for Tungsten 1.0: what it means, which workstreams reach it, in what order, how the work runs as Claude Code sessions, and how the release is checked. Every file here started as a draft on 2026-10-03; the definition gate moved the implementation plan, the workflow and the release checklist to in progress. General plan rules: [plans README](../README.md).

## Now

- **Phase:** 5, in progress. 0.42 released on 2026-10-04 (`v0.42.0`, squash-merged as `28abcce`) with Step 0 (0a–0d, `D-106`, `D-107`) and the roadmap catalog and page (`D-109`). W14a, milestone M32 (plan archived at `docs/plans/archive/1.0/phase5-milestone-32-headless-harness.md`): the frame body, the headless harness (`D-110`) and example 01's tests on it, released on 2026-10-04 (`v0.43.0`, squash-merged as `9f10746`). 0.44, outside the register: test-suite overhead (`D-111`, `D-112`; plan archived at `docs/plans/archive/test-suite-overhead.md`), released on 2026-10-04 (`v0.44.0`, squash-merged as `d1a0665`). W2 R0, milestone M33 (plan archived at `docs/plans/archive/1.0/phase5-milestone-33-interned-asset-ids.md`): interned sprite IDs (`D-113`) and extract culling (`D-114`), released on 2026-10-04 (`v0.45.0`, squash-merged as `6a61cc2`). W1 M0a, milestone M34 (plan archived at `docs/plans/archive/1.0/phase5-milestone-34-text-engine-split.md`): the text engine split, font families and packaged-only fonts (`D-115`–`D-117`), cut on 2026-10-04 as `v0.46.0`, awaiting the owner's tag and merge.
- **Next:** the owner ships 0.46 (commit, tag, pull request, merge, then the post-merge block for branch `0.47`); then W11a with W7a, milestone M35: plan not yet written.
- **Last gate passed:** definition, 2026-10-03 ([record](implementation-plan.md#11-gate-records), `D-102`–`D-104`), signed by the owner on 2026-10-03.

A session that changes any of these lines updates them in the same patch ([workflow](workflow.md) §5).

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
| [workflow.md](workflow.md) | How the program runs as Claude Code sessions: session types, autonomy levels, what every milestone owes before its release, sharing the tree and the reference machine, owner touchpoints, the tooling Step 0 proposes |
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
- **Graduation.** Once the owner settles a workstream's tier and scope, its section moves from criteria.md into its file verbatim, with links fixed for the new place. Criteria.md then keeps a short placement paragraph, as §4 already does for W1. The file's steps become its candidates in order, each with a done-when; the execution steps live in each candidate's milestone plan. Until then a workstream file holds placement only and never a copy of the scoping text, so each fact has one source. A milestone plan can be written before its workstream graduates, with criteria.md as its scoping source.
- **Repository check.** `just repo-check` reads this folder too (Step 0a): it checks every file's headers but this one's and flags a finished file left here, naming `docs/plans/archive/1.0/` as its place.
- **Lifecycle.** A finished, abandoned or superseded file moves to `docs/plans/archive/1.0/` with the same basename, and the rest of the folder follows after 1.0. Agents never read the archive.
- **Former paths.** `docs/plans/1.0-criteria-draft.md` is now [criteria.md](criteria.md), and `docs/plans/ui-text-suite-draft.md` is now [w01-ui-text-suite.md](w01-ui-text-suite.md). The released sections of `CHANGELOG.md` and the 0.40 QA plan still name the old paths; `DESIGN.md` names the new one since 0.40 QA step 11.
