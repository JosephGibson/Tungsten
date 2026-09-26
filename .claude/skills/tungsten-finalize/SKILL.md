---
name: tungsten-finalize
description: Pre-finalize docs pass for a Tungsten milestone branch before merge or tag — CHANGELOG [Unreleased] and the version cut, README/DESIGN status, LLM_INDEX and DECISION_INDEX rows, plan status and archiving. Ends with just ctx and just repo-check.
---

# tungsten-finalize

Run once per milestone branch, after the last code change and before merging or tagging. Follow the Docs session rules in [AGENTS.md](../../../AGENTS.md):

- Read a whole doc before editing it. The exceptions are `CHANGELOG.md`, `DECISIONS.md` and `DESIGN.md`: run `rg -n '^#' <file>` and read only the section you edit.
- Decisions are immutable. A new decision adds its index row in the same change.
- Never open `docs/plans/archive/`.

Work on the branch only. Commits, merges and tags are the owner's call.

## 1. Inventory the branch

```bash
git log --oneline main..HEAD
git diff --stat main...HEAD
git diff --unified=0 main...HEAD -- DECISIONS.md | rg '^\+## D-'   # new decisions
fd -e md --max-depth 1 . docs/plans                                 # active plans
just release-check                                                  # must pass before editing
```

If `release-check` fails, fix the drift first. The usual causes are a version bumped at branch start or a hand-edited status line. The workspace version changes only at the cut (`D-071`).

## 2. CHANGELOG `[Unreleased]`

Complete the section from the inventory, in the file's existing style:

- Start with a `Summary:` line that names the plan.
- Group changes under `### Added`, `### Changed`, `### Fixed` and `### Removed`, as bullets with a bold lead-in that cite `D-NNN` IDs and file paths.
- End with the `DECISIONS.md adds D-NNN–D-NNN` line.

Don't add a version heading; the cut writes it.

## 3. Decisions and indexes

- Every new `## D-NNN` heading needs its row in the right section of [DECISION_INDEX.md](../../../docs/DECISION_INDEX.md); the `decision_index` test fails without it. A reversal adds a new entry, marks the old one `Superseded by D-NNN` and updates both rows.
- In [LLM_INDEX.md](../../../docs/LLM_INDEX.md), add rows for new task areas and fix paths that moved or were deleted. It must stay under 8 KiB (`just ctx`).

## 4. Plans

- Set `status` on every plan the branch finished (`done`, `abandoned` or `superseded`) and tick its done-when checks. Then `git mv` it to `docs/plans/archive/` under the same basename, without listing or reading that directory. `just repo-check` fails on finished plans left in `docs/plans/`.
- Update milestone status lines in the phase plan, for example "done — shipped in `0.NN`" in `docs/plans/phase4.md`.
- Repoint links to a moved plan at `archive/<name>.md`.

## 5. Cut the version

Branch `0.NN` ships as `0.NN.0`; a later fix release on it bumps the patch number.

```bash
just release-cut 0.NN.0   # [Unreleased] -> [0.NN.0] - today; Cargo.toml, Cargo.lock, status lines
```

The cut refuses in these cases:

- the tree is inconsistent;
- `[Unreleased]` is empty;
- the version isn't above the current one;
- the date is earlier than the last release (`--date YYYY-MM-DD` overrides today).

Check the result with `git diff --unified=1`.

## 6. Status prose

- The cut already rewrote the `Workspace vX.Y.Z` text. Update the rest of the status in `README.md` ("Status") and in the status paragraph at the top of `DESIGN.md`: branch name, shipped milestones and the next active plan. Update the README documents table for new or moved docs.
- Find other version or branch mentions with `rg -n '0\.NN' -g '*.md' --glob '!CHANGELOG.md' --glob '!DECISIONS.md'`. Leave historical statements as written.
- Edit `AGENTS.md` only when commands or rules changed, since it is at its 6 KiB budget.

## 7. Verify

```bash
just check        # when code changed on the branch
just ctx
just repo-check   # includes the version/changelog check
```

Report the version, the plans you moved and anything left open. Then hand over the release commands from the "Releases" section of [README.md](../../../README.md): `git tag -a v0.NN.0 -m "Tungsten 0.NN.0"` and `git push origin v0.NN.0`.
