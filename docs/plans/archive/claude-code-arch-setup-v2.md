# Claude Code Stack — Arch Linux (v2, Tungsten-aware)

Linear rollout. CLI tools + rtk hook + workflow protocols. No MCP servers added.

**Stack:** `ripgrep`, `fd`, `jq`, `tokei`, `ast-grep`, `difftastic`, `rtk`
**Targets:** Claude Code CLI and the VS Code extension (shared `~/.claude/` config).
**Assumes:** Arch Linux, bash or zsh, `sudo`, Claude Code already installed, git remote access.

This revision fixes the binary-name conflict, the settings-overwrite hazard, and the milestone naming clash from v1. Each step is idempotent; destructive edits back up first.

---

## Step 0 — Preflight (run this first, skip nothing)

Snapshot current state so you can roll back, and capture a before-baseline.

```bash
# Binary inventory
for b in rg fd jq tokei ast-grep difft rtk; do
  printf '%-12s %s\n' "$b" "$(command -v "$b" || echo MISSING)"
done

# Config inventory
ls ~/.claude 2>/dev/null
test -f ~/.claude/CLAUDE.md     && echo "HAS global CLAUDE.md"     || echo "NO global CLAUDE.md"
test -f ~/.claude/settings.json && echo "HAS global settings.json" || echo "NO global settings.json"
```

**Back up anything that exists before Steps 5 and 7 touch it:**

```bash
ts=$(date +%Y%m%dT%H%M%S)
mkdir -p ~/.claude/backups
cp -n ~/.claude/settings.json  ~/.claude/backups/settings.json.$ts 2>/dev/null || true
cp -n ~/.claude/CLAUDE.md      ~/.claude/backups/CLAUDE.md.$ts     2>/dev/null || true
```

**In a live Claude Code session, capture the token baseline before you change anything:**

```
/context
```

Screenshot the breakdown (Messages / System / MCP / Tools). Without this number, Step 10 has nothing to compare against.

---

## Step 1 — System packages

All of these live in the `extra` repo on current Arch; AUR is not needed.

```bash
sudo pacman -Syu --needed \
  ripgrep fd jq tokei ast-grep difftastic \
  rustup git base-devel
```

**Binary names, explicitly:**

- `ripgrep` → `rg`
- `fd-find` is called `fd` on Arch (not `fdfind`)
- `ast-grep` → **`ast-grep`** on Arch, *not* `sg`. Arch's `/usr/bin/sg` is owned by the `shadow` package (group-switching utility). Do **not** use `sg` in CLAUDE.md rules on this system.
- `difftastic` → `difft`
- `tokei` → `tokei`

Verify:

```bash
rg --version && fd --version && jq --version \
  && tokei --version && ast-grep --version && difft --version
```

---

## Step 2 — Rust toolchain (for rtk)

```bash
rustup default stable

# Put cargo on PATH for both shells, idempotently
for rc in ~/.bashrc ~/.zshrc; do
  [ -f "$rc" ] && grep -q 'cargo/bin' "$rc" \
    || echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> "$rc"
done
export PATH="$HOME/.cargo/bin:$PATH"
```

---

## Step 3 — Install rtk (from git, not crates.io)

The crate name `rtk` on crates.io is a different project ("Rust Type Kit") and is **not** what you want.

```bash
cargo install --git https://github.com/rtk-ai/rtk --locked
```

**Verify both the binary and a functional subcommand:**

```bash
rtk --version   # expect "rtk 0.2x.x" or newer
rtk gain        # must return without "command not found"; shows token-gain stats
```

If `rtk gain` says "command not found," you installed the wrong crate. Uninstall (`cargo uninstall rtk`) and repeat the git install.

---

## Step 4 — Config structure

```bash
mkdir -p ~/.claude ~/.claude/hooks ~/.claude/backups
```

---

## Step 5 — Write `~/.claude/CLAUDE.md`

This file is **global** and applies to every project, so it must defer to per-project conventions when they exist. The version below:

