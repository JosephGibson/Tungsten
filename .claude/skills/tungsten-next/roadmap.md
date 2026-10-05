# Roadmap sync (Claude Code only)

The [roadmap artifact](https://claude.ai/artifact/EyM2iTgnnActfbMBZKcav9) is a viewer. Its source is [page.html](page.html) beside this file and holds no stop, question or prompt; everything it shows comes from its database, and this sync is the database's only writer apart from the owner's ticks.

| Document | Written by | Shape |
| --- | --- | --- |
| `meta/catalog` | This sync, when the hash changes | `{schema, hash, catalog}`, the payload `scripts/roadmap.py catalog` writes from `docs/plans/1.0/roadmap.json` and the Flow table in [prompts.md](prompts.md): schema 2, each stop with its `stages`, and `catalog.sessions` (below) |
| `meta/now` | This sync, every run | `{synced_at, commit, branch, release, stop, headline, next, after, heads_up, recovery, tree}` (below) |
| `stops/<id>` | This sync: `status`, `release`, `milestone`, `plan`, `plan_steps`, `evidence`. The owner on the page: `steps` | A stop with no document is `todo` with nothing ticked |

## Fields

- **status:** `todo`, `plan` (written, not approved), `run`, `ready` (the register row says cut, no tag yet), `review` (tagged, not on `origin/main`), `done` (on `origin/main`, or the gate's records signed), `skip`. Gates and spikes: `todo`, `done`, `skip`. `scripts/roadmap.py status` derives them from the tree, and its evidence column says why; never set one by hand.
- **steps:** the owner's ticks, keyed `<status>:<key>` with the keys in [prompts.md](prompts.md) (`todo:plan`, `plan:approve`), so a status change hides the previous status's ticks. Never write them. Title-keyed ticks in older documents (`s0`) stay as they are.
- **Pairs** (`D-108`): the catalog folds a pair into its first stop, so a pair has one document.
- **stages** (`D-118`): the payload gives each stop its Flow row's steps (level C releases take the level C row; other releases, levels A, B or unset, the A-or-B row; `custom` stops none), each `{key, title, where, at, note}` with `at` a list of statuses. A session step carries `rec` `{mode, model, effort}`, `mode` being `auto` or `plan`; a same-session step's `rec` is `{keeps, mode, model, effort, if_new}`: the values of the step that opened its session, and what to use when the 60% rule makes it a new session. `recommend` in `scripts/roadmap.py` is the rule. `catalog.sessions` holds `release` and `verify`, the session prompts outside every flow. Nothing here is hand-written: a Flow edit or a re-rated stop changes the hash, and the next sync rewrites the catalog.
- **meta/now**, from this run's reply: `synced_at` (ISO time), `commit` (`HEAD`'s short hash) and `branch`; `release` and `stop` (the helper's next release and the stop the reply names); `headline` (the reply's first line, without markup); `next` `{key, title, where, text, rec}` (the Next step's prompts.md key and Step title, `new`, `same` or `you`, the filled prompt, or an owner step's checklist as plain lines, and its recommendation); `after` and `heads_up` (the reply's lines); `recovery` (`resume`, `compact`, `release` and `verify` filled for this stop as `{key, title, text, rec}`, leaving out those that don't apply); `tree` (below). No `flow`: the page takes the stop's stages from the catalog.
- **rec** in `meta/now` is copied from the payload, never worked out: the stop's stage with that key, or that stage's `if_new` when the 60% rule makes a same-session step a new session; for `resume`, the stop's `run` or `run-c1` stage; for `release` and `verify`, `catalog.sessions`. `compact`, `review-fixes`, `answer`, `records-lag` and owner steps have none.
- **tree**, the text after each `state.sh` label at sync time: `upstream` (the `branch:` line's upstream, then the `vs` line's counts), `main` (the `origin/main:` line, then `origin/main in HEAD:`), `fetch` (`last fetch:`), `tag` (the `tag v…` line), `uncommitted` (the release commit section's `tracked:` and `untracked:` lines), `sessions` (`other agent sessions in this tree:`), `capture` (`perf capture running:`), `decisions` (`decisions not in HEAD:`).

## Sync

1. Gather, in SKILL §1's round: the helper's `status.json` and `catalog.json` in the scratchpad, and `ArtifactData` `list` of `meta` and of `stops`.
2. Compare. `meta/catalog`: write it, by `file_path` to `catalog.json`, when its `hash` differs from the listed document's or there is none. `stops/<id>`: an `update` with only the derived fields that differ from the listed document (a missing document reads as `todo` with nothing set; a derived `todo` with nothing set needs no document). `meta/now`: a `set` every run.
3. Write one `batch`, each entry pinned with `if_version` from the list (omit it only when creating a document). A batch holds at most 50 entries and 1 MiB, and a document at most 256 KiB; split a larger write. On a version conflict, list again and redo that entry.
4. Report on the reply's Roadmap line what changed (`s0: ready → done; meta/now: W14a plan; catalog e26916c2`), or `already in step`.

## Republishing the page

A sync never republishes. Only a session that changes `page.html` republishes it: read the live artifact with the Artifact tool's `read` and Read the saved file whole, then publish `page.html` to the same URL, omitting `capabilities` so the stored `db` capability and contract carry forward.

A change to the catalog's shape moves `SCHEMA` in `scripts/roadmap.py`, `roadmap.json`'s `schema` and the page's together. The page keeps reading the previous schema too, is republished first, and the sync that writes the new schema follows, so the live page can always read its `db`.
