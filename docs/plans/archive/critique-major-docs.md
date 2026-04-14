---
status: done
source: gpt-5 critique via `/critique` on 2026-04-14
scope: AGENTS.md, PHASE2.md, DECISIONS.md, DESIGN.md
plan: docs/plans/resolve-docs-critique.md
---

# Critique findings — major docs

External (non-Claude) critique of the four major design/process docs. Grouped by theme; each item links back to the originating doc and severity. This is a **findings list**, not an execution plan — decide per item whether to act, defer, or reject.

## Done when

- Each HIGH finding has an explicit disposition: accepted (→ milestone/task), rejected (→ note in DECISIONS.md), or deferred (→ PHASE2.md note).
- MEDIUM/LOW items are at least triaged.

---

## Theme 1 — Audio real-time safety and device format (M8)

Appears in **DESIGN**, **PHASE2**, **DECISIONS**. Three related HIGHs collapse into one workstream.

- **RT-unsafe callback path.** Mixer runs in cpal's callback thread and drains `std::sync::mpsc::try_recv`. std mpsc can lock/allocate; unbounded drain makes callback time unpredictable. (DESIGN #1 HIGH, PHASE2 #3 HIGH)
  - Suggested: preallocated lock-free SPSC ring, capped drain per callback, all heap/IO off the callback thread.
- **No device format conversion plan.** D-028/D-029 decode to `Vec<f32>` but don't specify resampling, channel up/down-mix, sample format conversion (i16/u16/f32), or mix headroom/limiter. cpal devices vary; mismatches yield wrong pitch, stutter, clipping, or crashes. (DECISIONS #2 HIGH, DESIGN #5 MEDIUM)
  - Suggested: pick internal mixer format (e.g. f32 stereo @ device rate), convert at load or pre-callback, document device-change handling, add headroom/soft-clip.
