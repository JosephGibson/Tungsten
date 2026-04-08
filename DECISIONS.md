# DECISIONS.md

Append-only log of non-obvious decisions for Tungsten. Exists so future-me (or an AI session picking up cold) doesn't have to re-litigate settled questions.

## Rules

- **Append-only.** Never edit or delete an entry. If a decision is reversed, add a new entry that supersedes the old one by number.
- **Numbered sequentially.** `D-001`, `D-002`, etc.
- **Dated.** ISO format.
- **Short.** Context, decision, alternatives, consequences. If an entry is more than ~15 lines, I'm probably overthinking it.

## Template

```
## D-NNN — Title
**Date:** YYYY-MM-DD
**Status:** Active | Superseded by D-XXX
**Context:** What prompted this?
**Decision:** What did I pick?
**Alternatives:** What else was on the table?
**Consequences:** What does this commit me to?
```

---

## D-001 — Project name: Tungsten
**Date:** 2026-04-07
**Status:** Active
**Context:** Needed a name. Element names are an easy well; Tungsten is dense, rare, high-melting-point, not already taken by a prominent Rust project I could find.
**Decision:** Tungsten. Crate prefix `tungsten-`. Umbrella crate is `tungsten`.
**Alternatives:** Several other element names; various abstract words. Nothing else felt better.
**Consequences:** Crate naming locked in.

## D-002 — Framing: hobby project, fun first
**Date:** 2026-04-07
**Status:** Active
**Context:** Early versions of the design treated this as a serious engineering project with quality gates, mandatory documentation, contrarian architectural rules, and heavy process. Wrong framing for a solo hobby project. The biggest risk is not technical debt — it's losing interest and abandoning the project.
**Decision:** Optimize every process and architectural decision for "will I want to come back to this on a Saturday." Kill criteria documented in `DESIGN.md` to replace quiet abandonment with explicit reassessment.
**Alternatives:** Treat it as a serious engineering project with full process. Rejected: that process is what kills hobby projects.
**Consequences:** Most "rules" are soft. No CI gate. No mandatory docs. No self-review checklist. Judgment over compliance.

## D-003 — Native only, no WASM
**Date:** 2026-04-07
**Status:** Active
**Context:** wgpu supports browser targets, but supporting WASM constrains dependency choices and doubles the test matrix.
**Decision:** Native targets only (Linux, macOS, Windows).
**Alternatives:** Dual-target from day one. Rejected on cost.
**Consequences:** Free use of any native-only crate. Revisiting would be nontrivial.

## D-004 — wgpu as renderer, not ash or glow
**Date:** 2026-04-07
**Status:** Active
**Context:** Need a cross-platform GPU API.
**Decision:** `wgpu`.
**Alternatives:** `ash` (raw Vulkan, too much yak-shaving for triangles), `glow`/OpenGL (dated, less transferable).
**Consequences:** Renderer is a wgpu wrapper. Kill-criteria fallback if wgpu proves too painful is `pixels` or `macroquad`, not escalation to `ash`.

## D-005 — Hand-rolled ECS, no external crate
**Date:** 2026-04-07
**Status:** Active
**Context:** ECS is one of the main things I want to understand by building. `bevy_ecs` and `hecs` exist but using them defeats the purpose.
**Decision:** Build it by hand. Naive `HashMap<TypeId, ...>` first; evolve based on real pain. No external ECS crate, ever.
**Alternatives:** `hecs` (most reasonable external option). Rejected: the build is the point.
**Consequences:** Performance will be poor early. Archetypal rewrite is possible but not committed. If naive stays good enough forever, that's a success, not a failure.

## D-006 — Cargo workspace, three crates to start
**Date:** 2026-04-07
**Status:** Active
**Context:** Earlier draft proposed six crates; over-splitting for this size.
**Decision:** `tungsten-core` (ECS, math, config, time, resources, asset registry types), `tungsten-render` (wgpu wrapper, sprite drawing, samplers), `tungsten` (umbrella + winit app loop). Split further only when a crate becomes genuinely unwieldy.
**Alternatives:** Single crate; six+ crates. Both rejected.
**Consequences:** Some "logically separate" concepts (config, math) live inside `tungsten-core` until they grow.

## D-007 — `tungsten-render` is allowed to know about `tungsten-core` types
**Date:** 2026-04-07
**Status:** Active
**Context:** An earlier draft forbade any dependency from the renderer to the ECS. On reflection, that rule is a contrarian bet for a solo hobby project, and Bevy couples them for good reasons.
**Decision:** `tungsten-render` may depend on `tungsten-core` and use its types where convenient.
**Alternatives:** Enforce the separation (previous position). Rejected as process-for-process's-sake.
**Consequences:** Simpler glue code in M3 and beyond. Renderer can be tested against hand-built data or against a World, whichever is more convenient.

