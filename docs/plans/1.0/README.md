# Road to 1.0

Planning for Tungsten 1.0: what it means, which workstreams reach it, in what order, how the work runs as Claude Code sessions, and how the release is checked. Every file here started as a draft on 2026-10-03; the definition gate moved the implementation plan, the workflow and the release checklist to in progress. General plan rules: [plans README](../README.md).

## Now

- **Phase:** 5, in progress. 0.42 released on 2026-10-04 (`v0.42.0`, squash-merged as `28abcce`) with Step 0 (0a–0d, `D-106`, `D-107`) and the roadmap catalog and page (`D-109`). W14a, milestone M32 (plan archived at `docs/plans/archive/1.0/phase5-milestone-32-headless-harness.md`): the frame body, the headless harness (`D-110`) and example 01's tests on it, released on 2026-10-04 (`v0.43.0`, squash-merged as `9f10746`). 0.44, outside the register: test-suite overhead (`D-111`, `D-112`; plan archived at `docs/plans/archive/test-suite-overhead.md`), released on 2026-10-04 (`v0.44.0`, squash-merged as `d1a0665`). W2 R0, milestone M33 (plan archived at `docs/plans/archive/1.0/phase5-milestone-33-interned-asset-ids.md`): interned sprite IDs (`D-113`) and extract culling (`D-114`), released on 2026-10-04 (`v0.45.0`, squash-merged as `6a61cc2`). W1 M0a, milestone M34 (plan archived at `docs/plans/archive/1.0/phase5-milestone-34-text-engine-split.md`): the text engine split, font families and packaged-only fonts (`D-115`–`D-117`), released on 2026-10-04 (`v0.46.0`, squash-merged as `20bd160`). 0.47, outside the register: the roadmap page's stages and session recommendations (`D-118`), released on 2026-10-04 (`v0.47.0`, squash-merged as `71b00be`). W11a with W7a, milestone M35 (plan archived at `docs/plans/archive/1.0/phase5-milestone-35-logs-crash-reports.md`): the user folder, engine logger and crash reports (`D-119`), release symbols with the crash-report probe (`D-120`) and CI's Windows test job (`D-121`), released on 2026-10-05 (`v0.48.0`, squash-merged as `0a1f32f`). W9a with W12a, milestone M36 (plan archived at `docs/plans/archive/1.0/phase5-milestone-36-licence-notices-template.md`): third-party licence notices in every release archive (`D-122`), then standalone games: the engine font, the action map's own path, the template with its outside-copy check and the getting-started guide's first draft (`D-123`), released on 2026-10-05 (`v0.49.0`, squash-merged as `346ca9d`). W1 M1, milestone M37 (plan archived at `docs/plans/archive/1.0/phase5-milestone-37-core-ui-model.md`): the core UI model, Taffy behind Tungsten style types, hit testing, focus, the `TextNodeStore` seam and `UiHarness` (`D-124`, `D-125`), released on 2026-10-05 (`v0.50.0`, squash-merged as `01f9c55`, PR #50). 0.51, outside the register: the glyph gate and the frame-loop gate (`D-126`–`D-132`) and the graduation of W15, W2 and W3, plans and decisions only, released on 2026-10-06 (`v0.51.0`, squash-merged as `764ef4e`, PR #51). W15a, milestone M38 (plan archived at `docs/plans/archive/1.0/phase5-milestone-38-schedule-stages-plugins.md`): the schedule of five closed stages, plugins and `DefaultPlugins`, the template and examples on them, `WindowSize` and `PhysicsPlugin` in core and the `particles` row on three named systems (`D-133`), released on 2026-10-06 (`v0.52.0`, squash-merged as `36b766d`, PR #53). W3a, milestone M39 (plan archived at `docs/plans/archive/1.0/phase5-milestone-39-game-clock-timers.md`): the `Time` resource with the real and game clocks, scale, pause, elapsed time and the frame index, a plain `Timer`, every engine, example and template reader on `Time`, transitions on the real clock and `DeltaTime` deprecated (`D-134`), cut on 2026-10-06 as `v0.53.0` (squash commit and PR number follow the merge).
- **Next:** 0.53 is cut on branch `0.53`; once the owner opens and merges its pull request, the branch `0.54` starts from `origin/main` (the release guide's post-merge block). Track C continues with W15b, tuple queries and bundles ([w15](w15-authoring-api.md) step 2; release 0.54), then W3b; W8a and W1 M0b with W2 R2 are still open from the queue's third item (`/tungsten-next` gives the order and the prompt). The frame-loop gate's decisions stand (`D-128`–`D-132`): W15a's stages carry W3a's `Time`, and they are what W3b steps and what W15b and W15c register through.
- **Last gate passed:** the frame-loop gate, with the glyph gate in the same session, 2026-10-06 ([records](implementation-plan.md#11-gate-records), `D-126`–`D-132`), signed by the owner on 2026-10-06. Before it, the definition gate, 2026-10-03 (`D-102`–`D-104`).

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
