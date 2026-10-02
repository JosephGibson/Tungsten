# UI, interface and text suite — exploratory draft

- **status:** draft
- **goal:** Explore an ergonomic, efficient UI foundation for game interfaces and engine debug views, including a measured comparison of current text rendering and MSDF, and the text-engine, DPI and input changes that foundation needs.
- **non-goals:** Implementation, dependency changes, settled design decisions, release work, or changes to existing examples. Editable fields, docking and native multi-window UI are outside the first slice.
- **files to touch:** This draft only. Future implementation locations below are proposals, not a change list for this session.
- **ordered steps:** Review the relevant engine seams; research primary sources; collect owner preferences; draft alternatives and a recommendation; identify open questions and future acceptance checks.
- **done-when:** The draft records confirmed preferences, source-backed research, code-verified constraints, proposed ownership/render/input contracts, performance evaluation, a milestone ladder, a migration outline and the adjacent systems the foundation must not preclude, each placed on the ladder, with unresolved choices clearly identified and a recommendation beside each.

Date: 2026-10-02, revised the same day after checking the text, input and window paths against the code and the locked crate sources (§2). A review pass the same day verified glyphon's atlas growth and AccessKit's Linux thread against their sources, corrected the measurement contract (§4), split the first milestone (§11) and added the systems the first pass had not covered (§14). All API names, feature stages and numerical targets below are provisional. This is a discussion document; implementation planning follows design finalization.

## Context and confirmed preferences

Tungsten has a hand-written ECS, a synchronous frame loop, manifest assets, and cached screen-space text, but no general widget/layout/focus system. Preserve its three-crate architecture and extract/draw seam. Make game UI and debug tools equally important consumers of one foundation. Start with display text and buttons; editable fields follow. Windows initially mean in-game panels/dialogs and movable debug windows; docking can wait. Evaluate MSDF alongside the current renderer using quality and performance evidence. The authoring API is still undecided. Existing examples remain unchanged throughout exploration and implementation until the design and foundation are finalized.

## Proposal at a glance

Provisional. Each line is argued in the section named and none is a decision.

- **Model:** one retained `UiTree` with generational `WidgetId`s in `tungsten-core`, GPU-free. Builders, handles and `UiEvent`s first; a keyed immediate facade for debug windows only if the pause-menu and inspector comparison asks for it (§5).
- **Layout:** Tungsten-owned style types over Taffy's low-level traits implemented on `UiTree`, so there is no second tree to keep in sync (§3).
- **Text:** split `render/text.rs` into a device-free text engine (fonts, shaping, measurement, caret geometry) and a GPU half, so layout measures and the renderer draws from the same retained buffers (§4). This is the first milestone and changes no pixels (§11).
- **Paint:** one ordered paint list in the final overlay, batched by pipeline, texture and clip; text interleaves with panels through several glyph batches. Whether a batch can re-prepare alone depends on when the atlas is trimmed, not on glyphon itself (T1b, §6).
- **Input:** an ordered raw event stream routed before systems, UI navigation on engine-owned `ui_*` actions, and a filtered gameplay view (§7).
- **Glyphs:** glyphon stays the default while MSDF and an owned glyph renderer are scored on capability as well as cost; they are one experiment, not two (§9).
- **Facts that shape the rest:** any changed text section re-prepares every section; fallback fonts come from the host machine; the engine has no DPI, modifier, focus or IME events; tweens reach only entities; `display.scale_mode` and `logging.level` are parsed but never applied (§2).
- **Not covered by the first pass:** a style model, UI animation, directional focus, binding-aware prompts, localization data, debug consoles, world-anchored text and a headless test harness. §14 proposes each and places it on the ladder.

## 1. What the project already provides

This is a focused architectural review, not a full project correctness audit. Example source was inspected read-only to identify migration consumers. The findings in §2 were checked against the code and the locked crate sources on 2026-10-02.

| Existing surface | Implication for UI |
| --- | --- |
| [`tungsten-core`](../../crates/tungsten-core/src/lib.rs): `World`, resources, `CommandBuffer`, `EventQueue<T>`, `ActionMap`, opaque asset handles | UI state and interactions can participate in ordinary game systems without GPU dependencies. |
| [`App`](../../crates/tungsten/src/app.rs): serial systems, command flush, event rotation, reload, extract, render | Input routing needs an explicit position before game systems. Layout/extraction should see their final mutations. |
| [`TextPipeline`](../../crates/tungsten-render/src/text.rs): glyphon/cosmic-text, advanced shaping, registered fonts, wrap/clip bounds, bounded layout cache | Reuse shaping and caching. Expose measurement before UI layout; currently shaping is private to the rendering pipeline. |
| [`pass order`](../../crates/tungsten-render/src/passes/order.rs): scene → post → optional SMAA → text overlay | Expand the final overlay to include all screen UI primitives. Keep game UI out of scene post effects by default. |
| [`input bridge`](../../crates/tungsten/src/input_bridge.rs) and [`InputState`](../../crates/tungsten-core/src/input.rs) | Current bridge handles physical keys, pointer and scroll state. It does not forward text, IME or an ordered UI input stream; gamepad support is absent. |
| [`DebugHud`](../../crates/tungsten/src/debug_hud.rs), [`SystemTimingOverlay`](../../crates/tungsten/src/systems_overlay.rs), [`InspectorState`](../../crates/tungsten/src/inspector.rs) | Existing resource/provider models should survive migration. Their text refresh is throttled; outlines repeat text draws, and anchoring uses a monospace-width heuristic. |
| [`StateStack`](../../crates/tungsten/src/state.rs) and [`DebugDraw`](../../crates/tungsten-core/src/debug_draw.rs) | UI roots need state cleanup and pause/resume behavior. Physics lines and collider geometry remain world-space diagnostics. |
| `App::window_event` in [`app.rs`](../../crates/tungsten/src/app.rs) | Handles close, resize, key, mouse button, cursor, wheel and redraw only. No scale-factor, modifier, focus, cursor-leave, IME or touch events. |
| `gpu` and `integrated` rows in the [benchmarks](../perf/benchmarks.md) | The current text path already has workloads: 100 sections of 40 characters with a `text_change` knob, and HUD lines plus name tags. They are the baseline for any text comparison. |
| [Known issues](../known-issues.md) | Screen-space text draws after transitions (`D-093`), and capture completion is not reliable (P3). Both reach UI. |

Locked dependencies at review: wgpu 30.0.1, glyphon 0.12.0, cosmic-text 0.19.0, winit 0.30.13. `Cargo.toml` permits winit starting at 0.30.12; the lockfile establishes the version reviewed. Candidates checked on crates.io, not locked: taffy 0.14.0, accesskit_winit 0.34.1.

Relevant decisions: D-006 (three crates), D-015 (dependencies), D-016/D-018 (handles/extract), D-026 (text), D-039/D-040 (commands/events), D-044/D-047 (independent debug tools), D-045 (input), D-078 (benchmarks), D-085 (text caching), D-087 (direct/capture presentation), D-093 (transitions). See the [decision index](../DECISION_INDEX.md); this draft adds no decisions.

## 2. Constraints in today's text, window and input paths

Verified on 2026-10-02 against [`text.rs`](../../crates/tungsten-render/src/text.rs), [`app.rs`](../../crates/tungsten/src/app.rs), [`input_bridge.rs`](../../crates/tungsten/src/input_bridge.rs) and the glyphon 0.12.0 and cosmic-text 0.19.0 sources. These are facts about the code, not preferences; the plan below is shaped around them.

**Text path**