## D-008 — Config is JSON, loaded once at startup
**Date:** 2026-04-07
**Status:** Active
**Context:** Engine-level parameters shouldn't require recompilation to change.
**Decision:** JSON via `serde_json`. Single `tungsten.json` at workspace root. Loaded once, validated, passed by value to subsystems. No global, no hot reload. Missing → defaults with warning. Invalid → fatal naming the bad field.
**Alternatives:** TOML (nicer to hand-write, candidate for later); RON (niche); hardcoded constants (ruled out by principle 5).
**Consequences:** `serde` and `serde_json` are Phase 1 deps. Each config section lives near the subsystem it configures.

## D-009 — Manifest-driven assets, ID-referenced
**Date:** 2026-04-07
**Status:** Active
**Context:** Sprites, animations, and sounds need to be loaded somehow. Two paths: code-driven (`load_sprite("path.png")`) or manifest-driven (a JSON file lists everything; code references by ID).
**Decision:** Manifest-driven. `assets/manifest.json` registers every asset by string ID. Game code references assets by ID, never by path. Manifest paths are relative to the manifest file. Validation at load time catches missing files and unresolved references.
**Alternatives:** Code-driven (simpler but couples code to file paths and makes hot reload painful); hybrid (no clear win over pure manifest).
**Consequences:** Slight extra ceremony to add new assets (edit manifest). Renaming/moving files is a manifest edit, not a code change. The indirection layer makes Phase 2 hot reload feasible. Errors get caught at startup, not at use site.

## D-010 — Custom JSON animation format, not Aseprite export
**Date:** 2026-04-07
**Status:** Active
**Context:** Frame-based animation needs a data format. Two reasonable options: roll my own JSON or adopt Aseprite's well-known JSON export schema.
**Decision:** Roll my own. Each animation lives in its own file under `assets/animations/`, registered in the manifest. Schema: `looping` (bool), `frames` (array of `{sprite: id, duration_ms: u32}`).
**Alternatives:** Aseprite's export format (standard, used by many indie games, but a third-party schema I'd be locked into); inlined animations in the manifest (rejected — manifest grows hostile to read).
**Consequences:** Tiny dep surface. Format under my control, can evolve. If I ever author in Aseprite, I write a converter. Per-frame durations (rather than fixed framerate) keep emphasis frames possible.

