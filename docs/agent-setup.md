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

- Canonical copies: `.claude/skills/tungsten-wgpu/` and `.claude/skills/tungsten-perf/` (tracked). Codex finds them through the symlinks `.agents/skills/<name> -> ../../.claude/skills/<name>`. Relative links inside `SKILL.md` resolve through both paths.
- Frontmatter holds only `name` and a description of 300 characters or fewer; bodies stay under 8 KiB. Claude's `paths` field isn't a Codex activation contract, so don't rely on it.
- **Windows:** symlinks need Developer Mode (or an elevated shell) and `core.symlinks=true` before cloning (`git clone -c core.symlinks=true …`). Without them Git writes plain text files and Codex won't see the skills; Claude is unaffected.

## Search filters

- `.ignore` hides the archive, build and perf outputs from ripgrep-based tools (rg, fd, Claude Grep/Glob, Codex). `.gitignore` covers Git and build noise.
- Claude's Glob ignores `.gitignore` unless `CLAUDE_CODE_GLOB_NO_IGNORE=false`, which `.claude/settings.json` sets. Grep already honors `.gitignore` and `.ignore`.
- `.claudeignore` was removed: 2.1.110's Grep didn't honor it in a controlled test.
- Filters aren't access boundaries. The archive rule is enforced for Claude by a project `Read(./docs/plans/archive/**)` deny, which a scratch-project test confirmed for launches at the project root (subdirectory launches weren't verified). For everything else it's the `AGENTS.md` hard rule. Never bypass filters with `rg -uu`.

## Permissions, sandbox and trust

- `.claude/settings.json` allows exact shared `just` commands, workspace build/test/check commands, the four example launch commands, and bounded discovery probes (`rg --files crates examples scripts`, `fd --type f . crates examples scripts`, `ast-grep --version`). No execution prefixes or wildcard arguments are granted. Other searches, parameterized perf captures and install commands use normal interactive approval or personal rules. Hooks and model choices belong in `.claude/settings.local.json` (ignored) or user settings; managed settings override both. Check effective rules with `/permissions` in an interactive session.
- Don't add blanket `cargo`/`just` allows: recipes, build scripts and tests execute repo code.
- Codex suggestion for interactive work: `codex --sandbox workspace-write --ask-for-approval on-request`. First builds download crates and may need approval, and so may writes under `.agents/` or `.codex/`. Project `.codex/` config and non-managed hooks only apply once the project is trusted. Exec-policy rules govern commands outside the sandbox and aren't a push ban.
- Deferred: hooks, custom subagents, blanket execution allows and LSP plugins. Explicit commands (`just` recipes and the raw `cargo` commands they wrap) are the shared workflow.

## Verifying a client

Run from the repo root, then from `crates/tungsten-render/`:

```bash
claude -p 'Without tools, list the instruction files loaded into your context.'
codex exec --sandbox read-only 'Without tools, list the AGENTS.md files you were given.'
```

Model self-reports can be wrong about duplication. For a firm answer, compare `claude -p --output-format json` usage across directories that contain canary files. Earlier setup testing recorded Claude Code 2.1.110 behavior above. During the 2026-09-25 repository review, Claude was absent from the shell, so effective permissions and its discovery commands could not be rerun. Codex CLI 0.155.0-alpha.16.3 passed ephemeral read-only discovery from both directories: root rules at the root, root plus render rules from the renderer, and both skill symlinks resolved. These are model self-reports, not proof against duplicate context injection. Recheck after client upgrades; no blanket claim about older clients' model access is made.

## Local check tiers

Run from the repository root with Rust, just, Python 3.9+, Bash and ShellCheck installed.

| Command | Repeated work it replaces |
| --- | --- |
| `just repo-check` | Asset-directory/manifest coverage comparisons, required active-plan headers/status, local links and decision references in the eight maintained docs, agent configuration checks; then existing Rust manifest and decision-index tests |
| `just quick` | The edit-loop sequence: format check, context budgets/links, repository QA and `cargo check --workspace --all-targets --locked` |
| `just script-test` | Perf/smoke script regressions, ShellCheck, and synthetic repository-checker tests |

`quick` does not replace final `just check` or GPU smoke. `scripts/check-repo.py` is read-only and uses the Python standard library; it never traverses the plan archive, follows directory symlinks, guesses asset IDs or edits manifests. External URLs and link fragments require manual checking. It reports in-progress plans for review; age alone cannot identify abandonment.

Asset coverage exceptions are explicit: complete font families and their inventory README; four vendored LYGIA helper fragments and their license; and example 03's explicitly loaded `scene.json` (D-046). The tracked legacy `examples/01_platformer/assets/sprites/player.png` is reported as a deletion candidate, not silently removed. A new exception needs review in the checker, not a broad ignored extension.

The exact-command permission syntax follows [Claude's permission rules](https://code.claude.com/docs/en/permissions); Codex instruction discovery follows [AGENTS.md guidance](https://developers.openai.com/codex/guides/agents-md/). These controls do not make repository code trusted: review recipe changes before executing them.