- Uses `ast-grep`, not `sg` (shadow conflict).
- Defers to project `CLAUDE.md` / `AGENTS.md` / `docs/LLM_INDEX.md` before falling back.
- Uses the `docs/plans/<descriptive-topic>.md` plan convention (matches Tungsten's [docs/plans/](~/Projects/Tungsten/docs/plans/) scheme) instead of the v1 `milestone-<NN>` scheme.
- Keeps the protocols short enough to stay cheap.

Only run this block after Step 0 backed up any prior file.

```bash
cat > ~/.claude/CLAUDE.md <<'EOF'
## Read Discipline

If the project has `CLAUDE.md`, `AGENTS.md`, or `docs/LLM_INDEX.md`, read those
first and follow their read-path order. Only fall back to the rules below when
none of those exist.

Before reading any file:
1. `rg -l "<query>"` — filenames only
2. `rg -n -A 5 "<query>" <file>` — targeted context on the shortlist
3. Full file read only when structural surgery is required

Before reading JSON >20 lines: `jq` with an explicit field projection.
Before searching code structure (calls, patterns): `ast-grep --lang <L> -p '<pattern>'`.
Before any large `git diff`: `git diff --unified=1` or `difft`.
First call in an unknown repo: `tokei --output json`.

## Tool Preferences

- Text/regex search: `rg` with `-l`, `-F` for literals, `-t <lang>` to scope, `--max-count` to cap
- Structural code search: `ast-grep --lang <L> -p '<pattern>'` (on Arch: `ast-grep`, never `sg` — that is shadow-utils)
- File discovery: `fd --absolute-path`
- Diff: `git diff --unified=1` or `difft`
- JSON: always project needed fields with `jq`

## Forbidden

- No `--color` flags, no `bat`, no `eza`
- No `grep -R` (use `rg` or `ast-grep`)
- No `find ... -exec` (use `rg` or `fd -x`)
- No raw JSON dumps
- No unbounded `git diff`

## rtk

rtk is installed and hooks every Bash call via PreToolUse. Common verbose
commands (`git`/`cargo`/`npm`/`docker`/`go`) are compressed transparently; the
raw command still runs — only the output Claude sees is compressed. Scripts
that pipe output internally are unaffected. Use `rtk proxy <cmd>` only if the
default filter causes a problem.

## Planning Protocol

Use for milestone or feature planning sessions. Defer to the project's plan
convention when one is documented (e.g. Tungsten uses
`docs/plans/<descriptive-topic>.md` with `status`/goal/non-goals/files-to-touch/
ordered steps/done-when headers). Fall back to the generic flow below
otherwise.

1. `tokei --output json` — structural overview before any file read
2. Open `AGENTS.md`, `docs/LLM_INDEX.md`, or `README.md` (in that order), skip
   those that don't exist
3. Per relevant module: `ast-grep --lang <L> -p '<pattern>' -l` to locate
   boundaries without reading
4. Full file reads only when a structural question cannot be answered otherwise
5. Before writing the plan: `/compact focus on <scope>`
6. Output a single file at `docs/plans/<descriptive-topic>.md`. Required
   header fields: `status` (`draft` / `in progress` / `done`), goal, non-goals,
   files to touch, ordered steps, done-when checks. Include a short context
   digest (<500 tokens) at the top of the file rather than a separate
   `context.md`, unless the project convention says otherwise.

## Execution Protocol

Use for plan-execution sessions:

1. Read the plan file first. Do not re-explore the codebase — the plan is the
   map.
2. Per task: `ast-grep` / `rg -l` to locate exact touch points before editing
3. After each edit: scoped `rg -n -A 3` to verify; never full-file re-read
4. Use `git diff --unified=1` for checkpointing
5. When `/context` shows >60% Messages: `/compact focus on remaining tasks`
6. Update the plan's `status` field when done. Do not leave plans in
   `in progress` after the session ends.
EOF
```

---

## Step 6 — `.claudeignore` template

Provides a sane starting point for **new** projects. It must never overwrite a project that already has one.

```bash
cat > ~/.claude/claudeignore.template <<'EOF'
# Build artifacts
node_modules/
target/
**/target/
dist/
build/

# Lock files and logs
*.lock
*.log

# Secrets
.env*
*.pem
*.key

# Profiling / coverage artifacts
coverage/
perf-runs/
flamegraph.svg
perf.data
perf.data.old

# Archived planning docs — high token cost, low value
docs/plans/archive/
CHANGELOG.md

# IDE state
.vscode/
.idea/

# Binary assets (opt in per project)
# **/*.ttf
# **/*.png
# **/*.ogg
EOF
```

**To apply to a project without clobbering an existing ignore:**

```bash
cd /path/to/project
test -f .claudeignore || cp ~/.claude/claudeignore.template ./.claudeignore
```

Tungsten already has a stricter [.claudeignore](~/Projects/Tungsten/.claudeignore); do not overwrite it.

---

## Step 7 — Activate the rtk hook

`rtk init -g` edits `~/.claude/settings.json`. That file on this machine already contains permissions, the codex plugin registration, the context7 MCP, `defaultMode: bypassPermissions`, and more. The merge must preserve all of that.

**Back up, install, diff:**

```bash
ts=$(date +%Y%m%dT%H%M%S)
cp ~/.claude/settings.json ~/.claude/backups/settings.json.pre-rtk.$ts
rtk init -g
diff -u ~/.claude/backups/settings.json.pre-rtk.$ts ~/.claude/settings.json
```

**Hand-verify the diff.** The only change should be an added `hooks.PreToolUse` entry for `Bash` pointing at `~/.claude/hooks/rtk-rewrite.sh`. If anything else changed (permissions, plugins, model, MCP, `additionalDirectories`), restore the backup:

```bash
cp ~/.claude/backups/settings.json.pre-rtk.$ts ~/.claude/settings.json
```

…and raise it as an rtk bug.

**Confirm the hook is present and executable:**

```bash
test -x ~/.claude/hooks/rtk-rewrite.sh && echo "hook installed" || echo "hook MISSING"
jq '.hooks' ~/.claude/settings.json
```

**Note on permission mode.** This machine runs `defaultMode: bypassPermissions`. That means the rtk PreToolUse hook fires without any confirmation prompt. That is expected — but it also means a misbehaving hook will silently mutate every Bash call Claude makes. Step 7b exists to catch that.

---

## Step 7b — Tungsten smoke (catch hook regressions early)

The engine has two commands with heavy output that are the most likely to trip up rtk's filter:

```bash
cd ~/Projects/Tungsten

# Workspace build + tests — rtk should compress cargo output visibly
cargo test --workspace 2>&1 | tail -20

# Example smoke runner — tails failing logs; rtk should not drop that tail
bash scripts/smoke-examples.sh

# Perf capture — produces a markdown report consumed by another script
./scripts/perf-capture.sh ecs-high-load 60
bash scripts/test-perf-capture.sh
```

**Success criteria:**

- `cargo test --workspace` exits 0 (it already does pre-rtk; this confirms rtk didn't break return-code passthrough).
- `scripts/smoke-examples.sh` passes every example.
- `scripts/test-perf-capture.sh` passes. This is the critical one: if rtk ate lines from the perf report, the regression check fails and you'll see it here.
- `rtk gain --history | tail -20` shows non-zero compression on the cargo commands.

If any of the three fail, restore the settings.json backup from Step 7, then investigate.

---

## Step 8 — Full verification

```bash
# Binaries
rg --version && fd --version && jq --version \
  && tokei --version && ast-grep --version && difft --version \
  && rtk --version && claude --version

# rtk working
rtk gain

# Hook registered
jq '.hooks.PreToolUse' ~/.claude/settings.json
```

Inside a Claude Code session, confirm the baseline is unchanged at idle (rtk is runtime, so it should not bloat `/context`):

```
/context
```

Compare to the Step 0 screenshot. Messages / System / MCP should match within noise. If they don't, something in Step 5 or 7 bloated idle context.

---

## Step 9 — Per-project usage

**Bootstrapping a new project:**

```bash
cd /path/to/project
test -f .claudeignore || cp ~/.claude/claudeignore.template ./.claudeignore
mkdir -p docs/plans
```

**Plan session (fresh context):**

> *"Plan &lt;scope&gt; using the Planning Protocol in `~/.claude/CLAUDE.md`. Defer to any project CLAUDE.md or AGENTS.md. Output a single `docs/plans/<topic>.md`."*

**Execute session (fresh context):**

> *"Execute `docs/plans/<topic>.md` following the Execution Protocol. Start by reading the plan; do not re-explore."*

For Tungsten specifically, the project `CLAUDE.md` + `AGENTS.md` + `docs/LLM_INDEX.md` already encode a tighter read path; the global CLAUDE.md defers to those, so no per-project override is needed.

---

## Step 10 — Measure (against the Step 0 baseline)

Run a normal week with the new stack, then check `/context` at the points you would previously have compacted.

**Target:** Messages &lt; 40% of the window at peak. Total &lt; 60% before the first `/compact`.

**If Messages is still &gt;60% at peak:**

- Confirm Claude is actually using `rg -l` before reading whole files. Check a transcript.
- Tighten CLAUDE.md — move the most important rule to the top of the file and make it prescriptive.
- Run `rtk gain --history` to confirm rtk is active and compressing.
- Only then consider Phase 2 (below).

**If idle context grew &gt;5% versus Step 0:** something in the new CLAUDE.md or the hook is bloating the baseline. Revert the relevant piece.

---

## Step 11 — Optional: back up config

`~/.claude/settings.json` can contain OAuth material, so think before pushing it anywhere. If you do version-control, keep the gitignore strict and the repo private.

```bash
cd ~/.claude
git init
cat > .gitignore <<'EOF'
state/
projects/
plugins/cache/
sessions/
shell-snapshots/
history.jsonl
*.log
*.db
.credentials.json
oauth_*
session-env/
telemetry/
usage-data/
mcp-needs-auth-cache.json
live-today-stats.json
paste-cache/
EOF
git add CLAUDE.md claudeignore.template hooks/ settings.json .gitignore backups/.gitkeep 2>/dev/null
git commit -m "Initial Claude Code config"
```

---

## Directory map (post-install)

```
~/.claude/
├── CLAUDE.md                   # global protocols; defers to project conventions
├── settings.json               # rtk hook registered; everything else preserved
├── claudeignore.template       # bootstrap only; never cp over an existing one
├── backups/                    # pre-change snapshots
└── hooks/
    └── rtk-rewrite.sh          # installed by rtk init -g
```

---

## Rollback

Everything in this plan is reversible.

```bash
# Restore a prior settings.json
ls ~/.claude/backups/settings.json.*
cp ~/.claude/backups/settings.json.<timestamp> ~/.claude/settings.json

# Restore or remove global CLAUDE.md
cp ~/.claude/backups/CLAUDE.md.<timestamp> ~/.claude/CLAUDE.md   # or: rm ~/.claude/CLAUDE.md

# Uninstall rtk
cargo uninstall rtk
rm -f ~/.claude/hooks/rtk-rewrite.sh

# Uninstall system packages (only if you truly don't want them)
sudo pacman -R tokei ast-grep difftastic
# (keep rg / fd / jq / git / rustup — they are load-bearing elsewhere)
```

---

## When to escalate (Phase 2)

Only add these if `/context` shows the v2 protocol is not enough.

| Symptom | Add | Cost |
| --- | --- | --- |
| Execute sessions burn tokens grepping the wrong files for symbol lookups | `cclsp` MCP server + LSP kit (`find_definition`, `find_references`, `rename_symbol`) | ~3–5k idle tokens, ~800 with tool-search deferral; ~40× saving per symbol lookup |
| Plan sessions still sprawl despite structural search | `ast-grep-mcp` (`cargo install ast-grep-mcp`) | ~2–3k idle tokens |
| Claude routinely ignores "use `ast-grep` not `grep`" in CLAUDE.md | force-ast-grep hook; blocks `grep -R` on source files at the tool-call layer | negligible; acts only on matching calls |

Do not add preemptively. Measure first.

---

## Blind spots fixed vs. v1

1. **`sg` binary conflict.** On Arch, `/usr/bin/sg` is owned by `shadow` (group-switch), not ast-grep. The v1 CLAUDE.md rules told Claude to invoke `sg --lang ...`, which on this system calls the wrong binary. v2 uses `ast-grep` everywhere and calls this out explicitly in Step 1 and the `Forbidden` list.
2. **Settings.json overwrite risk.** The host's `~/.claude/settings.json` already contains a populated `permissions.allow/deny`, codex plugin, context7 MCP, `additionalDirectories`, model choice, and spinner config. v1 ran `rtk init -g` without a backup or verification; v2 requires backup + diff + rollback path in Step 7.
3. **`.claudeignore` clobbering.** The existing project ignore in [Tungsten's .claudeignore](~/Projects/Tungsten/.claudeignore) is stricter than v1's template (excludes `perf-runs/`, `CHANGELOG.md`, binary assets, `docs/plans/archive/`). v2 keeps the template bootstrap-only and guards the copy with `test -f`.
4. **Plan-file naming collision.** v1 hard-coded `docs/milestone-<NN>-plan.md`. Tungsten's project CLAUDE.md mandates `docs/plans/<descriptive-topic>.md` with specific header fields. v2's global protocol defers to project conventions and falls back to the Tungsten-style layout.
5. **Read path assumed `README.md` and `ARCHITECTURE.md`.** Neither is the right entry point for this repo — Tungsten uses `AGENTS.md` + `docs/LLM_INDEX.md` + `docs/DECISION_INDEX.md`. v2 lists those filenames first and treats `README.md` as a fallback.
6. **No before-baseline.** v1 said "measure at peak" but defined no before-number. v2's Step 0 captures `/context` pre-change and Step 10 compares against it with concrete targets (Messages &lt;40% at peak, idle growth &lt;5%).
7. **Hook-regression risk unvalidated.** v1 did not exercise rtk against Tungsten's `scripts/test-perf-capture.sh`, whose regex regression check is the most likely place an output-compressor would silently break something. v2's Step 7b runs it explicitly and makes it a gate.
8. **`bypassPermissions` interaction unstated.** With `defaultMode: bypassPermissions`, a PreToolUse hook fires with no prompt — useful, but makes silent failures more consequential. v2 calls this out in Step 7.
9. **Backup-repo hygiene.** v1's suggested `.gitignore` for `~/.claude` missed several token/state files (`analytics.db`, `sessions/`, `shell-snapshots/`, `paste-cache/`, `mcp-needs-auth-cache.json`, `telemetry/`). v2 expands it.
10. **Step idempotency.** v1's `cp`, `cat >`, and `grep -q | echo >>` commands were not all idempotent. v2 uses `cp -n`, `test -f || cp`, guarded `grep -q || echo >>`, and timestamped backups so a partial re-run is safe.
