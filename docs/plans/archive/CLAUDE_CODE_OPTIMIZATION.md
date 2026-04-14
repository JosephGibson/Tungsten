# Claude Code optimization report — Tungsten

**Scope:** Repository layout, documentation, CLI workflows, token-efficient tooling, indexing strategies, and **Claude Code Bash permissions** (default-allow patterns) for AI-assisted development.  
**History:** Initial read-only survey (2026-04-14); merged with follow-up research on CLI output bloat and project indexing.

---

## Executive summary

- **Documentation is strong but duplicated:** milestone status, stack summary, and read-order guidance repeat across `README.md`, `DESIGN.md`, `PHASE2.md`, `CLAUDE.md`, and parts of `AGENTS.md`. Agents that open several of these burn the same tokens repeatedly.
- **Navigation is better than average, but habits matter:** `AGENTS.md` and `docs/LLM_INDEX.md` are high-signal. The gap is **enforcing** “index first, then narrow `rg`, then read one file” — otherwise agents still glob or read large files top-to-bottom.
- **CLI output is a separate problem from “which CLI exists.”** Verbose `cargo test`, unfiltered `cargo metadata`, and unbounded `rg` can dominate context. Worse: **truncating build output blindly** (e.g. `cargo build | head`) can hide the real error at the end and force expensive rebuilds — see [§5](#5-cli-output-budget-token-aware-patterns).
- **Indexing is two channels:** (1) **Terminal / Claude Code** only sees what tools return unless you add **maps** (`LLM_INDEX`, optional per-crate `CLAUDE.md`, optional generated `PROJECT_INDEX`). (2) **Cursor (and similar)** can use **semantic codebase indexing** + `.cursorignore` — orthogonal to `.claudeignore`; refresh index after ignore changes — see [§6](#6-project-indexing-avoid-full-tree-scans).
- **Build and verify workflows are Cargo-centric:** no `Makefile`/`justfile`; `scripts/smoke-examples.sh` **duplicates** example package names from `Cargo.toml` — good candidate for `cargo metadata` + `jq` ([§4](#4-cli-leverage-opportunities)).
- **Code complexity is moderate:** small ECS footprint; “fat” modules (`manifest.rs`, `tilemap.rs`, `app.rs`) reward **search-first** reading, not architectural over-engineering.
- **`.claudeignore` is a win:** shrinks what gets pulled into agent context for binary assets and `target/`.
- **Bash approval friction burns turns:** Claude Code normally **asks** before each new shell command. Pre-approve **read-only–ish** project commands (`cargo *`, `rg *`, scoped `git *`, `scripts/smoke-examples.sh`, etc.) via **`permissions.allow`** in `.claude/settings.json` (team) or `.claude/settings.local.json` (personal) so routine builds/tests/search run without repeated prompts — see [§9](#9-claude-code-permissions-default-allow-routine-bash).

---

## 1. Project structure

### Root layout (high level)

| Path | Role |
|------|------|
| `Cargo.toml` | Workspace: `tungsten-core`, `tungsten-render`, `tungsten`, ten `examples/NN_*` packages, shared `[workspace.dependencies]`. |
| `crates/tungsten-core/` | ECS, assets/manifest/registry, config, input, time, physics, audio command types. **Entry:** `src/lib.rs`. |
| `crates/tungsten-render/` | wgpu renderer, sprite/quad/text pipelines. **Entry:** `src/lib.rs`. |
| `crates/tungsten/` | winit `App`, asset loading, hot reload. **Entry:** `src/lib.rs`. |
| `examples/01_window` … `10_platformer` | Current workspace examples (`example-NN-…`); derive dynamically from `cargo metadata` to avoid drift. |
| `assets/` | Shared assets + root `manifest.json`. |
| `scripts/smoke-examples.sh` | GPU smoke: workspace build, each example with `TUNGSTEN_SMOKE_FRAMES` + `timeout`. |
| `tungsten.json` | Engine config (startup). |
| `docs/` | `LLM_INDEX.md`, `plans/` (session plans + archive). |
| `.claudeignore` | Excludes `target/`, fonts, png, ogg from agent context. |
| `.claude/` | Local Claude Code settings (machine-specific). |

### Entry points for understanding code

- **Rust APIs:** each crate’s `lib.rs` + paths in `docs/LLM_INDEX.md`.
- **App / smoke:** `crates/tungsten/src/app.rs` (`App`, `TUNGSTEN_SMOKE_FRAMES`, system order).
- **Manifest tests (layer 1):** `crates/tungsten-core/tests/manifests.rs`.

### Rough scale (indicative)

- **~47** Rust files under `crates/` + `examples/`; **~7.3k** lines under `crates/`.
- **ECS:** hundreds of lines across a few files — not a sprawling framework.
- **Assets:** `manifest.rs` and `tilemap.rs` are among the largest files (“fat modules”).

---

## 2. Documentation audit

### Inventory (markdown and agent-oriented)

| Document | Approx. size | Audience | Claude Code relevance |
|----------|--------------|----------|------------------------|
| `AGENTS.md` | ~147 lines | Any AI assistant | **High** — commands, invariants, test layers. |
| `CLAUDE.md` | Short | Claude Code | **High** — router; keep minimal. |
| `docs/LLM_INDEX.md` | ~19 lines | Agents | **High** — subsystem → path. |
| `README.md` | ~41 lines | Humans | **Medium** — duplicates quick start in `AGENTS.md`. |
| `DESIGN.md` | ~301 lines | Architecture | **Medium** — skip for small edits. |
| `PHASE2.md` | ~197 lines | Roadmap | **Medium** — milestone work only. |
| `DECISIONS.md` | ~263 lines | Rationale | **Targeted** — grep `D-0xx`, do not read serially. |
| `CHANGELOG.md` | Growing | Releases | **Low** unless shipping. |
| `docs/plans/*.md` | Variable (can be large) | Handoff | **High** only when the active plan is in scope. |
| `docs/plans/archive/*` | Historical | Archaeology | **Low**. |
| `assets/fonts/README.md` | ~78 lines | Fonts | **Low** for engine code. |

### Redundancy and drift

- **Status line** repeated across `README`, `DESIGN`, `PHASE2`, `CLAUDE.md`.
- **Quick start** duplicated in `README` and `AGENTS.md`.
- **Read order:** `README` (human) vs `AGENTS` (AI) differs by design — worth one explicit sentence in `CLAUDE.md` so agents do not follow the human path by mistake.
- **`AGENTS.md`** mentions `<!-- OPEN: ... -->` in `PHASE2.md`; at last check **no such markers** were present — reconcile to avoid useless searches.

### What Claude Code needs vs. noise

| Need | Source |
|------|--------|
| Build / test / smoke | `AGENTS.md` |
| Where code goes / bans | `AGENTS.md` |
| “Where is X?” | **`docs/LLM_INDEX.md` first**, then scoped `rg` |
| Why a rule exists | `DECISIONS.md` by ID |
| Milestone scope | `PHASE2.md` or active `docs/plans/…` |
| Philosophy | `DESIGN.md` — optional for most edits |

---

## 3. Complexity assessment

### Not over-engineered

- **Three crates** (D-006): clear seam, GPU out of core.
- **Small-to-moderate ECS** surface with clear crate boundaries; avoid over-indexing on architecture docs for routine edits.
- **`App`:** ordered `Vec` of systems — traceable; some `Box<dyn …>` noise is acceptable.

### Where agents pay cost

- **Large files:** `manifest.rs`, `tilemap.rs`, `app.rs` — use **search + line ranges**, not full-file reads.
- **Long plans:** noise unless that specific plan is in scope.
- **`DECISIONS.md`:** grep-by-ID, not front-to-back.

### Simplification angles

- **Jump table + search** over “read DESIGN then DECISIONS end-to-end.”
- **One active plan file** for multi-session work (`docs/plans/`).
- Treat **CHANGELOG** as human/release-only for agents.

---

## 4. CLI leverage opportunities

Gains come from **workspace-derived lists**, **scoped search**, and **structured JSON slices** — not from adding heavy task runners prematurely.

### A. Smoke script ↔ workspace members

**Before:** `scripts/smoke-examples.sh` hardcodes `EXAMPLES=(…)` parallel to `Cargo.toml`.

**After:**

```bash
cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.name | test("^example-")) | .name' | sort
```

Loop the smoke run over that list (filter/order if some packages should be skipped). **Benefit:** no drift when adding `examples/11_foo`.

### B. Scoped discovery (prefer filenames-only, then open one file)

```bash
rg --files -g '*.rs' crates/tungsten-core/src/assets/
rg -n "ResolvedManifest" crates/tungsten-core/src/assets/
rg -n "TUNGSTEN_SMOKE_FRAMES" .
```

### C. Optional Cargo aliases

`.cargo/config.toml`:

```toml
[alias]
t-check = "fmt --all && test --workspace"
t-build = "build --workspace"
```

Document in `AGENTS.md` if adopted (discoverability).

### D. Manifests / JSON

```bash
fd manifest.json
jq '.sprites | keys' assets/manifest.json
```

### E. Git (scoped)

```bash
git log -n 5 --oneline -- crates/tungsten-core/src/ecs/
git blame -L 120,180 crates/tungsten/src/app.rs
git diff --stat
```

---

## 5. CLI output budget (token-aware patterns)

Agent loops pay for **every line** of tool output in the transcript. Practitioner and research notes converge on: **bound output**, **higher signal per line**, and **avoid truncation that hides errors**.

### Design principle

Treat CLI invocations like APIs: a **default path** that returns *enough* to act, plus **narrow follow-ups**. See [Rethinking CLI interfaces for AI](https://www.notcheckmark.com/2025/07/rethinking-cli-interfaces-for-ai/) — blind `cargo build | head -n 100` can drop the real diagnostic, force a **full rebuild** to see more, and give **no hint** that output was cut off.

### `cargo build` / `cargo check`

| Goal | Prefer | Avoid |
|------|--------|--------|
| Shorter diagnostics | `cargo build --message-format=short` | Wall of default rustc spans on huge crates |
| First failure visibility | On non-zero exit, rerun once: `… 2>&1 \| tail -n 80` | `build \| head` on the **first** attempt |
| Noise (only when appropriate) | Scoped `RUSTFLAGS` if you truly only want errors | Silencing warnings during real feature work |

### `cargo test`

| Prefer | Why |
|--------|-----|
| `cargo test --workspace` with **`-q`** or test harness `--quiet` | Less per-test chatter |
| **One package:** `cargo test -p tungsten-core` | Avoids whole-workspace paste |
| **One filter:** `cargo test -p tungsten-core manifest` | Narrows to a module/name |
| `--no-fail-fast` | Only when you need *all* failures (output grows) |

### `cargo metadata`

| Prefer | Avoid |
|--------|--------|
| `cargo metadata --no-deps` + **`jq`** selecting fields | Pasting full JSON into chat |
| `jq -c` for one-line records | Pretty-printed multi-screen blobs |

### `rg` (ripgrep) as a volume knob

| Pattern | Use when |
|---------|----------|
| `rg --files -g '*.rs' PATH` | File list **without** file bodies |
| `rg -l 'pattern' PATH` | Only paths with hits — then read **one** file |
| `rg -m 20 'pattern' PATH` | Cap matching lines **per file** (floods stop) |
| `--max-columns 200` | Skip/truncate absurdly long lines |
| `rg -c` | See match density before deep reading |

**Workflow:** `rg -l 'HotReload'` → pick 1 path → read with offset/limit if the tool supports it.

### `git`

- `git diff --stat` or `git diff -- path` — not whole-repo diffs in chat.
- `git log -n 5 --oneline -- path` — not full history.

### Defer or cap (high expansion risk)

| Command | Note |
|---------|------|
| `cargo tree` | Explodes wide; if needed: narrow `-p` and cap lines, or grep `Cargo.toml` first |
| `cargo doc` / rustdoc HTML | Human-first; poor as an agent index |

### Smoke / long-running scripts

- Success path: **`--quiet`** where logs are discarded anyway.
- Failure path: keep **tail of log** + **exit code** + **log path** (the existing smoke script already tails on failure — good pattern).

---

## 6. Project indexing (avoid full-tree scans)

### Two channels (do not conflate)

| Channel | What helps | What does not |
|---------|------------|----------------|
| **Terminal / Claude Code** | Static maps in repo (`LLM_INDEX`, optional per-crate `CLAUDE.md`, optional generated index) | Cursor’s embeddings unless the product explicitly uses them in that session |
| **Cursor / IDE** | Codebase indexing + `.cursorignore` / `.cursorindexingignore`; refresh after changes | Replacing `LLM_INDEX` for CLI-only workflows |

### Layer A — Mandatory “index first” (policy)

- Open **`docs/LLM_INDEX.md`** before broad `glob` / `list_dir` of `crates/`.
- Encode as **one imperative line** in `CLAUDE.md` or workspace rules so models actually do it.

### Layer B — Hierarchical `CLAUDE.md` (Claude Code pattern)

- Root **`CLAUDE.md`** stays a **short router**.
- Optional **`crates/tungsten-core/CLAUDE.md`** (etc.): **hot files**, ECS invariants, “do not touch render here” — local context without copying all of `AGENTS.md`.

### Layer C — Existing repo hooks

- **`.claudeignore`:** already drops `target/` and heavy binaries — good for token and noise.
- **`docs/LLM_INDEX.md`:** keep updated when subsystems move.

### Layer D — Cursor-style indexing (if you use Cursor)

- **`.cursorignore`:** align intent with `.claudeignore` where both apply (e.g. large tracked assets you never want in semantic search).
- Third-party guides note **`.cursorindexingignore`** for “exclude from index but maybe still readable” style splits; after edits, **refresh codebase indexing** in settings (Cursor documents indexing behavior in their blog/docs).

### Layer E — Generated or hand-maintained architecture map (optional)

- Community tools such as [**claude-code-project-index**](https://github.com/ericbuess/claude-code-project-index) produce a **`PROJECT_INDEX.json`**-style artifact (structure / symbols / relationships) so the agent reads **one file** instead of many discovery passes.
- **Tradeoffs:** regeneration when the tree changes; the index **must stay small** (module → key types → paths — not every private function) or it becomes its own token tax.
- **Alternative for Tungsten’s size:** a **short `docs/CODEMAP.md`** (80–120 lines) updated when architecture shifts — often beats a stale auto dump.

### Layer F — Workflow (no new files)

- **Subagent / sub-thread** with only `LLM_INDEX` + 2–3 paths + task text; merge in parent — limits quadratic growth of tool output ([agent loop / context constraints](https://www.augmentcode.com/guides/ai-agent-loop-token-cost-context-constraints)).
- **Plan file** lists the **exact paths to touch** after exploration so the next turn does not re-walk the tree.

### Tungsten-specific note

The repo is **small** (~47 Rust files); “scan every file” is rarely *required*. The failure mode is **habit** (unguided discovery). **Maps + policy** fix that more than new dependencies.

---

## 7. CLI tooling tradeoffs (bloat vs value)

### Low risk / high reward

| Tool | Role | Token note |
|------|------|------------|
| **`rg`** | Search | Use `-l`, `-m`, paths, `--files` — see §5 |
| **`cargo`** | Build / test / metadata | Short message format; scoped tests |
| **`git`** | History / scope | `--oneline`, path-scoped diff/log |

### Medium value / watch onboarding

| Tool | Role | Risk |
|------|------|------|
| **`jq`** | Slice `cargo metadata`, manifests | Syntax errors; worth it for scripts |
| **`fd`** | File listing | Extra binary; optional vs `rg --files` |

### Often defer

| Tool | Why |
|------|-----|
| **`just` / `make`** | Second language on Cargo; revisit if script count ≥ 3–4 real workflows |
| **`gh`** | Solo workflow per `AGENTS.md`; useful if you lean into PRs/releases |
| **Extra static analysis** beyond `clippy` | Hobby scope; clippy stays advisory |

### Discoverability

Anything not mentioned in **`AGENTS.md`** or **`CLAUDE.md`** is easy for agents to miss — one-line pointers beat “tool exists somewhere.”

---

## 8. `CLAUDE.md` recommendations

**Goal:** maximum routing signal, minimum tokens.

### Keep

- “Read **`AGENTS.md`** first.”
- Pointer to **`docs/LLM_INDEX.md`**.
- Session plans under **`docs/plans/`**.

### Add / tighten

1. **Index-first rule:** “Open `docs/LLM_INDEX.md` before broad repo search or glob.”
2. **Read path:** `AGENTS.md` → `LLM_INDEX.md` → **only** files for this task → scoped `rg`. Skip full `DESIGN.md` / `DECISIONS.md` unless architecture or rationale work.
3. **Decisions:** grep `D-0xx` or keyword in `DECISIONS.md`; never default to reading from line 1.
4. **Milestones:** `PHASE2.md` **or** one active `docs/plans/*.md` — not both unless reconciling drift.
5. **CHANGELOG:** skip unless releasing or answering “what changed in version X?”
6. **Humans vs agents:** one sentence — humans may start from `README`/`DESIGN`; agents start from `AGENTS`/`LLM_INDEX`.
7. **Optional:** hierarchical **`crates/.../CLAUDE.md`** when a subsystem gets heavy.
8. **CLI habit:** point to §5 of *this* doc or duplicate a **three-line** “bounded `rg` / scoped `cargo test`” reminder if you want it in-repo.
9. **Bash permissions:** one line pointing maintainers to **§9** of this doc (or to [Claude Code permissions](https://code.claude.com/docs/en/permissions)) so project-level `permissions.allow` patterns stay discoverable.

### Avoid in root `CLAUDE.md`

- Full command tables (live in `AGENTS.md` or this report).
- Long architecture prose.
- Duplicate status paragraphs (link `PHASE2.md` / `README.md`).

---

## 9. Claude Code permissions: default-allow routine Bash

Claude Code treats **Bash** as a permissioned tool: by default, the first use of each command shape tends to **prompt** for approval (“Yes” / “Yes, don’t ask again”). That protects you, but on a Rust workspace it also means **`cargo test`**, **`cargo build`**, **`rg`**, and **`git diff`** can each stall the agent while waiting for clicks — wasting time and context.

**Goal:** Let Claude run **expected, low-risk project commands** without asking every time, while still **denying** dangerous patterns (`git push`, `rm -rf`, blanket `curl | sh`, etc.).

Official docs: [Configure permissions](https://code.claude.com/docs/en/permissions) (`permissions.allow` / `deny`, wildcard rules, `/permissions` UI). Rules are evaluated **deny → ask → allow**; **deny wins**.

### Where to put rules

| File | Use |
|------|-----|
| **`.claude/settings.json`** | **Versioned** and shared if committed — good for **Tungsten-shaped** allowlists. |
| **`.claude/settings.local.json`** | **Gitignored**, personal machine — good for extra tools (`fd`, `tokei`) or stricter experiments. |

Do **not** commit secrets into either file.

Note: this repo currently ignores the whole `.claude/` directory in `.gitignore`. If you want shared team defaults, either unignore and commit only `.claude/settings.json`, or keep permissions local and document a bootstrap snippet in `AGENTS.md`.

### Example starter allowlist (Tungsten)

Adjust to taste; tighten wildcards if you want stricter gates.

```json
{
  "permissions": {
    "allow": [
      "Bash(cargo *)",
      "Bash(rg *)",
      "Bash(git status *)",
      "Bash(git diff *)",
      "Bash(git log *)",
      "Bash(git show *)",
      "Bash(git blame *)",
      "Bash(./scripts/smoke-examples.sh *)",
      "Bash(timeout *)",
      "Bash(jq *)",
      "Bash(wc *)",
      "Bash(tail *)",
      "Bash(head *)",
      "Bash(sort *)",
      "Bash(fd *)"
    ],
    "deny": [
      "Bash(git push *)",
      "Bash(git reset *)",
      "Bash(git clean *)",
      "Bash(rm *)",
      "Bash(curl *)",
      "Bash(wget *)"
    ]
  }
}
```

**Notes:**

- **Compound commands** (`&&`, `|`, etc.): each segment must match an allow rule independently — overly narrow allows can still prompt on pipelines; prefer simple invocations or widen patterns deliberately ([docs](https://code.claude.com/docs/en/permissions)).
- **`cargo publish`**, **`git commit`**, **`git stash`**: omitted above on purpose — you may want those to **ask** until you trust the workflow. Add `Bash(git commit *)` if you want commits auto-approved (still review diffs).
- **Smoke script** uses `timeout` and `cargo run` inside the script — allowing `Bash(cargo *)` covers inner `cargo` when the *invoked* command is the script; if Claude runs raw `cargo run -p example-…`, `Bash(cargo *)` covers that too.

### Permission *modes* (optional, heavier hammer)

Claude Code also has **permission modes** (`defaultMode` in settings): e.g. `acceptEdits` auto-accepts many **file** operations; `bypassPermissions` skips most prompts but is **unsafe** outside isolated VMs — see official [permission modes](https://code.claude.com/docs/en/permission-modes). For Tungsten, a **narrow `permissions.allow` list** is usually safer than global bypass.

### If you meant the Unix **`expect(1)`** program

That tool automates **interactive** TTY programs (password prompts, menus). You *can* add `Bash(expect *)`, but:

- Expect scripts often **embed secrets** or encourage **credential prompts** — high leak risk in logs and chat.
- Prefer **non-interactive** flags (`GIT_TERMINAL_PROMPT=0`, `cargo`/`apt` non-interactive modes, env tokens) instead.

Only allow `expect` deliberately, and almost never check expect scripts into the repo without review.

---

## Prioritized recommendations

| Priority | Recommendation | Impact | Effort |
|----------|----------------|--------|--------|
| P0 | **Index-first policy** in `CLAUDE.md` / rules: `LLM_INDEX` before broad discovery | High | Low |
| P0 | **Narrow reading** + `DECISIONS` grep-by-ID in agent docs | High | Low |
| P0 | Fix **`OPEN` markers** wording in `AGENTS.md` vs `PHASE2.md` reality | Medium | Low |
| P0 | Document **bounded CLI** (§5): especially **no `build \| head` on first try** | High | Low |
| P1 | **Smoke script** derives `example-*` from `cargo metadata` (+ `jq`) | Medium | Low–medium |
| P1 | Optional **`.cargo/config.toml`** aliases; one line in `AGENTS.md` | Medium | Low |
| P1 | If using **Cursor:** `.cursorignore` + index refresh discipline | Medium–High in IDE | Low |
| P1 | Add **`.claude/settings.json`** with `permissions.allow` for routine `cargo` / `rg` / scoped `git` / project scripts; pair with explicit **`deny`** for push/rm/curl | High — fewer stalled agent turns | Low |
| P1 | Decide shared-permissions strategy: commit `.claude/settings.json` with a targeted `.gitignore` exception, or keep local-only with documented bootstrap | High clarity | Low |
| P2 | Optional **per-crate `CLAUDE.md`** (core / render / umbrella) | High for deep subsystem work | Medium |
| P2 | **Fat modules:** module “map” comment or split *when already editing* | Medium | Medium |
| P2 | Huge inactive plans: add one-line scope banner at top (e.g. “Read only when this plan is active”) | Medium | Low |
| P2 | Optional **`PROJECT_INDEX` / `CODEMAP.md`** if file count grows a lot | Medium | Medium |
| P3 | **`just` / `make`** only after orchestration pain justifies it | Low until then | Medium ongoing |
| P3 | **MCP / symbol-peek tools** only if discovery remains expensive | Medium in huge repos | Higher |

---

## Concrete next steps (ordered)

1. **`CLAUDE.md` + `AGENTS.md`:** index-first sentence; agent read path; fix/remove stale `OPEN` marker guidance; optional three-line CLI budget pointer to this file’s §5; one-line pointer to **§9** (Bash `permissions.allow`).
2. **Permissions bootstrap:** choose one path and document it clearly:
   - shared: commit `.claude/settings.json` plus a narrow `.gitignore` exception for that file, or
   - local: keep `.claude/` ignored and provide copy-paste `permissions.allow`/`deny` defaults in `AGENTS.md`.
   Verify with `/permissions` in a session.
3. **`scripts/smoke-examples.sh`:** drive example list from `cargo metadata` (`^example-`); preserve `timeout` / `TUNGSTEN_SMOKE_FRAMES` / failure tails.
4. **Optional `.cargo/config.toml`** + one-line mention in `AGENTS.md`.
5. **Light status dedup:** canonical status in `README` + `PHASE2`; shorten headers elsewhere if desired.
6. **Multi-step work:** keep using `docs/plans/<topic>.md`; add **explicit file list** after reconnaissance.
7. **If on Cursor:** add/align `.cursorignore`; refresh codebase index after changes.
8. **If ECS/render split grows:** add `crates/tungsten-core/CLAUDE.md` or a ≤120-line `docs/CODEMAP.md` before adopting generated index tooling.
9. **Revisit `just`/`make`** when a third non-trivial script joins smoke.

---

## References (external)

- [Claude Code docs / overview](https://code.claude.com/docs/en/overview) — `CLAUDE.md`, sessions, product context.
- [Claude Code — Configure permissions](https://code.claude.com/docs/en/permissions) — `permissions.allow` / `deny`, `Bash(*)` wildcards, `/permissions`.
- [claude-code-project-index](https://github.com/ericbuess/claude-code-project-index) — optional `PROJECT_INDEX`-style generation.
- [Rethinking CLI interfaces for AI](https://www.notcheckmark.com/2025/07/rethinking-cli-interfaces-for-ai/) — truncation, `head`, agent UX.
- [Cursor: secure codebase indexing](https://cursor.com/blog/secure-codebase-indexing) — embedding/indexing model (IDE).
- [OpenDev / context engineering (arXiv)](https://arxiv.org/html/2603.05344v1) — tool results, compaction, agent harness design.
- [AI agent loop token costs](https://www.augmentcode.com/guides/ai-agent-loop-token-cost-context-constraints) — scope limits, resets, coordinator patterns.

---

*End of report.*
