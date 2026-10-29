# Agent setup

How Claude Code and Codex pick up this repo's instructions, skills and search filters, and which controls are deliberately left to each user. Rules themselves live in [`AGENTS.md`](../AGENTS.md).

## Instructions

- `AGENTS.md` is the only instruction body. `CLAUDE.md` contains just `@AGENTS.md`, so both tools read the same text. Keep `@` imports out of `AGENTS.md`: Codex doesn't document expanding them.
- Scoped rules: `crates/tungsten-render/AGENTS.md`, with a matching one-line `CLAUDE.md` import. `templates/basic/AGENTS.md` and its `CLAUDE.md` are the rules of a game made from the template, which `tungsten new` copies; `just ctx` holds them to 12,288 bytes and 100 lines (`D-183`), and they load when a session works in the template.
- **Codex** reads one instruction file per directory from the repo root to the launch directory (`AGENTS.override.md`, then `AGENTS.md`); the combined default budget is 32 KiB. A root launch doesn't preload nested files, so the root `AGENTS.md` tells agents to read the render file before editing there.
- **Claude Code:** the [current official guidance](https://code.claude.com/docs/en/memory#agentsmd) (consulted 2026-10-12) documents native `AGENTS.md` support from 2.1.277. With a project `CLAUDE.md` present, the default still uses it and its imports; even the mode that reads both files deduplicates an imported `AGENTS.md`. Keep the one-line compatibility imports for older or unsupported sessions. This newer behavior is documented, not locally re-probed.
- **Local probe**, Claude Code 2.1.311 (2026-10-29, [the checks below](#verifying-a-client)), agreeing with 2.1.110's (2026-09-25):
  - Launched at the repo root, the root body loads once, through `CLAUDE.md`'s import. 2.1.110 loaded the render rules when a file in `crates/tungsten-render/` was read; 2.1.311 wasn't probed for it.
  - Launched inside `crates/tungsten-render/`, the crate's `CLAUDE.md` and `AGENTS.md` load, and the root `CLAUDE.md` without its `@AGENTS.md`, which counts as an external import. It stays out until you approve external imports in the interactive prompt; headless `claude -p` runs there never get the root rules. **Launch Claude from the repo root.**
  - A headless run ignores `.claude/settings.json`'s allow list until the workspace's trust dialog has been accepted once.
  - 2.1.110 didn't read a bare `AGENTS.md`; that result does not describe clients with native support.

## Skills

- Canonical copies (tracked): `.claude/skills/tungsten-wgpu/`, `.claude/skills/tungsten-perf/`, `.claude/skills/tungsten-decision/` (writing, amending and superseding `DECISIONS.md` entries), `.claude/skills/tungsten-finalize/` (branch documentation and version cut), `.claude/skills/tungsten-release/` (release checks, command hand-off, verification and recovery), `.claude/skills/tungsten-milestone/` (writing and running a large plan, with its skeleton) and `.claude/skills/tungsten-next/` (the owner's next session prompt, from `ROADMAP.md` and the tree). The canonical release procedure is [releases.md](releases.md). Codex finds them through the symlinks `.agents/skills/<name> -> ../../.claude/skills/<name>`. Relative links inside `SKILL.md` resolve through both paths.
- Frontmatter holds only `name` and a description of 300 characters or fewer; bodies stay under 8 KiB. Claude's `paths` field isn't a Codex activation contract, so don't rely on it.
- **Windows:** symlinks need Developer Mode (or an elevated shell) and `core.symlinks=true` before cloning (`git clone -c core.symlinks=true …`). Without them Git writes plain text files and Codex won't see the skills; Claude is unaffected.

## Search filters

- `.ignore` hides the archive, build and perf outputs from ripgrep-based tools (rg, fd, Claude Grep/Glob, Codex tools that use those search programs). `.gitignore` covers Git and build noise.
- Claude's Glob ignores `.gitignore` unless `CLAUDE_CODE_GLOB_NO_IGNORE=false`, which `.claude/settings.json` sets. Grep already honors `.gitignore` and `.ignore`.
- `.claudeignore` was removed: 2.1.110's Grep didn't honor it in a controlled test.
- Filters aren't access boundaries. The archive rule is enforced for Claude by a project `Read(./docs/plans/archive/**)` deny, which a scratch-project test confirmed for launches at the project root (subdirectory launches weren't verified). For everything else it's the `AGENTS.md` hard rule. Never bypass filters with `rg -uu`.

## Permissions, sandbox and trust

- `.claude/settings.json` allows exact shared `just` commands (`just physics-release` bare and with `-- --nocapture`, the form physics gates run; the API, notices, template and level checks), workspace build/test/check commands, the launch commands of examples 01 and 02, and bounded discovery probes (`rg --files crates examples scripts`, `fd --type f . crates examples scripts`, `ast-grep --version`). No execution prefixes or wildcard arguments are granted. Other searches, parameterized perf captures and install commands use normal interactive approval or personal rules. Hooks and model choices belong in `.claude/settings.local.json` (ignored) or user settings; managed settings override both. Check effective rules with `/permissions` in an interactive session.
- Don't add blanket `cargo`/`just` allows: recipes, build scripts and tests execute repo code.
- Committing (`D-215`, superseding `D-105`): sessions commit at each logical step in Conventional Commits, staging only the paths they changed, checked by the `commit-msg` hook that `just hooks` enables; a parallel session works in its own worktree. Sessions push the version branch after each commit (`git push origin 0.NNN`, `D-216`); tags, merges, every other push and history rewrites stay with the owner, whose tag push publishes a release ([operating rules](operating-rules.md#git), `D-227`). See [plan execution](plans/README.md#executing-a-plan) and [release handoff](releases.md#handoff-and-next-branch).
- Codex suggestion for interactive work: `codex --sandbox workspace-write --ask-for-approval on-request`. First builds download crates and may need approval, and so may writes under `.agents/` or `.codex/`. Project `.codex/` config and non-managed hooks only apply once the project is trusted. Exec-policy rules govern commands outside the sandbox and aren't a push ban.
- Deferred: hooks, custom subagents, blanket execution allows and LSP plugins. Explicit commands (`just` recipes and the raw `cargo` commands they wrap) are the shared workflow.

## Verifying a client

Run from the repo root, then from `crates/tungsten-render/`:

```bash
claude -p 'Without tools, list the instruction files loaded into your context.'
codex exec --sandbox read-only 'Without tools, list the AGENTS.md files you were given.'
```

Model self-reports can be wrong about duplication. For a firm answer, compare `claude -p --output-format json` usage across directories that contain canary files. Last verified 2026-10-29 with Claude Code 2.1.311 (results under [Instructions](#instructions)) and Codex CLI 0.160.0, which listed the root `AGENTS.md` from the root, and the root and render files from `crates/tungsten-render/`; rerun both checks after a client upgrade.

## Local check tiers

Run from the repository root with Rust, just, Python 3.12+, Bash, ShellCheck and actionlint installed. The [validation matrix](validation.md#change-to-check-matrix) owns change-to-check rules and targeted edit-loop commands. `just checks [PATH…]` prints recommendations only; no paths selects staged/unstaged/untracked Git changes, and `--base REF` includes committed work since an explicit base. See [selector semantics](validation.md#read-only-selector).

`just quick` combines format/context/repository QA and workspace type checking. It is broader than a targeted edit-loop test and does not replace final `just check` or applicable hardware checks. `just ci` mirrors the informational CPU CI sequence; `just --list` and [justfile](../justfile) describe the recipes, including `level-check`, `physics-release`, API and template checks. Pixel tests skipped by a CPU run are **not run**, as the [evidence rules](validation.md#evidence-and-hardware-boundaries) explain.

`just repo-check` checks asset coverage, active plan headers, maintained docs and active program documents one folder deep (including their README maps), local inline/reference destinations and Markdown fragments, decision IDs, plan citations, agent configuration, and `ROADMAP.md`'s sections, Now's one to three items and one-line items. Archive targets are notes and never probed; fenced Markdown examples and the milestone skeleton's future-location links are not active-document navigation. Version/changelog/DESIGN consistency and Rust manifest/index tests run afterward. Other Markdown and external URLs need separate review. In-progress plans are review notes; dates do not establish abandonment. Missing citations fail; historical changelog/decision citations and script-test fixtures remain exempt.

The checker is read-only and standard-library-only; it never traverses the plan archive, follows directory symlinks during enumeration, guesses asset IDs or edits manifests. Asset coverage exceptions are listed in [assets](assets.md#coverage-exceptions); a new one needs a scoped checker review.

API snapshots use the pinned toolchain and `cargo-public-api` 0.52.0 (`cargo install --locked cargo-public-api@0.52.0`, `D-107`). Unused-dependency review uses `cargo shear --locked` (`cargo install --locked cargo-shear`); the test modules included with `#[path]` are listed under its `ignored-paths` in `Cargo.toml`. These tools stay local. Release preflight additionally needs Git, authenticated gh and network; use [the release guide](releases.md#ordinary-release) for its read-only inspection and the owner's block.

The exact-command permission syntax follows [Claude's permission rules](https://code.claude.com/docs/en/permissions); Codex instruction discovery follows [AGENTS.md guidance](https://developers.openai.com/codex/guides/agents-md/). These controls do not make repository code trusted: review recipe changes before executing them.
