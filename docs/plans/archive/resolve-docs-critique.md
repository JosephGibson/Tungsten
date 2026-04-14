---
status: done
goal: Address the GPT-5 critique of AGENTS.md, PHASE2.md, DECISIONS.md, DESIGN.md
non-goals:
  - Cross-platform CI pipeline (D-002 stands; hobby project)
  - EntityId generational indices (D-021, defer to M12)
  - Full asset path canonicalization / symlink tests (hobby-project risk is low)
  - Cross-platform Rust rewrite of smoke-examples.sh (document Linux-only instead)
  - D-018 seam rework (extract-phase ID resolution already addresses this correctly)
---

# Plan: Resolve Major Docs Critique

## Context

An external GPT-5 critique (`docs/plans/critique-major-docs.md`) reviewed AGENTS.md,
PHASE2.md, DECISIONS.md, and DESIGN.md and found 9 HIGH, 8 MEDIUM, and 1 LOW issue.
Most are documentation gaps or contradictions. Two warrant real code changes (user confirmed):
the audio RT-safety callback and the tilemap format (adopt Tiled verbatim before M13).

---

## Disposition of every finding

### ACCEPTED → fix in this plan

| Finding | Severity | Action |
|---------|----------|--------|
| M12/M13 dependency contradiction | HIGH | PHASE2.md one-liner fix |
| Layer 1 test misses cross-manifest IDs | HIGH | manifests.rs code fix |
| glyphon branch-pinned, not commit-pinned | HIGH | Cargo.toml + D-026 note |
| Smoke test is Linux-only — undocumented | HIGH | AGENTS.md note |
| Audio mpsc callback RT-unsafe | HIGH | Fix: replace with lock-free ring |
| Tilemap format ambiguous (custom vs Tiled) | HIGH | Adopt Tiled .tmj; update D-032 |
| Variable-dt physics undocumented risk | HIGH | DESIGN.md + PHASE2.md note |
| Tilemap collider perf (O(tiles × substeps)) | HIGH | DECISIONS.md budget note |
| ECS "no breaking changes" is too broad | MEDIUM | PHASE2.md M12 compat section |
| Handle ABA — extract-phase mitigation undocumented | MEDIUM | DESIGN.md clarification |
| Manifest merge order unspecified | MEDIUM | DECISIONS.md note |
| Shaders excluded from M9 hot reload — undocumented | LOW | AGENTS.md one-liner |

### DEFERRED → DECISIONS.md note only

| Finding | Reason |
|---------|--------|
| Audio device format conversion plan | M8 complete, hobby-scope acceptable; note in D-029 |
| Audio hot-reload PCM race | Audio is not hot-reloadable in M9; clarify in DESIGN.md |
| World drop / GPU resource teardown order | Document known teardown order in D-014 |
| M11 physics stability (stacking, moving platforms) | Explicit M11 scope note; advanced cases are M13 |

### REJECTED

| Finding | Reason |
|---------|--------|
| D-018 renderer-reads-registry seam rework | Extract-phase resolves all IDs → handles before render; DESIGN.md clarification suffices |
| EntityId generational indices | D-021 accepted tradeoff; correct fix is M12 ECS rewrite |
| Full asset path containment / symlink tests | Hobby project; no untrusted asset sources |
| Cross-platform Rust smoke runner | D-002 (no CI); document Linux-only instead |

---

## Files to touch

- `docs/plans/resolve-docs-critique.md` — this plan (create first)
- `PHASE2.md` — M12/M13 dep wording, M12 compat definition, M11 scope note
- `AGENTS.md` — smoke test Linux note, shader exclusion note
- `DESIGN.md` — extract-phase ABA clarification, variable-dt note, audio hot-reload scope
- `DECISIONS.md` — amend D-026, D-029, D-032, D-033; add D-034 (audio RT); add D-035 (tilemap collider budget); note D-014 teardown
- `Cargo.toml` — pin glyphon to a specific commit hash
- `crates/tungsten-core/tests/manifests.rs` — aggregate-and-check global ID uniqueness
- `crates/tungsten/src/audio.rs` — replace mpsc with lock-free ring buffer
- `crates/tungsten-core/src/assets/tilemap.rs` — adopt Tiled .tmj schema
- `crates/tungsten/src/tilemap_extract.rs` — update extract to match new schema
- `examples/09_tilemap/assets/` — replace custom .tmj files with Tiled-format JSON
- `docs/plans/critique-major-docs.md` — update status to `in progress` at start, `done` at end

---

## Ordered steps

### Phase A — Documentation fixes (all text edits, no code)

- [x] A1. Create this plan file; mark `critique-major-docs.md` → `in progress`
- [x] A2. PHASE2.md edits
- [x] A3. AGENTS.md edits
- [x] A4. DESIGN.md edits
- [x] A5. DECISIONS.md edits

### Phase B — Small code fixes

- [x] B1. Pin glyphon to a commit hash in Cargo.toml
- [x] B2. Add `all_manifest_ids_are_globally_unique` test to manifests.rs

### Phase C — Audio callback fix

- [x] C1. Add `rtrb` to `crates/tungsten/Cargo.toml`
- [x] C2. Replace mpsc with rtrb ring buffer in `crates/tungsten/src/audio.rs`
- [x] C3. `cargo test --workspace` green

### Phase D — Tiled tilemap format adoption

- [x] D1. Read Tiled .tmj schema; plan struct changes
- [x] D2. Rewrite `TilemapData` parser in `crates/tungsten-core/src/assets/tilemap.rs`
- [x] D3. Verify `crates/tungsten/src/tilemap_extract.rs` (no changes needed — internal repr unchanged)
- [x] D4. Convert `examples/09_tilemap/assets/tilemaps/demo.tmj` and `examples/10_platformer/assets/tilemaps/level.tmj` to Tiled format
- [x] D5. DECISIONS.md D-032 updated
- [x] D6. `cargo test --workspace` green

---

## Done when

- [x] `critique-major-docs.md` status → `done`
- [x] PHASE2.md: M13 dep reads "M7–M11, and M12 if executed"; M12 compat levels defined; M11 scope note present
- [x] AGENTS.md: smoke-test Linux note present; shader hot-reload exclusion note present
- [x] DECISIONS.md: D-026, D-029, D-032, D-033, D-014 all amended; D-034 and D-035 added
- [x] DESIGN.md: extract-phase ABA note present; variable-dt note present; audio hot-reload scope clarified
- [x] `cargo build --workspace` green after glyphon commit pin
- [x] `cargo test --workspace` green with new `all_manifest_ids_are_globally_unique` test
- [x] Audio callback uses rtrb ring buffer
- [x] Tilemap loader parses Tiled .tmj schema; `cargo test --workspace` green
- [x] All 9 HIGH findings have explicit dispositions recorded above