| Finding | Consequence |
| --- | --- |
| One prepared frame, one draw. `TextLayoutCache::update` reports "unchanged" only when every section matches; any change re-prepares all sections through a single glyphon `TextRenderer`, which owns one vertex buffer and draws it in one call. | A HUD of 99 static labels and one live counter still rebuilds every glyph vertex each frame. Shaping is cached; the glyph walk is not. "Update only changed labels" is true of layout, not of glyph preparation. Interleaving text with panels needs more than this wrapper (§6). |
| The layout cache key is content, font, size, line height and buffer width and height bits. | Every width a layout pass probes (min-content, max-content, final) becomes its own shaped buffer. cosmic-text caches shaping and line layout separately (`shape_opt`, `layout_opt`), and `Buffer::set_size` only marks a relayout, so UI text should keep one retained `Buffer` per text node and resize it. Keep `D-085`'s bounded age and spare-buffer reuse for content-keyed HUD strings. |
| `FontSystem::new()` loads system fonts and the platform fallback list; the unit tests build an empty database instead. | Fallback glyphs (CJK, emoji, symbols) come from whatever the host has, so layouts, visual tests and replays can differ between machines, and startup pays a font scan that cosmic-text documents as up to about a second in release. A UI that declares language coverage needs a packaged-fonts-only mode (`new_with_locale_and_db_and_fallback` with an explicit database) or a deliberate opt-in to system fallback. |
| A manifest font ID registers the first face of one file (`load_font`) and draws with that face's family, weight and style; bold is a separate ID (`sans_bold`). Only three static faces are registered: the variable Inter, JetBrains Mono and Source Serif 4 files in `assets/fonts/` are not. | A span that switches to bold must resolve through fontdb's family matching (family plus weight and style), not through a face ID, so the text style names a family and the manifest needs a family view of its faces (§14). Two manifests that register different builds of one family make that match ambiguous. |
| `TextSection` carries font, size, line height, colour, position and bounds. The calls use `Shaping::Advanced`, `set_text(.., None)` (no alignment), `TextArea.scale = 1.0`, the default wrap and no ellipsis or spans. | cosmic-text 0.19 already offers `Align` (including `Justified` and `End`), `Wrap::{None, Glyph, Word, WordOrGlyph}`, `Ellipsize` (start, middle or end, limited by lines or height), `Hinting`, `set_rich_text`, `Buffer::hit` and `layout_runs` (caret and selection geometry). Each span's `Attrs` also carries letter spacing, OpenType features (tabular figures keep a live counter's width fixed), its own metrics, a `metadata` tag and a `TextDecoration`. None is reachable from Tungsten. They belong in the text-engine API from the start, and most join the cache key. Layout computes decoration spans (`DecorationSpan`), but glyphon 0.12 has no decoration code, so underline and strikethrough are paint-list rectangles. |
| A section without bounds wraps at the viewport width (`buffer_size`). | "Unbounded" in a UI label must mean no wrap, stated explicitly. Measurement always receives an explicit maximum width. |
| `Hinting::Enabled` snaps glyph advances during layout and, per its documentation, only looks right with physical-pixel layout and no later scaling. `Disabled` (today's behavior) uses subpixel positions. | The DPI policy is a real fork: (a) layout in logical units with a draw-time scale, so cached layouts survive a scale change but small text is slightly softer, or (b) layout in physical pixels with hinting, so HUD text is crisper but a scale change relayouts everything. Prototype both before fixing §6's scale rule (the claim in (a) that glyphon's `TextArea.scale` rasterizes at physical size is unproven here). |
| The atlas is safe today only because the frame signature covers every section. `update` skips `prepare` when nothing changed, so glyphon's vertices persist and their atlas slots must stay put (`prepare` skips on the premise that nothing else writes the atlas). When the packer is full, glyphon evicts any least-recently-used glyph that is not in `glyphs_in_use`, and `trim` clears that set after each prepared frame. | Under today's `trim` placement, retaining text batches across frames while another batch prepares is unsafe: a new glyph in batch B can evict and overwrite a slot that batch A's retained vertices still sample, so several batches on one atlas (T1, §6) must re-prepare together whenever any of them changes. Growth is not the hazard. `grow` keeps every allocation in place, re-uploads the cached glyphs into the larger texture and rebuilds the bind group, and vertices store texel UVs that the shader divides by `textureDimensions` at draw time (glyphon 0.12 `text_atlas.rs`, `shader.wgsl`), so retained vertices survive it. Growth does re-rasterize every cached glyph, a CPU spike that scales with the cache, and the atlas starts at 256² and never shrinks. Eviction is the hazard, and when `trim` runs decides it (T1b, §6). |

**Window, input and surrounding behavior**

| Finding | Consequence |
| --- | --- |
| The engine has no DPI model: the window is requested in `PhysicalSize`, `Resized` is the only display event, `ScaleFactorChanged` is unhandled, the cursor is stored in physical pixels and text positions are physical. | On a 2× display, text sized in pixels appears half as large as on a 1× display. A logical-unit UI needs the scale factor in a core resource first; that small change also benefits existing text. |
| The bridge forwards `KeyCode`, mouse buttons, cursor and wheel. It drops modifiers, focus, cursor-leave, IME, produced text, the repeat flag and the logical key. `translate_key` maps a fixed whitelist, and anything else becomes `KeyCode::Other(<winit enum discriminant>)`, including Home, End, Delete, PageUp, PageDown and every modifier. | Shift+Tab, Ctrl+C/V/A, caret movement and text entry cannot be built on it. A winit discriminant is not a stable identifier across winit versions, so UI-visible keys need real variants. Nothing handles window focus loss, so keys held across an alt-tab are never released; capture and cancel rules must cover it. |
| `StateStack::transition_cover()` exists, but a transition draws before the overlay, so it does not cover screen-space text; example 03 fades its own text (`known-issues.md`, `D-093`). | A UI root opacity multiplier that a state can drive from the cover value belongs in the foundation. Otherwise every game screen reimplements it. |
| Capture success is unreliable: a skipped acquisition can report success and readback errors only warn (P3). | UI screenshot checks that compare direct and capture paths (§6) need an explicit capture-completion contract, or a compensating check, first. |
| The current text path has baselines and history. `D-087`'s capture reads the `text` pass at p50 0.06 ms; `D-085` replaced a cache that produced a 41–43 ms frame every 120 frames and a 731 MiB peak RSS. | Comparisons start from the `gpu` and `integrated` rows, not from nothing. Any UI text cache keeps `D-085`'s bounded behavior. |
| `EventQueue::iter()` yields the previous window and then the current one; `iter_current()` yields the current window only (`D-040`). | UI events published before systems and read with `iter_current()` are seen exactly once; read with `iter()`, they are seen in two frames (§7). |
| `TweenChannel` writes the owning entity's `Transform`, `Sprite` colour and material uniforms (`tungsten/tweens.rs::apply_channels`). The ECS has no change detection. | UI nodes that are not entities cannot be tween targets, so UI animation needs tracks of its own (§14). Binding game data to labels is compare-on-write: a setter that ignores unchanged values is the change detection. |
| `display.scale_mode` (`Stretch` or `Integer`) and `logging.level` are parsed but have no runtime application (`DESIGN.md`); every example calls `env_logger::init()` itself. | The UI scale policy (§6) and `scale_mode` need one definition covering scene and UI, not two unrelated settings. An in-game log console (§14) needs an engine-owned logger, which would also apply `logging.level`. |
| Nothing controls the cursor: no code sets a cursor icon, visibility or grab, or calls `set_ime_allowed`. | Hover cursors (pointer, text beam, resize edges), game cursors and IME enablement are new bridge work (§7, §14). |

## 3. Research: patterns worth borrowing

The observations below come from primary documentation. The Tungsten recommendations are our interpretation, not claims that another engine's design or performance transfers automatically.

| Reference | Observed pattern | Proposed lesson for Tungsten |
| --- | --- | --- |
| [Bevy UI](https://docs.rs/bevy/latest/bevy/ui/) and [text nodes](https://docs.rs/bevy/latest/bevy/ui/widget/struct.Text.html) | UI uses entities/components, layout nodes and text components; layout owns UI transforms. | Expose UI state to systems, but separate UI layout from world transforms/cameras. Borrow the pattern, not Bevy's ECS dependency. |
| [Godot Control](https://docs.godotengine.org/en/4.6/classes/class_control.html) and [focus navigation](https://docs.godotengine.org/en/stable/tutorials/ui/gui_navigation.html) | Controls provide pointer filtering and explicit focus neighbors; mapped actions drive navigation. | Treat input consumption, initial focus, focus restoration and navigation as foundational contracts. |
| [Unity UI Toolkit event handling](https://docs.unity3d.com/6000.3/Documentation/Manual/UIE-Events-Handling.html) | Hierarchical events have a target and propagation through ancestors. | Distinguish internal widget event routing from application actions; a modal consumes input before gameplay receives it. |
| [egui](https://github.com/emilk/egui#why-immediate-mode) | Per-frame functions return button responses; stable identities preserve interaction/window state. Its documentation discusses layout and large scrolling-content tradeoffs. | Study its concise API and debug widgets. Immediate authoring still needs stable identity and retained internal state. Benchmark either model rather than declaring one universally faster. |
| [Taffy 0.14 layout APIs](https://docs.rs/taffy/0.14.0/taffy/) | A layout solver with text/image measurement hooks. Its low-level API (`TraversePartialTree`, `LayoutPartialTree`, `CacheTree`, `compute_root_layout`, flexbox/block/grid computes, all present in the 0.14.0 source) runs over a caller-owned tree. Features are gated individually; the default set also enables `grid`, `float_layout` and `calc`. | Strong candidate for layout only. Tungsten still owns widgets, input, themes and painting. Implement the low-level traits on `UiTree` instead of mirroring into `TaffyTree`: no second tree and no sync pass, and `CacheTree` is where dirty-root relayout plugs in. Start with `flexbox` and `block_layout`; add `grid` only when a screen needs it. |
| [cosmic-text shaping](https://docs.rs/cosmic-text/0.19.0/cosmic_text/enum.Shaping.html), [glyph output](https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.LayoutGlyph.html), [editing](https://docs.rs/cosmic-text/0.19.0/cosmic_text/trait.Edit.html) | Advanced shaping supplies font fallback and glyph/cluster information; editing has separate APIs. | Use shaped metrics for measurement and drawing. Future editing needs clusters, selection and caret geometry, independent of rasterization. |
| [AccessKit](https://accesskit.dev/) | A platform accessibility bridge for custom-rendered toolkits, including a winit adapter. On Linux, `accesskit_unix` 0.24.0 spawns a thread that runs its executor (`async-io` by default, or a current-thread `tokio` runtime), keeps its connection in `static OnceLock`s and calls the action handlers from that thread (`context.rs`, `adapter.rs`). | Keep semantic roles, labels, values and stable widget IDs in the design; evaluate platform integration separately. The Linux adapter collides with the no-async-runtime and two-thread rules: it is a third background thread, and its handler calls must cross to the main thread through a channel, as the watcher's events do. Integration is either Windows/macOS first or a deliberate rule amendment. |

### Build versus adopt

Recommend a Tungsten-owned UI model and renderer integration, with focused existing primitives reused underneath. Taffy is the first layout dependency to evaluate under D-015 rule 3 (solved layout primitive); it is not approved or added here. A deliberately narrow hand-written stack/anchor layout is the alternative if we decide Flexbox is unnecessary. Avoid accidentally committing to a home-grown full CSS implementation.

The project's precedent leans toward owning what is in scope: the ECS (`D-005`) and the mixer (`D-029`) are hand-rolled although crates existed. That puts the burden on Taffy to show that the layout the first screens need is larger than what we would write. The M1 spike measures it: Taffy against a stack/anchor layout on the same fixture, by lines of code, features actually used and compile time.

Using egui directly is a serious alternative: it would supply many debug controls and editing features sooner. Tradeoffs to evaluate are asset/theme integration, control of text rasterization, persistent game-screen APIs, and the dependency policy. A complete toolkit needs a clear D-015 justification or a new policy decision. It is not prohibited merely because it is external; no exemption is assumed either. Using separate gameplay and debug toolkits would duplicate focus, input, style and text behavior, so a shared foundation is the current preference.

Dependency exposure, by `D-015` rule (1 platform API, 2 data format, 3 solved primitive):

| Candidate | Rule | Conflict to resolve before adoption |
| --- | --- | --- |
| Taffy | 3 | None seen: CPU only, no threads. Confirm the feature set and compile-time cost in a spike. |
| AccessKit and its winit adapter | 1 | Linux adapter spawns its own executor thread (above). |
| A clipboard crate | 1 | Check whether its Linux backend keeps a helper thread. |
| An MSDF generator (msdfgen / msdf-atlas-gen, or a Rust port) | 3 | The repository's "no asset preprocessing" rule for offline generation; the two-thread limit for runtime generation. |
| egui as the toolkit | none cleanly | A whole toolkit needs its own decision. |
| A localization format (Fluent, `fluent-bundle`) | 2 | Only if shipped locales need plural or gender rules beyond a hand-rolled `{name}` substitution (§14). |

Every new crate also passes `just deps`. The `RUSTSEC-2026-0192` exception for the cosmic-text/fontdb stack's `ttf-parser` is tracked in `known-issues.md` and stays in view while the font stack grows.

## 4. Proposed ownership inside the engine

Keep the existing three crates initially. D-006 already allows a later split when justified by size; a fourth `tungsten-ui` crate brings little benefit at this exploratory stage.

| Layer | Proposed responsibility |
| --- | --- |
| `tungsten-core::ui` | GPU-free widget IDs, tree/state, style/layout values, semantic text requests, computed rectangles, focus/capture state, UI events and CPU layout/hit testing. A text-measurement interface accepts neutral metrics. No winit/wgpu types. |
| `tungsten::ui` | Public builders/re-exports, app stage coordination, OS input translation, game input routing, asset resolution, state-owned roots, platform cursor/IME/clipboard bridges later, and debug-view adapters. |
| `tungsten-render::ui` plus existing text code | Ordered UI drawing, GPU buffers, clipping, textured panels/images and glyph rasterization. The existing device-free text layout cache gains a measurement/shaping interface callable before drawing. |

Prefer a `UiTree` resource containing a compact arena with generational `WidgetId`s. Game entities/resources can hold widget handles, and normal systems update properties or react to events. UI nodes need not each be a game ECS entity. This keeps sibling order, parent links and UI lifetime explicit, with interaction updates local to UI storage. An ECS-per-widget approach remains an alternative if direct component queries prove more valuable; its performance should be measured rather than assumed.

Use one authoritative tree and explicit mutation methods that mark layout/paint dirty. A node removed from a screen invalidates its handles and releases focus, pointer capture and cached data. The umbrella maps screen roots to state lifetimes and removes them on exit; pause/resume visibility and interaction policy are explicit. Persistent debug roots are independent of game-state roots. All state is passed explicitly or stored in resources; processing stays on the main thread.

Text ownership needs particular care: **measurement and rendering must share the same font selection, shaping and wrapping results**. The lowest-change starting point is to expose the GPU-free portion of the current render text cache through a neutral measurement interface used by the app coordinator. Layout calls it before extract; core does not import glyphon. The renderer later receives resolved geometry/layout handles and never queries or mutates `World`. If that placement becomes awkward, a CPU text service can move to the umbrella in a later design decision. Do not create a second font database/cache just for measurement.

**Text engine seam (pseudocode, not compile-checked).** The device-free half of `render/text.rs` becomes its own module, with the glyphon atlas and draw code in a GPU module beside it. The umbrella owns the instance and lends it to layout.

```rust
// tungsten-core::ui: neutral types, no glyphon, no wgpu
pub struct StyledText { spans: Vec<TextSpan> }          // one span by default
pub struct TextStyle {
    family: FontFamilyId, weight: u16, italic: bool,      // matched within the family (§2)
    size: f32, line_height: f32, color: Rgba8, letter_spacing: f32,
    features: FontFeatureSet,                             // e.g. tabular figures
    align: TextAlign, wrap: TextWrap, overflow: TextOverflow,
}
pub struct TextMetrics { size: Vec2, first_baseline: f32, line_count: u32 }
pub enum MeasureWidth { MinContent, MaxContent, Definite(f32) } // Taffy's AvailableSpace
pub trait TextMeasure {
    /// `known_width` is a width layout has already fixed; it wins over `available`.
    /// Min- and max-content are intrinsic to the text, style and font epoch, so
    /// they are computed once per change and cached on the node. A definite width
    /// relayouts the node's retained buffer, reshapes only when the text, style or
    /// font epoch changed, and may leave the buffer at a width other than the final one.
    fn measure(&mut self, node: TextNodeId, known_width: Option<f32>,
               available: MeasureWidth) -> TextMetrics;
}

// render text engine (device-free); implements TextMeasure
fn set_text(&mut self, node: TextNodeId, text: &StyledText, style: &TextStyle);
fn commit_layout(&mut self, node: TextNodeId, size: Vec2); // final width; draw reads only committed layouts
fn glyphs(&self, node: TextNodeId) -> impl Iterator<Item = PositionedGlyph>; // face, glyph id, pos, size, colour
fn hit(&self, node: TextNodeId, point: Vec2) -> Option<TextCursor>;          // carets and selection later
fn font_epoch(&self) -> FontEpoch;                                           // bumps on font load or reload
```

Taffy's leaf measure function receives known dimensions and an `AvailableSpace` per axis (`Definite`, `MinContent` or `MaxContent`; `compute_leaf_layout` in 0.14.0), so an `Option<f32>` maximum width cannot express min-content. A layout at width 0 is not a general substitute: under `Wrap::Word` it yields the widest word, but under `WordOrGlyph` it breaks every word into glyphs, which is CSS's `overflow-wrap: anywhere` rather than the break-word behaviour that mode exists for. Min-content is the widest word for `Word` and `WordOrGlyph`, the widest glyph for `Glyph` and the unwrapped width for `None`; max-content is the unwrapped width in every mode. Both are cached per node, so only definite widths relayout. An empty label measures one line height so that it keeps its row; a style flag can opt out.

Core defines the trait and the umbrella injects the engine, so core never calls render (`D-007`). `FontEpoch` replaces today's blanket `clear_layouts()`: retained UI buffers compare epochs and reshape lazily, and nodes whose measured size changed mark layout dirty. Content-keyed HUD strings keep using the bounded cache through the same engine.

Measure and draw agree by construction under T1: glyphon draws from the engine's own retained `Buffer`s (today's `text_areas` hands it `&Buffer`), so there is one shaping result, not two. Two rules keep it true. (1) Probes leave a buffer at the last probed width, and Taffy's final pass need not measure last, so layout ends with `commit_layout` per node, which re-applies the committed width; extraction reads only committed layouts and a debug assertion checks the buffer's size against the committed box. (2) Font load and reload happen only at the asset-sync stage before layout (§7). `commit_layout` records the `FontEpoch` and the paint list carries it; a mismatch at draw is a bug that asserts in debug and skips the run with a log in release, and the next frame relayouts. Under T2 the commit snapshots a POD `PositionedGlyph` list, so both rules hold trivially.

Placement follows the glyph path. Under T1 the glyphon renderer needs the engine's `Buffer`s, so engine and GPU half stay together in `tungsten-render`. The umbrella already depends on render and reaches it through `Renderer` the way `load_font` does today, so layout calling the engine adds no crate edge. The frame is serial, so the borrows never overlap: layout takes `&mut` engine after systems and before extract, extract reads `&World` only, and draw takes `&` engine. Under T2 the GPU half consumes `PositionedGlyph` data, so the engine can move to the umbrella, or to a fourth crate if `D-006`'s size test is met, with no change to core. Layout unit tests never touch the renderer: core tests use the fixed-advance `TextMeasure` double, and engine tests are GPU-free (the existing text tests already build an empty font database). `PositionedGlyph` is the rasterizer-independent interface that §9 compares paths through. `TextSection` stays the extract type for existing consumers (`D-018`); the UI paint list is a new POD type through the same seam.

## 5. Authoring API: compare before committing

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

Proposed first choice: builders plus handles/events, a few convenience functions, and explicit game data binding in ordinary systems. No callback capturing `&mut World`, implicit observer graph, mandatory macros, custom markup language or editor in the first slice. Theme assets/data-defined screens remain future authoring options after the runtime model is understood. Built-in behaviours are a closed enum, following `D-054`'s easings; a game builds its own widget from nodes and reads low-level `UiEvent`s in its systems (§14).

Whichever model wins, label content is a `StyledText` from the first slice (one span by default), so rich spans, inline icons and localization keys later do not change any signature.

How to decide, using one pause menu and one live inspector written both ways:

- Lines of game code per screen, and how much cleanup code the retained version needs.
- Allocations and tree mutations per frame while the inspector's values change.
- Whether focus, scroll position and hover survive a rebuild without caller effort.
- Whether the screen can be exercised headlessly by widget ID and replayed input.

A plausible outcome is a hybrid: the retained tree is the only storage, and a keyed rebuild facade (sketch B) serves debug windows whose contents change every frame. That keeps one focus, input and paint path. Decide after the comparison, not before.

## 6. Rendering contract

Proposed ordinary frame: **Scene → PostStack → optional SMAA → UI overlay → Present**. Game UI, debug windows, text, carets and borders share the overlay. It uses screen coordinates, alpha compositing and no world-depth testing. Post effects, lighting and camera shake do not affect it by default. Local analytic/geometry anti-aliasing handles UI edges because scene SMAA precedes the overlay. One instanced SDF primitive pipeline (rectangle, corner radius, border width, fill and border colours, clip index) can draw panels, buttons, focus rings and scrollbar thumbs with shader-side edge anti-aliasing. The same distance function gives a soft drop shadow (an analytic blurred rounded rectangle) and two-stop gradients for a few more instance fields; reserve them in the instance layout rather than adding pipelines later. A nine-slice cut from an atlas page (`D-048`) cannot tile its centre with a repeat sampler, so it stretches or emits one quad per repeat.

The overlay should extend the existing final pass, preserving D-087's direct presentation: load the already-rendered swapchain and composite there. Capture frames instead composite into the existing screenshot source before the present blit. Future screenshot tests must compare both paths with UI visible. An empty UI should add no new render target or full-screen pass.

Use a single **ordered paint list** containing rectangles, borders, images/nine-slices and text runs. A rear window's text must be painted before a front window's background. Drawing all panels, then all images, then all text would violate this. Hit testing walks the corresponding visual order from front to back and obeys ancestor clips.

Batch adjacent compatible commands by pipeline, texture/atlas and clip; preserve order across overlaps. Reuse buffers and grow capacity only when needed. First clipping primitive is nested rectangular scissor intersection, with empty regions skipped. Text wrapping constraints and ancestor paint clips are independent; scrolling a clipped label must not silently change its line wrapping. Rounded visual corners do not imply rounded child clipping; stencil/mask clipping is a separate later feature. The first slice has exactly one clip model: integer physical-pixel rectangles. Primitives use the scissor; text uses glyphon's `TextBounds`, which `prepare` applies by trimming each text area's quads on the CPU. One function turns a layout clip into that integer rectangle (the rounding rule is defined once, at extraction) and feeds both, so primitive and glyph edges cannot disagree. Scissor changes end a batch, and that cost is accepted. Shader-side per-instance clipping is deferred until its quality and cost are measured; under T1 it could never cover text anyway, since glyphon clips on the CPU, so it only unifies batches if the glyph path is owned (T2). A paint command carries a clip index rather than a rectangle so that later change stays local.

Define one color/alpha convention and match it across primitive and glyph pipelines. glyphon's pipeline blends with `BlendState::ALPHA_BLENDING` (straight alpha), so a UI primitive pipeline that shares a frame with it matches that; premultiplied alpha is an option only if the glyph path is owned. Its atlas is created in `ColorMode::Accurate` (`TextAtlas::new`); `ColorMode::Web` reproduces browser blending and is documented as producing the results of most UI toolkits. Judge small light-on-dark text under both modes before fixing the choice.

The current glyphon wrapper prepares and renders a whole text collection (§2). Interleaving text with panels/images needs one of these:

| Option | What it is | Fits | Cost and risk |
| --- | --- | --- | --- |
| **T1** several `TextRenderer`s on one `TextAtlas` | Each glyphon renderer owns its vertex buffer and draws only what it prepared; atlas and viewport are shared (`TextRenderer::new` and `prepare` take the atlas in the 0.12 API; a prototype must confirm eviction behaves with several renderers per frame). Batches follow the partition rule below. | Few overlap levels (windows, popups): the common UI case. Keeps `D-026` intact and is reversible. | Under today's `trim` placement every batch re-prepares whenever any batch changes (§2), so T1 keeps today's frame-level skip and gains no per-window dirty tracking. Call the atlas `trim` once per frame after all batches. No per-glyph effects. |
| **T1b** T1 with deferred trim | `trim` runs only immediately before a frame that re-prepares every batch; such a frame is forced every K frames and on `PrepareError::AtlasFull`. In between, only changed batches re-prepare, and every retained batch's glyphs stay in `glyphs_in_use`, so nothing a retained batch samples can be evicted. | Per-window dirty batches without owning the glyph path. | Glyphs of replaced text stay pinned until the next full frame, so churn grows the atlas instead of recycling it; growth re-rasterizes the cache and the atlas never shrinks (§2). K trades prepare cost against atlas bytes. Still no per-glyph effects. |
| **T2** owned glyph renderer | cosmic-text shaping and swash rasterization with an owned atlas (`etagere`, glyphon's own packer, is already locked) and a WGSL instance pipeline. Text becomes ordinary paint-list instances. | Per-widget dirty batches, per-glyph effects (typewriter, wave), an MSDF slot, one blend and colour convention. | Re-implements atlas growth and eviction, subpixel bins and colour glyphs. Needs a decision entry amending `D-026`. The largest first-slice risk. |
| **T3** one collection drawn last | Today's wrapper. | The engine debug HUD only. | Breaks paint order for windows. Not for UI. |

Batch partition rule. Walk the ordered paint list once. A text batch is open while text commands arrive; keep the union rectangle `U` of the non-text commands seen since it opened. A later text command joins the open batch only if its rectangle misses `U`; otherwise the batch closes and a new one opens. The test is conservative, so it can over-split but never reorders paint, and it costs one pass with no pairwise overlap checks. Batches per frame are bounded by the alternations between overlapping layers (window, popup, tooltip), and the fixture reports that count against a stated cap.

Recommendation: build T1 first, behind a paint-list text command that names a text run rather than a glyphon type, so T2 can replace it without touching widgets. T1 must pass a prototype before the gate (§9): N overlapping windows with interleaved text and panels, batches prepared and drawn in arbitrary order, one `trim` at the end, and a check that no eviction changes an earlier batch's pixels. The same prototype runs T1b with an atlas-pressure case that forces a growth while batches are retained, and reports prepare time and atlas bytes for a few values of K. It is text plus one flat-colour quad in a scratch fixture, so the gate does not wait for M2's primitive pipeline. The §9 capability criteria decide when T2 is worth its cost. Repeatedly preparing one renderer inside the paint loop would overwrite state and undermine batching; do not promise the current wrapper is already sufficient.

Layer order is explicit: game HUD/screens, game popups/modals, then enabled developer views and their popups. Blocking follows input policy, not opacity: decorative HUD text is pointer-transparent; a modal intentionally blocks its lower layers; a debug window blocks only its own region unless explicitly modal. Define whether a game modal should still allow developer hotkeys. Each root also carries an opacity multiplier that a state can drive from `StateStack::transition_cover()`, because transitions draw before the overlay (§2).

Coordinates use logical UI units with top-left origin. Convert physical pointer coordinates into those units before hit testing, and convert layout/clips into physical pixels once at extraction. Track DPI changes separately from window size, which first needs the scale factor in a core resource (§2). OS scale and user UI scale multiply; a reference-resolution scale policy and pixel snapping can be opt-in. `display.scale_mode` (`Stretch` / `Integer`) is parsed but unapplied today (§2). Define what it means for the scene and the UI in one decision: a pixel-art game commonly integer-scales its scene and draws UI at native resolution, or integer-scales both, and the UI policy should not become a second, unrelated setting. The logical-versus-physical text layout fork in §2 (draw-time scale versus hinting) must be settled by prototype before this rule is fixed. World-attached labels can later project anchors into a screen root; actual world-space text is a separate rendering mode and is outside the first slice.

## 7. Input, events and the frame loop

An ordered input stream is needed in addition to the current held/edge snapshot. Preserve event order so a press and release received between redraws is still one valid click. Core receives engine-owned event types; the umbrella translates winit data.

Event inventory the bridge must add (§2 lists what it drops today), as an engine-owned core type:

```text
RawInput = Key { code, logical, text, state, repeat, mods }
         | PointerMoved | PointerButton | Wheel | PointerLeft
         | Modifiers | Focus(bool) | ScaleFactor(f64) | Resized
         | Ime(Preedit | Commit | Enabled | Disabled)      // with the editable-field milestone
```

Add real `KeyCode` variants for the keys UI needs (Home, End, Delete, PageUp, PageDown, and both sides of Shift, Control, Alt, Super) instead of widening `Other(<winit discriminant>)`. Window focus loss and pointer leave cancel hover, press and capture, and release held UI keys. Touch and pen are outside the first slice.

UI navigation reads engine-owned actions in the existing action map (`D-045`): for example `ui_accept`, `ui_cancel`, `ui_next`, `ui_prev`, `ui_up`, `ui_down`, `ui_left`, `ui_right`, with Enter, Space, Escape, Tab and arrows as defaults. Rebinding then works like every other control, and a later gamepad backend adds bindings without any widget change. Text-editing keys are not actions.

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

Event timing, checked against the loop in `app.rs`: `window_event` runs as events arrive, and every frame ends with a redraw already scheduled, either immediately or at the frame-cap deadline (`stage_pacing`, `about_to_wait`). Routing at the stage above therefore adds at most one frame, or one cap interval, of latency, and nothing is coalesced because the stream is ordered. Three rules make that explicit:

- Pointer and key events queue in arrival order, each with a sequence ID. Only adjacent pointer moves may coalesce, so a press and a release between redraws stay one click and a move before a press still positions it.
- Events that carry state rather than intent (`Resized`, `ScaleFactor`, `Focus`, `Modifiers`) update their core resources at event time, as `Resized` does today. Focus loss releases held keys then; the routing stage later cancels hover, press and capture.
- Routing is not done at event time. UI events publish into `EventQueue<UiEvent>` under `D-040`'s per-frame rotation and must precede systems deterministically. A future mode in which the loop idles without a redraw (power saving, occlusion) must wake the loop on input; today no such mode exists.

Font load and reload, like every other asset change, run only in the hot-reload and asset-sync stage before layout (§4).

Generate semantic events such as `Activated`, `CloseRequested` and later `ValueChanged` into `EventQueue<UiEvent>` **before systems**. Existing D-040 queues expose previous and current windows; a button action read with `iter()` every frame could execute twice. The convenience reader for UI actions reads `EventQueue::iter_current()`, which already exists, or tracks event sequence IDs. Preserve the existing global event rotation rather than inventing a second flush.

Keep raw physical input truthful for diagnostics and intentional global controls. Provide an explicit routed gameplay view/action-query path that filters consumed events, with held-key and release ownership to prevent stuck keys. A `wants_keyboard` boolean alone does not filter gameplay. Existing direct raw-input callers need migration before capture can be guaranteed for them.

Foundation behaviors: hover/pressed/disabled/focused states; pointer capture for window dragging; release-outside cancels a button activation; tab/shift-tab navigation; Enter/Space activation; focus scopes and restoration after a dialog; removal/hiding/focus loss cancels capture. Hover is recomputed from the last pointer position after every layout, scroll or root change, not only on pointer motion; otherwise a widget that moves under a still cursor keeps a stale hover. A drag starts past a small distance threshold, so a click on a title bar is not a zero-length drag. Double clicks are synthesized from timestamps and distance, since winit reports none. Resolve Escape as UI cancel/back before the mapped engine exit action. Keep engine toggles deliberate and remappable. Controller navigation needs a later input extension under D-045; this draft does not quietly assume a gamepad backend exists.

For future fields, physical keys are not text. Winit exposes [produced text and repeat information](https://docs.rs/winit/0.30.13/winit/event/struct.KeyEvent.html) and [IME preedit/commit events](https://docs.rs/winit/latest/winit/event/enum.Ime.html). The bridge will need those, modifier/shortcut handling, candidate-window caret positioning, clipboard and composition cancellation. Allow IME (`Window::set_ime_allowed`) only while an editable field has focus: winit 0.30 documents that while IME is allowed, keys typed during preedit arrive as `Ime` events instead of `KeyboardInput`, and recommends leaving it off for games, so an always-on IME would swallow game keys. Editing must respect graphemes, shaped clusters and bidirectional caret movement; byte slicing or one-glyph-per-character logic is insufficient.

## 8. Feature scope and likely omissions

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
- Animated opacity/position should invalidate paint or transforms appropriately. Layout animation is more expensive; avoid recomputing text just because a label moved or changed color. Engine tweens cannot reach UI nodes (§2); §14 proposes UI-owned tracks that reuse the easing types.
- Transform policy for the first slice: translation, uniform scale and opacity. Rotation applies to images only; clipping under rotation is unsupported. Text under animated scale needs a stated rule (re-rasterize, or scale cached quads and accept softness), which depends on the §9 outcome.
- Game-text effects: typewriter reveal, per-glyph colour, shake or wave, and in-shader outline or shadow. A single glyphon batch cannot do per-glyph effects cheaply, so decide before building dialogue widgets whether they are first-slice (§9 criteria).
- Rich text and inline icons: spans live in `StyledText`; icons can be glyphon `CustomGlyph`s (rasterized into the atlas, with a `scale`) or sprites in the paint list. Define baseline alignment for both.
- Localization: text arrives as string keys resolved at bind time. Layout values use logical `start`/`end` rather than `left`/`right`, so RTL mirroring needs no style rewrite. Test with a pseudo-locale that expands strings by 30–40%. String tables, locale-aware font fallback and a glyph-coverage test are in §14.
- UI audio stays with the game: focus, hover and activate sounds are systems that read `UiEvent` and send `AudioCommands`. The engine owns no UI sound set.
- Anchors and safe areas resolve against the viewport; define the ultrawide and letterbox rule together with the reference-resolution scale policy (§6).
- Pixel-art skins need an integer-scale option and nearest filtering for images and nine-slices, alongside smooth panels.

## 9. Glyph rendering evaluation: raster, MSDF or an owned path

This stays independent of the widget architecture, but not of the render path. MSDF is one rasterizer choice; the larger fork is who owns the glyph draw call (T1 or T2 in §6). glyphon cannot draw MSDF, so an MSDF experiment is also a prototype of T2, and the two are judged together.

MSDF stores distance information in three channels to preserve sharp corners when glyphs scale. [MSDFgen](https://github.com/Chlumsky/msdfgen#using-a-multi-channel-distance-field) describes median reconstruction and linear sampling. [MSDF atlas generator](https://github.com/Chlumsky/msdf-atlas-gen#atlas-types) documents MSDF/MTSDF, metadata and glyph-index export. MTSDF adds a true distance channel useful for some soft effects.

Proposed pipeline:

```text
Text + font + constraints
  → shared shaping/layout (cosmic-text)
  → positioned glyphs, metrics and clusters
  → glyphon raster path OR experimental MSDF path
  → the same ordered UI compositor
```

The `PositionedGlyph` list from §4 (face, glyph ID, position, size, colour) is the interface between the text engine and every rasterizer; define it before either path is built.

Score every path on capability as well as cost. Capabilities: per-glyph transforms and colour (typewriter, wave); in-shader outline, shadow and glow; animated size without re-rasterizing; crisp 8–14 px text under hinting; colour emoji and colour glyphs; variable fonts; one blend and colour convention with the primitive pipeline. Costs: atlas bytes, cold-frame latency, code and dependency footprint, hot-reload invalidation.

Choose using evidence. Scalable titles, animated sizes and outlined labels are promising MSDF cases; tiny text, pixel fonts, complex outlines and color emoji need quality tests and a fallback policy. There is no MSDF performance result for Tungsten yet; the current path's baselines are in §2 and §10. Atlas memory, generation time, upload costs and cold-frame latency may outweigh fewer scale-specific raster entries. Outlines/shadows could reduce today's repeated text sections, but that gain needs measurement too. Compare both paths at small HUD and large title sizes, fractional DPI/scale, rapid size changes, outlines, Latin ligatures, combining marks, RTL text, CJK and color emoji using fonts that actually cover each sample.

An experiment must settle:

1. Atlas acquisition: runtime generation/prewarming versus imported precomputed assets. The repository currently says **no asset preprocessing**; adopting an offline generator would require an explicit policy decision. Runtime generation must respect the two-background-thread limit and avoid expensive work in an interaction frame. No generator/tool/dependency is selected or installed here.
2. Coverage: map the shaper's **actual font face and glyph ID**, including ligatures/fallback, into the atlas. A Unicode-codepoint-only ASCII atlas cannot satisfy arbitrary shaped output. Pin source font identity/version and variable-font settings if applicable; retain fallback glyph rendering for misses.
3. Sampling: linear distance textures rather than the existing sprite sRGB upload assumptions, correct distance range/padding, coordinate/baseline conversion, scale-aware edge reconstruction and an explicit mip/minification policy. Implement any experiment in WGSL.
4. Lifecycle: bounded atlas pages/cache bytes, no eviction of in-flight glyph references, deterministic failure/fallback when full, and font reload invalidating shaped layouts and matching atlas content together.
5. Integration: current glyphon and experimental MSDF runs obey identical clips, paint ordering and alpha conventions. Preserve text measurement, caret and fallback behavior across both paths.

Provisional default: keep the current renderer while evaluating MSDF as an additional backend. Choose a default, per-font option or limited specialty path after comparing quality, frame tails, memory and complexity. Avoid a new broad renderer abstraction before the second path establishes a concrete need.

Gate: settle T1 versus T2 before the paint list (M2 in §11) is built, because the shape of its text command depends on it. T1 enters the gate only with its prototype result (§6), since sharing one atlas across batches is load-bearing for paint order. If the answer is "T1 now, revisit later", the run-reference command from §6 keeps that cheap to reverse.

## 10. Performance goals and evaluation

Design for bounded incremental CPU work, while recognizing that visible UI still draws into every presented game frame. Layout/shape/paint caching saves preparation; it does not eliminate GPU compositing. Start with dirty-root updates and profile before building fine-grained dependency machinery.

| Invariant / scenario | Evidence wanted after implementation |
| --- | --- |
| UI disabled or no roots | No extra UI GPU pass/target; near-zero routing/update cost; existing suite owned metrics remain within its established comparison policy. |
| Static menu/HUD | No reshaping or relayout after warm-up until an actual dependency changes; geometry/GPU resources reused; per-frame allocation count examined. |
| Hover/press/drag/color change | Touch the affected interaction/paint state; unchanged text layout stays cached. |
| Changing diagnostic values | Update only changed labels at the existing refresh rate; caches plateau under continuous unique text. |
| Static HUD plus one live label | Sections prepared and glyph vertices rebuilt per frame, against today's all-or-nothing prepare (§2). Baseline: the `gpu` row's `text_change` knob set to one section per frame. |
| Layout measurement | Shape count per layout pass: each text node shapes once per text, style or font-epoch change, and width probes only relayout. |
| Cold start | Font-database scan, first-glyph raster time and atlas growth (which re-rasterizes every cached glyph, §2), reported apart from frame metrics. |
| Large inspector/log/list | Measure visible content and input latency; eventual virtualization bounds work by visible rows. |
| World-anchored transient text | N concurrent short-lived labels with unique values (damage numbers): cache plateau, prepared glyphs per frame and p99 (§14). The `integrated` row's name tags, rewritten every frame, are the existing baseline. |
| Font/theme/DPI changes | Correct full invalidation when needed, with resize and cold-glyph p95/p99 spikes reported separately. |

Discuss initial targets of **≤0.25 ms additional CPU p95** for a warmed 100-widget menu and **≤0.5 ms** for a warmed 500-widget debug view on the reference machine at 1080p. These are proposed budgets, not measured outcomes or finalized acceptance limits. GPU budgets should account for filled screen area/transparency and be chosen after a simple prototype. Define widget counts precisely: include internal label/background nodes, not just top-level controls.

Track input, layout, shaping, paint/extract, GPU preparation/draw separately, plus draw batches, prepared sections and rebuilt vertices, shape counts, glyph misses/uploads, atlas/cache bytes and worst cold frames. Debug UI reads existing telemetry and publishes its own costs without creating a parallel timing system.

Future captures follow the canonical [profiling workflow](../perf/profiling-workflow.md#comparison-rule-and-capture-rules): matching workloads/builds/machine, quiet A/A, five repeats, and separate blocking GPU diagnostics. A new UI workload should live in a dedicated fixture/harness after design finalization; existing examples and benchmark workloads stay untouched beforehand. If a later benchmark migration changes work, bump its workload version and start a fresh baseline; do not present it as an engine-only speedup. The shape-run cache experiment that [`benchmarks.md`](../perf/benchmarks.md) leaves open targets the same shaping cost as M0a; run it against the split engine rather than the cache M0a replaces.

## 11. Migration outline and milestone ladder — after finalization

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

Suggested future sequence: finalize the architecture/API/acceptance criteria; build and validate an isolated text/button/panel fixture with both a gameplay screen and debug window; evaluate layout dependencies and MSDF in bounded experiments; validate engine integrations; migrate engine views; then migrate examples explicitly. No example rewrites before the finalized foundation passes its checks. As a ladder, each step becoming its own `phaseN-milestone-NN` plan once this draft is finalized:

| Step | Contents | Visible change | Done-when (sketch) |
| --- | --- | --- | --- |
| **M0a** text engine split | Device-free text engine and GPU half; retained buffers, `measure` with its three width modes, `FontEpoch`; alignment, wrap, ellipsis, hinting, letter spacing and font features reachable; font families (§14); font-fallback policy applied. | None | Text unit tests, including min-content per wrap mode; `just visual` unchanged; `just smoke`; the `gpu` row not regressed after an A/A of the untouched tree. |
| **M0b** input and display groundwork | Scale factor in a core resource with `ScaleFactorChanged` handled; window focus loss releases held keys; modifiers; pointer leave; real `KeyCode` variants for Home, End, Delete, PageUp, PageDown and both sides of each modifier; the ordered raw event stream recorded but not yet routed. Independent of M0a. | None, except that keys no longer stick after an alt-tab | Bridge and `InputState` unit tests (focus loss releases, event order kept, unknown keys still `Other`); existing examples' input unchanged; `just smoke`. |
| **M1** core UI model | `UiTree`, IDs, style model and theme tokens (§14), a Taffy spike behind Tungsten style types, hit testing, focus, roles and labels in node data, a fixed-advance `TextMeasure` test double, the headless `UiHarness` and layout snapshot tests (§14). | None; nothing draws | Headless layout, focus and invalidation tests; `just check`. Spike verdict recorded: Taffy or a hand-rolled stack/anchor layout. |
| **Gate** | T1, T1b or T2 (§9); the prototype from §6 with its eviction and growth checks; the DPI/hinting prototype result; the capability criteria §14 adds (typewriter reveal, bitmap fonts). | None | Recorded as decision entries. |
| **M2** overlay and paint list | UI overlay in the final pass; SDF primitive pipeline with shadow and gradient fields reserved; images and nine-slices; text batches; decorations; clip index; a hot-reloaded theme asset; a layout-bounds overlay and a hide-UI action; the fixture as a new example (§14). | Fixture only | `just smoke`; paint-list snapshot tests; direct and capture frames equal with UI visible, which needs P3's capture-completion contract, so schedule that fix before M2; an empty UI adds no pass or target; shader coverage test. |
| **M3** input routing | Routing of the M0b stream; `ui_*` actions with navigation repeat; `UiEvent` before systems; routed gameplay view; capture and focus rules; directional focus, focus visibility and hover cursors (§14). | None until a root exists | Event-order, release-outside, focus-loss, single-activation and directional-focus tests; raw `InputState` behavior for existing examples unchanged. |
| **M4** gameplay screen fixture | Pause menu and HUD with state-owned roots, transition opacity and keyboard navigation; UI animation tracks, binding-aware prompts and prewarm; a settings screen (display mode, UI and text scale, volume, one rebind) as the second scenario (§14). | Fixture | §10 budgets for a warmed 100-widget menu; no cold-glyph spike on the first open after a prewarm. |
| **M5** engine debug views | HUD, timing overlay and inspector as read-only views; scroll containers, which the inspector and log need; movable windows with persisted placement; world picking blocked behind windows; log console on an engine-owned logger; frame-time graph; picked-widget panel (§14). | Debug overlays | Perf suite telemetry rows not regressed; HUD toggles and defaults preserved. |
| **M6** examples | One example at a time, per the table above. | Examples | Per-example smoke; workload versions bumped where benchmark work changes. |
| Later | Virtualization, editable fields and IME, controller, accessibility, docking, data-defined screens, string tables and localization, world-anchored text, bitmap fonts, an editable inspector and a command console (§14). | | Their own plans. |

Decisions this work will likely need (IDs unassigned; each adds its `DECISION_INDEX.md` row in the same change):

- UI dependency admission under `D-015` (Taffy first; AccessKit and a clipboard crate later).
- The text engine split and the font-fallback policy (packaged fonts only, or system fallback).
- Font families in the manifest.
- The glyph path: T1, T1b or T2; T2 amends `D-026`.
- The DPI and hinting model, including the scale-factor resource, and what `display.scale_mode` means for scene and UI.
- The style model and theme asset: per-kind defaults, per-node overrides, inheritance limited to text properties.
- An engine-owned logger that applies `logging.level`, which the log console needs.
- UI input routing: the raw event stream, `ui_*` actions, the routed gameplay API and whether it becomes the default.
- Thread-rule scope if accessibility ships on Linux; the asset-preprocessing rule if MSDF atlases are generated offline.

Future meaningful tests include CPU layout/measurement/invalidation, with min-content per wrap mode; paint-list order and batch-partition snapshots; directional focus; event order and single activation per press/release; focus/modal/capture cleanup; state-root lifecycle; multilingual wrapping/fallback; DPI/scissor rounding; overlapping mixed primitives/text; direct/capture equality; cache saturation and continuous text churn. Renderer changes require the renderer's shader/layout, smoke and reference visual checks; substantial implementation finishes with `just check`. These are future checks, not tests added by this draft.

## 12. Risks and unknowns

| Risk | Why it matters | Early signal or mitigation |
| --- | --- | --- |
| T1 re-prepares every batch on any change | Under today's `trim` placement, eviction forbids retaining batches (§2), so many windows plus a large inspector could cost more than today's single pass, and a wrong retention shows as corrupted glyphs only when the atlas fills. | The prototype reports batch count and prepare time against a stated cap and runs an atlas-pressure case; T1b retains batches at the cost of atlas bytes; the gate (§9) chooses. |
| Measurement thrashes the text cache | Min-content, max-content and final width probes per label multiply shaping. | Retained buffer per node and the shape-count counter (§10), from M1. |
| Taffy fits wrapped text or hidden/collapsed semantics poorly | A solver mismatch surfaces late, after widgets depend on it. | M1 spike: a wrapped label in a row inside a column, hidden versus collapsed, min/max constraints. Fall back to a narrow hand-rolled layout. |
| Routed input breaks existing examples | Consumers read raw `InputState` directly. | Raw state stays truthful and unchanged; the routed view is additive; migrate per consumer (§11). |
| Fallback fonts differ per machine | Layout, visual tests and replays diverge without warning. | A packaged-fonts-only default and coverage tests per declared language. |
| Scope creep into editor, markup or docking | The non-goals are easy to erode while the foundation is interesting. | The milestone ladder, each step's done-when and the "Later" row. |
| Accessibility becomes a rewrite | Omitting roles, labels and stable IDs now makes platform support expensive later. | Keep them in node data from M1 even though nothing consumes them yet. |
| A UI move reads as an engine speedup or regression | Moving a benchmark's HUD to UI changes its workload, not engine speed. | Bump the workload version and start a fresh baseline (§10). |
| Style grows into a CSS cascade | Selector matching and inheritance rules grow without bound once screens depend on them. | Per-kind defaults plus per-node overrides, inheritance limited to text properties, no selectors (§14). |
| Packaged CJK fonts swell the repository | A Noto Sans CJK face is roughly 10–20 MB per weight and region, and the no-preprocessing rule forbids subsetting. | Declare CJK only where a game ships its font; system fallback stays an opt-in for debug text (§14). |
| The log console's logger becomes hidden global state | `log` installs one process-wide logger, and records arrive from any thread. | The console buffer lives in a resource and is shared with the logger by `Arc`; the logger's push never blocks; the setup gets a decision entry (§14). |

## 13. Choices for the next discussion

| Choice still open | Provisional recommendation |
| --- | --- |
| Public authoring model | Start with builders/handles/events; compare the two sketches using one pause menu and one live inspector before deciding. |
| Layout solver | Evaluate Taffy behind Tungsten-owned style types; keep the initial exposed layout subset small. |
| Visual direction | Small theme/token system with custom composition; clarify pixel-art skins versus smooth panels and when nine-slice skins are needed. |
| Initial scrolling | Scroll containers at M5: the inspector and the log console exceed the viewport. Virtualization waits for real row counts. |
| Root lifecycle/storage | A generational arena in a `World` resource, with explicit state-owned roots and persistent debug roots. |
| Input API compatibility | Add an explicit routed gameplay path and migrate consumers deliberately; agree whether a future breaking API release makes routed input the default. |
| MSDF atlas policy | Keep acquisition and coverage open; resolve the preprocessing/thread constraints before selecting a generator. |
| Performance acceptance | Agree workloads, widget counts and reference-machine CPU/GPU/cold-start/memory budgets before implementation. |
| Text/languages and future input | Declare packaged-font coverage; decide whether IME-complete single-line editing is the first editing milestone. |
| Font-fallback policy | Packaged fonts only by default, system fallback as an explicit opt-in. Changes today's behavior on machines that rely on system fonts. |
| DPI and text layout model | Prototype logical layout with a draw-time scale against physical layout with hinting (§2). Default to logical; allow physical for debug text if it reads better. |
| Glyph path | T1 behind a run-reference paint command, with T1b measured in the same prototype; revisit at the §9 gate using the capability criteria. |
| UI navigation input | Engine-owned `ui_*` actions in the action map; a gamepad backend stays a later, additive step. |
| Accessibility and the thread rule | Roles and labels in node data now; platform integration waits for a decision on the Linux adapter's executor thread. |
| Transitions and UI | A root opacity multiplier that states drive from `transition_cover()`. |
| Style model | A theme asset with per-widget-kind defaults and state variants, per-node overrides and inheritance of text properties only; hot-reloaded. |
| Font families | A manifest view grouping face IDs by weight and style, plus a packaged fallback chain; register the shipped variable fonts only when a weight axis is needed. |
| `display.scale_mode` | One definition for scene and UI scaling, decided with the DPI model. |
| UI animation | UI-owned tracks reusing `Easing`, `TweenRepeat` and `TweenDirection`, with completion as a `UiEvent`; paint-only properties first. |
| Typewriter reveal and bitmap fonts | Gate criteria (§9): T1 can only reshape per step or clip one line; T2 reveals by glyph count and can draw sprite-sheet glyphs. |
| Strings and localization | Per-locale string-table assets with `{name}` arguments, hand-rolled; Fluent only if plural rules outgrow it. |
| Logger ownership | An engine-owned logger that applies `logging.level` and feeds a bounded console buffer; examples drop `env_logger::init()` as they migrate. |
| UI fixture location | A new example (`examples/05_ui_lab`, name provisional), so smoke covers it and existing examples stay untouched. |

The next useful artifact is a tightened spec for the first text/button/panel/window slice, with one chosen API and an explicit input contract. Leave broader widget lists and MSDF selection open until those smaller decisions are clear. The smallest decision set that unblocks M0a (§11) is the font-fallback policy and the text engine's API surface (§4), including its measurement modes and font families. M0b needs only agreement that the scale factor and focus handling land before any UI; the rest can follow.

## 14. Systems not yet covered

Added by the review pass of 2026-10-02. Each item states the gap, a proposal and where it lands on the ladder (§11). "Game-owned" means games build it from engine primitives; the engine supplies the hook, not the widget. None of it is decided.

**Style and theme**

- **Style model.** §8 asks for theme tokens, but nothing says how a node gets its style. Proposal: a theme holds tokens (colours, spacing, radii, font families) and per-widget-kind defaults with state variants (normal, hovered, pressed, focused, disabled). A node may override any property. Only text properties (family, size, colour, alignment) inherit down the tree, so a label inside a button takes the button's text style. No selectors and no cascade beyond that. Resolved styles are cached per node behind a theme epoch, as text is behind `FontEpoch`. (M1 data; M2 paint)
- **Theme as an asset.** A manifest section (`ui_themes`, ID required) reloaded through the existing watcher (`D-031`, `D-053`), so a running game restyles live: the cheapest high-value loop this suite can offer. Colours are authored in sRGB and converted once at load to the pipelines' convention (§6). High-contrast and colourblind-safe themes are then data. (M2)
- **Skins.** Nine-slice insets as an optional field on the sprite entry, like `normal_map`, so skins reuse sprite IDs and atlas pages. Pixel-art skins pair with integer UI scale (§6). (M2)

**Behaviour and composition**

- **Custom widgets.** §8's "custom composition" has no mechanism yet. Proposal: built-in behaviours are a closed enum (`Button`; later `Toggle`, `Slider`), following `D-054`. Any node can be marked interactive, and it then emits low-level `UiEvent`s (pointer enter, leave, press, release and click; focus gained and lost) that game systems read. An inventory slot is an image node, the interactive flag and a system: no `dyn Widget` and no callbacks. (M1, M3)
- **Custom paint.** Graphs, radial meters and minimap frames need shapes the SDF rectangle cannot draw. Reserve a triangle-mesh paint command, first used by the frame-time graph, and later a material-backed quad on `D-058` materials. (Mesh at M5; materials later)
- **Popups.** A popup, tooltip or dropdown must escape its owner's clip and paint above sibling windows, so it is a separate root on the popup layer, owned by a widget and removed with it. Placement is a small solver: preferred side, flip when it does not fit, clamp to the safe area. Tooltips need show and hide delays and a keyboard-focus rule. (Broader controls)

**Focus and navigation**

- **Directional focus.** The `ui_up`/`ui_down`/`ui_left`/`ui_right` actions (§7) need a resolution rule: explicit neighbours first; otherwise the nearest focusable node in the current focus scope whose centre lies in that direction, scored by distance along the axis plus a weighted perpendicular offset. Wrap-around is a per-scope flag. (M3)
- **Navigation repeat.** A held direction repeats after an initial delay at a fixed rate. The engine owns the timer, so the keyboard and a future gamepad stick repeat alike; OS key repeat does not drive actions. (M3)
- **Focus visibility.** The focus ring shows after keyboard or gamepad navigation and hides after pointer use (CSS `:focus-visible`), driven by a last-used-device resource that prompts also read. (M3)
- **Scroll into view.** Navigating into a clipped, scrolled region scrolls the focused widget into view; otherwise keyboard navigation in the inspector moves focus off-screen. (M5, with scroll containers)

**Input presentation**

- **Binding-aware prompts.** Text such as `Press {action:ui_accept}` resolves through `ActionMap::bindings` to the bound key's display name and resolves again when `input.json` reloads (`D-045`). Names follow the physical `KeyCode`, so an AZERTY player sees US labels; winit 0.30 has no keymap query, so that is a stated limit. Inline icons (§8) later replace names with glyphs. (M4)
- **Rebinding.** A rebind screen needs a capture-next-input mode on the raw stream that bypasses UI navigation; `ActionMap::replace_bindings_and_persist` already exists. The capture mode is engine-owned, the screen game-owned. (M4 scenario)
- **Cursors.** Nothing controls the cursor today (§2). A hovered widget requests a system cursor (pointer, text beam, resize edges) through the umbrella. A game cursor is either a winit 0.30 custom cursor or a sprite drawn last in the overlay, one frame behind. Cursor visibility and grab for gameplay go through the same bridge. (M3 for system cursors; game cursors later)

**Text for games**

- **Tabular figures.** Counters and timers change width as their digits change, so rows jitter; the HUD sidesteps this with a monospace font and `MONO_ADVANCE_RATIO`. Per-span `font_features` can request `tnum`, which Inter, the shipped sans, provides. (M0a API; M2 use)
- **Allocation-free values.** Without change detection (§2), a live label formats with `write!` into a retained buffer and compares before `set_text`, so an unchanged value costs a format and a compare, never an allocation or a re-prepare. (M4)
- **Typewriter reveal.** Dialogue reveals the glyphs of a layout that must not reflow as it grows. Under T1 the choices are rebuilding the spans each step, which reshapes, or one clipped text area per line, which only works left to right; T2 reveals by glyph count. Record it as a gate criterion (§9) before any dialogue widget. (Gate)
- **Inline markup.** Translated strings carry their own emphasis, colours, icons and prompts, so spans cannot be built only in code. A closed tag set (bold, italic, colour token, icon ID, action prompt) parses into `StyledText`; an unknown tag shows literally and logs once. It is a text format, not the screen markup §5 excludes. Span `metadata` (§2) carries the tag for hit tests, so a glossary term can open a tooltip. (With localization)
- **Font families.** Weight changes need the family view from §2. Proposal: `font_families` in the manifest, naming face IDs per weight and style plus a fallback chain of packaged families, validated at load as animations validate their sprite IDs. With an explicit database, the chain is a cosmic-text `Fallback` implementation, because the default lists name system families such as `Noto Sans Hebrew`. (M0a)
- **Bitmap fonts.** Pixel-art games often want sprite-sheet fonts. Their glyphs map to sprite regions with fixed or listed advances, so they need the owned glyph path (T2) or image paint commands; the metrics file is plain data, and loading it is not preprocessing. (Later; a capability note for the gate)
- **Prewarm.** The first open of a pause menu shapes and rasterizes its glyphs in an interaction frame. Prewarm builds a root hidden during a loading state, or rasterizes a declared character set (digits for a HUD font), and reports the cost under cold start (§10). (M4)

**Localization**

- **String tables.** A `strings` manifest section per locale (keyed IDs, `{name}` arguments), hot-reloaded, with a fallback locale and a missing-key marker visible in debug builds. Plural rules are where a hand-rolled format stops: adopt Fluent (§3) only if shipped locales need more than one/other. (Later)
- **Locale-aware fonts.** Han unification maps the Japanese, Chinese and Korean forms of a character to one codepoint, and each region expects its own glyph shapes; cosmic-text resolves Han fallback by locale (`script_fallback(Script::Han, locale)`). The active locale is therefore an input to font selection, and switching locale bumps `FontEpoch`. (With the fallback policy)
- **Coverage test.** A layer-1 test that every string of each declared locale shapes without missing glyphs in that locale's packaged faces, in the manner of `manifests.rs`. It turns "declare supported languages" (§8) into a check. (Later)
- **Size cost.** Packaged-only fallback (§13) means shipping the faces, and CJK faces are large (§12). Declare CJK only where a game ships its font. (Fallback decision)

**Animation**

- **UI tracks.** Engine tweens write entity components only (§2). The UI tree owns animation tracks on node properties (opacity, translation, scale, colour), reusing `Easing`, `TweenRepeat` and `TweenDirection` and ticked in the UI stage, and publishes completion as a `UiEvent` with a tag, as tweens carry `on_complete_tag`. Paint-only properties never relayout; size animation is allowed but counted as layout work. (M4)
- **State transitions.** Changes between interaction states may ease over a theme duration on the same tracks. (M4)
- **Reduced motion.** One setting that zeroes UI durations and scales camera-shake trauma (`D-073`). (Later)

**Debug tooling on the foundation**

- **Log console.** The engine never owns the logger (§2). Proposal: an engine logger setup that applies `logging.level` and tees records into a bounded buffer that a console resource also holds by `Arc`. The logger runs on whichever thread logs, so its push never blocks: `try_send` on a bounded `std::sync::mpsc` channel, drained on the main thread, counting drops. It needs scroll containers and, at volume, virtualization. Examples drop `env_logger::init()` as they migrate. (M5)
- **Frame-time graph.** A sparkline of `FrameTimings::interval_ms` beside the HUD numbers, on the mesh paint command. (M5)
- **UI self-inspection.** A layout-bounds overlay, a picked-widget panel (rectangle, resolved style, dirty cause, paint batch) and an event log. Cheap once the paint list exists, and the main tool for debugging the suite itself. (Overlay at M2; panel at M5)
- **Window persistence.** Debug window position, size and open state saved to a workspace-local file, following `ActionMap::persist`. (M5)
- **Hide UI.** One engine action that hides every game root, for clean screenshots and scene-only visual checks. (M2)
- **Editable inspector.** `Inspectable::inspect_rows` returns `Vec<(&'static str, String)>`: read-only, and allocating on every call. Live tuning needs typed rows (number with range, flag, colour) and a write path back to the component, which is its own decision. (After M5)
- **Command console.** Registered debug commands with typed arguments, opened by an engine action. Needs editable fields. (After fields)

**World-anchored and transient text**

Damage numbers, name tags and interaction prompts are numerous, short-lived and placed by the camera; they do not belong in the retained tree. Proposal: a world-label path of POD entries projected at extract, as example 02's name tags are today, drawn in the overlay's lowest layer with culling and a per-frame cap, through the content-keyed bounded cache (`D-085`). Per-glyph pop and scale effects make it a T2 consumer. Its perf scenario is in §10. (Later; scenario from M2)

**Accessibility settings**

The screen-reader bridge waits (§3), but these settings must stay possible: a text scale separate from UI scale, a minimum text size, theme variants and reduced motion (above), hold-to-confirm for destructive actions, and captions. Captions are game-owned timed text tied to `AudioCommands` playback, built on the engine's text box, safe-area anchoring and text scale. (Later; M4 exercises text scale)

**Testing**

- **Harness.** `UiHarness` is a headless tree with the fixed-advance `TextMeasure`, scripted raw input and a step function, querying widgets by ID or label. Layout and paint-list snapshots are plain-text dumps (rectangles, command kinds, batch boundaries) compared in ordinary unit tests, with no snapshot dependency, so paint order and the batch partition (§6) become GPU-free tests. (M1; paint lists from M2)
- **Fixture.** The isolated fixture (§11) is a new example (`examples/05_ui_lab`, name provisional), so `just smoke` covers it and the existing examples stay untouched. It holds the pause menu, the HUD, the settings screen and a debug window. (M2)
