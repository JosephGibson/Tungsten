# Operating rules

How work is picked, planned, committed and released from H1 on (`D-215`, the [roadmap redo](plans/roadmap-redo.md)'s §4). Each rule names the failure it prevents; a rule that can't name one is cut at the next review, and anything a script does is a link to the script. The redo's H2 (step 7.4) rewrites the skills and the release flow against these rules.

## Status

- [`ROADMAP.md`](../ROADMAP.md) is the only status record: Now, Next and Later, one line per item. It replaced the register, `roadmap.json`, the roadmap page and the 1.0 README's owned fields, which grew a row, a stop and a field per change and drifted from each other.
- Now holds one to three items; `just repo-check` refuses more, since a long Now is a waterfall again.
- The commit that starts or finishes an item updates `ROADMAP.md`, so status never lags the tree.
- Items keep their IDs (`e01tomes`); milestone numbers ended with the register at M85.

## Habits

The owner's answer of 2026-10-23: toward agile by a gradient, the 1.0 batch planned properly, fully iterative after 1.0.

- A new idea is parked as one line in Later, not planned. Planned on arrival, ideas grew the queue: the notes of 2026-10-25 added five example stops in a day.
- The owner plays before the next thing is planned. Each playtest since 0.97 added or re-cut example stops planned before it.
- An agent names scope creep when it sees it and proposes the smallest version. A silent "no scope creep" rule lets a task grow unseen.
- The redo's order is intent, re-cut after each game finding. A fixed order kept Phase 7 unrevised while example 01 measured the seams.

## Git

- Agents commit at each logical step and push the version branch after each commit (`git push origin 0.NNN`, the whole command, `D-216`); the owner tags and merges, and every other push stays the owner's. An uncommitted tree (`D-105`, superseded) lost the history of every step inside a milestone's one squashed commit, and an unpushed one kept a session's commits on one machine until the owner pushed by hand.
- Stage only the paths you changed (`git add <paths>`), never `git add -A` or `git add .`, so one session's commit cannot take another's files.
- A second session in parallel works in its own worktree or clone, which the owner makes (`git worktree add` stays denied to agents), since two sessions share one index otherwise.
- Messages follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/): `type(scope): summary`, one line, imperative, lowercase, at most 72 characters, no attribution. Types: `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `build`, `ci`, `chore`, `revert`; `!` before the colon marks a break. The [`commit-msg` hook](../.githooks/commit-msg) checks it once `just hooks` has run, so a malformed message never lands.
- Each commit builds and passes the checks its change needs ([validation](validation.md)), so `git bisect` works.
- Until H2 moves publishing to a tag push, a version branch and its release pull request stay the publish step, merged without squashing so the commits survive.

## Records

- History stops growing per change: no register rows, catalog stops, roadmap-page syncs or decision amendments per change. A release's changelog section is written from its commits at the cut.
- Maintained docs are corrected in the same commit as the change that makes them wrong: `DESIGN.md`, the [LLM index](LLM_INDEX.md), `README.md`, [assets](assets.md), [validation](validation.md), [known issues](known-issues.md) and the template's docs. A doc left wrong misleads the next agent's first read.
- Frozen records carry a one-line banner naming what replaced them and are not edited again; step 9.7 deletes them. `check_plans` refuses a finished status outside the archive, so they keep their status under the banner.

## Plans

`D-140`'s routes, with the 1.0 milestone route retired (`D-215`). A milestone plan of about 312 lines per change, with its register and catalog rows, was most of M76–M85's bookkeeping.

- **Small:** no plan; the commit is the record.
- **Medium:** scope, non-goals, files and checks stated in the session before editing.
- **Large** (multi-session, or a risky change): one plan file, `docs/plans/<topic>.md`, with the [plans README](plans/README.md)'s headers, deleted when done. Remove its citations in the same commit, since `just repo-check` fails on a cited plan that is gone.
- Done means the tests pass and a demo works.
- A large plan or a risky change (concurrency, formats, the render seam) gets one critique round (`/critique`) at xhigh, and a second only when the first changed a mechanism: round 2s have mostly repaired round-1 fixes, and max has shown no lift over xhigh.

## Decisions

- A decision entry only for a choice that constrains later work: a dependency (with its `D-015` rule), a format or a policy. Short. Per-milestone entries gave `D-158` seven amendments and `D-194` eleven.
- Entries stay immutable, with a marker line on the old one and the index row in the same change (the `tungsten-decision` skill); a change that records no constraining choice needs no entry.

## Freeze process, removed

No break ledger, no level C or its API critique, no rule that prefab and scene files keep their fields, no `#[non_exhaustive]` sweeps (`D-215`). Breaking is fine while 0.x lasts, and saves and formats break only with a migration. The API snapshot in `api/` stays, as a free diff of the public surface.

## Releases

- Cut when something is worth shipping, not per change. A release per milestone forced a version, a changelog section and a decision entry onto every stop.
- Until H2's script, today's [release guide](releases.md) and `just release-cut`, without the register, catalog and roadmap steps.

## Tiers

- A session that generates art or sound runs on Opus 5.5 at max; a code session runs at xhigh (`D-151` as amended). Large art sessions lost quality at lower tiers, and a per-stop field lived in a catalog that is now frozen.

## Assets

The redo's [§6](plans/roadmap-redo.md#6-asset-sessions): a brief per asset family from the planning session; an Opus asset session that generates the files and a review sheet; the owner's review in rounds; accepted files locked; code sessions build against the brief's contract and leave `tools/` art alone. A code session that also draws loses the art's quality and the code's tier at once.

## Docs

- `missing_docs` only falls: `just docs-ratchet`, which `just release-cut` runs first, counts the public items without rustdoc per crate and fails on a rise over [the baseline](../scripts/missing-docs.json), so undocumented surface cannot grow between releases. Rustdoc lands per system as it settles, the rest in step 9.4.
