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

- `.claude/settings.json` grants nothing. It sets the Glob option and the archive deny. Personal allows, hooks and model choices belong in `.claude/settings.local.json` (ignored) or user settings, and managed settings override both. Check effective rules with `/permissions` in an interactive session.
- Don't add blanket `cargo`/`just` allows: recipes, build scripts and tests execute repo code.
- Codex suggestion for interactive work: `codex --sandbox workspace-write --ask-for-approval on-request`. First builds download crates and may need approval, and so may writes under `.agents/` or `.codex/`. Project `.codex/` config and non-managed hooks only apply once the project is trusted. Exec-policy rules govern commands outside the sandbox and aren't a push ban.
- Deferred: hooks, custom subagents, blanket execution allows and LSP plugins. Explicit commands (`just` recipes and the raw `cargo` commands they wrap) are the shared workflow.

## Verifying a client

Run from the repo root, then from `crates/tungsten-render/`:

```bash
claude -p 'Without tools, list the instruction files loaded into your context.'
codex exec --sandbox read-only 'Without tools, list the AGENTS.md files you were given.'
```

Model self-reports can be wrong about duplication. For a firm answer, compare `claude -p --output-format json` usage across directories that contain canary files. Status on 2026-09-25: Claude Code 2.1.110 verified as above. Codex CLI 0.157.0 verified: root launch loads the root file once, a render launch adds the render file once, and both skills are found through `.agents/skills/`. Codex 0.120.0 can no longer reach current models.
