# Agent setup

How Claude Code and Codex pick up this repo's instructions, skills and search filters, and which controls are deliberately left to each user. Rules themselves live in [`AGENTS.md`](../AGENTS.md).

## Instructions

- `AGENTS.md` is the only instruction body. `CLAUDE.md` contains just `@AGENTS.md`, so both tools read the same text. Keep `@` imports out of `AGENTS.md`: Codex doesn't document expanding them.
- Scoped rules: `crates/tungsten-render/AGENTS.md`, with a matching one-line `CLAUDE.md` import.
- **Codex** reads one instruction file per directory from the repo root to the launch directory (`AGENTS.override.md`, then `AGENTS.md`); the combined default budget is 32 KiB. A root launch doesn't preload nested files, so the root `AGENTS.md` tells agents to read the render file before editing there.
- **Claude Code** loads `CLAUDE.md` from the launch directory and its ancestors, and loads nested ones when it reads files below. Measured with 2.1.110 (2026-09-25):
  - Launched at the repo root, the root body loads once, and the render rules load when a file in `crates/tungsten-render/` is read.
  - Launched inside `crates/tungsten-render/`, the root `@AGENTS.md` counts as an external import. It stays out until you approve external imports in the interactive prompt; headless `claude -p` runs there never get the root rules. **Launch Claude from the repo root.**
  - 2.1.110 doesn't read a bare `AGENTS.md`. Newer clients that do may also read it alongside the import, so re-run the discovery check after upgrading.

## Skills

- Canonical copies (tracked): `.claude/skills/tungsten-wgpu/`, `.claude/skills/tungsten-perf/`, `.claude/skills/tungsten-decision/` (writing, amending and superseding `DECISIONS.md` entries), `.claude/skills/tungsten-finalize/` (branch documentation and version cut), `.claude/skills/tungsten-release/` (release checks, command hand-off, verification and recovery), `.claude/skills/tungsten-milestone/` (writing and running a 1.0 milestone plan, with its skeleton) and `.claude/skills/tungsten-next/` (the owner's next session prompt on the road to 1.0, synced with the roadmap artifact). The canonical release procedure is [releases.md](releases.md). Codex finds them through the symlinks `.agents/skills/<name> -> ../../.claude/skills/<name>`. Relative links inside `SKILL.md` resolve through both paths.
- Frontmatter holds only `name` and a description of 300 characters or fewer; bodies stay under 8 KiB. Claude's `paths` field isn't a Codex activation contract, so don't rely on it.
- **Windows:** symlinks need Developer Mode (or an elevated shell) and `core.symlinks=true` before cloning (`git clone -c core.symlinks=true …`). Without them Git writes plain text files and Codex won't see the skills; Claude is unaffected.

## Search filters

- `.ignore` hides the archive, build and perf outputs from ripgrep-based tools (rg, fd, Claude Grep/Glob, Codex tools that use those search programs). `.gitignore` covers Git and build noise.
- Claude's Glob ignores `.gitignore` unless `CLAUDE_CODE_GLOB_NO_IGNORE=false`, which `.claude/settings.json` sets. Grep already honors `.gitignore` and `.ignore`.
- `.claudeignore` was removed: 2.1.110's Grep didn't honor it in a controlled test.
- Filters aren't access boundaries. The archive rule is enforced for Claude by a project `Read(./docs/plans/archive/**)` deny, which a scratch-project test confirmed for launches at the project root (subdirectory launches weren't verified). For everything else it's the `AGENTS.md` hard rule. Never bypass filters with `rg -uu`.

## Permissions, sandbox and trust

- `.claude/settings.json` allows exact shared `just` commands (`just physics-release` bare and with `-- --nocapture`, the form physics gates run), workspace build/test/check commands, the four example launch commands, and bounded discovery probes (`rg --files crates examples scripts`, `fd --type f . crates examples scripts`, `ast-grep --version`). No execution prefixes or wildcard arguments are granted. Other searches, parameterized perf captures and install commands use normal interactive approval or personal rules. Hooks and model choices belong in `.claude/settings.local.json` (ignored) or user settings; managed settings override both. Check effective rules with `/permissions` in an interactive session.
- Don't add blanket `cargo`/`just` allows: recipes, build scripts and tests execute repo code.
- Committing (`D-097`): agents commit plan work locally with `git add <paths>` and `git commit`, which a personal allow must grant; the project allowlist can't, since it takes exact commands only. `git push`, tags, merges and history rewrites stay with the human.
- Codex suggestion for interactive work: `codex --sandbox workspace-write --ask-for-approval on-request`. First builds download crates and may need approval, and so may writes under `.agents/` or `.codex/`. Project `.codex/` config and non-managed hooks only apply once the project is trusted. Exec-policy rules govern commands outside the sandbox and aren't a push ban.
- Deferred: hooks, custom subagents, blanket execution allows and LSP plugins. Explicit commands (`just` recipes and the raw `cargo` commands they wrap) are the shared workflow.

