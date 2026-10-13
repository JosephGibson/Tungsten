# Decision Index

Navigation keys and short takeaways for every decision heading in [`DECISIONS.md`](../DECISIONS.md). `crates/tungsten-core/tests/decision_index.rs` fails if a heading is missing here or an unknown ID appears, so a new decision adds its row in the same change. Read only the matching rows for a task; each `D-NNN` points to its canonical heading and the measurements/edge rules there. Find it with `rg -n '^## D-NNN\b' DECISIONS.md`, then read that section through the next decision heading. Historical plan paths may now live under `docs/plans/archive/` with the same basename; agents never open the archive.

## Foundations

| Decision | Takeaway |
| --- | --- |
| `D-001` | Project name is Tungsten; crate prefix stays `tungsten-`. |
| `D-002` | Local judgment over formal process; no CI-first workflow. Narrowed by `D-070`: CPU-only CI reports but doesn't block. |
| `D-003` | Native only. No WASM target or WASM-driven design compromises. |
| `D-004` | `wgpu` is the renderer. |
| `D-005` | No external ECS crate. ECS work stays in-project by design. |
| `D-006` | Three-crate workspace split is intentional: `tungsten-core`, `tungsten-render`, `tungsten`. |
| `D-007` | `tungsten-render` may depend on `tungsten-core`; strict isolation is not the goal. |
| `D-008` | Config loads at startup from the game's folder. Amended by `D-119`: ID/log validation; `D-123`: game-local path. |
| `D-102` | 1.0 means a complete game engine and stable library, tested by an acceptance game. Amended by `D-131`: R2 only, R1/R4 to 1.x. |
| `D-103` | Semver promises umbrella API, file formats and CLI; excludes direct crate/backend types. Amends `D-069`: MSRV bumps in minors. |
| `D-132` | Acceptance-game map sets W6/W13 tiers; bodies on roots, tilemap builder in W16b, world labels after M3. |

## Assets / Rendering

