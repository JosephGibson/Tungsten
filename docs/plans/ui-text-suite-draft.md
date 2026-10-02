# UI, interface and text suite — exploratory draft

- **status:** draft
- **goal:** Explore an ergonomic, efficient UI foundation for game interfaces and engine debug views, including a measured comparison of current text rendering and MSDF.
- **non-goals:** Implementation, dependency changes, settled design decisions, release work, or changes to existing examples. Editable fields, docking and native multi-window UI are outside the first slice.
- **files to touch:** This draft only. Future implementation locations below are proposals, not a change list for this session.
- **ordered steps:** Review the relevant engine seams; research primary sources; collect owner preferences; draft alternatives and a recommendation; identify open questions and future acceptance checks.
- **done-when:** The draft records confirmed preferences, source-backed research, proposed ownership/render/input contracts, performance evaluation, and a migration outline, with unresolved choices clearly identified.

Date: 2026-10-02. All API names, feature stages and numerical targets below are provisional. This is a discussion document; implementation planning follows design finalization.

## Context and confirmed preferences

Tungsten has a hand-written ECS, a synchronous frame loop, manifest assets, and cached screen-space text, but no general widget/layout/focus system. Preserve its three-crate architecture and extract/draw seam. Make game UI and debug tools equally important consumers of one foundation. Start with display text and buttons; editable fields follow. Windows initially mean in-game panels/dialogs and movable debug windows; docking can wait. Evaluate MSDF alongside the current renderer using quality and performance evidence. The authoring API is still undecided. Existing examples remain unchanged throughout exploration and implementation until the design and foundation are finalized.

## 1. What the project already provides

This is a focused architectural review, not a full project correctness audit. Example source was inspected read-only to identify migration consumers. Existing unrelated working-tree changes are outside this draft.

| Existing surface | Implication for UI |
| --- | --- |
| [`tungsten-core`](../../crates/tungsten-core/src/lib.rs): `World`, resources, `CommandBuffer`, `EventQueue<T>`, `ActionMap`, opaque asset handles | UI state and interactions can participate in ordinary game systems without GPU dependencies. |
| [`App`](../../crates/tungsten/src/app.rs): serial systems, command flush, event rotation, reload, extract, render | Input routing needs an explicit position before game systems. Layout/extraction should see their final mutations. |
| [`TextPipeline`](../../crates/tungsten-render/src/text.rs): glyphon/cosmic-text, advanced shaping, registered fonts, wrap/clip bounds, bounded layout cache | Reuse shaping and caching. Expose measurement before UI layout; currently shaping is private to the rendering pipeline. |
| [`pass order`](../../crates/tungsten-render/src/passes/order.rs): scene → post → optional SMAA → text overlay | Expand the final overlay to include all screen UI primitives. Keep game UI out of scene post effects by default. |
| [`input bridge`](../../crates/tungsten/src/input_bridge.rs) and [`InputState`](../../crates/tungsten-core/src/input.rs) | Current bridge handles physical keys, pointer and scroll state. It does not forward text, IME or an ordered UI input stream; gamepad support is absent. |
| [`DebugHud`](../../crates/tungsten/src/debug_hud.rs), [`SystemTimingOverlay`](../../crates/tungsten/src/systems_overlay.rs), [`InspectorState`](../../crates/tungsten/src/inspector.rs) | Existing resource/provider models should survive migration. Their text refresh is throttled; outlines repeat text draws, and anchoring uses a monospace-width heuristic. |
| [`StateStack`](../../crates/tungsten/src/state.rs) and [`DebugDraw`](../../crates/tungsten-core/src/debug_draw.rs) | UI roots need state cleanup and pause/resume behavior. Physics lines and collider geometry remain world-space diagnostics. |

Locked dependencies at review: wgpu 30.0.1, glyphon 0.12.0, cosmic-text 0.19.0, winit 0.30.13. `Cargo.toml` permits winit starting at 0.30.12; the lockfile establishes the version reviewed.