## Verifying a client

Run from the repo root, then from `crates/tungsten-render/`:

```bash
claude -p 'Without tools, list the instruction files loaded into your context.'
codex exec --sandbox read-only 'Without tools, list the AGENTS.md files you were given.'
```

Model self-reports can be wrong about duplication. For a firm answer, compare `claude -p --output-format json` usage across directories that contain canary files. Last verified with Claude Code 2.1.110 and Codex CLI 0.155.0-alpha.16.3; rerun both checks after a client upgrade.

## Local check tiers

Run from the repository root with Rust, just, Python 3.12+, Bash, ShellCheck and actionlint installed.

| Command | Repeated work it replaces |
| --- | --- |
| `just repo-check` | Asset-directory/manifest coverage comparisons, required active-plan headers/status, local links and decision references in the maintained docs, `docs/plans/<name>.md` citations in tracked code, scripts and docs (a missing plan fails, an archived one is a note; `CHANGELOG.md`, `DECISIONS.md` and the script tests' fixtures are exempt), agent configuration checks, workspace version/`CHANGELOG.md`/DESIGN status agreement (`scripts/release.py check`, `D-071`, `D-074`); then existing Rust manifest and decision-index tests |
| `just release-preflight VERSION --repo OWNER/REPO` | Read-only file, live branch/tag, pull-request, GitHub release and exact-commit run inspection that prints the remaining hand-off commands (`D-079`); requires Git, authenticated gh and network; see [releases.md](releases.md) |
| `just quick` | The edit-loop sequence: format check, context budgets/links, repository QA and `cargo check --workspace --all-targets --locked` |
| `just script-test` | Perf/smoke script regressions, ShellCheck, `actionlint` over `.github/workflows/`, and repository-checker, release-script and temporary-Git-repository preflight tests |
| `just physics-release` | The release-only physics run: `cargo test --release` over `physics_determinism`, `physics_tunneling` and `physics_containment` in `tungsten-core`, built with the perf runner's flags (`TUNGSTEN_PERF_RUSTFLAGS` overrides, see [profiling-workflow.md](perf/profiling-workflow.md)). The determinism and containment tests are ignored in debug, so `just check` reports them as ignored and does not cover them |
| `just ci` | The six recipes CI runs, in its order: `check`, `bench-build`, `deps`, `ctx`, `repo-check`, `script-test` (`D-070`); GPU smoke, `just visual` and perf captures stay separate |
| `just api` / `just api-check` | Public-API snapshots in `api/` from the pinned toolchain's rustdoc JSON (`D-107`; install `cargo install --locked cargo-public-api@0.52.0`); `api-check` fails on a stale file and runs in the release checks; not in CI |
| `just udeps` | Unused-dependency search with `cargo shear` (install `cargo install --locked cargo-shear`); not in CI. Its unlinked-file warnings for `src/tests/` modules included through `#[path]` are false positives |

`quick` does not replace final `just check` or GPU smoke. `scripts/check-repo.py` is read-only and uses the Python standard library; it never traverses the plan archive, follows directory symlinks, guesses asset IDs or edits manifests. The checker covers its maintained `DOCS` list; other Markdown files, external URLs and link fragments need separate checking. It reports in-progress plans for review; age alone cannot identify abandonment.

Asset coverage exceptions are listed in [assets](assets.md#coverage-exceptions); a new one needs review in the checker, not a broad ignored extension.

The exact-command permission syntax follows [Claude's permission rules](https://code.claude.com/docs/en/permissions); Codex instruction discovery follows [AGENTS.md guidance](https://developers.openai.com/codex/guides/agents-md/). These controls do not make repository code trusted: review recipe changes before executing them.