| Decision | Takeaway |
| --- | --- |
| `D-009` | Assets are manifest-driven and referenced by stable IDs, never file paths in game code. |
| `D-010` | Animations use Tungsten’s own small JSON format. |
| `D-011` | Sprite filter mode is per-sprite in the manifest. |
| `D-012` | Hot reload was deferred from Phase 1, but the ID-based asset model was kept compatible. |
| `D-013` | Shared/example assets and game-local assets. Amended by `D-123`: the game owns its folder, the engine its font. |
| `D-014` | Asset registry is a `World` resource, not a global singleton. |
| `D-016` | Core owns opaque asset handles only; no `wgpu` types in `tungsten-core`. |
| `D-017` | Multiple manifests compose by extension only; duplicate IDs are fatal. Amended by `D-089`: references are checked on the merged set. |
| `D-018` | Extract reads `&World`, passes plain data to render. Amended by `D-128`: five scheduled stages precede extract. |
| `D-023` | WGSL shaders are embedded with `include_str!`; shader edits require rebuilds. Narrowed by `D-057`: shader bodies now hot-reload; signature changes still rebuild. |
| `D-026` | Text rendering uses `glyphon` / `cosmic-text`. |
| `D-032` | Tilemaps use Tiled-compatible `.tmj` data and reuse the sprite render path instead of a separate tile pipeline. |
| `D-042` | Engine render components opt into default extract. Amended by `D-113`: IDs; `D-114`: culling; `D-138`: additive channels/tiles. |
| `D-048` | Deterministic per-filter shelf atlases; padding/inset, renderer-minted handles and rebuild on growth. |
| `D-054` | M24 easings are a closed `enum` with pure `apply(t)`; no trait object or dependency. |
| `D-055` | M24: one `Tween` per entity with `Vec<TweenChannel>` sharing easing/duration; extra tweens in a scene entry log `ERROR`, first wins. |
| `D-052` | Umbrella merges roots once; loaders consume the result. Amended by `D-089`: merged validation and reload. |
| `D-053` | Reload support: [DESIGN matrix](../DESIGN.md#hot-reload--m9); sounds stay session-static. Amended by `D-089`: rebuild all roots. |
| `D-057` | Manifest WGSL bodies hot-reload after Naga validation; signatures rebuild. Narrows `D-023`. Amended by `D-091`: stock post reload. |
| `D-058` | Manifest materials, reorderable post stack, uniform tweens. Narrows `D-023`, `D-055`. Amended by `D-090`: slot params; `D-091`: reload; `D-101`: batch UBOs. |
| `D-059` | Renderer-owned SMAA tail before text. Amended by `D-087`: normal frames present directly, captures retain the blit. |
| `D-060` | Bloom records an HDR mip pyramid and composite; scene color stays sRGB. Mip count is startup-only. |
| `D-061` | Forward normal-mapped lighting, capped at 16 lights; normal/emissive sibling atlases; lit wins over material. |
| `D-073` | CPU parallax, trauma shake and dedicated squash/stretch components. Narrows `D-018`, `D-042`, `D-055`. |
| `D-085` | Bounded text layout cache and reused post GPU objects. Amended by `D-090`: slot uniform writes; `D-117`: cache in `TextEngine`. Extract: `D-086`. |
| `D-086` | Extract reuses buffers and compact sort keys. Amended by `D-113`: registry IDs replace asset cache; `D-114`: culled keys stay. |
| `D-087` | Last full-screen stage writes swapchain, then text; captures keep the blit. Amends `D-059`. |
| `D-089` | Validate references on merged roots; any root reload rebuilds the graph. Amends `D-017`, `D-052`, `D-053`. |
| `D-090` | Stock post parameters belong to each stack slot. Amends `D-058`, `D-085`. |
| `D-091` | Stock post pipelines build from the shader cache and rebuild on body reload. Amends `D-057`, `D-058`. |
| `D-093` | Instanced mesh particles and screen transitions. Amends `D-051`. Amended by `D-129`: transitions use real time. |
| `D-101` | Each material batch gets its own UBO slot, preventing last-write aliasing (`D-090` slots). Amends `D-058`. |
| `D-113` | Interned `SpriteAssetId` indexes the registry; file formats keep names. Amends `D-042`, `D-086`. |
| `D-114` | Cull off-view stock/lit sprites, keep sort keys; material sprites remain. Amends `D-042`, `D-086`. |
| `D-115` | Manifest font families and ordered fallback chain validate against the merged graph. |
| `D-116` | Packaged fonts by default; system faces opt in through `render.system_fonts`. `unicode-script`: `D-015` rule 2. |
| `D-117` | Device-free `TextEngine`, GPU `TextPipeline`, neutral core types and retained nodes. Amends `D-085`. Amended by `D-125`: `TextNodeStore` seam. |
| `D-123` | Game-local config/assets, engine font, template outside-copy check and canonical input reload path. Amends `D-008`, `D-013`, `D-045`. |
| `D-126` | 1.0 glyph path: shared atlas, batch renderers, deferred trim (T1b); owned path/R3 deferred. Extends `D-026`. |
| `D-127` | Logical UI layout, physical text rasterization, hinting off; scene scaling deferred to 1.x. |
| `D-145` | W2 R2 late acquire: passes before the swapchain stage record and submit before the acquire; a resize inside it records them again; `render_encode` stays whole, `render_encode_pre_acquire` its part before. |

## Dependencies / Tooling

| Decision | Takeaway |
| --- | --- |
| `D-015` | New dependencies must satisfy one of three acceptance rules: platform API, well-specified format, or solved primitive. |
| `D-019` | `pollster` blocks on `wgpu` async init. |
| `D-020` | `bytemuck` handles GPU POD layout. |
| `D-025` | Project license is MIT. |
| `D-027` | `cpal` handles audio device access. |
| `D-028` | `symphonia` handles audio decoding. PCM WAV added by `D-077`. |
| `D-031` | Hot reload uses `notify` and a simple watcher/event flow, not an async runtime. |
| `D-034` | Audio command channel uses `rtrb` SPSC ring buffer. |
| `D-037` | `criterion` is used for render-side micro-benchmarks. |
| `D-038` | Frame timing uses inline `Instant` instrumentation in `app.rs`; no extra profiling crate for core telemetry. |
| `D-041` | Release/profile tuning is the baseline. Amended by `D-096`: core dev opt-level 1; `D-111`: unpacked debuginfo. |
| `D-068` | One shared instruction body, scoped render rules and symlinked skills; `just ctx`. Amended by `D-106`: 12 KiB index. |
| `D-069` | Pinned Rust, edition 2024, strict CPU and dependency gates. Amended by `D-103`: 1.x MSRV bumps only in minors. |
| `D-070` | Informational CPU CI; GPU/audio/perf local. Narrows `D-002`. Amended by `D-112`: dev-profile benches; `D-121`: Windows tests. Releases: `D-071`. |
| `D-071` | Version/changelog cut and release archives. CPU levels: `D-072`. README/handoff superseded by `D-074`; tag-push trigger by `D-079`. Amended by `D-120`: debug/probe; `D-122`: notices. |
| `D-072` | Portable/v3 builds behind std-only launchers. Supersedes `D-071`'s generic build. Amended by `D-122`: notices. |
| `D-074` | Canonical release guide, version-free README and Git preflight. Supersedes `D-071`'s README/handoff; post-merge tags/local squash superseded by `D-079`. |
| `D-079` | Human commit/tag/push/PR handoff; merge publishes the tagged tree. Supersedes `D-071`'s tag trigger and `D-074`'s post-merge tag/local squash. |
| `D-077` | Symphonia's `pcm` codec is enabled, so PCM WAV decodes (the platformer's synthesized sound effects); extends `D-028`. |
| `D-078` | Seeded benchmark suite, stdlib runner, owned metrics and per-run verdicts; pacing evidence here. Narrows `D-044`. Amended by `D-095`: validity. |
| `D-095` | Background load or tree edits invalidate captures; digest equality is informational. Amends `D-078`. Amended by `D-141`: pre-build refusal replaces duplicate manual prechecks. |
| `D-096` | Core dev/test opt-level 1; release/perf builds unchanged. Amends `D-041`. |
| `D-097` | Human publication; terse evidence and one changelog line. Patch-series clause superseded by `D-098`; local-commit clause by `D-105`. |
| `D-098` | Remove per-step patch handoff, superseding `D-097`'s patch-series clause. Per-step commit clause superseded by `D-105`. |
| `D-104` | Tracked public-API snapshots. Amended by `D-105`: before release; `D-107`: pinned toolchain, api/ paths and release check. |
| `D-105` | Sessions leave work uncommitted; human release block makes the only commit. Supersedes `D-097`/`D-098` commit clauses; amends `D-104`. |
| `D-106` | 12 KiB on-demand index; root rules stay 6 KiB with asset detail linked out. Amends `D-068`. |
| `D-107` | Pinned-toolchain rustdoc JSON snapshots, `just api`/`api-check`. Amends `D-104`. Amended by `D-110`: umbrella testing feature. |
| `D-108` | Adjacent A/B candidates with separate files may share plan/release; revises the definition gate's one-release rule. |
| `D-109` | Checked roadmap catalog and one prompt home; viewer fed from tree-derived state. Amended by `D-118`: stages/session recommendations. |
| `D-111` | Dev/test debuginfo in unpacked .dwo files; supported backtraces, smaller/faster builds. Amends `D-041`. |
| `D-112` | CI builds benchmarks in dev profile; local bench profile remains. Amends `D-070`. |
| `D-118` | Roadmap stages from prompt flows; session model/mode/effort by stop. Amends `D-109`. Amended by `D-136`: A/B releases use Opus max. |
| `D-120` | Separate identity-checked release debug archives and crash probe. Amends `D-071`. |
| `D-121` | Informational CPU-only Windows test job. Amends `D-070`. |
| `D-122` | Release archives include version-checked third-party/license notices; `just notices`. Amends `D-071`, `D-072`. |
| `D-124` | Taffy behind Tungsten layout styles, never public (`D-015` rule 3); no grid, rounding at extraction. |