Relevant decisions: D-006 (three crates), D-015 (dependencies), D-016/D-018 (handles/extract), D-026 (text), D-039/D-040 (commands/events), D-044/D-047 (independent debug tools), D-045 (input), D-085 (text caching), D-087 (direct/capture presentation). See the [decision index](../DECISION_INDEX.md); this draft adds no decisions.

## 2. Research: patterns worth borrowing

The observations below come from primary documentation. The Tungsten recommendations are our interpretation, not claims that another engine's design or performance transfers automatically.

| Reference | Observed pattern | Proposed lesson for Tungsten |
| --- | --- | --- |
| [Bevy UI](https://docs.rs/bevy/latest/bevy/ui/) and [text nodes](https://docs.rs/bevy/latest/bevy/ui/widget/struct.Text.html) | UI uses entities/components, layout nodes and text components; layout owns UI transforms. | Expose UI state to systems, but separate UI layout from world transforms/cameras. Borrow the pattern, not Bevy's ECS dependency. |
| [Godot Control](https://docs.godotengine.org/en/4.6/classes/class_control.html) and [focus navigation](https://docs.godotengine.org/en/stable/tutorials/ui/gui_navigation.html) | Controls provide pointer filtering and explicit focus neighbors; mapped actions drive navigation. | Treat input consumption, initial focus, focus restoration and navigation as foundational contracts. |
| [Unity UI Toolkit event handling](https://docs.unity3d.com/6000.3/Documentation/Manual/UIE-Events-Handling.html) | Hierarchical events have a target and propagation through ancestors. | Distinguish internal widget event routing from application actions; a modal consumes input before gameplay receives it. |
| [egui](https://github.com/emilk/egui#why-immediate-mode) | Per-frame functions return button responses; stable identities preserve interaction/window state. Its documentation discusses layout and large scrolling-content tradeoffs. | Study its concise API and debug widgets. Immediate authoring still needs stable identity and retained internal state. Benchmark either model rather than declaring one universally faster. |
| [Taffy 0.14 layout APIs](https://docs.rs/taffy/0.14.0/taffy/) | A layout solver, with text/image measurement hooks and a low-level API for an existing tree. | Strong candidate for layout only. Tungsten still owns widgets, input, themes and painting. Evaluate a small enabled feature set. |
| [cosmic-text shaping](https://docs.rs/cosmic-text/0.19.0/cosmic_text/enum.Shaping.html), [glyph output](https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.LayoutGlyph.html), [editing](https://docs.rs/cosmic-text/0.19.0/cosmic_text/trait.Edit.html) | Advanced shaping supplies font fallback and glyph/cluster information; editing has separate APIs. | Use shaped metrics for measurement and drawing. Future editing needs clusters, selection and caret geometry, independent of rasterization. |
| [AccessKit](https://accesskit.dev/) | A platform accessibility bridge for custom-rendered toolkits, including a winit adapter. | Keep semantic roles, labels, values and stable widget IDs in the design; evaluate platform integration separately. |

### Build versus adopt

Recommend a Tungsten-owned UI model and renderer integration, with focused existing primitives reused underneath. Taffy is the first layout dependency to evaluate under D-015 rule 3 (solved layout primitive); it is not approved or added here. A deliberately narrow hand-written stack/anchor layout is the alternative if we decide Flexbox is unnecessary. Avoid accidentally committing to a home-grown full CSS implementation.

Using egui directly is a serious alternative: it would supply many debug controls and editing features sooner. Tradeoffs to evaluate are asset/theme integration, control of text rasterization, persistent game-screen APIs, and the dependency policy. A complete toolkit needs a clear D-015 justification or a new policy decision. It is not prohibited merely because it is external; no exemption is assumed either. Using separate gameplay and debug toolkits would duplicate focus, input, style and text behavior, so a shared foundation is the current preference.

## 3. Proposed ownership inside the engine

Keep the existing three crates initially. D-006 already allows a later split when justified by size; a fourth `tungsten-ui` crate brings little benefit at this exploratory stage.

| Layer | Proposed responsibility |
| --- | --- |
| `tungsten-core::ui` | GPU-free widget IDs, tree/state, style/layout values, semantic text requests, computed rectangles, focus/capture state, UI events and CPU layout/hit testing. A text-measurement interface accepts neutral metrics. No winit/wgpu types. |
| `tungsten::ui` | Public builders/re-exports, app stage coordination, OS input translation, game input routing, asset resolution, state-owned roots, platform cursor/IME/clipboard bridges later, and debug-view adapters. |
| `tungsten-render::ui` plus existing text code | Ordered UI drawing, GPU buffers, clipping, textured panels/images and glyph rasterization. The existing device-free text layout cache gains a measurement/shaping interface callable before drawing. |

Prefer a `UiTree` resource containing a compact arena with generational `WidgetId`s. Game entities/resources can hold widget handles, and normal systems update properties or react to events. UI nodes need not each be a game ECS entity. This keeps sibling order, parent links and UI lifetime explicit, with interaction updates local to UI storage. An ECS-per-widget approach remains an alternative if direct component queries prove more valuable; its performance should be measured rather than assumed.

Use one authoritative tree and explicit mutation methods that mark layout/paint dirty. A node removed from a screen invalidates its handles and releases focus, pointer capture and cached data. The umbrella maps screen roots to state lifetimes and removes them on exit; pause/resume visibility and interaction policy are explicit. Persistent debug roots are independent of game-state roots. All state is passed explicitly or stored in resources; processing stays on the main thread.

Text ownership needs particular care: **measurement and rendering must share the same font selection, shaping and wrapping results**. The lowest-change starting point is to expose the GPU-free portion of the current render text cache through a neutral measurement interface used by the app coordinator. Layout calls it before extract; core does not import glyphon. The renderer later receives resolved geometry/layout handles and never queries or mutates `World`. If that placement becomes awkward, a CPU text service can move to the umbrella in a later design decision. Do not create a second font database/cache just for measurement.

## 4. Authoring API: compare before committing

Two illustrative sketches follow. They are pseudocode, not implemented or compile-checked APIs.

**A. Build once, update through systems — current recommendation**

```rust
// State entry/startup: build a screen and keep the returned handles.
let resume_button = ui.screen(UiLayer::Game, |screen| {
    screen.column(|column| {
        column.label("Paused");
        column.button("Resume")
    })
});

// Ordinary game systems: read an activation, then request a game action.
if ui_events.activated_this_frame(resume_button) {
    state_stack.request_pop();
}
ui.set_text(hud.health_label, health_text);
```

Good fit for persistent menus, state-owned screens and cached layouts. Stable handles make focus and automation straightforward. The cost is handle ownership, explicit updates, and cleanup. Convenience setters should ignore unchanged values so game systems can stay simple.

**B. Describe UI each frame — alternative**

```rust
ui.window("pause", |ui| {
    ui.label("Paused");
    if ui.button("resume", "Resume").clicked() {
        actions.push(GameAction::Resume);
    }
});
```

Concise for changing debug views. It needs stable keys, reconciliation or repeated layout, and an explicit interaction stage; it cannot run in read-only extract and mutate gameplay there. Data-heavy views still need virtualization and bounded caches. A lightweight keyed authoring layer over the same tree is possible later, but supporting both public models immediately adds work.

Proposed first choice: builders plus handles/events, a few convenience functions, and explicit game data binding in ordinary systems. No callback capturing `&mut World`, implicit observer graph, mandatory macros, custom markup language or editor in the first slice. Theme assets/data-defined screens remain future authoring options after the runtime model is understood.

## 5. Rendering contract

Proposed ordinary frame: **Scene → PostStack → optional SMAA → UI overlay → Present**. Game UI, debug windows, text, carets and borders share the overlay. It uses screen coordinates, alpha compositing and no world-depth testing. Post effects, lighting and camera shake do not affect it by default. Local analytic/geometry anti-aliasing handles UI edges because scene SMAA precedes the overlay.

The overlay should extend the existing final pass, preserving D-087's direct presentation: load the already-rendered swapchain and composite there. Capture frames instead composite into the existing screenshot source before the present blit. Future screenshot tests must compare both paths with UI visible. An empty UI should add no new render target or full-screen pass.

Use a single **ordered paint list** containing rectangles, borders, images/nine-slices and text runs. A rear window's text must be painted before a front window's background. Drawing all panels, then all images, then all text would violate this. Hit testing walks the corresponding visual order from front to back and obeys ancestor clips.

Batch adjacent compatible commands by pipeline, texture/atlas and clip; preserve order across overlaps. Reuse buffers and grow capacity only when needed. First clipping primitive is nested rectangular scissor intersection, with empty regions skipped. Text wrapping constraints and ancestor paint clips are independent; scrolling a clipped label must not silently change its line wrapping. Rounded visual corners do not imply rounded child clipping; stencil/mask clipping is a separate later feature. Define one color/alpha convention and match it across primitive and glyph pipelines.

The current glyphon wrapper prepares and renders a whole text collection. Interleaving text with panels/images needs a supported way to draw prepared text ranges, multiple retained batches, or a deeper text integration. This is a design/prototype question: repeatedly preparing one renderer inside the paint loop would overwrite state and undermine batching. Do not promise the current wrapper is already sufficient.

Layer order is explicit: game HUD/screens, game popups/modals, then enabled developer views and their popups. Blocking follows input policy, not opacity: decorative HUD text is pointer-transparent; a modal intentionally blocks its lower layers; a debug window blocks only its own region unless explicitly modal. Define whether a game modal should still allow developer hotkeys.

Coordinates use logical UI units with top-left origin. Convert physical pointer coordinates into those units before hit testing, and convert layout/clips into physical pixels once at extraction. Track DPI changes separately from window size. OS scale and user UI scale multiply; a reference-resolution scale policy and pixel snapping can be opt-in. World-attached labels can later project anchors into a screen root; actual world-space text is a separate rendering mode and is outside the first slice.

## 6. Input, events and the frame loop

An ordered input stream is needed in addition to the current held/edge snapshot. Preserve event order so a press and release received between redraws is still one valid click. Core receives engine-owned event types; the umbrella translates winit data.

Proposed stage placement:

```text
OS events → queued raw input
redraw → apply display changes / update time
       → update committed UI geometry if resize/DPI made it stale
       → route UI input / focus / capture; publish UI events and routed game input
       → registered systems → particles → tweens
       → existing command flush and event rotation → hot reload
       → sync UI changes/assets; measure and lay out dirty roots
       → extract from &World → render → clear frame input → telemetry
```

Input normally targets the last committed layout. Resolve viewport/DPI changes before routing so clicks match displayed geometry. A newly opened screen becomes interactive at the next routing boundary; never send the same pointer event into a screen created by its own handler. Avoid unbounded layout/event feedback loops. Startup must produce initial geometry before the first interaction.

Generate semantic events such as `Activated`, `CloseRequested` and later `ValueChanged` into `EventQueue<UiEvent>` **before systems**. Existing D-040 queues expose previous and current windows; a button action read with `iter()` every frame could execute twice. The convenience reader for UI actions must read the current window or track event sequence IDs. Preserve the existing global event rotation rather than inventing a second flush.

Keep raw physical input truthful for diagnostics and intentional global controls. Provide an explicit routed gameplay view/action-query path that filters consumed events, with held-key and release ownership to prevent stuck keys. A `wants_keyboard` boolean alone does not filter gameplay. Existing direct raw-input callers need migration before capture can be guaranteed for them.

Foundation behaviors: hover/pressed/disabled/focused states; pointer capture for window dragging; release-outside cancels a button activation; tab/shift-tab navigation; Enter/Space activation; focus scopes and restoration after a dialog; removal/hiding/focus loss cancels capture. Resolve Escape as UI cancel/back before the mapped engine exit action. Keep engine toggles deliberate and remappable. Controller navigation needs a later input extension under D-045; this draft does not quietly assume a gamepad backend exists.

For future fields, physical keys are not text. Winit exposes [produced text and repeat information](https://docs.rs/winit/0.30.13/winit/event/struct.KeyEvent.html) and [IME preedit/commit events](https://docs.rs/winit/latest/winit/event/enum.Ime.html). The bridge will need those, modifier/shortcut handling, candidate-window caret positioning, clipboard and composition cancellation. Editing must respect graphemes, shaped clusters and bidirectional caret movement; byte slicing or one-glyph-per-character logic is insufficient.

## 7. Feature scope and likely omissions

These are proposed stages, not an approved implementation schedule. Both gameplay and debug views get acceptance scenarios in each shared stage.

| Stage | Proposed contents |
| --- | --- |
| Foundation / first useful slice | Roots/layers, persistent IDs, row/column/anchor layout, padding/gaps/alignment, fixed/content/fill sizing, min/max constraints; panels, labels with wrap/alignment, images, buttons; disabled/hover/focus styles; rectangular clips; pointer routing and keyboard focus; dialogs and movable debug windows; a small theme. |
| Broader controls | Scroll containers and scrollbars, checkboxes, sliders/progress bars, collapsible sections, tooltip/popups, window resizing; virtualized lists when telemetry/inspector views require them. |
| Editable fields | Single-line first: selection, caret, clipboard, undo/redo, validation/commit/cancel, IME and grapheme-aware editing. Multiline/rich editing is a separate expansion. |
| Later product choices | Controller navigation, richer text spans/icons/localization interfaces, data-defined screens/themes, docking, drag/drop inventories, world-space UI, native multi-window UI, platform accessibility integration. |

Important details to preserve in the shape of the foundation:

- Custom composition: games can combine primitives into their own widgets with shared layout, focus and paint behavior. The engine should not own demo-specific controls.
- Theme tokens for colors, spacing, fonts, borders and interaction states; asset-backed/nine-slice skins and an adjustable text/UI scale. Avoid a fixed visual identity that only suits developer tools.
- Hidden versus collapsed versus disabled semantics, stable sibling order, modality and popup ownership. State exit must clean up descendant windows/popups too.
- Text fallback, line breaking, overflow/ellipsis policy, baseline alignment, explicit font IDs and font reload invalidation. Declare supported languages from actual packaged fonts rather than claiming all Unicode is covered.
- Semantic roles/labels, visible keyboard focus and readable disabled states from the start. Platform screen-reader support can follow without replacing widget identities.
- Deterministic input replay and widget lookup for tests; UI diagnostics for measured size, clip rectangles, dirty causes, focus/capture and paint batches.
- Animated opacity/position should invalidate paint or transforms appropriately. Layout animation is more expensive; avoid recomputing text just because a label moved or changed color. Reuse engine timing/tween concepts where their ownership fits.

## 8. MSDF evaluation, independent of widget architecture

MSDF stores distance information in three channels to preserve sharp corners when glyphs scale. [MSDFgen](https://github.com/Chlumsky/msdfgen#using-a-multi-channel-distance-field) describes median reconstruction and linear sampling. [MSDF atlas generator](https://github.com/Chlumsky/msdf-atlas-gen#atlas-types) documents MSDF/MTSDF, metadata and glyph-index export. MTSDF adds a true distance channel useful for some soft effects.

Proposed pipeline:

```text
Text + font + constraints
  → shared shaping/layout (cosmic-text)
  → positioned glyphs, metrics and clusters
  → glyphon raster path OR experimental MSDF path
  → the same ordered UI compositor
```

Choose using evidence. Scalable titles, animated sizes and outlined labels are promising MSDF cases; tiny text, pixel fonts, complex outlines and color emoji need quality tests and a fallback policy. There is no performance result for Tungsten yet. Atlas memory, generation time, upload costs and cold-frame latency may outweigh fewer scale-specific raster entries. Outlines/shadows could reduce today's repeated text sections, but that gain needs measurement too. Compare both paths at small HUD and large title sizes, fractional DPI/scale, rapid size changes, outlines, Latin ligatures, combining marks, RTL text, CJK and color emoji using fonts that actually cover each sample.

An experiment must settle:

1. Atlas acquisition: runtime generation/prewarming versus imported precomputed assets. The repository currently says **no asset preprocessing**; adopting an offline generator would require an explicit policy decision. Runtime generation must respect the two-background-thread limit and avoid expensive work in an interaction frame. No generator/tool/dependency is selected or installed here.
2. Coverage: map the shaper's **actual font face and glyph ID**, including ligatures/fallback, into the atlas. A Unicode-codepoint-only ASCII atlas cannot satisfy arbitrary shaped output. Pin source font identity/version and variable-font settings if applicable; retain fallback glyph rendering for misses.
3. Sampling: linear distance textures rather than the existing sprite sRGB upload assumptions, correct distance range/padding, coordinate/baseline conversion, scale-aware edge reconstruction and an explicit mip/minification policy. Implement any experiment in WGSL.
4. Lifecycle: bounded atlas pages/cache bytes, no eviction of in-flight glyph references, deterministic failure/fallback when full, and font reload invalidating shaped layouts and matching atlas content together.
5. Integration: current glyphon and experimental MSDF runs obey identical clips, paint ordering and alpha conventions. Preserve text measurement, caret and fallback behavior across both paths.

Provisional default: keep the current renderer while evaluating MSDF as an additional backend. Choose a default, per-font option or limited specialty path after comparing quality, frame tails, memory and complexity. Avoid a new broad renderer abstraction before the second path establishes a concrete need.

## 9. Performance goals and evaluation

Design for bounded incremental CPU work, while recognizing that visible UI still draws into every presented game frame. Layout/shape/paint caching saves preparation; it does not eliminate GPU compositing. Start with dirty-root updates and profile before building fine-grained dependency machinery.

| Invariant / scenario | Evidence wanted after implementation |
| --- | --- |
| UI disabled or no roots | No extra UI GPU pass/target; near-zero routing/update cost; existing suite owned metrics remain within its established comparison policy. |
| Static menu/HUD | No reshaping or relayout after warm-up until an actual dependency changes; geometry/GPU resources reused; per-frame allocation count examined. |
| Hover/press/drag/color change | Touch the affected interaction/paint state; unchanged text layout stays cached. |
| Changing diagnostic values | Update only changed labels at the existing refresh rate; caches plateau under continuous unique text. |
| Large inspector/log/list | Measure visible content and input latency; eventual virtualization bounds work by visible rows. |
| Font/theme/DPI changes | Correct full invalidation when needed, with resize and cold-glyph p95/p99 spikes reported separately. |

Discuss initial targets of **≤0.25 ms additional CPU p95** for a warmed 100-widget menu and **≤0.5 ms** for a warmed 500-widget debug view on the reference machine at 1080p. These are proposed budgets, not measured outcomes or finalized acceptance limits. GPU budgets should account for filled screen area/transparency and be chosen after a simple prototype. Define widget counts precisely: include internal label/background nodes, not just top-level controls.

Track input, layout, shaping, paint/extract, GPU preparation/draw separately, plus draw batches, glyph misses/uploads, atlas/cache bytes and worst cold frames. Debug UI reads existing telemetry and publishes its own costs without creating a parallel timing system.

Future captures follow the canonical [profiling workflow](../perf/profiling-workflow.md#comparison-rule-and-capture-rules): matching workloads/builds/machine, quiet A/A, five repeats, and separate blocking GPU diagnostics. A new UI workload should live in a dedicated fixture/harness after design finalization; existing examples and benchmark workloads stay untouched beforehand. If a later benchmark migration changes work, bump its workload version and start a fresh baseline; do not present it as an engine-only speedup.

## 10. Migration outline — after finalization

Keep current consumers functioning while the foundation is built and checked in an isolated UI fixture. A temporary adapter for existing `TextSection` output may ease transition, but define its layer/units explicitly and set an eventual removal milestone. Migration is a later task; this session changes none of these consumers.

| Consumer | Later migration intent |
| --- | --- |
| Engine `DebugHud` | Present existing providers as labels/rows; preserve custom row APIs, toggles, defaults, EWMA and refresh intervals. Replace width heuristics with measured layout. |
| Engine system timing overlay | Shared window/table or row layout, preserving its independent toggle and smoothing. Scrolling/virtualization follows real row counts. |
| Engine inspector | Shared read-only window first; preserve `Inspectable`, selected entity identity and picking. UI-consumed pointer input must not pick world entities behind a window. Editing is a separate feature. |
| Physics debug | Move its controls/status into UI. Keep collider/line geometry in `DebugDraw` and the scene pass. |
| Example 01 | Instruction/status text and outline behavior; preserve game inputs and update frequency. |
| Example 02 | GPU text stress and integrated HUD/projected tags; distinguish benchmark payload from interface chrome. Preserve deterministic workload semantics or deliberately version changes. |
| Example 03 | Menu/gameplay/pause screens; bind buttons and root lifetime to existing state actions. Preserve keyboard paths and animation intent. |
| Example 04 | Shader/post-AA/bloom readouts and controls, preserving fixture-lock behavior and post-independent text. |

Suggested future sequence: finalize the architecture/API/acceptance criteria; build and validate an isolated text/button/panel fixture with both a gameplay screen and debug window; evaluate layout dependencies and MSDF in bounded experiments; validate engine integrations; migrate engine views; then migrate examples explicitly. No example rewrites before the finalized foundation passes its checks.

Future meaningful tests include CPU layout/measurement/invalidation; event order and single activation per press/release; focus/modal/capture cleanup; state-root lifecycle; multilingual wrapping/fallback; DPI/scissor rounding; overlapping mixed primitives/text; direct/capture equality; cache saturation and continuous text churn. Renderer changes require the renderer's shader/layout, smoke and reference visual checks; substantial implementation finishes with `just check`. These are future checks, not tests added by this draft.

## 11. Choices for the next discussion

| Choice still open | Provisional recommendation |
| --- | --- |
| Public authoring model | Start with builders/handles/events; compare the two sketches using one pause menu and one live inspector before deciding. |
| Layout solver | Evaluate Taffy behind Tungsten-owned style types; keep the initial exposed layout subset small. |
| Visual direction | Small theme/token system with custom composition; clarify pixel-art skins versus smooth panels and when nine-slice skins are needed. |
| Initial scrolling | Add it to the first slice if migrated debug data regularly exceeds the viewport; otherwise stage it next. |
| Root lifecycle/storage | A generational arena in a `World` resource, with explicit state-owned roots and persistent debug roots. |
| Input API compatibility | Add an explicit routed gameplay path and migrate consumers deliberately; agree whether a future breaking API release makes routed input the default. |
| MSDF atlas policy | Keep acquisition and coverage open; resolve the preprocessing/thread constraints before selecting a generator. |
| Performance acceptance | Agree workloads, widget counts and reference-machine CPU/GPU/cold-start/memory budgets before implementation. |
| Text/languages and future input | Declare packaged-font coverage; decide whether IME-complete single-line editing is the first editing milestone. |

The next useful artifact is a tightened spec for the first text/button/panel/window slice, with one chosen API and an explicit input contract. Leave broader widget lists and MSDF selection open until those smaller decisions are clear.
