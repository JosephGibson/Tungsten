# Decision Index

Use this as the cheap rationale lookup before opening [`DECISIONS.md`](../DECISIONS.md).

This file intentionally references every current `D-xxx` heading in [`DECISIONS.md`](../DECISIONS.md). The workspace test suite checks that coverage, so any new decision should add a matching short takeaway here in the same change.

If you need deeper context, grep the specific `D-0xx` entry in the full log instead of reading it serially.

## Foundations

| Decision | Takeaway |
| --- | --- |
| `D-001` | Project name is Tungsten; crate prefix stays `tungsten-`. |
| `D-002` | Local judgment over formal process; no CI-first workflow. |
| `D-003` | Native only. No WASM target or WASM-driven design compromises. |
| `D-004` | `wgpu` is the renderer. |
| `D-005` | No external ECS crate. ECS work stays in-project by design. |
| `D-006` | Three-crate workspace split is intentional: `tungsten-core`, `tungsten-render`, `tungsten`. |
| `D-007` | `tungsten-render` may depend on `tungsten-core`; strict isolation is not the goal. |
| `D-008` | One workspace-root `tungsten.json`, loaded at startup. Missing file falls back to defaults; invalid JSON is fatal. |

## Assets / Rendering

| Decision | Takeaway |
| --- | --- |
| `D-009` | Assets are manifest-driven and referenced by stable IDs, never file paths in game code. |
| `D-010` | Animations use Tungsten’s own small JSON format. |
| `D-011` | Sprite filter mode is per-sprite in the manifest. |
| `D-012` | Hot reload was deferred from Phase 1, but the ID-based asset model was kept compatible. |
| `D-013` | Shared assets live under workspace `assets/`; examples can have local `examples/NN_name/assets/`. |
| `D-014` | Asset registry is a `World` resource, not a global singleton. |
| `D-016` | Core owns opaque asset handles only; no `wgpu` types in `tungsten-core`. |
| `D-017` | Multiple manifests compose by extension only; duplicate IDs are fatal. |
| `D-018` | Extract plain render data before drawing; renderer should not need long-lived mutable `World` access. |
| `D-023` | WGSL shaders are embedded with `include_str!`; shader edits require rebuilds. |
| `D-026` | Text rendering uses `glyphon` / `cosmic-text`. |
| `D-032` | Tilemaps use Tiled-compatible `.tmj` data and reuse the sprite render path instead of a separate tile pipeline. |
| `D-042` | `Transform`, `Sprite`, `Visibility`, and `Tag` are engine-level components; default sprite extraction is explicit and opt-in through those components. |

## Dependencies / Tooling

| Decision | Takeaway |
| --- | --- |
| `D-015` | New dependencies must satisfy one of three acceptance rules: platform API, well-specified format, or solved primitive. |
| `D-019` | `pollster` blocks on `wgpu` async init. |
| `D-020` | `bytemuck` handles GPU POD layout. |
| `D-025` | Project license is MIT. |
| `D-027` | `cpal` handles audio device access. |
| `D-028` | `symphonia` handles audio decoding. |
| `D-031` | Hot reload uses `notify` and a simple watcher/event flow, not an async runtime. |
| `D-034` | Audio command channel uses `rtrb` SPSC ring buffer. |
| `D-037` | `criterion` is used for render-side micro-benchmarks. |
| `D-038` | Frame timing uses inline `Instant` instrumentation in `app.rs`; no extra profiling crate for core telemetry. |
| `D-041` | Release/profile tuning is part of the current baseline; perf comparisons should assume those settings. |

## ECS / Runtime Flow

| Decision | Takeaway |
| --- | --- |
| `D-021` | Entity IDs started as `u32`; generational IDs shipped in M12. |
| `D-022` | Panic on programmer errors, return `Option`/`Result` on runtime conditions. |
| `D-024` | Phase 1 close-out observations were recorded to guide Phase 2. |
| `D-029` | Audio mixer stays hand-rolled; no `kira`. |
| `D-030` | The M12 ECS rewrite required an explicit go/no-go decision. |
| `D-033` | Physics is hand-rolled in `tungsten-core`; `Position` stays separate from gameplay render components. |
| `D-035` | Manifest merge order is call-site order, usually shared manifest first then example-local. |
| `D-036` | Archetypal ECS rewrite is intentional and benchmark-validated. |
| `D-039` | `CommandBuffer` is a world resource with post-system flush; deferred structural changes are visible to extract/render in the same frame and to systems on the next frame. |
| `D-040` | `EventQueue<T>` keeps two windows (`previous`, `current`) and flushes once per frame after systems. |
| `D-043` | Display settings live in `tungsten.json`, runtime changes go through `request_display_settings`, and actual window/surface mutation happens only at a frame boundary. |

## When To Open Full `DECISIONS.md`

- You are considering a new dependency.
- A change would alter the core/render seam, asset-ID model, or frame-order invariants.
- You think a current behavior looks wrong but it may be intentional.
- You need the detailed rationale or consequences behind a specific `D-0xx`.