| `D-140` | Bounded maintenance plans in-session; saved handoffs and milestone/API reviews remain. Canonical checks: [validation](validation.md). |
| `D-141` | Perf run/suite/sweep refuse known encoder/foreign-build load before building; existing override and measured scans remain. Amends `D-095`. |

## ECS / Runtime Flow

| Decision | Takeaway |
| --- | --- |
| `D-021` | Entity IDs started as `u32`; generational IDs shipped in M12. |
| `D-022` | Panic on programmer errors, return `Option`/`Result` on runtime conditions. |
| `D-024` | Phase 1 close-out observations were recorded to guide Phase 2. |
| `D-029` | Audio mixer stays hand-rolled; no `kira`. |
| `D-030` | The M12 ECS rewrite required an explicit go/no-go decision. |
| `D-033` | Hand-rolled core physics; position separate from rendering. Dt limit amended by `D-088`, `D-094`. |
| `D-035` | Manifest merge order is call-site order, usually shared manifest first then example-local. |
| `D-036` | Archetypal ECS rewrite is intentional and benchmark-validated. Its storage description is amended by `D-083`. |
| `D-039` | Deferred structural changes flush before extract. Amended by `D-084`: typed queues and reused buffer. |
| `D-040` | Two-window typed events. Amended by `D-128`: World registration/flush; `D-137`: step views; `D-139`: zero-step holds. |
| `D-043` | Display config and runtime apply at frame boundaries through requested settings. |
| `D-044` | Off-by-default umbrella telemetry HUD. Narrowed by `D-078`: suite measures overhead; input toggles follow `D-045`. |
| `D-045` | Core action names/bindings with watcher reload. Amended by `D-123`: game-local canonical input path. |
| `D-046` | Engine dispatcher, state stack and scene-owned entity cleanup; scenes reuse render components. |
| `D-047` | Core debug data, independent overlays and offscreen screenshot readback; GPU labels/groups always on. Amended by `D-142`: one-frame captures whose result comes back. |
| `D-049` | In-tree PCG32 + SplitMix64; no RNG dependency. |
| `D-050` | Emitters and live particles retain Arc config snapshots across reload. |
| `D-051` | Entity per particle; deferred despawn and explicit budgets. Amended by `D-093`: mesh particles. |
| `D-056` | TweenComplete event and deferred terminal removal. Amended by `D-099`: same-frame replacement survives. |
| `D-062` | Reuse spatial broadphase across substeps. Supersedes `D-033` rebuild; staging/budget/prefilter superseded by `D-075`; layout amended by `D-080`. |
| `D-063` | Warm-started soft constraints. Map rebuild superseded by `D-076`; substep order amended by `D-092`. |
| `D-064` | Speculative CCD, four substeps, seam face masks. Supersedes `D-033` CCD clause. Amended by `D-092`: solver-injected travel. |
| `D-065` | Deterministic island sleeping, explicit wake paths. Amended by `D-082`: parallel arrays replace rebuilt keyed map. |
| `D-066` | Dense SoA gather/writeback around substeps. Event-order clause superseded by `D-075`. |
| `D-067` | Physics stays serial; parallel prototype dropped on measured costs; threading rule stands. |
| `D-075` | Persistent pairs under travel budgets. Supersedes `D-062` staging/prefilter and `D-066` event order. Budget rebuild/radius superseded by `D-081`; sweep amended by `D-080`. |
| `D-076` | Warm starts follow persistent pair indices. Supersedes `D-063` map rebuild. Amended by `D-081`: repair by pair key. |
| `D-080` | Statics sweep grid, direct compact cell table, compact flags. Amends `D-062` layout and `D-075` sweep note. |
| `D-081` | Repair pairs only for tripped proxies; full rebuild for wakes/large changes. Supersedes `D-075` rule (a)/radius; amends `D-076`. |
| `D-082` | Sleep arrays parallel to proxies, rebuilt only on body-sequence change. Amends `D-065` keyed map/rebuild. |
| `D-083` | Sorted columns, unboxed moves and slot-table access. Amends `D-036` storage; accepted costs recorded here. |
| `D-084` | Typed command queues, function-pointer removes, reused buffer and grouped inserts. Amends `D-039`; accepted regression recorded here. |
| `D-088` | Wall dt cap and smoke pin stand. Amends `D-033`. Amended by `D-094`: physics bound; `D-110`: harness uncapped; `D-129`: Time/fixed loop. |
| `D-092` | Arrival pass clamps solver-pushed bodies against reached neighbours. Amends `D-063`, `D-064`; residuals/accepted costs recorded here. |
| `D-094` | Direct physics step bound defaults to 1/30 s. Amends `D-088`, `D-033`. Amended by `D-129`: bounded fixed loop; direct-call bound stands. |
| `D-099` | Flush removes a completed tween only while still pending, sparing replacements. Amends `D-056`. |
| `D-100` | Camera smoothing is a per-1/60 s rate, independent of frame rate. |
| `D-110` | Harness runs the real frame body without devices, behind testing feature. Amends `D-088`, `D-107`. Amended by `D-125`: UiHarness. |
| `D-119` | Game ID selects user folder; engine logger and crash hook installed once. Amends `D-008`: validation. |
| `D-125` | Core generational UI tree, theme/layout, hit testing/focus and TextNodeStore seam; headless UiHarness. Amends `D-110`, `D-117`. |
| `D-128` | Five closed schedule stages, named systems/plugins and core event registration. Amends `D-018`, `D-040`. |
| `D-129` | Real/game Time, bounded fixed steps, timers and default interpolation. Amends `D-088`, `D-093`, `D-094`. |
| `D-130` | Safe tuple queries/filters and bundles; old arities deprecated to freeze. Amended by `D-135`: landed forms/accessor, measured costs, PrevPosition at W3b. |
| `D-131` | 1.0 rendering scope R0/R2; no workers or Send/Sync bounds; R1 to 1.x. Amends `D-102`; `D-067` and two-thread rule stand. |
| `D-133` | W15a landed: schedule plugins, named engine systems, core WindowSize; particles benchmark ownership. |
| `D-134` | W3a landed: Time/Timer edge rules and engine clock migration; DeltaTime deprecated. |
| `D-135` | W15b landed: tuple/slice queries, accessors and one-move bundles; accepted costs recorded here. Amends `D-130`. |
| `D-136` | Fable for level C/gates/spikes; A/B releases use Opus max. Amends `D-118`. |
| `D-137` | W3b landed: bounded fixed loop, input/event step views and interpolation. Amends `D-040`. Amended by `D-139`: zero-step holds; `D-146`: stopped-clock input edges drop. |
| `D-138` | W15c landed: additive extracts, tiles before sprites, explicit replace preserved, lit tiles (`D-061`). Amends `D-042`. |
| `D-139` | Zero-step frames hold bounded events for fixed readers; HUD counts step contacts. Amends `D-040`, `D-137`. |
| `D-142` | Render frames return presented/skipped or a failed frame; one-frame captures report their result; `App::run` returns the render failure, else the capture error. Amends `D-047`. |
| `D-143` | Burst `once: false` re-arms through `rearm_burst` and never drains; the latch is `burst_fired`; zero total pulses emit nothing and fail `validate`. |
| `D-144` | W1 M0b input groundwork: 28 more key variants, an ordered per-frame raw input stream in `InputState`, focus loss releasing held input, modifiers, pointer leave and the `ScaleFactor` resource. |
| `D-146` | A zero-step frame on a stopped clock (game dt 0, real dt above 0) drops the fixed view's input edges: a press made while paused or at scale 0 reaches no step; a zero-dt frame keeps them. Amends `D-137`. |

## When To Open a Decision

Find the relevant heading with `rg -n 'D-0NN' DECISIONS.md`, then read that section only.

- You are considering a new dependency.
- A change would alter the core/render seam, asset-ID model, or frame-order invariants.
- You think a current behavior looks wrong but it may be intentional.
- You need the detailed rationale or consequences behind a specific `D-0xx`.