- **Hot-reload of PCM buffers races the callback.** (DESIGN #4 MEDIUM, PHASE2 #3 HIGH)
  - Suggested: Arc/RCU-style swap so callback never sees partially updated buffers; double-buffer handles; stress test (rapid trigger + reload + device change) as M8 acceptance.

## Theme 2 — Physics stability and scaling (M11)

- **Variable-dt physics is a stability trap.** Committing to 2D physics under variable timestep risks non-determinism, tunneling, frame-rate-dependent behavior. (DESIGN #2 HIGH)
  - Suggested: semi-fixed accumulator loop for physics now, even if rendering stays variable-dt.
- **Tilemap collider rebuild is O(tiles × substeps).** D-033.4 scans every TilemapInstance per substep; with up to 8 substeps and a 256×256 map, millions of AABBs per frame. (DECISIONS #3 HIGH)
  - Suggested: restrict generation to region overlapped by dynamic AABBs (or camera); or pre-bake a static spatial index rebuilt on hot reload only. State a tile-count budget.
- **No stability acceptance for platformer cases.** Grid + MTV + restitution has no tests for stacking, moving platforms, contact persistence, high-speed edges. (PHASE2 #5 MEDIUM)
  - Suggested: add targeted tests (3–5 stacked bodies, moving platform, fast entity vs tile edge) or explicitly scope them out for M13.

## Theme 3 — Handle/registry lifetime and hot-reload ABA (M9, cross-cutting)

- **World drop can leak GPU resources.** D-014 lets World (and registry) drop while renderer keeps wgpu resource lifetime; renderer pools keyed by dead handles aren't notified. Teardown boundary undefined. (DECISIONS #4 MEDIUM)
  - Suggested: tie renderer pools to registry lifetime, or explicit invalidation/free on World drop, or guaranteed drop order — and document it.
- **Handle swap risks in-flight use + ABA.** Draw lists caching handles can UAF across swap; pool index recycling without generations enables ABA. (DESIGN #4 MEDIUM)
  - Suggested: generation/epoch tags on handles; defer destruction to end-of-frame / GPU fence / audio block boundary.

## Theme 4 — ECS rewrite compatibility (M12)

- **"No breaking changes to examples" is unrealistic for archetypal storage.** Even identical signatures can change iteration order/determinism, borrow aliasing, add/remove side effects. (PHASE2 #4 MEDIUM)
  - Suggested: define compatibility levels (source/binary/behavior); add determinism/ordering tests; consider feature-flag gating new storage for M13.
- **M13 depends on M7–M12 but M12 is "conditional" (D-030).** Dependency graph makes M12 de facto mandatory, contradicting the skip option. (PHASE2 #1 HIGH)
  - Suggested: restate M13 dep as "M7–M11 and M12 if executed."

## Theme 5 — Asset pipeline: IDs, paths, manifests

- **Layer 1 test doesn't detect cross-manifest duplicate IDs.** Loading each manifest in isolation can't surface global collisions, yet AGENTS claims duplicates are "fatal at load time." (AGENTS #1 HIGH)
  - Suggested: aggregate all discovered manifests into one registry in the test and assert global uniqueness.
- **Manifest merge order is implied but not specified.** D-017 allows later manifests to reference earlier IDs; the loader doesn't document how order is chosen (call-site? lexical? DFS include?). (DECISIONS #5 MEDIUM)
  - Suggested: specify a deterministic order; fatal diagnostic for forward references.
- **Asset path containment / symlink policy is undocumented despite past bug.** No rule for canonicalization, `..`, symlinks/junctions across platforms. (AGENTS #4 MEDIUM)
  - Suggested: canonicalize against whitelisted root (assets/ and example-local assets/); reject post-resolution escapes; tests with `..` and symlinks on Unix + Windows.
- **D-018's renderer-reads-registry exception leaks the core/render seam.** "Renderer doesn't need mutable World at draw" but may read registry for ID resolution — weakens the boundary, risks borrow conflicts. (AGENTS #3 MEDIUM)
  - Suggested: resolve all IDs → handles in extract; pass only POD/handles to render; enumerate any true exceptions with a minimal read-only API.
- **Shaders don't fit the "IDs enable hot reload" narrative.** Shaders are `include_str!`'d in tungsten-render/src, not manifest-tracked, so M9 hot reload doesn't apply. (AGENTS #5 LOW)
  - Suggested: explicitly document shaders as code-bundled and excluded from M9 (or spec a separate plan).

## Theme 6 — Build reproducibility and CI

- **Unpinned git dep + no CI = fragile builds.** glyphon pinned to `main` (D-026) plus no CI gate (D-002) means upstream can silently break builds, even for old commits; bisect becomes hard. (DECISIONS #1 HIGH)
  - Suggested: pin to a commit hash or vendor a snapshot; minimal CI that builds on the three target OSes.
- **Smoke test runner is Bash-only.** `scripts/smoke-examples.sh` uses Bash + `/tmp` + GNU `timeout`; not portable to Windows, shaky on macOS. With no CI and GPU-required examples, wiring regressions will slip past non-Linux contributors. (AGENTS #2 HIGH)
  - Suggested: cross-platform Rust runner (per-example timeouts, `std::env::temp_dir`, backend overrides); document a Windows path.

## Theme 7 — Content tooling for M13

- **"Custom .tmj" is ambiguous.** Using Tiled's extension with an incompatible schema blocks off-the-shelf authoring and slows M13 content iteration. (PHASE2 #2 HIGH)
  - Suggested: adopt Tiled's `.tmj` verbatim, or rename the format, or ship a tested importer before M13 starts.

## Theme 8 — Entity identity

- **`u32` EntityId with no generations.** D-021 accepts ID reuse risk for Phase 1, but Phase 2 features (targets/parents/constraints) will cache IDs, creating silent stale-reference bugs. (DESIGN #3 MEDIUM)
  - Suggested: monotonic allocator with tombstones, or generational indices with liveness checks, before Phase 2 inter-entity features land.

---

## Severity roll-up

**HIGH (9):** Audio RT-safety (DESIGN/PHASE2), audio device format (DECISIONS), tilemap collider perf (DECISIONS), variable-dt physics (DESIGN), M13/M12 dependency contradiction (PHASE2), tilemap format ambiguity (PHASE2), Layer 1 duplicate-ID test gap (AGENTS), smoke runner portability (AGENTS), unpinned git dep + no CI (DECISIONS).

**MEDIUM (8):** Physics stability scenarios, ECS behavior-compat, handle swap ABA, World-drop leaks, manifest merge order, asset path containment, D-018 seam leak, EntityId generations, audio hot-reload race, audio format mismatches.

**LOW (1):** Shaders outside M9 hot reload.