## D-011 — Per-sprite filter mode in the manifest
**Date:** 2026-04-07
**Status:** Active
**Context:** Want to support both pixel art (needs nearest-neighbor) and high-res sprites/UI (wants bilinear) in the same project, possibly the same scene. A global filter setting can't express this.
**Decision:** Filter mode is a per-sprite property in the manifest, with values `nearest` or `linear`. Default `nearest`. Renderer creates a sampler per filter mode and binds the right one when drawing each sprite.
**Alternatives:** Global filter (rejected — can't mix art styles); per-draw-call API (rejected — pushes the choice into game code); inferred from file naming convention (rejected — magic).
**Consequences:** Sampler management lives in the renderer. Two samplers (one nearest, one linear) created at startup. Mixing art styles in one frame is free. Future blend modes, mipmap settings, wrap modes can be added as additional manifest fields without breaking changes.

## D-012 — Hot reload of assets is a Phase 2 milestone
**Date:** 2026-04-07
**Status:** Active
**Context:** Hot reload is a major dev-experience win for 2D games. Runtime cost is essentially zero if implemented right. Implementation cost is moderate — about a weekend with `notify`. Risk of doing it in M5: scope creep on an already-large milestone.
**Decision:** M5 ships without hot reload. Hot reload is a planned Phase 2 milestone. The M5 design must preserve the indirection that makes hot reload feasible: every asset reference goes through the registry by ID, no direct GPU handles in game code.
**Alternatives:** Ship hot reload in M5 (rejected — scope risk); skip hot reload entirely (rejected — too valuable for the cost).
**Consequences:** M5 stays focused on the basic asset path. Phase 2 picks this up cleanly because the architectural prerequisites are in place.

## D-013 — Asset directory layout: by-type at workspace root
**Date:** 2026-04-07
**Status:** Active
**Context:** Three plausible layouts: flat `assets/`, by-type subdirectories, or per-example assets only.
**Decision:** Shared `assets/` at workspace root, organized by type (`sprites/`, `animations/`, `sounds/`), with `manifest.json` at its root. Examples that need throwaway assets ship `examples/NN_name/assets/` with a local manifest. The asset loader takes a manifest path.
**Alternatives:** Flat layout (becomes a junk drawer); per-example only (no shared content); deeply nested by category (premature).
**Consequences:** Manifest sections match folder structure, easy to scan. Adding a new asset type later is a new directory and manifest section.

## D-014 — Asset registry is a Resource in the World
**Date:** 2026-04-08
**Status:** Active
**Context:** The asset registry (sprite ID → runtime asset handle, animation ID → animation data) needs to live somewhere. Two natural options: as a `Resource` in the ECS World, alongside `DeltaTime` and `InputState`, or as a separately-passed object threaded through whatever needs it.
**Decision:** The registry is a `Resource` in the World. Systems that need asset lookup get it through the same mechanism as any other resource. The renderer may read it when resolving asset IDs during extraction or draw setup.
**Alternatives:** A separately-owned `Assets` object passed to whatever needs it (more explicit, but a second "global-ish" pathway in a design that's trying to have one). A static/singleton (ruled out — no global mutable state).
**Consequences:** The registry's lifetime is tied to the World's. Systems have uniform access to assets. The renderer's dependency on core types is justified partly by needing to read this resource. If the World is ever dropped and recreated, the registry and its opaque handles die with it, while the renderer remains responsible for the actual `wgpu` resource lifetime behind those handles.

## D-015 — Dependency philosophy: three acceptance rules
**Date:** 2026-04-08
**Status:** Active
**Context:** Earlier drafts let in `winit`, `wgpu`, `glam`, `serde_json`, `image`, future `notify` and `cpal`, but forbade `bevy_ecs` / `hecs` / `macroquad`. The rule separating these was never stated, which meant every future dep question would be argued from scratch and the answers would drift.
**Decision:** A dependency is acceptable if at least one of three rules applies:
1. It abstracts a platform API I'd otherwise have to write OS-specific code for (`winit`, `wgpu`, `notify`, `cpal`).
2. It implements a well-specified data format that isn't the interesting part of what I'm building (`serde_json`, `image`).
3. It provides a math/primitive that isn't architecture and is a solved problem (`glam`).
A crate that hands me something the project is supposed to teach me to build is not acceptable (any ECS crate, any higher-level engine, any rendering helper). Gray-zone crates get an explicit decision log entry when considered.
**Alternatives:** No stated rule (status quo, ruled out for drift reasons). A stricter "no external crates ever" rule (ruled out — would force me to write my own JSON parser and PNG decoder, which is the opposite of the learning target). A looser "whatever ships the game" rule (ruled out — defeats principle 2).
**Consequences:** Future dep decisions reference these rules and either find a match or become decision log entries arguing the gray zone. `symphonia` will probably be a gray-zone decision when audio starts. `rodio` / `kira` are already excluded by the "hands me an engine" side.

## D-016 — `tungsten-core` stores opaque asset handles, not `wgpu` types
**Date:** 2026-04-08
**Status:** Active
**Context:** The core/render seam already said core owns registry types while render uploads textures and returns handles, but "handle" was underspecified. Leaving it vague risked either leaking `wgpu` types into `tungsten-core` or pushing ad hoc ownership decisions into implementation.
**Decision:** `tungsten-core` stores opaque runtime asset handles (newtypes or IDs), not raw `wgpu` objects. Core owns manifest data, decoded CPU-side asset data, animation data, and the registry shape. `tungsten-render` owns GPU textures, samplers, and other `wgpu` resources in internal pools keyed by those handles.
**Alternatives:** Let core store `wgpu` texture objects directly (tighter coupling, weaker crate boundary). Make render own the full registry (rejected — breaks the "assets as a World resource" rule).
**Consequences:** `tungsten-core` stays free of `wgpu` types. The registry remains the only game-facing lookup path, while render retains ownership of GPU resource lifetime and implementation details.

## D-017 — Multiple manifests compose by extension, never override
**Date:** 2026-04-08
**Status:** Active
**Context:** The design allowed shared assets at the workspace root and example-local manifests, but did not define how multiple manifests combine. That ambiguity would force the first loader implementation to invent conflict rules on the fly.
**Decision:** Multiple manifests compose by extension only. Asset IDs must be globally unique across the merged manifest set; duplicate IDs are fatal at load time. Each path resolves relative to the manifest file that declared it. Later manifests may reference earlier IDs, but they may not replace them.
**Alternatives:** Last-wins override semantics (rejected — too implicit and easy to misuse). Separate namespaces per manifest (rejected — extra ceremony with little payoff for Phase 1).
**Consequences:** Manifest composition stays simple and predictable. Example-local assets can extend shared content without silently shadowing it.

## D-018 — Phase 1 rendering extracts plain data before drawing
**Date:** 2026-04-08
**Status:** Active
**Context:** The frame loop already separated tick and render, and the renderer was allowed to know about core types, but the exact World-to-render handoff was not named. That left unnecessary room for borrow-checker fights and accidental renderer dependence on long-lived World access.
**Decision:** In Phase 1, systems mutate the `World` during `tick`, then the app extracts plain render data (`QuadInstance`, `SpriteInstance`, or similar) into temporary buffers and passes slices of that data into `tungsten-render`. The renderer may read the asset registry to resolve IDs to runtime handles, but it should not require long-lived mutable access to the `World` during draw.
**Alternatives:** Let render operate directly on the `World` for the full frame (simpler at first, but tighter coupling and more borrow friction). Force a hard ECS/render separation everywhere (rejected — too much purity for this project).
**Consequences:** Direct-data render APIs stay first-class for testing. The borrow boundary is clearer, and Phase 1 keeps the simple single-threaded flow without committing to a more elaborate extraction architecture.
